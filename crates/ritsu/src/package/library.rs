//! `ritsu gen --library` (DESIGN 17.16): the package of each language as a library its registry
//! takes (npm, PyPI, a Go module), named by the module's `library` lines and versioned by its
//! `version`, with what it depends on as ranges. A library does not hold the parts of a dependency
//! that is a library itself: it reads them from that library, by the library's name, so that two
//! libraries that read one dependency read one copy of it, and its types are one type.
//!
//! Which library holds a module's parts is decided from the `library` lines of the tree alone, so
//! that the library and the code that uses it decide it alike ([`holder`]): walking the requires
//! from the module at hand to the module, breadth first in the order of each `ritsu.mod`'s requires
//! (as the tree's prefixes are made), the first module on the way that has a `library` line of the
//! language is its holder, or the holder that module finds the same way from itself is. A module
//! no library on the way holds is the package's own to hold: under `--embed <name>`, at
//! `deps/<name>/` as the app form holds it; else the run stops (exit 2), as it does when `--embed`
//! names a dependency a library holds. Each module directory a library holds is put in the table
//! the generators import by ([`ritsu_emit::package::Place::with_libraries`]), with the path the
//! language imports it by.
//!
//! The type of each unit the package's code names is the one of the library the code reads it with
//! ([`Units::shared`]), read from that library's `units` module and handed on under the same name,
//! so that an amount one library gives is an amount the other takes.

use super::{Refusal, Target, kaname_modules, read, refusal};
use ritsu_base::modules::Tree;
use ritsu_base::modules::names::Target as Line;
use ritsu_base::modules::semver::Version;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_emit::package::Place;
use ritsu_project::{Joined, Project};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

/// The TypeScript the package's `prepare` builds it with into `dist/`: the version the tests build
/// it with (dandori's tools/temporal/package.json).
pub const TYPESCRIPT: &str = "7.0.2";

/// The package of one language written as a library.
#[derive(Clone, Debug)]
pub struct Library {
    /// Its name in its registry: the npm package's, the Python distribution's, the Go module's path
    /// (with `/v<major>` from version 2 on).
    pub name: String,
    /// The name its code is imported by: the npm package's, the Python package's (`acme_pricing`),
    /// the Go module's path.
    pub import: String,
    /// The Go package of its root directory, which doc.go declares.
    pub go_package: String,
    /// Its version, as the language writes one (PEP 440 for Python).
    pub version: String,
    /// The SPDX expression of the module's `license` line.
    pub license: Option<String>,
    /// The module's `description` line, which the registries show beside the name.
    pub description: Option<String>,
    /// The module's `source` line: where its code is, which npm's provenance checks the package's
    /// `repository` against.
    pub source: Option<String>,
    /// Each module's directory the package reads from a library, with the path it imports that
    /// directory by ([`Place::with_libraries`]).
    pub table: Arc<Vec<(String, String)>>,
    /// The libraries its code may import, longest import path first.
    pub reads: Vec<Read>,
    /// Whose type each unit is.
    pub units: Units,
}

/// A library the package's code may import.
#[derive(Clone, Debug)]
pub struct Read {
    /// The path its code is imported by (as [`Library::import`]).
    pub import: String,
    /// Its name in its registry (as [`Library::name`]).
    pub name: String,
    /// The version the tree chooses: the lower end of the range the package asks for.
    pub version: Version,
}

impl Library {
    /// The library a module the package's code imports is in, by the module's path in the language
    /// (a TypeScript specifier, a Python module, a Go import path), whose parts `sep` divides.
    pub fn read_of(&self, path: &str, sep: char) -> Option<&Read> {
        self.reads.iter().find(|r| path == r.import || (path.starts_with(&r.import) && path[r.import.len()..].starts_with(sep)))
    }
}

/// The libraries the package reads its units' types from, and the units each holds.
#[derive(Clone, Debug, Default)]
pub struct Units {
    /// The libraries the package reads (each by its node in the tree).
    reads: Vec<usize>,
    /// Each library the package reads, or that one of those reads in turn: the types of the units
    /// its `units` module holds (by their names), and the libraries it reads.
    libs: BTreeMap<usize, (BTreeSet<String>, Vec<usize>)>,
    /// Each such library's `units` module, by the path the language imports it by.
    paths: BTreeMap<usize, String>,
    /// And its name, for what is said.
    names: BTreeMap<usize, String>,
}

impl Units {
    /// The library whose type of the unit named `brand` the package (`at` None) or the library `at`
    /// names: None for its own. Err with two libraries that do not read each other when both hold
    /// a type of it.
    fn owner(&self, at: Option<usize>, brand: &str, depth: usize) -> Result<Option<usize>, (usize, usize)> {
        let reads = match at {
            None => &self.reads,
            Some(k) => match self.libs.get(&k) {
                Some((_, r)) => r,
                None => return Ok(None),
            },
        };
        let mut found: Option<usize> = None;
        for d in reads {
            let Some((brands, _)) = self.libs.get(d) else { continue };
            if !brands.contains(brand) || depth > self.libs.len() {
                continue;
            }
            let o = self.owner(Some(*d), brand, depth + 1)?.unwrap_or(*d);
            match found {
                Some(f) if f != o => return Err((f, o)),
                _ => found = Some(o),
            }
        }
        Ok(found)
    }

