//! `let x = try e` read as the `match` it means (DESIGN 26.3), once the check of types has passed it:
//! the analysis of ranges, the reference interpreter and the generated code read a `match` and
//! nothing else.
//!
//! In a block, `let x = try e` and the lines after it are
//!
//! ```text
//! match e                          match e
//!   err try => err(try)              none => none
//!   ok x => <the lines after it>     some x => <the lines after it>
//! ```
//!
//! for a `result` and for a value that may be none. The `match` is written where the `let` is (its
//! line and column), so that what is said of it, and a failure in it, is said at the line written;
//! `try` names the `err`, a word no name of a module can be (E002). The new expressions take ids after
//! the parser's, with their types and what their names name.

use crate::ast::*;
use crate::ty::Ty;
use crate::types::{Ref, Typed};

/// Every `let … = try …` of the module's functions, read as its `match`. `rets` is each function's
/// answer type, in the order of the module's functions.
pub fn desugar(m: &mut Module, typed: &mut Typed, rets: &[Ty]) {
    let mut ids = m.exprs;
    for (f, ret) in m.fns.iter_mut().zip(rets) {
        let mut w = W { ids: &mut ids, typed, ret };
        w.block(&mut f.body);
    }
    m.exprs = ids;
}

struct W<'a> {
    ids: &'a mut usize,
    typed: &'a mut Typed,
    ret: &'a Ty,
}

impl W<'_> {
    /// A new expression of the type `t`, what its name names `r`.
    fn mk(&mut self, span: Span, kind: ExprKind, t: Ty, r: Ref) -> Expr {
        let id = *self.ids;
        *self.ids += 1;
        self.typed.types.push(t);
        self.typed.refs.push(r);
        Expr { id, span, kind }
    }

    fn block(&mut self, b: &mut Block) {
        for l in &mut b.lets {
            self.expr(&mut l.value);
        }
        self.expr(&mut b.answer);
        let Some(i) = b.lets.iter().position(|l| l.tried.is_some()) else { return };
        let rest_lets = b.lets.split_off(i + 1);
        let l = b.lets.pop().expect("the `let` with `try`");
        let answer_ty = self.typed.types.get(b.answer.id).cloned().unwrap_or(Ty::Error);
        let placeholder = Expr { id: usize::MAX, span: b.answer.span, kind: ExprKind::None };
        let answer = std::mem::replace(&mut b.answer, placeholder);
        let mut rest = Block { lets: rest_lets, answer };
        // the lines after it may have `try` too
        self.block(&mut rest);
        let at = l.tried.unwrap_or(l.span);
        let span = Span { line: l.span.line, col: at.col, end: l.value.span.end.max(at.end) };
        let value_ty = self.typed.types.get(l.value.id).cloned().unwrap_or(Ty::Error);
        let ret = self.ret.clone();
        let arms = match value_ty {
            Ty::Result(_, e) => {
                let err_var = self.mk(at, ExprKind::Var("try".into()), (*e).clone(), Ref::Local);
                let callee = self.mk(at, ExprKind::Var("err".into()), Ty::Error, Ref::Builtin("err"));
                let call = self.mk(at, ExprKind::Call(Box::new(callee), vec![Arg::Pos(err_var)]), ret.clone(), Ref::None);
                vec![
                    Arm { patterns: vec![Pattern::Value(vec![("err".into(), at)], Some(("try".into(), at)), at)], body: Block { lets: vec![], answer: call }, span: at },
                    Arm { patterns: vec![Pattern::Value(vec![("ok".into(), at)], Some((l.name.clone(), l.span)), at)], body: rest, span: at },
                ]
            }
            _ => {
                let none = self.mk(at, ExprKind::None, ret.clone(), Ref::None);
                vec![
                    Arm { patterns: vec![Pattern::None(at)], body: Block { lets: vec![], answer: none }, span: at },
                    Arm { patterns: vec![Pattern::Some(l.name.clone(), l.span)], body: rest, span: at },
                ]
            }
        };
        b.answer = self.mk(span, ExprKind::Match(Box::new(l.value), arms), answer_ty, Ref::None);
    }

    fn expr(&mut self, e: &mut Expr) {
        match &mut e.kind {
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
            // no other expression holds a block with `let` lines
            _ => {}
        }
    }
}
