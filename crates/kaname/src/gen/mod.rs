//! The code kaname generates (DESIGN 10): the modules of a project as part of the one package `ritsu
//! gen` writes, in TypeScript, Python and Go, under the package's `glue/`, beside the rules', the
//! dates' and the books' code they call. Nothing else writes it: a module calls the package's
//! `rules/` and `dates/`, so there is no `kaname gen` of its own (DESIGN 10.1).
//!
//! - [`plan`]: a function's body as the reference interpreter evaluates it, its checks where the
//!   analysis does not show a place (DESIGN 10.4); [`ex`], the tree it is written as, and how each
//!   target prints it.
//! - [`module`]: a module as a file of the target; [`books`], the transfers it builds and the function
//!   that makes them through chobo's client of the book (DESIGN 10.6).
//! - The helper every module reads, `glue/kaname.*` (DESIGN 10.1): written from `runtime/`.
//! - [`harness`]: a program that runs the generated code on the vectors, for the tests that hold it to
//!   the reference interpreter (DESIGN 16.4).

pub mod books;
pub mod ex;
pub mod harness;
pub mod module;
pub mod plan;

use crate::check::Checked;
use ritsu_base::text::Lang;
use ritsu_units::Unit;
use std::rc::Rc;

/// A language a package is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    TypeScript,
    Python,
    Go,
}

impl Target {
    pub const ALL: [Target; 3] = [Target::TypeScript, Target::Python, Target::Go];

    pub fn key(self) -> &'static str {
        match self {
            Target::TypeScript => "typescript",
            Target::Python => "python",
            Target::Go => "go",
        }
    }

    pub fn parse(s: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.key() == s)
    }
}

/// How a module's code is written: the path its head names (from the project's root, as `ritsu gen`
/// names a file), the language of its comments and its messages, and Go's import path of the
/// package's directory (a rule's package is `<module>/rules/<package>`).
#[derive(Clone, Debug)]
pub struct Options {
    pub shown: String,
    pub lang: Lang,
    pub go_module: String,
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

/// The helper every module of the package reads (DESIGN 10.1), with its head and the language of its
/// messages.
pub fn helper(t: Target, lang: Lang) -> (String, String) {
    let head = ritsu_emit::header::generated("kaname");
    let word = if lang == Lang::Ja { "ja" } else { "en" };
    let (path, src) = match t {
        Target::TypeScript => ("glue/kaname.ts", TS_HELPER),
        Target::Python => ("glue/kaname.py", PY_HELPER),
        Target::Go => ("glue/kaname/kaname.go", GO_HELPER),
    };
    (path.to_string(), src.replace("@HEAD@", &head).replace("@LANG@", word))
}

/// Everything kaname writes into a package of `t` for `modules` (each with the path from the project's
/// root its head names): the helper, each module, the types of the transfers the modules build and
/// the functions that make them, and the indexes of `glue/`.
pub fn package(modules: &[(Rc<Checked>, String)], t: Target, lang: Lang, go_module: &str) -> Made {
    let mut made = Made::default();
    if modules.is_empty() {
        return made;
    }
    let (hp, hs) = helper(t, lang);
    made.files.push((hp, hs, None));
    let mut names: Vec<String> = vec!["kaname".to_string()];
    for (c, shown) in modules {
        let opts = Options { shown: shown.clone(), lang, go_module: go_module.to_string() };
        let (path, text, units) = module::module(c, t, &opts);
        made.files.push((path, text, Some(shown.clone())));
        names.push(c.module.name.ascii().to_string());
        for u in units {
            if !made.units.contains(&u) {
                made.units.push(u);
            }
        }
    }
    let checked: Vec<Rc<Checked>> = modules.iter().map(|(c, _)| c.clone()).collect();
    let built = books::built(&checked);
    let opts = Options { shown: String::new(), lang, go_module: go_module.to_string() };
    let mut book_names: Vec<String> = Vec::new();
    for (book, ops) in &built {
        let Some(facts) = checked.iter().find_map(|c| crate::value::Program::of(c).books.get(book).cloned()) else { continue };
        for (path, text) in books::files(t, &facts, ops, &opts) {
            made.files.push((path, text, None));
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
        book_names.push(facts.name.clone());
    }
    // the indexes: each module by its name, and the books' transfers
    let head = ritsu_emit::header::generated("kaname");
    match t {
        Target::TypeScript => {
            let mut index = format!("// {head}\n// The modules of kaname of the package, each by its name, and the transfers they build.\n\n");
            for n in &names {
                index.push_str(&format!("export * as {n} from \"./{n}\";\n"));
            }
            if !book_names.is_empty() {
                index.push_str("export * as books from \"./books/index\";\n");
                let mut b = format!("// {head}\n// The transfers the modules of kaname build, by book.\n\n");
                for n in &book_names {
                    b.push_str(&format!("export * as {n} from \"./{n}\";\nexport * as {n}_apply from \"./{n}_apply\";\n"));
                }
                made.files.push(("glue/books/index.ts".into(), b, None));
            }
            made.files.push(("glue/index.ts".into(), index, None));
        }
        Target::Python => {
            let mut init = format!("# {head}\n\"\"\"The modules of kaname of the package, each by its name, and the transfers they build.\"\"\"\n\n");
            for n in &names {
                init.push_str(&format!("from . import {n} as {n}\n"));
            }
            if !book_names.is_empty() {
                init.push_str("from . import books as books\n");
                let mut b = format!("# {head}\n\"\"\"The transfers the modules of kaname build, by book.\"\"\"\n\n");
                for n in &book_names {
                    b.push_str(&format!("from . import {n} as {n}\nfrom . import {n}_apply as {n}_apply\n"));
                }
                made.files.push(("glue/books/__init__.py".into(), b, None));
            }
            made.files.push(("glue/__init__.py".into(), init, None));
        }
        Target::Go => {}
    }
    made
}