    /// The units whose types the package reads from a library, of the types its code names (`brands`:
    /// each type's name, with its unit as the table spells it first): each type's name, with the
    /// path of that library's `units` module (for rulec's `units_module_in`). Exit 2 when two
    /// libraries that do not read each other each hold a type of one of them: a package has one
    /// type of each unit.
    pub fn shared(&self, brands: &[(String, String)], lang: Lang) -> Result<BTreeMap<String, String>, Refusal> {
        let mut out = BTreeMap::new();
        for (brand, shown) in brands {
            match self.owner(None, brand, 0) {
                Ok(Some(o)) => {
                    out.insert(brand.clone(), self.paths[&o].clone());
                }
                Ok(None) => {}
                Err((a, b)) => {
                    let (a, b) = (&self.names[&a], &self.names[&b]);
                    return Err(refusal(
                        2,
                        tr!(
                            "単位 {shown} の型を、互いに依存しない二つのライブラリ `{a}` と `{b}` がそれぞれ持っています。一つのパッケージは、一つの単位に一つの型しか持てません。片方のライブラリをもう片方に依存させるか、アプリの形（--library なし）で生成してください",
                            "the unit {shown} has a type in each of two libraries that do not depend on each other, `{a}` and `{b}`, and a package has one type of each unit: make one of the libraries depend on the other, or generate the app form (without --library)"
                        ),
                        lang,
                    ));
                }
            }
        }
        Ok(out)
    }
}

fn line(t: Target) -> Line {
    match t {
        Target::TypeScript => Line::Typescript,
        Target::Python => Line::Python,
        Target::Go => Line::Go,
    }
}

/// The name the module of node `i` gives its library of `t`, by its `library` line.
fn named(tree: &Tree, i: usize, t: Target) -> Option<&str> {
    tree.nodes[i].manifest.libraries.iter().find(|l| l.target == line(t)).map(|l| l.name.as_str())
}

/// The languages the module at hand has a `library` line of, in `--target`'s order.
fn lines_of(tree: &Tree) -> Vec<Target> {
    Target::ALL.into_iter().filter(|t| named(tree, 0, *t).is_some()).collect()
}

/// What `--library` runs on: the module at hand's tree, and the languages it writes (`--target`'s,
/// or every language the module has a `library` line of). Exit 2 when the root has no `ritsu.mod`,
/// no `version`, or no line of a language asked for.
pub fn targets(project: &Project, asked: Option<&str>, lang: Lang) -> Result<Vec<Target>, Refusal> {
    let Some(Ok(tree)) = project.tree.as_deref() else {
        return Err(refusal(
            2,
            tr!(
                "`--library` は、ritsu.mod のあるモジュールでだけ使えます。`ritsu mod init` でモジュールにしてください",
                "`--library` makes a library of a module, and there is no ritsu.mod at the root: make it a module with `ritsu mod init`"
            ),
            lang,
        ));
    };
    if tree.main().manifest.version.is_none() {
        return Err(refusal(
            2,
            tr!("`--library` には、ritsu.mod の `version` の行が要ります。それがパッケージの版になります", "`--library` needs the `version` line of ritsu.mod: it is the version of the packages"),
            lang,
        ));
    }
    let lines = lines_of(tree);
    match asked {
        Some(t) => {
            let Some(t) = Target::parse(t) else { return Ok(Vec::new()) };
            if !lines.contains(&t) {
                let w = t.key();
                return Err(refusal(
                    2,
                    tr!(
                        "ritsu.mod に `library {w}` の行がありません。パッケージの名前にする `library {w} \"<名前>\"` を書いてください",
                        "ritsu.mod has no `library {w}` line: write `library {w} \"<name>\"`, the name of the package"
                    ),
                    lang,
                ));
            }
            Ok(vec![t])
        }
        None if lines.is_empty() => Err(refusal(
            2,
            tr!(
                "ritsu.mod に `library` の行がありません。書く言語ごとに、パッケージの名前にする `library typescript|python|go \"<名前>\"` を書いてください",
                "ritsu.mod has no `library` line: write `library typescript|python|go \"<name>\"` for each language to write, the name of its package"
            ),
            lang,
        )),
        None => Ok(lines),
    }
}

/// The module at hand's files `--library` makes the packages of: every file of the module but what
/// its `exclude` lines leave out, as the module is sent (DESIGN 17.12).
pub fn whole_module(mut project: Project) -> Project {
    let except: Vec<String> = match project.tree.as_deref() {
        Some(Ok(tree)) => tree.main().manifest.excludes.iter().filter_map(|e| ritsu_base::paths::join("", &e.value).ok()).collect(),
        _ => return project,
    };
    project.files.retain(|f| !except.iter().any(|e| ritsu_base::paths::contains(e, &f.rel)));
    project
}

/// The nodes after `from` on the walk of its requires to `to`, breadth first in the order of each
/// `ritsu.mod`'s requires (as the tree's prefixes are made from the module at hand), each with the
/// name of the require it is reached by. None when the walk does not reach `to`.
fn walk(tree: &Tree, from: usize, to: usize) -> Option<Vec<(String, usize)>> {
    let n = tree.nodes.len();
    let mut came: Vec<Option<(usize, String)>> = vec![None; n];
    let mut seen = vec![false; n];
    seen[from] = true;
    let mut queue = VecDeque::from([from]);
    while let Some(i) = queue.pop_front() {
        for (name, j) in &tree.nodes[i].deps {
            if !seen[*j] {
                seen[*j] = true;
                came[*j] = Some((i, name.clone()));
                queue.push_back(*j);
            }
        }
    }
    if !seen[to] {
        return None;
    }
    let mut out = Vec::new();
    let mut at = to;
    while at != from {
        let (up, name) = came[at].clone()?;
        out.push((name, at));
        at = up;
    }
    out.reverse();
    Some(out)
}

