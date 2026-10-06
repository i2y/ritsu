//! Cedar's policies and schemas (DESIGN 4.18), read and written as Cedar 4.13.0 reads and writes
//! them (the language of version 4.5; <https://github.com/cedar-policy/cedar> at `v4.13.0`).
//! ritsu reads Cedar as a standard format, as it reads `.proto`, OpenAPI and OpenSpec, and checks
//! the languages against it; sekisho writes it.
//!
//! - Policies (`.cedar`): [`parse_policies`] reads a policy set into [`Policy`] values whose every
//!   expression knows its line and column; [`policies_to_json`] writes the JSON policy format
//!   (byte for byte what `cedar translate-policy --direction cedar-to-json` prints);
//!   [`format_policies`] is `cedar format` (comments kept); [`write_policies`] writes a policy
//!   set as `cedar format` lays it out.
//! - Schemas: [`parse_schema`] reads the Cedar schema format (`.cedarschema`), [`parse_schema_json`]
//!   the JSON schema format; [`schema_to_json`] and [`write_schema`] write them as
//!   `cedar translate-schema` does in each direction.
//!
//! What the official tool does not read, these do not read either, and they say which line and
//! column and what, in both languages ([`Error`], the shape of [`crate::yaml::Error`]).
//! `tests/cedar.rs` holds every function to what the official CLI printed for the files in
//! `tests/fixtures/cedar/` (written by `expected.sh` beside them), so the tests need no Cedar.

mod ast;
mod cst;
mod est;
mod format;
mod lexer;
mod pretty;
mod schema;
mod write;

pub use ast::*;
pub use schema::*;

use crate::json::Json;
use crate::text::Text;

/// Why a text is not read, and where (lines and columns from 1; a column counts characters).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub line: usize,
    pub col: usize,
    pub message: Text,
}

/// Read a policy set: its static policies and templates, in the order they are written.
pub fn parse_policies(src: &str) -> Result<PolicySet, Error> {
    let parsed = cst::parse(src)?;
    ast::convert(src, &parsed)
}

/// The policy set in Cedar's JSON policy format, as `cedar translate-policy --direction
/// cedar-to-json` prints it: `templates`, `staticPolicies` and an empty `templateLinks`, each
/// policy under its id. [`Json::compact`] of it is the CLI's line without the line feed.
pub fn policies_to_json(set: &PolicySet) -> Json {
    est::policy_set(set)
}

/// One policy or template in the JSON policy format (the EST).
pub fn policy_to_json(p: &Policy) -> Json {
    est::policy(p)
}

/// `cedar format`: the policy set laid out in `line_width` columns, indented by `indent_width`,
/// its comments kept. `cedar format` uses 80 and 2.
pub fn format_policies(src: &str, line_width: usize, indent_width: usize) -> Result<String, Error> {
    format::format(src, line_width, indent_width)
}

/// The policy set as Cedar text laid out as `cedar format` lays it out (80 columns, two spaces):
/// what sekisho writes. It reads back as the same policies. A policy Cedar would not read (a
/// reserved word as a name, an action of another type than `Action`) is an error, said as the
/// reader says it, at its place in the unformatted text.
pub fn write_policies(set: &PolicySet) -> Result<String, Error> {
    write::policies(set)
}

/// One policy as Cedar text, laid out as [`write_policies`] lays it out.
pub fn write_policy(p: &Policy) -> Result<String, Error> {
    write::policies(&PolicySet { policies: vec![p.clone()] })
}

/// An expression as Cedar text, on one line, with the parentheses the grammar needs.
pub fn write_expr(e: &Expr) -> String {
    write::expr(e)
}

/// Read a schema in the Cedar schema format (`.cedarschema`).
pub fn parse_schema(src: &str) -> Result<Schema, Error> {
    schema::parse_cedar(src)
}

/// Read a schema in the JSON schema format.
pub fn parse_schema_json(src: &str) -> Result<Schema, Error> {
    schema::parse_json(src)
}

/// The schema in the JSON schema format, as `cedar translate-schema --direction cedar-to-json`
/// prints it ([`Json::compact`] of it).
pub fn schema_to_json(s: &Schema) -> Json {
    schema::to_json(s)
}

/// The schema in the Cedar schema format, as `cedar translate-schema --direction json-to-cedar`
/// prints it (without the line feed `println!` adds). What that format cannot write is an
/// error, as it is for the CLI: an entity type and a common type of one name in a named
/// namespace, and an entity whose shape is not a record.
pub fn write_schema(s: &Schema) -> Result<String, Error> {
    schema::write(s)
}
