//! The expressions and the statements of the generated code, as a tree, and how each target prints
//! them (DESIGN 10.3). The plan builds the tree once; a target writes its own syntax: its operators
//! and their precedence, where parentheses go, and for Go the spacing `gofmt` gives a binary
//! expression (go/printer's `binaryExpr`), so that the Go is what gofmt writes and `gofmt -l` lists
//! nothing. Rust writes no parentheses the compiler calls unused (DESIGN 36.8).

use super::Target;

/// A binary operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl Op {
    fn is_comparison(self) -> bool {
        matches!(self, Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge)
    }

    /// The operator as the target writes it.
    fn word(self, t: Target) -> &'static str {
        match (self, t) {
            (Op::Add, _) => "+",
            (Op::Sub, _) => "-",
            (Op::Mul, _) => "*",
            (Op::Div, _) => "/",
            (Op::Eq, Target::TypeScript) => "===",
            (Op::Ne, Target::TypeScript) => "!==",
            (Op::Eq, _) => "==",
            (Op::Ne, _) => "!=",
            (Op::Lt, _) => "<",
            (Op::Le, _) => "<=",
            (Op::Gt, _) => ">",
            (Op::Ge, _) => ">=",
            (Op::And, Target::Python) => "and",
            (Op::Or, Target::Python) => "or",
            (Op::And, _) => "&&",
            (Op::Or, _) => "||",
        }
    }

    /// The precedence in the target (higher binds tighter). Go's are the language's own (1 to 5).
    fn prec(self, t: Target) -> u8 {
        match t {
            Target::Go => match self {
                Op::Or => 1,
                Op::And => 2,
                Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => 3,
                Op::Add | Op::Sub => 4,
                Op::Mul | Op::Div => 5,
            },
            Target::TypeScript => match self {
                Op::Or => 1,
                Op::And => 2,
                Op::Eq | Op::Ne => 3,
                Op::Lt | Op::Le | Op::Gt | Op::Ge => 4,
                Op::Add | Op::Sub => 5,
                Op::Mul | Op::Div => 6,
            },
            Target::Python => match self {
                Op::Or => 1,
                Op::And => 2,
                // `not` is 3
                Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => 4,
                Op::Add | Op::Sub => 5,
                Op::Mul | Op::Div => 6,
            },
            // `as` is 6, the unary operators 7
            Target::Rust => match self {
                Op::Or => 1,
                Op::And => 2,
                Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => 3,
                Op::Add | Op::Sub => 4,
                Op::Mul | Op::Div => 5,
            },
        }
    }
}

/// An expression of the generated code.
#[derive(Clone, Debug, PartialEq)]
pub enum Ex {
    /// A name, a literal: what needs no parentheses.
    Atom(String),
    Call(Box<Ex>, Vec<Ex>),
    /// A call with its arguments by name (Python's keyword arguments).
    CallKw(Box<Ex>, Vec<(String, Ex)>),
    Bin(Op, Box<Ex>, Box<Ex>),
    Neg(Box<Ex>),
    Not(Box<Ex>),
    /// `x.f`
    Dot(Box<Ex>, String),
    /// `x[i]`
    Index(Box<Ex>, Box<Ex>),
    /// TypeScript's `c ? a : b`, Python's `a if c else b` (Go has none).
    Cond(Box<Ex>, Box<Ex>, Box<Ex>),
    /// A record built: TypeScript's object literal `{ a: x }`, Go's composite literal `T{A: x}`,
    /// Python's call `T(a=x)`. The type is the target's (empty for TypeScript).
    Record(String, Vec<(String, Ex)>),
    /// A list written out: TypeScript's and Python's `[a, b]`, Go's `[]T{a, b}` (the type is Go's).
    List(String, Vec<Ex>),
    /// A conversion of the target: TypeScript's `x as T`, Go's `T(x)`, Python's `T(x)`, Rust's
    /// `x as T`.
    Cast(Box<Ex>, String),
    /// `*p` (Go, Rust)
    Deref(Box<Ex>),
    /// `&x` (Rust)
    Ref(Box<Ex>),
    /// `x?` (Rust): the value, or the function returns the error.
    Try(Box<Ex>),
}

