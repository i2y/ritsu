//! `ritsu mod diff <old> [<new>]` (DESIGN 17.9.6): the differences between two versions of a module,
//! its files, its `require`s, the behavior of its rules, dates files, calendars and gates, and the
//! interface of each file that changed (DESIGN 17.9.5).
//!
//! - A side is `<module>@<version>` (the version's files in the store, or in the module at hand's
//!   copy of it), a directory (the module it is in) or `.` (the module at hand). Two files, in any of
//!   the spellings of DESIGN 17.9.1, give the difference of that file alone.
//! - Behavior: rulec's difference over the whole input space, koyomi's and sekisho's, each asked in
//!   this process. Every rule, dates file, calendar and gate both versions have is compared, not only
//!   those whose bytes changed: a file reads others (a rule it applies, a calendar, a dependency the
//!   lock raised), and the same bytes may answer otherwise.
//! - Interface: what the language's `api` prints, compared by name; what a file holds (DESIGN 6.4)
//!   for dandori, geas and `.proto`; the digest for any other file. A book also goes through what
//!   `chobo check --diff-base` holds a book to (chobo's `diffbase::between`): E050 and E051 are the
//!   differences that break the data already in a database.
//!
//! [`ModDiff`] is also what `ritsu mod report` (DESIGN 17.10) puts under each module that changed.

use super::Ctx;
use ritsu_base::cli::Args;
use ritsu_base::fs::Files;
use ritsu_base::json::Json;
use ritsu_base::modules::{address, digest, manifest};
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_project::Joined;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The most lines of files, or of changes of one interface, a text or a Markdown shows; the JSON
/// holds every one.
const SHOWN: usize = 200;

// ── The command ─────────────────────────────────────────────────────────

/// `ritsu mod diff`: the exit code. `a.pos` holds `<old>` and maybe `<new>`.
pub fn diff(ctx: &Ctx, a: &Args) -> u8 {
    let budget = match budget_of(a) {
        Ok(b) => b,
        Err(t) => return ctx.refuse(t),
    };
    let old = a.pos[0].as_str();
    let new = a.pos.get(1).map(String::as_str).unwrap_or(".");
    let joined = Joined::new();
    let d = match (read_side(ctx, old), read_side(ctx, new)) {
        (Err(t), _) | (_, Err(t)) => return ctx.refuse(t),
        (Ok(Side::Module(o)), Ok(Side::Module(n))) => modules(&o, &n, budget, ctx.lang, &joined),
        (Ok(Side::File(o)), Ok(Side::File(n))) => files(&o, &n, budget, ctx.lang, &joined),
        _ => {
            return ctx.refuse(tr!(
                "`ritsu mod diff` は、二つのモジュール（`<名前>@<版>`、ディレクトリ、`.`）か、二つのファイルを比べます",
                "`ritsu mod diff` compares two modules (`<module>@<version>`, a directory or `.`) or two files"
            ));
        }
    };
    let mut out = std::io::stdout();
    let _ = match ctx.format.as_str() {
        "json" => writeln!(out, "{}", to_json(&d, ctx.lang).pretty()),
        "markdown" => write!(out, "{}", markdown(&d, ctx.lang)),
        _ => write!(out, "{}", text(&d, ctx.lang)),
    };
    d.exit()
}

/// `--budget`: None for each language's own default. Err, saying why, for a value that is no
/// positive integer.
pub fn budget_of(a: &Args) -> Result<Option<u64>, Text> {
    match a.get("--budget") {
        None => Ok(None),
        Some(b) => match b.replace([',', '_'], "").parse::<u64>() {
            Ok(n) if n > 0 => Ok(Some(n)),
            _ => Err(tr!("`--budget {b}` は正の整数ではありません", "`--budget {b}` is not a positive integer")),
        },
    }
}

// ── The sides ───────────────────────────────────────────────────────────

/// A version of a module, read from where its files are.
pub struct Module {
    /// How the side was written (`example.com/acme/pricing@1.0.0`, a directory, `.`), which the
    /// output names it by and puts before each of its files.
    pub label: String,
    pub root: PathBuf,
    pub name: String,
    pub version: Option<String>,
    /// The digest of the version's OCI manifest, for a version from the store.
    pub digest: Option<String>,
    pub manifest: manifest::Manifest,
    /// Its files (what it carries when it is pushed), by the path from its root.
    pub files: BTreeMap<String, Vec<u8>>,
    /// The files it is read through: a git revision for a module a path reaches, read at a
    /// revision by `ritsu mod report`; None for the files as they are.
    pub through: Option<Rc<dyn Files>>,
}

/// A file as DESIGN 17.9.1 writes it, read through the files of its side.
pub struct FileSide {
    pub spelled: String,
    pub opened: address::Opened,
}

enum Side {
    Module(Module),
    File(FileSide),
}

/// A side as the command line writes it.
fn read_side(ctx: &Ctx, spelled: &str) -> Result<Side, Text> {
    if spelled == "." {
        let root = ctx.root()?;
        return Module::at(&root, ctx.shown(&root), None).map(Side::Module);
    }
    let p = Path::new(spelled);
    if ritsu_base::fs::is_dir(p) {
        let abs = ritsu_base::fs::canonicalize(p).unwrap_or_else(|_| ctx.here().join(p));
        let Some(root) = ritsu_base::modules::refer::raw_module_root(&abs) else {
            return Err(tr!(
                "`{spelled}` はモジュールの中にありません（そこにも上にも `ritsu.mod` がありません）",
                "`{spelled}` is in no module: there is no `ritsu.mod` there or above it"
            ));
        };
        // its files are shown from the directory the run is in, through the module's root
        return Module::at(&root, ctx.shown(&root), None).map(Side::Module);
    }
    if !ritsu_base::fs::exists(p)
        && let Some((name, version)) = module_version(spelled)
    {
        let at_hand = ctx.root().ok();
        let Some((store, digest)) = address::find_version(at_hand.as_deref(), &name, &version) else {
            return Err(address::fetch_version(&name.to_string(), &version));
        };
        return Module::at(&store.files(&digest), spelled.to_string(), Some(digest)).map(Side::Module);
    }
    let a = address::parse(spelled)?;
    let opened = address::open(&a)?;
    if !opened.with(|| ritsu_base::fs::is_file(Path::new(&opened.path))) {
        return Err(tr!("`{spelled}` を読めません", "cannot read `{spelled}`"));
    }
    Ok(Side::File(FileSide { spelled: spelled.to_string(), opened }))
}

/// `<module>@<version>`: a module's name and a version.
fn module_version(s: &str) -> Option<(ritsu_base::modules::names::ModuleName, ritsu_base::modules::semver::Version)> {
    let (name, version) = s.rsplit_once('@')?;
    if name.is_empty() || name.starts_with('@') {
        return None;
    }
    let version = ritsu_base::modules::semver::Version::parse(version).ok()?;
    let name = ritsu_base::modules::names::ModuleName::parse(name).ok()?;
    Some((name, version))
}

impl Module {
    /// The module whose root is `root`, shown as `label`.
    pub fn at(root: &Path, label: String, digest: Option<String>) -> Result<Module, Text> {
        Module::through(root, label, digest, None)
    }

    /// The same, read through `files` (and every file of it read through them after).
    pub fn through(root: &Path, label: String, digest: Option<String>, files: Option<Rc<dyn Files>>) -> Result<Module, Text> {
        let mut m = within(files.clone(), || Module::read(root, label, digest))?;
        m.through = files;
        Ok(m)
    }

    fn read(root: &Path, label: String, digest: Option<String>) -> Result<Module, Text> {
        let file = root.join(manifest::FILE);
        let bytes = ritsu_base::fs::raw::read(&file).map_err(|_| tr!("`{label}` の `ritsu.mod` を読めません", "cannot read the `ritsu.mod` of `{label}`"))?;
        let m = manifest::parse(&bytes, manifest::FILE).map_err(|ds| {
            let first = ds.first().map(|d| d.message.clone()).unwrap_or_default();
            tr!("`{label}` の `ritsu.mod` を読めません（{}）", "cannot read the `ritsu.mod` of `{label}` ({})", first.ja; first.en)
        })?;
        let files: BTreeMap<String, Vec<u8>> = digest::module_files(root, &m)?.into_iter().collect();
        Ok(Module { name: m.module.value.to_string(), version: m.version.as_ref().map(|v| v.value.to_string()), label, root: root.to_path_buf(), digest, manifest: m, files, through: None })
    }

