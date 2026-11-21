//! The words of the language, all of them, in one table (DESIGN 3.2).
//!
//! Every keyword has one English spelling and no synonym. The lexer cuts every word out as a word;
//! the parser matches words against the constants here by where they stand, so `person` is a type
//! after a field's `:`, a way to choose an approver after `who`'s `=`, and may be a field's name. A
//! word the other languages of ritsu use is used here only in the same meaning (DESIGN 3.6): a stage
//! is a `stage` (rulec and kaname write `step` in a rate's type), a stage is passed up with
//! `escalate`, a flow is started with `launch`, an approval is taken back with `retract approval`, a
//! file is an `attachment`, and the texts in another language are `labels`.
//!
//! A name cannot be one of the seven [`RESERVED`] words (E002): the words a condition and a cell are
//! made of, where a name spelled the same would let a line be read two ways (`when x is none`, `|
//! dept_head and accounting |`). A field cannot be named `requester` or `ticket` either, which an
//! expression reads (`requester.id`, `ticket.round`; DESIGN 3.4).

// The first word of a file, which says its kind (DESIGN 3.1).
pub const REQUEST: &str = "request";
pub const ROUTE: &str = "route";
pub const ORGANIZATION: &str = "organization";
pub const MASTERS: &str = "masters";

// Words that start a line at the left margin.
pub const DESCRIPTION: &str = "description";
pub const OWNER: &str = "owner";
pub const EFFECTIVE: &str = "effective";
pub const USE: &str = "use";
pub const FORM: &str = "form";
pub const INPUTS: &str = "inputs";
pub const WHO: &str = "who";
pub const EVERY: &str = "every";
pub const STAGE: &str = "stage";
pub const TABLE: &str = "table";
pub const TICKET: &str = "ticket";
pub const CLAIMS: &str = "claims";
pub const EXAMPLES: &str = "examples";
pub const SCENARIO: &str = "scenario";
pub const LABELS: &str = "labels";
pub const POSITIONS: &str = "positions";
pub const TEAM: &str = "team";
pub const KINDS: &str = "kinds";
pub const DATA: &str = "data";
pub const MASTER: &str = "master";
pub const KEY: &str = "key";
pub const FIELDS: &str = "fields";

/// What `step` is in rulec and kaname, the step of a rate's type (`rate[step 1%]`): written where
/// ringi takes `stage`, it is E001, which says to write `stage` (DESIGN 3.6).
pub const STEP: &str = "step";

// After `use`.
pub const CALENDAR: &str = "calendar";
pub const BOOK: &str = "book";
pub const FLOW: &str = "flow";
pub const RULE: &str = "rule";
pub const DATES: &str = "dates";
pub const GATE: &str = "gate";
pub const FROM: &str = "from";

// The types of a field (DESIGN 4.1), and rulec's words in them.
pub const TEXT: &str = "text";
pub const LONG: &str = "long";
pub const NUMBER: &str = "number";
pub const MONEY: &str = "money";
pub const RATE: &str = "rate";
pub const QUANTITIES: [&str; 7] = ["mass", "length", "area", "volume", "duration", "temperature", "sound"];
pub const DATE: &str = "date";
pub const CHOICE: &str = "choice";
pub const ATTACHMENT: &str = "attachment";
pub const PERSON: &str = "person";
pub const BOOL: &str = "bool";
pub const ROWS: &str = "rows";

// The marks of a field.
pub const RANGE: &str = "range";
pub const COUNT: &str = "count";
pub const OPTIONAL: &str = "optional";
pub const REQUIRED: &str = "required";
pub const SHOWN: &str = "shown";
pub const WHEN: &str = "when";
pub const APPROVERS: &str = "approvers";
pub const ONLY: &str = "only";
pub const OPEN: &str = "open";
pub const IN: &str = "in";
pub const ROUND: &str = "round";
/// rulec's five ways of rounding (`round down(1JPY)`).
pub const ROUNDINGS: [&str; 5] = ["up", "down", "half_up", "half_down", "half_even"];

// A route and its stages (DESIGN 5.1).
pub const AT: &str = "at";
pub const LEAST: &str = "least";
pub const OF: &str = "of";
pub const ALL: &str = "all";
pub const CIRCULATE: &str = "circulate";
pub const TO: &str = "to";
pub const NOTE: &str = "note";
pub const AND: &str = "and";

