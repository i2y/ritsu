//! What a ticket does when it is submitted and when it ends (DESIGN 7.4): the `on` lines of the
//! request's `ticket` block, each a hold, a post or a void of a book's transfer, or a flow to
//! launch, with its arguments and the condition it is done on.
//!
//! The arguments and the conditions are small expressions over the ticket: the values of its form,
//! `ticket.id` and `ticket.round`, `requester.id` and the team of a kind above the requester
//! (`requester.department`), a rule's output, a master's field in the row a field chose. They are
//! evaluated here, on the values the ticket holds and the part of the organization the event comes
//! with. A `post` and a `void` are given only while the hold of the ticket's round is open, so that
//! doing the list twice posts once.

use super::{Effects, Held, Machine, OrgPart, Refusal, Ticket, What};
use crate::ast::{ActionKind, BinOp, CmpOp, Ending, Expr, ExprKind, Field, TicketLine, TypeExpr};
use ritsu_base::text::Text;
use ritsu_ports::{Value, Values};
use ritsu_units::Rat;
use std::cmp::Ordering;

/// A value of an expression of an `on` line.
#[derive(Clone, Debug)]
enum V {
    Num(Rat),
    Day(i64),
    Str(String),
    Bool(bool),
    None,
}

impl V {
    fn same(&self, o: &V) -> bool {
        match (self, o) {
            (V::Num(a), V::Num(b)) => a.cmp_to(*b) == Ordering::Equal,
            (V::Day(a), V::Day(b)) => a == b,
            (V::Str(a), V::Str(b)) => a == b,
            (V::Bool(a), V::Bool(b)) => a == b,
            (V::None, V::None) => true,
            _ => false,
        }
    }

