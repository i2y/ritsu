//! Lines of tokens into a [`File`] (DESIGN 2.1, 9.2). The lexer says E001 (and E004 for a tab);
//! the parser says the rest of what is wrong with the shape of a file:
//!
//! - E002: where a name goes, something else is written (a string, a number, the end of the line).
//! - E003: the sections, or the lines of a block, out of their order; one written twice that is
//!   written once; a line a block needs that is missing; a block's head with nothing under it.
//! - E004: the indentation does not line up, or an indented line follows nothing that takes one.
//! - E005: a line that cannot be written where it is, or that is not of the form its first word
//!   takes.
//!
//! One error a line: after it, the rest of the line is passed over, and so are the lines under a
//! line that could not be read. A file with any of these is not handed on (`Parsed::file` is
//! None), so the names are not checked on a tree with holes in it.

use crate::ast::*;
use crate::diag::Diag;
use crate::kw;
use crate::lex::{self, Line, Tok, Token};
use ritsu_base::text::Text;
use std::collections::HashSet;

/// What the parser made of a file: the syntax tree, when the file has no syntax error, and the
/// diagnostics.
pub struct Parsed {
    pub file: Option<File>,
    pub diags: Vec<Diag>,
}

/// Parse the text of `path`.
pub fn parse(path: &str, src: &str) -> Parsed {
    let (lines, mut diags) = lex::lex(path, src);
    let bad: HashSet<usize> = diags.iter().filter_map(|d| d.line).collect();
    let mut p = Parser { path, src, diags: Vec::new() };
    let file = p.run(&lines, &bad);
    diags.append(&mut p.diags);
    crate::diag::sort(&mut diags);
    let file = if diags.iter().any(|d| d.is_error()) { None } else { file };
    Parsed { file, diags }
}

/// What is wrong inside a line: the column, the code and the message.
struct Bad {
    col: usize,
    code: &'static str,
    msg: Text,
    notes: Vec<Text>,
}

/// A line that is not of a form that can be written there (E005).
fn form(col: usize, msg: Text) -> Bad {
    Bad { col, code: "E005", msg, notes: vec![] }
}

impl Bad {
    fn note(mut self, t: Text) -> Bad {
        self.notes.push(t);
        self
    }
}

/// How a token reads in a message.
fn show(t: &Tok) -> String {
    match t {
        Tok::Word(w) => w.clone(),
        Tok::Str(s) => format!("\"{s}\""),
        Tok::Num(n) => n.raw.clone(),
        Tok::Date(d) => crate::types::day_text(*d),
        Tok::LParen => "(".into(),
        Tok::RParen => ")".into(),
        Tok::LBracket => "[".into(),
        Tok::RBracket => "]".into(),
        Tok::Colon => ":".into(),
        Tok::ColonColon => "::".into(),
        Tok::Comma => ",".into(),
        Tok::Dot => ".".into(),
        Tok::Pipe => "|".into(),
        Tok::Question => "?".into(),
        Tok::Eq => "=".into(),
        Tok::Lt => "<".into(),
        Tok::Le => "<=".into(),
        Tok::Gt => ">".into(),
        Tok::Ge => ">=".into(),
        Tok::Minus => "-".into(),
    }
}

/// A cursor over the tokens of one line.
struct Cur<'a> {
    line: &'a Line,
    i: usize,
}

impl<'a> Cur<'a> {
    fn new(line: &'a Line) -> Cur<'a> {
        Cur { line, i: 0 }
    }

    fn peek(&self) -> Option<&'a Tok> {
        self.line.tokens.get(self.i).map(|t| &t.tok)
    }

    fn peek_at(&self, k: usize) -> Option<&'a Tok> {
        self.line.tokens.get(self.i + k).map(|t| &t.tok)
    }

    fn token(&self) -> Option<&'a Token> {
        self.line.tokens.get(self.i)
    }

    fn bump(&mut self) -> Option<&'a Token> {
        let t = self.line.tokens.get(self.i);
        self.i += 1;
        t
    }

    /// The column of the next token, or just past the end of the line.
    fn col(&self) -> usize {
        match self.token() {
            Some(t) => t.col,
            None => self.line.chars.len() + 1,
        }
    }

    fn span(&self) -> Span {
        Span { line: self.line.no, col: self.col() }
    }

    fn at_end(&self) -> bool {
        self.i >= self.line.tokens.len()
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Word(x)) if x == w)
    }

    fn is_word_at(&self, k: usize, w: &str) -> bool {
        matches!(self.peek_at(k), Some(Tok::Word(x)) if x == w)
    }

    fn eat_word(&mut self, w: &str) -> bool {
        if self.is_word(w) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn found(&self) -> Text {
        match self.peek() {
            Some(t) => tr!("書かれているのは `{}` です", "found `{}`", show(t)),
            None => tr!("行が終わっています", "the line ends"),
        }
    }

    /// The line ends here.
    fn done(&self) -> Result<(), Bad> {
        match self.peek() {
            None => Ok(()),
            Some(t) => Err(form(self.col(), tr!("`{}` はここには書けません", "`{}` cannot be written here", show(t)))),
        }
    }

    fn want_word(&mut self, w: &str) -> Result<(), Bad> {
        if self.eat_word(w) {
            Ok(())
        } else {
            let f = self.found();
            Err(form(self.col(), tr!("ここには `{w}` が要ります（{}）", "`{w}` is needed here ({})", f.ja; f.en)))
        }
    }

    fn want(&mut self, t: &Tok, what: &str) -> Result<(), Bad> {
        if self.eat(t) {
            Ok(())
        } else {
            let f = self.found();
            Err(form(self.col(), tr!("ここには `{what}` が要ります（{}）", "`{what}` is needed here ({})", f.ja; f.en)))
        }
    }

    /// A name (E002 when something else is written).
    fn name(&mut self, what: &Text) -> Result<(String, Span), Bad> {
        let span = self.span();
        match self.peek() {
            Some(Tok::Word(w)) => {
                self.i += 1;
                Ok((w.clone(), span))
            }
            _ => {
                let f = self.found();
                Err(Bad { col: span.col, code: "E002", msg: tr!("ここには{}の名前を書いてください（{}）", "{} goes here, by name ({})", what.ja, f.ja; what.en, f.en), notes: vec![] })
            }
        }
    }

    fn reference(&mut self, what: &Text) -> Result<Ref, Bad> {
        let (word, span) = self.name(what)?;
        Ok(Ref { word, span })
    }

    /// `a, b, c`: one or more.
    fn refs(&mut self, what: &Text) -> Result<Vec<Ref>, Bad> {
        let mut out = vec![self.reference(what)?];
        while self.eat(&Tok::Comma) {
            out.push(self.reference(what)?);
        }
        Ok(out)
    }

    fn string(&mut self, what: &Text) -> Result<(String, Span), Bad> {
        let span = self.span();
        match self.peek() {
            Some(Tok::Str(s)) => {
                self.i += 1;
                Ok((s.clone(), span))
            }
            _ => {
                let f = self.found();
                Err(form(span.col, tr!("ここには{}を `\"…\"` で書いてください（{}）", "{} goes here, in quotes: `\"…\"` ({})", what.ja, f.ja; what.en, f.en)))
            }
        }
    }

    /// `name` or `name(alias)`.
    fn declared(&mut self, what: &Text) -> Result<Name, Bad> {
        let (text, span) = self.name(what)?;
        let mut alias = None;
        if self.peek() == Some(&Tok::LParen) {
            self.bump();
            let s = self.span();
            let (a, _) = self.name(&tr!("別名", "the alias"))?;
            self.want(&Tok::RParen, ")")?;
            alias = Some((a, s));
        }
        Ok(Name { text, alias, span })
    }

    /// A constant: a number with its unit (a `-` before it for one below zero), a date, `true`, `false`.
    fn literal(&mut self) -> Result<(Lit, Span), Bad> {
        let span = self.span();
        let minus = self.eat(&Tok::Minus);
        match self.peek() {
            Some(Tok::Num(n)) => {
                let mut n = n.clone();
                self.bump();
                if minus {
                    n.value = ritsu_units::Rat::zero().sub(n.value);
                    n.raw = format!("-{}", n.raw);
                }
                Ok((Lit::Num(n), span))
            }
            Some(Tok::Date(d)) if !minus => {
                let d = *d;
                self.bump();
                Ok((Lit::Date(d), span))
            }
            Some(Tok::Word(w)) if !minus && (w == kw::TRUE || w == kw::FALSE) => {
                let b = w == kw::TRUE;
                self.bump();
                Ok((Lit::Bool(b), span))
            }
            _ => {
                let f = self.found();
                Err(form(self.col(), tr!("ここには定数（`50GBP`、`2026-10-01`、`true` など）が要ります（{}）", "a constant goes here (`50GBP`, `2026-10-01`, `true`, …) ({})", f.ja; f.en)))
            }
        }
    }

    fn starts_literal(&self) -> bool {
        match self.peek() {
            Some(Tok::Num(_) | Tok::Date(_) | Tok::Minus) => true,
            Some(Tok::Word(w)) => w == kw::TRUE || w == kw::FALSE,
            _ => false,
        }
    }
}

