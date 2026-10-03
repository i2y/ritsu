//! What the generators of ritsu share about the languages they write (DESIGN 9.2). A generator
//! stays its language's own (9.1); what it writes about the target's surface is here, once:
//!
//! - [`words`]: the words each target will not take as a name, by the standard that lists
//!   them, and [`copies`]: the tables rulec and dandori hold their names to.
//! - [`ident`]: a name of the generated code from a name of the source.
//! - [`lit`]: a string of the source as a literal of the target.
//! - [`header`]: the line that says a file is generated, and the comments it is written in.
//!
//! koyomi and chobo write with all of these; rulec and dandori read their tables of words from
//! [`copies`] (C.11), and keep their own ways of making names and literals.

pub mod copies;
pub mod header;
pub mod ident;
pub mod lit;
pub mod words;
