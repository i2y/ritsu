//! `ritsu mod report --base <revision> [--head <revision>]` (DESIGN 17.10): what raising the
//! dependencies changes, for a pull request. The files of the module at hand stay as the head has
//! them; only the dependencies change, from those the base's lock pins to those the head's pins.
//!
//! 1. The two trees of modules (each revision's `ritsu.mod` and `ritsu.lock`, and the modules a path
//!    reaches as that revision has them), compared module by module: added, removed, upgraded,
//!    downgraded, from another source, a tag that moved (the same version, another digest), and a
//!    module a path reaches whose files changed.
//! 2. The check of the module at hand (`ritsu check --deps-warnings`) twice: with the head's lock,
//!    and with the base's (the head's files, the base's `ritsu.mod` and `ritsu.lock`, the modules a
//!    path reaches as the base has them). A diagnostic only the first has is new, one only the
//!    second has is gone (by the tool, the code, the file, the line and the text). Each new one goes
//!    to the changed modules it is about: the module of its file, else the changed modules its file
//!    reads, through other files too. A changed book's E050 and E051 (DESIGN 17.9.5) are new errors
//!    of its module.
//! 3. For each module that changed, what `ritsu mod diff` says of its two versions; and each file of
//!    the module at hand that has a diff of behavior (a rule, a dates file, a calendar, a gate) and
//!    reads a changed module, compared as the base's lock reads it and as the head's does: the
//!    answers of yours that change.
//!
//! What the two locks pin is fetched first, as `ritsu mod fetch --rev <base>` and `ritsu mod fetch`
//! (or `--rev <head>`) fetch it ([`fetch::tree`]), the base's with the base's key files; and before
//! a changed module's two versions are compared, what the lock of each pins (each is read as a
//! module of its own). Nothing is asked of a registry when the store has them; what was fetched is
//! said on the standard error, and what stops a fetch of the two locks stops the report (exit 2).
//!
//! The exit code: 1 for an error with the head's lock (and with `--fail-on-change`, a difference
//! shown in behavior); else 2 when a module that changed could not be compared, which the report
//! says in that module's place and counts at the top; else 0.

use super::{Ctx, fetch};
use super::diff::{self, Behavior, ModDiff, Module, Spot};
use ritsu_base::cli::Args;
use ritsu_base::diag::Severity;
use ritsu_base::fs::{Disk, Files, GitRevision, Kind, Meta};
use ritsu_base::json::Json;
use ritsu_base::modules::graph::{self, Node, Problem, Tree};
use ritsu_base::modules::{digest, lock, manifest, store};
use ritsu_base::text::{Lang, Text, capitalize, plural};
use ritsu_base::tr;
use ritsu_project::Joined;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

/// The most characters of the Markdown: a comment of a pull request holds 65,536.
pub const LIMIT: usize = 60_000;

/// The most rows of a table of behavior or of an interface in the Markdown.
const ROWS: usize = 50;

/// `ritsu mod report`: the exit code. `--base` is required, and this function is the one to say so.
pub fn report(ctx: &Ctx, a: &Args) -> u8 {
    let Some(base) = a.get("--base") else {
        return ctx.refuse(tr!("`--base <git の版>`（比べる前の版）を付けてください", "`--base <revision>` is required: the revision to compare with"));
    };
    let budget = match diff::budget_of(a) {
        Ok(b) => b,
        Err(t) => return ctx.refuse(t),
    };
    let root = match ctx.root() {
        Ok(r) => r,
        Err(t) => return ctx.refuse(t),
    };
    let r = match Report::make(&root, base, a.get("--head"), budget, ctx.lang) {
        Ok(r) => r,
        Err(said) => {
            for t in said {
                ctx.refuse(t);
            }
            return 2;
        }
    };
    let code = r.exit(a.has("--fail-on-change"));
    let out = match ctx.format.as_str() {
        "json" => format!("{}\n", r.to_json(ctx.lang, code).pretty()),
        "markdown" => r.markdown(ctx.lang, code),
        _ => r.text(ctx.lang, code),
    };
    let _ = write!(io::stdout(), "{out}");
    code
}

// ── What a report holds ────────────────────────────────────────────────

/// How many errors and warnings a check found.
#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub errors: usize,
    pub warnings: usize,
}

/// How a module of the tree changed between the two revisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Added,
    Removed,
    Upgraded,
    Downgraded,
    /// From a registry on one side and a path on the other, or from another path.
    Source,
    /// The same version from a registry, with another digest: the tag moved.
    Moved,
    /// A module a path reaches, at the same version, whose files changed.
    Changed,
}

impl Change {
    /// The word of the JSON.
    pub fn word(self) -> &'static str {
        match self {
            Change::Added => "added",
            Change::Removed => "removed",
            Change::Upgraded => "upgraded",
            Change::Downgraded => "downgraded",
            Change::Source => "source",
            Change::Moved => "moved",
            Change::Changed => "changed",
        }
    }

    fn said(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Change::Added, Lang::Ja) => "足した",
            (Change::Removed, Lang::Ja) => "消した",
            (Change::Upgraded, Lang::Ja) => "上げた",
            (Change::Downgraded, Lang::Ja) => "下げた",
            (Change::Source, Lang::Ja) => "取り先が変わった",
            (Change::Moved, Lang::Ja) => "タグが動いた",
            (Change::Changed, Lang::Ja) => "ファイルが変わった",
            (Change::Added, _) => "added",
            (Change::Removed, _) => "removed",
            (Change::Upgraded, _) => "upgraded",
            (Change::Downgraded, _) => "downgraded",
            (Change::Source, _) => "from another source",
            (Change::Moved, _) => "the tag moved",
            (Change::Changed, _) => "its files changed",
        }
    }
}

/// A module's version on one side, and the digest of its OCI manifest for one from a registry.
#[derive(Clone, Debug)]
pub struct Pin {
    pub version: Option<String>,
    pub digest: Option<String>,
}

/// A module that changed.
pub struct Changed {
    pub module: String,
    /// How the files of the module at hand reach it (`@pricing`, `@pricing/@tax`): the head's, else
    /// the base's.
    pub prefix: String,
    pub change: Change,
    pub old: Option<Pin>,
    pub new: Option<Pin>,
    /// The new diagnostics about it, by their places in [`Report::new`].
    pub diagnostics: Vec<usize>,
    /// What `ritsu mod diff` says of its two versions; None for a module added or removed.
    pub diff: Option<Result<ModDiff, Text>>,
    /// The files of the module at hand that read it, each compared as the two locks read it.
    pub yours: Vec<Behavior>,
}

