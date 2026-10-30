//! The language's fixed vocabulary. All of it lives here (§1.1).
//!
//! Keywords have exactly one canonical English spelling; no synonyms are provided. The lexer
//! cuts every word out as an identifier, and the parser and the type checker match those
//! against this table. To change a word, touch only this file. The README's keyword table and
//! the E009 reserved-word check are both drawn from this table.

// --- Words that can start a line (in the order of §1.2)
pub const RULE: &str = "rule";
/// The heading of a file that holds enums and groups for rules to read, and no function
/// (§15.212): `enums content v1`. It is read on the first line only, where no name stands, so
/// it is not reserved (compatibility.md: the reserved words are fixed).
pub const ENUMS: &str = "enums";
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
/// The condition of a clause that applies to every input. Under `machine`, the line that says
/// what holds of the carried values in every state a case can reach: `always 残り日数 >=1
/// when 猶予中` (§15.217), where `when` names the states it speaks of.
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
/// `implements proto "api/shop/v1/member_shipping.proto" MemberShippingService` — the Connect
/// service the rule is served as is the one a contract declares, with the contract's names and
/// numbers (§15.274). A line head, read where a line starts outside every block; not reserved, so
/// `implements : bool` under `inputs` is an input of that name.
pub const IMPLEMENTS: &str = "implements";
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
/// `machine 注文(order) over 遷移` — the rule is one step of a state machine: one output comes
/// back as one input on the next call, and the claims under it are about every sequence of
/// calls (§15.148). The generated code stays a pure function; the host keeps the state.
pub const MACHINE: &str = "machine";
/// A named sequence of steps from the machine's initial state, with what each step answers:
/// the `examples` of a machine (§15.148).
pub const SCENARIO: &str = "scenario";
/// `prerequisites research over tech` — which values of an enum a case has to hold before it can
/// take another, with the lines under it (§15.222). A block head where a line starts outside every
/// block; a declaration named `prerequisites` under `inputs` is still that declaration, so the word
/// is not reserved (§15.157).
pub const PREREQUISITES: &str = "prerequisites";
/// `chances` — a table of claims about how likely each answer is when the `random` inputs are
/// drawn uniformly (§15.224): the inputs that are not random, the outputs a claim is about, and
/// the chance in the last column. A block head where a line starts outside every block, read the
/// way `prerequisites` is, so the word is not reserved (§15.157).
pub const CHANCES: &str = "chances";
/// `open stackable : money[USD] combine sum range …` — a value other files may add terms to, with the
/// terms under it (§15.227). After the name on a `table` or `clause` line, and after the last value
/// of an `enum` line, the mark that opens the definition or the enum to contributions. Read where a
/// line starts outside every block and after those names only, so it is not reserved (§15.157).
pub const OPEN: &str = "open";
/// `combine sum|product|max|min` on an `open` line (§15.227). Read there only.
pub const COMBINE: &str = "combine";
/// The way of combining that multiplies the terms (§15.227). Read after `combine` only.
pub const PRODUCT: &str = "product";
/// `contribution regional_campaign v1` — the heading of a file that adds to another rule's open
/// places (§15.227). Read on the first line only, where no name stands, as `enums` is.
pub const CONTRIBUTION: &str = "contribution";
/// `to "<file>" signature:<digest>` — the file a contribution adds to (§15.227). A line head in a
/// contribution file only.
pub const TO: &str = "to";
/// `into stackable` — the open value the terms under it are added to (§15.227). A line head in a
/// contribution file only.
pub const INTO: &str = "into";
/// `extend kind with garden "Garden supplies"` — a value added to an open enum (§15.228). A line
/// head in a contribution file only.
pub const EXTEND: &str = "extend";

/// The line-head keywords. `rule` is header-only, so it is not included.
/// A test checks that the README's keyword table matches this list.
pub const LINE_HEAD: &[&str] = &[
    DESCRIPTION, IMPORT, ENUM, GROUP, INPUTS, ELEMENTS, OUTPUTS, DERIVE, DEFINE, CONSTRAINT, TABLE,
    FOLD, COUNT, SUM, MAX, MIN, SEQUENCE, RESULT, EXAMPLES, POLICY, OVERRIDES, CLAUSE, SOURCE, APPLY,
    SHAPE, MACHINE, SCENARIO, PREREQUISITES, CHANCES, OPEN, IMPLEMENTS,
];

/// The line heads of a `contribution` file besides those of a rule it may write (§15.227): where it
/// adds, what it comes after, the terms it adds to an open value, the values it adds to an open
/// enum. None is reserved.
pub const CONTRIBUTION_HEAD: &[&str] = &[TO, AFTER, INTO, EXTEND];