/// The library of `t` that holds the parts of node `x` for the library (or package) of node `k`:
/// the first module with a `library` line of `t` on `k`'s walk to `x`, or the one that module's walk
/// finds in turn; `k` itself when no module on the way has one.
fn holder(tree: &Tree, t: Target, k: usize, x: usize, depth: usize) -> usize {
    let Some(path) = walk(tree, k, x) else { return k };
    match path.iter().find(|(_, i)| named(tree, *i, t).is_some()) {
        Some((_, l)) if *l != k && depth <= tree.nodes.len() => holder(tree, t, *l, x, depth + 1),
        _ => k,
    }
}

/// The directory of node `x`'s parts in the package of node `h`, which holds them: `deps/<name>/`
/// for each require on `h`'s walk to `x`, as `h`'s own `ritsu gen` puts them.
fn inner(tree: &Tree, h: usize, x: usize) -> String {
    walk(tree, h, x).unwrap_or_default().iter().map(|(name, _)| format!("deps/{name}/")).collect()
}

/// The name Python imports a distribution by: its name in lowercase, with each run of `-`, `_` and
/// `.` one `_`. None when that is not a name Python imports (it begins with a digit) or is a word
/// Python keeps.
pub fn python_import(dist: &str) -> Option<String> {
    let mut out = String::new();
    let mut run = false;
    for c in dist.chars() {
        if matches!(c, '-' | '_' | '.') {
            if !run {
                out.push('_');
            }
            run = true;
        } else {
            out.push(c.to_ascii_lowercase());
            run = false;
        }
    }
    let good = out.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && out.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    (good && !ritsu_emit::words::python::KEYWORDS.contains(&out.as_str())).then_some(out)
}

/// A Go module's path for version `v` of a library: its `library` line's, with `/v<major>` after it
/// from version 2 on, as Go's modules name a major version.
fn go_module(line: &str, v: &Version) -> String {
    match v.major.parse::<u64>() {
        Ok(m) if m >= 2 => format!("{line}/v{m}"),
        _ => line.to_string(),
    }
}

/// The Go package of a library's root directory: the last part of its path (before `/vN`), as a Go
/// identifier, with `p` before it when it does not begin with a letter or is a word Go keeps.
fn go_package(line: &str) -> String {
    let last = line.rsplit('/').next().unwrap_or(line);
    let s: String = last.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    if s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && !ritsu_emit::words::go::KEYWORDS.contains(&s.as_str()) { s } else { format!("p{s}") }
}

/// A version as Python's packages write it (PEP 440): `1.2.3`, and a pre-release of SemVer's
/// `alpha`, `beta` or `rc` with its number (`1.2.3-rc.1`, `1.2.3-rc1`) as `1.2.3rc1`. None for any
/// other pre-release, which PEP 440 has no form of.
pub fn pep440(v: &Version) -> Option<String> {
    let base = format!("{}.{}.{}", v.major, v.minor, v.patch);
    if v.pre.is_empty() {
        return Some(base);
    }
    let kind = |w: &str| match w {
        "alpha" | "a" => Some("a"),
        "beta" | "b" => Some("b"),
        "rc" => Some("rc"),
        _ => None,
    };
    let number = |w: &str| if w.is_empty() { Some(0) } else { w.parse::<u64>().ok().filter(|_| w.bytes().all(|b| b.is_ascii_digit())) };
    let (k, n) = match v.pre.as_slice() {
        [w] => {
            let at = w.find(|c: char| c.is_ascii_digit()).unwrap_or(w.len());
            (kind(&w[..at])?, number(&w[at..])?)
        }
        [w, n] => (kind(w)?, number(n).filter(|_| !n.is_empty())?),
        _ => return None,
    };
    Some(format!("{base}{k}{n}"))
}

/// The range npm writes for a version the package asks for at least (DESIGN 17.16): `^x.y.z`, which
/// takes the versions up to the next major one (for `0.y.z`, up to `0.(y+1).0`); for `0.0.z`, where
/// `^` takes no other version, `>=0.0.z <0.1.0`.
pub fn npm_range(v: &Version) -> String {
    if v.major == "0" && v.minor == "0" { format!(">={v} <0.1.0") } else { format!("^{v}") }
}

/// The range Python writes for it: `>=x.y.z,<(x+1).0.0`, and `<0.(y+1).0` for `0.y.z`. None for a
/// pre-release PEP 440 has no form of.
pub fn python_range(v: &Version) -> Option<String> {
    let low = pep440(v)?;
    let high = if v.major == "0" { format!("0.{}.0", v.minor.parse::<u64>().ok()? + 1) } else { format!("{}.0.0", v.major.parse::<u64>().ok()? + 1) };
    Some(format!(">={low},<{high}"))
}

/// The version of a constant the generators are tested with (`1.24.0`).
pub fn version_of(v: &str) -> Version {
    Version::parse(v).unwrap_or_else(|_| unreachable!("a version ritsu writes reads"))
}

/// The node of the module a file of the project is in, by its path from the root.
fn node_of(project: &Project, tree: &Tree, rel: &str) -> Option<usize> {
    match project.dependency_of(rel) {
        Some(prefix) => tree.by_prefix(&prefix),
        None => Some(0),
    }
}

/// How a module of the tree is said: `tax` (`example.com/acme/tax`), by the name the module at hand
/// walks to it with.
fn said(tree: &Tree, i: usize) -> Text {
    let n = &tree.nodes[i];
    let short = n.prefix.rsplit('/').next().unwrap_or(&n.prefix).trim_start_matches('@');
    let name = &n.name;
    tr!("`{short}`（{name}）", "`{short}` ({name})")
}