/// What a report says.
pub struct Report {
    pub base: String,
    /// The head's revision; None for the working tree.
    pub head: Option<String>,
    pub base_check: Counts,
    pub head_check: Counts,
    /// The diagnostics only the check with the head's lock has, as `ritsu check --format json`
    /// writes them, and each E050 and E051 of a changed book against its old version: errors first.
    pub new: Vec<Json>,
    /// How many of [`Report::new`] are a book's E050 and E051, which the check does not count.
    pub breaks: usize,
    /// The diagnostics only the check with the base's lock has.
    pub gone: Vec<Json>,
    pub modules: Vec<Changed>,
}

impl Report {
    /// The errors with the head's lock: the check's, and each E050 and E051 of a changed book.
    pub fn errors(&self) -> usize {
        self.head_check.errors + self.breaks
    }

    /// Whether a difference is shown in behavior: in a changed module's files, or in yours.
    pub fn behavior_changed(&self) -> bool {
        self.modules.iter().any(|m| matches!(&m.diff, Some(Ok(d)) if d.behavior_differs()) || m.yours.iter().any(differs))
    }

    /// How many modules that changed have no `ritsu mod diff`, for it could not be made (what a
    /// version reads cannot be fetched, a version cannot be read).
    pub fn not_compared(&self) -> usize {
        self.modules.iter().filter(|m| matches!(m.diff, Some(Err(_)))).count()
    }

    /// 1 for an error with the head's lock, and with `--fail-on-change` for a difference shown in
    /// behavior too; else 2 when a module that changed could not be compared, for the report lacks
    /// it (as `ritsu mod diff` says 2 when it shows no difference and could not compare a file);
    /// else 0.
    pub fn exit(&self, fail_on_change: bool) -> u8 {
        if self.errors() > 0 || (fail_on_change && self.behavior_changed()) {
            1
        } else if self.not_compared() > 0 {
            2
        } else {
            0
        }
    }

    /// The modules not compared, as the summary at the top says them; None when every one was.
    fn not_compared_line(&self, lang: Lang) -> Option<String> {
        let k = self.not_compared();
        (k > 0).then(|| if lang == Lang::Ja { format!("{} 個", diff::n(k)) } else { diff::n(k) })
    }
}

fn differs(b: &Behavior) -> bool {
    b.result.as_ref().is_ok_and(|s| s.differs)
}

// ── Making it ───────────────────────────────────────────────────────────

impl Report {
    /// The report of the module whose root is `root`, from the revision `base` to `head` (the
    /// working tree when None). Err, with what to say (exit 2), when it cannot be made.
    pub fn make(root: &Path, base: &str, head: Option<&str>, budget: Option<u64>, lang: Lang) -> Result<Report, Vec<Text>> {
        let base_files: Rc<dyn Files> = Rc::new(GitRevision::open(root, base).map_err(|t| vec![t])?);
        let head_files: Option<Rc<dyn Files>> = match head {
            Some(h) => Some(Rc::new(GitRevision::open(root, h).map_err(|t| vec![t])?)),
            None => None,
        };
        // what each lock pins, fetched before either tree is read
        for (files, rev) in [(Some(base_files.clone()), Some(base)), (head_files.clone(), head)] {
            let done = diff::within(files, || fetch::tree(root)).map_err(|t| {
                let (ja, en) = (Report::lock_of(rev, Lang::Ja, "`"), Report::lock_of(rev, Lang::En, "`"));
                vec![tr!("{} のモジュールを取り出せません。", "cannot fetch the modules of {}: ", ja; en).then(&t)]
            })?;
            fetch::told(lang, &done);
        }
        let base_tree = match diff::within(Some(base_files.clone()), || graph::load(root)) {
            Ok(t) => t,
            Err(Problem::Stop(said)) => return Err(said),
            Err(Problem::Diags(ds)) => {
                let first = ds.first().map(|d| d.message.clone()).unwrap_or_default();
                return Err(vec![tr!(
                    "`{base}` のモジュールの木を読めないので、比べられません（{}）",
                    "the tree of modules at `{base}` cannot be read, so nothing can be compared with it ({})",
                    first.ja; first.en
                )]);
            }
        };
        // a tree the head cannot read is what its check says, as errors; no module is compared
        let head_tree = match diff::within(head_files.clone(), || graph::load(root)) {
            Ok(t) => Some(t),
            Err(Problem::Stop(said)) => return Err(said),
            Err(Problem::Diags(_)) => None,
        };
        let base_view: Rc<dyn Files> = Rc::new(BaseLock::new(root, base, base_files.clone(), head_files.clone(), &base_tree));
        let (with_base, base_check) = check(Some(base_view.clone()), root, budget, lang)?;
        let (with_head, head_check) = check(head_files.clone(), root, budget, lang)?;
        let keys = |ds: &[Json]| ds.iter().map(key).collect::<BTreeSet<_>>();
        let (base_keys, head_keys) = (keys(&with_base), keys(&with_head));
        let gone: Vec<Json> = with_base.iter().filter(|d| !head_keys.contains(&key(d))).cloned().collect();

        let sides = Sides {
            root,
            base_tree: &base_tree,
            head_tree: head_tree.as_ref(),
            base_files: &base_files,
            base_view: &base_view,
            head_files: &head_files,
            // the index keeps what it was asked by the path, so each side asks its own
            base_index: Joined::new(),
            head_index: Joined::new(),
        };
        let mut modules = match &head_tree {
            Some(h) => changes(&base_tree, h, &base_files, &head_files),
            None => Vec::new(),
        };
        for m in &mut modules {
            m.diff = sides.module_diff(m, budget, lang);
        }

        // the new diagnostics, errors first: the check's, then each changed book's E050 and E051,
        // which are its module's
        let mut new: Vec<(Json, Option<usize>)> = with_head.iter().filter(|d| !base_keys.contains(&key(d))).map(|d| (d.clone(), None)).collect();
        let mut breaks = 0;
        for (k, m) in modules.iter().enumerate() {
            let Some(Ok(d)) = &m.diff else { continue };
            for i in &d.interface {
                for f in i.breaks.iter().filter(|f| f.severity == Severity::Error) {
                    new.push((break_json(m, &i.file, f), Some(k)));
                    breaks += 1;
                }
            }
        }
        new.sort_by_key(|(d, _)| d.get("severity").and_then(Json::as_str) != Some("error"));
        for (at, (d, owner)) in new.iter().enumerate() {
            let owners = match owner {
                Some(k) => vec![*k],
                None => sides.owners(d, &modules),
            };
            for k in owners {
                modules[k].diagnostics.push(at);
            }
        }

        // the files of yours that read a changed module
        if let Some(h) = &head_tree {
            let names: BTreeSet<&str> = modules.iter().map(|m| m.module.as_str()).collect();
            let local = diff::within(head_files.clone(), || digest::module_files(root, &h.main().manifest)).map_err(|t| vec![t])?;
            let joined = Joined::new();
            // the two sides of a file of yours are the same file, told apart by the lock each is
            // read with: `invoice.cal (the lock of main)`
            let (base_lock, head_lock) = (Report::lock_of(Some(base), lang, ""), Report::lock_of(head, lang, ""));
            let mut yours: Vec<(Behavior, BTreeSet<String>)> = Vec::new();
            for (rel, _) in local {
                let Some(tool) = diff::behaves(&rel) else { continue };
                let read: BTreeSet<String> = sides.reads(&rel).into_iter().filter(|n| names.contains(n.as_str())).collect();
                if read.is_empty() {
                    continue;
                }
                let at = root.join(&rel).to_string_lossy().into_owned();
                let old = Spot { read: at.clone(), shown: format!("{rel} ({base_lock})"), through: Some(base_view.clone()) };
                let now = Spot { read: at, shown: format!("{rel} ({head_lock})"), through: head_files.clone() };
                let result = diff::behavior(tool, &old, &now, budget, lang, &joined);
                yours.push((Behavior { file: rel, tool, result }, read));
            }
            for m in &mut modules {
                m.yours = yours.iter().filter(|(_, read)| read.contains(&m.module)).map(|(b, _)| b.clone()).collect();
            }
        }
        Ok(Report { base: base.to_string(), head: head.map(str::to_string), base_check, head_check, new: new.into_iter().map(|(d, _)| d).collect(), breaks, gone, modules })
    }
}

