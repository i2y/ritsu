//! The words a target language will not take as a name, by the standard that lists them. A
//! generator holds every name it writes to the words of its target: one a target refuses stops
//! that target's build (koyomi says so first, with E009), and one a target already uses would
//! hide it (chobo and dandori write a `_` after it).
//!
//! The lists are the standards' own, in their order. What a generator holds a name to is a
//! [`Words`]: the lists it reads together. The tables rulec and dandori hold their names to are
//! in [`crate::copies`], with what each adds to the standards; the two read them from there.

/// Words made of lists, read as one set.
#[derive(Clone, Copy, Debug)]
pub struct Words(pub &'static [&'static [&'static str]]);

impl Words {
    pub fn contains(&self, w: &str) -> bool {
        self.0.iter().any(|l| l.contains(&w))
    }

    /// Every word, in the order of the lists, a word in two lists twice.
    pub fn iter(&self) -> impl Iterator<Item = &'static str> {
        self.0.iter().flat_map(|l| l.iter().copied())
    }
}

pub mod typescript {
    use super::Words;

    /// ECMAScript 2025 (ECMA-262, 16th edition) §13.1: the reserved words.
    pub const RESERVED: &[&str] = &[
        "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default",
        "delete", "do", "else", "enum", "export", "extends", "false", "finally", "for", "function",
        "if", "import", "in", "instanceof", "new", "null", "return", "super", "switch", "this",
        "throw", "true", "try", "typeof", "var", "void", "while", "with", "yield",
    ];

    /// The words strict mode reserves besides (`await` in a module is in [`RESERVED`]).
    pub const STRICT: &[&str] = &["let", "static", "implements", "interface", "package", "private", "protected", "public"];

    /// The names strict mode will not bind, and the values the globals already hold.
    pub const UNBINDABLE: &[&str] = &["arguments", "eval", "undefined", "NaN", "Infinity"];

    /// Every word a name of strict-mode code cannot be.
    pub const NAMES: Words = Words(&[RESERVED, STRICT, UNBINDABLE]);
}

pub mod python {
    use super::Words;

    /// Python 3.14.6, `keyword.kwlist`.
    pub const KEYWORDS: &[&str] = &[
        "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
        "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
        "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return",
        "try", "while", "with", "yield",
    ];

    /// Python 3.14.6, `keyword.softkwlist`.
    pub const SOFT_KEYWORDS: &[&str] = &["_", "case", "match", "type"];

    /// Every word a name cannot be, the soft keywords among them.
    pub const NAMES: Words = Words(&[KEYWORDS, SOFT_KEYWORDS]);
}

pub mod go {
    use super::Words;

    /// The Go Programming Language Specification (go1.25): the 25 keywords.
    pub const KEYWORDS: &[&str] = &[
        "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough",
        "for", "func", "go", "goto", "if", "import", "interface", "map", "package", "range",
        "return", "select", "struct", "switch", "type", "var",
    ];

    /// The predeclared identifiers: the types, the constants, the zero value and the builtin
    /// functions. A name may be one, and then hides it from what follows.
    pub const PREDECLARED: &[&str] = &[
        "any", "bool", "byte", "comparable", "complex64", "complex128", "error", "float32",
        "float64", "int", "int8", "int16", "int32", "int64", "rune", "string", "uint", "uint8",
        "uint16", "uint32", "uint64", "uintptr", "true", "false", "iota", "nil", "append", "cap",
        "clear", "close", "complex", "copy", "delete", "imag", "len", "make", "max", "min", "new",
        "panic", "print", "println", "real", "recover",
    ];

    /// Every word a name can be only by hiding one of the language's.
    pub const NAMES: Words = Words(&[KEYWORDS, PREDECLARED]);
}

pub mod rust {
    use super::Words;

    /// The Rust Reference (Rust 1.94, edition 2024): the strict and the reserved keywords.
    pub const KEYWORDS: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do",
        "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
        "gen",
    ];

    pub const NAMES: Words = Words(&[KEYWORDS]);
}

pub mod postgresql {
    use super::Words;

    /// PostgreSQL 18.0, `src/include/parser/kwlist.h`: the keywords Appendix C lists as reserved,
    /// reserved (can be function or type), and non-reserved (cannot be function or type). A name
    /// of a function or of a parameter cannot be one of them.
    pub const KEYWORDS: &[&str] = &[
        "all", "analyse", "analyze", "and", "any", "array", "as", "asc", "asymmetric", "both",
        "case", "cast", "check", "collate", "column", "constraint", "create", "current_catalog",
        "current_date", "current_role", "current_time", "current_timestamp", "current_user",
        "default", "deferrable", "desc", "distinct", "do", "else", "end", "except", "false",
        "fetch", "for", "foreign", "from", "grant", "group", "having", "in", "initially",
        "intersect", "into", "lateral", "leading", "limit", "localtime", "localtimestamp", "not",
        "null", "offset", "on", "only", "or", "order", "placing", "primary", "references",
        "returning", "select", "session_user", "some", "symmetric", "system_user", "table", "then",
        "to", "trailing", "true", "union", "unique", "user", "using", "variadic", "when", "where",
        "window", "with", "authorization", "binary", "collation", "concurrently", "cross",
        "current_schema", "freeze", "full", "ilike", "inner", "is", "isnull", "join", "left",
        "like", "natural", "notnull", "outer", "overlaps", "right", "similar", "tablesample",
        "verbose", "between", "bigint", "bit", "boolean", "char", "character", "coalesce", "dec",
        "decimal", "exists", "extract", "float", "greatest", "grouping", "inout", "int", "integer",
        "interval", "json", "json_array", "json_arrayagg", "json_exists", "json_object",
        "json_objectagg", "json_query", "json_scalar", "json_serialize", "json_table",
        "json_value", "least", "merge_action", "national", "nchar", "none", "normalize", "nullif",
        "numeric", "out", "overlay", "position", "precision", "real", "row", "setof", "smallint",
        "substring", "time", "timestamp", "treat", "trim", "values", "varchar", "xmlattributes",
        "xmlconcat", "xmlelement", "xmlexists", "xmlforest", "xmlnamespaces", "xmlparse", "xmlpi",
        "xmlroot", "xmlserialize", "xmltable",
    ];

    /// The reserved words of PL/pgSQL (`src/pl/plpgsql/src/pl_reserved_kwlist.h`) that
    /// [`KEYWORDS`] does not have: a PL/pgSQL body names its parameters.
    pub const PLPGSQL: &[&str] = &["begin", "by", "declare", "execute", "foreach", "if", "loop", "strict", "while"];

    pub const NAMES: Words = Words(&[PLPGSQL, KEYWORDS]);
}
