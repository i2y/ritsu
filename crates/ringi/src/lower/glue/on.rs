//! The `on` lines in a request's glue (DESIGN 7.4, 12.1): what a ticket does as it is submitted and
//! as it ends, as the machine reads it (`machine.OnLine`): each line's action, its book and transfer
//! (or the flow it launches), the condition it is done on (`when_<n>`) and its arguments (`args_<n>`),
//! each a function of the ticket (`machine.Ref`), the part of the organization and the form, so that
//! the machine asks for a condition and for the arguments only of a line it does, as the reference
//! does ([`crate::engine`]'s `on`).
//!
//! A condition and an argument are read as the reference reads them: a field's value as the form
//! holds it, `ticket.id`, `ticket.round`, `requester.id` and the team of a kind above the requester
//! (`requester.department`), a constant beside a field in the field's unit and a word beside a
//! choice as its value. An argument of a book's transfer is given as the book takes it (an amount
//! as the whole number it keeps in its unit, anything else as a text), and is refused (`book`) where
//! it is none; an argument of a flow is given as the form holds it, and as nothing where it is none.

use super::form::{FormOf, Fld, Vk, arg_of};
use super::{ending_word, not_yet, quoted};
use crate::ast::{ActionKind, Call, CmpOp, Ending, Expr, ExprKind, TicketLine, TypeExpr};
use crate::engine::Machine;
use crate::lower::words;
use ritsu_base::text::Text;
use ritsu_units::Rat;

/// A value of an `on` line as the glue reads it: an expression of kaname, what it is, and whether
/// it may be none.
struct Got<'f> {
    expr: String,
    is: Is<'f>,
    opt: bool,
    /// The expression as written, which a refusal says.
    written: String,
}

enum Is<'f> {
    /// A field of the form, as the form holds it.
    Field(&'f Fld),
    Str,
    /// A number, with the least it moves by.
    Num(Rat),
    Day,
    Bool,
}

struct On<'x, 's, 'a, 'f> {
    m: &'x Machine<'s, 'a>,
    form: &'f FormOf<'x, 's, 'a>,
}