/// `range >=1GBP <=10_000GBP`, after `range`: one end or both, in either order.
fn bounds(c: &mut Cur, span: Span) -> Result<Range, Bad> {
    let (mut lo, mut hi) = (None, None);
    loop {
        let end = match c.peek() {
            Some(Tok::Ge) if lo.is_none() => &mut lo,
            Some(Tok::Le) if hi.is_none() => &mut hi,
            Some(Tok::Gt | Tok::Lt) => {
                return Err(form(c.col(), tr!("範囲の端は `>=` と `<=` で書いてください（端の値も範囲に入ります）", "the ends of a range are written with `>=` and `<=` (an end is in the range)")));
            }
            _ => break,
        };
        c.bump();
        *end = Some(c.literal()?);
    }
    if lo.is_none() && hi.is_none() {
        let f = c.found();
        return Err(form(c.col(), tr!("範囲は `range >=<下限> <=<上限>` と書いてください（{}）", "a range is written `range >=<low> <=<high>` ({})", f.ja; f.en)));
    }
    Ok(Range { span, lo, hi })
}

/// `refund_limit : money[GBP, incl_tax]  range >=0GBP <=10_000GBP`.
fn field(c: &mut Cur, what: &Text) -> Result<Field, Bad> {
    let span = c.span();
    let name = c.declared(what)?;
    if c.peek() != Some(&Tok::Colon) {
        let f = c.found();
        return Err(form(c.col(), tr!("名前のあとに `:` と型を書いてください（{}）", "the name is followed by `:` and the type ({})", f.ja; f.en))
            .note(tr!("`suspended : bool`、`amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP` のように書きます。", "Write it like `suspended : bool` or `amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP`.")));
    }
    c.bump();
    let ty_span = c.span();
    let ty = match c.peek() {
        Some(Tok::Word(w)) => {
            let w = w.clone();
            let start = c.col();
            c.bump();
            if c.peek() == Some(&Tok::LBracket) {
                // a unit, as rulec writes it, up to its `]`
                let mut close = None;
                while let Some(t) = c.bump() {
                    if t.tok == Tok::RBracket {
                        close = Some(t.end);
                        break;
                    }
                }
                let Some(end) = close else {
                    return Err(form(ty_span.col, tr!("`[` が閉じていません（`]` が足りません）", "the `[` is not closed (a `]` is missing)")));
                };
                Type::Unit(c.line.slice(start, end))
            } else if w == kw::BOOL {
                Type::Bool
            } else if w == kw::DATE {
                Type::Date
            } else if w == kw::NUMBER || w == kw::RATE {
                Type::Unit(w)
            } else {
                Type::Named(Ref { word: w, span: ty_span })
            }
        }
        _ => {
            let f = c.found();
            return Err(form(c.col(), tr!("`:` のあとに型を書いてください（{}）", "the type goes after `:` ({})", f.ja; f.en)));
        }
    };
    let optional = c.eat(&Tok::Question);
    let range = if c.is_word(kw::RANGE) {
        let s = c.span();
        c.bump();
        Some(bounds(c, s)?)
    } else {
        None
    };
    c.done()?;
    Ok(Field { name, span, ty, ty_span, optional, range })
}

/// One input given to a rule or a date function: `amount: amount`, `limit: principal.refund_limit`.
fn arg(c: &mut Cur) -> Result<Arg, Bad> {
    let name = c.reference(&tr!("規則か日付の関数の入力", "the input of the rule or the date function"))?;
    c.want(&Tok::Colon, ":")?;
    let value = if c.is_word(kw::TODAY) {
        let s = c.span();
        c.bump();
        ArgValue::Today(s)
    } else if c.starts_literal() {
        let (l, s) = c.literal()?;
        ArgValue::Lit(l, s)
    } else {
        ArgValue::Path(path(c)?)
    };
    Ok(Arg { name, value })
}

/// `(a: x, b: y)`, after the name of the call.
fn args(c: &mut Cur) -> Result<Vec<Arg>, Bad> {
    c.want(&Tok::LParen, "(")?;
    let mut out = Vec::new();
    if !c.eat(&Tok::RParen) {
        loop {
            out.push(arg(c)?);
            if c.eat(&Tok::RParen) {
                break;
            }
            c.want(&Tok::Comma, ",")?;
        }
    }
    Ok(out)
}

/// What a computed value is, after its `=` (DESIGN 3.2, 3.3).
fn computation(c: &mut Cur) -> Result<Computation, Bad> {
    if c.eat_word(kw::TODAY) {
        let col = c.col();
        let op = match c.peek() {
            Some(Tok::Lt) => Op::Lt,
            Some(Tok::Le) => Op::Le,
            Some(Tok::Gt) => Op::Gt,
            Some(Tok::Ge) => Op::Ge,
            Some(Tok::Word(w)) if w == kw::IS => Op::Is,
            _ => {
                let f = c.found();
                return Err(form(col, tr!("`today` のあとには `<`、`<=`、`>`、`>=`、`is` のどれかを書いてください（{}）", "`today` is followed by `<`, `<=`, `>`, `>=` or `is` ({})", f.ja; f.en)));
            }
        };
        c.bump();
        // `today is open in uk`; a dates file named `open` is followed by its `.`
        if op == Op::Is && c.is_word(kw::OPEN) && c.is_word_at(1, kw::IN) {
            c.bump();
            c.bump();
            let calendar = c.reference(&tr!("カレンダー（`use calendar` の名前）", "the calendar (the name of a `use calendar`)"))?;
            return Ok(Computation::Open { calendar });
        }
        let date = if (c.is_word(kw::PRINCIPAL) || c.is_word(kw::RESOURCE)) && c.peek_at(1) == Some(&Tok::Dot) {
            DateValue::Attr(path(c)?)
        } else {
            let dates = c.reference(&tr!("日付のファイル（`use dates` の名前）", "the dates file (the name of a `use dates`)"))?;
            if c.peek() != Some(&Tok::Dot) {
                let f = c.found();
                return Err(form(c.col(), tr!(
                    "日付は `<use dates の名前>.<日付>(<入力>: <値>)` か、`resource.<日付の属性>` で書いてください（{}）",
                    "a date is `<dates>.<date>(<input>: <value>)`, or `resource.<a date attribute>` ({})",
                    f.ja;
                    f.en
                )));
            }
            c.bump();
            let date = c.reference(&tr!("日付の関数", "the date function"))?;
            let args = args(c)?;
            DateValue::Call { dates, date, args }
        };
        return Ok(Computation::Date { op, date });
    }
    let rule = match c.peek() {
        Some(Tok::Word(_)) => c.reference(&tr!("規則", "the rule"))?,
        _ => {
            let f = c.found();
            return Err(form(c.col(), tr!(
                "`=` のあとには、規則の出力（`<規則>(<入力>: <値>).<出力>`）か、日付の比べ方（`today <= …`、`today is open in …`）を書いてください（{}）",
                "`=` is followed by a rule's output (`<rule>(<input>: <value>).<output>`) or a test of today (`today <= …`, `today is open in …`) ({})",
                f.ja;
                f.en
            )));
        }
    };
    let args = args(c)?;
    if c.peek() != Some(&Tok::Dot) {
        let f = c.found();
        return Err(form(c.col(), tr!("規則の呼び出しのあとに `.<出力>` を書いてください（{}）", "the call of the rule is followed by `.<output>` ({})", f.ja; f.en)));
    }
    c.bump();
    let output = c.reference(&tr!("規則の出力", "the rule's output"))?;
    Ok(Computation::Rule { rule, args, output })
}