    /// How a file of this side is shown: after the side's label, or as it is when the side is the
    /// directory the run is in.
    fn shown(&self, rel: &str) -> String {
        if self.label == "." { rel.to_string() } else { format!("{}/{rel}", self.label) }
    }

    /// One of its files, as a side of a diff of behavior.
    fn spot(&self, rel: &str) -> Spot {
        Spot { read: self.root.join(rel).to_string_lossy().into_owned(), shown: self.shown(rel), through: self.through.clone() }
    }
}

/// `f`, run reading `files` (the files as they are when None).
pub fn within<R>(files: Option<Rc<dyn Files>>, f: impl FnOnce() -> R) -> R {
    match files {
        Some(x) => ritsu_base::fs::with(x, f),
        None => f(),
    }
}

/// One side of a diff of behavior: the file as the language reads it (a path, or a spelling of
/// DESIGN 17.9.1), as the output shows it, and the files it is read through.
#[derive(Clone)]
pub struct Spot {
    pub read: String,
    pub shown: String,
    pub through: Option<Rc<dyn Files>>,
}

// ── What a diff holds ───────────────────────────────────────────────────

/// One side, as the output heads it.
#[derive(Clone, Debug, Default)]
pub struct Head {
    /// The side as written.
    pub path: String,
    pub module: Option<String>,
    pub version: Option<String>,
    pub digest: Option<String>,
}

/// The files of the two versions, by the path from each root.
#[derive(Clone, Debug, Default)]
pub struct FileLists {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
    pub same: usize,
}

/// A `require` line, by the dependency's name.
#[derive(Clone, Debug, PartialEq)]
pub struct Req {
    pub name: String,
    /// The module it is: the repository of an `oci` line, the path of a `path` one.
    pub module: String,
    pub version: Option<String>,
    /// The words after the name (`oci "example.com/acme/tax" 1.0.0`, `path "../tax"`).
    pub words: String,
}

#[derive(Clone, Debug, Default)]
pub struct Requires {
    pub added: Vec<Req>,
    pub removed: Vec<Req>,
    /// The name, and the words of the old line and the new one.
    pub changed: Vec<(String, String, String)>,
}

/// What a language's diff said of one file.
#[derive(Clone, Debug)]
pub struct Said {
    pub text: String,
    pub markdown: String,
    pub json: Json,
    /// Whether the answers differ (the language's own diff exits 1).
    pub differs: bool,
    /// Whether the claim that every other input gets the same answer was earned.
    pub total: bool,
    /// Whether it says anything beyond "the same": the deciding policies of a gate changed, say.
    pub more: bool,
}

/// The behavior of one file both versions have.
#[derive(Clone, Debug)]
pub struct Behavior {
    pub file: String,
    pub tool: Tool,
    /// Err, with what the language said, when the two could not be compared.
    pub result: Result<Said, Text>,
}

impl Behavior {
    /// Whether the language's own text is worth showing under the file.
    pub(super) fn shows(&self) -> bool {
        self.result.as_ref().is_ok_and(|s| s.differs || !s.total || s.more)
    }
}

/// How an interface changed at one place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum How {
    Added,
    Removed,
    Changed,
}

impl How {
    fn word(self) -> &'static str {
        match self {
            How::Added => "added",
            How::Removed => "removed",
            How::Changed => "changed",
        }
    }
}

/// One change of an interface: where (a JSON Pointer with names in the place of positions), how,
/// and the values.
#[derive(Clone, Debug)]
pub struct IChange {
    pub at: String,
    pub how: How,
    pub old: Option<Json>,
    pub new: Option<Json>,
}

impl IChange {
    /// A candidate for a change that breaks a user: a removal or a change, not of a description
    /// (words for a person, which no caller reads).
    pub fn may_break(&self) -> bool {
        self.how != How::Added && !self.at.rsplit('/').next().is_some_and(|k| k == "description")
    }
}

/// The interface of one file whose bytes changed.
#[derive(Clone, Debug)]
pub struct Iface {
    pub file: String,
    pub tool: Option<Tool>,
    /// `api`, `items` or `digest`.
    pub how: &'static str,
    /// The version at the head of the file (`v1` → `v2`), when it changed.
    pub version: Option<(String, String)>,
    pub changes: Vec<IChange>,
    /// A book's E050, E051 and W107 against the old version.
    pub breaks: Vec<ritsu_ports::Finding>,
    /// Why the interface was compared by its digest alone.
    pub note: Option<Text>,
}

impl Iface {
    /// Whether nothing is to be said of it but that it is the same: the language's own view of the
    /// file did not change, though its bytes did. The text and the Markdown count these in one line;
    /// the JSON lists each.
    pub fn quiet(&self) -> bool {
        self.how != "digest" && self.changes.is_empty() && self.breaks.is_empty() && self.note.is_none() && self.version.is_none()
    }

    /// A book's E050 or E051: what the data already in a database cannot follow.
    pub fn breaking(&self) -> bool {
        self.breaks.iter().any(|f| f.severity == ritsu_base::diag::Severity::Error)
    }
}

/// The diff of two versions of a module, or of two files.
#[derive(Clone, Debug, Default)]
pub struct ModDiff {
    pub old: Head,
    pub new: Head,
    /// Two files, not two modules.
    pub of_files: bool,
    pub files: FileLists,
    pub requires: Requires,
    pub behavior: Vec<Behavior>,
    pub interface: Vec<Iface>,
}

impl ModDiff {
    /// Whether a difference is shown in behavior or in an interface: answers that differ, a file of
    /// a language added or removed (all it holds came or went), a change of an interface, a book's
    /// E050 or E051. A file of no language (data, a document) whose bytes changed is listed, and is
    /// no such difference.
    pub fn differs(&self) -> bool {
        self.behavior_differs()
            || self.files.added.iter().chain(&self.files.removed).any(|f| tool_of(f).is_some())
            || self.interface.iter().any(|i| !i.changes.is_empty() || i.breaking())
    }

    /// Whether the answers of a file differ.
    pub fn behavior_differs(&self) -> bool {
        self.behavior.iter().any(|b| b.result.as_ref().is_ok_and(|s| s.differs))
    }

    /// Whether the behavior of a file could not be compared.
    pub fn uncompared(&self) -> bool {
        self.behavior.iter().any(|b| b.result.is_err())
    }

    /// Whether the behavior of every file was compared whole.
    pub fn whole(&self) -> bool {
        self.behavior.iter().all(|b| b.result.as_ref().is_ok_and(|s| s.total))
    }

    /// 1 for a difference shown, 2 when nothing is shown and a file could not be compared, else 0.
    pub fn exit(&self) -> u8 {
        if self.differs() {
            1
        } else if self.uncompared() {
            2
        } else {
            0
        }
    }
}

// ── Comparing ───────────────────────────────────────────────────────────

/// The language whose diff of behavior a file has: rulec, koyomi, sekisho.
pub(super) fn behaves(rel: &str) -> Option<Tool> {
    [Tool::Rulec, Tool::Koyomi, Tool::Sekisho].into_iter().find(|t| t.extensions().iter().any(|e| rel.ends_with(e)))
}

/// The language of a file, by its ending.
pub(super) fn tool_of(rel: &str) -> Option<Tool> {
    [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Sekisho, Tool::Kaname, Tool::Yuen, Tool::Sakai, Tool::Dandori, Tool::Geas, Tool::Proto]
        .into_iter()
        .find(|t| t.extensions().iter().any(|e| rel.ends_with(e)))
}

fn require_of(r: &manifest::Require) -> Req {
    let (module, version) = match &r.source {
        manifest::Source::Oci { repository, min } => (repository.clone(), Some(min.to_string())),
        manifest::Source::Path { path } => (path.clone(), None),
    };
    Req { name: r.name.clone(), module, version, words: r.source.words() }
}

