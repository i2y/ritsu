//! `ritsu gen` (DESIGN 9.3): the rules, the dates, the clients of the books, the workflows, the
//! functional cores (kaname's modules) and the code that asks the gates of a project, as one package
//! for each of TypeScript, Python and Go, with the gates' Cedar beside them, written once. kaname's
//! modules go under glue/, a name kept because a host's code imports them by that path.
//!
//! ```text
//! generated/typescript/          generated/python/              generated/go/
//!   package.json                   pyproject.toml                 doc.go
//!   index.ts                       <name>/__init__.py, py.typed
//!   rules/<alias>.ts               <name>/rules/<alias>.py        rules/<package>/<alias>.go
//!   dates/<alias>.ts               <name>/dates/<alias>.py        dates/<package>/<package>.go
//!   books/<book>.ts                <name>/books/<book>.py         books/<package>/book.go
//!   flows/<flow>/…                 <name>/flows/<flow>/…          flows/<package>/…
//!   glue/<module>.ts, kaname.ts    <name>/glue/<module>.py, …     glue/<package>/<package>.go, …
//!   glue/books/<book>.ts, …        <name>/glue/books/<book>.py, … glue/books/<package>/…
//!   authz/<gate>.ts
//!   units.ts                       <name>/units.py                units/units.go
//! generated/cedar/<gate>.cedar, .cedarschema, .cedarschema.json, .policies.json
//! ```
//!
//! Each language writes its part with its own generator (DESIGN 9.1): rulec the rule's module,
//! koyomi the dates file's, chobo the client of the book (and, on PostgreSQL, the SQL it calls),
//! dandori the workflow for Temporal, kaname the module's code under glue/, which calls the
//! package's rules/ and dates/ and builds the transfers of its books (kaname's DESIGN 10), and
//! sekisho the gate's Cedar and the code that builds its requests and asks them (sekisho's DESIGN
//! 5.6), which reads the package's rules/ and dates/. A workflow reads the rules, the dates and the books from the
//! package's rules/, dates/ and books/ (dandori's `InPackage`), not from code put beside it: the
//! imports name the package's modules, and the books the transport takes are typed with the
//! package's clients, so that the type checker of the target holds the two languages to each
//! other. An amount of money or of a quantity that a rule takes or returns, or that a book's call
//! takes, is of the package's one type of its unit, from `units`, so that the type checker holds
//! a rule's output given to a book in code written by hand to the book's unit as well. Every file
//! begins with the same head (ritsu's version, and the file it is made from by
//! its path from the project's root; DESIGN 9.2). `--check` writes nothing and says what is stale,
//! as `rulec gen --check` does.
//!
//! In a module (DESIGN 17.8), the package holds the files of the dependencies the project's files
//! reach ([`Project::reached`]), each dependency's parts under `deps/<name>/` (`deps/pricing/`,
//! `deps/pricing/deps/tax/`), with indexes and `__init__.py` at each level, and each part's head
//! naming its file the canonical way with its module's name and version. Every generator is handed
//! where its part goes and where each file it reads goes ([`ritsu_emit::package::Place`]). The rules
//! are made with the contributions `ritsu check` merges for the same paths, and their findings are
//! counted as it counts them, so that the two say the same of a tree. Outside a module, the package
//! is what it was before there were modules (M14).
//!
//! `--library` writes each package as a library its registry takes instead (DESIGN 17.16, the
//! submodule [`library`]): named and versioned by the module's `ritsu.mod`, with what it depends on
//! as ranges, reading the parts of a dependency that is a library from that library.

mod library;

use crate::cli;
use ritsu_base::diag::Severity;
use ritsu_base::json::Json;
use ritsu_base::modules::Problem;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_emit::header::{Comment, Origin};
use ritsu_emit::package::{Module, Place};
use ritsu_ports::{Finding, Part, Verdict};
use ritsu_project::{File, Joined, Project};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A language a package is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    TypeScript,
    Python,
    Go,
}

impl Target {
    pub const ALL: [Target; 3] = [Target::TypeScript, Target::Python, Target::Go];

    /// The value of `--target`, and the directory under `--out`.
    pub fn key(self) -> &'static str {
        match self {
            Target::TypeScript => "typescript",
            Target::Python => "python",
            Target::Go => "go",
        }
    }

    fn parse(s: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.key() == s)
    }

    /// dandori's target for the workflow (Temporal, in this language).
    fn dandori(self) -> &'static str {
        match self {
            Target::TypeScript => "temporal",
            Target::Python => "temporal-python",
            Target::Go => "temporal-go",
        }
    }
}

/// Where the books are kept: the client chobo writes for each (DESIGN 9.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Books {
    Postgres,
    TigerBeetle,
    /// MySQL, TiDB or Vitess (chobo's DESIGN 4.5)
    MySql,
}

impl Books {
    fn chobo(self, t: Target) -> chobo::target::Target {
        use chobo::target::Target as C;
        match (self, t) {
            (Books::Postgres, Target::TypeScript) => C::PostgresTypeScript,
            (Books::Postgres, Target::Python) => C::PostgresPython,
            (Books::Postgres, Target::Go) => C::PostgresGo,
            (Books::TigerBeetle, Target::TypeScript) => C::TigerBeetleTypeScript,
            (Books::TigerBeetle, Target::Python) => C::TigerBeetlePython,
            (Books::TigerBeetle, Target::Go) => C::TigerBeetleGo,
            (Books::MySql, Target::TypeScript) => C::MySqlTypeScript,
            (Books::MySql, Target::Python) => C::MySqlPython,
            (Books::MySql, Target::Go) => C::MySqlGo,
        }
    }

    /// The SQL the clients need in the database, beside them: PostgreSQL's schema and functions,
    /// or the MySQL family's tables; TigerBeetle needs none.
    fn sql(self) -> Option<chobo::target::Target> {
        match self {
            Books::Postgres => Some(chobo::target::Target::Postgres),
            Books::MySql => Some(chobo::target::Target::MySql),
            Books::TigerBeetle => None,
        }
    }
}

/// What a package is made with, besides the project.
#[derive(Clone, Debug)]
pub struct Options {
    /// The package's name: the npm package's, the Python package's (its directory) and, unless
    /// `module` says otherwise, the Go import path of the package's directory.
    pub name: String,
    /// The Go import path of the package's directory: a rule's package is `<module>/rules/<package>`.
    pub module: String,
    pub books: Books,
    /// Where the code of the gates asks: Cedar in the process, or Verified Permissions.
    pub authorizer: sekisho::r#gen::Authorizer,
    pub lang: Lang,
    /// The package as a library (`--library`, DESIGN 17.16): None for the app form.
    pub library: Option<library::Library>,
}

/// The versions the dependencies of a package are written with: the ones the generated code is
/// tested with here (dandori's crates/dandori/tools/temporal/package.json and
/// tools/temporal-python/requirements.txt, chobo's crates/chobo/tools/runner).
const TEMPORAL_TYPESCRIPT: &str = "1.24.0";
const TEMPORAL_PYTHON: &str = "1.33.0";
const TIGERBEETLE: &str = "0.17.9";
/// The Cedar the TypeScript of a gate asks, or the AWS SDK's client of Verified Permissions, at the
/// versions sekisho's tools/runner-ts locks (sekisho's generator says them).
const CEDAR_WASM: &str = sekisho::r#gen::typescript::CEDAR_WASM;
const AWS_SDK_VERIFIEDPERMISSIONS: &str = sekisho::r#gen::typescript::AWS_SDK;
/// And the Python of a gate: Cedar's Python binding, or boto3 for Verified Permissions (sekisho's
/// tools/runner-py).
const CEDARPY: &str = "4.12.1";
const BOTO3: &str = "1.43.103";
/// And the Go modules the Go of a package imports, with the versions tested here (dandori's
/// tools/temporal-go/go.mod, chobo's tools/runner/go/go.mod), which doc.go names: a package of Go
/// is a directory of the module it is put in, so its requirements are that module's go.mod's. Each
/// with the `go` line of its go.mod at that version, which a library's go.mod is at least
/// (DESIGN 17.16; the tests read the lines back from the module cache).
const GO_MODULES: &[(&str, &str, &str)] = &[
    ("go.temporal.io/sdk", "v1.49.0", "1.26.0"),
    ("go.temporal.io/api", "v1.63.5", "1.25.4"),
    ("github.com/jackc/pgx/v5", "v5.11.0", "1.25.0"),
    ("github.com/tigerbeetle/tigerbeetle-go", "v0.17.9", "1.17"),
    ("github.com/go-sql-driver/mysql", "v1.10.1", "1.24.0"),
    ("github.com/aws/aws-sdk-go-v2", "v1.47.1", "1.24"),
    ("github.com/aws/aws-sdk-go-v2/config", "v1.33.6", "1.24"),
    ("github.com/aws/aws-sdk-go-v2/service/lambda", "v1.110.0", "1.24"),
    ("github.com/aws/aws-sdk-go-v2/service/sns", "v1.47.2", "1.24"),
    ("github.com/aws/aws-sdk-go-v2/service/sqs", "v1.52.1", "1.24"),
    ("github.com/aws/smithy-go", "v1.28.1", "1.24"),
    ("github.com/openai/openai-go/v3", "v3.68.0", "1.25.0"),
    ("github.com/anthropics/anthropic-sdk-go", "v1.78.0", "1.24"),
    ("google.golang.org/protobuf", "v1.36.11", "1.23"),
    ("github.com/cedar-policy/cedar-go", "v1.8.0", "1.23.0"),
    ("github.com/aws/aws-sdk-go-v2/service/verifiedpermissions", "v1.41.1", "1.24"),
];
/// The oldest Go release the Go project supports (it supports the two newest; go1.27 and go1.26 on
/// 2026-10-28, https://go.dev/doc/devel/release#policy), as the `go` line of a library's go.mod
/// writes it when no module it requires asks for a later one (DESIGN 17.16).
const GO_FLOOR: &str = "1.26.0";
/// Modules the ones above ask for at a version with a known vulnerability, the version the tests
/// run instead (the go.mod of chobo's tools/runner/go and of dandori's tools/temporal-go), and the
/// advisory. `go mod tidy` gives a module the version a requirement asks for, not the newest (Go's
/// minimal version selection), so a package that imports the module on the left would otherwise
/// be built with the vulnerable one; doc.go says to raise it (DESIGN 9.3). pgx v5.11.0 asks for
/// golang.org/x/text v0.29.0, whose normalization its SCRAM authentication reaches (GO-2026-5970,
/// fixed in v0.39.0). An entry goes once the module on the left asks for a fixed version. The last
/// is the `go` line of the raised version's go.mod, as for [`GO_MODULES`].
const GO_RAISED: &[(&str, &str, &str, &str, &str)] = &[("github.com/jackc/pgx/v5", "golang.org/x/text", "v0.42.0", "GO-2026-5970", "1.26.0")];