/// The languages' libraries of the module at hand, for `targets` (DESIGN 17.16): Err (exit 2) when a
/// module the package reaches is not held by a library and `--embed` does not name it, when
/// `--embed` names a dependency it cannot hold, when a contribution the package holds adds to a rule
/// of a library, or when a library's name or version cannot be written in its language. `project`
/// is the project the package is made of, `whole` every file of the tree; the units of each library
/// the package reads are found from the files that library is made of, as its own `ritsu gen
/// --library` makes it.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    project: &Project,
    whole: &Project,
    joined: &Joined,
    verdicts: &Rc<ritsu_cross::ProgramVerdicts>,
    contributions: &[String],
    targets: &[Target],
    embed: &[String],
    lang: Lang,
) -> Result<Vec<(Target, Library)>, Refusal> {
    let Some(Ok(tree)) = project.tree.as_deref() else { return Ok(Vec::new()) };
    let n = tree.nodes.len();
    let root = tree.main();
    let version = root.manifest.version.as_ref().map(|v| v.value.clone()).unwrap_or_else(|| Version::new(0, 0, 0));
    // the modules the package's files are in
    let reached: BTreeSet<usize> = project.files.iter().filter_map(|f| node_of(project, tree, &f.rel)).collect();
    // `--embed`: each name a dependency of the tree is reached by, and none that a library holds
    let short = |i: usize| tree.nodes[i].prefix.rsplit('/').next().unwrap_or("").trim_start_matches('@').to_string();
    let mut embedded: BTreeSet<usize> = BTreeSet::new();
    for e in embed {
        let matched: Vec<usize> = (1..n).filter(|i| short(*i) == *e).collect();
        if matched.is_empty() {
            return Err(refusal(2, tr!("`--embed {e}`：モジュールの木に、`{e}` という名前の依存がありません", "`--embed {e}`: no dependency of the tree of modules is named `{e}`"), lang));
        }
        for i in matched {
            for t in targets {
                let w = t.key();
                if named(tree, i, *t).is_some() {
                    let m = said(tree, i);
                    let (ja, en) = (&m.ja, &m.en);
                    return Err(refusal(
                        2,
                        Text::new(
                            format!("`--embed {e}`：依存 {ja}は `library {w}` の行を持つので、このパッケージはその部分をライブラリから読みます。`--embed` は、行の無い依存にだけ付けられます"),
                            format!("`--embed {e}`: the dependency {en} has a `library {w}` line, so it is read from that library; `--embed` is for a dependency with no line"),
                        ),
                        lang,
                    ));
                }
                let h = holder(tree, *t, 0, i, 0);
                if h != 0 {
                    let (m, by) = (said(tree, i), named(tree, h, *t).unwrap_or_default());
                    let (ja, en) = (&m.ja, &m.en);
                    return Err(refusal(
                        2,
                        Text::new(
                            format!("`--embed {e}`：依存 {ja}の部分は、それを中に入れているライブラリ `{by}` から読みます。`--embed` は、どのライブラリも中に入れていない依存にだけ付けられます"),
                            format!("`--embed {e}`: the parts of the dependency {en} are read from the library `{by}`, which holds them; `--embed` is for a dependency no library holds"),
                        ),
                        lang,
                    ));
                }
            }
            embedded.insert(i);
        }
    }
    let license = root.manifest.license.as_ref().map(|l| l.value.clone());
    let source = root.manifest.source.as_ref().map(|s| s.value.clone());
    let description = root.manifest.description.as_ref().map(|d| d.value.clone());
    let mut out = Vec::new();
    for t in targets {
        let t = *t;
        let w = t.key();
        // who holds each module's parts
        let held_by: Vec<usize> = (0..n).map(|x| if x == 0 { 0 } else { holder(tree, t, 0, x, 0) }).collect();
        let missing: Vec<usize> = reached.iter().copied().filter(|x| *x != 0 && held_by[*x] == 0 && !embedded.contains(x)).collect();
        if !missing.is_empty() {
            let names: Vec<Text> = missing.iter().map(|i| said(tree, *i)).collect();
            let ja = names.iter().map(|t| t.ja.clone()).collect::<Vec<_>>().join("、");
            let en = names.iter().map(|t| t.en.clone()).collect::<Vec<_>>().join(", ");
            let flags = missing.iter().map(|i| format!("--embed {}", short(*i))).collect::<Vec<_>>().join(" ");
            let them = if missing.len() == 1 { "the dependency" } else { "the dependencies" };
            return Err(refusal(
                2,
                Text::new(
                    format!(
                        "依存 {ja}の ritsu.mod に、`library {w}` の行がありません。ライブラリの形のパッケージは、依存の部分を依存のライブラリから読みます。その ritsu.mod に `library {w} \"<名前>\"` を書くか、`{flags}` を付けて、このパッケージの deps/ に入れてください"
                    ),
                    format!(
                        "the ritsu.mod of {them} {en} has no `library {w}` line, and a library reads its dependencies' parts from their libraries: write `library {w} \"<name>\"` there, or give `{flags}` to hold the parts in this package's deps/"
                    ),
                ),
                lang,
            ));
        }
        // a contribution the package holds, to a rule a library holds: the library's code does not
        // take it
        for c in contributions.iter().filter(|c| rulec::link::is_contribution_file(c)) {
            let Some(ci) = ritsu_base::paths::from_root(&project.root, Path::new(c)).and_then(|r| node_of(whole, tree, &r)) else { continue };
            if held_by[ci] != 0 {
                continue;
            }
            let Some((_, rule)) = rulec::link::target_of(c) else { continue };
            let Some(ri) = ritsu_base::paths::from_root(&project.root, Path::new(&rule)).and_then(|r| node_of(whole, tree, &r)) else { continue };
            if held_by[ri] != 0 {
                let by = named(tree, held_by[ri], t).unwrap_or_default();
                return Err(refusal(
                    2,
                    tr!(
                        "`{c}` は、ライブラリ `{by}` が持つ規則 `{rule}` に寄与しています。ライブラリのコードは、それを使う側の寄与を受けられません。寄与を外すか、アプリの形（--library なし）で生成してください",
                        "`{c}` contributes to `{rule}`, a rule the library `{by}` holds, and a library's code takes no contribution from the code that uses it: take the contribution out, or generate the app form (without --library)"
                    ),
                    lang,
                ));
            }
        }
        // each library's name and version in the language
        let mut import_of: BTreeMap<usize, String> = BTreeMap::new();
        let mut reads = Vec::new();
        for i in 0..n {
            let Some(line_name) = named(tree, i, t) else { continue };
            let Some(v) = tree.nodes[i].version().cloned() else {
                let m = said(tree, i);
                let (ja, en) = (&m.ja, &m.en);
                return Err(refusal(
                    2,
                    Text::new(
                        format!("依存 {ja}は `library {w}` の行を持ちますが、版がありません。このパッケージが求める範囲の下の端にする版を、その ritsu.mod の `version` の行に書いてください"),
                        format!("the dependency {en} has a `library {w}` line and no version: write the `version` line of its ritsu.mod, the lower end of the range this package asks for"),
                    ),
                    lang,
                ));
            };
            let (name, import) = match t {
                Target::TypeScript => (line_name.to_string(), line_name.to_string()),
                Target::Python => {
                    let Some(import) = python_import(line_name) else {
                        return Err(refusal(
                            2,
                            tr!(
                                "`library python \"{line_name}\"` から作る import の名前は、Python で import できる名前になりません（数字で始まるか、Python の予約語です）",
                                "`library python \"{line_name}\"` gives an import name Python cannot import (it begins with a digit, or is a word Python keeps)"
                            ),
                            lang,
                        ));
                    };
                    // the package's own version is said below
                    if i != 0 && pep440(&v).is_none() {
                        let m = said(tree, i);
                        let (ja, en) = (&m.ja, &m.en);
                        return Err(refusal(
                            2,
                            Text::new(
                                format!(
                                    "依存 {ja}の版 {v} のプレリリースは、Python の版の形（PEP 440）にできません。Python のパッケージの版にできるのは、alpha、beta、rc と番号のプレリリースだけです"
                                ),
                                format!("the pre-release of {v}, the version of the dependency {en}, has no form among Python's versions (PEP 440): only alpha, beta and rc with a number have one"),
                            ),
                            lang,
                        ));
                    }
                    (line_name.to_string(), import)
                }
                Target::Go => {
                    let m = go_module(line_name, &v);
                    (m.clone(), m)
                }
            };
            import_of.insert(i, import.clone());
            if i != 0 {
                reads.push(Read { import, name, version: v });
            }
        }
        reads.sort_by(|a, b| b.import.len().cmp(&a.import.len()).then(a.import.cmp(&b.import)));
        // where the package reads each module a library holds
        let sep = if t == Target::Python { "." } else { "/" };
        let root_of = |h: usize, x: usize| -> String {
            let dir = inner(tree, h, x);
            let base = import_of.get(&h).cloned().unwrap_or_default();
            if dir.is_empty() { base } else { format!("{base}{sep}{}", dir.trim_end_matches('/').replace('/', sep)) }
        };
        let mut table: Vec<(String, String)> = (1..n).filter(|x| held_by[*x] != 0).map(|x| (ritsu_emit::package::deps_dir(&tree.nodes[x].prefix), root_of(held_by[x], x))).collect();
        table.sort();
        // the units of the libraries the package reads, and of those they read
        let mut units = Units { reads: reached.iter().map(|x| held_by[*x]).filter(|h| *h != 0).collect::<BTreeSet<_>>().into_iter().collect(), ..Units::default() };
        let mut todo: Vec<usize> = units.reads.clone();
        while let Some(d) = todo.pop() {
            if units.libs.contains_key(&d) {
                continue;
            }
            let (brands, its) = library_units(whole, tree, joined, verdicts, contributions, t, d, lang)?;
            todo.extend(its.iter().copied());
            units.paths.insert(d, format!("{}{sep}units", import_of.get(&d).cloned().unwrap_or_default()));
            units.names.insert(d, named(tree, d, t).unwrap_or_default().to_string());
            units.libs.insert(d, (brands, its));
        }
        let (name, import, shown_version) = match t {
            Target::TypeScript => {
                let l = named(tree, 0, t).unwrap_or_default().to_string();
                (l.clone(), l, version.to_string())
            }
            Target::Python => {
                let Some(v) = pep440(&version) else {
                    return Err(refusal(
                        2,
                        tr!(
                            "版 {version} のプレリリースは、Python の版の形（PEP 440）にできません。Python のパッケージの版にできるのは、alpha、beta、rc と番号のプレリリースだけです",
                            "the pre-release of {version} has no form among Python's versions (PEP 440): only alpha, beta and rc with a number have one"
                        ),
                        lang,
                    ));
                };
                (named(tree, 0, t).unwrap_or_default().to_string(), import_of.get(&0).cloned().unwrap_or_default(), v)
            }
            Target::Go => {
                let m = import_of.get(&0).cloned().unwrap_or_default();
                (m.clone(), m, version.to_string())
            }
        };
        let go_package = go_package(named(tree, 0, Target::Go).unwrap_or_default());
        out.push((
            t,
            Library { name, import, go_package, version: shown_version, license: license.clone(), description: description.clone(), source: source.clone(), table: Arc::new(table), reads, units },
        ));
    }
    Ok(out)
}