/// `principal.suspended`, `resource.status`, or a bare name (an input or a computed value).
fn path(c: &mut Cur) -> Result<Path, Bad> {
    let head = match c.peek() {
        Some(Tok::Word(w)) if w == kw::PRINCIPAL || w == kw::RESOURCE => Some(w == kw::PRINCIPAL),
        _ => None,
    };
    if let Some(principal) = head {
        let word = if principal { kw::PRINCIPAL } else { kw::RESOURCE };
        c.bump();
        if c.peek() != Some(&Tok::Dot) {
            let f = c.found();
            return Err(form(c.col(), tr!("`{word}` のあとには `.<属性>` を書いてください（{}）", "`{word}` is followed by `.<attribute>` ({})", f.ja; f.en)));
        }
        c.bump();
        let r = c.reference(&tr!("属性", "the attribute"))?;
        // `resource.order.customer`: a relation of two steps, which v1 does not write (DESIGN 2.5)
        if c.peek() == Some(&Tok::Dot) {
            let shown = format!("{word}.{}", r.word);
            return Err(Bad {
                col: c.col(),
                code: "E104",
                msg: tr!("v1 の関係は一段だけです。`{shown}` からさらに属性をたどれません", "A relation goes one step in v1, and nothing goes on from `{shown}`"),
                notes: vec![tr!(
                    "関係は一段だけたどれます（`resource.customer is principal`、`resource.tenant is principal.tenant`、`principal in resource.team`）。二段たどる答えが要るなら、それを計算する規則の真偽の出力にしてください。",
                    "A relation goes one step (`resource.customer is principal`, `resource.tenant is principal.tenant`, `principal in resource.team`); where two steps are needed, make the answer a true-or-false output of a rule that computes it."
                )],
            });
        }
        return Ok(if principal { Path::Principal(r) } else { Path::Resource(r) });
    }
    match c.peek() {
        Some(Tok::Word(w)) if !kw::is_reserved(w) => Ok(Path::Local(c.reference(&tr!("値", "the value"))?)),
        _ => {
            let f = c.found();
            Err(form(c.col(), tr!(
                "ここには、条件が読む値（`principal.<属性>`、`resource.<属性>`、入力か計算した値の名前）が要ります（{}）",
                "a value goes here (`principal.<attribute>`, `resource.<attribute>`, or the name of an input or a computed value) ({})",
                f.ja;
                f.en
            )))
        }
    }
}

/// The right side of `is`.
fn rhs(c: &mut Cur) -> Result<Rhs, Bad> {
    if c.is_word(kw::PRINCIPAL) && c.peek_at(1) != Some(&Tok::Dot) {
        let s = c.span();
        c.bump();
        return Ok(Rhs::Principal(s));
    }
    if (c.is_word(kw::PRINCIPAL) || c.is_word(kw::RESOURCE)) && c.peek_at(1) == Some(&Tok::Dot) {
        return Ok(Rhs::Path(path(c)?));
    }
    if c.starts_literal() {
        let (l, s) = c.literal()?;
        return Ok(Rhs::Lit(l, s));
    }
    match c.peek() {
        Some(Tok::Word(w)) if !kw::is_reserved(w) => Ok(Rhs::Word(c.reference(&tr!("値", "the value"))?)),
        _ => {
            let f = c.found();
            Err(form(c.col(), tr!(
                "`is` のあとには、列挙の値、定数、`principal`、`principal.<属性>`、`resource.<属性>` のどれかを書いてください（{}）",
                "`is` is followed by a value of an enum, a constant, `principal`, `principal.<attribute>` or `resource.<attribute>` ({})",
                f.ja;
                f.en
            )))
        }
    }
}

/// One condition (DESIGN 3.1).
fn atom(c: &mut Cur) -> Result<(Atom, Span), Bad> {
    let span = c.span();
    if c.is_word(kw::PRINCIPAL) && c.peek_at(1) != Some(&Tok::Dot) {
        c.bump();
        if c.eat_word(kw::IN) {
            if (c.is_word(kw::PRINCIPAL) || c.is_word(kw::RESOURCE)) && c.peek_at(1) == Some(&Tok::Dot) {
                return Ok((Atom::InGroup(path(c)?), span));
            }
            return Ok((Atom::InRole(c.reference(&tr!("役割", "the role"))?), span));
        }
        if c.eat_word(kw::IS) {
            if c.eat_word(kw::WORKFLOW) {
                return Ok((Atom::IsWorkflow(c.reference(&tr!("ワークフロー", "the workflow"))?), span));
            }
            return Ok((Atom::IsType(c.reference(&tr!("型", "the type"))?), span));
        }
        let f = c.found();
        return Err(form(c.col(), tr!("`principal` のあとには `in`、`is`、`.<属性>` のどれかを書いてください（{}）", "`principal` is followed by `in`, `is` or `.<attribute>` ({})", f.ja; f.en)));
    }
    let left = path(c)?;
    if c.eat_word(kw::IS) {
        let not = c.eat_word(kw::NOT);
        let right = rhs(c)?;
        return Ok((Atom::Is { left, not, right }, span));
    }
    let op = match c.peek() {
        Some(Tok::Lt) => Some(Op::Lt),
        Some(Tok::Le) => Some(Op::Le),
        Some(Tok::Gt) => Some(Op::Gt),
        Some(Tok::Ge) => Some(Op::Ge),
        Some(Tok::Eq) => {
            return Err(form(c.col(), tr!("等しいことは `=` ではなく `is` で書いてください", "the same is written `is`, not `=`")));
        }
        _ => None,
    };
    if let Some(op) = op {
        c.bump();
        let right = c.literal()?;
        return Ok((Atom::Compare { left, op, right }, span));
    }
    Ok((Atom::Holds(left), span))
}

fn factor(c: &mut Cur) -> Result<Expr, Bad> {
    if c.is_word(kw::NOT) {
        let s = c.span();
        c.bump();
        return Ok(Expr::Not(Box::new(factor(c)?), s));
    }
    if c.eat(&Tok::LParen) {
        let e = expr(c)?;
        c.want(&Tok::RParen, ")")?;
        return Ok(e);
    }
    let (a, s) = atom(c)?;
    Ok(Expr::Atom(a, s))
}

fn term(c: &mut Cur) -> Result<Expr, Bad> {
    let mut parts = vec![factor(c)?];
    while c.eat_word(kw::AND) {
        parts.push(factor(c)?);
    }
    Ok(if parts.len() == 1 { parts.pop().unwrap() } else { Expr::And(parts) })
}

fn expr(c: &mut Cur) -> Result<Expr, Bad> {
    let mut parts = vec![term(c)?];
    while c.eat_word(kw::OR) {
        parts.push(term(c)?);
    }
    Ok(if parts.len() == 1 { parts.pop().unwrap() } else { Expr::Or(parts) })
}

/// The block indented lines belong to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Block {
    /// A section that takes no indented lines (`use`, `enum`, …).
    None,
    /// A section whose head could not be read, or is out of place: its lines are passed over.
    Skip,
    Role(usize),
    Principal(usize),
    Workflow(usize),
    Resource(usize),
    Action(usize),
    Policy(usize),
    Expect(usize),
    Separate(usize),
}

/// The block under a line of a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sub {
    None,
    /// Under a line that could not be read: passed over.
    Skip,
    Attributes,
    Input,
    Context,
}

