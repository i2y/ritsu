//! A `.kn` from its text to its diagnostics (DESIGN 11.1, `check`): the syntax (`parse.rs`), the
//! files its `use` lines read and its names (`names.rs`), then every expression's type
//! (`types.rs`). What `kaname check` prints for a file, as text and as JSON, and what `ritsu check`
//! prints of it, is here too.
//!
//! A module another one reads (`use module`) is checked as a module of its own, once in a run:
//! [`Run`] keeps what each came to, and the modules being read, to say a ring (E203).

use crate::ast::{Module, UseKind};
use crate::diag::{self, Diag};
use crate::names::{Dc, Decls, Target, World, beside, canonical, said_notes};
use crate::suite::Suite;
use crate::types::{self, Typed};
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text, capitalize};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A module that passes its check, with everything the check found of it.
pub struct Checked {
    /// The file, as diagnostics show it.
    pub path: String,
    /// The file on the disk: what tells one module from another.
    pub canon: PathBuf,
    pub src: String,
    pub module: Module,
    /// What each `use` line reads, in the module's order.
    pub uses: Vec<Target>,
    /// Every file the module reaches.
    pub world: World,
    pub decls: Decls,
    /// Every expression's type and what its names name, by the expression's id.
    pub typed: Typed,
    /// What the analysis of ranges finds of it (DESIGN 4, 5): set once the module's types pass.
    pub analysis: std::cell::OnceCell<crate::ranges::Analysis>,
}

/// What the check of one file comes to.
pub struct Outcome {
    /// The file, as it was given.
    pub path: String,
    pub diags: Vec<Diag>,
    /// The module, when nothing is wrong with it.
    pub checked: Option<Rc<Checked>>,
    /// The module whose names and types pass, analysed (DESIGN 5), whatever the analysis and the
    /// examples find: what shows where a module can fail, and what a test runs a module the
    /// analysis finds wrong with.
    pub analysed: Option<Rc<Checked>>,
}

impl Outcome {
    pub fn has_errors(&self) -> bool {
        diag::has_errors(&self.diags)
    }

    /// Whether the module reads what this run cannot (E202): the command is run where a language is
    /// not joined, which is not the file's fault, and exits 2.
    pub fn unjoined(&self) -> bool {
        self.diags.iter().any(|d| d.code == "E202")
    }

    /// The exit code of `kaname check` for this file alone: 0, 1, or 2 for E202.
    pub fn exit(&self) -> u8 {
        if self.unjoined() {
            2
        } else if self.has_errors() {
            1
        } else {
            0
        }
    }

    /// What the line that says the file passes says after the file: `1 function, total` (DESIGN
    /// 11.1), the functions with their effects.
    pub fn summary(&self) -> Option<Text> {
        let c = self.checked.as_ref()?;
        Some(crate::effects::summary(c))
    }
}

/// A run of the check: the languages it joins, what each module came to, and the modules being read
/// now (E203).
pub struct Run {
    pub suite: Suite,
    kept: RefCell<BTreeMap<(PathBuf, String), Rc<Outcome>>>,
    reading: RefCell<Vec<PathBuf>>,
}

impl Run {
    pub fn new(suite: Suite) -> Run {
        Run { suite, kept: RefCell::default(), reading: RefCell::default() }
    }

    /// The check of the file at `path`, as it is given; Err when it cannot be read. It is read
    /// through ritsu-base's `fs`, the disk or the files a page in the browser holds.
    pub fn file(&self, path: &str) -> Result<Rc<Outcome>, String> {
        let key = (canonical(Path::new(path)), path.to_string());
        if let Some(o) = self.kept.borrow().get(&key) {
            return Ok(o.clone());
        }
        let src = ritsu_base::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let o = Rc::new(self.text(path, &src));
        self.kept.borrow_mut().insert(key, o.clone());
        Ok(o)
    }

