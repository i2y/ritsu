//! The port of flows (DESIGN 7.4, X2): the calls a workflow makes to rules, each with what
//! dandori knows of the values it gives. dandori gathers the range of every value from every place
//! it is put (dandori's DESIGN 1.3), and holds the rule's ranges to them itself (its E014); what it
//! does not check is a rule's precondition across two inputs, which ritsu-cross asks rulec about
//! with these ranges.

use crate::{Rules, Said};
use ritsu_base::text::Text;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// One call of a rule in a flow.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleCall {
    /// The line of the call, from 1.
    pub line: usize,
    /// The rule's file, as dandori reaches it (what `Rules` is asked with).
    pub rule: PathBuf,
    /// The name the flow uses the rule by (`use rule <name> from …`).
    pub name: String,
    /// Each input the call gives, in the order written.
    pub args: Vec<CallArg>,
}

/// One value a call gives a rule.
#[derive(Clone, Debug, PartialEq)]
pub struct CallArg {
    /// The rule's input.
    pub input: String,
    /// The value as the flow writes it (`order.total`, `3`).
    pub shown: String,
    /// The numbers it can be, on the wire (both ends included; None at an open end), when every
    /// place it comes from has a range; None when one does not.
    pub range: Option<(Option<i128>, Option<i128>)>,
    /// A place it can come from that has no range, in both languages, when there is one.
    pub unknown: Option<Text>,
}

/// What dandori answers for a flow.
pub trait Flows {
    /// The calls the flow at `file` makes to rules, read with the rules through `rules`, when the
    /// flow passes dandori's check; else what the check says.
    fn rule_calls(&self, file: &Path, rules: Rc<dyn Rules>) -> Result<Vec<RuleCall>, Vec<Said>>;
}
