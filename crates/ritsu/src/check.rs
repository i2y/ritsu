//! `ritsu check` (DESIGN 8.1, 8.3, 8.4): every file of a project checked by its language's own
//! `check`, the languages that give facts first, each with the languages it reads joined; then
//! what ritsu checks across the languages; then one line that sums it up. The text is what each
//! language's `check` prints, with the tool's word in each headline (`error[rulec E032]`), since
//! the codes of the languages overlap; the JSON is one object, each language's diagnostic in its
//! own JSON with the tool and the file from the root put outside it.
//!
//! In a module (DESIGN 17.8), the tree of modules is read first: what stops the run stops it (exit
//! 2), and its errors are said with no language's check run (exit 1). A check of the whole module
//! checks the files of every dependency too, says their errors as the module's own, counts their
//! warnings on a line of their own (each is shown with `--deps-warnings`), and runs none of their
//! claims.

pub mod modules;

use crate::cli;
use ritsu_base::diag::Severity;
use ritsu_base::json::Json;
use ritsu_base::modules::Problem;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_ports::{Checked, Finding, Part, Verdict};
use ritsu_cross::Borders;
use ritsu_project::{File, Joined, ORDER, Project};
use std::io::Write;
use std::path::Path;

/// How a file of the project came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Passes,
    Fails,
    Unchecked,
}

/// Everything `ritsu check` says of a project.
pub struct Report<'a> {
    pub project: &'a Project,
    /// What each language's `check` said, unit by unit, in the order they were checked.
    pub units: Vec<(Tool, Checked)>,
    /// What ritsu says itself, across the languages, and of the modules.
    pub ours: Vec<Finding>,
    pub borders: Borders,
    /// The lines before the summary (DESIGN 17.6, 17.8): each contribution that adds to a rule of
    /// another module, each module a path gives in place of a registry's version, each module a
    /// walk passed over.
    pub notes: Vec<Text>,
    /// The contributions that add to a rule of another module, by their paths from the root.
    pub crossings: Vec<modules::Crossing>,
    /// `--deps-warnings`: each warning of a dependency's file is shown, not only counted.
    pub deps_warnings: bool,
    /// How many claims the `.geas` files of the dependencies make, which were not run, by file.
    pub claims_not_run: Vec<(String, usize)>,
    /// The tree of modules could not be read: no file was checked.
    pub unread: bool,
}

/// The languages whose `check` takes one file at a time: whatever their check of a file comes
/// to is that file's.
const ONE_FILE: [Tool; 7] = [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Geas, Tool::Kaname, Tool::Dandori, Tool::Sekisho];

