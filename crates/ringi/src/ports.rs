//! ringi as `ritsu check` joins it (ritsu's DESIGN 3.2, 6.4, 8.3): each `.ringi` of a project,
//! checked with the languages it reads and is lowered to joined ([`Suite`]), and what the check
//! says, as `ringi check` prints it. A file of ringi's that others read (an organization, masters,
//! a shared route) is checked once in a run, however many files read it.
//!
//! ringi answers three ports: `Requests` (the files a request is lowered to, each line with the
//! line of the request it comes from; its form; the days it can be used on; what its ticket does),
//! `References` (the files its `use` lines read, by which `ritsu check` tells a line that waits for
//! a file that does not pass) and `Items` (what a file holds, by kind and name, as yuen names it;
//! [`crate::items`]).

use crate::ast::{Field, FileKind, TypeExpr, UseKind};
use crate::check::Run;
use crate::suite::Suite;
use ritsu_base::naming::{Name, Tool};
use ritsu_base::text::Lang;
use ritsu_ports::{LoweredFile, Read, Reference, RequestField, RequestForm, RequestUsable, Requests, Said};
use std::path::Path;

/// One thing kaname says of the code ringi lays for a request (DESIGN 9.2), as `ritsu gen` hands it
/// to [`Engine::glue_refused`]: kaname's code, or for a function that is not total its effect as
/// kaname writes it (`fails range`); the line it is at; and whether that is in the machine of the
/// tickets rather than in the request's glue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlueSaid {
    pub code: String,
    pub line: Option<usize>,
    pub machine: bool,
    /// Whether it is a function that is not total, rather than a diagnostic.
    pub not_total: bool,
}

/// ringi, as `ritsu check` reaches it, with the languages a `.ringi` reads and is lowered to joined
/// (`ritsu ringi` and `ritsu check` hand it every one; the default joins none, and a file that
/// needs another language then says so, E008).
pub struct Engine {
    run: Run,
    /// The server's API this run writes (`ritsu gen --target requests`), as the system opens it:
    /// the run makes it what ringi writes now (ritsu's X20, DESIGN 9.3).
    writes_api: std::cell::RefCell<Option<std::path::PathBuf>>,
}

impl Default for Engine {
    fn default() -> Engine {
        Engine::new(Suite::default())
    }
}

impl Engine {
    /// ringi with the languages `suite` joins, reached through `ritsu` (`ritsu check`, `ritsu gen`):
    /// the commands the checks name are named `ritsu ringi …` (ritsu's DESIGN 6.2, item 10).
    pub fn new(suite: Suite) -> Engine {
        let run = Run::new(suite);
        run.set_through("ritsu");
        Engine { run, writes_api: std::cell::RefCell::new(None) }
    }

    /// The server's API `ritsu gen` writes in this run (`<out>/requests/openapi.json`), said before
    /// any file is checked: a flow that reads it is not held to the document as it is before the
    /// run, which the run writes again (ritsu's X20, DESIGN 9.3).
    pub fn set_writes_api(&self, path: &Path) {
        *self.writes_api.borrow_mut() = Some(ritsu_base::paths::opened(path));
    }

    /// The server's API this run writes, as the system opens it; None outside `ritsu gen
    /// --target requests`.
    pub fn writes_api(&self) -> Option<std::path::PathBuf> {
        self.writes_api.borrow().clone()
    }

    /// [`Engine::checked`], with what the run reads besides the files (`ritsu check --through`).
    /// The engine's own run, when it was told them ([`Engine::set_options`]): each file is checked
    /// once in the run, and the checks across the languages read that check, so a request that
    /// cannot be used through the day does not pass for them either. Else a run of its own, so that
    /// the checks the ports keep are the ones with what the engine was told.
    pub fn checked_with(&self, root: &Path, files: &[String], opts: &crate::check::Options, lang: Lang) -> Vec<ritsu_ports::Checked> {
        if *opts == self.run.opts() {
            return self.checked(root, files, lang);
        }
        let run = Run::with(self.run.suite.clone(), opts.clone());
        if let Some(outer) = self.run.reached_through() {
            run.set_through(outer);
        }
        run.set_root(root);
        run.set_lang(lang);
        files
            .iter()
            .map(|f| {
                let o = run.file(f);
                crate::check::unit(root, f, o.as_deref(), lang)
            })
            .collect()
    }

