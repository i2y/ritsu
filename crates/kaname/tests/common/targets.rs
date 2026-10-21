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

/// The Python package's name and Go's module and import path of the package's directory, as the tests
/// lay a package out.
pub const PY_NAME: &str = "pkg";
pub const GO_MODULE: &str = "kanametest";
pub const GO_PACKAGE: &str = "kanametest/pkg";

/// The test modules of `tests/modules/`, checked as the file `t.kn` beside the crate (where they read
/// the examples' rules by their paths), and the ones only these tests run.
pub const MODULES: [(&str, &str); 15] = [
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
    ("temps", include_str!("../modules/temps.kn")),
    ("temps.ja", include_str!("../modules/temps.ja.kn")),
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
/// Python and Go.
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
    let dir = package_dir(root, t);
    let mut written: BTreeMap<String, String> = BTreeMap::new();
    let mut put = |rel: String, text: String| {
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
        put(format!("rules/{rel}"), body);
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
        };
        let rel = kt.file(&u.alias).split_once('/').map(|(_, r)| r.to_string()).unwrap();
        put(format!("dates/{rel}"), body);
    }
    // the books' clients, on PostgreSQL
    for file in books.values() {
        let (src, c) = chobo::check::check_file(file).unwrap();
        let book = c.book.as_ref().expect("the book passes chobo's check");
        let stem = file.file_name().unwrap().to_string_lossy().trim_end_matches(".book").to_string();
        let origin = ritsu_emit::header::Origin::at(&file.to_string_lossy(), src.as_bytes());
        units.extend(chobo::client::package_units(book));
        let ct = match t {
            Target::TypeScript => chobo::target::Target::PostgresTypeScript,
            Target::Python => chobo::target::Target::PostgresPython,
            Target::Go => chobo::target::Target::PostgresGo,
        };
        let built = chobo::target::build_in(book, &stem, ct, &origin, Some(&chobo::client::InPackage { module: GO_PACKAGE.to_string() })).unwrap_or_else(|_| panic!("{}", file.display()));
        for (rel, body) in built {
            put(format!("books/{rel}"), body);
        }
    }
    // kaname's
    let made = kaname::r#gen::package(modules, t, lang, GO_PACKAGE);
    units.extend(made.units.clone());
    for (rel, body, _) in &made.files {
        put(rel.clone(), body.clone());
    }
    if let Some((rel, body)) = rulec::codegen::units_module(key, &units) {
        put(rel, body);
    }
    match t {
        Target::Python => {
            for sub in ["", "rules/", "dates/", "books/"] {
                put(format!("{sub}__init__.py"), String::new());
            }
            put("py.typed".into(), String::new());
        }
        Target::Go | Target::TypeScript => {}
    }
    written.into_keys().collect()
}

/// Every vector of every function of the modules, run in the reference interpreter with every
/// language joined and the messages in `lang` (DESIGN 9), then the inputs outside what the
/// parameters declare (the entry's failure `input`): the cases, how many vectors do not stop, and how
/// many of the cases are outside.
pub fn cases(modules: &[(Rc<Checked>, String)], lang: Lang) -> (Vec<Case>, usize, usize) {
    let checked: Vec<Rc<Checked>> = modules.iter().map(|(c, _)| c.clone()).collect();
    let suite = super::joined();
    let (mut cases, diverged) = kaname::r#gen::harness::cases(&checked, &suite, lang);
    let outside = kaname::r#gen::harness::outside_cases(&checked, &suite, lang);
    let n = outside.len();
    cases.extend(outside);
    (cases, diverged, n)
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

/// Where a test writes its packages when `KANAME_KEEP` names a directory, to be read after it; else
/// None, and the test writes them in a directory that goes with it.
pub fn keep_in() -> Option<PathBuf> {
    let d = std::env::var_os("KANAME_KEEP").filter(|v| !v.is_empty())?;
    let d = PathBuf::from(d);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}
