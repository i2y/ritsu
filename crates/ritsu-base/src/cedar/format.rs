//! `cedar format` (`cedar-policy-formatter` 4.13.0): each node of the concrete tree becomes a
//! document as `pprint/doc.rs` builds it, the comments are taken from the tokens they sit
//! around in the order it takes them (a comment is used once), each policy is rendered and its
//! blank lines dropped, and the policies are joined by a blank line.

use super::cst::{self, Node, Span};
use super::lexer::Token;
use super::pretty::*;
use super::Error;

#[derive(Clone, Debug, Default)]
struct Comment<'s> {
    leading: Vec<&'s str>,
    trailing: &'s str,
}

struct WTok<'s> {
    start: usize,
    end: usize,
    comment: Comment<'s>,
}

struct Cx<'s> {
    toks: Vec<WTok<'s>>,
    indent: isize,
}

/// The comments in a text: each `//` to the end of its line, trimmed.
fn comments_in(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(k) = text[i..].find("//") {
        let s = i + k;
        let e = text[s..].find(['\n', '\r']).map(|x| s + x).unwrap_or(text.len());
        out.push(text[s..e].trim());
        i = e;
    }
    out
}

/// The tokens, each with the comment lines before it (after the line break that follows the
/// token before) and the rest of the line after it; and the comments after the last token.
fn token_stream<'s>(src: &'s str, tokens: &[Token]) -> (Vec<WTok<'s>>, Vec<&'s str>) {
    if tokens.is_empty() {
        return (Vec::new(), comments_in(src));
    }
    let mut out = Vec::new();
    let mut leading = &src[..tokens[0].start];
    for (k, t) in tokens.iter().enumerate() {
        let after = match tokens.get(k + 1) {
            Some(n) => &src[t.end..n.start],
            None => &src[t.end..],
        };
        let (trailing, next_leading) = after.split_once('\n').unwrap_or((after, ""));
        out.push(WTok { start: t.start, end: t.end, comment: Comment { leading: comments_in(leading), trailing: trailing.trim() } });
        leading = next_leading;
    }
    let eof = comments_in(leading);
    (out, eof)
}

fn leading_doc(leading: &[&str]) -> Doc {
    if leading.is_empty() {
        nil()
    } else {
        hardline().app(intersperse(leading.iter().map(|c| text(*c)), hardline())).app(hardline())
    }
}

fn trailing_doc(trailing: &str, next: Doc) -> Doc {
    if trailing.is_empty() { next } else { space().app(text(trailing)).app(hardline()) }
}

fn add_comment(d: Doc, c: &Comment, next: Doc) -> Doc {
    leading_doc(&c.leading).app(d).app(trailing_doc(c.trailing, next))
}

