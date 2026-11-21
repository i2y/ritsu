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
use ritsu_ports::{Answer, Column, ColumnType, DaySet, Item, Outside, Precondition, Reference, RuleError, RuleFacts, Said, Value, Values};
use std::path::Path;

/// rulec, as the ports reach it. It keeps each rule it has checked, by the file and its text, so
/// that a program asking one rule many times (an evaluation per call of a workflow) checks it
/// once; and the report `rulec check` prints of it, so that `ritsu check`, which prints that report
/// and then asks the rule's facts for the workflows that use it, checks it once too. A rule is
/// kept by the file, however its path is written, and named the way the run writes its paths
/// (ritsu's DESIGN 4.15), so every way of asking gets the same answer. A check is kept while every
/// file it read reads the same ([`Seen`]): a rule of the same text whose enums, callee or
/// contributions read otherwise (the other side of a diff, a git revision, the module at hand with
/// another lock) is checked again, and both checks are kept.
#[derive(Default)]
pub struct Engine {
    /// By the file and the text: the checks kept, each with what its files read.
    checked: std::sync::Mutex<std::collections::HashMap<(std::path::PathBuf, String), Vec<Kept>>>,
    /// By the file (its path made absolute and folded, so two spellings of one path are one file),
    /// its text and the language: the path the report was made with, what its files read, and the
    /// report. A rule's findings are written in one language as they are found, so a rule that does
    /// not pass is checked again for the other language when another language says its errors in
    /// both ([`Engine::refused`]).
    reports: std::sync::Mutex<std::collections::HashMap<(std::path::PathBuf, String, RLang), (String, std::sync::Arc<Seen>, std::sync::Arc<crate::Report>)>>,
    /// How many times a rule's text has been checked whole (what [`Engine::checks`] says).
    checks: std::sync::atomic::AtomicUsize,
    /// The port of dates a rule's `range from koyomi` is read through (§15.174); None where no
    /// koyomi is joined.
    dates: Option<crate::days::Port>,
    /// The most nodes one check of a rule visits (`--budget`), when it is not the default
    /// (§15.209).
    budget: Option<i64>,
    /// The contribution files among the files of the project, as the person gave them, chosen once
    /// when the project is set: they are merged into the rules they reach (§15.231). `None` outside a
    /// project.
    set: std::sync::Mutex<Option<Vec<String>>>,
}

/// The file a path names, made absolute and folded as the system opens it
/// (`ritsu_base::paths::opened`): how the checks and the reports are kept.
fn the_file(path: &str) -> std::path::PathBuf {
    ritsu_base::paths::opened(Path::new(path))
}

/// A rule read and checked, or what check said of it.
type Checked2 = std::sync::Arc<Result<(RuleFile, Checked), Vec<Said>>>;

/// A check kept, with what the files it read came to.
type Kept = (std::sync::Arc<Seen>, Checked2);

/// How many checks of one text are kept: the rule read through as many sets of files in one run
/// (the two sides of a diff, the base and the head of `ritsu mod report`).
const KEPT_EACH: usize = 4;

// What a check read through `ritsu_base::fs` (ritsu-base's `Seen`): the files a rule reads besides
// itself (what it imports and applies, the contributions to it, the days of a koyomi date) at the
// paths the files of the thread were asked for, and the files that say where a module's
// dependencies are (its `ritsu.mod`, its lock) that the view of them read.
use ritsu_base::fs::{Seen, watched};

/// What the rule at `rule`, of the text `src`, checked with the reads `seen` rests on
/// ([`ritsu_ports::Rules::rests_on`]).
fn rests_on(seen: &Seen, rule: &Path, src: &str) -> String {
    ritsu_ports::rests_on(rule, Some(src), seen.reads())
}

fn rlang(l: Lang) -> RLang {
    match l {
        Lang::Ja => RLang::Ja,
        Lang::En => RLang::En,
    }
}

/// The file's text, and its path the way the run writes its paths (ritsu's DESIGN 4.15): what its
/// check names it by, however the caller wrote it.
fn read(rule: &Path) -> Result<(String, String), Vec<Said>> {
    let path = ritsu_base::paths::show(rule);
    match ritsu_base::fs::read_to_string(&path) {
        Ok(src) => Ok((path, src)),
        Err(e) => Err(vec![Said::unreadable(&path, &e)]),
    }
}

/// A value's type as the port hands it over: a number with its unit and its range on the wire, a
/// date with its range as day numbers, an enum with the values its input's range lets in (§15.210,
/// §15.251). An input's range is the one the entry guard holds it to; an output's is the one its
/// answers lie in, as `output_values`, the certificate's `output_ranges` and the schema say
/// (§15.271).
fn column_type(c: &Checked, name: &str, ty: &Ty) -> ColumnType {
    match ty {
        Ty::Bool => ColumnType::Bool,
        Ty::Str => ColumnType::Str,
        Ty::Date => {
            let (min, max) = day_range(c, name);
            ColumnType::Date { min, max }
        }
        Ty::Enum(e) => ColumnType::Enum { name: e.clone(), range: c.enum_ranges.get(name).cloned() },
        Ty::Opt(t) => ColumnType::Opt(Box::new(column_type(c, name, t))),
        _ => {
            let step = matches!(ty, Ty::Rate).then(|| Rat::new(1, c.wire_scale(name)));
            let (min, max) = c.wire_range(name).unwrap_or((None, None));
            ColumnType::Num { written: ty.to_string(), unit: ty.unit(step), min, max, round: None }
        }
    }
}

/// The range of a date as day numbers (days since 1970-01-01, the count the generated code holds a
/// date in): the one its line declares, or the hull of the days of a koyomi date an input's range is;
/// None at an end not written. The facts' column carries it (ritsu's DESIGN 3.2).
fn day_range(c: &Checked, name: &str) -> (Option<i64>, Option<i64>) {
    let day = |r: Option<Rat>| r.map(|v| (v.num / v.den) as i64);
    let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
    (day(lo), day(hi))
}

fn column(c: &Checked, n: &Name) -> Column {
    let ty = c.ty_of(&n.text).unwrap_or(Ty::Unknown);
    Column { name: n.text.clone(), alias: pub_name_of(n), ty: column_type(c, &n.text, &ty) }
}

/// The unit of the total `name`, as the integers of its precondition count it (§15.333): the type's
/// unit, with the step one integer counts where the total is held finer than a whole unit (a total
/// of a value worked out on each element that keeps its fractions, §15.314), a rate's as a rate's.
/// What a sentence says the ends of the total in: a total of a `define` on each element has no
/// column of its own that a caller sees.
pub fn sum_unit(c: &Checked, name: &str) -> Option<ritsu_units::Unit> {
    let ty = c.ty_of(name)?;
    let scale = c.scales.get(name).copied().unwrap_or(1);
    let step = (scale != 1 || matches!(ty.present(), Ty::Rate)).then(|| Rat::new(1, scale));
    let mut unit = ty.present().unit(step)?;
    unit.step = step;
    Some(unit)
}

/// An output's column, with the step its `round` puts its answers on: `round down(0.5%)` of a
/// `rate[step 0.25%]` travels in quarters of a percent and answers on halves, which a module that
/// reads the answer takes as a value on 0.5% (kaname's DESIGN 6.1).
fn output_column(c: &Checked, o: &crate::ast::OutDecl) -> Column {
    fn put(t: &mut ColumnType, grid: Option<Rat>) {
        match t {
            ColumnType::Opt(x) => put(x, grid),
            ColumnType::Num { round, .. } => *round = grid,
            _ => {}
        }
    }
    let mut col = column(c, &o.name);
    let ty = c.ty_of(&o.name.text).unwrap_or(Ty::Unknown);
    put(&mut col.ty, o.rounding.as_ref().and_then(|rd| crate::types::lit_value_in_pub(&rd.grid, ty.present())));
    col
}

