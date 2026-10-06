//! The writer: a policy set as Cedar text, with the parentheses the grammar needs and no others,
//! laid out by [`super::format::format`] (so the text is what `cedar format` makes of it).

use super::ast::*;
use super::Error;

/// A string's inside, escaped so that Cedar reads it back as the same characters.
pub(crate) fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        push_escaped(&mut o, c);
    }
    o
}

fn push_escaped(o: &mut String, c: char) {
    match c {
        '\\' => o.push_str("\\\\"),
        '"' => o.push_str("\\\""),
        '\n' => o.push_str("\\n"),
        '\r' => o.push_str("\\r"),
        '\t' => o.push_str("\\t"),
        '\0' => o.push_str("\\0"),
        c if c.is_control() => o.push_str(&format!("\\u{{{:x}}}", c as u32)),
        c => o.push(c),
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", escape(s))
}

const RESERVED: [&str; 9] = ["if", "true", "false", "then", "else", "in", "is", "has", "like"];

/// Whether a word can stand bare as an attribute or a record's key.
fn bare(s: &str) -> bool {
    let mut cs = s.chars();
    matches!(cs.next(), Some(c) if c == '_' || c.is_ascii_alphabetic())
        && cs.all(|c| c == '_' || c.is_ascii_alphanumeric())
        && !RESERVED.contains(&s)
        && s != "__cedar"
}

fn entity(e: &EntityUid) -> String {
    format!("{}::{}", e.ty, quote(&e.id))
}

fn entity_or_slot(e: &EntityOrSlot) -> String {
    match e {
        EntityOrSlot::Entity(u) => entity(u),
        EntityOrSlot::Slot(s) => s.as_str().to_string(),
    }
}

fn scope(var: &str, s: &Scope) -> String {
    match s {
        Scope::Any => var.to_string(),
        Scope::Eq(e) => format!("{var} == {}", entity_or_slot(e)),
        Scope::In(e) => format!("{var} in {}", entity_or_slot(e)),
        Scope::Is(t) => format!("{var} is {t}"),
        Scope::IsIn(t, e) => format!("{var} is {t} in {}", entity_or_slot(e)),
    }
}

fn action_scope(s: &ActionScope) -> String {
    match s {
        ActionScope::Any => "action".to_string(),
        ActionScope::Eq(e) => format!("action == {}", entity(e)),
        ActionScope::In(e) => format!("action in {}", entity(e)),
        ActionScope::InList(es) => format!("action in [{}]", es.iter().map(entity).collect::<Vec<_>>().join(", ")),
    }
}

/// The grammar's levels, loosest first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Expr,
    Or,
    And,
    Relation,
    Add,
    Mult,
    Unary,
    Member,
}

fn level(e: &Expr) -> Level {
    match &e.kind {
        ExprKind::If { .. } => Level::Expr,
        ExprKind::Binary { op: BinOp::Or, .. } => Level::Or,
        ExprKind::Binary { op: BinOp::And, .. } => Level::And,
        ExprKind::Binary { op: BinOp::Add | BinOp::Sub, .. } => Level::Add,
        ExprKind::Binary { op: BinOp::Mul, .. } => Level::Mult,
        ExprKind::Binary { .. } | ExprKind::Has { .. } | ExprKind::Like { .. } | ExprKind::Is { .. } => Level::Relation,
        ExprKind::Not(_) | ExprKind::Neg(_) => Level::Unary,
        ExprKind::Long(n) if *n < 0 => Level::Unary,
        _ => Level::Member,
    }
}

fn at(e: &Expr, want: Level) -> String {
    let s = expr(e);
    if level(e) < want { format!("({s})") } else { s }
}

/// `!` or `-` as many times as the grammar allows in a row (four), then parentheses.
fn unary(sym: char, mut e: &Expr) -> String {
    let mut n = 0;
    while let (ExprKind::Not(inner), '!') | (ExprKind::Neg(inner), '-') = (&e.kind, sym) {
        n += 1;
        e = inner;
        if n == 4 {
            break;
        }
    }
    let ops: String = std::iter::repeat_n(sym, n).collect();
    // `-5` is the literal −5, so a minus before a literal that is not negative needs
    // parentheses, and a negative one adds a fifth minus when four are written already
    let operand = match (&e.kind, sym) {
        (ExprKind::Long(v), '-') if *v >= 0 => format!("({v})"),
        (ExprKind::Not(_), '!') | (ExprKind::Neg(_), '-') => format!("({})", expr(e)),
        (ExprKind::Long(v), '-') if *v < 0 && n < 4 => expr(e),
        (ExprKind::Long(_), '-') => format!("({})", expr(e)),
        _ => at(e, Level::Member),
    };
    format!("{ops}{operand}")
}