impl Ex {
    pub fn atom(s: impl Into<String>) -> Ex {
        Ex::Atom(s.into())
    }

    pub fn call(f: impl Into<String>, args: Vec<Ex>) -> Ex {
        Ex::Call(Box::new(Ex::Atom(f.into())), args)
    }

    pub fn bin(op: Op, a: Ex, b: Ex) -> Ex {
        Ex::Bin(op, Box::new(a), Box::new(b))
    }

    pub fn dot(self, f: impl Into<String>) -> Ex {
        Ex::Dot(Box::new(self), f.into())
    }

    /// Whether the expression can be written twice without doing anything twice: a name, a literal,
    /// a field of one.
    pub fn is_simple(&self) -> bool {
        match self {
            Ex::Atom(_) => true,
            Ex::Dot(x, _) => x.is_simple(),
            Ex::Deref(x) => x.is_simple(),
            _ => false,
        }
    }

    /// The expression, written by `t`.
    pub fn print(&self, t: Target) -> String {
        match t {
            Target::Go => go_expr(self, 0, 1),
            _ => expr(self, t, 0),
        }
    }

    /// `&x`
    pub fn borrow(self) -> Ex {
        Ex::Ref(Box::new(self))
    }

    /// `x?`
    pub fn tried(self) -> Ex {
        Ex::Try(Box::new(self))
    }

    /// `x.f(args)`: a method of the value (Rust).
    pub fn method(self, f: &str, args: Vec<Ex>) -> Ex {
        Ex::Call(Box::new(self.dot(f)), args)
    }

    /// Whether the expression returns from the function where it fails: Rust's `?` in it, which a
    /// closure cannot hold.
    pub fn tries(&self) -> bool {
        match self {
            Ex::Try(_) => true,
            Ex::Atom(a) => a.contains(")?") || a.ends_with('?'),
            Ex::Call(f, xs) => f.tries() || xs.iter().any(Ex::tries),
            Ex::CallKw(f, xs) => f.tries() || xs.iter().any(|(_, x)| x.tries()),
            Ex::Bin(_, a, b) | Ex::Index(a, b) => a.tries() || b.tries(),
            Ex::Neg(x) | Ex::Not(x) | Ex::Dot(x, _) | Ex::Cast(x, _) | Ex::Deref(x) | Ex::Ref(x) => x.tries(),
            Ex::Cond(c, a, b) => c.tries() || a.tries() || b.tries(),
            Ex::Record(_, fs) => fs.iter().any(|(_, x)| x.tries()),
            Ex::List(_, xs) => xs.iter().any(Ex::tries),
        }
    }
}

/// The precedence of what is not a binary expression, as an operand: a unary expression binds
/// tighter than any binary operator, a call or a name tighter still.
const UNARY: u8 = 7;
const PRIMARY: u8 = 9;

fn own_prec(x: &Ex, t: Target) -> u8 {
    match x {
        Ex::Bin(op, ..) => op.prec(t),
        Ex::Neg(_) | Ex::Ref(_) | Ex::Deref(_) if t == Target::Rust => UNARY,
        Ex::Neg(_) => UNARY,
        Ex::Not(_) => {
            if t == Target::Python {
                3
            } else {
                UNARY
            }
        }
        Ex::Cond(..) => 0,
        // `x as T` binds as a relational operator does
        Ex::Cast(..) if t == Target::TypeScript => 4,
        // and in Rust tighter than `*`
        Ex::Cast(..) if t == Target::Rust => 6,
        _ => PRIMARY,
    }
}

/// TypeScript and Python: an expression in a place that binds at `min` or tighter, in parentheses
/// when it binds looser. A comparison inside a comparison is put in parentheses too (Python chains
/// them).
fn expr(x: &Ex, t: Target, min: u8) -> String {
    let p = own_prec(x, t);
    let s = expr_bare(x, t);
    if p < min { format!("({s})") } else { s }
}