/// The types of the units the library of node `d` holds in its `units` module, by their names, and
/// the libraries it reads: from the files its own `ritsu gen --library` makes it of (its module's,
/// and those of the modules they reach that no library holds for it).
#[allow(clippy::too_many_arguments)]
fn library_units(
    whole: &Project,
    tree: &Tree,
    joined: &Joined,
    verdicts: &Rc<ritsu_cross::ProgramVerdicts>,
    contributions: &[String],
    t: Target,
    d: usize,
    lang: Lang,
) -> Result<(BTreeSet<String>, Vec<usize>), Refusal> {
    let prefix = tree.nodes[d].prefix.clone();
    let mut own = whole.clone();
    own.files.retain(|f| whole.dependency_of(&f.rel).as_deref() == Some(prefix.as_str()));
    let reach = own.reached(joined);
    let mut held: BTreeSet<String> = BTreeSet::new();
    let mut reads: BTreeSet<usize> = BTreeSet::new();
    for f in &reach.files {
        let Some(x) = node_of(&reach, tree, &f.rel) else { continue };
        let h = holder(tree, t, d, x, 0);
        if h == d {
            held.insert(f.rel.clone());
        } else {
            reads.insert(h);
        }
    }
    let rulec_lang = if lang == Lang::Ja { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En };
    let mut units = Vec::new();
    for f in reach.of(Tool::Rulec).into_iter().filter(|f| held.contains(&f.rel)) {
        if rulec::link::is_contribution_file(&f.shown) {
            continue;
        }
        let src = read(&reach, f, lang)?;
        let (_, _, used) = rulec::link::with_set(contributions, || rulec::i18n::with(rulec_lang, || rulec::codegen::package_parts_at(&src, &f.shown, &f.rel, t.key(), "units", Place::here())))
            .map_err(|e| refusal(1, Text::new(e.clone(), e), lang))?;
        units.extend(used);
    }
    for f in reach.of(Tool::Chobo).into_iter().filter(|f| held.contains(&f.rel)) {
        let (_, c) = chobo::check::check_file(Path::new(&f.shown)).map_err(|e| refusal(2, Text::new(e.clone(), e), lang))?;
        let Some(book) = c.book.as_ref() else {
            let shown = &f.shown;
            return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), lang));
        };
        units.extend(chobo::client::package_units(book));
    }
    let modules: Vec<(Rc<kaname::check::Checked>, String, Place)> =
        kaname_modules(&reach, joined, verdicts, lang)?.into_iter().filter(|(_, rel)| held.contains(rel)).map(|(c, rel)| (c, rel, Place::here())).collect();
    if !modules.is_empty() {
        let kt = kaname::r#gen::Target::parse(t.key()).expect("the three targets are kaname's");
        units.extend(kaname::r#gen::package_in(&modules, kt, lang, "units").units);
    }
    let brands = units.iter().filter_map(|u| u.first_spelling().type_name()).collect();
    Ok((brands, reads.into_iter().collect()))
}

