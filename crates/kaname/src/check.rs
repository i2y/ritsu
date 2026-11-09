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
    /// Every lambda of its functions, by its id, with the function it is written in: what a function
    /// value is called with (DESIGN 27.3), made the first time one is looked up.
    pub lambdas: std::cell::OnceCell<BTreeMap<usize, (usize, crate::ast::Expr)>>,
    /// The same module with its properties read as functions after its own (DESIGN 38.2): what a
    /// property's vectors run in and what is written for Lean. None for a module with no properties,
    /// and for that module itself.
    pub lowered: Option<Rc<Checked>>,
    /// Its properties, in the order written (empty in [`Checked::lowered`]).
    pub properties: Vec<CheckedProperty>,
}

/// A property of a module (DESIGN 38): as written, the function it is read as in
/// [`Checked::lowered`], and what the analysis of ranges found of that function.
#[derive(Clone, Debug)]
pub struct CheckedProperty {
    pub decl: crate::ast::Property,
    /// The function in [`Checked::lowered`].
    pub lowered: usize,
    pub facts: crate::ranges::FnFacts,
}

/// The module the stages after the check read (DESIGN 38.2): `full`, checked with its properties
/// read as functions after its `n` own, with them left out of its functions, its declarations and
/// its analysis, and kept beside.
fn without_properties(full: Rc<Checked>, n: usize) -> Rc<Checked> {
    if full.module.properties.is_empty() {
        return full;
    }
    let mut module = full.module.clone();
    module.fns.truncate(n);
    let mut decls = full.decls.clone();
    decls.fns.truncate(n);
    let analysis = std::cell::OnceCell::new();
    let mut properties = Vec::new();
    if let Some(a) = full.analysis.get() {
        for (k, p) in full.module.properties.iter().enumerate() {
            properties.push(CheckedProperty { decl: p.clone(), lowered: n + k, facts: a.fns.get(n + k).cloned().unwrap_or_default() });
        }
        let mut own = a.clone();
        own.fns.truncate(n);
        let _ = analysis.set(own);
    }
    Rc::new(Checked {
        path: full.path.clone(),
        canon: full.canon.clone(),
        src: full.src.clone(),
        module,
        uses: full.uses.clone(),
        world: full.world.clone(),
        decls,
        typed: full.typed.clone(),
        analysis,
        lambdas: std::cell::OnceCell::new(),
        lowered: Some(full),
        properties,
    })
}

