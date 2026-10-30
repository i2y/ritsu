//! ritsu's ports, as rulec answers them (ritsu's DESIGN 3.2): [`Engine`] implements the port of
//! rules (`ritsu_ports::Rules`), and gives what a rule file holds and what it names outside
//! itself (`Items`, `References`). A program that holds rulec as a library — dandori, yuen, sakai,
//! ritsu's checks across languages — reads a rule through these, with the units the JSON of
//! `rulec api` only describes in words, instead of running rulec and reading its JSON.
//!
//! Nothing here decides anything rulec's check does not: the facts are read off the checked rule
//! by the same helpers `api`, `certificate` and `schema` write their JSON from, and the page is
//! the one `doc` draws.

use crate::ast::{Item as AstItem, Name, RuleFile, SourceKind};
use crate::codegen::pub_name_of;
use crate::i18n::{self, Lang as RLang};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use ritsu_base::naming::{Name as Naming, Tool};
use ritsu_base::text::{Lang, Text};
use ritsu_ports::{Answer, Column, ColumnType, DaySet, Item, Precondition, Reference, RuleError, RuleFacts, Said, Value, Values};
use std::path::Path;

/// rulec, as the ports reach it. It keeps each rule it has checked, by its path and its text, so
/// that a program asking one rule many times (an evaluation per call of a workflow) checks it
/// once; and the report `rulec check` prints of it, so that `ritsu check`, which prints that report
/// and then asks the rule's facts for the workflows that use it, checks it once too. A check is
/// kept while every file it read reads the same ([`Seen`]): a rule of the same text whose
/// enums, callee or contributions read otherwise (the other side of a diff, a git revision, the
/// module at hand with another lock) is checked again, and both checks are kept.
#[derive(Default)]
pub struct Engine {
    /// By the path and the text: the checks kept, each with what its files read.
    checked: std::sync::Mutex<std::collections::HashMap<(String, String), Vec<Kept>>>,
    /// By the file (its path with its links followed, so two spellings of one path are one file)
    /// and its text: the path the report was made with, its language, what its files read, and the
    /// report.
    reports: std::sync::Mutex<std::collections::HashMap<(std::path::PathBuf, String), (String, RLang, std::sync::Arc<Seen>, std::sync::Arc<crate::Report>)>>,
    /// How many times a rule's text has been checked whole (what [`Engine::checks`] says).
    checks: std::sync::atomic::AtomicUsize,
    /// The port of dates a rule's `range from koyomi` is read through (§15.174); None where no
    /// koyomi is joined.
    dates: Option<crate::days::Port>,
    /// The most nodes one check of a rule visits (`--budget`), when it is not the default
    /// (§15.209).
    budget: Option<i64>,
    /// The files of the project, as the person gave them: the contributions among them are merged
    /// into the rules they reach (§15.231). `None` outside a project.
    set: std::sync::Mutex<Option<Vec<String>>>,
}

/// The file a path names, its links followed and `..` folded: how the reports are kept.
fn the_file(path: &str) -> std::path::PathBuf {
    ritsu_base::fs::canonicalize(path).unwrap_or_else(|_| std::path::PathBuf::from(path))
}

/// A rule read and checked, or what check said of it.
type Checked2 = std::sync::Arc<Result<(RuleFile, Checked), Vec<Said>>>;

/// A check kept, with what the files it read came to.
type Kept = (std::sync::Arc<Seen>, Checked2);

/// How many checks of one text are kept: the rule read through as many sets of files in one run
/// (the two sides of a diff, the base and the head of `ritsu mod report`).
const KEPT_EACH: usize = 4;

/// What a check read through `ritsu_base::fs`, each question once, with its answer: the files a
/// rule reads besides itself (what it imports and applies, the contributions to it, the days of a
/// koyomi date) at the paths the files of the thread were asked for, and the files that say where a
/// module's dependencies are (its `ritsu.mod`, its lock) that the view of them read.
#[derive(Default)]
struct Seen(Vec<Look>);

/// A question a check asked the files, and the answer (None where there was nothing).
#[derive(Clone, PartialEq)]
enum Look {
    Read(std::path::PathBuf, Option<Vec<u8>>),
    Meta(std::path::PathBuf, Option<ritsu_base::fs::Meta>),
    Dir(std::path::PathBuf, Option<Vec<(std::ffi::OsString, ritsu_base::fs::Kind)>>),
    Canonical(std::path::PathBuf, Option<std::path::PathBuf>),
}

impl Look {
    /// The question, to ask it once.
    fn question(&self) -> (u8, &Path) {
        match self {
            Look::Read(p, _) => (0, p),
            Look::Meta(p, _) => (1, p),
            Look::Dir(p, _) => (2, p),
            Look::Canonical(p, _) => (3, p),
        }
    }

    /// Whether the files the thread reads now answer the question the same.
    fn holds(&self) -> bool {
        use ritsu_base::fs::raw;
        match self {
            Look::Read(p, was) => raw::read(p).ok() == *was,
            Look::Meta(p, was) => raw::metadata(p).ok() == *was,
            Look::Dir(p, was) => raw::read_dir(p).ok() == *was,
            Look::Canonical(p, was) => raw::canonicalize(p).ok() == *was,
        }
    }
}

impl Seen {
    /// Whether every file the check read reads the same now.
    fn still(&self) -> bool {
        self.0.iter().all(Look::holds)
    }

    /// What a rule of the text `src` checked with these reads rests on ([`ritsu_ports::Rules::rests_on`]):
    /// a digest of the text and of every file read with its bytes, in the order of the paths.
    fn rests_on(&self, src: &str) -> String {
        let mut read: Vec<(&Path, &Option<Vec<u8>>)> = self.0.iter().filter_map(|l| if let Look::Read(p, b) = l { Some((p.as_path(), b)) } else { None }).collect();
        read.sort_by(|a, b| a.0.cmp(b.0));
        read.dedup_by(|a, b| a.0 == b.0);
        fn put(all: &mut Vec<u8>, b: &[u8]) {
            all.extend_from_slice(&(b.len() as u64).to_le_bytes());
            all.extend_from_slice(b);
        }
        let mut all = Vec::new();
        put(&mut all, src.as_bytes());
        for (p, b) in read {
            put(&mut all, p.as_os_str().as_encoded_bytes());
            match b {
                Some(b) => {
                    all.push(1);
                    put(&mut all, b);
                }
                None => all.push(0),
            }
        }
        crate::sha256::hex(&all)
    }
}

/// The files a check reads through: every call handed on to what the thread read before (the disk,
/// or the files of an outer `ritsu_base::fs::with`), each question noted once with its answer. It
/// stands for the files it hands on to ([`ritsu_base::fs::Files::stands_for`]), so the tree of a
/// module kept for those is read here too.
struct Recorder {
    under: Option<std::rc::Rc<dyn ritsu_base::fs::Files>>,
    stands: usize,
    seen: std::cell::RefCell<(Vec<Look>, std::collections::HashSet<(u8, std::path::PathBuf)>)>,
}