/// How a diagnostic is matched between the two checks: its tool, code, file, line and text.
fn key(d: &Json) -> (String, String, String, String, String) {
    let s = |k: &str| d.get(k).map(|v| v.as_str().map_or_else(|| v.compact(), str::to_string)).unwrap_or_default();
    (s("tool"), s("code"), s("file"), s("line"), s("message"))
}

/// `ritsu check --deps-warnings` of the whole module at `root`, reading `files`: its diagnostics as
/// its JSON writes them, and how many errors and warnings. Err, with what it said, when it could
/// not check (exit 2).
fn check(files: Option<Rc<dyn Files>>, root: &Path, budget: Option<u64>, lang: Lang) -> Result<(Vec<Json>, Counts), Vec<Text>> {
    // a module with no file of ritsu's languages of its own (one that only gathers dependencies) has
    // nothing for the check to say, which `ritsu check` stops on: no diagnostic, and the modules are
    // compared all the same
    let mut under = Vec::new();
    diff::within(files.clone(), || ritsu_base::paths::walk(root, ".", &[], &mut under));
    if !under.iter().any(|f| ritsu_project::project::kind_of(f).is_some()) {
        return Ok((Vec::new(), Counts::default()));
    }
    let at = root.to_string_lossy().into_owned();
    let mut args: Vec<String> = vec![at.clone(), "--root".into(), at, "--format".into(), "json".into(), "--deps-warnings".into()];
    if let Some(b) = budget {
        args.push("--budget".into());
        args.push(b.to_string());
    }
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = diff::within(files, || ritsu::check::run(&args, lang, &mut out, &mut err));
    let v = match (code, ritsu_base::json::parse(String::from_utf8_lossy(&out).trim())) {
        (0 | 1, Ok(v)) => v,
        _ => {
            let head = if lang == Lang::Ja { "エラー: " } else { "error: " };
            let said: Vec<Text> = String::from_utf8_lossy(&err).lines().filter(|l| !l.trim().is_empty()).map(|l| Text::same(l.strip_prefix(head).unwrap_or(l))).collect();
            return Err(if said.is_empty() { vec![tr!("`ritsu check` がモジュールを検査できませんでした", "`ritsu check` could not check the module")] } else { said });
        }
    };
    let diags: Vec<Json> = v.get("diagnostics").and_then(Json::as_arr).map(<[Json]>::to_vec).unwrap_or_default();
    let count = |sev: &str| diags.iter().filter(|d| d.get("severity").and_then(Json::as_str) == Some(sev)).count();
    let counts = Counts { errors: count("error"), warnings: count("warning") };
    Ok((diags, counts))
}

/// A book's E050 or E051 against its old version, as the check's JSON writes a diagnostic: the file
/// from the module at hand's root, and the dependency it is in after it.
fn break_json(m: &Changed, file: &str, f: &ritsu_ports::Finding) -> Json {
    let path = format!("{}/{file}", m.prefix);
    let put = |o: &mut Vec<(String, Json)>| {
        o.push(("file".into(), Json::str(path.as_str())));
        o.push(("dependency".into(), Json::str(m.prefix.as_str())));
    };
    let mut o: Vec<(String, Json)> = vec![("tool".into(), Json::str("chobo"))];
    match &f.json {
        Json::Obj(kv) => {
            if !kv.iter().any(|(k, _)| k == "file") {
                put(&mut o);
            }
            for (k, v) in kv {
                if k == "file" {
                    put(&mut o);
                } else {
                    o.push((k.clone(), v.clone()));
                }
            }
        }
        _ => put(&mut o),
    }
    Json::Obj(o)
}

/// The modules of the two trees, the module at hand left out, that changed: a tag that moved first,
/// then by name.
fn changes(base: &Tree, head: &Tree, base_files: &Rc<dyn Files>, head_files: &Option<Rc<dyn Files>>) -> Vec<Changed> {
    let find = |t: &Tree, name: &str| t.nodes.iter().skip(1).find(|n| n.name.to_string() == name).cloned();
    let names: BTreeSet<String> = base.nodes.iter().skip(1).chain(head.nodes.iter().skip(1)).map(|n| n.name.to_string()).collect();
    let mut out = Vec::new();
    for name in names {
        let (b, h) = (find(base, &name), find(head, &name));
        let by_version = |b: &Node, h: &Node| match b.version().cmp(&h.version()) {
            Ordering::Less => Some(Change::Upgraded),
            Ordering::Greater => Some(Change::Downgraded),
            Ordering::Equal => None,
        };
        let change = match (&b, &h) {
            (None, Some(_)) => Change::Added,
            (Some(_), None) => Change::Removed,
            (Some(b), Some(h)) => match (b.pinned.is_some(), h.pinned.is_some()) {
                (true, true) => match by_version(b, h) {
                    Some(c) => c,
                    None if b.digests().map(|d| d.0) != h.digests().map(|d| d.0) => Change::Moved,
                    None => continue,
                },
                (false, false) if b.path != h.path => Change::Source,
                (false, false) => {
                    // a module a path reaches has no digest in the lock: its files at the two revisions
                    let files = |n: &Node, f: Option<Rc<dyn Files>>| diff::within(f, || digest::module_files(&n.dir, &n.manifest)).ok();
                    if files(b, Some(base_files.clone())) == files(h, head_files.clone()) {
                        continue;
                    }
                    by_version(b, h).unwrap_or(Change::Changed)
                }
                _ => Change::Source,
            },
            (None, None) => continue,
        };
        let pin = |n: &Node| Pin { version: n.version().map(|v| v.to_string()), digest: n.digests().map(|d| d.0.to_string()) };
        out.push(Changed {
            module: name,
            prefix: h.as_ref().or(b.as_ref()).map(|n| n.prefix.clone()).unwrap_or_default(),
            change,
            old: b.as_ref().map(pin),
            new: h.as_ref().map(pin),
            diagnostics: Vec::new(),
            diff: None,
            yours: Vec::new(),
        });
    }
    out.sort_by_key(|m| m.change != Change::Moved);
    out
}

