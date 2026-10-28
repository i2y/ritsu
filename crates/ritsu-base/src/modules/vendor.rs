//! Which copy of a module from a registry a run reads (DESIGN 17.15): the module's own copy
//! (`.ritsu/modules/`, which `ritsu mod vendor` makes) first, then the store.

use super::store::Store;
use std::path::Path;

/// Where the files of a module of the tree are read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Where {
    /// A directory a `require … path` names.
    Path,
    /// The store.
    Store,
    /// The copy in the module at hand.
    Vendor,
    /// The module at hand itself.
    Here,
}

impl Where {
    /// The word the JSON writes (`ritsu check`'s `dependencies`, DESIGN 17.21).
    pub fn word(self) -> &'static str {
        match self {
            Where::Path | Where::Here => "path",
            Where::Store => "store",
            Where::Vendor => "vendor",
        }
    }
}

/// The copy of the entry of `digest` that the module whose root is `module_root` reads: its own copy
/// when it has the entry with its files, else the store's, else none.
pub fn find(module_root: &Path, digest: &str, store: Option<&Store>) -> Option<(Store, Where)> {
    let own = Store::vendored(module_root);
    if own.has_files(digest) {
        return Some((own, Where::Vendor));
    }
    match store {
        Some(s) if s.has_files(digest) => Some((s.clone(), Where::Store)),
        _ => None,
    }
}