/// The TypeScript configuration `prepare` builds the library with: CommonJS into `dist/`, with the
/// declarations its `exports` name (DESIGN 17.16).
pub fn tsconfig(head: &str) -> String {
    format!(
        "{head}// How `npm install` (its `prepare`) builds the package into dist/, which package.json's exports name.\n{{\n  \"compilerOptions\": {{\n    \"strict\": true,\n    \"declaration\": true,\n    \"outDir\": \"dist\",\n    \"rootDir\": \".\",\n    \"module\": \"nodenext\",\n    \"moduleResolution\": \"nodenext\",\n    \"target\": \"es2022\",\n    \"skipLibCheck\": true\n  }},\n  \"include\": [\"**/*.ts\"],\n  \"exclude\": [\"dist\", \"node_modules\"]\n}}\n"
    )
}

/// The modules a library's TypeScript loads only when it is asked to, by a literal dynamic
/// `import("…")` (a workflow's agent loads its SDK so), that are neither what it depends on
/// (`known`) nor relative nor Node's own.
pub fn optional_modules<'a>(bodies: impl IntoIterator<Item = &'a str>, known: impl Fn(&str) -> bool) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for body in bodies {
        for (at, _) in body.match_indices("import(\"") {
            let name = body[at + 8..].split('"').next().unwrap_or("");
            if !name.is_empty() && !name.starts_with('.') && !name.starts_with("node:") && !known(name) {
                out.insert(name.to_string());
            }
        }
    }
    out
}

/// optional-modules.d.ts: declares the [`optional_modules`], so that `npm install` builds the
/// library without them. tsc emits nothing of it to dist/, and the declarations in dist/ do not
/// name them; a host that asks for one installs it.
pub fn optional_modules_dts(head: &str, names: &BTreeSet<String>) -> String {
    let mut s = format!(
        "{head}// The modules the package's code loads only when it is asked to, as a workflow's agent loads its\n// SDK. They are declared here so that the package builds without them; a host that asks for one\n// installs it.\n\n"
    );
    for n in names {
        s.push_str(&format!("declare module {};\n", ritsu_emit::lit::json(n)));
    }
    s
}