fn requires(o: &manifest::Manifest, n: &manifest::Manifest) -> Requires {
    let mut out = Requires::default();
    for r in &o.requires {
        match n.require(&r.name) {
            None => out.removed.push(require_of(r)),
            Some(x) if x.source != r.source => out.changed.push((r.name.clone(), r.source.words(), x.source.words())),
            Some(_) => {}
        }
    }
    for r in &n.requires {
        if o.require(&r.name).is_none() {
            out.added.push(require_of(r));
        }
    }
    out
}

/// Two versions of a module.
pub fn modules(o: &Module, n: &Module, budget: Option<u64>, lang: Lang, joined: &Joined) -> ModDiff {
    let head = |m: &Module| Head { path: m.label.clone(), module: Some(m.name.clone()), version: m.version.clone(), digest: m.digest.clone() };
    let mut d = ModDiff { old: head(o), new: head(n), requires: requires(&o.manifest, &n.manifest), ..ModDiff::default() };
    for (rel, bytes) in &o.files {
        match n.files.get(rel) {
            None => d.files.removed.push(rel.clone()),
            Some(b) if b != bytes => d.files.changed.push(rel.clone()),
            Some(_) => d.files.same += 1,
        }
    }
    d.files.added = n.files.keys().filter(|k| !o.files.contains_key(*k)).cloned().collect();
    // nothing changed: the same answers, which need no walk
    let moved = !d.files.added.is_empty() || !d.files.removed.is_empty() || !d.files.changed.is_empty();
    for rel in o.files.keys().filter(|k| moved && n.files.contains_key(*k)) {
        let Some(tool) = behaves(rel) else { continue };
        let result = behavior(tool, &o.spot(rel), &n.spot(rel), budget, lang, joined);
        d.behavior.push(Behavior { file: rel.clone(), tool, result });
    }
    // `ritsu.mod` changes are said as the versions of the title and the `require` lines
    for rel in d.files.changed.iter().filter(|r| r.as_str() != manifest::FILE) {
        let at = |m: &Module| At { read: m.root.join(rel), files: m.through.clone(), root: m.root.clone(), rel: rel.clone(), label: m.label.clone(), shown: m.shown(rel) };
        d.interface.push(interface(rel, &at(o), &at(n), lang, joined));
    }
    d
}

/// Two files.
fn files(o: &FileSide, n: &FileSide, budget: Option<u64>, lang: Lang, joined: &Joined) -> ModDiff {
    let head = |f: &FileSide| Head { path: f.spelled.clone(), ..Head::default() };
    let mut d = ModDiff { old: head(o), new: head(n), of_files: true, ..ModDiff::default() };
    let read = |f: &FileSide| f.opened.with(|| ritsu_base::fs::read(Path::new(&f.opened.path))).unwrap_or_default();
    let (ob, nb) = (read(o), read(n));
    let file = n.spelled.clone();
    if ob == nb {
        d.files.same = 1;
    } else {
        d.files.changed.push(file.clone());
    }
    if let Some(tool) = behaves(&o.opened.path).or_else(|| behaves(&n.opened.path)) {
        // each language reads the spellings of DESIGN 17.9.1 itself
        let spot = |f: &FileSide| Spot { read: f.spelled.clone(), shown: f.spelled.clone(), through: None };
        let result = behavior(tool, &spot(o), &spot(n), budget, lang, joined);
        d.behavior.push(Behavior { file: file.clone(), tool, result });
    }
    if ob != nb {
        let at = |f: &FileSide| {
            let read = PathBuf::from(&f.opened.path);
            let root = f.opened.with(|| ritsu_base::paths::find_root(&read));
            let rel = f.opened.with(|| ritsu_base::paths::from_root(&root, &read)).unwrap_or_else(|| f.opened.path.clone());
            At { read, files: f.opened.files.clone(), root, rel, label: f.spelled.clone(), shown: f.spelled.clone() }
        };
        d.interface.push(interface(&file, &at(o), &at(n), lang, joined));
    }
    d
}

/// What one language's diff says of a file over two versions, each side read through its own files.
pub fn behavior(tool: Tool, old: &Spot, new: &Spot, budget: Option<u64>, lang: Lang, joined: &Joined) -> Result<Said, Text> {
    match tool {
        Tool::Rulec => {
            let rl = if lang == Lang::Ja { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En };
            let budget = budget.map(|b| usize::try_from(b).unwrap_or(usize::MAX));
            let w = rulec::i18n::with(rl, || rulec::cli::diff_whole_through((&old.read, old.through.clone()), (&new.read, new.through.clone()), (&old.shown, &new.shown), budget))
                .map_err(Text::same)?;
            let json = ritsu_base::json::parse(&w.json).map_err(|_| Text::same(w.json.clone()))?;
            Ok(Said { text: w.text, markdown: w.markdown, json, differs: w.differs, total: w.total, more: false })
        }
        Tool::Koyomi => {
            let budget = budget.unwrap_or(koyomi::check::DEFAULT_BUDGET);
            // a spelling of DESIGN 17.9.1 is read as koyomi reads it; a path through the side's files
            let side = |s: &Spot| match &s.through {
                None => koyomi::diff::spelled(&s.read, budget).map(|mut x| {
                    x.shown = s.shown.clone();
                    x
                }),
                Some(f) => koyomi::diff::side(&s.read, &s.shown, Some(f.clone()), budget),
            };
            let (o, n) = (side(old)?, side(new)?);
            let d = koyomi::diff::diff(&o, &n, budget)?;
            let json = ritsu_base::json::parse(&serde_json::to_string(&koyomi::diff::to_json(&d, lang)).unwrap_or_default()).map_err(|_| Text::same("koyomi diff"))?;
            Ok(Said { text: koyomi::diff::text(&d, lang), markdown: koyomi::diff::markdown(&d, lang), json, differs: d.differs(), total: true, more: false })
        }
        Tool::Sekisho => {
            let suite: sekisho::suite::Suite = joined.sekisho().into();
            let walk = budget.map_or(sekisho::check::DEFAULT_BUDGET as u128, u128::from);
            // the check of each side is held to the larger of `--budget` and the default, as
            // `sekisho diff` holds it
            let opts = sekisho::check::Options { budget: budget.unwrap_or(0).max(sekisho::check::DEFAULT_BUDGET), root: None };
            let side = |s: &Spot| match &s.through {
                None => sekisho::diff::side(&s.read, &suite, &opts).map(|mut x| {
                    x.shown = s.shown.clone();
                    x
                }),
                Some(f) => sekisho::diff::side_through(&s.read, &s.shown, Some(f.clone()), &suite, &opts),
            };
            let (o, n) = (side(old)?, side(new)?);
            let d = sekisho::diff::diff(&o, &n, &suite, walk)?;
            let more = d.actions.iter().any(|a| !a.groups.is_empty());
            Ok(Said { text: sekisho::diff::text(&d, lang), markdown: sekisho::diff::markdown(&d, lang), json: sekisho::diff::to_json(&d, lang), differs: d.differs(), total: d.unknown() == 0, more })
        }
        _ => unreachable!("only rulec, koyomi and sekisho make a diff of behavior"),
    }
}

/// Where one side's file is read: the path, the files it is read through, the root of its references
/// and the path from it, the side's label, and the file as the output shows it.
struct At {
    read: PathBuf,
    files: Option<Rc<dyn Files>>,
    root: PathBuf,
    rel: String,
    label: String,
    shown: String,
}

impl At {
    fn with<R>(&self, f: impl FnOnce() -> R) -> R {
        match &self.files {
            Some(x) => ritsu_base::fs::with(x.clone(), f),
            None => f(),
        }
    }
}