/// What a report reads its two sides through.
struct Sides<'a> {
    root: &'a Path,
    base_tree: &'a Tree,
    head_tree: Option<&'a Tree>,
    base_files: &'a Rc<dyn Files>,
    /// The module at hand with the base's lock.
    base_view: &'a Rc<dyn Files>,
    head_files: &'a Option<Rc<dyn Files>>,
    base_index: Joined,
    head_index: Joined,
}

impl Sides<'_> {
    /// What `ritsu mod diff` says of a changed module's two versions, each read as its revision has
    /// it, the files of each named after its prefix and version (`@pricing@1.1.0/rules/fee.rule`).
    fn module_diff(&self, m: &Changed, budget: Option<u64>, lang: Lang) -> Option<Result<ModDiff, Text>> {
        let b = self.base_tree.nodes.iter().skip(1).find(|n| n.name.to_string() == m.module)?;
        let h = self.head_tree?.nodes.iter().skip(1).find(|n| n.name.to_string() == m.module)?;
        let label = |n: &Node| match n.version() {
            Some(v) => format!("{}@{v}", n.prefix),
            None => n.prefix.clone(),
        };
        let digest = |n: &Node| n.digests().map(|d| d.0.to_string());
        // each version is read as a module of its own: what its own lock pins, fetched first
        let fetched = |n: &Node, files: Option<Rc<dyn Files>>| -> Result<(), Text> {
            let l = label(n);
            let done = diff::within(files, || fetch::tree(&n.dir)).map_err(|t| tr!("`{l}` が読むモジュールを取り出せません。", "cannot fetch the modules `{l}` reads: ").then(&t))?;
            fetch::told(lang, &done);
            Ok(())
        };
        if let Err(t) = fetched(b, Some(self.base_files.clone())).and_then(|()| fetched(h, self.head_files.clone())) {
            return Some(Err(t));
        }
        let old = Module::through(&b.dir, label(b), digest(b), Some(self.base_files.clone()));
        let new = Module::through(&h.dir, label(h), digest(h), self.head_files.clone());
        Some(match (old, new) {
            // what the index and the checks keep is kept by the path: a module of its own each
            (Ok(o), Ok(n)) => Ok(diff::modules(&o, &n, budget, lang, &Joined::new())),
            (Err(t), _) | (_, Err(t)) => Err(t),
        })
    }

    /// The modules a file (from the root) reads, through the files it names and the files they
    /// name, as the head's lock reads them and as the base's does.
    fn reads(&self, rel: &str) -> BTreeSet<String> {
        let mut out = walk(&self.head_index, self.root, rel, self.head_tree, self.head_files.clone());
        out.extend(walk(&self.base_index, self.root, rel, Some(self.base_tree), Some(self.base_view.clone())));
        out
    }

    /// The changed modules a new diagnostic is about: the module of its file, when that module
    /// changed; else the changed modules its file reads.
    fn owners(&self, d: &Json, modules: &[Changed]) -> Vec<usize> {
        let Some(file) = d.get("file").and_then(Json::as_str) else { return Vec::new() };
        let of = |name: &str| modules.iter().position(|m| m.module == name);
        let own = [self.head_tree, Some(self.base_tree)].into_iter().flatten().find_map(|t| node_of(t, file).map(|i| t.nodes[i].name.to_string()));
        if let Some(k) = own.and_then(|n| of(&n)) {
            return vec![k];
        }
        let read = self.reads(file);
        modules.iter().enumerate().filter(|(_, m)| read.contains(&m.module)).map(|(k, _)| k).collect()
    }
}

/// The modules a file reads, reading `files`: the modules of the file and of each file it names,
/// through and through.
fn walk(joined: &Joined, root: &Path, rel: &str, tree: Option<&Tree>, files: Option<Rc<dyn Files>>) -> BTreeSet<String> {
    let Some(tree) = tree else { return BTreeSet::new() };
    diff::within(files, || {
        let mut out = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut todo = vec![rel.to_string()];
        while let Some(f) = todo.pop() {
            if !seen.insert(f.clone()) {
                continue;
            }
            if let Some(i) = node_of(tree, &f) {
                out.insert(tree.nodes[i].name.to_string());
            }
            let Some(tool) = diff::tool_of(&f) else { continue };
            if let Some(Ok(refs)) = joined.index.references(tool, root, &f) {
                todo.extend(refs.into_iter().map(|r| r.target.path));
            }
        }
        out
    })
}

/// The node of the tree a file (from the root) is in: `@pricing/@tax/rules/vat_rate.rule` is in the
/// node `pricing` reaches by `tax`. None for a file of the module at hand.
fn node_of(tree: &Tree, rel: &str) -> Option<usize> {
    let chain: Vec<&str> = rel.split('/').map_while(|p| p.strip_prefix('@')).collect();
    if chain.is_empty() { None } else { tree.walk(&chain) }
}

// ── The module at hand with the base's lock ─────────────────────────────

/// The files of the module at hand as the head has them, with the base's `ritsu.mod` and
/// `ritsu.lock` (none, where the base has none), and the modules a path reaches in the base's tree,
/// the module's copy of the store and the key files of the base's `signature` lines as the base has
/// them (the records of the checks name the base's keys, DESIGN 17.14). A module from a registry is
/// read from the store by the base's lock. Nothing is written.
struct BaseLock {
    laid: BTreeMap<PathBuf, Option<Vec<u8>>>,
    /// The directories read from the base.
    routes: Vec<PathBuf>,
    base: Rc<dyn Files>,
    under: Option<Rc<dyn Files>>,
    rev: String,
}

impl BaseLock {
    fn new(root: &Path, rev: &str, base: Rc<dyn Files>, under: Option<Rc<dyn Files>>, tree: &Tree) -> BaseLock {
        let read = |name: &str| diff::within(Some(base.clone()), || ritsu_base::fs::raw::read(&root.join(name)).ok());
        let laid: BTreeMap<PathBuf, Option<Vec<u8>>> = [manifest::FILE, lock::FILE].into_iter().map(|n| (fold(&root.join(n)), read(n))).collect();
        let mut routes: Vec<PathBuf> = tree.nodes.iter().skip(1).filter(|n| n.path.is_some()).map(|n| fold(&n.dir)).collect();
        routes.push(fold(&root.join(store::VENDOR)));
        if let Some(Some(bytes)) = laid.get(&fold(&root.join(manifest::FILE)))
            && let Ok(m) = manifest::parse(bytes, manifest::FILE)
        {
            for s in &m.signatures {
                if let manifest::Policy::Key { path, .. } = &s.policy {
                    routes.push(fold(&root.join(path)));
                }
            }
        }
        BaseLock { laid, routes, base, under, rev: rev.to_string() }
    }

