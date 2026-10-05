//! The checked program: every name resolved, every value typed, every call bound to a
//! task or a rule. The generators and the reference interpreter read only this.

use crate::diag::Text;
use crate::rulec::RuleInfo;
use ritsu_units::{Dim, Unit};
use crate::syntax::Kind;
use std::collections::BTreeMap;

pub type EnumId = usize;
pub type RecordId = usize;

#[derive(Clone, Debug)]
pub enum Ty {
    Int,
    Str,
    Bool,
    /// an RFC 3339 moment in UTC, `2026-10-01T10:00:00Z`, as Step Functions' Wait takes it
    Timestamp,
    /// a day of the calendar, without a time or a zone: `2026-10-01` (koyomi's and rulec's `date`)
    Date,
    /// a number with a unit, spelled as rulec spells it and written (`money[JPY, incl_tax]`), and
    /// counted on the wire in that unit (a rate, in its step)
    Num(Unit),
    Enum(EnumId),
    Record(RecordId),
    List(Box<Ty>),
    /// a value that may be absent; read as null when it is
    Opt(Box<Ty>),
    /// any JSON value, carried along without being looked into
    Json,
}

/// Two types are the same when they are the same type, and two units the same when they are the
/// same unit (ritsu-units' `Unit::same`): `money[JPY, incl_tax]` is `money[円, incl_tax]`, and
/// `mass[kg]` is not `mass[g]`, which dandori does not convert (its expressions compute nothing, P1).
impl PartialEq for Ty {
    fn eq(&self, o: &Ty) -> bool {
        match (self, o) {
            (Ty::Num(a), Ty::Num(b)) => a.same(b),
            (Ty::Enum(a), Ty::Enum(b)) => a == b,
            (Ty::Record(a), Ty::Record(b)) => a == b,
            (Ty::List(a), Ty::List(b)) | (Ty::Opt(a), Ty::Opt(b)) => a == b,
            (Ty::Int, Ty::Int) | (Ty::Str, Ty::Str) | (Ty::Bool, Ty::Bool) | (Ty::Timestamp, Ty::Timestamp) | (Ty::Date, Ty::Date) | (Ty::Json, Ty::Json) => true,
            _ => false,
        }
    }
}

impl Eq for Ty {}

impl Ty {
    /// The type without its `?`.
    pub fn inner(&self) -> &Ty {
        match self {
            Ty::Opt(t) => t,
            t => t,
        }
    }

    /// Whether a value of `self` can be given where `want` is expected.
    pub fn fits(&self, want: &Ty) -> bool {
        if self == want || *want == Ty::Json {
            return true;
        }
        match (self, want) {
            (t, Ty::Opt(w)) => t.fits(w),
            (Ty::List(a), Ty::List(b)) => a.fits(b),
            _ => false,
        }
    }
}

/// The whole numbers a value may be, both ends included (`range >=1 <=30`); an end that is
/// None is open. A range belongs to a place a value is put: an input, an output, a field of a
/// record, a parameter or the answer of a task, an input or an output of a rule (as rulec gives
/// it). For a `T?` or a `list[T]`, it holds the numbers inside.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Range {
    pub lo: Option<i64>,
    pub hi: Option<i64>,
}

impl Range {
    pub fn exactly(n: i64) -> Range {
        Range { lo: Some(n), hi: Some(n) }
    }

    pub fn contains(&self, n: i64) -> bool {
        self.lo.map_or(true, |lo| n >= lo) && self.hi.map_or(true, |hi| n <= hi)
    }

    /// Whether every number of `self` is one of `outer`'s.
    pub fn within(&self, outer: &Range) -> bool {
        let lo = match (outer.lo, self.lo) {
            (None, _) => true,
            (Some(o), Some(s)) => s >= o,
            (Some(_), None) => false,
        };
        let hi = match (outer.hi, self.hi) {
            (None, _) => true,
            (Some(o), Some(s)) => s <= o,
            (Some(_), None) => false,
        };
        lo && hi
    }

    /// The smallest range that holds the numbers of both.
    pub fn hull(&self, o: &Range) -> Range {
        Range { lo: self.lo.zip(o.lo).map(|(a, b)| a.min(b)), hi: self.hi.zip(o.hi).map(|(a, b)| a.max(b)) }
    }

    /// A number just outside: one below the low end, else one above the high end.
    pub fn beyond(&self) -> Option<i64> {
        self.lo.and_then(|lo| lo.checked_sub(1)).or(self.hi.and_then(|hi| hi.checked_add(1)))
    }

    /// The tests of the ends, each made by `test(">=", 1)`, for the generated code to join.
    pub fn tests(&self, test: impl Fn(&str, i64) -> String) -> Vec<String> {
        self.lo.map(|lo| test(">=", lo)).into_iter().chain(self.hi.map(|hi| test("<=", hi))).collect()
    }

    /// As a `.flow` writes it: `>=1 <=30`.
    pub fn show(&self) -> String {
        let mut parts = vec![];
        if let Some(lo) = self.lo {
            parts.push(format!(">={lo}"));
        }
        if let Some(hi) = self.hi {
            parts.push(format!("<={hi}"));
        }
        parts.join(" ")
    }
}