impl Recorder {
    fn files(&self) -> &dyn ritsu_base::fs::Files {
        match &self.under {
            Some(f) => f.as_ref(),
            None => &ritsu_base::fs::Disk,
        }
    }

    fn note(&self, look: Look) {
        let mut s = self.seen.borrow_mut();
        let (kind, p) = look.question();
        if s.1.insert((kind, p.to_path_buf())) {
            s.0.push(look);
        }
    }
}

impl ritsu_base::fs::Files for Recorder {
    fn read(&self, p: &Path) -> std::io::Result<Vec<u8>> {
        let got = self.files().read(p);
        self.note(Look::Read(p.to_path_buf(), got.as_ref().ok().cloned()));
        got
    }

    fn metadata(&self, p: &Path) -> std::io::Result<ritsu_base::fs::Meta> {
        let got = self.files().metadata(p);
        self.note(Look::Meta(p.to_path_buf(), got.as_ref().ok().copied()));
        got
    }

    fn read_dir(&self, p: &Path) -> std::io::Result<Vec<(std::ffi::OsString, ritsu_base::fs::Kind)>> {
        let got = self.files().read_dir(p);
        self.note(Look::Dir(p.to_path_buf(), got.as_ref().ok().cloned()));
        got
    }

    fn write(&self, p: &Path, bytes: &[u8]) -> std::io::Result<()> {
        self.files().write(p, bytes)
    }

    fn create_dir_all(&self, p: &Path) -> std::io::Result<()> {
        self.files().create_dir_all(p)
    }

    fn current_dir(&self) -> std::io::Result<std::path::PathBuf> {
        self.files().current_dir()
    }

    fn canonicalize(&self, p: &Path) -> std::io::Result<std::path::PathBuf> {
        let got = self.files().canonicalize(p);
        self.note(Look::Canonical(p.to_path_buf(), got.as_ref().ok().cloned()));
        got
    }

    fn revision(&self) -> Option<String> {
        self.files().revision()
    }

    fn stands_for(&self) -> Option<usize> {
        Some(self.stands)
    }
}

/// `f` run with every read of `ritsu_base::fs` noted: what it came to, and what it read.
fn watched<R>(f: impl FnOnce() -> R) -> (R, Seen) {
    let rec = std::rc::Rc::new(Recorder { under: ritsu_base::fs::current(), stands: ritsu_base::fs::id(), seen: Default::default() });
    let got = ritsu_base::fs::with(rec.clone(), f);
    let (looks, _) = rec.seen.take();
    (got, Seen(looks))
}

fn rlang(l: Lang) -> RLang {
    match l {
        Lang::Ja => RLang::Ja,
        Lang::En => RLang::En,
    }
}

/// The file's text, as the caller named it.
fn read(rule: &Path) -> Result<(String, String), Vec<Said>> {
    let path = rule.to_string_lossy().to_string();
    match ritsu_base::fs::read_to_string(rule) {
        Ok(src) => Ok((path, src)),
        Err(e) => Err(vec![Said::unreadable(&path, &e.to_string())]),
    }
}

/// What check says of a rule that does not pass, each error in both languages.
fn refused(path: &str, src: &str) -> Vec<Said> {
    let ja = i18n::with(RLang::Ja, || crate::report(src, path).diags);
    let en = i18n::with(RLang::En, || crate::report(src, path).diags);
    ja.iter()
        .zip(&en)
        .filter(|(d, _)| d.severity == crate::diag::Severity::Error)
        .map(|(j, e)| Said { code: j.code.to_string(), file: path.to_string(), line: j.line(), message: Text::new(j.title.clone(), e.title.clone()) })
        .collect()
}

/// The rule, when it passes the whole of check (as `api` and `certificate` ask of it), the check
/// held to `budget`.
fn checked(path: &str, src: &str, budget: i64) -> Result<(RuleFile, Checked), Vec<Said>> {
    let rep = crate::report_with(src, path, budget);
    if crate::has_error(&rep.diags) {
        return Err(refused(path, src));
    }
    crate::prepare(src, path).map_err(|_| refused(path, src))
}

/// A value's type as the port hands it over: a number with its unit and its range on the wire, an
/// enum with the values its input's range lets in (§15.210, §15.251). An input's range is the one
/// the entry guard holds it to; an output's is the one its answers lie in, as `output_values`, the
/// certificate's `output_ranges` and the schema say (§15.271).
fn column_type(c: &Checked, name: &str, ty: &Ty) -> ColumnType {
    match ty {
        Ty::Bool => ColumnType::Bool,
        Ty::Str => ColumnType::Str,
        Ty::Date => ColumnType::Date,
        Ty::Enum(e) => ColumnType::Enum { name: e.clone(), range: c.enum_ranges.get(name).cloned() },
        Ty::Opt(t) => ColumnType::Opt(Box::new(column_type(c, name, t))),
        _ => {
            let step = matches!(ty, Ty::Rate).then(|| Rat::new(1, c.wire_scale(name)));
            let (min, max) = c.wire_range(name).unwrap_or((None, None));
            ColumnType::Num { written: ty.to_string(), unit: ty.unit(step), min, max }
        }
    }
}

fn column(c: &Checked, n: &Name) -> Column {
    let ty = c.ty_of(&n.text).unwrap_or(Ty::Unknown);
    Column { name: n.text.clone(), alias: pub_name_of(n), ty: column_type(c, &n.text, &ty) }
}

fn preconditions(f: &RuleFile, c: &Checked) -> Vec<Precondition> {
    crate::verify::preconditions(f, c)
        .into_iter()
        .map(|p| match p {
            crate::verify::Pre::Rel { left, op, right } => match (left.as_slice(), right.as_slice()) {
                ([l], [r]) => Precondition::Relation { left: l.clone(), op: op.to_string(), right: r.clone() },
                _ => Precondition::SumRelation { left, op: op.to_string(), right },
            },
            crate::verify::Pre::Sum { name, over, of, max } => Precondition::Sum { name, over, of, max },
            crate::verify::Pre::Length { sequence, max } => Precondition::Length { sequence, max },
            crate::verify::Pre::Days { input, file, date, days } => Precondition::Days { input, file, date, days: days.into_iter().collect() },
        })
        .collect()
}

impl Engine {
    pub fn new() -> Engine {
        Engine::default()
    }

    /// The engine with koyomi joined: a rule whose range is a date of a koyomi file reads its
    /// days through `dates` (§15.174).
    pub fn with_dates(dates: crate::days::Port) -> Engine {
        Engine { dates: Some(dates), ..Engine::default() }
    }

    /// The same engine, with every check of a rule held to `budget` nodes, the region checks and
    /// the machine's alike: what `ritsu check --budget` passes on (§15.209).
    pub fn with_budget(mut self, budget: i64) -> Engine {
        self.budget = Some(budget);
        self
    }

    /// The budget a check of a rule is held to.
    fn budget(&self) -> i64 {
        self.budget.unwrap_or(crate::region::DEFAULT_BUDGET)
    }

