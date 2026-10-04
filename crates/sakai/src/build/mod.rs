//! `sakai build` (DESIGN 7; PLAN C.6 to C.11): the settings of an import linter, written from a map
//! that passes check, or held to the map with `--check` (E502). Each of the four tools gets the
//! same table of groups and directions (`areas.rs`), in its own words, under the same header: the
//! map it was written from and the command that wrote it.

pub mod archunit;
pub mod areas;
pub mod depcruise;
pub mod go_arch_lint;
pub mod import_linter;

use crate::check::{self, Checked, Outcome};
use crate::diag::{self, Diag};
use ritsu_base::text::{Lang, Text, spaced};
use crate::model::Model;
use crate::paths;
use areas::{Areas, Language};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    ImportLinter,
    DependencyCruiser,
    ArchUnit,
    GoArchLint,
}

impl Target {
    pub const ALL: [Target; 4] = [Target::ImportLinter, Target::DependencyCruiser, Target::ArchUnit, Target::GoArchLint];

    pub fn word(self) -> &'static str {
        match self {
            Target::ImportLinter => "import-linter",
            Target::DependencyCruiser => "dependency-cruiser",
            Target::ArchUnit => "archunit",
            Target::GoArchLint => "go-arch-lint",
        }
    }

    pub fn from_word(w: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.word() == w)
    }

    pub fn language(self) -> Language {
        match self {
            Target::ImportLinter => Language::Python,
            Target::DependencyCruiser => Language::TypeScript,
            Target::ArchUnit => Language::Java,
            Target::GoArchLint => Language::Go,
        }
    }

    /// The file the settings are written to.
    pub fn file_name(self) -> &'static str {
        match self {
            Target::ImportLinter => ".importlinter",
            Target::DependencyCruiser => ".dependency-cruiser.cjs",
            Target::ArchUnit => "SakaiContextsTest.java",
            Target::GoArchLint => ".go-arch-lint.yml",
        }
    }

    fn comment(self) -> &'static str {
        match self {
            Target::ImportLinter | Target::GoArchLint => "#",
            Target::DependencyCruiser | Target::ArchUnit => "//",
        }
    }

    /// How many entries the settings have, in the tool's word for them.
    fn entries(self, n: usize) -> Text {
        match self {
            Target::ImportLinter => tr!("契約 {n} 件", "{n} contracts"),
            Target::DependencyCruiser => tr!("規則 {n} 件", "{n} rules"),
            Target::ArchUnit => tr!("規則 {n} 件", "{n} rules"),
            Target::GoArchLint => tr!("コンポーネント {n} 件", "{n} components"),
        }
    }
}

/// What `sakai build` came to.
pub struct Built {
    /// The map's check, with build's own diagnostics (E501, E502) after its own.
    pub outcome: Outcome,
    /// The settings file, and what was done with it, when nothing was wrong.
    pub done: Option<(PathBuf, Text)>,
}

/// The header every settings file starts with, in the tool's comments: the map, from the file's
/// directory, and the command that wrote it. It holds no digest of the `.ctx` files: one would
/// change with any edit of them, a comment's too, and `--check` would then send every settings
/// file to be written again with nothing in it changed.
fn header(m: &Model, target: Target, file: &Path, lang: Lang) -> String {
    let dir = file.parent().unwrap_or(Path::new("."));
    let map = paths::between(dir, &paths::on_disk(&m.root, &m.map.file));
    let cmd = match lang {
        Lang::En => format!("sakai build --target {}", target.word()),
        Lang::Ja => format!("sakai build --target {} --lang ja", target.word()),
    };
    let lines = match lang {
        Lang::En => [format!("Written by `{cmd}` from {map}."), "Edit the .ctx files and write it again; `sakai build --check` says whether it is up to date.".to_string()],
        Lang::Ja => [format!("`{cmd}` が {map} から書いた設定。"), "直すときは .ctx を直して書き直す。古くなっていないかは `sakai build --check` が言う。".to_string()],
    };
    let c = target.comment();
    lines.iter().map(|l| format!("{c} {l}\n")).collect()
}

/// The settings of `target` for a map that passed check: the text and how many entries it has.
pub fn render(c: &Checked, a: &Areas, target: Target, file: &Path, lang: Lang) -> (String, usize) {
    let (body, n) = match target {
        Target::ImportLinter => import_linter::render(c, a, lang),
        Target::DependencyCruiser => depcruise::render(c, a, lang),
        Target::ArchUnit => archunit::render(c, a, lang),
        Target::GoArchLint => go_arch_lint::render(c, a, lang),
    };
    (format!("{}\n{body}", header(&c.model, target, file, lang)), n)
}

