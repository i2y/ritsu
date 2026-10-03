//! The surface syntax: physical lines with their indentation, the tokens on each line,
//! and the tree the parser builds. Blocks are marked by indentation alone.

use crate::diag::{Diag, Text};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Str(String),
    Int(i64),
    Float(f64),
    Sym(&'static str),
    /// A bound of a range with its unit, as rulec writes one: `1kg`, `1.5%`, `100万円`. The value
    /// is exact, `万` and `億` multiplied in.
    Quantity { value: ritsu_units::Rat, unit: String, raw: String },
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

#[derive(Clone, Debug)]
struct Line {
    indent: usize,
    toks: Vec<Token>,
    line: usize,
    end_col: usize,
}

// ---------------------------------------------------------------------------
// The tree

pub type Name = (String, Span);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Standard,
    Express,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub name: Option<Name>,
    pub version: u32,
    /// `implements <api>.<Service>` on the `workflow` line: the service of a `.proto` the workflow implements
    pub implements: Option<Vec<Name>>,
    pub description: Option<String>,
    pub kind: Kind,
    pub uses: Vec<UseRule>,
    /// `use openapi|smithy|proto <name> from "<path>"`: the descriptions of the APIs the tasks call
    pub apis: Vec<UseApi>,
    pub enums: Vec<EnumDecl>,
    pub records: Vec<RecordDecl>,
    pub inputs: Vec<Field>,
    pub outputs: Vec<Field>,
    pub tasks: Vec<TaskDecl>,
    pub cases: Vec<CaseDecl>,
    pub flow: Option<(Block, Span)>,
    pub on_failure: Option<(Block, Span)>,
    /// what runs when the workflow is asked to stop (a cancellation)
    pub on_cancel: Option<(Block, Span)>,
}

#[derive(Clone, Debug)]
pub struct UseRule {
    pub name: Name,
    pub path: String,
    /// `lambda "<function>"`, where it was written
    pub lambda: Option<(String, Span)>,
    /// Temporal: the rule is called as a local activity
    pub local: bool,
    /// `connect "<url>"`: the rule's Connect service is at this URL, and every platform calls it there
    pub connect: Option<(String, Span)>,
    /// `connection "<EventBridge connection>"`: how Step Functions' HTTP Task reaches the service
    pub connection: Option<(String, Span)>,
}

