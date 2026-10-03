//! The words of the language, all of them, in one table (DESIGN 1.2).
//!
//! Every keyword has one English spelling and no synonym. The lexer cuts every word out as it
//! is; the parser matches words against the constants here by position. That a name is not a
//! keyword (E002) is read from [`RESERVED`], and the reference reads [`TABLE`], so a word added
//! here reaches both.

pub const MAP: &str = "map";
pub const CONTEXT: &str = "context";
pub const DESCRIPTION: &str = "description";
pub const OWNER: &str = "owner";
pub const ALSO: &str = "also";
pub const USE: &str = "use";
pub const COVERS: &str = "covers";
pub const EXCEPT: &str = "except";
pub const ROOT: &str = "root";
pub const CODE: &str = "code";
pub const TEST: &str = "test";
pub const OWNS: &str = "owns";
pub const DIR: &str = "dir";
pub const PUBLISHED: &str = "published";
pub const LANGUAGE: &str = "language";
pub const OPEN: &str = "open";
pub const HOST: &str = "host";
pub const SERVICE: &str = "service";
pub const GENERATED: &str = "generated";
pub const TERMS: &str = "terms";
pub const MEANS: &str = "means";
pub const AS: &str = "as";
pub const UPSTREAM: &str = "upstream";
pub const DOWNSTREAM: &str = "downstream";
pub const THROUGH: &str = "through";
pub const LAYER: &str = "layer";
pub const ENUM: &str = "enum";
pub const TERM: &str = "term";
pub const REFUSE: &str = "refuse";
pub const SHARED: &str = "shared";
pub const KERNEL: &str = "kernel";
pub const WITH: &str = "with";
pub const PARTNERSHIP: &str = "partnership";
pub const SEPARATE: &str = "separate";
pub const WAYS: &str = "ways";
pub const FROM: &str = "from";
pub const CONFORMIST: &str = "conformist";
pub const ANTICORRUPTION: &str = "anticorruption";
pub const CUSTOMER: &str = "customer";
pub const SUPPLIER: &str = "supplier";
pub const MESSAGE: &str = "message";
pub const FIELD: &str = "field";
pub const VALUE: &str = "value";
pub const METHOD: &str = "method";

/// The languages a map can say where the code is (`code python "py"`).
pub const LANGUAGES: &[&str] = &["python", "typescript", "java", "go"];

/// The words, by where they are written, as DESIGN 1.2 lists them. A keyword of several words is
/// one entry.
pub const TABLE: &[(&str, &[&str])] = &[
    ("map file", &["map", "description", "use context", "covers", "except", "proto root", "code", "python", "typescript", "java", "go", "test"]),
    ("context file", &["context", "description", "owner", "also", "owns", "dir", "published language", "open host service", "generated dir", "terms", "means", "as"]),
    (
        "relationship",
        &[
            "upstream",
            "downstream",
            "conformist",
            "anticorruption layer",
            "customer",
            "supplier",
            "through",
            "layer",
            "enum",
            "term",
            "refuse",
            "shared kernel with",
            "partnership with",
            "separate ways from",
        ],
    ),
    ("tool", &["rulec", "dandori", "koyomi", "chobo", "geas", "proto", "file", "yurai", "sakai"]),
    (
        "kind",
        &[
            "input", "output", "enum", "value", "table", "clause", "define", "derive", "machine", "source", "date", "claim", "unit", "account", "transfer", "service",
            "method", "message", "field", "requirement", "context", "term",
        ],
    ),
];

/// The words a name cannot be: every word of every keyword in [`TABLE`].
pub fn is_reserved(w: &str) -> bool {
    TABLE.iter().any(|(_, ws)| ws.iter().any(|k| k.split(' ').any(|p| p == w)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_word_of_a_keyword_is_reserved() {
        for w in ["map", "published", "language", "anticorruption", "layer", "ways", "rulec", "value", "refuse"] {
            assert!(super::is_reserved(w), "{w}");
        }
        assert!(!super::is_reserved("在庫"));
        assert!(!super::is_reserved("Order"));
    }
}
