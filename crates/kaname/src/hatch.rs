//! The warning of a decision written in code (DESIGN 7, W501): a choice a rulec table should hold,
//! made in an `if` or a `match` of the glue code instead, where no check of completeness or overlap
//! reaches it and no page shows it to the person who approves it.
//!
//! Two shapes are warned of:
//!
//! 1. An `if` or a `match` that answers a constant of the business in one of its branches: an
//!    amount, a quantity, a rate, a date, a value of an enum, `true` or `false`, a record of only
//!    those; and in a `match`, a number too. An `if` that answers numbers is left alone: the end of
//!    a recursion and the case of an empty list answer them.
//! 2. A comparison of a value with a literal of the business that is not 0 (an amount, a quantity,
//!    a rate, a date, a number), made a value anywhere but in the condition of an `if` or the value
//!    a `match` splits: in a `let`, an argument, an answer, a condition given to a builtin.
//!
//! What is not warned of: a branch that answers `none`, a `match` that only splits `none` from
//! `some x`, a `match` that maps each value of one enum to the value of another of the same name, a
//! comparison with 0 made a value. A branch that narrows a value to the range of the rule it calls
//! answers the rule's outputs and `none`, which is no constant: it is not warned of.
//!
//! A chain of `else if` is one decision, warned of once, at its first `if`.

use crate::ast::*;
use crate::check::Checked;
use crate::diag::Diag;
use crate::ty::Ty;
use crate::types::Ref;
use ritsu_base::text::Text;
use std::collections::BTreeSet;

/// W501 for every decision written in the module's functions.
pub fn check(c: &Checked) -> Vec<Diag> {
    let mut h = Hatch { c, out: Vec::new(), chained: BTreeSet::new() };
    for f in &c.module.fns {
        h.block(&f.body);
    }
    h.out
}

struct Hatch<'a> {
    c: &'a Checked,
    out: Vec<Diag>,
    /// The `if`s that are an `else if` of a chain warned of (or not) at its first.
    chained: BTreeSet<usize>,
}

/// What a branch answers, as the warning reads it.
enum Answer {
    /// A constant of the business, as written.
    Constant(String),
    /// A call of a rule or a date: the decision is the table's.
    Rule,
    /// Anything else: `none`, a value computed, an output of a rule.
    Other,
}

