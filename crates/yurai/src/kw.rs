//! The words of the language, all of them, in one table (DESIGN 1.2).
//!
//! Every keyword has one English spelling and no synonym. The lexer cuts every word out as a
//! name; the parser matches names against the constants here by position. The check that a
//! name is not a keyword (E002) reads [`RESERVED`], and the table in the reference reads
//! [`TABLE`], so a word added here reaches both. The words of a naming (the tools and their
//! kinds) live in `names.rs`: they are keywords only where a naming is written.

// Words that start a line.
pub const REQUIREMENTS: &str = "requirements";
pub const DESCRIPTION: &str = "description";
pub const ROLE: &str = "role";
pub const SOURCE: &str = "source";
pub const SCOPE: &str = "scope";
pub const REQUIREMENT: &str = "requirement";

// A source.
pub const LAW: &str = "law";
pub const FILE: &str = "file";
pub const URL: &str = "url";
pub const ASOF: &str = "asof";
/// Written with the digest after it, `sha256:e880059021fbb67d`; the lexer reads the two as one.
pub const SHA256: &str = "sha256";
pub const EGOV: &str = "egov";
pub const ECFR: &str = "ecfr";

// The lines of a requirement.
pub const TEXT: &str = "text";
pub const IN: &str = "in";
pub const FORCE: &str = "force";
pub const OWNER: &str = "owner";
pub const REPLACES: &str = "replaces";
pub const FROM: &str = "from";
pub const DECIDED: &str = "decided";
pub const BY: &str = "by";
pub const SATISFIED: &str = "satisfied";
pub const VERIFIED: &str = "verified";
pub const NOT: &str = "not";

// The record under a link or a waiver.
pub const REVIEWED: &str = "reviewed";
pub const APPROVED: &str = "approved";

/// The words, by where they are written, as DESIGN 1.2 lists them.
pub const TABLE: &[(&str, &[&str])] = &[
    ("line", &[REQUIREMENTS, DESCRIPTION, ROLE, SOURCE, SCOPE, REQUIREMENT]),
    ("source", &[LAW, FILE, URL, ASOF, "sha256:", EGOV, ECFR, SOURCE]),
    (
        "requirement",
        &[TEXT, "in force", OWNER, REPLACES, FROM, DECIDED, BY, "satisfied by", "verified by", "not satisfied", "not verified"],
    ),
    ("record", &[REVIEWED, APPROVED, BY, "->"]),
    ("symbol", &["@", "..", ",", "->", "=", "#"]),
];

/// The words a name cannot be (E002): the words that start a line, and the words of the lines
/// of a requirement, its records included (DESIGN 1.2). The tools and kinds are not here: a
/// requirement may be called `output`.
pub const RESERVED: &[&str] = &[
    REQUIREMENTS, DESCRIPTION, ROLE, SOURCE, SCOPE, REQUIREMENT, TEXT, IN, FORCE, OWNER, REPLACES, FROM, DECIDED, BY, SATISFIED,
    VERIFIED, NOT, REVIEWED, APPROVED,
];

pub fn is_reserved(w: &str) -> bool {
    RESERVED.contains(&w)
}
