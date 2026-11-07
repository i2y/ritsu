//! `ritsu impact --base <revision> [--head <revision>] [--route <file.rule>]`: whether a change moved
//! any answer and who it concerns, in one place, and where the project's own rule sends it. A rule
//! decides the route, with the facts below as its inputs, so that which changes a person reads is
//! written as a table and checked like any other.
//!
//! Nothing here says what a language means (DESIGN P1, P4). The facts are what the languages already
//! answer: the two checks are `ritsu check` (with no claim run: one side of the change need not be on
//! the disk, and nothing is run or written), the differences are what `ritsu mod diff` says of the two
//! versions, the owners are `yuen affected`'s and the claims are `geas affected`'s, each given the
//! `git diff` of the two sides. What could not be compared is counted and makes exit 2 (P5).
//!
//! The project needs no `ritsu.mod`: its root is found as `ritsu check` finds it.

mod say;

use crate::modules::diff::{self, Module, ModDiff, behaves, tool_of, within};
use crate::modules::fetch;
use crate::modules::report::{self, Counts};
use crate::package;
use ritsu::cli::{FACTS, FactKind};
use ritsu_base::cli::Args;
use ritsu_base::diag::Severity;
use ritsu_base::fs::{Files, GitRevision};
use ritsu_base::json::Json;
use ritsu_base::modules::manifest;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_base::udiff::{self, FileDiff, Mark};
use ritsu_ports::{Claims as _, ColumnType, RuleFacts, Rules as _, Value};
use ritsu_project::Joined;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

/// `ritsu impact`: the exit code.
pub fn command(args: &[String], lang: Lang) -> u8 {
    let table = ritsu::cli::table();
    let cmd = table.command("impact").expect("impact is in the table");
    // `--lang` anywhere on the line is ritsu's, as in `ritsu check`
    let asked = args.iter().enumerate().find_map(|(i, x)| if x == "--lang" { args.get(i + 1).cloned() } else { x.strip_prefix("--lang=").map(str::to_string) });
    let lang = if asked.is_some() { Lang::pick(asked.as_deref(), "RITSU_LANG") } else { lang };
    let a = match table.parse(cmd, args) {
        Ok(a) => a,
        Err(e) => return refuse(e, lang),
    };
    if a.has("--help") {
        print!("{}", table.help_cmd(cmd, lang));
        return 0;
    }
    run(&a, lang)
}

fn refuse(t: Text, lang: Lang) -> u8 {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    eprintln!("{head}: {}", t.get(lang));
    2
}

fn run(a: &Args, lang: Lang) -> u8 {
    let Some(base) = a.get("--base") else {
        return refuse(tr!("`--base <git の版>`（比べる前の版）を付けてください", "`--base <revision>` is required: the revision to compare with"), lang);
    };
    let budget = match diff::budget_of(a) {
        Ok(b) => b,
        Err(t) => return refuse(t, lang),
    };
    let root = match a.get("--root") {
        Some(r) => match ritsu_base::fs::canonicalize(Path::new(r)) {
            Ok(p) if ritsu_base::fs::is_dir(&p) => p,
            _ => return refuse(tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory"), lang),
        },
        None => {
            let found = ritsu_base::paths::find_root(Path::new("."));
            ritsu_base::fs::canonicalize(&found).unwrap_or(found)
        }
    };
    let joined = Joined::with_budget(budget);
    // the rule is read first: what stops it stops the run before anything is walked
    let route = match a.get("--route") {
        Some(file) => match Route::read(file, &joined) {
            Ok(r) => Some(r),
            Err(said) => {
                for t in said {
                    refuse(t, lang);
                }
                return 2;
            }
        },
        None => None,
    };
    let mut i = match Impact::make(&root, base, a.get("--head"), budget, &joined, lang) {
        Ok(i) => i,
        Err(t) => return refuse(t, lang),
    };
    let mut route_stopped = false;
    if let Some(r) = route {
        match r.answer(&i.facts, &joined) {
            Ok(answered) => i.route = Some(answered),
            Err(t) => {
                refuse(t, lang);
                route_stopped = true;
            }
        }
    }
    let code = if route_stopped { 2 } else { i.exit() };
    let out = match a.get("--format") {
        Some("json") => format!("{}\n", i.to_json(lang, code).pretty()),
        Some("markdown") => i.markdown(lang, code),
        _ => i.text(lang, code),
    };
    // what a language says of a file, it says with the path it was given; a comment on a pull request
    // has no use for where the runner keeps the project
    let out = out.replace(&format!("{}/", root.display()), "").replace(&root.display().to_string(), ".");
    let _ = write!(std::io::stdout(), "{out}");
    code
}

// ── What it holds ───────────────────────────────────────────────────────

/// What a changed file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A file of one of ritsu's languages, or a `.proto`.
    Spec,
    /// A file a generator wrote: its head says so.
    Generated,
    /// Code written by hand, by its extension.
    Code,
    /// Prose for people, by its extension and its name ([`is_prose`]).
    Doc,
    Other,
}

impl Kind {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Kind::Spec => "spec",
            Kind::Generated => "generated",
            Kind::Code => "code",
            Kind::Doc => "doc",
            Kind::Other => "other",
        }
    }
}

/// A file that differs between the two sides.
pub(crate) struct Changed {
    pub path: String,
    /// `added`, `removed`, `changed` or `renamed`.
    pub how: &'static str,
    /// The path before, for a file renamed.
    pub from: Option<String>,
    pub kind: Kind,
    pub tool: Option<Tool>,
    /// For code written by hand, the lines that changed; 1 when the diff has no line.
    pub lines: usize,
    /// Whether the diff has no line of the file: a rename with nothing else, a mode.
    pub hunkless: bool,
}

/// One side's check.
#[derive(Default)]
pub(crate) struct Side {
    pub counts: Counts,
    pub diags: Vec<Json>,
    /// What kept the check from saying anything of a file, or of the whole.
    pub trouble: Vec<Text>,
}

