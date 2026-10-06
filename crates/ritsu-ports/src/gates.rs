//! The port of gates (sekisho's DESIGN 8.3, `Gates`): what sekisho knows of a `.gate` that passes
//! its check, or of a Cedar policy set with its schema, and how far someone who asks is allowed an
//! action over every combination sekisho's check walks. sekisho answers it; the checks across the
//! borders (ritsu-cross: an operation a context opens with no action to guard it, X15; what a
//! workflow calls against what it is allowed, X16) receive it.
//!
//! [`GatePorts`] is the other way round: the ports sekisho reads a `.gate` with, as `ritsu
//! sekisho` and `ritsu check` join them (ritsu-project's `Joined::sekisho`).

use crate::{Books, Dates, Flows, Found, Items, Ports, Rules, Said};
use ritsu_base::naming::Name;
use ritsu_base::text::Text;
use std::path::Path;
use std::rc::Rc;

/// What sekisho knows of one `.gate` that passes its check, or of a Cedar policy set with its
/// schema.
#[derive(Clone, Debug, PartialEq)]
pub struct GateFacts {
    /// The file's name and alias (`gate 返金(refunds_ja)`), its version and SHA-256 (in hex), and
    /// the Cedar namespace.
    pub name: String,
    pub alias: String,
    pub version: String,
    pub sha256: String,
    pub namespace: String,
    /// In the order written.
    pub actions: Vec<GateAction>,
    pub workflows: Vec<GateWorkflow>,
    /// Each policy: its name, its `@id`, permit or forbid, and its line.
    pub policies: Vec<GatePolicy>,
    /// Each expectation and separation, by name, with its line (from 1).
    pub expects: Vec<(String, usize)>,
    pub separations: Vec<(String, usize)>,
}

/// One action of a gate.
#[derive(Clone, Debug, PartialEq)]
pub struct GateAction {
    pub name: String,
    pub alias: String,
    /// The line it is declared on, from 1.
    pub line: usize,
    /// The operations the action guards, as references (`openapi "api/orders.json" operation
    /// refundOrder`, `proto "shop/v1/orders.proto" service Orders method Refund`), each with the
    /// line of its `guards`.
    pub guards: Vec<(Name, usize)>,
    /// The principal and resource types the action takes, by alias (`User`, `Workflow`).
    pub principals: Vec<String>,
    pub resources: Vec<String>,
    /// `nobody "<reason>"`: the action is meant to be allowed to no one.
    pub nobody: Option<String>,
}

/// One workflow a gate names as a principal.
#[derive(Clone, Debug, PartialEq)]
pub struct GateWorkflow {
    /// The name the gate gives it (`返品`, `returns`), and its alias, the Cedar id of `Workflow`.
    pub name: String,
    pub alias: String,
    /// The `.flow`, as a reference from the root (`dandori "flows/returns.flow"`).
    pub flow: Name,
    pub line: usize,
}

/// One policy of a gate.
#[derive(Clone, Debug, PartialEq)]
pub struct GatePolicy {
    pub name: String,
    /// Its `@id` (`refunds/clerks_refund_within_their_limit`).
    pub id: String,
    /// A permit; false for a forbid.
    pub permit: bool,
    pub line: usize,
}

/// Who asks, for [`Gates::allowed`].
#[derive(Clone, Debug, PartialEq)]
pub enum Asker {
    /// The workflow of this name (`workflow <name> from …`).
    Workflow(String),
    /// A principal of this type holding exactly these roles (and the roles they include).
    Roles { ty: String, roles: Vec<String> },
}

/// How far an asker is allowed an action, over every combination of the rest.
#[derive(Clone, Debug, PartialEq)]
pub enum Allowance {
    Always,
    /// Allowed in some combinations and denied in others: one of each, in both languages.
    Sometimes { allowed: Text, denied: Text },
    Never,
}

/// What sekisho answers for a gate. `file` is the `.gate`, or a policy set of Cedar with its schema
/// beside it (the `.cedar`, or its `.cedarschema` or `.cedarschema.json`), from `root`, the root
/// the references are written from (ritsu's DESIGN 6.2, item 3), as the port of what files hold is
/// asked (`Items`).
pub trait Gates {
    /// What sekisho knows of the file, when it passes check; else what check says.
    fn facts(&self, root: &Path, file: &str) -> Result<GateFacts, Vec<Said>>;

    /// Whether `asker` is allowed `action` (by its name or its alias), over every combination the
    /// check walks; undecided with the reason when the walk is over budget, a language cannot say
    /// what a value can come to, or a policy of Cedar written by hand reads what sekisho does not
    /// count; what check says when the file does not pass it.
    fn allowed(&self, root: &Path, file: &str, action: &str, asker: &Asker) -> Result<Found<Allowance>, Vec<Said>>;

    /// Whether sekisho is joined at all: false for a port that reads no gate, so that the language
    /// that asks can say so once, as with rules (`Rules::joined`).
    fn joined(&self) -> bool {
        true
    }
}

/// The ports sekisho reads a `.gate` with (sekisho's DESIGN 8.1), as `ritsu sekisho` and `ritsu
/// check` join them once: the rules and the dates files its computed values call (and the
/// calendars `today is open in` reads), the books whose transfers an action guards, and dandori
/// for the workflows a gate names — whether a `.flow` passes its check ([`Flows::crossings`], read
/// with [`GatePorts::ports`]) and what it holds ([`Items`]). The contracts an action guards
/// (OpenAPI and AsyncAPI documents, `.proto` files) need no port: every language reads them with
/// the same readers (`ritsu_base::openapi`, ritsu-proto).
///
/// The binary of sekisho's own crate holds ports that join nothing, each answering `joined()`
/// with false, as dandori's does (`NoRules`), and a `.gate` that reads another language is refused
/// there (E209).
#[derive(Clone)]
pub struct GatePorts {
    pub rules: Rc<dyn Rules>,
    pub dates: Rc<dyn Dates>,
    pub books: Rc<dyn Books>,
    pub flows: Rc<dyn Flows>,
    pub items: Rc<dyn Items>,
}

impl GatePorts {
    /// The rules, dates files and books, as dandori reads a flow with them (what
    /// [`Flows::crossings`] takes).
    pub fn ports(&self) -> Ports {
        Ports { rules: self.rules.clone(), dates: self.dates.clone(), books: self.books.clone() }
    }
}