/// The interface of a file whose bytes changed (DESIGN 17.9.5).
fn interface(file: &str, o: &At, n: &At, lang: Lang, joined: &Joined) -> Iface {
    let tool = tool_of(&o.rel).or_else(|| tool_of(&n.rel));
    let mut out = Iface { file: file.to_string(), tool, how: "digest", version: None, changes: Vec::new(), breaks: Vec::new(), note: None };
    let Some(tool) = tool else { return out };
    match tool {
        Tool::Dandori | Tool::Geas | Tool::Proto => match (o.with(|| items_of(tool, &o.root, &o.rel, &o.label, joined)), n.with(|| items_of(tool, &n.root, &n.rel, &n.label, joined))) {
            (Ok(a), Ok(b)) => {
                out.how = "items";
                out.changes = compare_items(&a, &b);
            }
            (Err(t), _) | (_, Err(t)) => out.note = Some(t),
        },
        _ => match (o.with(|| api_of(tool, &o.read, &o.label, joined)), n.with(|| api_of(tool, &n.read, &n.label, joined))) {
            (Ok(a), Ok(b)) => {
                out.how = "api";
                let (a, va) = without_heads(tool, cleaned(a));
                let (b, vb) = without_heads(tool, cleaned(b));
                compare_json("", &a, &b, &mut out.changes);
                if let (Some(x), Some(y)) = (va, vb)
                    && x != y
                {
                    // said once, at the top: the same change of a `version` inside is not listed again
                    out.changes.retain(|c| !(c.at.ends_with("/version") && c.old.as_ref().map(scalar).as_ref() == Some(&x) && c.new.as_ref().map(scalar).as_ref() == Some(&y)));
                    out.version = Some((x, y));
                }
            }
            (Err(t), _) | (_, Err(t)) => out.note = Some(t),
        },
    }
    if tool == Tool::Chobo {
        let text = |a: &At| a.with(|| ritsu_base::fs::read_to_string(&a.read)).ok();
        if let (Some(a), Some(b)) = (text(o), text(n)) {
            match chobo::diffbase::between(&a, &b, &o.label) {
                Ok(ds) => {
                    out.breaks = ds
                        .into_iter()
                        .map(|mut d| {
                            d.file = n.shown.clone();
                            d.rel = file.to_string();
                            ritsu_ports::Finding::of(&d, Some(file.to_string()), lang)
                        })
                        .collect()
                }
                Err(t) => out.note = Some(t),
            }
        }
    }
    out
}

/// What a language prints with `api` for the file at `path` (of the side `label`), asked in this
/// process. Err, saying so, for a file that does not pass the language's check.
fn api_of(tool: Tool, path: &Path, label: &str, joined: &Joined) -> Result<Json, Text> {
    let p = path.to_string_lossy().into_owned();
    let word = tool.word();
    let not_passing = || {
        tr!(
            "`{label}` のファイルが `ritsu {word} check` を通らないので、api の JSON を比べられません。中身のダイジェストだけを比べました",
            "the file of `{label}` does not pass `ritsu {word} check`, so its api JSON is not compared; only its digest is"
        )
    };
    let parse = |s: &str| ritsu_base::json::parse(s.trim()).map_err(|_| not_passing());
    let captured = |run: &dyn Fn(&[String], &mut dyn Write, &mut dyn Write) -> u8| -> Result<Json, Text> {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let args = vec!["api".to_string(), p.clone()];
        if run(&args, &mut out, &mut err) != 0 {
            return Err(not_passing());
        }
        parse(&String::from_utf8_lossy(&out))
    };
    match tool {
        Tool::Rulec => {
            let src = ritsu_base::fs::read_to_string(path).map_err(|_| not_passing())?;
            let (f, c) = rulec::prepare(&src, &p).map_err(|_| not_passing())?;
            parse(&rulec::codegen::Gen::new(&f, &c, &src, &p).api())
        }
        Tool::Koyomi => {
            let mut loader = koyomi::calendar::Loader::default();
            let o = koyomi::check::check_file(&p, &koyomi::check::Options::default(), &mut loader).map_err(|_| not_passing())?;
            if o.has_errors() {
                return Err(not_passing());
            }
            let v = match &o.checked {
                Some(koyomi::check::Checked::Calendar(c)) => koyomi::api::calendar_file_json(c),
                Some(koyomi::check::Checked::Dates(m, _)) => koyomi::api::dates_json(m),
                None => return Err(not_passing()),
            };
            parse(&serde_json::to_string(&v).unwrap_or_default())
        }
        Tool::Chobo => {
            let (src, c) = chobo::check::check_file(path).map_err(|_| not_passing())?;
            if chobo::diag::has_errors(&c.diags) {
                return Err(not_passing());
            }
            let book = c.book.ok_or_else(not_passing)?;
            let rep = c.report.unwrap_or_else(|| chobo::check::report(&book));
            let stem = path.file_name().map(|f| f.to_string_lossy().trim_end_matches(".book").to_string()).unwrap_or_default();
            parse(&serde_json::to_string(&chobo::api::api(&book, &src, &rep, &stem)).unwrap_or_default())
        }
        Tool::Sekisho => captured(&|args, out, err| sekisho::run::run(args, joined.sekisho().into(), out, err)),
        Tool::Kaname => captured(&|args, out, err| ritsu::languages::kaname(args, out, err)),
        Tool::Yuen => captured(&|args, out, err| yuen::run::run(args, joined.yuen(), out, err)),
        Tool::Sakai => captured(&|args, out, err| sakai::run::run(args, joined.sakai(), out, err)),
        _ => Err(not_passing()),
    }
}

/// The api JSON without what says nothing of the interface (DESIGN 17.9.5): the version of the tool
/// that printed it (`"rulec": "0.37.0"`, chobo's `"v"`), and where the root is from the run. The
/// version at the head of the file (kaname's is its module's, beside the digest of its text) is
/// taken out, to be said on its own.
fn without_heads(tool: Tool, j: Json) -> (Json, Option<String>) {
    let Json::Obj(pairs) = j else { return (j, None) };
    let mut version = None;
    let mut kept = Vec::new();
    for (k, v) in pairs {
        if k == tool.word() || k == "v" || k == "source_sha256" || k == "root" {
            continue;
        }
        if k == "version" {
            version = Some(scalar(&v));
            continue;
        }
        if k == "module"
            && tool == Tool::Kaname
            && let Json::Obj(inner) = &v
        {
            let mut rest = Vec::new();
            for (ik, iv) in inner {
                match ik.as_str() {
                    "version" => version = Some(scalar(iv)),
                    "sha256" => {}
                    _ => rest.push((ik.clone(), iv.clone())),
                }
            }
            kept.push((k, Json::Obj(rest)));
            continue;
        }
        kept.push((k, v));
    }
    (Json::Obj(kept), version)
}

/// The api JSON without what changes with any byte of the file and says nothing of its shape, at any
/// depth: the digest of its text (`source_sha256`), and its examples (`example`, `examples`: what a
/// language's search finds, or the claims a file checks itself by, not the shape a user calls).
fn cleaned(j: Json) -> Json {
    match j {
        Json::Obj(pairs) => Json::Obj(pairs.into_iter().filter(|(k, _)| !matches!(k.as_str(), "example" | "examples" | "source_sha256")).map(|(k, v)| (k, cleaned(v))).collect()),
        Json::Arr(xs) => Json::Arr(xs.into_iter().map(cleaned).collect()),
        other => other,
    }
}

fn scalar(v: &Json) -> String {
    match v {
        Json::Str(s) => s.clone(),
        other => other.compact(),
    }
}