/// What the requirements say.
#[derive(Default)]
pub(crate) struct Requirements {
    /// Whether the project's `.req` files were read.
    pub known: bool,
    /// Why not, when they were not.
    pub note: Option<Text>,
    /// The requirements that moved, each as `yuen affected --format json` writes one.
    pub moved: Vec<Json>,
    pub owners: Vec<String>,
    /// The changes no requirement reaches: the file, and why (`unclaimed`, `untraced`, `unnamed`,
    /// `no_requirements`).
    pub unknown: Vec<(String, &'static str)>,
}

/// Lines of code written by hand no claim runs: the file, the side (None when not known), the lines
/// as ranges (None when not known), how many, and why.
pub(crate) type Unclaimed = (String, Option<String>, Option<String>, usize, &'static str);

/// What the records of the claims say of the change.
#[derive(Default)]
pub(crate) struct ClaimsSaid {
    /// Whether every spec has a record that fits the change.
    pub known: bool,
    /// Each spec with why it is not known, when it is not.
    pub specs: Vec<(String, Option<Text>)>,
    /// Why nothing is known, when no spec is there.
    pub note: Option<Text>,
    /// The spec and the claim that were touched, its status, and the lines as `file:side:ranges`.
    pub touched: Vec<(String, String, String, Vec<ritsu_ports::TouchedLines>)>,
    pub unclaimed: Vec<Unclaimed>,
}

/// Whether the generated files were held to what `ritsu gen` writes.
#[derive(Default)]
pub(crate) struct GeneratedSaid {
    pub checked: bool,
    pub stale: Vec<String>,
    pub note: Option<Text>,
}

/// A fact's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fact {
    Count(i128),
    Yes(bool),
}

/// What a route rule answered.
pub(crate) struct Answered {
    pub file: String,
    /// Each output: its name and value as the rule writes them, and its public name and value.
    pub outputs: Vec<(String, String, String, Json)>,
}

pub(crate) struct Impact {
    pub base: String,
    /// The head's revision; None for the working tree.
    pub head: Option<String>,
    pub files: Vec<Changed>,
    pub base_check: Side,
    pub head_check: Side,
    /// The diagnostics only the head has, errors first, each as `ritsu check --format json` writes
    /// it; a book's E050 and E051, and a file's E310, against its old version among them.
    pub new: Vec<Json>,
    /// The diagnostics only the base has.
    pub gone: Vec<Json>,
    pub diff: ModDiff,
    pub requirements: Requirements,
    pub claims: ClaimsSaid,
    pub generated: GeneratedSaid,
    /// What could not be compared or decided: what, and why.
    pub undecided: Vec<(String, Text)>,
    pub facts: Vec<(&'static str, Fact)>,
    pub route: Option<Answered>,
}

impl Impact {
    pub(crate) fn fact(&self, name: &str) -> i128 {
        match self.facts.iter().find(|(n, _)| *n == name) {
            Some((_, Fact::Count(k))) => *k,
            Some((_, Fact::Yes(b))) => i128::from(*b),
            None => 0,
        }
    }

    pub(crate) fn new_errors(&self) -> usize {
        self.new.iter().filter(|d| d.get("severity").and_then(Json::as_str) == Some("error")).count()
    }

