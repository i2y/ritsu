//! The port of gates (sekisho's DESIGN 8.3, `Gates`): what sekisho knows of a `.gate` that passes
//! its check, or of a Cedar policy set with its schema, and how far someone who asks is allowed an
//! action over every combination sekisho's check walks. sekisho answers it; the checks across the
//! borders (ritsu-cross: an operation a context opens with no action to guard it, X15; what a
//! workflow calls against what it is allowed, X16) receive it.
//!
//! X16 asks at each call of a flow ([`Gates::allowed_at`]): over the combinations the runs of the
//! flow that reach the call can come to, by what the call gives the operation, what a `match` on
//! the way holds, and what the gate says the flow holds of the resource (`workflow … from` with a
//! `resource` line under it, [`GateWorkflow::holds`]).
//!
//! [`GatePorts`] is the other way round: the ports sekisho reads a `.gate` with, as `ritsu
//! sekisho` and `ritsu check` join them (ritsu-project's `Joined::sekisho`).

use crate::{Books, Dates, Flows, Found, Items, OperationCall, Ports, Rules, Said};
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
    /// The inputs of its own the action takes (its `input` lines), in the order written: what its
    /// caller gives it beside the principal, the resource and the time, and what a call of an
    /// operation it guards gives the argument of each name. None for a Cedar pair written by hand,
    /// whose context is its caller's to say, and whose schema writes neither a unit nor a range of
    /// it (a `Long` is any number).
    pub inputs: Vec<GateInput>,
}

/// One `input` of a gate's action.
#[derive(Clone, Debug, PartialEq)]
pub struct GateInput {
    /// Its name and alias: a call gives it the argument named either way.
    pub name: String,
    pub alias: String,
    /// What it holds, as the port of `fed by` says what a place of a gate holds: a number with the
    /// type as the gate writes it, its unit and its range (an end not written is the most a gate's
    /// number can be, 2⁵³ − 1, or the least), an enum with its values, a bool, a date with its
    /// days; `T?` where it may be absent.
    pub holds: crate::Carried,
    /// The line it is declared on, from 1.
    pub line: usize,
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
    /// What the gate says the flow holds of a resource it acts on (the `resource` lines under the
    /// workflow), in the order written.
    pub holds: Vec<GateHolds>,
}

/// One `resource <type> from <value>` under a workflow of a gate, with the lines under it: where
/// the flow gives the value as the resource's id (the argument an action's `resource <type> from
/// <argument>` names), each attribute written is the value of the flow beside it.
#[derive(Clone, Debug, PartialEq)]
pub struct GateHolds {
    /// The resource type, by its name, and the value of the flow that is its id (`request_id`,
    /// `order.id`).
    pub resource: String,
    pub id: String,
    /// The line of the `resource` line, from 1.
    pub line: usize,
    /// Each attribute with the value of the flow it is (`amount = order.total`), and its line.
    pub attributes: Vec<(String, String, usize)>,
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

/// One attribute held to a value, for [`Gates::allowed_with`]: a question asked over the
/// combinations where the attribute has the value, and over every value of the rest.
#[derive(Clone, Debug, PartialEq)]
pub struct GateGiven {
    /// True for an attribute of the resource, false for one of the principal.
    pub of_resource: bool,
    /// The attribute, by its name or its alias.
    pub attribute: String,
    pub value: GateGivenValue,
}

/// The value [`GateGiven`] holds an attribute to.
#[derive(Clone, Debug, PartialEq)]
pub enum GateGivenValue {
    /// A bool's value.
    Bool(bool),
    /// A value of an enum, by its name or its alias.
    Enum(String),
    /// An attribute that points to an entity: the entity is the principal (true), or it is not.
    IsPrincipal(bool),
    /// An attribute that points to an entity: the principal is in it, the entity itself or a group
    /// it is a member of (Cedar's `in`; true), or it is not.
    HoldsPrincipal(bool),
}

impl GateGiven {
    pub fn resource(attribute: &str, value: GateGivenValue) -> GateGiven {
        GateGiven { of_resource: true, attribute: attribute.to_string(), value }
    }

    pub fn principal(attribute: &str, value: GateGivenValue) -> GateGiven {
        GateGiven { of_resource: false, attribute: attribute.to_string(), value }
    }

    /// The attribute as a gate writes it: `resource.self_approval`.
    pub fn place(&self) -> String {
        format!("{}.{}", if self.of_resource { "resource" } else { "principal" }, self.attribute)
    }

