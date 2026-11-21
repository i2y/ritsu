//! The names of a `.ringi` (DESIGN 3.4) and the files its `use` lines read (DESIGN 3.5).
//!
//! A name is declared in one of its spaces: the fields (of the form, or of a shared route's
//! `inputs`), the fields of each row of a `rows` field, the values of each `choice`, the stages,
//! the ways to choose an approver (`who`), the tables and their columns, the aliases of the `use`
//! lines; and, in the files a request reads, the positions and kinds of team of an organization,
//! the masters and their fields. A name the generated code and the JSON take is ASCII, or gives
//! its ASCII name in brackets right after it (`種別(category)`); a reference names what it refers
//! to by its name, not by its ASCII name.
//!
//! The files of an organization, of masters and of a shared route are ringi's, and ringi checks
//! them as it checks the file that reads them; a file of another language is asked about through
//! the ports in a later stage (DESIGN 9.1), and here only whether it is there and of its kind.

use crate::ast::*;
use crate::check::Run;
use crate::diag::Diag;
use crate::kw;
use ritsu_base::text::Text;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// What a `use` line read.
#[derive(Clone)]
pub enum Got {
    /// A file of ringi's that passes its check, with the data it read: an organization's versions,
    /// the rows of masters, or, for a shared route, the data of the organization it reads.
    Ringi(Rc<File>, Option<Rc<crate::org::Data>>),
    /// A rule that passes rulec's check, at the path the line names (as it is given, from where the
    /// program runs), with what rulec knows of it: its inputs, outputs and enums.
    Rule(PathBuf, Rc<ritsu_ports::RuleFacts>),
    /// A file of another language that passes its check, at the path the line names: what it holds
    /// is asked through the ports.
    Other(PathBuf),
    /// Nothing: what is wrong with the line is said (E005, E006, E202).
    Failed,
}

/// A file with what its `use` lines read, the place every check reads names in.
pub struct Scope<'a> {
    pub file: &'a File,
    pub path: &'a str,
    pub src: &'a str,
    /// What each of the file's `use` lines read, in their order.
    pub got: Vec<Got>,
}

/// A way to choose an approver, and the file it is declared in: this one, or a shared route it uses.
pub struct WhoRef<'b> {
    pub who: &'b Who,
    pub shared: bool,
}

impl<'a> Scope<'a> {
    pub fn err(&self, code: &'static str, at: Span, msg: Text) -> Diag {
        Diag::error(code, self.path, at.line, at.col, msg).source(self.src)
    }

    pub fn warn(&self, code: &'static str, at: Span, msg: Text) -> Diag {
        Diag::warning(code, self.path, at.line, at.col, msg).source(self.src)
    }

    /// The fields an expression reads: a request's form, a shared route's inputs.
    pub fn fields(&self) -> &'a [Field] {
        match self.file.kind {
            FileKind::Route => &self.file.inputs,
            _ => &self.file.form,
        }
    }

    pub fn field(&self, name: &str) -> Option<&'a Field> {
        self.fields().iter().find(|f| f.name.text == name)
    }

    /// The `use` line whose alias is `name`, and what it read.
    pub fn alias(&self, name: &str) -> Option<(&'a Use, &Got)> {
        self.file.uses.iter().zip(&self.got).find(|(u, _)| u.alias.as_ref().is_some_and(|a| a.text == name))
    }

    /// The `use` line of that kind whose alias is `name`, and what it read.
    pub fn alias_of(&self, name: &str, kind: UseKind) -> Option<(&'a Use, &Got)> {
        self.alias(name).filter(|(u, _)| u.kind == kind)
    }

    pub fn uses_of(&self, kind: UseKind) -> impl Iterator<Item = &'a Use> {
        self.file.uses.iter().filter(move |u| u.kind == kind)
    }

    /// The organization the file reads: None when it has no `use organization`, Some(None) when
    /// the file it names cannot be read (which is said).
    pub fn org(&self) -> Option<Option<&File>> {
        let (_, g) = self.file.uses.iter().zip(&self.got).find(|(u, _)| u.kind == UseKind::Organization)?;
        Some(match g {
            Got::Ringi(f, _) => Some(f.as_ref()),
            _ => None,
        })
    }

    /// The data of the organization the file reads: through its own `use organization`, else
    /// through the shared route it uses. None when it reads no organization, or the organization's
    /// file does not pass its check (said, E202).
    pub fn org_rc(&self) -> Option<Rc<crate::org::Data>> {
        let own = self.file.uses.iter().zip(&self.got).find(|(u, _)| u.kind == UseKind::Organization);
        if let Some((_, g)) = own {
            return match g {
                Got::Ringi(_, d) => d.clone(),
                _ => None,
            };
        }
        self.file.uses.iter().zip(&self.got).filter(|(u, _)| u.kind == UseKind::Route).find_map(|(_, g)| match g {
            Got::Ringi(_, Some(d)) => Some(d.clone()),
            _ => None,
        })
    }

    /// The versions of the organization the file reads, when there is data.
    pub fn org_data(&self) -> Option<&crate::org::Org> {
        let own = self.file.uses.iter().zip(&self.got).find(|(u, _)| u.kind == UseKind::Organization);
        let got = match own {
            Some((_, g)) => Some(g),
            None => self.file.uses.iter().zip(&self.got).filter(|(u, _)| u.kind == UseKind::Route).map(|(_, g)| g).find(|g| matches!(g, Got::Ringi(_, Some(_)))),
        };
        match got {
            Some(Got::Ringi(_, Some(d))) => match d.as_ref() {
                crate::org::Data::Org(o) => Some(o),
                _ => None,
            },
            _ => None,
        }
    }

    /// The rows of the masters a `use masters` reads, by its alias, when the file has data.
    pub fn masters_data(&self, alias: &str) -> Option<&crate::org::masters::MastersData> {
        match self.alias_of(alias, UseKind::Masters) {
            Some((_, Got::Ringi(_, Some(d)))) => match d.as_ref() {
                crate::org::Data::Masters(m) => Some(m),
                _ => None,
            },
            _ => None,
        }
    }

    /// The file a `use` of a ringi file read, by its alias; None when there is no such line, or
    /// it could not be read.
    pub fn ringi(&self, alias: &str, kind: UseKind) -> Option<&File> {
        match self.alias_of(alias, kind) {
            Some((_, Got::Ringi(f, _))) => Some(f.as_ref()),
            _ => None,
        }
    }

    /// Whether a `use` line of that kind has that alias, whether it could be read or not.
    pub fn is_alias(&self, alias: &str, kind: UseKind) -> bool {
        self.alias_of(alias, kind).is_some()
    }

    /// Whether the line of that alias read nothing (said already): what refers to it says nothing
    /// more.
    pub fn failed(&self, alias: &str) -> bool {
        matches!(self.alias(alias), Some((_, Got::Failed)))
    }

    /// What rulec knows of the rule a `use rule` reads, by its alias: None when there is no such
    /// line, or the rule could not be read (which is said).
    pub fn rule_facts(&self, alias: &str) -> Option<&ritsu_ports::RuleFacts> {
        match self.alias_of(alias, UseKind::Rule) {
            Some((_, Got::Rule(_, f))) => Some(f.as_ref()),
            _ => None,
        }
    }

    /// The path of the file of another language a `use` line reads, by its alias, as it is given.
    pub fn other_path(&self, alias: &str) -> Option<&Path> {
        match self.alias(alias) {
            Some((_, Got::Rule(p, _) | Got::Other(p))) => Some(p.as_path()),
            _ => None,
        }
    }

    /// The shared routes the route uses: `route standard(…)`, or the cells of a table whose output
    /// is `route`. None for any it cannot read.
    pub fn shared_routes(&self) -> Vec<Option<&File>> {
        match &self.file.route {
            Some(Route::Shared { alias, .. }) => vec![self.ringi(&alias.text, UseKind::Route)],
            Some(Route::Table(t)) if t.chooses_route => t
                .rows
                .iter()
                .flat_map(|r| r.stages.iter())
                .filter_map(|(s, _)| match s {
                    Spec::Route(w) => Some(self.ringi(&w.text, UseKind::Route)),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The stages of the route, in their order: the file's own, or those of the shared routes it
    /// uses. None when a shared route it uses cannot be read, so its stages are not known.
    pub fn stages(&self) -> Option<Vec<&Name>> {
        match &self.file.route {
            None => Some(Vec::new()),
            Some(Route::Stages(lines)) => Some(lines.iter().map(|l| &l.name).collect()),
            Some(Route::Table(t)) if !t.chooses_route => Some(t.stages.iter().collect()),
            Some(_) => {
                let mut out: Vec<&Name> = Vec::new();
                for f in self.shared_routes() {
                    for n in own_stages(f?) {
                        if !out.iter().any(|o| o.text == n.text) {
                            out.push(n);
                        }
                    }
                }
                Some(out)
            }
        }
    }

    /// The ways to choose an approver a stage, a claim or a label of the file can name: its own,
    /// and those of the shared routes it uses.
    pub fn who_all(&self) -> Vec<WhoRef<'_>> {
        let mut out: Vec<WhoRef> = self.file.who.iter().map(|w| WhoRef { who: w, shared: false }).collect();
        for f in self.shared_routes().into_iter().flatten() {
            out.extend(f.who.iter().map(|w| WhoRef { who: w, shared: true }));
        }
        out
    }

    pub fn who(&self, name: &str) -> Option<&'a Who> {
        self.file.who.iter().find(|w| w.name.text == name)
    }

    /// The calendar a count of business days without `in` is counted in: the file's one calendar.
    pub fn calendars(&self) -> Vec<&'a Use> {
        self.uses_of(UseKind::Calendar).collect()
    }
}

