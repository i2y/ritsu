//! The checked program: every name resolved, every value typed, every call bound to a
//! task or a rule. The generators and the reference interpreter read only this.

use crate::rulec::RuleInfo;
use crate::syntax::Kind;
use std::collections::BTreeMap;

pub type EnumId = usize;
pub type RecordId = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    Int,
    Str,
    Bool,
    /// an RFC 3339 moment in UTC, `2026-10-01T10:00:00Z`, as Step Functions' Wait takes it
    Timestamp,
    /// a number with a unit, as rulec spells it
    Num(String),
    Enum(EnumId),
    Record(RecordId),
    List(Box<Ty>),
    /// a value that may be absent; read as null when it is
    Opt(Box<Ty>),
    /// any JSON value, carried along without being looked into
    Json,
}

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
    /// the outputs of a rule, which a call of the rule returns
    RuleOutputs(usize),
}

#[derive(Clone, Debug)]
pub struct RecordDef {
    pub name: String,
    pub fields: Vec<(String, Ty)>,
    /// the fields that have a range
    pub ranges: BTreeMap<String, Range>,
    pub origin: RecordOrigin,
}

#[derive(Clone, Debug)]
pub struct RuleUse {
    pub name: String,
    pub info: RuleInfo,
    pub lambda: Option<String>,
    /// Temporal: called as a local activity, in the worker that runs the workflow; the other
    /// platforms call it as they call any rule
    pub local: bool,
    pub outputs: RecordId,
    pub line: usize,
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
    pub line: usize,
}

impl TaskDef {
    /// A task that changes something on the other side, so that doing it twice is not the
    /// same as doing it once, unless the task says it is. An agent only reads and answers;
    /// an event only comes.
    pub fn changes_things(&self) -> bool {
        !self.idempotent && !self.event && !matches!(self.machine, Some(TaskMachine::Observes)) && !matches!(self.binding, Some(Binding::Agent { .. }))
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
}

impl TExpr {
    pub fn ty(&self) -> Ty {
        match self {
            TExpr::Var { ty, .. } => ty.clone(),
            TExpr::Str(_) | TExpr::Interp(_) => Ty::Str,
            TExpr::Int(_) => Ty::Int,
            TExpr::Bool(_) => Ty::Bool,
            TExpr::Enum(_, e) => Ty::Enum(*e),
            TExpr::None(t) | TExpr::Record { ty: t, .. } | TExpr::List { ty: t, .. } => t.clone(),
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
}

#[derive(Clone, Debug)]
pub struct Model {
    pub name: String,
    pub version: u32,
    pub description: String,
    pub kind: Kind,
    pub source_file: String,
    pub rules: Vec<RuleUse>,
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
}

impl Model {
    /// A platform's refusal of `on cancel`, when the workflow has one: the platform stops a run
    /// at once and runs nothing after.
    pub fn refuse_on_cancel(&self, en: &str, ja: &str) -> Option<crate::diag::Diag> {
        self.on_cancel.as_ref().map(|_| crate::diag::Diag::error("E050", self.on_cancel_line, 1, en, ja))
    }

    /// A platform's refusal of each task that says `event` (E050): only a Temporal workflow can
    /// be sent a value by its id and a name.
    pub fn refuse_events(&self, p: Platform) -> Vec<crate::diag::Diag> {
        let (en, ja) = match p {
            Platform::Temporal => return vec![],
            Platform::StepFunctions => (
                "Step Functions sends a value to an execution only through the token a task hands on (`callback`)",
                "Step Functions が実行に値を送れるのは、タスクが渡すトークンを通してだけです（`callback`）",
            ),
            Platform::Durable => (
                "Lambda durable functions sends a value to an execution only through the id of a callback a step hands on (`callback`)",
                "Lambda durable functions が実行に値を送れるのは、ステップが渡すコールバックの ID を通してだけです（`callback`）",
            ),
            Platform::Argo => (
                "dandori does not write it for Argo Workflows yet; there, it would be a step suspended until it is resumed by its name",
                "Argo Workflows 向けにはまだ書けません。書くなら、名前で再開されるまで止まる suspend のステップにすることになります",
            ),
            Platform::Graph => (
                "dandori does not write it for pydantic-graph yet; there, the value would come to `Deps` by the task's name",
                "pydantic-graph 向けにはまだ書けません。書くなら、値はタスクの名前で `Deps` に届くことになります",
            ),
        };
        self.tasks
            .iter()
            .filter(|t| t.event)
            .map(|t| crate::diag::Diag::error("E050", t.line, 1, format!("`{}` waits for an event sent to the workflow by name; {en}", t.name), format!("`{}` はワークフローに名前で送られてくるイベントを待ちます。{ja}", t.name)))
            .collect()
    }

    pub fn ty_name(&self, t: &Ty) -> String {
        match t {
            Ty::Int => "int".into(),
            Ty::Str => "string".into(),
            Ty::Bool => "bool".into(),
            Ty::Timestamp => "timestamp".into(),
            Ty::Num(u) => u.clone(),
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