impl Hatch<'_> {
    fn ty(&self, e: &Expr) -> &Ty {
        self.c.typed.types.get(e.id).unwrap_or(&Ty::Error)
    }

    fn rf(&self, e: &Expr) -> &Ref {
        self.c.typed.refs.get(e.id).unwrap_or(&Ref::None)
    }

    fn src(&self, sp: Span) -> String {
        let line = self.c.src.lines().nth(sp.line.saturating_sub(1)).unwrap_or("");
        let cs: Vec<char> = line.chars().collect();
        let (a, b) = (sp.col.saturating_sub(1).min(cs.len()), sp.end.saturating_sub(1).min(cs.len()));
        cs[a..b.max(a)].iter().collect::<String>().trim().to_string()
    }

    fn block(&mut self, b: &Block) {
        for l in &b.lets {
            self.expr(&l.value, false);
        }
        self.expr(&b.answer, false);
    }

    /// `cond`: the expression is (through `and`, `or` and `not`) the condition of an `if` or the value
    /// a `match` splits.
    fn expr(&mut self, e: &Expr, cond: bool) {
        match &e.kind {
            ExprKind::If(c, t, f) => {
                // the chain is marked before its `else if`s are reached
                if !self.chained.contains(&e.id) {
                    self.decision_if(e);
                }
                self.expr(c, true);
                self.block(t);
                self.block(f);
            }
            ExprKind::Match(s, arms) => {
                self.expr(s, true);
                for a in arms {
                    self.block(&a.body);
                }
                self.decision_match(e, s, arms);
            }
            ExprKind::Bin(a, BinOp::And | BinOp::Or, b) => {
                self.expr(a, cond);
                self.expr(b, cond);
            }
            ExprKind::Not(x) => self.expr(x, cond),
            ExprKind::Bin(a, op, b) => {
                if op.is_comparison() && !cond {
                    self.threshold(e, a, b);
                }
                self.expr(a, false);
                self.expr(b, false);
            }
            ExprKind::Call(callee, args) => {
                self.expr(callee, false);
                for x in args {
                    self.expr(x.expr(), false);
                }
            }
            ExprKind::Lambda(_, body) => self.expr(body, false),
            ExprKind::Field(b, _, _) | ExprKind::Neg(b) | ExprKind::In(b, _, _) => self.expr(b, false),
            ExprKind::Record(_, fs) => {
                for f in fs {
                    self.expr(&f.value, false);
                }
            }
            ExprKind::List(xs) => {
                for x in xs {
                    self.expr(x, false);
                }
            }
            ExprKind::Block(b) => self.block(b),
            _ => {}
        }
    }

    /// A literal of the business as written (an amount, a quantity, a rate, a number, a date), with
    /// whether it is 0.
    fn literal(&self, e: &Expr) -> Option<(String, bool)> {
        match &e.kind {
            ExprKind::Num(n) => Some((self.src(e.span), n.value.num == 0)),
            ExprKind::Date(_) => Some((self.src(e.span), false)),
            ExprKind::Neg(x) => self.literal(x).map(|(_, z)| (self.src(e.span), z)),
            _ => None,
        }
    }

    /// The second shape: a comparison of a value with a literal of the business, made a value.
    fn threshold(&mut self, e: &Expr, a: &Expr, b: &Expr) {
        let lit = match (self.literal(a), self.literal(b)) {
            (Some(_), Some(_)) | (None, None) => return,
            (Some(l), None) | (None, Some(l)) => l,
        };
        if lit.1 || !matches!(self.ty(a), Ty::Num(_) | Ty::Date) && !matches!(self.ty(b), Ty::Num(_) | Ty::Date) {
            return;
        }
        let shown = self.src(e.span);
        let c = self.c;
        let d = Diag::at("W501", &c.path, e.span.line, e.span.col, tr!(
            "`{shown}` は、業務のしきい値との比べを値にしています。表に書く判断です",
            "`{shown}` makes a value of a comparison with a threshold of the business: a decision, which belongs in a table"
        ))
        .source(&c.src)
        .note(tr!(
            "しきい値は rulec の表に書き、比べた答えを規則の出力として受け取ってください。表に書けば、完全性と重なりを rulec が確かめ、承認する人がページで読めます。",
            "Write the threshold in a rulec table and take what it decides as an output of the rule: there, rulec checks it is complete and without overlap, and the person who approves it reads it on its page."
        ))
        .note(tr!(
            "規則に渡す値を、規則の入力の範囲に狭めるための比べなら、`if` の条件に書いてください。`if` の条件の比べは、警告になりません。",
            "A comparison that narrows a value to the range of a rule's input before the rule is called goes in the condition of an `if`, which is not warned of."
        ));
        self.out.push(d);
    }

    /// What a branch answers: its block's answer, through a block of `let` lines; a branch that
    /// answers no constant and calls a rule or a date anywhere hands the decision to it.
    fn answer(&self, b: &Block, numbers: bool) -> Answer {
        match self.classify(&b.answer, numbers) {
            Answer::Other => {
                let mut all = Vec::new();
                b.walk(&mut all);
                let calls = all.iter().any(|x| matches!(&x.kind, ExprKind::Call(callee, _) if matches!(self.rf(callee), Ref::Rule(_) | Ref::Date(..))));
                if calls { Answer::Rule } else { Answer::Other }
            }
            other => other,
        }
    }

    fn classify(&self, e: &Expr, numbers: bool) -> Answer {
        match &e.kind {
            ExprKind::Num(n) => {
                let unit = !n.unit.is_empty();
                if unit || numbers { Answer::Constant(self.src(e.span)) } else { Answer::Other }
            }
            ExprKind::Neg(x) => match self.classify(x, numbers) {
                Answer::Constant(_) => Answer::Constant(self.src(e.span)),
                other => other,
            },
            ExprKind::Date(_) | ExprKind::Bool(_) => Answer::Constant(self.src(e.span)),
            ExprKind::Var(_) | ExprKind::Field(..) if matches!(self.rf(e), Ref::EnumValue(..)) => Answer::Constant(self.src(e.span)),
            ExprKind::Record(_, fs) if !fs.is_empty() && fs.iter().all(|f| matches!(self.classify(&f.value, numbers), Answer::Constant(_)) || matches!(f.value.kind, ExprKind::None)) => {
                Answer::Constant(self.src(e.span))
            }
            ExprKind::Block(b) => self.answer(b, numbers),
            ExprKind::Field(base, _, _) => match self.classify(base, numbers) {
                Answer::Rule => Answer::Rule,
                _ => Answer::Other,
            },
            ExprKind::Call(callee, _) if matches!(self.rf(callee), Ref::Rule(_) | Ref::Date(..)) => Answer::Rule,
            _ => Answer::Other,
        }
    }

    /// The answers of an `if` and of the `else if`s chained to it, marking the chained ones.
    fn chain<'e>(&mut self, e: &'e Expr, out: &mut Vec<&'e Block>) {
        let ExprKind::If(_, t, f) = &e.kind else { return };
        out.push(t);
        if f.lets.is_empty()
            && let ExprKind::If(..) = &f.answer.kind
        {
            self.chained.insert(f.answer.id);
            self.chain(&f.answer, out);
        } else {
            out.push(f);
        }
    }

    /// The first shape, for an `if` and its chain.
    fn decision_if(&mut self, e: &Expr) {
        let mut blocks = Vec::new();
        self.chain(e, &mut blocks);
        let answers: Vec<Answer> = blocks.iter().map(|b| self.answer(b, false)).collect();
        self.decision(e, answers, tr!("`if`", "`if`"));
    }

    /// The first shape, for a `match`: not for one that only splits `none` from `some x`, nor for one
    /// that maps each value of an enum to the value of the same name of another.
    fn decision_match(&mut self, e: &Expr, s: &Expr, arms: &[Arm]) {
        if arms.iter().all(|a| a.patterns.iter().all(|p| matches!(p, Pattern::None(_) | Pattern::Some(..)))) {
            return;
        }
        if self.same_names(s, arms) {
            return;
        }
        let answers: Vec<Answer> = arms.iter().map(|a| self.answer(&a.body, true)).collect();
        self.decision(e, answers, tr!("`match`", "`match`"));
    }

    /// Whether each arm takes one value of an enum and answers the value of the same name (or alias)
    /// of another enum.
    fn same_names(&self, s: &Expr, arms: &[Arm]) -> bool {
        let Ty::Enum(from) = self.ty(s).present() else { return false };
        arms.iter().all(|a| {
            let [Pattern::Value(path, _)] = a.patterns.as_slice() else { return false };
            let word = &path[path.len() - 1].0;
            let Some(taken) = values(self.c, from).into_iter().find(|n| n.is(word)) else { return false };
            let ans = &a.body.answer;
            match self.rf(ans) {
                Ref::EnumValue(to, v) if to != from && a.body.lets.is_empty() => {
                    let names = |n: &Name| -> Vec<String> { std::iter::once(n.text.clone()).chain(n.alias.as_ref().map(|(x, _)| x.clone())).collect() };
                    let given = values(self.c, to).into_iter().find(|n| n.text == *v);
                    given.is_some_and(|g| names(&g).iter().any(|x| names(&taken).contains(x)))
                }
                _ => false,
            }
        })
    }

    fn decision(&mut self, e: &Expr, answers: Vec<Answer>, word: Text) {
        let constants: Vec<String> = answers
            .iter()
            .filter_map(|a| match a {
                Answer::Constant(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        if constants.is_empty() {
            return;
        }
        let rule = answers.iter().any(|a| matches!(a, Answer::Rule));
        let mut shown: Vec<String> = Vec::new();
        for k in constants {
            if !shown.contains(&k) && shown.len() < 3 {
                shown.push(k);
            }
        }
        let list_ja = shown.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join("、");
        let list_en = shown.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", ");
        let c = self.c;
        let msg = if word.en == "`match`" {
            tr!(
                "この {} は、分けた値ごとに業務の定数（{list_ja}）を答えにしています。表に書く判断です",
                "This {} answers a constant of the business for the values it splits ({list_en}): a decision, which belongs in a table",
                word.ja;
                word.en
            )
        } else {
            tr!(
                "この {} は、比べた値によって業務の定数（{list_ja}）を選んでいます。表に書く判断です",
                "This {} chooses a constant of the business by the values it compares ({list_en}): a decision, which belongs in a table",
                word.ja;
                word.en
            )
        };
        let mut d = Diag::at("W501", &c.path, e.span.line, e.span.col, msg).source(&c.src).note(tr!(
            "判断を rulec の表に書いて、規則を呼んでください。表に書けば、完全性と重なりを rulec が確かめ、承認する人がページで読めます。",
            "Write the decision in a rulec table and call the rule: there, rulec checks it is complete and without overlap, and the person who approves it reads it on its page."
        ));
        if rule {
            d = d.note(tr!(
                "ほかの分岐は規則を呼んでいます。この場合に答えが要らないなら、`none` を答えにしてください。この場合も規則が決めるなら、規則に行を足し、その入力の範囲を広げてください。",
                "Another branch calls a rule. If no answer is meant in this case, answer `none`; if the rule decides this case too, give it a row and widen the range of its input."
            ));
        }
        self.out.push(d);
    }
}

/// The values of an enum, by their names and aliases: one of the module, of a module it reads, or
/// of a rule.
fn values(c: &Checked, id: &crate::ty::EnumId) -> Vec<Name> {
    use crate::ty::Owner;
    match &id.owner {
        Owner::Module(file) => {
            let decls = if *file == c.canon {
                &c.decls
            } else {
                match c.world.modules.get(file) {
                    Some(m) => &m.decls,
                    None => return Vec::new(),
                }
            };
            decls.enums.iter().find(|e| e.name.text == id.name).map(|e| e.values.clone()).unwrap_or_default()
        }
        Owner::Rule(file) => c
            .world
            .rules
            .get(file)
            .and_then(|f| f.enums.iter().find(|e| e.name == id.name))
            .map(|e| e.values.iter().map(|v| Name { text: v.name.clone(), alias: (v.public != v.name).then(|| (v.public.clone(), Span::default())), span: Span::default() }).collect())
            .unwrap_or_default(),
    }
}