impl<'s> Cx<'s> {
    fn take(&mut self, k: usize) -> Comment<'s> {
        std::mem::take(&mut self.toks[k].comment)
    }

    // The tokens are in the order of the text, so each lookup is a binary search (Cedar's
    // formatter walks the list from the start each time; the answers are the same).

    fn at_start(&mut self, s: Span) -> Option<Comment<'s>> {
        let k = self.toks.binary_search_by_key(&s.start, |t| t.start).ok()?;
        Some(self.take(k))
    }

    fn leading_at_start(&mut self, s: Span) -> Option<Vec<&'s str>> {
        let k = self.toks.binary_search_by_key(&s.start, |t| t.start).ok()?;
        Some(std::mem::take(&mut self.toks[k].comment.leading))
    }

    /// The first token at or after the end (or the first token of all, as `find_or_first`).
    fn after_end(&mut self, s: Span) -> Option<Comment<'s>> {
        if self.toks.is_empty() {
            return None;
        }
        let k = self.toks.partition_point(|t| t.start < s.end);
        Some(self.take(if k == self.toks.len() { 0 } else { k }))
    }

    fn at_end(&mut self, s: Span) -> Option<Comment<'s>> {
        let k = self.toks.binary_search_by_key(&s.end, |t| t.end).ok()?;
        Some(self.take(k))
    }

    fn in_range(&mut self, start: usize, end: usize) -> Vec<Comment<'s>> {
        let mut out = Vec::new();
        let mut k = self.toks.partition_point(|t| t.start < start);
        while k < self.toks.len() && self.toks[k].end <= end {
            out.push(self.take(k));
            k += 1;
        }
        out
    }

    fn ident(&mut self, n: &Node<cst::Ident>) -> Option<Doc> {
        let c = self.at_start(n.span)?;
        Some(add_comment(text(n.node.as_str()), &c, nil()))
    }

    fn variable_def(&mut self, v: &Node<cst::VariableDef>) -> Option<Doc> {
        let vd = &v.node;
        let ind = self.indent;
        let start_comment = self.at_start(v.span)?;
        let var_doc = text(vd.variable.node.as_str());
        let is_doc = match &vd.entity_type {
            Some(et) => {
                let c = self.after_end(vd.variable.span)?;
                let first = line().app(add_comment(text("is"), &c, nil())).grp();
                let et_doc = self.add(et)?;
                let c2 = self.at_start(et.span)?;
                first.app(line().app(add_comment(et_doc, &c2, nil()))).nst(ind).grp()
            }
            None => nil(),
        };
        Some(match &vd.ineq {
            Some((op, rhs)) => {
                let op_comment = match &vd.entity_type {
                    Some(et) => self.after_end(et.span)?,
                    None => self.after_end(vd.variable.span)?,
                };
                let head = var_doc
                    .app(trailing_doc(start_comment.trailing, nil()))
                    .app(is_doc)
                    .app(line())
                    .app(add_comment(text(op.as_str()), &op_comment, nil()))
                    .grp();
                let rhs_doc = self.expr(rhs).unwrap_or_else(nil);
                leading_doc(&start_comment.leading).app(head.app(line().app(rhs_doc).nst(ind)).grp())
            }
            None => add_comment(var_doc, &start_comment, nil()).app(is_doc),
        })
    }

    fn cond(&mut self, c: &Node<cst::Cond>) -> Option<Doc> {
        let ind = self.indent;
        let lb = self.after_end(c.node.cond.span)?;
        let rb = self.at_end(c.span)?;
        let cc = self.at_start(c.node.cond.span)?;
        let rb_doc = add_comment(text("}"), &rb, nil());
        let cond_doc = self.ident(&c.node.cond)?;
        let expr = c.node.expr.as_ref()?;
        let expr_leading = self.leading_at_start(expr.span)?;
        let expr_doc = self.expr(expr)?;
        let inner = trailing_doc(lb.trailing, line()).app(leading_doc(&expr_leading).app(expr_doc.grp())).nst(ind).app(line()).app(rb_doc).grp();
        Some(leading_doc(&cc.leading).app(cond_doc.app(trailing_doc(cc.trailing, line())).app(leading_doc(&lb.leading).app(text("{").app(inner))).grp()))
    }

    fn expr(&mut self, e: &Node<cst::Expr>) -> Option<Doc> {
        match &e.node {
            cst::Expr::Or(or) => self.or(or),
            cst::Expr::If(c, t, f) => {
                let ind = self.indent;
                let if_c = self.at_start(e.span)?;
                let then_c = self.after_end(c.span)?;
                let else_c = self.after_end(t.span)?;
                let pp = |s: &str, cm: &Comment, x: &Node<cst::Expr>, me: &mut Self| {
                    let d = me.expr(x).unwrap_or_else(nil);
                    add_comment(text(s), cm, nil()).app(line().app(d).nst(ind))
                };
                let a = pp("if", &if_c, c, self);
                let b = pp("then", &then_c, t, self);
                let d = pp("else", &else_c, f, self);
                Some(a.app(line()).app(b).app(line()).app(d).grp())
            }
        }
    }

    fn or(&mut self, or: &Node<cst::Or>) -> Option<Doc> {
        let es: Vec<&Node<cst::And>> = std::iter::once(&or.node.initial).chain(or.node.extended.iter()).collect();
        let mut d = nil();
        for e in &es[..es.len() - 1] {
            let c = self.after_end(e.span)?;
            let x = self.and(e).unwrap_or_else(nil);
            d = d.app(x).app(space()).app(add_comment(text("||"), &c, line()));
        }
        let last = self.and(es[es.len() - 1]).unwrap_or_else(nil);
        Some(d.app(last))
    }

    fn and(&mut self, and: &Node<cst::And>) -> Option<Doc> {
        let es: Vec<&Node<cst::Relation>> = std::iter::once(&and.node.initial).chain(and.node.extended.iter()).collect();
        let mut d = nil();
        for e in &es[..es.len() - 1] {
            let c = self.after_end(e.span)?;
            let x = self.relation(e).unwrap_or_else(nil);
            d = d.app(x).app(space()).app(add_comment(text("&&"), &c, line()));
        }
        let last = self.relation(es[es.len() - 1]).unwrap_or_else(nil);
        Some(d.app(last))
    }

    fn relation(&mut self, r: &Node<cst::Relation>) -> Option<Doc> {
        let ind = self.indent;
        match &r.node {
            cst::Relation::Common { initial, extended } => match extended.as_slice() {
                [] => self.add(initial),
                [(op, n)] => {
                    let a = self.add(initial)?;
                    let c = self.after_end(initial.span)?;
                    let b = self.add(n).unwrap_or_else(nil);
                    Some(a.app(space()).app(add_comment(text(op.as_str()), &c, nil())).app(space()).app(b))
                }
                _ => None,
            },
            cst::Relation::Has { target, field } | cst::Relation::Like { target, pattern: field } => {
                let word = if matches!(r.node, cst::Relation::Has { .. }) { "has" } else { "like" };
                let a = self.add(target)?;
                let c = self.after_end(target.span)?;
                let b = self.add(field)?;
                Some(a.app(line()).app(add_comment(text(word), &c, nil())).app(line()).app(b.nst(ind)).grp())
            }
            cst::Relation::IsIn { target, entity_type, in_entity } => {
                let a = self.add(target)?;
                let c = self.after_end(target.span)?;
                let t = self.add(entity_type)?;
                let doc_is = a.app(space()).app(add_comment(text("is"), &c, nil())).app(space()).app(t.nst(ind));
                let d = match in_entity {
                    Some(ie) => {
                        let c2 = self.after_end(entity_type.span)?;
                        let x = self.add(ie)?;
                        doc_is.app(line()).app(add_comment(text("in"), &c2, nil())).app(space()).app(x.nst(ind))
                    }
                    None => doc_is,
                };
                Some(d.grp())
            }
        }
    }

    fn add(&mut self, a: &Node<cst::Add>) -> Option<Doc> {
        let mut d = self.mult(&a.node.initial)?;
        let mut prev = a.node.initial.span;
        for (op, m) in &a.node.extended {
            let o = if *op == cst::AddOp::Plus { "+" } else { "-" };
            let c = self.after_end(prev)?;
            let x = self.mult(m).unwrap_or_else(nil);
            d = d.app(space()).app(add_comment(text(o), &c, nil())).app(line()).app(x);
            prev = m.span;
        }
        Some(d.grp())
    }

    fn mult(&mut self, m: &Node<cst::Mult>) -> Option<Doc> {
        let mut d = self.unary(&m.node.initial)?;
        let mut prev = m.node.initial.span;
        for (op, u) in &m.node.extended {
            let o = match op {
                cst::MultOp::Times => "*",
                cst::MultOp::Divide => "/",
                cst::MultOp::Mod => "%",
            };
            let c = self.after_end(prev)?;
            let x = self.unary(u).unwrap_or_else(nil);
            d = d.app(space()).app(add_comment(text(o), &c, nil())).app(line()).app(x);
            prev = u.span;
        }
        Some(d.grp())
    }

    fn unary(&mut self, u: &Node<cst::Unary>) -> Option<Doc> {
        match u.node.op {
            None => self.member(&u.node.item),
            Some(cst::NegOp::OverBang) | Some(cst::NegOp::OverDash) => None,
            Some(cst::NegOp::Bang(n)) | Some(cst::NegOp::Dash(n)) => {
                let cs = self.in_range(u.span.start, u.node.item.span.start);
                if cs.len() != n as usize {
                    return None;
                }
                let sym = if matches!(u.node.op, Some(cst::NegOp::Bang(_))) { "!" } else { "-" };
                let ops = intersperse(cs.iter().map(|c| add_comment(text(sym), c, nil())), nil());
                let item = self.member(&u.node.item)?;
                Some(ops.app(item))
            }
        }
    }

    fn member(&mut self, m: &Node<cst::Member>) -> Option<Doc> {
        let item = self.primary(&m.node.item)?;
        let mut accs = Vec::new();
        for a in &m.node.access {
            accs.push(self.mem_access(a).unwrap_or_else(nil));
        }
        Some(item.app(intersperse(accs, line_()).nst(self.indent)).grp())
    }

    fn mem_access(&mut self, a: &Node<cst::MemAccess>) -> Option<Doc> {
        let ind = self.indent;
        match &a.node {
            cst::MemAccess::Field(f) => {
                let c = self.at_start(a.span)?;
                let d = add_comment(text("."), &c, nil());
                Some(d.app(self.ident(f).unwrap_or_else(nil)))
            }
            cst::MemAccess::Call(args) => {
                let c = self.at_start(a.span)?;
                let open = add_comment(text("("), &c, nil());
                let body = if args.is_empty() {
                    nil()
                } else {
                    let mut d = self.expr(&args[0])?;
                    let mut prev = args[0].span;
                    for x in &args[1..] {
                        let cm = self.after_end(prev)?;
                        let xd = self.expr(x).unwrap_or_else(nil);
                        d = d.app(add_comment(text(","), &cm, nil())).app(line()).app(xd);
                        prev = x.span;
                    }
                    line_().app(d).nst(ind).app(line_())
                };
                let c2 = self.at_end(a.span)?;
                Some(open.app(body).app(add_comment(text(")"), &c2, nil())))
            }
            cst::MemAccess::Index(idx) => {
                let c = self.at_start(a.span)?;
                let open = add_comment(text("["), &c, nil());
                let d = self.expr(idx).unwrap_or_else(nil);
                let c2 = self.at_end(a.span)?;
                Some(open.app(d).app(add_comment(text("]"), &c2, nil())))
            }
        }
    }

    fn name(&mut self, n: &Node<cst::Name>) -> Option<Doc> {
        let path = &n.node.path;
        if path.is_empty() {
            return self.ident(&n.node.name);
        }
        let mut d = self.ident(&path[0])?;
        let mut prev = path[0].span;
        for p in &path[1..] {
            let c = self.after_end(prev)?;
            d = d.app(add_comment(text("::"), &c, nil()));
            d = d.app(self.ident(p)?);
            prev = p.span;
        }
        let c = self.after_end(path[path.len() - 1].span)?;
        let last = add_comment(text("::"), &c, nil());
        let id = self.ident(&n.node.name).unwrap_or_else(nil);
        Some(d.app(last).app(id))
    }

    fn string(&mut self, s: &Node<String>) -> Option<Doc> {
        let c = self.at_start(s.span)?;
        Some(add_comment(text(format!("\"{}\"", s.node)), &c, nil()))
    }

    fn primary(&mut self, p: &Node<cst::Primary>) -> Option<Doc> {
        let ind = self.indent;
        match &p.node {
            cst::Primary::Literal(l) => {
                let t = match &l.node {
                    cst::Literal::True => "true".to_string(),
                    cst::Literal::False => "false".to_string(),
                    cst::Literal::Num(n) => n.to_string(),
                    cst::Literal::Str(s) => format!("\"{}\"", s.node),
                };
                let c = self.at_start(l.span)?;
                Some(add_comment(text(t), &c, nil()))
            }
            cst::Primary::Ref(r) => match &r.node {
                cst::Ref::Uid { path, eid } => {
                    let d = self.name(path)?;
                    let c = self.after_end(path.span)?;
                    let d = d.app(add_comment(text("::"), &c, nil()));
                    Some(d.app(self.string(eid)?))
                }
                cst::Ref::Ref { .. } => None,
            },
            cst::Primary::Name(n) => self.name(n),
            cst::Primary::Slot(s) => {
                let c = self.at_start(s.span)?;
                Some(add_comment(text(s.node.as_str()), &c, nil()))
            }
            cst::Primary::Expr(e) => {
                let c = self.at_start(p.span)?;
                let open = add_comment(text("("), &c, nil());
                let inner = self.expr(e)?;
                let c2 = self.at_end(p.span)?;
                Some(open.app(inner.nst(1)).app(add_comment(text(")"), &c2, nil())).grp())
            }
            cst::Primary::EList(el) => {
                let inner = if el.is_empty() {
                    nil()
                } else {
                    let mut d = self.expr(&el[0])?;
                    let mut prev = el[0].span;
                    for v in &el[1..] {
                        let c = self.after_end(prev)?;
                        let vd = self.expr(v).unwrap_or_else(nil);
                        d = d.app(add_comment(text(","), &c, nil())).app(line()).app(vd);
                        prev = v.span;
                    }
                    d
                };
                let c1 = self.at_start(p.span)?;
                let open = add_comment(text("["), &c1, nil());
                let c2 = self.at_end(p.span)?;
                let close = add_comment(text("]"), &c2, nil());
                Some(open.app(inner.nst(1)).app(close))
            }
            cst::Primary::RInits(ri) => {
                let inner = if ri.is_empty() {
                    nil()
                } else {
                    let mut d = self.rec_init(&ri[0])?;
                    let mut prev = ri[0].span;
                    for v in &ri[1..] {
                        let c = self.after_end(prev)?;
                        let vd = self.rec_init(v).unwrap_or_else(nil);
                        d = d.app(add_comment(text(","), &c, nil())).app(line()).app(vd);
                        prev = v.span;
                    }
                    line().app(d).app(line()).grp()
                };
                let c1 = self.at_start(p.span)?;
                let open = add_comment(text("{"), &c1, nil());
                let c2 = self.at_end(p.span)?;
                let close = add_comment(text("}"), &c2, nil());
                Some(open.app(inner.nst(ind)).app(close))
            }
        }
    }

    fn rec_init(&mut self, r: &Node<cst::RecInit>) -> Option<Doc> {
        let key = self.expr(&r.node.0)?;
        let value = self.expr(&r.node.1)?;
        let c = self.after_end(r.node.0.span)?;
        Some(key.app(add_comment(text(":"), &c, nil())).app(space()).app(value))
    }

    fn annotation(&mut self, a: &Node<cst::Annotation>) -> Option<Doc> {
        let id = self.ident(&a.node.key).unwrap_or_else(nil);
        let c = self.at_start(a.span)?;
        let at = add_comment(text("@"), &c, nil());
        let val = match &a.node.value {
            Some(v) => {
                let lc = self.after_end(a.node.key.span)?;
                let lp = add_comment(text("("), &lc, nil());
                let vd = self.string(v).unwrap_or_else(nil);
                let rc = self.at_end(a.span)?;
                let rp = add_comment(text(")"), &rc, hardline());
                lp.app(vd).app(rp)
            }
            None => hardline(),
        };
        Some(at.app(id).app(val))
    }

    fn policy(&mut self, node: &Node<cst::Policy>) -> Option<Doc> {
        let p = &node.node;
        let ind = self.indent;
        let mut annos = Vec::new();
        for a in &p.annotations {
            annos.push(self.annotation(a).unwrap_or_else(nil));
        }
        let anno_doc = intersperse(annos, nil());
        let eff_leading = self.leading_at_start(p.effect.span)?;
        let eff_doc = self.ident(&p.effect)?;
        let vars = &p.variables;
        if vars.len() < 3 {
            return None;
        }
        let principal = self.variable_def(&vars[0])?;
        let action = self.variable_def(&vars[1])?;
        let resource = self.variable_def(&vars[2])?;
        let plain = vars[..3].iter().all(|v| v.node.ineq.is_none() && v.node.entity_type.is_none());
        let vars_doc = if plain {
            let c0 = self.after_end(vars[0].span)?;
            let c1 = self.after_end(vars[1].span)?;
            principal.app(add_comment(text(","), &c0, space())).app(action).app(add_comment(text(","), &c1, space())).app(resource).nst(ind).grp()
        } else {
            let c0 = self.after_end(vars[0].span)?;
            let c1 = self.after_end(vars[1].span)?;
            hardline()
                .app(principal.app(add_comment(text(","), &c0, hardline())).app(action).app(add_comment(text(","), &c1, hardline())).app(resource))
                .nst(ind)
                .app(hardline())
        };
        let mut conds = Vec::new();
        for c in &p.conds {
            conds.push(self.cond(c).unwrap_or_else(nil));
        }
        let has_conds = !conds.is_empty();
        let cond_doc = intersperse(conds, hardline());
        let lp = self.after_end(p.effect.span)?;
        let head = leading_doc(&eff_leading).app(eff_doc.app(line()).app(add_comment(text("("), &lp, nil())).grp());
        let rp = self.after_end(vars[2].span)?;
        let close = add_comment(text(")"), &rp, if has_conds { hardline() } else { nil() });
        let semi = self.at_end(node.span)?;
        Some(anno_doc.app(head).app(vars_doc).app(close).app(cond_doc).app(add_comment(text(";"), &semi, nil())))
    }
}