fn preconditions(f: &RuleFile, c: &Checked) -> Vec<Precondition> {
    crate::verify::preconditions(f, c)
        .into_iter()
        .map(|p| match p {
            crate::verify::Pre::Rel { left, op, right } => match (left.as_slice(), right.as_slice()) {
                ([l], [r]) => Precondition::Relation { left: l.clone(), op: op.to_string(), right: r.clone() },
                _ => Precondition::SumRelation { left, op: op.to_string(), right },
            },
            crate::verify::Pre::Sum { name, over, of, max, min } => {
                let unit = sum_unit(c, &name);
                Precondition::Sum { name, over, of, max, min, unit }
            }
            crate::verify::Pre::Count { name, over, min } => Precondition::Count { name, over, min },
            crate::verify::Pre::Length { sequence, max } => Precondition::Length { sequence, max },
            crate::verify::Pre::Unique { sequence, field } => Precondition::Unique { sequence, field },
            crate::verify::Pre::Days { input, file, date, days } => Precondition::Days { input, file, date, days: days.into_iter().collect() },
            crate::verify::Pre::Never { said, conds } => Precondition::Never { said, conds },
            crate::verify::Pre::Define { name, range, from, over } => Precondition::Define { name, range, from, over },
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
    /// the rules they reach, in every check and every answer of a rule. They are chosen here, once,
    /// as the files the thread reads now say (`link::contributions`): a check or an answer reads no
    /// file of the project to choose them again, so what a check is kept with is what it read
    /// (ritsu's DESIGN 4.15).
    pub fn set_project(&self, files: Vec<String>) {
        let chosen = crate::link::contributions(&files);
        *self.set.lock().unwrap_or_else(|e| e.into_inner()) = Some(chosen);
    }

    /// The contribution files of the project the engine answers for, as [`Engine::set_project`]
    /// chose them, each as the run writes it: what a program that merges them itself (the code
    /// `ritsu gen` makes of a rule) hands to `link::with_contributions`. Empty outside a project.
    pub fn contributions(&self) -> Vec<String> {
        self.set.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default()
    }

    /// Runs `f` with this engine's port of dates joined on the thread, and the project's
    /// contributions merged into the rules they reach.
    fn joined<R>(&self, f: impl FnOnce() -> R) -> R {
        let set = self.set.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match set {
            Some(chosen) => crate::link::with_contributions(chosen, || crate::days::with(self.dates.clone(), f)),
            None => crate::days::with(self.dates.clone(), f),
        }
    }

    /// The rule at `rule`, checked: read again when its text has changed, or a file its check read
    /// reads otherwise. A report `check` already made of the same text, while what it read reads the
    /// same, says whether the rule passes, so it is not checked a second time.
    fn rule(&self, rule: &Path) -> Result<(String, String, Checked2), Vec<Said>> {
        self.joined(|| self.rule_here(rule)).map(|(path, src, got, _)| (path, src, got))
    }

    /// The rule at `rule` as the engine keeps its check (read, merged with the contributions of the
    /// project the engine was given, and typed), with its path the way the run writes its paths and
    /// its text: what `ritsu gen` makes a rule's module from, so that the rule is checked once in the
    /// run (ritsu's DESIGN 4.15). Err, or an Err inside, is what check says of a rule that cannot be
    /// read or does not pass.
    #[allow(clippy::type_complexity)]
    pub fn prepared(&self, rule: &Path) -> Result<(String, String, std::sync::Arc<Result<(RuleFile, Checked), Vec<Said>>>), Vec<Said>> {
        self.rule(rule)
    }

    /// The rule at `rule` in the JavaScript of a page, in `lang` (`Gen::page_javascript`: its
    /// module, with the module of each `enums` file it reads inside it), made from the checks the
    /// engine keeps of the rule and of those files, which are not checked again (ritsu's DESIGN
    /// 4.15): what `ritsu gen` writes into the functions of a request's screens (ringi's DESIGN
    /// 13.7). Err, or an Err inside, as [`Engine::prepared`].
    pub fn page_javascript(&self, rule: &Path, lang: Lang) -> Result<String, Vec<Said>> {
        let (path, src, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        let enums = |p: &str| {
            let (path, src, got) = self.rule(Path::new(p)).ok()?;
            let (file, checked) = got.as_ref().as_ref().ok()?;
            (file.kind == crate::ast::FileKind::Enums).then(|| crate::enums::EnumsFile { file: file.clone(), checked: checked.clone(), src, path })
        };
        Ok(self.joined(|| i18n::with(rlang(lang), || crate::codegen::Gen::new(f, c, &src, &path).page_javascript_with(&enums))))
    }

    /// What stops the code `target` writes of the rule at `rule` into a package (§15.335): E153, each
    /// W121 of the rule's check that names the target as one whose code would not build or would
    /// answer otherwise, but one that does so only where `rulec gen` writes the rule's module beside
    /// the language's own (`places::package_stops`), as `rulec gen` says it, in `lang`, each as
    /// [`Engine::checked`] gives a finding, its file from `root`. Empty when the rule cannot be read
    /// or nothing stops the target: what `ritsu gen` asks of each rule before it writes it into a
    /// package of `target`. A report kept in either language says whether any W121 names the target,
    /// so the rule is checked again only to say one that does in `lang`.
    pub fn stops(&self, rule: &Path, target: &str, root: &Path, lang: Lang) -> Vec<ritsu_ports::Finding> {
        let Ok((path, src)) = read(rule) else { return Vec::new() };
        // whether a W121 names the target at all: which of those stop it is `package_stops`'s alone
        let names = |r: &crate::Report| r.diags.iter().any(|d| d.code == "W121" && d.targets.contains(&target));
        if self.kept_report(&path, &src, false).is_some_and(|r| !names(&r)) {
            return Vec::new();
        }
        let rl = rlang(lang);
        let r = self.report(&path, &src, rl);
        let ds = i18n::with(rl, || crate::places::package_stops(&r.diags, target));
        let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
        let file = ritsu_base::paths::from_root(root, Path::new(&path));
        ds.iter().map(|d| finding(d, &path, file.clone(), &lines, rl)).collect()
    }

    /// [`Engine::rule`], with what the check read.
    fn rule_here(&self, rule: &Path) -> Result<(String, String, Checked2, std::sync::Arc<Seen>), Vec<Said>> {
        let (path, src) = read(rule)?;
        let key = (the_file(&path), crate::sha256::hex(src.as_bytes()));
        let mut cache = self.checked.lock().unwrap_or_else(|e| e.into_inner());
        let kept = cache.entry(key).or_default();
        if let Some((seen, got)) = kept.iter().find(|(seen, _)| seen.still()) {
            return Ok((path, src, got.clone(), seen.clone()));
        }
        // whether the rule passes, read off a report `check` made of the same text in either
        // language under any spelling of its path, or off one the thread kept in either language
        // (the check of a rule another applies, of an `enums` file a rule imports: `kept_verdict`),
        // else off one made now in the thread's language, which the rule's own findings are read
        // off when the run shows them after (§15.320)
        let (got, mut seen) = watched(|| {
            let r = self
                .kept_report(&path, &src, false)
                .or_else(|| self.joined(|| crate::kept_verdict(&src, &path, self.budget()).map(std::sync::Arc::new)))
                .unwrap_or_else(|| self.report(&path, &src, i18n::current()));
            std::sync::Arc::new(if crate::has_error(&r.diags) { Err(self.refused(&path, &src)) } else { crate::prepare(&src, &path).map_err(|_| self.refused(&path, &src)) })
        });
        // the answer rests on what the report's check read as well
        for l in [RLang::Ja, RLang::En] {
            if let Some((_, was, _)) = self.reports.lock().unwrap_or_else(|e| e.into_inner()).get(&(the_file(&path), crate::sha256::hex(src.as_bytes()), l)) {
                seen.extend(was);
            }
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
        let key = (the_file(path), crate::sha256::hex(src.as_bytes()), lang);
        if let Some((p, seen, r)) = self.reports.lock().unwrap_or_else(|e| e.into_inner()).get(&key)
            && p == path
            && seen.still()
        {
            return r.clone();
        }
        self.checks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (r, seen) = self.joined(|| watched(|| i18n::with(lang, || crate::report_with(src, path, self.budget()))));
        let r = std::sync::Arc::new(r);
        self.reports.lock().unwrap_or_else(|e| e.into_inner()).insert(key, (path.to_string(), std::sync::Arc::new(seen), r.clone()));
        r
    }

    /// A report kept for the rule's text, in either language, while what it read reads the same:
    /// with `named`, only one that names the file `path`, as its findings do (whether the rule
    /// passes is the same under any spelling of its path).
    fn kept_report(&self, path: &str, src: &str, named: bool) -> Option<std::sync::Arc<crate::Report>> {
        let reports = self.reports.lock().unwrap_or_else(|e| e.into_inner());
        [RLang::Ja, RLang::En]
            .into_iter()
            .find_map(|l| reports.get(&(the_file(path), crate::sha256::hex(src.as_bytes()), l)).filter(|(p, seen, _)| (!named || p == path) && seen.still()).map(|(_, _, r)| r.clone()))
    }

    /// What check says of a rule that does not pass, each error in both languages: the reports kept
    /// for its text in each ([`Engine::report`]).
    fn refused(&self, path: &str, src: &str) -> Vec<Said> {
        let (ja, en) = (self.report(path, src, RLang::Ja), self.report(path, src, RLang::En));
        ja.diags
            .iter()
            .zip(&en.diags)
            .filter(|(d, _)| d.severity == crate::diag::Severity::Error)
            .map(|(j, e)| Said { code: j.code.to_string(), file: path.to_string(), line: j.line(), message: Text::new(j.title.clone(), e.title.clone()) })
            .collect()
    }

    /// How many times the engine has checked a rule's text whole, for `check` or for a port's
    /// answer: once for each rule in a run that asks the same rule both ways (ritsu's DESIGN 6.1).
    pub fn checks(&self) -> usize {
        self.checks.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The files the errors of the rule at `path` are at, as its findings name them (one about a
    /// line of a contribution merged into the rule names the contribution, §15.227), each once, as
    /// the program reaches them: what ritsu reads of a rule whose errors [`Engine::checked`] said
    /// for a file above (`note …: its errors are said above`), which says none of them, to find the
    /// files it fails for (ritsu's DESIGN 8.3). The report kept for the rule is read, in the language
    /// it was made in, else one made now in the thread's language (§15.320); empty when the file
    /// cannot be read.
    pub fn errors_at(&self, path: &str) -> Vec<String> {
        let Ok(src) = ritsu_base::fs::read_to_string(path) else { return Vec::new() };
        let r = self.kept_report(path, &src, true).unwrap_or_else(|| self.report(path, &src, i18n::current()));
        let mut out: Vec<String> =
            r.diags.iter().filter(|d| d.severity == crate::diag::Severity::Error).map(|d| d.marks.first().and_then(|m| m.span.file_path()).unwrap_or(path).to_string()).collect();
        out.sort();
        out.dedup();
        out
    }

    /// `rulec check` of each file, as `ritsu check` prints it (ritsu's DESIGN 8.3): every finding,
    /// as the command prints it and as its `--format json` prints it, then what the command prints
    /// after them (`ok rules/送料.rule`). `files` are as the person gave them, from where the
    /// program runs; `root` is the project's, which the findings' files are written from.
    pub fn checked(&self, root: &Path, files: &[String], lang: Lang) -> Vec<ritsu_ports::Checked> {
        use ritsu_ports::{Checked, Part, Verdict};
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
            .map(|given| {
                // the rule named the way the run writes its paths, as every other question names it
                // (ritsu's DESIGN 4.15), so the report made here is the one they read
                let path = &ritsu_base::paths::show_str(given);
                // a file `ritsu check` cannot read is said in the words every command of ritsu says it in
                let src = match ritsu_base::fs::read_to_string(path) {
                    Ok(src) => src,
                    Err(e) => return Checked::unchecked(given, i18n::with(rl, || format!("{}\n", ritsu_base::cli::refusal(&i18n::text(&ritsu_base::cli::cannot_read(path, &e)), i18n::base())))),
                };
                let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
                let r = self.report(path, &src, rl);
                let file = ritsu_base::paths::from_root(root, Path::new(path));
                let shown: Vec<&crate::diag::Diag> = r.diags.iter().filter(|d| said.insert(crate::finding_key(d, path))).collect();
                let mut parts: Vec<Part> = shown.iter().copied().map(|d| Part::Finding(finding(d, path, file.clone(), &lines, rl))).collect();
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
                // `rulec check --format json` prints a line for each diagnostic, and no object of a
                // file
                Checked { label: given.clone(), parts, verdict, json: ritsu_base::json::Json::Null }
            })
            .collect()
    }

    /// The facts of a rule already read and checked.
    pub fn facts_of(&self, f: &RuleFile, c: &Checked, src: &str, path: &str) -> Result<RuleFacts, Vec<Said>> {
        let g = crate::codegen::Gen::new(f, c, src, path);
        let (typescript, python, go, rust) = g.calls();
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
            // what a workflow writes of the rule it calls: `fed by` moves nothing generated (§15.285)
            sha256: ritsu_base::fed::hash(src),
            rulec: env!("CARGO_PKG_VERSION").to_string(),
            inputs: f.inputs.iter().map(|i| column(c, &i.name)).collect(),
            outputs: f.outputs.iter().map(|o| output_column(c, o)).collect(),
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
            rust,
        })
    }

    /// The outputs for these inputs, as rulec's reference evaluator answers them, refusing what the
    /// generated code refuses at its door ([`ritsu_ports::Rules::eval`]); `seen` takes the value it
    /// refuses the call for, where that is one the rule works out outside the range it accepts for
    /// it ([`ritsu_ports::Rules::eval_outside`]).
    fn answer(&self, rule: &Path, inputs: &Values, seen: &mut Option<Outside>) -> Result<Values, RuleError> {
        use crate::eval::Val;
        let (_, _, got) = self.rule(rule).map_err(RuleError::Unread)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| RuleError::Unread(e.clone()))?;
        // An input with two or more things wrong with it is refused for the first of them, in the
        // order every door looks (§15.328, `door::read`), each said in the port's own words.
        let mut vals = std::collections::HashMap::new();
        // The first look: each input there and of its kind, then each list and each field of each
        // of its elements.
        for i in &f.inputs {
            let name = &i.name.text;
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            let v = inputs.iter().find(|(n, _)| n == name).map(|(_, v)| v).ok_or_else(|| RuleError::Input(ritsu_base::tr!("入力 `{name}` がありません", "the input `{name}` is missing")))?;
            // `none`, which the evaluator holds as the word itself
            if matches!((&ty, v), (Ty::Opt(_), Value::None)) {
                vals.insert(name.clone(), Val::Enum(crate::kw::NONE.into()));
                continue;
            }
            vals.insert(name.clone(), to_val(c, name, &ty, v).map_err(RuleError::Input)?);
        }
        let mut lengths: Vec<usize> = Vec::new();
        for el in &f.elements {
            let name = &el.name.text;
            let Some((_, Value::List(xs))) = inputs.iter().find(|(n, _)| n == name) else {
                return Err(RuleError::Input(ritsu_base::tr!("並び `{name}` がありません", "the list `{name}` is missing")));
            };
            let mut seq = Vec::new();
            for (k, x) in xs.iter().enumerate() {
                let mut one = std::collections::BTreeMap::new();
                for fd in &el.fields {
                    let fname = &fd.name.text;
                    // A field is said at its place on the list, `lines[1].amount`, as every door
                    // says it (§15.324).
                    let said = crate::door::element(name, k, fname);
                    let ty = c.ty_of(fname).unwrap_or(Ty::Unknown);
                    let v = x.iter().find(|(n, _)| n == fname).map(|(_, v)| v).ok_or_else(|| RuleError::Input(ritsu_base::tr!("要素のフィールド `{said}` がありません", "an element's field `{said}` is missing")))?;
                    if matches!((&ty, v), (Ty::Opt(_), Value::None)) {
                        one.insert(fname.clone(), Val::Enum(crate::kw::NONE.into()));
                        continue;
                    }
                    one.insert(fname.clone(), to_val_as(c, fname, &said, &ty, v).map_err(RuleError::Input)?);
                }
                seq.push(one);
            }
            lengths.push(seq.len());
            vals.insert(name.clone(), Val::Seq(seq));
        }
        // The second look: each input inside its range, held as the generated code's door holds it.
        for i in &f.inputs {
            let name = &i.name.text;
            if crate::door::ranged(c, name, name, &c.ty_of(name).unwrap_or(Ty::Unknown), &vals[name]).is_err() {
                return Err(RuleError::Input(outside(name, name)));
            }
        }
        // Two dates compare by their day numbers, as the generated code's door compares them
        // (§15.198).
        for k in &f.constraints {
            if !crate::vectors::constraint_holds(k, &|n| vals.get(n)) {
                return Err(RuleError::Input(ritsu_base::tr!("`{}` が成り立ちません", "`{}` does not hold", k.said())));
            }
        }
        for n in f.door_nevers() {
            if !crate::vectors::never_clear(n, c, &|m| vals.get(m)) {
                let said = n.said();
                return Err(RuleError::Input(ritsu_base::tr!("起こらない組み合わせです: {said}", "this combination does not happen: {said}")));
            }
        }
        // A date of koyomi's days takes those days only, and the door refuses any other
        // (§15.174): after the constraints and name by name, where the generated code looks.
        let mut names: Vec<&String> = c.day_sets.keys().collect();
        names.sort();
        for name in names {
            let set = &c.day_sets[name];
            if let Some(Val::Date(y, m, d)) = vals.get(name)
                && set.days.binary_search(&(crate::types::date_ord(*y, *m, *d).num as i64)).is_err()
            {
                let (file, date) = set.from.as_ref().map(|fr| (fr.file.clone(), fr.date.clone())).unwrap_or_default();
                return Err(RuleError::Input(ritsu_base::tr!(
                    "入力 `{name}` は、{file} の {date} がとる日ではありません",
                    "the input `{name}` is not a day {date} of {file} comes to"
                )));
            }
        }
        // No longer than the smallest top a `count` over it writes, as the generated code's door
        // holds it (§15.58).
        for (el, n) in f.elements.iter().zip(&lengths) {
            let name = &el.name.text;
            if let Some(cap) = crate::door::count_cap(f, c, name).filter(|cap| *n as i128 > *cap) {
                return Err(RuleError::Input(ritsu_base::tr!("並び `{name}` の要素が多すぎます（{cap} 件まで）", "the list `{name}` has too many elements (at most {cap})")));
            }
        }
        // What the function works out, in the order it does: on the walk, each element's fields
        // held to their ranges and its `unique` fields (§15.290, §15.251), the values worked out
        // outside the range the rule accepts for them (§15.313), weights whose total passes int64
        // (§15.226), a `sum` or a `count` outside its range once the walk is over (§15.100,
        // §15.314), a drawn value not below the weights, and the rule's own contradiction, two
        // elements both taking under `take_unique` (§15.56). Elements are said by their places, the
        // earlier first (§15.324).
        use crate::eval::Refused;
        match crate::eval::walked_refusal(f, c, vals.clone()) {
            None => {}
            Some(Refused::Element(s, crate::door::ElementRefused::Range { field, at })) => {
                return Err(RuleError::Input(outside(&field, &crate::door::element(&f.elements[s].name.text, at, &field))));
            }
            Some(Refused::Element(s, crate::door::ElementRefused::Unique { field, before, here })) => {
                let name = &f.elements[s].name.text;
                let (a, b) = (crate::door::place(name, before), crate::door::place(name, here));
                return Err(RuleError::Input(ritsu_base::tr!(
                    "並び `{name}` の要素に、`{field}` が同じものが二つあります（`{a}` と `{b}`）",
                    "the list `{name}` has two elements with the same `{field}` (`{a}` and `{b}`)"
                )));
            }
            Some(Refused::Outside(o)) => {
                let o = Outside::Define { at: o.at.map(|(s, k)| (f.elements[s].name.text.clone(), k)), name: o.name };
                let t = o.text();
                *seen = Some(o);
                return Err(RuleError::Input(t));
            }
            Some(Refused::Heavy(seq)) => {
                return Err(RuleError::Input(ritsu_base::tr!(
                    "並び `{seq}` の重みの合計が int64 に収まりません",
                    "the total of the weights of the list `{seq}` does not fit in int64"
                )));
            }
            Some(Refused::Past(p)) => {
                let o = Outside::Summary { name: p.name };
                let t = o.text();
                *seen = Some(o);
                return Err(RuleError::Input(t));
            }
            Some(Refused::Drawn(name)) => {
                return Err(RuleError::Input(ritsu_base::tr!(
                    "入力 `{name}` が、重みの合計より小さくありません",
                    "the input `{name}` is not below the total of the weights"
                )));
            }
            Some(Refused::Contradiction(b)) => {
                if let Some(crate::ast::Item::Fold(fold)) = f.items.get(b.item) {
                    let verdict = &fold.verdict;
                    let (a, h) = (crate::door::place(&fold.over, b.before), crate::door::place(&fold.over, b.here));
                    return Err(RuleError::Contradiction(ritsu_base::tr!(
                        "畳み込み `{verdict}` で、`take_unique` に二つの要素が当たりました（`{a}` と `{h}`）",
                        "two elements matched a `take_unique` of the fold `{verdict}` (`{a}` and `{h}`)"
                    )));
                }
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
}

/// The value of an input on the wire, as rulec's evaluator holds it.
fn to_val(c: &Checked, name: &str, ty: &Ty, v: &Value) -> Result<crate::eval::Val, Text> {
    to_val_as(c, name, name, ty, v)
}

/// [`to_val`], with the sentences calling the value `said`: for a field of an element, its place
/// on the list, `lines[1].amount` (rulec's §15.324).
fn to_val_as(c: &Checked, name: &str, said: &str, ty: &Ty, v: &Value) -> Result<crate::eval::Val, Text> {
    use crate::eval::Val;
    let wrong = || ritsu_base::tr!("`{said}` に {ty} として読めない値が渡されています", "`{said}` is given a value that is not a {ty}");
    match (ty, v) {
        (Ty::Opt(t), v) => to_val_as(c, name, said, t, v),
        (Ty::Bool, Value::Bool(b)) => Ok(Val::Bool(*b)),
        (Ty::Str, Value::Str(s)) => Ok(Val::Str(s.clone())),
        (Ty::Enum(e), Value::Enum(x)) => {
            if c.enums.get(e).is_some_and(|vs| vs.contains(x)) {
                Ok(Val::Enum(x.clone()))
            } else {
                Err(ritsu_base::tr!("`{said}` の `{x}` は {e} の値ではありません", "`{x}` of `{said}` is not a value of {e}"))
            }
        }
        // `YYYY-MM-DD` of a day the calendar has, as the generated code's door reads a date: a
        // day it does not have (2026-02-30) was read into a date before (§15.251)
        (Ty::Date, Value::Date(d)) => match crate::door::civil(d) {
            Some((y, m, dd)) => Ok(Val::Date(y, m, dd)),
            None => Err(wrong()),
        },
        // A whole number int64 does not hold is past what a reader of every language takes, refused
        // where it is read and before the values after it, as the door refuses it (rulec's §15.298,
        // §15.328).
        (t, Value::Int(n)) if t.is_numeric() && (*n > i64::MAX as i128 || *n < i64::MIN as i128) => Err(outside(name, said)),
        (t, Value::Int(n)) if t.is_numeric() => Ok(Val::Num(crate::types::from_wire(*n, c.wire_scale(name)))),
        _ => Err(wrong()),
    }
}

/// The port's sentence of a value outside its range: an input by its name, and a field of an
/// element at its place on the list, `lines[1].amount` (`said`).
fn outside(name: &str, said: &str) -> Text {
    if name == said {
        ritsu_base::tr!("入力 `{name}` が範囲の外です", "the input `{name}` is outside its range")
    } else {
        ritsu_base::tr!("要素のフィールド `{said}` が範囲の外です", "an element's field `{said}` is outside its range")
    }
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

/// The range a call gives a name, each end absent where it is open.
type RangeOf<'a> = dyn Fn(&str) -> (Option<i128>, Option<i128>) + 'a;

/// Whether `l op r` holds between two rationals.
/// A `never` line over ranges (§15.282). One condition no value in its range meets keeps the line
/// whatever the others do. Every condition met inside a closed range is an example that breaks it,
/// and everything else cannot be told from ranges: a truth value or an enum has none, and an open
/// end does not say a value goes there.
///
/// A caller that knows every value it gives a truth value or an enum's input (`given`: the
/// values, each one of the input's own) is read the same way: a condition none of them meets
/// keeps the line, one all of them meet is met (the first of them is the example's), and one only
/// some meet cannot be told, as with no values at all.
fn never_kept(said: &str, conds: &[ritsu_ports::NeverCond], range: &RangeOf, given: &dyn Fn(&str) -> Option<Vec<Value>>) -> Answer<Values> {
    use ritsu_ports::NeverTest;
    let mut example: Vec<(String, Value)> = Vec::new();
    let mut open = false;
    for k in conds {
        if let Some(values) = given(&k.name) {
            let met: Vec<Option<bool>> = values.iter().map(|v| k.test.holds(v)).collect();
            if met.iter().all(|m| *m == Some(false)) {
                return Answer::Holds;
            }
            match values.first() {
                Some(first) if met.iter().all(|m| *m == Some(true)) => example.push((k.name.clone(), first.clone())),
                _ => open = true,
            }
            continue;
        }
        let (lo, hi) = range(&k.name);
        // The whole numbers of the range that meet the condition, as far as they can be told.
        let one: Option<Option<i128>> = match &k.test {
            NeverTest::Compare(ops) => {
                let (mut a, mut b) = (lo, hi);
                for (op, v) in ops {
                    match op.as_str() {
                        ">=" => a = Some(a.map_or(*v, |x| x.max(*v))),
                        ">" => a = Some(a.map_or(*v + 1, |x| x.max(*v + 1))),
                        "<=" => b = Some(b.map_or(*v, |x| x.min(*v))),
                        _ => b = Some(b.map_or(*v - 1, |x| x.min(*v - 1))),
                    }
                }
                match (a, b) {
                    (Some(x), Some(y)) if x > y => Some(None),
                    (Some(x), Some(_)) if lo.is_some() && hi.is_some() => Some(Some(x)),
                    _ => None,
                }
            }
            NeverTest::Among { values, yes: true } if !values.is_empty() && values.iter().all(|v| matches!(v, Value::Int(_))) => {
                let inside = |n: i128| lo.is_none_or(|x| n >= x) && hi.is_none_or(|y| n <= y);
                let first = values.iter().find_map(|v| if let Value::Int(n) = v { inside(*n).then_some(*n) } else { None });
                match first {
                    None => Some(None),
                    Some(n) if lo.is_some() && hi.is_some() => Some(Some(n)),
                    Some(_) => None,
                }
            }
            _ => None,
        };
        match one {
            Some(None) => return Answer::Holds,
            Some(Some(n)) => example.push((k.name.clone(), Value::Int(n))),
            None => open = true,
        }
    }
    if open {
        Answer::Undecided(ritsu_base::tr!(
            "渡す値の範囲だけでは、`{said}` の組み合わせにならないとは言えません",
            "ranges alone cannot show that the values given are never the combination of `{said}`"
        ))
    } else {
        Answer::Fails(example)
    }
}

fn holds(l: Rat, op: &str, r: Rat) -> bool {
    use std::cmp::Ordering::*;
    match (op, l.cmp_to(r)) {
        ("<=", Less | Equal) | ("<", Less) | (">=", Greater | Equal) | (">", Greater) => true,
        _ => false,
    }
}

/// One finding of a rule as `ritsu check` and the port's `findings` hand it over: its text as `rulec
/// check` prints it in `rl` and its object as `rulec check --format json` writes it (with the inputs
/// of an example, the rows and the fix), naming the file `path`; `file` is the file the finding
/// names, from the project's root.
fn finding(d: &crate::diag::Diag, path: &str, file: Option<String>, lines: &[String], rl: RLang) -> ritsu_ports::Finding {
    let json = ritsu_base::json::parse(&crate::diag::render_json(d, path)).unwrap_or(ritsu_base::json::Json::Null);
    ritsu_ports::Finding {
        code: d.code.to_string(),
        severity: match d.severity {
            crate::diag::Severity::Error => ritsu_base::diag::Severity::Error,
            crate::diag::Severity::Warning => ritsu_base::diag::Severity::Warning,
        },
        file,
        line: d.line(),
        text: i18n::with(rl, || crate::findings_text(std::slice::from_ref(d), lines)),
        json,
    }
}

impl ritsu_ports::Rules for Engine {
    /// The text and every file the check read, from the check kept for the files read now.
    fn rests_on(&self, rule: &Path) -> Option<String> {
        let (_, src, _, seen) = self.joined(|| self.rule_here(rule)).ok()?;
        Some(rests_on(&seen, rule, &src))
    }

    fn meaning(&self, rule: &Path) -> Option<String> {
        let (_, src, _) = self.rule(rule).ok()?;
        Some(format!("meaning:{}", crate::link::pin::meaning(&src)))
    }

    fn facts(&self, rule: &Path) -> Result<RuleFacts, Vec<Said>> {
        let (path, src, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        self.facts_of(f, c, &src, &path)
    }

    /// The report `check` makes of the rule, the one `ritsu check` prints (kept for the text and
    /// the files it read), every diagnostic of it, the ones about another file too (a contribution).
    fn findings(&self, rule: &Path, lang: Lang) -> Result<ritsu_ports::Found<Vec<ritsu_ports::Finding>>, Vec<Said>> {
        let path = rule.to_string_lossy().to_string();
        let src = ritsu_base::fs::read_to_string(&path).map_err(|e| vec![Said::unreadable(&path, &e)])?;
        let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
        let rl = rlang(lang);
        let r = self.report(&path, &src, rl);
        // a rule that passes is kept as read and checked too, by its file and text while what it
        // read reads the same, as a question of it keeps it ([`Engine::rule`]): what is asked of
        // it after, through other files that read the same (the package `ritsu gen` makes of the
        // files a request is lowered to, laid apart from the ones its check laid), is answered from
        // this check (ritsu's DESIGN 4.15)
        if !crate::has_error(&r.diags) {
            let _ = self.rule(rule);
        }
        Ok(ritsu_ports::Found::Value(r.diags.iter().map(|d| finding(d, &path, Some(path.clone()), &lines, rl)).collect()))
    }

    /// A relation between two inputs is decided over the box of their ranges: it holds for every
    /// pair exactly when it holds for the pair at the corner that tries it hardest, and that pair
    /// is the example when it does not. The bounds on a list's total and length are decided for a
    /// list as long as `max_len`, and the bottom of a total for one as short as `min_len`; without
    /// the length it needs, a bound is not decided. Which elements a `count` counts is nothing a
    /// range says, so its bottom is decided only where the longest list is shorter than it. The range a `define` is
    /// accepted in is decided over what the value reaches with the inputs held to the ranges
    /// (`over::define_kept`, §15.313), a field of an element taking its own range.
    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)], min_len: Option<i128>, max_len: Option<i128>) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>> {
        self.preconditions_hold_given(rule, ranges, &[], min_len, max_len)
    }

    /// The same, with every value the caller gives some of the truth values and the enums'
    /// inputs: a `never` line is decided over them too (`never_kept`). Values are read only for
    /// an input of truth values or of an enum, and only when each is a value of that input's
    /// type; anything else says nothing of the input, as if no values were given for it.
    fn preconditions_hold_given(
        &self,
        rule: &Path,
        ranges: &[(String, Option<i128>, Option<i128>)],
        values: &[(String, Vec<Value>)],
        min_len: Option<i128>,
        max_len: Option<i128>,
    ) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>> {
        let (path, src, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        let given = |name: &str| -> Option<Vec<Value>> {
            let (_, vs) = values.iter().find(|(n, _)| n == name)?;
            let own: bool = match c.ty_of(name)? {
                Ty::Bool => vs.iter().all(|v| matches!(v, Value::Bool(_))),
                Ty::Enum(e) => {
                    let names = c.enums.get(&e)?;
                    vs.iter().all(|v| matches!(v, Value::Enum(x) if names.contains(x)))
                }
                _ => false,
            };
            own.then(|| vs.clone())
        };
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
                    // `max` and `min` count in the total's own step, which is finer than an
                    // element's on the wire where the total adds up a value worked out on each
                    // element that keeps its fractions (§15.314): the ends of the elements are
                    // added up at that step, and an example gives an element at its own.
                    Precondition::Sum { name, over, of, max, min, .. } => match (max_len, range(of).1) {
                        (Some(n), Some(hi)) => {
                            let at_total = |v: Rat| crate::types::wire_int(v, c.scales.get(name).copied().unwrap_or(1));
                            let top = crate::types::wire_int(hi, c.wire_scale(of));
                            let most = if top > 0 { at_total(hi).saturating_mul(n) } else { 0 };
                            if most <= *max {
                                // The bottom, where the range starts above zero (§15.314). A
                                // list that comes in under it at its longest and largest never
                                // reaches it: that list is the example. Otherwise the total is
                                // smallest with the list as short as it gets and every element
                                // at the bottom of its range (an element is not below zero, and
                                // an empty list comes to 0): at or above the bottom it holds, and
                                // under it that list is the example.
                                match (min, min_len, range(of).0) {
                                    (None, ..) => Answer::Holds,
                                    (Some(lo), ..) if most < *lo => {
                                        let one = vec![(of.clone(), Value::Int(top))];
                                        Answer::Fails(vec![(over.clone(), Value::List(vec![one; if top > 0 { n.clamp(0, 10_000) as usize } else { 0 }]))])
                                    }
                                    (Some(lo), Some(k), Some(b)) => {
                                        let (bottom, k) = (crate::types::wire_int(b, c.wire_scale(of)).max(0), k.max(0));
                                        if at_total(b).max(0).saturating_mul(k) >= *lo {
                                            Answer::Holds
                                        } else {
                                            let one = vec![(of.clone(), Value::Int(bottom))];
                                            Answer::Fails(vec![(over.clone(), Value::List(vec![one; k.clamp(0, 10_000) as usize]))])
                                        }
                                    }
                                    (Some(_), None, _) => Answer::Undecided(ritsu_base::tr!(
                                        "`{name}` は `{over}` の合計の下限でもあります。並びがいちばん短いときの長さが分からないので、決められません",
                                        "`{name}` bounds the total over `{over}` from below as well, and how short the list can be is not known, so it cannot be decided"
                                    )),
                                    (Some(_), Some(_), None) => Answer::Undecided(ritsu_base::tr!(
                                        "`{name}` は `{over}` の `{of}` の合計の下限でもあります。`{of}` の範囲に下限が無いので、決められません",
                                        "`{name}` bounds the total of `{of}` over `{over}` from below as well, and the range of `{of}` has no lower end, so it cannot be decided"
                                    )),
                                }
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
                    // How many elements are counted is between none and all of them, whatever
                    // the ranges of their fields: a range does not show which ones are. A list
                    // shorter than the bottom at its longest never reaches it.
                    Precondition::Count { over, min, .. } if max_len.is_some_and(|n| n < *min) => {
                        Answer::Fails(vec![(over.clone(), Value::List(vec![Vec::new(); max_len.unwrap_or(0).clamp(0, 10_000) as usize]))])
                    }
                    Precondition::Count { name, over, .. } => Answer::Undecided(ritsu_base::tr!(
                        "`{name}` は `{over}` を数えた数の下限です。どの要素が数に入るかは、範囲と長さからは示せません",
                        "`{name}` bounds from below how many elements of `{over}` are counted, and ranges and a length cannot show which ones are"
                    )),
                    Precondition::Length { sequence, max } => match max_len {
                        Some(n) if n <= *max => Answer::Holds,
                        Some(_) => Answer::Fails(vec![(sequence.clone(), Value::List(vec![Vec::new(); (*max + 1).clamp(0, 10_000) as usize]))]),
                        None => Answer::Undecided(ritsu_base::tr!(
                            "`{sequence}` の長さの上限です。並びの長さの上限が分からないので、決められません",
                            "it bounds the length of `{sequence}`, and how long the list can be is not known, so it cannot be decided"
                        )),
                    },
                    // A range says nothing of whether two elements share a value: a list of one element
                    // at most is the only one that cannot have two.
                    Precondition::Unique { sequence, field } => match max_len {
                        Some(n) if n <= 1 => Answer::Holds,
                        _ => Answer::Undecided(ritsu_base::tr!(
                            "`{sequence}` の要素どうしで `{field}` が同じにならないことは、範囲と長さからは示せません",
                            "ranges and a length cannot show that no two elements of `{sequence}` have the same `{field}`"
                        )),
                    },
                    Precondition::Days { input, .. } => Answer::Undecided(ritsu_base::tr!(
                        "`{input}` は koyomi の日付がとる日だけをとります。問い合わせには範囲しか入らないので、渡す日がその日のどれかであることまでは示せません",
                        "`{input}` takes only the days of a koyomi date, and a range alone cannot show that the day given is one of them"
                    )),
                    Precondition::Never { said, conds } => never_kept(
                        said,
                        conds,
                        &|name| {
                            let sc = c.wire_scale(name);
                            let (lo, hi) = range(name);
                            (lo.map(|v| crate::types::wire_int(v, sc)), hi.map(|v| crate::types::wire_int(v, sc)))
                        },
                        &given,
                    ),
                    // What the value reaches with the inputs held to the ranges (§15.313), each
                    // field of an element in its own range; the example names the inputs it is
                    // worked out from, and each list whose fields it reads.
                    Precondition::Define { name, range: said, from, .. } => {
                        let lists: Vec<&String> = f.elements.iter().filter(|e| e.fields.iter().any(|x| from.contains(&x.name.text))).map(|e| &e.name.text).collect();
                        match self.joined(|| crate::over::define_kept(f, c, &src, &path, name, said, ranges)) {
                            Answer::Holds => Answer::Holds,
                            Answer::Undecided(why) => Answer::Undecided(why),
                            Answer::Fails(at) => Answer::Fails(
                                from.iter().chain(lists).filter_map(|n| at.get(n).map(|v| (n.clone(), from_val(c, n, v)))).collect(),
                            ),
                        }
                    }
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
        let over = |l: RLang| self.joined(|| crate::days::over(input, given.clone(), || i18n::with(l, || crate::report(&src, &path).diags)));
        let breaks = |d: &crate::diag::Diag| d.severity == crate::diag::Severity::Error && matches!(d.code, "E101" | "E102" | "E105");
        // the check over the days once; again in the other language only for the sentence of an
        // example
        let en = over(RLang::En);
        if !en.iter().any(breaks) {
            return Ok(Answer::Holds);
        }
        let ja = over(RLang::Ja);
        let first = ja.iter().zip(&en).find(|(d, _)| breaks(d));
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
    /// to) with both ends rounded the way it rounds. Where every row that decides it writes a
    /// number, those numbers. The examples (§15.270) are values the output comes to, each with an
    /// input from the rule's vectors (which take every row, and hold the inputs to their ranges and
    /// constraints) that comes to it ([`examples_of`]).
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

    /// The same reading for several outputs of one call: the combinations the rows some input may
    /// reach write, table by table, and an input for each found in the vectors over the ranges
    /// (`crate::over::outputs_together`). Exact when the two meet.
    fn outputs_together(&self, rule: &Path, outputs: &[String], ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<ritsu_ports::Found<Vec<(Vec<Value>, Values)>>, Vec<Said>> {
        let (path, src, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        Ok(self.joined(|| crate::over::outputs_together(f, c, &src, &path, outputs, ranges)))
    }

    fn eval(&self, rule: &Path, inputs: &Values) -> Result<Values, RuleError> {
        self.answer(rule, inputs, &mut None)
    }

    fn eval_outside(&self, rule: &Path, inputs: &Values) -> Result<Values, (RuleError, Option<Outside>)> {
        let mut seen = None;
        self.answer(rule, inputs, &mut seen).map_err(|e| (e, seen))
    }

    /// The inputs of the vectors `rulec vectors` makes of the rule as its check kept it (no check
    /// of its own), each input as `eval` takes it, the lists it walks among them.
    fn vector_inputs(&self, rule: &Path) -> Result<Vec<Values>, Vec<Said>> {
        let (_, _, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        let names: Vec<&String> = f.inputs.iter().map(|i| &i.name.text).chain(f.elements.iter().map(|e| &e.name.text)).collect();
        Ok(self.joined(|| {
            crate::vectors::generate(f, c).iter().map(|v| names.iter().filter_map(|n| v.input.get(*n).map(|x| ((*n).clone(), from_val(c, n, x)))).collect()).collect()
        }))
    }

    fn doc(&self, rule: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>> {
        // the check kept for the rule: the page is drawn from it, not from a check of its own
        let (path, src, got) = self.rule(rule)?;
        self.joined(|| i18n::with(rlang(lang), || {
            let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
            Ok(if html {
                let js = crate::codegen::Gen::new(&f, &c, &src, &path).page_javascript();
                crate::doc::render_html_named(&f, &c, &src, &path, shown, &js)
            } else {
                crate::doc::render_named(&f, &c, &src, &path, shown)
            })
        }))
    }

    fn diff(&self, old: &ritsu_ports::DiffSide, new: &ritsu_ports::DiffSide, budget: Option<usize>) -> Result<ritsu_ports::Found<ritsu_ports::RuleDiff>, Vec<Said>> {
        rule_diff(self, old, new, budget)
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
    end = m.possibles.iter().map(|p| p.span.line).fold(end, usize::max);
    m.onces.iter().map(|o| o.span.line).fold(end, usize::max)
}

/// The path of a file a rule names, from the root: written from the rule's directory, or from the
/// root of the module of ritsu's it is in for `@<name>/…` (ritsu's DESIGN 17.6); where a `..` of it
/// crosses a symbolic link, the file the system opens there, the one the rule reads (DESIGN 4.15).
fn from_root(root: &Path, file: &str, written: &str) -> Option<String> {
    ritsu_base::modules::join_opened(root, file, written).ok()
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
            return Err(self.refused(&path, &src));
        };
        // Where a value comes from says nothing of what the item means, and a version of it stays
        // the same (§15.285): the definition is read without `fed by`, which keeps the lines.
        let formatted: Vec<String> = crate::fmt::format(&ritsu_base::fed::without(&src)).lines().map(String::from).collect();
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
    /// document a `source` copies (`source … file`), the koyomi date an input takes its days from
    /// (`range from koyomi`, §15.174), and where hand-written code takes an input's value from
    /// (`fed by`, §15.285). An enum, a message or a service of a `.proto` is named from the file's
    /// package.
    fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>> {
        let disk = ritsu_base::paths::on_disk(root, file);
        let (path, src) = read(&disk)?;
        let parsed = crate::parse::parse(&src, &path);
        let Some(f) = parsed.file else {
            return Err(self.refused(&path, &src));
        };
        let contract = |proto: &str| ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(root, proto)).ok().and_then(|s| crate::proto::read(proto, &s).ok());
        // a `.proto`'s name for a message or an enum, from its package: `shop.v1.Order` is `Order`
        let local = |proto: &str, written: &str| -> String {
            match contract(proto).and_then(|p| p.package) {
                Some(p) => written.strip_prefix(&format!("{p}.")).unwrap_or(written).to_string(),
                None => written.to_string(),
            }
        };
        // an enum an `import proto` line names, as `check` finds it: one nested in a message is
        // named after the message (`Order.Status`)
        let enum_path = |proto: &str, written: &str| -> String {
            contract(proto).and_then(|f| f.enum_named(written).ok().map(|e| e.path.clone())).unwrap_or_else(|| local(proto, written))
        };
        let mut out = Vec::new();
        for im in &f.enum_imports {
            let Some(p) = from_root(root, file, &im.file) else { continue };
            let (target, how) = match im.kind {
                crate::ast::EnumSource::Proto => (Naming::file(Tool::Proto, &p).with("enum", enum_path(&p, &im.source)), "import proto"),
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
        for fb in f.inputs.iter().chain(f.elements.iter().flat_map(|el| &el.fields)).filter_map(|i| i.fed.as_ref()) {
            if let Ok(target) = ritsu_base::fed::from_root(root, file, &fb.name) {
                out.push(Reference { line: fb.span.line, target, how: "fed by".into() });
            }
        }
        out.sort_by_key(|r| r.line);
        Ok(out)
    }

    /// What a rule reads (ritsu's DESIGN 6.4): what it names ([`ritsu_ports::Read::of_named`]), but
    /// for the documents its `source` lines copy, which it pins and is not made of (§15.68). A
    /// contribution reads the file it adds to and those it comes after (`to`, `after`, §15.227),
    /// which `references` leaves out. A rule reads the contributions merged into it, which it does
    /// not name: those of the project the engine was given ([`Engine::set_project`]), each from the
    /// root; an engine given no project merges none into any rule. A file out of the root is not
    /// said.
    fn reads(&self, root: &Path, file: &str, named: Vec<Reference>) -> Vec<ritsu_ports::Read> {
        let mut out = ritsu_ports::Read::of_named(named.into_iter().filter(|r| r.how != "source").collect());
        let at = ritsu_base::paths::on_disk(root, file).to_string_lossy().into_owned();
        let top = ritsu_base::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let from_root = |shown: &str| -> Option<String> {
            let rel = ritsu_base::fs::canonicalize(Path::new(shown)).ok()?.strip_prefix(&top).ok()?.to_string_lossy().replace('\\', "/");
            (rel != file).then_some(rel)
        };
        if crate::link::is_contribution_file(&at) {
            let to = crate::link::target_of(&at).map(|(f, _)| f);
            for (line, f) in crate::link::names_of(&at) {
                let how = if to.as_deref() == Some(f.as_str()) { "to" } else { "after" };
                if let Some(rel) = from_root(&f) {
                    out.push(ritsu_ports::Read { line: Some(line), target: Naming::file(Tool::Rulec, rel), how: how.into() });
                }
            }
        }
        let set = self.contributions();
        if !set.is_empty() {
            for c in crate::link::with_contributions(set, || crate::link::merged_into(&at)) {
                if let Some(rel) = from_root(&c) {
                    out.push(ritsu_ports::Read { line: None, target: Naming::file(Tool::Rulec, rel), how: "contribution".into() });
                }
            }
        }
        out
    }
}

impl ritsu_ports::Feeds for Engine {
    /// The inputs and the fields of elements with `fed by` (§15.285), each with what its line takes:
    /// a rule that does not pass its check answers nothing, as every other question of the rule.
    /// Only what this file writes itself: a contribution becomes the rule it adds to, whose lines
    /// name their sources from that rule's directory and are asked of when that rule is.
    fn feeds(&self, root: &Path, file: &str, _ports: &ritsu_ports::Ports) -> Result<Vec<ritsu_ports::Fed>, Vec<Said>> {
        let (path, src, got) = self.rule(&ritsu_base::paths::on_disk(root, file))?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        let Some(own) = crate::parse::parse(&src, &path).file else { return Ok(Vec::new()) };
        let places = own.inputs.iter().map(|i| (None, i)).chain(own.elements.iter().flat_map(|el| el.fields.iter().map(move |fd| (Some(el), fd))));
        if places.clone().all(|(_, i)| i.fed.is_none()) {
            return Ok(Vec::new());
        }
        let facts = self.facts_of(f, c, &src, &path)?;
        let mut out = Vec::new();
        for (el, i) in places {
            let Some(fb) = &i.fed else { continue };
            let n = &i.name.text;
            let columns = match el {
                None => &facts.inputs,
                Some(el) => match facts.elements.iter().find(|(name, ..)| name == &el.name.text) {
                    Some((_, _, fields)) => fields,
                    None => continue,
                },
            };
            let Some(col) = columns.iter().find(|k| &k.name == n) else { continue };
            let takes = takes_of(&facts, c, n, &col.ty);
            out.push(ritsu_ports::Fed {
                line: fb.span.line,
                place: match el {
                    None => Naming::file(Tool::Rulec, file).with("input", n),
                    Some(el) => Naming::file(Tool::Rulec, file).with("elements", &el.name.text).with("field", n),
                },
                takes,
                source: ritsu_base::fed::from_root(root, file, &fb.name),
                written: fb.written.clone(),
            });
        }
        Ok(out)
    }

    /// An input of the rule, for the next place the code gives its value to.
    fn holds(&self, root: &Path, file: &str, thing: &Naming, _ports: &ritsu_ports::Ports) -> Result<Option<ritsu_ports::Carried>, Vec<Said>> {
        let [(kind, n)] = thing.items.as_slice() else { return Ok(None) };
        if kind != "input" {
            return Ok(None);
        }
        let (path, src, got) = self.rule(&ritsu_base::paths::on_disk(root, file))?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        let facts = self.facts_of(f, c, &src, &path)?;
        Ok(facts.inputs.iter().find(|k| &k.name == n).map(|col| takes_of(&facts, c, n, &col.ty)))
    }
}

/// What an input or a field of an element takes: what the facts say, with the days of a koyomi date
/// an input's range is, which they do not say.
fn takes_of(facts: &RuleFacts, c: &Checked, n: &str, ty: &ColumnType) -> ritsu_ports::Carried {
    use ritsu_ports::Carried;
    let takes = Carried::of_column(facts, ty);
    let Some(d) = c.day_sets.get(n) else { return takes };
    let days = Carried::Days(d.days.iter().copied().collect());
    match takes {
        Carried::Date { .. } => days,
        Carried::Opt(t) if matches!(*t, Carried::Date { .. }) => Carried::Opt(Box::new(days)),
        other => other,
    }
}

// ── What two versions of a rule answer differently (`Rules::diff`) ──────

/// [`ritsu_ports::Rules::diff`]: each side read through its own files and checked as `rulec diff`
/// reads it (an unpinned or changed callee let through, `prepare_lenient`), the two compared as
/// `rulec diff` compares them (`vdiff::diff`), and what it found handed over as its JSON says it.
/// The walk's words are English, and Japanese too where it says why a box was not settled or why the
/// two cannot be compared: the walk is made again in Japanese then, which a rule that passes both
/// checks rarely needs.
fn rule_diff(e: &Engine, old: &ritsu_ports::DiffSide, new: &ritsu_ports::DiffSide, budget: Option<usize>) -> Result<ritsu_ports::Found<ritsu_ports::RuleDiff>, Vec<Said>> {
    let side = |s: &ritsu_ports::DiffSide| -> Result<(String, RuleFile, Checked), Vec<Said>> {
        let go = || {
            let (path, src) = read(&s.rule)?;
            let got = e.joined(|| crate::prepare_lenient(&src, &path));
            got.map(|(f, c)| (path.clone(), f, c)).map_err(|_| e.refused(&path, &src))
        };
        match &s.files {
            Some(f) => ritsu_base::fs::with(f.clone(), go),
            None => go(),
        }
    };
    let (_, of, oc) = side(old)?;
    let (path, nf, nc) = side(new)?;
    if of.name.text != nf.name.text {
        let (a, b) = (&of.name.text, &nf.name.text);
        return Err(vec![Said { code: String::new(), file: path, line: None, message: ritsu_base::tr!("別の規則を比べようとしています（`{a}` と `{b}`）", "comparing different rules (`{a}` and `{b}`)") }]);
    }
    let budget = budget.unwrap_or(crate::vdiff::DEFAULT_BUDGET);
    let walk = |l: RLang| e.joined(|| i18n::with(l, || crate::vdiff::diff((&of, &oc), (&nf, &nc), budget)));
    let en = walk(RLang::En);
    let ja = (en.blocked.is_some() || !en.unknown.is_empty()).then(|| walk(RLang::Ja));
    let both = |en: &str, ja: Option<&String>| Text::new(ja.cloned().unwrap_or_else(|| en.to_string()), en.to_string());
    let held = |r: &crate::vdiff::Region| en.axes.iter().zip(r).map(|(ax, sel)| held_of(ax, sel)).collect::<Vec<_>>();
    Ok(ritsu_ports::Found::Value(ritsu_ports::RuleDiff {
        rule: en.rule.clone(),
        old_version: en.old_version.clone(),
        new_version: en.new_version.clone(),
        columns: en
            .axes
            .iter()
            .map(|ax| ritsu_ports::DiffColumn {
                name: ax.col.clone(),
                kind: match ax.kind {
                    crate::vdiff::Kind::Input => ritsu_ports::DiffKind::Input,
                    crate::vdiff::Kind::Derived => ritsu_ports::DiffKind::Derived,
                    crate::vdiff::Kind::Walk => ritsu_ports::DiffKind::Walk,
                },
                date: ax.date,
                rate: ax.shown == 100,
                unit: ax.unit.clone(),
            })
            .collect(),
        declared: en.domain.iter().map(|d| ritsu_ports::DiffDeclared { what: d.what.to_string(), name: d.name.clone(), old: d.old.clone(), new: d.new.clone() }).collect(),
        changes: en
            .changes
            .iter()
            .map(|ch| ritsu_ports::DiffChange {
                held: held(&ch.region),
                outputs: ch.outs.clone(),
                uniform: ch.uniform,
                old_rows: ch.old_rows.clone(),
                new_rows: ch.new_rows.clone(),
                witness: ch.witness.iter().map(|(k, v)| (k.clone(), crate::vectors::show(v))).collect(),
                cells: ch.cells,
            })
            .collect(),
        unknown: en
            .unknown
            .iter()
            .enumerate()
            .map(|(i, (r, why))| ritsu_ports::DiffUnknown { held: held(r), why: both(why, ja.as_ref().and_then(|j| j.unknown.get(i)).map(|(_, w)| w)) })
            .collect(),
        cells: en.cells,
        feasible: en.feasible,
        same: en.same,
        differing: en.differing,
        unsettled: en.unsettled,
        unrealized: en.unrealized,
        over_budget: en.over_budget,
        blocked: en.blocked.as_ref().map(|b| both(b, ja.as_ref().and_then(|j| j.blocked.as_ref()))),
    }))
}

/// What a box holds of one axis, as the JSON's `accepts` says it: every value, the words, or the
/// numbers in runs, a run of coordinates next to each other joined as the text joins them.
fn held_of(ax: &crate::vdiff::Axis, sel: &std::collections::BTreeSet<usize>) -> ritsu_ports::DiffHeld {
    use crate::vdiff::Coord;
    if sel.len() == ax.len() {
        return ritsu_ports::DiffHeld::Any;
    }
    let numeric = sel.iter().any(|&i| matches!(ax.coords[i], Coord::Point(_) | Coord::Open(..)));
    if !numeric {
        let words = sel.iter().filter_map(|&i| if let Coord::Word(w) = &ax.coords[i] { Some(w.clone()) } else { None }).collect();
        let other = sel.iter().any(|&i| matches!(ax.coords[i], Coord::OtherStr));
        return ritsu_ports::DiffHeld::Words { words, other };
    }
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for &i in sel {
        match runs.last_mut() {
            Some((_, end)) if *end + 1 == i => *end = i,
            _ => runs.push((i, i)),
        }
    }
    ritsu_ports::DiffHeld::Runs(runs.into_iter().map(|(a, b)| (crate::vdiff::ival(&ax.coords[a], ax.step).0, crate::vdiff::ival(&ax.coords[b], ax.step).1)).collect())
}