/// A package: each file by its path under the package's directory, with the file of the project it
/// is made from (None for what the package itself is made of: the index and the manifest).
pub struct Package {
    pub files: Vec<(String, String, Option<String>)>,
}

/// Why nothing is generated: what the text prints (on the standard error), the exit code, and for
/// the JSON the sentence that says why and the diagnostics behind it, each as `ritsu check
/// --format json` writes one.
pub struct Refusal {
    pub said: String,
    pub code: u8,
    pub message: String,
    pub diagnostics: Vec<Json>,
}

fn refusal(code: u8, said: Text, lang: Lang) -> Refusal {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    let message = said.get(lang).to_string();
    Refusal { said: format!("{head}: {message}\n"), code, message, diagnostics: Vec::new() }
}

/// A generator's own diagnostics, which stop it after the files passed their checks: printed as
/// the language prints them, and in the JSON with the sentence `why`.
fn stopped(said: String, code: u8, why: Text, diagnostics: Vec<Json>, lang: Lang) -> Refusal {
    Refusal { said, code, message: why.get(lang).to_string(), diagnostics }
}

/// A diagnostic of a language, from its own JSON, as `ritsu check --format json` writes it.
fn diagnostic(tool: &str, file: Option<String>, json: Json) -> Json {
    crate::check::entry(tool, &Finding { code: String::new(), severity: Severity::Error, file, line: None, text: String::new(), json })
}

/// A value of serde_json (dandori's and chobo's diagnostics), as ritsu-base's JSON.
fn from_serde(v: &serde_json::Value) -> Json {
    ritsu_base::json::parse(&v.to_string()).unwrap_or(Json::Null)
}

/// Where the parts of the file of the project at `rel` go in the package (DESIGN 17.8): at the
/// package's directory for a file of the module at hand, under `deps/<name>/` for each dependency of
/// the chain a dependency's file is written with, which names its module and version in the head of
/// what is made from it. Outside a module, every file's parts are at the package's directory, and
/// so is every file they read (M14).
fn place_of(project: &Project, rel: &str) -> Place {
    let Some(Ok(tree)) = project.tree.as_deref() else { return Place::here() };
    let module = project.dependency_of(rel).and_then(|p| tree.by_prefix(&p)).map(|i| {
        let n = &tree.nodes[i];
        Module { name: n.name.to_string(), version: n.version().map(|v| v.to_string()) }
    });
    Place::of(&project.root, rel, module)
}

/// Whether every file of the languages a package is made of passes its language's check (kaname's
/// with what the checks across the borders came to at its calls, as `ritsu check` checks it); the
/// diagnostics of those that do not, as `ritsu check` prints them, when one does not. In a module,
/// what rulec says of a line of a contribution is filed under the contribution, as `ritsu check`
/// files it ([`Project::place_findings`], with the files `ritsu check` reads for the same paths,
/// `whole`), and a file an error is filed under does not pass either (DESIGN 17.8); a rule of a
/// dependency that fails only for such errors is not named among the files that fail
/// ([`Project::failed_for_others`]), as the line of the dependencies of `ritsu check` does not count
/// it.
fn checked(project: &Project, whole: &Project, joined: &Joined, verdicts: &Rc<ritsu_cross::ProgramVerdicts>, lang: Lang) -> Result<(), Refusal> {
    let mut said = String::new();
    let mut found: Vec<Json> = Vec::new();
    let mut worst = 0u8;
    let mut failing: Vec<String> = Vec::new();
    let mut all: Vec<(Tool, ritsu_ports::Checked)> = Vec::new();
    for tool in [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Kaname, Tool::Sekisho] {
        let files: Vec<String> = project.of(tool).iter().map(|f| f.shown.clone()).collect();
        if files.is_empty() {
            continue;
        }
        let units = match tool {
            Tool::Rulec => joined.rulec.checked(&project.root, &files, lang),
            Tool::Koyomi => joined.koyomi.checked(&project.root, &files, lang),
            Tool::Kaname => {
                let v: Rc<dyn ritsu_ports::Verdicts> = verdicts.clone();
                joined.kaname.checked_with(&project.root, &files, &joined.ports(), Some(v), lang)
            }
            Tool::Sekisho => sekisho::check::checked(&project.root, &files, &joined.sekisho().into(), lang),
            _ => joined.chobo.checked(&project.root, &files, lang),
        };
        all.extend(units.into_iter().map(|u| (tool, u)));
    }
    if project.tree.is_some() {
        whole.place_findings(&mut all, lang);
    }
    for (tool, u) in all.into_iter().filter(|(_, u)| u.verdict != Verdict::Passes) {
        worst = worst.max(if u.verdict == Verdict::Unchecked { 2 } else { 1 });
        for p in &u.parts {
            match p {
                Part::Finding(f) => {
                    said.push_str(&f.text_of(tool.word()));
                    found.push(crate::check::entry(tool.word(), f));
                }
                Part::Text(s) => said.push_str(s),
            }
        }
        // in a module, the files its errors are filed under, when they are others of the project's
        // (a contribution's)
        let others: Vec<String> = if project.tree.is_none() {
            Vec::new()
        } else {
            u.findings()
                .filter(|f| f.severity == Severity::Error)
                .filter_map(|f| f.file.as_deref().and_then(|r| whole.holds(r)))
                .map(|f| f.shown.clone())
                .filter(|s| *s != u.label)
                .collect()
        };
        // a rule of a dependency that fails only for what a contribution of another file adds to it
        // is not the file that fails: the contribution is (as `ritsu check` counts it)
        let for_others = project.tree.is_some() && ritsu_base::paths::from_root(&whole.root, Path::new(&u.label)).is_some_and(|r| whole.failed_for_others(&r, &u));
        if !for_others && !failing.contains(&u.label) {
            failing.push(u.label);
        }
        for s in others {
            if !failing.contains(&s) {
                failing.push(s);
            }
        }
    }
    // dandori with the dates and the books joined too, which its check of a project does not pass on
    for f in project.of(Tool::Dandori) {
        let got = with_ports(joined, || dandori::check::check_file(Path::new(&f.shown)));
        match got {
            Err(e) => {
                worst = 2;
                said.push_str(&format!("{e}\n"));
                failing.push(f.shown.clone());
            }
            Ok((src, c)) if dandori::diag::has_errors(&c.diags) => {
                worst = worst.max(if c.diags.iter().any(|d| d.code == "E018") { 2 } else { 1 });
                for d in &c.diags {
                    let text = dandori::commands::render(std::slice::from_ref(d), &f.shown, &src, lang);
                    let tag = format!("[{}]", d.code);
                    said.push_str(&match text.find(&tag) {
                        Some(at) => format!("{}[dandori {}]{}", &text[..at], d.code, &text[at + tag.len()..]),
                        None => text,
                    });
                    found.push(diagnostic("dandori", Some(f.rel.clone()), from_serde(&d.to_json(lang))));
                }
                failing.push(f.shown.clone());
            }
            Ok(_) => {}
        }
    }
    if failing.is_empty() {
        return Ok(());
    }
    let n = failing.len();
    let list = failing.join(", ");
    let r = refusal(
        worst,
        tr!("検査を通らないファイルが {n} 個あるので、何も生成しません: {list}", "{n} file(s) do not pass check, so nothing is generated: {list}"),
        lang,
    );
    Err(Refusal { said: format!("{said}{}", r.said), diagnostics: found, ..r })
}

/// The modules of kaname of the project, as they go into a package: each checked with the ports of
/// the rules, the dates and the books joined and with what the checks across the borders came to at
/// its calls, as `ritsu check` checks it (a place they show is not checked again in the code;
/// kaname's DESIGN 10.4), with its path from the root. Err when one does not pass, or reads a rule,
/// a dates file, a book or another module the project does not hold (the code of a module reads
/// them from the package).
pub fn kaname_modules(project: &Project, joined: &Joined, verdicts: &Rc<ritsu_cross::ProgramVerdicts>, lang: Lang) -> Result<Vec<(Rc<kaname::check::Checked>, String)>, Refusal> {
    let files = project.of(Tool::Kaname);
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let v: Rc<dyn ritsu_ports::Verdicts> = verdicts.clone();
    let suite: kaname::suite::Suite = joined.ports().into();
    let run = kaname::check::Run::new(suite.with_verdicts(v));
    let mut out = Vec::new();
    for f in files {
        let shown = &f.shown;
        let o = run.file(shown).map_err(|e| refusal(2, tr!("`{shown}` を読めません: {e}", "cannot read `{shown}`: {e}"), lang))?;
        let Some(c) = o.checked.clone() else {
            return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), lang));
        };
        // what the module reads beside it has to be the project's, for the package to hold it
        let p = kaname::value::Program::of(&c);
        let read: Vec<PathBuf> = p.rules.keys().chain(p.dates.keys()).chain(p.books.keys()).map(|k| p.file(k)).chain(c.world.modules.values().map(|m| PathBuf::from(&m.path))).collect();
        for path in read {
            let rel = ritsu_base::paths::from_root(&project.root, &path);
            if rel.as_ref().and_then(|r| project.holds(r)).is_none() {
                let other = rel.unwrap_or_else(|| path.display().to_string());
                return Err(refusal(
                    1,
                    tr!(
                        "`{shown}` が読む {other} が、プロジェクトのファイルにありません。パッケージはプロジェクトのファイルから作るので、そのファイルも ritsu gen に渡してください",
                        "`{shown}` reads {other}, which is not one of the project's files; a package is made of the project's files, so give ritsu gen that file too"
                    ),
                    lang,
                ));
            }
        }
        out.push((c, f.rel.clone()));
    }
    Ok(out)
}

/// `f`, with the ports of the rules, the dates and the books joined, as dandori reads a flow.
fn with_ports<R>(joined: &Joined, f: impl FnOnce() -> R) -> R {
    let dates: Rc<dyn ritsu_ports::Dates> = joined.koyomi.clone();
    let books: Rc<dyn ritsu_ports::Books> = joined.chobo.clone();
    dandori::sources::with_ports(joined.rules(), dates, books, f)
}

