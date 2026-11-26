//! `ritsu gen` (DESIGN 9.3, 9.6): the rules, the dates, the clients of the books, the workflows, the
//! functional cores (kaname's modules) and the code that asks the gates of a project, as one package
//! for each language it is given (TypeScript, Python, Go, Rust), the gates' Cedar beside them,
//! written once, when it is given `cedar`, and the Lean of the modules of kaname, their definitions
//! and the statements of their properties, when it is given `lean` (kaname's DESIGN 38.4). What it
//! is given is `--target`'s, or else the `target` line of
//! the module's ritsu.mod (DESIGN 17.3); with neither, nothing is written. Every target given is
//! made before any is written, so a run that stops writes none of them (DESIGN 9.6). kaname's
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
//! generated/rust/                the crate of the submodule [`rust`], which holds no flows and no authz
//!   Cargo.toml, lib.rs
//!   rules.rs, rules/<alias>.rs
//!   dates.rs, dates/<alias>.rs
//!   books.rs, books/<book>.rs
//!   glue.rs, glue/<module>.rs, kaname.rs, door.rs
//!   glue/books.rs, glue/books/<book>.rs, <book>_apply.rs
//!   units.rs, wasm.rs, <name>.wit
//! generated/cedar/<gate>.cedar, .cedarschema, .cedarschema.json, .policies.json
//! generated/lean/                 lean-toolchain, lakefile.toml, .gitignore
//!   Kaname/Prelude.lean, Rules.lean, Books.lean, <module>.lean
//!   samples/<module>.lean
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

mod bundle;
mod kinds;
mod laid;
mod library;
mod rust;
mod wasm;
/// Public so that the tests read what it writes with other readers of zip.
pub mod zip;

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
    Rust,
}

impl Target {
    pub const ALL: [Target; 4] = [Target::TypeScript, Target::Python, Target::Go, Target::Rust];

    /// The value of `--target`, and the directory under `--out`.
    pub fn key(self) -> &'static str {
        match self {
            Target::TypeScript => "typescript",
            Target::Python => "python",
            Target::Go => "go",
            Target::Rust => "rust",
        }
    }

    fn parse(s: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.key() == s)
    }

    /// dandori's target for the workflow (Temporal, in this language); Rust has none.
    fn dandori(self) -> Option<&'static str> {
        match self {
            Target::TypeScript => Some("temporal-typescript"),
            Target::Python => Some("temporal-python"),
            Target::Go => Some("temporal-go"),
            Target::Rust => None,
        }
    }

    /// Whether the package holds the workflows and the code that asks the gates: dandori and sekisho
    /// write neither in Rust, so its package leaves them out, and `ritsu gen` says so (DESIGN 9.3).
    fn holds_flows_and_gates(self) -> bool {
        self != Target::Rust
    }
}

/// The target that is no language: the gates' Cedar, written once beside the packages, into
/// `<out>/cedar/` (DESIGN 9.3).
pub const CEDAR: &str = "cedar";

/// The other target that is no language: the Lean of the modules of kaname, their definitions and
/// the statements of their properties, which a proof beside each module proves (kaname's DESIGN
/// 38.4), written once into `<out>/lean/`. The other languages write nothing there: the rules and
/// the dates a module calls are given to it from outside (`Inner`).
pub const LEAN: &str = "lean";

/// The third target that is no language: the project's kinds of request apart from any language
/// (ringi's DESIGN 12.2), written once into `<out>/requests/`: the description of each kind
/// (`types/`), the server's API (`openapi.json`), an agent's tools (`mcp/tools.json`), the schema and
/// the columns of the organization's data (`organization/`), each kind's vectors (`vectors/`) and
/// those of its entry `worked` (`vectors/worked/`), the vectors of finding a person, a team or a row
/// of a master (`organization/find.jsonl`, apart from any kind), and each kind's screens with their
/// catalog and functions (`a2ui/`, ringi's DESIGN 13). The kinds are the requests whose code the
/// packages hold.
pub const REQUESTS: &str = "requests";

/// The extras `--extra` takes, beside `--target requests` (ringi's DESIGN 12.6), in the order they
/// are written whatever the order they are given in: `wasm`, the package of Rust built for
/// wasm32-unknown-unknown into `<out>/requests/kinds.wasm`, with `wasm.json`, how a server calls it;
/// `bundle`, every file of `<out>/requests/` with the files the kinds are made from in one zip,
/// `<out>/requests.zip`, which needs `wasm`.
pub const EXTRA_WORDS: [&str; 2] = ["wasm", "bundle"];

/// The names `--target` takes, in the order `ritsu gen` writes them whatever the order they are
/// given in (DESIGN 9.6): each language's, in [`Target::ALL`]'s order, then `cedar`, `lean` and
/// `requests`. They are the words of the `target` line of a ritsu.mod (ritsu-base's `TARGET_WORDS`,
/// which the table of `gen` takes its choices from): ritsu-base cannot read [`Target`], so a test
/// holds the two to each other.
pub fn target_names() -> Vec<&'static str> {
    Target::ALL.iter().map(|t| t.key()).chain([CEDAR, LEAN, REQUESTS]).collect()
}

/// The names given, in the order of [`target_names`]: the order they are written in, and said in.
fn in_order(given: &[&str]) -> Vec<&'static str> {
    target_names().into_iter().filter(|n| given.contains(n)).collect()
}

/// The languages `ritsu gen` wrote up to 0.40.0 when it was given no `--target` (DESIGN 9.6), and
/// the Cedar too when the project had gates or `<out>/cedar/` was there. Kept as 0.40.0 had it: a
/// target added later is not one of them.
pub const UP_TO_0_40: [&str; 3] = ["typescript", "python", "go"];

/// Where the books are kept: the client chobo writes for each (DESIGN 9.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Books {
    Postgres,
    TigerBeetle,
    /// MySQL, TiDB or Vitess (chobo's DESIGN 4.5)
    MySql,
}