/// package.json of the library: its name, version, description (the module's `description` line,
/// else `what`, with `generated`, since JSON has no comment to carry it) and license, the module's
/// `source` as its `repository` (npm's provenance checks it against the repository the package is
/// built in), CommonJS built into `dist/` (the `exports` of every module of the package), the
/// runtime it imports as ranges from the versions tested (`runtime`), the libraries it imports as
/// `peerDependencies` (`peers`), and the TypeScript that builds it.
pub fn package_json(l: &Library, what: &str, generated: &str, runtime: &BTreeMap<String, &str>, peers: &BTreeMap<String, String>) -> String {
    let j = ritsu_emit::lit::json;
    let about = l.description.as_ref().map_or_else(|| what.to_string(), |d| format!("{}.", one_line(d).trim_end_matches('.')));
    let mut o = format!("{{\n  \"name\": {},\n  \"version\": {},\n  \"description\": {}", j(&l.name), j(&l.version), j(&format!("{about} {generated}")));
    if let Some(license) = &l.license {
        o.push_str(&format!(",\n  \"license\": {}", j(license)));
    }
    if let Some(source) = &l.source {
        o.push_str(&format!(",\n  \"repository\": {{\n    \"type\": \"git\",\n    \"url\": {}\n  }}", j(source)));
    }
    o.push_str(",\n  \"type\": \"commonjs\",\n  \"exports\": {\n    \".\": {\n      \"types\": \"./dist/index.d.ts\",\n      \"default\": \"./dist/index.js\"\n    },\n    \"./*\": {\n      \"types\": \"./dist/*.d.ts\",\n      \"default\": \"./dist/*.js\"\n    }\n  },\n  \"files\": [\n    \"dist\"\n  ],\n  \"scripts\": {\n    \"prepare\": \"tsc -p tsconfig.json\"\n  }");
    let block = |o: &mut String, key: &str, items: Vec<(String, String)>| {
        if !items.is_empty() {
            let lines: Vec<String> = items.iter().map(|(n, v)| format!("    {}: {}", j(n), j(v))).collect();
            o.push_str(&format!(",\n  \"{key}\": {{\n{}\n  }}", lines.join(",\n")));
        }
    };
    block(&mut o, "dependencies", runtime.iter().map(|(n, v)| (n.clone(), npm_range(&version_of(v)))).collect());
    block(&mut o, "peerDependencies", peers.iter().map(|(n, v)| (n.clone(), v.clone())).collect());
    block(&mut o, "devDependencies", vec![("typescript".to_string(), TYPESCRIPT.to_string())]);
    o.push_str("\n}\n");
    o
}

/// pyproject.toml of the library: its name, version, description (the module's `description` line)
/// and license, the Python the app form asks for, and what it imports as ranges (`deps`: each
/// distribution with its range), built by setuptools from the version that reads a license's SPDX
/// expression (PEP 639).
pub fn pyproject(l: &Library, head: &str, what: &str, deps: &[(String, String)]) -> String {
    let j = ritsu_emit::lit::json;
    let mut o = format!("{head}# {what}\n\n[project]\nname = {}\nversion = {}\n", j(&l.name), j(&l.version));
    if let Some(description) = &l.description {
        o.push_str(&format!("description = {}\n", j(&one_line(description))));
    }
    if let Some(license) = &l.license {
        o.push_str(&format!("license = {}\n", j(license)));
    }
    let deps_toml: String = deps.iter().map(|(n, r)| format!("  {},\n", j(&format!("{n}{r}")))).collect();
    o.push_str(&format!(
        "requires-python = \">=3.10\"\ndependencies = [\n{deps_toml}]\n\n[build-system]\nrequires = [\"setuptools>=77\"]\nbuild-backend = \"setuptools.build_meta\"\n\n[tool.setuptools.packages.find]\ninclude = [{}, {}]\n\n[tool.setuptools.package-data]\n\"*\" = [\"py.typed\", \"*.sql\"]\n",
        j(&l.import),
        j(&format!("{}.*", l.import))
    ));
    o
}

/// A description on one line, as the registries show it (PEP 621's `description` is one line).
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The `go` line of a library's go.mod: the higher of the oldest Go release supported (`floor`) and
/// the `go` lines of the modules it requires (`asked`, each `1.24` or `1.26.0`), as written.
pub fn go_line<'a>(floor: &'a str, asked: impl IntoIterator<Item = &'a str>) -> &'a str {
    let key = |v: &str| -> Vec<u64> {
        let mut k: Vec<u64> = v.split('.').map(|p| p.parse().unwrap_or(0)).collect();
        k.resize(3, 0);
        k
    };
    asked.into_iter().fold(floor, |best, v| if key(v) > key(best) { v } else { best })
}

