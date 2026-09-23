//! The part of CEL that a contract's validation rules are written in (§15.140).
//!
//! Protovalidate lets a `.proto` say in CEL, the Common Expression Language, what a message
//! has to satisfy: `this.min_weight <= this.max_weight` on a message, `this >= 1` on a field.
//! Those are the conditions that relate one field to another, and they are exactly what the
//! per-field reading of §15.132 could not see. This module parses an expression and reads the
//! part of it that is a condition on whole numbers, strings and booleans as a
//! [`Formula`](crate::relation::Formula).
//!
//! **What is read is the decidable part, and only that.** Linear arithmetic over fields and
//! constants, the six comparisons, `in` against a list of literals, `size()` of a repeated
//! field, `has()` where presence says something about the value, `!`, `&&`, `||` and `? :`.
//! Everything else — division, `%`, string functions, timestamps, macros such as `all` — is
//! an unknown atom, and the caller makes it true once the whole condition is built. Reading an
//! unread condition as true can only make the contract admit more than it does: a check may
//! then report a mismatch that is not there, and never passes one that is. That is the
//! direction §15.132 chose for the rules it did not read, kept here.
//!
//! Negations are pushed down to the atoms as the expression is read — a comparison negates
//! exactly, a membership to its complement, an unknown atom to an unknown atom — so that once
//! the unknown atoms are made true, every one of them stands where true is the wider reading.

use crate::num::Rat;
use crate::relation::{Atom, Formula, Lin, Rel, Term};