    /// The project the engine answers for (§15.231): the contributions among `files` are merged into
    /// the rules they reach, in every check and every answer of a rule.
    pub fn set_project(&self, files: Vec<String>) {
        *self.set.lock().unwrap_or_else(|e| e.into_inner()) = Some(files);
    }

    /// Runs `f` with this engine's port of dates joined on the thread, and the project's
    /// contributions merged into the rules they reach.
    fn joined<R>(&self, f: impl FnOnce() -> R) -> R {
        let set = self.set.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match set {
            Some(files) => crate::link::with_set(&files, || crate::days::with(self.dates.clone(), f)),
            None => crate::days::with(self.dates.clone(), f),
        }
    }

    /// The rule at `rule`, checked: read again when its text has changed, or a file its check read
    /// reads otherwise. A report `check` already made of the same text, while what it read reads the
    /// same, says whether the rule passes, so it is not checked a second time.
    fn rule(&self, rule: &Path) -> Result<(String, String, Checked2), Vec<Said>> {
        self.joined(|| self.rule_here(rule)).map(|(path, src, got, _)| (path, src, got))
    }

    /// [`Engine::rule`], with what the check read.
    fn rule_here(&self, rule: &Path) -> Result<(String, String, Checked2, std::sync::Arc<Seen>), Vec<Said>> {
        let (path, src) = read(rule)?;
        let key = (path.clone(), crate::sha256::hex(src.as_bytes()));
        let reported = self
            .reports
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(the_file(&path), key.1.clone()))
            .map(|(_, _, seen, r)| (seen.clone(), r.clone()))
            .filter(|(seen, _)| seen.still());
        let mut cache = self.checked.lock().unwrap_or_else(|e| e.into_inner());
        let kept = cache.entry(key).or_default();
        if let Some((seen, got)) = kept.iter().find(|(seen, _)| seen.still()) {
            return Ok((path, src, got.clone(), seen.clone()));
        }
        let (got, mut seen) = watched(|| {
            std::sync::Arc::new(match &reported {
                Some((_, r)) if crate::has_error(&r.diags) => Err(refused(&path, &src)),
                Some(_) => crate::prepare(&src, &path).map_err(|_| refused(&path, &src)),
                None => {
                    self.checks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    checked(&path, &src, self.budget())
                }
            })
        });
        // read off a report, the answer rests on what the report's check read as well
        if let Some((was, _)) = &reported {
            seen.0.extend(was.0.iter().cloned());
        }
        if kept.len() >= KEPT_EACH {
            kept.remove(0);
        }
        let seen = std::sync::Arc::new(seen);
        kept.push((seen.clone(), got.clone()));
        Ok((path, src, got, seen))
    }

    /// The report `rulec check` makes of a rule's text, in `lang`, naming the file `path` (its
    /// findings are written as they are found, so a report is kept for the language and the path
    /// it was made with, while what it read reads the same; whether the rule passes,
    /// [`Engine::rule`] reads off any of them).
    fn report(&self, path: &str, src: &str, lang: RLang) -> std::sync::Arc<crate::Report> {
        let key = (the_file(path), crate::sha256::hex(src.as_bytes()));
        let mut reports = self.reports.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((p, l, seen, r)) = reports.get(&key)
            && p == path
            && *l == lang
            && seen.still()
        {
            return r.clone();
        }
        self.checks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (r, seen) = self.joined(|| watched(|| i18n::with(lang, || crate::report_with(src, path, self.budget()))));
        let r = std::sync::Arc::new(r);
        reports.insert(key, (path.to_string(), lang, std::sync::Arc::new(seen), r.clone()));
        r
    }

    /// How many times the engine has checked a rule's text whole, for `check` or for a port's
    /// answer: once for each rule in a run that asks the same rule both ways (ritsu's DESIGN 6.1).
    pub fn checks(&self) -> usize {
        self.checks.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// `rulec check` of each file, as `ritsu check` prints it (ritsu's DESIGN 8.3): every finding,
    /// as the command prints it and as its `--format json` prints it, then what the command prints
    /// after them (`ok rules/送料.rule`). `files` are as the person gave them, from where the
    /// program runs; `root` is the project's, which the findings' files are written from.
    pub fn checked(&self, root: &Path, files: &[String], lang: Lang) -> Vec<ritsu_ports::Checked> {
        use ritsu_ports::{Checked, Finding, Part, Verdict};
        let rl = rlang(lang);
        // The files checked together are the project whose contributions are merged (§15.231),
        // unless the engine was given one.
        if self.set.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
            self.set_project(files.to_vec());
        }
        // A finding said for one file is not said again for another: a rule and a contribution to
        // it are each checked in its own scope (§15.231).
        let mut said: std::collections::HashSet<String> = std::collections::HashSet::new();
        files
            .iter()
            .map(|path| {
                let Ok(src) = ritsu_base::fs::read_to_string(path) else {
                    return Checked::unchecked(path, i18n::with(rl, || format!("{}\n", crate::tr!("error: `{path}` を読めません", "error: cannot read `{path}`"))));
                };
                let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
                let r = self.report(path, &src, rl);
                let file = ritsu_base::paths::from_root(root, Path::new(path));
                let shown: Vec<&crate::diag::Diag> = r.diags.iter().filter(|d| said.insert(crate::finding_key(d, path))).collect();
                let mut parts: Vec<Part> = shown
                    .iter()
                    .copied()
                    .map(|d| {
                        let json = ritsu_base::json::parse(&crate::diag::render_json(d, path)).unwrap_or(ritsu_base::json::Json::Null);
                        Part::Finding(Finding {
                            code: d.code.to_string(),
                            severity: match d.severity {
                                crate::diag::Severity::Error => ritsu_base::diag::Severity::Error,
                                crate::diag::Severity::Warning => ritsu_base::diag::Severity::Warning,
                            },
                            file: file.clone(),
                            line: d.line(),
                            text: i18n::with(rl, || crate::findings_text(std::slice::from_ref(d), &lines)),
                            json,
                        })
                    })
                    .collect();
                // Its errors were all said for a file above: one line says so, as `rulec check` does.
                let all_said = crate::has_error(&r.diags) && !shown.iter().any(|d| d.severity == crate::diag::Severity::Error);
                if all_said {
                    parts.push(Part::Text(i18n::with(rl, || crate::tr!("note {path}: エラーは上のファイルで言いました\n", "note {path}: its errors are said above\n"))));
                } else {
                    parts.push(Part::Text(i18n::with(rl, || crate::check_tail_of(&r.shadow, &r.diags, path, 0, &r.merged, r.adds_to.as_deref()))));
                }
                // E129: no koyomi is joined to read the rule's range from (§15.174), which the
                // command exits 2 for.
                let verdict = if r.diags.iter().any(|d| d.code == "E129") {
                    Verdict::Unchecked
                } else if crate::has_error(&r.diags) {
                    Verdict::Fails
                } else {
                    Verdict::Passes
                };
                Checked { label: path.clone(), parts, verdict }
            })
            .collect()
    }

    /// The facts of a rule already read and checked.
    pub fn facts_of(&self, f: &RuleFile, c: &Checked, src: &str, path: &str) -> Result<RuleFacts, Vec<Said>> {
        let g = crate::codegen::Gen::new(f, c, src, path);
        let (typescript, python, go) = g.calls();
        let machine = match crate::cert::machine_facts(f, c) {
            None => None,
            Some(Ok(m)) => Some(m),
            Some(Err(_)) => {
                let why = |l| i18n::with(l, || crate::cert::machine_facts(f, c).and_then(|r| r.err()).unwrap_or_default());
                return Err(vec![Said {
                    code: String::new(),
                    file: path.to_string(),
                    line: f.machine.as_ref().map(|m| m.span.line),
                    message: Text::new(
                        format!("このステートマシンの遷移は、表の行から読めません: {}", why(RLang::Ja)),
                        format!("the moves of this machine cannot be read off a table's rows: {}", why(RLang::En)),
                    ),
                }]);
            }
        };
        Ok(RuleFacts {
            rule: f.name.text.clone(),
            alias: pub_name_of(&f.name),
            version: f.version.clone(),
            sha256: crate::sha256::hex(src.as_bytes()),
            rulec: env!("CARGO_PKG_VERSION").to_string(),
            inputs: f.inputs.iter().map(|i| column(c, &i.name)).collect(),
            outputs: f.outputs.iter().map(|o| column(c, &o.name)).collect(),
            elements: f.elements.iter().map(|el| (el.name.text.clone(), pub_name_of(&el.name), el.fields.iter().map(|fd| column(c, &fd.name)).collect())).collect(),
            enums: g.rule_enums(),
            machine,
            preconditions: preconditions(f, c),
            // An `enums` file is made into modules of types and nothing else (§15.212): no service
            // is generated from it, so none is answered (§15.258), as `rulec api` writes none.
            connect: (f.kind != crate::ast::FileKind::Enums).then(|| g.connect_facts()),
            typescript,
            python,
            go,
        })
    }
}