    /// The check of `src`, the text of the file `path`.
    pub fn text(&self, path: &str, src: &str) -> Outcome {
        // a key written in the file is in the repository whether the file reads or not (W901)
        let keys = crate::security::keys(path, src);
        let parsed = crate::parse::parse(path, src);
        let Some(module) = parsed.module else {
            let mut diags = parsed.diags;
            diags.extend(keys);
            diag::sort(&mut diags);
            return Outcome { path: path.to_string(), diags, checked: None, analysed: None };
        };
        if let Some(d) = self.unjoined(&module, path, src) {
            let mut diags = vec![d];
            diags.extend(keys);
            diag::sort(&mut diags);
            return Outcome { path: path.to_string(), diags, checked: None, analysed: None };
        }
        let canon = canonical(Path::new(path));
        self.reading.borrow_mut().push(canon.clone());
        let (uses, mut diags) = self.uses(&module, path, src);
        self.reading.borrow_mut().pop();
        // what a module read reads cannot be read here: that alone is said, as for this module
        if diags.iter().any(|d| d.code == "E202") {
            diags.retain(|d| d.code == "E202");
            diags.extend(keys);
            diag::sort(&mut diags);
            return Outcome { path: path.to_string(), diags, checked: None, analysed: None };
        }
        let read_all = uses.iter().all(Option::is_some);
        let mut world = World::default();
        let mut by_name: BTreeMap<String, (usize, Target)> = BTreeMap::new();
        for (i, (u, t)) in module.uses.iter().zip(&uses).enumerate() {
            if let Some(t) = t {
                world.add(t);
                by_name.entry(u.name.clone()).or_insert((i, t.clone()));
            }
        }
        let mut dc = Dc { path, src, canon: &canon, module: &module, uses: &by_name, world: &world, diags: Vec::new() };
        dc.names();
        let decls = dc.declarations();
        let n = module.exprs;
        let (typed, more) = if read_all {
            types::check(dc, &decls)
        } else {
            // the names inside the functions would name what could not be read: that is said once, at
            // its `use` line
            (Typed { types: vec![crate::ty::Ty::Error; n], refs: vec![types::Ref::None; n], bound: BTreeMap::new() }, dc.diags)
        };
        diags.extend(more);
        let mut checked = if diag::has_errors(&diags) {
            None
        } else {
            Some(Rc::new(Checked { path: path.to_string(), canon, src: src.to_string(), module, uses: uses.into_iter().flatten().collect(), world, decls, typed, analysis: std::cell::OnceCell::new() }))
        };
        // the analysis of ranges, the effects and the decisions written in code, once the types hold
        // (DESIGN 4, 5, 7)
        let analysed = checked.clone();
        if let Some(c) = &checked {
            let (analysis, found) = crate::ranges::analyze(c, &self.suite);
            let _ = c.analysis.set(analysis);
            diags.extend(found);
            if diag::has_errors(&diags) {
                checked = None;
            }
        }
        // the examples run once everything else holds (E601)
        if let Some(c) = &checked {
            let failed = crate::examples::check(c, &self.suite);
            if !failed.is_empty() {
                diags.extend(failed);
                checked = None;
            }
        }
        diags.extend(keys);
        diag::sort(&mut diags);
        Outcome { path: path.to_string(), diags, checked, analysed }
    }

    /// E202 at the first line that reads a language this run does not join.
    fn unjoined(&self, m: &Module, path: &str, src: &str) -> Option<Diag> {
        let u = m.uses.iter().find(|u| !self.suite.reads(u.kind))?;
        let (what, tool) = match u.kind {
            UseKind::Rule => (tr!("規則", "rules"), "rulec"),
            UseKind::Dates => (tr!("日付のファイル", "dates files"), "koyomi"),
            _ => (tr!("帳簿", "books"), "chobo"),
        };
        let name = &u.name;
        let cmd = crate::run::with_ritsu();
        Some(
            Diag::error(
                "E202",
                path,
                u.line,
                1,
                tr!(
                    "この kaname は{}を読めません（{tool} が入っていません）。このモジュールは `{name}` を読みます",
                    "This kaname cannot read {} ({tool} is not in it), and the module reads `{name}`",
                    what.ja;
                    what.en
                ),
            )
            .source(src)
            .note(tr!(
                "kaname 単独のバイナリには、ほかの言語が入っていません。同じコマンドを、すべての言語をつないだ ritsu で、`{cmd}` のように走らせてください。",
                "The binary of kaname's own crate holds no other language; run the same command with every language joined, through ritsu: `{cmd}`."
            )),
        )
    }