impl crate::rulec::Column {
    /// The range rulec gives the column: its schema's minimum and maximum.
    pub fn range(&self) -> Option<Range> {
        match &self.ty {
            crate::rulec::RType::Num { min, max, .. } if min.is_some() || max.is_some() => Some(Range { lo: *min, hi: *max }),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EnumDef {
    /// how the enum is written in a `.flow`: `status` for a local one, `payment_intent.status` for a rule's
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordOrigin {
    Local,
    /// the outputs of a rule, which a call of the rule returns; for a date of a dates file
    /// (`RuleKind::Date`), the day and its time, which a call of the date returns
    RuleOutputs(usize),
    /// a hold of a book's transfer (`stock.reserve`): the parameters of its key and its state, which
    /// a task that holds, posts or voids it answers, and a case that follows it is held in
    Hold { book: usize, transfer: String },
    /// a message of a `.proto`, made into a record (`use proto`, by the name `api`)
    Proto { api: String },
}

#[derive(Clone, Debug)]
pub struct RecordDef {
    pub name: String,
    pub fields: Vec<(String, Ty)>,
    /// the fields that have a range
    pub ranges: BTreeMap<String, Range>,
    pub origin: RecordOrigin,
}

/// A rule called as a Connect service (`connect "<url>"` under `use rule`): where to POST, what the
/// request and the response are made of, and the zero values the response's JSON may leave out.
#[derive(Clone, Debug)]
pub struct RuleConnect {
    /// where the service is, as `connect` writes it, without the slash at its end: `https://rules.example.com`
    pub base: String,
    /// the service's URL and the path `rulec api` gives: `https://rules.example.com/rulec.urgency.v1.UrgencyService/Decide`
    pub url: String,
    pub request: Vec<crate::rulec::WireField>,
    pub response: Vec<crate::rulec::WireField>,
    /// what `apis::fill` reads: the response's fields that protobuf's JSON leaves out at their zero
    /// value, by their JSON keys (`{"f": {"urgent": false, "carrier": "CARRIER_UNSPECIFIED"}}`); an
    /// enum's is the `.proto`'s name for its value 0, which `rulec api` gives (`ACTIVE` in a contract
    /// that puts a value of its own at 0)
    pub zeros: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct RuleUse {
    pub name: String,
    pub info: RuleInfo,
    /// the Lambda function that wraps the rule, for Step Functions and Lambda durable functions
    pub lambda: Option<String>,
    /// Temporal: called as a local activity, in the worker that runs the workflow; the other
    /// platforms call it as they call any rule
    pub local: bool,
    /// the rule is called at its Connect service on every platform, in place of the code that is
    /// written with the workflow
    pub connect: Option<RuleConnect>,
    /// Step Functions: the EventBridge connection its HTTP Task calls the service through
    pub connection: Option<String>,
    pub outputs: RecordId,
    pub line: usize,
    /// what it is: a rule of rulec's, a date of a dates file that koyomi computes (called as a rule
    /// is), or the life of a hold of a book's transfer (which a case follows, and nothing calls)
    pub kind: RuleKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuleKind {
    Rule,
    /// `use dates terms from "…"`, called as `terms.payment(received: …)`: koyomi's function for a
    /// date, which answers the day, and its time when the date says one (`at 09:00`)
    Date(DateCall),
    /// `case h : stock.reserve follows stock.reserve`: the life of a hold of the book's transfer,
    /// as chobo gives it (held, posted, voided, expired)
    Hold { book: usize, transfer: String },
}

/// A date of a dates file, as the code dandori writes calls koyomi's generated code for it.
#[derive(Clone, Debug, PartialEq)]
pub struct DateCall {
    /// the name `use dates` gives the file, and the date's name in it
    pub file: String,
    pub date: String,
    /// the ASCII aliases of the file and of the date, which koyomi names its code by
    pub file_alias: String,
    pub alias: String,
    /// the inputs the date reads, in the order written: name, alias, and whether it is a day
    /// (else a whole number)
    pub params: Vec<(String, String, bool)>,
    /// the date turns into a time of the day (`at 09:00`, `at end of day`)
    pub at: bool,
    /// the calendar's UTC offset, in minutes east: a time given for a day is read as the day it
    /// falls on there
    pub offset: Option<i32>,
}

impl RuleUse {
    /// The date of a dates file this is, when it is one.
    pub fn date(&self) -> Option<&DateCall> {
        match &self.kind {
            RuleKind::Date(d) => Some(d),
            _ => None,
        }
    }

    /// How a `.flow` names the machine it is: `payment_intent.payment` for a rule's, `stock.reserve`
    /// for the holds of a book's transfer.
    pub fn machine_label(&self) -> String {
        match (&self.kind, &self.info.machine) {
            (RuleKind::Hold { .. }, _) => self.name.clone(),
            (_, Some(mc)) => format!("{}.{}", self.name, mc.name),
            _ => self.name.clone(),
        }
    }

    /// Whether this is a rule of rulec's: its code goes with the workflow, and `rulec doc` draws it.
    pub fn is_rule(&self) -> bool {
        self.kind == RuleKind::Rule
    }
}

/// A book of chobo's that the tasks call (`use book stock from "inventory.book"`).
#[derive(Clone, Debug)]
pub struct BookUse {
    pub name: String,
    pub path: std::path::PathBuf,
    pub facts: ritsu_ports::BookFacts,
    /// Step Functions: the Lambda function that runs the book's operations
    pub lambda: Option<String>,
    pub line: usize,
}

impl BookUse {
    pub fn transfer(&self, name: &str) -> Option<&ritsu_ports::Transfer> {
        self.facts.transfers.iter().find(|t| t.name == name)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Binding {
    Lambda(String),
    Http { method: String, url: String, form: bool },
    /// an AWS API, named as Step Functions' AWS SDK integrations name it: `sns` and `publish`
    Aws { service: String, action: String },
    /// an agent: the model, told what to do, reads the arguments and answers in the task's type;
    /// `url` is the server of an Open Responses API other than OpenAI's, and `effort` how hard the
    /// model reasons, as the provider names the levels
    Agent { provider: Provider, instructions: String, model: String, url: Option<String>, effort: Option<String> },
    /// Jev, TypeSafe's System One model: it reads the arguments (its state) and answers the
    /// questions the task's type asks, all in one request
    Jev(Jev),
    /// an operation of a book's transfer (`book stock.reserve.hold`): `do`, `hold`, `post` or `void`,
    /// run by chobo's client; a refusal comes back as the error its reason names
    Book(BookOp),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BookOp {
    pub book: usize,
    pub transfer: String,
    pub op: String,
}

/// Where every target sends a Jev task's request.
pub const JEV_URL: &str = "https://api.typesafe.ai/v1/systemone";

/// What a Jev task asks, and how its answer is read.
#[derive(Clone, Debug, PartialEq)]
pub struct Jev {
    pub model: String,
    /// in the order of the answer's fields; one, whose `field` is None, when the answer is the
    /// question's own value
    pub questions: Vec<Question>,
    /// the fields that take how sure Jev is of a question's answer, as a rate: (field, the
    /// question, how many of the rate's steps make the whole)
    pub confidences: Vec<(String, usize, u64)>,
    /// `confidence 0.8 else unsure`: an answer less sure than this fails the call with the error
    pub floor: Option<(f64, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestionKind {
    /// one of the enum's values, with the probability of each
    Choice,
    /// a place on a scale of levels, the enum's values in the order the task lists them
    Score,
    /// the probability that the answer is yes
    Noul,
}

/// One question of a Jev task, which fills a field of the answer, or is the answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    /// the question's id in the request and the answer: the field, or `answer`
    pub id: String,
    pub field: Option<String>,
    pub kind: QuestionKind,
    pub instructions: String,
    /// for a choice, every value of the enum with what it means, when the task says; for a
    /// score, the levels from the lowest, each with what it means; for a noul, what yes and no
    /// mean (`true` and `false`), when the task says
    pub options: Vec<(String, Option<String>)>,
}

impl Jev {
    /// Whether the task says how sure an answer must be, or hands on how sure it is.
    pub fn uses_confidence(&self) -> bool {
        self.floor.is_some() || !self.confidences.is_empty()
    }
}

/// How many steps of a rate make the whole (100%): 100 for `rate[step 1%]`, 10000 for
/// `rate[step 0.01%]`. None for another unit, and for a step that does not go into 100% a whole
/// number of times.
pub fn rate_per(unit: &Unit) -> Option<u64> {
    match (&unit.dim, unit.step) {
        (Dim::Rate, Some(step)) if step.num == 1 && step.den > 0 => u64::try_from(step.den).ok(),
        _ => None,
    }
}

/// Whose models an agent runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// OpenAI: the Responses API on Step Functions, OpenAI's Agents SDK in the code dandori writes;
    /// with `url`, any server of the Responses API as Open Responses specifies it, which every
    /// target calls over HTTP
    OpenAi,
    /// Claude: the Messages API on Step Functions, Anthropic's SDK in the code dandori writes
    Claude,
}

impl Provider {
    /// The name the `.flow` and the generated code give it.
    pub fn name(self) -> &'static str {
        match self {
            Provider::OpenAi => "openai",
            Provider::Claude => "claude",
        }
    }
}

/// A declared error, and how the other side says it: an HTTP status, or an AWS API's exception.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrDef {
    pub name: String,
    pub status: Option<u16>,
    pub exception: Option<String>,
}

/// The platforms a workflow compiles to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    StepFunctions,
    Temporal,
    Durable,
    Argo,
    /// pydantic-graph: a graph that runs in the process that calls it
    Graph,
}

/// How a platform calls a task.
#[derive(Clone, Debug, PartialEq)]
pub enum Via<'a> {
    Lambda(&'a str),
    Http { method: &'a str, url: &'a str, form: bool },
    Aws { service: &'a str, action: &'a str },
    /// an agent of OpenAI's or Claude's (see `Provider`), or on another server that speaks
    /// OpenAI's Responses API as Open Responses specifies it (`url`)
    Agent { provider: Provider, instructions: &'a str, model: &'a str, url: Option<&'a str>, effort: Option<&'a str> },
    /// Jev, over HTTP: a POST to `JEV_URL`, the same on every platform
    Jev(&'a Jev),
    /// an operation of a book of chobo's: through the transport, by chobo's client; on Step
    /// Functions, by the book's Lambda function
    Book(&'a BookOp),
    /// Step Functions: a nested execution of another state machine
    StateMachine(&'a str),
    /// Temporal: a child workflow
    Workflow(&'a str),
    /// Lambda durable functions: a function the durable execution invokes and waits for
    DurableFunction(&'a str),
    /// Argo Workflows: a container of the user's image
    Image(&'a str),
    /// Argo Workflows: a workflow made from a WorkflowTemplate, run to its end
    ArgoTemplate(&'a str),
    /// Temporal and durable functions: the implementation the user writes
    Own,
    /// Temporal: nothing is called; the workflow waits for a value sent to it by name (an Update)
    Event,
}

#[derive(Clone, Debug)]
pub struct Retry {
    pub times: u32,
    pub every: u64,
    pub backoff: f64,
    /// the declared errors to retry on; empty means the transient failures of the platform
    pub on: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum TaskMachine {
    Starts { rule: usize, then: Vec<String> },
    Sends { event: String, column: Option<String> },
    Observes,
}

/// Another `.flow` that a task runs as its child workflow: where it is, and its checked model,
/// which the task's parameters, answer and errors are held to.
#[derive(Clone, Debug)]
pub struct ChildFlow {
    pub path: String,
    pub model: Model,
}

#[derive(Clone, Debug)]
pub struct TaskDef {
    pub name: String,
    pub params: Vec<(String, Ty)>,
    /// the parameters that have a range
    pub param_ranges: BTreeMap<String, Range>,
    /// None: the task answers nothing the workflow reads
    pub result: Option<Ty>,
    /// the range of the answer, when the answer is a number (or a list of numbers, …)
    pub result_range: Option<Range>,
    pub binding: Option<Binding>,
    pub connection: Option<String>,
    pub queue: Option<String>,
    pub workflow: Option<String>,
    pub state_machine: Option<String>,
    pub durable_function: Option<String>,
    pub image: Option<String>,
    pub argo_template: Option<String>,
    pub errors: Vec<ErrDef>,
    pub retry: Option<Retry>,
    pub timeout: Option<u64>,
    pub key: bool,
    /// the parameter of an AWS API that takes the idempotency key
    pub key_param: Option<String>,
    pub idempotent: bool,
    pub machine: Option<TaskMachine>,
    pub refused_as: Option<String>,
    pub callback: bool,
    /// Temporal: the task calls nothing; it waits for a value sent to the workflow by its id
    /// and the task's name (an Update), which the other platforms cannot do
    pub event: bool,
    /// the child workflow is this `.flow`
    pub flow: Option<Box<ChildFlow>>,
    /// `connect`: the task is an HTTP call by the Connect protocol, and this names the zero values
    /// its answer's JSON may leave out (`apis::Op::zeros`), which the generated code fills in
    pub connect: Option<serde_json::Value>,
    /// an `event` or a `callback` task whose value a method of the service the workflow implements
    /// sends (DESIGN 1.14): the zero values protobuf's JSON leaves out of the method's request, which
    /// the workflow's code fills in when the value comes, before it checks it
    pub answer_zeros: Option<serde_json::Value>,
    pub line: usize,
}

impl TaskDef {
    /// A task that changes something on the other side, so that doing it twice is not the
    /// same as doing it once, unless the task says it is. An agent only reads and answers;
    /// an event only comes.
    pub fn changes_things(&self) -> bool {
        !self.idempotent && !self.event && !matches!(self.machine, Some(TaskMachine::Observes)) && !matches!(self.binding, Some(Binding::Agent { .. }) | Some(Binding::Jev(_)) | Some(Binding::Book(_)))
    }

    /// The operation of a book this task is, when it is one.
    pub fn book(&self) -> Option<&BookOp> {
        match &self.binding {
            Some(Binding::Book(b)) => Some(b),
            _ => None,
        }
    }

    /// What the task asks Jev, when it is a Jev task.
    pub fn jev(&self) -> Option<&Jev> {
        match &self.binding {
            Some(Binding::Jev(j)) => Some(j),
            _ => None,
        }
    }

    /// How the platform calls this task; None when it cannot (Step Functions without a way to call it).
    pub fn via(&self, p: Platform) -> Option<Via<'_>> {
        if self.event {
            return (p == Platform::Temporal).then_some(Via::Event);
        }
        let bound = self.binding.as_ref().map(|b| match b {
            Binding::Lambda(f) => Via::Lambda(f),
            Binding::Http { method, url, form } => Via::Http { method, url, form: *form },
            Binding::Aws { service, action } => Via::Aws { service, action },
            Binding::Agent { provider, instructions, model, url, effort } => Via::Agent { provider: *provider, instructions, model, url: url.as_deref(), effort: effort.as_deref() },
            Binding::Jev(j) => Via::Jev(j),
            Binding::Book(b) => Via::Book(b),
        });
        match p {
            Platform::StepFunctions => self.state_machine.as_deref().map(Via::StateMachine).or(bound),
            Platform::Temporal => Some(self.workflow.as_deref().map(Via::Workflow).or(bound).unwrap_or(Via::Own)),
            Platform::Durable => Some(self.durable_function.as_deref().map(Via::DurableFunction).or(bound).unwrap_or(Via::Own)),
            // a task of the user's is a container of their image; the others run the code dandori writes
            Platform::Argo => self.argo_template.as_deref().map(Via::ArgoTemplate).or(self.image.as_deref().map(Via::Image)).or(bound),
            // a task that is a workflow elsewhere is a function the user writes here
            Platform::Graph => Some(bound.unwrap_or(Via::Own)),
        }
    }

    /// Whether a platform runs another workflow for this task, so that its errors come from the child's `fail`.
    pub fn is_child(&self, p: Platform) -> bool {
        matches!(self.via(p), Some(Via::StateMachine(_)) | Some(Via::Workflow(_)) | Some(Via::DurableFunction(_)) | Some(Via::ArgoTemplate(_)))
    }

    pub fn error(&self, name: &str) -> Option<&ErrDef> {
        self.errors.iter().find(|e| e.name == name)
    }
}

/// The service of a `.proto` that the workflow implements (`workflow … implements <api>.<Service>`,
/// DESIGN 1.14): its methods, each marked by one of dandori's options with the way it reaches a run.
#[derive(Clone, Debug)]
pub struct ServiceUse {
    /// the name `use proto` gives the `.proto`, and the path it reads it from
    pub api: String,
    pub file: String,
    /// the service's full name: `shop.v1.FulfillmentService`
    pub name: String,
    /// where `implements` names it, which E017 points at
    pub line: usize,
    pub col: usize,
    /// the `.proto`, as `use proto` read it
    pub doc: crate::apis::Api,
    pub methods: Vec<MethodUse>,
    /// the zero values protobuf's JSON leaves out of the request of the method that starts a run
    /// (`apis::zeros_of`), which every platform fills in before it checks the input
    pub input_zeros: serde_json::Value,
}

impl ServiceUse {
    /// The service's name without its package: `FulfillmentService`.
    pub fn simple(&self) -> &str {
        self.name.rsplit('.').next().unwrap_or(&self.name)
    }

    /// How a message names a method: `FulfillmentService/Fulfill`.
    pub fn label(&self, method: &MethodUse) -> String {
        format!("{}/{}", self.simple(), method.name)
    }
}

/// A method of the service a workflow implements.
#[derive(Clone, Debug)]
pub struct MethodUse {
    pub name: String,
    /// the full names of its request and its response
    pub request: String,
    pub response: String,
    pub streams: bool,
    /// dandori's marks on it, in the order start, event, answer, status: one, when the service is
    /// right; none or more are kept for E017 to say so
    pub marks: Vec<Mark>,
}

impl MethodUse {
    pub fn starts(&self) -> bool {
        self.marks.iter().any(|k| matches!(k, Mark::Start { .. }))
    }

    pub fn is_status(&self) -> bool {
        self.marks.contains(&Mark::Status)
    }
}

/// How a method of the service reaches a run: dandori's options on it (`proto/dandori/v1/options.proto`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mark {
    /// `(dandori.v1.start)`: it starts a run; `fails`, the names a run can fail with
    Start { fails: Vec<String> },
    /// `(dandori.v1.event)`: it sends a run the value the `event` task `task` waits for
    Event { task: String },
    /// `(dandori.v1.answer)`: it answers the `callback` task `task`
    Answer { task: String },
    /// `(dandori.v1.status)`: it asks a run where it is
    Status,
}

impl Mark {
    /// The option, as a `.proto` writes it: `(dandori.v1.start)`.
    pub fn option(&self) -> &'static str {
        match self {
            Mark::Start { .. } => "(dandori.v1.start)",
            Mark::Event { .. } => "(dandori.v1.event)",
            Mark::Answer { .. } => "(dandori.v1.answer)",
            Mark::Status => "(dandori.v1.status)",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CaseDef {
    pub name: String,
    pub record: RecordId,
    pub rule: usize,
    pub state_field: String,
    /// the held inputs that are axes of the machine's table, fixed to one coordinate
    pub held: Vec<(usize, usize)>,
    pub held_values: Vec<(String, String)>,
    /// events that happen on the other side without the workflow: (axis, coordinate, name)
    pub external: Vec<(usize, usize, String)>,
    /// (index into the machine's `decides`, value) of the output that says the event was refused
    pub refused_when: Option<(usize, String)>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct TStmt {
    pub kind: TK,
    pub line: usize,
    /// unique in the program; the generators name states and idempotency keys after it
    pub site: usize,
}

#[derive(Clone, Debug)]
pub enum Target {
    Let(String),
    Case(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Callee {
    Task(usize),
    Rule(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HErr {
    Declared(String),
    Timeout,
    Failure,
}

#[derive(Clone, Debug)]
pub struct THandler {
    pub errors: Vec<HErr>,
    pub body: Vec<TStmt>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct TArm {
    pub values: Vec<String>,
    /// the arm for a case that has not been started, or for an absent value
    pub none: bool,
    /// `some x =>`: the arm for a value that is there, which the arm reads as `x`
    pub some: Option<String>,
    pub body: Vec<TStmt>,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IPart {
    Lit(String),
    Hole(TExpr),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TExpr {
    Var { name: String, fields: Vec<String>, ty: Ty },
    Str(String),
    Int(i64),
    Bool(bool),
    Enum(String, EnumId),
    /// `none`, of the optional type it is given as
    None(Ty),
    /// a record written out: of a record type, or of `json`
    Record { fields: Vec<(String, TExpr)>, ty: Ty },
    List { items: Vec<TExpr>, ty: Ty },
    /// a string with values put in
    Interp(Vec<IPart>),
    /// `now`: the moment the statement runs, as the platform's clock reads it (on Temporal, the
    /// workflow's time, which a replay reads again the same)
    Now,
}

impl TExpr {
    pub fn ty(&self) -> Ty {
        match self {
            TExpr::Var { ty, .. } => ty.clone(),
            TExpr::Str(_) | TExpr::Interp(_) => Ty::Str,
            TExpr::Int(_) => Ty::Int,
            TExpr::Bool(_) => Ty::Bool,
            TExpr::Now => Ty::Timestamp,
            TExpr::Enum(_, e) => Ty::Enum(*e),
            TExpr::None(t) | TExpr::Record { ty: t, .. } | TExpr::List { ty: t, .. } => t.clone(),
        }
    }

    /// Whether the expression reads `now`.
    pub fn has_now(&self) -> bool {
        match self {
            TExpr::Now => true,
            TExpr::Record { fields, .. } => fields.iter().any(|(_, x)| x.has_now()),
            TExpr::List { items, .. } => items.iter().any(|x| x.has_now()),
            TExpr::Interp(parts) => parts.iter().any(|p| matches!(p, IPart::Hole(x) if x.has_now())),
            _ => false,
        }
    }

    /// Every variable the expression reads.
    pub fn vars(&self) -> Vec<&TExpr> {
        let mut out = Vec::new();
        fn go<'a>(e: &'a TExpr, out: &mut Vec<&'a TExpr>) {
            match e {
                TExpr::Var { .. } => out.push(e),
                TExpr::Record { fields, .. } => fields.iter().for_each(|(_, x)| go(x, out)),
                TExpr::List { items, .. } => items.iter().for_each(|x| go(x, out)),
                TExpr::Interp(parts) => parts.iter().for_each(|p| {
                    if let IPart::Hole(x) = p {
                        go(x, out)
                    }
                }),
                _ => {}
            }
        }
        go(self, &mut out);
        out
    }

    /// The source form, for messages and names: `pi.status`, `"jpy"`.
    pub fn show(&self) -> String {
        match self {
            TExpr::Var { name, fields, .. } => std::iter::once(name.clone()).chain(fields.iter().cloned()).collect::<Vec<_>>().join("."),
            TExpr::Str(s) => s.clone(),
            TExpr::Int(n) => n.to_string(),
            TExpr::Bool(b) => b.to_string(),
            TExpr::Enum(v, _) => v.clone(),
            TExpr::None(_) => "none".into(),
            TExpr::Now => "now".into(),
            TExpr::Record { fields, .. } => format!("{{{}}}", fields.iter().map(|(f, x)| format!("{f}: {}", x.show())).collect::<Vec<_>>().join(", ")),
            TExpr::List { items, .. } => format!("[{}]", items.iter().map(|x| x.show()).collect::<Vec<_>>().join(", ")),
            TExpr::Interp(parts) => parts
                .iter()
                .map(|p| match p {
                    IPart::Lit(s) => s.clone(),
                    IPart::Hole(x) => format!("{{{}}}", x.show()),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TK {
    Call { target: Option<Target>, callee: Callee, args: Vec<(String, TExpr)>, handlers: Vec<THandler> },
    /// `let x = <value>`
    Assign { name: String, expr: TExpr },
    Match { expr: TExpr, arms: Vec<TArm> },
    Wait { seconds: u64 },
    /// until a moment given as an RFC 3339 string
    WaitUntil { at: TExpr },
    Repeat { times: u32, body: Vec<TStmt> },
    /// `for var in list at most max`; `parallel` is how many rounds run at a time (0: all).
    /// `result` is the variable that gets the list of what each round yields, and what it
    /// yields; `locals` are the variables a parallel round sets, which each round has its own of.
    For { var: String, list: TExpr, max: u32, parallel: Option<u32>, body: Vec<TStmt>, result: Option<(String, TExpr)>, locals: Vec<String> },
    Break,
    Pass,
    Succeed { fields: Vec<(String, TExpr)> },
    Fail { error: String, cause: Option<TExpr>, leaving: Vec<usize> },
    /// A rule's precondition that ritsu could not decide where the flow calls the rule (ritsu's X2,
    /// DESIGN 1.17), checked here, as soon as the values the call gives are made: a run whose values
    /// break it fails with `Dandori.BrokenPrecondition`. Never written in a `.flow`: `prechecks`
    /// puts it in after the check, from what `ritsu dandori` hands over.
    Check(PreCheck),
}

/// A rule's precondition, checked where the code of the workflow runs (`TK::Check`).
#[derive(Clone, Debug)]
pub struct PreCheck {
    /// the rule called, and the line of the call the check is for
    pub rule: usize,
    pub call_line: usize,
    pub test: PreTest,
}

#[derive(Clone, Debug)]
pub enum PreTest {
    /// `left op right` between two inputs of the rule (`op` is `<=`, `<`, `>=` or `>`): each the
    /// input's name and the value the call gives it, a whole number on the wire
    Relation { left: (String, TExpr), op: String, right: (String, TExpr) },
    /// a date input that takes only the days of a koyomi date (rulec's `range from koyomi`): the
    /// input, the value the call gives it, the file and the date as the rule names them, and the
    /// days, `YYYY-MM-DD` in order
    Days { input: String, value: TExpr, file: String, date: String, days: Vec<String> },
}

impl PreCheck {
    /// The values the check reads.
    pub fn exprs(&self) -> Vec<&TExpr> {
        match &self.test {
            PreTest::Relation { left, right, .. } => vec![&left.1, &right.1],
            PreTest::Days { value, .. } => vec![value],
        }
    }

    /// The precondition, as ritsu says it: `asked <= paid`, `pay_day in koyomi "terms.cal" date payment`.
    pub fn text(&self) -> String {
        match &self.test {
            PreTest::Relation { left, op, right } => format!("{} {op} {}", left.0, right.0),
            PreTest::Days { input, file, date, .. } => format!("{input} in koyomi \"{file}\" date {date}"),
        }
    }

    /// The cause a run that breaks it fails with, the same on every platform.
    pub fn cause(&self, m: &Model) -> String {
        format!("line {}: the values given to the rule {} break its precondition {}", self.call_line, m.rules[self.rule].name, self.text())
    }
}

impl TK {
    /// The values the statement itself holds, not those of the blocks under it.
    pub fn exprs(&self) -> Vec<&TExpr> {
        match self {
            TK::Call { args, .. } => args.iter().map(|(_, e)| e).collect(),
            TK::Assign { expr, .. } | TK::Match { expr, .. } => vec![expr],
            TK::WaitUntil { at } => vec![at],
            TK::For { list, result, .. } => std::iter::once(list).chain(result.as_ref().map(|(_, y)| y)).collect(),
            TK::Succeed { fields } => fields.iter().map(|(_, e)| e).collect(),
            TK::Fail { cause, .. } => cause.iter().collect(),
            TK::Check(c) => c.exprs(),
            _ => vec![],
        }
    }

    /// Whether the statement itself reads `now`.
    pub fn reads_now(&self) -> bool {
        self.exprs().iter().any(|e| e.has_now())
    }
}

/// Where a flow of a package of ritsu's (ritsu's DESIGN 9.3) finds what it reads beside its own code:
/// a flow is at flows/<name>/, beside the package's rules/, dates/ and books/.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InPackage {
    /// The Go module the package is (its go.mod): a rule's package is `<module>/rules/<package>`.
    pub go_module: String,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub name: String,
    pub version: u32,
    pub description: String,
    pub kind: Kind,
    pub source_file: String,
    /// The `.flow` as the head of every generated file names it (ritsu's DESIGN 9.2): its name, or
    /// under `ritsu gen` its path from the project's root.
    pub source_path: String,
    /// The SHA-256 of the `.flow`'s bytes, in hex.
    pub source_sha256: String,
    /// Built as a flow of a package of ritsu's (`ritsu gen`, ritsu's DESIGN 9.3): the workflow reads
    /// the rules, the dates and the books from the package's rules/, dates/ and books/; None for a
    /// build of its own, which reads them from code put beside it.
    pub package: Option<InPackage>,
    pub rules: Vec<RuleUse>,
    /// the books of chobo's the tasks call
    pub books: Vec<BookUse>,
    pub enums: Vec<EnumDef>,
    pub records: Vec<RecordDef>,
    pub inputs: Vec<(String, Ty)>,
    pub outputs: Vec<(String, Ty)>,
    /// the inputs and the outputs that have a range
    pub input_ranges: BTreeMap<String, Range>,
    pub output_ranges: BTreeMap<String, Range>,
    pub tasks: Vec<TaskDef>,
    pub cases: Vec<CaseDef>,
    /// every variable: the inputs, the `let`s and the cases, with its type
    pub vars: Vec<(String, Ty)>,
    pub flow: Vec<TStmt>,
    pub on_failure: Option<Vec<TStmt>>,
    /// what runs when the workflow is cancelled: Temporal asks a workflow to stop, and it
    /// settles what it must before it ends as cancelled
    pub on_cancel: Option<Vec<TStmt>>,
    /// the line of `on cancel`
    pub on_cancel_line: usize,
    /// for a call on a case: the states its answer may carry, which the generated code checks
    pub monitors: BTreeMap<usize, (usize, Vec<String>)>,
    /// the service of a `.proto` the workflow implements
    pub service: Option<ServiceUse>,
}

impl Model {
    /// The head every file the builds write begins with (ritsu's DESIGN 9.2): ritsu's version, and
    /// the `.flow` with the digest of its bytes, each line a comment that starts with `mark`.
    pub fn head(&self, mark: &str) -> String {
        let version = self.version.to_string();
        let source = ritsu_emit::header::Source { path: &self.source_path, kind: "workflow", name: &self.name, version: &version, sha256: &self.source_sha256 };
        format!("{mark} {}\n{mark} {}\n", ritsu_emit::header::generated("dandori"), source.line().get(ritsu_base::text::Lang::En))
    }

    /// `<name> v<version>: <description>`, a comment of as many lines as the description has. The
    /// description is a string of the `.flow`, which may hold `\n` (and the breaks TypeScript and
    /// YAML end a line at); written after one comment mark, the rest of it was a line of the
    /// generated code, and in the YAML for Argo a document of its own (DESIGN 4.7).
    pub fn titled(&self, c: ritsu_emit::header::Comment) -> String {
        let described = if self.description.is_empty() { String::new() } else { format!(": {}", self.description) };
        c.lines(&format!("{} v{}{described}", self.name, self.version))
    }

    /// Whether a statement of the workflow reads `now`.
    pub fn uses_now(&self) -> bool {
        self.all_stmts().iter().any(|s| s.kind.reads_now())
    }

    /// Whether the workflow calls a date of a dates file.
    pub fn calls_dates(&self) -> bool {
        self.all_stmts().iter().any(|s| matches!(&s.kind, TK::Call { callee: Callee::Rule(r), .. } if self.rules[*r].date().is_some()))
    }

    /// Whether a value of the workflow is a day (`date`): an input, an output, a field of a record
    /// it holds, a parameter or the answer of a task, a variable.
    pub fn uses_dates(&self) -> bool {
        fn has(m: &Model, t: &Ty, seen: &mut Vec<RecordId>) -> bool {
            match t {
                Ty::Date => true,
                Ty::List(x) | Ty::Opt(x) => has(m, x, seen),
                Ty::Record(r) if !seen.contains(r) => {
                    seen.push(*r);
                    m.records[*r].fields.iter().any(|(_, f)| has(m, f, seen))
                }
                _ => false,
            }
        }
        let mut seen = Vec::new();
        let tys = self.inputs.iter().chain(&self.outputs).chain(&self.vars).map(|(_, t)| t).chain(self.tasks.iter().flat_map(|t| t.params.iter().map(|(_, p)| p).chain(t.result.iter())));
        tys.into_iter().collect::<Vec<_>>().into_iter().any(|t| has(self, t, &mut seen))
    }

    /// A platform's refusal of `on cancel`, when the workflow has one: the platform stops a run
    /// at once and runs nothing after.
    pub fn refuse_on_cancel(&self, why: Text) -> Option<crate::diag::Diag> {
        self.on_cancel.as_ref().map(|_| crate::diag::Diag::error("E050", self.on_cancel_line, 1, why))
    }

    /// A platform's refusal of each task that says `event` (E050): only a Temporal workflow can
    /// be sent a value by its id and a name.
    pub fn refuse_events(&self, p: Platform) -> Vec<crate::diag::Diag> {
        let Text { en, ja } = match p {
            Platform::Temporal => return vec![],
            Platform::StepFunctions => tr!("Step Functions が実行に値を送れるのは、タスクが渡すトークンを通してだけです（`callback`）", "Step Functions sends a value to an execution only through the token a task hands on (`callback`)"),
            Platform::Durable => tr!("Lambda durable functions が実行に値を送れるのは、ステップが渡すコールバックの ID を通してだけです（`callback`）", "Lambda durable functions sends a value to an execution only through the id of a callback a step hands on (`callback`)"),
            Platform::Argo => tr!("dandori は、Argo Workflows 向けにはまだこれを生成しません。生成するなら、名前で再開されるまで止まる suspend のステップになります", "dandori does not write it for Argo Workflows yet; there, it would be a step suspended until it is resumed by its name"),
            Platform::Graph => tr!("dandori は、pydantic-graph 向けにはまだこれを生成しません。生成するなら、値はタスクの名前で `Deps` に届くことになります", "dandori does not write it for pydantic-graph yet; there, the value would come to `Deps` by the task's name"),
        };
        self.tasks
            .iter()
            .filter(|t| t.event)
            .map(|t| crate::diag::Diag::error("E050", t.line, 1, tr!("`{}` はワークフローに名前で送られてくるイベントを待ちます。{ja}", "`{}` waits for an event sent to the workflow by name; {en}", t.name)))
            .collect()
    }

    /// A platform's refusal of the method of the service the workflow implements that asks a run where
    /// it is (E050): only Temporal answers it, by the query `dandori.status`.
    pub fn refuse_status(&self, p: Platform) -> Vec<crate::diag::Diag> {
        let Some(s) = &self.service else { return vec![] };
        let words = |label: &str| match p {
            Platform::Temporal => None,
            Platform::StepFunctions | Platform::Durable => {
                let name = if p == Platform::StepFunctions { "Step Functions" } else { "Lambda durable functions" };
                Some(tr!("`{label}` は実行がいまどこにいるかを聞きますが、{name} にはそれに答える手段がありません。答えるのはクエリ `dandori.status` を持つ Temporal だけなので、{name} 向けのフローが実装するサービスからは、このメソッドを外してください", "`{label}` asks a run where it is, and {name} has no way to answer it; only Temporal answers the query `dandori.status`, so leave the method out of the service the flow implements for {name}"))
            }
            Platform::Argo | Platform::Graph => {
                let name = if p == Platform::Argo { "Argo Workflows" } else { "pydantic-graph" };
                Some(tr!("`{label}` は実行がいまどこにいるかを聞きますが、dandori は {name} ではまだそれに答えられません。答えるのはクエリ `dandori.status` を持つ Temporal だけなので、{name} 向けのフローが実装するサービスからは、このメソッドを外してください", "`{label}` asks a run where it is, and dandori does not answer it on {name} yet; only Temporal answers it, by the query `dandori.status`, so leave the method out of the service the flow implements for {name}"))
            }
        };
        s.methods.iter().filter(|x| x.is_status()).filter_map(|x| words(&s.label(x))).map(|why| crate::diag::Diag::error("E050", s.line, 1, why)).collect()
    }

    /// The method of the service the workflow implements that starts a run.
    pub fn service_start(&self) -> Option<&MethodUse> {
        self.service.as_ref()?.methods.iter().find(|x| x.starts())
    }

    /// The method of the service the workflow implements that asks a run where it is.
    pub fn service_status(&self) -> Option<&MethodUse> {
        self.service.as_ref()?.methods.iter().find(|x| x.is_status())
    }

    pub fn ty_name(&self, t: &Ty) -> String {
        match t {
            Ty::Int => "int".into(),
            Ty::Str => "string".into(),
            Ty::Bool => "bool".into(),
            Ty::Timestamp => "timestamp".into(),
            Ty::Date => "date".into(),
            Ty::Num(u) => u.to_string(),
            Ty::Enum(e) => self.enums[*e].name.clone(),
            Ty::Record(r) => self.records[*r].name.clone(),
            Ty::List(t) => format!("list[{}]", self.ty_name(t)),
            Ty::Opt(t) => format!("{}?", self.ty_name(t)),
            Ty::Json => "json".into(),
        }
    }

    pub fn var_ty(&self, name: &str) -> Option<&Ty> {
        self.vars.iter().find(|(n, _)| n == name).map(|(_, t)| t)
    }

    pub fn field_ty(&self, rec: RecordId, field: &str) -> Option<&Ty> {
        self.records[rec].fields.iter().find(|(n, _)| n == field).map(|(_, t)| t)
    }

    pub fn field_range(&self, rec: RecordId, field: &str) -> Option<Range> {
        self.records[rec].ranges.get(field).copied()
    }

    /// The range of what `callee` answers: a task's `-> int range …`. A rule answers a record,
    /// whose fields have the ranges rulec gives them.
    pub fn answer_range(&self, callee: &Callee) -> Option<Range> {
        match callee {
            Callee::Task(t) => self.tasks[*t].result_range,
            Callee::Rule(_) => None,
        }
    }

    /// The range rulec gives the input `param` of the rule.
    pub fn rule_input_range(&self, rule: usize, param: &str) -> Option<Range> {
        self.rules[rule].info.inputs.iter().find(|c| c.name == param).and_then(|c| c.range())
    }

    pub fn case_index(&self, name: &str) -> Option<usize> {
        self.cases.iter().position(|c| c.name == name)
    }

    pub fn machine(&self, case: usize) -> &crate::rulec::Machine {
        self.rules[self.cases[case].rule].info.machine.as_ref().expect("a case follows a machine")
    }

    /// Visit every statement, depth first, in source order.
    pub fn walk<'a>(stmts: &'a [TStmt], f: &mut dyn FnMut(&'a TStmt)) {
        for s in stmts {
            f(s);
            match &s.kind {
                TK::Call { handlers, .. } => {
                    for h in handlers {
                        Model::walk(&h.body, f);
                    }
                }
                TK::Match { arms, .. } => {
                    for a in arms {
                        Model::walk(&a.body, f);
                    }
                }
                TK::Repeat { body, .. } | TK::For { body, .. } => Model::walk(body, f),
                _ => {}
            }
        }
    }

    pub fn all_stmts(&self) -> Vec<&TStmt> {
        let mut out = Vec::new();
        Model::walk(&self.flow, &mut |s| out.push(s));
        if let Some(f) = &self.on_failure {
            Model::walk(f, &mut |s| out.push(s));
        }
        if let Some(f) = &self.on_cancel {
            Model::walk(f, &mut |s| out.push(s));
        }
        out
    }
}