// The ways to choose an approver (DESIGN 5.2).
pub const MANAGER: &str = "manager";
pub const HEAD: &str = "head";
pub const NEAREST: &str = "nearest";
pub const ABOVE: &str = "above";
pub const REQUESTER: &str = "requester";
pub const CHOSEN: &str = "chosen";
pub const BY: &str = "by";
pub const PREVIOUS: &str = "previous";
pub const APPROVER: &str = "approver";
pub const LOOKUP: &str = "lookup";
pub const AGENT: &str = "agent";
pub const ELSE: &str = "else";

// Deadlines (DESIGN 5.10).
pub const DUE: &str = "due";
pub const REMIND: &str = "remind";
pub const AFTER: &str = "after";
pub const BUSINESS: &str = "business";
pub const DAY: &str = "day";
pub const DAYS: &str = "days";
pub const END: &str = "end";
pub const LATE: &str = "late";
pub const EMPTY: &str = "empty";
pub const ESCALATE: &str = "escalate";
pub const THEN: &str = "then";
pub const EXPIRE: &str = "expire";
pub const STAY: &str = "stay";
pub const SKIP: &str = "skip";

// The rules of a ticket (DESIGN 5.5-5.7, 7).
pub const SELF: &str = "self";
pub const APPROVAL: &str = "approval";
pub const ALLOWED: &str = "allowed";
pub const NOT: &str = "not";
pub const SAME: &str = "same";
pub const APPROVES: &str = "approves";
/// `same approver approves once`: one approval for two stages, as rulec's `once` is at most one
/// call (DESIGN 3.6).
pub const ONCE: &str = "once";
/// `same approver approves once, at the last stage`: the last of the stages one person answers, as
/// kaname's `xs.last()` is the last of a list (DESIGN 3.6).
pub const LAST: &str = "last";
// `submitted by everyone except team board` (DESIGN 5.13): `except` leaves people out, as rulec's and
// sakai's `except` leaves values out of a set (DESIGN 3.6).
pub const SUBMITTED: &str = "submitted";
pub const EVERYONE: &str = "everyone";
pub const POSITION: &str = "position";
pub const HIGHER: &str = "higher";
pub const EXCEPT: &str = "except";
pub const SEND: &str = "send";
pub const BACK: &str = "back";
pub const THE: &str = "the";
pub const ANY: &str = "any";
pub const EARLIER: &str = "earlier";
pub const MOST: &str = "most";
pub const TIMES: &str = "times";
pub const RESUBMIT: &str = "resubmit";
pub const WITHIN: &str = "within";
pub const WITHDRAW: &str = "withdraw";
pub const UNTIL: &str = "until";
pub const IT: &str = "it";
pub const IS: &str = "is";
pub const DECIDED: &str = "decided";
pub const RETRACT: &str = "retract";
pub const NEXT: &str = "next";
pub const ANSWERS: &str = "answers";
pub const PAUSE: &str = "pause";
pub const EDIT: &str = "edit";
pub const ON: &str = "on";
pub const HOLD: &str = "hold";
pub const POST: &str = "post";
pub const VOID: &str = "void";
pub const LAUNCH: &str = "launch";
/// `comment required for reject, send back` (DESIGN 7.2): the words that start the line, and the
/// operations it can name, as a scenario's `event` writes them. Its words are here alone: the
/// parser reads them from here, and E006, E409 and the table below say them from here.
pub const COMMENT_REQUIRED: &str = "comment required for";
pub const REASONED: [&str; 2] = ["reject", "send back"];
/// The ways a ticket can be taken somewhere, after `on` (DESIGN 3.3): `sent back` is two words.
pub const ENDINGS: [&str; 6] = ["submitted", "approved", "sent back", "rejected", "withdrawn", "expired"];

// Claims (DESIGN 6.4).
pub const POSSIBLE: &str = "possible";
pub const REMINDS: &str = "reminds";
pub const BEFORE: &str = "before";
pub const ENDS: &str = "ends";