    /// `ringi check` of each file, as `ritsu check` prints it (ritsu's DESIGN 8.3), with the
    /// languages the engine is handed: what [`crate::check::unit`] gives, each check kept for the
    /// run. `files` are as the person gave them, from where the program runs; `root` is the
    /// project's, which the paths in the JSON are written from.
    pub fn checked(&self, root: &Path, files: &[String], lang: Lang) -> Vec<ritsu_ports::Checked> {
        self.run.set_root(root);
        files
            .iter()
            .map(|f| {
                let o = self.run.file(f);
                crate::check::unit(root, f, o.as_deref(), lang)
            })
            .collect()
    }

    /// What the engine's run reads besides the files from now on (`ritsu check --through`), said
    /// before any file is checked: every question of the engine is answered from the one check of
    /// each file with them (ritsu's DESIGN 4.15).
    pub fn set_options(&self, opts: &crate::check::Options) {
        self.run.set_options(opts);
    }

    /// The root of the project the engine answers for (`ritsu check`, `ritsu gen`), said before any
    /// file is checked: a gate a request reads is asked of sekisho from it, as the project's own
    /// check of the gate asks it, so that the gate is checked once in the run (ritsu's DESIGN 4.15).
    pub fn set_root(&self, root: &Path) {
        self.run.set_root(root);
    }

    /// The language of the run the engine answers for (`--lang`), said before any file is checked:
    /// the files a request is lowered to are asked of rulec and koyomi in it first, as the run asks
    /// of them later, so that each is checked once in the run (ritsu's DESIGN 4.15).
    pub fn set_lang(&self, lang: Lang) {
        self.run.set_lang(lang);
    }

    /// The glue of the request at `file` (DESIGN 12.1, 12.2): the module of kaname's that joins it to
    /// the machine of its tickets, reading the machine's module where it is laid (`ticket`, as the
    /// program reaches it). Err with why it is not written: the file does not pass its check, it is
    /// no request, or it has what the glue does not lower yet.
    pub fn glue(&self, file: &Path, ticket: &Path) -> Result<crate::lower::glue::Glue, ritsu_base::text::Text> {
        let o = self.passing(file).map_err(|said| {
            let first = said.first().map(|s| s.message.clone()).unwrap_or_default();
            tr!("検査を通りません: {}", "it does not pass its check: {}", first.ja; first.en)
        })?;
        crate::engine::with(&o, &self.run, |m, _| crate::lower::glue::glue(m, ticket))?
    }

    /// What `ritsu gen` says of the request at `file` (as the run reads it; `root` is the
    /// project's) when kaname says anything of the code ringi lays for it (DESIGN 9.2, 12.3): its
    /// glue, or the machine of the tickets its code runs. ringi writes both, so it is ringi's bug:
    /// E201 at the request's first line, with kaname's code and the line of each thing it says
    /// (`said`) and none of kaname's words. As [`Engine::checked`] gives a unit, one that fails.
    pub fn glue_refused(&self, root: &Path, file: &str, said: &[GlueSaid], lang: Lang) -> ritsu_ports::Checked {
        let src = ritsu_base::fs::read_to_string(file).unwrap_or_default();
        let at = self.run.file(file).ok().and_then(|o| o.file.as_ref().map(|f| f.name.span)).unwrap_or(crate::ast::Span { line: 1, col: 1, end: 1 });
        let o = crate::check::Outcome::failed(file, vec![crate::mapped::glue_bug(file, &src, at, said)]);
        crate::check::unit(root, file, Ok(&o), lang)
    }