impl Books {
    /// Where kaname's Rust makes the transfers (kaname's DESIGN 36.6).
    fn store(self) -> kaname::r#gen::Store {
        match self {
            Books::TigerBeetle => kaname::r#gen::Store::TigerBeetle,
            Books::Postgres | Books::MySql => kaname::r#gen::Store::Sql,
        }
    }

    fn chobo(self, t: Target) -> chobo::target::Target {
        use chobo::target::Target as C;
        match (self, t) {
            (Books::Postgres, Target::TypeScript) => C::PostgresTypeScript,
            (Books::Postgres, Target::Python) => C::PostgresPython,
            (Books::Postgres, Target::Go) => C::PostgresGo,
            (Books::TigerBeetle, Target::TypeScript) => C::TigerBeetleTypeScript,
            (Books::TigerBeetle, Target::Python) => C::TigerBeetlePython,
            (Books::TigerBeetle, Target::Go) => C::TigerBeetleGo,
            (Books::Postgres, Target::Rust) => C::PostgresRust,
            (Books::TigerBeetle, Target::Rust) => C::TigerBeetleRust,
            (Books::MySql, Target::Rust) => C::MySqlRust,
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
    /// The language of what the package's code says (its comments, and the sentences it answers
    /// with): `--lang`, else English, whatever the environment says (`Lang::made`, DESIGN 4.1).
    pub lang: Lang,
    /// The language of what the run prints while it makes the package (a refusal, the diagnostics
    /// that stop it): `--lang`, else the environment's (`Lang::pick`).
    pub said: Lang,
    /// The package as a library (`--library`, DESIGN 17.16): None for the app form.
    pub library: Option<library::Library>,
    /// The kinds of request whose code the package holds, for its list of them (ringi's DESIGN
    /// 12.2): none in a project with no request.
    pub kinds: Vec<ritsu_project::requests::Kind>,
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
    ("github.com/tigerbeetle/tigerbeetle-go", "v0.17.10", "1.17"),
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
    /// The files that are no text (the module of `--extra wasm`, the zip of `--extra bundle`), the
    /// same way; under `--check`, None for a file that is not what the run writes, whatever is there
    /// (a module that is not the build of the crate the run makes).
    pub bytes: Vec<(String, Option<Vec<u8>>, Option<String>)>,
    /// Whether its files are written into a directory other targets write into too (`<out>/` for
    /// the zip of `--extra bundle`): nothing else there is taken away.
    pub alone: bool,
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
    let message = said.get(lang).to_string();
    Refusal { said: format!("{}\n", ritsu_base::cli::refusal(&message, lang)), code, message, diagnostics: Vec::new() }
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

/// Whether `ritsu check` finds nothing that makes the code of the package wrong (DESIGN 9.3): every
/// file of the languages a package is made of passes its language's check (a flow's with the rules,
/// the dates files and the books joined, kaname's with what the checks across the borders came to at
/// its calls, as `ritsu check` checks them), and the checks across the languages, of the modules and
/// sakai's of the references that cross a boundary find no error; else the diagnostics of what does
/// not pass, as `ritsu check` prints them. yuen's errors and geas's claims do not stop it. What
/// rulec says in the check of one file of a line of another is filed under the file the line is in,
/// as `ritsu check` files it, in a module and out of one ([`Project::place_findings`], with the files `ritsu check` reads for the same paths,
/// `whole`), and a file an error is filed under does not pass either (DESIGN 17.8); a rule of a
/// dependency that fails only for such errors is not named among the files that fail
/// ([`Project::failed_for_others`]), as the line of the dependencies of `ritsu check` does not count
/// it.
fn checked(project: &Project, whole: &Project, joined: &Joined, verdicts: &Rc<ritsu_cross::ProgramVerdicts>, laid: Option<&ritsu_project::requests::Laid>, screens: Option<&ringi::screens::Given>, lang: Lang) -> Result<(), Refusal> {
    let mut all: Vec<(Tool, ritsu_ports::Checked)> = Vec::new();
    for tool in MADE_FROM {
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
            Tool::Sekisho => joined.sekisho.checked(&project.root, &files, lang),
            // with the rules, the dates files and the books joined, as `ritsu check` checks a flow
            Tool::Dandori => joined.dandori.checked_with(&project.root, &files, &joined.ports(), lang),
            _ => joined.chobo.checked(&project.root, &files, lang),
        };
        all.extend(units.into_iter().map(|u| (tool, u)));
    }
    // the requests, whose lowered files and glue go into the package (ringi's DESIGN 12.3): one that
    // does not pass stops the run as any file the package is made from does
    let requests: Vec<String> = project.of(Tool::Ringi).iter().map(|f| f.shown.clone()).collect();
    if !requests.is_empty() {
        all.extend(joined.ringi.checked_with(&project.root, &requests, &project.ringi, lang).into_iter().map(|u| (Tool::Ringi, u)));
    }
    whole.place_findings(&mut all, lang);
    // What else `ritsu check` finds that makes the code wrong (DESIGN 9.3): ritsu's own errors,
    // across the languages and of the modules, and sakai's references that cross a boundary the map
    // does not let them (its E2xx) from a file the package is made from: a workflow that calls a
    // service the other side does not open, a gate that guards another context's operation, whose
    // code would make that call. sakai's other errors are of the map, the contexts, the documents
    // and the settings, which no package holds, or say again what a language's check says (E105).
    // Each is said as `ritsu check` says it, and the file it is at does not pass.
    let mut theirs: Vec<(&str, Finding)> = Vec::new();
    if !project.of(Tool::Sakai).is_empty() {
        let mut args = project.given_for(Tool::Sakai);
        args.extend(project.dependency_dirs(Tool::Sakai));
        let made_from = |f: &Finding| f.file.as_deref().and_then(|r| project.holds(r)).is_some_and(|x| MADE_FROM.contains(&x.tool));
        let crosses = |f: &Finding| f.code.starts_with("E2");
        for u in sakai::run::checked(&args, Some(&project.root_arg()), &joined.sakai(), lang) {
            theirs.extend(u.findings().filter(|f| f.severity == Severity::Error && crosses(f) && made_from(f)).map(|f| ("sakai", f.clone())));
        }
    }
    let mut crossed = ritsu_cross::check_with(project, joined, verdicts, lang);
    // what kaname says of the code ringi lays for the requests is ringi's bug, said at the request
    // (ringi's DESIGN 9.2): a warning stops the package too
    let mut screens_refused = Vec::new();
    if let Some(laid) = laid {
        laid::said_as_ringi(project, laid, joined, &mut all, &mut crossed.findings, lang);
        // what the screens of the kinds say before they are written (ringi's DESIGN 13.3, 4.8)
        if let Some(given) = screens {
            screens_refused = laid::screens_said(project, laid, joined, given, &mut all, lang);
        }
    }
    theirs.extend(crossed.findings.into_iter().filter(|f| f.severity == Severity::Error).map(|f| ("ritsu", f)));
    if let Some(Ok(tree)) = project.tree.as_deref() {
        let of_modules = tree.found.iter().map(|d| crate::check::modules::finding(project, d, lang)).chain(crate::check::modules::uses(project, joined, tree, lang));
        theirs.extend(of_modules.filter(|f| f.severity == Severity::Error).map(|f| ("ritsu", f)));
    }
    // a line that reads a file that does not pass is said under that file, after the rest, as
    // `ritsu check` says it (DESIGN 8.3)
    let others: Vec<&Finding> = theirs.iter().map(|(_, f)| f).collect();
    let waiting = crate::check::waiting::Waiting::of(project, &all, &others, joined, &crate::check::ONE_FILE);
    let mut said = String::new();
    let mut found: Vec<Json> = Vec::new();
    let mut worst = 0u8;
    let mut failing: Vec<String> = Vec::new();
    for (i, (tool, u)) in all.iter().enumerate().filter(|(_, (_, u))| u.verdict != Verdict::Passes) {
        worst = worst.max(if u.verdict == Verdict::Unchecked { 2 } else { 1 });
        for (j, p) in u.parts.iter().enumerate() {
            match p {
                Part::Finding(f) => {
                    let w = waiting.at(i, j);
                    if w.is_none() {
                        said.push_str(&f.text_of(tool.word()));
                    }
                    found.push(crate::check::entry_waiting(tool.word(), f, w));
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
        let for_others = project.tree.is_some() && ritsu_base::paths::from_root(&whole.root, Path::new(&u.label)).is_some_and(|r| whole.failed_for_others(&r, u));
        if !for_others && !failing.contains(&u.label) {
            failing.push(u.label.clone());
        }
        for s in others {
            if !failing.contains(&s) {
                failing.push(s);
            }
        }
    }
    for (tool, f) in &theirs {
        worst = worst.max(1);
        said.push_str(&f.text_of(tool));
        found.push(crate::check::entry(tool, f));
        let shown = f.file.as_deref().map(|r| project.holds(r).map_or_else(|| r.to_string(), |x| x.shown.clone()));
        if let Some(s) = shown
            && !failing.contains(&s)
        {
            failing.push(s);
        }
    }
    let theirs_said: Vec<(&str, &Finding)> = theirs.iter().map(|(t, f)| (*t, f)).collect();
    said.push_str(&waiting.text(project, &all, &theirs_said, lang));
    if failing.is_empty() && theirs.is_empty() {
        return Ok(());
    }
    // the files that wait are named apart from the ones that do not pass of their own
    let waits = |shown: &String| project.files.iter().find(|f| f.shown == *shown).is_some_and(|f| waiting.files.contains_key(&f.rel));
    let (waiting_files, failing): (Vec<String>, Vec<String>) = failing.into_iter().partition(waits);
    // a request whose screens alone do not pass passes its check: it cannot be made into
    // `requests`, as a file a target cannot be made from is said (ringi's DESIGN 13.3)
    let only_screens = |shown: &String| {
        screens_refused.contains(shown) && all.iter().filter(|(_, u)| u.label == *shown && u.verdict != Verdict::Passes).count() == 1 && !theirs.iter().any(|(_, f)| f.file.as_deref().and_then(|r| project.holds(r)).is_some_and(|x| x.shown == *shown))
    };
    let (for_requests, failing): (Vec<String>, Vec<String>) = failing.into_iter().partition(only_screens);
    let r = refusal(worst, nothing_generated(&failing, &for_requests, &waiting_files), lang);
    Err(Refusal { said: format!("{said}{}", r.said), diagnostics: found, ..r })
}

/// The last line of a check that stops `ritsu gen`: the files that do not pass check; the requests
/// that pass it and whose screens alone cannot be made (ringi's DESIGN 13.3), said as a file a
/// target cannot be made from is said; and the files that wait for them.
fn nothing_generated(failing: &[String], for_requests: &[String], waiting: &[String]) -> Text {
    let (n, list) = (failing.len(), failing.join(", "));
    let named = Text::list(&for_requests.iter().map(|s| Text::same(format!("`{s}`"))).collect::<Vec<_>>());
    let mut why = if n > 0 {
        tr!("検査を通らないファイルが {n} 個あるので、何も生成しません: {list}", "{n} file(s) do not pass check, so nothing is generated: {list}")
    } else if !for_requests.is_empty() {
        tr!("{} からは {REQUESTS} のパッケージを生成できません", "{} cannot be made into the {REQUESTS} package", named.ja; named.en)
    } else {
        tr!("検査のエラーがあるので、何も生成しません", "the check finds errors, so nothing is generated")
    };
    if n > 0 && !for_requests.is_empty() {
        why.ja.push_str(&format!("。{} からは {REQUESTS} のパッケージを生成できません", named.ja));
        why.en.push_str(&format!("; {} cannot be made into the {REQUESTS} package", named.en));
    }
    if !waiting.is_empty() {
        let (k, wlist) = (waiting.len(), waiting.join(", "));
        why.ja.push_str(&format!("。それを待っているファイル {k} 個: {wlist}"));
        why.en.push_str(&format!("; {k} file(s) wait for them: {wlist}"));
    }
    why
}

/// The languages a package is made from, in the order `ritsu check` checks them: their own checks,
/// and sakai's references across a boundary from their files, stop `ritsu gen` (DESIGN 9.3).
const MADE_FROM: [Tool; 6] = [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Kaname, Tool::Dandori, Tool::Sekisho];

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
        let o = run.file(shown).map_err(|e| refusal(2, ritsu_base::cli::cannot_read(shown, &e), lang))?;
        let Some(c) = o.checked.clone() else {
            return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), lang));
        };
        // what the module reads beside it has to be the project's, for the package to hold it
        let p = kaname::value::Program::of(&c);
        let read: Vec<PathBuf> = p.rules.keys().chain(p.dates.keys()).chain(p.books.keys()).map(|k| p.file(k)).chain(c.world.modules.values().map(|m| PathBuf::from(&m.path))).collect();
        for path in read {
            let rel = ritsu_base::paths::from_root(&project.root, &path);
            if rel.as_ref().and_then(|r| project.holds(r)).is_none() {
                // the glue of a request reads what the request reads (`use rule`): said of the request
                let lowered = Path::new(&f.rel).parent();
                let request = project.of(Tool::Ringi).into_iter().find(|r| lowered == Some(ringi::lower::dir_of(&r.rel).as_path()));
                let shown = request.map_or(shown, |r| &r.shown);
                let other = rel.unwrap_or_else(|| ritsu_base::paths::show(&path));
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
/// of kaname as [`kaname_modules`] checks them, the rules with the contributions the engine of rules
/// was given merged (the rulec files `ritsu check` reads for the same paths, chosen once for the
/// run). What stops it for the modules' sake is made sure of before, by [`modules_hold`].
pub fn package(project: &Project, joined: &Joined, modules: &[(Rc<kaname::check::Checked>, String)], target: Target, o: &Options) -> Result<Package, Refusal> {
    let lang = o.lang;
    let said = o.said;
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
    // no module of its own. The engine chose them once for the run, and no file is read to choose
    // them again for each rule.
    let chosen = joined.rulec.contributions();
    // the units the rules and the books take amounts in, for the package's `units` module
    let mut units = Vec::new();
    for f in project.of(Tool::Rulec) {
        if rulec::link::is_contribution_file(&f.shown) {
            continue;
        }
        let place = at(&f.rel);
        if place.read_from_library(&place.dir) {
            continue;
        }
        // the rule as the check of the run kept it (DESIGN 4.15), merged with the contributions the
        // engine was given, which are `chosen`
        let refused = || {
            let shown = &f.shown;
            refusal(1, tr!("`{shown}` は検査を通っていないので生成しません", "`{shown}` does not pass check, so nothing is generated"), said)
        };
        let (path, src, kept) = joined.rulec.prepared(Path::new(&f.shown)).map_err(|_| refused())?;
        let Ok((rf, rc)) = kept.as_ref() else { return Err(refused()) };
        // a name the code of the target cannot take stops its package, and the run writes nothing
        // (rulec's E153, §15.335)
        let stops = joined.rulec.stops(Path::new(&f.shown), target.key(), &project.root, said);
        if !stops.is_empty() {
            let text: String = stops.iter().map(|s| s.text.as_str()).collect();
            let json = stops.iter().map(|s| crate::check::entry("rulec", s)).collect();
            let (shown, to) = (&f.shown, target.key());
            return Err(stopped(text, 1, tr!("`{shown}` からは {to} のパッケージを生成できません", "`{shown}` cannot be made into the {to} package"), json, said));
        }
        // A file of enums is the module of its types, beside the rules that read them (rulec's
        // §15.212).
        let (rel, body, used) = rulec::link::with_contributions(chosen.clone(), || rulec::i18n::with(rulec_lang, || rulec::codegen::package_parts_of(rf, rc, &src, &path, &f.rel, target.key(), &o.module, place.clone())))
            .map_err(|e| refusal(1, Text::new(e.clone(), e), said))?;
        units.extend(used);
        named.push(("rules", module_of(&rel), f.rel.clone()));
        kinds.entry(place.dir.clone()).or_default().entry("rules").or_default().push(module_of(&rel));
        files.push((format!("{base}{}rules/{rel}", place.dir), body, Some(f.rel.clone())));
    }
    // koyomi: each dates file's and calendar's module
    for f in project.of(Tool::Koyomi) {
        // the file as the check of the run kept it (DESIGN 4.15)
        let outcome = joined.koyomi.outcome(Path::new(&f.shown)).map_err(|e| refusal(2, ritsu_base::cli::cannot_read(&f.shown, &e), said))?;
        let Some(checked) = outcome.checked.as_ref().filter(|_| !outcome.has_errors()) else {
            let shown = &f.shown;
            return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), said));
        };
        let place = at(&f.rel);
        if place.read_from_library(&place.dir) {
            continue;
        }
        let u = koyomi::codegen::unit_in(checked, lang, Some(&f.rel), place.module.as_ref());
        let t = match target {
            Target::TypeScript => koyomi::naming::Target::TypeScript,
            Target::Python => koyomi::naming::Target::Python,
            Target::Go => koyomi::naming::Target::Go,
            Target::Rust => koyomi::naming::Target::Rust,
        };
        let body = koyomi::codegen::package_module(&outcome, &u, t).map_err(|ds| {
            let text: String = ds.iter().map(|d| d.render(said)).collect();
            let json = ds.iter().map(|d| diagnostic("koyomi", Some(f.rel.clone()), d.to_json(said))).collect();
            let (shown, to) = (&f.shown, target.key());
            stopped(text, 1, tr!("`{shown}` からは {to} のパッケージを生成できません", "`{shown}` cannot be made into the {to} package"), json, said)
        })?;
        // koyomi gen's own path, under its target's directory
        let rel = t.file(&u.alias).split_once('/').map(|(_, r)| r.to_string()).unwrap_or_default();
        named.push(("dates", module_of(&rel), f.rel.clone()));
        kinds.entry(place.dir.clone()).or_default().entry("dates").or_default().push(module_of(&rel));
        files.push((format!("{base}{}dates/{rel}", place.dir), body, Some(f.rel.clone())));
    }
    // chobo: each book's client, and on PostgreSQL the schema and the functions it calls, on the
    // MySQL family the tables its calls run on
    for f in project.of(Tool::Chobo) {
        // the book as the check of the run kept it (DESIGN 4.15)
        let (src, c) = joined.chobo.kept(Path::new(&f.shown)).map_err(|t| refusal(2, t, said))?;
        let Some(book) = c.book.as_ref() else {
            let shown = &f.shown;
            return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), said));
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
                let text: String = ds.iter().map(|d| chobo::diag::Show::shown(d, &f.shown, &src, said)).collect();
                let json = ds.iter().map(|d| diagnostic("chobo", Some(f.rel.clone()), from_serde(&chobo::diag::Show::json_in(d, &f.shown, &src, said)))).collect();
                let (shown, to) = (&f.shown, target.key());
                stopped(text, 1, tr!("`{shown}` からは {to} のパッケージを生成できません", "`{shown}` cannot be made into the {to} package"), json, said)
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
    // what a package of Rust leaves out, which `ritsu gen` says ([`rust::left_out`])
    let held = |tool: Tool| if target.holds_flows_and_gates() { project.of(tool) } else { Vec::new() };
    // dandori: each workflow, for Temporal in the language, reading the package's rules, dates and books
    let module = o.module.clone();
    for f in held(Tool::Dandori) {
        let place = at(&f.rel);
        if place.read_from_library(&place.dir) {
            continue;
        }
        let built = with_ports(joined, || -> Result<(Vec<(String, String)>, String), Refusal> {
            // the flow as the check of the run kept it (DESIGN 4.15)
            let kept = joined.dandori.kept_check(Path::new(&f.shown), &joined.ports()).map_err(|e| refusal(2, e, said))?;
            let (src, c) = &*kept;
            let Some(mut m) = c.model.clone() else {
                let shown = &f.shown;
                return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), said));
            };
            // what the workflow reads beside its own code has to be the project's, for the package to hold it
            let read: Vec<PathBuf> = m.rules.iter().map(|r| r.info.path.clone()).chain(m.books.iter().map(|b| b.path.clone())).collect();
            for p in read {
                let rel = ritsu_base::paths::from_root(&project.root, &p);
                if rel.as_ref().and_then(|r| project.holds(r)).is_none() {
                    let (shown, other) = (&f.shown, rel.unwrap_or_else(|| ritsu_base::paths::show(&p)));
                    return Err(refusal(
                        1,
                        tr!(
                            "`{shown}` が読む {other} が、プロジェクトのファイルにありません。パッケージはプロジェクトのファイルから作るので、そのファイルも ritsu gen に渡してください",
                            "`{shown}` reads {other}, which is not one of the project's files; a package is made of the project's files, so give ritsu gen that file too"
                        ),
                        said,
                    ));
                }
            }
            m.source_path = f.rel.clone();
            m.package = Some(dandori::model::InPackage { go_module: module.clone(), place: place.clone() });
            let dir = match target {
                Target::Go => dandori::temporal_go::package(&m),
                _ => dandori::render::ident(&m.name),
            };
            match target.dandori().and_then(|to| dandori::commands::build(&m, to)) {
                Some(Ok(files)) => Ok((files, dir)),
                Some(Err(diags)) => {
                    let json = diags.iter().map(|d| diagnostic("dandori", Some(f.rel.clone()), from_serde(&d.to_json(said)))).collect();
                    let (shown, to) = (&f.shown, target.key());
                    Err(stopped(dandori::commands::render(&diags, &f.shown, src, said), 1, tr!("`{shown}` からは {to} のパッケージを生成できません", "`{shown}` cannot be made into the {to} package"), json, said))
                }
                None => unreachable!("the targets that hold workflows are dandori's"),
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
    for f in held(Tool::Sekisho) {
        let outcome = gate(project, joined, f, said)?;
        let place = at(&f.rel);
        if place.read_from_library(&place.dir) {
            continue;
        }
        let stops = sekisho::spellings::stops(&outcome, target.key());
        if !stops.is_empty() {
            let text: String = stops.iter().map(|d| d.render(said)).collect();
            let json = stops.iter().map(|d| diagnostic("sekisho", Some(f.rel.clone()), d.to_json(said))).collect();
            let (shown, to) = (&f.shown, target.key());
            return Err(stopped(text, 1, tr!("`{shown}` からは {to} のパッケージを生成できません", "`{shown}` cannot be made into the {to} package"), json, said));
        }
        let to = sekisho::r#gen::Target { name: target.key(), authorizer: o.authorizer, go_module: o.module.clone(), place: place.clone() };
        let made = sekisho::r#gen::generate(&outcome, &to, &f.rel, lang, &gates).map_err(|e| {
            let shown = &f.shown;
            refusal(2, tr!("`{shown}` から生成できません（{e}）。sekisho の不具合です", "cannot generate from `{shown}` ({e}): a bug in sekisho"), said)
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
        let kt = kaname::r#gen::Target::parse(target.key()).expect("the four targets are kaname's");
        let placed: Vec<(Rc<kaname::check::Checked>, String, Place)> = modules.iter().map(|(c, rel)| (c.clone(), rel.clone(), at(rel))).filter(|(_, _, p)| !p.read_from_library(&p.dir)).collect();
        for (c, rel, _) in &placed {
            let ds = kaname::r#gen::stops(c, kt);
            if !ds.is_empty() {
                let text: String = ds.iter().map(|d| d.render(said)).collect();
                let json = ds.iter().map(|d| diagnostic("kaname", Some(rel.clone()), d.to_json(said))).collect();
                let (shown, to) = (&c.path, target.key());
                return Err(stopped(text, 1, tr!("`{shown}` からは {to} のパッケージを生成できません", "`{shown}` cannot be made into the {to} package"), json, said));
            }
        }
        let made = kaname::r#gen::package_for(&placed, kt, lang, &o.module, o.books.store());
        units.extend(made.units);
        for (c, rel, _) in &placed {
            named.push(("glue", c.module.name.ascii().to_string(), rel.clone()));
        }
        for (rel, body, from) in made.files {
            // glue/'s indexes are kaname's, at each module's directory it writes under
            kinds.entry(module_dir(&rel)).or_default().entry("glue").or_default();
            files.push((format!("{base}{rel}"), body, from));
        }
        if target == Target::Rust {
            files.extend(rust::wasm_files(project, &placed, &o.name, lang));
        }
        // the list of the kinds of request, with their entries that take JSON (ringi's DESIGN 12.2)
        let listed: Vec<(Rc<kaname::check::Checked>, String)> = placed.iter().map(|(c, rel, _)| (c.clone(), rel.clone())).collect();
        if let Some((rel, body)) = kinds::file(target, &listed, &o.kinds, o) {
            files.push((rel, body, None));
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
            l.units.shared(&brands, said)?
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
        Target::Rust => &rust::NO_FILE_OF_A_DIRECTORY,
        Target::TypeScript => &[],
    };
    if let Some((kind, name, from)) = named.iter().find(|(_, n, _)| kept.contains(&n.as_str())) {
        let language = match target {
            Target::Python => "Python",
            Target::Rust => "Rust",
            _ => "Go",
        };
        return Err(refusal(
            2,
            tr!(
                "`{from}` のモジュールの名前 `{name}` は {language} が予約している語なので、パッケージの {kind}/ に置けません。別の名前（別名）にしてください",
                "`{from}` makes the module `{name}`, a word {language} keeps, which the package cannot hold in its {kind}/; give it another name (an alias)"
            ),
            said,
        ));
    }
    files.extend(around(target, o, &kinds, &files));
    Ok(Package { files, bytes: Vec::new(), alone: false })
}

/// What stops the packages of a module for the modules' sake, made sure of once, before any target
/// is made (DESIGN 17.8): two flows of two modules that make one Temporal workflow type, two gates of
/// two modules that declare one name of Cedar's, unless the run writes no target that holds either
/// (`neither`: Rust's crate and the Lean hold neither), and, when Python or Rust is among the
/// targets, a dependency named with a word that language keeps. Each stops the run whatever target
/// is made; a stop of another kind is met where its target is made, as in a tree without a
/// manifest, which none of these can stop. Either way nothing is written, as every target is made
/// before any is (DESIGN 9.6). A file that does not pass is left to the target that reads it.
pub fn modules_hold(project: &Project, joined: &Joined, targets: &[Target], neither: bool, lang: Lang) -> Result<(), Refusal> {
    if project.tree.is_none() {
        return Ok(());
    }
    // a workflow runs by the name of its type, which a worker of a server's knows one of: two flows
    // of two modules that make one type would be one workflow
    let mut types: Vec<(String, String, &str)> = Vec::new();
    for f in if neither { Vec::new() } else { project.of(Tool::Dandori) } {
        let Some(m) = joined.dandori.kept_check(Path::new(&f.shown), &joined.ports()).ok().and_then(|kept| kept.1.model.clone()) else {
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
    let declared: Vec<Vec<(String, String)>> = if neither { Vec::new() } else { project.of(Tool::Sekisho).into_iter().filter_map(|f| gate(project, joined, f, lang).ok()).map(|o| sekisho::r#gen::cedar_names(&o)).collect() };
    cedar_names_hold(project, &declared, lang)?;
    // a dependency's name is the name of a Python package of the package's (DESIGN 17.3), which a
    // word Python keeps cannot be, and of a module of Rust's, which a word no file can be the module of
    // cannot: the directories of the modules the package holds a part of are the ones of its files (a
    // contribution has no module of its own)
    if targets.iter().any(|t| matches!(t, Target::Python | Target::Rust)) {
        let made = [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Dandori, Tool::Sekisho, Tool::Kaname];
        let dirs: Vec<String> = made
            .iter()
            .flat_map(|t| project.of(*t))
            .filter(|f| !(f.tool == Tool::Rulec && rulec::link::is_contribution_file(&f.shown)))
            .map(|f| place_of(project, &f.rel).dir)
            .collect();
        let named = |kept: &dyn Fn(&str) -> bool| dirs.iter().flat_map(|d| ritsu_emit::package::chain(d)).find(|n| kept(n));
        if let Some(name) = named(&|n| targets.contains(&Target::Python) && ritsu_emit::words::python::KEYWORDS.contains(&n)) {
            return Err(refusal(
                2,
                tr!(
                    "依存の名前 `{name}` は Python が予約している語なので、パッケージの deps/ に置けません。`require` で別の名前を付けてください",
                    "the dependency `{name}` is named with a word Python keeps, which the package cannot hold in its deps/; require it by another name"
                ),
                lang,
            ));
        }
        if let Some(name) = named(&|n| targets.contains(&Target::Rust) && rust::no_dependency(n)) {
            return Err(refusal(
                2,
                tr!(
                    "依存の名前 `{name}` は Rust のモジュールの名前にできない語なので、パッケージの deps/ に置けません。`require` で別の名前を付けてください",
                    "the dependency `{name}` is named with a word no module of Rust takes, which the package cannot hold in its deps/; require it by another name"
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

/// A gate's check, as the engine keeps it, of a gate that could be read.
struct Gate(Rc<Result<sekisho::check::Outcome, std::io::Error>>);

impl std::ops::Deref for Gate {
    type Target = sekisho::check::Outcome;
    fn deref(&self) -> &sekisho::check::Outcome {
        self.0.as_ref().as_ref().expect("a gate that could be read")
    }
}

/// A gate of the project, checked with every language joined, as the check of the run kept it
/// (DESIGN 4.15): Err when it does not pass, or when it reads a rule, a dates file or a calendar
/// the project does not hold (the code of its requests reads them from the package).
fn gate(project: &Project, joined: &Joined, f: &File, lang: Lang) -> Result<Gate, Refusal> {
    let shown = &f.shown;
    // the project's root, which the references of `@guards` are written from (as `ritsu check` gives it)
    let kept = joined.sekisho.outcome_of(&project.root, Path::new(shown));
    let o = match kept.as_ref() {
        Ok(o) => o,
        Err(e) => return Err(refusal(2, ritsu_base::cli::cannot_read(shown, e), lang)),
    };
    let Some(checked) = o.walked.as_ref().filter(|_| !o.has_errors()) else {
        return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), lang));
    };
    for u in checked.gate.uses.iter().filter(|u| matches!(u.kind, sekisho::model::UseKind::Rule | sekisho::model::UseKind::Dates | sekisho::model::UseKind::Calendar)) {
        let rel = ritsu_base::paths::from_root(&project.root, &u.file);
        if rel.as_ref().and_then(|r| project.holds(r)).is_none() {
            let other = rel.unwrap_or_else(|| ritsu_base::paths::show(&u.file));
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
    Ok(Gate(kept.clone()))
}

/// The Cedar of the project's gates, written once beside the packages (`<out>/cedar/`; sekisho's
/// DESIGN 5.6): each file by its path under that directory; and, for Verified Permissions, what of
/// it is over a quota (W401), as `sekisho gen --authorizer avp` says it, and in JSON. The Cedar's
/// `@doc` and heads are in `lang` (`Options::lang`); what the run says, in `in_said`.
pub fn cedar(project: &Project, joined: &Joined, authorizer: sekisho::r#gen::Authorizer, lang: Lang, in_said: Lang) -> Result<(Package, String, Vec<Json>), Refusal> {
    let gates: sekisho::suite::Suite = joined.sekisho().into();
    let mut files = Vec::new();
    let mut said = String::new();
    let mut warned = Vec::new();
    for f in project.of(Tool::Sekisho) {
        let outcome = gate(project, joined, f, in_said)?;
        if authorizer == sekisho::r#gen::Authorizer::Avp {
            for d in sekisho::r#gen::quotas(&outcome, in_said) {
                said.push_str(&d.render(in_said));
                warned.push(diagnostic("sekisho", Some(f.rel.clone()), d.to_json(in_said)));
            }
        }
        let place = place_of(project, &f.rel);
        let to = sekisho::r#gen::Target { name: "cedar", authorizer, go_module: String::new(), place: place.clone() };
        let made = sekisho::r#gen::generate(&outcome, &to, &f.rel, lang, &gates).map_err(|e| {
            let shown = &f.shown;
            refusal(2, tr!("`{shown}` から生成できません（{e}）。sekisho の不具合です", "cannot generate from `{shown}` ({e}): a bug in sekisho"), in_said)
        })?;
        // a dependency's under `deps/<name>/` beside the packages too
        for (rel, body) in made {
            files.push((format!("{}{}", place.dir, rel.strip_prefix("cedar/").unwrap_or(&rel)), body, Some(f.rel.clone())));
        }
    }
    Ok((Package { files, bytes: Vec::new(), alone: false }, said, warned))
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
            // Rust's are the crate's ([`rust::crate_files`])
            Target::Go | Target::Rust => {}
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
            Target::Go | Target::Rust => {}
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
            if files.iter().any(|(p, _, _)| p == "ticket_kinds.ts") {
                index.push_str("export * as ticket_kinds from \"./ticket_kinds\";\n");
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
            if files.iter().any(|(p, _, _)| *p == format!("{n}/ticket_kinds.py")) {
                init.push_str("from . import ticket_kinds as ticket_kinds\n");
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
            if files.iter().any(|(p, _, _)| p == "ticketkinds/ticketkinds.go") {
                doc.push_str("//\n// The kinds of request of its requests, each with its entries that take the JSON of the contract, are\n// in ticketkinds/.\n");
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
        Target::Rust => out.extend(rust::crate_files(o, all, files)),
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
/// name (`name`, the `--name` of every language), or, when Go keeps that too, the last part with
/// a `p` before it, as a last part that does not begin with a letter has.
pub(crate) fn go_package_name(module: &str, name: &str) -> String {
    let last = module.rsplit('/').next().unwrap_or(module);
    let s: String = last.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    let kept = |n: &str| ritsu_emit::words::go::KEYWORDS.contains(&n);
    match s.chars().next() {
        Some(c) if c.is_ascii_alphabetic() && !kept(&s) => s,
        Some(c) if c.is_ascii_alphabetic() && !kept(name) => name.to_string(),
        _ => format!("p{s}"),
    }
}

/// A package's name, as every language of a package takes it.
fn good_name(n: &str) -> bool {
    !n.is_empty() && n.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') && !ritsu_emit::words::python::KEYWORDS.contains(&n)
}

/// A Go import path, as a go.mod takes one.
pub(crate) fn good_module(m: &str) -> bool {
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
    /// The files an earlier run wrote for a target this one is not given (`<out>/cedar/`): said,
    /// and left as they are, under `--check` too (DESIGN 9.6).
    unasked: Vec<String>,
    /// And those an earlier run wrote for an extra of `requests` this one is not given (`--extra`).
    unasked_extras: Vec<String>,
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
    /// `--check`; `left` too, under both, for a file of a target not given (DESIGN 9.6).
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
/// head, setuptools's `build/` and `*.egg-info`, and a `go.sum`. The `target/` of a crate is never read:
/// no directory of a package of Rust is named so at its top, and what cargo writes there is no part of it.
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
            if d.as_path() == dir && name == "target" && target_of_a_crate(dir) {
                continue;
            }
            if library && d.as_path() == dir && (name == "dist" || name == "build" || name.ends_with(".egg-info") || name == "go.sum") {
                continue;
            }
            // a symbolic link is no part of the package: it is not followed, and nothing through
            // it is removed
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() {
                todo.push(p);
            } else if kind.is_file() && !planned.contains(&p) && generated_here(&p) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// That no two files of the project write one file of the package of `target` (rulec's §15.334):
/// one path, two paths that differ only in the case of their letters or in their Unicode
/// normalization (one file where the file system does not tell them apart), or, in Go, one
/// directory, which is one package that would declare the names of both. A file the package
/// writes for itself (an index, the manifest) is one of the two where a file of the project writes
/// the same.
fn written_once(p: &Package, target: &str, lang: Lang) -> Result<(), Refusal> {
    // each path in lower case, and each directory of Go's, with the first file that writes it
    let mut by: BTreeMap<String, (&str, &Option<String>)> = BTreeMap::new();
    let mut dirs: BTreeMap<&str, (&str, &Option<String>)> = BTreeMap::new();
    for (rel, from) in p.files.iter().map(|(r, _, f)| (r, f)).chain(p.bytes.iter().map(|(r, _, f)| (r, f))) {
        if let Some(prev) = by.insert(ritsu_emit::normal::nfc(rel).to_lowercase(), (rel.as_str(), from)) {
            return Err(refusal(2, twice(target, prev, (rel.as_str(), from), None), lang));
        }
        if target != "go" || !rel.ends_with(".go") {
            continue;
        }
        let dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
        match dirs.get(dir) {
            Some(&prev) if prev.1 != from => return Err(refusal(2, twice(target, prev, (rel.as_str(), from), Some(dir)), lang)),
            Some(_) => {}
            None => {
                dirs.insert(dir, (rel, from));
            }
        }
    }
    Ok(())
}

/// How each kind of file of a project takes another name, for a sentence that asks for one.
const NAMES_JA: &str = "規則と日付のファイルは別名、帳簿は名前、ワークフローは名前かファイルの名前を ASCII で";
const NAMES_EN: &str = "a rule or a dates file another alias, a book another name, a workflow a name or a file name in ASCII";

/// What [`written_once`] says of two writers of the package of `target`, each as (its path in the
/// package, the file of the project it is made from, None for the package's own): of one path, of two
/// that differ only in the case of their letters, or, with `dir`, of two files of one Go package.
fn twice(target: &str, (p, a): (&str, &Option<String>), (q, b): (&str, &Option<String>), dir: Option<&str>) -> Text {
    let (at, other) = (format!("{target}/{p}"), format!("{target}/{q}"));
    if let Some(dir) = dir {
        let d = if dir.is_empty() { format!("{target}/") } else { format!("{target}/{dir}/") };
        return match (a, b) {
            (Some(a), Some(b)) => tr!(
                "`{a}` と `{b}` が、どちらも Go のパッケージ {d} に書きます（{at} と {other}）。一つのパッケージで同じ名前を二度宣言するので組めません。どちらかの名前を変えてください（{NAMES_JA}）",
                "`{a}` and `{b}` both write into the Go package {d} of the package ({at} and {other}), which then declares the same names twice and does not build; give one of them another name ({NAMES_EN})"
            ),
            (Some(a), None) | (None, Some(a)) => tr!(
                "`{a}` が、パッケージが自分で書く Go のパッケージ {d} に書きます（{at} と {other}）。`{a}` の名前を変えてください（{NAMES_JA}）",
                "`{a}` writes into {d}, a Go package the package writes for itself ({at} and {other}); give `{a}` another name ({NAMES_EN})"
            ),
            (None, None) => tr!("パッケージが {d} に二度書きます。ritsu の不具合です。報告してください", "the package writes into {d} twice: a bug in ritsu, please report it"),
        };
    }
    match (a, b, p == q) {
        (Some(a), Some(b), true) => tr!(
            "`{a}` と `{b}` が、パッケージの同じ {at} を書きます。どちらかの名前を変えてください（{NAMES_JA}）",
            "`{a}` and `{b}` both write {at} of the package; give one of them another name ({NAMES_EN})"
        ),
        (Some(a), None, true) | (None, Some(a), true) => tr!(
            "`{a}` が、パッケージが自分で書く {at} を書きます。`{a}` の名前を変えてください（{NAMES_JA}）",
            "`{a}` writes {at}, which the package writes for itself; give `{a}` another name ({NAMES_EN})"
        ),
        (Some(a), Some(b), false) => tr!(
            "`{a}` の {at} と `{b}` の {other} は、大文字と小文字か Unicode の正規化だけが違います。ファイルシステムがそれらを区別しないと一つのファイルなので、どちらかの名前を変えてください（{NAMES_JA}）",
            "`{a}` writes {at} and `{b}` writes {other} of the package, which differ only in the case of their letters or in Unicode normalization and are one file where the file system does not tell them apart; give one of them another name ({NAMES_EN})"
        ),
        (Some(a), None, false) | (None, Some(a), false) => {
            // the project's file, and the package's own
            let (mine, own) = if b.is_none() { (&at, &other) } else { (&other, &at) };
            tr!(
                "`{a}` の {mine} と、パッケージが自分で書く {own} は、大文字と小文字だけが違います。大文字と小文字を区別しないファイルシステムでは一つのファイルなので、`{a}` の名前を変えてください（{NAMES_JA}）",
                "`{a}` writes {mine}, which differs only in the case of its letters from {own}, a file the package writes for itself, and is one file with it where the file system does not tell upper from lower case; give `{a}` another name ({NAMES_EN})"
            )
        }
        (None, None, _) => tr!("パッケージが {at} を二度書きます。ritsu の不具合です。報告してください", "the package writes {at} twice: a bug in ritsu, please report it"),
    }
}

/// The Lean of the project's modules of kaname (kaname's DESIGN 38.4), for `<out>/lean/` at `dir`:
/// each module's head names it by its path from the project's root (DESIGN 9.2), and the proof
/// beside a module (`<alias>.lean` beside its `.kn`) is read where it is, by a library whose
/// `srcDir` is the path from `dir` to it.
fn lean(project: &Project, modules: &[(Rc<kaname::check::Checked>, String)], dir: &Path) -> Package {
    use kaname::r#gen::lean;
    let named: Vec<(Rc<kaname::check::Checked>, Origin)> =
        modules.iter().map(|(c, rel)| (c.clone(), Origin::at(rel, ritsu_base::fed::without(&c.src).as_bytes()).of(place_of(project, rel).module))).collect();
    let checked: Vec<Rc<kaname::check::Checked>> = modules.iter().map(|(c, _)| c.clone()).collect();
    let layout = lean::Layout::InPlace { out: dir.to_path_buf(), proofs: lean::proofs_beside(&checked) };
    let (files, _) = lean::package_from(&named, &layout);
    Package { files: files.into_iter().map(|(p, text, from)| (p.to_string_lossy().replace('\\', "/"), text, from)).collect(), bytes: Vec::new(), alone: false }
}

/// What `--target requests` writes for the project's kinds of request (ringi's DESIGN 12.2): the
/// requests whose code the packages hold, each described by ringi; nothing when there is none.
fn requests(project: &Project, joined: &Joined, laid: &ritsu_project::requests::Laid, screens: Option<&ringi::screens::Given>, lang: Lang) -> Result<Package, Refusal> {
    let files: Vec<PathBuf> = laid.kinds.iter().map(|k| PathBuf::from(&k.request.shown)).collect();
    joined.ringi.requests_with(&files, &project.root, screens).map(|files| Package { files, bytes: Vec::new(), alone: false }).map_err(|e| {
        let (why_ja, why_en) = (e.ja, e.en);
        refusal(2, tr!("申請の種類を書き出せません（{why_ja}）。ringi の不具合です", "cannot describe the kinds of request ({why_en}): a bug in ringi"), lang)
    })
}

/// Each laid kind's `values.rule` as rulec writes it in JavaScript for a page (one module, with the
/// enums it reads inside it), by the kind's ASCII name: what the screens' `functions.js` holds
/// (ringi's DESIGN 13.7). Made after the laid files are checked, from what rulec kept of their
/// checks, so that no file is checked again (DESIGN 4.15); in English whatever the run's language,
/// as the screens' files are the same in every run. A kind whose form works out nothing has no
/// `values.rule`.
fn rules_in_javascript(joined: &Joined, laid: &ritsu_project::requests::Laid) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for k in &laid.kinds {
        let values = ringi::lower::dir_of(&k.request.shown).join("values.rule");
        if !laid.files.iter().any(|f| f.tool == Tool::Rulec && Path::new(&f.shown) == values) {
            continue;
        }
        let Ok(js) = joined.rulec.page_javascript(&values, Lang::En) else { continue };
        out.push((k.name.clone(), js));
    }
    out
}

/// Whether the directory of a package is a crate's: the `target/` of what cargo writes beside it.
fn target_of_a_crate(dir: &Path) -> bool {
    dir.join("Cargo.toml").is_file()
}

/// Whether a file begins with the head of a file ritsu's generators write (DESIGN 9.2): in a
/// comment (Markdown's too), or as the string of a key on one of the first lines of a JSON file,
/// JSON having no comment (the `"//"` of a library's package.json, DESIGN 17.16; the `"//"`,
/// `"$comment"`, `"x-ritsu-generated"` or, for the screens' catalog, `"description"` of what
/// `--target requests` writes); or a package.json of
/// the app's form (9.3), whose `description`, on its fourth line, ends with the head.
fn generated_here(p: &Path) -> bool {
    /// The string of a key on a line of its own, `"<key>": "<string>"`, with or without a comma after.
    fn string<'a>(line: &'a str, key: &str) -> Option<&'a str> {
        let rest = line.trim_start().strip_prefix('"')?.strip_prefix(key)?.strip_prefix("\": \"")?;
        Some(rest.trim_end().trim_end_matches(',').trim_end_matches('"'))
    }
    let Ok(text) = std::fs::read_to_string(p) else { return false };
    let head = |l: &str| ["rulec", "koyomi", "chobo", "dandori", "kaname", "sekisho", "ritsu"].iter().any(|t| l.contains(&format!("Code generated by {t} ")) && l.ends_with("DO NOT EDIT."));
    let lines: Vec<&str> = text.lines().take(4).collect();
    let markdown = |l: &str| l.trim().strip_prefix("<!-- ").and_then(|x| x.strip_suffix(" -->")).map(str::to_string);
    let keyed = |l: &str| ["//", "$comment", "x-ritsu-generated", "description"].iter().find_map(|k| string(l, k)).map(str::to_string);
    lines.iter().take(3).any(|l| head(&keyed(l).or_else(|| markdown(l)).unwrap_or_else(|| l.to_string())))
        || (p.file_name().is_some_and(|n| n == "package.json") && lines.get(3).and_then(|l| string(l, "description")).is_some_and(head))
}

/// The first symbolic link at or under `dir` on the way to `path`, including `path` itself; None
/// where every part is a directory or a regular file. `dir` itself is resolved before the run, so
/// the parts looked at here are strictly below it.
fn link_below(dir: &Path, path: &Path) -> Option<PathBuf> {
    let Ok(rel) = path.strip_prefix(dir) else { return None };
    let mut at = dir.to_path_buf();
    for part in rel.components() {
        at.push(part.as_os_str());
        if let Ok(m) = std::fs::symlink_metadata(&at)
            && m.file_type().is_symlink()
        {
            return Some(at);
        }
    }
    None
}

/// A temporary path beside `path`, unpredictable enough that a tree written before the run cannot
/// have a link at it. The file is written there and then renamed over `path`, so `path` itself is
/// never opened for writing.
fn temp_beside(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let name = path.file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into());
    path.with_file_name(format!(".{name}.ritsu-tmp-{}-{}-{stamp}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)))
}

/// Refuse a write through a link below the package's directory, naming the link.
fn no_link_below(dir: &Path, path: &Path, lang: Lang) -> Result<(), Refusal> {
    if let Some(link) = link_below(dir, path) {
        let (shown, at) = (path.display().to_string(), link.display().to_string());
        return Err(refusal(
            2,
            tr!("{shown} を書けません。{at} がリンクです。リンクを取り除くか、ほかの出力先にしてください", "cannot write {shown}: {at} is a symbolic link; remove it, or write somewhere else"),
            lang,
        ));
    }
    Ok(())
}

/// Write the package into `dir`, or under `check` say what differs from it. For `requests`,
/// `extras` are the extras the run is given (`--extra`): the files an earlier run wrote for one it
/// is not given are said, and left as they are ([`bundle::written_before`]).
fn emit(dir: &Path, target: &'static str, p: &Package, check: bool, library: bool, extras: &[&str], st: &mut Emitted, lang: Lang) -> Result<(), Refusal> {
    // what an earlier run wrote for the extras is read before anything of this run is written:
    // `bundle.json` written now names what this run lists, not what goes
    let zip_before = dir.parent().map_or_else(|| PathBuf::from(bundle::ZIP), |d| d.join(bundle::ZIP));
    let before = bundle::written_before(dir, &zip_before);
    let listed_before = bundle::listed(&dir.join(bundle::ABOUT));
    let all: Vec<(&String, Option<&[u8]>, &Option<String>)> =
        p.files.iter().map(|(rel, body, from)| (rel, Some(body.as_bytes()), from)).chain(p.bytes.iter().map(|(rel, body, from)| (rel, body.as_deref(), from))).collect();
    let planned: BTreeSet<PathBuf> = all.iter().map(|(rel, _, _)| dir.join(rel)).collect();
    for (rel, body, from) in all {
        let path = dir.join(rel);
        let shown = path.display().to_string();
        let existing = std::fs::read(&path).ok();
        if body.is_some() && existing.as_deref() == body {
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
        let Some(body) = body else {
            return Err(refusal(2, tr!("{shown} の中身を作らずに書こうとしました。ritsu の不具合です。報告してください", "{shown} is written with nothing made for it: a bug in ritsu, please report it"), lang));
        };
        no_link_below(dir, &path, lang)?;
        if let Some(parent) = path.parent() {
            no_link_below(dir, parent, lang)?;
            if let Err(e) = ritsu_base::fs::create_dir_all(parent) {
                return Err(refusal(2, ritsu_base::cli::cannot_create(&parent.display().to_string(), &e), lang));
            }
        }
        let tmp = temp_beside(&path);
        no_link_below(dir, &tmp, lang)?;
        if let Err(e) = ritsu_base::fs::write(&tmp, body).and_then(|_| std::fs::rename(&tmp, &path)) {
            let _ = std::fs::remove_file(&tmp);
            return Err(refusal(2, ritsu_base::cli::cannot_write(&shown, &e), lang));
        }
        st.note(target, &shown, from, "written");
        st.written.push(shown);
    }
    if p.alone {
        return Ok(());
    }
    let mut left = leftovers(dir, &planned, library);
    if target == REQUESTS {
        // a kind's vectors, and those of its entry `worked`, have no head to tell them by: they go
        // with the kind's description
        let theirs: Vec<PathBuf> = left
            .iter()
            .filter_map(|p| Some(p.strip_prefix(dir.join("types")).ok()?.to_str()?.strip_suffix(".json")?.to_string()))
            .flat_map(|stem| [dir.join("vectors").join(format!("{stem}.jsonl")), dir.join("vectors").join("worked").join(format!("{stem}.jsonl"))])
            .filter(|v| v.is_file() && !planned.contains(v))
            .collect();
        left.extend(theirs);
        // the files an extra wrote before: those of an extra the run is not given are said and left
        // as they are (DESIGN 9.6), as the Cedar of a target not given is; of one it is given, the
        // files `bundle.json` listed that the run does not write go, as a kind's vectors do
        let before = before.clone();
        let unasked: BTreeSet<&PathBuf> = before.iter().filter(|(x, _)| !extras.contains(x)).map(|(_, p)| p).collect();
        left.retain(|p| !unasked.contains(p));
        if extras.contains(&"bundle") {
            // a path the earlier bundle listed is inside `dir` as text, but a link below `dir`
            // could still make removing it remove a file outside; refuse it as a write through one
            for rel in &listed_before {
                no_link_below(dir, &dir.join(rel), lang)?;
            }
            left.extend(listed_before.iter().cloned().map(|rel| dir.join(rel)).filter(|p| p.is_file() && !planned.contains(p)));
        }
        for p in unasked {
            let shown = p.display().to_string();
            st.note(target, &shown, &None, "left");
            st.unasked_extras.push(shown);
        }
        left.sort();
        left.dedup();
    }
    for p in left {
        let shown = p.display().to_string();
        if check {
            st.note(target, &shown, &None, "left");
            st.left.push(shown);
        } else if std::fs::remove_file(&p).is_ok() {
            st.note(target, &shown, &None, "removed");
            st.removed.push(shown);
            // the screens of a kind no longer given have a directory of their own
            // (`a2ui/<name>/`): it goes with its last file; so do the directories of `source/` and
            // `diff/` a file no longer listed leaves empty
            if target == REQUESTS {
                if let Some(parent) = p.parent().filter(|d| d.parent() == Some(dir.join("a2ui").as_path())) {
                    let _ = std::fs::remove_dir(parent);
                }
                let mut up = p.parent();
                while let Some(d) = up.filter(|d| d.starts_with(dir.join(bundle::SOURCE)) || d.starts_with(dir.join(bundle::DIFF))) {
                    if std::fs::remove_dir(d).is_err() {
                        break;
                    }
                    up = d.parent();
                }
            }
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
/// nothing was generated, with the exit code (null when something was), and, when it was given no
/// target, the words of the line that writes what 0.40.0 wrote (DESIGN 9.6).
struct Answer {
    root: Option<String>,
    check: bool,
    out: String,
    targets: Vec<&'static str>,
    /// The extras asked for (`--extra`), in the order of [`EXTRA_WORDS`].
    extras: Vec<&'static str>,
    files: Vec<Emit>,
    diagnostics: Vec<Json>,
    error: Option<(u8, String)>,
    instead: Option<Vec<String>>,
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
            Some((c, m)) => {
                let mut pairs = vec![("code", Json::int(i128::from(*c))), ("message", Json::str(m))];
                if let Some(words) = &self.instead {
                    pairs.push(("instead", Json::arr(words.iter().map(Json::str))));
                }
                Json::obj(pairs)
            }
            None => Json::Null,
        };
        Json::obj([
            ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
            ("root", self.root.as_deref().map(Json::str).unwrap_or(Json::Null)),
            ("ok", Json::Bool(code == 0)),
            ("check", Json::Bool(self.check)),
            ("out", Json::str(&self.out)),
            ("targets", Json::arr(self.targets.iter().map(|t| Json::str(*t)))),
            ("extras", Json::arr(self.extras.iter().map(|t| Json::str(*t)))),
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

/// `ritsu gen [<path>...] [--target typescript|python|go|rust|cedar|lean|requests]... [--out <dir>] [--check]
/// [--books postgres|tigerbeetle|mysql] [--authorizer cedar|avp] [--name <name>] [--module <path>]
/// [--library [--embed <name>]...] [--root <dir>] [--format json]`: the exit code.
pub fn command(args: &[String], lang: Lang) -> u8 {
    let table = cli::table();
    let cmd = table.command("gen").expect("gen is in the table");
    let asked = Lang::flag_of(args);
    let lang = if asked.is_some() { Lang::pick(asked.as_deref(), "RITSU_LANG") } else { lang };
    // what the packages say is in `--lang`, else English, whatever the environment says (DESIGN 4.1)
    let made = Lang::made(asked.as_deref());
    let json = asks_json(args);
    let mut answer = Answer { root: None, check: args.iter().any(|a| a == "--check"), out: "generated".to_string(), targets: Vec::new(), extras: Vec::new(), files: Vec::new(), diagnostics: Vec::new(), error: None, instead: None, silent: false };
    let code = run_of(args, &table, cmd, lang, made, json, &mut answer);
    if json && !answer.silent {
        println!("{}", answer.json(code).pretty());
    }
    code
}

/// What `ritsu gen --check` comes to for the module whose root is `root`, for `ritsu impact`.
pub enum Fresh {
    /// Every file of the targets of its `target` line is what `ritsu gen` writes.
    Same,
    /// Those that are not, as the run writes them.
    Stale(Vec<String>),
    /// It could not say (the project does not pass its check, say), with why.
    Unknown(Text),
}

/// `ritsu gen --check`, with its `target` line and every default, of the module whose root is `root`
/// (the packages under `<root>/generated`), on the disk. Nothing is written. A package's comments
/// are in the language of the run that wrote it, so it is held to what a run in that language writes
/// ([`written_in`]); where that cannot be told, to what `ritsu gen` with no `--lang` writes,
/// English, whatever the environment says. `lang` is the language of what it says.
pub fn fresh(root: &Path, lang: Lang) -> Fresh {
    let made = written_in(&root.join("generated")).unwrap_or_else(|| Lang::made(None));
    let table = cli::table();
    let cmd = table.command("gen").expect("gen is in the table");
    let at = root.to_string_lossy().into_owned();
    let out = root.join("generated").to_string_lossy().into_owned();
    let args: Vec<String> = vec![at.clone(), "--root".into(), at, "--check".into(), "--out".into(), out.clone(), "--format".into(), "json".into()];
    let mut answer = Answer { root: None, check: true, out, targets: Vec::new(), extras: Vec::new(), files: Vec::new(), diagnostics: Vec::new(), error: None, instead: None, silent: false };
    let code = run_of(&args, &table, cmd, lang, made, true, &mut answer);
    match (code, answer.error) {
        (0, _) => Fresh::Same,
        (_, Some((_, why))) => Fresh::Unknown(Text::same(why)),
        (1, None) if !answer.silent => Fresh::Stale(answer.files.into_iter().filter(|f| matches!(f.state, "missing" | "stale" | "left")).map(|f| f.path).collect()),
        _ => Fresh::Unknown(tr!("`ritsu gen --check` がモジュールを読めませんでした", "`ritsu gen --check` could not read the module")),
    }
}

/// The language the generated files under `dir` were written in, by the line of what each is made
/// from (the second, ritsu-emit's `header`): Japanese where one says it in Japanese (`もと:`), else
/// English where one says it in English (`Source:`). A run in Japanese writes some files' line in
/// English (a workflow's), one in English none in Japanese. None where no file says it.
fn written_in(dir: &Path) -> Option<Lang> {
    let mut en = false;
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(d) = dirs.pop() {
        let Ok(entries) = ritsu_base::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() {
                dirs.push(e.path());
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            let Ok(bytes) = ritsu_base::fs::read(e.path()) else { continue };
            let text = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).into_owned();
            let Some(second) = text.lines().nth(1) else { continue };
            // after the comment's mark: `//`, `#`, `--`, ` *`
            let said = second.trim_start_matches(['/', '#', '-', '*', ' ']);
            if said.starts_with("もと: ") {
                return Some(Lang::Ja);
            }
            en |= said.starts_with("Source: ");
        }
    }
    en.then_some(Lang::En)
}

/// `ritsu gen` with `args`, printing nothing, for `ritsu ringi init`, which writes what it writes
/// once beside its own files: Err with why nothing was written.
pub(crate) fn quietly(args: &[String], lang: Lang) -> Result<(), Text> {
    let table = cli::table();
    let cmd = table.command("gen").expect("gen is in the table");
    // `--lang` of the arguments is the run's language, and the language of what it writes, as in
    // [`command`]
    let asked = Lang::flag_of(args);
    let lang = if asked.is_some() { Lang::pick(asked.as_deref(), "RITSU_LANG") } else { lang };
    let mut answer = Answer { root: None, check: false, out: String::new(), targets: Vec::new(), extras: Vec::new(), files: Vec::new(), diagnostics: Vec::new(), error: None, instead: None, silent: false };
    match (run_of(args, &table, cmd, lang, Lang::made(asked.as_deref()), true, &mut answer), answer.error) {
        (0, _) => Ok(()),
        (_, Some((_, why))) => Err(Text::same(why)),
        _ => Err(tr!("`ritsu gen` が生成できませんでした", "`ritsu gen` could not generate")),
    }
}

/// The version of the Go module `m` tested here, and the `go` line of its go.mod at that version
/// (`GO_MODULES`), for the go.mod `ritsu ringi init` writes; None for a module not listed.
pub(crate) fn go_module(m: &str) -> Option<(&'static str, &'static str)> {
    GO_MODULES.iter().find(|(name, ..)| *name == m).map(|(_, v, go)| (*v, *go))
}

/// The `go` line of a module whose requirements' go.mod files have the `go` lines `asked`: the
/// latest of them, and no earlier than the oldest Go release supported (`GO_FLOOR`).
pub(crate) fn go_line_of<'a>(asked: impl IntoIterator<Item = &'a str>) -> &'a str {
    library::go_line(GO_FLOOR, asked)
}

/// [`generate`] as a run of ritsu's, for [`command`] and [`fresh`] alike: a rule's `range from
/// koyomi` is read through koyomi's port, as in `ritsu rulec` (DESIGN 7.5), and rulec reads in the
/// run's language, as in `ritsu check` (`check::run_with`), so the checks the run keeps answer what
/// is asked of the rules after them, and each file is checked once. `lang` is the language of what
/// the run prints, `made_lang` the language of what the packages say (DESIGN 4.1).
fn run_of(args: &[String], table: &ritsu_base::cli::Table, cmd: &ritsu_base::cli::Cmd, lang: Lang, made_lang: Lang, json: bool, answer: &mut Answer) -> u8 {
    let rl = if lang == Lang::Ja { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En };
    rulec::days::with(Some(ritsu_project::Joined::dates()), || rulec::i18n::with(rl, || generate(args, table, cmd, lang, made_lang, json, answer)))
}

/// [`command`], the answer kept for the JSON as it goes.
fn generate(args: &[String], table: &ritsu_base::cli::Table, cmd: &ritsu_base::cli::Cmd, lang: Lang, made_lang: Lang, json: bool, answer: &mut Answer) -> u8 {
    // Nothing generated: the text says why on the standard error; the JSON keeps it.
    // What stops the run before its work (2) is said on the standard error with JSON too, as every
    // command of the toolchain says it (DESIGN 6.2, item 10).
    let say = |r: Refusal, answer: &mut Answer| -> u8 {
        if json {
            if r.code == 2 {
                eprint!("{}", r.said);
            }
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
    // the directory as the run holds a path a flag takes (DESIGN 4.15): `--out ./generated` is `generated`.
    // `--out` itself may be a link; nothing below it may be one (`no_link_below`).
    answer.out = ritsu_base::paths::fold_given(a.get("--out").unwrap_or("generated"));
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
        return say(refusal(2, tr!("`--module {module}` は Go のモジュールのパスにできません。`/` で区切った、英数字と `-._~` だけの部分で書きます（`example.com/acme/approvals`）", "`--module {module}` cannot be a Go module's path: write it as parts of letters, digits and `-._~`, between `/` (`example.com/acme/approvals`)"), lang), answer);
    }
    let books = match a.get("--books") {
        Some("tigerbeetle") => Books::TigerBeetle,
        Some("mysql") => Books::MySql,
        _ => Books::Postgres,
    };
    let authorizer = a.get("--authorizer").and_then(sekisho::r#gen::Authorizer::parse).unwrap_or(sekisho::r#gen::Authorizer::Cedar);
    // the targets (DESIGN 9.6): `--target`'s, else the `target` line of the module's ritsu.mod
    let given = in_order(&a.all("--target"));
    answer.targets = given.clone();
    // the paths the run holds are written the way the first path given was (DESIGN 4.15)
    let _way = ritsu_base::paths::run_from(a.pos.first().map_or(".", String::as_str));
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
            for t in said {
                eprintln!("{}", ritsu_base::cli::refusal(t.get(lang), lang));
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
    // a library is a module's, which has a version (DESIGN 17.16)
    if library && let Err(r) = library::module(&project, lang) {
        return say(r, answer);
    }
    // with no `--target`, the `target` line of the module's ritsu.mod (DESIGN 17.3); with neither,
    // nothing is written, and the line that writes what 0.40.0 wrote is said (DESIGN 9.6)
    let line = match project.tree.as_deref() {
        Some(Ok(tree)) => tree.main().manifest.targets.as_ref().map(|t| in_order(&t.value.iter().map(String::as_str).collect::<Vec<_>>())),
        _ => None,
    };
    let Some(names) = Some(given).filter(|g| !g.is_empty()).or(line) else {
        let (r, instead) = no_target(args, &project, library, Path::new(&answer.out), lang);
        answer.instead = instead;
        return say(r, answer);
    };
    answer.targets = names.clone();
    let cedar_given = names.contains(&CEDAR);
    let lean_given = names.contains(&LEAN);
    let requests_given = names.contains(&REQUESTS);
    // the extras of `requests` (ringi's DESIGN 12.6), in the order of [`EXTRA_WORDS`]
    let extras: Vec<&'static str> = EXTRA_WORDS.into_iter().filter(|e| a.all("--extra").contains(e)).collect();
    answer.extras = extras.clone();
    if let Some(e) = extras.first().filter(|_| !requests_given) {
        return say(refusal(2, tr!("`--extra {e}` は `requests` の付属物です。`--target requests` がありません", "`--extra {e}` is an extra of `requests`, and there is no `--target requests`"), lang), answer);
    }
    if extras.contains(&"bundle") && !extras.contains(&"wasm") {
        return say(
            refusal(
                2,
                tr!(
                    "`--extra bundle` には `--extra wasm` が要ります。バンドルは、サーバーが組み直さずに読み込む Wasm を運ぶ物です",
                    "`--extra bundle` needs `--extra wasm`: a bundle carries the Wasm a server loads with no build of its own"
                ),
                lang,
            ),
            answer,
        );
    }
    let diff_from = a.get("--diff-from").map(str::to_string);
    if diff_from.is_some() && !extras.contains(&"bundle") {
        return say(refusal(2, tr!("`--diff-from` は `--extra bundle` と一緒に使います", "`--diff-from` goes with `--extra bundle`"), lang), answer);
    }
    // the screens' catalog (ringi's DESIGN 13.2): its ID is a name for the project's catalog, written
    // as a URL the project owns, the same in every environment, which only the project knows:
    // `--catalog`'s, else the `catalog` line of the module's ritsu.mod; with neither, `requests`
    // writes nothing
    let catalog_line = match project.tree.as_deref() {
        Some(Ok(tree)) => tree.main().manifest.catalog.as_ref().map(|c| c.value.clone()),
        _ => None,
    };
    let catalog_given = a.get("--catalog").map(str::to_string);
    if catalog_given.is_some() && !requests_given {
        return say(refusal(2, tr!("`--catalog` は `--target requests` と一緒に使います", "`--catalog` goes with `--target requests`"), lang), answer);
    }
    if let Some(t) = catalog_given.as_deref().and_then(ritsu_base::modules::manifest::catalog_problem) {
        return say(refusal(2, tr!("`--catalog` の値が使えません: ", "`--catalog` cannot be used: ").then(&t), lang), answer);
    }
    let catalog = catalog_given.or(catalog_line).filter(|_| requests_given);
    if requests_given && catalog.is_none() {
        return say(
            refusal(
                2,
                tr!(
                    "`requests` は画面のテンプレートも書くので、テンプレートが指すカタログの ID が要ります。`--catalog <url>` か、モジュールなら ritsu.mod の `catalog \"<url>\"` の行で渡してください。ID は、このプロジェクトのカタログを指す名前で、自分の持つ URL の形で書きます。`requests/a2ui/catalog.json` を配る場所と同じにしておくと分かりやすいですが、同じである要はありません。環境が変わっても同じ値を使ってください（生成物は環境ごとに作り分けません）",
                    "`requests` writes the screens too, which need the ID of the catalog they name: give it with `--catalog <url>`, or in a module with the line `catalog \"<url>\"` of its ritsu.mod. The ID is a name for this project's catalog, written as a URL you own. The place `requests/a2ui/catalog.json` is served at is the easiest to follow, but the two need not be the same. Use the same value in every environment (what is generated is not made apart for each)"
                ),
                lang,
            ),
            answer,
        );
    }
    let targets: Vec<Target> = names.iter().filter_map(|n| Target::parse(n)).collect();
    // a library of Rust has three things to decide first (DESIGN 17.16): `--target`'s and the line's alike
    if library && targets.contains(&Target::Rust) {
        return say(rust::no_library(lang), answer);
    }
    // a library: a `library` line for each language it is written in, and its module's files but
    // what it leaves out
    let project = if library {
        if let Err(r) = library::lines(&project, &targets, lang) {
            return say(r, answer);
        }
        library::whole_module(project)
    } else {
        project
    };
    if [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Dandori, Tool::Kaname, Tool::Sekisho, Tool::Ringi].iter().all(|t| project.of(*t).is_empty()) {
        // in the words every command of the toolchain says a path with none of its files under it
        // (DESIGN 6.2, item 10): the files a package is made of
        let given = project.given.join("`, `");
        return say(refusal(2, ritsu_base::cli::no_files_under(&given, &[".rule", ".cal", ".book", ".flow", ".kn", ".gate", ".ringi"]), lang), answer);
    }
    let joined = Joined::new();
    // the lists of each version of the organization do not stop a package (ringi's DESIGN 12.3): its
    // code takes the organization at each event; what they say is said in a line for each request
    joined.ringi.keep_listing_apart();
    // the server's API the run writes is written again from the requests as they are: a flow that
    // reads it is not held to it as it was (X20, ringi's DESIGN 9.3), or the run that brings it up
    // to date would stop for its being out of date. It is held to the API the run writes instead,
    // laid where the run writes it before the check ([`api_laid`])
    let api_doc = ritsu_base::paths::absolute(&Path::new(&answer.out).join(REQUESTS).join("openapi.json"));
    if requests_given {
        joined.ringi.set_writes_api(&api_doc);
    }
    // the files the requests are lowered to are asked of rulec and koyomi in the run's language
    // first, as the run asks of them later: each is checked once
    joined.ringi.set_lang(lang);
    // The contributions are merged from the rulec files `ritsu check` reads for the same paths: in a
    // check of a whole module, every module's of the tree (DESIGN 17.8). They are set before any
    // rule is read.
    let whole = project.clone().with_dependencies();
    let contributions: Vec<String> = whole.of(Tool::Rulec).iter().map(|f| f.shown.clone()).collect();
    joined.rulec.set_project(contributions.clone());
    // the API `requests` writes, laid where it writes it for every check and every target made
    // (not for what is written): a flow that reads it is held to it as it is written. It is made
    // before the run reads anything else: what a gate names is what its check found, and its check
    // reads the flows of its workflows, which the run reads on the API laid
    let api = if requests_given { api_laid(&project, &joined, &api_doc) } else { None };
    // In a module, the files of the dependencies the files given reach go into the package with them.
    let project = within(&api, || project.reached(&joined));
    // what the checks across the borders come to at the calls of the modules of kaname, once
    let verdicts = Rc::new(ritsu_cross::ProgramVerdicts::new(&joined));
    if let Err(r) = within(&api, || checked(&project, &whole, &joined, &verdicts, None, None, lang)) {
        return say(r, answer);
    }
    // the files the requests are lowered to, the glue of each and the machine of their tickets, laid
    // over the files the languages read (ringi's DESIGN 12.3): checked as the project's own files
    // are, then made into the packages with them
    let laid = within(&api, || project.laid(&joined));
    // what the screens of the kinds are made with (ringi's DESIGN 13): the catalog's ID, which their
    // check reads, and each kind's `values.rule` in JavaScript, made once the laid files are checked
    let screens = catalog.map(|catalog| ringi::screens::Given { catalog, rules: Vec::new() });
    let project = if laid.files.is_empty() {
        project
    } else {
        let only = project.only_laid(&laid);
        if let Err(r) = over(&laid, &api, || checked(&only, &only, &joined, &verdicts, Some(&laid), screens.as_ref(), lang)) {
            return say(r, answer);
        }
        project.with_laid(&laid)
    };
    let screens = screens.map(|given| ringi::screens::Given { rules: over(&laid, &api, || rules_in_javascript(&joined, &laid)), ..given });
    let modules = match over(&laid, &api, || kaname_modules(&project, &joined, &verdicts, lang)) {
        Ok(m) => m,
        Err(r) => return say(r, answer),
    };
    // what stops the packages of a module for the modules' sake, once, before any target is made:
    // the workflows' and the gates' names only when a target given holds them (neither Rust's crate
    // nor the Lean does)
    let neither = !cedar_given && targets.iter().all(|t| !t.holds_flows_and_gates());
    if let Err(r) = over(&laid, &api, || modules_hold(&project, &joined, &targets, neither, lang)) {
        return say(r, answer);
    }
    // each language's library: where it reads each module, and what it says it depends on
    let libraries = if library {
        match over(&laid, &api, || library::plan(&project, &whole, &joined, &verdicts, &contributions, &targets, &embed, lang)) {
            Ok(l) => l,
            Err(r) => return say(r, answer),
        }
    } else {
        Vec::new()
    };
    let o = Options { name, module, books, authorizer, lang: made_lang, said: lang, library: None, kinds: laid.kinds.clone() };
    let out = PathBuf::from(&answer.out);
    let check = a.has("--check");
    // Every target given is made before any is written (DESIGN 9.6): one that cannot be made stops
    // the run with nothing written, so a run that stops leaves no target of its own behind; only a
    // write the disk refuses can.
    let mut made: Vec<(PathBuf, &'static str, Package, bool)> = Vec::new();
    for t in targets.iter().copied() {
        let o = for_target(&o, &libraries, t);
        let p = match over(&laid, &api, || package(&project, &joined, &modules, t, &o)) {
            Ok(p) => p,
            Err(r) => return say(r, answer),
        };
        if let Err(r) = written_once(&p, t.key(), lang) {
            return say(r, answer);
        }
        made.push((out.join(t.key()), t.key(), p, o.library.is_some()));
    }
    // the gates' Cedar, once beside the packages, when `cedar` is given (DESIGN 9.6)
    if cedar_given {
        let (p, warnings, warned) = match cedar(&project, &joined, authorizer, made_lang, lang) {
            Ok(p) => p,
            Err(r) => return say(r, answer),
        };
        if json {
            answer.diagnostics.extend(warned);
        } else {
            print!("{warnings}");
        }
        made.push((out.join("cedar"), CEDAR, p, false));
    }
    // the Lean of the modules of kaname, once beside the packages, when `lean` is given
    if lean_given {
        let p = over(&laid, &api, || lean(&project, &modules, &out.join(LEAN)));
        if let Err(r) = written_once(&p, LEAN, lang) {
            return say(r, answer);
        }
        made.push((out.join(LEAN), LEAN, p, false));
    }
    // the kinds of request apart from any language, once beside the packages, when `requests` is
    // given (ringi's DESIGN 12.2)
    if requests_given {
        let mut p = match over(&laid, &api, || requests(&project, &joined, &laid, screens.as_ref(), lang)) {
            Ok(p) => p,
            Err(r) => return say(r, answer),
        };
        // the extras (ringi's DESIGN 12.6): the module of the package of Rust and how it is called,
        // then the files the kinds are made from, with what changed, and the zip of it all
        if extras.contains(&"wasm") {
            // the crate `--target rust` writes, as it is written; a library of Rust (`--library`)
            // is laid otherwise, and the crate is made as a package of the project is
            let rust = match made.iter().find(|m| m.1 == Target::Rust.key() && !m.3) {
                Some(m) => m.2.files.clone(),
                None => {
                    let o = for_target(&o, &[], Target::Rust);
                    match over(&laid, &api, || package(&project, &joined, &modules, Target::Rust, &o)) {
                        Ok(p) => p.files,
                        Err(r) => return say(r, answer),
                    }
                }
            };
            let doors = kinds::door(&modules, &o.kinds);
            if doors.is_empty() {
                return say(
                    refusal(
                        2,
                        tr!(
                            "`--extra wasm` は、コードを書く申請の種類が一つも無いので、何も書きません",
                            "`--extra wasm` has no kind of request whose code is written, so nothing is written"
                        ),
                        lang,
                    ),
                    answer,
                );
            }
            let w = match wasm::make(&rust, &o.name, &doors, ringi::lower::shape::CONTRACT, made_lang.code(), check, &out.join(REQUESTS), lang) {
                Ok(w) => w,
                Err(r) => return say(r, answer),
            };
            p.files.push((wasm::ABOUT.to_string(), w.about, None));
            p.bytes.push((wasm::MODULE.to_string(), w.module, None));
        }
        if extras.contains(&"bundle") {
            let base = match diff_from.as_deref().map(|b| bundle::base(b, lang)).transpose() {
                Ok(b) => b,
                Err(r) => return say(r, answer),
            };
            let sources = match over(&laid, &api, || bundle::sources(&project, &joined, &laid.kinds, lang)) {
                Ok(s) => s,
                Err(r) => return say(r, answer),
            };
            let compared = match &base {
                Some(b) => match bundle::compare(b, &project, &laid.kinds, lang) {
                    Ok(c) => Some(c),
                    Err(r) => return say(r, answer),
                },
                None => None,
            };
            p.files.push((bundle::ABOUT.to_string(), bundle::about(&sources, base.as_ref().zip(compared.as_deref())), None));
            for (rel, text) in compared.as_deref().map(bundle::pages).unwrap_or_default() {
                p.files.push((rel, text, None));
            }
            for (rel, bytes) in sources {
                let from = rel.strip_prefix(&format!("{}/", bundle::SOURCE)).map(str::to_string);
                p.bytes.push((rel, Some(bytes), from));
            }
            // every file the run writes into <out>/requests/, and nothing else there; under
            // `--check`, none when one of them is not what the run writes
            let mut held: Vec<(String, Vec<u8>)> = p.files.iter().map(|(rel, text, _)| (rel.clone(), text.as_bytes().to_vec())).collect();
            let mut whole = true;
            for (rel, bytes, _) in &p.bytes {
                match bytes {
                    Some(b) => held.push((rel.clone(), b.clone())),
                    None => whole = false,
                }
            }
            let zipped = if whole {
                match zip::write(&held) {
                    Ok(z) => Some(z),
                    Err(why) => return say(refusal(2, tr!("バンドルの zip を書けません: ", "the bundle's zip cannot be written: ").then(&why), lang), answer),
                }
            } else {
                None
            };
            if let Err(r) = written_once(&p, REQUESTS, lang) {
                return say(r, answer);
            }
            made.push((out.join(REQUESTS), REQUESTS, p, false));
            made.push((out.clone(), REQUESTS, Package { files: Vec::new(), bytes: vec![(bundle::ZIP.to_string(), zipped, None)], alone: true }, false));
        } else {
            made.push((out.join(REQUESTS), REQUESTS, p, false));
        }
    }
    // the requests whose lists of the versions of the organization have errors, and a request whose
    // code is not written yet, said once the run is sure to write the rest
    if !targets.is_empty() {
        let (said, warned) = listings_said(&project, &joined, &laid, lang);
        if json {
            answer.diagnostics.extend(warned);
        } else {
            print!("{said}");
        }
    }
    if !laid.left.is_empty() && (!targets.is_empty() || requests_given) {
        let (said, warned) = requests_left_out(&laid, !targets.is_empty(), requests_given, lang);
        if json {
            answer.diagnostics.extend(warned);
        } else {
            print!("{said}");
        }
    }
    if targets.contains(&Target::Rust) {
        let (said, warned) = rust::left_out(&project, lang);
        if json {
            answer.diagnostics.extend(warned);
        } else {
            print!("{said}");
        }
    }
    let mut st = Emitted::default();
    for (dir, target, p, library) in &made {
        let emitted = emit(dir, *target, p, check, *library, &extras, &mut st, lang);
        answer.files.append(&mut st.files);
        if let Err(r) = emitted {
            return say(r, answer);
        }
    }
    // when `cedar` is not given, what an earlier run wrote into <out>/cedar/ is said and left as it is
    if !cedar_given {
        for p in leftovers(&out.join("cedar"), &BTreeSet::new(), false) {
            let shown = p.display().to_string();
            st.note(CEDAR, &shown, &None, "left");
            st.unasked.push(shown);
        }
        answer.files.append(&mut st.files);
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
        for p in &st.unasked {
            let _ = writeln!(stdout, "{}", tr!("もう生成しないファイルが残っています: {p}", "generated no more, and still there: {p}").get(lang));
        }
        for p in &st.unasked_extras {
            let _ = writeln!(stdout, "{}", tr!("もう生成しないファイルが残っています: {p}", "generated no more, and still there: {p}").get(lang));
        }
        if !st.unasked_extras.is_empty() {
            let _ = writeln!(
                stdout,
                "{}",
                tr!(
                    "注記: これらは `--extra wasm` か `--extra bundle` が書くファイルで、この実行は頼んでいません。gen は頼まれていない付属物のファイルに触らないので、要らなければ消してかまいません",
                    "note: `--extra wasm` or `--extra bundle` writes these files, and this run does not ask for it; gen touches no file of an extra it is not given, so they can be deleted if they are not wanted"
                )
                .get(lang)
            );
        }
        if !st.unasked.is_empty() {
            let _ = writeln!(
                stdout,
                "{}",
                tr!(
                    "注記: これらは `--target cedar` が書くファイルで、この実行は頼んでいません。gen は指定していない出力先のファイルに触らないので、要らなければ消してかまいません。`--target cedar` が書く二つの JSON（`.cedarschema.json`、`.policies.json`）は、頭のコメントが無いので挙げません",
                    "note: `--target cedar` writes these files, and this run does not ask for it; gen touches no file of a target it is not given, so they can be deleted if they are not wanted; the two JSON files it writes (`.cedarschema.json`, `.policies.json`) have no head to tell them by, and are not listed"
                )
                .get(lang)
            );
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

/// `f`, with the files the requests are lowered to laid over the files the languages read
/// ([`ritsu_project::requests::Laid`]); as it is when none is laid, so that a project with no request
/// reads its files as it did before there were requests.
fn over<R>(laid: &ritsu_project::requests::Laid, api: &Option<Rc<dyn ritsu_base::fs::Files>>, f: impl FnOnce() -> R) -> R {
    if !laid.files.is_empty() {
        // laid over the API the run writes, when it is laid ([`api_laid`])
        ritsu_base::fs::with(laid.overlay.clone(), f)
    } else {
        within(api, f)
    }
}

/// `f`, with the API the run writes laid where it writes it ([`api_laid`]); as it is when none is.
fn within<R>(api: &Option<Rc<dyn ritsu_base::fs::Files>>, f: impl FnOnce() -> R) -> R {
    match api {
        Some(files) => ritsu_base::fs::with(files.clone(), f),
        None => f(),
    }
}

/// The server's API `--target requests` writes (`<out>/requests/openapi.json`, ringi's DESIGN
/// 9.3), made from the requests before the check and laid where the run writes it, so that what
/// reads it (`use openapi`: a flow, a gate) is checked against the API the run writes, on the first
/// run too, when it is not written yet. The API holds what the requests take (their forms, the
/// operations of their tickets) and nothing of their flows and gates: it is made with every flow
/// and every gate taken to pass, unread (ringi's `Engine::set_unread`), as one that reads it
/// cannot pass before it is there: a flow a request reads (`use flow`), a gate it reads (`use
/// gate`), and a workflow such a gate names (`workflow … from`). They are checked on it once it is
/// laid. It is made by the run's own ringi, which keeps what it checks so: a request and the data it
/// reads are checked once in the run (DESIGN 4.15), and again only where a flow or a gate it reads
/// does not pass. None when nothing the project reaches reads the API the run writes
/// ([`reads_the_api`]), or no request passes.
fn api_laid(project: &Project, joined: &Joined, doc: &Path) -> Option<Rc<dyn ritsu_base::fs::Files>> {
    if !reads_the_api(project, joined, doc) {
        return None;
    }
    let files: Vec<PathBuf> = project.of(Tool::Ringi).iter().map(|f| PathBuf::from(&f.shown)).collect();
    // the run's ringi, told the run's language, the API it writes and to keep the lists apart before
    // (`generate`), reading no flow and no gate until the API is made
    joined.ringi.set_unread(true);
    let requests: &dyn ritsu_ports::Requests = joined.ringi.as_ref();
    let passing: Vec<PathBuf> = files.iter().filter(|f| matches!(requests.openapi(std::slice::from_ref(*f), &project.root), Ok(ritsu_ports::Found::Value(_)))).cloned().collect();
    let made = if passing.is_empty() { None } else { requests.openapi(&passing, &project.root).ok() };
    joined.ringi.set_unread(false);
    let Some(ritsu_ports::Found::Value(api)) = made else { return None };
    let mut ov = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    ov.put(doc, api);
    Some(Rc::new(ov))
}

/// Whether a flow or a gate the project reaches reads the API the run writes (`doc`, by `use
/// openapi`): one of the project, a flow a request or a shared route reads (`use flow`), a gate they
/// read (`use gate`, and the gates it reads), a workflow such a gate names (`workflow … from`), or a
/// flow such a flow runs as its child. One that reads another API leaves the run as it is. They are
/// read as each file is written (ringi's and dandori's references, a flow's and a gate's `use` lines
/// with their parsers), with no file checked: a check of what reads the API the run writes, before
/// the API is laid, would be kept for the run.
fn reads_the_api(project: &Project, joined: &Joined, doc: &Path) -> bool {
    let root = &project.root;
    let doc = ritsu_base::paths::opened(doc);
    // the file a `use` line of the file at `rel` writes, as the system opens it (the root aside: a
    // flow may read the API where the run writes it outside the root)
    let is_doc = |rel: &str, written: &str| {
        let from = ritsu_base::paths::on_disk(root, rel);
        ritsu_base::paths::opened(&from.parent().unwrap_or(root).join(written)) == doc
    };
    let tools = [Tool::Ringi, Tool::Dandori, Tool::Sekisho];
    let mut todo: Vec<(Tool, String)> = tools.iter().flat_map(|t| project.of(*t)).map(|f| (f.tool, f.rel.clone())).collect();
    let mut seen: BTreeSet<(Tool, String)> = BTreeSet::new();
    while let Some((tool, rel)) = todo.pop() {
        if !seen.insert((tool, rel.clone())) {
            continue;
        }
        if tool == Tool::Sekisho {
            let (apis, next) = gate_reads(root, &rel);
            if apis.iter().any(|w| is_doc(&rel, w)) {
                return true;
            }
            todo.extend(next);
            continue;
        }
        if tool == Tool::Dandori && flow_apis(root, &rel).iter().any(|w| is_doc(&rel, w)) {
            return true;
        }
        let Some(Ok(named)) = joined.index.references(tool, root, &rel) else { continue };
        todo.extend(named.into_iter().filter(|r| tools.contains(&r.target.tool) && r.target.items.is_empty()).map(|r| (r.target.tool, r.target.path)));
    }
    false
}

/// The OpenAPI documents the flow at `rel` reads (`use openapi`), as written, read with dandori's
/// parser.
fn flow_apis(root: &Path, rel: &str) -> Vec<String> {
    let Ok(src) = ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(root, rel)) else { return Vec::new() };
    let Ok(prog) = dandori::syntax::parse(&src) else { return Vec::new() };
    prog.apis.into_iter().filter(|a| matches!(a.kind, dandori::apis::ApiKind::OpenApi)).map(|a| a.path).collect()
}

/// The OpenAPI documents the gate at `rel` reads (`use openapi`), as written; and the gates it reads
/// (`use gate`) and the `.flow` of each workflow it names (`workflow … from`), from the root. Read
/// with sekisho's parser, as the gate is written: sekisho's references are what its check found,
/// and its check reads those flows.
fn gate_reads(root: &Path, rel: &str) -> (Vec<String>, Vec<(Tool, String)>) {
    let disk = ritsu_base::paths::on_disk(root, rel);
    let Ok(src) = ritsu_base::fs::read_to_string(&disk) else { return (Vec::new(), Vec::new()) };
    let parsed = sekisho::parse::parse(&ritsu_base::paths::show(&disk), &src);
    let Some(f) = parsed.file.filter(|_| !sekisho::diag::has_errors(&parsed.diags)) else { return (Vec::new(), Vec::new()) };
    let apis = f.uses.iter().filter(|u| matches!(u.kind, sekisho::ast::UseKind::Openapi)).map(|u| u.path.0.clone()).collect();
    let from_root = |written: &str| ritsu_base::modules::join_opened(root, rel, written).ok();
    let gates = f.uses.iter().filter(|u| matches!(u.kind, sekisho::ast::UseKind::Gate)).filter_map(|u| from_root(&u.path.0)).map(|p| (Tool::Sekisho, p));
    let flows = f.workflows.iter().filter_map(|w| from_root(&w.flow.0)).map(|p| (Tool::Dandori, p));
    (apis, gates.chain(flows).collect())
}

/// The requests whose code the packages leave out, and which `requests` leaves out of the kinds of
/// request it describes, as `ritsu gen` says them (a warning each, the other parts are written):
/// what of each the glue that joins it to the machine of its tickets does not lower yet (ringi's
/// DESIGN 12.1). `languages` and `requests` are whether the run writes the packages of languages,
/// and the kinds of request apart from any language: either says the request it leaves out.
fn requests_left_out(laid: &ritsu_project::requests::Laid, languages: bool, requests: bool, lang: Lang) -> (String, Vec<Json>) {
    let head = if lang == Lang::Ja { "警告" } else { "warning" };
    let mut said = String::new();
    let mut warned = Vec::new();
    for (f, why) in &laid.left {
        let shown = &f.shown;
        let (wja, wen) = (&why.ja, &why.en);
        let message = match (languages, requests) {
            (true, false) => tr!("`{shown}` のチケットのコードは書きません（{wja}）。ほかの部分は書きます", "The code of the tickets of `{shown}` is not written ({wen}); the other parts are written"),
            (false, _) => tr!("`{shown}` は `requests` の説明に入りません（{wja}）。ほかの部分は書きます", "`requests` does not describe `{shown}` ({wen}); the other parts are written"),
            (true, true) => tr!(
                "`{shown}` のチケットのコードは書かず、`requests` の説明にも入りません（{wja}）。ほかの部分は書きます",
                "The code of the tickets of `{shown}` is not written, nor does `requests` describe it ({wen}); the other parts are written"
            ),
        };
        let message = message.get(lang);
        said.push_str(&format!("{head}: {message}\n"));
        let json = Json::obj([("code", Json::Null), ("severity", Json::str(Severity::Warning.key())), ("line", Json::Null), ("message", Json::str(message))]);
        warned.push(crate::check::entry(Tool::Ringi.word(), &Finding { code: String::new(), severity: Severity::Warning, file: Some(f.rel.clone()), line: None, text: String::new(), json }));
    }
    (said, warned)
}

/// The requests whose lists of the versions of the organization have errors (ringi's E502–E504,
/// E508, E509), as `ritsu gen` says them: a warning each, with the codes and how many of each. They
/// do not stop the package, whose code takes the organization at each event (ringi's DESIGN 12.3);
/// `ritsu check` says what they are.
fn listings_said(project: &Project, joined: &Joined, laid: &ritsu_project::requests::Laid, lang: Lang) -> (String, Vec<Json>) {
    let head = if lang == Lang::Ja { "警告" } else { "warning" };
    let mut said = String::new();
    let mut warned = Vec::new();
    for f in project.of(Tool::Ringi) {
        let errors = joined.ringi.listing_errors(Path::new(&f.shown));
        if errors.is_empty() {
            continue;
        }
        let shown = &f.shown;
        let ja: Vec<String> = errors.iter().map(|(c, n)| format!("{c} が {n} 件")).collect();
        let en: Vec<String> = errors.iter().map(|(c, n)| format!("{c}: {n}")).collect();
        // a request whose code is left out for what its glue does not lower yet is said so apart
        let written = !laid.left.iter().any(|(l, _)| l.rel == f.rel);
        let (ja, en) = (ja.join("、"), en.join(", "));
        let message = if written {
            tr!(
                "`{shown}` の、組織の版ごとの承認者と申請者の一覧に誤りがあります（{ja}）。中身は `ritsu check` が言います。コードは組織をイベントごとに受け取るので、書きます",
                "The lists of the approvers and the requesters of each version of the organization of `{shown}` have errors ({en}), which `ritsu check` says; its code is written, as it takes the organization at each event"
            )
        } else {
            tr!(
                "`{shown}` の、組織の版ごとの承認者と申請者の一覧に誤りがあります（{ja}）。中身は `ritsu check` が言います。コードは組織をイベントごとに受け取るので、この一覧ではパッケージを止めません",
                "The lists of the approvers and the requesters of each version of the organization of `{shown}` have errors ({en}), which `ritsu check` says; they do not stop the package, whose code takes the organization at each event"
            )
        };
        let message = message.get(lang);
        said.push_str(&format!("{head}: {message}\n"));
        let json = Json::obj([("code", Json::Null), ("severity", Json::str(Severity::Warning.key())), ("line", Json::Null), ("message", Json::str(message))]);
        warned.push(crate::check::entry(Tool::Ringi.word(), &Finding { code: String::new(), severity: Severity::Warning, file: Some(f.rel.clone()), line: None, text: String::new(), json }));
    }
    (said, warned)
}

/// What stops `ritsu gen` given no target and no `target` line (DESIGN 9.6): that it writes only the
/// targets given, and the line that writes what 0.40.0 wrote with no `--target` — the words given
/// (`typed`, the words after `gen`), then `--target` of each of [`UP_TO_0_40`], and of `cedar` when
/// the project has gates or `<out>/cedar/` is there, as 0.40.0 wrote the Cedar then. Under
/// `--library`, 0.40.0 wrote the languages of the module's `library` lines, and stopped when it had
/// none: then there is no line to say. The JSON's `instead` is the line, as its words.
fn no_target(typed: &[String], project: &Project, library: bool, out: &Path, lang: Lang) -> (Refusal, Option<Vec<String>>) {
    let names = target_names().join("|");
    let first = if project.tree.is_some() {
        tr!(
            "`ritsu gen` が書くのは、指定した出力先だけです。`--target {names}` を一つ以上付けるか、モジュールの ritsu.mod に `target` の行（`target typescript python go`）を書いてください",
            "`ritsu gen` writes only the targets it is given: give one or more `--target {names}`, or write them on the `target` line of the module's ritsu.mod (`target typescript python go`)"
        )
    } else {
        tr!(
            "`ritsu gen` が書くのは、指定した出力先だけです。`--target {names}` を一つ以上付けるか、モジュールにして（`ritsu mod init`）ritsu.mod に `target` の行（`target typescript python go`）を書いてください",
            "`ritsu gen` writes only the targets it is given: give one or more `--target {names}`, or make it a module (`ritsu mod init`) and write them on the `target` line of its ritsu.mod (`target typescript python go`)"
        )
    };
    let held = if library { library::whole_module(project.clone()) } else { project.clone() };
    let cedar = !held.of(Tool::Sekisho).is_empty() || out.join(CEDAR).exists();
    let langs: Vec<&str> = if library { library::languages(project).into_iter().map(Target::key).filter(|k| UP_TO_0_40.contains(k)).collect() } else { UP_TO_0_40.to_vec() };
    if langs.is_empty() {
        return (refusal(2, first, lang), None);
    }
    let mut words: Vec<String> = ["ritsu", "gen"].into_iter().map(String::from).chain(typed.iter().cloned()).collect();
    for t in langs.into_iter().chain(cedar.then_some(CEDAR)) {
        words.extend(["--target".to_string(), t.to_string()]);
    }
    let line = words.iter().map(|w| ritsu_base::cli::shell_word(w)).collect::<Vec<_>>().join(" ");
    let what = match (library, cedar) {
        (false, false) => tr!(
            "注記: 0.40.0 までの `ritsu gen` は、`--target` が無ければ、TypeScript・Python・Go のパッケージを書きました。同じファイルを書くのは次のコマンドです",
            "note: up to 0.40.0, `ritsu gen` with no `--target` wrote the packages of TypeScript, Python and Go; this writes the same files:"
        ),
        (false, true) => tr!(
            "注記: 0.40.0 までの `ritsu gen` は、`--target` が無ければ、TypeScript・Python・Go のパッケージと、ゲートの Cedar を書きました。同じファイルを書くのは次のコマンドです",
            "note: up to 0.40.0, `ritsu gen` with no `--target` wrote the packages of TypeScript, Python and Go, and the gates' Cedar; this writes the same files:"
        ),
        (true, false) => tr!(
            "注記: 0.40.0 までの `ritsu gen --library` は、`--target` が無ければ、`library` の行の言語ごとにライブラリを書きました。同じファイルを書くのは次のコマンドです",
            "note: up to 0.40.0, `ritsu gen --library` with no `--target` wrote a library for each language of the `library` lines; this writes the same files:"
        ),
        (true, true) => tr!(
            "注記: 0.40.0 までの `ritsu gen --library` は、`--target` が無ければ、`library` の行の言語ごとのライブラリと、ゲートの Cedar を書きました。同じファイルを書くのは次のコマンドです",
            "note: up to 0.40.0, `ritsu gen --library` with no `--target` wrote a library for each language of the `library` lines, and the gates' Cedar; this writes the same files:"
        ),
    };
    let said = tr!("{}\n{}\n  {}", "{}\n{}\n  {}", first.ja, what.ja, line; first.en, what.en, line);
    (refusal(2, said, lang), Some(words))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The last line of a check that stops `ritsu gen` names the files by why they stop it: a
    /// request whose screens alone cannot be made passes check, and is said as a file a target
    /// cannot be made from is said (ringi's DESIGN 13.3).
    #[test]
    fn what_stops_gen_is_named_by_why_it_stops() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let said = |failing: &[&str], screens: &[&str], waiting: &[&str]| {
            let t = nothing_generated(&s(failing), &s(screens), &s(waiting));
            (t.en, t.ja)
        };
        assert_eq!(said(&[], &[], &[]).0, "the check finds errors, so nothing is generated");
        assert_eq!(said(&["a.ringi"], &[], &[]).0, "1 file(s) do not pass check, so nothing is generated: a.ringi");
        assert_eq!(said(&[], &["b.ringi"], &[]), ("`b.ringi` cannot be made into the requests package".into(), "`b.ringi` からは requests のパッケージを生成できません".into()));
        assert_eq!(said(&[], &["b.ringi", "c.ringi"], &[]).0, "`b.ringi` and `c.ringi` cannot be made into the requests package");
        assert_eq!(
            said(&["a.ringi"], &["b.ringi"], &["w.flow"]),
            (
                "1 file(s) do not pass check, so nothing is generated: a.ringi; `b.ringi` cannot be made into the requests package; 1 file(s) wait for them: w.flow".into(),
                "検査を通らないファイルが 1 個あるので、何も生成しません: a.ringi。`b.ringi` からは requests のパッケージを生成できません。それを待っているファイル 1 個: w.flow".into()
            )
        );
    }

    /// The names `--target` takes are the words of the `target` line of a ritsu.mod (DESIGN 17.3),
    /// in the order they are written in: ritsu-base reads the line, and cannot read [`Target`].
    #[test]
    fn the_targets_are_the_words_of_the_target_line() {
        assert_eq!(target_names(), ritsu_base::modules::manifest::TARGET_WORDS);
        assert_eq!(in_order(&["cedar", "go", "typescript"]), ["typescript", "go", "cedar"]);
    }

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

#[cfg(test)]
mod output_tests {
    use super::*;

    /// Two files whose paths differ only in their Unicode normalization (`が` as one character, and
    /// as `か` with the combining mark) are one file where the file system does not tell them apart.
    /// No project of a macOS file system can hold the two, so the check is tested here.
    #[test]
    fn two_paths_that_differ_only_in_normalization_are_one_file() {
        let p = Package {
            files: vec![
                ("requests/が.json".to_string(), String::new(), Some("a.ringi".to_string())),
                ("requests/か\u{3099}.json".to_string(), String::new(), Some("b.ringi".to_string())),
            ],
            bytes: Vec::new(),
            alone: false,
        };
        let Err(r) = written_once(&p, REQUESTS, Lang::En) else { panic!("the two paths are one file") };
        assert!(r.message.contains("differ only in the case of their letters or in Unicode normalization"), "{}", r.message);
        let Err(r) = written_once(&p, REQUESTS, Lang::Ja) else { panic!("the two paths are one file") };
        assert!(r.message.contains("大文字と小文字か Unicode の正規化だけが違います"), "{}", r.message);
    }

    /// A file of ritsu's that no run plans any more is found below the package's directory, but not
    /// through a symbolic link there: the link leads out, and nothing it leads to is removed.
    #[cfg(unix)]
    #[test]
    fn the_leftovers_are_not_looked_for_through_a_link() {
        let d = std::env::temp_dir().join(format!("ritsu-leftovers-link-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (package, outside) = (d.join("typescript"), d.join("outside"));
        std::fs::create_dir_all(package.join("rules")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let generated = "// Code generated by rulec 1.0.0. DO NOT EDIT.\n";
        std::fs::write(package.join("rules").join("old.ts"), generated).unwrap();
        std::fs::write(outside.join("other.ts"), generated).unwrap();
        std::os::unix::fs::symlink(&outside, package.join("linked")).unwrap();
        assert_eq!(leftovers(&package, &BTreeSet::new(), false), vec![package.join("rules").join("old.ts")]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_link_below_the_output_is_refused() {
        let d = std::env::temp_dir().join(format!("ritsu-out-link-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/tmp", d.join("requests")).unwrap();
            assert_eq!(link_below(&d, &d.join("requests").join("x")), Some(d.join("requests")));
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_source_the_bundle_before_listed_and_this_one_does_not_is_removed() {
        let d = std::env::temp_dir().join(format!("ritsu-bundle-stale-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let dir = d.join("requests");
        let about = |name: &str| format!("{{\"sources\":[{{\"path\":\"source/{name}\",\"sha256\":\"x\"}}],\"diff\":null}}\n");
        for (name, gone) in [("office.cal", false), ("holidays.cal", true)] {
            let p = Package {
                files: vec![(bundle::ABOUT.to_string(), about(name), None), (format!("source/{name}"), "calendar x v1\n".into(), None)],
                bytes: Vec::new(),
                alone: false,
            };
            let mut st = Emitted::default();
            emit(&dir, REQUESTS, &p, false, false, &["bundle"], &mut st, Lang::En).unwrap_or_else(|e| panic!("{}", e.said));
            if gone {
                assert!(!dir.join("source/office.cal").is_file(), "the source the bundle before listed is still there");
                assert!(st.removed.iter().any(|p| p.ends_with("source/office.cal")), "{:?}", st.removed);
            }
        }
        assert!(dir.join("source/holidays.cal").is_file());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_a_source_the_bundle_before_listed_is_refused() {
        let d = std::env::temp_dir().join(format!("ritsu-bundle-link-stale-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let dir = d.join("requests");
        let about = |path: &str| format!("{{\"sources\":[{{\"path\":\"{path}\",\"sha256\":\"x\"}}],\"diff\":null}}\n");
        let first = Package {
            files: vec![(bundle::ABOUT.to_string(), about("source/old/office.cal"), None), ("source/old/office.cal".into(), "calendar x v1\n".into(), None)],
            bytes: Vec::new(),
            alone: false,
        };
        let mut st = Emitted::default();
        emit(&dir, REQUESTS, &first, false, false, &["bundle"], &mut st, Lang::En).unwrap_or_else(|e| panic!("{}", e.said));
        let outside = d.join("outside");
        std::fs::create_dir(&outside).unwrap();
        let kept = outside.join("office.cal");
        std::fs::write(&kept, "keep\n").unwrap();
        std::fs::remove_dir_all(dir.join("source/old")).unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("source/old")).unwrap();
        let second = Package {
            files: vec![(bundle::ABOUT.to_string(), about("source/holidays.cal"), None), ("source/holidays.cal".into(), "calendar x v1\n".into(), None)],
            bytes: Vec::new(),
            alone: false,
        };
        let mut st = Emitted::default();
        assert!(emit(&dir, REQUESTS, &second, false, false, &["bundle"], &mut st, Lang::En).is_err());
        assert!(kept.is_file(), "the file outside the output was removed");
        let _ = std::fs::remove_dir_all(&d);
    }
}