#[derive(Clone, Debug)]
pub struct UseApi {
    pub kind: crate::apis::ApiKind,
    pub name: Name,
    pub path: String,
    /// `url "<base>"`: where the API is, in place of an OpenAPI document's server (a `.proto` needs it)
    pub url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct EnumDecl {
    pub name: Name,
    pub values: Vec<Name>,
}

#[derive(Clone, Debug)]
pub struct RecordDecl {
    pub name: Name,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: Name,
    pub ty: TypeExpr,
    pub range: Option<RangeDecl>,
}

/// `range >=1 <=30`, `range >=1kg`: the whole numbers a value may be, both ends included, in the
/// unit of its type. Either end can be left out.
#[derive(Clone, Debug, PartialEq)]
pub struct RangeDecl {
    pub lo: Option<Bound>,
    pub hi: Option<Bound>,
    pub span: Span,
}

/// One end of a range, as written.
#[derive(Clone, Debug, PartialEq)]
pub enum Bound {
    /// A whole number in the unit of the type: `30`.
    Int(i64),
    /// A number with a unit, as rulec writes a bound: `1kg`, `1.5%`, `100万円`. lower counts it in
    /// the unit of the type, and refuses one that does not come to a whole number of it.
    Quantity { value: ritsu_units::Rat, unit: String, raw: String, span: Span },
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeExpr {
    Int(Span),
    Str(Span),
    Bool(Span),
    Timestamp(Span),
    /// any JSON value, passed along without being looked into
    Json(Span),
    /// `Name` or `rule.Name`
    Named(Vec<Name>),
    /// `money[円, incl_tax]`: a number with a unit, spelled as rulec spells it
    Unit(String, Span),
    /// `list[T]`
    List(Box<TypeExpr>, Span),
    /// `T?`: a value that may be absent
    Opt(Box<TypeExpr>, Span),
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Int(s) | TypeExpr::Str(s) | TypeExpr::Bool(s) | TypeExpr::Timestamp(s) | TypeExpr::Json(s) | TypeExpr::Unit(_, s) | TypeExpr::List(_, s) | TypeExpr::Opt(_, s) => *s,
            TypeExpr::Named(v) => v[0].1,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Binding {
    Lambda(String),
    /// `http POST "<url>"`, or `http POST stripe "/v1/…"`: an operation of an OpenAPI document,
    /// whose server and path make the URL
    Http { method: String, url: String, form: bool, api: Option<Name> },
    /// `connect warehouse "StockService/Reserve"`: a method of a `.proto`'s service, called by
    /// the Connect protocol with JSON
    Connect { api: Name, method: String },
    /// an AWS API call, as Step Functions' AWS SDK integrations name it: `sns:publish`
    Aws { service: String, action: String },
    /// an agent that reads the arguments and answers in the task's type, told what to do by
    /// `instructions`; `agent claude "…"` names whose models it runs on (OpenAI's when it does not)
    Agent { provider: Option<Name>, instructions: String },
    /// Jev, TypeSafe's System One model, asked what the answer's type asks: `jev "…"` one
    /// question, whose answer is the task's; `jev` alone a question for each field of a record
    Jev(JevDecl),
}

/// What a `jev` clause asks. A question whose answer is the task's own has `ask`; one for each
/// field of a record answer is in `fields`.
#[derive(Clone, Debug)]
pub struct JevDecl {
    pub ask: Option<JevAsk>,
    pub fields: Vec<(Name, JevField)>,
}

/// One question: its instructions, whether it is a `score` (a place on a scale of levels) rather
/// than a choice, and under it each value with what it means: `returns "…"`, `true "…"`.
#[derive(Clone, Debug)]
pub struct JevAsk {
    pub instructions: String,
    pub score: Option<Span>,
    pub criteria: Vec<(Name, String)>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum JevField {
    Ask(JevAsk),
    /// `sure confidence of kind`: how sure Jev is of the answer to the field `kind`
    Confidence(Name),
}

#[derive(Clone, Debug)]
pub struct ErrDecl {
    pub name: Name,
    /// `card_declined = 402`: the HTTP status it comes back with
    pub status: Option<u16>,
    /// `在庫切れ = ConditionalCheckFailedException`: the AWS API's exception
    pub exception: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RetryDecl {
    pub times: u32,
    pub every: u64,
    pub backoff: f64,
    pub on: Vec<Name>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum MachineUse {
    Starts { machine: Vec<Name>, then: Vec<Name> },
    Sends { event: Name, column: Option<Name> },
    Observes,
}

#[derive(Clone, Debug)]
pub struct TaskDecl {
    pub name: Name,
    pub params: Vec<Field>,
    /// None: the task answers nothing the workflow reads
    pub result: Option<TypeExpr>,
    /// `-> int range >=0 <=100`
    pub result_range: Option<RangeDecl>,
    pub binding: Option<(Binding, Span)>,
    /// the model an `agent` task runs on
    pub model: Option<(String, Span)>,
    /// `url "<base>"`: the server an agent's Open Responses API is on, in place of OpenAI's
    pub url: Option<(String, Span)>,
    /// `effort low`: how hard an agent's model reasons, as its provider names the levels
    pub effort: Option<(String, Span)>,
    /// `confidence 0.8 else unsure`: a Jev answer less sure than this fails the call with the error
    pub confidence: Option<(f64, Name, Span)>,
    pub connection: Option<String>,
    /// Temporal: the task queue of the activity or the child workflow
    pub queue: Option<String>,
    /// Temporal: a child workflow of this type
    pub workflow: Option<(String, Span)>,
    /// Step Functions: a state machine started as a nested execution
    pub state_machine: Option<(String, Span)>,
    /// Lambda durable functions: a function invoked by the durable execution
    pub durable_function: Option<(String, Span)>,
    /// Argo Workflows: the container image that runs the task
    pub image: Option<(String, Span)>,
    /// Argo Workflows: a WorkflowTemplate run as a workflow of its own
    pub argo_template: Option<(String, Span)>,
    pub errors: Vec<ErrDecl>,
    pub retry: Option<RetryDecl>,
    pub timeout: Option<u64>,
    pub key: Option<Span>,
    /// `key ClientToken`: the parameter of an AWS API that takes the key
    pub key_param: Option<Name>,
    pub idempotent: bool,
    pub machine: Option<(MachineUse, Span)>,
    pub refused_as: Option<Name>,
    pub callback: Option<Span>,
    /// Temporal: the task calls nothing, and waits for a value sent to the workflow by name
    pub event: Option<Span>,
    /// `flow "<path>"`: the child workflow is another `.flow`, whose inputs, outputs and
    /// errors the task is held to
    pub flow: Option<(String, Span)>,
}

#[derive(Clone, Debug)]
pub struct CaseDecl {
    pub name: Name,
    pub record: TypeExpr,
    pub machine: Vec<Name>,
    pub held: Vec<(Name, Name)>,
    pub external: Vec<Name>,
    pub state_field: Option<Name>,
    /// `refused when <output> = <value>`
    pub refused_when: Option<(Name, Name)>,
}

pub type Block = Vec<Stmt>;

#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    /// `let x = t(…)`
    Let { name: Name, ty: Option<TypeExpr>, call: Call, handlers: Vec<Handler> },
    /// `let x = <value>`
    Assign { name: Name, ty: Option<TypeExpr>, expr: Expr },
    /// `t(…)`, when the answer is not kept
    Call { call: Call, handlers: Vec<Handler> },
    CaseCall { case: Name, call: Call, handlers: Vec<Handler> },
    Match { expr: Expr, arms: Vec<Arm> },
    Wait { seconds: u64 },
    WaitUntil { at: Expr },
    Repeat { times: u32, body: Block },
    /// `[let r =] for x in xs at most n [in parallel[, k at a time]]`
    For { var: Name, list: Expr, max: u32, parallel: Option<u32>, body: Block, result: Option<(Name, Option<TypeExpr>)> },
    Yield { expr: Expr },
    Break,
    Pass,
    Succeed { fields: Vec<(Name, Expr)> },
    Fail { error: Name, cause: Option<Expr>, leaving: Vec<Name> },
}

#[derive(Clone, Debug)]
pub struct Call {
    pub callee: Name,
    pub args: Vec<(Name, Expr)>,
}

#[derive(Clone, Debug)]
pub struct Handler {
    pub errors: Vec<Name>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Arm {
    pub values: Vec<Name>,
    /// `some x =>`: the value, when there is one, as `x`
    pub some: Option<Name>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Lit(String),
    Hole(Vec<Name>),
}

#[derive(Clone, Debug)]
pub enum Expr {
    Path(Vec<Name>),
    Str(String, Span),
    /// a string with values put in: `"注文 {受注.id} を出荷しました"`
    Interp(Vec<Part>, Span),
    Int(i64, Span),
    Bool(bool, Span),
    /// `{宛先: 客.メール, 本文: "…"}`
    Record(Vec<(Name, Expr)>, Span),
    /// `[a, b]`
    List(Vec<Expr>, Span),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Path(p) => p[0].1,
            Expr::Str(_, s) | Expr::Interp(_, s) | Expr::Int(_, s) | Expr::Bool(_, s) | Expr::Record(_, s) | Expr::List(_, s) => *s,
        }
    }
}

// ---------------------------------------------------------------------------
// Lexing

fn err(span: Span, message: Text) -> Diag {
    Diag::error("E001", span.line, span.col, message)
}

/// The exact value of a number as written (`-1.5`, digits only after `_` are taken out), times
/// `mult`; None when it does not fit 64 bits before the multiplier, or has more than nine places.
fn quantity(text: &str, mult: i128) -> Option<ritsu_units::Rat> {
    let (neg, digits) = match text.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, text),
    };
    let (whole, frac) = digits.split_once('.').unwrap_or((digits, ""));
    if frac.len() > 9 {
        return None;
    }
    let n: i64 = format!("{whole}{frac}").parse().ok()?;
    let n = i128::from(n).checked_mul(mult)?;
    ritsu_units::Rat::checked_new(if neg { -n } else { n }, 10i128.pow(frac.len() as u32))
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The two units of temperature, which are no letters: a word of their own right after `[`
/// (`temperature[℃]`), and the unit of a bound (`range >=41℉`), but never part of a name, which
/// the code dandori writes could not spell.
fn is_temperature(c: char) -> bool {
    c == '℃' || c == '℉'
}

fn lex(src: &str) -> Result<Vec<Line>, Diag> {
    let mut out = Vec::new();
    for (i, raw) in src.lines().enumerate() {
        let line = i + 1;
        let chars: Vec<char> = raw.chars().collect();
        let mut j = 0;
        let mut indent = 0;
        while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
            if chars[j] == '\t' {
                return Err(err(
                    Span { line, col: j + 1 },
                    tr!("インデントにはタブではなく空白を使ってください", "indent with spaces, not tabs"),
                ));
            }
            indent += 1;
            j += 1;
        }
        let mut toks = Vec::new();
        while j < chars.len() {
            let c = chars[j];
            let span = Span { line, col: j + 1 };
            if c == ' ' || c == '\t' {
                j += 1;
                continue;
            }
            if c == '#' {
                break;
            }
            if c == '"' {
                let mut s = String::new();
                j += 1;
                let mut closed = false;
                while j < chars.len() {
                    let d = chars[j];
                    if d == '\\' && j + 1 < chars.len() {
                        let e = chars[j + 1];
                        s.push(match e {
                            'n' => '\n',
                            't' => '\t',
                            other => other,
                        });
                        j += 2;
                        continue;
                    }
                    if d == '"' {
                        closed = true;
                        j += 1;
                        break;
                    }
                    s.push(d);
                    j += 1;
                }
                if !closed {
                    return Err(err(span, tr!("文字列が閉じていません", "this string is not closed")));
                }
                toks.push(Token { tok: Tok::Str(s), span });
                continue;
            }
            // a bound of a range: `>=-5`, and a number glued to a unit is told apart below
            let bound = matches!(toks.last(), Some(Token { tok: Tok::Sym(">=" | "<="), .. }));
            if c.is_ascii_digit() || (c == '-' && bound && chars.get(j + 1).is_some_and(|d| d.is_ascii_digit())) {
                let start = j;
                j += 1;
                while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '_') {
                    j += 1;
                }
                let mut is_float = false;
                if j + 1 < chars.len() && chars[j] == '.' && chars[j + 1].is_ascii_digit() {
                    is_float = true;
                    j += 1;
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                }
                let text: String = chars[start..j].iter().filter(|c| **c != '_').collect();
                // a bound with its unit, as rulec writes one: `>=1kg`, `<=100万円`, `<=50%`
                if bound && j < chars.len() && (is_ident_start(chars[j]) || is_temperature(chars[j]) || chars[j] == '%') {
                    let mut k = j;
                    let mult: i128 = match chars[k] {
                        '万' => 10_000,
                        '億' => 100_000_000,
                        _ => 1,
                    };
                    if mult > 1 {
                        k += 1;
                    }
                    let unit: String = match chars.get(k) {
                        Some('%') => "%".to_string(),
                        Some(c) if is_temperature(*c) => c.to_string(),
                        _ => chars[k..].iter().take_while(|c| is_ident_continue(**c)).collect(),
                    };
                    let end = k + unit.chars().count();
                    let raw: String = chars[start..end].iter().collect();
                    if unit.is_empty() {
                        return Err(err(span, tr!("`{raw}` のあとに単位を書きます", "a unit is expected after `{raw}`")));
                    }
                    let Some(value) = quantity(&text, mult) else {
                        return Err(err(span, tr!("数が大きすぎます", "this number is too large")));
                    };
                    toks.push(Token { tok: Tok::Quantity { value, unit, raw }, span });
                    j = end;
                    continue;
                }
                if j < chars.len() && is_ident_start(chars[j]) {
                    return Err(err(span, tr!("数と語のあいだに空白を入れてください", "put a space between a number and a word")));
                }
                if is_float {
                    toks.push(Token { tok: Tok::Float(text.parse().unwrap_or(0.0)), span });
                } else {
                    match text.parse::<i64>() {
                        Ok(n) => toks.push(Token { tok: Tok::Int(n), span }),
                        Err(_) => return Err(err(span, tr!("数が大きすぎます", "this number is too large"))),
                    }
                }
                continue;
            }
            if is_ident_start(c) {
                let start = j;
                while j < chars.len() && is_ident_continue(chars[j]) {
                    j += 1;
                }
                toks.push(Token { tok: Tok::Ident(chars[start..j].iter().collect()), span });
                continue;
            }
            if is_temperature(c) && matches!(toks.last(), Some(Token { tok: Tok::Sym("["), .. })) {
                toks.push(Token { tok: Tok::Ident(c.to_string()), span });
                j += 1;
                continue;
            }
            let two: String = chars[j..(j + 2).min(chars.len())].iter().collect();
            let sym2 = match two.as_str() {
                "->" => Some("->"),
                "<-" => Some("<-"),
                "=>" => Some("=>"),
                ">=" => Some(">="),
                "<=" => Some("<="),
                _ => None,
            };
            if let Some(s) = sym2 {
                toks.push(Token { tok: Tok::Sym(s), span });
                j += 2;
                continue;
            }
            let sym1 = match c {
                '(' => Some("("),
                ')' => Some(")"),
                ':' => Some(":"),
                ',' => Some(","),
                '.' => Some("."),
                '=' => Some("="),
                '|' => Some("|"),
                '[' => Some("["),
                ']' => Some("]"),
                '{' => Some("{"),
                '}' => Some("}"),
                '?' => Some("?"),
                // in a rate's step: rate[step 0.01%]
                '%' => Some("%"),
                _ => None,
            };
            if let Some(s) = sym1 {
                toks.push(Token { tok: Tok::Sym(s), span });
                j += 1;
                continue;
            }
            return Err(err(span, tr!("ここに `{c}` は書けません", "unexpected character `{c}`")));
        }
        if !toks.is_empty() {
            out.push(Line { indent, toks, line, end_col: chars.len() + 1 });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// A cursor over the tokens of one line

struct Cur<'a> {
    toks: &'a [Token],
    i: usize,
    line: usize,
    end_col: usize,
}

impl<'a> Cur<'a> {
    fn new(l: &'a Line) -> Cur<'a> {
        Cur { toks: &l.toks, i: 0, line: l.line, end_col: l.end_col }
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i).map(|t| &t.tok)
    }

    fn peek_at(&self, k: usize) -> Option<&Tok> {
        self.toks.get(self.i + k).map(|t| &t.tok)
    }

    fn span(&self) -> Span {
        self.toks.get(self.i).map(|t| t.span).unwrap_or(Span { line: self.line, col: self.end_col })
    }

    fn at_end(&self) -> bool {
        self.i >= self.toks.len()
    }

    fn is_sym(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Sym(x)) if *x == s)
    }