/// Where the settings go by default: the language's code directory (ArchUnit: its `test`).
fn default_dir(c: &Checked, a: &Areas, target: Target) -> Result<String, Box<Diag>> {
    let m = &c.model;
    if target != Target::ArchUnit {
        return Ok(a.dir.clone());
    }
    let code = m.map.code.iter().find(|x| x.language == "java").expect("the areas of java have a code line");
    code.test.clone().ok_or_else(|| {
        let line = m.map.ast.code.iter().find(|x| x.language == "java").map(|x| x.pos).unwrap_or_default();
        Box::new(diag::at("E501", &m.map.file, line.line, line.col, tr!("`code java` に `test` の行がありません", "The `code java` line has no `test` line under it")).source(&m.map.src).note(tr!(
            "sakai は ArchUnit の規則を JUnit のテストとして書くので、テストの置き場所を `  test \"<パス>\"` で書いてください（`--out` で替えることもできます）。",
            "The rules of ArchUnit are a JUnit test, written where the tests are: write it with `  test \"<path>\"` under the line (or give `--out`)."
        )))
    })
}

/// `sakai build <map> --target <tool> [--out <dir>] [--check]`. `out` is a directory on the disk.
/// The map is checked first, with no other language joined.
pub fn run(root: &Path, map: &str, target: Target, out: Option<&Path>, check_only: bool, lang: Lang) -> Result<Built, Text> {
    run_with(root, map, target, &crate::suite::Suite::default(), out, check_only, lang)
}

/// [`run`], the map checked with the languages `suite` joins.
pub fn run_with(root: &Path, map: &str, target: Target, suite: &crate::suite::Suite, out: Option<&Path>, check_only: bool, lang: Lang) -> Result<Built, Text> {
    let mut o = check::check_map_with(root, map, suite)?;
    if o.has_errors() {
        return Ok(Built { outcome: o, done: None });
    }
    let c = o.checked.as_ref().expect("a map with no errors is checked through");
    let a = match areas::areas(c, target.language()) {
        Ok(a) => a,
        Err(ds) => {
            o.diags.extend(ds);
            return Ok(Built { outcome: o, done: None });
        }
    };
    if target == Target::ImportLinter
        && let Some(f) = a.areas.iter().flat_map(|x| x.files.iter()).find(|f| !f.contains('/'))
    {
        // grimp, which import-linter reads the code with, takes packages only as its roots.
        let m = &c.model;
        let line = m.map.ast.code.iter().find(|x| x.language == "python").map(|x| x.pos).unwrap_or_default();
        let sf = paths::shown(&paths::join(&a.dir, f).unwrap_or_default());
        o.diags.push(diag::at("E501", &m.map.file, line.line, line.col, tr!("{sf} は Python の置き場所の直下のモジュールで、import-linter が読めません", "The module {sf} is right in the place of the Python code, where import-linter cannot read it")).source(&m.map.src).note(tr!(
            "import-linter（grimp）はパッケージだけをルートとして読みます。モジュールをパッケージのディレクトリに入れるか、`code python` に、パッケージを持つディレクトリを書いてください。",
            "Only packages can be the roots import-linter (grimp) reads: put the module in a package's directory, or make `code python` the directory that holds the packages."
        )));
        return Ok(Built { outcome: o, done: None });
    }
    if target == Target::ArchUnit
        && let Some(f) = a.areas.iter().flat_map(|x| x.files.iter()).find(|f| !f.contains('/'))
    {
        // A class in the default package is in no package a rule could name.
        let m = &c.model;
        let line = m.map.ast.code.iter().find(|x| x.language == "java").map(|x| x.pos).unwrap_or_default();
        let sf = paths::shown(&paths::join(&a.dir, f).unwrap_or_default());
        o.diags.push(diag::at("E501", &m.map.file, line.line, line.col, tr!("{sf} はデフォルトパッケージのクラスで、ArchUnit の規則に書けません", "The class {sf} is in the default package, which no ArchUnit rule can name")).source(&m.map.src).note(tr!(
            "ArchUnit の規則は、まとまりをパッケージで表します。クラスに `package` を書き、そのディレクトリに置いてください。",
            "The rules of ArchUnit name a group by its packages: give the class a `package`, and put it in that directory."
        )));
        return Ok(Built { outcome: o, done: None });
    }
    if target == Target::GoArchLint && !ritsu_base::fs::is_file(paths::on_disk(root, &a.dir).join("go.mod")) {
        let m = &c.model;
        let line = m.map.ast.code.iter().find(|x| x.language == "go").map(|x| x.pos).unwrap_or_default();
        let d = paths::shown(&a.dir);
        o.diags.push(diag::at("E501", &m.map.file, line.line, line.col, tr!("{d} に go.mod がありません", "There is no go.mod in {d}")).source(&m.map.src).note(tr!(
            "go-arch-lint は、go.mod のあるディレクトリをモジュールのルートとして読みます。`code go` には、go.mod のあるディレクトリを書いてください。",
            "go-arch-lint reads the directory with go.mod as the module's root: write that directory in `code go`."
        )));
        return Ok(Built { outcome: o, done: None });
    }
    let dir: PathBuf = match out {
        Some(d) => paths::absolute(d),
        None => match default_dir(c, &a, target) {
            Ok(d) => paths::on_disk(root, &d),
            Err(d) => {
                o.diags.push(*d);
                return Ok(Built { outcome: o, done: None });
            }
        },
    };
    let file = dir.join(target.file_name());
    let (text, n) = render(c, &a, target, &file, lang);
    let entries = target.entries(n);
    if check_only {
        match ritsu_base::fs::read_to_string(&file) {
            Ok(old) if old == text => Ok(Built { outcome: o, done: Some((file, tr!("いまの地図から書くものと同じ（{}）", "up to date ({})", entries.ja; entries.en))) }),
            got => {
                let d = stale(target, root, &file, got.ok().as_deref(), &text, lang);
                o.diags.push(d);
                Ok(Built { outcome: o, done: None })
            }
        }
    } else {
        ritsu_base::fs::create_dir_all(&dir).map_err(|e| {
            let (d, e) = (dir.display().to_string(), e.to_string());
            tr!("{d} を作れません: {e}", "cannot create {d}: {e}")
        })?;
        ritsu_base::fs::write(&file, &text).map_err(|e| {
            let (f, e) = (file.display().to_string(), e.to_string());
            tr!("{f} に書けません: {e}", "cannot write {f}: {e}")
        })?;
        Ok(Built { outcome: o, done: Some((file, tr!("書きました（{}）", "written ({})", entries.ja; entries.en))) })
    }
}