fn expr_bare(x: &Ex, t: Target) -> String {
    match x {
        Ex::Atom(s) => s.clone(),
        Ex::Call(f, args) => format!("{}({})", expr(f, t, PRIMARY), args.iter().map(|a| expr(a, t, 0)).collect::<Vec<_>>().join(", ")),
        // an argument with no keyword goes by its place: the order a generic function is given first
        // (DESIGN 26.7)
        Ex::CallKw(f, args) => format!(
            "{}({})",
            expr(f, t, PRIMARY),
            args.iter().map(|(k, a)| if k.is_empty() { expr(a, t, 0) } else { format!("{k}={}", expr(a, t, 0)) }).collect::<Vec<_>>().join(", ")
        ),
        Ex::Bin(op, a, b) => {
            let p = op.prec(t);
            // left-associative: the right operand of the same precedence goes in parentheses; and a
            // comparison never takes a comparison as an operand bare
            let lmin = if op.is_comparison() { p + 1 } else { p };
            let left = expr(a, t, lmin);
            // Rust reads `x as T < y` as the start of a type's arguments, and a struct's literal
            // after `if x ==` as the body of the `if`
            let left = if t == Target::Rust && matches!(**a, Ex::Cast(..) | Ex::Record(..)) && !left.starts_with('(') { format!("({left})") } else { left };
            let right = expr(b, t, p + 1);
            let right = if t == Target::Rust && matches!(**b, Ex::Record(..)) { format!("({right})") } else { right };
            format!("{left} {} {right}", op.word(t))
        }
        Ex::Neg(a) => {
            let inner = expr(a, t, UNARY);
            // `- -x` would read as a decrement in TypeScript
            if inner.starts_with('-') { format!("-({inner})") } else { format!("-{inner}") }
        }
        Ex::Ref(a) => format!("&{}", expr(a, t, UNARY)),
        Ex::Try(a) => format!("{}?", expr(a, t, PRIMARY)),
        Ex::Not(a) => match t {
            Target::Python => format!("not {}", expr(a, t, 3)),
            _ => format!("!{}", expr(a, t, UNARY)),
        },
        Ex::Dot(a, f) => format!("{}.{f}", expr(a, t, PRIMARY)),
        Ex::Index(a, i) => format!("{}[{}]", expr(a, t, PRIMARY), expr(i, t, 0)),
        Ex::Cond(c, a, b) => match t {
            Target::Python => format!("{} if {} else {}", expr(a, t, 1), expr(c, t, 1), expr(b, t, 0)),
            Target::Rust => format!("if {} {{ {} }} else {{ {} }}", rust_cond(c), expr(a, t, 0), expr(b, t, 0)),
            _ => format!("{} ? {} : {}", expr(c, t, 1), expr(a, t, 1), expr(b, t, 0)),
        },
        Ex::Record(ty, fields) => match t {
            Target::Python => format!("{ty}({})", fields.iter().map(|(k, v)| format!("{k}={}", expr(v, t, 0))).collect::<Vec<_>>().join(", ")),
            // the name of a field the value has is written once (`Claim { due }`), and the value
            // whose fields the others replace after them (`..base`)
            Target::Rust if fields.is_empty() => format!("{} {{}}", turbofish(ty)),
            Target::Rust => {
                let mut parts: Vec<String> = fields
                    .iter()
                    .filter(|(k, _)| k != "...")
                    .map(|(k, v)| match v {
                        Ex::Atom(a) if a == k => k.clone(),
                        _ => format!("{k}: {}", expr(v, t, 0)),
                    })
                    .collect();
                if let Some((_, base)) = fields.iter().find(|(k, _)| k == "...") {
                    parts.push(format!("..{}", expr(base, t, UNARY)));
                }
                format!("{} {{ {} }}", turbofish(ty), parts.join(", "))
            }
            _ if fields.is_empty() => "{}".to_string(),
            // a field named `...` is the spread of a value whose fields the others replace
            _ => format!("{{ {} }}", fields.iter().map(|(k, v)| if k == "..." { format!("...{}", expr(v, t, UNARY)) } else { format!("{k}: {}", expr(v, t, 0)) }).collect::<Vec<_>>().join(", ")),
        },
        Ex::List(_, xs) if t == Target::Rust => {
            if xs.is_empty() {
                "Vec::new()".to_string()
            } else {
                format!("vec![{}]", xs.iter().map(|a| expr(a, t, 0)).collect::<Vec<_>>().join(", "))
            }
        }
        Ex::List(_, xs) => format!("[{}]", xs.iter().map(|a| expr(a, t, 0)).collect::<Vec<_>>().join(", ")),
        Ex::Cast(a, ty) => match t {
            Target::TypeScript => format!("{} as {ty}", expr(a, t, UNARY)),
            Target::Rust => format!("{} as {ty}", expr(a, t, UNARY)),
            _ => format!("{ty}({})", expr(a, t, 0)),
        },
        Ex::Deref(a) if t == Target::Rust => format!("*{}", expr(a, t, UNARY)),
        Ex::Deref(a) => expr(a, t, PRIMARY),
    }
}