/// go.mod of the library: its module (with `/vN` from version 2), its `go` line, and what it
/// requires: each module its Go imports, at the version tested or the version the tree chooses
/// (`requires`), and the modules those ask for at a version with a known vulnerability, at the
/// version the tests run (`raised`, indirect). No go.sum: the code that uses the library sums what
/// its own go.mod resolves.
pub fn go_mod(l: &Library, head: &str, go: &str, requires: &[(String, String)], raised: &[(String, String)]) -> String {
    let mut o = format!("{head}\nmodule {}\n\ngo {go}\n", l.name);
    let block = |o: &mut String, items: &[(String, String)], note: &str| match items {
        [] => {}
        [(m, v)] => o.push_str(&format!("\nrequire {m} {v}{note}\n")),
        _ => {
            o.push_str("\nrequire (\n");
            for (m, v) in items {
                o.push_str(&format!("\t{m} {v}{note}\n"));
            }
            o.push_str(")\n");
        }
    };
    block(&mut o, requires, "");
    block(&mut o, raised, " // indirect");
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn the_ranges_take_up_to_the_next_major_version() {
        assert_eq!(npm_range(&v("1.1.0")), "^1.1.0");
        assert_eq!(npm_range(&v("0.17.9")), "^0.17.9");
        assert_eq!(npm_range(&v("0.0.3")), ">=0.0.3 <0.1.0");
        assert_eq!(python_range(&v("1.1.0")).as_deref(), Some(">=1.1.0,<2.0.0"));
        assert_eq!(python_range(&v("0.17.9")).as_deref(), Some(">=0.17.9,<0.18.0"));
        assert_eq!(python_range(&v("0.0.3")).as_deref(), Some(">=0.0.3,<0.1.0"));
        assert_eq!(python_range(&v("2.0.0-rc.1")).as_deref(), Some(">=2.0.0rc1,<3.0.0"));
    }

    #[test]
    fn a_pre_release_has_a_python_form_only_for_alpha_beta_and_rc() {
        assert_eq!(pep440(&v("1.0.0")).as_deref(), Some("1.0.0"));
        assert_eq!(pep440(&v("1.0.0-rc.1")).as_deref(), Some("1.0.0rc1"));
        assert_eq!(pep440(&v("1.0.0-alpha.2")).as_deref(), Some("1.0.0a2"));
        assert_eq!(pep440(&v("1.0.0-beta")).as_deref(), Some("1.0.0b0"));
        assert_eq!(pep440(&v("1.0.0-rc3")).as_deref(), Some("1.0.0rc3"));
        assert_eq!(pep440(&v("1.0.0-dev.1")), None);
        assert_eq!(pep440(&v("1.0.0-rc.1.2")), None);
    }

    #[test]
    fn python_imports_a_distribution_by_its_normal_name() {
        assert_eq!(python_import("acme-pricing").as_deref(), Some("acme_pricing"));
        assert_eq!(python_import("Acme.Pricing__Core").as_deref(), Some("acme_pricing_core"));
        assert_eq!(python_import("2fa"), None);
        assert_eq!(python_import("import"), None);
    }

    #[test]
    fn a_go_module_names_its_major_version_from_two() {
        assert_eq!(go_module("example.com/acme/pricing-go", &v("1.1.0")), "example.com/acme/pricing-go");
        assert_eq!(go_module("example.com/acme/pricing-go", &v("2.0.0")), "example.com/acme/pricing-go/v2");
        assert_eq!(go_package("example.com/acme/pricing-go"), "pricing_go");
        assert_eq!(go_package("example.com/go"), "pgo");
        assert_eq!(go_line("1.26.0", ["1.24", "1.25.4", "1.17"]), "1.26.0");
        assert_eq!(go_line("1.26.0", ["1.27.1"]), "1.27.1");
    }

    /// The package reads `pricing` (1) and `fees` (2), each with a type of GBPc: two types of one
    /// unit stop the run, unless one library reads the other, whose type it hands on.
    #[test]
    fn a_unit_has_one_type_in_a_package() {
        let set = |b: &[&str]| b.iter().map(|s| s.to_string()).collect::<BTreeSet<String>>();
        let units = Units {
            reads: vec![1, 2],
            libs: BTreeMap::from([(1, (set(&["GBPc"]), vec![])), (2, (set(&["GBPc", "Gram"]), vec![]))]),
            paths: BTreeMap::from([(1, "@acme/pricing/units".to_string()), (2, "@acme/fees/units".to_string())]),
            names: BTreeMap::from([(1, "@acme/pricing".to_string()), (2, "@acme/fees".to_string())]),
        };
        let brand = |b: &str, shown: &str| vec![(b.to_string(), shown.to_string())];
        let ok = |r: Result<BTreeMap<String, String>, Refusal>| r.unwrap_or_else(|e| panic!("{}", e.said));
        assert_eq!(ok(units.shared(&brand("Gram", "mass[g]"), Lang::En)), BTreeMap::from([("Gram".to_string(), "@acme/fees/units".to_string())]));
        assert_eq!(ok(units.shared(&brand("Hour", "time[h]"), Lang::En)), BTreeMap::new(), "a unit no library holds is the package's own");
        let en = units.shared(&brand("GBPc", "money[GBPc]"), Lang::En).unwrap_err();
        assert_eq!(en.code, 2);
        assert_eq!(
            en.said,
            "error: the unit money[GBPc] has a type in each of two libraries that do not depend on each other, `@acme/pricing` and `@acme/fees`, and a package has one type of each unit: make one of the libraries depend on the other, or generate the app form (without --library)\n"
        );
        let ja = units.shared(&brand("GBPc", "money[GBPc]"), Lang::Ja).unwrap_err();
        assert_eq!(
            ja.said,
            "エラー: 単位 money[GBPc] の型を、互いに依存しない二つのライブラリ `@acme/pricing` と `@acme/fees` がそれぞれ持っています。一つのパッケージは、一つの単位に一つの型しか持てません。片方のライブラリをもう片方に依存させるか、アプリの形（--library なし）で生成してください\n"
        );
        // fees reads pricing: its GBPc is pricing's
        let mut through = units.clone();
        through.libs.get_mut(&2).unwrap().1 = vec![1];
        assert_eq!(ok(through.shared(&brand("GBPc", "money[GBPc]"), Lang::En))["GBPc"], "@acme/pricing/units");
    }

    #[test]
    fn the_modules_loaded_when_asked_are_declared() {
        let body = "const a: any = await import(\"@anthropic-ai/sdk\");\nconst b = await import(\"./rules.ts\");\nconst c = await import(\"node:fs\");\nconst d = await import(\"@temporalio/client\");\nconst e = await import(known[0]);\nconst f = await import(\"@acme/pricing/rules/x\");\n";
        let known = |n: &str| n.starts_with("@temporalio/") || n.starts_with("@acme/pricing/");
        assert_eq!(optional_modules([body], known), BTreeSet::from(["@anthropic-ai/sdk".to_string()]));
        assert_eq!(optional_modules_dts("// head\n", &BTreeSet::from(["openai".to_string()])).lines().last(), Some("declare module \"openai\";"));
    }

    /// The TypeScript a library builds with is the one the tests build it with.
    #[test]
    fn the_typescript_is_dandoris_tools() {
        let manifest = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../dandori/tools/temporal/package.json")).unwrap();
        assert!(manifest.contains(&format!("\"typescript\": \"{TYPESCRIPT}\"")), "dandori's tools/temporal/package.json builds with another TypeScript than {TYPESCRIPT}:\n{manifest}");
    }
}