    fn under(&self) -> &dyn Files {
        match &self.under {
            Some(f) => f.as_ref(),
            None => &Disk,
        }
    }

    fn abs(&self, p: &Path) -> PathBuf {
        if ritsu_base::paths::rooted(p) { fold(p) } else { fold(&self.under().current_dir().unwrap_or_default().join(p)) }
    }

    /// The files a path is read from.
    fn pick(&self, abs: &Path) -> &dyn Files {
        if self.routes.iter().any(|r| abs.starts_with(r)) { self.base.as_ref() } else { self.under() }
    }
}

/// A path with `.` and `..` folded by their letters.
fn fold(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn not_there(p: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("{} is not in the base's tree", p.display()))
}

impl Files for BaseLock {
    fn read(&self, p: &Path) -> io::Result<Vec<u8>> {
        let a = self.abs(p);
        match self.laid.get(&a) {
            Some(Some(b)) => Ok(b.clone()),
            Some(None) => Err(not_there(p)),
            None => self.pick(&a).read(&a),
        }
    }

    fn metadata(&self, p: &Path) -> io::Result<Meta> {
        let a = self.abs(p);
        match self.laid.get(&a) {
            Some(Some(b)) => Ok(Meta { kind: Kind::File, len: b.len() as u64 }),
            Some(None) => Err(not_there(p)),
            None => self.pick(&a).metadata(&a),
        }
    }

    fn read_dir(&self, p: &Path) -> io::Result<Vec<(OsString, Kind)>> {
        let a = self.abs(p);
        let mut out = self.pick(&a).read_dir(&a)?;
        for (f, b) in &self.laid {
            let (Some(dir), Some(name)) = (f.parent(), f.file_name()) else { continue };
            if dir != a {
                continue;
            }
            out.retain(|(x, _)| x != name);
            if b.is_some() {
                out.push((name.to_os_string(), Kind::File));
            }
        }
        out.sort_by(|x, y| x.0.cmp(&y.0));
        Ok(out)
    }

    fn write(&self, p: &Path, _bytes: &[u8]) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, format!("{} is read with the lock of {}, which is not written to", p.display(), self.rev)))
    }

    fn create_dir_all(&self, p: &Path) -> io::Result<()> {
        self.write(p, &[])
    }

    fn current_dir(&self) -> io::Result<PathBuf> {
        self.under().current_dir()
    }

    fn canonicalize(&self, p: &Path) -> io::Result<PathBuf> {
        let a = self.abs(p);
        match self.laid.get(&a) {
            Some(Some(_)) => Ok(a.parent().map(|d| self.pick(d).canonicalize(d).unwrap_or_else(|_| d.to_path_buf())).map_or(a.clone(), |d| d.join(a.file_name().unwrap_or_default()))),
            Some(None) => Err(not_there(p)),
            None => self.pick(&a).canonicalize(&a),
        }
    }

    fn revision(&self) -> Option<String> {
        Some(self.rev.clone())
    }
}

// ── Saying it ───────────────────────────────────────────────────────────

impl Report {
    /// The head as the output names it: its revision in `q`, or the working tree.
    fn head_name(&self, lang: Lang, q: &str) -> String {
        match &self.head {
            Some(h) => format!("{q}{h}{q}"),
            None => tr!("作業ツリー", "the working tree").get(lang).to_string(),
        }
    }

    /// The lock of a side as the output names it, the revision in `q`: `the lock of `main``,
    /// `作業ツリーの lock`.
    fn lock_of(rev: Option<&str>, lang: Lang, q: &str) -> String {
        match (rev, lang) {
            (Some(r), Lang::Ja) => format!("{q}{r}{q} の lock"),
            (None, Lang::Ja) => "作業ツリーの lock".to_string(),
            (Some(r), _) => format!("the lock of {q}{r}{q}"),
            (None, _) => "the lock of the working tree".to_string(),
        }
    }

    /// The two locks as the output names them: the base's, the head's.
    fn locks(&self, lang: Lang, q: &str) -> (String, String) {
        (Report::lock_of(Some(&self.base), lang, q), Report::lock_of(self.head.as_deref(), lang, q))
    }

    fn versions(m: &Changed) -> String {
        let v = |p: &Option<Pin>| p.as_ref().and_then(|p| p.version.clone()).unwrap_or_else(|| "-".into());
        match (&m.old, &m.new) {
            (Some(_), Some(_)) => format!("{} → {}", v(&m.old), v(&m.new)),
            (None, _) => v(&m.new),
            (_, None) => v(&m.old),
        }
    }

    /// A tag that moved, said at the top: the version, and the two digests.
    fn moved_line(&self, m: &Changed, lang: Lang, q: &str) -> String {
        let v = m.new.as_ref().and_then(|p| p.version.clone()).unwrap_or_default();
        let d = |p: &Option<Pin>| p.as_ref().and_then(|p| p.digest.clone()).unwrap_or_default();
        let (od, nd) = (d(&m.old), d(&m.new));
        let module = &m.module;
        let (base, head) = self.locks(lang, q);
        // a space between the particle and a revision, none before 作業ツリー
        let and = if head.starts_with(|c: char| c.is_ascii()) { "と " } else { "と" };
        tr!(
            "{q}{module}{q} {v} は、{base} {and}{head} で中身が違います（ダイジェスト {q}{od}{q} → {q}{nd}{q}）。同じ版が別の中身で出し直されていて、前に確かめたものではありません。",
            "{q}{module}{q} {v} in {base} is not the one in {head} (its digest went from {q}{od}{q} to {q}{nd}{q}): the same version was published again with other contents, and is not the one checked before."
        )
        .get(lang)
        .to_string()
    }

    fn counts_line(c: Counts, lang: Lang) -> String {
        if lang == Lang::Ja {
            format!("エラー {} 件、警告 {} 件", diff::n(c.errors), diff::n(c.warnings))
        } else {
            format!("{}, {}", plural(c.errors, "error", "errors"), plural(c.warnings, "warning", "warnings"))
        }
    }

    /// A diagnostic of the check's JSON in a line: `error[dandori E002] orders/place_order.flow:49:
    /// the text`.
    fn diag_line(d: &Json) -> String {
        let s = |k: &str| d.get(k).and_then(Json::as_str).unwrap_or_default();
        let at = match d.get("line").and_then(Json::as_int) {
            Some(l) => format!("{}:{l}", s("file")),
            None => s("file").to_string(),
        };
        format!("{}[{} {}] {at}: {}", s("severity"), s("tool"), s("code"), s("message"))
    }

