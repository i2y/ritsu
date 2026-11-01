//! What the tests of the generated code share (DESIGN 16.4): the modules they run, one package of a
//! target with every module in it, beside the rules', the dates' and the books' code the modules
//! call (each written by its own language's generator, called here as `ritsu gen` calls it), the
//! cases of every vector of every function and of the inputs outside what the parameters declare, and
//! the comparison of what the code answers with what the reference interpreter answers.

use kaname::check::Checked;
pub use kaname::r#gen::harness::Case;
use kaname::r#gen::Target;
use kaname::value::Program;
use ritsu_base::text::Lang;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The Python package's name (and Rust's crate's) and Go's module and import path of the package's
/// directory, as the tests lay a package out.
pub const PY_NAME: &str = "pkg";
pub const GO_MODULE: &str = "kanametest";
pub const GO_PACKAGE: &str = "kanametest/pkg";

/// The test modules of `tests/modules/`, checked as the file `t.kn` beside the crate (where they read
/// the examples' rules by their paths), and the ones only these tests run.
pub const MODULES: [(&str, &str); 34] = [
    ("calc", include_str!("../modules/calc.kn")),
    ("places", include_str!("../modules/places.kn")),
    ("lists", include_str!("../modules/lists.kn")),
    ("inner", include_str!("../modules/inner.kn")),
    ("rec", include_str!("../modules/rec.kn")),
    ("ranges", include_str!("../modules/ranges.kn")),
    ("walks", include_str!("../modules/walks.kn")),
    ("calls", include_str!("../modules/calls.kn")),
    ("names", include_str!("../modules/names.kn")),
    ("corners", include_str!("../modules/corners.kn")),
    ("bounds", include_str!("../modules/bounds.kn")),
    ("lift", include_str!("../modules/lift.kn")),
    ("lift.ja", include_str!("../modules/lift.ja.kn")),
    ("open", include_str!("../modules/open.kn")),
    ("open.ja", include_str!("../modules/open.ja.kn")),
    ("sums", include_str!("../modules/sums.kn")),
    ("sums.ja", include_str!("../modules/sums.ja.kn")),
    ("temps", include_str!("../modules/temps.kn")),
    ("temps.ja", include_str!("../modules/temps.ja.kn")),
    ("dicts", include_str!("../modules/dicts.kn")),
    ("dicts.ja", include_str!("../modules/dicts.ja.kn")),
    ("lists2", include_str!("../modules/lists2.kn")),
    ("lists2.ja", include_str!("../modules/lists2.ja.kn")),
    ("texts", include_str!("../modules/texts.kn")),
    ("texts.ja", include_str!("../modules/texts.ja.kn")),
    ("generic", include_str!("../modules/generic.kn")),
    ("generic.ja", include_str!("../modules/generic.ja.kn")),
    ("results", include_str!("../modules/results.kn")),
    ("results.ja", include_str!("../modules/results.ja.kn")),
    ("gcalls", include_str!("../modules/gcalls.kn")),
    ("hof", include_str!("../modules/hof.kn")),
    ("hof.ja", include_str!("../modules/hof.ja.kn")),
    ("trees", include_str!("../modules/trees.kn")),
    ("trees.ja", include_str!("../modules/trees.ja.kn")),
];

/// Every module the tests generate: the examples, and the test modules, with the path the head
/// of each names. A module whose names and types pass is generated whatever its analysis finds: what
/// fails where is what the code is held to.
pub fn modules() -> Vec<(Rc<Checked>, String)> {
    let mut out = Vec::new();
    for f in super::EXAMPLES {
        let o = super::check(f);
        out.push((o.analysed.clone().unwrap_or_else(|| panic!("{f}: {}", super::shown(&o, Lang::En))), f.to_string()));
    }
    for (name, src) in MODULES {
        // checked as a file beside the crate, each by its own name, where the paths it reads are
        // written from
        let o = super::check_text(&format!("{name}.kn"), src);
        let c = o.analysed.clone().unwrap_or_else(|| panic!("{name}: {}", super::shown(&o, Lang::En)));
        // the modules it reads go into the package too
        for (canon, m) in &c.world.modules {
            if !out.iter().any(|(x, _): &(Rc<Checked>, String)| x.canon == *canon) {
                out.push((m.clone(), m.path.clone()));
            }
        }
        out.push((c, format!("tests/modules/{name}.kn")));
    }
    out
}