// A scenario (DESIGN 10.2).
pub const GIVEN: &str = "given";
/// The columns of a scenario's table, in their order: the outputs start at `stage`. `comment` is
/// written where a row writes what a person wrote, and left out where none does.
pub const SCENARIO_COLUMNS: [&str; 6] = ["at", "event", "by", COMMENT, "stage", "status"];
/// A person's words on a ticket: an event of its own, and a column of a scenario (DESIGN 7.2, 10.2).
pub const COMMENT: &str = "comment";
/// The events a scenario's row can write (DESIGN 7.2).
pub const EVENTS: [&str; 21] = [
    "submit",
    "resubmit",
    "approve",
    "reject",
    "send back",
    "retract",
    "pause",
    "resume",
    "withdraw",
    "discard",
    "edit",
    "add approver",
    "hand over",
    COMMENT,
    "remind",
    "late",
    "resubmit late",
    "pause ends",
    "agent answered",
    "note ready",
    "flow failed",
];
/// The states of a ticket (DESIGN 7.1).
pub const STATES: [&str; 7] = ["in progress", "paused", "with requester", "approved", "rejected", "withdrawn", "expired"];

// Expressions (DESIGN 3.3, 4.5): rulec's spelling, and `is` to compare.
pub const OR: &str = "or";
pub const NONE: &str = "none";
pub const TRUE: &str = "true";
pub const FALSE: &str = "false";
pub const SUM: &str = "sum";

/// The languages a `labels` block can give its texts in.
pub const LANGS: [&str; 2] = ["ja", "en"];

/// The words, by where they are written, as DESIGN 3.2 lists them.
pub const WORDS: &[(&str, &[&str])] = &[
    ("line", &[REQUEST, ROUTE, ORGANIZATION, MASTERS, DESCRIPTION, OWNER, "effective from", USE, FORM, WHO, "every stage", TABLE, TICKET, CLAIMS, EXAMPLES, SCENARIO, LABELS, INPUTS, POSITIONS, "team kinds", DATA, MASTER, KEY, FIELDS]),
    ("use", &[ORGANIZATION, MASTERS, CALENDAR, BOOK, FLOW, RULE, DATES, ROUTE, GATE, FROM]),
    ("type", &[TEXT, "long text", NUMBER, MONEY, RATE, DATE, CHOICE, "choice from", ATTACHMENT, PERSON, TEAM, BOOL, ROWS]),
    ("mark", &[RANGE, COUNT, OPTIONAL, "required when", "shown when", "approvers only", "open in", ROUND]),
    ("stage", &["at least", OF, "%", "all of", "circulate to", "note from", WHEN, AND]),
    ("who", &[PERSON, TEAM, "manager of", "head of", "nearest … above", REQUESTER, "chosen by", "previous approver", IN, "lookup … by", AGENT, ELSE]),
    ("deadline", &["due in", "remind after", "business day(s)", "at end of day", AT, "when late", "when empty", ESCALATE, BY, THEN, EXPIRE, STAY, SKIP]),
    (
        "ticket",
        &[
            "self approval",
            ALLOWED,
            "not allowed",
            "same approver approves once",
            "at the last stage",
            "submitted by",
            EVERYONE,
            "everyone in",
            TEAM,
            POSITION,
            "or higher",
            IN,
            PERSON,
            EXCEPT,
            "send back",
            "to the requester",
            "to any earlier stage",
            "at most",
            TIMES,
            "resubmit within",
            "withdraw until",
            "it is decided",
            "retract approval until the next stage answers",
            "pause at most",
            "approvers edit",
            COMMENT_REQUIRED,
            REASONED[0],
            ON,
            HOLD,
            POST,
            VOID,
            LAUNCH,
        ],
    ),
    ("claim", &["only when", POSSIBLE, IS, "every stage reminds before it is due", "every ticket ends within"]),
    ("scenario", &[REQUESTER, GIVEN, FORM, "at", "event", "by", COMMENT, "-> stage", "status"]),
    ("expression", &[IS, "is not", NONE, AND, OR, NOT, TRUE, FALSE, "sum(…)", "+", "-", "*", "/", "business days in"]),
];

/// The words a name cannot be (E002): the ones a condition and a cell are made of.
pub const RESERVED: [&str; 7] = [AND, OR, NOT, IS, NONE, TRUE, FALSE];

pub fn is_reserved(w: &str) -> bool {
    RESERVED.contains(&w)
}

/// The names a field cannot take (E002): what an expression reads besides the fields (DESIGN 3.4).
pub const NOT_FIELDS: [&str; 2] = [REQUESTER, TICKET];