    /// The last line: what the exit code says, the revision in `q`.
    fn closing(&self, lang: Lang, code: u8, q: &str) -> String {
        let (_, head) = self.locks(lang, q);
        let errors = self.errors();
        let t = if errors > 0 {
            let (k, e) = (diff::n(errors), plural(errors, "error", "errors"));
            tr!("{head} での検査に、エラーが {k} 件あります。", "the check with {head} has {e}.")
        } else if self.modules.is_empty() {
            tr!("二つの lock は同じモジュールを指していて、{head} での検査にエラーはありません。", "the two locks pin the same modules, and the check with {head} has no error.")
        } else if code == 2 {
            let not = self.not_compared();
            let (k, m) = (diff::n(not), plural(not, "module", "modules"));
            if self.behavior_changed() {
                tr!(
                    "{head} での検査にエラーはありません。振る舞いに示せた違いがあり、比べられなかったモジュールが {k} 個あるので、exit 2 です。",
                    "the check with {head} has no error; a difference is shown in behavior, and {m} could not be compared, which makes exit 2."
                )
            } else {
                tr!(
                    "{head} での検査にエラーはありません。比べられなかったモジュールが {k} 個あり、振る舞いが変わらないとは言えないので、exit 2 です。",
                    "the check with {head} has no error; {m} could not be compared, so this does not say behavior stays the same, which makes exit 2."
                )
            }
        } else {
            match (self.behavior_changed(), code) {
                (true, 1) => tr!(
                    "{head} での検査にエラーはありません。振る舞いに示せた違いがあるので、`--fail-on-change` により exit 1 です。",
                    "the check with {head} has no error; a difference is shown in behavior, which `--fail-on-change` makes exit 1."
                ),
                (true, _) => tr!("{head} での検査にエラーはありません。振る舞いに示せた違いがあります。", "the check with {head} has no error; a difference is shown in behavior."),
                (false, _) => tr!("{head} での検査にエラーはなく、振る舞いに示せた違いもありません。", "the check with {head} has no error, and no difference is shown in behavior."),
            }
        };
        t.get(lang).to_string()
    }

    /// The report as text.
    pub fn text(&self, lang: Lang, code: u8) -> String {
        let c = diff::colon(lang);
        let e = diff::ends(lang);
        let mut o = format!("ritsu mod report{c}{} → {}\n", self.base, self.head_name(lang, ""));
        for m in self.modules.iter().filter(|m| m.change == Change::Moved) {
            o.push_str(&format!("\n{}{c}{}\n", tr!("タグが動きました", "a tag moved").get(lang), self.moved_line(m, lang, "")));
        }
        o.push('\n');
        let (base, head) = self.locks(lang, "");
        for (lock, counts) in [(base, self.base_check), (head, self.head_check)] {
            o.push_str(&format!("{}{c}{}\n", tr!("{lock} での検査", "the check with {lock}").get(lang), Report::counts_line(counts, lang)));
        }
        if let Some(k) = self.not_compared_line(lang) {
            o.push_str(&format!("{}{c}{k}\n", tr!("比べられなかったモジュール", "modules not compared").get(lang)));
        }
        for (title, ds) in [(tr!("新しい診断", "new diagnostics"), &self.new), (tr!("消えた診断", "diagnostics gone"), &self.gone)] {
            if ds.is_empty() {
                o.push_str(&format!("{}{c}{}\n", title.get(lang), tr!("ありません", "none").get(lang)));
                continue;
            }
            o.push_str(&format!("{}{e}\n", title.get(lang)));
            for d in ds {
                o.push_str(&format!("  {}\n", Report::diag_line(d)));
            }
        }
        for m in &self.modules {
            o.push_str(&format!("\n== {} ({}) {}{c}{}\n", m.module, m.prefix, Report::versions(m), m.change.said(lang)));
            if !m.diagnostics.is_empty() {
                o.push_str(&format!("\n{}{e}\n", tr!("新しい診断", "new diagnostics").get(lang)));
                for &i in &m.diagnostics {
                    o.push_str(&format!("  {}\n", Report::diag_line(&self.new[i])));
                }
            }
            match &m.diff {
                Some(Ok(d)) => o.push_str(&diff::body_text(d, lang)),
                Some(Err(t)) => o.push_str(&format!("\n{}{c}{}\n", tr!("比べられません", "not compared").get(lang), t.get(lang))),
                None => {}
            }
            if !m.yours.is_empty() {
                o.push_str(&format!("\n{}{e}\n", tr!("あなたの答えのうち、変わるもの", "your answers that change").get(lang)));
                for b in &m.yours {
                    o.push_str(&format!("  {} ({}){c}{}\n", b.file, b.tool.word(), diff::verdict(b, lang)));
                    if let (true, Ok(s)) = (b.shows(), &b.result) {
                        o.push_str(&diff::indent(&s.text, "    "));
                    }
                }
            }
        }
        o.push_str(&format!("\n{}\n", self.closing(lang, code, "")));
        o
    }

    /// The report as Markdown, to paste in a comment of a pull request: every word with an `@` in
    /// backticks, at most [`LIMIT`] characters.
    pub fn markdown(&self, lang: Lang, code: u8) -> String {
        let c = diff::colon(lang);
        let mut o = format!("## ritsu mod report{c}`{}` → {}\n", self.base, self.head_name(lang, "`"));
        for m in self.modules.iter().filter(|m| m.change == Change::Moved) {
            o.push_str(&format!("\n> [!WARNING]\n> **{}**{c}{}\n", tr!("タグが動きました", "A tag moved").get(lang), self.moved_line(m, lang, "`")));
        }
        o.push_str(&format!("\n| {} | {} | {} |\n|---|---|---|\n", tr!("検査", "Check").get(lang), tr!("エラー", "Errors").get(lang), tr!("警告", "Warnings").get(lang)));
        let (base, head) = self.locks(lang, "`");
        for (lock, counts) in [(base, self.base_check), (head, self.head_check)] {
            o.push_str(&format!("| {} | {} | {} |\n", sentence(tr!("{lock} で", "with {lock}").get(lang), lang), diff::n(counts.errors), diff::n(counts.warnings)));
        }
        if let Some(k) = self.not_compared_line(lang) {
            o.push_str(&format!("\n**{}**{c}{k}\n", tr!("比べられなかったモジュール", "Modules not compared").get(lang)));
        }
        if !self.new.is_empty() {
            o.push_str(&format!("\n### {} ({})\n", tr!("新しい診断", "New diagnostics").get(lang), diff::n(self.new.len())));
            o.push_str(&self.diag_table(&self.new, true, lang));
        }
        if !self.gone.is_empty() {
            o.push_str(&format!(
                "\n<details><summary>{} ({})</summary>\n{}\n</details>\n",
                tr!("消えた診断", "Diagnostics gone").get(lang),
                diff::n(self.gone.len()),
                self.diag_table(&self.gone, false, lang)
            ));
        }
        for m in &self.modules {
            o.push_str(&format!("\n### `{}` (`{}`) {}{c}{}\n\n", m.module, m.prefix, Report::versions(m), m.change.said(lang)));
            if !m.diagnostics.is_empty() {
                o.push_str(&format!("- {}{c}{}\n", tr!("新しい診断", "New diagnostics").get(lang), diff::n(m.diagnostics.len())));
            }
            match &m.diff {
                Some(Ok(d)) => o.push_str(&diff::body_markdown(d, lang, 4, ROWS)),
                Some(Err(t)) => o.push_str(&format!("- {}{c}{}\n", tr!("比べられません", "Not compared").get(lang), t.get(lang))),
                None => {}
            }
            if !m.yours.is_empty() {
                o.push_str(&format!("\n#### {}\n\n", tr!("あなたの答えのうち、変わるもの", "Your answers that change").get(lang)));
                let mut listing = false;
                for b in &m.yours {
                    match (b.shows(), &b.result) {
                        (true, Ok(s)) => {
                            if listing {
                                o.push('\n');
                            }
                            listing = false;
                            o.push_str(&format!(
                                "<details><summary><code>{}</code> ({}){c}{}</summary>\n\n{}\n\n</details>\n\n",
                                diff::html(&b.file),
                                b.tool.word(),
                                diff::verdict(b, lang),
                                diff::capped(&diff::folded(&s.markdown), ROWS, lang)
                            ));
                        }
                        _ => {
                            listing = true;
                            o.push_str(&format!("- `{}` ({}){c}{}\n", b.file, b.tool.word(), diff::verdict(b, lang)));
                        }
                    }
                }
                if !listing {
                    o.pop();
                }
            }
        }
        o.push_str(&format!("\n{}\n", sentence(&self.closing(lang, code, "`"), lang)));
        fit(&diff::guarded(&o), LIMIT, lang)
    }