    fn order(&self, o: &V) -> Option<Ordering> {
        match (self, o) {
            (V::Num(a), V::Num(b)) => Some(a.cmp_to(*b)),
            (V::Day(a), V::Day(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}

/// A value of the form, read as its field writes it: a number in its unit, a date as its day, a
/// choice by its name.
fn of_field(f: Option<&Field>, v: &Value) -> V {
    match v {
        Value::Bool(b) => V::Bool(*b),
        Value::Int(k) => {
            let step = match f.map(|f| &f.ty) {
                Some(TypeExpr::Num(u)) => crate::lower::words::step(u),
                _ => Rat::int(1),
            };
            V::Num(Rat::int(*k).mul(step))
        }
        Value::Str(s) | Value::Enum(s) => V::Str(s.clone()),
        Value::Date(d) => ritsu_ports::day_of(d).map(V::Day).unwrap_or(V::None),
        Value::List(_) | Value::None => V::None,
    }
}

struct Reads<'x, 's, 'a> {
    m: &'x Machine<'s, 'a>,
    t: &'x Ticket,
    part: &'x OrgPart,
}

impl Reads<'_, '_, '_> {
    fn field(&self, e: &Expr) -> Option<&Field> {
        match &e.kind {
            ExprKind::Path(p) if p.len() == 1 => self.m.s.field(&p[0].text),
            _ => None,
        }
    }

    /// A word that is a value of a choice: a single name that is no field.
    fn word(&self, e: &Expr) -> Option<String> {
        match &e.kind {
            ExprKind::Path(p) if p.len() == 1 && self.m.s.field(&p[0].text).is_none() => Some(p[0].text.clone()),
            _ => None,
        }
    }

    /// `e`, read beside `beside` where it is a constant or a word: a number in the unit of the
    /// field it is compared with, a word as the value of that field's choice.
    fn beside(&self, e: &Expr, beside: &Expr) -> Result<V, Text> {
        let f = self.field(beside);
        if let Some(w) = self.word(e) {
            let name = f.and_then(|f| f.options.iter().find(|o| o.name.text == w || o.name.ascii() == w)).map(|o| o.name.ascii().to_string()).unwrap_or(w);
            return Ok(V::Str(name));
        }
        if let (Some(f), ExprKind::Num(_) | ExprKind::Neg(_)) = (f, &e.kind)
            && let Some(c) = crate::lower::words::constant_in(e, &f.ty)
        {
            return Ok(V::Num(c.rat()));
        }
        self.eval(e)
    }

    fn eval(&self, e: &Expr) -> Result<V, Text> {
        let (s, t) = (self.m.s, self.t);
        let no = || {
            let w = crate::types::written(s.src, e.span);
            tr!("`{w}` を計算できません", "`{w}` cannot be worked out")
        };
        let num = |v: V| match v {
            V::Num(r) => Ok(r),
            _ => Err(no()),
        };
        let truth = |v: V| match v {
            V::Bool(b) => Ok(b),
            _ => Err(no()),
        };
        Ok(match &e.kind {
            ExprKind::Num(_) => crate::lower::words::constant(e).map(|c| V::Num(c.rat())).ok_or_else(no)?,
            ExprKind::Date(d) => V::Day(*d),
            ExprKind::Str(x) | ExprKind::Person(x) => V::Str(x.clone()),
            ExprKind::Team(w) => V::Str(w.text.clone()),
            ExprKind::None => V::None,
            ExprKind::Bool(b) => V::Bool(*b),
            ExprKind::Path(p) => match (p[0].text.as_str(), p.get(1).map(|w| w.text.as_str())) {
                (name, None) if s.field(name).is_some() => {
                    let f = s.field(name);
                    t.value(f.map(|f| f.name.ascii()).unwrap_or(name)).map(|v| of_field(f, v)).unwrap_or(V::None)
                }
                ("requester", None) | ("requester", Some("id")) => V::Str(t.requester.clone()),
                ("requester", Some(kind)) => {
                    let v = self.part.v();
                    let team = v.primary(&t.requester).and_then(|m| v.up(&m.team).into_iter().find(|x| x.kind == kind).map(|x| x.id.clone()));
                    team.map(V::Str).unwrap_or(V::None)
                }
                ("ticket", Some("id")) => V::Str(t.id.clone()),
                ("ticket", Some("round")) => V::Num(Rat::int(i128::from(t.round))),
                ("ticket", Some("attempt")) => V::Str(t.attempt.clone()),
                (alias, Some(out)) if s.rule_facts(alias).is_some() => {
                    let got = self.m.fs(|| super::eval::rule_output(s, self.m.suite, alias, out, &t.form)).map_err(|e| self.m.unrouted(e).text)?;
                    got.map(|v| of_field(None, &v)).unwrap_or(V::None)
                }
                _ => return Err(no()),
            },
            ExprKind::Sum(p) if p.len() == 2 => {
                let rows = s.field(&p[0].text).ok_or_else(no)?;
                let col = rows.children.iter().find(|c| c.name.text == p[1].text).ok_or_else(no)?;
                let mut sum = Rat::int(0);
                if let Some(Value::List(list)) = t.value(rows.name.ascii()) {
                    for row in list {
                        if let Some((_, v)) = row.iter().find(|(n, _)| n == col.name.ascii()) {
                            sum = sum.add(num(of_field(Some(col), v))?);
                        }
                    }
                }
                V::Num(sum)
            }
            ExprKind::Lookup(path, by) if path.len() == 3 => {
                let key = match t.value(s.field(&by.text).map(|f| f.name.ascii()).unwrap_or(&by.text)) {
                    Some(Value::Str(k) | Value::Enum(k)) => k.clone(),
                    _ => return Ok(V::None),
                };
                let row = self.part.row(&format!("{}.{}", path[0].text, path[1].text), &key);
                row.and_then(|r| r.values.iter().find(|(n, _)| *n == path[2].text)).map(|(_, x)| V::Str(x.clone())).unwrap_or(V::None)
            }
            ExprKind::Neg(x) => V::Num(Rat::int(0).sub(num(self.eval(x)?)?)),
            ExprKind::Bin(op, a, b) => {
                let (x, y) = (num(self.eval(a)?)?, num(self.eval(b)?)?);
                V::Num(match op {
                    BinOp::Add => x.add(y),
                    BinOp::Sub => x.sub(y),
                    BinOp::Mul => x.mul(y),
                    BinOp::Div if y.num == 0 => return Err(no()),
                    BinOp::Div => x.div(y),
                })
            }
            ExprKind::Cmp(op, a, b) => {
                let (x, y) = (self.beside(a, b)?, self.beside(b, a)?);
                V::Bool(match op {
                    CmpOp::Is => x.same(&y),
                    CmpOp::IsNot => !x.same(&y),
                    CmpOp::Lt => x.order(&y) == Some(Ordering::Less),
                    CmpOp::Le => matches!(x.order(&y), Some(Ordering::Less | Ordering::Equal)),
                    CmpOp::Gt => x.order(&y) == Some(Ordering::Greater),
                    CmpOp::Ge => matches!(x.order(&y), Some(Ordering::Greater | Ordering::Equal)),
                })
            }
            ExprKind::And(a, b) => V::Bool(truth(self.eval(a)?)? && truth(self.eval(b)?)?),
            ExprKind::Or(a, b) => V::Bool(truth(self.eval(a)?)? || truth(self.eval(b)?)?),
            ExprKind::Not(a) => V::Bool(!truth(self.eval(a)?)?),
            _ => return Err(no()),
        })
    }

    /// An argument as it is given: a field's value as the form holds it, anything else as it comes
    /// to (a number that is whole, a date, a text).
    fn arg(&self, e: &Expr) -> Result<Value, Text> {
        if let Some(f) = self.field(e) {
            return Ok(self.t.value(f.name.ascii()).cloned().unwrap_or(Value::None));
        }
        Ok(match self.eval(e)? {
            V::Num(r) if r.is_int() => Value::Int(r.num),
            V::Num(_) => {
                let w = crate::types::written(self.m.s.src, e.span);
                return Err(tr!("`{w}` が整数になりません", "`{w}` does not come to a whole number"));
            }
            V::Day(d) => Value::Date(crate::lex::date_text(d)),
            V::Str(s) => Value::Str(s),
            V::Bool(b) => Value::Bool(b),
            V::None => Value::None,
        })
    }
}

impl Reads<'_, '_, '_> {
    /// The arguments of a `hold`, a `post` or a `void` as the book's transfer takes them (chobo's
    /// call): an amount as the whole number the book keeps in its unit (`unit USD scale 2` counts
    /// cents), anything else as a text (`round: ticket.round` is `"1"`). A `hold` gives every
    /// parameter of the transfer, a `post` and a `void` the ones of its key. Where chobo is not
    /// joined, the arguments are given as they are written.
    fn book_args(&self, book: &str, transfer: &str, hold: bool, call: &crate::ast::Call) -> Result<Values, Text> {
        let Some((_, facts)) = self.m.books.iter().find(|(a, _)| a == book) else {
            return call.args.iter().map(|a| Ok((a.name.text.clone(), self.arg(&a.value)?))).collect();
        };
        let Some(tr) = facts.transfers.iter().find(|t| t.name == transfer) else {
            return Err(tr!("帳簿 `{book}` に振替 `{transfer}` がありません", "the book `{book}` has no transfer `{transfer}`"));
        };
        let mut out = Values::new();
        for a in &call.args {
            let (n, w) = (&a.name.text, crate::types::written(self.m.s.src, a.value.span));
            let Some(p) = tr.params.iter().find(|p| p.name == *n) else {
                return Err(tr!("振替 `{book}.{transfer}` は `{n}` を受け取りません", "the transfer `{book}.{transfer}` takes no `{n}`"));
            };
            let v = self.eval(&a.value)?;
            let none = || tr!("`{w}` が無いので、`{book}.{transfer}` の `{n}` に渡せません", "`{w}` is none, and `{n}` of `{book}.{transfer}` takes a value");
            out.push((n.clone(), match (&p.unit, v) {
                (_, V::None) => return Err(none()),
                (Some(u), V::Num(x)) => {
                    let scale = facts.units.iter().find(|b| b.name == *u).map(|b| b.scale).unwrap_or(0);
                    let kept = x.mul(Rat::int(10i128.pow(scale)));
                    if !kept.is_int() {
                        return Err(tr!("`{w}` は、帳簿 `{book}` の単位 `{u}` の刻みに合いません", "`{w}` is not a whole number of the unit `{u}` as the book `{book}` keeps it"));
                    }
                    Value::Int(kept.num)
                }
                (Some(u), _) => return Err(tr!("`{w}` は数ではないので、`{book}.{transfer}` の `{n}`（単位 `{u}`）に渡せません", "`{w}` is no number, and `{n}` of `{book}.{transfer}` is an amount of `{u}`")),
                (None, V::Str(x)) => Value::Str(x),
                (None, V::Num(x)) if x.is_int() => Value::Str(x.num.to_string()),
                (None, V::Num(x)) => Value::Str(crate::lex::decimal(x)),
                (None, V::Day(d)) => Value::Str(crate::lex::date_text(d)),
                (None, V::Bool(b)) => Value::Str(b.to_string()),
            }));
        }
        // what the call must give: every parameter to hold, the key to post and to void
        let wanted: Vec<&str> = if hold { tr.params.iter().map(|p| p.name.as_str()).collect() } else { tr.key.iter().map(String::as_str).collect() };
        if let Some(n) = wanted.iter().find(|n| !out.iter().any(|(k, _)| k == *n)) {
            return Err(tr!("`{book}.{transfer}` に `{n}` を渡していません", "`{book}.{transfer}` is not given `{n}`"));
        }
        Ok(out)
    }
}

impl Machine<'_, '_> {
    /// What the ticket does at `ending` (DESIGN 7.4): each `on` line of it whose condition holds,
    /// in the order written.
    pub(crate) fn on(&self, ending: Ending, t: &mut Ticket, part: &OrgPart, fx: &mut Effects) -> Result<(), Refusal> {
        let Some(tk) = &self.s.file.ticket else { return Ok(()) };
        let no = |text: Text| Refusal::new("unread", text);
        for l in &tk.lines {
            let TicketLine::On { endings, action, call, when, .. } = l else { continue };
            if !endings.iter().any(|(e, _)| *e == ending) {
                continue;
            }
            let first = call.target[0].text.clone();
            let transfer = call.target.get(1).map(|w| w.text.clone()).unwrap_or_default();
            // a post and a void are given only while the hold of this round is open: with none,
            // nothing is asked of the line, not even its arguments
            let open = t.holds.iter().position(|h| h.book == first && h.transfer == transfer && h.round == t.round);
            if matches!(action, ActionKind::Post | ActionKind::Void) && open.is_none() {
                continue;
            }
            let args: Values = {
                let r = Reads { m: self, t, part };
                if let Some(w) = when {
                    match r.eval(w).map_err(no)? {
                        V::Bool(true) => {}
                        _ => continue,
                    }
                }
                match action {
                    ActionKind::Launch => call.args.iter().map(|a| Ok((a.name.text.clone(), r.arg(&a.value)?))).collect::<Result<Values, Text>>().map_err(no)?,
                    _ => r.book_args(&first, &transfer, *action == ActionKind::Hold, call).map_err(|t| Refusal::new("book", t))?,
                }
            };
            match action {
                ActionKind::Launch => fx.put(t.round, What::Launch { flow: first, args }),
                ActionKind::Hold => {
                    t.holds.push(Held { book: first.clone(), transfer: transfer.clone(), round: t.round });
                    fx.put(t.round, What::Book { op: "hold", book: first, transfer, args });
                }
                ActionKind::Post | ActionKind::Void => {
                    let Some(k) = open else { continue };
                    t.holds.remove(k);
                    fx.put(t.round, What::Book { op: if *action == ActionKind::Post { "post" } else { "void" }, book: first, transfer, args });
                }
            }
        }
        Ok(())
    }
}