    /// What `ritsu gen --target requests` writes once into `<out>/requests/` (DESIGN 12.2) for the
    /// requests at `files` (as the run reads them), their paths written from `root`: each file's
    /// path under that directory, its text, and the request it is written from. Err with why a
    /// request is not described: it does not pass its check, or is no request with a machine.
    pub fn requests(&self, files: &[std::path::PathBuf], root: &Path) -> Result<Vec<(String, String, Option<String>)>, ritsu_base::text::Text> {
        self.requests_with(files, root, None)
    }

    /// [`Engine::requests`], with the screens of the kinds (`a2ui/`, DESIGN 13) after the rest when
    /// `screens` gives what they are made with.
    pub fn requests_with(&self, files: &[std::path::PathBuf], root: &Path, screens: Option<&crate::screens::Given>) -> Result<Vec<(String, String, Option<String>)>, ritsu_base::text::Text> {
        let kinds = self.kinds(files, root)?;
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        let mut out = crate::r#gen::files(&kinds);
        if let Some(given) = screens {
            out.extend(crate::screens::files(&kinds, given)?);
        }
        Ok(out)
    }

    /// What is said of the screens of the request at `file` before they are written (DESIGN 13.3,
    /// 4.8): E704 of the ranges of its numbers, E703 of its labels, E701 and W702 of what its
    /// screens send, as the outcome of a check that does not pass. None when nothing is said, and
    /// for a file that is no kind of request.
    pub fn screens_checked(&self, root: &Path, file: &str, given: &crate::screens::Given) -> Option<crate::check::Outcome> {
        let kind = self.kinds(&[std::path::PathBuf::from(file)], root).ok()?.into_iter().next()?;
        let said = crate::screens::said(&kind, given);
        if said.is_empty() {
            return None;
        }
        let src = ritsu_base::fs::read_to_string(file).unwrap_or_default();
        let diags = said
            .into_iter()
            .map(|s| {
                let d = if s.code.starts_with('W') { crate::diag::Diag::warning(s.code, file, s.line, 1, s.message) } else { crate::diag::Diag::error(s.code, file, s.line, 1, s.message) };
                s.notes.into_iter().fold(d.source(&src), |d, n| d.note(n))
            })
            .collect();
        Some(crate::check::Outcome::failed(file, diags))
    }

    /// [`Engine::screens_checked`], as [`Engine::checked`] gives a unit.
    pub fn screens_refused(&self, root: &Path, file: &str, given: &crate::screens::Given, lang: Lang) -> Option<ritsu_ports::Checked> {
        let o = self.screens_checked(root, file, given)?;
        Some(crate::check::unit(root, file, Ok(&o), lang))
    }