impl Report<'_> {
    fn findings(&self) -> impl Iterator<Item = (&str, &Finding)> {
        self.units.iter().flat_map(|(t, u)| u.findings().map(move |f| (t.word(), f))).chain(self.ours.iter().map(|f| ("ritsu", f)))
    }

    /// Whether this is a check of a whole module that has dependencies (DESIGN 17.8): the files of
    /// the dependencies are checked with the module's, and said apart from them.
    fn with_dependencies(&self) -> bool {
        self.project.whole && matches!(self.project.tree.as_deref(), Some(Ok(t)) if t.nodes.len() > 1)
    }

    /// The dependency a file (from the root) is in, in a check of a whole module: `@pricing`.
    fn dependency(&self, rel: Option<&str>) -> Option<String> {
        if !self.with_dependencies() {
            return None;
        }
        self.project.dependency_of(rel?)
    }

    /// Whether the text and the JSON leave a finding out: a warning (or a note) of a dependency's
    /// file, in a check of a whole module without `--deps-warnings`. It is counted all the same.
    fn hidden(&self, f: &Finding) -> bool {
        !self.deps_warnings && f.severity != Severity::Error && self.dependency(f.file.as_deref()).is_some()
    }

    /// Whether the text leaves out what a unit of a dependency prints when it passes (`ok
    /// @pricing/rules/fee.rule`), with `--deps-warnings` too: its label is a dependency's file, or a
    /// dependency itself (yuen's project of a module). `ritsu check @pricing` prints them.
    fn quiet(&self, u: &Checked) -> bool {
        u.verdict == Verdict::Passes
            && self.with_dependencies()
            && ritsu_base::paths::from_root(&self.project.root, Path::new(&u.label)).is_some_and(|r| self.project.dependency_of(&r).is_some())
    }

    fn status(&self, f: &File) -> Status {
        let unit = |t: Tool| self.units.iter().filter(move |(ut, _)| *ut == t).map(|(_, u)| u);
        let own = ONE_FILE.contains(&f.tool);
        if (own && unit(f.tool).any(|u| u.label == f.shown && u.verdict == Verdict::Unchecked)) || (!own && unit(f.tool).any(|u| u.verdict == Verdict::Unchecked)) {
            return Status::Unchecked;
        }
        let error_in_it = self.findings().any(|(_, x)| x.severity == Severity::Error && x.file.as_deref() == Some(f.rel.as_str()));
        // A file of a dependency whose check failed only for what a contribution of another file
        // adds to it passes: the errors are the contribution's (DESIGN 17.8).
        let for_others = |u: &Checked| self.dependency(Some(&f.rel)).is_some() && self.project.failed_for_others(&f.rel, u);
        if error_in_it || (own && unit(f.tool).any(|u| u.label == f.shown && u.verdict == Verdict::Fails && !for_others(u))) {
            return Status::Fails;
        }
        // A map (or a yuen project) given by this file that fails for a file of no language's,
        // such as a crate's Cargo.toml (sakai's DESIGN 7.7): the failure is the file's given.
        let in_project = |x: &Finding| x.file.as_deref().is_some_and(|p| self.project.holds(p).is_some());
        if !own && unit(f.tool).any(|u| u.label == f.shown && u.verdict == Verdict::Fails && !u.findings().any(|x| x.severity == Severity::Error && in_project(x))) {
            return Status::Fails;
        }
        Status::Passes
    }

    /// 0 when nothing is wrong (warnings may be printed), 1 when a language or ritsu found an
    /// error, 2 when a file could not be checked (DESIGN 8.4).
    pub fn code(&self) -> u8 {
        if self.units.iter().any(|(_, u)| u.verdict == Verdict::Unchecked) {
            2
        } else if self.units.iter().any(|(_, u)| u.verdict == Verdict::Fails) || self.findings().any(|(_, f)| f.severity == Severity::Error) {
            1
        } else {
            0
        }
    }

    /// What a person reads: each language's text, ritsu's own findings, the notes, the lines of
    /// the dependencies, and the line that sums them up.
    pub fn text(&self, lang: Lang) -> String {
        let mut out = String::new();
        for (t, u) in &self.units {
            let quiet = self.quiet(u);
            for p in &u.parts {
                match p {
                    Part::Finding(f) if !self.hidden(f) => out.push_str(&f.text_of(t.word())),
                    Part::Text(s) if !quiet => out.push_str(s),
                    _ => {}
                }
            }
        }
        for f in self.ours.iter().filter(|f| !self.hidden(f)) {
            out.push_str(&f.text_of("ritsu"));
        }
        for line in self.notes.iter().chain(&self.dependency_line()).chain(&self.claims_line()) {
            out.push_str(line.get(lang));
            out.push('\n');
        }
        out.push_str(self.summary().get(lang));
        out.push('\n');
        out
    }

    /// How the files `files` came out, with the findings `pick` takes counted, in both languages:
    /// `all pass (2 warnings)`, `1 fail (1 error), 1 pass (2 warnings)`.
    ///
    /// Each finding is counted with the files that came out as the file it is about did: a warning
    /// of a file that passes is counted with the files that pass, whether or not another file fails
    /// (`1 fail (1 error), 1 pass (2 warnings)`), as it is when every file passes (`all pass (2
    /// warnings)`). A finding of no file of the project is counted with the ones that fail: when
    /// every file passes and such a finding is an error (a name that starts with `@` right under the
    /// root of a module, E307), the files do not all pass as the check goes, and the count is said
    /// bare (`1 error`), with the files that pass after it if they have findings of their own.
    fn came_out(&self, files: &[&File], pick: &dyn Fn(&Finding) -> bool) -> (String, String) {
        let statuses: Vec<Status> = files.iter().map(|f| self.status(f)).collect();
        let fails = statuses.iter().filter(|s| **s == Status::Fails).count();
        let unchecked = statuses.iter().filter(|s| **s == Status::Unchecked).count();
        let passes = statuses.iter().filter(|s| **s == Status::Passes).count();
        let passing: Vec<&str> = files.iter().zip(&statuses).filter(|(_, s)| **s == Status::Passes).map(|(f, _)| f.rel.as_str()).collect();
        let of_passing = |x: &Finding| x.file.as_deref().is_some_and(|p| passing.contains(&p));
        // The counts of the findings `also` takes, in both languages: `(ja, en)`, empty for none.
        let counts = |also: &dyn Fn(&Finding) -> bool| -> (Vec<String>, Vec<String>) {
            let errors = self.findings().filter(|(_, f)| f.severity == Severity::Error && pick(f) && also(f)).count();
            let warnings = self.findings().filter(|(_, f)| f.severity == Severity::Warning && pick(f) && also(f)).count();
            let mut ja = Vec::new();
            let mut en = Vec::new();
            if errors > 0 {
                ja.push(format!("エラー {errors} 件"));
                en.push(ritsu_base::text::plural(errors, "error", "errors"));
            }
            if warnings > 0 {
                ja.push(format!("警告 {warnings} 件"));
                en.push(ritsu_base::text::plural(warnings, "warning", "warnings"));
            }
            (ja, en)
        };
        let error_of_no_file = self.findings().any(|(_, x)| x.severity == Severity::Error && pick(x) && !of_passing(x));
        if fails == 0 && unchecked == 0 && !error_of_no_file {
            return in_brackets("どれも検査を通りました", "all pass", &counts(&|_| true));
        }
        let mut ja = Vec::new();
        let mut en = Vec::new();
        let elsewhere = counts(&|x| !of_passing(x));
        if fails > 0 {
            let (j, e) = in_brackets(&format!("検査を通らないもの {fails} 個"), &format!("{fails} fail"), &elsewhere);
            ja.push(j);
            en.push(e);
        } else if !elsewhere.0.is_empty() {
            ja.push(elsewhere.0.join("、"));
            en.push(elsewhere.1.join(", "));
        }
        let theirs = counts(&of_passing);
        if passes > 0 && !theirs.0.is_empty() {
            let (j, e) = in_brackets(&format!("通るもの {passes} 個"), &format!("{passes} pass"), &theirs);
            ja.push(j);
            en.push(e);
        }
        if unchecked > 0 {
            ja.push(format!("確かめられなかったもの {unchecked} 個"));
            en.push(format!("{unchecked} not checked"));
        }
        (ja.join("、"), en.join(", "))
    }

    /// The last line: how many files of each language, how they came out, and the borders between
    /// the languages ritsu checked. In a check of a whole module, the module's own files and what
    /// is found in them (and in no file); the dependencies have a line of their own.
    fn summary(&self) -> Text {
        if self.unread {
            let (ja, en) = in_brackets("モジュールを読めません", "the modules could not be read", &self.counts_of(&|_| true));
            return Text::new(format!("ritsu check: {ja}。ファイルは検査していません"), format!("ritsu check: {en}; no file was checked"));
        }
        let ours: Vec<&File> = self.project.files.iter().filter(|f| self.dependency(Some(&f.rel)).is_none()).collect();
        let n = ours.len();
        let by_tool = by_tool(&ours);
        let (ja_by, en_by) = (by_tool.join("、"), by_tool.join(", "));
        let (ja, en) = self.came_out(&ours, &|x| self.dependency(x.file.as_deref()).is_none());
        let (checked, undecided) = (self.borders.checked(), self.borders.undecided);
        let files_en = if n == 1 { "file" } else { "files" };
        Text::new(
            format!("ritsu check: ファイル {n} 個（{ja_by}）。{ja}。言語の境目: 確かめた {checked} か所、決められない {undecided} か所"),
            format!("ritsu check: {n} {files_en} ({en_by}): {en}; borders between the languages: {checked} checked, {undecided} undecided"),
        )
    }

    /// The counts of the errors and the warnings `pick` takes, in both languages.
    fn counts_of(&self, pick: &dyn Fn(&Finding) -> bool) -> (Vec<String>, Vec<String>) {
        let errors = self.findings().filter(|(_, f)| f.severity == Severity::Error && pick(f)).count();
        let warnings = self.findings().filter(|(_, f)| f.severity == Severity::Warning && pick(f)).count();
        let mut ja = Vec::new();
        let mut en = Vec::new();
        if errors > 0 {
            ja.push(format!("エラー {errors} 件"));
            en.push(ritsu_base::text::plural(errors, "error", "errors"));
        }
        if warnings > 0 {
            ja.push(format!("警告 {warnings} 件"));
            en.push(ritsu_base::text::plural(warnings, "warning", "warnings"));
        }
        (ja, en)
    }

    /// The line of the dependencies, in a check of a whole module (DESIGN 17.8): how many modules
    /// and files, how they came out, and the warnings of each dependency, which `--deps-warnings`
    /// shows one by one.
    fn dependency_line(&self) -> Option<Text> {
        if !self.with_dependencies() {
            return None;
        }
        let Some(Ok(tree)) = self.project.tree.as_deref() else { return None };
        let theirs: Vec<&File> = self.project.files.iter().filter(|f| self.dependency(Some(&f.rel)).is_some()).collect();
        let (m, k) = (tree.nodes.len() - 1, theirs.len());
        let by_tool = by_tool(&theirs);
        let (ja_by, en_by) = if by_tool.is_empty() { (String::new(), String::new()) } else { (format!("（{}）", by_tool.join("、")), format!(" ({})", by_tool.join(", "))) };
        let of_theirs = |x: &Finding| self.dependency(x.file.as_deref()).is_some();
        let (ja, en) = if k == 0 {
            let (cj, ce) = self.counts_of(&of_theirs);
            if cj.is_empty() { (String::new(), String::new()) } else { (format!("。{}", cj.join("、")), format!(": {}", ce.join(", "))) }
        } else {
            let (j, e) = self.came_out(&theirs, &of_theirs);
            (format!("。{j}"), format!(": {e}"))
        };
        let modules_en = if m == 1 { "module" } else { "modules" };
        let files_en = if k == 1 { "file" } else { "files" };
        let mut t = Text::new(format!("依存: モジュール {m} 個、ファイル {k} 個{ja_by}{ja}"), format!("dependencies: {m} {modules_en}, {k} {files_en}{en_by}{en}"));
        let each: Vec<(String, usize)> = tree.nodes[1..]
            .iter()
            .map(|n| (n.prefix.clone(), self.findings().filter(|(_, f)| f.severity == Severity::Warning && self.dependency(f.file.as_deref()).as_deref() == Some(n.prefix.as_str())).count()))
            .filter(|(_, w)| *w > 0)
            .collect();
        if !each.is_empty() {
            let ja: Vec<String> = each.iter().map(|(p, w)| format!("{p} {w} 件")).collect();
            let en: Vec<String> = each.iter().map(|(p, w)| format!("{p} {w}")).collect();
            t.ja.push_str(&format!("。依存ごとの警告: {}", ja.join("、")));
            t.en.push_str(&format!("; warnings by dependency: {}", en.join(", ")));
            if !self.deps_warnings {
                t.ja.push_str("（一つずつは `--deps-warnings` で出ます）");
                t.en.push_str(" (`--deps-warnings` shows each)");
            }
        }
        Some(t)
    }

    /// The line of the claims of the dependencies' `.geas` files, which were not run (DESIGN 17.8).
    fn claims_line(&self) -> Option<Text> {
        if self.claims_not_run.is_empty() {
            return None;
        }
        let k = self.claims_not_run.len();
        let n: usize = self.claims_not_run.iter().map(|(_, c)| c).sum();
        let (files_en, claims_en) = (if k == 1 { "file" } else { "files" }, if n == 1 { "claim" } else { "claims" });
        Some(Text::new(
            format!("依存: .geas のファイル {k} 個の主張 {n} 件は、走らせていません。依存の主張は、依存の作者のプログラムについてのものです"),
            format!("dependencies: {n} {claims_en} in {k} .geas {files_en}, not run: a dependency's claims are about its author's programs"),
        ))
    }

    /// The JSON (DESIGN 8.3): the version, the root as seen from where ritsu runs, whether all is
    /// well, every file with its language and whether it passes, every diagnostic, and what the
    /// checks of the borders between the languages came to. When the root has a `ritsu.mod`
    /// (DESIGN 17.21), the module at hand and its dependencies, the dependency each file and each
    /// diagnostic is in, and the contributions that add to a rule of another module.
    pub fn json(&self) -> Json {
        let module = self.project.tree.is_some();
        let dependency = |rel: Option<&str>| -> Option<Json> { module.then(|| Json::opt_str(rel.and_then(|r| self.project.dependency_of(r)))) };
        let files = self.project.files.iter().filter(|_| !self.unread).map(|f| {
            let mut o: Vec<(String, Json)> = vec![("tool".into(), Json::str(f.tool.word())), ("file".into(), Json::str(&f.rel))];
            o.extend(dependency(Some(&f.rel)).map(|d| ("dependency".to_string(), d)));
            o.push(("ok".into(), Json::Bool(self.status(f) == Status::Passes)));
            Json::Obj(o)
        });
        let diagnostics = self.findings().filter(|(_, f)| !self.hidden(f)).map(|(t, f)| entry_in(t, f, dependency(f.file.as_deref())));
        let mut o: Vec<(String, Json)> = vec![
            ("ritsu".into(), Json::str(env!("CARGO_PKG_VERSION"))),
            ("root".into(), Json::str(self.project.root_shown())),
            ("ok".into(), Json::Bool(self.code() == 0)),
        ];
        if module {
            o.push(("module".into(), self.module_json()));
            o.push(("dependencies".into(), Json::arr(self.dependencies_json())));
        } else if !self.project.skipped.is_empty() {
            o.push(("skipped_modules".into(), Json::arr(self.project.skipped.iter().map(Json::str))));
        }
        o.push(("files".into(), Json::arr(files)));
        o.push(("diagnostics".into(), Json::arr(diagnostics)));
        if module {
            let contributions = self.crossings.iter().map(|c| {
                Json::obj([
                    ("file", Json::str(&c.file)),
                    ("dependency", Json::opt_str(self.project.dependency_of(&c.file))),
                    ("to", Json::str(&c.to)),
                    ("to_dependency", Json::opt_str(self.project.dependency_of(&c.to))),
                ])
            });
            o.push(("contributions".into(), Json::arr(contributions)));
        }
        o.push((
            "borders".into(),
            Json::obj([("held", Json::int(self.borders.held as i128)), ("failed", Json::int(self.borders.failed as i128)), ("undecided", Json::int(self.borders.undecided as i128))]),
        ));
        Json::Obj(o)
    }

    /// The module at hand: its name and version, from its `ritsu.mod` (null when that does not
    /// read).
    fn module_json(&self) -> Json {
        let manifest = match self.project.tree.as_deref() {
            Some(Ok(t)) => Some(t.main().manifest.clone()),
            _ => ritsu_base::fs::read(self.project.root.join(ritsu_base::modules::manifest::FILE))
                .ok()
                .and_then(|b| ritsu_base::modules::manifest::parse(&b, ritsu_base::modules::manifest::FILE).ok()),
        };
        match manifest {
            Some(m) => Json::obj([("name", Json::str(m.module.value.to_string())), ("version", Json::opt_str(m.version.map(|v| v.value.to_string())))]),
            None => Json::Null,
        }
    }

    /// Each dependency of the tree, in the canonical order (DESIGN 17.21): where it comes from and
    /// is read, and how its files came out.
    fn dependencies_json(&self) -> Vec<Json> {
        let Some(Ok(tree)) = self.project.tree.as_deref() else { return Vec::new() };
        tree.nodes[1..]
            .iter()
            .map(|n| {
                let in_it = |rel: Option<&str>| rel.and_then(|r| self.project.dependency_of(r)).as_deref() == Some(n.prefix.as_str());
                let files: Vec<&File> = self.project.files.iter().filter(|f| in_it(Some(&f.rel))).collect();
                let errors = self.findings().filter(|(_, f)| f.severity == Severity::Error && in_it(f.file.as_deref())).count();
                let warnings = self.findings().filter(|(_, f)| f.severity == Severity::Warning && in_it(f.file.as_deref())).count();
                let ok = errors == 0 && files.iter().all(|f| self.status(f) == Status::Passes);
                let claims: usize = self.claims_not_run.iter().filter(|(r, _)| in_it(Some(r))).map(|(_, c)| c).sum();
                let (digest, files_digest) = match n.digests() {
                    Some((d, f)) => (Json::str(d), Json::str(f)),
                    None => (Json::Null, Json::Null),
                };
                Json::obj([
                    ("prefix", Json::str(&n.prefix)),
                    ("name", Json::str(n.name.to_string())),
                    ("version", Json::opt_str(n.version().map(|v| v.to_string()))),
                    ("source", Json::str(n.origin().map_or("path", |o| o.kind()))),
                    ("path", Json::opt_str(n.path.clone())),
                    ("url", Json::opt_str(n.git().map(|(_, u)| u))),
                    ("commit", Json::opt_str(n.git().map(|(c, _)| c))),
                    ("digest", digest),
                    ("files_digest", files_digest),
                    ("where", Json::str(n.at.word())),
                    ("verified", Json::arr(n.verified.iter().map(Json::str))),
                    ("files", Json::int(files.len() as i128)),
                    ("ok", Json::Bool(ok)),
                    ("errors", Json::int(errors as i128)),
                    ("warnings", Json::int(warnings as i128)),
                    ("claims_not_run", Json::int(claims as i128)),
                ])
            })
            .collect()
    }
}