/// The value of an input on the wire, as rulec's evaluator holds it.
fn to_val(c: &Checked, name: &str, ty: &Ty, v: &Value) -> Result<crate::eval::Val, Text> {
    use crate::eval::Val;
    let wrong = || ritsu_base::tr!("`{name}` に {ty} として読めない値が渡されています", "`{name}` is given a value that is not a {ty}");
    match (ty, v) {
        (Ty::Opt(t), v) => to_val(c, name, t, v),
        (Ty::Bool, Value::Bool(b)) => Ok(Val::Bool(*b)),
        (Ty::Str, Value::Str(s)) => Ok(Val::Str(s.clone())),
        (Ty::Enum(e), Value::Enum(x)) => {
            if c.enums.get(e).is_some_and(|vs| vs.contains(x)) {
                Ok(Val::Enum(x.clone()))
            } else {
                Err(ritsu_base::tr!("`{name}` の `{x}` は {e} の値ではありません", "`{x}` of `{name}` is not a value of {e}"))
            }
        }
        // `YYYY-MM-DD` of a day the calendar has, as the generated code's door reads a date: a
        // day it does not have (2026-02-30) was read into a date before (§15.251)
        (Ty::Date, Value::Date(d)) => match crate::door::civil(d) {
            Some((y, m, dd)) => Ok(Val::Date(y, m, dd)),
            None => Err(wrong()),
        },
        (t, Value::Int(n)) if t.is_numeric() => Ok(Val::Num(crate::types::from_wire(*n, c.wire_scale(name)))),
        _ => Err(wrong()),
    }
}

/// Whether the value of the input or the element's field `name` is outside the range its
/// declaration holds it to, as the generated code's door holds it: a number by its value and a date
/// by its day number (reading only numbers let any date through, §15.198), and a value of an
/// ordered enum by the values its `range` lets in (§15.210, §15.251).
fn outside_range(c: &Checked, name: &str, v: &crate::eval::Val) -> bool {
    use crate::eval::Val;
    let at = match v {
        Val::Num(x) => *x,
        Val::Date(y, m, d) => crate::types::date_ord(*y, *m, *d),
        Val::Enum(x) => return c.enum_ranges.get(name).is_some_and(|allowed| !allowed.contains(x)),
        _ => return false,
    };
    c.ranges.get(name).is_some_and(|(lo, hi)| lo.is_some_and(|lo| at.cmp_to(lo) == std::cmp::Ordering::Less) || hi.is_some_and(|hi| at.cmp_to(hi) == std::cmp::Ordering::Greater))
}

/// Why `output` is not answered as a range of numbers: not a numeric output of the rule, or an
/// optional one, whose answer can be no value at all, which the answer has no room for (§15.249).
pub(crate) fn not_numeric(f: &crate::ast::RuleFile, c: &Checked, output: &str) -> Option<Text> {
    let declared = f.outputs.iter().any(|o| o.name.text == output);
    if declared && c.ty_of(output).is_some_and(|t| matches!(t, Ty::Opt(_)) && t.present().is_numeric()) {
        return Some(ritsu_base::tr!(
            "`{output}` には値が無いことがあるので、とる数を範囲で答えられません",
            "`{output}` may have no value, so what it comes to is not answered as a range of numbers"
        ));
    }
    if !declared || !c.ty_of(output).is_some_and(|t| t.is_numeric()) {
        return Some(ritsu_base::tr!("`{output}` は、この規則の数の出力ではありません", "`{output}` is not a numeric output of this rule"));
    }
    None
}

/// What `output_values` answers for a rule as checked (`Engine::output_values`): the output's
/// range on the wire, the numbers its rows write, and the examples from the rule's vectors.
pub(crate) fn output_values_of(f: &crate::ast::RuleFile, c: &Checked, output: &str) -> ritsu_ports::Found<ritsu_ports::OutputValues> {
    if let Some(why) = not_numeric(f, c, output) {
        return ritsu_ports::Found::Undecided(why);
    }
    let (min, max) = c.wire_range(output).unwrap_or((None, None));
    let values = row_numbers(c, output, &|_, _| true);
    let vectors = crate::vectors::generate(f, c);
    let examples = examples_of(c, output, &vectors, (min, max), values.as_deref());
    ritsu_ports::Found::Value(ritsu_ports::OutputValues { min, max, values, examples })
}

/// Every number the rows of the tables that decide `output` write, on the wire, when each row
/// `rows` takes (by the index of the table among `c.sets` and of the row) writes one; None when one
/// writes something else, or none of them is taken.
pub(crate) fn row_numbers(c: &Checked, output: &str, rows: &dyn Fn(usize, usize) -> bool) -> Option<Vec<i128>> {
    use crate::ast::{Lit, OutCell};
    let sc = c.wire_scale(output);
    let ty = c.ty_of(output).unwrap_or(Ty::Unknown);
    let mut literal: Vec<i128> = Vec::new();
    for (si, set) in c.sets.iter().enumerate() {
        let Some(oi) = set.table.outputs.iter().position(|o| o.name.text == output) else { continue };
        for (ri, row) in set.table.rows.iter().enumerate() {
            if !rows(si, ri) {
                continue;
            }
            match row.outs.get(oi) {
                Some(OutCell::Lit(Lit::Num(n))) => literal.push(crate::types::wire_int(crate::types::lit_value_in_pub(n, &ty)?, sc)),
                _ => return None,
            }
        }
    }
    if literal.is_empty() {
        return None;
    }
    literal.sort_unstable();
    literal.dedup();
    Some(literal)
}