    fn is_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(x)) if x == kw)
    }

    fn eat_sym(&mut self, s: &str) -> bool {
        if self.is_sym(s) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.is_kw(kw) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect_sym(&mut self, s: &str) -> Result<Span, Diag> {
        let sp = self.span();
        if self.eat_sym(s) {
            Ok(sp)
        } else {
            Err(err(sp, tr!("ここには `{s}` が要ります", "expected `{s}` here")))
        }
    }

    fn expect_kw(&mut self, kw: &str) -> Result<Span, Diag> {
        let sp = self.span();
        if self.eat_kw(kw) {
            Ok(sp)
        } else {
            Err(err(sp, tr!("ここには `{kw}` が要ります", "expected `{kw}` here")))
        }
    }

    fn ident(&mut self, what: Text) -> Result<Name, Diag> {
        let sp = self.span();
        match self.peek() {
            Some(Tok::Ident(s)) => {
                let s = s.clone();
                self.i += 1;
                Ok((s, sp))
            }
            _ => Err(err(sp, tr!("ここには{}が要ります", "expected {} here", what.ja; what.en))),
        }
    }

    fn string(&mut self, what: Text) -> Result<(String, Span), Diag> {
        let sp = self.span();
        match self.peek() {
            Some(Tok::Str(s)) => {
                let s = s.clone();
                self.i += 1;
                Ok((s, sp))
            }
            _ => Err(err(sp, tr!("ここには{}を二重引用符で書きます", "expected {} in double quotes here", what.ja; what.en))),
        }
    }

    fn int(&mut self, what: Text) -> Result<(i64, Span), Diag> {
        let sp = self.span();
        match self.peek() {
            Some(Tok::Int(n)) => {
                let n = *n;
                self.i += 1;
                Ok((n, sp))
            }
            _ => Err(err(sp, tr!("ここには{}が要ります", "expected {} here", what.ja; what.en))),
        }
    }

    fn expect_end(&self) -> Result<(), Diag> {
        if self.at_end() {
            Ok(())
        } else {
            Err(err(self.span(), tr!("行の終わりに余計なものがあります", "unexpected text at the end of the line")))
        }
    }

    fn names(&mut self, what: Text) -> Result<Vec<Name>, Diag> {
        let mut v = vec![self.ident(what.clone())?];
        while self.eat_sym(",") {
            v.push(self.ident(what.clone())?);
        }
        Ok(v)
    }

    fn qualname(&mut self) -> Result<Vec<Name>, Diag> {
        let mut v = vec![self.ident(tr!("名前", "a name"))?];
        while self.eat_sym(".") {
            v.push(self.ident(tr!("名前", "a name"))?);
        }
        Ok(v)
    }
}

// ---------------------------------------------------------------------------
// Parsing

struct Parser {
    lines: Vec<Line>,
    pos: usize,
}

const KEYWORDS: &[&str] = &[
    "workflow", "implements", "description", "kind", "use", "rule", "from", "enum", "record", "inputs", "outputs", "task", "case",
    "follows", "flow", "on", "let", "match", "wait", "repeat", "break", "succeed", "fail", "leaving", "lambda", "http",
    "aws", "connection", "queue", "machine", "durable", "function", "image", "template", "errors", "retry", "timeout", "key", "idempotent",
    "starts", "sends", "observes", "refused", "callback", "held", "external", "state", "then", "true", "false", "until",
    "pass", "for", "in", "at", "most", "parallel", "yield", "some", "none", "list", "json", "range",
    "openapi", "smithy", "proto", "connect", "url",
];

pub fn is_keyword(s: &str) -> bool {
    KEYWORDS.contains(&s)
}

fn duration(cur: &mut Cur) -> Result<u64, Diag> {
    let (n, sp) = cur.int(tr!("数", "a number"))?;
    if n < 0 {
        return Err(err(sp, tr!("時間は負にできません", "a duration cannot be negative")));
    }
    let unit_sp = cur.span();
    let (u, _) = cur.ident(tr!("単位（seconds・minutes・hours・days）", "a unit (seconds, minutes, hours or days)"))?;
    let mult = match u.as_str() {
        "second" | "seconds" => 1,
        "minute" | "minutes" => 60,
        "hour" | "hours" => 3600,
        "day" | "days" => 86400,
        _ => {
            return Err(err(
                unit_sp,
                tr!("`{u}` は時間の単位ではありません。seconds・minutes・hours・days のどれかを書きます", "`{u}` is not a unit of time; write seconds, minutes, hours or days"),
            ))
        }
    };
    Ok(n as u64 * mult)
}

fn type_expr(cur: &mut Cur) -> Result<TypeExpr, Diag> {
    let base = type_base(cur)?;
    if cur.is_sym("?") {
        let sp = cur.span();
        cur.i += 1;
        if cur.is_sym("?") {
            return Err(err(cur.span(), tr!("`?` は一つだけ書きます", "a type is made optional once; write one `?`")));
        }
        return Ok(TypeExpr::Opt(Box::new(base), sp));
    }
    Ok(base)
}

fn type_base(cur: &mut Cur) -> Result<TypeExpr, Diag> {
    let sp = cur.span();
    let first = cur.ident(tr!("型", "a type"))?;
    match first.0.as_str() {
        "int" => return Ok(TypeExpr::Int(sp)),
        "string" => return Ok(TypeExpr::Str(sp)),
        "bool" => return Ok(TypeExpr::Bool(sp)),
        "timestamp" => return Ok(TypeExpr::Timestamp(sp)),
        "json" => return Ok(TypeExpr::Json(sp)),
        "list" if cur.is_sym("[") => {
            cur.i += 1;
            let inner = type_expr(cur)?;
            if !cur.eat_sym("]") {
                return Err(err(cur.span(), tr!("リストの型が `]` で閉じていません", "this list type is not closed with `]`")));
            }
            return Ok(TypeExpr::List(Box::new(inner), sp));
        }
        _ => {}
    }
    if cur.is_sym("[") {
        // a unit, spelled as rulec spells it: money[円, incl_tax], rate[step 0.01%]
        cur.i += 1;
        let mut parts: Vec<String> = Vec::new();
        let mut part = String::new();
        loop {
            match cur.peek() {
                Some(Tok::Sym("]")) => {
                    cur.i += 1;
                    break;
                }
                Some(Tok::Ident(s)) => {
                    let s = s.clone();
                    if !part.is_empty() {
                        part.push(' ');
                    }
                    part.push_str(&s);
                    cur.i += 1;
                }
                Some(Tok::Int(n)) => {
                    let n = *n;
                    if !part.is_empty() {
                        part.push(' ');
                    }
                    part.push_str(&n.to_string());
                    cur.i += 1;
                }
                Some(Tok::Float(f)) => {
                    let f = *f;
                    if !part.is_empty() {
                        part.push(' ');
                    }
                    part.push_str(&f.to_string());
                    cur.i += 1;
                }
                Some(Tok::Sym("%")) => {
                    part.push('%');
                    cur.i += 1;
                }
                Some(Tok::Sym(",")) => {
                    parts.push(std::mem::take(&mut part));
                    cur.i += 1;
                }
                _ => return Err(err(cur.span(), tr!("単位が `]` で閉じていません", "this unit is not closed with `]`"))),
            }
        }
        if !part.is_empty() {
            parts.push(part);
        }
        return Ok(TypeExpr::Unit(format!("{}[{}]", first.0, parts.join(", ")), sp));
    }
    let mut v = vec![first];
    while cur.eat_sym(".") {
        v.push(cur.ident(tr!("名前", "a name"))?);
    }
    Ok(TypeExpr::Named(v))
}

fn field(cur: &mut Cur) -> Result<Field, Diag> {
    let name = cur.ident(tr!("フィールドの名前", "a field name"))?;
    cur.expect_sym(":")?;
    let ty = type_expr(cur)?;
    let range = range_decl(cur)?;
    Ok(Field { name, ty, range })
}

