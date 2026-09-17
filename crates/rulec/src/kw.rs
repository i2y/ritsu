//! The language's fixed vocabulary. All of it lives here (§1.1).
//!
//! Keywords have exactly one canonical English spelling; no synonyms are provided. The lexer
//! cuts every word out as an identifier, and the parser and the type checker match those
//! against this table. To change a word, touch only this file. The README's keyword table and
//! the E009 reserved-word check are both drawn from this table.

// --- Words that can start a line (in the order of §1.2)
pub const RULE: &str = "rule";
pub const DESCRIPTION: &str = "description";
pub const IMPORT: &str = "import";
pub const ENUM: &str = "enum";
pub const GROUP: &str = "group";
pub const INPUTS: &str = "inputs";
pub const OUTPUTS: &str = "outputs";
pub const DERIVE: &str = "derive";
pub const DEFINE: &str = "define";
pub const TABLE: &str = "table";
pub const POLICY: &str = "policy";
pub const RESULT: &str = "result";
pub const EXAMPLES: &str = "examples";
/// A relation between two inputs that always holds. It narrows the input space the checks
/// walk, so a combination the business cannot produce is not demanded of the table (§15.55).
pub const CONSTRAINT: &str = "constraint";
/// The fields of one element of a sequence the rule is given (§15.56). Declared like
/// `inputs`, because an element is a row of inputs.
pub const ELEMENTS: &str = "elements";
/// A named list of elements, written once and used by name in `examples` (§15.56).
pub const SEQUENCE: &str = "sequence";
/// Declares how the verdicts of a per-element table reduce to one answer (§15.56).
pub const FOLD: &str = "fold";

/// The line-head keywords. `rule` is header-only, so it is not included.
/// A test checks that the README's keyword table matches this list.
pub const LINE_HEAD: &[&str] = &[
    DESCRIPTION, IMPORT, ENUM, GROUP, INPUTS, ELEMENTS, OUTPUTS, DERIVE, DEFINE, CONSTRAINT, TABLE,
    FOLD, SEQUENCE, RESULT, EXAMPLES, POLICY,
];

// --- Declaration modifiers
pub const RANGE: &str = "range";
pub const ROUND: &str = "round";
/// Marks an input that serves only as a range check at the entry (silences W111).
pub const CONTRACT_ONLY: &str = "contract_only";
/// Marks an enum value: declares that it intentionally has no row of its own and falls
/// through to the default row.
pub const DEFAULT: &str = "default";
/// The step inside a type's brackets (`rate[step 1%]`).
pub const STEP: &str = "step";

// --- The arms of a `fold` (§15.56). What one verdict does to the walk.
/// Leave this element and look at the next.
pub const NEXT: &str = "next";
/// End the walk here.
pub const STOP: &str = "stop";
/// `stop with <value>` — end the walk with this answer.
pub const WITH: &str = "with";
/// Take this element's value. A second element that also takes is an error.
pub const TAKE_UNIQUE: &str = "take_unique";
/// Take the first element's value and ignore any later one.
pub const TAKE_FIRST: &str = "take_first";
/// `keep_max <value> by <key>` — hold this element's value, replacing what is held when
/// the key is larger.
pub const KEEP_MAX: &str = "keep_max";
/// The key of `keep_max`.
pub const BY: &str = "by";
/// The answer when the sequence has no elements. Declaring it is not optional.
pub const EMPTY: &str = "empty";
/// The answer when the walk reached the end without stopping. Declaring it is not optional.
pub const EXHAUSTED: &str = "exhausted";
/// Inside `exhausted`, the value `keep_max` or a `take` is holding.
pub const HELD: &str = "held";
/// `fold <column> over <sequence>`.
pub const OVER: &str = "over";

// --- Policies (§4)
pub const UNIQUE: &str = "unique";
pub const FIRST: &str = "first";

// --- Words that can appear in cells and expressions (§3)
pub const NOT: &str = "not";
pub const NONE: &str = "none";
pub const TRUE: &str = "true";
pub const FALSE: &str = "false";

// --- Types (§2.1)
pub const MONEY: &str = "money";
pub const MASS: &str = "mass";
pub const LENGTH: &str = "length";
pub const RATE: &str = "rate";
/// A whole number with no unit: a count of things, a number of days, a score (§2.1).
pub const NUMBER: &str = "number";
pub const BOOL: &str = "bool";
pub const STRING: &str = "string";
pub const DATE: &str = "date";
pub const INCL_TAX: &str = "incl_tax";
pub const EXCL_TAX: &str = "excl_tax";

// --- Rounding (§7.3). The directions use the same words as Java's RoundingMode, and are
// pinned down for negative values as well.
pub const UP: &str = "up";
pub const DOWN: &str = "down";
pub const HALF_UP: &str = "half_up";
pub const HALF_EVEN: &str = "half_even";
pub const HALF_DOWN: &str = "half_down";

// --- Functions allowed in the result expression
pub const MIN: &str = "min";
pub const MAX: &str = "max";

// --- The built-in namespace (`import std/都道府県`)
pub const STD: &str = "std";

/// Words that cannot be used as names (E009). Besides the line-head words, this includes the
/// words that cells, modifiers and expressions tell apart by position alone. A declaration
/// with the same name is silently misread by the line-oriented syntax.
pub const RESERVED: &[&str] = &[
    RULE, DESCRIPTION, IMPORT, ENUM, GROUP, INPUTS, ELEMENTS, OUTPUTS, DERIVE, DEFINE, CONSTRAINT,
    TABLE, FOLD, SEQUENCE, POLICY, RESULT, EXAMPLES, RANGE, ROUND, CONTRACT_ONLY, DEFAULT, NOT, NONE, TRUE,
    FALSE, MIN, MAX, UP, DOWN, HALF_UP, HALF_EVEN, HALF_DOWN, NEXT, STOP, WITH, TAKE_UNIQUE,
    TAKE_FIRST, KEEP_MAX, BY, EMPTY, EXHAUSTED, HELD, OVER,
];

/// Lists the words that may follow `policy`, for use in diagnostic text.
pub fn policies() -> String {
    tr!("{UNIQUE} と {FIRST}", "{UNIQUE} or {FIRST}")
}

/// Lists the line-head words, for use in diagnostic text.
pub fn line_heads() -> String {
    LINE_HEAD.join(" / ")
}