/// A part of a JSON Pointer: `~` and `/` escaped.
fn part(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

/// The names the elements of an array are matched by: their `name`s, when every element has a
/// string one and no two are the same.
fn names(a: &[Json]) -> Option<Vec<&str>> {
    let ns: Vec<&str> = a.iter().map(|x| x.get("name").and_then(Json::as_str)).collect::<Option<Vec<&str>>>()?;
    let unique: BTreeSet<&str> = ns.iter().copied().collect();
    (unique.len() == ns.len()).then_some(ns)
}

/// The changes from `old` to `new` under `at` (DESIGN 17.9.5): objects by their keys, arrays whose
/// elements each have a string `name` by it, other arrays by position.
pub fn compare_json(at: &str, old: &Json, new: &Json, out: &mut Vec<IChange>) {
    match (old, new) {
        (Json::Obj(a), Json::Obj(b)) => {
            for (k, v) in a {
                let here = format!("{at}/{}", part(k));
                match b.iter().find(|(x, _)| x == k) {
                    Some((_, w)) => compare_json(&here, v, w, out),
                    None => out.push(IChange { at: here, how: How::Removed, old: Some(v.clone()), new: None }),
                }
            }
            for (k, w) in b {
                if !a.iter().any(|(x, _)| x == k) {
                    out.push(IChange { at: format!("{at}/{}", part(k)), how: How::Added, old: None, new: Some(w.clone()) });
                }
            }
        }
        (Json::Arr(a), Json::Arr(b)) => match (names(a), names(b)) {
            (Some(na), Some(nb)) => {
                for (v, nm) in a.iter().zip(&na) {
                    let here = format!("{at}/{}", part(nm));
                    match nb.iter().position(|x| x == nm) {
                        Some(j) => compare_json(&here, v, &b[j], out),
                        None => out.push(IChange { at: here, how: How::Removed, old: Some(v.clone()), new: None }),
                    }
                }
                for (w, nm) in b.iter().zip(&nb) {
                    if !na.contains(nm) {
                        out.push(IChange { at: format!("{at}/{}", part(nm)), how: How::Added, old: None, new: Some(w.clone()) });
                    }
                }
            }
            _ => {
                for (i, (v, w)) in a.iter().zip(b).enumerate() {
                    compare_json(&format!("{at}/{i}"), v, w, out);
                }
                for (i, v) in a.iter().enumerate().skip(b.len()) {
                    out.push(IChange { at: format!("{at}/{i}"), how: How::Removed, old: Some(v.clone()), new: None });
                }
                for (i, w) in b.iter().enumerate().skip(a.len()) {
                    out.push(IChange { at: format!("{at}/{i}"), how: How::Added, old: None, new: Some(w.clone()) });
                }
            }
        },
        (a, b) if a == b => {}
        (a, b) => out.push(IChange { at: if at.is_empty() { "/".into() } else { at.to_string() }, how: How::Changed, old: Some(a.clone()), new: Some(b.clone()) }),
    }
}

/// What a file holds (DESIGN 6.4), each by its pairs written as a JSON Pointer
/// (`/record/order/field/amount`), with its definition.
fn items_of(tool: Tool, root: &Path, rel: &str, label: &str, joined: &Joined) -> Result<Vec<(String, String)>, Text> {
    let word = tool.word();
    let cannot = || {
        tr!(
            "`{label}` のファイルが `ritsu {word} check` を通らないので、中のものを比べられません。中身のダイジェストだけを比べました",
            "the file of `{label}` does not pass `ritsu {word} check`, so what it holds is not compared; only its digest is"
        )
    };
    let pointer = |items: &[(String, String)]| items.iter().map(|(k, v)| format!("/{}/{}", part(k), part(v))).collect::<String>();
    match tool {
        Tool::Proto => {
            let ps = yuen::proto::load(root, rel).map_err(|_| cannot())?;
            let file = ritsu_base::naming::Name::file(Tool::Proto, rel);
            let mut out = Vec::new();
            for kind in ["service", "method", "message", "field", "enum", "value"] {
                for nm in yuen::proto::gather(&ps, &file, kind) {
                    if let Ok(t) = yuen::proto::end_text(&ps, &nm) {
                        out.push((pointer(&nm.items), t));
                    }
                }
            }
            out.sort();
            Ok(out)
        }
        _ => match joined.index.items(tool, root, rel) {
            Some(Ok(items)) => Ok(items.iter().map(|i| (pointer(&i.naming.items), i.text.clone())).collect()),
            _ => Err(cannot()),
        },
    }
}

fn compare_items(a: &[(String, String)], b: &[(String, String)]) -> Vec<IChange> {
    let mut out = Vec::new();
    for (k, t) in a {
        match b.iter().find(|(x, _)| x == k) {
            None => out.push(IChange { at: k.clone(), how: How::Removed, old: Some(Json::str(t.as_str())), new: None }),
            Some((_, u)) if u != t => out.push(IChange { at: k.clone(), how: How::Changed, old: Some(Json::str(t.as_str())), new: Some(Json::str(u.as_str())) }),
            Some(_) => {}
        }
    }
    for (k, u) in b {
        if !a.iter().any(|(x, _)| x == k) {
            out.push(IChange { at: k.clone(), how: How::Added, old: None, new: Some(Json::str(u.as_str())) });
        }
    }
    out
}

// ── Saying it ───────────────────────────────────────────────────────────

pub(super) fn colon(lang: Lang) -> &'static str {
    if lang == Lang::Ja { "：" } else { ": " }
}

/// The colon after a heading whose lines follow it.
pub(super) fn ends(lang: Lang) -> &'static str {
    if lang == Lang::Ja { "：" } else { ":" }
}

pub(super) fn n(k: usize) -> String {
    ritsu_base::text::count(k as u64)
}

/// The title: `example.com/acme/pricing 1.0.0 → 1.1.0`, or the two files.
fn title(d: &ModDiff, q: &str) -> String {
    if d.of_files {
        return format!("{q}{}{q} → {q}{}{q}", d.old.path, d.new.path);
    }
    let v = |h: &Head| h.version.clone().unwrap_or_else(|| "-".into());
    match (&d.old.module, &d.new.module) {
        (Some(a), Some(b)) if a == b => format!("{q}{a}{q} {} → {}", v(&d.old), v(&d.new)),
        (a, b) => format!("{q}{}{q} {} → {q}{}{q} {}", a.clone().unwrap_or_default(), v(&d.old), b.clone().unwrap_or_default(), v(&d.new)),
    }
}

/// `2 changed, 1 the same`, of the files.
fn files_line(f: &FileLists, lang: Lang) -> String {
    let mut parts = Vec::new();
    for (k, ja, en) in [(f.added.len(), "足した", "added"), (f.removed.len(), "消した", "removed"), (f.changed.len(), "変わった", "changed")] {
        if k > 0 {
            parts.push(if lang == Lang::Ja { format!("{ja} {}", n(k)) } else { format!("{} {en}", n(k)) });
        }
    }
    if f.same > 0 {
        parts.push(if lang == Lang::Ja { format!("同じ {}", n(f.same)) } else { format!("{} the same", n(f.same)) });
    }
    if parts.is_empty() {
        return tr!("なし", "none").get(lang).to_string();
    }
    parts.join(if lang == Lang::Ja { "、" } else { ", " })
}

fn change_word(h: How, lang: Lang) -> &'static str {
    match (h, lang) {
        (How::Added, Lang::Ja) => "足した",
        (How::Removed, Lang::Ja) => "消した",
        (How::Changed, Lang::Ja) => "変わった",
        (How::Added, _) => "added",
        (How::Removed, _) => "removed",
        (How::Changed, _) => "changed",
    }
}

/// How a change is headed: `may break: removed`, or `added`.
fn change_head(l: &Line, lang: Lang) -> String {
    if l.may_break { format!("{}{}", tr!("壊しうる：", "may break: ").get(lang), change_word(l.how, lang)) } else { change_word(l.how, lang).to_string() }
}

/// The require lines that changed.
fn require_lines(r: &Requires, lang: Lang) -> Vec<String> {
    let c = colon(lang);
    let mut out = Vec::new();
    for x in &r.added {
        out.push(format!("{}{c}{} {}", change_word(How::Added, lang), x.name, x.words));
    }
    for x in &r.removed {
        out.push(format!("{}{c}{} {}", change_word(How::Removed, lang), x.name, x.words));
    }
    for (name, a, b) in &r.changed {
        out.push(format!("{}{c}{name} {a} → {b}", change_word(How::Changed, lang)));
    }
    out
}

/// What a language's diff came to, in a few words.
pub(super) fn verdict(b: &Behavior, lang: Lang) -> String {
    match &b.result {
        Ok(s) if s.differs => tr!("答えが変わります", "the answers differ"),
        Ok(s) if !s.total => tr!("示せた違いはありません（調べきれなかった入力があります）", "no difference shown (some inputs were not settled)"),
        Ok(s) if s.more => tr!("答えは同じです（ほかの変わりがあります）", "the same answers, with other changes"),
        Ok(_) => tr!("同じ答えです", "the same answers"),
        Err(t) => tr!("比べられません（{}）", "not compared ({})", t.ja; t.en),
    }
    .get(lang)
    .to_string()
}

/// The most characters of a value a line shows.
const WIDTH: usize = 60;

/// A value in a line: its compact JSON, cut at [`WIDTH`] characters.
fn cut(v: &Json) -> String {
    let s = v.compact();
    if s.chars().count() > WIDTH { format!("{}…", s.chars().take(WIDTH - 1).collect::<String>()) } else { s }
}