/// The condition of a Rust `if`: a struct's literal in it goes in parentheses, which the compiler
/// would read as the body of the `if`.
/// A type as a struct's literal of Rust names it: its arguments after `::` (`Page::<T> { … }`), which an
/// expression reads as a comparison otherwise.
fn turbofish(ty: &str) -> String {
    match ty.find('<') {
        Some(i) if !ty[..i].ends_with("::") => format!("{}::{}", &ty[..i], &ty[i..]),
        _ => ty.to_string(),
    }
}

pub fn rust_cond(c: &Ex) -> String {
    match c {
        Ex::Record(..) => format!("({})", expr(c, Target::Rust, 0)),
        _ => expr(c, Target::Rust, 0),
    }
}

// ── Go, as gofmt prints it ──────────────────────────────────────────────────

/// gofmt's `walkBinary`: whether the expression has operators of precedence 4 and 5, and the
/// cutoff a clash of an operator with a unary operand after it asks for. A part that will be in
/// parentheses is not looked into.
fn walk_binary(op: Op, a: &Ex, b: &Ex) -> (bool, bool, u8) {
    let p = op.prec(Target::Go);
    let (mut has4, mut has5, mut problem) = (p == 4, p == 5, 0u8);
    if let Ex::Bin(lop, la, lb) = a
        && lop.prec(Target::Go) >= p
    {
        let (h4, h5, mp) = walk_binary(*lop, la, lb);
        has4 |= h4;
        has5 |= h5;
        problem = problem.max(mp);
    }
    match b {
        Ex::Bin(rop, ra, rb) if rop.prec(Target::Go) > p => {
            let (h4, h5, mp) = walk_binary(*rop, ra, rb);
            has4 |= h4;
            has5 |= h5;
            problem = problem.max(mp);
        }
        Ex::Deref(_) if op == Op::Div => problem = 5,
        Ex::Neg(_) if op == Op::Sub => problem = problem.max(4),
        _ => {}
    }
    (has4, has5, problem)
}

/// gofmt's `cutoff`.
fn cutoff(op: Op, a: &Ex, b: &Ex, depth: u8) -> u8 {
    let (has4, has5, problem) = walk_binary(op, a, b);
    if problem > 0 {
        return problem + 1;
    }
    if has4 && has5 {
        return if depth == 1 { 5 } else { 4 };
    }
    if depth == 1 { 6 } else { 4 }
}

fn diff_prec(x: &Ex, prec: u8) -> u8 {
    match x {
        Ex::Bin(op, ..) if op.prec(Target::Go) == prec => 0,
        _ => 1,
    }
}

fn reduce_depth(depth: u8) -> u8 {
    if depth > 1 { depth - 1 } else { 1 }
}