/// A parsed CEL expression. Only as much of the language as can be parsed without guessing:
/// an expression outside it is refused whole, and a refused expression reads as true.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i128),
    /// A double literal, kept exactly: `1.5` is 3/2.
    Num(Rat),
    Str(String),
    Bool(bool),
    Null,
    Ident(String),
    Member(Box<Expr>, String),
    /// A call: the receiver for `x.f(y)`, none for `f(x)`.
    Call(Option<Box<Expr>>, String, Vec<Expr>),
    List(Vec<Expr>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Bin(Box<Expr>, Op, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Mul,
    Div,
    Rem,
    Add,
    Sub,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    In,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Int(i128),
    Num(Rat),
    Str(String),
    Ident(String),
    Sym(&'static str),
}

/// Parse one expression. `Err` for anything outside what this reads, with a short reason.
pub fn parse(src: &str) -> Result<Expr, String> {
    let toks = lex(src)?;
    let mut p = Parser { toks, at: 0 };
    let e = p.cond()?;
    if p.at != p.toks.len() {
        return Err(format!("unexpected {:?}", p.toks[p.at]));
    }
    Ok(e)
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let s: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    const SYMS: [&str; 22] = [
        "==", "!=", "<=", ">=", "&&", "||", "<", ">", "!", "+", "-", "*", "/", "%", "?", ":", "(", ")", "[", "]", ".", ",",
    ];
    while i < s.len() {
        let c = s[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            if c == '0' && i + 1 < s.len() && (s[i + 1] == 'x' || s[i + 1] == 'X') {
                i += 2;
                let h = i;
                while i < s.len() && s[i].is_ascii_hexdigit() {
                    i += 1;
                }
                let text: String = s[h..i].iter().collect();
                let v = i128::from_str_radix(&text, 16).map_err(|e| e.to_string())?;
                if i < s.len() && (s[i] == 'u' || s[i] == 'U') {
                    i += 1;
                }
                out.push(Tok::Int(v));
                continue;
            }
            while i < s.len() && s[i].is_ascii_digit() {
                i += 1;
            }
            let mut frac = false;
            if i + 1 < s.len() && s[i] == '.' && s[i + 1].is_ascii_digit() {
                frac = true;
                i += 1;
                while i < s.len() && s[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < s.len() && (s[i] == 'e' || s[i] == 'E') {
                // An exponent is a double CEL would read inexactly; refuse it rather than guess.
                return Err("exponent in a number".into());
            }
            let text: String = s[start..i].iter().collect();
            if frac {
                out.push(Tok::Num(decimal(&text).ok_or("number too long")?));
            } else {
                out.push(Tok::Int(text.parse::<i128>().map_err(|e| e.to_string())?));
                if i < s.len() && (s[i] == 'u' || s[i] == 'U') {
                    i += 1;
                }
            }
            continue;
        }
        if c == '"' || c == '\'' {
            let (text, next) = string(&s, i)?;
            out.push(Tok::Str(text));
            i = next;
            continue;
        }
        if (c == 'r' || c == 'R') && i + 1 < s.len() && (s[i + 1] == '"' || s[i + 1] == '\'') {
            // A raw string: no escapes. Only the single-quoted forms are read.
            let q = s[i + 1];
            let mut j = i + 2;
            let mut text = String::new();
            while j < s.len() && s[j] != q {
                text.push(s[j]);
                j += 1;
            }
            if j >= s.len() {
                return Err("unterminated string".into());
            }
            out.push(Tok::Str(text));
            i = j + 1;
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < s.len() && (s[i].is_alphanumeric() || s[i] == '_') {
                i += 1;
            }
            out.push(Tok::Ident(s[start..i].iter().collect()));
            continue;
        }
        let rest: String = s[i..(i + 2).min(s.len())].iter().collect();
        match SYMS.iter().find(|t| rest.starts_with(**t)) {
            Some(t) => {
                out.push(Tok::Sym(t));
                i += t.chars().count();
            }
            None => return Err(format!("unexpected character {c:?}")),
        }
    }
    Ok(out)
}

/// A decimal literal as an exact rational: `12.50` is 25/2.
fn decimal(text: &str) -> Option<Rat> {
    let (int, frac) = text.split_once('.')?;
    let den = 10i128.checked_pow(frac.len() as u32)?;
    let num = format!("{int}{frac}").parse::<i128>().ok()?;
    Rat::checked_new(num, den)
}

/// A quoted string starting at `s[i]`, with the escapes CEL shares with most languages.
/// Triple quotes are not read.
fn string(s: &[char], i: usize) -> Result<(String, usize), String> {
    let q = s[i];
    if i + 2 < s.len() && s[i + 1] == q && s[i + 2] == q {
        return Err("triple-quoted string".into());
    }
    let mut j = i + 1;
    let mut out = String::new();
    while j < s.len() {
        let c = s[j];
        if c == q {
            return Ok((out, j + 1));
        }
        if c == '\\' {
            j += 1;
            let e = *s.get(j).ok_or("unterminated escape")?;
            out.push(match e {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '\\' => '\\',
                '\'' => '\'',
                '"' => '"',
                _ => return Err(format!("escape \\{e}")),
            });
        } else {
            out.push(c);
        }
        j += 1;
    }
    Err("unterminated string".into())
}

struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }
    fn sym(&self, t: &str) -> bool {
        matches!(self.peek(), Some(Tok::Sym(s)) if *s == t)
    }
    fn eat(&mut self, t: &str) -> bool {
        if self.sym(t) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: &str) -> Result<(), String> {
        if self.eat(t) { Ok(()) } else { Err(format!("expected {t}")) }
    }

    fn cond(&mut self) -> Result<Expr, String> {
        let c = self.or()?;
        if self.eat("?") {
            let a = self.cond()?;
            self.expect(":")?;
            let b = self.cond()?;
            return Ok(Expr::Cond(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(c)
    }
    fn or(&mut self) -> Result<Expr, String> {
        let mut l = self.and()?;
        while self.eat("||") {
            let r = self.and()?;
            l = Expr::Bin(Box::new(l), Op::Or, Box::new(r));
        }
        Ok(l)
    }
    fn and(&mut self) -> Result<Expr, String> {
        let mut l = self.rel()?;
        while self.eat("&&") {
            let r = self.rel()?;
            l = Expr::Bin(Box::new(l), Op::And, Box::new(r));
        }
        Ok(l)
    }
    fn rel(&mut self) -> Result<Expr, String> {
        let mut l = self.add()?;
        loop {
            let op = if self.eat("<=") {
                Op::Le
            } else if self.eat(">=") {
                Op::Ge
            } else if self.eat("<") {
                Op::Lt
            } else if self.eat(">") {
                Op::Gt
            } else if self.eat("==") {
                Op::Eq
            } else if self.eat("!=") {
                Op::Ne
            } else if matches!(self.peek(), Some(Tok::Ident(w)) if w == "in") {
                self.at += 1;
                Op::In
            } else {
                return Ok(l);
            };
            let r = self.add()?;
            l = Expr::Bin(Box::new(l), op, Box::new(r));
        }
    }
    fn add(&mut self) -> Result<Expr, String> {
        let mut l = self.mul()?;
        loop {
            let op = if self.eat("+") {
                Op::Add
            } else if self.eat("-") {
                Op::Sub
            } else {
                return Ok(l);
            };
            let r = self.mul()?;
            l = Expr::Bin(Box::new(l), op, Box::new(r));
        }
    }
    fn mul(&mut self) -> Result<Expr, String> {
        let mut l = self.unary()?;
        loop {
            let op = if self.eat("*") {
                Op::Mul
            } else if self.eat("/") {
                Op::Div
            } else if self.eat("%") {
                Op::Rem
            } else {
                return Ok(l);
            };
            let r = self.unary()?;
            l = Expr::Bin(Box::new(l), op, Box::new(r));
        }
    }
    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat("!") {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        if self.eat("-") {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        self.postfix()
    }
    fn postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.primary()?;
        loop {
            if self.eat(".") {
                let Some(Tok::Ident(n)) = self.peek().cloned() else { return Err("expected a name after .".into()) };
                self.at += 1;
                if self.eat("(") {
                    let args = self.args(")")?;
                    e = Expr::Call(Some(Box::new(e)), n, args);
                } else {
                    e = Expr::Member(Box::new(e), n);
                }
            } else if self.eat("[") {
                let i = self.cond()?;
                self.expect("]")?;
                e = Expr::Index(Box::new(e), Box::new(i));
            } else {
                return Ok(e);
            }
        }
    }
    fn args(&mut self, close: &str) -> Result<Vec<Expr>, String> {
        let mut out = Vec::new();
        if self.eat(close) {
            return Ok(out);
        }
        loop {
            out.push(self.cond()?);
            if self.eat(close) {
                return Ok(out);
            }
            self.expect(",")?;
            // A trailing comma before the close is allowed in list literals.
            if self.eat(close) {
                return Ok(out);
            }
        }
    }
    fn primary(&mut self) -> Result<Expr, String> {
        let t = self.peek().cloned().ok_or("unexpected end")?;
        self.at += 1;
        match t {
            Tok::Int(v) => Ok(Expr::Int(v)),
            Tok::Num(r) => Ok(Expr::Num(r)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Ident(w) => match w.as_str() {
                "true" => Ok(Expr::Bool(true)),
                "false" => Ok(Expr::Bool(false)),
                "null" => Ok(Expr::Null),
                _ => {
                    if self.eat("(") {
                        let args = self.args(")")?;
                        Ok(Expr::Call(None, w, args))
                    } else {
                        Ok(Expr::Ident(w))
                    }
                }
            },
            Tok::Sym("(") => {
                let e = self.cond()?;
                self.expect(")")?;
                Ok(e)
            }
            Tok::Sym("[") => Ok(Expr::List(self.args("]")?)),
            other => Err(format!("unexpected {other:?}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Reading an expression as a condition
// ---------------------------------------------------------------------------

/// What a field is, as far as reading an expression about it needs. `presence` says whether
/// the field can be unset apart from holding its default — an `optional` field, a member of a
/// `oneof` — which decides what `has()` says about its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Int { presence: bool },
    Str { presence: bool },
    Bool { presence: bool },
    /// A repeated field: `size()` of it is a term.
    List,
    /// A message: a field of it can be read.
    Msg,
    /// A double, bytes, a map, a timestamp. Nothing about it is read.
    Other,
}

/// The condition under which a rule written in CEL lets a message through, with `this`
/// standing for the field at that path — empty for the message itself.
///
/// A Protovalidate rule returns a bool (false fails) or a string (a non-empty one fails, and
/// is the message). Both are read. What could not be read is left as an unknown atom: the
/// caller may still negate this, as a JSON Schema `not` would, and widens the whole condition
/// once it is built.
pub fn read(e: &Expr, this: &[String], kind: &dyn Fn(&[String]) -> Option<Kind>) -> Formula {
    Reader { this: this.to_vec(), kind }.valid(e)
}

/// The same from source text. An expression that does not parse reads as unknown.
pub fn read_src(src: &str, this: &[String], kind: &dyn Fn(&[String]) -> Option<Kind>) -> Formula {
    match parse(src) {
        Ok(e) => read(&e, this, kind),
        Err(_) => Formula::unknown(),
    }
}

struct Reader<'k> {
    this: Vec<String>,
    kind: &'k dyn Fn(&[String]) -> Option<Kind>,
}

/// A string an expression names: a literal, or a string field.
enum StrV {
    Lit(String),
    Field(Term),
}

/// A truth value an expression names: a literal, or a boolean field.
enum BoolV {
    Lit(bool),
    Field(Term),
}

type Pair = (Formula, Formula);

fn unknown() -> Pair {
    (Formula::unknown(), Formula::unknown())
}

fn constant(b: bool) -> Pair {
    if b { (Formula::True, Formula::False) } else { (Formula::False, Formula::True) }
}

impl Reader<'_> {
    /// The condition under which the rule passes: a bool that is true, or a string that is
    /// empty.
    fn valid(&self, e: &Expr) -> Formula {
        match e {
            Expr::Cond(c, a, b) => {
                let (yes, no) = self.test(c);
                Formula::or(vec![Formula::and(vec![yes, self.valid(a)]), Formula::and(vec![no, self.valid(b)])])
            }
            Expr::Str(s) => {
                if s.is_empty() { Formula::True } else { Formula::False }
            }
            // `'weight ' + string(this.w) + ' is over'` is a message whatever it says.
            Expr::Bin(..) if nonempty(e) => Formula::False,
            _ => self.test(e).0,
        }
    }

    /// A boolean expression as the pair (holds, fails), negations on the atoms.
    fn test(&self, e: &Expr) -> Pair {
        match e {
            Expr::Bool(b) => constant(*b),
            Expr::Not(x) => {
                let (y, n) = self.test(x);
                (n, y)
            }
            Expr::Bin(a, Op::And, b) => {
                let (ya, na) = self.test(a);
                let (yb, nb) = self.test(b);
                (Formula::and(vec![ya, yb]), Formula::or(vec![na, nb]))
            }
            Expr::Bin(a, Op::Or, b) => {
                let (ya, na) = self.test(a);
                let (yb, nb) = self.test(b);
                (Formula::or(vec![ya, yb]), Formula::and(vec![na, nb]))
            }
            Expr::Cond(c, a, b) => {
                let (yc, nc) = self.test(c);
                let (ya, na) = self.test(a);
                let (yb, nb) = self.test(b);
                (
                    Formula::or(vec![Formula::and(vec![yc.clone(), ya]), Formula::and(vec![nc.clone(), yb])]),
                    Formula::or(vec![Formula::and(vec![yc, na]), Formula::and(vec![nc, nb])]),
                )
            }
            Expr::Bin(a, op @ (Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::Eq | Op::Ne), b) => self.compare(a, *op, b),
            Expr::Bin(a, Op::In, b) => self.member(a, b),
            Expr::Call(None, f, args) if f == "has" && args.len() == 1 => self.has(&args[0]),
            Expr::Ident(_) | Expr::Member(..) => match self.boolv(e) {
                Some(BoolV::Field(t)) => (Formula::atom(Atom::Bool(t.clone(), true)), Formula::atom(Atom::Bool(t, false))),
                _ => unknown(),
            },
            _ => unknown(),
        }
    }

    fn compare(&self, a: &Expr, op: Op, b: &Expr) -> Pair {
        // A choice on either side opens into the two comparisons it stands for.
        if let Expr::Cond(c, x, y) = a {
            return self.choose(c, self.compare(x, op, b), self.compare(y, op, b));
        }
        if let Expr::Cond(c, x, y) = b {
            return self.choose(c, self.compare(a, op, x), self.compare(a, op, y));
        }
        if let (Some(x), Some(y)) = (self.num(a), self.num(b)) {
            let holds = match op {
                Op::Le => Formula::cmp(&x, Rel::Le, &y),
                Op::Lt => Formula::cmp(&x, Rel::Lt, &y),
                Op::Ge => Formula::cmp(&y, Rel::Le, &x),
                Op::Gt => Formula::cmp(&y, Rel::Lt, &x),
                Op::Eq => Formula::cmp(&x, Rel::Eq, &y),
                _ => Formula::cmp(&x, Rel::Ne, &y),
            };
            let fails = holds.not();
            return (holds, fails);
        }
        let eq = match op {
            Op::Eq => true,
            Op::Ne => false,
            _ => return unknown(),
        };
        let pair = match (self.strv(a), self.strv(b)) {
            (Some(StrV::Lit(s)), Some(StrV::Lit(t))) => Some(constant(s == t)),
            (Some(StrV::Field(t)), Some(StrV::Lit(s))) | (Some(StrV::Lit(s)), Some(StrV::Field(t))) => {
                Some((Formula::atom(Atom::Str(t.clone(), vec![s.clone()], true)), Formula::atom(Atom::Str(t, vec![s], false))))
            }
            (Some(_), Some(_)) => Some(unknown()),
            _ => None,
        };
        let pair = pair.or_else(|| match (self.boolv(a), self.boolv(b)) {
            (Some(BoolV::Lit(x)), Some(BoolV::Lit(y))) => Some(constant(x == y)),
            (Some(BoolV::Field(t)), Some(BoolV::Lit(v))) | (Some(BoolV::Lit(v)), Some(BoolV::Field(t))) => {
                Some((Formula::atom(Atom::Bool(t.clone(), v)), Formula::atom(Atom::Bool(t, !v))))
            }
            (Some(BoolV::Field(s)), Some(BoolV::Field(t))) => {
                let (s1, s0) = (Formula::atom(Atom::Bool(s.clone(), true)), Formula::atom(Atom::Bool(s, false)));
                let (t1, t0) = (Formula::atom(Atom::Bool(t.clone(), true)), Formula::atom(Atom::Bool(t, false)));
                Some((
                    Formula::or(vec![Formula::and(vec![s1.clone(), t1.clone()]), Formula::and(vec![s0.clone(), t0.clone()])]),
                    Formula::or(vec![Formula::and(vec![s1, t0]), Formula::and(vec![s0, t1])]),
                ))
            }
            _ => None,
        });
        match pair {
            Some((y, n)) => {
                if eq { (y, n) } else { (n, y) }
            }
            None => unknown(),
        }
    }

    /// `c ? x : y` where x and y are the two readings of the comparison.
    fn choose(&self, c: &Expr, x: Pair, y: Pair) -> Pair {
        let (yc, nc) = self.test(c);
        (
            Formula::or(vec![Formula::and(vec![yc.clone(), x.0]), Formula::and(vec![nc.clone(), y.0])]),
            Formula::or(vec![Formula::and(vec![yc, x.1]), Formula::and(vec![nc, y.1])]),
        )
    }

    /// `x in [a, b, …]` against a list of literals.
    fn member(&self, a: &Expr, b: &Expr) -> Pair {
        let Expr::List(items) = b else { return unknown() };
        if let Some(x) = self.num(a) {
            let mut yes = Vec::new();
            for it in items {
                let Some(v) = self.num(it).filter(|v| v.constant().is_some()) else { return unknown() };
                yes.push(Formula::cmp(&x, Rel::Eq, &v));
            }
            let holds = Formula::or(yes);
            let fails = holds.not();
            return (holds, fails);
        }
        let mut lits = Vec::new();
        for it in items {
            match it {
                Expr::Str(s) => lits.push(s.clone()),
                _ => return unknown(),
            }
        }
        match self.strv(a) {
            Some(StrV::Field(t)) => (Formula::atom(Atom::Str(t.clone(), lits.clone(), true)), Formula::atom(Atom::Str(t, lits, false))),
            Some(StrV::Lit(s)) => constant(lits.contains(&s)),
            None => unknown(),
        }
    }

    /// `has(this.f)`. On a field with no presence of its own it says the value is not the
    /// default; on one with presence, unset means the default and set means nothing.
    fn has(&self, x: &Expr) -> Pair {
        let Some(p) = self.path(x) else { return unknown() };
        let t = Term::Field(p.clone());
        let zero = || Formula::cmp(&Lin::term(t.clone()), Rel::Eq, &Lin::con(Rat::int(0)));
        let empty = || Formula::atom(Atom::Str(t.clone(), vec![String::new()], true));
        let off = || Formula::atom(Atom::Bool(t.clone(), false));
        match (self.kind)(&p) {
            Some(Kind::Int { presence: false }) => (zero().not(), zero()),
            Some(Kind::Str { presence: false }) => (empty().not(), empty()),
            Some(Kind::Bool { presence: false }) => (off().not(), off()),
            Some(Kind::Int { presence: true }) => (Formula::unknown(), zero()),
            Some(Kind::Str { presence: true }) => (Formula::unknown(), empty()),
            Some(Kind::Bool { presence: true }) => (Formula::unknown(), off()),
            Some(Kind::List) => {
                let n = Lin::term(Term::Size(p));
                let some = Formula::cmp(&Lin::con(Rat::int(1)), Rel::Le, &n);
                let none = some.not();
                (some, none)
            }
            _ => unknown(),
        }
    }

    /// The path of a field reached from `this`, or `None` for anything else.
    fn path(&self, e: &Expr) -> Option<Vec<String>> {
        match e {
            Expr::Ident(w) if w == "this" => Some(self.this.clone()),
            Expr::Member(x, n) => {
                let mut p = self.path(x)?;
                p.push(n.clone());
                Some(p)
            }
            _ => None,
        }
    }

    /// A whole-number value as a linear form, or `None` for anything that is not one.
    fn num(&self, e: &Expr) -> Option<Lin> {
        match e {
            Expr::Int(v) => Some(Lin::con(Rat::int(*v))),
            Expr::Num(r) => Some(Lin::con(*r)),
            Expr::Ident(_) | Expr::Member(..) => {
                let p = self.path(e)?;
                matches!((self.kind)(&p), Some(Kind::Int { .. })).then(|| Lin::term(Term::Field(p)))
            }
            Expr::Neg(x) => self.num(x)?.scale(Rat::int(-1)),
            Expr::Bin(a, Op::Add, b) => self.num(a)?.plus(&self.num(b)?),
            Expr::Bin(a, Op::Sub, b) => self.num(a)?.minus(&self.num(b)?),
            // A product is linear when one side is a constant.
            Expr::Bin(a, Op::Mul, b) => {
                let (x, y) = (self.num(a)?, self.num(b)?);
                match (x.constant(), y.constant()) {
                    (Some(k), _) => y.scale(k),
                    (_, Some(k)) => x.scale(k),
                    _ => None,
                }
            }
            // `size(this.items)` and `this.items.size()`.
            Expr::Call(None, f, args) if f == "size" && args.len() == 1 => self.size(&args[0]),
            Expr::Call(Some(x), f, args) if f == "size" && args.is_empty() => self.size(x),
            // `int(x)` and `uint(x)` of a whole number are that number.
            Expr::Call(None, f, args) if (f == "int" || f == "uint") && args.len() == 1 => {
                let v = self.num(&args[0])?;
                (v.k.is_int() && v.terms.values().all(|c| c.is_int())).then_some(v)
            }
            // Integer division truncates and `%` is a remainder: neither is linear over the
            // rationals, so neither is read. Nor is anything else.
            _ => None,
        }
    }

    fn size(&self, x: &Expr) -> Option<Lin> {
        let p = self.path(x)?;
        matches!((self.kind)(&p), Some(Kind::List)).then(|| Lin::term(Term::Size(p)))
    }

    fn strv(&self, e: &Expr) -> Option<StrV> {
        match e {
            Expr::Str(s) => Some(StrV::Lit(s.clone())),
            Expr::Ident(_) | Expr::Member(..) => {
                let p = self.path(e)?;
                matches!((self.kind)(&p), Some(Kind::Str { .. })).then(|| StrV::Field(Term::Field(p)))
            }
            _ => None,
        }
    }

    fn boolv(&self, e: &Expr) -> Option<BoolV> {
        match e {
            Expr::Bool(b) => Some(BoolV::Lit(*b)),
            Expr::Ident(_) | Expr::Member(..) => {
                let p = self.path(e)?;
                matches!((self.kind)(&p), Some(Kind::Bool { .. })).then(|| BoolV::Field(Term::Field(p)))
            }
            _ => None,
        }
    }
}

/// Whether a string expression is certainly not empty: a concatenation with a non-empty
/// literal in it.
fn nonempty(e: &Expr) -> bool {
    match e {
        Expr::Str(s) => !s.is_empty(),
        Expr::Bin(a, Op::Add, b) => nonempty(a) || nonempty(b),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::{Relation, Term};

    /// The fields of the message the tests read: whole numbers, a string, a flag, a list, and
    /// an `optional` number.
    fn kinds(p: &[String]) -> Option<Kind> {
        Some(match p.join(".").as_str() {
            "min_weight" | "max_weight" | "a" | "b" | "weight" => Kind::Int { presence: false },
            "coupon" => Kind::Int { presence: true },
            "region" => Kind::Str { presence: false },
            "express" => Kind::Bool { presence: false },
            "items" => Kind::List,
            "ratio" => Kind::Other,
            _ => return None,
        })
    }

    fn read(src: &str) -> Formula {
        read_src(src, &[], &kinds)
    }

    fn f(p: &str) -> Term {
        Term::Field(p.split('.').map(str::to_string).collect())
    }

    fn span(g: &Formula, t: &str) -> Vec<(Option<i128>, Option<i128>, Vec<i128>)> {
        Relation::of(g).span(&f(t), &Default::default(), &|t| t.word()).unwrap()
    }

    #[test]
    fn 読める式は線形の条件になる() {
        let got = read("this.min_weight <= this.max_weight");
        let Formula::Atom(Atom::Num(l, Rel::Le)) = got else { panic!("{got:?}") };
        assert_eq!(l.terms.get(&f("min_weight")), Some(&Rat::int(1)));
        assert_eq!(l.terms.get(&f("max_weight")), Some(&Rat::int(-1)));
    }

    #[test]
    fn 読めない部分は真として広く読む() {
        // `startsWith` is not read: the conjunction keeps only what is.
        let got = read("this.region.startsWith('a') && this.b > 0").widen();
        assert!(matches!(got, Formula::Atom(Atom::Num(_, Rel::Lt))), "{got:?}");
        // Under a negation an unread part still widens: !(U && x) is !U || !x, and !U is
        // unknown, which is true.
        assert_eq!(read("!(this.region.startsWith('a') && this.b > 0)").widen(), Formula::True);
        // Division truncates; the comparison it is in is not read. Nor is a double.
        assert_eq!(read("this.a / 2 <= this.b").widen(), Formula::True);
        assert_eq!(read("this.ratio <= 0.5").widen(), Formula::True);
    }

    #[test]
    fn 文字列を返す式は空で通る() {
        // Passes exactly when a <= b.
        let g = Formula::and(vec![read("this.a > this.b ? 'a must not exceed b' : ''"), read("this.b <= 10"), read("this.a >= 0")]);
        assert_eq!(span(&g, "a"), vec![(Some(0), Some(10), vec![])]);
        let g = read("this.weight > 100 ? 'weight ' + string(this.weight) + ' is over' : ''");
        assert_eq!(span(&Formula::and(vec![g, read("this.weight >= 0")]), "weight"), vec![(Some(0), Some(100), vec![])]);
    }

    #[test]
    fn 条件つきの上限は二つの場合になる() {
        let g = Formula::and(vec![read("this.weight <= (this.express ? 5000 : 30000)"), read("this.weight >= 1")]);
        let r = Relation::of(&g);
        let x = crate::relation::Extra { bools: vec![(f("express"), true)], ..Default::default() };
        assert_eq!(r.span(&f("weight"), &x, &|t| t.word()).unwrap(), vec![(Some(1), Some(5000), vec![])]);
    }

    #[test]
    fn 一覧と件数と存在() {
        let g = read("this.region in ['honshu', 'hokkaido']");
        assert_eq!(Relation::of(&g).strings(&f("region"), &|t| t.word()), Some(vec!["honshu".to_string(), "hokkaido".to_string()]));
        let g = Formula::and(vec![read("size(this.items) >= 1 && this.items.size() <= 50")]);
        let r = Relation::of(&g);
        assert_eq!(r.span(&Term::Size(vec!["items".into()]), &Default::default(), &|t| t.word()).unwrap(), vec![(Some(1), Some(50), vec![])]);
        // Unset, an `optional` number is 0: `!has(x) || x <= b` allows x = 0 or x ≤ b.
        let g = Formula::and(vec![read("!has(this.coupon) || this.coupon <= this.b"), read("this.b <= 10"), read("this.coupon >= 0")]);
        assert_eq!(span(&g, "coupon"), vec![(Some(0), Some(0), vec![]), (Some(0), Some(10), vec![])]);
        // With no presence of its own, `has` is "not the default".
        assert_eq!(span(&Formula::and(vec![read("has(this.a)"), read("this.a >= 0 && this.a <= 3")]), "a"), vec![(Some(0), Some(3), vec![0])]);
    }

    #[test]
    fn 構文は丸ごと断る() {
        assert!(parse("this.a <= ").is_err());
        assert!(parse("{'a': 1}").is_err());
        assert!(parse("1e3 > this.a").is_err());
        assert!(parse("this.a <= this.b").is_ok());
        assert!(parse("size(this.items) >= 1 && this.items.size() <= 50").is_ok());
        assert!(parse("this.region in ['honshu', \"hokkaido\"]").is_ok());
        assert_eq!(read("this.a <= "), Formula::unknown());
    }
}
