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
    pub origin: RecordOrigin,
}

#[derive(Clone, Debug)]
pub struct RuleUse {
    pub name: String,
    pub info: RuleInfo,
    pub lambda: Option<String>,
    pub outputs: RecordId,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Binding {
    Lambda(String),
    Http { method: String, url: String, form: bool },
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

#[derive(Clone, Debug)]
pub struct TaskDef {
    pub name: String,
    pub params: Vec<(String, Ty)>,
    pub result: Ty,
    pub binding: Option<Binding>,
    pub connection: Option<String>,
    pub errors: Vec<(String, Option<u16>)>,
    pub retry: Option<Retry>,
    pub timeout: Option<u64>,
    pub key: bool,
    pub idempotent: bool,
    pub machine: Option<TaskMachine>,
    pub refused_as: Option<String>,
    pub callback: bool,
    pub line: usize,
}

impl TaskDef {
    /// A task that changes something on the other side, so that doing it twice is not the
    /// same as doing it once, unless the task says it is.
    pub fn changes_things(&self) -> bool {
        !self.idempotent && !matches!(self.machine, Some(TaskMachine::Observes))
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
    /// the arm for a case that has not been started
    pub none: bool,
    pub body: Vec<TStmt>,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TExpr {
    Var { name: String, fields: Vec<String>, ty: Ty },
    Str(String),
    Int(i64),
    Bool(bool),
    Enum(String, EnumId),
}

impl TExpr {
    pub fn ty(&self) -> Ty {
        match self {
            TExpr::Var { ty, .. } => ty.clone(),
            TExpr::Str(_) => Ty::Str,
            TExpr::Int(_) => Ty::Int,
            TExpr::Bool(_) => Ty::Bool,
            TExpr::Enum(_, e) => Ty::Enum(*e),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TK {
    Call { target: Option<Target>, callee: Callee, args: Vec<(String, TExpr)>, handlers: Vec<THandler> },
    Match { expr: TExpr, arms: Vec<TArm> },
    Wait { seconds: u64 },
    /// until a moment given as an RFC 3339 string
    WaitUntil { at: TExpr },
    Repeat { times: u32, body: Vec<TStmt> },
    Break,
    Pass,
    Succeed { fields: Vec<(String, TExpr)> },
    Fail { error: String, cause: Option<String>, leaving: Vec<usize> },
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
    pub tasks: Vec<TaskDef>,
    pub cases: Vec<CaseDef>,
    /// every variable: the inputs, the `let`s and the cases, with its type
    pub vars: Vec<(String, Ty)>,
    pub flow: Vec<TStmt>,
    pub on_failure: Option<Vec<TStmt>>,
    /// for a call on a case: the states its answer may carry, which the generated code checks
    pub monitors: BTreeMap<usize, (usize, Vec<String>)>,
}

impl Model {
    pub fn ty_name(&self, t: &Ty) -> String {
        match t {
            Ty::Int => "int".into(),
            Ty::Str => "string".into(),
            Ty::Bool => "bool".into(),
            Ty::Timestamp => "timestamp".into(),
            Ty::Num(u) => u.clone(),
            Ty::Enum(e) => self.enums[*e].name.clone(),
            Ty::Record(r) => self.records[*r].name.clone(),
        }
    }

    pub fn var_ty(&self, name: &str) -> Option<&Ty> {
        self.vars.iter().find(|(n, _)| n == name).map(|(_, t)| t)
    }

    pub fn field_ty(&self, rec: RecordId, field: &str) -> Option<&Ty> {
        self.records[rec].fields.iter().find(|(n, _)| n == field).map(|(_, t)| t)
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
                TK::Repeat { body, .. } => Model::walk(body, f),
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
        out
    }
}
