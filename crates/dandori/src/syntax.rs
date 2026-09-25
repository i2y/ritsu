//! The surface syntax: physical lines with their indentation, the tokens on each line,
//! and the tree the parser builds. Blocks are marked by indentation alone.

use crate::diag::Diag;

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
    pub description: Option<String>,
    pub kind: Kind,
    pub uses: Vec<UseRule>,
    pub enums: Vec<EnumDecl>,
    pub records: Vec<RecordDecl>,
    pub inputs: Vec<Field>,
    pub outputs: Vec<Field>,
    pub tasks: Vec<TaskDecl>,
    pub cases: Vec<CaseDecl>,
    pub flow: Option<(Block, Span)>,
    pub on_failure: Option<(Block, Span)>,
}

#[derive(Clone, Debug)]
pub struct UseRule {
    pub name: Name,
    pub path: String,
    pub lambda: Option<String>,
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
    Http { method: String, url: String, form: bool },
    /// an AWS API call, as Step Functions' AWS SDK integrations name it: `sns:publish`
    Aws { service: String, action: String },
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
    pub binding: Option<(Binding, Span)>,
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

fn err(span: Span, en: impl Into<String>, ja: impl Into<String>) -> Diag {
    Diag::error("E001", span.line, span.col, en, ja)
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
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
                    "indent with spaces, not tabs",
                    "インデントにはタブではなく空白を使ってください",
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
                    return Err(err(span, "this string is not closed", "文字列が閉じていません"));
                }
                toks.push(Token { tok: Tok::Str(s), span });
                continue;
            }
            if c.is_ascii_digit() {
                let start = j;
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
                if j < chars.len() && is_ident_start(chars[j]) {
                    return Err(err(span, "put a space between a number and a word", "数と語のあいだに空白を入れてください"));
                }
                if is_float {
                    toks.push(Token { tok: Tok::Float(text.parse().unwrap_or(0.0)), span });
                } else {
                    match text.parse::<i64>() {
                        Ok(n) => toks.push(Token { tok: Tok::Int(n), span }),
                        Err(_) => return Err(err(span, "this number is too large", "数が大きすぎます")),
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
            let two: String = chars[j..(j + 2).min(chars.len())].iter().collect();
            let sym2 = match two.as_str() {
                "->" => Some("->"),
                "<-" => Some("<-"),
                "=>" => Some("=>"),
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
                _ => None,
            };
            if let Some(s) = sym1 {
                toks.push(Token { tok: Tok::Sym(s), span });
                j += 1;
                continue;
            }
            return Err(err(span, format!("unexpected character `{c}`"), format!("ここに `{c}` は書けません")));
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
            Err(err(sp, format!("expected `{s}` here"), format!("ここには `{s}` が要ります")))
        }
    }

    fn expect_kw(&mut self, kw: &str) -> Result<Span, Diag> {
        let sp = self.span();
        if self.eat_kw(kw) {
            Ok(sp)
        } else {
            Err(err(sp, format!("expected `{kw}` here"), format!("ここには `{kw}` が要ります")))
        }
    }

    fn ident(&mut self, what_en: &str, what_ja: &str) -> Result<Name, Diag> {
        let sp = self.span();
        match self.peek() {
            Some(Tok::Ident(s)) => {
                let s = s.clone();
                self.i += 1;
                Ok((s, sp))
            }
            _ => Err(err(sp, format!("expected {what_en} here"), format!("ここには{what_ja}が要ります"))),
        }
    }

    fn string(&mut self, what_en: &str, what_ja: &str) -> Result<(String, Span), Diag> {
        let sp = self.span();
        match self.peek() {
            Some(Tok::Str(s)) => {
                let s = s.clone();
                self.i += 1;
                Ok((s, sp))
            }
            _ => Err(err(sp, format!("expected {what_en} in double quotes here"), format!("ここには{what_ja}を二重引用符で書きます"))),
        }
    }

    fn int(&mut self, what_en: &str, what_ja: &str) -> Result<(i64, Span), Diag> {
        let sp = self.span();
        match self.peek() {
            Some(Tok::Int(n)) => {
                let n = *n;
                self.i += 1;
                Ok((n, sp))
            }
            _ => Err(err(sp, format!("expected {what_en} here"), format!("ここには{what_ja}が要ります"))),
        }
    }

    fn expect_end(&self) -> Result<(), Diag> {
        if self.at_end() {
            Ok(())
        } else {
            Err(err(self.span(), "unexpected text at the end of the line", "行の終わりに余計なものがあります"))
        }
    }

    fn names(&mut self, what_en: &str, what_ja: &str) -> Result<Vec<Name>, Diag> {
        let mut v = vec![self.ident(what_en, what_ja)?];
        while self.eat_sym(",") {
            v.push(self.ident(what_en, what_ja)?);
        }
        Ok(v)
    }

    fn qualname(&mut self) -> Result<Vec<Name>, Diag> {
        let mut v = vec![self.ident("a name", "名前")?];
        while self.eat_sym(".") {
            v.push(self.ident("a name", "名前")?);
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
    "workflow", "description", "kind", "use", "rule", "from", "enum", "record", "inputs", "outputs", "task", "case",
    "follows", "flow", "on", "let", "match", "wait", "repeat", "break", "succeed", "fail", "leaving", "lambda", "http",
    "aws", "connection", "queue", "machine", "durable", "function", "image", "template", "errors", "retry", "timeout", "key", "idempotent",
    "starts", "sends", "observes", "refused", "callback", "held", "external", "state", "then", "true", "false", "until",
    "pass", "for", "in", "at", "most", "parallel", "yield", "some", "none", "list", "json",
];

/// The kinds of number with a unit that rulec has: `money[円, incl_tax]`, `mass[kg]`, …
pub const UNIT_KINDS: &[&str] = &["mass", "length", "area", "volume", "duration", "temperature", "sound", "money", "rate"];

pub fn is_keyword(s: &str) -> bool {
    KEYWORDS.contains(&s)
}

fn duration(cur: &mut Cur) -> Result<u64, Diag> {
    let (n, sp) = cur.int("a number", "数")?;
    if n < 0 {
        return Err(err(sp, "a duration cannot be negative", "時間は負にできません"));
    }
    let unit_sp = cur.span();
    let (u, _) = cur.ident("a unit (seconds, minutes, hours or days)", "単位（seconds・minutes・hours・days）")?;
    let mult = match u.as_str() {
        "second" | "seconds" => 1,
        "minute" | "minutes" => 60,
        "hour" | "hours" => 3600,
        "day" | "days" => 86400,
        _ => {
            return Err(err(
                unit_sp,
                format!("`{u}` is not a unit of time; write seconds, minutes, hours or days"),
                format!("`{u}` は時間の単位ではありません。seconds・minutes・hours・days のどれかを書きます"),
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
            return Err(err(cur.span(), "a type is made optional once; write one `?`", "`?` は一つだけ書きます"));
        }
        return Ok(TypeExpr::Opt(Box::new(base), sp));
    }
    Ok(base)
}

fn type_base(cur: &mut Cur) -> Result<TypeExpr, Diag> {
    let sp = cur.span();
    let first = cur.ident("a type", "型")?;
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
                return Err(err(cur.span(), "this list type is not closed with `]`", "リストの型が `]` で閉じていません"));
            }
            return Ok(TypeExpr::List(Box::new(inner), sp));
        }
        _ => {}
    }
    if cur.is_sym("[") {
        // a unit, spelled as rulec spells it: money[円, incl_tax]
        cur.i += 1;
        let mut parts = Vec::new();
        loop {
            match cur.peek() {
                Some(Tok::Sym("]")) => {
                    cur.i += 1;
                    break;
                }
                Some(Tok::Ident(s)) => {
                    parts.push(s.clone());
                    cur.i += 1;
                }
                Some(Tok::Sym(",")) => {
                    cur.i += 1;
                }
                _ => return Err(err(cur.span(), "this unit is not closed with `]`", "単位が `]` で閉じていません")),
            }
        }
        return Ok(TypeExpr::Unit(format!("{}[{}]", first.0, parts.join(", ")), sp));
    }
    let mut v = vec![first];
    while cur.eat_sym(".") {
        v.push(cur.ident("a name", "名前")?);
    }
    Ok(TypeExpr::Named(v))
}

fn field(cur: &mut Cur) -> Result<Field, Diag> {
    let name = cur.ident("a field name", "フィールドの名前")?;
    cur.expect_sym(":")?;
    let ty = type_expr(cur)?;
    Ok(Field { name, ty })
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
                    let n = cur.ident("a field name", "フィールドの名前")?;
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
        _ => Err(err(sp, "expected a value here", "ここには値が要ります")),
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
                return Err(err(sp, "a `{` in this string is not closed; write `{{` for a brace itself", "文字列の中の `{` が閉じていません。波括弧そのものは `{{` と書きます"));
            }
            let inner: String = chars[start..j].iter().collect();
            let mut path = Vec::new();
            for part in inner.split('.') {
                let p = part.trim();
                let ok = p.chars().next().map(is_ident_start).unwrap_or(false) && p.chars().all(is_ident_continue);
                if !ok {
                    return Err(err(
                        sp,
                        format!("`{{{inner}}}` in this string is not a variable or a field of one; write `{{{{` for a brace itself"),
                        format!("文字列の中の `{{{inner}}}` は変数やそのフィールドではありません。波括弧そのものは `{{{{` と書きます"),
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
            return Err(err(sp, "a `}` in this string has no `{`; write `}}` for a brace itself", "文字列の中の `}` に対応する `{` がありません。波括弧そのものは `}}` と書きます"));
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
    let var = cur.ident("a variable name", "変数の名前")?;
    cur.expect_kw("in")?;
    let list = expr(cur)?;
    cur.expect_kw("at")?;
    cur.expect_kw("most")?;
    let (n, nsp) = cur.int("the most items the list can have", "リストの要素の数の上限")?;
    if n < 1 {
        return Err(err(nsp, "write the most items the list can have, 1 or more", "リストの要素の数の上限を 1 以上で書いてください"));
    }
    let mut parallel = None;
    if cur.eat_kw("in") {
        cur.expect_kw("parallel")?;
        let mut k = 0;
        if cur.eat_sym(",") {
            let (kk, ksp) = cur.int("how many rounds run at a time", "同時に回す数")?;
            if kk < 1 {
                return Err(err(ksp, "run at least one round at a time", "同時に回す数は 1 以上です"));
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
    let callee = cur.ident("the name of a task or a rule", "タスクか規則の名前")?;
    cur.expect_sym("(")?;
    let mut args = Vec::new();
    if !cur.eat_sym(")") {
        loop {
            let name = cur.ident("an argument name", "引数の名前")?;
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
    fn child_block(&mut self, parent: usize, what_en: &str, what_ja: &str, at: Span) -> Result<Block, Diag> {
        let indent = match self.cur_line() {
            Some(l) if l.indent > parent => l.indent,
            _ => {
                return Err(err(
                    at,
                    format!("{what_en} needs an indented block under it"),
                    format!("{what_ja}の下に、字下げしたブロックが要ります"),
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
                return Err(err(sp, "this line is indented more than the lines before it", "この行は前の行より深く字下げされています"));
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
                    return Err(err(sp, "this line is indented more than the lines before it", "この行は前の行より深く字下げされています"));
                }
                break;
            }
            let l = l.clone();
            let mut cur = Cur::new(&l);
            let sp = cur.span();
            if !cur.eat_kw("on") {
                return Err(err(
                    sp,
                    "only `on <error> =>` lines can be written under a call",
                    "呼び出しの下に書けるのは `on <エラー> =>` の行だけです",
                ));
            }
            let errors = cur.names("an error name", "エラーの名前")?;
            let arrow = cur.expect_sym("=>")?;
            self.pos += 1;
            let body = if cur.at_end() {
                self.child_block(child, "`on ... =>`", "`on ... =>`", arrow)?
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
                _ => return Err(err(sp, "`match` needs its arms indented under it", "`match` の下に、字下げした行き先が要ります")),
            };
            let mut arms = Vec::new();
            while let Some(al) = self.cur_line() {
                if al.indent != child {
                    if al.indent > child {
                        let s = al.toks[0].span;
                        return Err(err(s, "this line is indented more than the lines before it", "この行は前の行より深く字下げされています"));
                    }
                    break;
                }
                let al = al.clone();
                let mut ac = Cur::new(&al);
                let asp = ac.span();
                let (values, some) = if ac.is_kw("some") && matches!(ac.peek_at(1), Some(Tok::Ident(_))) && matches!(ac.peek_at(2), Some(Tok::Sym("=>"))) {
                    ac.i += 1;
                    (vec![], Some(ac.ident("a variable name", "変数の名前")?))
                } else {
                    (ac.names("a value", "値")?, None)
                };
                let arrow = ac.expect_sym("=>")?;
                self.pos += 1;
                let body = if ac.at_end() {
                    self.child_block(child, "a match arm", "match の行き先", arrow)?
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
            let body = self.child_block(indent, "`for`", "`for`", sp)?;
            return Ok(Stmt { kind: StmtKind::For { var, list, max, parallel, body, result: None }, span: sp });
        }
        if cur.is_kw("let") {
            // `let r = for …` takes a block under it
            let save = cur.i;
            cur.i += 1;
            let name = cur.ident("a variable name", "変数の名前")?;
            let ty = if cur.eat_sym(":") { Some(type_expr(&mut cur)?) } else { None };
            cur.expect_sym("=")?;
            if cur.is_kw("for") {
                let (var, list, max, parallel) = for_header(&mut cur)?;
                self.pos += 1;
                let body = self.child_block(indent, "`for`", "`for`", sp)?;
                return Ok(Stmt { kind: StmtKind::For { var, list, max, parallel, body, result: Some((name, ty)) }, span: sp });
            }
            cur.i = save;
        }
        if cur.is_kw("repeat") {
            cur.i += 1;
            cur.expect_kw("at")?;
            cur.expect_kw("most")?;
            let (n, nsp) = cur.int("a number of times", "回数")?;
            if n < 1 {
                return Err(err(nsp, "a loop runs at least once; write 1 or more", "ループは一回は回ります。1 以上を書いてください"));
            }
            cur.expect_kw("times")?;
            cur.expect_end()?;
            self.pos += 1;
            let body = self.child_block(indent, "`repeat`", "`repeat`", sp)?;
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
                        return Err(err(s, "this line is indented more than the lines before it", "この行は前の行より深く字下げされています"));
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
        let name = cur.ident("a variable name", "変数の名前")?;
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
                let n = cur.ident("an output name", "出力の名前")?;
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
        let error = cur.ident("an error name", "エラーの名前")?;
        let mut cause = None;
        if let Some(Tok::Str(_)) = cur.peek() {
            cause = Some(expr(cur)?);
        }
        let mut leaving = Vec::new();
        if cur.eat_kw("leaving") {
            leaving = cur.names("a case name", "案件の名前")?;
        }
        return Ok(StmtKind::Fail { error, cause, leaving });
    }
    if matches!(cur.peek(), Some(Tok::Ident(_))) && matches!(cur.peek_at(1), Some(Tok::Sym("<-"))) {
        let case = cur.ident("a case name", "案件の名前")?;
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
        "expected a statement: let, <case> <-, <task>(…), match, wait, repeat, for, break, pass, succeed, fail or yield",
        "ここには文が要ります（let・<案件> <-・<タスク>(…)・match・wait・repeat・for・break・pass・succeed・fail・yield）",
    ))
}

pub fn parse(src: &str) -> Result<Program, Diag> {
    let lines = lex(src)?;
    let mut p = Parser { lines, pos: 0 };
    let mut prog = Program {
        name: None,
        version: 1,
        description: None,
        kind: Kind::Standard,
        uses: vec![],
        enums: vec![],
        records: vec![],
        inputs: vec![],
        outputs: vec![],
        tasks: vec![],
        cases: vec![],
        flow: None,
        on_failure: None,
    };
    while let Some(l) = p.cur_line() {
        let l = l.clone();
        let mut cur = Cur::new(&l);
        let sp = cur.span();
        if l.indent != 0 {
            return Err(err(sp, "this line is indented, but nothing above it takes a block", "この行は字下げされていますが、その上の行は下にブロックを書く行ではありません"));
        }
        let kw = match cur.peek() {
            Some(Tok::Ident(s)) => s.clone(),
            _ => return Err(err(sp, "expected a declaration", "ここには宣言が要ります")),
        };
        cur.i += 1;
        match kw.as_str() {
            "workflow" => {
                let name = cur.ident("the workflow's name", "ワークフローの名前")?;
                let vsp = cur.span();
                let (v, _) = cur.ident("a version such as v1", "v1 のような版")?;
                let n = v.strip_prefix('v').and_then(|s| s.parse::<u32>().ok());
                match n {
                    Some(n) => prog.version = n,
                    None => return Err(err(vsp, "write the version as v1, v2, ...", "版は v1、v2 のように書きます")),
                }
                cur.expect_end()?;
                prog.name = Some(name);
                p.pos += 1;
            }
            "description" => {
                prog.description = Some(cur.string("a description", "説明")?.0);
                cur.expect_end()?;
                p.pos += 1;
            }
            "kind" => {
                let (k, ksp) = cur.ident("standard or express", "standard か express")?;
                prog.kind = match k.as_str() {
                    "standard" => Kind::Standard,
                    "express" => Kind::Express,
                    _ => return Err(err(ksp, "the kind is standard or express", "kind は standard か express です")),
                };
                cur.expect_end()?;
                p.pos += 1;
            }
            "use" => {
                cur.expect_kw("rule")?;
                let name = cur.ident("the rule's name", "規則の名前")?;
                cur.expect_kw("from")?;
                let (path, _) = cur.string("the path of the .rule file", ".rule ファイルのパス")?;
                cur.expect_end()?;
                p.pos += 1;
                let mut lambda = None;
                while let Some(cl) = p.cur_line() {
                    if cl.indent == 0 {
                        break;
                    }
                    let cl = cl.clone();
                    let mut cc = Cur::new(&cl);
                    if cc.eat_kw("lambda") {
                        lambda = Some(cc.string("the Lambda function", "Lambda 関数")?.0);
                        cc.expect_end()?;
                    } else {
                        return Err(err(cc.span(), "only `lambda \"<function>\"` can be written under `use rule`", "`use rule` の下に書けるのは `lambda \"<関数>\"` だけです"));
                    }
                    p.pos += 1;
                }
                prog.uses.push(UseRule { name, path, lambda });
            }
            "enum" => {
                let name = cur.ident("the enum's name", "列挙の名前")?;
                cur.expect_sym("=")?;
                let mut values = vec![cur.ident("a value", "値")?];
                while cur.eat_sym("|") {
                    values.push(cur.ident("a value", "値")?);
                }
                cur.expect_end()?;
                p.pos += 1;
                prog.enums.push(EnumDecl { name, values });
            }
            "record" | "inputs" | "outputs" => {
                let name = if kw == "record" { Some(cur.ident("the record's name", "レコードの名前")?) } else { None };
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
                    return Err(err(sp, format!("`{kw}` needs at least one field under it"), format!("`{kw}` の下にフィールドが一つは要ります")));
                }
                match kw.as_str() {
                    "record" => prog.records.push(RecordDecl { name: name.unwrap(), fields }),
                    "inputs" => prog.inputs.extend(fields),
                    _ => prog.outputs.extend(fields),
                }
            }
            "task" => {
                let name = cur.ident("the task's name", "タスクの名前")?;
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
                cur.expect_end()?;
                p.pos += 1;
                let mut t = TaskDecl {
                    name,
                    params,
                    result,
                    binding: None,
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
                }
                prog.tasks.push(t);
            }
            "case" => {
                let name = cur.ident("the case's name", "案件の名前")?;
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
                        let n = cc.ident("an input of the rule", "規則の入力")?;
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
                            _ => return Err(err(cc.span(), "expected a value here", "ここには値が要ります")),
                        };
                        c.held.push((n, v));
                    } else if cc.eat_kw("external") {
                        c.external.extend(cc.names("an event", "出来事")?);
                    } else if cc.eat_kw("state") {
                        c.state_field = Some(cc.ident("a field", "フィールド")?);
                    } else if cc.eat_kw("refused") {
                        cc.expect_kw("when")?;
                        let o = cc.ident("an output of the rule", "規則の出力")?;
                        cc.expect_sym("=")?;
                        let v = cc.ident("a value", "値")?;
                        c.refused_when = Some((o, v));
                    } else {
                        return Err(err(
                            csp,
                            "under `case` write held, external, state or refused when",
                            "`case` の下には held・external・state・refused when を書きます",
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
                    return Err(err(sp, "a workflow has one `flow`", "`flow` は一つだけ書けます"));
                }
                let b = p.child_block(0, "`flow`", "`flow`", sp)?;
                prog.flow = Some((b, sp));
            }
            "on" => {
                cur.expect_kw("failure")?;
                cur.expect_end()?;
                p.pos += 1;
                if prog.on_failure.is_some() {
                    return Err(err(sp, "a workflow has one `on failure`", "`on failure` は一つだけ書けます"));
                }
                let b = p.child_block(0, "`on failure`", "`on failure`", sp)?;
                prog.on_failure = Some((b, sp));
            }
            other => {
                return Err(err(
                    sp,
                    format!("`{other}` does not start a declaration; expected workflow, description, kind, use, enum, record, inputs, outputs, task, case, flow or on failure"),
                    format!("`{other}` で始まる宣言はありません（workflow・description・kind・use・enum・record・inputs・outputs・task・case・flow・on failure）"),
                ))
            }
        }
    }
    Ok(prog)
}

fn task_clause(cc: &mut Cur, t: &mut TaskDecl) -> Result<(), Diag> {
    let sp = cc.span();
    let (kw, _) = cc.ident("a task clause", "タスクの項目")?;
    if matches!(kw.as_str(), "lambda" | "http" | "aws") {
        if let Some((_, first)) = &t.binding {
            return Err(Diag::error(
                "E007",
                sp.line,
                sp.col,
                format!("the task is already called another way (line {}); a task is called by one of lambda, http and aws", first.line),
                format!("このタスクの呼び方はもう書かれています（{} 行目）。呼び方は lambda・http・aws のどれか一つです", first.line),
            ));
        }
    }
    match kw.as_str() {
        "lambda" => {
            let f = cc.string("the Lambda function", "Lambda 関数")?.0;
            t.binding = Some((Binding::Lambda(f), sp));
        }
        "aws" => {
            let (service, action) = if let Some(Tok::Str(_)) = cc.peek() {
                let (text, tsp) = cc.string("the AWS API", "AWS の API")?;
                match text.split_once(':') {
                    Some((a, b)) if !a.is_empty() && !b.is_empty() => (a.to_string(), b.to_string()),
                    _ => return Err(err(tsp, "write the AWS API as <service>:<action>, such as sns:publish", "AWS の API は sns:publish のように <サービス>:<操作> と書きます")),
                }
            } else {
                let (a, _) = cc.ident("an AWS service, such as sns", "AWS のサービス（sns など）")?;
                cc.expect_sym(":")?;
                let (b, _) = cc.ident("an API action, such as publish", "API の操作（publish など）")?;
                (a, b)
            };
            t.binding = Some((Binding::Aws { service, action }, sp));
        }
        "queue" => t.queue = Some(cc.string("the task queue", "タスクキュー")?.0),
        "workflow" => {
            if cc.eat_kw("template") {
                t.argo_template = Some((cc.string("the WorkflowTemplate", "WorkflowTemplate")?.0, sp));
            } else {
                t.workflow = Some((cc.string("the type of the child workflow", "子ワークフローの型")?.0, sp));
            }
        }
        "image" => t.image = Some((cc.string("the container image", "コンテナのイメージ")?.0, sp)),
        "state" => {
            cc.expect_kw("machine")?;
            t.state_machine = Some((cc.string("the state machine's ARN", "ステートマシンの ARN")?.0, sp));
        }
        "durable" => {
            cc.expect_kw("function")?;
            t.durable_function = Some((cc.string("the durable function", "durable function")?.0, sp));
        }
        "http" => {
            let (m, msp) = cc.ident("an HTTP method", "HTTP のメソッド")?;
            let method = m.to_ascii_uppercase();
            if !["GET", "POST", "PUT", "PATCH", "DELETE"].contains(&method.as_str()) {
                return Err(err(msp, "the method is GET, POST, PUT, PATCH or DELETE", "メソッドは GET・POST・PUT・PATCH・DELETE のどれかです"));
            }
            let url = cc.string("the URL", "URL")?.0;
            let form = cc.eat_kw("form");
            t.binding = Some((Binding::Http { method, url, form }, sp));
        }
        "connection" => t.connection = Some(cc.string("the EventBridge connection", "EventBridge の接続")?.0),
        "errors" => loop {
            let name = cc.ident("an error name", "エラーの名前")?;
            let mut status = None;
            let mut exception = None;
            if cc.eat_sym("=") {
                let vsp = cc.span();
                match cc.peek().cloned() {
                    Some(Tok::Int(n)) => {
                        cc.i += 1;
                        if !(400..=599).contains(&n) {
                            return Err(err(vsp, "an error status is between 400 and 599", "エラーのステータスは 400 から 599 です"));
                        }
                        status = Some(n as u16);
                    }
                    Some(Tok::Ident(x)) | Some(Tok::Str(x)) => {
                        cc.i += 1;
                        exception = Some(x);
                    }
                    _ => return Err(err(vsp, "expected an HTTP status or the name of an AWS API's exception here", "ここには HTTP のステータスか、AWS の API の例外の名前が要ります")),
                }
            }
            t.errors.push(ErrDecl { name, status, exception });
            if !cc.eat_sym(",") {
                break;
            }
        },
        "retry" => {
            let (n, nsp) = cc.int("a number of retries", "やり直す回数")?;
            if n < 1 {
                return Err(err(nsp, "retry at least once, or leave `retry` out", "一回以上を書くか、`retry` を書かないでください"));
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
                        _ => return Err(err(bsp, "expected a number here", "ここには数が要ります")),
                    };
                    cc.i += 1;
                    if r.backoff < 1.0 {
                        return Err(err(bsp, "the backoff is 1 or more", "backoff は 1 以上です"));
                    }
                } else if cc.eat_kw("on") {
                    r.on = cc.names("an error name", "エラーの名前")?;
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
                t.key_param = Some(cc.ident("the parameter that takes the key", "キーを渡す引数")?);
            }
        }
        "idempotent" => t.idempotent = true,
        "callback" => t.callback = Some(sp),
        "starts" => {
            let machine = cc.qualname()?;
            let mut then = Vec::new();
            if cc.eat_kw("then") {
                then = cc.names("an event", "出来事")?;
            }
            t.machine = Some((MachineUse::Starts { machine, then }, sp));
        }
        "sends" => {
            let first = cc.ident("an event", "出来事")?;
            if cc.eat_sym("=") {
                let v = cc.ident("an event", "出来事")?;
                t.machine = Some((MachineUse::Sends { event: v, column: Some(first) }, sp));
            } else {
                t.machine = Some((MachineUse::Sends { event: first, column: None }, sp));
            }
        }
        "observes" => t.machine = Some((MachineUse::Observes, sp)),
        "refused" => {
            cc.expect_kw("as")?;
            t.refused_as = Some(cc.ident("an error name", "エラーの名前")?);
        }
        other => {
            return Err(err(
                sp,
                format!("`{other}` is not a task clause; expected lambda, http, aws, connection, queue, workflow, state machine, durable function, image, workflow template, errors, retry, timeout, key, idempotent, callback, starts, sends, observes or refused as"),
                format!("`{other}` はタスクの項目ではありません（lambda・http・aws・connection・queue・workflow・state machine・durable function・image・workflow template・errors・retry・timeout・key・idempotent・callback・starts・sends・observes・refused as）"),
            ))
        }
    }
    Ok(())
}