    /// 1 for an error only the head has; else 2 when something could not be compared or decided; else
    /// 0. Where a rule sends the change has no part in it.
    pub(crate) fn exit(&self) -> u8 {
        if self.new_errors() > 0 {
            1
        } else if !self.undecided.is_empty() {
            2
        } else {
            0
        }
    }
}

// ── Making it ───────────────────────────────────────────────────────────

impl Impact {
    fn make(root: &Path, base: &str, head: Option<&str>, budget: Option<u64>, joined: &Joined, lang: Lang) -> Result<Impact, Text> {
        let base_rev = GitRevision::open(root, base)?;
        let head_rev = head.map(|h| GitRevision::open(root, h)).transpose()?;
        let bytes = git_diff(root, base_rev.commit(), head_rev.as_ref().map(GitRevision::commit))?;
        let files = udiff::parse(&bytes).map_err(|(line, why)| tr!("`git diff` の出力を読めません（{line} 行目）: {}", "cannot read what `git diff` printed (line {line}): {}", why.ja; why.en))?;
        let base_files: Rc<dyn Files> = Rc::new(base_rev);
        let head_files: Option<Rc<dyn Files>> = head_rev.map(|r| Rc::new(r) as Rc<dyn Files>);
        let mut undecided: Vec<(String, Text)> = Vec::new();

        // what each lock pins, as `ritsu mod report` fetches it
        for (files, rev) in [(Some(base_files.clone()), Some(base)), (head_files.clone(), head)] {
            match within(files, || fetch::tree(root)) {
                Ok(done) => fetch::told(lang, &done),
                Err(t) => {
                    let side = rev.map_or_else(|| "working tree".to_string(), str::to_string);
                    undecided.push((side, t));
                }
            }
        }

        let changed = classify(&files, &base_files, &head_files, root);

        // the two checks, with no claim run
        let base_check = check_side(Some(base_files.clone()), root, budget, joined, lang);
        let head_check = check_side(head_files.clone(), root, budget, joined, lang);
        for (side, name) in [(&base_check, base.to_string()), (&head_check, head.unwrap_or("working tree").to_string())] {
            for t in &side.trouble {
                undecided.push((format!("check: {name}"), t.clone()));
            }
        }

        // what `ritsu mod diff` says of the two sides
        let side_label = |rev: Option<&str>| rev.map_or_else(|| ".".to_string(), str::to_string);
        let old = Module::project(root, side_label(Some(base)), Some(base_files.clone()));
        let now = Module::project(root, side_label(head), head_files.clone());
        let d = match (&old, &now) {
            (Ok(o), Ok(n)) => diff::compared(o, n, budget, lang, joined, true),
            (Err(t), _) | (_, Err(t)) => {
                undecided.push(("ritsu.mod".to_string(), t.clone()));
                ModDiff::default()
            }
        };
        // two sides with no difference answer alike, whatever a file that does not pass says on both
        if !changed.is_empty() {
            for b in &d.behavior {
                match &b.result {
                    Err(t) => undecided.push((b.file.clone(), t.clone())),
                    Ok(s) if !s.total => undecided.push((b.file.clone(), tr!("調べきれなかった入力があります", "some inputs were not settled"))),
                    Ok(_) => {}
                }
            }
            for i in &d.interface {
                if let Some(note) = &i.note
                    && !undecided.iter().any(|(what, _)| *what == i.file)
                {
                    undecided.push((i.file.clone(), note.clone()));
                }
            }
        }
        // a file of a language that compares answers, changed where the walk does not go: a directory
        // with a `ritsu.mod` of its own, an `exclude` line
        if let (Ok(o), Ok(n)) = (&old, &now) {
            for f in changed.iter().filter(|f| f.kind == Kind::Spec && behaves(&f.path).is_some()) {
                if !o.files.contains_key(&f.path) && !n.files.contains_key(&f.path) {
                    undecided.push((f.path.clone(), tr!("このプロジェクトの検査が読まない場所のファイルです（モジュールの中なら、そのディレクトリを `--root` に渡すと比べられます）", "a file where this project's check does not read (in a module of its own, give that directory to `--root` to compare it)")));
                }
            }
        }

        // the new errors: the check's, then each changed book's and each changed file's breaks
        let mut new = only_in(&head_check.diags, &base_check.diags);
        for i in &d.interface {
            for (tool, f) in i.breaks.iter().filter(|(_, f)| f.severity == Severity::Error) {
                new.push(report::break_json_in(None, tool, &i.file, f));
            }
        }
        new.sort_by_key(|d| d.get("severity").and_then(Json::as_str) != Some("error"));
        let gone = only_in(&base_check.diags, &head_check.diags);

        // the files whose answers moved, what the bytes of the diff alone do not say
        let moved: Vec<String> = d.behavior.iter().filter(|b| b.result.as_ref().is_ok_and(|s| s.differs)).map(|b| b.file.clone()).collect();
        let requirements = requirements_of(root, &head_files, &bytes, &moved, &d, &changed, joined, lang);
        if let (false, Some(t)) = (requirements.known, requirements.note.clone())
            && has_file_of(&head_files, root, Tool::Yuen)
        {
            undecided.push(("requirements".to_string(), t));
        }
        let claims = claims_of(root, &head_files, &bytes, &files, &changed, joined);
        let generated = generated_of(root, head, &base_files, lang);

        let facts = facts_of(&changed, &new, &d, &undecided, &requirements, &claims, &generated);
        Ok(Impact { base: base.to_string(), head: head.map(str::to_string), files: changed, base_check, head_check, new, gone, diff: d, requirements, claims, generated, undecided, facts, route: None })
    }
}

/// The unified diff of the two sides, from `root`, as `git diff` writes it whatever the person's
/// configuration says: with the head as the working tree, the files git does not ignore and does not
/// track come as files added.
fn git_diff(root: &Path, base: &str, head: Option<&str>) -> Result<Vec<u8>, Text> {
    let git = |args: &[&str]| -> Result<std::process::Output, Text> {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["-c", "core.quotepath=false", "-c", "diff.noprefix=false", "-c", "diff.mnemonicPrefix=false", "-c", "core.safecrlf=false"])
            .args(args)
            .output()
            .map_err(|e| tr!("git を起動できません（{e}）", "cannot start git ({e})"))
    };
    let mut args = vec!["diff", "--no-color", "--no-ext-diff", "--no-textconv", "--full-index", "-M", "--relative", "--src-prefix=a/", "--dst-prefix=b/", base];
    args.extend(head);
    let o = git(&args)?;
    if !o.status.success() {
        let said = String::from_utf8_lossy(&o.stderr).trim().to_string();
        return Err(tr!("`git diff` が失敗しました: {said}", "`git diff` failed: {said}"));
    }
    let mut bytes = o.stdout;
    if head.is_none() {
        let listed = git(&["ls-files", "--others", "--exclude-standard", "-z"])?;
        for rel in listed.stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let rel = String::from_utf8_lossy(rel).into_owned();
            // exits 1 when the two differ
            let one = git(&["diff", "--no-color", "--no-ext-diff", "--no-textconv", "--full-index", "--no-index", "--src-prefix=a/", "--dst-prefix=b/", "--", "/dev/null", &rel])?;
            bytes.extend(one.stdout);
        }
    }
    Ok(bytes)
}

/// Whether a file's first lines say a generator wrote it: the line every generator of ritsu begins a
/// file with, which other generators write too (`Code generated … DO NOT EDIT.`), or the head of a
/// page for people (`<!-- Generated by sekisho …`, `<!-- rulec … が … から生成した`).
fn says_generated(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(8192)]);
    text.lines().take(10).any(|l| {
        let l = l.trim();
        (l.contains("Code generated ") && l.contains("DO NOT EDIT")) || l.starts_with("<!-- Generated by ") || (l.starts_with("<!-- rulec ") && l.contains("生成した"))
    })
}

/// What a file written by hand in a language with no record of its lines still is: code. The ones
/// geas records are [`geas::tree::is_source`].
const MORE_CODE: [&str; 25] = ["tsx", "jsx", "java", "kt", "kts", "swift", "rb", "php", "c", "h", "cc", "cpp", "hpp", "cs", "scala", "sh", "bash", "zsh", "sql", "lua", "ex", "exs", "dart", "vue", "svelte"];

fn is_code(path: &str) -> bool {
    if geas::tree::excluded(path) {
        return false;
    }
    geas::tree::is_source(path) || path.rsplit_once('.').is_some_and(|(_, e)| MORE_CODE.contains(&e))
}