/// The package of `target`, made from the project's files (which pass their checks) and its modules
/// of kaname as [`kaname_modules`] checks them, the rules with the contributions of the files
/// `contributions` merged (the rulec files `ritsu check` reads for the same paths). What stops it for
/// the modules' sake is made sure of before, by [`modules_hold`].
pub fn package(project: &Project, contributions: &[String], joined: &Joined, modules: &[(Rc<kaname::check::Checked>, String)], target: Target, o: &Options) -> Result<Package, Refusal> {
    let lang = o.lang;
    // where each file's parts go; a library reads the parts of the modules its libraries hold from
    // them, and holds none of them (DESIGN 17.16)
    let at = |rel: &str| -> Place {
        let p = place_of(project, rel);
        match &o.library {
            Some(l) => p.with_libraries(l.table.clone()),
            None => p,
        }
    };
    let mut files: Vec<(String, String, Option<String>)> = Vec::new();
    // what goes under each kind's directory of each module's directory (empty for the module at
    // hand, `deps/pricing/` for a dependency's), for the indexes: each module by its name
    let mut kinds: BTreeMap<String, BTreeMap<&str, Vec<String>>> = BTreeMap::new();
    // and the file of the project each is made from
    let mut named: Vec<(&str, String, String)> = Vec::new();
    let base = match target {
        Target::Python => format!("{}/", o.name),
        _ => String::new(),
    };
    // rulec: each rule's module
    let rulec_lang = if lang == Lang::Ja { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En };
    // The contributions are merged from the files `ritsu check` reads (rulec's §15.231, DESIGN 17.8):
    // each rule's module is the rule with every contribution that reaches it, and a contribution has
    // no module of its own.
    let rule_files = contributions;
    // the units the rules and the books take amounts in, for the package's `units` module
    let mut units = Vec::new();
    for f in project.of(Tool::Rulec) {
        if rulec::link::is_contribution_file(&f.shown) {
            continue;
        }
        let src = read(project, f, lang)?;
        let place = at(&f.rel);
        if place.read_from_library(&place.dir) {
            continue;
        }
        // A file of enums is the module of its types, beside the rules that read them (rulec's
        // §15.212).
        let (rel, body, used) = rulec::link::with_set(rule_files, || rulec::i18n::with(rulec_lang, || rulec::codegen::package_parts_at(&src, &f.shown, &f.rel, target.key(), &o.module, place.clone())))
            .map_err(|e| refusal(1, Text::new(e.clone(), e), lang))?;
        units.extend(used);
        named.push(("rules", module_of(&rel), f.rel.clone()));
        kinds.entry(place.dir.clone()).or_default().entry("rules").or_default().push(module_of(&rel));
        files.push((format!("{base}{}rules/{rel}", place.dir), body, Some(f.rel.clone())));
    }
    // koyomi: each dates file's and calendar's module
    let mut loader = koyomi::calendar::Loader::default();
    for f in project.of(Tool::Koyomi) {
        let outcome = koyomi::check::check_file(&f.shown, &koyomi::check::Options::default(), &mut loader).map_err(|e| refusal(2, Text::new(e.clone(), e), lang))?;
        let Some(checked) = outcome.checked.as_ref().filter(|_| !outcome.has_errors()) else {
            let shown = &f.shown;
            return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), lang));
        };
        let place = at(&f.rel);
        if place.read_from_library(&place.dir) {
            continue;
        }
        let u = koyomi::codegen::unit_in(checked, lang, Some(&f.rel), place.module.as_ref());
        let (t, body) = match target {
            Target::TypeScript => (koyomi::naming::Target::TypeScript, koyomi::codegen::typescript::module(&u)),
            Target::Python => (koyomi::naming::Target::Python, koyomi::codegen::python::module(&u)),
            Target::Go => (koyomi::naming::Target::Go, koyomi::codegen::go::module(&u)),
        };
        // koyomi gen's own path, under its target's directory
        let rel = t.file(&u.alias).split_once('/').map(|(_, r)| r.to_string()).unwrap_or_default();
        named.push(("dates", module_of(&rel), f.rel.clone()));
        kinds.entry(place.dir.clone()).or_default().entry("dates").or_default().push(module_of(&rel));
        files.push((format!("{base}{}dates/{rel}", place.dir), body, Some(f.rel.clone())));
    }
    // chobo: each book's client, and on PostgreSQL the schema and the functions it calls, on the
    // MySQL family the tables its calls run on
    for f in project.of(Tool::Chobo) {
        let (src, c) = chobo::check::check_file(Path::new(&f.shown)).map_err(|e| refusal(2, Text::new(e.clone(), e), lang))?;
        let Some(book) = c.book.as_ref() else {
            let shown = &f.shown;
            return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), lang));
        };
        let stem = Path::new(&f.rel).file_name().map(|n| n.to_string_lossy().trim_end_matches(".book").to_string()).unwrap_or_default();
        let place = at(&f.rel);
        if place.read_from_library(&place.dir) {
            continue;
        }
        let origin = Origin::at(&f.rel, src.as_bytes()).of(place.module.clone());
        let in_package = chobo::client::InPackage { module: o.module.clone(), place: place.clone() };
        units.extend(chobo::client::package_units(book));
        let mut wanted = vec![o.books.chobo(target)];
        wanted.extend(o.books.sql());
        for ct in wanted {
            let built = chobo::target::build_in(book, &stem, ct, &origin, Some(&in_package)).map_err(|ds| {
                let text: String = ds.iter().map(|d| chobo::diag::Show::shown(d, &f.shown, &src, lang)).collect();
                let json = ds.iter().map(|d| diagnostic("chobo", Some(f.rel.clone()), from_serde(&chobo::diag::Show::json_in(d, &f.shown, &src, lang)))).collect();
                let (shown, to) = (&f.shown, target.key());
                stopped(text, 1, tr!("`{shown}` からは {to} のパッケージを生成できません", "`{shown}` cannot be made into the {to} package"), json, lang)
            })?;
            for (rel, body) in built {
                if Some(ct) != o.books.sql() {
                    named.push(("books", module_of(&rel), f.rel.clone()));
                    kinds.entry(place.dir.clone()).or_default().entry("books").or_default().push(module_of(&rel));
                }
                files.push((format!("{base}{}books/{rel}", place.dir), body, Some(f.rel.clone())));
            }
        }
    }
    // dandori: each workflow, for Temporal in the language, reading the package's rules, dates and books
    let module = o.module.clone();
    for f in project.of(Tool::Dandori) {
        let place = at(&f.rel);
        if place.read_from_library(&place.dir) {
            continue;
        }
        let built = with_ports(joined, || -> Result<(Vec<(String, String)>, String), Refusal> {
            let (src, c) = dandori::check::check_file(Path::new(&f.shown)).map_err(|e| refusal(2, Text::new(e.clone(), e), lang))?;
            let Some(mut m) = c.model else {
                let shown = &f.shown;
                return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), lang));
            };
            // what the workflow reads beside its own code has to be the project's, for the package to hold it
            let read: Vec<PathBuf> = m.rules.iter().map(|r| r.info.path.clone()).chain(m.books.iter().map(|b| b.path.clone())).collect();
            for p in read {
                let rel = ritsu_base::paths::from_root(&project.root, &p);
                if rel.as_ref().and_then(|r| project.holds(r)).is_none() {
                    let (shown, other) = (&f.shown, rel.unwrap_or_else(|| p.display().to_string()));
                    return Err(refusal(
                        1,
                        tr!(
                            "`{shown}` が読む {other} が、プロジェクトのファイルにありません。パッケージはプロジェクトのファイルから作るので、そのファイルも ritsu gen に渡してください",
                            "`{shown}` reads {other}, which is not one of the project's files; a package is made of the project's files, so give ritsu gen that file too"
                        ),
                        lang,
                    ));
                }
            }
            m.source_path = f.rel.clone();
            m.package = Some(dandori::model::InPackage { go_module: module.clone(), place: place.clone() });
            let dir = match target {
                Target::Go => dandori::temporal_go::package(&m),
                _ => dandori::render::ident(&m.name),
            };
            match dandori::commands::build(&m, target.dandori()) {
                Some(Ok(files)) => Ok((files, dir)),
                Some(Err(diags)) => {
                    let json = diags.iter().map(|d| diagnostic("dandori", Some(f.rel.clone()), from_serde(&d.to_json(lang)))).collect();
                    let (shown, to) = (&f.shown, target.key());
                    Err(stopped(dandori::commands::render(&diags, &f.shown, &src, lang), 1, tr!("`{shown}` からは {to} のパッケージを生成できません", "`{shown}` cannot be made into the {to} package"), json, lang))
                }
                None => unreachable!("the three targets are dandori's"),
            }
        })?;
        let (built, dir) = built;
        named.push(("flows", dir.clone(), f.rel.clone()));
        kinds.entry(place.dir.clone()).or_default().entry("flows").or_default().push(dir);
        for (rel, body) in built {
            files.push((format!("{base}{}flows/{rel}", place.dir), body, Some(f.rel.clone())));
        }
    }
    // sekisho: each gate's code of the requests, which reads the package's rules/ and dates/
    let gates: sekisho::suite::Suite = joined.sekisho().into();
    for f in project.of(Tool::Sekisho) {
        let outcome = gate(project, &gates, f, lang)?;
        let place = at(&f.rel);
        if place.read_from_library(&place.dir) {
            continue;
        }
        let to = sekisho::r#gen::Target { name: target.key(), authorizer: o.authorizer, go_module: o.module.clone(), place: place.clone() };
        let made = sekisho::r#gen::generate(&outcome, &to, &f.rel, lang, &gates).map_err(|e| {
            let shown = &f.shown;
            refusal(2, tr!("`{shown}` から生成できません（{e}）。sekisho の不具合です", "cannot generate from `{shown}` ({e}): a bug in sekisho"), lang)
        })?;
        for (rel, body) in made {
            // `typescript/authz/refunds.ts` as the package holds it: `authz/refunds.ts`, under its
            // module's directory
            let rel = rel.split_once('/').map(|(_, r)| r.to_string()).unwrap_or(rel);
            let module = module_of(rel.strip_prefix("authz/").unwrap_or(&rel));
            named.push(("authz", module.clone(), f.rel.clone()));
            kinds.entry(place.dir.clone()).or_default().entry("authz").or_default().push(module);
            files.push((format!("{base}{}{rel}", place.dir), body, Some(f.rel.clone())));
        }
    }
    // kaname: each module's code under glue/, which calls the package's rules/ and dates/ and builds
    // the transfers of its books, with the helper every module reads and the types of the
    // transfers; glue/'s indexes are kaname's own (the books' transfers are under glue/books/)
    if !modules.is_empty() {
        let kt = kaname::r#gen::Target::parse(target.key()).expect("the three targets are kaname's");
        let placed: Vec<(Rc<kaname::check::Checked>, String, Place)> = modules.iter().map(|(c, rel)| (c.clone(), rel.clone(), at(rel))).filter(|(_, _, p)| !p.read_from_library(&p.dir)).collect();
        let made = kaname::r#gen::package_in(&placed, kt, lang, &o.module);
        units.extend(made.units);
        for (c, rel, _) in &placed {
            named.push(("glue", c.module.name.ascii().to_string(), rel.clone()));
        }
        for (rel, body, from) in made.files {
            // glue/'s indexes are kaname's, at each module's directory it writes under
            kinds.entry(module_dir(&rel)).or_default().entry("glue").or_default();
            files.push((format!("{base}{rel}"), body, from));
        }
    }
    // the package's one type of each unit they take amounts in (rulec's §15.248); a library's is the
    // type of the library it reads the unit's amounts from, when there is one (DESIGN 17.16)
    let shared = match &o.library {
        Some(l) => {
            let brands: Vec<(String, String)> = units
                .iter()
                .filter_map(|u| {
                    let first = u.first_spelling();
                    first.type_name().map(|b| (b, first.to_string()))
                })
                .collect();
            l.units.shared(&brands, lang)?
        }
        None => BTreeMap::new(),
    };
    if let Some((rel, body)) = rulec::codegen::units_module_in(target.key(), &units, &shared) {
        files.push((format!("{base}{rel}"), body, None));
    }
    for list in kinds.values_mut().flat_map(|k| k.values_mut()) {
        list.dedup();
    }
    // each module's directory the package holds a part of, and the ones on the way to it
    let dirs: Vec<String> = kinds.keys().cloned().collect();
    for d in dirs {
        let mut up = d.as_str();
        while let Some(parent) = parent_module_dir(up) {
            kinds.entry(parent.to_string()).or_default();
            up = parent;
        }
    }
    // a module the package's index cannot import by its name: the languages hold their names to
    // their own tables (rulec only warns of a word a target keeps, W121), and the package's index
    // and directories are the package's own (DESIGN 9.3)
    let kept: &[&str] = match target {
        Target::Python => ritsu_emit::words::python::KEYWORDS,
        Target::Go => ritsu_emit::words::go::KEYWORDS,
        Target::TypeScript => &[],
    };
    if let Some((kind, name, from)) = named.iter().find(|(_, n, _)| kept.contains(&n.as_str())) {
        let language = if target == Target::Python { "Python" } else { "Go" };
        return Err(refusal(
            2,
            tr!(
                "`{from}` のモジュールの名前 `{name}` は {language} が予約している語なので、パッケージの {kind}/ に置けません。別の名前（別名）にしてください",
                "`{from}` makes the module `{name}`, a word {language} keeps, which the package cannot hold in its {kind}/; give it another name (an alias)"
            ),
            lang,
        ));
    }
    files.extend(around(target, o, &kinds, &files));
    Ok(Package { files })
}

