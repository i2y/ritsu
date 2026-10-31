//! The ports of programs (kaname's DESIGN 14): what a module of kaname calls in rulec, koyomi and
//! chobo, each call with what kaname knows of the values it gives ([`Programs`], which kaname
//! answers), and what the checks across the borders came to at each of those places ([`Verdicts`],
//! which ritsu-cross answers and kaname reads).
//!
//! kaname holds the ranges of a rule's numeric inputs and of a koyomi date's whole-number inputs to
//! the values it gives them itself, by its analysis of ranges (its E301 and W301), as dandori does
//! (its E014). What only the other language can say is ritsu-cross's to decide, with the same
//! decisions it makes for a workflow (ritsu's DESIGN 7): a rule's preconditions (X2), the days given
//! to a rule's date input (X3 (a)), an amount of a transfer and the refusals that amount can meet
//! (X4), and the day given to a koyomi date (X6). kaname's values are an estimate from above, not a
//! range each value takes whatever the others are, so an example that breaks a check is one of the
//! function's own runs ([`Seen`]), never a corner of the ranges (kaname's DESIGN 6.4).
//!
//! The binary of kaname's own crate reads no other language, and is handed no verdicts: a place a
//! check across the borders decides waits for its answer there (kaname's DESIGN 4.3).

use crate::{Answer, Origin, Ports, Said, Values};
use ritsu_base::text::Text;
use std::path::{Path, PathBuf};

/// Where a module gives a value across a border: the line and the column of the expression, both
/// from 1, as kaname's diagnostics say them. A verdict names its place by it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct At {
    pub line: usize,
    pub col: usize,
}

/// The two ends of a range on the wire, both in; None at an end there is none for.
pub type Ends = (Option<i128>, Option<i128>);

/// One value a module gives a rule, a koyomi date or a transfer, with what kaname knows of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Given {
    pub at: At,
    /// What takes it: the rule's input (or the list the rule walks), the date's input, the
    /// transfer's parameter, by the name the other language gives it.
    pub to: String,
    /// The value as the module writes it: `invoice.amount`, `paid_on - due`.
    pub shown: String,
    /// What kaname's analysis of ranges finds the value can be (kaname's DESIGN 5), on the wire,
    /// both ends in, None at an end it finds none for: a number counted in the unit of what takes
    /// it (a rate as a count of its step), a date as its day number, a list by its count.
    pub range: Ends,
    /// Every kind of place the value can come from, as an `if` or a `match` joins them: the day of
    /// a koyomi date carried unchanged (`Origin::Day`), a numeric output of a rule carried unchanged
    /// (`Origin::Output`), and `Origin::Range`, with the range above, where kaname computes it or the
    /// host gives it. A value carried unchanged may be narrowed by a condition on the way, which
    /// `range` says.
    pub from: Vec<Origin>,
    /// For a list given to a list the rule walks: each numeric field of its elements, by the rule's
    /// name of it, with its range on the wire.
    pub fields: Vec<(String, Ends)>,
}

/// One call of a rule in a module.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgramRuleCall {
    pub at: At,
    /// The function the call is in.
    pub function: String,
    /// The rule's file, as kaname reaches it (what `Rules` is asked with), and the rule as the
    /// module writes it (the name its `use rule` gives it).
    pub rule: PathBuf,
    pub name: String,
    /// Each input the call gives, and each list it gives a list the rule walks, in the order written.
    pub args: Vec<Given>,
    /// The relations kaname knows hold at the call between two of the inputs it gives (kaname's
    /// DESIGN 5.3): `(input, op, input)`, `op` one of `<`, `<=` and `=`.
    pub relations: Vec<(String, String, String)>,
    /// The most elements a list given to a list the rule walks can have (the largest, where the
    /// call gives more than one), when kaname knows it.
    pub max_len: Option<i128>,
}

/// One call of a koyomi date in a module, with the day given to the date's date input. The date's
/// whole-number inputs are kaname's own to hold to their ranges, as a rule's numeric inputs are.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgramDateCall {
    pub at: At,
    /// The function the call is in.
    pub function: String,
    /// The dates file, as kaname reaches it (what `Dates` is asked with).
    pub file: PathBuf,
    /// The date as the module writes it (`terms.due`), and its name in the dates file.
    pub name: String,
    pub date: String,
    /// The day given to the date's date input.
    pub day: Given,
}

/// One transfer a module builds: an operation of a transfer of a book, with its amounts. kaname
/// writes nothing to the book: the host applies the transfer (kaname's DESIGN 6.3).
#[derive(Clone, Debug, PartialEq)]
pub struct ProgramTransfer {
    pub at: At,
    /// The function it is built in.
    pub function: String,
    /// The book, as kaname reaches it (what `Books` is asked with), the transfer, and the
    /// operation (`do`, `hold`, `post`, `void`).
    pub book: PathBuf,
    pub transfer: String,
    pub op: String,
    /// The operation as the module writes it: `ledger.capture.do`.
    pub name: String,
    /// Each amount it is given, in the order written.
    pub amounts: Vec<Given>,
}