/// Two long strings, each shown from a little before the first character where they differ.
fn around(x: &str, y: &str) -> (String, String) {
    let (xs, ys): (Vec<char>, Vec<char>) = (x.chars().collect(), y.chars().collect());
    let first = xs.iter().zip(&ys).take_while(|(a, b)| a == b).count();
    let start = first.saturating_sub(20);
    let window = |cs: &[char]| {
        let end = (start + WIDTH - 10).min(cs.len()).max(start.min(cs.len()));
        let mut s = Json::str(cs[start.min(cs.len())..end].iter().collect::<String>()).compact();
        // the quotes stay the ends of the string; where it is cut is marked inside them
        if start > 0 {
            s.insert(1, '…');
        }
        if end < cs.len() {
            s.insert(s.len() - 1, '…');
        }
        s
    };
    (window(&xs), window(&ys))
}

/// The old value and the new one of a change, as a line says them.
fn values_of(c: &IChange) -> Option<String> {
    if c.how != How::Changed {
        return None;
    }
    let (a, b) = (c.old.as_ref()?, c.new.as_ref()?);
    Some(match (a, b) {
        (Json::Str(x), Json::Str(y)) if x.chars().count() > WIDTH - 10 || y.chars().count() > WIDTH - 10 => {
            let (x, y) = around(x, y);
            format!("{x} → {y}")
        }
        _ => format!("{} → {}", cut(a), cut(b)),
    })
}

/// A line of the changes of an interface: one change, or changes alike at places that differ in one
/// part.
struct Line {
    how: How,
    may_break: bool,
    at: String,
    values: Option<String>,
}

/// The changes of an interface as lines, those that may break first. Changes alike (the same way,
/// the same values) at places that differ in one part only, three or more of them, are one line with
/// the parts in braces (`/{python,go,rust}/outputs/fee`): a rule's api has a part for each language it
/// is generated in, and a change of the rule shows in each.
fn grouped(changes: &[IChange]) -> Vec<Line> {
    type Key = (usize, String, String, How, bool, Option<String>);
    let values: Vec<Option<String>> = changes.iter().map(values_of).collect();
    let parts: Vec<Vec<&str>> = changes.iter().map(|c| c.at.split('/').collect()).collect();
    let key = |i: usize, k: usize| -> Key { (k, parts[i][..k].join("/"), parts[i][k + 1..].join("/"), changes[i].how, changes[i].may_break(), values[i].clone()) };
    let mut alike: BTreeMap<Key, Vec<usize>> = BTreeMap::new();
    for i in 0..changes.len() {
        for k in 1..parts[i].len() {
            alike.entry(key(i, k)).or_default().push(i);
        }
    }
    let mut placed = vec![false; changes.len()];
    let mut out = Vec::new();
    for i in 0..changes.len() {
        if placed[i] {
            continue;
        }
        let group = (1..parts[i].len()).find_map(|k| {
            let members: Vec<usize> = alike[&key(i, k)].iter().copied().filter(|&j| !placed[j]).collect();
            (members.len() >= 3).then_some((k, members))
        });
        let at = match group {
            Some((k, members)) => {
                let names: Vec<&str> = members.iter().map(|&j| parts[j][k]).collect();
                let mut at: Vec<String> = parts[i].iter().map(|p| p.to_string()).collect();
                at[k] = format!("{{{}}}", names.join(","));
                for j in members {
                    placed[j] = true;
                }
                at.join("/")
            }
            None => {
                placed[i] = true;
                changes[i].at.clone()
            }
        };
        out.push(Line { how: changes[i].how, may_break: changes[i].may_break(), at, values: values[i].clone() });
    }
    out.sort_by_key(|l| !l.may_break);
    out
}

/// A book's diagnostic against the old version, in a line: its code, its line, and what it says.
fn break_line(f: &ritsu_ports::Finding, lang: Lang, q: &str) -> String {
    let c = colon(lang);
    let label = if f.severity == ritsu_base::diag::Severity::Error { tr!("データを壊す差", "breaks the data") } else { tr!("注意", "warning") };
    let message = f.json.get("message").and_then(Json::as_str).unwrap_or_default();
    let line = match (f.line, lang) {
        (Some(l), Lang::Ja) => format!("（{l} 行目）"),
        (Some(l), _) => format!(" (line {l})"),
        (None, _) => String::new(),
    };
    format!("{}{c}{q}{}{q}{line}{c}{message}", label.get(lang), f.code)
}

/// The head of a file's interface: `rules/fee.rule (rulec, api): 3 changes, 2 of them may break a
/// user`, the file in `q`.
fn iface_line(i: &Iface, lang: Lang, q: &str) -> String {
    let how = match (i.how, lang) {
        ("api", _) => "api",
        ("items", Lang::Ja) => "中のもの",
        ("items", _) => "items",
        (_, Lang::Ja) => "ダイジェスト",
        _ => "digest",
    };
    let breaking = i.changes.iter().filter(|c| c.may_break()).count();
    let what = if i.how == "digest" {
        tr!("中身が変わりました", "its bytes changed").get(lang).to_string()
    } else if i.changes.is_empty() {
        tr!("違いはありません", "no change").get(lang).to_string()
    } else if lang == Lang::Ja {
        format!("差が {} 件、うち使う側を壊しうるもの {} 件", n(i.changes.len()), n(breaking))
    } else {
        format!("{}, {} of them may break a user", ritsu_base::text::plural(i.changes.len(), "change", "changes"), n(breaking))
    };
    let comma = if lang == Lang::Ja { "、" } else { ", " };
    let by = i.tool.map_or(how.to_string(), |t| format!("{}{comma}{how}", t.word()));
    format!("{q}{}{q} ({by}){}{what}", i.file, colon(lang))
}

/// How many files whose bytes changed keep their interface, in a line; None when there is none.
fn quiet_line(d: &ModDiff, lang: Lang, q: &str) -> Option<String> {
    let k = d.interface.iter().filter(|i| i.quiet()).count();
    let (all, files) = (k == d.interface.len(), ritsu_base::text::plural(k, "file", "files"));
    let t = match (k, all) {
        (0, _) => return None,
        (1, true) => {
            let f = &d.interface[0].file;
            tr!("{q}{f}{q} のインターフェースは変わりません", "the interface of {q}{f}{q} is the same")
        }
        (_, true) => tr!("{} 個のファイルのどれも、インターフェースは変わりません", "the interface of each of the {files} is the same", n(k); ),
        (_, false) => tr!("ほかの {} 個のファイルは、インターフェースが変わりません", "the interface of the other {files} is the same", n(k); ),
    };
    Some(t.get(lang).to_string())
}

fn closing(d: &ModDiff, lang: Lang) -> String {
    if d.differs() {
        tr!("振る舞いかインターフェースに、示せた違いがあります。", "a difference is shown in behavior or in an interface.")
    } else if d.uncompared() {
        tr!(
            "振る舞いを比べられなかったファイルがあるので、違いが無いとは言いません。",
            "the behavior of some files could not be compared, so this does not say there is no difference."
        )
    } else if !d.whole() {
        tr!("示せた違いはありません。ただし、調べきれなかった入力があります。", "no difference is shown, though some inputs were not settled.")
    } else {
        tr!("振る舞いにもインターフェースにも、違いはありません。", "no difference in behavior or in an interface.")
    }
    .get(lang)
    .to_string()
}

fn candidates(lang: Lang) -> String {
    tr!(
        "壊しうる差は候補です。使う側を壊すかどうかは、使う側の検査が決めます。",
        "a change that may break a user is a candidate: whether it does is for that user's check to say."
    )
    .get(lang)
    .to_string()
}

pub(super) fn indent(s: &str, by: &str) -> String {
    s.lines().map(|l| if l.is_empty() { String::new() } else { format!("{by}{l}") }).collect::<Vec<_>>().join("\n") + "\n"
}

/// The lines of the files added, removed and changed.
fn file_rows(f: &FileLists) -> Vec<(How, &String)> {
    let mut rows = Vec::new();
    for (how, list) in [(How::Added, &f.added), (How::Removed, &f.removed), (How::Changed, &f.changed)] {
        rows.extend(list.iter().map(|x| (how, x)));
    }
    rows
}