/// `head (counts)`, or `head` alone when there are none.
fn in_brackets(ja: &str, en: &str, (cj, ce): &(Vec<String>, Vec<String>)) -> (String, String) {
    if cj.is_empty() { (ja.to_string(), en.to_string()) } else { (format!("{ja}（{}）", cj.join("、")), format!("{en} ({})", ce.join(", "))) }
}

/// How many of the files are of each language, in the order of [`ORDER`]: `rulec 2`.
fn by_tool(files: &[&File]) -> Vec<String> {
    ORDER.into_iter().map(|t| (t, files.iter().filter(|f| f.tool == t).count())).filter(|(_, k)| *k > 0).map(|(t, k)| format!("{} {k}", t.word())).collect()
}

/// One diagnostic of the JSON: the language's own object, with the tool put first and the file
/// written from the project's root (in place of the language's own `file`, or after the tool when
/// the language writes none). `ritsu gen --format json` writes the diagnostics that stop it so.
pub fn entry(tool: &str, f: &Finding) -> Json {
    entry_in(tool, f, None)
}

/// [`entry`], with the dependency the file is in right after the file (DESIGN 17.21), when there
/// is one to write (`Some(null)` for a file of the module at hand).
fn entry_in(tool: &str, f: &Finding, dependency: Option<Json>) -> Json {
    let file = match &f.file {
        Some(p) => Json::str(p),
        None => Json::Null,
    };
    let mut o: Vec<(String, Json)> = vec![("tool".into(), Json::str(tool))];
    let put_file = |o: &mut Vec<(String, Json)>| {
        o.push(("file".into(), file.clone()));
        if let Some(d) = &dependency {
            o.push(("dependency".into(), d.clone()));
        }
    };
    match &f.json {
        Json::Obj(kv) => {
            if !kv.iter().any(|(k, _)| k == "file") {
                put_file(&mut o);
            }
            for (k, v) in kv {
                if k == "file" {
                    put_file(&mut o);
                } else {
                    o.push((k.clone(), v.clone()));
                }
            }
        }
        _ => {
            put_file(&mut o);
            o.push(("code".into(), Json::str(&f.code)));
            o.push(("severity".into(), Json::str(f.severity.key())));
            o.push(("line".into(), f.line.into()));
        }
    }
    Json::Obj(o)
}