/// Run `f` on a stack as deep as the reference interpreter's recursions need (DESIGN 8).
pub fn deep<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new().stack_size(256 << 20).spawn(f).unwrap().join().unwrap()
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The directory of the package of `t` under `root`: `root` itself for TypeScript, `root/pkg` for
/// Python, Go and Rust (whose crate it is).
pub fn package_dir(root: &Path, t: Target) -> PathBuf {
    match t {
        Target::TypeScript => root.to_path_buf(),
        _ => root.join(PY_NAME),
    }
}

/// The package of `t` for `modules`, written under `root` with the messages in `lang`: the rules', the
/// dates' and the books' code (the books' clients on PostgreSQL), the package's `units`, and
/// kaname's `glue/`. The paths written.
pub fn write_package(root: &Path, t: Target, lang: Lang, modules: &[(Rc<Checked>, String)]) -> Vec<String> {
    write_package_for(root, t, lang, modules, chobo::target::Backend::Postgres)
}

/// [`write_package`] for a package that keeps its books in `backend`: Rust's clients of the books are
/// chobo's of the backend, and its functions that make the transfers take the book by the trait of
/// the backend's clients (DESIGN 36.6); the other targets read none of it. A package of Rust is a
/// crate (ritsu's DESIGN 9.3): its `lib.rs` and the files that declare the modules of `rules/`,
/// `dates/` and `books/`, which `ritsu gen` writes, are written here as it writes them, with the
/// package's Wasm (DESIGN 36.11: the functions of the ABI, declared for wasm32 alone, and the books'
/// clients left out there), and the crate's Cargo.toml is the test's.
pub fn write_package_for(root: &Path, t: Target, lang: Lang, modules: &[(Rc<Checked>, String)], backend: chobo::target::Backend) -> Vec<String> {
    use chobo::target::Backend;
    let store = if backend == Backend::TigerBeetle { kaname::r#gen::Store::TigerBeetle } else { kaname::r#gen::Store::Sql };
    let dir = package_dir(root, t);
    let mut written: BTreeMap<String, String> = BTreeMap::new();
    let put = |written: &mut BTreeMap<String, String>, rel: String, text: String| {
        if let Some(prev) = written.get(&rel) {
            assert_eq!(prev, &text, "two files of the package at {rel}");
            return;
        }
        write(&dir.join(&rel), &text);
        written.insert(rel, text);
    };
    let mut units: Vec<ritsu_units::Unit> = Vec::new();
    let mut rules: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    let mut dates: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    let mut books: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    for (c, _) in modules {
        let p = Program::of(c);
        for k in p.rules.keys() {
            rules.insert(k.clone(), p.file(k));
        }
        for k in p.dates.keys() {
            dates.insert(k.clone(), p.file(k));
        }
        for k in p.books.keys() {
            books.insert(k.clone(), p.file(k));
        }
    }
    let key = t.key();
    // the rules, as rulec writes each into a package
    for file in rules.values() {
        let path = file.to_string_lossy().to_string();
        let src = std::fs::read_to_string(file).unwrap();
        let (rel, body, used) = rulec::codegen::package_parts(&src, &path, &path, key, GO_PACKAGE).unwrap_or_else(|e| panic!("{path}: {e}"));
        units.extend(used);
        put(&mut written, format!("rules/{rel}"), body);
    }
    // the dates files, as koyomi writes each
    let mut loader = koyomi::calendar::Loader::default();
    for file in dates.values() {
        let path = file.to_string_lossy().to_string();
        let o = koyomi::check::check_file(&path, &koyomi::check::Options::default(), &mut loader).unwrap();
        let checked = o.checked.as_ref().expect("the dates file passes koyomi's check");
        let u = koyomi::codegen::unit_shown(checked, lang, Some(&path));
        let (kt, body) = match t {
            Target::TypeScript => (koyomi::naming::Target::TypeScript, koyomi::codegen::typescript::module(&u)),
            Target::Python => (koyomi::naming::Target::Python, koyomi::codegen::python::module(&u)),
            Target::Go => (koyomi::naming::Target::Go, koyomi::codegen::go::module(&u)),
            Target::Rust => (koyomi::naming::Target::Rust, koyomi::codegen::rust::module(&u)),
        };
        let rel = kt.file(&u.alias).split_once('/').map(|(_, r)| r.to_string()).unwrap();
        put(&mut written, format!("dates/{rel}"), body);
    }
    // the books' clients, on PostgreSQL (Rust's on the backend)
    for file in books.values() {
        let (src, c) = chobo::check::check_file(file).unwrap();
        let book = c.book.as_ref().expect("the book passes chobo's check");
        let stem = file.file_name().unwrap().to_string_lossy().trim_end_matches(".book").to_string();
        let origin = ritsu_emit::header::Origin::at(&file.to_string_lossy(), src.as_bytes());
        units.extend(chobo::client::package_units(book));
        let ct = match (t, backend) {
            (Target::TypeScript, _) => chobo::target::Target::PostgresTypeScript,
            (Target::Python, _) => chobo::target::Target::PostgresPython,
            (Target::Go, _) => chobo::target::Target::PostgresGo,
            (Target::Rust, Backend::Postgres) => chobo::target::Target::PostgresRust,
            (Target::Rust, Backend::TigerBeetle) => chobo::target::Target::TigerBeetleRust,
            (Target::Rust, Backend::MySql) => chobo::target::Target::MySqlRust,
        };
        let built = chobo::target::build_in(book, &stem, ct, &origin, Some(&chobo::client::InPackage { module: GO_PACKAGE.to_string(), ..Default::default() })).unwrap_or_else(|_| panic!("{}", file.display()));
        for (rel, body) in built {
            put(&mut written, format!("books/{rel}"), body);
        }
    }
    // kaname's
    let placed: Vec<_> = modules.iter().map(|(c, s)| (c.clone(), s.clone(), ritsu_emit::package::Place::here())).collect();
    let made = kaname::r#gen::package_for(&placed, t, lang, GO_PACKAGE, store);
    units.extend(made.units.clone());
    for (rel, body, _) in &made.files {
        put(&mut written, rel.clone(), body.clone());
    }
    if let Some((rel, body)) = rulec::codegen::units_module(key, &units) {
        put(&mut written, rel, body);
    }
    match t {
        Target::Python => {
            for sub in ["", "rules/", "dates/", "books/"] {
                put(&mut written, format!("{sub}__init__.py"), String::new());
            }
            put(&mut written, "py.typed".into(), String::new());
        }
        Target::Rust => {
            // the crate's tree: each directory's modules declared in the file of its name, Rust 2018's
            // way (`rules.rs` and `rules/`), a name that is no ASCII identifier with its file's path
            let mut tree: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for rel in written.keys().cloned().collect::<Vec<_>>() {
                let Some((dir, file)) = rel.split_once('/') else {
                    if let Some(stem) = rel.strip_suffix(".rs") {
                        tree.entry(String::new()).or_default().push(mod_line("", stem));
                    }
                    continue;
                };
                if ["rules", "dates", "books"].contains(&dir)
                    && let Some(stem) = file.strip_suffix(".rs")
                    && !stem.contains('/')
                {
                    tree.entry(dir.to_string()).or_default().push(mod_line(&format!("{dir}/"), stem));
                }
            }
            for dir in ["rules", "dates", "books"] {
                if let Some(lines) = tree.get(dir) {
                    // the rules' and the dates' modules are their languages' code, which clippy is
                    // not held to here (chobo's clients are, in chobo's tests)
                    let lines: Vec<String> = if dir == "books" { lines.clone() } else { lines.iter().map(|l| format!("#[allow(clippy::all)]\n{l}")).collect() };
                    put(&mut written, format!("{dir}.rs"), format!("//! The package's {dir}, a module each.\n\n{}", lines.join("")));
                    // the books' clients wait on a database's driver, which the Wasm has none of
                    tree.entry(String::new()).or_default().push(if dir == "books" { kaname::r#gen::wasm::BOOKS.to_string() } else { mod_line("", dir) });
                }
            }
            put(&mut written, kaname::r#gen::wasm::FILE.into(), kaname::r#gen::wasm::abi(lang));
            let mut lib = String::from("//! The package of kaname's tests: the rules', the dates' and the books' code, the units, and kaname's.\n#![allow(confusable_idents, mixed_script_confusables, uncommon_codepoints)]\n\n");
            let mut top = tree.remove("").unwrap_or_default();
            top.push(kaname::r#gen::wasm::DECLARE.to_string());
            top.sort();
            top.dedup();
            for l in top {
                lib.push_str(&l);
            }
            put(&mut written, "lib.rs".into(), lib);
        }
        Target::Go | Target::TypeScript => {}
    }
    written.into_keys().collect()
}

/// The declaration of the module of the file `<dir><stem>.rs` in its directory's file: `pub mod stem;`
/// (a keyword of Rust as a raw identifier), with its path where the name is no ASCII identifier, which
/// Rust finds no file by.
fn mod_line(dir: &str, stem: &str) -> String {
    let name = ritsu_emit::ident::rust_name(stem);
    if ritsu_emit::ident::is_ascii_ident(stem) && ritsu_emit::ident::rust_raw(stem).is_some() { format!("pub mod {name};\n") } else { format!("#[path = \"{dir}{stem}.rs\"]\npub mod {name};\n") }
}

/// How many cases of each kind a target runs, and how many of them it does not, for they do not stop.
pub struct Counts {
    /// The vectors run, and those that do not stop.
    pub vectors: usize,
    pub vectors_not: usize,
    /// The inputs outside what the parameters declare.
    pub outside: usize,
    /// The values of types that hold themselves nested 200 and 201 deep run, and those that do not
    /// stop (DESIGN 28.1).
    pub nested: usize,
    pub nested_not: usize,
}

impl Counts {
    /// The line a test of a target prints of its cases, `agree` of them agreeing with the reference.
    pub fn line(&self, lang: Lang, agree: usize, modules: usize) -> String {
        let run = self.vectors + self.outside + self.nested;
        format!(
            "{lang:?}: {agree} of {run} cases agree with the reference: {} vectors ({} that do not stop are not run), {} inputs outside what the parameters declare and {} values of types that hold themselves nested 200 and 201 deep ({} that do not stop are not run), in {modules} modules",
            self.vectors, self.vectors_not, self.outside, self.nested, self.nested_not
        )
    }
}

/// Every vector of every function of the modules, run in the reference interpreter with every
/// language joined and the messages in `lang` (DESIGN 9), then the inputs outside what the
/// parameters declare (the entry's failure `input`), those the target `t` runs (DESIGN 22.4: Go's
/// `int64` holds no number past int64, Python's date no day past 9999-12-31), then the values of
/// types that hold themselves nested as deep as a host may give one and one deeper (DESIGN 28.1): the
/// cases, and how many of each kind it runs and does not.
pub fn cases(modules: &[(Rc<Checked>, String)], lang: Lang, t: Target) -> (Vec<Case>, Counts) {
    let checked: Vec<Rc<Checked>> = modules.iter().map(|(c, _)| c.clone()).collect();
    let suite = super::joined();
    let (cases, diverged) = kaname::r#gen::harness::cases(&checked, &suite, lang);
    let mut cases = kaname::r#gen::harness::of_target(&cases, t);
    let vectors = cases.len() - diverged;
    let outside = kaname::r#gen::harness::of_target(&kaname::r#gen::harness::outside_cases(&checked, &suite, lang), t);
    let n = outside.len();
    cases.extend(outside);
    let nested = kaname::r#gen::harness::of_target(&kaname::r#gen::harness::nested_cases(&checked, &suite, lang), t);
    let nested_not = nested.iter().filter(|c| c.expected.is_none()).count();
    let nested_run = nested.len() - nested_not;
    cases.extend(nested);
    (cases, Counts { vectors, vectors_not: diverged, outside: n, nested: nested_run, nested_not })
}

/// The cases as the harness reads them: those the interpreter answers.
pub fn cases_json(cases: &[Case]) -> String {
    kaname::r#gen::harness::cases_json(cases)
}

/// The harness's lines held to the cases: the number that agree, and each that does not.
pub fn compare(cases: &[Case], stdout: &str) -> (usize, Vec<String>) {
    kaname::r#gen::harness::compare(cases, stdout)
}

/// What a tool printed, both streams.
pub fn said(o: &std::process::Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

/// The crate's directory, where the tools are.
pub fn tools() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools")
}

/// The cargo that runs the tests (cargo gives the test process `CARGO`), run in `dir` with the target
/// directory every build of the generated crates shares (tests/rust.rs's and tests/wasm.rs's): the tests' own temporary directory in the
/// target directory (`CARGO_TARGET_TMPDIR`), so that the crates the books' clients need are built
/// once.
pub fn cargo_in(dir: &Path) -> std::process::Command {
    let mut c = std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    c.current_dir(dir).env("CARGO_TARGET_DIR", rust_target()).env_remove("RUSTFLAGS").env_remove("CARGO_ENCODED_RUSTFLAGS");
    c
}

pub fn rust_target() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("kaname-rust")
}

/// The crate's Cargo.toml (tools/rust's, at `edition`, its binary named `bin`) and Cargo.lock in
/// `dir`. The builds share a target directory, where a binary is the file of its name: each test
/// names its own. The library takes a version of its own too, `0.0.<n>` from the binary's name: to
/// cargo, crates of one name and version are one library whatever their directory, whose build is
/// fresh while the files it read, looked for in the directory at hand, are older than it. A crate
/// another test built after this one's files were written would be fresh, and the binary would answer
/// with the other's modules.
pub fn crate_files(dir: &Path, edition: &str, bin: &str) {
    let toml = std::fs::read_to_string(tools().join("rust/Cargo.toml")).unwrap();
    let lock = std::fs::read_to_string(tools().join("rust/Cargo.lock")).unwrap();
    assert!(toml.contains("edition = \"2024\""), "tools/rust/Cargo.toml names the 2024 edition");
    let named = "name = \"harness\"\npath = \"harness/main.rs\"";
    assert!(toml.contains(named), "tools/rust/Cargo.toml names the binary harness");
    let package = "name = \"pkg\"\nversion = \"0.0.0\"\n";
    assert!(toml.contains(package) && lock.contains(package), "tools/rust's crate is pkg 0.0.0");
    let own = format!("name = \"pkg\"\nversion = \"0.0.{}\"\n", bin.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b))));
    let toml = toml.replacen(package, &own, 1).replace("edition = \"2024\"", &format!("edition = \"{edition}\"")).replace(named, &format!("name = \"{bin}\"\npath = \"harness/main.rs\""));
    std::fs::write(dir.join("Cargo.toml"), toml).unwrap();
    std::fs::write(dir.join("Cargo.lock"), lock.replacen(package, &own, 1)).unwrap();
}

/// Whether cargo's clippy is here.
pub fn clippy() -> bool {
    ritsu_testkit::tools::runs(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()), &["clippy", "--version"])
}

/// Where a test writes its packages when `KANAME_KEEP` names a directory, to be read after it; else
/// None, and the test writes them in a directory that goes with it.
pub fn keep_in() -> Option<PathBuf> {
    let d = std::env::var_os("KANAME_KEEP").filter(|v| !v.is_empty())?;
    emptied(PathBuf::from(d))
}

/// [`keep_in`] for one of the tests of a file that has more than one: a directory of its own under
/// `KANAME_KEEP`, so that the tests, which run at once, do not write over each other's packages.
pub fn keep_in_for(test: &str) -> Option<PathBuf> {
    let d = std::env::var_os("KANAME_KEEP").filter(|v| !v.is_empty())?;
    emptied(PathBuf::from(d).join(test))
}

fn emptied(d: PathBuf) -> Option<PathBuf> {
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}
