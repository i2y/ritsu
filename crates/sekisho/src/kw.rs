//! The words of the language, all of them, in one table (DESIGN 9.1).
//!
//! Every keyword has one English spelling and no synonym. The lexer cuts every word out as a
//! word; the parser matches words against the constants here by position, so `principal` at the
//! start of a line declares a type, under a policy it picks the principals, and in a condition
//! it reads one (as in Cedar).
//!
//! A name or an alias cannot be one of the [`RESERVED`] words (E002): the words a condition, a
//! computed value or a type is made of, where a name spelled the same would let a line be read
//! two ways (`resource.status is not` — the value `not`, or `is not`?). The other keywords start
//! a line or stand where only they can, so a name may be spelled like them: an enum's value
//! `open`, an attribute `description`, an input `range`.

// Words that start a line at the left margin.
pub const GATE: &str = "gate";
pub const DESCRIPTION: &str = "description";
pub const NAMESPACE: &str = "namespace";
pub const USE: &str = "use";
pub const TODAY: &str = "today";
pub const ENUM: &str = "enum";
pub const ROLE: &str = "role";
pub const PRINCIPAL: &str = "principal";
pub const WORKFLOW: &str = "workflow";
pub const RESOURCE: &str = "resource";
pub const ACTION: &str = "action";
pub const PERMIT: &str = "permit";
pub const FORBID: &str = "forbid";
pub const EXPECT: &str = "expect";
pub const SEPARATE: &str = "separate";

// After `use`.
pub const RULE: &str = "rule";
pub const DATES: &str = "dates";
pub const CALENDAR: &str = "calendar";
pub const OPENAPI: &str = "openapi";
pub const PROTO: &str = "proto";
pub const ASYNCAPI: &str = "asyncapi";
pub const BOOK: &str = "book";
pub const FROM: &str = "from";

// Lines inside a block.
pub const INCLUDES: &str = "includes";
pub const CAN: &str = "can";
pub const ROLES: &str = "roles";
pub const ATTRIBUTES: &str = "attributes";
pub const GUARDS: &str = "guards";
pub const NOBODY: &str = "nobody";
pub const INPUT: &str = "input";
pub const CONTEXT: &str = "context";
pub const ACTIONS: &str = "actions";
pub const WHEN: &str = "when";
pub const UNLESS: &str = "unless";

// Conditions and computed values.
pub const IN: &str = "in";
pub const IS: &str = "is";
pub const NOT: &str = "not";
pub const AND: &str = "and";
pub const OR: &str = "or";
pub const ANY: &str = "any";
pub const OPEN: &str = "open";
pub const ALLOW: &str = "allow";
pub const DENY: &str = "deny";
pub const TRUE: &str = "true";
pub const FALSE: &str = "false";

// Types and ranges.
pub const BOOL: &str = "bool";
pub const DATE: &str = "date";
/// rulec's numbers that take no `[…]`: a whole number with no unit, and a rate with no step.
pub const NUMBER: &str = "number";
pub const RATE: &str = "rate";
pub const MONEY: &str = "money";
pub const RANGE: &str = "range";
pub const OFFSET: &str = "offset";

/// The words, by where they are written, as DESIGN 9.1 lists them. The words of the units are
/// rulec's (`ritsu_units`): `money[…]`, `mass[…]`, `length[…]` and the rest.
pub const TABLE: &[(&str, &[&str])] = &[
    ("line", &[GATE, DESCRIPTION, NAMESPACE, USE, TODAY, ENUM, ROLE, PRINCIPAL, WORKFLOW, RESOURCE, ACTION, PERMIT, FORBID, EXPECT, SEPARATE]),
    ("use", &[RULE, DATES, CALENDAR, OPENAPI, PROTO, ASYNCAPI, BOOK, GATE, FROM]),
    ("block", &[DESCRIPTION, INCLUDES, CAN, ROLES, ATTRIBUTES, GUARDS, PRINCIPAL, RESOURCE, FROM, NOBODY, INPUT, CONTEXT, ACTION, ACTIONS, WHEN, UNLESS]),
    ("condition", &[IN, IS, NOT, AND, OR, ANY, WORKFLOW, OPEN, TODAY, ALLOW, DENY, PRINCIPAL, RESOURCE, TRUE, FALSE]),
    ("type and range", &[BOOL, DATE, NUMBER, RATE, MONEY, "mass", "length", "area", "volume", "duration", "temperature", "sound", RANGE, OFFSET]),
];

/// The words a name or an alias cannot be (E002): the ones that would let a condition, a computed
/// value or a type be read two ways.
pub const RESERVED: &[&str] = &[
    // a condition: `principal in …`, `x is not y`, `a and b`, `principal is workflow w`
    AND, OR, NOT, IN, IS, PRINCIPAL, RESOURCE, WORKFLOW, TRUE, FALSE,
    // `action any`
    ANY,
    // a computed value: `today <= …`
    TODAY,
    // a type with no `[…]`: `x : bool`, `x : rate`
    BOOL, DATE, NUMBER, RATE,
];

pub fn is_reserved(w: &str) -> bool {
    RESERVED.contains(&w)
}

/// The words Cedar will not take as an identifier (Cedar 4.13.0, `cst_to_ast.rs`): a name of
/// sekisho's whose alias is one of them cannot be written in Cedar (E008). `true`, `false`, `in`
/// and `is` are sekisho's reserved words as well, and are said with E002 first.
pub const CEDAR_RESERVED: &[&str] = &["true", "false", "if", "then", "else", "in", "is", "has", "like"];

/// The type names a principal's or a resource's alias cannot be (E008): Cedar's `Action`, which a
/// schema cannot declare; the builtin types, which a type of the same name shadows (`Bool`,
/// `Long`, `String`, a warning of `cedar validate`) and the words of the JSON schema's types
/// (`Boolean`, `Entity`, `Extension`, `Record`, `Set`); and sekisho's own `Role` and `Workflow`.
pub const CEDAR_TYPES: &[&str] = &["Action", "Bool", "Boolean", "Entity", "Extension", "Long", "Record", "Set", "String", "Role", "Workflow"];