// --- Chances (§15.224–§15.226). Each word is read by its place, so none is reserved.
/// The mark at the end of an input's line: the caller draws the value uniformly from the integers
/// of its declared range (an enum's values, a truth value's two), independently of the other
/// inputs, within what the `constraint` lines leave (§15.224). Read in a declaration's tail only,
/// where no name stands.
pub const RANDOM: &str = "random";
/// The heading of the last column of a `chances` table: the claimed chance.
pub const CHANCE: &str = "chance";
/// `take_weighted <value> by <weight>` — the fold arm that picks one element with a chance in
/// proportion to its weight (§15.226): the weights of the elements on such verdicts add up as the
/// walk goes, and the first element whose running total passes the drawn value is taken.
pub const TAKE_WEIGHTED: &str = "take_weighted";
/// `draw -> <random input>` under a fold with `take_weighted`: the drawn value the running totals
/// are compared with. Read as that line only when what follows `->` is one name; `draw -> next` is
/// the arm of a verdict called `draw`.
pub const DRAW: &str = "draw";

// --- The lines under `prerequisites` (§15.222). Each word is read by its place in the line, so
// none is reserved: a value may be called `start`, and `start requires …` is that value's line.
/// `start agriculture, …`: the values that need nothing before them, which every other value is
/// reached from.
pub const START: &str = "start";
/// `pottery requires agriculture`: the values a case has to hold before it can take this one.
/// `,` is "and"; `or` inside a group is "any one of".
pub const REQUIRES: &str = "requires";
/// `charge requires shock_2 or drill_2`: one of the values on either side is enough.
pub const OR: &str = "or";
/// `requires nothing "<reason>"`: a value no case can take, with the reason beside it.
pub const NOTHING: &str = "nothing";
/// `ordered by era_of`: a table whose answer for a value is never below its prerequisites'.
pub const ORDERED: &str = "ordered";

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
/// Inside `exhausted`, the value `keep_max`, a `take` or a `take_weighted` is holding. Under `machine`, the line
/// naming the inputs a case holds from its first call to its last: `held 支払額` (§15.149).
pub const HELD: &str = "held";
/// `fold <column> over <sequence>`, and `count <name> over <sequence>`.
pub const OVER: &str = "over";

// --- The lines under `machine` (§15.148)
/// `carry 状態 -> 次の状態` — the output that is the next call's input. The first line is the
/// state; further lines carry a number with a range, a boolean or another enum along with it
/// (§15.217).
pub const CARRY: &str = "carry";
/// The state a case starts in, and `initial 残り 0`: where a further carried value starts.
pub const INITIAL: &str = "initial";
/// The states a case ends in. Nothing leads out of one, and every state a case can reach has
/// a way to one.
pub const FINAL: &str = "final";
/// `never 出荷済 after 取消` — no sequence of calls reaches the first states after the second.
pub const NEVER: &str = "never";
pub const AFTER: &str = "after";
/// `once 返金額 >0円` — at most one call in a case answers an output the cell accepts.
pub const ONCE: &str = "once";

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
/// `mod(<number>, <positive whole constant>)`: the remainder, from 0 to the constant less one,
/// whatever the sign of the number (§15.214). Read as a function only where a call is written,
/// which no name can be, so it stays out of [`RESERVED`]: the reserved words are the ones 1.0
/// fixed (§15.157), and a rule may still call an input `mod`.
pub const MOD: &str = "mod";
/// `isqrt(<number>)`: the largest whole number whose square does not pass the number
/// (§15.215). A function only where a call is written, as `mod` is.
pub const ISQRT: &str = "isqrt";

// --- The built-in namespace (`import std/us/states`, `import std/都道府県`; the paths are the
// prelude's, §15.182)
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
    EXHAUSTED, HELD, OVER, MACHINE, SCENARIO, CARRY, INITIAL, FINAL, NEVER, AFTER, ONCE,
];

/// The reserved words an enum **value** still cannot be (E009), §15.150. A value never starts
/// a line — it sits in a cell, after `=` in its `enum` line, or in a line under `machine` or
/// `fold` — so the line-oriented reading that makes every other reserved word unusable as a
/// name does not reach it. These are the ones a value's own position reads as something else:
/// the words of a cell, the mark after a value in its `enum` line, the word that divides a
/// `never` line, the two arms a `fold` line starts with, and the words that divide an arm.
pub const VALUE_RESERVED: &[&str] = &[NOT, NONE, TRUE, FALSE, STARTS_WITH, DEFAULT, AFTER, EMPTY, EXHAUSTED, BY, WITH];

/// Lists the words that may follow `policy`, for use in diagnostic text.
pub fn policies() -> String {
    tr!("{UNIQUE} と {FIRST}", "{UNIQUE} or {FIRST}")
}

/// Lists the line-head words, for use in diagnostic text.
pub fn line_heads() -> String {
    LINE_HEAD.join(" / ")
}