impl<'f> On<'_, '_, '_, 'f> {
    fn written(&self, e: &Expr) -> String {
        crate::types::written(self.m.s.src, e.span)
    }

    fn field(&self, e: &Expr) -> Option<(&'f Fld, &crate::ast::Field)> {
        match &e.kind {
            ExprKind::Path(p) if p.len() == 1 => {
                let f = self.m.s.field(&p[0].text)?;
                Some((self.form.fld(&p[0].text)?, f))
            }
            _ => None,
        }
    }

    /// A value an `on` line reads: a field, the ticket's ID, round and try, the requester and the team of
    /// a kind above them, a constant. None for what the glue does not read yet.
    fn value(&self, e: &Expr) -> Option<Got<'f>> {
        let written = self.written(e);
        let got = |expr: String, is: Is<'f>, opt: bool| Some(Got { expr, is, opt, written: written.clone() });
        if let Some((x, _)) = self.field(e) {
            return got(format!("form.{}", x.name), Is::Field(x), !x.required);
        }
        match &e.kind {
            ExprKind::Path(p) => match (p[0].text.as_str(), p.get(1).map(|w| w.text.as_str()), p.len()) {
                ("requester", None, 1) | ("requester", Some("id"), 2) => got("r.requester".into(), Is::Str, false),
                ("requester", Some(kind), 2) => got(format!("machine.kind_above(org: org, person: r.requester, kind: {})", quoted(self.m.names.team_kind(kind))), Is::Str, true),
                ("ticket", Some("id"), 2) => got("r.id".into(), Is::Str, false),
                ("ticket", Some("round"), 2) => got("r.round".into(), Is::Num(Rat::int(1)), false),
                ("ticket", Some("attempt"), 2) => got("r.attempt".into(), Is::Str, false),
                _ => None,
            },
            ExprKind::Str(x) | ExprKind::Person(x) => got(quoted(x), Is::Str, false),
            ExprKind::Team(w) => got(quoted(&w.text), Is::Str, false),
            ExprKind::Bool(b) => got(b.to_string(), Is::Bool, false),
            ExprKind::Date(d) => got(crate::lex::date_text(*d), Is::Day, false),
            ExprKind::Num(_) | ExprKind::Neg(_) => {
                let v = words::constant(e)?.rat();
                got(words::written(words::Val::Num(v), &TypeExpr::Num(plain())), Is::Num(v), false)
            }
            _ => None,
        }
    }

    /// The condition of a line, as a truth of kaname over `r`, `org` and `form`.
    fn cond(&self, e: &Expr) -> Result<String, Text> {
        let no = || not_yet(format!("the condition `{}` of an `on` line", self.written(e)));
        Ok(match &e.kind {
            ExprKind::And(a, b) => format!("({} and {})", self.cond(a)?, self.cond(b)?),
            ExprKind::Or(a, b) => format!("({} or {})", self.cond(a)?, self.cond(b)?),
            ExprKind::Not(a) => format!("not ({})", self.cond(a)?),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Path(_) => match self.field(e) {
                Some((x, _)) if x.vk == Vk::Bool && x.required => format!("form.{}", x.name),
                _ => return Err(no()),
            },
            ExprKind::Cmp(op, a, b) => {
                let (is, not) = match op {
                    CmpOp::Is => (true, false),
                    CmpOp::IsNot => (true, true),
                    _ => (false, false),
                };
                // `x is none`, `x is not none`
                if is && (matches!(a.kind, ExprKind::None) || matches!(b.kind, ExprKind::None)) {
                    let other = if matches!(a.kind, ExprKind::None) { b } else { a };
                    let v = self.value(other).ok_or_else(no)?;
                    let unit = match &v.is {
                        Is::Field(x) => x.unit.as_ref(),
                        _ => None,
                    };
                    let present = if v.opt { super::form::present(unit, &v.expr) } else { "true".to_string() };
                    return Ok(if not { present } else { format!("not ({present})") });
                }
                let (fa, fb) = (self.field(a), self.field(b));
                let (field, other, flipped) = match (fa, fb) {
                    (Some(f), None) => (f, b, false),
                    (None, Some(f)) => (f, a, true),
                    _ => {
                        // two values that are no field: the requester's ID beside a text
                        let (x, y) = (self.value(a).ok_or_else(no)?, self.value(b).ok_or_else(no)?);
                        if !is || x.opt || y.opt || !matches!(x.is, Is::Str) || !matches!(y.is, Is::Str) {
                            return Err(no());
                        }
                        let eq = format!("{} = {}", x.expr, y.expr);
                        return Ok(if not { format!("not ({eq})") } else { eq });
                    }
                };
                let (x, f) = field;
                if !x.required {
                    return Err(no());
                }
                // the other side beside the field: a word as the choice's value, a constant in its unit
                let o = match (&other.kind, x.vk) {
                    (ExprKind::Path(p), Vk::Str) if p.len() == 1 && self.m.s.field(&p[0].text).is_none() => quoted(&words::choice_ascii(f, &p[0].text)),
                    (ExprKind::Str(s) | ExprKind::Person(s), Vk::Str) => quoted(s),
                    (ExprKind::Team(w), Vk::Str) => quoted(&w.text),
                    (ExprKind::Num(_) | ExprKind::Neg(_), Vk::Num) => words::written(words::constant_in(other, &f.ty).ok_or_else(no)?, &f.ty),
                    (ExprKind::Date(d), Vk::Date) => crate::lex::date_text(*d),
                    (ExprKind::Bool(b), Vk::Bool) => b.to_string(),
                    _ => return Err(no()),
                };
                let v = format!("form.{}", x.name);
                let ordered = matches!(x.vk, Vk::Num | Vk::Date);
                let sign = match (op, flipped) {
                    (CmpOp::Is, _) => "=",
                    (CmpOp::IsNot, _) => "!=",
                    (_, _) if !ordered => return Err(no()),
                    (CmpOp::Lt, false) | (CmpOp::Gt, true) => "<",
                    (CmpOp::Le, false) | (CmpOp::Ge, true) => "<=",
                    (CmpOp::Gt, false) | (CmpOp::Lt, true) => ">",
                    (CmpOp::Ge, false) | (CmpOp::Le, true) => ">=",
                };
                if sign == "!=" { format!("not ({v} = {o})") } else { format!("{v} {sign} {o}") }
            }
            _ => return Err(no()),
        })
    }

    /// `when_<k>`: whether the line is done.
    fn when_fn(&self, k: usize, e: &Expr) -> Result<String, Text> {
        let c = self.cond(e)?;
        Ok(format!(
            "fn when_{k}(r: machine.Ref, org: machine.Org, form: Form) -> result[bool, machine.Refusal] total\n  description {}\n  match {c}\n    true => ok(true)\n    false => ok(false)\n\n",
            quoted(&format!("Whether the `on` line is done: `{}`", self.written(e)))
        ))
    }

    /// `args_<k>` of a line that holds, posts or voids a book's transfer: its arguments as the
    /// transfer takes them, in the order written. A value that is none is refused, the first first.
    fn book_args(&self, k: usize, book: &str, transfer: &str, hold: bool, call: &Call) -> Result<String, Text> {
        let shown = format!("{book}.{transfer}");
        let Some((_, facts)) = self.m.books.iter().find(|(a, _)| a == book) else { return Err(not_yet(format!("the arguments of `{shown}`, a book chobo does not answer for"))) };
        let Some(tr) = facts.transfers.iter().find(|t| t.name == transfer) else { return Err(not_yet(format!("the arguments of `{shown}`, a transfer the book does not have"))) };
        let mut opens = String::new();
        let mut depth = 2;
        let mut args = Vec::new();
        for (i, a) in call.args.iter().enumerate() {
            let n = &a.name.text;
            let Some(p) = tr.params.iter().find(|p| p.name == *n) else { return Err(not_yet(format!("the argument `{n}` of `{shown}`, which the transfer does not take"))) };
            let v = self.value(&a.value).ok_or_else(|| not_yet(format!("the argument `{n}: {}` of `{shown}`", self.written(&a.value))))?;
            let mut expr = v.expr.clone();
            if v.opt {
                let x = format!("x{i}");
                let p_ = " ".repeat(depth);
                let said = format!("`{}` is none, and `{n}` of `{shown}` takes a value", v.written);
                let said_ja = format!("`{}` が無いので、`{shown}` の `{n}` に渡せません", v.written);
                opens.push_str(&format!("{p_}match {expr}\n{p_}  none => err(machine.refusal(kind: machine.Kind.book, en: {}, ja: {}))\n{p_}  some {x} =>\n", quoted(&said), quoted(&said_ja)));
                depth += 4;
                expr = x;
            }
            let arg = match (&p.unit, &v.is) {
                (Some(u), Is::Field(x)) if x.vk == Vk::Num && !x.is_rate() => {
                    let scale = facts.units.iter().find(|b| b.name == *u).map(|b| b.scale).unwrap_or(0);
                    let times = Rat::int(10i128.pow(scale));
                    if !x.step().mul(times).is_int() {
                        return Err(not_yet(format!("the argument `{n}` of `{shown}`, which may not be a whole number of `{u}`")));
                    }
                    format!("machine.arg_num(name: {}, v: {})", quoted(n), scaled(&x.in_unit(&expr), scale))
                }
                (Some(u), Is::Num(step)) => {
                    let scale = facts.units.iter().find(|b| b.name == *u).map(|b| b.scale).unwrap_or(0);
                    if !step.mul(Rat::int(10i128.pow(scale))).is_int() {
                        return Err(not_yet(format!("the argument `{n}` of `{shown}`, which may not be a whole number of `{u}`")));
                    }
                    format!("machine.arg_num(name: {}, v: {})", quoted(n), scaled(&expr, scale))
                }
                (Some(_), _) => return Err(not_yet(format!("the argument `{n}` of `{shown}`, an amount given no number"))),
                (None, Is::Field(x)) => match x.vk {
                    Vk::Str => format!("machine.arg_str(name: {}, v: {expr})", quoted(n)),
                    Vk::Num if !x.is_rate() => format!("machine.arg_str(name: {}, v: text({}))", quoted(n), x.in_unit(&expr)),
                    Vk::Date | Vk::Bool => format!("machine.arg_str(name: {}, v: text({expr}))", quoted(n)),
                    _ => return Err(not_yet(format!("the argument `{n}` of `{shown}`"))),
                },
                (None, Is::Str) => format!("machine.arg_str(name: {}, v: {expr})", quoted(n)),
                (None, Is::Num(_) | Is::Day | Is::Bool) => format!("machine.arg_str(name: {}, v: text({expr}))", quoted(n)),
            };
            args.push(arg);
        }
        // what the call must give: every parameter to hold, the key to post and to void
        let wanted: Vec<&str> = if hold { tr.params.iter().map(|p| p.name.as_str()).collect() } else { tr.key.iter().map(String::as_str).collect() };
        let p_ = " ".repeat(depth);
        let answer = match wanted.iter().find(|w| !call.args.iter().any(|a| a.name.text == **w)) {
            Some(w) => format!("err(machine.refusal(kind: machine.Kind.book, en: {}, ja: {}))", quoted(&format!("`{shown}` is not given `{w}`")), quoted(&format!("`{shown}` に `{w}` が渡されていません"))),
            None => format!("ok([{}])", args.join(", ")),
        };
        Ok(format!(
            "fn args_{k}(r: machine.Ref, form: Form, org: machine.Org) -> result[list[machine.Arg], machine.Refusal] total\n  description {}\n{opens}{p_}{answer}\n\n",
            quoted(&format!("The arguments of `{shown}`"))
        ))
    }

    /// `args_<k>` of a line that launches a flow: each argument as the form holds it, and as nothing
    /// where it is none.
    fn launch_args(&self, k: usize, flow: &str, call: &Call) -> Result<String, Text> {
        let mut args = Vec::new();
        for a in &call.args {
            let n = &a.name.text;
            let v = self.value(&a.value).ok_or_else(|| not_yet(format!("the argument `{n}: {}` of the flow `{flow}`", self.written(&a.value))))?;
            let nothing = format!("machine.Arg {{ name: {}, value: machine.Val.nothing }}", quoted(n));
            let arg = match &v.is {
                Is::Field(x) => {
                    let one = |w: &str| {
                        let named = arg_of(x, w);
                        // the argument by the name the line gives it
                        named.replacen(&format!("name: {}", quoted(&x.name)), &format!("name: {}", quoted(n)), 1)
                    };
                    if v.opt {
                        format!("match {}\n      none => {nothing}\n      some x => {}", v.expr, one("x"))
                    } else {
                        one(&v.expr)
                    }
                }
                Is::Str if v.opt => format!("machine.arg_maybe_str(name: {}, v: {})", quoted(n), v.expr),
                Is::Str => format!("machine.arg_str(name: {}, v: {})", quoted(n), v.expr),
                Is::Num(step) if step.is_int() => format!("machine.arg_num(name: {}, v: {})", quoted(n), v.expr),
                Is::Num(_) => return Err(not_yet(format!("the argument `{n}` of the flow `{flow}`, which may not be a whole number"))),
                Is::Day => format!("machine.arg_str(name: {}, v: text({}))", quoted(n), v.expr),
                Is::Bool => format!("machine.arg_truth(name: {}, v: {})", quoted(n), v.expr),
            };
            args.push(arg);
        }
        // each argument a value of its own, one that may be none split first
        let mut body = String::new();
        let mut names = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let l = format!("a{i}");
            if a.starts_with("match ") {
                body.push_str(&format!("  let {l} : machine.Arg =\n    {a}\n"));
            } else {
                body.push_str(&format!("  let {l} = {a}\n"));
            }
            names.push(l);
        }
        body.push_str(&format!("  ok([{}])\n", names.join(", ")));
        Ok(format!(
            "fn args_{k}(r: machine.Ref, form: Form, org: machine.Org) -> result[list[machine.Arg], machine.Refusal] total\n  description {}\n{body}\n",
            quoted(&format!("The arguments of the flow `{flow}`"))
        ))
    }
}

