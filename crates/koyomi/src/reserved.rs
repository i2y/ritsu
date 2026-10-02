//! The words the five targets will not take as an identifier (E009, PLAN B.4). Every ASCII
//! alias becomes an identifier in each of them: a function, a parameter, a schema. An alias
//! that one of them refuses would stop that target's build, so `check` refuses it first.

/// ECMAScript 2025 (ECMA-262, 16th edition) §13.1 reserved words, the strict-mode reserved
/// words (`implements` … `yield`, and `await` in modules), and the names strict mode will not
/// bind or that the globals already hold.
pub const TYPESCRIPT: &[&str] = &[
    "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete", "do", "else",
    "enum", "export", "extends", "false", "finally", "for", "function", "if", "import", "in", "instanceof", "new",
    "null", "return", "super", "switch", "this", "throw", "true", "try", "typeof", "var", "void", "while", "with",
    "yield", "let", "static", "implements", "interface", "package", "private", "protected", "public", "arguments",
    "eval", "undefined", "NaN", "Infinity",
];

/// Python 3.14.6, `keyword.kwlist` and `keyword.softkwlist`.
pub const PYTHON: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
    "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal",
    "not", "or", "pass", "raise", "return", "try", "while", "with", "yield", "_", "case", "match", "type",
];

/// The Go Programming Language Specification (go1.25): the 25 keywords and the predeclared
/// identifiers. A parameter takes the alias as it is, and one named like a predeclared
/// identifier would hide it from the body.
pub const GO: &[&str] = &[
    "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "func", "go",
    "goto", "if", "import", "interface", "map", "package", "range", "return", "select", "struct", "switch", "type",
    "var", "any", "bool", "byte", "comparable", "complex64", "complex128", "error", "float32", "float64", "int",
    "int8", "int16", "int32", "int64", "rune", "string", "uint", "uint8", "uint16", "uint32", "uint64", "uintptr",
    "true", "false", "iota", "nil", "append", "cap", "clear", "close", "complex", "copy", "delete", "imag", "len",
    "make", "max", "min", "new", "panic", "print", "println", "real", "recover",
];

/// The Rust Reference (Rust 1.94, edition 2024): strict and reserved keywords.
pub const RUST: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
    "trait", "true", "type", "unsafe", "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box",
    "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen",
];

/// PostgreSQL 18.0, `src/include/parser/kwlist.h`: the keywords Appendix C lists as reserved,
/// reserved (can be function or type), and non-reserved (cannot be function or type). An
/// alias names a function or a parameter, and these cannot be one of the two. Then the
/// reserved words of PL/pgSQL (`src/pl/plpgsql/src/pl_reserved_kwlist.h`) that the list above
/// does not have: the generated functions are PL/pgSQL, and their bodies name the parameters.
pub const POSTGRESQL: &[&str] = &[
    "begin", "by", "declare", "execute", "foreach", "if", "loop", "strict", "while",
    "all", "analyse", "analyze", "and", "any", "array", "as", "asc", "asymmetric", "both", "case", "cast", "check",
    "collate", "column", "constraint", "create", "current_catalog", "current_date", "current_role", "current_time",
    "current_timestamp", "current_user", "default", "deferrable", "desc", "distinct", "do", "else", "end", "except",
    "false", "fetch", "for", "foreign", "from", "grant", "group", "having", "in", "initially", "intersect", "into",
    "lateral", "leading", "limit", "localtime", "localtimestamp", "not", "null", "offset", "on", "only", "or",
    "order", "placing", "primary", "references", "returning", "select", "session_user", "some", "symmetric",
    "system_user", "table", "then", "to", "trailing", "true", "union", "unique", "user", "using", "variadic", "when",
    "where", "window", "with", "authorization", "binary", "collation", "concurrently", "cross", "current_schema",
    "freeze", "full", "ilike", "inner", "is", "isnull", "join", "left", "like", "natural", "notnull", "outer",
    "overlaps", "right", "similar", "tablesample", "verbose", "between", "bigint", "bit", "boolean", "char",
    "character", "coalesce", "dec", "decimal", "exists", "extract", "float", "greatest", "grouping", "inout", "int",
    "integer", "interval", "json", "json_array", "json_arrayagg", "json_exists", "json_object", "json_objectagg",
    "json_query", "json_scalar", "json_serialize", "json_table", "json_value", "least", "merge_action", "national",
    "nchar", "none", "normalize", "nullif", "numeric", "out", "overlay", "position", "precision", "real", "row",
    "setof", "smallint", "substring", "time", "timestamp", "treat", "trim", "values", "varchar", "xmlattributes",
    "xmlconcat", "xmlelement", "xmlexists", "xmlforest", "xmlnamespaces", "xmlparse", "xmlpi", "xmlroot",
    "xmlserialize", "xmltable",
];

/// The targets, by the name a diagnostic gives them.
pub const TARGETS: &[(&str, &[&str])] =
    &[("TypeScript", TYPESCRIPT), ("Python", PYTHON), ("Go", GO), ("Rust", RUST), ("PostgreSQL", POSTGRESQL)];

/// The targets that will not take `alias` as it is.
pub fn refusing(alias: &str) -> Vec<&'static str> {
    TARGETS.iter().filter(|(_, ws)| ws.contains(&alias)).map(|(n, _)| *n).collect()
}