    /// What each `use` line reads, in the module's order (None where it cannot be read, which is
    /// said: E201, E202, E203).
    fn uses(&self, m: &Module, path: &str, src: &str) -> (Vec<Option<Target>>, Vec<Diag>) {
        let mut out = Vec::new();
        let mut diags = Vec::new();
        for u in &m.uses {
            let target = beside(path, &u.path);
            let shown = target.display().to_string();
            let at = |code: &'static str, msg: Text| Diag::error(code, path, u.path_span.line, u.path_span.col, msg).source(src);
            let ext = u.kind.extension();
            let word = u.kind.word();
            if !shown.ends_with(&format!(".{ext}")) {
                diags.push(at("E201", tr!("`use {word}` が読むのは `.{ext}` のファイルです。`{shown}` はそうではありません", "`use {word}` reads a `.{ext}` file, which `{shown}` is not")));
                out.push(None);
                continue;
            }
            if !ritsu_base::fs::is_file(&target) {
                diags.push(
                    at("E201", tr!("`{shown}` というファイルはありません", "There is no file `{shown}`"))
                        .note(tr!("パスは、このモジュールのファイルのあるディレクトリから書きます。", "A path is written from the directory of the module's file.")),
                );
                out.push(None);
                continue;
            }
            let canon = canonical(&target);
            let refused = |tool: &str, said: &[ritsu_ports::Said]| -> Diag {
                let mut d = at("E201", tr!("`{shown}` は {tool} の検査を通りません", "`{shown}` does not pass {tool}'s check"));
                for n in said_notes(said) {
                    d = d.note(n);
                }
                d.note(tr!("そのファイルを、{tool} の検査が通るように直してください。", "Correct that file until {tool}'s check passes it."))
            };
            let t = match u.kind {
                UseKind::Rule => match self.suite.rules.as_ref().map(|r| r.facts(&target)) {
                    Some(Ok(f)) => Some(Target::Rule { file: target.clone(), canon, facts: Rc::new(f) }),
                    Some(Err(said)) => {
                        diags.push(refused("rulec", &said));
                        None
                    }
                    None => None,
                },
                UseKind::Dates => match self.suite.dates.as_ref().map(|d| d.facts(&target)) {
                    Some(Ok(f)) => Some(Target::Dates { file: target.clone(), canon, facts: Rc::new(f) }),
                    Some(Err(said)) => {
                        diags.push(refused("koyomi", &said));
                        None
                    }
                    None => None,
                },
                UseKind::Book => match self.suite.books.as_ref().map(|b| b.facts(&target)) {
                    Some(Ok(f)) => Some(Target::Book { file: target.clone(), canon, facts: Rc::new(f) }),
                    Some(Err(said)) => {
                        diags.push(refused("chobo", &said));
                        None
                    }
                    None => None,
                },
                UseKind::Module => self.module_use(u, &shown, &canon, path, src, &mut diags),
            };
            out.push(t);
        }
        (out, diags)
    }

    /// Another module, checked: E203 when reading it comes back to a module being read, E201 when it
    /// does not pass, E202 when it reads what this run cannot.
    fn module_use(&self, u: &crate::ast::Use, shown: &str, canon: &Path, path: &str, src: &str, diags: &mut Vec<Diag>) -> Option<Target> {
        let at = |code: &'static str, msg: Text| Diag::error(code, path, u.path_span.line, u.path_span.col, msg).source(src);
        let ring = {
            let reading = self.reading.borrow();
            reading.iter().position(|p| p == canon).map(|i| reading[i..].iter().map(|p| p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()).collect::<Vec<_>>())
        };
        if let Some(files) = ring {
            let mut names = files;
            names.push(names[0].clone());
            let shown_ring = names.join(" → ");
            diags.push(
                at(
                    "E203",
                    tr!(
                        "`{shown}` を読むと、このモジュールに戻ってきます（`use module` が輪になっています）",
                        "Reading `{shown}` comes back to this module (the `use module` lines go round in a ring)"
                    ),
                )
                .note(tr!("輪: {shown_ring}", "The ring: {shown_ring}"))
                .note(tr!(
                    "読む向きを一つにしてください。両方が使うものは、両方が読む別のモジュールに置けます。関数の相互の再帰は、一つのモジュールの中で書けます。",
                    "Make them read one way only: what both need can go in a third module both read. Functions that call each other are written in one module."
                )),
            );
            return None;
        }
        let o = match self.file(shown) {
            Ok(o) => o,
            Err(e) => {
                diags.push(at("E201", tr!("`{shown}` を読めません: {e}", "Cannot read `{shown}`: {e}")));
                return None;
            }
        };
        if let Some(inner) = o.diags.iter().find(|d| d.code == "E202") {
            let (code, place) = (inner.code, inner.place());
            let cmd = crate::run::with_ritsu();
            diags.push(
                at("E202", tr!("この kaname は、`{shown}` が読むものを読めません", "This kaname cannot read what `{shown}` reads"))
                    .note(tr!("{code}: {}（{place}）", "{code}: {} ({place})", inner.message.ja; capitalize(&inner.message.en)))
                    .note(tr!(
                        "kaname 単独のバイナリには、ほかの言語が入っていません。同じコマンドを、すべての言語をつないだ ritsu で、`{cmd}` のように走らせてください。",
                        "The binary of kaname's own crate holds no other language; run the same command with every language joined, through ritsu: `{cmd}`."
                    )),
            );
            return None;
        }
        if o.has_errors() {
            if o.diags.iter().any(|d| d.code == "E203") {
                diags.push(at("E203", tr!("`{shown}` は、モジュールの読み込みの輪の中にあります", "`{shown}` is in a ring of modules that read each other")).note(tr!(
                    "輪がどこを通るかは、`kaname check {shown}` で読めます。読む向きを一つにしてください。",
                    "`kaname check {shown}` says where the ring goes; make the modules read one way only."
                )));
                return None;
            }
            let mut d = at("E201", tr!("`{shown}` は kaname の検査を通りません", "`{shown}` does not pass kaname's check"));
            for x in o.diags.iter().filter(|x| x.is_error()).take(3) {
                let (code, place) = (x.code, x.place());
                d = d.note(tr!("{code}: {}（{place}）", "{code}: {} ({place})", x.message.ja; capitalize(&x.message.en)));
            }
            diags.push(d.note(tr!("`kaname check {shown}` で、そのファイルの誤りを全部読めます。", "`kaname check {shown}` says everything that is wrong with it.")));
            return None;
        }
        o.checked.as_ref().map(|c| Target::Module(c.clone()))
    }
}