/// A number of no unit, as a constant of an `on` line is read where no field is beside it.
fn plain() -> ritsu_units::Unit {
    ritsu_units::Unit { dim: ritsu_units::Dim::Number, unit: String::new(), tax: None, step: None }
}

/// A number times 10 to the `scale`: an amount as the whole number a book keeps in its unit.
fn scaled(v: &str, scale: u32) -> String {
    if scale == 0 { v.to_string() } else { format!("{v} * {}", words::grouped(10i128.pow(scale))) }
}

/// Every ending, in the order the machine names them.
const ENDINGS: [Ending; 6] = [Ending::Submitted, Ending::Approved, Ending::SentBack, Ending::Rejected, Ending::Withdrawn, Ending::Expired];

/// The `on` lines of the request: `when_<k>` and `args_<k>` of each, and `on_lines`, which gives the
/// machine the lines of an ending in the order written.
pub(super) fn lines(m: &Machine, form: &FormOf) -> Result<String, Text> {
    let on = On { m, form };
    let mut out = String::new();
    let mut made: Vec<(Vec<Ending>, String)> = Vec::new();
    let mut k = 0;
    for l in m.s.file.ticket.iter().flat_map(|t| &t.lines) {
        let TicketLine::On { endings, action, call, when, .. } = l else { continue };
        k += 1;
        let first = call.target[0].text.clone();
        let transfer = call.target.get(1).map(|w| w.text.clone()).unwrap_or_default();
        let when_call = match when {
            None => "ok(true)".to_string(),
            Some(w) => {
                out.push_str(&on.when_fn(k, w)?);
                format!("when_{k}(r: r, org: org, form: form)")
            }
        };
        let action_word = match action {
            ActionKind::Hold => "hold",
            ActionKind::Post => "post",
            ActionKind::Void => "release",
            ActionKind::Launch => "launch",
        };
        out.push_str(&match action {
            ActionKind::Launch => on.launch_args(k, &first, call)?,
            _ => on.book_args(k, &first, &transfer, *action == ActionKind::Hold, call)?,
        });
        let line = format!(
            "machine.OnLine {{ action: machine.Action.{action_word}, book: {}, transfer: {}, when: {when_call}, args: args_{k}(r: r, form: form, org: org) }}",
            quoted(m.names.used(&first)),
            quoted(&transfer)
        );
        made.push((endings.iter().map(|(e, _)| *e).collect(), line));
    }
    // the lines of each ending, the endings of the same lines in one arm
    let mut arms: Vec<(Vec<Ending>, Vec<String>)> = Vec::new();
    for e in ENDINGS {
        let lines: Vec<String> = made.iter().filter(|(es, _)| es.contains(&e)).map(|(_, l)| l.clone()).collect();
        match arms.iter_mut().find(|(_, ls)| *ls == lines) {
            Some((es, _)) => es.push(e),
            None => arms.push((vec![e], lines)),
        }
    }
    let list = |ls: &[String]| format!("[{}]", ls.join(", "));
    let body = if arms.len() == 1 {
        format!("  {}\n", list(&arms[0].1))
    } else {
        let mut b = String::from("  match e\n");
        let mut rest = false;
        for (es, ls) in &arms {
            if ls.is_empty() {
                rest = true;
                continue;
            }
            let words: Vec<&str> = es.iter().map(|e| ending_word(*e)).collect();
            b.push_str(&format!("    {} => {}\n", words.join(", "), list(ls)));
        }
        if rest {
            b.push_str("    else => []\n");
        }
        b
    };
    out.push_str(&format!(
        "fn on_lines(e: machine.Ending, r: machine.Ref, org: machine.Org, form: Form) -> list[machine.OnLine] total\n  description \"What the ticket does at an ending: its `on` lines of it, in the order written\"\n{body}\n"
    ));
    Ok(out)
}