fn refuse(msg: Text, lang: Lang, err: &mut dyn Write) -> u8 {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    let _ = writeln!(err, "{head}: {}", msg.get(lang));
    2
}

/// `ritsu check [<path>...] [--root <dir>] [--format json] [--budget <n>] [--deps-warnings]`: the exit
/// code.
pub fn command(args: &[String], lang: Lang) -> u8 {
    let code = run(args, lang, &mut std::io::stdout(), &mut std::io::stderr());
    let _ = std::io::stdout().flush();
    code
}

/// [`command`], writing what it prints to `out` and `err`: what the binary runs, and the page in
/// the browser on the files it holds in memory (ritsu-wasm).
pub fn run(args: &[String], lang: Lang, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let table = cli::table();
    let cmd = table.command("check").expect("check is in the table");
    // `--lang` anywhere on the line is ritsu's, decided before anything is said
    let asked = args.iter().enumerate().find_map(|(i, x)| if x == "--lang" { args.get(i + 1).cloned() } else { x.strip_prefix("--lang=").map(str::to_string) });
    let lang = if asked.is_some() { Lang::pick(asked.as_deref(), "RITSU_LANG") } else { lang };
    let a = match table.parse(cmd, args) {
        Ok(a) => a,
        Err(e) => return refuse(e, lang, err),
    };
    if a.has("--help") {
        let _ = write!(out, "{}", table.help_cmd(cmd, lang));
        return 0;
    }
    // The budget of the checks that walk a space, the same number for every language that has
    // one (rulec, sekisho); a positive whole number.
    let budget = match a.get("--budget") {
        None => None,
        Some(v) => match v.parse::<u64>() {
            Ok(n) if n > 0 => Some(n),
            _ => return refuse(Text::new(format!("`--budget {v}` は正の整数ではありません"), format!("`--budget {v}` is not a positive integer")), lang, err),
        },
    };
    let project = match Project::load(&a.pos, a.get("--root")) {
        Ok(p) => p,
        Err(e) => return refuse(e, lang, err),
    };
    let deps_warnings = a.has("--deps-warnings");
    let json = a.get("--format") == Some("json");
    // In a module, the tree first (DESIGN 17.8): what stops the run is said and the run stops
    // (exit 2); the tree's errors are said with no language's check run (exit 1).
    match project.tree.clone().as_deref() {
        Some(Err(Problem::Stop(said))) => {
            let head = if lang == Lang::Ja { "エラー" } else { "error" };
            for t in said {
                let _ = writeln!(err, "{head}: {}", t.get(lang));
            }
            return 2;
        }
        Some(Err(Problem::Diags(ds))) => {
            let ours = ds.iter().map(|d| modules::finding(&project, d, lang)).collect();
            let report = Report { project: &project, units: Vec::new(), ours, borders: Borders::default(), notes: Vec::new(), crossings: Vec::new(), deps_warnings, claims_not_run: Vec::new(), unread: true };
            return print(&report, json, lang, out);
        }
        _ => {}
    }
    let project = project.with_dependencies();
    // a spec's claims start process groups; a signal stops them before ritsu goes
    if project.files.iter().any(|f| f.tool == Tool::Geas) {
        geas::proc::stop_groups_on_signals();
    }
    let joined = Joined::with_budget(budget);
    // The project's contributions are merged into the rules they reach (rulec's §15.231), for the
    // checks of the files and for what the checks across the languages ask of a rule. In a check
    // of a whole module, the project holds every module of the tree, so a contribution has its
    // effect wherever it is in the tree (DESIGN 17.8).
    joined.rulec.set_project(project.of(Tool::Rulec).iter().map(|f| f.shown.clone()).collect());
    // kaname decides the effects of its functions with what the checks across the borders came to at
    // their calls (kaname's DESIGN 4.3), and ritsu says those checks with the same decisions
    let verdicts = std::rc::Rc::new(ritsu_cross::ProgramVerdicts::new(&joined));
    let mut units = project.check_with(&joined, Some(verdicts.clone()), lang);
    if project.tree.is_some() {
        project.place_findings(&mut units, lang);
    }
    let crossed = ritsu_cross::check_with(&project, &joined, &verdicts, lang);
    let mut ours = crossed.findings;
    if let Some(Ok(tree)) = project.tree.as_deref() {
        ours.extend(tree.found.iter().map(|d| modules::finding(&project, d, lang)));
        ours.extend(modules::uses(&project, &joined, tree, lang));
    }
    let crossings = modules::crossings(&project);
    let notes = modules::notes(&project, &crossings);
    let claims_not_run = project
        .not_run
        .iter()
        .map(|f| (f.rel.clone(), ritsu_ports::Claims::claims(&*joined.geas, &ritsu_base::paths::on_disk(&project.root, &f.rel)).map(|c| c.len()).unwrap_or(0)))
        .collect();
    let report = Report { project: &project, units, ours, borders: crossed.borders, notes, crossings, deps_warnings, claims_not_run, unread: false };
    print(&report, json, lang, out)
}

/// Prints the report as text or as JSON: the exit code.
fn print(report: &Report, json: bool, lang: Lang, out: &mut dyn Write) -> u8 {
    if json {
        let _ = writeln!(out, "{}", report.json().pretty());
    } else {
        let _ = write!(out, "{}", report.text(lang));
    }
    report.code()
}