/// What stops the packages of a module for the modules' sake, made sure of before any target is
/// written (DESIGN 17.8): two flows of two modules that make one Temporal workflow type, two gates of
/// two modules that declare one name of Cedar's, and, when Python is among the targets, a dependency
/// named with a word Python keeps. Each stops the run whatever is written, so none of them leaves the
/// targets made before it; a stop of another kind is met where its target is made, as in a tree
/// without a manifest, which none of these can stop. A file that does not pass is left to the target
/// that reads it.
pub fn modules_hold(project: &Project, joined: &Joined, targets: &[Target], lang: Lang) -> Result<(), Refusal> {
    if project.tree.is_none() {
        return Ok(());
    }
    // a workflow runs by the name of its type, which a worker of a server's knows one of: two flows
    // of two modules that make one type would be one workflow
    let mut types: Vec<(String, String, &str)> = Vec::new();
    for f in project.of(Tool::Dandori) {
        let Some(m) = with_ports(joined, || dandori::check::check_file(Path::new(&f.shown)).ok().and_then(|(_, c)| c.model)) else {
            continue;
        };
        let (ty, dir) = (dandori::temporal::workflow_type(&m), place_of(project, &f.rel).dir);
        if let Some((_, _, other)) = types.iter().find(|(t, d, _)| *t == ty && *d != dir) {
            let (a, b) = (*other, f.shown.as_str());
            return Err(refusal(
                2,
                tr!(
                    "`{a}` と `{b}` は、どちらも Temporal のワークフローの型 `{ty}` を作ります。ワークフローは型の名前で走るので、一つのパッケージに二つは置けません。手元のワークフローなら、名前か版を変えてください",
                    "`{a}` and `{b}` both make the Temporal workflow type `{ty}`: a workflow runs by the name of its type, so one package cannot hold both; if one is this module's, give it another name or version"
                ),
                lang,
            ));
        }
        types.push((ty, dir, &f.shown));
    }
    // what each gate's Cedar declares, each name with the file that declares it
    let gates: sekisho::suite::Suite = joined.sekisho().into();
    let declared: Vec<Vec<(String, String)>> = project.of(Tool::Sekisho).into_iter().filter_map(|f| gate(project, &gates, f, lang).ok()).map(|o| sekisho::r#gen::cedar_names(&o)).collect();
    cedar_names_hold(project, &declared, lang)?;
    // a dependency's name is the name of a Python package of the package's (DESIGN 17.3), which a
    // word Python keeps cannot be: the directories of the modules the package holds a part of are
    // the ones of its files (a contribution has no module of its own)
    if targets.contains(&Target::Python) {
        let made = [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Dandori, Tool::Sekisho, Tool::Kaname];
        let dirs: Vec<String> = made
            .iter()
            .flat_map(|t| project.of(*t))
            .filter(|f| !(f.tool == Tool::Rulec && rulec::link::is_contribution_file(&f.shown)))
            .map(|f| place_of(project, &f.rel).dir)
            .collect();
        if let Some(name) = dirs.iter().flat_map(|d| ritsu_emit::package::chain(d)).find(|n| ritsu_emit::words::python::KEYWORDS.contains(n)) {
            return Err(refusal(
                2,
                tr!(
                    "依存の名前 `{name}` は Python が予約している語なので、パッケージの deps/ に置けません。`require` で別の名前を付けてください",
                    "the dependency `{name}` is named with a word Python keeps, which the package cannot hold in its deps/; require it by another name"
                ),
                lang,
            ));
        }
    }
    Ok(())
}

/// The directory of the module a part's path is under (`deps/pricing/` for
/// `deps/pricing/glue/fees.ts`): its leading `deps/<name>/` parts; empty for the module at hand's.
fn module_dir(rel: &str) -> String {
    let parts: Vec<&str> = rel.split('/').collect();
    let mut out = String::new();
    let mut i = 0;
    while i + 2 < parts.len() && parts[i] == "deps" {
        out.push_str(&format!("deps/{}/", parts[i + 1]));
        i += 2;
    }
    out
}

/// The directory of the module that requires the module at `dir` (`deps/pricing/` for
/// `deps/pricing/deps/tax/`, empty for `deps/pricing/`); None for the module at hand's.
fn parent_module_dir(dir: &str) -> Option<&str> {
    let d = dir.strip_suffix('/')?;
    let (up, _) = d.rsplit_once('/')?;
    let up = up.strip_suffix("deps")?;
    Some(up)
}

/// Holds the names the gates' Cedar declares to one another (DESIGN 17.8). `gates` has, for each
/// gate of the package, each name its Cedar declares with the file that declares it: the gate's own
/// and the ones of the files it reads with `use gate`. Two files of two modules that declare one
/// name of Cedar's (a type or an action in its namespace) would be one thing where their gates are
/// asked together, so the run stops. The files one gate reads together are one namespace, which
/// sekisho holds them to (E210, E211), whatever their modules; and in one module, the names are
/// sekisho's own concern, as they were before there were modules.
fn cedar_names_hold(project: &Project, gates: &[Vec<(String, String)>], lang: Lang) -> Result<(), Refusal> {
    let module = |file: &str| ritsu_base::paths::from_root(&project.root, Path::new(file)).and_then(|r| project.dependency_of(&r));
    let together = |a: &str, b: &str| gates.iter().any(|g| g.iter().any(|(f, _)| f == a) && g.iter().any(|(f, _)| f == b));
    let mut declared: Vec<(&str, &str)> = Vec::new();
    for (file, name) in gates.iter().flatten() {
        let (file, name) = (file.as_str(), name.as_str());
        if let Some((other, _)) = declared.iter().find(|(f, n)| *n == name && *f != file && module(f) != module(file) && !together(f, file)) {
            let (a, b) = (*other, file);
            return Err(refusal(
                2,
                tr!(
                    "`{a}` と `{b}` は、どちらも Cedar の `{name}` を宣言しています。Cedar は型と操作を名前空間の中の名前で知るので、一つのパッケージに二つは置けません。手元のゲートなら、`namespace` か名前を変えてください",
                    "`{a}` and `{b}` both declare the Cedar `{name}`: Cedar knows a type or an action by its name in its namespace, so one package cannot hold both; if one is this module's gate, give it another `namespace` or name"
                ),
                lang,
            ));
        }
        if !declared.contains(&(file, name)) {
            declared.push((file, name));
        }
    }
    Ok(())
}

/// A gate of the project, checked with every language joined: Err when it does not pass, or when
/// it reads a rule, a dates file or a calendar the project does not hold (the code of its requests
/// reads them from the package).
fn gate(project: &Project, suite: &sekisho::suite::Suite, f: &File, lang: Lang) -> Result<sekisho::check::Outcome, Refusal> {
    let shown = &f.shown;
    // the project's root, which the references of `@guards` are written from (as `ritsu check` gives it)
    let opts = sekisho::check::Options { root: Some(project.root.clone()), ..sekisho::check::Options::default() };
    let o = sekisho::check::check_file(shown, suite, &opts).map_err(|e| refusal(2, tr!("`{shown}` を読めません: {e}", "cannot read `{shown}`: {e}"), lang))?;
    let Some(checked) = o.walked.as_ref().filter(|_| !o.has_errors()) else {
        return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), lang));
    };
    for u in checked.gate.uses.iter().filter(|u| matches!(u.kind, sekisho::model::UseKind::Rule | sekisho::model::UseKind::Dates | sekisho::model::UseKind::Calendar)) {
        let rel = ritsu_base::paths::from_root(&project.root, &u.file);
        if rel.as_ref().and_then(|r| project.holds(r)).is_none() {
            let other = rel.unwrap_or_else(|| u.file.display().to_string());
            return Err(refusal(
                1,
                tr!(
                    "`{shown}` が読む {other} が、プロジェクトのファイルにありません。パッケージはプロジェクトのファイルから作るので、そのファイルも ritsu gen に渡してください",
                    "`{shown}` reads {other}, which is not one of the project's files; a package is made of the project's files, so give ritsu gen that file too"
                ),
                lang,
            ));
        }
    }
    Ok(o)
}