    /// The kinds of request at `files`, as `--target requests` describes them: what its files and
    /// the screens are made from. Err as [`Engine::requests`].
    pub fn kinds(&self, files: &[std::path::PathBuf], root: &Path) -> Result<Vec<crate::r#gen::Kind>, ritsu_base::text::Text> {
        self.kinds_made(files, root, true)
    }

    /// [`Engine::kinds`], with no vectors when `vectors` is false ([`crate::r#gen::Kind::made`]).
    fn kinds_made(&self, files: &[std::path::PathBuf], root: &Path, vectors: bool) -> Result<Vec<crate::r#gen::Kind>, ritsu_base::text::Text> {
        let mut kinds = Vec::new();
        for file in files {
            let o = self.passing(file).map_err(|said| {
                let first = said.first().map(|s| s.message.clone()).unwrap_or_default();
                tr!("検査を通りません: {}", "it does not pass its check: {}", first.ja; first.en)
            })?;
            let kind = crate::engine::with(&o, &self.run, |m, org| {
                // the organization's file beside its data: its positions and kinds of team
                let of = crate::r#gen::org_path(m.s).and_then(|p| {
                    let read = self.run.file(&ritsu_base::paths::show(&p)).ok()?;
                    read.file.clone().map(|f| (f, p))
                });
                crate::r#gen::Kind::made(m, org, &o, root, of.as_ref().map(|(f, p)| (&**f, p.clone())), vectors)
            })?;
            let mut kind = kind;
            self.declare_flows(&mut kind);
            kinds.push(kind);
        }
        Ok(kinds)
    }

    /// What dandori declares of each flow the kind uses (its workflow's name, version and type on
    /// Temporal, its outputs), when the run reads flows and the flow passes dandori's check.
    fn declare_flows(&self, kind: &mut crate::r#gen::Kind) {
        // the API is made with no flow read ([`Engine::set_unread`]), and holds nothing they declare
        if self.run.unread() {
            return;
        }
        let s = &self.run.suite;
        let (Some(flows), Some(rules), Some(dates), Some(books)) = (&s.flows, &s.rules, &s.dates, &s.books) else { return };
        let ports = ritsu_ports::Ports { rules: rules.clone(), dates: dates.clone(), books: books.clone() };
        for f in &mut kind.flows {
            if let Ok(ritsu_ports::Found::Value(d)) = flows.declared(&f.path, &ports) {
                f.declared = Some(crate::r#gen::FlowFacts { workflow: d.name, version: d.version, workflow_type: d.workflow_type, outputs: d.outputs.into_iter().map(|o| (o.name, o.ty, o.optional)).collect() });
            }
        }
    }

    /// From now on, what the approvers and the requesters of each version of the organization come
    /// to is kept apart from what a request is checked to (DESIGN 12.3): `ritsu gen`'s run, whose
    /// code takes the organization at each event. Said before any file is checked.
    pub fn keep_listing_apart(&self) {
        self.run.keep_listing_apart();
    }

    /// From now on until it is said again, every flow and every gate a file reads is taken to pass,
    /// unread (`Run::set_unread`): `ritsu gen --target requests` makes the server's API so, before a
    /// flow or a gate that reads it is checked, and what the run keeps of each file is the check
    /// that reads them where they pass.
    pub fn set_unread(&self, on: bool) {
        self.run.set_unread(on);
    }

    /// The errors the lists of each version of the organization say of the request at `file`, in a
    /// run that keeps them apart: each code, with how many, in the order of the codes.
    pub fn listing_errors(&self, file: &Path) -> Vec<(String, usize)> {
        let Ok(o) = self.run.file(&ritsu_base::paths::show(file)) else { return Vec::new() };
        let mut out: Vec<(String, usize)> = Vec::new();
        for d in o.listing.iter().filter(|d| d.is_error()) {
            match out.iter_mut().find(|(c, _)| *c == d.code) {
                Some((_, n)) => *n += 1,
                None => out.push((d.code.to_string(), 1)),
            }
        }
        out.sort();
        out
    }

    /// The check of the file, kept for the run; what its errors say when it does not pass.
    fn passing(&self, file: &Path) -> Result<std::rc::Rc<crate::check::Outcome>, Vec<Said>> {
        let shown = ritsu_base::paths::show(file);
        let o = self.run.file(&shown).map_err(|e| vec![Said::unreadable(&shown, &e)])?;
        if o.has_errors() {
            return Err(o.diags.iter().filter(|d| d.is_error()).map(Said::of).collect());
        }
        Ok(o)
    }
}

impl Requests for Engine {
    /// The files the request (or the shared route) is lowered to, as the check lays them; none for
    /// an organization and masters, which lower to nothing.
    fn lowered(&self, file: &Path) -> Result<Vec<LoweredFile>, Vec<Said>> {
        let o = self.passing(file)?;
        Ok(o.lowered.as_ref().map(|l| l.handed()).unwrap_or_default())
    }

    /// The request's form: each field with its type as written, its range on the wire, and the
    /// fields of each row.
    fn form(&self, file: &Path) -> Result<RequestForm, Vec<Said>> {
        let o = self.passing(file)?;
        let Some(f) = &o.file else { return Err(Vec::new()) };
        if f.kind != FileKind::Request {
            let shown = ritsu_base::paths::show(file);
            return Err(vec![Said { code: String::new(), file: shown, line: None, message: tr!("申請の種類のファイルではありません", "it is not a request's file") }]);
        }
        Ok(RequestForm { request: f.name.ascii().to_string(), fields: f.form.iter().map(field).collect() })
    }

