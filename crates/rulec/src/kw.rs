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
/// The line after `policy`: the definitions this table's rows take precedence over. The
/// exception is written after what it excepts, so the targets are always declared above.
pub const OVERRIDES: &str = "overrides";
/// A definition written as prose (§15.67): a condition, a value, and what it takes
/// precedence over. It is one row of a one-row table, with the columns it does not name left
/// as `-`.
pub const CLAUSE: &str = "clause";
/// The condition of a clause: `<column> <cell> and …`, or `always`.
pub const WHEN: &str = "when";
/// The value a clause gives its output: a literal or a name, as in an output cell.
pub const THEN: &str = "then";
/// The condition of a clause that applies to every input.
pub const ALWAYS: &str = "always";
/// A document a rule transcribes (§15.68): `source 法 = law "342AC0000000023" asof
/// 2026-04-01`, with the pinned digest of every fragment cited, or `source 郵便 = file "…"
/// sha256:…` for a document that has no fragments.
pub const SOURCE: &str = "source";
/// The two kinds of source, told apart by position after `=`; neither is a reserved name.
pub const LAW: &str = "law";
pub const FILE: &str = "file";
/// The date a law is read as of (the e-Gov `asof` parameter).
pub const ASOF: &str = "asof";
pub const URL: &str = "url";
/// A rule applied with its inputs bound — a provision applied mutatis mutandis (§15.69). The callee is expanded into this rule for the checks and the generators.
pub const APPLY: &str = "apply";

/// `shape order(order) = jsonschema "order.json" "#/$defs/Order"` — the shape of the object
/// the caller already has, borrowed from the contract it is already described by (§15.125).
pub const SHAPE: &str = "shape";
/// `dest(dest) : prefecture  from order.shipping.prefecture` — where an input is projected
/// from, so the glue between the caller's object and the rule's flat inputs is generated and
/// held to the contract rather than written by hand (§15.125).
pub const FROM: &str = "from";
/// `from any order.lines where category = chilled` and its companion.
pub const ANY: &str = "any";
pub const ALL: &str = "all";
/// Inside an `apply`: the callee definitions left out (`第20条（第2項を除く。）`).
pub const EXCEPT: &str = "except";
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
    FOLD, COUNT, SUM, SEQUENCE, RESULT, EXAMPLES, POLICY, OVERRIDES, CLAUSE, SOURCE, APPLY,
    SHAPE,
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
/// `fold <column> over <sequence>`, and `count <name> over <sequence>`.
pub const OVER: &str = "over";

// --- Counting the walk (§15.58)
/// `count 一致数(hits) over 納入先 where 判定 = 一致` — how many elements satisfy a test.
pub const COUNT: &str = "count";
pub const SUM: &str = "sum";
pub const STARTS_WITH: &str = "starts_with";
pub const ALLOCATE: &str = "allocate";
pub const OF: &str = "of";
/// The test of a `count`: a column of one element, and the value it must take.
pub const WHERE: &str = "where";

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
/// §15.83. Area and volume are dimensions of their own, not products of `length`: §2.1 does
/// no dimensional analysis, so `縦 × 横` is a modeling error (E103) and an area is either
/// taken as an input or looked up in a table.
pub const AREA: &str = "area";
pub const VOLUME: &str = "volume";
/// A span of time. `date` is a calendar day and has no arithmetic; this is the quantity a
/// rule compares — 3 hours of delay, 45 hours of overtime in a month, 30 minutes of parking.
pub const DURATION: &str = "duration";
/// §15.84. Two dimensions that are **ordered but not arithmetic**. A ℃ has a displaced zero,
/// so `気温 × 2` means nothing; a decibel is a logarithm, so adding two of them multiplies
/// what they measure. Rules only ever compare them against thresholds, and that is all they
/// are allowed to do here (E048).
pub const TEMPERATURE: &str = "temperature";
pub const SOUND: &str = "sound";
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
/// The other kind of import names the file an enum's value set is declared in (§15.59,
/// §15.60): `import proto "<file>" <Enum> -> <enum>` and `import jsonschema "<file>"
/// "<pointer>" -> <enum>`. Both are told apart by position — the word right after `import` —
/// so neither is a reserved name.
pub const PROTO: &str = "proto";
/// JSON Schema, and the schemas inside an OpenAPI document, which are the same thing.
pub const JSONSCHEMA: &str = "jsonschema";

/// Words that cannot be used as names (E009). Besides the line-head words, this includes the
/// words that cells, modifiers and expressions tell apart by position alone. A declaration
/// with the same name is silently misread by the line-oriented syntax.
pub const RESERVED: &[&str] = &[
    RULE, DESCRIPTION, IMPORT, ENUM, GROUP, INPUTS, ELEMENTS, OUTPUTS, DERIVE, DEFINE, CONSTRAINT,
    TABLE, FOLD, COUNT, SUM, OF, STARTS_WITH, WHERE, SEQUENCE, POLICY, OVERRIDES, CLAUSE, WHEN, THEN, ALWAYS, SOURCE, APPLY,
    EXCEPT, RESULT,
    EXAMPLES, RANGE, ROUND, CONTRACT_ONLY, DEFAULT, NOT, NONE, TRUE, FALSE, MIN, MAX, UP, DOWN,
    HALF_UP, HALF_EVEN, HALF_DOWN, ALLOCATE, NEXT, STOP, WITH, TAKE_UNIQUE, TAKE_FIRST, KEEP_MAX, BY, EMPTY,
    EXHAUSTED, HELD, OVER,
];

/// Lists the words that may follow `policy`, for use in diagnostic text.
pub fn policies() -> String {
    tr!("{UNIQUE} と {FIRST}", "{UNIQUE} or {FIRST}")
}

/// Lists the line-head words, for use in diagnostic text.
pub fn line_heads() -> String {
    LINE_HEAD.join(" / ")
}