/// Blank lines dropped outside strings and comments (`remove_empty_interior_lines`).
fn drop_blank_lines(s: &str) -> String {
    let mut out = String::new();
    if s.starts_with('\n') {
        out.push('\n');
    }
    for piece in s.split_inclusive('\n') {
        if !piece.trim().is_empty() || !piece.contains('\n') {
            out.push_str(piece);
        }
    }
    out
}

/// The next comment (`//` to the end of the line) or string (`"…"`, as the policy grammar
/// matches one) at or after `i`: its start and end.
fn next_comment_or_string(t: &str, i: usize) -> Option<(usize, usize)> {
    let b = t.as_bytes();
    let mut k = i;
    while k < b.len() {
        if b[k] == b'/' && b.get(k + 1) == Some(&b'/') {
            let e = t[k..].find(['\n', '\r']).map(|x| k + x).unwrap_or(t.len());
            return Some((k, e));
        }
        if b[k] == b'"' {
            // a string runs to the next unescaped quote; `\` takes any character but a line feed
            let mut j = k + 1;
            let mut ok = false;
            while j < b.len() {
                match b[j] {
                    b'"' => {
                        ok = true;
                        break;
                    }
                    b'\\' => {
                        if j + 1 < b.len() && b[j + 1] != b'\n' {
                            let w = t[j + 1..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                            j += 1 + w;
                        } else {
                            break;
                        }
                    }
                    _ => j += 1,
                }
            }
            if ok {
                return Some((k, j + 1));
            }
        }
        k += 1;
    }
    None
}

/// `remove_empty_lines`: blank lines dropped, but not inside strings and comments, and the
/// white space at both ends trimmed.
fn remove_empty_lines(t: &str) -> String {
    let mut i = 0;
    let mut out = String::new();
    while i < t.len() {
        match next_comment_or_string(t, i) {
            Some((s, e)) => {
                out.push_str(&drop_blank_lines(&t[i..s]));
                out.push_str(&t[s..e]);
                i = e;
            }
            None => {
                out.push_str(&drop_blank_lines(&t[i..]));
                break;
            }
        }
    }
    out.trim().to_string()
}

pub(crate) fn format(src: &str, width: usize, indent: usize) -> Result<String, Error> {
    let parsed = cst::parse(src)?;
    // what Cedar does not read, it does not format either
    super::ast::convert(src, &parsed)?;
    let (toks, eof) = token_stream(src, &parsed.tokens);
    let mut cx = Cx { toks, indent: indent as isize };
    let mut parts = Vec::new();
    for p in &parsed.policies {
        let Some(doc) = cx.policy(p) else {
            let (line, col) = super::lexer::Lines::new(src).at(p.span.start);
            return Err(Error { line, col, message: crate::tr!("このポリシーは整形できません", "this policy cannot be formatted") });
        };
        parts.push(remove_empty_lines(&render(&doc, width)));
    }
    let mut out = parts.join("\n\n");
    out.push('\n');
    for c in eof {
        out.push_str(c);
        out.push('\n');
    }
    Ok(out)
}
