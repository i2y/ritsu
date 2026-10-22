//! The words of the language, all of them, in one table (DESIGN 2.2).
//!
//! Every keyword has one English spelling and no synonym. The lexer cuts every word out as a word;
//! the parser matches words against the constants here by position, so `count` declares the
//! length of a list after a type, and is the builtin that counts in a call.
//!
//! A name cannot be one of the fourteen [`RESERVED`] words (E002): the words an expression or a
//! block is made of, where a name spelled the same would let a line be read two ways (`let in =
//! 3`). The other keywords start a line or stand where only they can, so a name may be spelled
//! like them: a field `total`, a parameter `description`, an argument `count`. The words of the
//! types that rulec shares (`money`, `rate`, `range`, `step`, …) are rulec's, with rulec's meaning.

// Words that start a line at the left margin.
pub const MODULE: &str = "module";
pub const DESCRIPTION: &str = "description";
pub const USE: &str = "use";
pub const ENUM: &str = "enum";
pub const RECORD: &str = "record";
pub const FN: &str = "fn";
pub const EXAMPLES: &str = "examples";
/// `private fn`, `private record`, `private enum` (DESIGN 28.2): read at the start of a line alone.
pub const PRIVATE: &str = "private";

// After `use`: the word of the first line of the file it reads, then `from`.
pub const RULE: &str = "rule";
pub const DATES: &str = "dates";
pub const BOOK: &str = "book";
pub const FROM: &str = "from";

// Types and their declarations: rulec's, and the list.
pub const NUMBER: &str = "number";
pub const BOOL: &str = "bool";
pub const STRING: &str = "string";
pub const DATE: &str = "date";
pub const RATE: &str = "rate";
pub const MONEY: &str = "money";
pub const QUANTITIES: [&str; 7] = ["mass", "length", "area", "volume", "duration", "temperature", "sound"];
pub const LIST: &str = "list";
/// `dict[K, V]` and `set[T]` (DESIGN 24.2): read as types only where `[` comes after them, so that a
/// record or an enum may still be named so.
pub const DICT: &str = "dict";
pub const SET: &str = "set";
/// `entry[K, V]`, an element of what `d.entries()` answers, and `indexed[T]`, one of `xs.indexed()`
/// (DESIGN 24.3): read as types where `[` comes after them.
pub const ENTRY: &str = "entry";
pub const INDEXED: &str = "indexed";
/// `running[T, W]`: an element of what `running_totals` answers (DESIGN 3.6).
pub const RUNNING: &str = "running";
/// `result[T, E]`: a value, or why there is none (DESIGN 26.3); read as a type where `[` comes after
/// it.
pub const RESULT: &str = "result";
/// `K: key`: a type parameter held to the types that can be keys (DESIGN 26.1); read after `:` in
/// the type parameters alone.
pub const KEY: &str = "key";
pub const STEP: &str = "step";
pub const INCL_TAX: &str = "incl_tax";
pub const EXCL_TAX: &str = "excl_tax";
pub const RANGE: &str = "range";
pub const COUNT: &str = "count";

// The end of a function's head: its effect (DESIGN 4).
pub const TOTAL: &str = "total";
pub const FAILS: &str = "fails";
pub const DIVERGES: &str = "diverges";
/// The kinds of failure, after `fails` (DESIGN 4.2).
pub const KINDS: [&str; 5] = ["rule", "date", "range", "overflow", "zero"];

// The body of a function.
pub const LET: &str = "let";
pub const IF: &str = "if";
pub const THEN: &str = "then";
pub const ELSE: &str = "else";
pub const MATCH: &str = "match";
pub const SOME: &str = "some";
pub const NONE: &str = "none";
pub const TRUE: &str = "true";
pub const FALSE: &str = "false";
pub const AND: &str = "and";
pub const OR: &str = "or";
pub const NOT: &str = "not";
/// `fee in USDc`: the same value in another unit of its dimension (DESIGN 3.2).
pub const IN: &str = "in";
/// `let x = try e`: an `err` or a `none` of `e` is the function's answer (DESIGN 26.3).
pub const TRY: &str = "try";

