//! The code kaname generates (DESIGN 10): the modules of a project as part of the one package `ritsu
//! gen` writes, in TypeScript, Python, Go and Rust (DESIGN 36), under the package's `glue/`, beside
//! the rules', the dates' and the books' code they call. Nothing else writes it: a module calls the package's
//! `rules/` and `dates/`, so there is no `kaname gen` of its own (DESIGN 10.1).
//!
//! - [`plan`]: a function's body as the reference interpreter evaluates it, its checks where the
//!   analysis does not show a place (DESIGN 10.4); [`ex`], the tree it is written as, and how each
//!   target prints it.
//! - [`module`]: a module as a file of the target; [`books`], the transfers it builds and the function
//!   that makes them through chobo's client of the book (DESIGN 10.6).
//! - The helper every module reads, `glue/kaname.*` (DESIGN 10.1): written from `runtime/`.
//! - [`door`]: Rust's `glue::door::answer`, which answers a call written as a line of JSON (the input
//!   of the package's Wasm, DESIGN 36.11), and which the native harness calls too; [`wasm`], the
//!   functions of the canonical ABI the package's Wasm exports, which hand a call to it, and the
//!   package's world.
//! - [`harness`]: a program that runs the generated code on the vectors, for the tests that hold it to
//!   the reference interpreter (DESIGN 16.4).

pub mod books;
pub mod door;
pub mod ex;
pub mod harness;
pub mod module;
pub mod plan;
pub mod wasm;

use crate::check::Checked;
use ritsu_base::text::Lang;
use ritsu_base::tr;
use ritsu_units::Unit;
use std::rc::Rc;

/// A language a package is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    TypeScript,
    Python,
    Go,
    Rust,
}

impl Target {
    pub const ALL: [Target; 4] = [Target::TypeScript, Target::Python, Target::Go, Target::Rust];

    pub fn key(self) -> &'static str {
        match self {
            Target::TypeScript => "typescript",
            Target::Python => "python",
            Target::Go => "go",
            Target::Rust => "rust",
        }
    }

    pub fn parse(s: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.key() == s)
    }
}

/// Where the package keeps its books (`ritsu gen --books`), which Rust's function that makes a
/// module's transfers takes the book by (DESIGN 36.6): the clients of PostgreSQL and of the MySQL
/// family take a `Database`, TigerBeetle's a `TigerBeetleClient`. TypeScript, Python and Go read
/// none of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Store {
    #[default]
    Sql,
    TigerBeetle,
}

impl Store {
    /// The trait of the book's client the function takes the database by.
    pub fn bound(self) -> &'static str {
        match self {
            Store::Sql => "Database",
            Store::TigerBeetle => "TigerBeetleClient",
        }
    }
}

/// How a module's code is written: the path its head names (from the project's root, as `ritsu gen`
/// names a file), the language of its comments and its messages, Go's import path of the
/// package's directory (a rule's package is `<module>/rules/<package>`), where the module goes
/// in the package, and through it each file it reads (`deps/<name>/` for a dependency's, ritsu's
/// DESIGN 17.8), and where the package keeps its books.
#[derive(Clone, Debug)]
pub struct Options {
    pub shown: String,
    pub lang: Lang,
    pub go_module: String,
    pub place: ritsu_emit::package::Place,
    pub store: Store,
}

impl Options {
    /// The directory of the code of the file at `path` (as the module holds it), in the package.
    pub fn dir_of(&self, path: &std::path::Path) -> String {
        self.place.dir_of(path)
    }

    /// The directory the module's own code is in, in the package: `glue/`, or under a dependency's.
    pub fn here(&self) -> String {
        format!("{}glue/", self.place.dir)
    }
}

/// What kaname writes into a package: each file by its path under the package's directory (the
/// Python package's directory for Python), its text, and the project's file it is made from (None
/// for the helper, the books' files and the indexes); and the units whose types its code names,
/// which the package's `units` declares.
#[derive(Clone, Debug, Default)]
pub struct Made {
    pub files: Vec<(String, String, Option<String>)>,
    pub units: Vec<Unit>,
}

const TS_HELPER: &str = include_str!("runtime/kaname.ts");
const PY_HELPER: &str = include_str!("runtime/kaname.py");
const GO_HELPER: &str = include_str!("runtime/kaname.go");
const RS_HELPER: &str = include_str!("runtime/kaname.rs");