/// A file on the disk as `Diag::file` holds a path: from the root when it is under it.
fn diag_path(root: &Path, file: &Path) -> String {
    paths::from_root(root, file).unwrap_or_else(|| file.display().to_string())
}

/// E502: the settings on the disk are not what the map writes now.
fn stale(target: Target, root: &Path, file: &Path, old: Option<&str>, new: &str, lang: Lang) -> Diag {
    let at = diag_path(root, file);
    let f = paths::shown(&at);
    let t = target.word();
    let Some(old) = old else {
        return diag::whole("E502", &at, tr!("{f} がありません", "The settings file {f} is not there")).note(tr!("`sakai build --target {t}` で書いてください。", "Write it with `sakai build --target {t}`."));
    };
    let (ol, nl): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let i = (0..ol.len().max(nl.len())).find(|&i| ol.get(i) != nl.get(i)).unwrap_or(0);
    let mut d = diag::at("E502", &at, i + 1, 1, tr!("{f} が、いまの地図から書く設定と違います", "The settings file {f} differs from what the map writes now")).source(old);
    match nl.get(i) {
        Some(l) => {
            let l = l.trim();
            d = d.note(tr!("いまの地図から書くと、この行は `{l}` です。", "From the map as it is now, the line is `{l}`."));
        }
        None => d = d.note(tr!("いまの地図から書くと、この行はありません。", "From the map as it is now, there is no such line.")),
    }
    // The header says which language the file was written in; another --lang writes other words.
    let written_ja = old.lines().next().is_some_and(|l| l.contains("--lang ja"));
    if written_ja != (lang == Lang::Ja) {
        let was = if written_ja { "ja" } else { "en" };
        d = d.note(tr!("このファイルは --lang {was} で書かれています。確かめるときも、同じ --lang を渡してください。", "The file was written with --lang {was}; check it with the same --lang."));
    }
    d.note(tr!("`sakai build --target {t}` で書き直してください。", "Write it again with `sakai build --target {t}`."))
}

/// What `build` prints after the diagnostics: the file, from where sakai runs, and what was done.
pub fn render_done(root: &Path, done: &(PathBuf, Text), lang: Lang) -> String {
    let f = paths::shown(&diag_path(root, &done.0));
    format!("{f}: {}\n", spaced(&done.1, lang))
}
