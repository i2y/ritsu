//! Modules (DESIGN 17): the sources of the nine languages carried across projects and repositories.
//! A module is a directory with a `ritsu.mod` at its root and the files under it (those of a module
//! below it with a `ritsu.mod` of its own left out); it has a version, and it requires other
//! modules, by a path, from an OCI registry or from the tags of a git repository. A file names a
//! dependency's file by `@<name>/…`, read from the root of the module the file is in.
//!
//! - [`address`]: how the diffs name the two sides they compare (`@<name>@<version>/…`, `…@<git
//!   revision>`), and the files each side is read through.
//! - [`lex`]: the words of a line of `ritsu.mod` and `ritsu.lock`, which are ritsu's lexis.
//! - [`manifest`], [`lock`], [`semver`] and [`names`]: `ritsu.mod` (read, and changed a line at a
//!   time), `ritsu.lock`, versions and their order, and the forms of the names (E301–E303).
//! - [`git`]: what a dependency from a git repository is, as far as it is decided without running
//!   git: its URL and the module name it gives, the tags of its versions, the commits.
//! - [`tar`], [`digest`] and [`artifact`]: the tar of a module's files, written one way and read in
//!   that way only; the digest of a directory's files; the OCI manifest of a version, built in fixed
//!   bytes and checked when fetched.
//! - [`graph`]: the tree of modules — read and held to the lock (E304–E306, E308, E309, W302), and
//!   chosen by Minimal Version Selection — and how each module's files are written
//!   ([`graph::Node::prefix`]).
//! - [`store`] and [`vendor`]: the store of the modules fetched, a module's copy of it, and an entry
//!   made from a directory with no registry ([`store::put_directory`]).
//! - [`refer`]: where a path written in a file leads ([`refer::refer`], [`refer::join`]), and how
//!   [`crate::fs`] sees a dependency at `R/@<name>` ([`refer::map`]). Outside a module, a path is what
//!   it was (M14).
//!
//! Nothing here depends on anything but std (DESIGN 3.1, P9). Nothing here goes to the network: what
//! fetches and pushes is the `ritsu` crate's.

pub mod address;
pub mod artifact;
pub mod digest;
pub mod git;
pub mod graph;
pub mod lex;
pub mod lock;
pub mod manifest;
pub mod names;
pub mod refer;
pub mod semver;
pub mod store;
pub mod tar;
pub mod vendor;

pub use graph::{Node, Problem, Tree};
pub use refer::{Refused, dependency_root, forget, from_own_root, given, given_all, join, map, missing, module_of, module_words, module_words_from, outside_note, outside_note_for, prefix_of, refer, refer_shown, tree, under_dependency};

/// A diagnostic of a module's files: no part of a language's own.
pub type Diag = crate::diag::Diag<()>;

/// `ritsu.mod` cannot be read: a word it does not know, a word with no value, a string's escape, a
/// statement written once that is there twice, no `module` first, a full-width space.
pub const E301: &str = "E301";
/// A value of `ritsu.mod` of the wrong form: a module's name, a version, a dependency's name, a
/// `path` that is absolute, a git URL ritsu does not take or its `dir`, a `signature`'s start, a
/// `library`'s name, the version of `ritsu >=`, a dependency's name twice.
pub const E302: &str = "E302";
/// A dependency from a registry or a git repository, and no `ritsu.lock`, or one that cannot be read.
pub const E303: &str = "E303";
/// `ritsu.lock` does not match `ritsu.mod`.
pub const E304: &str = "E304";
/// The files in the store, or in a module's copy of it, differ from the lock's digest.
pub const E305: &str = "E305";
/// A module of one name from two places, or from two sources the module at hand has not chosen
/// between.
pub const E306: &str = "E306";
/// A name that starts with `@` right under the root of a module.
pub const E307: &str = "E307";
/// A module from a registry with a `require … path` or a `require … git`, or one from a git
/// repository with a `require … path`.
pub const E308: &str = "E308";
/// A `signature` line that covers a module from a git repository, whose signature ritsu cannot
/// check.
pub const E309: &str = "E309";
/// A value of an enum both versions of a module have, whose number on the Connect wire changed: the
/// method's path being the same, a message one version writes, the other reads as another value.
pub const E310: &str = "E310";
/// A dependency no file uses.
pub const W301: &str = "W301";
/// A line of `ritsu.lock` for a module nothing requires now.
pub const W302: &str = "W302";
/// A file that names a dependency of a dependency, which its module does not require.
pub const W303: &str = "W303";