/// `range >=1 <=30`, `range >=0`, `range <=100`, `range >=1kg`, after a type.
fn range_decl(cur: &mut Cur) -> Result<Option<RangeDecl>, Diag> {
    let span = cur.span();
    if !cur.eat_kw("range") {
        return Ok(None);
    }
    let (mut lo, mut hi) = (None, None);
    loop {
        let end = if cur.is_sym(">=") && lo.is_none() {
            &mut lo
        } else if cur.is_sym("<=") && hi.is_none() {
            &mut hi
        } else {
            break;
        };
        cur.i += 1;
        let at = cur.span();
        *end = Some(match cur.peek() {
            Some(Tok::Quantity { value, unit, raw }) => {
                let b = Bound::Quantity { value: *value, unit: unit.clone(), raw: raw.clone(), span: at };
                cur.i += 1;
                b
            }
            _ => Bound::Int(cur.int(tr!("整数", "a whole number"))?.0),
        });
    }
    if lo.is_none() && hi.is_none() {
        return Err(err(cur.span(), tr!("範囲は `range >=<下限> <=<上限>` と書きます。端は片方だけでもかまいません", "a range is written `range >=<low> <=<high>`, with one end or both")));
    }
    Ok(Some(RangeDecl { lo, hi, span }))
}

fn expr(cur: &mut Cur) -> Result<Expr, Diag> {
    let sp = cur.span();
    match cur.peek().cloned() {
        Some(Tok::Str(s)) => {
            cur.i += 1;
            string_expr(&s, sp)
        }
        Some(Tok::Int(n)) => {
            cur.i += 1;
            Ok(Expr::Int(n, sp))
        }
        Some(Tok::Ident(s)) if s == "true" || s == "false" => {
            cur.i += 1;
            Ok(Expr::Bool(s == "true", sp))
        }
        Some(Tok::Ident(_)) => Ok(Expr::Path(cur.qualname()?)),
        Some(Tok::Sym("{")) => {
            cur.i += 1;
            let mut fields = Vec::new();
            if !cur.eat_sym("}") {
                loop {
                    let n = cur.ident(tr!("フィールドの名前", "a field name"))?;
                    cur.expect_sym(":")?;
                    let e = expr(cur)?;
                    fields.push((n, e));
                    if cur.eat_sym("}") {
                        break;
                    }
                    cur.expect_sym(",")?;
                }
            }
            Ok(Expr::Record(fields, sp))
        }
        Some(Tok::Sym("[")) => {
            cur.i += 1;
            let mut items = Vec::new();
            if !cur.eat_sym("]") {
                loop {
                    items.push(expr(cur)?);
                    if cur.eat_sym("]") {
                        break;
                    }
                    cur.expect_sym(",")?;
                }
            }
            Ok(Expr::List(items, sp))
        }
        _ => Err(err(sp, tr!("ここには値が要ります", "expected a value here"))),
    }
}

/// A string, with `{path}` put in where it is written; `{{` and `}}` are the braces themselves.
fn string_expr(s: &str, sp: Span) -> Result<Expr, Diag> {
    let chars: Vec<char> = s.chars().collect();
    let mut parts = Vec::new();
    let mut lit = String::new();
    let mut holes = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '{' {
            if chars.get(i + 1) == Some(&'{') {
                lit.push('{');
                i += 2;
                continue;
            }
            let start = i + 1;
            let mut j = start;
            while j < chars.len() && chars[j] != '}' {
                j += 1;
            }
            if j >= chars.len() {
                return Err(err(sp, tr!("文字列の中の `{{` が閉じていません。波括弧そのものは `{{{{` と書きます", "a `{{` in this string is not closed; write `{{{{` for a brace itself")));
            }
            let inner: String = chars[start..j].iter().collect();
            let mut path = Vec::new();
            for part in inner.split('.') {
                let p = part.trim();
                let ok = p.chars().next().map(is_ident_start).unwrap_or(false) && p.chars().all(is_ident_continue);
                if !ok {
                    return Err(err(
                        sp,
                        tr!("文字列の中の `{{{inner}}}` は変数やそのフィールドではありません。波括弧そのものは `{{{{` と書きます", "`{{{inner}}}` in this string is not a variable or a field of one; write `{{{{` for a brace itself"),
                    ));
                }
                path.push((p.to_string(), sp));
            }
            if !lit.is_empty() {
                parts.push(Part::Lit(std::mem::take(&mut lit)));
            }
            parts.push(Part::Hole(path));
            holes = true;
            i = j + 1;
            continue;
        }
        if c == '}' {
            if chars.get(i + 1) == Some(&'}') {
                lit.push('}');
                i += 2;
                continue;
            }
            return Err(err(sp, tr!("文字列の中の `}}` に対応する `{{` がありません。波括弧そのものは `}}}}` と書きます", "a `}}` in this string has no `{{`; write `}}}}` for a brace itself")));
        }
        lit.push(c);
        i += 1;
    }
    if !holes {
        return Ok(Expr::Str(lit, sp));
    }
    if !lit.is_empty() {
        parts.push(Part::Lit(lit));
    }
    Ok(Expr::Interp(parts, sp))
}

/// `for x in xs at most n [in parallel[, k at a time]]`, from `for` on.
fn for_header(cur: &mut Cur) -> Result<(Name, Expr, u32, Option<u32>), Diag> {
    cur.expect_kw("for")?;
    let var = cur.ident(tr!("変数の名前", "a variable name"))?;
    cur.expect_kw("in")?;
    let list = expr(cur)?;
    cur.expect_kw("at")?;
    cur.expect_kw("most")?;
    let (n, nsp) = cur.int(tr!("リストの要素の数の上限", "the most items the list can have"))?;
    if n < 1 {
        return Err(err(nsp, tr!("リストの要素の数の上限を 1 以上で書いてください", "write the most items the list can have, 1 or more")));
    }
    let mut parallel = None;
    if cur.eat_kw("in") {
        cur.expect_kw("parallel")?;
        let mut k = 0;
        if cur.eat_sym(",") {
            let (kk, ksp) = cur.int(tr!("同時に回す数", "how many rounds run at a time"))?;
            if kk < 1 {
                return Err(err(ksp, tr!("同時に回す数は 1 以上です", "run at least one round at a time")));
            }
            cur.expect_kw("at")?;
            cur.expect_kw("a")?;
            cur.expect_kw("time")?;
            k = kk as u32;
        }
        parallel = Some(k);
    }
    cur.expect_end()?;
    Ok((var, list, n as u32, parallel))
}

fn call(cur: &mut Cur) -> Result<Call, Diag> {
    let callee = cur.ident(tr!("タスクか規則の名前", "the name of a task or a rule"))?;
    cur.expect_sym("(")?;
    let mut args = Vec::new();
    if !cur.eat_sym(")") {
        loop {
            let name = cur.ident(tr!("引数の名前", "an argument name"))?;
            cur.expect_sym(":")?;
            let e = expr(cur)?;
            args.push((name, e));
            if cur.eat_sym(")") {
                break;
            }
            cur.expect_sym(",")?;
        }
    }
    Ok(Call { callee, args })
}

impl Parser {
    fn cur_line(&self) -> Option<&Line> {
        self.lines.get(self.pos)
    }

    /// The lines indented deeper than `parent`, parsed as one block. All of them must share
    /// the indentation of the first.
    fn child_block(&mut self, parent: usize, what: Text, at: Span) -> Result<Block, Diag> {
        let indent = match self.cur_line() {
            Some(l) if l.indent > parent => l.indent,
            _ => {
                return Err(err(
                    at,
                    tr!("{}の下に、字下げしたブロックが要ります", "{} needs an indented block under it", what.ja; what.en),
                ))
            }
        };
        self.block(indent)
    }

    fn block(&mut self, indent: usize) -> Result<Block, Diag> {
        let mut out = Vec::new();
        while let Some(l) = self.cur_line() {
            if l.indent < indent {
                break;
            }
            if l.indent > indent {
                let sp = l.toks[0].span;
                return Err(err(sp, tr!("この行は前の行より深く字下げされています", "this line is indented more than the lines before it")));
            }
            out.push(self.stmt(indent)?);
        }
        Ok(out)
    }