impl Block {
    /// The word of the head, and the words that start its lines, in their order.
    fn words(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Block::Role(_) => (kw::ROLE, &[kw::DESCRIPTION, kw::INCLUDES, kw::CAN]),
            Block::Principal(_) => (kw::PRINCIPAL, &[kw::DESCRIPTION, kw::ROLES, kw::ATTRIBUTES]),
            Block::Workflow(_) => (kw::WORKFLOW, &[kw::DESCRIPTION]),
            Block::Resource(_) => (kw::RESOURCE, &[kw::DESCRIPTION, kw::ATTRIBUTES]),
            Block::Action(_) => (kw::ACTION, &[kw::DESCRIPTION, kw::GUARDS, kw::PRINCIPAL, kw::RESOURCE, kw::NOBODY, kw::INPUT, kw::CONTEXT]),
            Block::Policy(_) => ("permit", &[kw::DESCRIPTION, kw::PRINCIPAL, kw::ACTION, kw::WHEN]),
            Block::Expect(_) => (kw::EXPECT, &[kw::DESCRIPTION, kw::PRINCIPAL, kw::ACTION, kw::WHEN]),
            Block::Separate(_) => (kw::SEPARATE, &[kw::DESCRIPTION, kw::ACTIONS]),
            Block::None | Block::Skip => ("", &[]),
        }
    }

    /// Where a line's first word goes in the block (`unless` with `when`).
    fn rank(self, w: &str) -> Option<usize> {
        let w = if w == kw::UNLESS { kw::WHEN } else { w };
        self.words().1.iter().position(|x| *x == w)
    }

    /// How the lines of the block go, for a note.
    fn order(self, head: &str) -> Text {
        let ws: Vec<Text> = self.words().1.iter().map(|w| if *w == kw::WHEN { tr!("`when` と `unless`", "`when` and `unless`") } else { Text::same(format!("`{w}`")) }).collect();
        let list = Text::list(&ws);
        tr!("`{head}` の中は、{} の順に書いてください。", "Under `{head}`, the lines go: {}.", list.ja; list.en)
    }
}

/// The words that start a line at the left margin, in their order (DESIGN 2.1).
fn top_rank(w: &str) -> Option<u8> {
    Some(match w {
        kw::DESCRIPTION => 1,
        kw::NAMESPACE => 2,
        kw::USE => 3,
        kw::TODAY => 4,
        kw::ENUM => 5,
        kw::ROLE => 6,
        kw::PRINCIPAL => 7,
        kw::WORKFLOW => 8,
        kw::RESOURCE => 9,
        kw::ACTION => 10,
        kw::PERMIT | kw::FORBID => 11,
        kw::EXPECT => 12,
        kw::SEPARATE => 13,
        _ => return None,
    })
}

fn top_order() -> Text {
    tr!(
        "ファイルは、見出し（`gate`）、`description`、`namespace`、`use`、`today`、`enum`、`role`、`principal`、`workflow`、`resource`、`action`、`permit` と `forbid`、`expect`、`separate` の順に書いてください。",
        "A file goes: the heading (`gate`), `description`, `namespace`, `use`, `today`, `enum`, `role`, `principal`, `workflow`, `resource`, `action`, `permit` and `forbid`, `expect`, `separate`."
    )
}

/// The words that start a line inside a block only.
const BLOCK_ONLY: &[&str] = &[kw::INCLUDES, kw::CAN, kw::ROLES, kw::ATTRIBUTES, kw::GUARDS, kw::NOBODY, kw::INPUT, kw::CONTEXT, kw::ACTIONS, kw::WHEN, kw::UNLESS];

/// What the parser keeps of the block it is in.
struct Open {
    block: Block,
    /// The head's line, and the last line of the block so far.
    head: usize,
    last: usize,
    indent: Option<usize>,
    last_rank: usize,
    seen: Vec<&'static str>,
    sub: Sub,
    sub_indent: Option<usize>,
    /// The line, the column and the word of the head of the block under a line, and how many lines
    /// it has.
    sub_head: Option<(usize, usize, &'static str)>,
    sub_count: usize,
    /// A line of the block could not be read: what the block lacks is not said.
    broken: bool,
}

impl Open {
    fn new(block: Block, head: usize) -> Open {
        Open { block, head, last: head, indent: None, last_rank: 0, seen: Vec::new(), sub: Sub::None, sub_indent: None, sub_head: None, sub_count: 0, broken: false }
    }
}

struct Parser<'a> {
    path: &'a str,
    src: &'a str,
    diags: Vec<Diag>,
}

impl<'a> Parser<'a> {
    fn err(&mut self, code: &'static str, line: usize, col: usize, msg: Text) -> &mut Diag {
        self.diags.push(Diag::error(code, self.path, line, col, msg).source(self.src));
        self.diags.last_mut().unwrap()
    }

    fn bad(&mut self, line: usize, b: Bad) {
        let mut d = Diag::error(b.code, self.path, line, b.col, b.msg).source(self.src);
        d.notes = b.notes;
        self.diags.push(d);
    }

    fn run(&mut self, lines: &[Line], bad: &HashSet<usize>) -> Option<File> {
        let mut it = lines.iter().filter(|l| !l.tokens.is_empty() || bad.contains(&l.no));
        // The heading.
        let Some(first) = it.next() else {
            self.err("E003", 1, 1, tr!("ファイルが空です。一行目は `gate <名前> v1` です", "The file is empty; its first line is `gate <name> v1`"));
            return None;
        };
        if bad.contains(&first.no) {
            return None;
        }
        let t0 = &first.tokens[0];
        if first.indent != 0 || t0.tok != Tok::Word(kw::GATE.into()) {
            self.err("E003", first.no, t0.col, tr!("ファイルは `gate` の行で始めてください", "A file starts with a `gate` line")).notes.push(tr!(
                "`gate refunds v1` のように、`gate`、ファイルの名前、バージョンの順に書き始めてください。日本語の名前には `gate 返金(refunds) v1` のように ASCII の別名を付けます。",
                "Start it like `gate refunds v1`: `gate`, the file's name, and its version. A Japanese name takes an ASCII alias: `gate 返金(refunds) v1`."
            ));
            return None;
        }
        let mut c = Cur::new(first);
        c.bump();
        let header = (|| -> Result<(Name, String), Bad> {
            let n = c.declared(&tr!("ファイル", "the file"))?;
            let vcol = c.col();
            let v = match c.peek() {
                Some(Tok::Word(v)) if v.len() > 1 && v.starts_with('v') && v[1..].chars().all(|ch| ch.is_ascii_digit()) => v.clone(),
                _ => {
                    let f = c.found();
                    return Err(form(vcol, tr!("見出しの最後には、`v1` のようなバージョンを書いてください（{}）", "the heading ends with a version, such as `v1` ({})", f.ja; f.en)));
                }
            };
            c.bump();
            c.done()?;
            Ok((n, v))
        })();
        let (name, version) = match header {
            Ok(x) => x,
            Err(b) => {
                self.bad(first.no, b);
                (Name { text: String::new(), alias: None, span: Span { line: first.no, col: 1 } }, String::new())
            }
        };
        let mut f = File {
            path: self.path.to_string(),
            src: self.src.to_string(),
            name,
            version,
            description: None,
            namespace: None,
            uses: vec![],
            today: None,
            enums: vec![],
            roles: vec![],
            principals: vec![],
            workflows: vec![],
            resources: vec![],
            actions: vec![],
            policies: vec![],
            expects: vec![],
            separates: vec![],
        };
        let mut open = Open::new(Block::None, first.no);
        let mut last_rank = 0u8;
        let mut seen: Vec<String> = Vec::new();
        for line in it {
            let indented = line.chars.first().is_some_and(|c| *c == ' ' || *c == '\t');
            if bad.contains(&line.no) {
                // a line the lexer could not read: what is under it is passed over, and what the
                // block lacks is not said on top of it
                if !indented {
                    self.close(&mut f, &mut open);
                    open = Open::new(Block::Skip, line.no);
                } else {
                    open.last = line.no;
                    open.broken = true;
                    match open.indent {
                        Some(bi) if line.indent > bi && open.sub != Sub::None => open.sub_count += 1,
                        _ => {
                            self.close_sub(&mut open);
                            open.sub = Sub::Skip;
                        }
                    }
                }
                continue;
            }
            if indented {
                self.indented(&mut f, &mut open, line);
                continue;
            }
            // A line at the left margin starts a section.
            self.close(&mut f, &mut open);
            let t0 = &line.tokens[0];
            open = Open::new(Block::Skip, line.no);
            let Tok::Word(w) = &t0.tok else {
                self.err("E005", line.no, t0.col, tr!("行の初めに `{}` は書けません", "A line cannot start with `{}`", show(&t0.tok))).notes.push(top_order());
                continue;
            };
            let w = w.as_str();
            if w == kw::GATE {
                self.err("E003", line.no, t0.col, tr!("見出しは一つのファイルに一つです", "A file has one heading"));
                continue;
            }
            let Some(r) = top_rank(w) else {
                let d = self.err("E005", line.no, t0.col, tr!("`{w}` は行の初めに書く語ではありません", "`{w}` is not a word a line starts with"));
                if BLOCK_ONLY.contains(&w) {
                    d.notes.push(tr!("`{w}` の行は、それを取るブロックの下に字下げして書きます。", "A `{w}` line is indented under the block that takes it."));
                } else {
                    d.notes.push(top_order());
                }
                continue;
            };
            if [kw::DESCRIPTION, kw::NAMESPACE, kw::TODAY].contains(&w) && seen.iter().any(|s| s == w) {
                self.err("E003", line.no, t0.col, tr!("`{w}` が二度書かれています", "`{w}` is written twice"));
                continue;
            }
            if r < last_rank {
                self.err("E003", line.no, t0.col, tr!("`{w}` の位置が違います", "`{w}` is out of place")).notes.push(top_order());
                continue;
            }
            last_rank = r;
            seen.push(w.to_string());
            match self.top(&mut f, w, line) {
                Ok(block) => open = Open::new(block, line.no),
                Err(b) => self.bad(line.no, b),
            }
        }
        self.close(&mut f, &mut open);
        Some(f)
    }