    /// A table of diagnostics, with the changed modules each new one is about when `owners`.
    fn diag_table(&self, ds: &[Json], owners: bool, lang: Lang) -> String {
        let mut t = format!(
            "\n| {} | {} | {} | {} | {} |",
            tr!("種類", "Severity").get(lang),
            tr!("コード", "Code").get(lang),
            tr!("ファイル", "File").get(lang),
            tr!("行", "Line").get(lang),
            tr!("内容", "Message").get(lang)
        );
        if owners {
            t.push_str(&format!(" {} |", tr!("モジュール", "Module").get(lang)));
        }
        t.push_str(if owners { "\n|---|---|---|---|---|---|\n" } else { "\n|---|---|---|---|---|\n" });
        for (i, d) in ds.iter().enumerate().take(ROWS) {
            let s = |k: &str| d.get(k).and_then(Json::as_str).unwrap_or_default().to_string();
            let line = d.get("line").and_then(Json::as_int).map(|l| l.to_string()).unwrap_or_default();
            let file = s("file");
            let file = if file.is_empty() { String::new() } else { format!("`{}`", diff::cell(&file)) };
            t.push_str(&format!("| {} | `{} {}` | {file} | {line} | {} |", s("severity"), s("tool"), s("code"), diff::cell(&s("message"))));
            if owners {
                let of: Vec<String> = self.modules.iter().filter(|m| m.diagnostics.contains(&i)).map(|m| format!("`{}`", m.module)).collect();
                t.push_str(&format!(" {} |", of.join(", ")));
            }
            t.push('\n');
        }
        if ds.len() > ROWS {
            t.push_str(&format!("\n{}\n", sentence(&diff::more_line(ds.len() - ROWS, lang), lang)));
        }
        t
    }

    /// The report as JSON (DESIGN 17.21).
    pub fn to_json(&self, lang: Lang, code: u8) -> Json {
        let counts = |c: Counts| Json::obj([("errors", Json::int(c.errors as i128)), ("warnings", Json::int(c.warnings as i128))]);
        let pin = |p: &Option<Pin>| p.as_ref().map_or(Json::Null, |p| Json::obj([("version", Json::opt_str(p.version.clone())), ("digest", Json::opt_str(p.digest.clone()))]));
        let modules = self.modules.iter().map(|m| {
            Json::obj([
                ("module", Json::str(m.module.as_str())),
                ("prefix", Json::str(m.prefix.as_str())),
                ("change", Json::str(m.change.word())),
                ("old", pin(&m.old)),
                ("new", pin(&m.new)),
                ("diagnostics", Json::arr(m.diagnostics.iter().map(|&i| Json::int(i as i128)))),
                (
                    "diff",
                    match &m.diff {
                        Some(Ok(d)) => Json::obj(diff::json_fields(d, lang)),
                        _ => Json::Null,
                    },
                ),
                (
                    "error",
                    match &m.diff {
                        Some(Err(t)) => Json::str(t.get(lang)),
                        _ => Json::Null,
                    },
                ),
                ("yours", Json::arr(m.yours.iter().map(|b| diff::behavior_json(b, lang)))),
            ])
        });
        Json::obj([
            ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
            ("base", Json::obj([("rev", Json::str(self.base.as_str()))])),
            ("head", Json::obj([("rev", Json::opt_str(self.head.clone()))])),
            (
                "check",
                Json::obj([
                    ("base", counts(self.base_check)),
                    ("head", counts(self.head_check)),
                    ("new", Json::arr(self.new.iter().cloned())),
                    ("gone", Json::arr(self.gone.iter().cloned())),
                ]),
            ),
            ("modules", Json::arr(modules)),
            ("ok", Json::Bool(code == 0)),
        ])
    }
}

/// A sentence as it starts: with a capital in English, as it is in Japanese (which may start with a
/// revision in lower case).
fn sentence(s: &str, lang: Lang) -> String {
    if lang == Lang::Ja { s.to_string() } else { capitalize(s) }
}