    fn handlers(&mut self, indent: usize) -> Result<Vec<Handler>, Diag> {
        let mut out = Vec::new();
        let child = match self.cur_line() {
            Some(l) if l.indent > indent => l.indent,
            _ => return Ok(out),
        };
        while let Some(l) = self.cur_line() {
            if l.indent != child {
                if l.indent > child {
                    let sp = l.toks[0].span;
                    return Err(err(sp, tr!("この行は前の行より深く字下げされています", "this line is indented more than the lines before it")));
                }
                break;
            }
            let l = l.clone();
            let mut cur = Cur::new(&l);
            let sp = cur.span();
            if !cur.eat_kw("on") {
                return Err(err(
                    sp,
                    tr!("呼び出しの下に書けるのは `on <エラー> =>` の行だけです", "only `on <error> =>` lines can be written under a call"),
                ));
            }
            let errors = cur.names(tr!("エラーの名前", "an error name"))?;
            let arrow = cur.expect_sym("=>")?;
            self.pos += 1;
            let body = if cur.at_end() {
                self.child_block(child, tr!("`on ... =>`", "`on ... =>`"), arrow)?
            } else {
                let k = simple_stmt(&mut cur)?;
                cur.expect_end()?;
                vec![Stmt { kind: k, span: arrow }]
            };
            out.push(Handler { errors, body, span: sp });
        }
        Ok(out)
    }

    fn stmt(&mut self, indent: usize) -> Result<Stmt, Diag> {
        let l = self.cur_line().unwrap().clone();
        let mut cur = Cur::new(&l);
        let sp = cur.span();
        if cur.is_kw("match") {
            cur.i += 1;
            let e = expr(&mut cur)?;
            cur.expect_end()?;
            self.pos += 1;
            let child = match self.cur_line() {
                Some(c) if c.indent > indent => c.indent,
                _ => return Err(err(sp, tr!("`match` の下に、字下げした分岐が要ります", "`match` needs its arms indented under it"))),
            };
            let mut arms = Vec::new();
            while let Some(al) = self.cur_line() {
                if al.indent != child {
                    if al.indent > child {
                        let s = al.toks[0].span;
                        return Err(err(s, tr!("この行は前の行より深く字下げされています", "this line is indented more than the lines before it")));
                    }
                    break;
                }
                let al = al.clone();
                let mut ac = Cur::new(&al);
                let asp = ac.span();
                let (values, some) = if ac.is_kw("some") && matches!(ac.peek_at(1), Some(Tok::Ident(_))) && matches!(ac.peek_at(2), Some(Tok::Sym("=>"))) {
                    ac.i += 1;
                    (vec![], Some(ac.ident(tr!("変数の名前", "a variable name"))?))
                } else {
                    (ac.names(tr!("値", "a value"))?, None)
                };
                let arrow = ac.expect_sym("=>")?;
                self.pos += 1;
                let body = if ac.at_end() {
                    self.child_block(child, tr!("match の分岐", "a match arm"), arrow)?
                } else {
                    let k = simple_stmt(&mut ac)?;
                    ac.expect_end()?;
                    vec![Stmt { kind: k, span: arrow }]
                };
                arms.push(Arm { values, some, body, span: asp });
            }
            return Ok(Stmt { kind: StmtKind::Match { expr: e, arms }, span: sp });
        }
        if cur.is_kw("for") {
            let (var, list, max, parallel) = for_header(&mut cur)?;
            self.pos += 1;
            let body = self.child_block(indent, tr!("`for`", "`for`"), sp)?;
            return Ok(Stmt { kind: StmtKind::For { var, list, max, parallel, body, result: None }, span: sp });
        }
        if cur.is_kw("let") {
            // `let r = for …` takes a block under it
            let save = cur.i;
            cur.i += 1;
            let name = cur.ident(tr!("変数の名前", "a variable name"))?;
            let ty = if cur.eat_sym(":") { Some(type_expr(&mut cur)?) } else { None };
            cur.expect_sym("=")?;
            if cur.is_kw("for") {
                let (var, list, max, parallel) = for_header(&mut cur)?;
                self.pos += 1;
                let body = self.child_block(indent, tr!("`for`", "`for`"), sp)?;
                return Ok(Stmt { kind: StmtKind::For { var, list, max, parallel, body, result: Some((name, ty)) }, span: sp });
            }
            cur.i = save;
        }
        if cur.is_kw("repeat") {
            cur.i += 1;
            cur.expect_kw("at")?;
            cur.expect_kw("most")?;
            let (n, nsp) = cur.int(tr!("回数", "a number of times"))?;
            if n < 1 {
                return Err(err(nsp, tr!("ループは一回は回ります。1 以上を書いてください", "a loop runs at least once; write 1 or more")));
            }
            cur.expect_kw("times")?;
            cur.expect_end()?;
            self.pos += 1;
            let body = self.child_block(indent, tr!("`repeat`", "`repeat`"), sp)?;
            return Ok(Stmt { kind: StmtKind::Repeat { times: n as u32, body }, span: sp });
        }
        let kind = simple_stmt(&mut cur)?;
        cur.expect_end()?;
        self.pos += 1;
        let kind = match kind {
            StmtKind::Let { name, ty, call, .. } => StmtKind::Let { name, ty, call, handlers: self.handlers(indent)? },
            StmtKind::Call { call, .. } => StmtKind::Call { call, handlers: self.handlers(indent)? },
            StmtKind::CaseCall { case, call, .. } => StmtKind::CaseCall { case, call, handlers: self.handlers(indent)? },
            other => {
                if let Some(nl) = self.cur_line() {
                    if nl.indent > indent {
                        let s = nl.toks[0].span;
                        return Err(err(s, tr!("この行は前の行より深く字下げされています", "this line is indented more than the lines before it")));
                    }
                }
                other
            }
        };
        Ok(Stmt { kind, span: sp })
    }
}

/// A statement that fits on one line: everything but `match`, `repeat` and `for`.
fn simple_stmt(cur: &mut Cur) -> Result<StmtKind, Diag> {
    let sp = cur.span();
    if cur.eat_kw("let") {
        let name = cur.ident(tr!("変数の名前", "a variable name"))?;
        let ty = if cur.eat_sym(":") { Some(type_expr(cur)?) } else { None };
        cur.expect_sym("=")?;
        if matches!(cur.peek(), Some(Tok::Ident(_))) && matches!(cur.peek_at(1), Some(Tok::Sym("("))) {
            let c = call(cur)?;
            return Ok(StmtKind::Let { name, ty, call: c, handlers: vec![] });
        }
        let e = expr(cur)?;
        return Ok(StmtKind::Assign { name, ty, expr: e });
    }
    if cur.eat_kw("yield") {
        return Ok(StmtKind::Yield { expr: expr(cur)? });
    }
    if cur.eat_kw("wait") {
        if cur.eat_kw("until") {
            return Ok(StmtKind::WaitUntil { at: expr(cur)? });
        }
        return Ok(StmtKind::Wait { seconds: duration(cur)? });
    }
    if cur.eat_kw("break") {
        return Ok(StmtKind::Break);
    }
    if cur.eat_kw("pass") {
        return Ok(StmtKind::Pass);
    }
    if cur.eat_kw("succeed") {
        let mut fields = Vec::new();
        if !cur.at_end() {
            loop {
                let n = cur.ident(tr!("出力の名前", "an output name"))?;
                cur.expect_sym("=")?;
                let e = expr(cur)?;
                fields.push((n, e));
                if !cur.eat_sym(",") {
                    break;
                }
            }
        }
        return Ok(StmtKind::Succeed { fields });
    }
    if cur.eat_kw("fail") {
        let error = cur.ident(tr!("エラーの名前", "an error name"))?;
        let mut cause = None;
        if let Some(Tok::Str(_)) = cur.peek() {
            cause = Some(expr(cur)?);
        }
        let mut leaving = Vec::new();
        if cur.eat_kw("leaving") {
            leaving = cur.names(tr!("案件の名前", "a case name"))?;
        }
        return Ok(StmtKind::Fail { error, cause, leaving });
    }
    if matches!(cur.peek(), Some(Tok::Ident(_))) && matches!(cur.peek_at(1), Some(Tok::Sym("<-"))) {
        let case = cur.ident(tr!("案件の名前", "a case name"))?;
        cur.expect_sym("<-")?;
        let c = call(cur)?;
        return Ok(StmtKind::CaseCall { case, call: c, handlers: vec![] });
    }
    if matches!(cur.peek(), Some(Tok::Ident(_))) && matches!(cur.peek_at(1), Some(Tok::Sym("("))) {
        let c = call(cur)?;
        return Ok(StmtKind::Call { call: c, handlers: vec![] });
    }
    Err(err(
        sp,
        tr!("ここには文が要ります（let・<案件> <-・<タスク>(…)・match・wait・repeat・for・break・pass・succeed・fail・yield）", "expected a statement: let, <case> <-, <task>(…), match, wait, repeat, for, break, pass, succeed, fail or yield"),
    ))
}