pub(super) fn more_line(k: usize, lang: Lang) -> String {
    tr!("ほか {} 件（JSON に全部あります）", "and {} more (the JSON has them all)", n(k)).get(lang).to_string()
}

/// The diff as text.
pub fn text(d: &ModDiff, lang: Lang) -> String {
    let c = colon(lang);
    let mut o = format!("ritsu mod diff{c}{}\n", title(d, ""));
    o.push_str(&format!("  {}{c}{}\n", tr!("古い版", "old").get(lang), d.old.path));
    o.push_str(&format!("  {}{c}{}\n", tr!("新しい版", "new").get(lang), d.new.path));
    o.push_str(&body_text(d, lang));
    o
}

/// The text after the heads, which `ritsu mod report` puts under each module.
pub fn body_text(d: &ModDiff, lang: Lang) -> String {
    let c = colon(lang);
    let mut o = String::new();
    if !d.of_files {
        o.push_str(&format!("\n{}{c}{}\n", tr!("ファイル", "files").get(lang), files_line(&d.files, lang)));
        let rows = file_rows(&d.files);
        for (how, f) in rows.iter().take(SHOWN) {
            o.push_str(&format!("  {}{c}{f}\n", change_word(*how, lang)));
        }
        if rows.len() > SHOWN {
            o.push_str(&format!("  {}\n", more_line(rows.len() - SHOWN, lang)));
        }
        let rl = require_lines(&d.requires, lang);
        if rl.is_empty() {
            o.push_str(&format!("require{c}{}\n", tr!("変わりません", "no change").get(lang)));
        } else {
            o.push_str(&format!("require{}\n", ends(lang)));
            for l in rl {
                o.push_str(&format!("  {l}\n"));
            }
        }
    }
    if !d.behavior.is_empty() {
        o.push_str(&format!("\n{}{}\n", tr!("振る舞い", "behavior").get(lang), ends(lang)));
        for b in &d.behavior {
            o.push_str(&format!("  {} ({}){c}{}\n", b.file, b.tool.word(), verdict(b, lang)));
            if let (true, Ok(s)) = (b.shows(), &b.result) {
                o.push_str(&indent(&s.text, "    "));
            }
        }
    }
    if !d.interface.is_empty() {
        o.push_str(&format!("\n{}{}\n", tr!("インターフェース", "interface").get(lang), ends(lang)));
        for i in d.interface.iter().filter(|i| !i.quiet()) {
            o.push_str(&format!("  {}\n", iface_line(i, lang, "")));
            if let Some((a, b)) = &i.version {
                o.push_str(&format!("    {}{c}{a} → {b}\n", tr!("ファイルの頭の版", "the version at the head of the file").get(lang)));
            }
            for f in &i.breaks {
                o.push_str(&format!("    {}\n", break_line(f, lang, "")));
            }
            let g = grouped(&i.changes);
            for l in g.iter().take(SHOWN) {
                match &l.values {
                    Some(v) if i.how == "api" => o.push_str(&format!("    {}{c}{}{c}{v}\n", change_head(l, lang), l.at)),
                    _ => o.push_str(&format!("    {}{c}{}\n", change_head(l, lang), l.at)),
                }
            }
            if g.len() > SHOWN {
                o.push_str(&format!("    {}\n", more_line(g.len() - SHOWN, lang)));
            }
            if let Some(t) = &i.note {
                o.push_str(&format!("    {}\n", t.get(lang)));
            }
        }
        if let Some(l) = quiet_line(d, lang, "") {
            o.push_str(&format!("  {l}\n"));
        }
        if d.interface.iter().any(|i| i.changes.iter().any(IChange::may_break)) {
            o.push_str(&format!("  {}\n", candidates(lang)));
        }
    }
    o.push_str(&format!("\n{}\n", closing(d, lang)));
    o
}

/// A cell of a Markdown table: `|` escaped, on one line.
pub(super) fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

pub(super) fn html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The diff as Markdown.
pub fn markdown(d: &ModDiff, lang: Lang) -> String {
    let c = colon(lang);
    let mut o = format!("### ritsu mod diff{c}{}\n\n", title(d, "`"));
    o.push_str(&format!("- {}{c}`{}`\n", tr!("古い版", "Old").get(lang), d.old.path));
    o.push_str(&format!("- {}{c}`{}`\n", tr!("新しい版", "New").get(lang), d.new.path));
    o.push_str(&body_markdown(d, lang, 4, SHOWN));
    guarded(&o)
}

/// A language's Markdown with each table held to `rows` rows, and a line after a table cut, saying
/// how many more rows the JSON has.
pub(super) fn capped(md: &str, rows: usize, lang: Lang) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let table = lines[i].starts_with('|') && lines.get(i + 1).is_some_and(|l| l.starts_with("|---") || l.starts_with("| ---") || l.starts_with("|:"));
        if !table {
            out.push(lines[i].to_string());
            i += 1;
            continue;
        }
        let end = (i + 2..lines.len()).find(|&j| !lines[j].starts_with('|')).unwrap_or(lines.len());
        let body = end - i - 2;
        out.extend(lines[i..(i + 2 + body.min(rows))].iter().map(|l| l.to_string()));
        if body > rows {
            out.push(String::new());
            out.push(ritsu_base::text::capitalize(&tr!("表の行はほか {} 行あります（JSON に全部あります）", "and {} more rows (the JSON has them all)", n(body - rows)).get(lang).to_string()));
        }
        i = end;
    }
    let mut s = out.join("\n");
    if md.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// Markdown with every word that holds an `@` in backticks, outside the code it already has (a span
/// in backticks, `<code>`, a block between fences): GitHub takes `@name` for a person to notify
/// (DESIGN 17.10).
pub fn guarded(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut fence = false;
    for (k, line) in md.split('\n').enumerate() {
        if k > 0 {
            out.push('\n');
        }
        if line.trim_start().starts_with("```") {
            fence = !fence;
            out.push_str(line);
            continue;
        }
        if fence {
            out.push_str(line);
            continue;
        }
        let mut word = String::new();
        let flush = |word: &mut String, out: &mut String| {
            if word.contains('@') {
                out.push('`');
                out.push_str(word);
                out.push('`');
            } else {
                out.push_str(word);
            }
            word.clear();
        };
        let mut rest = line;
        while let Some(c) = rest.chars().next() {
            if c == '`' {
                flush(&mut word, &mut out);
                let ticks = rest.len() - rest.trim_start_matches('`').len();
                let close = &rest[..ticks];
                match rest[ticks..].find(close) {
                    Some(at) => {
                        out.push_str(&rest[..ticks + at + ticks]);
                        rest = &rest[ticks + at + ticks..];
                    }
                    None => {
                        out.push_str(rest);
                        rest = "";
                    }
                }
                continue;
            }
            if rest.starts_with("<code>") {
                flush(&mut word, &mut out);
                let end = rest.find("</code>").map_or(rest.len(), |at| at + "</code>".len());
                out.push_str(&rest[..end]);
                rest = &rest[end..];
                continue;
            }
            if c.is_whitespace() || c == '|' {
                flush(&mut word, &mut out);
                out.push(c);
            } else {
                word.push(c);
            }
            rest = &rest[c.len_utf8()..];
        }
        flush(&mut word, &mut out);
    }
    out
}

/// A language's Markdown inside a fold: its headings as bold lines, so that they do not stand among
/// the headings of the page.
pub(super) fn folded(md: &str) -> String {
    let mut fence = false;
    let mut out = Vec::new();
    for l in md.trim_end().lines() {
        if l.trim_start().starts_with("```") {
            fence = !fence;
        }
        match l.trim_start_matches('#').strip_prefix(' ') {
            Some(rest) if !fence && l.starts_with('#') => out.push(format!("**{}**", rest.trim())),
            _ => out.push(l.to_string()),
        }
    }
    out.join("\n")
}

/// The require lines that changed, as Markdown list items.
fn require_items(r: &Requires, lang: Lang) -> Vec<String> {
    let c = colon(lang);
    let mut out = Vec::new();
    for x in &r.added {
        out.push(format!("{}{c}`{} {}`", change_word(How::Added, lang), x.name, x.words));
    }
    for x in &r.removed {
        out.push(format!("{}{c}`{} {}`", change_word(How::Removed, lang), x.name, x.words));
    }
    for (name, a, b) in &r.changed {
        out.push(format!("{}{c}`{name}`{c}`{a}` → `{b}`", change_word(How::Changed, lang)));
    }
    out
}