    /// What is held, for a message: the attribute as a gate writes it, and its value in words.
    pub fn text(&self) -> Text {
        let p = self.place();
        match &self.value {
            GateGivenValue::Bool(b) => ritsu_base::tr!("`{p}` が {b}", "`{p}` is {b}"),
            GateGivenValue::Enum(v) => ritsu_base::tr!("`{p}` が `{v}`", "`{p}` is `{v}`"),
            GateGivenValue::IsPrincipal(true) => ritsu_base::tr!("`{p}` が principal 自身", "`{p}` is the principal"),
            GateGivenValue::IsPrincipal(false) => ritsu_base::tr!("`{p}` が principal でない", "`{p}` is not the principal"),
            GateGivenValue::HoldsPrincipal(true) => ritsu_base::tr!("principal が `{p}` に入っている", "the principal is in `{p}`"),
            GateGivenValue::HoldsPrincipal(false) => ritsu_base::tr!("principal が `{p}` に入っていない", "the principal is not in `{p}`"),
        }
    }
}

/// What [`Gates::allowed_at`] answers for one call of a flow.
#[derive(Clone, Debug, PartialEq)]
pub struct AllowedAt {
    /// How far the workflow is allowed the action over the combinations the runs that reach the
    /// call can come to.
    pub allowance: Found<Allowance>,
    /// What the answer is held to, in the gate's words with the flow's beside them, each in both
    /// languages (`resource.amount` (the flow's `order.total`) is 0..1000): only what is held to
    /// less than the gate lets it be.
    pub held: Vec<Text>,
    /// The lines of the gate the answer rests on for what the flow holds of the resource (`amount =
    /// order.total` under `resource Order from order.id`), each as the gate writes it, with its
    /// line. Nothing checks that the resource's data is what the flow holds: the gate says so.
    pub rests_on: Vec<(Text, usize)>,
    /// What the answer does not count of what the runs that reach the call hold, each with why, as
    /// a sentence without its full stop, in both languages: what a `match` leaves of a value the
    /// gate has no word for, a `resource` line under the workflow that says nothing of this call.
    /// Leaving it out leaves more combinations, never fewer, so an answer of always or never has
    /// none: counting it would not change the answer.
    pub unheld: Vec<Text>,
    /// The attributes of the resource the answer reads, as the gate writes them
    /// (`resource.amount`), where no `resource` line under the workflow says anything of the
    /// resource at this call: the answer counts every value of each, and whether the flow holds
    /// them to fewer is written nowhere. (Under a `resource` line that is there, an attribute it
    /// leaves out is one the gate says the flow holds nothing of, and is not listed. Nor is an
    /// attribute that points to an entity, which no line says a value of a flow for.) An answer of
    /// always or never has none: nothing said of them would change it.
    pub unrelated: Vec<String>,
}

impl AllowedAt {
    /// An answer that rests on nothing the flow holds.
    pub fn of(allowance: Found<Allowance>) -> AllowedAt {
        AllowedAt { allowance, held: Vec::new(), rests_on: Vec::new(), unheld: Vec::new(), unrelated: Vec::new() }
    }
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

    /// [`Gates::allowed`] with some attributes held to values: how far a principal of the type
    /// `principal` (by its name or its alias; None for every type the action takes), whatever
    /// roles it holds, is allowed `action` over the combinations the check walks that agree with
    /// `given`, read as the check reads them. An attribute no policy of the action reads holds
    /// nothing: the answer does not rest on it. A value no combination has (an attribute held to
    /// be the principal, of another type than the principal's) leaves none, which is never
    /// allowed. A `.gate` is answered for; the default, and Cedar written by hand, is undecided,
    /// which changes nothing for a language that does not ask.
    fn allowed_with(&self, root: &Path, file: &str, action: &str, principal: Option<&str>, given: &[GateGiven]) -> Result<Found<Allowance>, Vec<Said>> {
        let _ = (root, file, action, principal, given);
        Ok(Found::Undecided(ritsu_base::tr!("属性の値を決めた問いには、`.gate` だけが答えます", "only a `.gate` answers a question that holds attributes to values")))
    }

    /// How far the workflow `workflow` is allowed `action` at one call of its flow (X16): over the
    /// combinations the check walks that the runs of the flow that reach `call` can come to. An
    /// input of the action is held to what the call gives the operation's argument of its name; an
    /// attribute of the resource to the value of the flow the gate says it is
    /// ([`GateWorkflow::holds`]), where the call gives that `resource` line's value as the
    /// resource's id; each to what a `match` on the way to the call leaves of it; and a rule the
    /// flow has called with the same values, to the answers the `match` on its answer leaves. What
    /// the flow holds that the gate has no word for holds nothing, and is said
    /// ([`AllowedAt::unheld`]). A `.gate` is answered for; the default, and Cedar written by hand,
    /// is [`Gates::allowed`] for the workflow, which rests on nothing the flow holds.
    fn allowed_at(&self, root: &Path, file: &str, action: &str, workflow: &str, call: &OperationCall) -> Result<AllowedAt, Vec<Said>> {
        let _ = call;
        self.allowed(root, file, action, &Asker::Workflow(workflow.to_string())).map(AllowedAt::of)
    }

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