/// What the output `output` comes to on each of `vectors`, on the wire.
pub(crate) fn wire_outputs(c: &Checked, output: &str, vectors: &[crate::vectors::Vector]) -> Vec<i128> {
    let sc = c.wire_scale(output);
    vectors
        .iter()
        .flat_map(|v| v.outputs.iter().filter(|(n, _)| n == output))
        .filter_map(|(_, x)| match x {
            Some(crate::eval::Val::Num(r)) => Some(crate::types::wire_int(*r, sc)),
            _ => None,
        })
        .collect()
}

/// The examples of what the output `output` comes to (§15.270): the two `ends` of its range, the
/// numbers its rows write (`values`), then the fewest and the most it comes to over `vectors`,
/// in that order, each with the first of `vectors` that comes to it, as `eval` takes the input. A
/// value is given once, and one no vector comes to (an end the output never reaches) has none.
/// `output_values` gives it the rule's vectors; `output_values_over` those inside the ranges it
/// holds the inputs to (§15.273).
pub(crate) fn examples_of(c: &Checked, output: &str, vectors: &[crate::vectors::Vector], ends: (Option<i128>, Option<i128>), values: Option<&[i128]>) -> Vec<(i128, Values)> {
    let sc = c.wire_scale(output);
    let wire = |x: &Option<crate::eval::Val>| match x {
        Some(crate::eval::Val::Num(r)) => Some(crate::types::wire_int(*r, sc)),
        _ => None,
    };
    let reached = wire_outputs(c, output, vectors);
    let mut wanted: Vec<i128> = ends.0.into_iter().chain(ends.1).collect();
    wanted.extend(values.into_iter().flatten().copied());
    wanted.extend(reached.iter().min().into_iter().chain(reached.iter().max()).copied());
    let mut examples: Vec<(i128, Values)> = Vec::new();
    for w in wanted {
        if examples.iter().any(|(v, _)| *v == w) {
            continue;
        }
        let hit = vectors.iter().find(|v| v.outputs.iter().any(|(n, x)| n == output && wire(x) == Some(w)));
        if let Some(v) = hit {
            let inputs: Values = v.input.iter().map(|(n, x)| (n.clone(), from_val(c, n, x))).collect();
            examples.push((w, inputs));
        }
    }
    examples
}

/// An output of the evaluator, on the wire: `none` of a value that may be absent is the absence.
pub(crate) fn from_val(c: &Checked, name: &str, v: &crate::eval::Val) -> Value {
    use crate::eval::Val;
    match v {
        Val::Enum(s) if s == crate::kw::NONE && matches!(c.ty_of(name), Some(Ty::Opt(_))) => Value::None,
        Val::Enum(s) => Value::Enum(s.clone()),
        Val::Num(r) => Value::Int(crate::types::wire_int(*r, c.wire_scale(name))),
        Val::Bool(b) => Value::Bool(*b),
        Val::Str(s) => Value::Str(s.clone()),
        Val::Date(y, m, d) => Value::Date(format!("{y:04}-{m:02}-{d:02}")),
        Val::Seq(xs) => Value::List(xs.iter().map(|x| x.iter().map(|(k, v)| (k.clone(), from_val(c, k, v))).collect()).collect()),
    }
}

/// Whether `l op r` holds between two rationals.
fn holds(l: Rat, op: &str, r: Rat) -> bool {
    use std::cmp::Ordering::*;
    match (op, l.cmp_to(r)) {
        ("<=", Less | Equal) | ("<", Less) | (">=", Greater | Equal) | (">", Greater) => true,
        _ => false,
    }
}

impl ritsu_ports::Rules for Engine {
    /// The text and every file the check read, from the check kept for the files read now.
    fn rests_on(&self, rule: &Path) -> Option<String> {
        let (_, src, _, seen) = self.joined(|| self.rule_here(rule)).ok()?;
        Some(seen.rests_on(&src))
    }

