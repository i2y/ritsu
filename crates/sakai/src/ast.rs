//! What a `.ctx` says, as written (DESIGN 1, PLAN B.2): the two kinds of file, with the
//! position of everything a diagnostic may point at. Paths are as written; `resolve.rs` makes
//! them paths from the root.

use crate::naming::{Tool, Written};

/// A line and a column, both from 1; the column counts characters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

/// A `"…"` and where it starts.
#[derive(Clone, Debug, PartialEq)]
pub struct Str {
    pub value: String,
    pub pos: Pos,
}

/// `map 通販(shop) v1` or `context 在庫(inventory) v1`.
#[derive(Clone, Debug, PartialEq)]
pub struct Heading {
    pub name: String,
    pub alias: Option<String>,
    pub version: String,
    pub pos: Pos,
    pub name_pos: Pos,
}

#[derive(Clone, Debug, PartialEq)]
pub enum File {
    Map(MapFile),
    Context(ContextFile),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapFile {
    pub heading: Heading,
    pub description: Option<Str>,
    pub uses: Vec<Str>,
    pub covers: Vec<Str>,
    pub except: Vec<Str>,
    pub proto_roots: Vec<Str>,
    pub code: Vec<Code>,
}

/// `code python "py"`, and for Java the line under it, `test "…"`.
#[derive(Clone, Debug, PartialEq)]
pub struct Code {
    pub language: String,
    pub path: Str,
    pub test: Option<Str>,
    pub pos: Pos,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContextFile {
    pub heading: Heading,
    pub description: Option<Str>,
    pub owner: Option<Str>,
    pub also: Vec<Str>,
    pub owns: Vec<Item>,
    pub published: Vec<Published>,
    pub terms: Vec<Term>,
    pub relations: Vec<Relation>,
}

/// A file or a directory under `owns`, `layer` and `shared kernel with`: `dir "…"`, or a tool's
/// file (`rulec "…"`).
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// None for `dir`.
    pub tool: Option<Tool>,
    pub path: Str,
}

/// `published language <package>` and the lines under it (DESIGN 1.4).
#[derive(Clone, Debug, PartialEq)]
pub struct Published {
    pub package: String,
    pub pos: Pos,
    pub package_pos: Pos,
    pub protos: Vec<Str>,
    pub rulec: Option<Str>,
    pub services: Vec<(String, Pos)>,
    pub generated: Vec<Str>,
}

/// A term of the glossary (DESIGN 1.6).
#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub name: String,
    pub pos: Pos,
    pub definition: Option<Str>,
    pub as_term: Option<AsTerm>,
    pub means: Vec<Element>,
    pub also: Vec<Str>,
}

/// `as 受注.注文`.
#[derive(Clone, Debug, PartialEq)]
pub struct AsTerm {
    pub context: String,
    pub term: String,
    pub pos: Pos,
}

/// A thing in an artifact, as written: the long form of DESIGN 2.1, or the short form of
/// DESIGN 2.7 (a proto element of the context's own published language, or of a `through`
/// package), which `resolve.rs` turns into the long one.
#[derive(Clone, Debug, PartialEq)]
pub enum Element {
    Long { written: Written, pos: Pos },
    Short { kind: String, name: String, child: Option<(String, String)>, pos: Pos },
}

impl Element {
    pub fn pos(&self) -> Pos {
        match self {
            Element::Long { pos, .. } | Element::Short { pos, .. } => *pos,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Relation {
    pub kind: RelKind,
    pub partner: String,
    pub partner_pos: Pos,
    pub pos: Pos,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RelKind {
    Upstream(Upstream),
    /// `downstream <context> supplier`
    Downstream,
    SharedKernel(Vec<Item>),
    Partnership,
    SeparateWays,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Conformist,
    Acl,
    Customer,
}

impl Role {
    pub fn word(self) -> &'static str {
        match self {
            Role::Conformist => "conformist",
            Role::Acl => "anticorruption layer",
            Role::Customer => "customer",
        }
    }

    /// The key `api` gives it.
    pub fn key(self) -> &'static str {
        match self {
            Role::Conformist => "conformist",
            Role::Acl => "anticorruption_layer",
            Role::Customer => "customer",
        }
    }
}

/// `upstream <context> <role>` and the lines under it (DESIGN 1.5, 1.7).
#[derive(Clone, Debug, PartialEq)]
pub struct Upstream {
    pub roles: Vec<(Role, Pos)>,
    pub through: Vec<(String, Pos)>,
    pub layer: Vec<Item>,
    pub enums: Vec<EnumMap>,
    pub terms: Vec<TermMap>,
}

/// `enum <upstream enum> -> <target>` and its value lines.
#[derive(Clone, Debug, PartialEq)]
pub struct EnumMap {
    pub from: String,
    pub from_pos: Pos,
    pub target: Target,
    pub values: Vec<ValueMap>,
    pub pos: Pos,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// A proto enum (long form, or `enum <name>` of the context's own published language), or a
    /// rule's enum.
    Element(Element),
    /// A name only: the values on the right are taken as written.
    Name(String, Pos),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValueMap {
    pub from: String,
    pub from_pos: Pos,
    pub to: ValueTo,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ValueTo {
    Value(String, Pos),
    Refuse(Option<Str>, Pos),
}

/// `term <upstream term> -> <downstream term>`.
#[derive(Clone, Debug, PartialEq)]
pub struct TermMap {
    pub from: String,
    pub from_pos: Pos,
    pub to: String,
    pub to_pos: Pos,
    pub pos: Pos,
}