/// The Cedar of the project's gates, written once beside the packages (`<out>/cedar/`; sekisho's
/// DESIGN 5.6): each file by its path under that directory; and, for Verified Permissions, what of
/// it is over a quota (W401), as `sekisho gen --authorizer avp` says it, and in JSON.
pub fn cedar(project: &Project, joined: &Joined, authorizer: sekisho::r#gen::Authorizer, lang: Lang) -> Result<(Package, String, Vec<Json>), Refusal> {
    let gates: sekisho::suite::Suite = joined.sekisho().into();
    let mut files = Vec::new();
    let mut said = String::new();
    let mut warned = Vec::new();
    for f in project.of(Tool::Sekisho) {
        let outcome = gate(project, &gates, f, lang)?;
        if authorizer == sekisho::r#gen::Authorizer::Avp {
            for d in sekisho::r#gen::quotas(&outcome, lang) {
                said.push_str(&d.render(lang));
                warned.push(diagnostic("sekisho", Some(f.rel.clone()), d.to_json(lang)));
            }
        }
        let place = place_of(project, &f.rel);
        let to = sekisho::r#gen::Target { name: "cedar", authorizer, go_module: String::new(), place: place.clone() };
        let made = sekisho::r#gen::generate(&outcome, &to, &f.rel, lang, &gates).map_err(|e| {
            let shown = &f.shown;
            refusal(2, tr!("`{shown}` から生成できません（{e}）。sekisho の不具合です", "cannot generate from `{shown}` ({e}): a bug in sekisho"), lang)
        })?;
        // a dependency's under `deps/<name>/` beside the packages too
        for (rel, body) in made {
            files.push((format!("{}{}", place.dir, rel.strip_prefix("cedar/").unwrap_or(&rel)), body, Some(f.rel.clone())));
        }
    }
    Ok((Package { files }, said, warned))
}

/// The file of the project, read.
fn read(project: &Project, f: &File, lang: Lang) -> Result<String, Refusal> {
    let p = ritsu_base::paths::on_disk(&project.root, &f.rel);
    ritsu_base::fs::read_to_string(&p).map_err(|e| {
        let shown = &f.shown;
        refusal(2, tr!("`{shown}` を読めません: {e}", "cannot read `{shown}`: {e}"), lang)
    })
}

/// The name a module goes by in an index: its file's name without the extension, or a package's
/// directory.
fn module_of(rel: &str) -> String {
    match rel.split_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => rel.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or_else(|| rel.to_string()),
    }
}

/// The indexes of the parts of each dependency (DESIGN 17.8), by their paths under the package's
/// directory: for each module's directory `dir` the package holds parts of under `deps/`, an index
/// of its kinds and of the dependencies it holds parts of in turn (`deps/pricing/index.ts`,
/// `<name>/deps/pricing/__init__.py`), each kind's index of its modules (glue/'s is kaname's), and
/// for each directory with dependencies under it, an index of them (`deps/index.ts`). Go's packages
/// are their directories, and have none.
fn dependency_indexes(target: Target, o: &Options, kinds: &BTreeMap<String, BTreeMap<&str, Vec<String>>>) -> Vec<(String, String, Option<String>)> {
    let head = |c: Comment| c.line(&ritsu_emit::header::generated("ritsu"));
    let mut out = Vec::new();
    // the dependencies of each module's directory the package holds parts of, by their names
    let mut below: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for d in kinds.keys() {
        if let Some(up) = parent_module_dir(d) {
            let name = ritsu_emit::package::chain(d).last().copied().unwrap_or_default();
            below.entry(up).or_default().push(name);
        }
    }
    let base = if target == Target::Python { format!("{}/", o.name) } else { String::new() };
    for (dir, names) in &below {
        match target {
            Target::TypeScript => {
                let mut index = format!("{}// The dependencies of the package whose parts it holds, each by its name.\n\n", head(Comment::Slashes));
                for n in names {
                    index.push_str(&format!("export * as {n} from \"./{n}/index\";\n"));
                }
                out.push((format!("{dir}deps/index.ts"), index, None));
            }
            Target::Python => {
                let mut init = format!("{}\"\"\"The dependencies of the package whose parts it holds, each by its name.\"\"\"\n\n", head(Comment::Hash));
                for n in names {
                    init.push_str(&format!("from . import {n} as {n}\n"));
                }
                out.push((format!("{base}{dir}deps/__init__.py"), init, None));
            }
            Target::Go => {}
        }
    }
    for (dir, ks) in kinds.iter().filter(|(d, _)| !d.is_empty()) {
        let prefix = ritsu_emit::package::chain(dir).iter().map(|n| format!("@{n}")).collect::<Vec<_>>().join("/");
        let what = format!("The parts of the dependency `{prefix}` the package holds, by their kinds.");
        // its kinds and its dependencies, in the order of their names
        let mut entries: Vec<&str> = ks.keys().copied().collect();
        if below.contains_key(dir.as_str()) {
            entries.push("deps");
        }
        entries.sort();
        match target {
            Target::TypeScript => {
                let mut index = format!("{}// {what}\n\n", head(Comment::Slashes));
                for k in &entries {
                    index.push_str(&format!("export * as {k} from \"./{k}/index\";\n"));
                }
                out.push((format!("{dir}index.ts"), index, None));
                for (kind, modules) in ks.iter().filter(|(k, _)| **k != "glue") {
                    let mut sub = format!("{}// The {kind} of the dependency `{prefix}`, each by its name.\n\n", head(Comment::Slashes));
                    for m in modules {
                        let path = if *kind == "flows" { format!("./{m}/client") } else { format!("./{m}") };
                        sub.push_str(&format!("export * as {m} from \"{path}\";\n"));
                    }
                    out.push((format!("{dir}{kind}/index.ts"), sub, None));
                }
            }
            Target::Python => {
                let mut init = format!("{}\"\"\"{what}\"\"\"\n\n", head(Comment::Hash));
                for k in &entries {
                    init.push_str(&format!("from . import {k} as {k}\n"));
                }
                out.push((format!("{base}{dir}__init__.py"), init, None));
                for (kind, modules) in ks.iter().filter(|(k, _)| **k != "glue") {
                    let mut sub = format!("{}\"\"\"The {kind} of the dependency `{prefix}`, each by its name.\"\"\"\n\n", head(Comment::Hash));
                    for m in modules {
                        sub.push_str(&format!("from . import {m} as {m}\n"));
                    }
                    out.push((format!("{base}{dir}{kind}/__init__.py"), sub, None));
                }
            }
            Target::Go => {}
        }
    }
    out
}