/// The Markdown after the heads, its headings at `level`: what `ritsu mod report` puts under each
/// module. Every path is in backticks.
pub fn body_markdown(d: &ModDiff, lang: Lang, level: usize, rows: usize) -> String {
    let c = colon(lang);
    let h = "#".repeat(level);
    let mut o = String::new();
    if !d.of_files {
        o.push_str(&format!("- {}{c}{}\n", tr!("ファイル", "Files").get(lang), files_line(&d.files, lang)));
        let rl = require_items(&d.requires, lang);
        if rl.is_empty() {
            o.push_str(&format!("- require{c}{}\n", tr!("変わりません", "no change").get(lang)));
        } else {
            o.push_str(&format!("- require{}\n", ends(lang)));
            for l in rl {
                o.push_str(&format!("  - {l}\n"));
            }
        }
        let files = file_rows(&d.files);
        if !files.is_empty() {
            o.push_str(&format!("\n| {} | {} |\n|---|---|\n", tr!("変わり方", "Change").get(lang), tr!("ファイル", "File").get(lang)));
            for (how, f) in files.iter().take(rows) {
                o.push_str(&format!("| {} | `{}` |\n", change_word(*how, lang), cell(f)));
            }
            if files.len() > rows {
                o.push_str(&format!("\n{}\n", ritsu_base::text::capitalize(&more_line(files.len() - rows, lang))));
            }
        }
    }
    if !d.behavior.is_empty() {
        o.push_str(&format!("\n{h} {}\n\n", tr!("振る舞い", "Behavior").get(lang)));
        let mut listing = false;
        for b in &d.behavior {
            match (b.shows(), &b.result) {
                (true, Ok(s)) => {
                    if listing {
                        o.push('\n');
                    }
                    listing = false;
                    o.push_str(&format!(
                        "<details><summary><code>{}</code> ({}){c}{}</summary>\n\n{}\n\n</details>\n\n",
                        html(&b.file),
                        b.tool.word(),
                        verdict(b, lang),
                        capped(&folded(&s.markdown), rows, lang)
                    ));
                }
                _ => {
                    listing = true;
                    o.push_str(&format!("- `{}` ({}){c}{}\n", b.file, b.tool.word(), verdict(b, lang)));
                }
            }
        }
        if !listing {
            o.pop();
        }
    }
    if !d.interface.is_empty() {
        o.push_str(&format!("\n{h} {}\n", tr!("インターフェース", "Interface").get(lang)));
        for i in d.interface.iter().filter(|i| !i.quiet()) {
            o.push_str(&format!("\n- {}\n", iface_line(i, lang, "`")));
            if let Some((a, b)) = &i.version {
                o.push_str(&format!("  - {}{c}`{a}` → `{b}`\n", tr!("ファイルの頭の版", "The version at the head of the file").get(lang)));
            }
            for f in &i.breaks {
                o.push_str(&format!("  - {}\n", ritsu_base::text::capitalize(&break_line(f, lang, "**"))));
            }
            if let Some(t) = &i.note {
                o.push_str(&format!("  - {}\n", t.get(lang)));
            }
            let g = grouped(&i.changes);
            if !g.is_empty() {
                o.push_str(&format!(
                    "\n| {} | {} | {} |\n|---|---|---|\n",
                    tr!("変わり方", "Change").get(lang),
                    tr!("場所", "Where").get(lang),
                    tr!("古い値 → 新しい値", "Old → new").get(lang)
                ));
                for l in g.iter().take(rows) {
                    let v = l.values.as_ref().filter(|_| i.how == "api").map(|v| format!("`{}`", cell(v))).unwrap_or_default();
                    o.push_str(&format!("| {} | `{}` | {v} |\n", change_head(l, lang), cell(&l.at)));
                }
                if g.len() > rows {
                    o.push_str(&format!("\n{}\n", ritsu_base::text::capitalize(&more_line(g.len() - rows, lang))));
                }
            }
        }
        if let Some(l) = quiet_line(d, lang, "`") {
            o.push_str(&format!("\n- {}\n", ritsu_base::text::capitalize(&l)));
        }
        if d.interface.iter().any(|i| i.changes.iter().any(IChange::may_break)) {
            o.push_str(&format!("\n{}\n", ritsu_base::text::capitalize(&candidates(lang))));
        }
    }
    o.push_str(&format!("\n{}\n", ritsu_base::text::capitalize(&closing(d, lang))));
    o
}

fn head_json(h: &Head) -> Json {
    Json::obj([("path", Json::str(h.path.as_str())), ("module", Json::opt_str(h.module.clone())), ("version", Json::opt_str(h.version.clone())), ("digest", Json::opt_str(h.digest.clone()))])
}

fn req_json(r: &Req) -> Json {
    Json::obj([("name", Json::str(r.name.as_str())), ("module", Json::str(r.module.as_str())), ("version", Json::opt_str(r.version.clone()))])
}

/// One file's interface as the JSON holds it (DESIGN 17.21).
pub fn iface_json(i: &Iface, lang: Lang) -> Json {
    let changes = i.changes.iter().map(|c| {
        Json::obj([
            ("at", Json::str(c.at.as_str())),
            ("change", Json::str(c.how.word())),
            ("old", c.old.clone().unwrap_or(Json::Null)),
            ("new", c.new.clone().unwrap_or(Json::Null)),
            ("may_break", Json::Bool(c.may_break())),
        ])
    });
    Json::obj([
        ("file", Json::str(i.file.as_str())),
        ("tool", i.tool.map_or(Json::Null, |t| Json::str(t.word()))),
        ("how", Json::str(i.how)),
        ("version", i.version.as_ref().map_or(Json::Null, |(a, b)| Json::obj([("old", Json::str(a.as_str())), ("new", Json::str(b.as_str()))]))),
        ("changes", Json::arr(changes)),
        ("breaks", Json::arr(i.breaks.iter().map(|f| f.json.clone()))),
        ("note", i.note.as_ref().map_or(Json::Null, |t| Json::str(t.get(lang)))),
    ])
}

/// One file's behavior as the JSON holds it.
pub fn behavior_json(b: &Behavior, lang: Lang) -> Json {
    let (result, error) = match &b.result {
        Ok(s) => (s.json.clone(), Json::Null),
        Err(t) => (Json::Null, Json::str(t.get(lang))),
    };
    Json::obj([("file", Json::str(b.file.as_str())), ("tool", Json::str(b.tool.word())), ("result", result), ("error", error)])
}

/// The diff as JSON (DESIGN 17.21).
pub fn to_json(d: &ModDiff, lang: Lang) -> Json {
    let mut pairs = vec![("ritsu", Json::str(env!("CARGO_PKG_VERSION")))];
    pairs.extend(json_fields(d, lang));
    Json::obj(pairs)
}

/// The fields of [`to_json`] after `ritsu`, which `ritsu mod report` puts in each module's `diff`.
pub fn json_fields(d: &ModDiff, lang: Lang) -> Vec<(&'static str, Json)> {
    let strs = |xs: &[String]| Json::arr(xs.iter().map(|x| Json::str(x.as_str())));
    let changed = d.requires.changed.iter().map(|(name, a, b)| Json::obj([("name", Json::str(name.as_str())), ("old", Json::str(a.as_str())), ("new", Json::str(b.as_str()))]));
    vec![
        ("old", head_json(&d.old)),
        ("new", head_json(&d.new)),
        ("files", Json::obj([("added", strs(&d.files.added)), ("removed", strs(&d.files.removed)), ("changed", strs(&d.files.changed)), ("same", Json::int(d.files.same as i128))])),
        ("requires", Json::obj([("added", Json::arr(d.requires.added.iter().map(req_json))), ("removed", Json::arr(d.requires.removed.iter().map(req_json))), ("changed", Json::arr(changed))])),
        ("behavior", Json::arr(d.behavior.iter().map(|b| behavior_json(b, lang)))),
        ("interface", Json::arr(d.interface.iter().map(|i| iface_json(i, lang)))),
        ("differs", Json::Bool(d.differs())),
    ]
}