/// The helper every module of the package reads (DESIGN 10.1), with its head and the language of its
/// messages.
pub fn helper(t: Target, lang: Lang) -> (String, String) {
    let head = ritsu_emit::header::generated("kaname");
    let word = if lang == Lang::Ja { "ja" } else { "en" };
    let (path, src) = match t {
        Target::TypeScript => ("glue/kaname.ts", TS_HELPER),
        Target::Python => ("glue/kaname.py", PY_HELPER),
        Target::Go => ("glue/kaname/kaname.go", GO_HELPER),
        Target::Rust => ("glue/kaname.rs", RS_HELPER),
    };
    (path.to_string(), src.replace("@HEAD@", &head).replace("@LANG@", word))
}

/// What stops the code of `t` of a module that passes its check (DESIGN 36.3): E008, the W001 of a
/// module whose name the generated Rust cannot take, as that warning says it, at the same place, and
/// in place of its last note what to do. `ritsu gen` asks this before it makes a package; the other
/// targets take the name.
pub fn stops(c: &Checked, t: Target) -> Vec<crate::diag::Diag> {
    if t != Target::Rust {
        return Vec::new();
    }
    crate::names::no_rust_module(&c.module, &c.path, &c.src)
        .map(|mut d| {
            d.code = "E008";
            d.severity = crate::diag::Severity::Error;
            d.notes.pop();
            d.note(tr!(
                "別の名前か別名にしてください。使わない出力先なら、`--target` で使う出力先を挙げてください。",
                "Choose another name or alias. For a target you do not use, name the ones you use with `--target`."
            ))
        })
        .into_iter()
        .collect()
}

/// Everything kaname writes into a package of `t` for `modules` (each with the path from the project's
/// root its head names): the helper, each module, the types of the transfers the modules build and
/// the functions that make them, and the indexes of `glue/`.
pub fn package(modules: &[(Rc<Checked>, String)], t: Target, lang: Lang, go_module: &str) -> Made {
    let placed: Vec<(Rc<Checked>, String, ritsu_emit::package::Place)> = modules.iter().map(|(c, s)| (c.clone(), s.clone(), ritsu_emit::package::Place::here())).collect();
    package_in(&placed, t, lang, go_module)
}

/// [`package`] with where each module goes in the package (ritsu's DESIGN 17.8): a dependency's
/// module, and the types of the transfers of a dependency's book, under the dependency's directory
/// (`deps/<name>/glue/`), each reading what it reads where that is. The helper and the units are the
/// package's one, at its `glue/kaname` and `units`; the indexes of `glue/` hold the module at hand's
/// modules, as before there were modules.
pub fn package_in(modules: &[(Rc<Checked>, String, ritsu_emit::package::Place)], t: Target, lang: Lang, go_module: &str) -> Made {
    package_for(modules, t, lang, go_module, Store::default())
}