/// What the package itself is made of: its index, its manifest with what it depends on, and the
/// package files of Python; and in a module, the indexes of the dependencies' parts.
fn around(target: Target, o: &Options, all: &BTreeMap<String, BTreeMap<&str, Vec<String>>>, files: &[(String, String, Option<String>)]) -> Vec<(String, String, Option<String>)> {
    let head = |c: Comment| c.line(&ritsu_emit::header::generated("ritsu"));
    let none = BTreeMap::new();
    let kinds = all.get("").unwrap_or(&none);
    // the module at hand's index names its dependencies' parts too
    let deps = all.keys().any(|d| !d.is_empty());
    let (glue, authz) = (kinds.contains_key("glue"), kinds.contains_key("authz"));
    let what = match (glue, authz) {
        (false, false) => "The rules, the dates, the clients of the books and the workflows of a project, as one package (ritsu gen).",
        (false, true) => "The rules, the dates, the clients of the books, the workflows and the code that asks the gates of a project, as one package (ritsu gen).",
        (true, false) => "The rules, the dates, the clients of the books, the workflows and the functional cores (kaname's modules) of a project, as one package (ritsu gen).",
        (true, true) => "The rules, the dates, the clients of the books, the workflows, the functional cores (kaname's modules) and the code that asks the gates of a project, as one package (ritsu gen).",
    };
    let generated = ritsu_emit::header::generated("ritsu");
    let mut out = Vec::new();
    match target {
        Target::TypeScript => {
            let mut index = format!("{}// {what}\n\n", head(Comment::Slashes));
            for kind in top(kinds, deps) {
                index.push_str(&format!("export * as {kind} from \"./{kind}/index\";\n"));
            }
            for (kind, modules) in kinds {
                // glue/'s index is kaname's
                if *kind == "glue" {
                    continue;
                }
                let mut sub = format!("{}// The {kind} of the package, each by its name.\n\n", head(Comment::Slashes));
                for m in modules {
                    // a flow's index is its client: how to start it, answer it and ask it
                    let path = if *kind == "flows" { format!("./{m}/client") } else { format!("./{m}") };
                    sub.push_str(&format!("export * as {m} from \"{path}\";\n"));
                }
                out.push((format!("{kind}/index.ts"), sub, None));
            }
            if files.iter().any(|(p, _, _)| p == "units.ts") {
                index.push_str("export * as units from \"./units\";\n");
            }
            out.push(("index.ts".to_string(), index, None));
            // what the TypeScript imports from outside the package; for a library, the libraries
            // too, by their ranges
            let mut deps: BTreeMap<String, &str> = BTreeMap::new();
            let mut peers: BTreeMap<String, String> = BTreeMap::new();
            for (path, body, _) in files.iter().filter(|(p, _, _)| p.ends_with(".ts")) {
                let _ = path;
                for line in body.lines() {
                    let Some(at) = line.find(" from \"") else { continue };
                    if !(line.starts_with("import ") || line.starts_with("export ")) {
                        continue;
                    }
                    let name = line[at + 7..].split('"').next().unwrap_or("");
                    if name.starts_with('.') || name.starts_with("node:") {
                        continue;
                    }
                    if let Some(r) = o.library.as_ref().and_then(|l| l.read_of(name, '/')) {
                        peers.insert(r.name.clone(), library::npm_range(&r.version));
                        continue;
                    }
                    if name.starts_with("@temporalio/") {
                        deps.insert(name.to_string(), TEMPORAL_TYPESCRIPT);
                    } else if name == "tigerbeetle-node" {
                        deps.insert(name.to_string(), TIGERBEETLE);
                    } else if name == "@cedar-policy/cedar-wasm" || name.starts_with("@cedar-policy/cedar-wasm/") {
                        deps.insert("@cedar-policy/cedar-wasm".to_string(), CEDAR_WASM);
                    } else if name == "@aws-sdk/client-verifiedpermissions" {
                        deps.insert(name.to_string(), AWS_SDK_VERIFIEDPERMISSIONS);
                    }
                }
            }
            let deps_json: Vec<String> = deps.iter().map(|(d, v)| format!("    {}: {}", ritsu_emit::lit::json(d), ritsu_emit::lit::json(v))).collect();
            let mut manifest = format!("{{\n  \"name\": {},\n  \"private\": true,\n  \"description\": {}", ritsu_emit::lit::json(&o.name), ritsu_emit::lit::json(&format!("{what} {generated}")));
            if !deps_json.is_empty() {
                manifest.push_str(&format!(",\n  \"dependencies\": {{\n{}\n  }}", deps_json.join(",\n")));
            }
            manifest.push_str("\n}\n");
            match &o.library {
                None => out.push(("package.json".to_string(), manifest, None)),
                Some(l) => {
                    out.push(("package.json".to_string(), library::package_json(l, what, &generated, &deps, &peers), None));
                    out.push(("tsconfig.json".to_string(), library::tsconfig(&head(Comment::Slashes)), None));
                    let bodies = files.iter().filter(|(p, _, _)| p.ends_with(".ts")).map(|(_, b, _)| b.as_str());
                    let known = |name: &str| deps.keys().any(|d| name == d || name.starts_with(&format!("{d}/"))) || l.read_of(name, '/').is_some();
                    let optional = library::optional_modules(bodies, known);
                    if !optional.is_empty() {
                        out.push(("optional-modules.d.ts".to_string(), library::optional_modules_dts(&head(Comment::Slashes), &optional), None));
                    }
                }
            }
        }
        Target::Python => {
            let n = &o.name;
            let mut init = format!("{}\"\"\"{what}\"\"\"\n\n", head(Comment::Hash));
            for kind in top(kinds, deps) {
                init.push_str(&format!("from . import {kind} as {kind}\n"));
            }
            if files.iter().any(|(p, _, _)| *p == format!("{n}/units.py")) {
                init.push_str("from . import units as units\n");
            }
            out.push((format!("{n}/__init__.py"), init, None));
            out.push((format!("{n}/py.typed"), String::new(), None));
            for (kind, modules) in kinds.iter().filter(|(k, _)| **k != "glue") {
                let mut sub = format!("{}\"\"\"The {kind} of the package, each by its name.\"\"\"\n\n", head(Comment::Hash));
                for m in modules {
                    sub.push_str(&format!("from . import {m} as {m}\n"));
                }
                out.push((format!("{n}/{kind}/__init__.py"), sub, None));
            }
            let flows = files.iter().any(|(p, _, _)| p.contains("/flows/"));
            let tigerbeetle = o.books == Books::TigerBeetle && kinds.contains_key("books");
            // what the code of the gates asks with: Cedar's binding, or boto3's client of Verified
            // Permissions, which the caller makes and hands in
            let gates = files.iter().any(|(p, _, _)| p.starts_with(&format!("{n}/authz/")) && p.ends_with(".py"));
            let mut deps: Vec<(&str, &str)> = Vec::new();
            match (gates, o.authorizer) {
                (true, sekisho::r#gen::Authorizer::Cedar) => deps.push(("cedarpy", CEDARPY)),
                (true, sekisho::r#gen::Authorizer::Avp) => deps.push(("boto3", BOTO3)),
                _ => {}
            }
            if flows {
                deps.push(("temporalio", TEMPORAL_PYTHON));
            }
            if tigerbeetle {
                deps.push(("tigerbeetle", TIGERBEETLE));
            }
            if let Some(l) = &o.library {
                // as ranges from the versions tested, with the libraries its Python imports
                let mut wanted: Vec<(String, String)> = deps.iter().map(|(d, v)| (d.to_string(), library::python_range(&library::version_of(v)).unwrap_or_default())).collect();
                for (_, body, _) in files.iter().filter(|(p, _, _)| p.ends_with(".py")) {
                    for line in body.lines().map(str::trim_start) {
                        let module = line.strip_prefix("from ").or_else(|| line.strip_prefix("import ")).and_then(|r| r.split_whitespace().next()).unwrap_or("");
                        if let Some(r) = l.read_of(module, '.')
                            && !wanted.iter().any(|(n, _)| *n == r.name)
                        {
                            wanted.push((r.name.clone(), library::python_range(&r.version).unwrap_or_default()));
                        }
                    }
                }
                wanted.sort();
                out.push(("pyproject.toml".to_string(), library::pyproject(l, &head(Comment::Hash), what, &wanted), None));
            } else {
                let deps_toml: String = deps.iter().map(|(d, v)| format!("  {},\n", ritsu_emit::lit::json(&format!("{d}=={v}")))).collect();
                out.push((
                    "pyproject.toml".to_string(),
                    format!(
                        "{}# {what}\n\n[project]\nname = {}\nversion = \"0.0.0\"\nrequires-python = \">=3.10\"\ndependencies = [\n{deps_toml}]\n\n[build-system]\nrequires = [\"setuptools>=68\"]\nbuild-backend = \"setuptools.build_meta\"\n\n[tool.setuptools.packages.find]\ninclude = [{}, {}]\n\n[tool.setuptools.package-data]\n\"*\" = [\"py.typed\", \"*.sql\"]\n",
                        head(Comment::Hash),
                        ritsu_emit::lit::json(n),
                        ritsu_emit::lit::json(n),
                        ritsu_emit::lit::json(&format!("{n}.*"))
                    ),
                    None,
                ));
            }
        }
        Target::Go => {
            // the modules the package's Go imports, as doc.go names them; for a library, the
            // libraries too, at the versions the tree chooses
            let mut imported: BTreeSet<&str> = BTreeSet::new();
            let mut libraries: BTreeMap<String, String> = BTreeMap::new();
            for (_, body, _) in files.iter().filter(|(p, _, _)| p.ends_with(".go")) {
                for line in body.lines() {
                    let l = line.trim();
                    let path = l.rsplit('"').nth(1).unwrap_or("");
                    if !l.ends_with('"') || path.is_empty() {
                        continue;
                    }
                    if let Some(r) = o.library.as_ref().and_then(|l| l.read_of(path, '/')) {
                        libraries.insert(r.name.clone(), format!("v{}", r.version));
                    } else if let Some((m, ..)) = GO_MODULES.iter().filter(|(m, ..)| path == *m || path.starts_with(&format!("{m}/"))).max_by_key(|(m, ..)| m.len()) {
                        imported.insert(m);
                    }
                }
            }
            let package = match &o.library {
                Some(l) => l.go_package.clone(),
                None => go_package_name(&o.module, &o.name),
            };
            let mut doc = match (glue, authz) {
                (false, true) => format!("{}\n// Package {package} is the rules, the dates, the clients of the books, the workflows and the code that\n// asks the gates of a project, as one package (ritsu gen): rules/, dates/, books/, flows/ and authz/ under\n// the import path {}.\n", head(Comment::Slashes), o.module),
                (false, false) => format!("{}\n// Package {package} is the rules, the dates, the clients of the books and the workflows of a project, as\n// one package (ritsu gen): rules/, dates/, books/ and flows/ under the import path {}.\n", head(Comment::Slashes), o.module),
                (true, false) => format!("{}\n// Package {package} is the rules, the dates, the clients of the books, the workflows and the functional\n// cores (kaname's modules) of a project, as one package (ritsu gen): rules/, dates/, books/, flows/ and\n// glue/ under the import path {}.\n", head(Comment::Slashes), o.module),
                (true, true) => format!("{}\n// Package {package} is the rules, the dates, the clients of the books, the workflows, the functional cores\n// (kaname's modules) and the code that asks the gates of a project, as one package (ritsu gen): rules/,\n// dates/, books/, flows/, glue/ and authz/ under the import path {}.\n", head(Comment::Slashes), o.module),
            };
            if files.iter().any(|(p, _, _)| p == "units/units.go") {
                doc.push_str("//\n// The types of the units its rules and books take amounts in are in units/, one of each unit.\n");
            }
            if !imported.is_empty() && o.library.is_none() {
                doc.push_str("//\n// It imports these modules, which the go.mod of the module it is in requires (`go mod tidy`); these are\n// the versions it is tested with:\n//\n");
                for m in &imported {
                    let v = GO_MODULES.iter().find(|(x, ..)| x == m).map(|(_, v, _)| *v).unwrap_or("");
                    doc.push_str(&format!("//\t{m} {v}\n"));
                }
                let raised: Vec<_> = GO_RAISED.iter().filter(|(by, ..)| imported.contains(by)).collect();
                if !raised.is_empty() {
                    doc.push_str("//\n// Of the modules those require, these are tested at a later version than the one asked for, which\n// has a known vulnerability; raise them in the go.mod (`go get <module>@<version>`):\n//\n");
                    for (by, m, v, advisory, _) in raised {
                        doc.push_str(&format!("//\t{m} {v} ({advisory}, through {by})\n"));
                    }
                }
            }
            doc.push_str(&format!("package {package}\n"));
            out.push(("doc.go".to_string(), doc, None));
            if let Some(l) = &o.library {
                // its go.mod: what it imports, at the versions tested and the ones the tree chooses,
                // and what those ask for at a version with a known vulnerability, raised
                let tested = |m: &str| GO_MODULES.iter().find(|(x, ..)| *x == m);
                let mut requires: Vec<(String, String)> = imported.iter().filter_map(|m| tested(m)).map(|(m, v, _)| (m.to_string(), v.to_string())).collect();
                requires.extend(libraries.iter().map(|(m, v)| (m.clone(), v.clone())));
                requires.sort();
                let raised: Vec<_> = GO_RAISED.iter().filter(|(by, ..)| imported.contains(by)).collect();
                let asked = imported.iter().filter_map(|m| tested(m)).map(|(.., g)| *g).chain(raised.iter().map(|(.., g)| *g));
                let go = library::go_line(GO_FLOOR, asked);
                let raised: Vec<(String, String)> = raised.iter().map(|(_, m, v, ..)| (m.to_string(), v.to_string())).collect();
                out.push(("go.mod".to_string(), library::go_mod(l, &head(Comment::Slashes), go, &requires, &raised), None));
            }
        }
    }
    out.extend(dependency_indexes(target, o, all));
    out
}

/// The names the index of the module at hand exports, in order: its kinds, and `deps` when the
/// package holds parts of its dependencies.
fn top<'a>(kinds: &BTreeMap<&'a str, Vec<String>>, deps: bool) -> Vec<&'a str> {
    let mut out: Vec<&str> = kinds.keys().copied().collect();
    if deps {
        out.push("deps");
        out.sort();
    }
    out
}

/// The name of the Go package of the directory at `module`: its last part, as a Go identifier. A
/// last part Go keeps as a word cannot name a package: the directory `<out>/go`, put in a module as
/// it is, would be `package go`, which no Go compiler takes. The package then takes the package's
/// name (`name`, the `--name` of all three languages), or, when Go keeps that too, the last part with
/// a `p` before it, as a last part that does not begin with a letter has.
fn go_package_name(module: &str, name: &str) -> String {
    let last = module.rsplit('/').next().unwrap_or(module);
    let s: String = last.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    let kept = |n: &str| ritsu_emit::words::go::KEYWORDS.contains(&n);
    match s.chars().next() {
        Some(c) if c.is_ascii_alphabetic() && !kept(&s) => s,
        Some(c) if c.is_ascii_alphabetic() && !kept(name) => name.to_string(),
        _ => format!("p{s}"),
    }
}

/// A package's name, as all three of its languages take it.
fn good_name(n: &str) -> bool {
    !n.is_empty() && n.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') && !ritsu_emit::words::python::KEYWORDS.contains(&n)
}

