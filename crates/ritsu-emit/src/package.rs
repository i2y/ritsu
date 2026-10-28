//! Where a part goes in the package `ritsu gen` writes (ritsu's DESIGN 17.8). The parts of the files
//! of the module at hand are at the package's directory, as they were before there were modules; a
//! dependency's are under `deps/<name>/`, one level for each name of the chain its files are written
//! with (`@pricing/@tax/rules/vat_rate.rule` under `deps/pricing/deps/tax/`), so that two modules
//! with a file of one name do not write one part. A generator keeps its own names for its parts
//! (DESIGN 9.1) and puts them under the directory of their module, which `ritsu gen` hands it
//! ([`Place`]): the one of the file it writes from, and through it the one of each file that file
//! reads. The path one part imports another by is made from the two directories ([`ts_path`],
//! [`py_path`], [`py_from`]; Go's import paths are the package's module and the directory).
//!
//! Outside a module, and for every file of the module at hand, the directory is empty, and every
//! path is what it was before there were modules (ritsu's DESIGN 17.1, M14).

use std::path::{Path, PathBuf};

/// A dependency's module, as the head of a file made from one of its files names it: its name and,
/// when it has one, its version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    pub version: Option<String>,
}

/// Where the parts of a file of a project go in a package.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Place {
    /// The project's root, absolute, when it has a `ritsu.mod`: the paths of the files are written
    /// from it. None outside a module.
    root: Option<PathBuf>,
    /// The directory of the module of the file the parts are made from, under the package's
    /// directory: empty for the module at hand, `deps/pricing/`, `deps/pricing/deps/tax/`.
    pub dir: String,
    /// The dependency the file is of, for the head of its parts: None for a file of the module at
    /// hand.
    pub module: Option<Module>,
}

impl Place {
    /// The parts of a file of the module at hand, or of any file of a project whose root has no
    /// `ritsu.mod`: at the package's directory, with every file they read.
    pub fn here() -> Place {
        Place::default()
    }

    /// The place of the file at `rel` (its canonical path from `root`, a root with a `ritsu.mod`),
    /// of the dependency `module` (None for a file of the module at hand).
    pub fn of(root: &Path, rel: &str, module: Option<Module>) -> Place {
        Place { root: Some(root.to_path_buf()), dir: deps_dir(rel), module }
    }

    /// The directory of the parts of another file the parts read, by its path as the generator
    /// holds it (from where the run is, or absolute): where that file's parts are in the package.
    pub fn dir_of(&self, path: &Path) -> String {
        let Some(root) = &self.root else { return String::new() };
        ritsu_base::paths::from_root(root, path).map(|rel| deps_dir(&rel)).unwrap_or_default()
    }

    /// The same, for a path from the project's root.
    pub fn dir_of_rel(&self, rel: &str) -> String {
        if self.root.is_none() { String::new() } else { deps_dir(rel) }
    }

    /// The path from the project's root of another file, by its path as the generator holds it;
    /// None outside a module.
    pub fn rel_of(&self, path: &Path) -> Option<String> {
        ritsu_base::paths::from_root(self.root.as_ref()?, path)
    }

    /// The place of another file of the same project, by its path as the generator holds it, for
    /// parts that name no module in their head (kaname's types of a book's transfers).
    pub fn of_file(&self, path: &Path) -> Place {
        Place { root: self.root.clone(), dir: self.dir_of(path), module: None }
    }
}

/// The directory of the parts of the file at `rel` (a path from the root of a module, written the
/// canonical way): `deps/<name>/` for each `@<name>` it begins with; empty for a file of the module.
pub fn deps_dir(rel: &str) -> String {
    rel.split('/').take_while(|p| p.len() > 1 && p.starts_with('@')).map(|p| format!("deps/{}/", &p[1..])).collect()
}

/// The names of the dependencies on the way to the directory `dir` ([`deps_dir`]): `["pricing",
/// "tax"]` for `deps/pricing/deps/tax/`.
pub fn chain(dir: &str) -> Vec<&str> {
    dir.split('/').collect::<Vec<_>>().chunks(2).filter_map(|c| if c.len() == 2 && c[0] == "deps" && !c[1].is_empty() { Some(c[1]) } else { None }).collect()
}

/// The parts of `from` (a directory under the package's directory, `rules/` or empty) and of `to`'s
/// directory they share, and what is left of each.
fn apart<'a>(from: &'a str, to: &'a str) -> (usize, Vec<&'a str>) {
    let from: Vec<&str> = from.split('/').filter(|p| !p.is_empty()).collect();
    let to: Vec<&str> = to.split('/').filter(|p| !p.is_empty()).collect();
    let dirs = to.len().saturating_sub(1);
    let mut same = 0;
    while same < from.len() && same < dirs && from[same] == to[same] {
        same += 1;
    }
    (from.len() - same, to[same..].to_vec())
}