pub fn parse(src: &str) -> Result<Program, Diag> {
    let lines = lex(src)?;
    let mut p = Parser { lines, pos: 0 };
    let mut prog = Program {
        name: None,
        version: 1,
        implements: None,
        description: None,
        kind: Kind::Standard,
        uses: vec![],
        apis: vec![],
        enums: vec![],
        records: vec![],
        inputs: vec![],
        outputs: vec![],
        tasks: vec![],
        cases: vec![],
        flow: None,
        on_failure: None,
        on_cancel: None,
    };
    while let Some(l) = p.cur_line() {
        let l = l.clone();
        let mut cur = Cur::new(&l);
        let sp = cur.span();
        if l.indent != 0 {
            return Err(err(sp, tr!("この行は字下げされていますが、その上の行は下にブロックを書く行ではありません", "this line is indented, but nothing above it takes a block")));
        }
        let kw = match cur.peek() {
            Some(Tok::Ident(s)) => s.clone(),
            _ => return Err(err(sp, tr!("ここには宣言が要ります", "expected a declaration"))),
        };
        cur.i += 1;
        match kw.as_str() {
            "workflow" => {
                let name = cur.ident(tr!("ワークフローの名前", "the workflow's name"))?;
                let vsp = cur.span();
                let (v, _) = cur.ident(tr!("v1 のようなバージョン", "a version such as v1"))?;
                let n = v.strip_prefix('v').and_then(|s| s.parse::<u32>().ok());
                match n {
                    Some(n) => prog.version = n,
                    None => return Err(err(vsp, tr!("バージョンは v1、v2 のように書きます", "write the version as v1, v2, ..."))),
                }
                // the service of a `.proto` the workflow implements: `<api>.<Service>`
                if cur.eat_kw("implements") {
                    let isp = cur.span();
                    let q = cur.qualname()?;
                    if q.len() < 2 {
                        return Err(err(isp, tr!("サービスは、`use proto` で付けた名前のあとに `implements shop.FulfillmentService` のように書きます", "write the service after the name `use proto` gave the `.proto`, as `implements shop.FulfillmentService`")));
                    }
                    prog.implements = Some(q);
                }
                cur.expect_end()?;
                prog.name = Some(name);
                p.pos += 1;
            }
            "description" => {
                prog.description = Some(cur.string(tr!("説明", "a description"))?.0);
                cur.expect_end()?;
                p.pos += 1;
            }
            "kind" => {
                let (k, ksp) = cur.ident(tr!("standard か express", "standard or express"))?;
                prog.kind = match k.as_str() {
                    "standard" => Kind::Standard,
                    "express" => Kind::Express,
                    _ => return Err(err(ksp, tr!("kind は standard か express です", "the kind is standard or express"))),
                };
                cur.expect_end()?;
                p.pos += 1;
            }
            "use" if !cur.is_kw("rule") => {
                let ksp = cur.span();
                let (k, _) = cur.ident(tr!("`rule`・`openapi`・`smithy`・`proto` のどれか", "`rule`, `openapi`, `smithy` or `proto`"))?;
                let kind = match k.as_str() {
                    "openapi" => crate::apis::ApiKind::OpenApi,
                    "smithy" => crate::apis::ApiKind::Smithy,
                    "proto" => crate::apis::ApiKind::Proto,
                    _ => return Err(err(ksp, tr!("`use rule`・`use openapi`・`use smithy`・`use proto` のどれかを書きます", "write `use rule`, `use openapi`, `use smithy` or `use proto`"))),
                };
                let name = cur.ident(tr!("API の名前", "the API's name"))?;
                cur.expect_kw("from")?;
                let (path, _) = cur.string(tr!("API の記述のパス", "the path of the API's description"))?;
                cur.expect_end()?;
                p.pos += 1;
                let mut url = None;
                while let Some(cl) = p.cur_line() {
                    if cl.indent == 0 {
                        break;
                    }
                    let cl = cl.clone();
                    let mut cc = Cur::new(&cl);
                    if cc.eat_kw("url") {
                        url = Some(cc.string(tr!("API の場所", "where the API is"))?.0);
                        cc.expect_end()?;
                    } else {
                        return Err(err(cc.span(), tr!("`use openapi`・`use smithy`・`use proto` の下に書けるのは `url \"<API の場所>\"` だけです", "only `url \"<where the API is>\"` can be written under `use openapi`, `use smithy` and `use proto`")));
                    }
                    p.pos += 1;
                }
                prog.apis.push(UseApi { kind, name, path, url });
            }
            "use" => {
                cur.expect_kw("rule")?;
                let name = cur.ident(tr!("規則の名前", "the rule's name"))?;
                cur.expect_kw("from")?;
                let (path, _) = cur.string(tr!(".rule ファイルのパス", "the path of the .rule file"))?;
                cur.expect_end()?;
                p.pos += 1;
                let mut lambda = None;
                let mut local = false;
                let mut connect = None;
                let mut connection = None;
                while let Some(cl) = p.cur_line() {
                    if cl.indent == 0 {
                        break;
                    }
                    let cl = cl.clone();
                    let mut cc = Cur::new(&cl);
                    let at = cc.span();
                    if cc.eat_kw("lambda") {
                        lambda = Some((cc.string(tr!("Lambda 関数", "the Lambda function"))?.0, at));
                        cc.expect_end()?;
                    } else if cc.eat_kw("connect") {
                        connect = Some((cc.string(tr!("規則のサービスの URL", "the URL of the rule's service"))?.0, at));
                        cc.expect_end()?;
                    } else if cc.eat_kw("connection") {
                        connection = Some((cc.string(tr!("EventBridge の接続", "the EventBridge connection"))?.0, at));
                        cc.expect_end()?;
                    } else if cc.eat_kw("local") {
                        local = true;
                        cc.expect_end()?;
                    } else {
                        return Err(err(
                            cc.span(),
                            tr!("`use rule` の下に書けるのは `lambda \"<関数>\"`・`connect \"<URL>\"`・`connection \"<接続>\"`・`local` だけです", "only `lambda \"<function>\"`, `connect \"<url>\"`, `connection \"<connection>\"` and `local` can be written under `use rule`"),
                        ));
                    }
                    p.pos += 1;
                }
                prog.uses.push(UseRule { name, path, lambda, local, connect, connection });
            }
            "enum" => {
                let name = cur.ident(tr!("列挙の名前", "the enum's name"))?;
                cur.expect_sym("=")?;
                let mut values = vec![cur.ident(tr!("値", "a value"))?];
                while cur.eat_sym("|") {
                    values.push(cur.ident(tr!("値", "a value"))?);
                }
                cur.expect_end()?;
                p.pos += 1;
                prog.enums.push(EnumDecl { name, values });
            }
            "record" | "inputs" | "outputs" => {
                let name = if kw == "record" { Some(cur.ident(tr!("レコードの名前", "the record's name"))?) } else { None };
                cur.expect_end()?;
                p.pos += 1;
                let mut fields = Vec::new();
                while let Some(fl) = p.cur_line() {
                    if fl.indent == 0 {
                        break;
                    }
                    let fl = fl.clone();
                    let mut fc = Cur::new(&fl);
                    fields.push(field(&mut fc)?);
                    fc.expect_end()?;
                    p.pos += 1;
                }
                if fields.is_empty() {
                    return Err(err(sp, tr!("`{kw}` の下にフィールドが一つは要ります", "`{kw}` needs at least one field under it")));
                }
                match kw.as_str() {
                    "record" => prog.records.push(RecordDecl { name: name.unwrap(), fields }),
                    "inputs" => prog.inputs.extend(fields),
                    _ => prog.outputs.extend(fields),
                }
            }
            "task" => {
                let name = cur.ident(tr!("タスクの名前", "the task's name"))?;
                cur.expect_sym("(")?;
                let mut params = Vec::new();
                if !cur.eat_sym(")") {
                    loop {
                        params.push(field(&mut cur)?);
                        if cur.eat_sym(")") {
                            break;
                        }
                        cur.expect_sym(",")?;
                    }
                }
                let result = if cur.eat_sym("->") { Some(type_expr(&mut cur)?) } else { None };
                let result_range = if result.is_some() { range_decl(&mut cur)? } else { None };
                cur.expect_end()?;
                p.pos += 1;
                let mut t = TaskDecl {
                    name,
                    params,
                    result,
                    result_range,
                    binding: None,
                    model: None,
                    effort: None,
                    confidence: None,
                    url: None,
                    connection: None,
                    queue: None,
                    workflow: None,
                    state_machine: None,
                    durable_function: None,
                    image: None,
                    argo_template: None,
                    errors: vec![],
                    retry: None,
                    timeout: None,
                    key: None,
                    key_param: None,
                    idempotent: false,
                    machine: None,
                    refused_as: None,
                    callback: None,
                    event: None,
                    flow: None,
                };
                while let Some(cl) = p.cur_line() {
                    if cl.indent == 0 {
                        break;
                    }
                    let cl = cl.clone();
                    let mut cc = Cur::new(&cl);
                    task_clause(&mut cc, &mut t)?;
                    cc.expect_end()?;
                    p.pos += 1;
                    // what a `jev` asks is written under it
                    if cc.toks.first().map(|x| &x.tok) == Some(&Tok::Ident("jev".into())) {
                        if let Some((Binding::Jev(jd), _)) = &mut t.binding {
                            jev_block(&mut p, cl.indent, jd)?;
                        }
                    }
                }
                prog.tasks.push(t);
            }
            "case" => {
                let name = cur.ident(tr!("案件の名前", "the case's name"))?;
                cur.expect_sym(":")?;
                let record = type_expr(&mut cur)?;
                cur.expect_kw("follows")?;
                let machine = cur.qualname()?;
                cur.expect_end()?;
                p.pos += 1;
                let mut c = CaseDecl { name, record, machine, held: vec![], external: vec![], state_field: None, refused_when: None };
                while let Some(cl) = p.cur_line() {
                    if cl.indent == 0 {
                        break;
                    }
                    let cl = cl.clone();
                    let mut cc = Cur::new(&cl);
                    let csp = cc.span();
                    if cc.eat_kw("held") {
                        let n = cc.ident(tr!("規則の入力", "an input of the rule"))?;
                        cc.expect_sym("=")?;
                        let v = match cc.peek().cloned() {
                            Some(Tok::Ident(s)) => {
                                let s2 = cc.span();
                                cc.i += 1;
                                (s, s2)
                            }
                            Some(Tok::Int(n)) => {
                                let s2 = cc.span();
                                cc.i += 1;
                                (n.to_string(), s2)
                            }
                            _ => return Err(err(cc.span(), tr!("ここには値が要ります", "expected a value here"))),
                        };
                        c.held.push((n, v));
                    } else if cc.eat_kw("external") {
                        c.external.extend(cc.names(tr!("イベント", "an event"))?);
                    } else if cc.eat_kw("state") {
                        c.state_field = Some(cc.ident(tr!("フィールド", "a field"))?);
                    } else if cc.eat_kw("refused") {
                        cc.expect_kw("when")?;
                        let o = cc.ident(tr!("規則の出力", "an output of the rule"))?;
                        cc.expect_sym("=")?;
                        let v = cc.ident(tr!("値", "a value"))?;
                        c.refused_when = Some((o, v));
                    } else {
                        return Err(err(
                            csp,
                            tr!("`case` の下には held・external・state・refused when を書きます", "under `case` write held, external, state or refused when"),
                        ));
                    }
                    cc.expect_end()?;
                    p.pos += 1;
                }
                prog.cases.push(c);
            }
            "flow" => {
                cur.expect_end()?;
                p.pos += 1;
                if prog.flow.is_some() {
                    return Err(err(sp, tr!("`flow` は一つだけ書けます", "a workflow has one `flow`")));
                }
                let b = p.child_block(0, tr!("`flow`", "`flow`"), sp)?;
                prog.flow = Some((b, sp));
            }
            "on" => {
                if cur.eat_kw("cancel") {
                    cur.expect_end()?;
                    p.pos += 1;
                    if prog.on_cancel.is_some() {
                        return Err(err(sp, tr!("`on cancel` は一つだけ書けます", "a workflow has one `on cancel`")));
                    }
                    let b = p.child_block(0, tr!("`on cancel`", "`on cancel`"), sp)?;
                    prog.on_cancel = Some((b, sp));
                } else {
                    cur.expect_kw("failure")?;
                    cur.expect_end()?;
                    p.pos += 1;
                    if prog.on_failure.is_some() {
                        return Err(err(sp, tr!("`on failure` は一つだけ書けます", "a workflow has one `on failure`")));
                    }
                    let b = p.child_block(0, tr!("`on failure`", "`on failure`"), sp)?;
                    prog.on_failure = Some((b, sp));
                }
            }
            other => {
                return Err(err(
                    sp,
                    tr!("`{other}` で始まる宣言はありません（workflow・description・kind・use・enum・record・inputs・outputs・task・case・flow・on failure・on cancel）", "`{other}` does not start a declaration; expected workflow, description, kind, use, enum, record, inputs, outputs, task, case, flow, on failure or on cancel"),
                ))
            }
        }
    }
    Ok(prog)
}