/// A Go import path, as a go.mod takes one.
fn good_module(m: &str) -> bool {
    !m.is_empty() && !m.starts_with('/') && !m.ends_with('/') && m.split('/').all(|p| !p.is_empty() && p != "." && p != ".." && p.chars().all(|c| c.is_ascii_alphanumeric() || "-._~".contains(c)))
}

/// What a write or a check came to: each state's files as the text lists them, and every file
/// of the packages with its state, for the JSON.
#[derive(Default)]
struct Emitted {
    written: Vec<String>,
    removed: Vec<String>,
    stale: Vec<String>,
    missing: Vec<String>,
    left: Vec<String>,
    files: Vec<Emit>,
}

/// One file of a package, as the JSON says it.
struct Emit {
    /// `typescript`, `python`, `go`, or `cedar` for the gates' Cedar beside them.
    target: &'static str,
    /// The path as the text says it: under `--out`, from where ritsu runs.
    path: String,
    /// The file of the project it is made from, from the root; None for what the package itself
    /// is made of (the index, the manifest) and for a file that is generated no more.
    from: Option<String>,
    /// `written`, `same` or `removed` when ritsu writes; `same`, `missing`, `stale` or `left` under
    /// `--check`.
    state: &'static str,
}

impl Emitted {
    fn note(&mut self, target: &'static str, path: &str, from: &Option<String>, state: &'static str) {
        self.files.push(Emit { target, path: path.to_string(), from: from.clone(), state });
    }
}