/// The extensions of prose. Not `.txt`: a list of a language's dependencies and a build's file end in it.
const PROSE: [&str; 4] = ["md", "markdown", "rst", "adoc"];
/// The names in capitals that are only read. Any other name in capitals has a role in how the project
/// is worked on: instructions for whoever changes the code, a licence, a policy.
const PROSE_NAMES: [&str; 4] = ["README", "CHANGELOG", "CHANGES", "CONTRIBUTING"];

/// Whether a file is prose for people: nothing in a directory with a hidden name, where a tool's
/// settings and instructions are.
fn is_prose(path: &str) -> bool {
    let (dirs, name) = path.rsplit_once('/').unwrap_or(("", path));
    if dirs.split('/').any(|d| d.starts_with('.')) || !name.rsplit_once('.').is_some_and(|(_, e)| PROSE.contains(&e)) {
        return false;
    }
    let stem = name.split('.').next().unwrap_or(name);
    stem.chars().any(char::is_lowercase) || PROSE_NAMES.contains(&stem)
}

/// The lines a file's hunks change, blank ones left out: for each run of removed and added lines, the
/// more of the two, so that one line edited is one line.
fn changed_lines(f: &FileDiff) -> usize {
    let mut total = 0;
    for h in &f.hunks {
        let (mut removed, mut added) = (0, 0);
        for l in &h.lines {
            let blank = l.text.iter().all(u8::is_ascii_whitespace);
            match l.mark {
                Mark::Context => {
                    total += removed.max(added);
                    (removed, added) = (0, 0);
                }
                Mark::Removed if !blank => removed += 1,
                Mark::Added if !blank => added += 1,
                _ => {}
            }
        }
        total += removed.max(added);
    }
    total
}

/// The files of the diff, each with what it is, its head read on the side that has it.
fn classify(files: &[FileDiff], base: &Rc<dyn Files>, head: &Option<Rc<dyn Files>>, root: &Path) -> Vec<Changed> {
    let mut out: Vec<Changed> = files
        .iter()
        .map(|f| {
            let path = f.path().to_string();
            let how = if f.added() {
                "added"
            } else if f.deleted() {
                "removed"
            } else if f.old_path != f.new_path {
                "renamed"
            } else {
                "changed"
            };
            let bytes = if f.deleted() { within(Some(base.clone()), || ritsu_base::fs::read(root.join(&path))) } else { within(head.clone(), || ritsu_base::fs::read(root.join(&path))) };
            let tool = tool_of(&path);
            let kind = if bytes.as_deref().is_ok_and(says_generated) {
                Kind::Generated
            } else if tool.is_some() {
                Kind::Spec
            } else if is_code(&path) {
                Kind::Code
            } else if is_prose(&path) {
                Kind::Doc
            } else {
                Kind::Other
            };
            let hunkless = f.hunks.is_empty();
            let lines = match kind {
                Kind::Code if hunkless => 1,
                Kind::Code => changed_lines(f),
                _ => 0,
            };
            Changed { from: (how == "renamed").then(|| f.old_path.clone()).flatten(), path, how, kind, tool, lines, hunkless }
        })
        .collect();
    out.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    out
}

/// The files of a language under the root on one side, from the root, the dependencies' left out.
fn files_of(side: &Option<Rc<dyn Files>>, root: &Path, tool: Tool) -> Vec<String> {
    let mut under = Vec::new();
    within(side.clone(), || ritsu_base::paths::walk(root, ".", &[], &mut under));
    under.retain(|f| tool_of(f) == Some(tool));
    under
}

fn has_file_of(side: &Option<Rc<dyn Files>>, root: &Path, tool: Tool) -> bool {
    !files_of(side, root, tool).is_empty()
}

/// `ritsu check` of one side, its `.geas` files only read (nothing is run: the side may not be on the
/// disk).
fn check_side(files: Option<Rc<dyn Files>>, root: &Path, budget: Option<u64>, joined: &Joined, lang: Lang) -> Side {
    let mut side = Side::default();
    let args = report::check_args(root, budget);
    let ran = report::check_ran(files.clone(), root, lang, |o, e| ritsu_project::project::without_own_claims(|| ritsu::check::run(&args, lang, o, e)));
    if let Some((diags, counts)) = ran.found {
        side.diags = diags;
        side.counts = counts;
    }
    if ran.code > 1 || !ran.said.is_empty() {
        side.trouble = ran.said;
        if side.trouble.is_empty() {
            side.trouble.push(tr!("`ritsu check` が確かめられないファイルがありました", "`ritsu check` could not check a file"));
        }
    }
    let specs: Vec<String> = files_of(&files, root, Tool::Geas).into_iter().map(|f| root.join(f).to_string_lossy().into_owned()).collect();
    let found = within(files, || joined.geas.read_only(root, &specs, lang));
    for f in &found {
        match f.severity {
            Severity::Error => side.counts.errors += 1,
            Severity::Warning => side.counts.warnings += 1,
            _ => {}
        }
        side.diags.push(ritsu::check::entry("geas", f));
    }
    side
}

/// The diagnostics of `a` that `b` does not have. Two with the same tool, code, file and text are one
/// diagnostic wherever each is: a line added above a warning does not make it new. When `a` has more of
/// them than `b`, those at lines `b` does not have them at are the new ones.
fn only_in(a: &[Json], b: &[Json]) -> Vec<Json> {
    type Key = (String, String, String, String);
    let no_line = |d: &Json| -> Key {
        let (t, c, f, _, m) = report::key(d);
        (t, c, f, m)
    };
    let exact: BTreeSet<_> = b.iter().map(report::key).collect();
    let mut left: BTreeMap<Key, usize> = BTreeMap::new();
    for d in b {
        *left.entry(no_line(d)).or_default() += 1;
    }
    let mut kept: Vec<bool> = vec![true; a.len()];
    for (i, d) in a.iter().enumerate() {
        if exact.contains(&report::key(d)) {
            let k = left.entry(no_line(d)).or_default();
            if *k > 0 {
                *k -= 1;
                kept[i] = false;
            }
        }
    }
    for (i, d) in a.iter().enumerate() {
        if kept[i] {
            let k = left.entry(no_line(d)).or_default();
            if *k > 0 {
                *k -= 1;
                kept[i] = false;
            }
        }
    }
    a.iter().zip(kept).filter(|(_, k)| *k).map(|(d, _)| d.clone()).collect()
}