impl Checked {
    /// The lambda of the expression `id`, and the function it is written in (DESIGN 27.3).
    pub fn lambda(&self, id: usize) -> Option<(usize, &crate::ast::Expr)> {
        let all = self.lambdas.get_or_init(|| {
            let mut out = BTreeMap::new();
            for (f, decl) in self.module.fns.iter().enumerate() {
                let mut es = Vec::new();
                decl.body.walk(&mut es);
                for e in es {
                    if let crate::ast::ExprKind::Lambda(..) = e.kind {
                        out.insert(e.id, (f, e.clone()));
                    }
                }
            }
            out
        });
        all.get(&id).map(|(f, e)| (*f, e))
    }
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
    /// What the vectors (and `--lean`) said of each property of [`Outcome::analysed`] (DESIGN 38.3):
    /// none when something else is wrong with the module, as the properties run last.
    pub properties: Vec<crate::properties::PropertyReport>,
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
            return Outcome { path: path.to_string(), diags, checked: None, analysed: None, properties: Vec::new() };
        };
        if let Some(d) = self.unjoined(&module, path, src) {
            let mut diags = vec![d];
            diags.extend(keys);
            diag::sort(&mut diags);
            return Outcome { path: path.to_string(), diags, checked: None, analysed: None, properties: Vec::new() };
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
            return Outcome { path: path.to_string(), diags, checked: None, analysed: None, properties: Vec::new() };
        }
        let read_all = uses.iter().all(Option::is_some);
        // while the module is checked, its properties are functions after its own (DESIGN 38.2): the
        // names, the types, the analysis and the effects read them as they read a function
        let own = module.fns.len();
        let mut module = module;
        let lowered: Vec<crate::ast::FnDecl> = module.properties.iter().map(|p| p.lowered.clone()).collect();
        module.fns.extend(lowered);
        let mut world = World::default();
        let mut by_name: BTreeMap<String, (usize, Target)> = BTreeMap::new();
        for (i, (u, t)) in module.uses.iter().zip(&uses).enumerate() {
            if let Some(t) = t {
                world.add(t);
                by_name.entry(u.name.clone()).or_insert((i, t.clone()));
            }
        }
        let mut dc = Dc { path, src, canon: &canon, module: &module, uses: &by_name, world: &world, diags: Vec::new(), scope: Vec::new() };
        dc.names();
        let decls = dc.declarations();
        let n = module.exprs;
        let (typed, more) = if read_all {
            types::check(dc, &decls)
        } else {
            // the names inside the functions would name what could not be read: that is said once, at
            // its `use` line
            (Typed { types: vec![crate::ty::Ty::Error; n], refs: vec![types::Ref::None; n], bound: BTreeMap::new(), inst: BTreeMap::new(), named: BTreeMap::new() }, dc.diags)
        };
        diags.extend(more);
        // `let … = try …` is read as the `match` it means, once the types pass (DESIGN 26.3)
        let (mut module, mut typed) = (module, typed);
        if !diag::has_errors(&diags) {
            let rets: Vec<crate::ty::Ty> = decls.fns.iter().map(|f| f.ret.ty.clone()).collect();
            crate::tried::desugar(&mut module, &mut typed, &rets);
            // a function written as a value is read as the lambda that calls it (DESIGN 27.1)
            crate::wrapped::desugar(&mut module, &mut typed);
        }
        let mut checked = if diag::has_errors(&diags) {
            None
        } else {
            Some(Rc::new(Checked {
                path: path.to_string(),
                canon,
                src: src.to_string(),
                module,
                uses: uses.into_iter().flatten().collect(),
                world,
                decls,
                typed,
                analysis: std::cell::OnceCell::new(),
                lambdas: std::cell::OnceCell::new(),
                lowered: None,
                properties: Vec::new(),
            }))
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
        // the module the stages after the check read: its properties out of its functions
        let analysed = analysed.map(|c| without_properties(c, own));
        let mut checked = if checked.is_some() { analysed.clone() } else { None };
        // the properties, on their vectors, last (DESIGN 38.3)
        let mut properties = Vec::new();
        if let Some(c) = &checked {
            properties = crate::properties::run(c, &self.suite);
            let found = crate::properties::diags(c, &properties);
            if diag::has_errors(&found) {
                checked = None;
            }
            diags.extend(found);
        }
        diags.extend(keys);
        diag::sort(&mut diags);
        Outcome { path: path.to_string(), diags, checked, analysed, properties }
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
            let at = |code: &'static str, msg: Text| Diag::error(code, path, u.path_span.line, u.path_span.col, msg).source(src);
            let target = match beside(path, &u.path) {
                Ok(t) => t,
                Err(r) => {
                    diags.push(at("E201", r.text()));
                    out.push(None);
                    continue;
                }
            };
            let shown = target.display().to_string();
            let ext = u.kind.extension();
            let word = u.kind.word();
            if !shown.ends_with(&format!(".{ext}")) {
                diags.push(at("E201", tr!("`use {word}` が読むのは `.{ext}` のファイルです。`{shown}` はそうではありません", "`use {word}` reads a `.{ext}` file, which `{shown}` is not")));
                out.push(None);
                continue;
            }
            if !ritsu_base::fs::is_file(&target) {
                let d = match ritsu_base::modules::missing(&target) {
                    Some(t) => at("E201", t),
                    None => at("E201", tr!("`{shown}` というファイルはありません", "There is no file `{shown}`"))
                        .note(tr!("パスは、このモジュールのファイルのあるディレクトリから書きます。", "A path is written from the directory of the module's file.")),
                };
                let outside = ritsu_base::modules::outside_note_for(Path::new(path).parent().unwrap_or(Path::new("")), &u.path);
                diags.push(match outside {
                    Some(n) => d.note(n),
                    None => d,
                });
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
                // a rule with no outputs (an `enums` file) is read for its enums: a call of it, and its
                // outputs, stop where they are written (E201, DESIGN 2.7)
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

/// The codes the properties are said with (DESIGN 38.3), which `--lean` says again of a module.
const OF_PROPERTIES: [&str; 5] = ["E603", "E604", "W603", "W604", "W605"];

/// `--lean` (DESIGN 38.6) on the outcomes of the files given: each module with a proof beside it sent
/// to Lean once, in one run, and its properties said again with what Lean said of them. A module
/// whose proof Lean does not take fails, as one with a counterexample does. Err when Lean is needed
/// and cannot be run (exit 2).
pub fn with_lean(outcomes: &[Rc<Outcome>], opts: &crate::lean::LeanOptions) -> Result<Vec<Rc<Outcome>>, crate::lean::LeanError> {
    let mut sent: Vec<(usize, Rc<Checked>)> = Vec::new();
    for (i, o) in outcomes.iter().enumerate() {
        let Some(c) = &o.analysed else { continue };
        if o.properties.is_empty() || !ritsu_base::fs::is_file(crate::properties::proof_path(c)) || sent.iter().any(|(_, s)| s.canon == c.canon) {
            continue;
        }
        sent.push((i, c.clone()));
    }
    if sent.is_empty() {
        return Ok(outcomes.to_vec());
    }
    let modules: Vec<Rc<Checked>> = sent.iter().map(|(_, c)| c.clone()).collect();
    let verdicts = crate::lean::check(&modules, opts)?;
    let mut out = outcomes.to_vec();
    for (m, c) in modules.iter().enumerate() {
        let mine: Vec<(usize, crate::lean::Verdict)> = verdicts.iter().filter(|((n, _), _)| *n == m).map(|((_, k), v)| (*k, v.clone())).collect();
        // the same module given twice is answered alike
        for (i, o) in outcomes.iter().enumerate().filter(|(_, o)| o.analysed.as_ref().is_some_and(|a| a.canon == c.canon)) {
            let properties = crate::properties::with_verdicts(c, &o.properties, &mine);
            let mut diags: Vec<Diag> = o.diags.iter().filter(|d| !(OF_PROPERTIES.contains(&d.code) && d.file == c.path)).cloned().collect();
            let said = crate::properties::diags(c, &properties);
            let failed = diag::has_errors(&said);
            diags.extend(said);
            diag::sort(&mut diags);
            let checked = if failed { None } else { o.checked.clone() };
            out[i] = Rc::new(Outcome { path: o.path.clone(), diags, checked, analysed: o.analysed.clone(), properties });
        }
    }
    Ok(out)
}

/// The root of a run (ritsu's DESIGN 6.2, item 3; 17.6): `--root`, else the nearest directory above
/// the first path given that holds `.git` or `ritsu.mod`, else the directory of that file.
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

/// The `--format json` of one file: `kaname`, `file`, `ok`, `summary`, `functions`, `properties`,
/// `diagnostics`, in that order. `functions` is each function with its effect (DESIGN 4.5), when the
/// analysis ran; `properties` each property with its answer (DESIGN 38.3), when they ran.
pub fn to_json(o: &Outcome, root: &Path, lang: Lang) -> Json {
    let file = ritsu_base::paths::from_root(root, Path::new(&o.path)).unwrap_or_else(|| o.path.clone());
    Json::obj([
        ("kaname", Json::str(env!("CARGO_PKG_VERSION"))),
        ("file", Json::str(file)),
        ("ok", Json::from(!o.has_errors())),
        ("summary", Json::opt_str(o.summary().map(|t| t.get(lang).to_string()))),
        ("functions", o.analysed.as_ref().map(|c| crate::effects::to_json(c)).unwrap_or(Json::Null)),
        ("properties", o.analysed.as_ref().map(|c| crate::properties::to_json(c, &o.properties)).unwrap_or(Json::Null)),
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
            Unit { label: f.to_string(), parts, verdict, json: to_json(o, root, lang) }
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
