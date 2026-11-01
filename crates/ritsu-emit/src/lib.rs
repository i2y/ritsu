//! What the generators of ritsu share about the languages they write (DESIGN 9.2). A generator
//! stays its language's own (9.1); what it writes about the target's surface is here, once:
//!
//! - [`words`]: the words each target will not take as a name, by the standard that lists
//!   them, and [`copies`]: the tables rulec and dandori hold their names to.
//! - [`ident`]: a name of the generated code from a name of the source, and whether a target
//!   takes a name of the source as it is, by the tables of [`xid`], made from the Unicode
//!   Character Database (THIRD_PARTY_NOTICES).
//! - [`lit`]: a string of the source as a literal of the target.
//! - [`header`]: the line that says a file is generated, and the comments it is written in.
//! - [`package`]: where a part goes in the package `ritsu gen` writes, and the paths one part imports
//!   another by.
//! - [`wasm`]: a Wasm module that answers `call: func(input: string) -> string`: how it is built,
//!   its world, and the functions of the canonical ABI it exports.
//!
//! koyomi, chobo and kaname write with these (chobo writes no Wasm), and rulec writes its Wasm
//! module with [`wasm`]; rulec and dandori read their tables of words from [`copies`] (C.11), and
//! keep their own ways of making names and literals.

pub mod copies;
pub mod header;
pub mod ident;
pub mod lit;
pub mod package;
pub mod wasm;
pub mod words;
pub mod xid;