/// `yuen affected` of the head's `.req` files, what `ritsu mod diff` says moved added to it.
#[allow(clippy::too_many_arguments)]
fn requirements_of(root: &Path, head: &Option<Rc<dyn Files>>, bytes: &[u8], moved: &[String], d: &ModDiff, changed: &[Changed], joined: &Joined, lang: Lang) -> Requirements {
    let matter = |f: &&Changed| matches!(f.kind, Kind::Spec | Kind::Code);
    let mut r = Requirements::default();
    if !has_file_of(head, root, Tool::Yuen) {
        r.note = Some(tr!("このプロジェクトに `.req` はありません", "this project has no `.req`"));
        r.unknown = changed.iter().filter(matter).map(|f| (f.path.clone(), "no_requirements")).collect();
    } else {
        let at = root.to_string_lossy().into_owned();
        let said = within(head.clone(), || yuen::affected::of_diff(std::slice::from_ref(&at), Some(&at), bytes, "git diff", moved, &joined.yuen(), lang));
        let read = said.and_then(|v| ritsu_base::json::parse(&v.to_string()).map_err(|_| tr!("`yuen affected` の答えを読めません", "cannot read what `yuen affected` answered")));
        match read {
            Err(t) => {
                // the first thing it said: what stops the `.req` files is in the head's check, in full
                let first = |s: &str| s.lines().next().unwrap_or_default().to_string();
                r.note = Some(Text::new(first(&t.ja), first(&t.en)));
                r.unknown = changed.iter().filter(matter).map(|f| (f.path.clone(), "no_requirements")).collect();
            }
            Ok(v) => {
                r.known = true;
                r.moved = v.get("requirements").and_then(Json::as_arr).map(<[Json]>::to_vec).unwrap_or_default();
                r.owners = v.get("owners").and_then(Json::as_arr).map(|xs| xs.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
                let kind_of = |p: &str| changed.iter().find(|f| f.path == p).map(|f| f.kind);
                for u in v.get("unreached").and_then(Json::as_arr).unwrap_or_default() {
                    let (Some(file), Some(why)) = (u.get("file").and_then(Json::as_str), u.get("why").and_then(Json::as_str)) else { continue };
                    if matches!(kind_of(file), Some(Kind::Spec | Kind::Code) | None) && !r.unknown.iter().any(|(f, _)| f == file) {
                        r.unknown.push((file.to_string(), if why == "unclaimed" { "unclaimed" } else { "untraced" }));
                    }
                }
                for o in v.get("others").and_then(Json::as_arr).unwrap_or_default() {
                    let Some(file) = o.as_str() else { continue };
                    if matches!(kind_of(file), Some(Kind::Spec | Kind::Code)) && !r.unknown.iter().any(|(f, _)| f == file) {
                        r.unknown.push((file.to_string(), "unnamed"));
                    }
                }
            }
        }
    }
    // what the requirements themselves say moved: one the base has and the head does not, say
    let have: BTreeSet<String> = r.moved.iter().filter_map(|j| j.get("name").and_then(Json::as_str).map(str::to_string)).collect();
    for w in d.why.iter().filter(|w| !have.contains(&w.name)) {
        let owner = w.owner.1.clone().or_else(|| w.owner.0.clone());
        if let Some(o) = &owner
            && !r.owners.contains(o)
        {
            r.owners.push(o.clone());
        }
        r.moved.push(Json::obj([
            ("name", Json::str(w.name.as_str())),
            ("version", Json::Null),
            ("file", Json::str(w.file.as_str())),
            ("line", w.line.map_or(Json::Null, Json::int)),
            ("owner", Json::opt_str(owner)),
            ("from", Json::arr([])),
            ("decided", Json::Null),
        ]));
    }
    r
}

/// The lines of ranges as `geas affected` writes them (`3-5`, `3-5,7`).
fn lines_of(ranges: &str) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for part in ranges.split(',').map(str::trim) {
        match part.split_once('-') {
            Some((a, b)) => {
                if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>()) {
                    out.extend(a..=b);
                }
            }
            None => out.extend(part.parse::<u32>().ok()),
        }
    }
    out
}

/// Lines as ranges (`3-5,7`).
fn ranges_of(lines: &BTreeSet<u32>) -> String {
    let mut parts: Vec<(u32, u32)> = Vec::new();
    for &l in lines {
        match parts.last_mut() {
            Some((_, b)) if *b + 1 == l => *b = l,
            _ => parts.push((l, l)),
        }
    }
    parts.iter().map(|(a, b)| if a == b { a.to_string() } else { format!("{a}-{b}") }).collect::<Vec<_>>().join(",")
}