/// The root of a run (ritsu's DESIGN 6.2, item 3): `--root`, else the nearest directory above the
/// first path given that holds `.git`, else the directory of that file.
pub fn root_of(root: Option<&Path>, first: &str) -> PathBuf {
    match root {
        Some(r) => ritsu_base::paths::absolute(r),
        None => ritsu_base::paths::find_root(Path::new(first)),
    }
}

/// The line that says a file passes: `ok late_payment.kn: 1 function`.
pub fn ok_line(o: &Outcome, lang: Lang) -> Option<String> {
    o.summary().map(|s| format!("ok {}: {}\n", o.path, s.get(lang)))
}

/// What `kaname check` prints for one file: each diagnostic, then the line that says it passes.
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

/// The `--format json` of one file: `kaname`, `file`, `ok`, `summary`, `functions`, `diagnostics`,
/// in that order. `functions` is each function with its effect (DESIGN 4.5), when the analysis ran.
pub fn to_json(o: &Outcome, root: &Path, lang: Lang) -> Json {
    let file = ritsu_base::paths::from_root(root, Path::new(&o.path)).unwrap_or_else(|| o.path.clone());
    Json::obj([
        ("kaname", Json::str(env!("CARGO_PKG_VERSION"))),
        ("file", Json::str(file)),
        ("ok", Json::from(!o.has_errors())),
        ("summary", Json::opt_str(o.summary().map(|t| t.get(lang).to_string()))),
        ("functions", o.analysed.as_ref().map(|c| crate::effects::to_json(c)).unwrap_or(Json::Null)),
        ("diagnostics", Json::arr(rooted(o, root).iter().map(|d| d.to_json(lang)))),
    ])
}

/// What `ritsu check` prints of one file (`f`, as the person gave it) that the check came to, or
/// could not read (ritsu's DESIGN 8.3): every finding, as `kaname check` prints it and as its
/// `--format json` prints it, then the line that says the file passes.
pub fn unit(root: &Path, f: &str, checked: Result<&Outcome, String>, lang: Lang) -> ritsu_ports::Checked {
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
            Unit { label: f.to_string(), parts, verdict }
        }
        Err(e) => {
            let msg = tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}");
            let head = if lang == Lang::Ja { "エラー" } else { "error" };
            Unit::unchecked(f, format!("{head}: {}\n", msg.get(lang)))
        }
    }
}

/// Shorthand for a test or a tool: the file at `path` with the languages `suite` joins.
pub fn check(path: &str, suite: Suite) -> Result<Rc<Outcome>, String> {
    Run::new(suite).file(path)
}
