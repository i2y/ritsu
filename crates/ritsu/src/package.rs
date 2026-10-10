//! `ritsu gen` (DESIGN 9.3): the rules, the dates, the clients of the books, the workflows and the
//! code that asks the gates of a project, as one package for each of TypeScript, Python and Go,
//! with the gates' Cedar beside them, written once.
//!
//! ```text
//! generated/typescript/          generated/python/              generated/go/
//!   package.json                   pyproject.toml                 doc.go
//!   index.ts                       <name>/__init__.py, py.typed
//!   rules/<alias>.ts               <name>/rules/<alias>.py        rules/<package>/<alias>.go
//!   dates/<alias>.ts               <name>/dates/<alias>.py        dates/<package>/<package>.go
//!   books/<book>.ts                <name>/books/<book>.py         books/<package>/book.go
//!   flows/<flow>/…                 <name>/flows/<flow>/…          flows/<package>/…
//!   authz/<gate>.ts
//! generated/cedar/<gate>.cedar, .cedarschema, .cedarschema.json, .policies.json
//! ```
//!
//! Each language writes its part with its own generator (DESIGN 9.1): rulec the rule's module,
//! koyomi the dates file's, chobo the client of the book (and, on PostgreSQL, the SQL it calls),
//! dandori the workflow for Temporal, sekisho the gate's Cedar and the code that builds its
//! requests and asks them (sekisho's DESIGN 5.6), which reads the package's rules/ and dates/. A workflow reads the rules, the dates and the books from the
//! package's rules/, dates/ and books/ (dandori's `InPackage`), not from code put beside it: the
//! imports name the package's modules, and the books the transport takes are typed with the
//! package's clients, so that the type checker of the target holds the two languages to each
//! other. Every file begins with the same head (ritsu's version, and the file it is made from by
//! its path from the project's root; DESIGN 9.2). `--check` writes nothing and says what is stale,
//! as `rulec gen --check` does.

use crate::cli;
use ritsu_base::diag::Severity;
use ritsu_base::json::Json;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_emit::header::{Comment, Origin};
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
/// is a directory of the module it is put in, so its requirements are that module's go.mod's.
const GO_MODULES: &[(&str, &str)] = &[
    ("go.temporal.io/sdk", "v1.49.0"),
    ("go.temporal.io/api", "v1.63.5"),
    ("github.com/jackc/pgx/v5", "v5.11.0"),
    ("github.com/tigerbeetle/tigerbeetle-go", "v0.17.9"),
    ("github.com/aws/aws-sdk-go-v2", "v1.47.1"),
    ("github.com/aws/aws-sdk-go-v2/config", "v1.33.6"),
    ("github.com/aws/aws-sdk-go-v2/service/lambda", "v1.110.0"),
    ("github.com/aws/aws-sdk-go-v2/service/sns", "v1.47.2"),
    ("github.com/aws/aws-sdk-go-v2/service/sqs", "v1.52.1"),
    ("github.com/aws/smithy-go", "v1.28.1"),
    ("github.com/openai/openai-go/v3", "v3.68.0"),
    ("github.com/anthropics/anthropic-sdk-go", "v1.78.0"),
    ("google.golang.org/protobuf", "v1.36.11"),
    ("github.com/cedar-policy/cedar-go", "v1.8.0"),
    ("github.com/aws/aws-sdk-go-v2/service/verifiedpermissions", "v1.41.1"),
];
/// Modules the ones above ask for at a version with a known vulnerability, the version the tests
/// run instead (the go.mod of chobo's tools/runner/go and of dandori's tools/temporal-go), and the
/// advisory. `go mod tidy` gives a module the version a requirement asks for, not the newest (Go's
/// minimal version selection), so a package that imports the module on the left would otherwise
/// be built with the vulnerable one; doc.go says to raise it (DESIGN 9.3). pgx v5.11.0 asks for
/// golang.org/x/text v0.29.0, whose normalization its SCRAM authentication reaches (GO-2026-5970,
/// fixed in v0.39.0). An entry goes once the module on the left asks for a fixed version.
const GO_RAISED: &[(&str, &str, &str, &str)] = &[("github.com/jackc/pgx/v5", "golang.org/x/text", "v0.42.0", "GO-2026-5970")];

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