/// `geas affected` of each `.geas` the head has, and the lines of code written by hand no claim runs:
/// where a spec has no record that fits the change, every changed line no other spec's claim runs.
fn claims_of(root: &Path, head: &Option<Rc<dyn Files>>, bytes: &[u8], files: &[FileDiff], changed: &[Changed], joined: &Joined) -> ClaimsSaid {
    let mut c = ClaimsSaid::default();
    let specs = files_of(head, root, Tool::Geas);
    if specs.is_empty() {
        c.note = Some(tr!("このプロジェクトに `.geas` はありません", "this project has no `.geas`"));
    }
    let mut unclaimed: BTreeSet<(String, String, u32, String)> = BTreeSet::new();
    let mut ran: BTreeSet<(String, String, u32)> = BTreeSet::new();
    let mut known = !specs.is_empty();
    for spec in &specs {
        match within(head.clone(), || joined.geas.affected(&root.join(spec), Some(root), bytes, "git diff", &[])) {
            Ok(a) => {
                c.specs.push((spec.clone(), None));
                for t in a.claims {
                    for l in &t.lines {
                        ran.extend(lines_of(&l.lines).into_iter().map(|n| (l.file.clone(), l.side.clone(), n)));
                    }
                    c.touched.push((spec.clone(), t.name, t.status, t.lines));
                }
                for u in a.unclaimed {
                    unclaimed.extend(lines_of(&u.lines).into_iter().map(|n| (u.file.clone(), u.side.clone(), n, u.why.clone())));
                }
            }
            Err(said) => {
                known = false;
                let first = said.first().map_or_else(|| tr!("記録を読めません", "the record cannot be read"), |s| s.message.clone());
                c.specs.push((spec.clone(), Some(first)));
            }
        }
    }
    c.known = known;
    let code: BTreeMap<&str, usize> = changed.iter().filter(|f| f.kind == Kind::Code).map(|f| (f.path.as_str(), f.lines)).collect();
    if known {
        let said: BTreeSet<(&str, &str, u32)> = unclaimed.iter().map(|(f, s, l, _)| (f.as_str(), s.as_str(), *l)).collect();
        // a line added that the record holds no statement on: a table, a constant, a tag. `geas affected`
        // has nothing to look up for it, and what it says can move an answer.
        let mut no_statement: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
        for f in files.iter().filter(|f| !f.deleted() && code.contains_key(f.path()) && geas::tree::is_source(f.path())) {
            for line in added_lines(f) {
                let at = (f.path().to_string(), "after".to_string(), line);
                if !ran.contains(&at) && !said.contains(&(f.path(), "after", line)) {
                    no_statement.entry(f.path().to_string()).or_default().insert(line);
                }
            }
        }
        let mut by: BTreeMap<(String, String, String), BTreeSet<u32>> = BTreeMap::new();
        for (file, side, line, why) in unclaimed {
            if code.contains_key(file.as_str()) && !ran.contains(&(file.clone(), side.clone(), line)) {
                by.entry((file, side, why)).or_default().insert(line);
            }
        }
        for ((file, side, why), lines) in by {
            c.unclaimed.push((file, Some(side), Some(ranges_of(&lines)), lines.len(), if why == "not run" { "not_run" } else { "not_reported" }));
        }
        for (file, lines) in no_statement {
            c.unclaimed.push((file, Some("after".to_string()), Some(ranges_of(&lines)), lines.len(), "no_statement"));
        }
        // a language geas keeps no record of: no claim is known to run it
        for (file, lines) in code.iter().filter(|(f, n)| !geas::tree::is_source(f) && **n > 0) {
            c.unclaimed.push(((*file).to_string(), None, None, *lines, "no_record"));
        }
        // a diff with no line of the file has nothing to look up: a new name can take a file out of a
        // build, and no record says so
        for f in changed.iter().filter(|f| f.kind == Kind::Code && f.hunkless && geas::tree::is_source(&f.path)) {
            c.unclaimed.push((f.path.clone(), None, None, 1, "no_lines"));
        }
    } else {
        for (file, lines) in code.iter().filter(|(_, n)| **n > 0) {
            let by_a_claim = ran.iter().filter(|(f, _, _)| f == file).count();
            let left = lines.saturating_sub(by_a_claim);
            if left > 0 {
                c.unclaimed.push(((*file).to_string(), None, None, left, "no_record"));
            }
        }
    }
    c
}

/// `ritsu gen --check` when the head is the working tree and `ritsu.mod` has a `target` line.
fn generated_of(root: &Path, head: Option<&str>, base: &Rc<dyn Files>, lang: Lang) -> GeneratedSaid {
    let mut g = GeneratedSaid::default();
    if head.is_some() {
        g.note = Some(tr!("head が git の版なので、確かめていません（作業ツリーで走らせると確かめます）", "not checked, for the head is a git revision (run in the working tree, it is)"));
        return g;
    }
    let targets = ritsu_base::fs::read(root.join(manifest::FILE)).ok().and_then(|b| manifest::parse(&b, manifest::FILE).ok()).and_then(|m| m.targets);
    if targets.is_none() {
        g.note = Some(tr!("`ritsu.mod` に `target` の行が無いので、確かめていません", "not checked: `ritsu.mod` has no `target` line"));
        return g;
    }
    let out = root.join("generated");
    if !ritsu_base::fs::is_dir(&out) && !within(Some(base.clone()), || ritsu_base::fs::is_dir(&out)) {
        g.note = Some(tr!("`generated` がどちらの版にも無いので、確かめていません", "not checked: neither side has `generated`"));
        return g;
    }
    match package::fresh(root, lang) {
        package::Fresh::Same => g.checked = true,
        package::Fresh::Stale(files) => {
            g.checked = true;
            g.stale = files;
        }
        package::Fresh::Unknown(t) => g.note = Some(t),
    }
    g
}

/// The lines a file's hunks add, by their number after the change, blank ones and whole-line comments
/// left out.
fn added_lines(f: &FileDiff) -> Vec<u32> {
    let path = f.path();
    let comment: &[&str] = if path.ends_with(".py") { &["#"] } else { &["//"] };
    let mut out = Vec::new();
    for h in &f.hunks {
        let mut new = if h.new_len == 0 { h.new_start + 1 } else { h.new_start };
        for l in &h.lines {
            if l.mark == Mark::Added {
                let text = String::from_utf8_lossy(&l.text);
                let text = text.trim();
                if !text.is_empty() && !comment.iter().any(|c| text.starts_with(c)) {
                    out.push(new);
                }
            }
            if l.mark != Mark::Removed {
                new += 1;
            }
        }
    }
    out
}