/// [`package_in`] for a package that keeps its books in `store`, which Rust's functions that make
/// the transfers read (DESIGN 36.6).
pub fn package_for(modules: &[(Rc<Checked>, String, ritsu_emit::package::Place)], t: Target, lang: Lang, go_module: &str, store: Store) -> Made {
    let mut made = Made::default();
    if modules.is_empty() {
        return made;
    }
    let (hp, hs) = helper(t, lang);
    made.files.push((hp, hs, None));
    // the names each index of glue/ holds, by the directory of its module in the package: the
    // module at hand's (with the helper), then each dependency's
    let mut names: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    names.insert(String::new(), vec!["kaname".to_string()]);
    for (c, shown, place) in modules {
        let opts = Options { shown: shown.clone(), lang, go_module: go_module.to_string(), place: place.clone(), store };
        let (path, text, units) = module::module(c, t, &opts);
        made.files.push((format!("{}{path}", place.dir), text, Some(shown.clone())));
        names.entry(place.dir.clone()).or_default().push(c.module.name.ascii().to_string());
        for u in units {
            if !made.units.contains(&u) {
                made.units.push(u);
            }
        }
    }
    let checked: Vec<Rc<Checked>> = modules.iter().map(|(c, _, _)| c.clone()).collect();
    let built = books::built(&checked);
    let mut book_names: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for (book, ops) in &built {
        let Some(facts) = checked.iter().find_map(|c| crate::value::Program::of(c).books.get(book).cloned()) else { continue };
        // the types of a book's transfers are where the book's client is: under its module's directory
        let place = modules[0].2.of_file(book);
        let opts = Options { shown: String::new(), lang, go_module: go_module.to_string(), place: place.clone(), store };
        for (path, text) in books::files(t, &facts, ops, &opts) {
            made.files.push((format!("{}{path}", place.dir), text, None));
        }
        // the units of the amounts the types of the transfers name
        for tr in &facts.transfers {
            for p in &tr.params {
                if let Some(u) = p.unit.as_ref().and_then(|u| crate::names::book_unit(&facts, u))
                    && u.type_name().is_some()
                    && !made.units.contains(&u)
                {
                    made.units.push(u);
                }
            }
        }
        // Rust's module of the types is the client's (an ASCII name)
        book_names.entry(place.dir.clone()).or_default().push(if t == Target::Rust { facts.rust.module.clone() } else { facts.name.clone() });
    }
    for dir in book_names.keys() {
        names.entry(dir.clone()).or_default();
    }
    // the indexes: each module by its name, and the books' transfers, for each directory of glue/
    let head = ritsu_emit::header::generated("kaname");
    for (dir, names) in &names {
        let book_names = book_names.get(dir).cloned().unwrap_or_default();
        match t {
            Target::TypeScript => {
                let mut index = format!("// {head}\n// The modules of kaname of the package, each by its name, and the transfers they build.\n\n");
                for n in names {
                    index.push_str(&format!("export * as {n} from \"./{n}\";\n"));
                }
                if !book_names.is_empty() {
                    index.push_str("export * as books from \"./books/index\";\n");
                    let mut b = format!("// {head}\n// The transfers the modules of kaname build, by book.\n\n");
                    for n in &book_names {
                        b.push_str(&format!("export * as {n} from \"./{n}\";\nexport * as {n}_apply from \"./{n}_apply\";\n"));
                    }
                    made.files.push((format!("{dir}glue/books/index.ts"), b, None));
                }
                made.files.push((format!("{dir}glue/index.ts"), index, None));
            }
            Target::Python => {
                let mut init = format!("# {head}\n\"\"\"The modules of kaname of the package, each by its name, and the transfers they build.\"\"\"\n\n");
                for n in names {
                    init.push_str(&format!("from . import {n} as {n}\n"));
                }
                if !book_names.is_empty() {
                    init.push_str("from . import books as books\n");
                    let mut b = format!("# {head}\n\"\"\"The transfers the modules of kaname build, by book.\"\"\"\n\n");
                    for n in &book_names {
                        b.push_str(&format!("from . import {n} as {n}\nfrom . import {n}_apply as {n}_apply\n"));
                    }
                    made.files.push((format!("{dir}glue/books/__init__.py"), b, None));
                }
                made.files.push((format!("{dir}glue/__init__.py"), init, None));
            }
            // the tree of Rust's modules: `glue.rs` holds the modules of its directory (the helper
            // and the answers of Wasm's call in the package's own), `glue/books.rs` the books'
            Target::Rust => {
                let mut index = format!("// {head}\n//! The modules of kaname of the package, each by its name, and the transfers they build.\n\n");
                for n in names {
                    index.push_str(&format!("pub mod {};\n", ritsu_emit::ident::rust_name(n)));
                }
                if !book_names.is_empty() {
                    index.push_str("pub mod books;\n");
                    let mut b = format!("// {head}\n//! The transfers the modules of kaname build, by book, and the functions that make them through the books' clients.\n\n");
                    for n in &book_names {
                        b.push_str(&format!("pub mod {};\n#[cfg(not(target_arch = \"wasm32\"))]\npub mod {};\n", ritsu_emit::ident::rust_name(n), ritsu_emit::ident::rust_name(&format!("{n}_apply"))));
                    }
                    made.files.push((format!("{dir}glue/books.rs"), b, None));
                }
                if dir.is_empty() {
                    index.push_str("pub mod door;\n");
                }
                made.files.push((format!("{dir}glue.rs"), index, None));
            }
            Target::Go => {}
        }
    }
    if t == Target::Rust {
        made.files.push(("glue/door.rs".to_string(), door::door(modules, lang), None));
    }
    made
}