    fn facts(&self, rule: &Path) -> Result<RuleFacts, Vec<Said>> {
        let (path, src, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        self.facts_of(f, c, &src, &path)
    }

    /// A relation between two inputs is decided over the box of their ranges: it holds for every
    /// pair exactly when it holds for the pair at the corner that tries it hardest, and that pair
    /// is the example when it does not. The bounds on a list's total and length are about a list
    /// whose length the question does not carry, and are not decided.
    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)], max_len: Option<i128>) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>> {
        let (_, _, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        let range = |name: &str| -> (Option<Rat>, Option<Rat>) {
            let sc = c.wire_scale(name);
            match ranges.iter().find(|(n, ..)| n == name) {
                Some((_, lo, hi)) => (lo.map(|v| crate::types::from_wire(v, sc)), hi.map(|v| crate::types::from_wire(v, sc))),
                None => c.ranges.get(name).copied().unwrap_or((None, None)),
            }
        };
        Ok(preconditions(f, c)
            .into_iter()
            .map(|p| {
                let a = match &p {
                    Precondition::Relation { left, op, right } => {
                        let ((llo, lhi), (rlo, rhi)) = (range(left), range(right));
                        // the corner: the left side as large as it gets against the right as small,
                        // for `<` and `<=`; the other way round for `>` and `>=`
                        let corner = if op.starts_with('<') { (lhi, rlo) } else { (llo, rhi) };
                        // A date at the corner is a date, as the port gives every other date
                        // (§15.198); a number goes on the wire as its integer.
                        let at = |name: &str, v: Rat| -> Value {
                            if c.ty_of(name) == Some(Ty::Date) {
                                let (y, m, d) = crate::types::ord_to_date(v);
                                from_val(c, name, &crate::eval::Val::Date(y, m, d))
                            } else {
                                Value::Int(crate::types::wire_int(v, c.wire_scale(name)))
                            }
                        };
                        match corner {
                            (Some(l), Some(r)) if holds(l, op, r) => Answer::Holds,
                            (Some(l), Some(r)) => Answer::Fails(vec![(left.clone(), at(left, l)), (right.clone(), at(right, r))]),
                            _ => Answer::Undecided(ritsu_base::tr!(
                                "`{left}` か `{right}` の範囲に上限か下限が無いので、`{left} {op} {right}` がどの値でも成り立つとは言えません",
                                "`{left}` or `{right}` has an open end, so `{left} {op} {right}` cannot be shown to hold for every value"
                            )),
                        }
                    }
                    // A side that adds inputs up (§15.216): the corner is each of them at the end
                    // that tries the relation hardest, the sum of those against the other side's.
                    // Every input is a number here: a date is not added (E048).
                    Precondition::SumRelation { left, op, right } => {
                        let small_first = op.starts_with('<');
                        // the end of each input that tries it hardest: the top of the side that
                        // should be the smaller, the bottom of the one that should be the larger
                        let end = |name: &str, top: bool| if top { range(name).1 } else { range(name).0 };
                        let pick = |ns: &[String], top: bool| -> Option<Vec<(String, Rat)>> { ns.iter().map(|n| end(n, top).map(|v| (n.clone(), v))).collect() };
                        let total = |vs: &[(String, Rat)]| vs.iter().fold(Rat::zero(), |acc, (_, v)| acc.add(*v));
                        let said = format!("{} {op} {}", left.join(" + "), right.join(" + "));
                        match (pick(left, small_first), pick(right, !small_first)) {
                            (Some(l), Some(r)) if holds(total(&l), op, total(&r)) => Answer::Holds,
                            (Some(l), Some(r)) => Answer::Fails(
                                l.iter().chain(r.iter()).map(|(n, v)| (n.clone(), Value::Int(crate::types::wire_int(*v, c.wire_scale(n))))).collect(),
                            ),
                            _ => Answer::Undecided(ritsu_base::tr!(
                                "足す入力のどれかの範囲に上限か下限が無いので、`{said}` がどの値でも成り立つとは言えません",
                                "one of the inputs it adds has an open end, so `{said}` cannot be shown to hold for every value"
                            )),
                        }
                    }
                    // The total is largest with the list as long as it gets and every element at
                    // the top of its range (none when that top is below zero: the empty list's
                    // total, 0, is then the largest).
                    Precondition::Sum { name, over, of, max } => match (max_len, range(of).1) {
                        (Some(n), Some(hi)) => {
                            let top = crate::types::wire_int(hi, c.wire_scale(of));
                            let most = if top > 0 { top.saturating_mul(n) } else { 0 };
                            if most <= *max {
                                Answer::Holds
                            } else {
                                let one = vec![(of.clone(), Value::Int(top))];
                                Answer::Fails(vec![(over.clone(), Value::List(vec![one; n.clamp(0, 10_000) as usize]))])
                            }
                        }
                        (None, _) => Answer::Undecided(ritsu_base::tr!(
                            "`{name}` は `{over}` の合計の上限です。並びの長さの上限が分からないので、決められません",
                            "`{name}` bounds a total over `{over}`, and how long the list can be is not known, so it cannot be decided"
                        )),
                        (Some(_), None) => Answer::Undecided(ritsu_base::tr!(
                            "`{name}` は `{over}` の `{of}` の合計の上限です。`{of}` の範囲に上限が無いので、決められません",
                            "`{name}` bounds the total of `{of}` over `{over}`, and the range of `{of}` has no upper end, so it cannot be decided"
                        )),
                    },
                    Precondition::Length { sequence, max } => match max_len {
                        Some(n) if n <= *max => Answer::Holds,
                        Some(_) => Answer::Fails(vec![(sequence.clone(), Value::List(vec![Vec::new(); (*max + 1).clamp(0, 10_000) as usize]))]),
                        None => Answer::Undecided(ritsu_base::tr!(
                            "`{sequence}` の長さの上限です。並びの長さの上限が分からないので、決められません",
                            "it bounds the length of `{sequence}`, and how long the list can be is not known, so it cannot be decided"
                        )),
                    },
                    Precondition::Days { input, .. } => Answer::Undecided(ritsu_base::tr!(
                        "`{input}` は koyomi の日付がとる日だけをとります。問い合わせには範囲しか入らないので、渡す日がその日のどれかであることまでは示せません",
                        "`{input}` takes only the days of a koyomi date, and a range alone cannot show that the day given is one of them"
                    )),
                };
                (p, a)
            })
            .collect())
    }

    /// The rule checked again with the date input `input` taking only `days` (inside what the
    /// rule declares for it), as a rule whose range is a koyomi date is checked over its days
    /// (§15.174): its tables complete, without overlap, every row reached. The first error that
    /// says otherwise is the example; a rule that does not pass its own check is refused.
    fn checked_over(&self, rule: &Path, input: &str, days: &DaySet) -> Result<Answer<Text>, Vec<Said>> {
        let (path, src, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        if !f.inputs.iter().any(|i| i.name.text == input) || c.ty_of(input) != Some(Ty::Date) {
            return Ok(Answer::Undecided(ritsu_base::tr!("`{input}` は、この規則の日付の入力ではありません", "`{input}` is not a date input of this rule")));
        }
        let given: Vec<i64> = days.iter().copied().collect();
        let both = |l: RLang| self.joined(|| crate::days::over(input, given.clone(), || i18n::with(l, || crate::report(&src, &path).diags)));
        let (ja, en) = (both(RLang::Ja), both(RLang::En));
        let first = ja.iter().zip(&en).find(|(d, _)| d.severity == crate::diag::Severity::Error && matches!(d.code, "E101" | "E102" | "E105"));
        Ok(match first {
            None => Answer::Holds,
            Some((j, e)) => {
                let line = |d: &crate::diag::Diag| d.line().map(|l| format!(":{l}")).unwrap_or_default();
                Answer::Fails(Text::new(format!("[{}] {path}{}: {}", j.code, line(j), j.title), format!("[{}] {path}{}: {}", e.code, line(e), e.title)))
            }
        })
    }

    /// The output's range is the one its answers lie in (§15.268, §15.269): a range written on its
    /// line, or what decides it (a table's rows, a `define`'s arithmetic, what its `result` comes
    /// to) put out to the grid it rounds to. Where every row that decides it writes a number, those
    /// numbers. The examples (§15.270) are values the output comes to, each with an input from the
    /// rule's vectors (which take every row, and hold the inputs to their ranges and constraints)
    /// that comes to it ([`examples_of`]).
    fn output_values(&self, rule: &Path, output: &str) -> Result<ritsu_ports::Found<ritsu_ports::OutputValues>, Vec<Said>> {
        let (_, _, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        Ok(self.joined(|| output_values_of(f, c, output)))
    }

    /// What `output_values` answers with the inputs named in `ranges` held to them, as
    /// `outputs_over` takes them (`crate::over::values_over`, §15.273).
    fn output_values_over(&self, rule: &Path, output: &str, ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<ritsu_ports::Found<ritsu_ports::OutputValues>, Vec<Said>> {
        let (path, src, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        Ok(self.joined(|| crate::over::values_over(f, c, &src, &path, output, ranges)))
    }

    /// The rule read again with each input held to its range, the rows some input may reach
    /// found through the sieve of the completeness proof, and an input for each of their values
    /// found in the vectors over the ranges (`crate::over`). Exact when the two meet.
    fn outputs_over(&self, rule: &Path, output: &str, ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<ritsu_ports::Found<Vec<(Value, Values)>>, Vec<Said>> {
        let (path, src, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        Ok(self.joined(|| crate::over::outputs_over(f, c, &src, &path, output, ranges)))
    }

    fn date_range(&self, rule: &Path, input: &str) -> Result<(Option<i64>, Option<i64>), Vec<Said>> {
        let (_, _, got) = self.rule(rule)?;
        let (_, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        let (lo, hi) = c.ranges.get(input).copied().unwrap_or((None, None));
        let day = |r: Option<Rat>| r.map(|v| (v.num / v.den) as i64);
        Ok((day(lo), day(hi)))
    }

    fn eval(&self, rule: &Path, inputs: &Values) -> Result<Values, RuleError> {
        use crate::eval::Val;
        let (_, _, got) = self.rule(rule).map_err(RuleError::Unread)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| RuleError::Unread(e.clone()))?;
        let mut vals = std::collections::HashMap::new();
        for i in &f.inputs {
            let name = &i.name.text;
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            let v = inputs.iter().find(|(n, _)| n == name).map(|(_, v)| v).ok_or_else(|| RuleError::Input(ritsu_base::tr!("入力 `{name}` がありません", "the input `{name}` is missing")))?;
            // `none`, which the evaluator holds as the word itself
            if matches!((&ty, v), (Ty::Opt(_), Value::None)) {
                vals.insert(name.clone(), Val::Enum(crate::kw::NONE.into()));
                continue;
            }
            let val = to_val(c, name, &ty, v).map_err(RuleError::Input)?;
            if outside_range(c, name, &val) {
                return Err(RuleError::Input(ritsu_base::tr!("入力 `{name}` が範囲の外です", "the input `{name}` is outside its range")));
            }
            // A date of koyomi's days takes those days only, and the door refuses any other
            // (§15.174).
            if let (Val::Date(y, m, d), Some(set)) = (&val, c.day_sets.get(name))
                && set.days.binary_search(&(crate::types::date_ord(*y, *m, *d).num as i64)).is_err()
            {
                let (file, date) = set.from.as_ref().map(|fr| (fr.file.clone(), fr.date.clone())).unwrap_or_default();
                return Err(RuleError::Input(ritsu_base::tr!(
                    "入力 `{name}` は、{file} の {date} がとる日ではありません",
                    "the input `{name}` is not a day {date} of {file} comes to"
                )));
            }
            vals.insert(name.clone(), val);
        }
        for el in &f.elements {
            let name = &el.name.text;
            let Some((_, Value::List(xs))) = inputs.iter().find(|(n, _)| n == name) else {
                return Err(RuleError::Input(ritsu_base::tr!("並び `{name}` がありません", "the list `{name}` is missing")));
            };
            let mut seq = Vec::new();
            for x in xs {
                let mut one = std::collections::BTreeMap::new();
                for fd in &el.fields {
                    let fname = &fd.name.text;
                    let ty = c.ty_of(fname).unwrap_or(Ty::Unknown);
                    let v = x.iter().find(|(n, _)| n == fname).map(|(_, v)| v).ok_or_else(|| RuleError::Input(ritsu_base::tr!("要素のフィールド `{fname}` がありません", "an element's field `{fname}` is missing")))?;
                    if matches!((&ty, v), (Ty::Opt(_), Value::None)) {
                        one.insert(fname.clone(), Val::Enum(crate::kw::NONE.into()));
                        continue;
                    }
                    let val = to_val(c, fname, &ty, v).map_err(RuleError::Input)?;
                    // a field is held to its range as an input is, the way the generated code's
                    // door holds each element's (§15.251)
                    if outside_range(c, fname, &val) {
                        return Err(RuleError::Input(ritsu_base::tr!("要素のフィールド `{fname}` が範囲の外です", "an element's field `{fname}` is outside its range")));
                    }
                    one.insert(fname.clone(), val);
                }
                seq.push(one);
            }
            vals.insert(name.clone(), Val::Seq(seq));
        }
        // Two dates compare by their day numbers, as the generated code's door compares them
        // (§15.198).
        for k in &f.constraints {
            if !crate::vectors::constraint_holds(k, &|n| vals.get(n)) {
                return Err(RuleError::Input(ritsu_base::tr!("`{}` が成り立ちません", "`{}` does not hold", k.said())));
            }
        }
        let (outs, _, _) = crate::eval::run_all(f, c, vals);
        outs.into_iter()
            .map(|(name, v)| match v {
                Some(v) => Ok((name.clone(), from_val(c, &name, &v))),
                None => Err(RuleError::Contradiction(ritsu_base::tr!("出力 `{name}` を決める行がありません", "no row decides the output `{name}`"))),
            })
            .collect()
    }

    fn doc(&self, rule: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>> {
        let (path, src) = read(rule)?;
        self.joined(|| i18n::with(rlang(lang), || {
            let (f, c) = checked(&path, &src, self.budget())?;
            Ok(if html {
                let js = crate::codegen::Gen::new(&f, &c, &src, &path).javascript();
                crate::doc::render_html_named(&f, &c, &src, &path, shown, &js)
            } else {
                crate::doc::render_named(&f, &c, &src, &path, shown)
            })
        }))
    }
}

/// The last line of each thing a rule writes, past its first: a table's last row, a machine's
/// last line, a source's last pin.
fn table_end(t: &crate::ast::Table) -> usize {
    let mut end = t.span.line;
    for r in &t.rows {
        end = end.max(r.span.line);
        end = r.out_spans.iter().chain(&r.cell_spans).map(|s| s.line).fold(end, usize::max);
    }
    t.overrides.iter().map(|o| o.span.line).fold(end, usize::max)
}

fn machine_end(m: &crate::ast::MachineDecl) -> usize {
    let mut end = m.span.line;
    for l in [m.carry.as_ref().map(|c| c.2.line), m.initial.as_ref().map(|i| i.1.line), m.final_span.as_ref().map(|s| s.line), m.held_span.as_ref().map(|s| s.line)].into_iter().flatten() {
        end = end.max(l);
    }
    end = m.nevers.iter().map(|n| n.span.line).fold(end, usize::max);
    end = m.carries.iter().map(|c| c.2.line).chain(m.initials.iter().map(|i| i.2.line)).chain(m.alwayses.iter().map(|a| a.span.line)).fold(end, usize::max);
    m.onces.iter().map(|o| o.span.line).fold(end, usize::max)
}

/// The path of a file a rule names, from the root: written from the rule's directory, or from the
/// root of the module of ritsu's it is in for `@<name>/…` (ritsu's DESIGN 17.6).
fn from_root(root: &Path, file: &str, written: &str) -> Option<String> {
    ritsu_base::modules::join(root, file, written).ok()
}

impl ritsu_ports::Sources for Engine {
    /// The sources the rule file declares itself (not one an `apply` brings in: the other file
    /// declares it, and its copies sit beside that file), each with its pins, when the rule
    /// passes the whole of check, which holds every copy to its pin (§15.68).
    fn sources(&self, file: &Path) -> Result<Vec<ritsu_ports::Source>, Vec<Said>> {
        let (_, _, got) = self.rule(file)?;
        let (f, _) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        Ok(f.sources
            .iter()
            .filter(|s| s.base.is_none())
            .map(|s| ritsu_ports::Source {
                name: s.name.text.clone(),
                line: s.span.line,
                kind: match &s.kind {
                    SourceKind::Law { db, id, asof } => ritsu_ports::SourceKind::Law {
                        db: db.word().to_string(),
                        id: id.clone(),
                        asof: asof.clone(),
                        pins: s.pins.iter().map(|p| (p.fragment.clone(), p.hash.clone())).collect(),
                    },
                    SourceKind::File { path, url, hash } => ritsu_ports::SourceKind::File { path: path.clone(), url: url.clone(), pin: hash.clone() },
                },
            })
            .collect())
    }
}

impl ritsu_ports::Items for Engine {
    /// Each input, output, enum (and each of its values), table, clause, define, derive,
    /// machine and source the file writes itself (what an `apply` brings in is the other file's).
    /// The definition of each is its lines as `rulec fmt` writes them: the columns aligned, the
    /// symbols in ASCII, so that realigning a table changes nothing.
    fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        let (path, src) = read(&ritsu_base::paths::on_disk(root, file))?;
        let parsed = crate::parse::parse(&src, &path);
        let Some(f) = parsed.file else {
            return Err(refused(&path, &src));
        };
        let formatted: Vec<String> = crate::fmt::format(&src).lines().map(String::from).collect();
        let text = |from: usize, to: usize| -> String { formatted.get(from.saturating_sub(1)..to.min(formatted.len())).map(|ls| ls.join("\n")).unwrap_or_default() };
        let naming = |kind: &str, name: &str| Naming::file(Tool::Rulec, file).with(kind, name);
        let mut out = Vec::new();
        let one = |naming: Naming, from: usize, to: usize, out: &mut Vec<Item>| out.push(Item { naming, lines: (from, to), text: text(from, to) });
        for i in &f.inputs {
            one(naming("input", &i.name.text), i.span.line, i.span.line, &mut out);
        }
        for o in &f.outputs {
            one(naming("output", &o.name.text), o.span.line, o.span.line, &mut out);
        }
        for e in &f.enums {
            let line = e.span.line;
            out.push(Item { naming: naming("enum", &e.name.text), lines: (line, line), text: text(line, line) });
            for v in &e.values {
                let written = match &v.ascii {
                    Some(a) => format!("{}({a})", v.text),
                    None => v.text.clone(),
                };
                out.push(Item { naming: naming("enum", &e.name.text).with("value", &v.text), lines: (line, line), text: written });
            }
        }
        for it in &f.items {
            match it {
                AstItem::Table(t) => {
                    let Some(n) = &t.name else { continue };
                    if t.applied.is_some() {
                        continue;
                    }
                    one(naming(if t.clause { "clause" } else { "table" }, &n.text), t.span.line, table_end(t), &mut out);
                }
                AstItem::Define(d) => one(naming("define", &d.name.text), d.span.line, d.span.line, &mut out),
                AstItem::Derived(d) => one(naming("derive", &d.name.text), d.span.line, d.span.line, &mut out),
                AstItem::Agg(_) | AstItem::Fold(_) => {}
            }
        }
        if let Some(m) = &f.machine {
            one(naming("machine", &m.name.text), m.span.line, machine_end(m), &mut out);
        }
        for s in f.sources.iter().filter(|s| s.base.is_none()) {
            let end = s.pins.iter().map(|p| p.span.line).fold(s.span.line, usize::max);
            one(naming("source", &s.name.text), s.span.line, end, &mut out);
        }
        out.sort_by_key(|i| i.lines.0);
        Ok(out)
    }
}

impl ritsu_ports::References for Engine {
    /// The files a rule names: the `.proto` or JSON Schema an enum's values come from
    /// (`import proto`, `import jsonschema`), the `enums` file an enum is read from whole
    /// (`import enum`, §15.212), the contract a `shape` reads its inputs out of, the contract
    /// whose service the rule implements (`implements`, §15.274), a rule it applies (`apply`), a
    /// document a `source` copies (`source … file`), and the koyomi date an input takes its days
    /// from (`range from koyomi`, §15.174). An enum, a message or a service of a `.proto` is named
    /// from the file's package.
    fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>> {
        let disk = ritsu_base::paths::on_disk(root, file);
        let (path, src) = read(&disk)?;
        let parsed = crate::parse::parse(&src, &path);
        let Some(f) = parsed.file else {
            return Err(refused(&path, &src));
        };
        // a `.proto`'s name for a message or an enum, from its package: `shop.v1.Order` is `Order`
        let local = |proto: &str, written: &str| -> String {
            let package = ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(root, proto)).ok().and_then(|s| crate::proto::read(proto, &s).ok()).and_then(|p| p.package);
            match package {
                Some(p) => written.strip_prefix(&format!("{p}.")).unwrap_or(written).to_string(),
                None => written.to_string(),
            }
        };
        let mut out = Vec::new();
        for im in &f.enum_imports {
            let Some(p) = from_root(root, file, &im.file) else { continue };
            let (target, how) = match im.kind {
                crate::ast::EnumSource::Proto => (Naming::file(Tool::Proto, &p).with("enum", local(&p, &im.source)), "import proto"),
                crate::ast::EnumSource::JsonSchema => (Naming::file(Tool::File, &p), "import jsonschema"),
            };
            out.push(Reference { line: im.span.line, target, how: how.into() });
        }
        for ef in &f.enum_froms {
            let Some(p) = from_root(root, file, &ef.file) else { continue };
            out.push(Reference { line: ef.span.line, target: Naming::file(Tool::Rulec, &p).with("enum", &ef.name.text), how: "import enum".into() });
        }
        for sh in &f.shapes {
            let Some(p) = from_root(root, file, &sh.file) else { continue };
            let target = match sh.source {
                crate::ast::EnumSource::Proto => Naming::file(Tool::Proto, &p).with("message", local(&p, &sh.at)),
                crate::ast::EnumSource::JsonSchema => Naming::file(Tool::File, &p),
            };
            out.push(Reference { line: sh.span.line, target, how: "shape".into() });
        }
        if let Some(d) = &f.implements
            && let Some(p) = from_root(root, file, &d.file)
        {
            out.push(Reference { line: d.span.line, target: Naming::file(Tool::Proto, &p).with("service", local(&p, &d.service)), how: "implements".into() });
        }
        for a in &f.applies {
            if let Some(p) = from_root(root, file, &a.path) {
                out.push(Reference { line: a.span.line, target: Naming::file(Tool::Rulec, &p), how: "apply".into() });
            }
        }
        for s in f.sources.iter().filter(|s| s.base.is_none()) {
            if let SourceKind::File { path: written, .. } = &s.kind
                && let Some(p) = from_root(root, file, written)
            {
                out.push(Reference { line: s.span.line, target: Naming::file(Tool::File, &p), how: "source".into() });
            }
        }
        for i in &f.inputs {
            if let Some(d) = i.range.as_ref().and_then(|r| r.days.as_ref())
                && let Some(p) = from_root(root, file, &d.file)
            {
                out.push(Reference { line: d.span.line, target: Naming::file(Tool::Koyomi, &p).with("date", &d.date), how: "range from koyomi".into() });
            }
        }
        out.sort_by_key(|r| r.line);
        Ok(out)
    }
}