/// The stages a route file or a request declares itself, in their order.
pub fn own_stages(f: &File) -> Vec<&Name> {
    match &f.route {
        Some(Route::Stages(lines)) => lines.iter().map(|l| &l.name).collect(),
        Some(Route::Table(t)) if !t.chooses_route => t.stages.iter().collect(),
        _ => Vec::new(),
    }
}

/// The file a `use` line names, written from the directory of the file it is in.
pub fn beside(file: &str, written: &str) -> Result<PathBuf, ritsu_base::modules::Refused> {
    ritsu_base::modules::refer_shown(Path::new(file).parent().unwrap_or(Path::new("")), written)
}

/// A file as one is told from another: what the file system says, or as written.
pub fn canonical(p: &Path) -> PathBuf {
    ritsu_base::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// The first word of a file: what a `.ringi` and a `.cal` say their kind with.
pub fn first_word(src: &str) -> Option<String> {
    for line in src.lines() {
        let t = line.trim_start();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        return t.split(|c: char| c.is_whitespace()).next().map(str::to_string);
    }
    None
}

/// What each `use` line reads: E005 where it cannot be read, E006 for a kind of file a shared route
/// does not read, E202 for a file of ringi's that does not pass its check. A file of ringi's is
/// checked through the run, once in a run however many files read it.
pub fn read_uses(f: &File, path: &str, src: &str, run: &Run, diags: &mut Vec<Diag>) -> Vec<Got> {
    let mut out = Vec::new();
    for u in &f.uses {
        let at = |code: &'static str, msg: Text| Diag::error(code, path, u.path_span.line, u.path_span.col, msg).source(src);
        let word = u.kind.word();
        if f.kind == FileKind::Masters && u.kind != UseKind::Organization {
            diags.push(
                Diag::error("E006", path, u.line, 1, tr!("マスタのファイルが `use` で読めるのは、組織だけです", "A file of masters reads only an organization with `use`"))
                    .source(src)
                    .note(tr!("組織を読むと、マスタの人の列を、組織のどの版にもいるかで確かめます（E505）。", "With the organization read, the people of the masters' rows are checked against each of its versions (E505).")),
            );
            out.push(Got::Failed);
            continue;
        }
        if f.kind == FileKind::Route && u.kind == UseKind::Route {
            diags.push(
                Diag::error("E006", path, u.line, 1, tr!("共有の経路のファイルは、ほかの経路を `use route` で読めません", "A shared route's file cannot read another route with `use route`"))
                    .source(src)
                    .note(tr!("経路を組み合わせるのは、申請の種類のファイルです。", "Routes are put together in a request's file.")),
            );
            out.push(Got::Failed);
            continue;
        }
        // a path is written from the file it is in (DESIGN 3.5): one from the root of the disk names
        // another file once the tree is copied, and the files a request is lowered to cannot name it
        if ritsu_base::paths::is_absolute(&u.path) {
            diags.push(at("E005", ritsu_base::paths::PathError::Absolute.text(&u.path)).note(tr!(
                "このファイルのあるディレクトリから書けば、ツリーをどこへ写しても同じファイルを指します（上のディレクトリは `../`）。",
                "Written from the directory this file is in, it names the same file wherever the tree is copied (`../` is the directory above)."
            )));
            out.push(Got::Failed);
            continue;
        }
        let target = match beside(path, &u.path) {
            Ok(t) => t,
            Err(r) => {
                diags.push(at("E005", r.text()));
                out.push(Got::Failed);
                continue;
            }
        };
        // the path as the file wrote it, in what is said of what it wrote, however the file was
        // given; the file it reaches, as the run writes its paths, where it is read (ritsu's DESIGN
        // 4.15)
        let shown = u.path.clone();
        let reached = ritsu_base::paths::show(&target);
        let ext = u.kind.extension();
        if !shown.ends_with(&format!(".{ext}")) {
            diags.push(at("E005", tr!("`use {word}` が読むのは `.{ext}` のファイルです。`{shown}` はそうではありません", "`use {word}` reads a `.{ext}` file, which `{shown}` is not")));
            out.push(Got::Failed);
            continue;
        }
        if !ritsu_base::fs::is_file(&target) {
            let d = match ritsu_base::modules::missing(&target) {
                Some(t) => at("E005", t),
                None => at("E005", tr!("`{shown}` というファイルはありません", "There is no file `{shown}`"))
                    .note(tr!("パスは、このファイルのあるディレクトリから書きます。", "A path is written from the directory of the file it is in.")),
            };
            let outside = ritsu_base::modules::outside_note_for(Path::new(path).parent().unwrap_or(Path::new("")), &u.path);
            diags.push(match outside {
                Some(n) => d.note(n),
                None => d,
            });
            out.push(Got::Failed);
            continue;
        }
        if let Some(first) = u.kind.first_word() {
            let text = ritsu_base::fs::read_to_string(&target).unwrap_or_default();
            let found = first_word(&text).unwrap_or_default();
            if found != first {
                let mut d = at(
                    "E005",
                    tr!(
                        "`use {word}` が読むのは `{first}` で始まるファイルです。`{shown}` は `{found}` で始まります",
                        "`use {word}` reads a file that starts with `{first}`, and `{shown}` starts with `{found}`"
                    ),
                );
                if let Some(k) = UseKind::ALL.iter().find(|k| k.first_word() == Some(found.as_str()) && k.extension() == ext) {
                    let w = k.word();
                    d = d.note(tr!("そのファイルを読むなら、`use {w}` と書いてください。", "To read that file, write `use {w}`."));
                }
                diags.push(d);
                out.push(Got::Failed);
                continue;
            }
        }
        if let Some(tool) = u.kind.tool() {
            // the file's own language is asked whether it passes; one that does not is said here,
            // where the line reads it, and the line waits for it in `ritsu check`
            let read = other(run, u.kind, &target, path);
            if let Err(said) = read {
                let mut d = at("E202", tr!("`{shown}` は {tool} の検査を通りません", "`{shown}` does not pass {tool}'s check"));
                for x in said.iter().filter(|x| !x.code.is_empty()).take(3) {
                    let code = &x.code;
                    let place = match x.line {
                        Some(l) => format!("{}:{l}", x.file),
                        None => x.file.clone(),
                    };
                    let (ja, en) = (ritsu_base::text::spaced(&x.message, ritsu_base::text::Lang::Ja), ritsu_base::text::spaced(&x.message, ritsu_base::text::Lang::En));
                    d = d.note(tr!("{code}: {ja}（`{place}`）", "{code}: {en} ({place})"));
                }
                if let Some(x) = said.iter().find(|x| x.code.is_empty()) {
                    let (ja, en) = (x.message.ja.clone(), x.message.en.clone());
                    d = d.note(tr!("{ja}", "{en}"));
                }
                diags.push(d.note(tr!("そのファイルを、{tool} の検査が通るように直してください。", "Correct that file until {tool}'s check passes.")));
                out.push(Got::Failed);
                continue;
            }
            match read {
                Ok(Some(facts)) => out.push(Got::Rule(target, Rc::new(facts))),
                _ => out.push(Got::Other(target)),
            }
            continue;
        }
        match run.file(&reached) {
            Ok(o) if !o.has_errors() => match &o.file {
                Some(file) => out.push(Got::Ringi(file.clone(), o.data.clone())),
                None => out.push(Got::Failed),
            },
            Ok(o) => {
                let mut d = at("E202", tr!("`{shown}` は ringi の検査を通りません", "`{shown}` does not pass ringi's check"));
                for x in o.diags.iter().filter(|d| d.is_error()).take(3) {
                    let (code, place) = (x.code, x.place());
                    let (ja, en) = (ritsu_base::text::spaced(&x.message, ritsu_base::text::Lang::Ja), ritsu_base::text::spaced(&x.message, ritsu_base::text::Lang::En));
                    d = d.note(tr!("{code}: {ja}（`{place}`）", "{code}: {en} ({place})"));
                }
                diags.push(d.note(tr!("そのファイルを、検査が通るように直してください。", "Correct that file until its check passes.")));
                out.push(Got::Failed);
            }
            Err(e) => {
                diags.push(at("E005", tr!("`{shown}` を読めません: {e}", "Cannot read `{shown}`: {e}")));
                out.push(Got::Failed);
            }
        }
    }
    out
}

/// Whether every flow and every gate the file `f` at `path` reads (`use flow`, `use gate`) passes the
/// check of its language: what a check of the file that read them taken to pass, unread, needs to
/// be the check that reads them ([`Run::set_unread`]). A line that names no file it can read is said
/// by the check either way.
pub fn flows_and_gates_pass(f: &File, path: &str, run: &Run) -> bool {
    f.uses.iter().filter(|u| matches!(u.kind, UseKind::Flow | UseKind::Gate)).all(|u| match beside(path, &u.path) {
        Ok(target) => !ritsu_base::fs::is_file(&target) || other(run, u.kind, &target, path).is_ok(),
        Err(_) => true,
    })
}

/// The file of another language a `use` line reads (at `target`, from the file at `path`), asked of
/// its language: a rule's facts; Ok(None) for another file that passes, or when the language is not
/// joined (which E008 says before); what the language says when it does not pass its check.
fn other(run: &Run, kind: UseKind, target: &Path, path: &str) -> Result<Option<ritsu_ports::RuleFacts>, Vec<ritsu_ports::Said>> {
    let s = &run.suite;
    // a flow and a gate taken to pass, unread, as they are where they pass ([`Run::set_unread`])
    if run.unread() && matches!(kind, UseKind::Flow | UseKind::Gate) {
        return Ok(None);
    }
    match kind {
        UseKind::Rule => match &s.rules {
            Some(r) => r.facts(target).map(Some),
            None => Ok(None),
        },
        UseKind::Calendar => s.dates.as_ref().map_or(Ok(()), |d| d.calendar(target).map(|_| ())).map(|_| None),
        UseKind::Dates => s.dates.as_ref().map_or(Ok(()), |d| d.facts(target).map(|_| ())).map(|_| None),
        UseKind::Book => s.books.as_ref().map_or(Ok(()), |b| b.facts(target).map(|_| ())).map(|_| None),
        UseKind::Flow => match (&s.flows, &s.rules, &s.dates, &s.books) {
            (Some(f), Some(r), Some(d), Some(b)) => {
                let ports = ritsu_ports::Ports { rules: r.clone(), dates: d.clone(), books: b.clone() };
                f.crossings(target, &ports).map(|_| None)
            }
            _ => Ok(None),
        },
        UseKind::Gate => {
            let root = run.root_for(path);
            match (&s.gates, ritsu_base::paths::from_root(&root, target)) {
                (Some(g), Some(rel)) => g.facts(&root, &rel).map(|_| None),
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

// ── Declarations: E002 and E003 ──

/// A name as one of a space declares it.
struct Decl {
    text: String,
    span: Span,
    alias: Option<Word>,
    /// Whether the generated code takes it, so that it needs an ASCII name (claims and scenarios
    /// are named for people only).
    coded: bool,
}

impl Decl {
    fn name(n: &Name) -> Decl {
        Decl { text: n.text.clone(), span: n.span, alias: n.alias.clone(), coded: true }
    }

    fn word(w: &Word) -> Decl {
        Decl { text: w.text.clone(), span: w.span, alias: None, coded: false }
    }

    fn ascii(&self) -> &str {
        self.alias.as_ref().map(|a| a.text.as_str()).unwrap_or(&self.text)
    }
}

/// The words the generated code cannot take as a name: TypeScript's in strict mode, Python's
/// keywords and Go's (kaname's E007 holds its names to the same).
pub fn taken_word(w: &str) -> bool {
    ritsu_emit::words::typescript::NAMES.contains(w) || ritsu_emit::words::python::KEYWORDS.contains(&w) || ritsu_emit::words::go::KEYWORDS.contains(&w)
}

/// E002: a name that cannot be used.
fn usable(s: &Scope, d: &Decl, field: bool, out: &mut Vec<Diag>) {
    let w = &d.text;
    if kw::is_reserved(w) {
        out.push(s.err("E002", d.span, tr!("`{w}` は条件を書く語なので、名前にできません", "`{w}` is a word conditions are written with, and cannot be a name")).note(reserved_note()));
        return;
    }
    if field && kw::NOT_FIELDS.contains(&w.as_str()) {
        out.push(s.err("E002", d.span, tr!("`{w}` は式が読む語（`{w}.…`）なので、フィールドの名前にできません", "`{w}` is what an expression reads (`{w}.…`), and cannot be a field's name")));
        return;
    }
    if !d.coded {
        return;
    }
    if let Some(a) = &d.alias {
        let t = &a.text;
        if !ritsu_emit::ident::is_ascii_ident(t) {
            out.push(s.err("E002", a.span, tr!("`{t}` は ASCII の名前ではありません", "`{t}` is not an ASCII name")).note(ascii_note()));
            return;
        }
    } else if !ritsu_emit::ident::is_ascii_ident(w) {
        out.push(
            s.err("E002", d.span, tr!("`{w}` には ASCII の名前が要ります。`{w}(ascii_name)` のように、括弧の中に書いてください", "`{w}` needs an ASCII name: write it in brackets right after it, as in `{w}(ascii_name)`"))
                .note(tr!(
                    "生成するコードと JSON は ASCII の名前を使います。名前は英字か `_` で始め、英字、数字、`_` で書きます。",
                    "The generated code and the JSON take the ASCII name: a letter or `_`, then letters, digits and `_`."
                )),
        );
        return;
    }
    let a = d.ascii();
    if a == "_" {
        let at = d.alias.as_ref().map(|x| x.span).unwrap_or(d.span);
        out.push(s.err("E002", at, tr!("`_` だけでは名前にできません", "`_` alone cannot be a name")).note(tr!(
            "生成するコードの言語は、`_` だけの名前を、値を持たない名前として読みます（Go、Rust、Python の `match`）。別の名前にするか、`名前(ascii_name)` のように別の ASCII の名前を括弧に書いてください。",
            "The languages the code is generated in read a name of `_` alone as one that holds no value (Go, Rust, and Python's `match`). Choose another name, or give another ASCII name in brackets, as in `name(ascii_name)`."
        )));
        return;
    }
    if taken_word(a) {
        let at = d.alias.as_ref().map(|x| x.span).unwrap_or(d.span);
        out.push(s.err("E002", at, tr!("`{a}` は生成するコードの言語の予約語なので、名前にできません", "`{a}` is a reserved word of a language the code is generated in, and cannot be a name")).note(tr!(
            "TypeScript、Python、Go のどれかが、`{a}` を名前に使えません。別の名前にするか、`名前(ascii_name)` のように別の ASCII の名前を括弧に書いてください。",
            "One of TypeScript, Python and Go cannot take `{a}` as a name. Choose another name, or give another ASCII name in brackets, as in `name(ascii_name)`."
        )));
        return;
    }
    if crate::lower::words::KANAME_HELPERS.contains(&a) {
        let at = d.alias.as_ref().map(|x| x.span).unwrap_or(d.span);
        let all = crate::lower::words::KANAME_HELPERS.iter().map(|w| format!("`{w}`")).collect::<Vec<_>>().join(" ");
        out.push(s.err("E002", at, tr!("`{a}` は生成するコードが補助に使う名前なので、名前にできません", "`{a}` is a name the generated code keeps for its helpers, and cannot be a name")).note(tr!(
            "kaname が生成するコードは、{all} を補助のモジュールとその型に使います。別の名前にするか、`名前(ascii_name)` のように別の ASCII の名前を括弧に書いてください。",
            "The code kaname generates keeps {all} for its helper module and its types. Choose another name, or give another ASCII name in brackets, as in `name(ascii_name)`."
        )));
    }
}

/// E002 for the name of a request that cannot begin the names of the files it is lowered to and of
/// the code generated from them (`expense_route`, `expense_terms`): one that
/// starts with `_` (Go reads no file whose name starts with `_`), ends with `_` (`x__terms`), or has
/// two `_` in a row, which rustc warns of in a module's name (koyomi's E016 when the code is
/// generated). The check passed such a name, and `ritsu gen` refused it or wrote code that did not
/// build.
fn file_named(s: &Scope, d: &Decl, out: &mut Vec<Diag>) {
    let a = d.ascii();
    if !ritsu_emit::ident::is_ascii_ident(a) || !(a.starts_with('_') || a.ends_with('_') || a.contains("__")) {
        return;
    }
    let at = d.alias.as_ref().map(|x| x.span).unwrap_or(d.span);
    let said = if a.starts_with('_') {
        tr!("`{a}` は `_` で始まるので、申請の種類の名前にできません", "`{a}` starts with `_`, and cannot name a kind of request")
    } else if a.ends_with('_') {
        tr!("`{a}` は `_` で終わるので、申請の種類の名前にできません", "`{a}` ends with `_`, and cannot name a kind of request")
    } else {
        tr!("`{a}` は `_` が二つ続くので、申請の種類の名前にできません", "`{a}` has two `_` in a row, and cannot name a kind of request")
    };
    out.push(s.err("E002", at, said).note(tr!(
        "申請の種類の名前は、下ろしたファイルと、そこから生成するコードのモジュールやパッケージの名前の頭になります（`{a}_route`）。Go は `_` で始まる名前のファイルを読まず、Rust は `_` が二つ続くモジュールの名前を警告します。英字で始まり、英字か数字で終わり、`_` が続かない名前にしてください。",
        "The name of a kind of request begins the names of the files it is lowered to, and of the modules and packages of the code generated from them (`{a}_route`). Go reads no file whose name starts with `_`, and rustc warns of a module's name with two `_` in a row. Choose a name that starts with a letter, ends with a letter or a digit, and has no two `_` in a row."
    )));
}

/// E002 for a field of the form named with a word kaname reserves that no language the code is
/// generated in reserves (`then`, `match`, `some`, `none`): the glue declares each field of the form
/// as a field of a record, and kaname's generated code reads and writes the JSON of a record by the
/// names it declares, so that no other name can stand in for the word.
fn glued(s: &Scope, d: &Decl, out: &mut Vec<Diag>) {
    let a = d.ascii();
    if !crate::lower::words::KANAME_RESERVED.contains(&a) {
        return;
    }
    let at = d.alias.as_ref().map(|x| x.span).unwrap_or(d.span);
    out.push(s.err("E002", at, tr!("`{a}` は kaname の予約語なので、フィールドの名前にできません", "`{a}` is a word kaname reserves, and cannot be a field's name")).note(tr!(
        "チケットの JSON は、フィールドを ASCII の名前で持ちます。それを読み書きする kaname のコードは、予約語を名前にできません。別の名前にするか、`名前(ascii_name)` のように別の ASCII の名前を括弧に書いてください。",
        "The JSON of a ticket holds each field by its ASCII name, and the kaname code that reads and writes it cannot take a word kaname reserves as a name. Choose another name, or give another ASCII name in brackets, as in `name(ascii_name)`."
    )));
}

/// E002 for two fields of the form, or of one row, whose ASCII names are one name in the code
/// generated for Go (`a_b` and `aB` are both `AB`): the records of the glue hold each field as a
/// field of theirs, which Go writes in Pascal case, and the JSON of a ticket holds it by its ASCII
/// name, so that no other name can stand in for either.
fn one_in_go(s: &Scope, decls: &[Decl], out: &mut Vec<Diag>) {
    let go = |a: &str| ritsu_emit::ident::go_exported(&ritsu_emit::ident::pascal(a));
    // `_` alone is E002 of its own ([`usable`]), said once
    let named = |a: &str| ritsu_emit::ident::is_ascii_ident(a) && a != "_";
    for (i, d) in decls.iter().enumerate() {
        let a = d.ascii();
        if !named(a) {
            continue;
        }
        let g = go(a);
        let Some(first) = decls[..i].iter().find(|x| x.ascii() != a && named(x.ascii()) && go(x.ascii()) == g) else { continue };
        let (other, line) = (first.ascii(), first.span.line);
        let at = d.alias.as_ref().map(|x| x.span).unwrap_or(d.span);
        out.push(
            s.err("E002", at, tr!("`{a}` は、フィールド `{other}`（{line} 行目）と、Go に生成するコードで同じ名前 `{g}` になります", "`{a}` and the field `{other}` (line {line}) are one name in the code generated for Go, `{g}`")).note(tr!(
                "チケットの JSON は、フィールドを ASCII の名前で持ちます。生成するコードのレコードはそれをフィールドに持ち、Go はフィールドの名前を PascalCase で書くので、二つを分けられません。JSON のキーなので、どちらもほかの名前に置き換えられません。どちらかを別の名前にするか、`名前(ascii_name)` のように別の ASCII の名前を括弧に書いてください。",
                "The JSON of a ticket holds each field by its ASCII name, and the records of the generated code hold it as a field of theirs, which Go writes in PascalCase, so the two cannot be told apart there. A key of the JSON cannot be written under another name. Choose another name for one of the two, or give it another ASCII name in brackets, as in `name(ascii_name)`."
            )),
        );
    }
}

/// E002 for a way to choose an approver named as the value a stage takes where the file writes
/// several ways or a share of one (`a and b` is `a_and_b`, `all of legal` is `all_of_legal`): a
/// ticket holds the value of each stage by that name (`stages` of its JSON), where the way's name
/// would be the same, and neither can be written otherwise.
fn made_values(s: &Scope, who: &[Decl], out: &mut Vec<Diag>) {
    let f = s.file;
    let mut specs: Vec<(&Spec, usize)> = Vec::new();
    match &f.route {
        Some(Route::Stages(lines)) => specs.extend(lines.iter().map(|l| (&l.spec, l.line))),
        Some(Route::Table(t)) => specs.extend(t.rows.iter().flat_map(|r| r.stages.iter().map(move |(sp, _)| (sp, r.line)))),
        _ => {}
    }
    specs.extend(f.tables.iter().flat_map(|t| t.rows.iter().flat_map(|r| r.stages.iter().map(move |(sp, _)| (sp, r.line)))));
    if let Some(e) = &f.examples {
        specs.extend(e.rows.iter().flat_map(|r| r.stages.iter().map(move |(sp, _)| (sp, r.line))));
    }
    for c in &f.claims {
        if let ClaimBody::OnlyWhen { spec, .. } | ClaimBody::Possible { spec, .. } = &c.body {
            specs.push((spec, c.line));
        }
    }
    let mut said: Vec<&str> = Vec::new();
    for (spec, line) in specs {
        let made = match spec {
            Spec::Who(ws) => ws.len() > 1,
            Spec::Quorum(..) | Spec::Circulate(_) | Spec::Note(_) => true,
            Spec::Empty | Spec::RuleOutput(_) | Spec::Route(_) => false,
        };
        let Some((v, _)) = made.then(|| crate::lower::words::spec_value(s, spec)).flatten() else { continue };
        let Some(d) = who.iter().find(|d| d.coded && d.ascii() == v) else { continue };
        if said.contains(&d.ascii()) {
            continue;
        }
        said.push(d.ascii());
        let w = spec.shown();
        let at = d.alias.as_ref().map(|x| x.span).unwrap_or(d.span);
        out.push(
            s.err("E002", at, tr!("`{v}` は、`{w}`（{line} 行目）がステップに与える値と同じ名前なので、承認者の決め方の名前にできません", "`{v}` is the value `{w}` (line {line}) gives a stage too, and cannot name a way to choose an approver")).note(tr!(
                "チケットは、ステップの承認者の決め方を、そのステップの値として ASCII の名前で持ちます（JSON の `stages` の `value`）。`{w}` の値は `{v}` なので、二つを分けられません。決め方を別の名前にするか、`名前(ascii_name)` のように別の ASCII の名前を括弧に書いてください。",
                "A ticket holds how a stage chooses its approvers as the stage's value, by its ASCII name (`value` in `stages` of its JSON), and the value of `{w}` is `{v}`, so the two cannot be told apart. Choose another name for the way, or give it another ASCII name in brackets, as in `name(ascii_name)`."
            )),
        );
    }
}

/// Whether the input column `k` of a named table is named as a field of the form and its rows name
/// ways to choose an approver: the column holds the field's values (DESIGN 5.10), and a way named
/// in it is no value of the field. E002 says so at the column, and no cell of it is read further.
pub fn ways_in_field_column(s: &Scope, t: &NamedTable, k: usize) -> bool {
    let Some(col) = t.inputs.get(k) else { return false };
    let Some(f) = s.field(&col.text) else { return false };
    let way = |n: &str| s.who_all().iter().any(|w| w.who.name.text == n || w.who.name.ascii() == n);
    let value = |n: &str| f.options.iter().any(|o| o.name.text == n || o.name.ascii() == n);
    t.rows.iter().filter_map(|r| r.tests.get(k)).any(|(test, _)| match test {
        Test::Values { values, .. } => values.iter().any(|v| matches!(&v.kind, ExprKind::Path(p) if p.len() == 1 && way(&p[0].text) && !value(&p[0].text))),
        _ => false,
    })
}

fn reserved_note() -> Text {
    let words = kw::RESERVED.iter().map(|w| format!("`{w}`")).collect::<Vec<_>>().join(" ");
    tr!("名前にできない語は {words} の 7 つです。", "The seven words that cannot be names are {words}.")
}

fn ascii_note() -> Text {
    tr!("ASCII の名前は、英字か `_` で始め、英字、数字、`_` で書きます。", "An ASCII name is a letter or `_`, then letters, digits and `_`.")
}

/// The name of a space of names, as it sits inside a Japanese sentence: with a space on the side
/// where a backquote meets the Japanese around it, as the suite's Japanese writes it (`は `who`
/// に`).
fn mid(t: &str) -> String {
    let mut s = t.to_string();
    if s.starts_with('`') {
        s.insert(0, ' ');
    }
    if s.ends_with('`') {
        s.push(' ');
    }
    s
}

/// E003: a name declared twice in one space, or one ASCII name given to two names of it.
fn once(s: &Scope, decls: &[Decl], space: &Text, out: &mut Vec<Diag>) {
    for (i, d) in decls.iter().enumerate() {
        if let Some(first) = decls[..i].iter().find(|x| x.text == d.text) {
            let (w, line) = (&d.text, first.span.line);
            out.push(s.err("E003", d.span, tr!("`{w}` は{}に二度宣言されています（最初は {line} 行目）", "`{w}` is declared twice in {} (first on line {line})", mid(&space.ja); space.en)));
            continue;
        }
        if !d.coded {
            continue;
        }
        let a = d.ascii();
        if let Some(first) = decls[..i].iter().find(|x| x.coded && x.ascii() == a) {
            let (other, line) = (&first.text, first.span.line);
            let at = d.alias.as_ref().map(|x| x.span).unwrap_or(d.span);
            out.push(s.err(
                "E003",
                at,
                tr!("ASCII の名前 `{a}` は、{}の二つの名前に付いています（{line} 行目の `{other}`）", "The ASCII name `{a}` is given to two names in {} (`{other}` on line {line})", mid(&space.ja); space.en),
            ));
        }
    }
}

/// E002 and E003 for every space of the file.
pub fn declarations(s: &Scope, out: &mut Vec<Diag>) {
    let f = s.file;
    let named = Decl::name(&f.name);
    let before = out.len();
    usable(s, &named, false, out);
    if out.len() == before && f.kind == crate::ast::FileKind::Request {
        file_named(s, &named, out);
    }
    // the aliases of the `use` lines
    let aliases: Vec<Decl> = f.uses.iter().filter_map(|u| u.alias.as_ref()).map(Decl::name).collect();
    for d in &aliases {
        usable(s, d, false, out);
    }
    once(s, &aliases, &tr!("`use` の別名", "the aliases of the `use` lines"), out);
    // the fields, and the fields of each row
    let fields = s.fields();
    fields_space(s, fields, &tr!("フォーム", "the form"), true, out);
    for fd in fields {
        if let Some((u, _)) = s.alias(&fd.name.text) {
            let (w, line) = (&fd.name.text, u.line);
            out.push(s.err("E003", fd.name.span, tr!("`{w}` は `use` の別名でもあるので（{line} 行目）、フィールドの名前にできません", "`{w}` is the alias of a `use` line too (line {line}), so it cannot name a field")));
        }
    }
    // the stages
    let stages: Vec<Decl> = own_stages(f).into_iter().map(Decl::name).collect();
    for d in &stages {
        usable(s, d, false, out);
    }
    once(s, &stages, &tr!("経路のステップ", "the stages of the route"), out);
    // the ways to choose an approver
    let who: Vec<Decl> = f.who.iter().map(|w| Decl::name(&w.name)).collect();
    for d in &who {
        usable(s, d, false, out);
        // `then skip` and `then stay` read the words, where `then <name>` reads a way (DESIGN 5.4)
        if [kw::SKIP, kw::STAY].contains(&d.text.as_str()) {
            let w = &d.text;
            out.push(s.err(
                "E002",
                d.span,
                tr!(
                    "`{w}` は `when empty escalate, then {w}` の語なので、承認者の決め方の名前にできません",
                    "`{w}` is the word of `when empty escalate, then {w}`, and cannot name a way to choose an approver"
                ),
            ));
        }
    }
    once(s, &who, &tr!("`who`", "`who`"), out);
    made_values(s, &who, out);
    // the tables and their columns
    let tables: Vec<Decl> = f.tables.iter().map(|t| Decl::name(&t.name)).collect();
    for d in &tables {
        usable(s, d, false, out);
    }
    once(s, &tables, &tr!("表", "the tables"), out);
    for t in &f.tables {
        let cols: Vec<Decl> = t.inputs.iter().chain(&t.outputs).map(Decl::name).collect();
        for d in &cols {
            usable(s, d, false, out);
        }
        let n = &t.name.text;
        once(s, &cols, &tr!("表 `{n}` の列", "the columns of the table `{n}`"), out);
    }
    // claims and scenarios, named for people
    let claims: Vec<Decl> = f.claims.iter().map(|c| Decl::word(&c.name)).collect();
    for d in &claims {
        usable(s, d, false, out);
    }
    once(s, &claims, &tr!("主張", "the claims"), out);
    let scenarios: Vec<Decl> = f.scenarios.iter().map(|c| Decl::word(&c.name)).collect();
    for d in &scenarios {
        usable(s, d, false, out);
    }
    once(s, &scenarios, &tr!("シナリオ", "the scenarios"), out);
    // the labels: a language once, and a key once in it
    for (i, l) in f.labels.iter().enumerate() {
        if let Some(first) = f.labels[..i].iter().find(|x| x.lang.text == l.lang.text) {
            let (lang, line) = (&l.lang.text, first.line);
            out.push(s.err("E003", l.lang.span, tr!("`labels {lang}` は一度だけ書きます（最初は {line} 行目）", "`labels {lang}` is written once (first on line {line})")));
        }
        let keys: Vec<Decl> = l.entries.iter().map(|e| Decl { text: label_key(&e.key), span: e.span, alias: None, coded: false }).collect();
        let lang = &l.lang.text;
        once(s, &keys, &tr!("`labels {lang}`", "`labels {lang}`"), out);
    }
    // an organization's positions and kinds of team
    let positions: Vec<Decl> = f.positions.iter().map(|p| Decl::name(&p.name)).collect();
    for d in &positions {
        usable(s, d, false, out);
    }
    once(s, &positions, &tr!("役職", "the positions"), out);
    let kinds: Vec<Decl> = f.team_kinds.iter().map(|p| Decl::name(&p.name)).collect();
    for d in &kinds {
        usable(s, d, false, out);
    }
    once(s, &kinds, &tr!("チームの種類", "the kinds of team"), out);
    // masters and their fields
    let masters: Vec<Decl> = f.masters.iter().map(|m| Decl::name(&m.name)).collect();
    for d in &masters {
        usable(s, d, false, out);
    }
    once(s, &masters, &tr!("マスタ", "the masters"), out);
    for m in &f.masters {
        let n = &m.name.text;
        fields_space(s, &m.fields, &tr!("マスタ `{n}` のフィールド", "the fields of the master `{n}`"), false, out);
    }
}

/// The fields of one space, the values of each `choice` and the fields of each row. A field of the
/// form (`form`) and of its rows is a field of a record of the glue too.
fn fields_space(s: &Scope, fields: &[Field], space: &Text, form: bool, out: &mut Vec<Diag>) {
    let decls: Vec<Decl> = fields.iter().map(|f| Decl::name(&f.name)).collect();
    for d in &decls {
        let before = out.len();
        usable(s, d, true, out);
        if form && out.len() == before {
            glued(s, d, out);
        }
    }
    once(s, &decls, space, out);
    if form {
        one_in_go(s, &decls, out);
    }
    for fd in fields {
        let n = &fd.name.text;
        if !fd.options.is_empty() {
            let values: Vec<Decl> = fd.options.iter().map(|o| Decl::name(&o.name)).collect();
            for d in &values {
                usable(s, d, false, out);
            }
            once(s, &values, &tr!("`{n}` の値", "the values of `{n}`"), out);
        }
        if !fd.children.is_empty() {
            fields_space(s, &fd.children, &tr!("明細 `{n}` の行", "the row of `{n}`"), form, out);
        }
    }
}

/// A label's key as it is written: `category.travel`, `stage first`.
pub fn label_key(k: &LabelKey) -> String {
    match k {
        LabelKey::Request => "request".into(),
        LabelKey::Field(p) => dotted(p),
        LabelKey::Stage(w) => format!("stage {}", w.text),
        LabelKey::Who(w) => format!("who {}", w.text),
    }
}

// ── References: E004 and E007 ──

/// `The X declared: a, b and c`, when there are any.
pub fn declared(what: Text, names: &[String]) -> Option<Text> {
    if names.is_empty() {
        return None;
    }
    let shown: Vec<Text> = names.iter().take(12).map(|n| Text::same(format!("`{n}`"))).collect();
    let list = Text::list(&shown);
    Some(tr!("宣言されている{}: {}", "The {} declared: {}", what.ja, list.ja; what.en, list.en))
}

/// E004 with the names that are declared where it looks.
pub fn undeclared(s: &Scope, at: Span, msg: Text, what: Text, names: &[String]) -> Diag {
    // one name declared nowhere is said once, where the file first writes it, with the other lines
    // that write it in a note (ritsu's DESIGN 8.3; `diag::sort`): among the names of one kind
    // declared in one place, `what` and the names declared there (a master's fields are not the
    // form's, nor the columns of one rows another's)
    let scope = std::iter::once(what.en.as_str()).chain(names.iter().map(String::as_str)).collect::<Vec<_>>().join("\n");
    let d = s.err("E004", at, msg).same_in(scope, crate::types::written(s.src, at));
    match declared(what, names) {
        Some(n) => d.note(n),
        None => d,
    }
}

fn who_names(s: &Scope, all: bool) -> Vec<String> {
    if all { s.who_all().iter().map(|w| w.who.name.text.clone()).collect() } else { s.file.who.iter().map(|w| w.name.text.clone()).collect() }
}

fn aliases_of(s: &Scope, kinds: &[UseKind]) -> Vec<String> {
    s.file.uses.iter().filter(|u| kinds.contains(&u.kind)).filter_map(|u| u.alias.as_ref().map(|a| a.text.clone())).collect()
}

/// E004 for a way to choose an approver a stage, a cell or a claim names (`all`: the shared
/// routes' too).
pub fn who_ref(s: &Scope, w: &Word, all: bool, out: &mut Vec<Diag>) -> bool {
    let known = if all { s.who_all().iter().any(|x| x.who.name.text == w.text) } else { s.who(&w.text).is_some() };
    if !known {
        let n = &w.text;
        out.push(undeclared(s, w.span, tr!("`{n}` は `who` に宣言がありません", "`{n}` is not declared in `who`"), tr!("承認者の決め方", "ways to choose an approver"), &who_names(s, all)));
    }
    known
}

/// E004 for what a spec names: the ways to choose, a flow, a rule's output, a shared route.
pub fn spec_refs(s: &Scope, spec: &Spec, all: bool, out: &mut Vec<Diag>) {
    match spec {
        Spec::Empty => {}
        Spec::Who(ws) => {
            for w in ws {
                who_ref(s, w, all, out);
            }
        }
        Spec::Quorum(_, w, _) | Spec::Circulate(w) => {
            who_ref(s, w, all, out);
        }
        Spec::Note(w) => {
            if !s.is_alias(&w.text, UseKind::Flow) {
                let n = &w.text;
                out.push(undeclared(s, w.span, tr!("`{n}` は `use flow` の別名ではありません", "`{n}` is not the alias of a `use flow`"), tr!("flow の別名", "aliases of flows"), &aliases_of(s, &[UseKind::Flow])));
            }
        }
        Spec::RuleOutput(p) => {
            let n = &p[0].text;
            if !s.is_alias(n, UseKind::Rule) {
                out.push(undeclared(s, p[0].span, tr!("`{n}` は `use rule` の別名ではありません", "`{n}` is not the alias of a `use rule`"), tr!("規則の別名", "aliases of rules"), &aliases_of(s, &[UseKind::Rule])));
            } else if p.len() != 2 {
                let whole = dotted(p);
                out.push(s.err("E004", p[0].span, tr!("`{whole}` は規則の出力ではありません。規則の別名と出力の名前を `authority.decider` のように書いてください", "`{whole}` is no rule's output: write the rule's alias and the output's name, as in `authority.decider`")));
            } else if let Some(facts) = s.rule_facts(n) {
                // the output names the approver: one of the rule's outputs, and an enum
                let whole = dotted(p);
                match facts.outputs.iter().find(|o| o.name == p[1].text || o.alias == p[1].text) {
                    None => {
                        let names: Vec<String> = facts.outputs.iter().map(|o| o.name.clone()).collect();
                        out.push(undeclared(s, p[1].span, tr!("規則 `{n}` に、出力 `{whole}` はありません", "The rule `{n}` has no output `{whole}`"), tr!("規則の出力", "outputs of the rule"), &names));
                    }
                    Some(o) if !matches!(o.ty, ritsu_ports::ColumnType::Enum { .. }) => {
                        out.push(
                            s.err("E009", p[1].span, tr!("承認者を決める `{whole}` は、列挙の出力ではありません", "`{whole}`, which names the approver, is no output of an enum"))
                                .note(tr!("規則の出力で承認者を決めるときは、どの値も `who` の承認者の決め方の名前になっている、列挙の出力を使ってください。", "To take the approver from a rule's output, use an output of an enum whose every value names a way to choose approvers in `who`.")),
                        );
                    }
                    Some(o) => {
                        // every value of the output names a way to choose approvers (E203)
                        let ritsu_ports::ColumnType::Enum { name: en, .. } = &o.ty else { return };
                        let Some(e) = facts.enums.iter().find(|e| &e.name == en) else { return };
                        let all = s.who_all();
                        let missing: Vec<String> = e.values.iter().filter(|v| !all.iter().any(|w| w.who.name.text == v.public || w.who.name.ascii() == v.public || w.who.name.text == v.name)).map(|v| v.public.clone()).collect();
                        if !missing.is_empty() {
                            let vs = missing.iter().map(|v| format!("`{v}`")).collect::<Vec<_>>().join(", ");
                            let first = missing[0].clone();
                            out.push(
                                s.err("E203", p[1].span, tr!("`{whole}` の値 {vs} に、`who` の承認者の決め方がありません", "The values {vs} of `{whole}` name no way to choose approvers in `who`"))
                                    .note(tr!("規則の出力で承認者を決めるときは、出力の値ごとに、同じ名前の承認者の決め方を `who` に書きます。", "When a rule's output names the approver, `who` declares a way to choose approvers by the name of each value of the output."))
                                    .note(tr!("`who` に、`{first} = …` のように足してください。", "Add it to `who`, as in `{first} = …`.")),
                            );
                        }
                    }
                }
            }
        }
        Spec::Route(w) => {
            if !s.is_alias(&w.text, UseKind::Route) {
                let n = &w.text;
                out.push(undeclared(s, w.span, tr!("`{n}` は `use route` の別名ではありません", "`{n}` is not the alias of a `use route`"), tr!("共有の経路の別名", "aliases of shared routes"), &aliases_of(s, &[UseKind::Route])));
            }
        }
    }
}

/// E004 for a calendar named with `in`, and for business days counted with no calendar to count
/// them in (none, or two and no `in`).
pub fn calendar_ref(s: &Scope, cal: Option<&Word>, at: Span, out: &mut Vec<Diag>) {
    match cal {
        Some(w) => {
            if !s.is_alias(&w.text, UseKind::Calendar) {
                let n = &w.text;
                out.push(undeclared(s, w.span, tr!("`{n}` は `use calendar` の別名ではありません", "`{n}` is not the alias of a `use calendar`"), tr!("カレンダーの別名", "aliases of calendars"), &aliases_of(s, &[UseKind::Calendar])));
            }
        }
        None => {
            let cals = s.calendars();
            if cals.is_empty() {
                out.push(s.err("E004", at, tr!("営業日を数えるカレンダーがありません", "There is no calendar to count the business days in")).note(tr!(
                    "営業日はカレンダーの開いている日で数えます。`use calendar office from \"office.cal\"` のように、カレンダーを読んでください。",
                    "Business days are the open days of a calendar: read one, as in `use calendar office from \"office.cal\"`."
                )));
            } else if cals.len() > 1 {
                let names: Vec<String> = cals.iter().filter_map(|u| u.alias.as_ref().map(|a| a.text.clone())).collect();
                let first = names.first().cloned().unwrap_or_default();
                out.push(s.err("E004", at, tr!("カレンダーが二つ以上あるので、どれで営業日を数えるかを `in` で書いてください", "There are two calendars or more: say which counts the business days, with `in`")).note(tr!(
                    "`in {first}` のように書いてください。",
                    "Write it as `in {first}`."
                )));
            }
        }
    }
}

fn deadline_refs(s: &Scope, d: &Deadlines, out: &mut Vec<Diag>) {
    for l in &d.lines {
        match l {
            Deadline::Due { calendar, line, .. } | Deadline::Remind { calendar, line, .. } => calendar_ref(s, calendar.as_ref(), Span { line: *line, col: 1, end: 2 }, out),
            Deadline::Late { first, then, .. } => {
                for late in std::iter::once(first).chain(then.iter()) {
                    if let Late::Escalate(Some(t)) = late
                        && !s.file.tables.iter().any(|x| x.name.text == t.text)
                    {
                        let n = &t.text;
                        let names: Vec<String> = s.file.tables.iter().map(|x| x.name.text.clone()).collect();
                        out.push(undeclared(s, t.span, tr!("`{n}` という表はありません", "There is no table `{n}`"), tr!("表", "tables"), &names));
                    }
                }
            }
            Deadline::Empty { what, .. } => {
                // the way it goes to where nobody is above, declared in this file's `who`
                if let Some(w) = what.at_top_way() {
                    who_ref(s, w, false, out);
                }
            }
        }
    }
}

/// A master of the file a `use masters` alias reads, by `alias.master`: E004 where it is not one.
/// None, said, or not known (the file could not be read).
pub fn master<'s>(s: &'s Scope, alias: &Word, name: &Word, out: &mut Vec<Diag>) -> Option<&'s Master> {
    let a = &alias.text;
    if !s.is_alias(a, UseKind::Masters) {
        out.push(undeclared(s, alias.span, tr!("`{a}` は `use masters` の別名ではありません", "`{a}` is not the alias of a `use masters`"), tr!("マスタの別名", "aliases of masters"), &aliases_of(s, &[UseKind::Masters])));
        return None;
    }
    let f = s.ringi(a, UseKind::Masters)?;
    let m = f.masters.iter().find(|m| m.name.text == name.text);
    if m.is_none() {
        let n = &name.text;
        let names: Vec<String> = f.masters.iter().map(|m| m.name.text.clone()).collect();
        out.push(undeclared(s, name.span, tr!("`{a}` に `{n}` というマスタはありません", "`{a}` has no master `{n}`"), tr!("マスタ", "masters"), &names));
    }
    m
}

/// E004 and E007 for everything the file names but its expressions (`types.rs` reads those).
pub fn references(s: &Scope, out: &mut Vec<Diag>) {
    let f = s.file;
    // a `choice from`, an `open in`, in the fields and in the fields of each row
    let mut all: Vec<&Field> = Vec::new();
    for fd in s.fields() {
        all.push(fd);
        all.extend(fd.children.iter());
    }
    for fd in all {
        if let TypeExpr::ChoiceFrom(p) = &fd.ty {
            let n = &p[0].text;
            if s.is_alias(n, UseKind::Masters) {
                if p.len() != 2 {
                    let whole = dotted(p);
                    out.push(s.err("E004", p[0].span, tr!("`{whole}` はマスタではありません。マスタは `m.accounts` のように書きます", "`{whole}` is no master: a master is written as `m.accounts`")));
                } else {
                    master(s, &p[0], &p[1], out);
                }
                // a master's rows are its data: the screens show each by its row
                if let Some(t) = fd.value_titles.first() {
                    let whole = dotted(p);
                    out.push(s.err("E001", t.name.span, tr!("マスタから選ぶ `{whole}` の下には、行を書けません。値はマスタの行です", "A `choice from` the master `{whole}` has no lines under it: its values are the master's rows")));
                }
            } else if s.is_alias(n, UseKind::Rule) {
                if p.len() != 2 {
                    let whole = dotted(p);
                    out.push(s.err("E004", p[0].span, tr!("`{whole}` は規則の列挙ではありません。規則の列挙は `authority.kind` のように書きます", "`{whole}` is no enum of a rule: it is written as `authority.kind`")));
                } else if let Some(facts) = s.rule_facts(n) {
                    match facts.enums.iter().find(|e| e.name == p[1].text || e.public == p[1].text) {
                        Some(e) => value_titles(s, fd, e, &dotted(p), out),
                        None => {
                            let whole = dotted(p);
                            let names: Vec<String> = facts.enums.iter().map(|e| e.name.clone()).collect();
                            out.push(undeclared(s, p[1].span, tr!("規則 `{n}` に、列挙 `{whole}` はありません", "The rule `{n}` has no enum `{whole}`"), tr!("規則の列挙", "enums of the rule"), &names));
                        }
                    }
                }
            } else {
                out.push(undeclared(
                    s,
                    p[0].span,
                    tr!("`{n}` は `use masters` か `use rule` の別名ではありません", "`{n}` is not the alias of a `use masters` or a `use rule`"),
                    tr!("マスタと規則の別名", "aliases of masters and rules"),
                    &aliases_of(s, &[UseKind::Masters, UseKind::Rule]),
                ));
            }
        }
        if let Some(c) = &fd.open_in {
            calendar_ref(s, Some(c), c.span, out);
        }
    }
    // the route
    match &f.route {
        Some(Route::Table(t)) => {
            for col in &t.inputs {
                let n = &col[0].text;
                let field = s.field(n);
                // who makes the request, and the ticket, are no column of a table: E309 says so
                // where the route is lowered
                if field.is_none() && matches!(n.as_str(), "requester" | "ticket") {
                    continue;
                }
                if field.is_none() && !s.is_alias(n, UseKind::Rule) {
                    out.push(undeclared(
                        s,
                        col[0].span,
                        tr!("経路の表の列 `{n}` は、フィールドでも規則の別名でもありません", "The column `{n}` of the route is neither a field nor the alias of a rule"),
                        tr!("フィールド", "fields"),
                        &s.fields().iter().map(|x| x.name.text.clone()).collect::<Vec<_>>(),
                    ));
                    continue;
                }
                let whole = col.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(".");
                match (field, col.len()) {
                    (Some(_), 1) => {}
                    (Some(_), _) => {
                        let x = &col[1].text;
                        out.push(s.err("E004", col[1].span, tr!("`{n}` に `{x}` はありません", "`{n}` has no `{x}`")));
                    }
                    (None, 2) => {
                        if let Some(facts) = s.rule_facts(n)
                            && !facts.outputs.iter().any(|o| o.name == col[1].text || o.alias == col[1].text)
                        {
                            let names: Vec<String> = facts.outputs.iter().map(|o| o.name.clone()).collect();
                            out.push(undeclared(s, col[1].span, tr!("規則 `{n}` に、出力 `{whole}` はありません", "The rule `{n}` has no output `{whole}`"), tr!("規則の出力", "outputs of the rule"), &names));
                        }
                    }
                    (None, 1) => {
                        out.push(s.err("E004", col[0].span, tr!("`{n}` は規則の別名です。経路の表の列には、`{n}.<出力>` のように規則の出力を書いてください", "`{n}` is the alias of a rule: a column of the route reads one of its outputs, as in `{n}.<output>`")));
                    }
                    (None, _) => {
                        out.push(s.err("E004", col[2].span, tr!("`{whole}` は読めません。規則の出力は `{n}.<出力>` と書きます", "`{whole}` cannot be read: a rule's output is written `{n}.<output>`")));
                    }
                }
            }
            for r in &t.rows {
                for (spec, _) in &r.stages {
                    spec_refs(s, spec, false, out);
                }
            }
        }
        Some(Route::Stages(lines)) => {
            for l in lines {
                spec_refs(s, &l.spec, false, out);
                if let Some(d) = &l.deadlines {
                    deadline_refs(s, d, out);
                }
            }
        }
        Some(Route::Shared { alias, .. }) => {
            if !s.is_alias(&alias.text, UseKind::Route) {
                let n = &alias.text;
                out.push(undeclared(s, alias.span, tr!("`{n}` は `use route` の別名ではありません", "`{n}` is not the alias of a `use route`"), tr!("共有の経路の別名", "aliases of shared routes"), &aliases_of(s, &[UseKind::Route])));
            }
        }
        None => {}
    }
    // the ways to choose an approver
    for w in &f.who {
        who_refs(s, w, out);
    }
    if let Some(d) = &f.every_stage {
        deadline_refs(s, d, out);
    }
    // the tables: a column that names a field holds its values, another the ways to choose
    for t in &f.tables {
        // a column named as a field whose rows name ways to choose an approver (E002)
        for (k, col) in t.inputs.iter().enumerate() {
            if !ways_in_field_column(s, t, k) {
                continue;
            }
            let (c, line) = (&col.text, s.field(&col.text).map(|x| x.name.span.line).unwrap_or(0));
            out.push(
                s.err(
                    "E002",
                    col.span,
                    tr!(
                        "列 `{c}` はフィールド `{c}`（{line} 行目）と同じ名前なので、そのフィールドの値を書く列ですが、行に承認者の決め方を書いています",
                        "The column `{c}` has the name of the field `{c}` (line {line}), so it holds the field's values, and its rows name ways to choose an approver"
                    ),
                )
                .note(tr!(
                    "名前の付いた表の入力の列は、名前がフォームのフィールドなら、そのフィールドの値を書く列で、そうでなければ承認者の決め方の名前を書く列です。フィールドの名前はチケットの JSON のキーなので、ほかの名前に置き換えられません。承認者の決め方を書く列に、どのフィールドにも無い名前を付けてください（`| current | -> goes_to |`）。",
                    "An input column of a named table holds the values of the field of the form it is named as, and any other the names of the ways to choose an approver. A field's name is a key of the JSON of a ticket, and cannot be written under another name: give the column of the ways a name no field has (`| current | -> goes_to |`)."
                )),
            );
        }
        for r in &t.rows {
            for (k, (test, _)) in r.tests.iter().enumerate() {
                let Some(col) = t.inputs.get(k) else { continue };
                if s.field(&col.text).is_some() {
                    continue;
                }
                if let Test::Values { values, .. } = test {
                    for v in values {
                        match &v.kind {
                            ExprKind::Path(p) if p.len() == 1 => {
                                who_ref(s, &p[0], false, out);
                            }
                            _ => out.push(s.err("E004", v.span, tr!("この列には `who` の名前を書きます", "This column holds names declared in `who`"))),
                        }
                    }
                }
            }
            for (spec, _) in &r.stages {
                spec_refs(s, spec, false, out);
            }
        }
    }
    // the ticket
    if let Some(t) = &f.ticket {
        ticket_refs(s, t, out);
    }
    // claims, examples and scenarios name the ways to choose of the shared routes too
    for c in &f.claims {
        match &c.body {
            ClaimBody::OnlyWhen { spec, .. } | ClaimBody::Possible { spec, .. } => spec_refs(s, spec, true, out),
            _ => {}
        }
    }
    if let Some(e) = &f.examples {
        for col in &e.inputs {
            let n = &col[0].text;
            if s.field(n).is_none() && !s.is_alias(n, UseKind::Rule) {
                out.push(undeclared(
                    s,
                    col[0].span,
                    tr!("例の列 `{n}` は、フィールドでも規則の別名でもありません", "The column `{n}` of the examples is neither a field nor the alias of a rule"),
                    tr!("フィールド", "fields"),
                    &s.fields().iter().map(|x| x.name.text.clone()).collect::<Vec<_>>(),
                ));
            }
        }
        for r in &e.rows {
            for (spec, _) in &r.stages {
                spec_refs(s, spec, true, out);
            }
        }
    }
    for sc in &f.scenarios {
        for g in &sc.given {
            book_call(s, g, "given", out);
        }
        for (n, _) in &sc.form {
            if s.field(&n.text).is_none() {
                let w = &n.text;
                out.push(undeclared(s, n.span, tr!("`{w}` というフィールドはありません", "There is no field `{w}`"), tr!("フィールド", "fields"), &s.fields().iter().map(|x| x.name.text.clone()).collect::<Vec<_>>()));
            }
        }
    }
    // the labels
    for l in &f.labels {
        for e in &l.entries {
            label_ref(s, e, out);
        }
    }
    // a master's key
    for m in &f.masters {
        if let Some(k) = &m.key
            && !m.fields.iter().any(|x| x.name.text == k.text)
        {
            let n = &k.text;
            out.push(undeclared(s, k.span, tr!("キー `{n}` は、このマスタのフィールドではありません", "The key `{n}` is not a field of this master"), tr!("フィールド", "fields"), &m.fields.iter().map(|x| x.name.text.clone()).collect::<Vec<_>>()));
        }
    }
}

fn need_org(s: &Scope, at: Span, how: &str, out: &mut Vec<Diag>) -> Option<Option<()>> {
    match s.org() {
        None => {
            out.push(s.err("E004", at, tr!("`{how}` は組織を読みますが、このファイルに `use organization` がありません", "`{how}` reads the organization, and the file has no `use organization`")).note(tr!(
                "`use organization org from \"company.ringi\"` のように、組織のファイルを読んでください。",
                "Read the organization's file, as in `use organization org from \"company.ringi\"`."
            )));
            None
        }
        Some(None) => Some(None),
        Some(Some(_)) => Some(Some(())),
    }
}

/// E004 for what a way to choose an approver reads.
fn who_refs(s: &Scope, w: &Who, out: &mut Vec<Diag>) {
    let line_at = Span { line: w.line, col: w.name.span.col, end: w.name.span.end };
    match &w.how {
        How::Person(..) | How::Team(_) => {}
        How::ManagerOf => {
            need_org(s, line_at, "manager of requester", out);
        }
        How::HeadOf(k) => {
            if need_org(s, line_at, "head of requester", out) == Some(Some(())) {
                kind_ref(s, k, out);
            }
        }
        How::Nearest(p) => {
            if need_org(s, line_at, "nearest", out) == Some(Some(()))
                && let Some(Some(org)) = s.org()
                && !org.positions.iter().any(|x| x.name.text == p.text)
            {
                let n = &p.text;
                let names: Vec<String> = org.positions.iter().map(|x| x.name.text.clone()).collect();
                out.push(undeclared(s, p.span, tr!("`{n}` は組織の役職ではありません", "`{n}` is not a position of the organization"), tr!("役職", "positions"), &names));
            }
        }
        How::ChosenBy { field, .. } => {
            if s.field(&field.text).is_none() {
                let n = &field.text;
                out.push(undeclared(s, field.span, tr!("`{n}` というフィールドはありません", "There is no field `{n}`"), tr!("フィールド", "fields"), &s.fields().iter().map(|x| x.name.text.clone()).collect::<Vec<_>>()));
            }
        }
        How::Lookup { path, by } => {
            if path.len() != 3 {
                let whole = dotted(path);
                out.push(s.err("E004", path[0].span, tr!("`{whole}` はマスタのフィールドではありません。`m.systems.owner` のように書いてください", "`{whole}` is no field of a master: write it as `m.systems.owner`")));
            } else if let Some(m) = master(s, &path[0], &path[1], out)
                && !m.fields.iter().any(|x| x.name.text == path[2].text)
            {
                let (n, mn) = (&path[2].text, &m.name.text);
                out.push(undeclared(s, path[2].span, tr!("マスタ `{mn}` に `{n}` というフィールドはありません", "The master `{mn}` has no field `{n}`"), tr!("フィールド", "fields"), &m.fields.iter().map(|x| x.name.text.clone()).collect::<Vec<_>>()));
            }
            if s.field(&by.text).is_none() {
                let n = &by.text;
                out.push(undeclared(s, by.span, tr!("`{n}` というフィールドはありません", "There is no field `{n}`"), tr!("フィールド", "fields"), &s.fields().iter().map(|x| x.name.text.clone()).collect::<Vec<_>>()));
            }
        }
        How::Agent { flow, otherwise } => {
            if !s.is_alias(&flow.text, UseKind::Flow) {
                let n = &flow.text;
                out.push(undeclared(s, flow.span, tr!("`{n}` は `use flow` の別名ではありません", "`{n}` is not the alias of a `use flow`"), tr!("flow の別名", "aliases of flows"), &aliases_of(s, &[UseKind::Flow])));
            }
            who_ref(s, otherwise, false, out);
        }
    }
}

/// E004 for a kind of team the organization does not have (`requester.department`).
pub fn kind_ref(s: &Scope, k: &Word, out: &mut Vec<Diag>) {
    if let Some(Some(org)) = s.org()
        && !org.team_kinds.iter().any(|x| x.name.text == k.text)
    {
        let n = &k.text;
        let names: Vec<String> = org.team_kinds.iter().map(|x| x.name.text.clone()).collect();
        out.push(undeclared(s, k.span, tr!("`{n}` は組織のチームの種類ではありません", "`{n}` is not a kind of team of the organization"), tr!("チームの種類", "kinds of team"), &names));
    }
}

/// E004 for a call of a book's transfer: `budget.reserve(…)`.
fn book_call(s: &Scope, c: &Call, word: &str, out: &mut Vec<Diag>) {
    let n = &c.target[0].text;
    if !s.is_alias(n, UseKind::Book) {
        out.push(undeclared(s, c.target[0].span, tr!("`{n}` は `use book` の別名ではありません", "`{n}` is not the alias of a `use book`"), tr!("帳簿の別名", "aliases of books"), &aliases_of(s, &[UseKind::Book])));
    } else if c.target.len() != 2 {
        let whole = dotted(&c.target);
        out.push(s.err("E004", c.target[0].span, tr!("`{word}` には帳簿の振替を `budget.reserve(…)` のように書きます。`{whole}` はそうではありません", "`{word}` takes a transfer of a book, as in `budget.reserve(…)`, which `{whole}` is not")));
    }
}

fn ticket_refs(s: &Scope, t: &Ticket, out: &mut Vec<Diag>) {
    for l in &t.lines {
        match l {
            TicketLine::Resubmit { calendar, line, .. } | TicketLine::Pause { calendar, line, .. } => calendar_ref(s, calendar.as_ref(), Span { line: *line, col: 1, end: 2 }, out),
            TicketLine::ApproversEdit { fields, .. } => {
                for w in fields {
                    if s.field(&w.text).is_none() {
                        let n = &w.text;
                        out.push(undeclared(s, w.span, tr!("`{n}` というフィールドはありません", "There is no field `{n}`"), tr!("フィールド", "fields"), &s.fields().iter().map(|x| x.name.text.clone()).collect::<Vec<_>>()));
                    }
                }
            }
            TicketLine::SubmittedBy { groups, except, line, col } => {
                let at = Span { line: *line, col: *col, end: col + "submitted".len() };
                if need_org(s, at, "submitted by", out) != Some(Some(())) {
                    continue;
                }
                let Some(Some(org)) = s.org() else { continue };
                for g in groups.iter().chain(except) {
                    if let Group::Position { position: p, .. } = g
                        && !org.positions.iter().any(|x| x.name.text == p.text)
                    {
                        let n = &p.text;
                        let names: Vec<String> = org.positions.iter().map(|x| x.name.text.clone()).collect();
                        out.push(undeclared(s, p.span, tr!("`{n}` は組織の役職ではありません", "`{n}` is not a position of the organization"), tr!("役職", "positions"), &names));
                    }
                }
            }
            TicketLine::On { action, call, .. } => match action {
                ActionKind::Launch => {
                    let n = &call.target[0].text;
                    if call.target.len() != 1 || !s.is_alias(n, UseKind::Flow) {
                        let whole = dotted(&call.target);
                        out.push(undeclared(s, call.target[0].span, tr!("`launch` が始めるのは flow です。`{whole}` は `use flow` の別名ではありません", "`launch` starts a flow, and `{whole}` is not the alias of a `use flow`"), tr!("flow の別名", "aliases of flows"), &aliases_of(s, &[UseKind::Flow])));
                    }
                }
                a => book_call(s, call, a.word(), out),
            },
            _ => {}
        }
    }
}

/// The value of the rule's enum `e` a line under a `choice from` names: by its name or its public
/// name, as the rule writes them.
fn enum_value<'e>(e: &'e ritsu_ports::RuleEnum, n: &Name) -> Option<&'e ritsu_ports::EnumValue> {
    e.values.iter().find(|v| v.name == n.text || v.public == n.text || v.public == n.ascii())
}

/// The rule's enum a `choice from` chooses from (`authority.kind`); None for any other field, and
/// where the rule is not read.
pub fn enum_of<'s>(s: &'s Scope, f: &Field) -> Option<&'s ritsu_ports::RuleEnum> {
    let TypeExpr::ChoiceFrom(p) = &f.ty else { return None };
    let [a, e] = p.as_slice() else { return None };
    if !s.is_alias(&a.text, UseKind::Rule) {
        return None;
    }
    s.rule_facts(&a.text)?.enums.iter().find(|x| x.name == e.text || x.public == e.text)
}