/// The files of the package's directory that ritsu wrote before (their head says so) and this run
/// does not write: what a rule taken out of the project left behind. A library's directory leaves
/// out what its build writes beside the package (DESIGN 17.16): tsc's `dist/`, whose files keep the
/// head, setuptools's `build/` and `*.egg-info`, and a `go.sum`.
fn leftovers(dir: &Path, planned: &BTreeSet<PathBuf>, library: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        let Ok(rd) = ritsu_base::fs::read_dir(&d) else { continue };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "node_modules" || name == "__pycache__" {
                continue;
            }
            if library && d.as_path() == dir && (name == "dist" || name == "build" || name.ends_with(".egg-info") || name == "go.sum") {
                continue;
            }
            if p.is_dir() {
                todo.push(p);
            } else if !planned.contains(&p) && generated_here(&p) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Whether a file begins with the head of a file ritsu's generators write (DESIGN 9.2).
fn generated_here(p: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(p) else { return false };
    text.lines().take(3).any(|l| ["rulec", "koyomi", "chobo", "dandori", "kaname", "sekisho", "ritsu"].iter().any(|t| l.contains(&format!("Code generated by {t} ")) && l.ends_with("DO NOT EDIT.")))
}

/// Write the package into `dir`, or under `check` say what differs from it.
fn emit(dir: &Path, target: &'static str, p: &Package, check: bool, library: bool, st: &mut Emitted, lang: Lang) -> Result<(), Refusal> {
    let planned: BTreeSet<PathBuf> = p.files.iter().map(|(rel, _, _)| dir.join(rel)).collect();
    for (rel, body, from) in &p.files {
        let path = dir.join(rel);
        let shown = path.display().to_string();
        let existing = std::fs::read(&path).ok();
        if existing.as_deref() == Some(body.as_bytes()) {
            st.note(target, &shown, from, "same");
            continue;
        }
        if check {
            if existing.is_none() {
                st.note(target, &shown, from, "missing");
                st.missing.push(shown);
            } else {
                st.note(target, &shown, from, "stale");
                st.stale.push(shown);
            }
            continue;
        }
        if let Some(parent) = path.parent()
            && std::fs::create_dir_all(parent).is_err()
        {
            let d = parent.display().to_string();
            return Err(refusal(2, tr!("`{d}` を作れません", "cannot create `{d}`"), lang));
        }
        if std::fs::write(&path, body).is_err() {
            return Err(refusal(2, tr!("`{shown}` に書けません", "cannot write `{shown}`"), lang));
        }
        st.note(target, &shown, from, "written");
        st.written.push(shown);
    }
    for p in leftovers(dir, &planned, library) {
        let shown = p.display().to_string();
        if check {
            st.note(target, &shown, &None, "left");
            st.left.push(shown);
        } else if std::fs::remove_file(&p).is_ok() {
            st.note(target, &shown, &None, "removed");
            st.removed.push(shown);
        }
    }
    Ok(())
}

/// Whether the line asks for JSON (`--format json`, `--format=json`): read before the line is, so
/// that a line that does not read is answered in JSON too.
fn asks_json(args: &[String]) -> bool {
    args.iter().enumerate().any(|(i, x)| x == "--format=json" || (x == "--format" && args.get(i + 1).is_some_and(|v| v == "json")))
}

/// What `ritsu gen --format json` prints (DESIGN 8.5, 9.3): one object, its first keys those of
/// `ritsu check --format json` — the version, the root as seen from where ritsu runs (null when
/// no project was read), whether all is well (the exit code is 0) — then whether it was `--check`,
/// `--out` and the languages asked for, every file of the packages with its state, the
/// diagnostics that stopped it or warn (each as `ritsu check --format json` writes one), and why
/// nothing was generated, with the exit code (null when something was).
struct Answer {
    root: Option<String>,
    check: bool,
    out: String,
    targets: Vec<&'static str>,
    files: Vec<Emit>,
    diagnostics: Vec<Json>,
    error: Option<(u8, String)>,
    /// No JSON is written: the modules of the tree could not be read, and what to run was said on
    /// the standard error (DESIGN 17.8).
    silent: bool,
}

impl Answer {
    fn json(&self, code: u8) -> Json {
        let file = |e: &Emit| {
            Json::obj([
                ("target", Json::str(e.target)),
                ("path", Json::str(&e.path)),
                ("from", e.from.as_deref().map(Json::str).unwrap_or(Json::Null)),
                ("state", Json::str(e.state)),
            ])
        };
        let error = match &self.error {
            Some((c, m)) => Json::obj([("code", Json::int(i128::from(*c))), ("message", Json::str(m))]),
            None => Json::Null,
        };
        Json::obj([
            ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
            ("root", self.root.as_deref().map(Json::str).unwrap_or(Json::Null)),
            ("ok", Json::Bool(code == 0)),
            ("check", Json::Bool(self.check)),
            ("out", Json::str(&self.out)),
            ("targets", Json::arr(self.targets.iter().map(|t| Json::str(*t)))),
            ("files", Json::arr(self.files.iter().map(file))),
            ("diagnostics", Json::arr(self.diagnostics.iter().cloned())),
            ("error", error),
        ])
    }
}

/// The options of one language's package: for a library, its name in that language (the npm
/// package's, the Python package's, the Go package of its root with the module's path) and the
/// library itself.
fn for_target(o: &Options, libraries: &[(Target, library::Library)], t: Target) -> Options {
    match libraries.iter().find(|(lt, _)| *lt == t) {
        None => o.clone(),
        Some((_, l)) => {
            let name = if t == Target::Go { l.go_package.clone() } else { l.import.clone() };
            Options { name, module: l.import.clone(), library: Some(l.clone()), ..o.clone() }
        }
    }
}

/// `ritsu gen [<path>...] [--target typescript|python|go] [--out <dir>] [--check] [--books
/// postgres|tigerbeetle] [--authorizer cedar|avp] [--name <name>] [--module <path>] [--library
/// [--embed <name>]...] [--root <dir>] [--format json]`: the exit code.
pub fn command(args: &[String], lang: Lang) -> u8 {
    let table = cli::table();
    let cmd = table.command("gen").expect("gen is in the table");
    let asked = args.iter().enumerate().find_map(|(i, x)| if x == "--lang" { args.get(i + 1).cloned() } else { x.strip_prefix("--lang=").map(str::to_string) });
    let lang = if asked.is_some() { Lang::pick(asked.as_deref(), "RITSU_LANG") } else { lang };
    let json = asks_json(args);
    let mut answer = Answer { root: None, check: args.iter().any(|a| a == "--check"), out: "generated".to_string(), targets: Vec::new(), files: Vec::new(), diagnostics: Vec::new(), error: None, silent: false };
    let code = generate(args, &table, cmd, lang, json, &mut answer);
    if json && !answer.silent {
        println!("{}", answer.json(code).pretty());
    }
    code
}

/// [`command`], the answer kept for the JSON as it goes.
fn generate(args: &[String], table: &ritsu_base::cli::Table, cmd: &ritsu_base::cli::Cmd, lang: Lang, json: bool, answer: &mut Answer) -> u8 {
    // Nothing generated: the text says why on the standard error; the JSON keeps it.
    let say = |r: Refusal, answer: &mut Answer| -> u8 {
        if json {
            answer.diagnostics.extend(r.diagnostics);
            answer.error = Some((r.code, r.message));
        } else {
            eprint!("{}", r.said);
        }
        r.code
    };
    let a = match table.parse(cmd, args) {
        Ok(a) => a,
        Err(e) => return say(refusal(2, e, lang), answer),
    };
    if a.has("--help") {
        print!("{}", table.help_cmd(cmd, lang));
        return 0;
    }
    answer.out = a.get("--out").unwrap_or("generated").to_string();
    // `--library` (DESIGN 17.16): the whole module, named by its ritsu.mod
    let library = a.has("--library");
    let embed: Vec<String> = a.all("--embed").into_iter().map(str::to_string).collect();
    if !embed.is_empty() && !library {
        return say(refusal(2, tr!("`--embed` は `--library` と一緒に使います", "`--embed` goes with `--library`"), lang), answer);
    }
    if library && !a.pos.is_empty() {
        return say(
            refusal(
                2,
                tr!(
                    "`--library` は、モジュール全体（`exclude` の行が外すものを除く）をライブラリにします。パスを渡さずに走らせてください",
                    "`--library` makes a library of the whole module (but what its `exclude` lines leave out): run it with no path"
                ),
                lang,
            ),
            answer,
        );
    }
    if library && (a.has("--name") || a.has("--module")) {
        return say(
            refusal(
                2,
                tr!(
                    "`--library` には `--name` も `--module` も付けられません。ライブラリの名前は、ritsu.mod の `library` の行から取ります",
                    "`--name` and `--module` do not go with `--library`: a library is named by the `library` lines of ritsu.mod"
                ),
                lang,
            ),
            answer,
        );
    }
    let name = a.get("--name").unwrap_or("generated").to_string();
    if !good_name(&name) {
        return say(
            refusal(
                2,
                tr!(
                    "`--name {name}` は使えません。名前は小文字の英字で始め、小文字の英字、数字、`_` だけで書いてください（Python の予約語は使えません）",
                    "`--name {name}` cannot name a package: it starts with a lowercase letter and has only lowercase letters, digits and `_` (and is not a word Python keeps)"
                ),
                lang,
            ),
            answer,
        );
    }
    let module = a.get("--module").unwrap_or(&name).to_string();
    if !good_module(&module) {
        return say(refusal(2, tr!("`--module {module}` は Go の import のパスになりません", "`--module {module}` is not a Go import path"), lang), answer);
    }
    let books = match a.get("--books") {
        Some("tigerbeetle") => Books::TigerBeetle,
        Some("mysql") => Books::MySql,
        _ => Books::Postgres,
    };
    let authorizer = a.get("--authorizer").and_then(sekisho::r#gen::Authorizer::parse).unwrap_or(sekisho::r#gen::Authorizer::Cedar);
    let targets: Vec<Target> = match a.get("--target") {
        Some(t) => Target::parse(t).into_iter().collect(),
        None => Target::ALL.to_vec(),
    };
    answer.targets = targets.iter().map(|t| t.key()).collect();
    let project = match Project::load(&a.pos, a.get("--root")) {
        Ok(p) => p,
        Err(e) => return say(refusal(2, e, lang), answer),
    };
    answer.root = Some(project.root_shown());
    // In a module, the tree first, as `ritsu check` reads it (DESIGN 17.8): what stops the run is said
    // on the standard error, with no JSON (exit 2); the tree's errors are said, and nothing is
    // generated (exit 1).
    match project.tree.clone().as_deref() {
        Some(Err(Problem::Stop(said))) => {
            let head = if lang == Lang::Ja { "エラー" } else { "error" };
            for t in said {
                eprintln!("{head}: {}", t.get(lang));
            }
            answer.silent = true;
            return 2;
        }
        Some(Err(Problem::Diags(ds))) => {
            let found: Vec<Finding> = ds.iter().map(|d| crate::check::modules::finding(&project, d, lang)).collect();
            let text: String = found.iter().map(|f| f.text_of("ritsu")).collect();
            let n = found.iter().filter(|f| f.severity == Severity::Error).count();
            let errors = ritsu_base::text::plural(n, "error", "errors");
            let why = tr!("モジュールを読めません（エラー {n} 件）。何も生成しません", "the modules could not be read ({errors}), so nothing is generated");
            let r = refusal(1, why.clone(), lang);
            let diagnostics = found.iter().map(|f| crate::check::entry("ritsu", f)).collect();
            return say(Refusal { said: format!("{text}{}", r.said), diagnostics, ..r }, answer);
        }
        _ => {}
    }
    // a library: the languages of its `library` lines, and its module's files but what it leaves out
    let (project, targets) = if library {
        match library::targets(&project, a.get("--target"), lang) {
            Ok(ts) => {
                answer.targets = ts.iter().map(|t| t.key()).collect();
                (library::whole_module(project), ts)
            }
            Err(r) => return say(r, answer),
        }
    } else {
        (project, targets)
    };
    if [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Dandori, Tool::Kaname, Tool::Sekisho].iter().all(|t| project.of(*t).is_empty()) {
        let given = project.given.join(" ");
        return say(
            refusal(
                2,
                tr!(
                    "{given} には、パッケージにするファイル（.rule、.cal、.book、.flow、.kn、.gate）がありません",
                    "there is no file to make a package of (.rule, .cal, .book, .flow, .kn, .gate) in {given}"
                ),
                lang,
            ),
            answer,
        );
    }
    let joined = Joined::new();
    // The contributions are merged from the rulec files `ritsu check` reads for the same paths: in a
    // check of a whole module, every module's of the tree (DESIGN 17.8). They are set before any
    // rule is read.
    let whole = project.clone().with_dependencies();
    let contributions: Vec<String> = whole.of(Tool::Rulec).iter().map(|f| f.shown.clone()).collect();
    joined.rulec.set_project(contributions.clone());
    // In a module, the files of the dependencies the files given reach go into the package with them.
    let project = project.reached(&joined);
    // what the checks across the borders come to at the calls of the modules of kaname, once
    let verdicts = Rc::new(ritsu_cross::ProgramVerdicts::new(&joined));
    if let Err(r) = checked(&project, &whole, &joined, &verdicts, lang) {
        return say(r, answer);
    }
    let modules = match kaname_modules(&project, &joined, &verdicts, lang) {
        Ok(m) => m,
        Err(r) => return say(r, answer),
    };
    // what stops the packages of a module for the modules' sake, before any target is written
    if let Err(r) = modules_hold(&project, &joined, &targets, lang) {
        return say(r, answer);
    }
    // each language's library: where it reads each module, and what it says it depends on
    let libraries = if library {
        match library::plan(&project, &whole, &joined, &verdicts, &contributions, &targets, &embed, lang) {
            Ok(l) => l,
            Err(r) => return say(r, answer),
        }
    } else {
        Vec::new()
    };
    let o = Options { name, module, books, authorizer, lang, library: None };
    let out = PathBuf::from(&answer.out);
    let check = a.has("--check");
    let mut st = Emitted::default();
    for t in targets {
        let o = for_target(&o, &libraries, t);
        let p = match package(&project, &contributions, &joined, &modules, t, &o) {
            Ok(p) => p,
            Err(r) => return say(r, answer),
        };
        // two files of the project that would write one file of the package
        let mut by: BTreeMap<&str, &Option<String>> = BTreeMap::new();
        for (rel, _, from) in &p.files {
            if let Some(prev) = by.insert(rel.as_str(), from) {
                let (a1, b1) = (prev.clone().unwrap_or_default(), from.clone().unwrap_or_default());
                let at = format!("{}/{rel}", t.key());
                return say(
                    refusal(
                        2,
                        tr!(
                            "`{a1}` と `{b1}` が、パッケージの同じ {at} を書きます。どちらかの名前を変えてください（規則と日付のファイルは別名、帳簿は名前、ワークフローは名前かファイルの名前を ASCII で）",
                            "`{a1}` and `{b1}` both write {at} of the package; give one of them another name (a rule or a dates file another alias, a book another name, a workflow a name or a file name in ASCII)"
                        ),
                        lang,
                    ),
                    answer,
                );
            }
        }
        let emitted = emit(&out.join(t.key()), t.key(), &p, check, o.library.is_some(), &mut st, lang);
        answer.files.append(&mut st.files);
        if let Err(r) = emitted {
            return say(r, answer);
        }
    }
    // the gates' Cedar, once beside the packages, whatever the targets
    if !project.of(Tool::Sekisho).is_empty() || out.join("cedar").exists() {
        let (p, warnings, warned) = match cedar(&project, &joined, authorizer, lang) {
            Ok(p) => p,
            Err(r) => return say(r, answer),
        };
        if json {
            answer.diagnostics.extend(warned);
        } else {
            print!("{warnings}");
        }
        let emitted = emit(&out.join("cedar"), "cedar", &p, check, false, &mut st, lang);
        answer.files.append(&mut st.files);
        if let Err(r) = emitted {
            return say(r, answer);
        }
    }
    if !json {
        let mut stdout = std::io::stdout();
        for p in &st.written {
            let _ = writeln!(stdout, "{}", tr!("生成しました: {p}", "generated: {p}").get(lang));
        }
        for p in &st.removed {
            let _ = writeln!(stdout, "{}", tr!("もう生成しないので消しました: {p}", "removed, as it is generated no more: {p}").get(lang));
        }
        for p in &st.missing {
            let _ = writeln!(stdout, "{}", tr!("ありません: {p}", "missing: {p}").get(lang));
        }
        for p in &st.stale {
            let _ = writeln!(stdout, "{}", tr!("生成物が古いか手で編集されています: {p}", "generated file is stale or hand-edited: {p}").get(lang));
        }
        for p in &st.left {
            let _ = writeln!(stdout, "{}", tr!("もう生成しないファイルが残っています: {p}", "generated no more, and still there: {p}").get(lang));
        }
        // the ritsu that wrote the lock is a record (DESIGN 17.4): when it is not this one, what
        // differs may be what the two versions write
        let differs = !(st.missing.is_empty() && st.stale.is_empty() && st.left.is_empty());
        if check
            && differs
            && let Some(Ok(tree)) = project.tree.as_deref()
            && let Some(lock) = &tree.lock
            && lock.ritsu != ritsu_base::modules::graph::running()
        {
            let (then, now) = (lock.ritsu.to_string(), env!("CARGO_PKG_VERSION"));
            let _ = writeln!(
                stdout,
                "{}",
                tr!(
                    "備考: ritsu.lock を書いた ritsu は {then} で、いまの ritsu は {now} です。生成物の違いは、版の違いから来ているかもしれません",
                    "note: ritsu.lock was written by ritsu {then}, and this is ritsu {now}: what differs may come from the two versions"
                )
                .get(lang)
            );
        }
        let _ = stdout.flush();
    }
    u8::from(check && !(st.missing.is_empty() && st.stale.is_empty() && st.left.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `go` line of each Go module a package imports, and of each raised one, is the one its
    /// go.mod says at that version, read from the module cache the tools level fills (from the
    /// go.mod of dandori's tools/temporal-go, chobo's tools/runner/go and sekisho's
    /// tools/runner-go): a library's go.mod is at least the highest of those its Go imports.
    #[test]
    fn the_go_lines_are_those_of_the_modules() {
        let go = || std::process::Command::new("go").arg("version").output().is_ok_and(|o| o.status.success());
        if !ritsu_testkit::ready(ritsu_testkit::Need::Go, go, "go is not installed; the go lines of the Go modules a package imports are not read back") {
            return;
        }
        let cache = std::process::Command::new("go").args(["env", "GOMODCACHE"]).output().unwrap();
        let cache = PathBuf::from(String::from_utf8_lossy(&cache.stdout).trim());
        // a module path's capitals are written `!` and the small letter in the cache
        let escaped = |m: &str| m.chars().map(|c| if c.is_ascii_uppercase() { format!("!{}", c.to_ascii_lowercase()) } else { c.to_string() }).collect::<String>();
        let raised = GO_RAISED.iter().map(|(_, m, v, _, go)| (*m, *v, *go));
        let mut wrong = Vec::new();
        for (m, v, go) in GO_MODULES.iter().copied().chain(raised) {
            let file = cache.join("cache/download").join(escaped(m)).join("@v").join(format!("{v}.mod"));
            let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{m} {v} is not in the module cache ({}: {e})", file.display()));
            let line = text.lines().find_map(|l| l.trim().strip_prefix("go ")).map(str::trim);
            if line != Some(go) {
                wrong.push(format!("{m} {v}: go {}, not {go}", line.unwrap_or("(none)")));
            }
        }
        assert!(wrong.is_empty(), "the go lines are not those of the modules' go.mod:\n{}", wrong.join("\n"));
        println!("the go lines of {} Go modules are those of their go.mod", GO_MODULES.len() + GO_RAISED.len());
    }
}