/// The file of a language with no diff of behavior that changed: only that it changed is known
/// (`.flow`, `.kn`, `.ctx`, `.geas`, `.proto`). A `.req` says what moved requirement by requirement, and
/// a `.book` what it holds.
fn opaque(tool: Option<Tool>) -> bool {
    matches!(tool, Some(Tool::Dandori | Tool::Kaname | Tool::Sakai | Tool::Geas | Tool::Proto))
}

fn facts_of(changed: &[Changed], new: &[Json], d: &ModDiff, undecided: &[(String, Text)], requirements: &Requirements, claims: &ClaimsSaid, generated: &GeneratedSaid) -> Vec<(&'static str, Fact)> {
    let of_kind = |k: Kind| changed.iter().filter(|f| f.kind == k).count() as i128;
    let severity = |s: &str| new.iter().filter(|d| d.get("severity").and_then(Json::as_str) == Some(s)).count() as i128;
    let (mut rule, mut date, mut gained, mut lost, mut policies, mut book) = (0i128, 0i128, 0i128, 0i128, 0i128, 0i128);
    let total = |j: &Json, k: &str| j.get("total").and_then(|t| t.get(k)).and_then(Json::as_int).unwrap_or(0);
    let regions = |j: &Json| j.get("changes").and_then(Json::as_arr).map_or(0, |c| c.len() as i128);
    for b in &d.behavior {
        let Ok(s) = &b.result else { continue };
        match b.tool {
            Tool::Rulec => rule += regions(&s.json).max(i128::from(s.differs)),
            Tool::Koyomi => date += regions(&s.json).max(i128::from(s.differs)),
            Tool::Sekisho => {
                let (g, l) = (total(&s.json, "gained"), total(&s.json, "lost"));
                gained += g;
                lost += l;
                policies += total(&s.json, "reasons");
                // an action, a role or an attribute only one side has moves no combination's answer
                if s.differs && g + l == 0 {
                    policies += 1;
                }
            }
            _ => {}
        }
    }
    let both = d.files.added.iter().chain(&d.files.removed);
    for f in both.clone() {
        match tool_of(f) {
            Some(Tool::Rulec) => rule += 1,
            Some(Tool::Koyomi) => date += 1,
            Some(Tool::Sekisho) => policies += 1,
            Some(Tool::Chobo) => book += 1,
            _ => {}
        }
    }
    let (mut iface, mut breaks) = (0i128, 0i128);
    for i in &d.interface {
        iface += i.changes.len() as i128;
        let wrong = i.breaks.iter().filter(|(_, f)| f.severity == Severity::Error).count() as i128;
        breaks += i.changes.iter().filter(|c| c.may_break()).count() as i128 + wrong;
        if i.tool == Some(Tool::Chobo) {
            book += i.changes.len() as i128 + i.breaks.len() as i128;
        }
    }
    let languages = |xs: &[String]| xs.iter().filter(|f| tool_of(f).is_some()).count() as i128;
    iface += languages(&d.files.added) + languages(&d.files.removed) + (d.requires.added.len() + d.requires.removed.len() + d.requires.changed.len()) as i128;
    breaks += languages(&d.files.removed) + d.requires.removed.len() as i128;
    let opaque_specs = changed.iter().filter(|f| f.kind == Kind::Spec && opaque(f.tool)).count() as i128;
    let code_lines: usize = changed.iter().filter(|f| f.kind == Kind::Code).map(|f| f.lines).sum();
    let claims_touched = claims.touched.len() as i128;
    let unclaimed_lines: usize = claims.unclaimed.iter().map(|u| u.3).sum();
    // what `ritsu gen --check` holds is `generated/`: elsewhere, the head of a file is all that says a
    // generator wrote it
    let generated_unchecked = changed.iter().filter(|f| f.kind == Kind::Generated && !(generated.checked && f.path.starts_with("generated/"))).count() as i128;
    let value = |name: &str| -> Fact {
        match name {
            "spec_files" => Fact::Count(of_kind(Kind::Spec)),
            "generated_files" => Fact::Count(of_kind(Kind::Generated)),
            "code_files" => Fact::Count(of_kind(Kind::Code)),
            "doc_files" => Fact::Count(of_kind(Kind::Doc)),
            "other_files" => Fact::Count(of_kind(Kind::Other)),
            "code_lines" => Fact::Count(code_lines as i128),
            "new_errors" => Fact::Count(severity("error")),
            "new_warnings" => Fact::Count(severity("warning")),
            "rule_changes" => Fact::Count(rule),
            "date_changes" => Fact::Count(date),
            "allow_gained" => Fact::Count(gained),
            "allow_lost" => Fact::Count(lost),
            "policies_moved" => Fact::Count(policies),
            "book_changes" => Fact::Count(book),
            "interface_changes" => Fact::Count(iface),
            "interface_breaks" => Fact::Count(breaks),
            "undecided" => Fact::Count(undecided.len() as i128),
            "opaque_specs" => Fact::Count(opaque_specs),
            "requirements_moved" => Fact::Count(requirements.moved.len() as i128),
            "requirements_unknown" => Fact::Count(requirements.unknown.len() as i128),
            "claims_touched" => Fact::Count(claims_touched),
            "unclaimed_lines" => Fact::Count(unclaimed_lines as i128),
            "claims_known" => Fact::Yes(claims.known),
            "generated_stale" => Fact::Yes(!generated.stale.is_empty()),
            "generated_checked" => Fact::Yes(generated.checked),
            "generated_unchecked" => Fact::Count(generated_unchecked),
            other => unreachable!("the fact `{other}` has no value"),
        }
    };
    FACTS.iter().map(|(name, _)| (*name, value(name))).collect()
}

// ── The rule that says where a change goes ──────────────────────────────

/// A rule whose inputs are all facts, each of the type of its fact.
struct Route {
    file: String,
    path: PathBuf,
    facts: RuleFacts,
    /// Each input: the fact it takes.
    takes: Vec<&'static str>,
}

