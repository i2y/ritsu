//! The one reader of `.proto` files (DESIGN 4.10). It began as sakai's (the elements with their
//! lines, the names of types resolved across files) and reads, besides, what rulec and dandori
//! read with their own readers: Protovalidate's rules on fields and messages, the `buf.yaml`
//! and `buf.lock` of a module (rulec), every option as a tree, and the files a file imports
//! through every level (dandori).
//!
//! - [`read`]: one file — `syntax`, `package`, `import` (`public`, `weak`), messages (nested
//!   ones by their dotted names), fields (`optional`, `repeated`, `map`, `oneof`, `json_name`),
//!   enums and their values and numbers, services and their methods (streaming or not), and
//!   every option as written ([`Opt`]).
//! - [`value`]: an option's value in protobuf's text format, and the tree the options of an
//!   element make ([`value::tree`]).
//! - [`validate`]: what `(buf.validate.field)`, `(buf.validate.message)` and
//!   `(buf.validate.oneof)` ask. CEL is kept as text.
//! - [`buf`]: `buf.yaml`'s `deps` and `buf.lock`'s pins.
//! - [`load`] and [`load_from`]: many files, and the names across them ([`Protos::resolve`]).
//!   Google's well-known types, `buf/validate/validate.proto` and dandori's
//!   `dandori/v1/options.proto` are known without their files.
//!
//! What a language makes of what is read — rulec's aliases for an enum's values, sakai's value
//! that says nothing is set, dandori's refusal of proto2 — stays with the language.

pub mod buf;
mod load;
mod model;
mod read;
pub mod validate;
pub mod value;

pub use load::{DANDORI_OPTIONS, Issue, Protos, Resolved, Symbol, WELL_KNOWN, import_candidates, is_known, known_package, load, load_from};
pub use model::{Enum, EnumValue, Field, Import, Label, Message, Method, Oneof, Opt, ProtoFile, Service, Type, json_name};
pub use read::{Problem, ReadError, SCALARS, read};
pub use value::Value;