fn task_clause(cc: &mut Cur, t: &mut TaskDecl) -> Result<(), Diag> {
    let sp = cc.span();
    let (kw, _) = cc.ident(tr!("タスクの項目", "a task clause"))?;
    if matches!(kw.as_str(), "lambda" | "http" | "aws" | "agent" | "connect" | "jev") {
        if let Some((_, first)) = &t.binding {
            return Err(Diag::error(
                "E007",
                sp.line,
                sp.col,
                tr!("このタスクの呼び出し方はもう書かれています（{} 行目）。呼び出し方は lambda・http・connect・aws・agent・jev のどれか一つです", "the task is already called another way (line {}); a task is called by one of lambda, http, connect, aws, agent and jev", first.line),
            ));
        }
    }
    match kw.as_str() {
        "lambda" => {
            let f = cc.string(tr!("Lambda 関数", "the Lambda function"))?.0;
            t.binding = Some((Binding::Lambda(f), sp));
        }
        "aws" => {
            let (service, action) = if let Some(Tok::Str(_)) = cc.peek() {
                let (text, tsp) = cc.string(tr!("AWS の API", "the AWS API"))?;
                match text.split_once(':') {
                    Some((a, b)) if !a.is_empty() && !b.is_empty() => (a.to_string(), b.to_string()),
                    _ => return Err(err(tsp, tr!("AWS の API は sns:publish のように <サービス>:<操作> と書きます", "write the AWS API as <service>:<action>, such as sns:publish"))),
                }
            } else {
                let (a, _) = cc.ident(tr!("AWS のサービス（sns など）", "an AWS service, such as sns"))?;
                cc.expect_sym(":")?;
                let (b, _) = cc.ident(tr!("API の操作（publish など）", "an API action, such as publish"))?;
                (a, b)
            };
            t.binding = Some((Binding::Aws { service, action }, sp));
        }
        "queue" => t.queue = Some(cc.string(tr!("タスクキュー", "the task queue"))?.0),
        "workflow" => {
            if cc.eat_kw("template") {
                t.argo_template = Some((cc.string(tr!("WorkflowTemplate", "the WorkflowTemplate"))?.0, sp));
            } else {
                t.workflow = Some((cc.string(tr!("子ワークフローの型", "the type of the child workflow"))?.0, sp));
            }
        }
        "image" => t.image = Some((cc.string(tr!("コンテナのイメージ", "the container image"))?.0, sp)),
        "state" => {
            cc.expect_kw("machine")?;
            t.state_machine = Some((cc.string(tr!("ステートマシンの ARN", "the state machine's ARN"))?.0, sp));
        }
        "durable" => {
            cc.expect_kw("function")?;
            t.durable_function = Some((cc.string(tr!("durable function", "the durable function"))?.0, sp));
        }
        "http" => {
            let (m, msp) = cc.ident(tr!("HTTP のメソッド", "an HTTP method"))?;
            let method = m.to_ascii_uppercase();
            if !["GET", "POST", "PUT", "PATCH", "DELETE"].contains(&method.as_str()) {
                return Err(err(msp, tr!("メソッドは GET・POST・PUT・PATCH・DELETE のどれかです", "the method is GET, POST, PUT, PATCH or DELETE")));
            }
            let api = match cc.peek() {
                Some(Tok::Ident(_)) => Some(cc.ident(tr!("API", "the API"))?),
                _ => None,
            };
            let url = cc.string(if api.is_some() { tr!("操作のパス", "the operation's path") } else { tr!("URL", "the URL") })?.0;
            let form = cc.eat_kw("form");
            t.binding = Some((Binding::Http { method, url, form, api }, sp));
        }
        "connect" => {
            let api = cc.ident(tr!("API（`use proto` の名前）", "the API (`use proto`)"))?;
            let method = cc.string(tr!("メソッド（`Service/Method`）", "the method, as `Service/Method`"))?.0;
            t.binding = Some((Binding::Connect { api, method }, sp));
        }
        "agent" => {
            let provider = match cc.peek() {
                Some(Tok::Ident(_)) => Some(cc.ident(tr!("どこのエージェントか（openai か claude）", "whose agent it is, openai or claude"))?),
                _ => None,
            };
            let instructions = cc.string(tr!("エージェントへの指示", "what the agent is to do"))?.0;
            t.binding = Some((Binding::Agent { provider, instructions }, sp));
        }
        "jev" => {
            // `jev score "…"` or `jev "…"`: one question; `jev` alone: a question for each field
            let score = if cc.is_kw("score") && matches!(cc.peek_at(1), Some(Tok::Str(_))) {
                let ssp = cc.span();
                cc.i += 1;
                Some(ssp)
            } else {
                None
            };
            let ask = match cc.peek() {
                Some(Tok::Str(_)) => {
                    let (instructions, isp) = cc.string(tr!("Jev に尋ねること", "the question Jev answers"))?;
                    Some(JevAsk { instructions, score, criteria: vec![], span: isp })
                }
                _ if score.is_some() => return Err(err(cc.span(), tr!("`jev score` のあとに、尋ねることを二重引用符で書きます", "write the question after `jev score`, in double quotes"))),
                _ => None,
            };
            t.binding = Some((Binding::Jev(JevDecl { ask, fields: vec![] }), sp));
        }
        "confidence" => {
            let vsp = cc.span();
            let v = match cc.peek().cloned() {
                Some(Tok::Float(f)) => f,
                Some(Tok::Int(n)) => n as f64,
                _ => return Err(err(vsp, tr!("ここには Jev がどれだけ確かでなければならないかを、0.8 のような 0 から 1 の数で書きます", "expected how sure Jev must be, a number from 0 to 1 such as 0.8"))),
            };
            cc.i += 1;
            if !(v > 0.0 && v <= 1.0) {
                return Err(err(vsp, tr!("確信度は 0 より大きく、1 以下です", "a confidence is more than 0 and at most 1")));
            }
            cc.expect_kw("else")?;
            let e = cc.ident(tr!("確信度が足りない答えで呼び出しが失敗するときのエラー", "the error a less sure answer fails the call with"))?;
            t.confidence = Some((v, e, sp));
        }
        "model" => t.model = Some((cc.string(tr!("モデル", "the model"))?.0, sp)),
        "url" => t.url = Some((cc.string(tr!("エージェントのサーバーの場所", "where the agent's server is"))?.0, sp)),
        "effort" => t.effort = Some((cc.ident(tr!("low・medium・high のようなエフォート", "an effort, such as low, medium or high"))?.0, sp)),
        "connection" => t.connection = Some(cc.string(tr!("EventBridge の接続", "the EventBridge connection"))?.0),
        "errors" => loop {
            let name = cc.ident(tr!("エラーの名前", "an error name"))?;
            let mut status = None;
            let mut exception = None;
            if cc.eat_sym("=") {
                let vsp = cc.span();
                match cc.peek().cloned() {
                    Some(Tok::Int(n)) => {
                        cc.i += 1;
                        if !(400..=599).contains(&n) {
                            return Err(err(vsp, tr!("エラーのステータスは 400 から 599 です", "an error status is between 400 and 599")));
                        }
                        status = Some(n as u16);
                    }
                    Some(Tok::Ident(x)) | Some(Tok::Str(x)) => {
                        cc.i += 1;
                        exception = Some(x);
                    }
                    _ => return Err(err(vsp, tr!("ここには HTTP のステータスか、AWS の API の例外の名前が要ります", "expected an HTTP status or the name of an AWS API's exception here"))),
                }
            }
            t.errors.push(ErrDecl { name, status, exception });
            if !cc.eat_sym(",") {
                break;
            }
        },
        "retry" => {
            let (n, nsp) = cc.int(tr!("リトライの回数", "a number of retries"))?;
            if n < 1 {
                return Err(err(nsp, tr!("一回以上を書くか、`retry` を書かないでください", "retry at least once, or leave `retry` out")));
            }
            cc.expect_kw("times")?;
            let mut r = RetryDecl { times: n as u32, every: 1, backoff: 2.0, on: vec![], span: sp };
            loop {
                if cc.eat_kw("every") {
                    r.every = duration(cc)?;
                } else if cc.eat_kw("backoff") {
                    let bsp = cc.span();
                    r.backoff = match cc.peek().cloned() {
                        Some(Tok::Int(n)) => n as f64,
                        Some(Tok::Float(f)) => f,
                        _ => return Err(err(bsp, tr!("ここには数が要ります", "expected a number here"))),
                    };
                    cc.i += 1;
                    if r.backoff < 1.0 {
                        return Err(err(bsp, tr!("backoff は 1 以上です", "the backoff is 1 or more")));
                    }
                } else if cc.eat_kw("on") {
                    r.on = cc.names(tr!("エラーの名前", "an error name"))?;
                } else {
                    break;
                }
            }
            t.retry = Some(r);
        }
        "timeout" => t.timeout = Some(duration(cc)?),
        "key" => {
            t.key = Some(sp);
            if let Some(Tok::Ident(_)) = cc.peek() {
                t.key_param = Some(cc.ident(tr!("キーを渡す引数", "the parameter that takes the key"))?);
            }
        }
        "idempotent" => t.idempotent = true,
        "callback" => t.callback = Some(sp),
        "event" => t.event = Some(sp),
        "flow" => t.flow = Some((cc.string(tr!("子の .flow のパス", "the path of the child's .flow"))?.0, sp)),
        "starts" => {
            let machine = cc.qualname()?;
            let mut then = Vec::new();
            if cc.eat_kw("then") {
                then = cc.names(tr!("イベント", "an event"))?;
            }
            t.machine = Some((MachineUse::Starts { machine, then }, sp));
        }
        "sends" => {
            let first = cc.ident(tr!("イベント", "an event"))?;
            if cc.eat_sym("=") {
                let v = cc.ident(tr!("イベント", "an event"))?;
                t.machine = Some((MachineUse::Sends { event: v, column: Some(first) }, sp));
            } else {
                t.machine = Some((MachineUse::Sends { event: first, column: None }, sp));
            }
        }
        "observes" => t.machine = Some((MachineUse::Observes, sp)),
        "refused" => {
            cc.expect_kw("as")?;
            t.refused_as = Some(cc.ident(tr!("エラーの名前", "an error name"))?);
        }
        other => {
            return Err(err(
                sp,
                tr!("`{other}` はタスクの項目ではありません（lambda・http・connect・aws・agent・jev・model・effort・confidence・url・connection・flow・queue・workflow・state machine・durable function・image・workflow template・errors・retry・timeout・key・idempotent・callback・event・starts・sends・observes・refused as）", "`{other}` is not a task clause; expected lambda, http, connect, aws, agent, jev, model, effort, confidence, url, connection, flow, queue, workflow, state machine, durable function, image, workflow template, errors, retry, timeout, key, idempotent, callback, event, starts, sends, observes or refused as"),
            ))
        }
    }
    Ok(())
}