/// Every call a module makes into the other languages.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgramCrossings {
    pub rules: Vec<ProgramRuleCall>,
    pub dates: Vec<ProgramDateCall>,
    pub transfers: Vec<ProgramTransfer>,
}

/// What one run of a function's vectors (kaname's DESIGN 9) gave one of the places where it
/// crosses a border: an input of the function, and what it takes the place to.
#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    pub at: At,
    /// The call of the function, as the module would write it: `settle(payment: Payment { … })`.
    pub call: String,
    /// The function's input, as `ritsu kaname run --input` takes it: a JSON object.
    pub input: String,
    /// What the place was given, on the wire, by what takes it: every input of a rule (where a
    /// precondition is about the call), the day given to a date input, an amount.
    pub given: Values,
}

/// What kaname answers for a module.
pub trait Programs {
    /// Every call the module at `file` makes into rulec, koyomi and chobo, read with the other
    /// languages through `ports`, when the module's names and types pass kaname's check (its
    /// analysis of ranges reads such a module, whatever else its check finds); else what the check
    /// says.
    fn crossings(&self, file: &Path, ports: &Ports) -> Result<ProgramCrossings, Vec<Said>>;

    /// What the vectors of the function named `function` give each of its places across the
    /// borders, a run at a time: asked where kaname's ranges do not show a check, since only then
    /// are the vectors run (kaname's DESIGN 5.7). None for a module whose names and types do not
    /// pass, or a function it does not have.
    fn seen(&self, file: &Path, ports: &Ports, function: &str) -> Vec<Seen>;

    /// Whether kaname is joined at all.
    fn joined(&self) -> bool {
        true
    }
}

/// The check across a border a verdict is of (ritsu's DESIGN 7.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Crossing {
    /// A precondition of a rule, at a call of it (X2).
    Precondition,
    /// The day given to a date input of a rule that declares a range (X3 (a)).
    RuleDay,
    /// An amount given to a transfer (X4).
    Amount,
    /// The refusals an operation of a transfer can meet with the amounts it is given (X4).
    Refusals,
    /// The day given to the date input of a koyomi date (X6).
    DateDay,
}

impl Crossing {
    /// The check's word, as `ritsu check --format json` names it in the record of a border
    /// (ritsu's DESIGN 7.15): `precondition`, `rule_date_input`, `transfer_amount`,
    /// `transfer_refusals`, `koyomi_date_input`. The marks of ritsu's DESIGN 7.2 (X2 to X6) are no
    /// words of an output (DESIGN 4.3).
    pub fn check(self) -> &'static str {
        match self {
            Crossing::Precondition => "precondition",
            Crossing::RuleDay => "rule_date_input",
            Crossing::Amount => "transfer_amount",
            Crossing::Refusals => "transfer_refusals",
            Crossing::DateDay => "koyomi_date_input",
        }
    }
}

/// What a check across a border came to at one place of a module.
#[derive(Clone, Debug, PartialEq)]
pub struct CallVerdict {
    pub at: At,
    pub check: Crossing,
    /// What at the place it is about: the precondition as rulec writes it (`before <= upto`), the
    /// input of the rule or of the date, the transfer's parameter, the operation (`capture.do`).
    pub about: String,
    /// What it came to (ritsu's P5): shown to hold; broken by what a run of the function's vectors
    /// gave the place; or undecided, and why.
    pub outcome: Answer<Seen>,
    /// The code of ritsu's ledger it is said with (`E203`, `W203`), and the headline, where it does
    /// not hold.
    pub said: Option<(String, Text)>,
    /// For the refusals: the reasons the operation can be refused with at the amounts it is given,
    /// as chobo's search finds them; None where it cannot be decided (the book's every reason of
    /// its bounds can come, then).
    pub refusals: Option<Vec<String>>,
}

/// What the checks across the borders came to at a module's calls, for kaname (its DESIGN 14.5):
/// ritsu-cross answers it, and kaname decides the effects of its functions with it, so that a
/// function shown total is one whose every call holds. `ritsu kaname` and `ritsu check` hand it over;
/// the binary of kaname's own crate reads no other language and is handed none.
pub trait Verdicts {
    /// Every verdict at the calls of the module at `file` (as kaname was given it), in the order of
    /// the calls; none for a module whose names and types do not pass kaname's check.
    fn verdicts(&self, file: &Path) -> Vec<CallVerdict>;
}