    /// An indented line: a line of the block above, or of the block under one of its lines.
    fn indented(&mut self, f: &mut File, open: &mut Open, line: &Line) {
        match open.block {
            Block::Skip => return,
            Block::None => {
                self.err("E004", line.no, line.indent + 1, tr!("この行は字下げされていますが、上の行は字下げした行を取りません", "This line is indented, but the line above takes no indented lines")).notes.push(tr!(
                    "字下げした行を取るのは、`role`、`principal`、`workflow`、`resource`、`action`、`permit`、`forbid`、`expect`、`separate` と、その中の `attributes`、`input`、`context` です。",
                    "The lines that take indented lines are `role`, `principal`, `workflow`, `resource`, `action`, `permit`, `forbid`, `expect` and `separate`, and inside them `attributes`, `input` and `context`."
                ));
                return;
            }
            _ => {}
        }
        open.last = line.no;
        let bi = *open.indent.get_or_insert(line.indent);
        if line.indent == bi {
            self.close_sub(open);
            let first = match &line.tokens[0].tok {
                Tok::Word(w) => w.clone(),
                _ => String::new(),
            };
            match self.block_line(f, open, line) {
                Ok(sub) => {
                    open.sub = sub;
                    if sub != Sub::None {
                        let w = match sub {
                            Sub::Attributes => kw::ATTRIBUTES,
                            Sub::Input => kw::INPUT,
                            _ => kw::CONTEXT,
                        };
                        open.sub_head = Some((line.no, line.tokens[0].col, w));
                    }
                }
                Err(b) => {
                    self.bad(line.no, b);
                    if [kw::ATTRIBUTES, kw::INPUT, kw::CONTEXT].contains(&first.as_str()) {
                        open.sub = Sub::Skip;
                    }
                }
            }
            return;
        }
        if line.indent > bi && open.sub != Sub::None {
            if open.sub == Sub::Skip {
                return;
            }
            let si = *open.sub_indent.get_or_insert(line.indent);
            if line.indent != si {
                self.err("E004", line.no, line.indent + 1, tr!(
                    "字下げが上の行とそろっていません（上は {si} 文字、この行は {} 文字）",
                    "The indentation does not line up with the line above ({si} spaces there, {} here)",
                    line.indent
                ));
                return;
            }
            open.sub_count += 1;
            if let Err(b) = self.sub_line(f, open, line) {
                self.bad(line.no, b);
            }
            return;
        }
        let msg = if line.indent > bi {
            tr!(
                "字下げが上の行とそろっていません（ブロックの行は {bi} 文字、この行は {} 文字）。もう一段の字下げを取るのは `attributes`、`input`、`context` だけです",
                "The indentation does not line up ({bi} spaces for the lines of the block, {} here); only `attributes`, `input` and `context` take lines indented further",
                line.indent
            )
        } else {
            tr!("字下げが上の行とそろっていません（ブロックの行は {bi} 文字、この行は {} 文字）", "The indentation does not line up ({bi} spaces for the lines of the block, {} here)", line.indent)
        };
        self.err("E004", line.no, line.indent + 1, msg);
    }

    /// The end of the block under a line: it has at least one line.
    fn close_sub(&mut self, open: &mut Open) {
        if let (Some((line, col, w)), 0) = (open.sub_head, open.sub_count) {
            let example = match w {
                kw::ATTRIBUTES => tr!("`    suspended : bool`", "`    suspended : bool`"),
                kw::INPUT => tr!("`    amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP`", "`    amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP`"),
                _ => tr!("`    in_period = today <= refund_terms.last_day(paid_on: resource.paid_on)`", "`    in_period = today <= refund_terms.last_day(paid_on: resource.paid_on)`"),
            };
            self.err("E003", line, col, tr!("`{w}` の下に行がありません", "`{w}` has no lines under it")).notes.push(tr!(
                "下に一段深く字下げして、{} のように書いてください。要らなければ `{w}` の行を消してください。",
                "Write them under it, indented one step further, like {}; or remove the `{w}` line if there are none.",
                example.ja;
                example.en
            ));
        }
        open.sub = Sub::None;
        open.sub_indent = None;
        open.sub_head = None;
        open.sub_count = 0;
    }

    /// The end of a block: the lines it needs, and its last line.
    fn close(&mut self, f: &mut File, open: &mut Open) {
        self.close_sub(open);
        let need = |w: &'static str| !open.broken && !open.seen.contains(&w);
        let mut missing: Vec<(&str, Text)> = Vec::new();
        match open.block {
            Block::Action(i) => {
                let a = &mut f.actions[i];
                a.lines.1 = open.last;
                if need(kw::PRINCIPAL) {
                    missing.push((kw::PRINCIPAL, tr!("`  principal User, Workflow` のように、リクエストを出しうる principal の型を並べてください。", "List the principal types a request can come from, like `  principal User, Workflow`.")));
                }
                if need(kw::RESOURCE) {
                    missing.push((kw::RESOURCE, tr!("`  resource Order from orderId` のように、resource の型を書いてください。", "Write the resource types, like `  resource Order from orderId`.")));
                }
            }
            Block::Policy(i) => {
                f.policies[i].lines.1 = open.last;
                if need(kw::ACTION) {
                    missing.push((kw::ACTION, tr!("`  action refund_order` か `  action any` のように、ポリシーが効く action を書いてください。", "Write the actions it applies to, like `  action refund_order` or `  action any`.")));
                }
            }
            Block::Expect(i) => {
                f.expects[i].lines.1 = open.last;
                if need(kw::ACTION) {
                    missing.push((kw::ACTION, tr!("`  action refund_order` のように、期待が選ぶ action を書いてください。", "Write the actions it picks, like `  action refund_order`.")));
                }
            }
            Block::Separate(i) => {
                f.separates[i].lines.1 = open.last;
                if need(kw::ACTIONS) {
                    missing.push((kw::ACTIONS, tr!("`  actions refund_order, export_refunds` のように、同じ人に両方は許さない action を二つ以上並べてください。", "List two or more actions no one is allowed together, like `  actions refund_order, export_refunds`.")));
                }
            }
            Block::Role(i) => f.roles[i].lines.1 = open.last,
            Block::Principal(i) => f.principals[i].lines.1 = open.last,
            Block::Resource(i) => f.resources[i].lines.1 = open.last,
            Block::Workflow(i) => f.workflows[i].lines.1 = open.last,
            Block::None | Block::Skip => {}
        }
        let head = match open.block {
            Block::Policy(i) => f.policies[i].effect.word(),
            b => b.words().0,
        };
        for (w, note) in missing {
            self.err("E003", open.head, 1, tr!("`{head}` に `{w}` の行がありません", "This `{head}` has no `{w}` line")).notes.push(note);
        }
        open.block = Block::None;
    }