/// Whether every file of the four languages a package is made of passes its language's check; the
/// diagnostics of those that do not, as `ritsu check` prints them, when one does not.
fn checked(project: &Project, joined: &Joined, lang: Lang) -> Result<(), Refusal> {
    let mut said = String::new();
    let mut found: Vec<Json> = Vec::new();
    let mut worst = 0u8;
    let mut failing: Vec<String> = Vec::new();
    for tool in [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Sekisho] {
        let files: Vec<String> = project.of(tool).iter().map(|f| f.shown.clone()).collect();
        if files.is_empty() {
            continue;
        }
        let units = match tool {
            Tool::Rulec => joined.rulec.checked(&project.root, &files, lang),
            Tool::Koyomi => joined.koyomi.checked(&project.root, &files, lang),
            Tool::Sekisho => sekisho::check::checked(&project.root, &files, &joined.sekisho().into(), lang),
            _ => joined.chobo.checked(&project.root, &files, lang),
        };
        for u in units.into_iter().filter(|u| u.verdict != Verdict::Passes) {
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
            failing.push(u.label);
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

/// `f`, with the ports of the rules, the dates and the books joined, as dandori reads a flow.
fn with_ports<R>(joined: &Joined, f: impl FnOnce() -> R) -> R {
    let dates: Rc<dyn ritsu_ports::Dates> = joined.koyomi.clone();
    let books: Rc<dyn ritsu_ports::Books> = joined.chobo.clone();
    dandori::sources::with_ports(joined.rules(), dates, books, f)
}

/// The package of `target`, made from the project's files (which pass their checks).
pub fn package(project: &Project, joined: &Joined, target: Target, o: &Options) -> Result<Package, Refusal> {
    let lang = o.lang;
    let mut files: Vec<(String, String, Option<String>)> = Vec::new();
    // what goes under each kind's directory, for the indexes: each module by its name
    let mut kinds: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    // and the file of the project each is made from
    let mut named: Vec<(&str, String, String)> = Vec::new();
    let base = match target {
        Target::Python => format!("{}/", o.name),
        _ => String::new(),
    };
    // rulec: each rule's module
    let rulec_lang = if lang == Lang::Ja { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En };
    for f in project.of(Tool::Rulec) {
        let src = read(project, f, lang)?;
        let (rel, body) = rulec::i18n::with(rulec_lang, || rulec::codegen::package_module(&src, &f.shown, &f.rel, target.key())).map_err(|e| refusal(1, Text::new(e.clone(), e), lang))?;
        named.push(("rules", module_of(&rel), f.rel.clone()));
        kinds.entry("rules").or_default().push(module_of(&rel));
        files.push((format!("{base}rules/{rel}"), body, Some(f.rel.clone())));
    }
    // koyomi: each dates file's and calendar's module
    let mut loader = koyomi::calendar::Loader::default();
    for f in project.of(Tool::Koyomi) {
        let outcome = koyomi::check::check_file(&f.shown, &koyomi::check::Options::default(), &mut loader).map_err(|e| refusal(2, Text::new(e.clone(), e), lang))?;
        let Some(checked) = outcome.checked.as_ref().filter(|_| !outcome.has_errors()) else {
            let shown = &f.shown;
            return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), lang));
        };
        let u = koyomi::codegen::unit_shown(checked, lang, Some(&f.rel));
        let (t, body) = match target {
            Target::TypeScript => (koyomi::naming::Target::TypeScript, koyomi::codegen::typescript::module(&u)),
            Target::Python => (koyomi::naming::Target::Python, koyomi::codegen::python::module(&u)),
            Target::Go => (koyomi::naming::Target::Go, koyomi::codegen::go::module(&u)),
        };
        // koyomi gen's own path, under its target's directory
        let rel = t.file(&u.alias).split_once('/').map(|(_, r)| r.to_string()).unwrap_or_default();
        named.push(("dates", module_of(&rel), f.rel.clone()));
        kinds.entry("dates").or_default().push(module_of(&rel));
        files.push((format!("{base}dates/{rel}"), body, Some(f.rel.clone())));
    }
    // chobo: each book's client, and on PostgreSQL the schema and the functions it calls
    for f in project.of(Tool::Chobo) {
        let (src, c) = chobo::check::check_file(Path::new(&f.shown)).map_err(|e| refusal(2, Text::new(e.clone(), e), lang))?;
        let Some(book) = c.book.as_ref() else {
            let shown = &f.shown;
            return Err(refusal(1, tr!("`{shown}` は検査を通らないので、生成しません", "`{shown}` does not pass check, so nothing is generated from it"), lang));
        };
        let stem = Path::new(&f.rel).file_name().map(|n| n.to_string_lossy().trim_end_matches(".book").to_string()).unwrap_or_default();
        let origin = Origin::at(&f.rel, src.as_bytes());
        let mut wanted = vec![o.books.chobo(target)];
        if o.books == Books::Postgres {
            wanted.push(chobo::target::Target::Postgres);
        }
        for ct in wanted {
            let built = chobo::target::build(book, &stem, ct, &origin).map_err(|ds| {
                let text: String = ds.iter().map(|d| chobo::diag::Show::shown(d, &f.shown, &src, lang)).collect();
                let json = ds.iter().map(|d| diagnostic("chobo", Some(f.rel.clone()), from_serde(&chobo::diag::Show::json_in(d, &f.shown, &src, lang)))).collect();
                let (shown, to) = (&f.shown, target.key());
                stopped(text, 1, tr!("`{shown}` からは {to} のパッケージを生成できません", "`{shown}` cannot be made into the {to} package"), json, lang)
            })?;
            for (rel, body) in built {
                if ct != chobo::target::Target::Postgres {
                    named.push(("books", module_of(&rel), f.rel.clone()));
                    kinds.entry("books").or_default().push(module_of(&rel));
                }
                files.push((format!("{base}books/{rel}"), body, Some(f.rel.clone())));
            }
        }
    }
    // dandori: each workflow, for Temporal in the language, reading the package's rules, dates and books
    let module = o.module.clone();
    for f in project.of(Tool::Dandori) {
        let built = with_ports(joined, || -> Result<(Vec<(String, String)>, String, String), Refusal> {
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
            m.package = Some(dandori::model::InPackage { go_module: module.clone() });
            let dir = match target {
                Target::Go => dandori::temporal_go::package(&m),
                _ => dandori::render::ident(&m.name),
            };
            match dandori::commands::build(&m, target.dandori()) {
                Some(Ok(files)) => Ok((files, dir, src)),
                Some(Err(diags)) => {
                    let json = diags.iter().map(|d| diagnostic("dandori", Some(f.rel.clone()), from_serde(&d.to_json(lang)))).collect();
                    let (shown, to) = (&f.shown, target.key());
                    Err(stopped(dandori::commands::render(&diags, &f.shown, &src, lang), 1, tr!("`{shown}` からは {to} のパッケージを生成できません", "`{shown}` cannot be made into the {to} package"), json, lang))
                }
                None => unreachable!("the three targets are dandori's"),
            }
        })?;
        let (built, dir, _) = built;
        named.push(("flows", dir.clone(), f.rel.clone()));
        kinds.entry("flows").or_default().push(dir);
        for (rel, body) in built {
            files.push((format!("{base}flows/{rel}"), body, Some(f.rel.clone())));
        }
    }
    // sekisho: each gate's code of the requests, which reads the package's rules/ and dates/
    let gates: sekisho::suite::Suite = joined.sekisho().into();
    for f in project.of(Tool::Sekisho) {
        let outcome = gate(project, &gates, f, lang)?;
        let to = sekisho::r#gen::Target { name: target.key(), authorizer: o.authorizer, go_module: o.module.clone() };
        let made = sekisho::r#gen::generate(&outcome, &to, &f.rel, lang, &gates).map_err(|e| {
            let shown = &f.shown;
            refusal(2, tr!("`{shown}` から生成できません（{e}）。sekisho の不具合です", "cannot generate from `{shown}` ({e}): a bug in sekisho"), lang)
        })?;
        for (rel, body) in made {
            // `typescript/authz/refunds.ts` as the package holds it: `authz/refunds.ts`
            let rel = rel.split_once('/').map(|(_, r)| r.to_string()).unwrap_or(rel);
            let module = module_of(rel.strip_prefix("authz/").unwrap_or(&rel));
            named.push(("authz", module.clone(), f.rel.clone()));
            kinds.entry("authz").or_default().push(module);
            files.push((format!("{base}{rel}"), body, Some(f.rel.clone())));
        }
    }
    for list in kinds.values_mut() {
        list.dedup();
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
        let to = sekisho::r#gen::Target { name: "cedar", authorizer, go_module: String::new() };
        let made = sekisho::r#gen::generate(&outcome, &to, &f.rel, lang, &gates).map_err(|e| {
            let shown = &f.shown;
            refusal(2, tr!("`{shown}` から生成できません（{e}）。sekisho の不具合です", "cannot generate from `{shown}` ({e}): a bug in sekisho"), lang)
        })?;
        for (rel, body) in made {
            files.push((rel.strip_prefix("cedar/").unwrap_or(&rel).to_string(), body, Some(f.rel.clone())));
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

/// What the package itself is made of: its index, its manifest with what it depends on, and the
/// package files of Python.
fn around(target: Target, o: &Options, kinds: &BTreeMap<&str, Vec<String>>, files: &[(String, String, Option<String>)]) -> Vec<(String, String, Option<String>)> {
    let head = |c: Comment| c.line(&ritsu_emit::header::generated("ritsu"));
    let what = if kinds.contains_key("authz") {
        "The rules, the dates, the clients of the books, the workflows and the code that asks the gates of a project, as one package (ritsu gen)."
    } else {
        "The rules, the dates, the clients of the books and the workflows of a project, as one package (ritsu gen)."
    };
    let generated = ritsu_emit::header::generated("ritsu");
    let mut out = Vec::new();
    match target {
        Target::TypeScript => {
            let mut index = format!("{}// {what}\n\n", head(Comment::Slashes));
            for (kind, modules) in kinds {
                index.push_str(&format!("export * as {kind} from \"./{kind}/index\";\n"));
                let mut sub = format!("{}// The {kind} of the package, each by its name.\n\n", head(Comment::Slashes));
                for m in modules {
                    // a flow's index is its client: how to start it, answer it and ask it
                    let path = if *kind == "flows" { format!("./{m}/client") } else { format!("./{m}") };
                    sub.push_str(&format!("export * as {m} from \"{path}\";\n"));
                }
                out.push((format!("{kind}/index.ts"), sub, None));
            }
            out.push(("index.ts".to_string(), index, None));
            // what the TypeScript imports from outside the package
            let mut deps: BTreeMap<String, &str> = BTreeMap::new();
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
            out.push(("package.json".to_string(), manifest, None));
        }
        Target::Python => {
            let n = &o.name;
            let mut init = format!("{}\"\"\"{what}\"\"\"\n\n", head(Comment::Hash));
            for kind in kinds.keys() {
                init.push_str(&format!("from . import {kind} as {kind}\n"));
            }
            out.push((format!("{n}/__init__.py"), init, None));
            out.push((format!("{n}/py.typed"), String::new(), None));
            for (kind, modules) in kinds {
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
            let mut deps = Vec::new();
            match (gates, o.authorizer) {
                (true, sekisho::r#gen::Authorizer::Cedar) => deps.push(format!("cedarpy=={CEDARPY}")),
                (true, sekisho::r#gen::Authorizer::Avp) => deps.push(format!("boto3=={BOTO3}")),
                _ => {}
            }
            if flows {
                deps.push(format!("temporalio=={TEMPORAL_PYTHON}"));
            }
            if tigerbeetle {
                deps.push(format!("tigerbeetle=={TIGERBEETLE}"));
            }
            let deps_toml: String = deps.iter().map(|d| format!("  {},\n", ritsu_emit::lit::json(d))).collect();
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
        Target::Go => {
            // the modules the package's Go imports, as doc.go names them
            let mut imported: BTreeSet<&str> = BTreeSet::new();
            for (_, body, _) in files.iter().filter(|(p, _, _)| p.ends_with(".go")) {
                for line in body.lines() {
                    let l = line.trim();
                    let path = l.rsplit('"').nth(1).unwrap_or("");
                    if !l.ends_with('"') || path.is_empty() {
                        continue;
                    }
                    if let Some((m, _)) = GO_MODULES.iter().filter(|(m, _)| path == *m || path.starts_with(&format!("{m}/"))).max_by_key(|(m, _)| m.len()) {
                        imported.insert(m);
                    }
                }
            }
            let package = go_package_name(&o.module);
            let mut doc = if kinds.contains_key("authz") {
                format!("{}\n// Package {package} is the rules, the dates, the clients of the books, the workflows and the code that\n// asks the gates of a project, as one package (ritsu gen): rules/, dates/, books/, flows/ and authz/ under\n// the import path {}.\n", head(Comment::Slashes), o.module)
            } else {
                format!("{}\n// Package {package} is the rules, the dates, the clients of the books and the workflows of a project, as\n// one package (ritsu gen): rules/, dates/, books/ and flows/ under the import path {}.\n", head(Comment::Slashes), o.module)
            };
            if !imported.is_empty() {
                doc.push_str("//\n// It imports these modules, which the go.mod of the module it is in requires (`go mod tidy`); these are\n// the versions it is tested with:\n//\n");
                for m in &imported {
                    let v = GO_MODULES.iter().find(|(x, _)| x == m).map(|(_, v)| *v).unwrap_or("");
                    doc.push_str(&format!("//\t{m} {v}\n"));
                }
                let raised: Vec<_> = GO_RAISED.iter().filter(|(by, ..)| imported.contains(by)).collect();
                if !raised.is_empty() {
                    doc.push_str("//\n// Of the modules those require, these are tested at a later version than the one asked for, which\n// has a known vulnerability; raise them in the go.mod (`go get <module>@<version>`):\n//\n");
                    for (by, m, v, advisory) in raised {
                        doc.push_str(&format!("//\t{m} {v} ({advisory}, through {by})\n"));
                    }
                }
            }
            doc.push_str(&format!("package {package}\n"));
            out.push(("doc.go".to_string(), doc, None));
        }
    }
    out
}

/// The name of the Go package of the directory at `module`: its last part, as a Go identifier.
fn go_package_name(module: &str) -> String {
    let last = module.rsplit('/').next().unwrap_or(module);
    let s: String = last.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    if s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) { s } else { format!("p{s}") }
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
/// does not write: what a rule taken out of the project left behind.
fn leftovers(dir: &Path, planned: &BTreeSet<PathBuf>) -> Vec<PathBuf> {
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
    text.lines().take(3).any(|l| ["rulec", "koyomi", "chobo", "dandori", "sekisho", "ritsu"].iter().any(|t| l.contains(&format!("Code generated by {t} ")) && l.ends_with("DO NOT EDIT.")))
}

/// Write the package into `dir`, or under `check` say what differs from it.
fn emit(dir: &Path, target: &'static str, p: &Package, check: bool, st: &mut Emitted, lang: Lang) -> Result<(), Refusal> {
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
    for p in leftovers(dir, &planned) {
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

/// `ritsu gen [<path>...] [--target typescript|python|go] [--out <dir>] [--check] [--books
/// postgres|tigerbeetle] [--authorizer cedar|avp] [--name <name>] [--module <path>] [--root <dir>]
/// [--format json]`: the exit code.
pub fn command(args: &[String], lang: Lang) -> u8 {
    let table = cli::table();
    let cmd = table.command("gen").expect("gen is in the table");
    let asked = args.iter().enumerate().find_map(|(i, x)| if x == "--lang" { args.get(i + 1).cloned() } else { x.strip_prefix("--lang=").map(str::to_string) });
    let lang = if asked.is_some() { Lang::pick(asked.as_deref(), "RITSU_LANG") } else { lang };
    let json = asks_json(args);
    let mut answer = Answer { root: None, check: args.iter().any(|a| a == "--check"), out: "generated".to_string(), targets: Vec::new(), files: Vec::new(), diagnostics: Vec::new(), error: None };
    let code = generate(args, &table, cmd, lang, json, &mut answer);
    if json {
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
    let books = if a.get("--books") == Some("tigerbeetle") { Books::TigerBeetle } else { Books::Postgres };
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
    if [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Dandori, Tool::Sekisho].iter().all(|t| project.of(*t).is_empty()) {
        let given = project.given.join(" ");
        return say(
            refusal(
                2,
                tr!(
                    "{given} には、パッケージにするファイル（.rule、.cal、.book、.flow、.gate）がありません",
                    "there is no file to make a package of (.rule, .cal, .book, .flow, .gate) in {given}"
                ),
                lang,
            ),
            answer,
        );
    }
    let joined = Joined::new();
    if let Err(r) = checked(&project, &joined, lang) {
        return say(r, answer);
    }
    let o = Options { name, module, books, authorizer, lang };
    let out = PathBuf::from(&answer.out);
    let check = a.has("--check");
    let mut st = Emitted::default();
    for t in targets {
        let p = match package(&project, &joined, t, &o) {
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
        let emitted = emit(&out.join(t.key()), t.key(), &p, check, &mut st, lang);
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
        let emitted = emit(&out.join("cedar"), "cedar", &p, check, &mut st, lang);
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
        let _ = stdout.flush();
    }
    u8::from(check && !(st.missing.is_empty() && st.stale.is_empty() && st.left.is_empty()))
}