/// The words, by where they are written, as DESIGN 2.2 lists them.
pub const TABLE: &[(&str, &[&str])] = &[
    ("line", &[MODULE, DESCRIPTION, USE, PRIVATE, ENUM, RECORD, FN, EXAMPLES]),
    ("use", &[RULE, DATES, BOOK, MODULE, FROM]),
    ("type", &[NUMBER, BOOL, STRING, DATE, RATE, MONEY, "mass", "length", "area", "volume", "duration", "temperature", "sound", LIST, DICT, SET, ENTRY, INDEXED, RESULT, RUNNING, STEP, INCL_TAX, EXCL_TAX, RANGE, COUNT]),
    ("type parameter", &[KEY]),
    ("effect", &[TOTAL, FAILS, DIVERGES, "rule", "date", "range", "overflow", "zero"]),
    ("body", &[DESCRIPTION, LET, TRY, IF, THEN, ELSE, MATCH, SOME, NONE, TRUE, FALSE, AND, OR, NOT, IN]),
];

/// The words a name cannot be (E002): the ones an expression or a block would read as something
/// else.
pub const RESERVED: [&str; 14] = [LET, TRY, IF, THEN, ELSE, MATCH, SOME, NONE, TRUE, FALSE, AND, OR, NOT, IN];

pub fn is_reserved(w: &str) -> bool {
    RESERVED.contains(&w)
}

/// The builtin functions written as functions (DESIGN 3.7, 21.4): read as builtins only where a call
/// is written, and not reserved; a function of the module may not take one of their names (E006).
/// Each of them but `numbers`, `char`, `ok` and `err` may be written after a value too, the value its
/// first argument (`lines.sum(l => l.amount)` is `sum(lines, l => l.amount)`).
pub const BUILTINS: [&str; 28] = [
    "min",
    "max",
    "mod",
    "isqrt",
    "allocate",
    "up",
    "down",
    "half_up",
    "half_down",
    "half_even",
    "count",
    "sum",
    "any",
    "all",
    "map",
    "filter",
    "first",
    "max_of",
    "min_of",
    "running_totals",
    "reduce",
    "starts_with",
    "concat",
    "text",
    "numbers",
    "char",
    "ok",
    "err",
];

/// The builtins written only after a value, the value their first argument (DESIGN 21.4, 24.3,
/// 25.2): `d.get(k)`, `xs.sort_by(x => x.priority)`, `s.trim()`. A function or a `use` of a module may
/// take one of their names (`fn split`): a call by the name alone is the module's, and one after a
/// value is the value's builtin. Which builtin a name is comes from the type of the value: `slice`,
/// `take`, `drop` and `contains` are a list's and a string's.
pub const METHODS: [&str; 54] = [
    "get",
    "put",
    "remove",
    "has",
    "keys",
    "values",
    "entries",
    "map_values",
    "merge",
    "merge_with",
    "add",
    "union",
    "intersect",
    "difference",
    "to_list",
    "append",
    "at",
    "last",
    "slice",
    "take",
    "drop",
    "reverse",
    "sort",
    "sort_by",
    "sort_by_desc",
    "distinct",
    "contains",
    "flatten",
    "flat_map",
    "filter_map",
    "indexed",
    "max_by",
    "min_by",
    "to_set",
    "to_dict",
    "group_by",
    "join",
    "chars",
    "code_points",
    "to_text",
    "index_of",
    "ends_with",
    "split",
    "words",
    "replace",
    "trim",
    "trim_start",
    "trim_end",
    "lower_ascii",
    "upper_ascii",
    "pad_start",
    "pad_end",
    "parse_number",
    "parse_date",
];

/// The builtins written only as functions: `numbers(1, 5)`, `char(10)`, and the values of a `result`,
/// `ok(x)` and `err(e)` (DESIGN 26.3).
pub const FUNCTIONS_ONLY: [&str; 4] = ["numbers", "char", "ok", "err"];

pub fn is_builtin(w: &str) -> bool {
    BUILTINS.contains(&w)
}

/// Whether a builtin is written only after a value.
pub fn is_method(w: &str) -> bool {
    METHODS.contains(&w)
}

/// The five ways of rounding, rulec's (DESIGN 3.3).
pub const ROUNDINGS: [&str; 5] = ["up", "down", "half_up", "half_down", "half_even"];

/// The names the generated code keeps for itself in every target (DESIGN 10.1): the helper module
/// and its types. A name of the module whose alias is one of them is E007.
pub const HELPERS: [&str; 6] = ["kaname", "KanameError", "Q", "Date", "Rat", "glue"];
