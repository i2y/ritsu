//! The words of the language, all of them, in one table (DESIGN 2.2).
//!
//! Every keyword has one English spelling and no synonym. The lexer cuts every word out as a word;
//! the parser matches words against the constants here by position, so `count` declares the
//! length of a list after a type, and is the builtin that counts in a call.
//!
//! A name cannot be one of the thirteen [`RESERVED`] words (E002): the words an expression or a
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
/// `running[T, W]`: an element of what `running_totals` answers (DESIGN 3.6).
pub const RUNNING: &str = "running";
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

/// The words, by where they are written, as DESIGN 2.2 lists them.
pub const TABLE: &[(&str, &[&str])] = &[
    ("line", &[MODULE, DESCRIPTION, USE, ENUM, RECORD, FN, EXAMPLES]),
    ("use", &[RULE, DATES, BOOK, MODULE, FROM]),
    ("type", &[NUMBER, BOOL, STRING, DATE, RATE, MONEY, "mass", "length", "area", "volume", "duration", "temperature", "sound", LIST, RUNNING, STEP, INCL_TAX, EXCL_TAX, RANGE, COUNT]),
    ("effect", &[TOTAL, FAILS, DIVERGES, "rule", "date", "range", "overflow", "zero"]),
    ("body", &[DESCRIPTION, LET, IF, THEN, ELSE, MATCH, SOME, NONE, TRUE, FALSE, AND, OR, NOT, IN]),
];

/// The words a name cannot be (E002): the ones an expression or a block would read as something
/// else.
pub const RESERVED: [&str; 13] = [LET, IF, THEN, ELSE, MATCH, SOME, NONE, TRUE, FALSE, AND, OR, NOT, IN];

pub fn is_reserved(w: &str) -> bool {
    RESERVED.contains(&w)
}

/// The builtin functions (DESIGN 3.7): read as builtins only where a call is written, and not
/// reserved; a function of the module may not take one of their names (E006).
pub const BUILTINS: [&str; 24] = [
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
];

pub fn is_builtin(w: &str) -> bool {
    BUILTINS.contains(&w)
}

/// The five ways of rounding, rulec's (DESIGN 3.3).
pub const ROUNDINGS: [&str; 5] = ["up", "down", "half_up", "half_down", "half_even"];

/// The names the generated code keeps for itself in every target (DESIGN 10.1): the helper module
/// and its types. A name of the module whose alias is one of them is E007.
pub const HELPERS: [&str; 6] = ["kaname", "KanameError", "Q", "Date", "Rat", "glue"];