fn type_said(t: &ColumnType) -> String {
    match t {
        ColumnType::Bool => "bool".into(),
        ColumnType::Str => "string".into(),
        ColumnType::Date => "date".into(),
        ColumnType::Enum { name, .. } => format!("enum {name}"),
        ColumnType::Num { written, .. } => written.clone(),
        ColumnType::Opt(x) => format!("{}?", type_said(x)),
    }
}

impl Route {
    /// The rule at `file`, when it can take the facts. Err, with each thing that keeps it from it: it
    /// does not pass its check, an input that is no fact (with the names there are), an input of
    /// another type than its fact.
    fn read(file: &str, joined: &Joined) -> Result<Route, Vec<Text>> {
        let path = PathBuf::from(file);
        if !ritsu_base::fs::is_file(&path) {
            return Err(vec![tr!("経路の規則 `{file}` がありません", "there is no route rule `{file}`")]);
        }
        let facts = joined.rulec.facts(&path).map_err(|said| {
            let mut out = vec![tr!("経路の規則 `{file}` は検査を通りません", "the route rule `{file}` does not pass its check")];
            out.extend(said.iter().map(|s| {
                let at = s.line.map_or(String::new(), |l| format!(":{l}"));
                let (ja, en) = (&s.message.ja, &s.message.en);
                let code = &s.code;
                tr!("{file}{at}: {code} {ja}", "{file}{at}: {code} {en}")
            }));
            out
        })?;
        let mut said = Vec::new();
        let mut takes = Vec::new();
        for (name, _, _) in &facts.elements {
            said.push(tr!("経路の規則 `{file}` の並び `{name}` は、事実から渡せません", "the list `{name}` of the route rule `{file}` cannot be given from the facts"));
        }
        for i in &facts.inputs {
            let Some((fact, kind)) = FACTS.iter().find(|(n, _)| *n == i.alias || *n == i.name) else {
                let names = FACTS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ");
                let name = &i.name;
                said.push(tr!(
                    "経路の規則 `{file}` の入力 `{name}` は、事実の名前ではありません。使える名前: {names}",
                    "the input `{name}` of the route rule `{file}` is named after no fact. The names there are: {names}"
                ));
                continue;
            };
            let name = &i.name;
            let ty = type_said(&i.ty);
            let fits = match (kind, &i.ty) {
                (FactKind::Yes, ColumnType::Bool) => true,
                (FactKind::Count, ColumnType::Num { written, .. }) => written == "number",
                _ => false,
            };
            if !fits {
                said.push(match kind {
                    FactKind::Yes => tr!(
                        "経路の規則 `{file}` の入力 `{name}` は {ty} ですが、事実 `{fact}` は真偽（bool）です",
                        "the input `{name}` of the route rule `{file}` is {ty}, and the fact `{fact}` is a yes or no (bool)"
                    ),
                    FactKind::Count => tr!(
                        "経路の規則 `{file}` の入力 `{name}` は {ty} ですが、事実 `{fact}` は数（number）です",
                        "the input `{name}` of the route rule `{file}` is {ty}, and the fact `{fact}` is a count (number)"
                    ),
                });
                continue;
            }
            takes.push(*fact);
        }
        if !said.is_empty() {
            return Err(said);
        }
        Ok(Route { file: file.to_string(), path, facts, takes })
    }

    /// What the rule answers for the facts. Err when a fact is outside the range of its input, which
    /// is said and not rounded, or the rule refuses them.
    fn answer(&self, facts: &[(&'static str, Fact)], joined: &Joined) -> Result<Answered, Text> {
        let file = &self.file;
        let mut inputs: Vec<(String, Value)> = Vec::new();
        for (i, fact) in self.facts.inputs.iter().zip(&self.takes) {
            let value = facts.iter().find(|(n, _)| n == fact).map(|(_, v)| *v).unwrap_or(Fact::Count(0));
            match value {
                Fact::Yes(b) => inputs.push((i.name.clone(), Value::Bool(b))),
                Fact::Count(k) => {
                    if let ColumnType::Num { min, max, .. } = &i.ty
                        && (min.is_some_and(|m| k < m) || max.is_some_and(|m| k > m))
                    {
                        let name = &i.name;
                        let range = [min.map(|m| format!(">={m}")), max.map(|m| format!("<={m}"))].into_iter().flatten().collect::<Vec<_>>().join(" ");
                        return Err(tr!(
                            "事実 `{fact}` の値 {k} は、経路の規則 `{file}` の入力 `{name}` の範囲（{range}）の外です。丸めずに止めます",
                            "the fact `{fact}` is {k}, outside the range ({range}) of the input `{name}` of the route rule `{file}`; it is not rounded"
                        ));
                    }
                    inputs.push((i.name.clone(), Value::Int(k)));
                }
            }
        }
        let out = joined.rulec.eval(&self.path, &inputs).map_err(|e| match e {
            ritsu_ports::RuleError::Input(t) | ritsu_ports::RuleError::Contradiction(t) => t,
            ritsu_ports::RuleError::Unread(_) => tr!("経路の規則 `{file}` を読めません", "cannot read the route rule `{file}`"),
        })?;
        let public = |v: &str| -> String { self.facts.enums.iter().flat_map(|e| &e.values).find(|x| x.name == v).map_or_else(|| v.to_string(), |x| x.public.clone()) };
        let outputs = self
            .facts
            .outputs
            .iter()
            .filter_map(|o| {
                let (_, v) = out.iter().find(|(n, _)| *n == o.name)?;
                let (shown, json) = match v {
                    Value::Bool(b) => (b.to_string(), Json::Bool(*b)),
                    Value::Int(k) => (k.to_string(), Json::int(*k)),
                    Value::Str(s) | Value::Date(s) => (s.clone(), Json::str(s.as_str())),
                    Value::Enum(e) => (e.clone(), Json::str(public(e))),
                    Value::None => ("none".to_string(), Json::Null),
                    Value::List(_) => return None,
                };
                Some((o.name.clone(), shown, o.alias.clone(), json))
            })
            .collect();
        Ok(Answered { file: file.clone(), outputs })
    }
}