    /// A line at the left margin: the section it starts.
    fn top(&mut self, f: &mut File, w: &str, line: &Line) -> Result<Block, Bad> {
        let mut c = Cur::new(line);
        let span = c.span();
        c.bump();
        let lines = (line.no, line.no);
        match w {
            kw::DESCRIPTION => {
                f.description = Some(c.string(&tr!("説明", "the description"))?);
                c.done()?;
                Ok(Block::None)
            }
            kw::NAMESPACE => {
                let s = c.span();
                let mut segs = vec![c.name(&tr!("名前空間", "the namespace"))?.0];
                while c.eat(&Tok::ColonColon) {
                    segs.push(c.name(&tr!("名前空間", "the namespace"))?.0);
                }
                c.done()?;
                f.namespace = Some((segs, s));
                Ok(Block::None)
            }
            kw::USE => {
                let kcol = c.col();
                let kind = match c.peek() {
                    Some(Tok::Word(k)) => UseKind::of(k),
                    _ => None,
                };
                let Some(kind) = kind else {
                    let fd = c.found();
                    return Err(form(kcol, tr!(
                        "`use` のあとには `rule`、`dates`、`calendar`、`openapi`、`proto`、`asyncapi`、`book`、`gate` のどれかを書いてください（{}）",
                        "`use` is followed by `rule`, `dates`, `calendar`, `openapi`, `proto`, `asyncapi`, `book` or `gate` ({})",
                        fd.ja;
                        fd.en
                    )));
                };
                c.bump();
                let name = if kind == UseKind::Gate {
                    None
                } else {
                    let n = c.name(&tr!("読むもの（このファイルの中で使う名前）", "what is read (the name the file uses it by)"))?;
                    c.want_word(kw::FROM)?;
                    Some(n)
                };
                let p = c.string(&tr!("ファイル", "the file"))?;
                c.done()?;
                f.uses.push(Use { kind, name, path: p, span });
                Ok(Block::None)
            }
            kw::TODAY => {
                let form_note = tr!(
                    "`today range >=2026-10-01 <=2028-10-31 offset +00:00` のように、範囲と、日を変えるオフセットを書いてください。",
                    "Write the range and the offset the day changes at, like `today range >=2026-10-01 <=2028-10-31 offset +00:00`."
                );
                if !c.is_word(kw::RANGE) {
                    let fd = c.found();
                    return Err(form(c.col(), tr!("`today` のあとには `range` を書いてください（{}）", "`today` is followed by `range` ({})", fd.ja; fd.en)).note(form_note));
                }
                let rs = c.span();
                c.bump();
                let range = bounds(&mut c, rs)?;
                if !c.eat_word(kw::OFFSET) {
                    let fd = c.found();
                    return Err(form(c.col(), tr!("範囲のあとに `offset` を書いてください（{}）", "the range is followed by `offset` ({})", fd.ja; fd.en)).note(form_note));
                }
                let (text, col) = line.rest.clone().unwrap_or_default();
                if text.is_empty() {
                    return Err(form(line.chars.len() + 1, tr!("`offset` のあとに、`+00:00` の形でオフセットを書いてください", "`offset` is followed by the offset, like `+00:00`")).note(form_note));
                }
                f.today = Some(Today { span, range, offset: (text, Span { line: line.no, col }) });
                Ok(Block::None)
            }
            kw::ENUM => {
                let name = c.declared(&tr!("列挙", "the enum"))?;
                c.want(&Tok::Eq, "=")?;
                let mut values = vec![c.declared(&tr!("列挙の値", "a value of the enum"))?];
                while c.eat(&Tok::Pipe) {
                    values.push(c.declared(&tr!("列挙の値", "a value of the enum"))?);
                }
                c.done()?;
                f.enums.push(EnumDecl { name, span, values });
                Ok(Block::None)
            }
            kw::ROLE => {
                let name = c.declared(&tr!("役割", "the role"))?;
                c.done()?;
                f.roles.push(RoleDecl { name, span, lines, description: None, includes: vec![], can: None });
                Ok(Block::Role(f.roles.len() - 1))
            }
            kw::PRINCIPAL | kw::RESOURCE => {
                let principal = w == kw::PRINCIPAL;
                let name = c.declared(&if principal { tr!("principal の型", "the principal type") } else { tr!("resource の型", "the resource type") })?;
                c.done()?;
                let kind = if principal { EntityKind::Principal } else { EntityKind::Resource };
                let e = EntityDecl { kind, name, span, lines, description: None, roles: vec![], attributes: vec![] };
                if principal {
                    f.principals.push(e);
                    Ok(Block::Principal(f.principals.len() - 1))
                } else {
                    f.resources.push(e);
                    Ok(Block::Resource(f.resources.len() - 1))
                }
            }
            kw::WORKFLOW => {
                let name = c.declared(&tr!("ワークフロー", "the workflow"))?;
                c.want_word(kw::FROM)?;
                let flow = c.string(&tr!("dandori のフロー（`.flow`）", "the dandori flow (`.flow`)"))?;
                c.done()?;
                f.workflows.push(WorkflowDecl { name, span, lines, flow, description: None });
                Ok(Block::Workflow(f.workflows.len() - 1))
            }
            kw::ACTION => {
                let name = c.declared(&tr!("action", "the action"))?;
                c.done()?;
                f.actions.push(ActionDecl {
                    name,
                    span,
                    lines,
                    description: None,
                    guards: vec![],
                    principals: (vec![], span),
                    resources: (vec![], span),
                    resource_from: None,
                    nobody: None,
                    input: vec![],
                    context: vec![],
                });
                Ok(Block::Action(f.actions.len() - 1))
            }
            kw::PERMIT | kw::FORBID => {
                let effect = if w == kw::PERMIT { Effect::Permit } else { Effect::Forbid };
                let name = c.declared(&tr!("ポリシー", "the policy"))?;
                c.done()?;
                f.policies.push(Policy { effect, name, span, lines, body: empty_body(span) });
                Ok(Block::Policy(f.policies.len() - 1))
            }
            kw::EXPECT => {
                let allow = if c.eat_word(kw::ALLOW) {
                    true
                } else if c.eat_word(kw::DENY) {
                    false
                } else {
                    let fd = c.found();
                    return Err(form(c.col(), tr!("`expect` のあとには `allow` か `deny` を書いてください（{}）", "`expect` is followed by `allow` or `deny` ({})", fd.ja; fd.en)));
                };
                let name = c.declared(&tr!("期待", "the expectation"))?;
                c.done()?;
                f.expects.push(Expect { allow, name, span, lines, body: empty_body(span) });
                Ok(Block::Expect(f.expects.len() - 1))
            }
            kw::SEPARATE => {
                let name = c.declared(&tr!("職務の分離", "the separation"))?;
                c.done()?;
                f.separates.push(Separate { name, span, lines, description: None, actions: (vec![], span) });
                Ok(Block::Separate(f.separates.len() - 1))
            }
            _ => Err(form(span.col, tr!("`{w}` は行の初めに書く語ではありません", "`{w}` is not a word a line starts with"))),
        }
    }