/// An expression of Go as gofmt prints it: in a place of precedence `prec1` (parentheses when it
/// binds looser), at `depth` (1 at the top of a statement).
fn go_expr(x: &Ex, prec1: u8, depth: u8) -> String {
    match x {
        Ex::Bin(op, a, b) => {
            let prec = op.prec(Target::Go);
            if prec < prec1 {
                return format!("({})", go_expr(x, 0, reduce_depth(depth)));
            }
            let blank = prec < cutoff(*op, a, b, depth);
            let left = go_expr(a, prec, depth + diff_prec(a, prec));
            let right = go_expr(b, prec + 1, depth + 1);
            if blank { format!("{left} {} {right}", op.word(Target::Go)) } else { format!("{left}{}{right}", op.word(Target::Go)) }
        }
        Ex::Neg(a) | Ex::Not(a) | Ex::Deref(a) => {
            let sym = match x {
                Ex::Neg(_) => "-",
                Ex::Not(_) => "!",
                _ => "*",
            };
            if 6 < prec1 {
                return format!("({})", go_expr(x, 0, reduce_depth(depth)));
            }
            let inner = go_expr(a, 6, depth);
            // `- -x` and `--x`: gofmt keeps them apart only by parentheses
            if sym == "-" && inner.starts_with('-') { format!("-({})", go_expr(a, 0, reduce_depth(depth))) } else { format!("{sym}{inner}") }
        }
        Ex::Atom(s) => s.clone(),
        Ex::Call(f, args) => {
            let d = if args.len() > 1 { depth + 1 } else { depth };
            format!("{}({})", go_expr(f, 7, depth), args.iter().map(|a| go_expr(a, 0, d)).collect::<Vec<_>>().join(", "))
        }
        Ex::CallKw(f, args) => go_expr(&Ex::Call(f.clone(), args.iter().map(|(_, a)| a.clone()).collect()), prec1, depth),
        Ex::Dot(a, f) => format!("{}.{f}", go_expr(a, 7, depth)),
        Ex::Index(a, i) => format!("{}[{}]", go_expr(a, 7, 1), go_expr(i, 0, depth + 1)),
        Ex::Cond(..) => unreachable!("Go has no conditional expression"),
        Ex::Record(ty, fields) => format!("{ty}{{{}}}", fields.iter().map(|(k, v)| format!("{k}: {}", go_expr(v, 0, 1))).collect::<Vec<_>>().join(", ")),
        Ex::List(ty, xs) => format!("{ty}{{{}}}", xs.iter().map(|a| go_expr(a, 0, 1)).collect::<Vec<_>>().join(", ")),
        Ex::Cast(a, ty) => {
            // a conversion is a call of the type
            format!("{ty}({})", go_expr(a, 0, depth))
        }
        Ex::Ref(_) | Ex::Try(_) => unreachable!("Go has no `&x` or `x?` of a value"),
    }
    .to_string()
}

// ── Statements ─────────────────────────────────────────────────────────────

/// A statement of the generated code.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// A line as it is: `const x = …`, `return x`, a comment.
    Line(String),
    /// `if c1 { … } else if c2 { … } else { … }`: each condition with its body, and the last
    /// `else`'s body (empty for none).
    If(Vec<(Ex, Vec<Node>)>, Vec<Node>),
    /// A statement with a body: `for (…)`, `try`, a function. The head, the body, then each clause
    /// after it with its body (`catch (e)`, `except X as e`). The head and the clauses are written
    /// as the target writes them, without the brace or the colon.
    Block(String, Vec<Node>, Vec<(String, Vec<Node>)>),
    /// A blank line.
    Blank,
    /// Go's `switch x := v.(type) {` (the head, written without the brace), each `case` with its
    /// types and its body, and the body of `default`; Rust's `match v {` with each arm's pattern
    /// and its body, and the body of `_` (none where it is empty).
    Switch(String, Vec<(String, Vec<Node>)>, Vec<Node>),
}