/// The lines under a `jev` line, indented deeper than it (`indent`): under `jev "…"`, each value
/// with what it means; under `jev` alone, a question for each field of the record, or the field
/// that takes how sure Jev is of another's answer, each with its values' meanings under it.
fn jev_block(p: &mut Parser, indent: usize, jd: &mut JevDecl) -> Result<(), Diag> {
    let mut level: Option<usize> = None;
    while let Some(l) = p.cur_line() {
        if l.indent <= indent {
            break;
        }
        let l = l.clone();
        let mut cc = Cur::new(&l);
        let sp = cc.span();
        match level {
            None => level = Some(l.indent),
            Some(lv) if l.indent < lv => return Err(err(sp, tr!("この行は、`jev` の下のそれより前の行より浅く字下げされています", "this line is indented less than the lines before it under `jev`"))),
            Some(lv) if l.indent > lv => {
                // a value's meaning, under a field's question
                let criteria = match jd.fields.last_mut() {
                    Some((_, JevField::Ask(a))) => &mut a.criteria,
                    Some((_, JevField::Confidence(_))) => return Err(err(sp, tr!("確信度を受け取るフィールドは何も尋ねないので、その下には何も書きません", "a field that takes how sure Jev is asks nothing, so nothing is written under it"))),
                    None => return Err(err(sp, tr!("この行は前の行より深く字下げされています", "this line is indented more than the lines before it"))),
                };
                criteria.push(meaning(&mut cc)?);
                cc.expect_end()?;
                p.pos += 1;
                continue;
            }
            _ => {}
        }
        if let Some(ask) = &mut jd.ask {
            let m = meaning(&mut cc)?;
            cc.expect_end()?;
            ask.criteria.push(m);
            p.pos += 1;
            continue;
        }
        let field = cc.ident(tr!("答えのフィールド", "a field of the answer"))?;
        let what = if cc.eat_kw("confidence") {
            cc.expect_kw("of")?;
            JevField::Confidence(cc.ident(tr!("どのフィールドの答えの確信度か", "the field whose answer it is how sure of"))?)
        } else {
            let score = if cc.is_kw("score") && matches!(cc.peek_at(1), Some(Tok::Str(_))) {
                let ssp = cc.span();
                cc.i += 1;
                Some(ssp)
            } else {
                None
            };
            match cc.peek() {
                Some(Tok::Str(_)) => {
                    let (instructions, isp) = cc.string(tr!("そのフィールドについて Jev に尋ねること", "the question Jev answers for the field"))?;
                    JevField::Ask(JevAsk { instructions, score, criteria: vec![], span: isp })
                }
                _ => {
                    return Err(err(
                        cc.span(),
                        tr!("`jev` の下には、`{}` について尋ねることを書きます（`{} \"<質問>\"`、`{} score \"<質問>\"`、`{} confidence of <フィールド>` のどれか）", "under `jev`, write what Jev is asked for `{}`: `{} \"<question>\"`, `{} score \"<question>\"`, or `{} confidence of <field>`", field.0, field.0, field.0, field.0),
                    ))
                }
            }
        };
        cc.expect_end()?;
        jd.fields.push((field, what));
        p.pos += 1;
    }
    Ok(())
}

/// `returns "<what it means>"`: a value of the answer, and what it means to Jev.
fn meaning(cc: &mut Cur) -> Result<(Name, String), Diag> {
    let v = cc.ident(tr!("答えの値", "a value of the answer"))?;
    let (text, _) = cc.string(tr!("その値の意味", "what the value means"))?;
    Ok((v, text))
}