    /// The days the request (or the shared route) can be used on; nothing ends for an organization
    /// and masters.
    fn usable(&self, file: &Path) -> Result<RequestUsable, Vec<Said>> {
        let o = self.passing(file)?;
        Ok(o.usable.as_ref().map(|u| u.handed()).unwrap_or_default())
    }

    /// What ringi knows of the request's ticket ([`crate::ticket`]), as its check found it;
    /// undecided for a file that is no request (a shared route, an organization, masters).
    fn ticket(&self, file: &Path) -> Result<ritsu_ports::Found<ritsu_ports::RequestTicket>, Vec<Said>> {
        let o = self.passing(file)?;
        Ok(match &o.ticket {
            Some(t) => ritsu_ports::Found::Value(t.clone()),
            None => ritsu_ports::Found::Undecided(tr!("申請の種類のファイルではありません", "it is not a request's file")),
        })
    }

    /// The `openapi.json` `--target requests` would write of the requests now ([`Engine::kinds`]):
    /// what ringi's check says of one that does not pass; undecided where the kinds cannot be
    /// described (a file that is no request, an organization with no data to run the scenarios
    /// on).
    fn openapi(&self, files: &[std::path::PathBuf], root: &Path) -> Result<ritsu_ports::Found<String>, Vec<Said>> {
        for f in files {
            let o = self.passing(f)?;
            if !matches!(o.file.as_deref().map(|x| &x.kind), Some(FileKind::Request)) {
                return Ok(ritsu_ports::Found::Undecided(tr!("申請の種類のファイルではありません", "it is not a request's file")));
            }
        }
        Ok(match self.kinds_made(files, root, false) {
            Ok(kinds) => ritsu_ports::Found::Value(crate::r#gen::openapi_json(&kinds)),
            Err(t) => ritsu_ports::Found::Undecided(t),
        })
    }
}

/// A field as the port carries it.
pub(crate) fn field(f: &Field) -> RequestField {
    let (lo, hi) = match &f.ty {
        TypeExpr::Num(u) => {
            let (lo, hi) = crate::lower::words::ends(f);
            let step = crate::lower::words::step(u);
            let wire = |r: ritsu_units::Rat| {
                let q = r.div(step);
                q.is_int().then_some(q.num)
            };
            (lo.and_then(wire), hi.and_then(wire))
        }
        TypeExpr::Date => {
            let (lo, hi) = crate::lower::words::ends(f);
            (lo.filter(|r| r.is_int()).map(|r| r.num), hi.filter(|r| r.is_int()).map(|r| r.num))
        }
        TypeExpr::Text | TypeExpr::LongText | TypeExpr::Rows => {
            // a count's ends, an end not in moved in by one (`count <100` takes 99 at most)
            let b = f.count.as_ref();
            let end = |v: Option<&crate::ast::Bound>, up: bool| {
                let v = v?;
                let r = crate::lower::words::constant(&v.value)?.rat();
                let n = r.is_int().then_some(r.num)?;
                Some(if v.inclusive { n } else if up { n + 1 } else { n - 1 })
            };
            (end(b.and_then(|b| b.lo.as_ref()), true), end(b.and_then(|b| b.hi.as_ref()), false))
        }
        _ => (None, None),
    };
    RequestField {
        name: f.name.text.clone(),
        code_name: f.name.ascii().to_string(),
        label: f.title.clone().unwrap_or_default(),
        ty: f.ty.word(),
        lo,
        hi,
        values: f.options.iter().map(|o| o.name.ascii().to_string()).collect(),
        optional: f.optional.is_some(),
        computed: f.computed.is_some(),
        approvers_only: f.approvers_only.is_some(),
        line: f.line,
        fields: f.children.iter().map(field).collect(),
    }
}

/// The file read and parsed, for what it holds and names; else what the parser says (or why it
/// cannot be read), naming the file the way the run writes its paths (ritsu's DESIGN 4.15).
pub(crate) fn parsed(root: &Path, file: &str) -> Result<(crate::ast::File, String), Vec<Said>> {
    let disk = ritsu_base::paths::on_disk(root, file);
    let path = ritsu_base::paths::show(&disk);
    let src = ritsu_base::fs::read_to_string(&disk).map_err(|e| vec![Said::unreadable(&path, &e)])?;
    let p = crate::parse::parse(&path, &src);
    match p.file {
        Some(f) => Ok((f, src)),
        None => Err(p.diags.iter().filter(|d| d.is_error()).map(Said::of).collect()),
    }
}

impl ritsu_ports::Items for Engine {
    /// What the file holds, by kind and name ([`crate::items::things`]): a file that parses says
    /// it, whether or not it passes its check.
    fn items(&self, root: &Path, file: &str) -> Result<Vec<ritsu_ports::Item>, Vec<Said>> {
        let (f, src) = parsed(root, file)?;
        Ok(crate::items::things(&f, &src).into_iter().map(|t| ritsu_ports::Item { naming: Name::file(Tool::Ringi, file).with(t.kind, t.name), lines: t.lines, text: t.text }).collect())
    }
}

impl ritsu_ports::References for Engine {
    /// The file each `use` line reads ([`uses_of`]).
    fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>> {
        uses_of(root, file)
    }