    /// A line of a block: the block under it, when it opens one.
    fn block_line(&mut self, f: &mut File, open: &mut Open, line: &Line) -> Result<Sub, Bad> {
        let t0 = &line.tokens[0];
        let head = match open.block {
            Block::Policy(i) => f.policies[i].effect.word(),
            b => b.words().0,
        };
        let Tok::Word(w) = &t0.tok else {
            return Err(form(t0.col, tr!("`{head}` の中の行は `{}` で始められません", "A line under `{head}` cannot start with `{}`", show(&t0.tok))).note(open.block.order(head)));
        };
        let w = w.as_str();
        let Some(rank) = open.block.rank(w) else {
            let msg = if matches!(open.block, Block::Principal(_) | Block::Resource(_) | Block::Action(_)) && line.tokens.get(1).is_some_and(|t| t.tok == Tok::Colon) {
                let under = if matches!(open.block, Block::Action(_)) { kw::INPUT } else { kw::ATTRIBUTES };
                tr!("この行は `{under}` の下に、もう一段字下げして書いてください", "This line goes under `{under}`, indented one step further")
            } else {
                tr!("`{w}` の行は `{head}` の中には書けません", "A `{w}` line cannot be written under `{head}`")
            };
            return Err(form(t0.col, msg).note(open.block.order(head)));
        };
        let repeats = [kw::GUARDS, kw::WHEN, kw::UNLESS].contains(&w);
        let word: &'static str = open.block.words().1[rank];
        let word = if w == kw::UNLESS { kw::UNLESS } else { word };
        if !repeats && open.seen.contains(&word) {
            return Err(Bad { col: t0.col, code: "E003", msg: tr!("`{w}` が二度書かれています", "`{w}` is written twice"), notes: vec![] });
        }
        // written, if out of place: the block is not said to lack it as well
        open.seen.push(word);
        if rank < open.last_rank {
            return Err(Bad { col: t0.col, code: "E003", msg: tr!("`{w}` の位置が違います", "`{w}` is out of place"), notes: vec![open.block.order(head)] });
        }
        open.last_rank = rank;
        let mut c = Cur::new(line);
        let span = c.span();
        c.bump();
        if w == kw::DESCRIPTION {
            let d = Some(c.string(&tr!("説明", "the description"))?);
            c.done()?;
            match open.block {
                Block::Role(i) => f.roles[i].description = d,
                Block::Principal(i) => f.principals[i].description = d,
                Block::Resource(i) => f.resources[i].description = d,
                Block::Workflow(i) => f.workflows[i].description = d,
                Block::Action(i) => f.actions[i].description = d,
                Block::Policy(i) => f.policies[i].body.description = d,
                Block::Expect(i) => f.expects[i].body.description = d,
                Block::Separate(i) => f.separates[i].description = d,
                Block::None | Block::Skip => {}
            }
            return Ok(Sub::None);
        }
        match (open.block, w) {
            (Block::Role(i), kw::INCLUDES) => {
                f.roles[i].includes = c.refs(&tr!("役割", "the role"))?;
                c.done()?;
            }
            (Block::Role(i), kw::CAN) => {
                f.roles[i].can = Some((c.refs(&tr!("action", "the action"))?, span));
                c.done()?;
            }
            (Block::Principal(i), kw::ROLES) => {
                f.principals[i].roles = c.refs(&tr!("役割", "the role"))?;
                c.done()?;
            }
            (Block::Principal(_) | Block::Resource(_), kw::ATTRIBUTES) => {
                c.done()?;
                return Ok(Sub::Attributes);
            }
            (Block::Action(i), kw::GUARDS) => {
                let api = c.reference(&tr!("契約か帳簿（`use openapi`、`use proto`、`use asyncapi`、`use book` の名前）", "the contract or the book (the name of a `use openapi`, `use proto`, `use asyncapi` or `use book`)"))?;
                let os = c.span();
                let (operation, quoted) = match c.peek() {
                    Some(Tok::Str(s)) => {
                        let s = s.clone();
                        c.bump();
                        (s, true)
                    }
                    Some(Tok::Word(op)) => {
                        let mut op = op.clone();
                        c.bump();
                        // a book's transfer and its operation: `receive.do`
                        if c.eat(&Tok::Dot) {
                            let (o, _) = c.name(&tr!("振替の操作（`do`、`hold`、`post`、`void`）", "the transfer's operation (`do`, `hold`, `post`, `void`)"))?;
                            op = format!("{op}.{o}");
                        }
                        (op, false)
                    }
                    _ => {
                        let fd = c.found();
                        return Err(Bad {
                            col: os.col,
                            code: "E002",
                            msg: tr!(
                                "ここには守る操作（`operationId`、`\"POST /orders/{{orderId}}/refunds\"`、`\"Service/Method\"`、`<振替>.<操作>`）を書いてください（{}）",
                                "the operation it guards goes here (an `operationId`, `\"POST /orders/{{orderId}}/refunds\"`, `\"Service/Method\"`, `<transfer>.<operation>`) ({})",
                                fd.ja;
                                fd.en
                            ),
                            notes: vec![],
                        });
                    }
                };
                c.done()?;
                f.actions[i].guards.push(Guard { api, operation: (operation, os), quoted, span });
            }
            (Block::Action(i), kw::PRINCIPAL) => {
                f.actions[i].principals = (c.refs(&tr!("principal の型", "the principal type"))?, span);
                c.done()?;
            }
            (Block::Action(i), kw::RESOURCE) => {
                f.actions[i].resources = (c.refs(&tr!("resource の型", "the resource type"))?, span);
                if c.eat_word(kw::FROM) {
                    let s = c.span();
                    let p = match c.peek() {
                        Some(Tok::Word(p)) | Some(Tok::Str(p)) => p.clone(),
                        _ => {
                            let fd = c.found();
                            return Err(Bad {
                                col: s.col,
                                code: "E002",
                                msg: tr!("`from` のあとには、resource の ID を取る操作の引数を書いてください（{}）", "`from` is followed by the operation's parameter the resource's id is taken from ({})", fd.ja; fd.en),
                                notes: vec![],
                            });
                        }
                    };
                    c.bump();
                    f.actions[i].resource_from = Some((p, s));
                }
                c.done()?;
            }
            (Block::Action(i), kw::NOBODY) => {
                f.actions[i].nobody = Some(c.string(&tr!("だれにも許さない理由", "why no one is allowed it"))?);
                c.done()?;
            }
            (Block::Action(_), kw::INPUT) => {
                c.done()?;
                return Ok(Sub::Input);
            }
            (Block::Action(_), kw::CONTEXT) => {
                c.done()?;
                return Ok(Sub::Context);
            }
            (Block::Policy(_) | Block::Expect(_), kw::PRINCIPAL) => {
                let who = if c.eat_word(kw::IN) {
                    Who::In(c.refs(&tr!("役割", "the role"))?)
                } else if c.eat_word(kw::IS) {
                    if c.eat_word(kw::WORKFLOW) {
                        Who::Workflow(c.reference(&tr!("ワークフロー", "the workflow"))?)
                    } else {
                        Who::Is(c.reference(&tr!("型", "the type"))?)
                    }
                } else {
                    let fd = c.found();
                    return Err(form(c.col(), tr!(
                        "`principal` のあとには、`in <役割>, …`、`is <型>`、`is workflow <ワークフロー>` のどれかを書いてください（{}）",
                        "`principal` is followed by `in <role>, …`, `is <type>` or `is workflow <workflow>` ({})",
                        fd.ja;
                        fd.en
                    )));
                };
                c.done()?;
                let b = match open.block {
                    Block::Policy(i) => &mut f.policies[i].body,
                    Block::Expect(i) => &mut f.expects[i].body,
                    _ => unreachable!(),
                };
                b.principal = Some((who, span));
            }
            (Block::Policy(_) | Block::Expect(_), kw::ACTION) => {
                let what = if c.is_word(kw::ANY) {
                    c.bump();
                    What::Any
                } else {
                    What::Actions(c.refs(&tr!("action", "the action"))?)
                };
                if matches!(what, What::Any) && !c.at_end() {
                    return Err(form(c.col(), tr!("`action any` は一つだけで書いてください（ほかの action と並べられません）", "`action any` stands alone (it is not listed with other actions)")));
                }
                c.done()?;
                let b = match open.block {
                    Block::Policy(i) => &mut f.policies[i].body,
                    Block::Expect(i) => &mut f.expects[i].body,
                    _ => unreachable!(),
                };
                b.action = (what, span);
            }
            (Block::Policy(_) | Block::Expect(_), kw::WHEN | kw::UNLESS) => {
                let when = w == kw::WHEN;
                let text = line.rest_from(c.col());
                if c.at_end() {
                    return Err(form(c.col(), tr!("`{w}` のあとに条件を書いてください", "`{w}` is followed by a condition")));
                }
                let e = expr(&mut c)?;
                c.done()?;
                let b = match open.block {
                    Block::Policy(i) => &mut f.policies[i].body,
                    Block::Expect(i) => &mut f.expects[i].body,
                    _ => unreachable!(),
                };
                b.conds.push(Cond { when, expr: e, span, text });
            }
            (Block::Separate(i), kw::ACTIONS) => {
                let refs = c.refs(&tr!("action", "the action"))?;
                c.done()?;
                if refs.len() < 2 {
                    return Err(form(span.col, tr!("`actions` には action を二つ以上並べてください", "`actions` lists two actions or more")));
                }
                f.separates[i].actions = (refs, span);
            }
            _ => unreachable!("every word a block ranks is read"),
        }
        Ok(Sub::None)
    }