/// The statements, written by `t` at `depth` levels of indentation.
pub fn print(nodes: &[Node], t: Target, depth: usize, out: &mut String) {
    let pad = |d: usize| match t {
        Target::Go => "\t".repeat(d),
        Target::TypeScript => "  ".repeat(d),
        Target::Python | Target::Rust => "    ".repeat(d),
    };
    for n in nodes {
        match n {
            Node::Line(s) => {
                out.push_str(&pad(depth));
                out.push_str(s);
                out.push('\n');
            }
            Node::Blank => out.push('\n'),
            Node::If(arms, els) => {
                for (i, (c, body)) in arms.iter().enumerate() {
                    let cond = if t == Target::Rust { rust_cond(c) } else { c.print(t) };
                    let head = match (t, i) {
                        (Target::Python, 0) => format!("if {cond}:"),
                        (Target::Python, _) => format!("elif {cond}:"),
                        (Target::TypeScript, 0) => format!("if ({cond}) {{"),
                        (Target::TypeScript, _) => format!("}} else if ({cond}) {{"),
                        (Target::Go | Target::Rust, 0) => format!("if {cond} {{"),
                        (Target::Go | Target::Rust, _) => format!("}} else if {cond} {{"),
                    };
                    out.push_str(&pad(depth));
                    out.push_str(&head);
                    out.push('\n');
                    body_of(body, t, depth + 1, out);
                }
                if !els.is_empty() {
                    out.push_str(&pad(depth));
                    out.push_str(if t == Target::Python { "else:\n" } else { "} else {\n" });
                    body_of(els, t, depth + 1, out);
                }
                if t != Target::Python {
                    out.push_str(&pad(depth));
                    out.push_str("}\n");
                }
            }
            Node::Switch(head, cases, default) if t == Target::Rust => {
                out.push_str(&pad(depth));
                out.push_str(head);
                out.push_str(" {\n");
                for (pat, body) in cases {
                    out.push_str(&pad(depth + 1));
                    if body.is_empty() {
                        out.push_str(&format!("{pat} => {{}}\n"));
                        continue;
                    }
                    out.push_str(&format!("{pat} => {{\n"));
                    print(body, t, depth + 2, out);
                    out.push_str(&pad(depth + 1));
                    out.push_str("}\n");
                }
                if !default.is_empty() {
                    out.push_str(&pad(depth + 1));
                    out.push_str("_ => {\n");
                    print(default, t, depth + 2, out);
                    out.push_str(&pad(depth + 1));
                    out.push_str("}\n");
                }
                out.push_str(&pad(depth));
                out.push_str("}\n");
            }
            Node::Switch(head, cases, default) => {
                out.push_str(&pad(depth));
                out.push_str(head);
                out.push_str(" {\n");
                for (label, body) in cases {
                    out.push_str(&pad(depth));
                    out.push_str(&format!("case {label}:\n"));
                    print(body, t, depth + 1, out);
                }
                // no `default` where nothing is done for the other values
                if !default.is_empty() {
                    out.push_str(&pad(depth));
                    out.push_str("default:\n");
                    print(default, t, depth + 1, out);
                }
                out.push_str(&pad(depth));
                out.push_str("}\n");
            }
            Node::Block(head, body, clauses) => {
                out.push_str(&pad(depth));
                out.push_str(head);
                out.push_str(if t == Target::Python { ":\n" } else { " {\n" });
                body_of(body, t, depth + 1, out);
                for (c, b) in clauses {
                    out.push_str(&pad(depth));
                    if t == Target::Python {
                        out.push_str(&format!("{c}:\n"));
                    } else {
                        out.push_str(&format!("}} {c} {{\n"));
                    }
                    body_of(b, t, depth + 1, out);
                }
                if t != Target::Python {
                    out.push_str(&pad(depth));
                    // Rust: a closure given a name ends the `let` of it
                    out.push_str(if t == Target::Rust && head.starts_with("let ") { "};\n" } else { "}\n" });
                }
            }
        }
    }
}

/// Whether every way through the statements ends in a `return`: the last a `return`, or an `if`
/// with an `else`, or a `match`, every branch of which does.
fn returns(nodes: &[Node]) -> bool {
    match nodes.last() {
        Some(Node::Line(s)) => s.starts_with("return ") && s.ends_with(';'),
        Some(Node::If(arms, els)) => !els.is_empty() && returns(els) && arms.iter().all(|(_, b)| returns(b)),
        Some(Node::Switch(_, cases, default)) => cases.iter().all(|(_, b)| returns(b)) && (default.is_empty() || returns(default)),
        _ => false,
    }
}