/// The titles under a `choice from` the rule's enum `e` (`whole`, DESIGN 4.2): each of a value of
/// the enum (E004), and of each value once (E003).
fn value_titles(s: &Scope, fd: &Field, e: &ritsu_ports::RuleEnum, whole: &str, out: &mut Vec<Diag>) {
    for (i, t) in fd.value_titles.iter().enumerate() {
        let v = &t.name.text;
        let Some(value) = enum_value(e, &t.name) else {
            let names: Vec<String> = e.values.iter().map(|x| x.name.clone()).collect();
            out.push(undeclared(s, t.name.span, tr!("列挙 `{whole}` に、値 `{v}` はありません", "The enum `{whole}` has no value `{v}`"), tr!("列挙の値", "values of the enum"), &names));
            continue;
        };
        if let Some(first) = fd.value_titles[..i].iter().find(|x| enum_value(e, &x.name).is_some_and(|y| std::ptr::eq(y, value))) {
            let line = first.line;
            out.push(s.err("E003", t.name.span, tr!("`{v}` の題は二度書いてあります（最初は {line} 行目）", "The title of `{v}` is written twice (first on line {line})")));
        }
    }
}

/// E007: a label for nothing.
fn label_ref(s: &Scope, e: &Label, out: &mut Vec<Diag>) {
    let key = label_key(&e.key);
    let nothing = |what: Text| s.err("E007", e.span, tr!("`{key}` の文言が指すものがありません。{}", "The label `{key}` is for nothing: {}", what.ja; what.en));
    match &e.key {
        LabelKey::Request => {
            if s.file.kind != FileKind::Request {
                out.push(nothing(tr!("このファイルは申請の種類ではありません", "this file is not a request")));
            }
        }
        LabelKey::Field(p) => {
            let Some(f) = s.field(&p[0].text) else {
                let n = &p[0].text;
                out.push(nothing(tr!("`{n}` というフィールドはありません", "there is no field `{n}`")));
                return;
            };
            // a value of a `choice`, or of the rule's enum a `choice from` chooses from
            let value = |f: &Field, w: &Word| f.options.iter().any(|o| o.name.text == w.text) || enum_of(s, f).is_some_and(|e| e.values.iter().any(|v| v.name == w.text || v.public == w.text));
            let ok = match p.len() {
                1 => true,
                2 => value(f, &p[1]) || f.children.iter().any(|c| c.name.text == p[1].text),
                3 => f.children.iter().find(|c| c.name.text == p[1].text).is_some_and(|c| value(c, &p[2])),
                _ => false,
            };
            if !ok {
                let n = &f.name.text;
                out.push(nothing(tr!("`{n}` にその値も行のフィールドもありません", "`{n}` has no such value and no such field of its row")));
            }
        }
        LabelKey::Stage(w) => {
            if let Some(stages) = s.stages()
                && !stages.iter().any(|n| n.text == w.text)
            {
                let n = &w.text;
                out.push(nothing(tr!("`{n}` というステップは経路にありません", "the route has no stage `{n}`")));
            }
        }
        LabelKey::Who(w) => {
            if !s.who_all().iter().any(|x| x.who.name.text == w.text) {
                let n = &w.text;
                out.push(nothing(tr!("`{n}` は `who` に宣言がありません", "`{n}` is not declared in `who`")));
            }
        }
    }
}
