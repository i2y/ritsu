//! A function of a module written as a value (`total_by(xs: lines, amount: line_total)`,
//! `lines.map(price_of)`, DESIGN 27.1), and a function value given to a builtin by its name
//! (`lines.map(f)`), read as the lambda that calls it once the check of types has passed them:
//!
//! ```text
//! price_of            (#1) => price_of(line: #1)
//! lines.map(f)        lines.map((#1) => f(#1))
//! ```
//!
//! written where the name is. The lambda's parameters are named by words no name of a module can be, so
//! that nothing it says of them meets a name of the function it is written in. The reference
//! interpreter, the analysis of ranges and the generated code read a lambda and nothing else, and a
//! builtin a lambda of its own: a failure in the call (an argument outside the range of the function's
//! parameter) is said at the line the name is written on, and its place is the place of the function
//! that wrote the name, as a lambda written there would be. A lambda given to a builtin is its own, a
//! call of nothing (DESIGN 27.3), so the call in it is one call deep, as the function value's call is.
//! The lambda takes the id of the name; the call, the name it calls and its arguments take ids after
//! the parser's, with their types and what their names name.

use crate::ast::*;
use crate::ty::Ty;
use crate::types::{NamedFn, Ref, Typed};
use std::collections::BTreeMap;

/// Every function written as a value in the module's functions, and every function value a builtin is
/// given by its name, read as the lambda that calls it.
pub fn desugar(m: &mut Module, typed: &mut Typed) {
    let named = typed.named.clone();
    let mut w = W { ids: m.exprs, typed, named: &named };
    for f in &mut m.fns {
        w.block(&mut f.body);
    }
    m.exprs = w.ids;
}

/// The name of the lambda's `i`th parameter, from 1: no word of a module is spelled with `#`.
pub fn param_word(i: usize) -> String {
    format!("#{i}")
}

struct W<'a> {
    ids: usize,
    typed: &'a mut Typed,
    named: &'a BTreeMap<usize, NamedFn>,
}

impl W<'_> {
    /// A new expression of the type `t`, what its name names `r`.
    fn mk(&mut self, span: Span, kind: ExprKind, t: Ty, r: Ref) -> Expr {
        let id = self.ids;
        self.ids += 1;
        self.typed.types.push(t);
        self.typed.refs.push(r);
        Expr { id, span, kind }
    }

    fn block(&mut self, b: &mut Block) {
        for l in &mut b.lets {
            self.expr(&mut l.value);
        }
        self.expr(&mut b.answer);
    }

    fn expr(&mut self, e: &mut Expr) {
        if let Some(n) = self.named.get(&e.id) {
            self.wrap(e, n);
            return;
        }
        match &mut e.kind {
            ExprKind::Num(_) | ExprKind::Date(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::None | ExprKind::Var(_) => {}
            ExprKind::Field(b, _, _) | ExprKind::Neg(b) | ExprKind::Not(b) | ExprKind::In(b, _, _) | ExprKind::Lambda(_, b) => self.expr(b),
            ExprKind::Call(c, args) => {
                self.expr(c);
                let builtin = matches!(self.typed.refs.get(c.id), Some(Ref::Builtin(_) | Ref::Method(_)));
                for a in args {
                    match a {
                        Arg::Named(_, _, x) | Arg::Pos(x) => {
                            // a function value a builtin is given by its name
                            if builtin
                                && !self.named.contains_key(&x.id)
                                && let ExprKind::Var(_) = x.kind
                                && let Some(Ty::Fn(ps, r, _)) = self.typed.types.get(x.id).cloned()
                            {
                                self.call_of(x, &ps, &r);
                            } else {
                                self.expr(x);
                            }
                        }
                    }
                }
            }
            ExprKind::Record(_, base, fs) => {
                if let Some(b) = base {
                    self.expr(b);
                }
                for f in fs {
                    self.expr(&mut f.value);
                }
            }
            ExprKind::List(xs) => {
                for x in xs {
                    self.expr(x);
                }
            }
            ExprKind::Dict(pairs) => {
                for (k, v) in pairs {
                    self.expr(k);
                    self.expr(v);
                }
            }
            ExprKind::Bin(a, _, b) => {
                self.expr(a);
                self.expr(b);
            }
            ExprKind::If(c, t, f) => {
                self.expr(c);
                self.block(t);
                self.block(f);
            }
            ExprKind::Match(s, arms) => {
                self.expr(s);
                for a in arms {
                    self.block(&mut a.body);
                }
            }
            ExprKind::Block(b) => self.block(b),
        }
    }

    /// The name `e` of a function value given to a builtin, made the builtin's own lambda that calls it:
    /// the lambda's type is its answer's, as a builtin's lambda's is its body's.
    fn call_of(&mut self, e: &mut Expr, ps: &[Ty], r: &Ty) {
        let at = e.span;
        let f = self.mk(at, e.kind.clone(), self.typed.types.get(e.id).cloned().unwrap_or(Ty::Error), Ref::Local);
        let mut params = Vec::new();
        let mut args = Vec::new();
        for (i, t) in ps.iter().enumerate() {
            let w = param_word(i + 1);
            params.push((w.clone(), at));
            args.push(Arg::Pos(self.mk(at, ExprKind::Var(w), t.clone(), Ref::Local)));
        }
        let call = self.mk(at, ExprKind::Call(Box::new(f), args), r.clone(), Ref::CallValue);
        e.kind = ExprKind::Lambda(params, Box::new(call));
        if let Some(t) = self.typed.types.get_mut(e.id) {
            *t = r.clone();
        }
        if let Some(x) = self.typed.refs.get_mut(e.id) {
            *x = Ref::None;
        }
    }

    /// The name `e` of a function, made the lambda that calls it with its parameters, by the names the
    /// function declares them by.
    fn wrap(&mut self, e: &mut Expr, n: &NamedFn) {
        let at = e.span;
        let callee_kind = match &e.kind {
            ExprKind::Field(base, f, sp) => {
                let ExprKind::Var(u) = &base.kind else { return };
                let b = self.mk(base.span, ExprKind::Var(u.clone()), Ty::Error, Ref::None);
                ExprKind::Field(Box::new(b), f.clone(), *sp)
            }
            other => other.clone(),
        };
        let callee = self.mk(at, callee_kind, Ty::Error, Ref::Fn(n.file.clone(), n.name.clone()));
        let mut params = Vec::new();
        let mut args = Vec::new();
        for (i, (p, t)) in n.pnames.iter().zip(&n.params).enumerate() {
            let w = param_word(i + 1);
            params.push((w.clone(), at));
            let x = self.mk(at, ExprKind::Var(w), t.clone(), Ref::Local);
            args.push(Arg::Named(p.clone(), at, x));
        }
        let call = self.mk(at, ExprKind::Call(Box::new(callee), args), n.ret.clone(), Ref::None);
        if !n.inst.is_empty() {
            self.typed.inst.insert(call.id, n.inst.clone());
        }
        e.kind = ExprKind::Lambda(params, Box::new(call));
        if let Some(r) = self.typed.refs.get_mut(e.id) {
            *r = Ref::None;
        }
    }
}