/// The specifier TypeScript imports `to` (a module's path under the package's directory, without its
/// extension: `units`, `deps/pricing/rules/fee`) by from a module in the directory `from` (under the
/// package's directory: `rules/`, `flows/place_order/`): `../units`, `./fee`,
/// `../../deps/pricing/rules/fee`.
pub fn ts_path(from: &str, to: &str) -> String {
    let (up, rest) = apart(from, to);
    let lead = if up == 0 { "./".to_string() } else { "../".repeat(up) };
    format!("{lead}{}", rest.join("/"))
}

/// The module Python imports `to` (as for [`ts_path`]) by, relative to a module in the directory
/// `from`: `..units`, `.fee`, `...deps.pricing.rules.fee`.
pub fn py_path(from: &str, to: &str) -> String {
    let (up, rest) = apart(from, to);
    format!("{}{}", ".".repeat(up + 1), rest.join("."))
}

/// [`py_path`] split for `from <package> import <name>`: (`..`, `units`), (`...deps.pricing.rules`,
/// `fee`).
pub fn py_from(from: &str, to: &str) -> (String, String) {
    let p = py_path(from, to);
    let dots = p.chars().take_while(|c| *c == '.').count();
    match p[dots..].rsplit_once('.') {
        Some((pkg, name)) => (format!("{}{pkg}", &p[..dots]), name.to_string()),
        None => (p[..dots].to_string(), p[dots..].to_string()),
    }
}

/// The Go import path of `to` (as for [`ts_path`]) in the package whose module is `go_module`.
pub fn go_path(go_module: &str, to: &str) -> String {
    format!("{go_module}/{to}")
}

/// A name for an import of the part at `dir` (a [`Place::dir`]) that is `name` in the module at hand:
/// `name` there, and with the dependencies' names before it under `deps/` (`pricing_fee`), so that the
/// parts of two modules with one name are two names in the file that imports both.
pub fn local_name(dir: &str, name: &str) -> String {
    let c = chain(dir);
    if c.is_empty() { name.to_string() } else { format!("{}_{name}", c.join("_")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dependency_is_under_deps_a_level_for_each_name() {
        assert_eq!(deps_dir("rules/fee.rule"), "");
        assert_eq!(deps_dir("@pricing/rules/fee.rule"), "deps/pricing/");
        assert_eq!(deps_dir("@pricing/@tax/rules/vat_rate.rule"), "deps/pricing/deps/tax/");
        assert_eq!(chain("deps/pricing/deps/tax/"), vec!["pricing", "tax"]);
        assert!(chain("").is_empty());
    }

    #[test]
    fn the_paths_are_what_they_were_in_the_module_at_hand() {
        assert_eq!(ts_path("rules/", "units"), "../units");
        assert_eq!(ts_path("rules/", "rules/fee"), "./fee");
        assert_eq!(ts_path("flows/place_order/", "rules/fee"), "../../rules/fee");
        assert_eq!(ts_path("glue/", "glue/books/ledger"), "./books/ledger");
        assert_eq!(ts_path("glue/", "glue/kaname"), "./kaname");
        assert_eq!(py_path("rules/", "units"), "..units");
        assert_eq!(py_path("rules/", "rules/fee"), ".fee");
        assert_eq!(py_path("flows/place_order/", "rules/fee"), "...rules.fee");
        assert_eq!(py_from("books/", "units"), ("..".to_string(), "units".to_string()));
        assert_eq!(py_from("authz/", "rules/fee"), ("..rules".to_string(), "fee".to_string()));
    }

    #[test]
    fn a_dependency_is_reached_through_the_package_directory() {
        assert_eq!(ts_path("deps/pricing/rules/", "units"), "../../../units");
        assert_eq!(ts_path("flows/place_order/", "deps/pricing/rules/fee"), "../../deps/pricing/rules/fee");
        assert_eq!(ts_path("deps/pricing/rules/", "deps/pricing/deps/tax/rules/vat"), "../deps/tax/rules/vat");
        assert_eq!(py_path("deps/pricing/rules/", "units"), "....units");
        assert_eq!(py_from("deps/pricing/books/", "units"), ("....".to_string(), "units".to_string()));
        assert_eq!(py_path("flows/x/", "deps/pricing/rules/fee"), "...deps.pricing.rules.fee");
        assert_eq!(go_path("example.com/shop", "deps/pricing/rules/fee"), "example.com/shop/deps/pricing/rules/fee");
        assert_eq!(local_name("deps/pricing/", "fee"), "pricing_fee");
        assert_eq!(local_name("", "fee"), "fee");
    }
}