    /// What the file names but for `fed by` ([`Read::of_named`]), plus what each `data` line reads
    /// without naming it (`references` is left alone, so that nothing the index counts of a naming
    /// changes): an organization's folder of versions, and a master's table. A line whose path
    /// cannot be written from the root reads nothing.
    fn reads(&self, root: &Path, file: &str, named: Vec<Reference>) -> Vec<Read> {
        let mut out = Read::of_named(named);
        let Ok((f, _)) = parsed(root, file) else { return out };
        let mut data = |written: &str, line: usize| {
            if let Ok(target) = ritsu_base::modules::join_opened(root, file, written) {
                out.push(Read { line: Some(line), target: Name::file(Tool::File, target), how: "data".to_string() });
            }
        };
        if let Some((written, span)) = &f.data {
            data(written, span.line);
        }
        for m in &f.masters {
            if let Some((written, span)) = &m.data {
                data(written, span.line);
            }
        }
        out
    }
}

/// The file each `use` line of `file` (from the root) reads, from the root: a file of ringi's (an
/// organization, masters, a shared route) and a file of another language alike, at the line of its
/// path, with `use <kind>` as how. The file is the one the system opens (`modules::join_opened`,
/// ritsu's DESIGN 4.15). A line whose path cannot be written from the root names nothing. What the
/// project's index reads of a file of ringi's (ritsu's DESIGN 6.4), and what the page of a shared
/// route reads of the requests that take it ([`crate::doc`]).
pub fn uses_of(root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>> {
    let (f, _) = parsed(root, file)?;
    let mut out = Vec::new();
    for u in &f.uses {
        let Ok(target) = ritsu_base::modules::join_opened(root, file, &u.path) else { continue };
        let tool = match u.kind {
            UseKind::Organization | UseKind::Masters | UseKind::Route => Tool::Ringi,
            UseKind::Rule => Tool::Rulec,
            UseKind::Calendar | UseKind::Dates => Tool::Koyomi,
            UseKind::Book => Tool::Chobo,
            UseKind::Flow => Tool::Dandori,
            UseKind::Gate => Tool::Sekisho,
        };
        out.push(Reference { line: u.path_span.line, target: Name::file(tool, target), how: format!("use {}", u.kind.word()) });
    }
    Ok(out)
}

/// The files of ringi's under the root, in path order, walked as a project's files are (ritsu's
/// DESIGN 6.2): what passes over is what [`ritsu_base::paths::walk`] passes over, another module
/// below the root among it.
pub fn files_under(root: &Path) -> Vec<String> {
    let mut all = Vec::new();
    ritsu_base::paths::walk(root, ".", &[], &mut all);
    all.into_iter().filter(|f| Path::new(f).extension().and_then(|x| x.to_str()) == Tool::Ringi.extension()).collect()
}