/// Markdown held to `limit` characters: when it is longer, its first lines, as many as fit with the
/// fence and the folds they leave open closed after them, and a line that says it was cut and where
/// all of it is.
pub fn fit(md: &str, limit: usize, lang: Lang) -> String {
    if md.chars().count() <= limit {
        return md.to_string();
    }
    let note = format!(
        "\n{}\n",
        tr!(
            "_この報告は 60,000 字で切りました（PR のコメントに入る長さ）。全部は `ritsu mod report --format json` にあります。_",
            "_This report is cut at 60,000 characters, for the length a comment of a pull request holds; `ritsu mod report --format json` has all of it._"
        )
        .get(lang)
    );
    const FENCE: &str = "```\n";
    const FOLD: &str = "\n</details>\n";
    let closings = |fence: bool, open: usize| usize::from(fence) * FENCE.len() + open * FOLD.len();
    let room = limit.saturating_sub(note.chars().count());
    // after each line: the characters so far, whether a fence is open, and the folds open
    let (mut count, mut fence, mut open) = (0usize, false, 0usize);
    let mut best = (0usize, false, 0usize);
    let mut end = 0usize;
    for line in md.split_inclusive('\n') {
        count += line.chars().count();
        let t = line.trim_start();
        if t.starts_with("```") {
            fence = !fence;
        } else if !fence {
            open += t.matches("<details>").count();
            open = open.saturating_sub(t.matches("</details>").count());
        }
        if count + closings(fence, open) > room {
            break;
        }
        end += line.len();
        best = (count, fence, open);
    }
    let mut kept = md[..end].to_string();
    let (_, fence, open) = best;
    if !kept.ends_with('\n') {
        kept.push('\n');
    }
    if fence {
        kept.push_str(FENCE);
    }
    for _ in 0..open {
        kept.push_str(FOLD);
    }
    kept.push_str(&note);
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A report far longer than a comment holds, cut inside a fold, a table and a fence.
    fn long() -> String {
        let mut md = String::from("## ritsu mod report: `main` → the working tree\n");
        for i in 0..400 {
            md.push_str(&format!("\n<details><summary><code>rules/r{i}.rule</code> (rulec): the answers differ</summary>\n\n"));
            md.push_str("| amount | old | new |\n|---|---|---|\n");
            for k in 0..20 {
                md.push_str(&format!("| {k} | `small` | `large` |\n"));
            }
            md.push_str("\n```\nrulec diff rules/r.rule\n```\n\n</details>\n");
        }
        md
    }

    #[test]
    fn a_long_report_is_cut_with_its_folds_and_fences_closed() {
        let md = long();
        assert!(md.chars().count() > LIMIT);
        for lang in [Lang::En, Lang::Ja] {
            let cut = fit(&md, LIMIT, lang);
            assert!(cut.chars().count() <= LIMIT, "{}", cut.chars().count());
            assert!(cut.len() < md.len());
            assert_eq!(cut.matches("<details>").count(), cut.matches("</details>").count());
            assert_eq!(cut.lines().filter(|l| l.trim_start().starts_with("```")).count() % 2, 0);
            assert!(cut.trim_end().ends_with(if lang == Lang::Ja { "にあります。_" } else { "has all of it._" }), "{}", &cut[cut.len() - 200..]);
            assert!(cut.contains("`ritsu mod report --format json`"));
        }
        // cut in the middle of a fence, a fold left open: both are closed
        for limit in [5_000, 5_100, 5_200, 5_300, 5_400] {
            let cut = fit(&md, limit, Lang::En);
            assert!(cut.chars().count() <= limit);
            assert_eq!(cut.matches("<details>").count(), cut.matches("</details>").count(), "{limit}");
            assert_eq!(cut.lines().filter(|l| l.trim_start().starts_with("```")).count() % 2, 0, "{limit}");
        }
        let short = "## ritsu mod report\n\nnothing\n";
        assert_eq!(fit(short, LIMIT, Lang::En), short);
    }

    #[test]
    fn every_word_with_an_at_is_in_backticks() {
        let md = "a @pricing/rules/fee.rule b `@tax/x` | <code>@a</code> | x@y\n```\n@fenced\n```\n";
        assert_eq!(diff::guarded(md), "a `@pricing/rules/fee.rule` b `@tax/x` | <code>@a</code> | `x@y`\n```\n@fenced\n```\n");
    }

    /// A report of `modules`, with `errors` in the check with the head's lock.
    fn report_of(modules: Vec<Changed>, errors: usize) -> Report {
        let head_check = Counts { errors, warnings: 0 };
        Report { base: "one".into(), head: None, base_check: Counts::default(), head_check, new: Vec::new(), breaks: 0, gone: Vec::new(), modules }
    }

    /// pricing, upgraded, with what `ritsu mod diff` says of it.
    fn pricing(diff: Option<Result<ModDiff, Text>>) -> Changed {
        Changed { module: "example.com/acme/pricing".into(), prefix: "@pricing".into(), change: Change::Upgraded, old: None, new: None, diagnostics: Vec::new(), diff, yours: Vec::new() }
    }

    /// The order of the exit codes: an error with the head's lock is 1 whatever was not compared, and
    /// so is a difference shown in behavior with `--fail-on-change`; a module not compared is 2 when
    /// nothing makes it 1; a module with no diff to make (added or removed) is no module not
    /// compared. The summary at the top and the last line say the 2, with a difference shown or not.
    #[test]
    fn a_module_not_compared_makes_exit_2_when_nothing_makes_it_1() {
        let not = || Some(Err(tr!("比べられません", "cannot be compared")));
        assert_eq!(report_of(Vec::new(), 0).exit(false), 0);
        assert_eq!(report_of(vec![pricing(None)], 0).exit(true), 0);
        let r = report_of(vec![pricing(None), pricing(not())], 0);
        assert_eq!((r.not_compared(), r.exit(false), r.exit(true)), (1, 2, 2));
        assert_eq!(report_of(vec![pricing(not())], 1).exit(false), 1);
        let text = r.text(Lang::En, 2);
        assert!(text.contains("\nmodules not compared: 1\nnew diagnostics: none\n"), "{text}");
        assert!(text.ends_with("\nthe check with the lock of the working tree has no error; 1 module could not be compared, so this does not say behavior stays the same, which makes exit 2.\n"), "{text}");
        let md = r.markdown(Lang::Ja, 2);
        assert!(md.contains("|\n\n**比べられなかったモジュール**：1 個\n"), "{md}");
        assert!(md.ends_with("\n作業ツリーの lock での検査にエラーはありません。比べられなかったモジュールが 1 個あり、振る舞いが変わらないとは言えないので、exit 2 です。\n"), "{md}");
        // every module compared: no such line
        assert!(!report_of(vec![pricing(None)], 0).text(Lang::En, 0).contains("not compared"));
        // a difference shown in a file of yours: 1 with `--fail-on-change`, else still 2
        let said = diff::Said { text: String::new(), markdown: String::new(), json: Json::Null, differs: true, total: true, more: false };
        let mut shown = pricing(not());
        shown.yours = vec![Behavior { file: "orders/total.rule".into(), tool: ritsu_base::naming::Tool::Rulec, result: Ok(said) }];
        let r = report_of(vec![shown], 0);
        assert_eq!((r.behavior_changed(), r.exit(false), r.exit(true)), (true, 2, 1));
        let text = r.text(Lang::En, 2);
        assert!(text.ends_with("\nthe check with the lock of the working tree has no error; a difference is shown in behavior, and 1 module could not be compared, which makes exit 2.\n"), "{text}");
        let text = r.text(Lang::Ja, 2);
        assert!(text.ends_with("\n作業ツリーの lock での検査にエラーはありません。振る舞いに示せた違いがあり、比べられなかったモジュールが 1 個あるので、exit 2 です。\n"), "{text}");
    }
}
