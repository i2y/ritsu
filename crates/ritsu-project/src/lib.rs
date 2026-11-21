//! A project of ritsu, read as one (DESIGN 6): the files a person names, found and sorted by
//! language; every language's engine made once and joined to the languages that read others; and
//! the index of what each file holds and names outside itself, through which every reference
//! between files is resolved.
//!
//! - [`Joined`]: the ten languages, each made once. It hands dandori the ports of rules,
//!   dates and books, ringi the ports of the six languages it reads, and yuen and sakai the ports
//!   they read with the project's [`ritsu_ports::Index`], so that a rule they all read is read once,
//!   and a naming is looked up in one place.
//! - [`Project`]: the files under the paths given, each with its language (by its extension), in
//!   the order the languages are checked: the ones that give facts first (rulec, koyomi, chobo,
//!   geas, `.proto`), then the ones that receive them (kaname, then dandori, then sekisho, then
//!   ringi, then yuen and sakai).
//! - [`Project::references`]: every reference the files make, resolved through the index — the
//!   file it lands on, whether that file is part of the project, and what the language of that
//!   file says of the thing named.
//! - [`Project::check`]: each language's own `check` of the project's files, in that order, as
//!   its command prints it, for `ritsu check` to print again (DESIGN 8.3); and what rulec says in
//!   the check of one file of a line of another, filed under the file the line is in
//!   ([`Project::place_findings`]), as `ritsu check` and `ritsu gen` both count it (DESIGN 8.3,
//!   17.8).
//! - [`Project::laid`]: the files the project's requests are lowered to, the glue that joins each
//!   to the machine of its tickets and the machine, laid over the files the languages read, for
//!   the package `ritsu gen` makes (ringi's DESIGN 12.3).
//! - [`Project::named`]: what each file names of other files, as paths from the root, for the
//!   dependencies a module uses (W301, W303), and [`Project::reached`]: the files of the
//!   dependencies the files given reach, which `ritsu gen` makes a package of (DESIGN 17.8).
//!
//! This is the connecting layer (DESIGN 3.1): it depends on the base layer and on the ten
//! languages, and no language depends on it.

pub mod check;
pub mod joined;
pub mod named;
pub mod project;
pub mod requests;
pub mod resolve;

pub use joined::Joined;
pub use project::{File, ORDER, Project};
pub use resolve::{Landing, Resolved};