/// Rust: a body that ends in a `return`, as Rust's code ends a body: the value of the last `return`
/// is the body's value, into every branch of the `if` or the `match` that ends it (clippy's
/// `needless_return`), and a name given a value just before it is returned is that value
/// (`let_and_return`).
pub fn tail(mut nodes: Vec<Node>) -> Vec<Node> {
    if !returns(&nodes) {
        return nodes;
    }
    match nodes.pop() {
        Some(Node::Line(s)) => {
            let x = s["return ".len()..s.len() - 1].to_string();
            let folded = match nodes.last() {
                Some(Node::Line(prev)) if prev.starts_with(&format!("let {x}: ")) && prev.ends_with(';') => prev.split_once(" = ").map(|(_, v)| v[..v.len() - 1].to_string()),
                _ => None,
            };
            match folded {
                Some(v) => {
                    nodes.pop();
                    nodes.push(Node::Line(v));
                }
                None => nodes.push(Node::Line(x)),
            }
        }
        Some(Node::If(arms, els)) => {
            let arms = arms.into_iter().map(|(c, b)| (c, tail(b))).collect();
            nodes.push(Node::If(arms, tail(els)));
        }
        Some(Node::Switch(h, cases, default)) => {
            let cases = cases.into_iter().map(|(p, b)| (p, tail(b))).collect();
            nodes.push(Node::Switch(h, cases, tail(default)));
        }
        Some(n) => nodes.push(n),
        None => {}
    }
    nodes
}

fn body_of(nodes: &[Node], t: Target, depth: usize, out: &mut String) {
    if nodes.is_empty() && t == Target::Python {
        out.push_str(&format!("{}pass\n", "    ".repeat(depth)));
    } else {
        print(nodes, t, depth, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Ex {
        Ex::atom(s)
    }

    #[test]
    fn go_spaces_as_gofmt_does() {
        let sum = Ex::bin(Op::Add, a("a"), a("b"));
        let prod = Ex::bin(Op::Mul, a("c"), a("d"));
        assert_eq!(sum.print(Target::Go), "a + b");
        assert_eq!(Ex::bin(Op::Add, a("a"), prod.clone()).print(Target::Go), "a + c*d");
        assert_eq!(Ex::call("f", vec![sum.clone(), prod.clone()]).print(Target::Go), "f(a+b, c*d)");
        assert_eq!(Ex::call("f", vec![sum.clone()]).print(Target::Go), "f(a + b)");
        assert_eq!(Ex::bin(Op::Mul, sum.clone(), a("c")).print(Target::Go), "(a + b) * c");
        assert_eq!(Ex::call("f", vec![Ex::bin(Op::Mul, sum.clone(), a("c")), a("x")]).print(Target::Go), "f((a+b)*c, x)");
        let cmp = Ex::bin(Op::Lt, a("a"), a("b"));
        assert_eq!(Ex::bin(Op::And, cmp.clone(), Ex::bin(Op::Ge, a("b"), a("c"))).print(Target::Go), "a < b && b >= c");
        assert_eq!(Ex::bin(Op::Sub, a("a"), Ex::bin(Op::Sub, a("b"), a("c"))).print(Target::Go), "a - (b - c)");
        assert_eq!(Ex::Record("T".into(), vec![("A".into(), sum.clone())]).print(Target::Go), "T{A: a + b}");
    }

    #[test]
    fn typescript_and_python_parenthesize_by_precedence() {
        let sum = Ex::bin(Op::Add, a("a"), a("b"));
        assert_eq!(Ex::bin(Op::Mul, sum.clone(), a("c")).print(Target::TypeScript), "(a + b) * c");
        assert_eq!(Ex::bin(Op::Sub, a("a"), sum.clone()).print(Target::Python), "a - (a + b)");
        let cmp = Ex::bin(Op::Lt, a("a"), a("b"));
        assert_eq!(Ex::bin(Op::Eq, cmp.clone(), a("t")).print(Target::Python), "(a < b) == t");
        assert_eq!(Ex::Not(Box::new(Ex::bin(Op::And, a("x"), a("y")))).print(Target::Python), "not (x and y)");
        assert_eq!(Ex::Cast(Box::new(sum), "units.USDc".into()).print(Target::TypeScript), "(a + b) as units.USDc");
    }
}