    /// A line of the block under a line: an attribute, an input, or a computed value.
    fn sub_line(&mut self, f: &mut File, open: &Open, line: &Line) -> Result<(), Bad> {
        let mut c = Cur::new(line);
        match (open.block, open.sub) {
            (Block::Principal(i), Sub::Attributes) => {
                let fd = field(&mut c, &tr!("属性", "the attribute"))?;
                f.principals[i].attributes.push(fd);
            }
            (Block::Resource(i), Sub::Attributes) => {
                let fd = field(&mut c, &tr!("属性", "the attribute"))?;
                f.resources[i].attributes.push(fd);
            }
            (Block::Action(i), Sub::Input) => {
                let fd = field(&mut c, &tr!("入力", "the input"))?;
                f.actions[i].input.push(fd);
            }
            (Block::Action(i), Sub::Context) => {
                let span = c.span();
                let name = c.declared(&tr!("計算した値", "the computed value"))?;
                if c.peek() != Some(&Tok::Eq) {
                    let fd = c.found();
                    return Err(form(c.col(), tr!("計算した値の名前のあとに `=` と計算を書いてください（{}）", "the computed value's name is followed by `=` and how it is computed ({})", fd.ja; fd.en)).note(tr!(
                        "`refund_band = refund_limit(amount: amount, limit: principal.refund_limit).band`、`in_period = today <= refund_terms.last_day(paid_on: resource.paid_on)`、`business_day = today is open in uk` のように書きます。",
                        "Write it like `refund_band = refund_limit(amount: amount, limit: principal.refund_limit).band`, `in_period = today <= refund_terms.last_day(paid_on: resource.paid_on)` or `business_day = today is open in uk`."
                    )));
                }
                c.bump();
                let text = line.rest_from(c.col());
                let value = computation(&mut c)?;
                c.done()?;
                f.actions[i].context.push(Computed { name, span, value, text });
            }
            _ => {}
        }
        Ok(())
    }
}

/// A body before its lines are read; a policy without an `action` line is said (E003) and dropped.
fn empty_body(span: Span) -> Body {
    Body { description: None, principal: None, action: (What::Actions(vec![]), span), conds: vec![] }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(src: &str) -> Vec<&'static str> {
        parse("t.gate", src).diags.iter().map(|d| d.code).collect()
    }

    #[test]
    fn a_small_file_reads() {
        let src = "gate t v1\n\nrole clerk\n  can view\n\nprincipal User\n  roles clerk\n  attributes\n    suspended : bool\n\nresource Order\n\naction view\n  principal User\n  resource Order\n\npermit clerks_view\n  principal in clerk\n  action view\n  unless principal.suspended\n";
        let p = parse("t.gate", src);
        assert!(p.diags.is_empty(), "{:?}", p.diags.iter().map(|d| d.render(ritsu_base::text::Lang::En)).collect::<Vec<_>>());
        let f = p.file.unwrap();
        assert_eq!(f.principals[0].attributes.len(), 1);
        assert_eq!(f.principals[0].lines, (6, 9));
        assert_eq!(f.policies[0].body.conds.len(), 1);
        assert_eq!(f.policies[0].lines, (17, 20));
    }

    #[test]
    fn the_shape_of_a_file() {
        assert_eq!(codes("role clerk\n"), vec!["E003"]);
        assert_eq!(codes("gate t v1\naction a\n  principal User\n"), vec!["E003"]);
        assert_eq!(codes("gate t v1\naction a\n  resource R\n  principal User\n"), vec!["E003"]);
        assert_eq!(codes("gate t v1\nrole r\n  description \"x\"\n   can a\n"), vec!["E004"]);
        assert_eq!(codes("gate t v1\nrole r\n  roles a\n"), vec!["E005"]);
        assert_eq!(codes("gate t v1\nrole \"r\"\n"), vec!["E002"]);
        assert_eq!(codes("gate t v1\nprincipal U\n  attributes\nresource R\n"), vec!["E003"]);
        assert_eq!(codes("gate t v1\nuse rules x from \"a\"\n"), vec!["E005"]);
        assert_eq!(codes("gate t v1\ntoday range >=2026-01-01 <=2026-12-31\n"), vec!["E005"]);
        assert_eq!(codes("gate t v1\naction a\n  principal U\n  resource R\n  input\n    x money[GBP]\n"), vec!["E005"]);
        assert_eq!(codes("gate t v1\npermit p\n  action a\n  when x == 1\n"), vec!["E001"]);
        assert_eq!(codes("gate t v1\nuse rule r from \"r.rule\"\n  x\n"), vec!["E004"]);
        assert_eq!(codes("gate t v1\naction a\n  principal U\n  resource R\nrole r\n"), vec!["E003"]);
        assert_eq!(codes("gate t v1\ndescription \"a\"\ndescription \"b\"\n"), vec!["E003"]);
        assert_eq!(codes("gate t v1\nseparate s\n  actions a\n"), vec!["E005"]);
        assert_eq!(codes("gate t v1\npermit p\n  action any, a\n"), vec!["E005"]);
        assert_eq!(codes("gate t v1\npermit p\n  when x\n  action a\n"), vec!["E003"]);
        assert_eq!(codes("gate t v1\nprincipal U\n  suspended : bool\n"), vec!["E005"]);
        assert_eq!(codes("gate t v1\naction a\n  principal U\n  resource R\n  context\n    x = r(a: \"s\").y\n"), vec!["E005"]);
    }

    #[test]
    fn conditions_read_as_written() {
        let src = "gate t v1\npermit p\n  action a\n  when (x or principal in resource.team) and not resource.customer is principal\n  unless amount <= -5GBP\n  when resource.status is not refunded\n  when principal is workflow w\n";
        let p = parse("t.gate", src);
        assert!(p.diags.is_empty(), "{:?}", p.diags.iter().map(|d| d.render(ritsu_base::text::Lang::En)).collect::<Vec<_>>());
        let f = p.file.unwrap();
        let conds = &f.policies[0].body.conds;
        assert!(matches!(&conds[0].expr, Expr::And(parts) if matches!(&parts[0], Expr::Or(_)) && matches!(&parts[1], Expr::Not(..))));
        assert!(matches!(&conds[1].expr, Expr::Atom(Atom::Compare { op: Op::Le, right: (Lit::Num(n), _), .. }, _) if n.raw == "-5GBP"));
        assert!(matches!(&conds[2].expr, Expr::Atom(Atom::Is { not: true, right: Rhs::Word(_), .. }, _)));
        assert!(matches!(&conds[3].expr, Expr::Atom(Atom::IsWorkflow(_), _)));
        assert_eq!(conds[1].text, "amount <= -5GBP");
    }
}