fn args(xs: &[Expr]) -> String {
    xs.iter().map(expr).collect::<Vec<_>>().join(", ")
}

fn attr_access(recv: &Expr, a: &str) -> String {
    let r = at(recv, Level::Member);
    if bare(a) { format!("{r}.{a}") } else { format!("{r}[{}]", quote(a)) }
}

fn pattern(p: &[PatternElem]) -> String {
    let mut o = String::from("\"");
    for e in p {
        match e {
            PatternElem::Wildcard => o.push('*'),
            PatternElem::Char('*') => o.push_str("\\*"),
            PatternElem::Char(c) => push_escaped(&mut o, *c),
        }
    }
    o.push('"');
    o
}

pub(crate) fn expr(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Long(n) => n.to_string(),
        ExprKind::Str(s) => quote(s),
        ExprKind::Entity(u) => entity(u),
        ExprKind::Var(v) => v.as_str().to_string(),
        ExprKind::Slot(s) => s.as_str().to_string(),
        ExprKind::Not(_) => unary('!', e),
        ExprKind::Neg(_) => unary('-', e),
        ExprKind::Binary { op, left, right } => {
            let (l, r) = match op {
                BinOp::Or => (Level::Or, Level::And),
                BinOp::And => (Level::And, Level::Relation),
                BinOp::Add | BinOp::Sub => (Level::Add, Level::Mult),
                BinOp::Mul => (Level::Mult, Level::Unary),
                _ => (Level::Add, Level::Add),
            };
            format!("{} {} {}", at(left, l), op.as_str(), at(right, r))
        }
        ExprKind::GetAttr { expr: x, attr } => attr_access(x, attr),
        ExprKind::Has { expr: x, attrs } => {
            let t = at(x, Level::Add);
            if attrs.len() == 1 && !bare(&attrs[0]) {
                format!("{t} has {}", quote(&attrs[0]))
            } else if attrs.iter().all(|a| bare(a)) {
                format!("{t} has {}", attrs.join("."))
            } else {
                // a path of names one of which cannot stand bare: each step on its own
                let mut parts = Vec::new();
                let mut recv = (**x).clone();
                for a in attrs {
                    parts.push(format!("{} has {}", at(&recv, Level::Add), if bare(a) { a.clone() } else { quote(a) }));
                    recv = Expr::new(ExprKind::GetAttr { expr: Box::new(recv), attr: a.clone() });
                }
                format!("({})", parts.join(" && "))
            }
        }
        ExprKind::Like { expr: x, pattern: p } => format!("{} like {}", at(x, Level::Add), pattern(p)),
        ExprKind::Is { expr: x, ty, in_expr } => match in_expr {
            Some(i) => format!("{} is {ty} in {}", at(x, Level::Add), at(i, Level::Add)),
            None => format!("{} is {ty}", at(x, Level::Add)),
        },
        ExprKind::If { cond, then, els } => format!("if {} then {} else {}", expr(cond), expr(then), expr(els)),
        ExprKind::Set(xs) => format!("[{}]", args(xs)),
        ExprKind::Record(kv) => {
            let items: Vec<String> = kv.iter().map(|(k, v)| format!("{}: {}", if bare(k) { k.clone() } else { quote(k) }, expr(v))).collect();
            format!("{{{}}}", items.join(", "))
        }
        ExprKind::Method { expr: recv, name, args: xs } => format!("{}.{name}({})", at(recv, Level::Member), args(xs)),
        ExprKind::Call { func, args: xs } => format!("{func}({})", args(xs)),
    }
}

/// One policy as unformatted Cedar text.
pub(crate) fn policy_text(p: &Policy) -> String {
    let mut o = String::new();
    for a in &p.annotations {
        match &a.value {
            Some(v) => o.push_str(&format!("@{}({})\n", a.key, quote(v))),
            None => o.push_str(&format!("@{}\n", a.key)),
        }
    }
    let effect = match p.effect {
        Effect::Permit => "permit",
        Effect::Forbid => "forbid",
    };
    o.push_str(&format!("{effect} ({}, {}, {})", scope("principal", &p.principal), action_scope(&p.action), scope("resource", &p.resource)));
    for c in &p.conditions {
        let w = match c.kind {
            CondKind::When => "when",
            CondKind::Unless => "unless",
        };
        o.push_str(&format!("\n{w} {{ {} }}", expr(&c.body)));
    }
    o.push_str(";\n");
    o
}

pub(crate) fn policies(set: &PolicySet) -> Result<String, Error> {
    let text: Vec<String> = set.policies.iter().map(policy_text).collect();
    super::format::format(&text.join("\n"), 80, 2)
}
