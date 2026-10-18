//! The `examples` of a module, run (DESIGN 12, E601): each line calls a function of the module in the
//! reference interpreter, as a host would call it, and the answer it gives, or the kind of failure it
//! ends in, is held to what the line writes. `check` runs them once the module passes everything
//! else, so `ritsu check` runs them too.

use crate::ast::{Arg, Example, ExprKind, Expect};
use crate::check::Checked;
use crate::diag::Diag;
use crate::interp::{Failure, Interp, Kind, Stop};
use crate::suite::Suite;
use crate::types::Ref;
use crate::value::{Program, Value};
use ritsu_base::text::Text;
use std::rc::Rc;

/// What one example comes to: an answer, or why it has none.
pub enum Came {
    Answer(Value),
    Stopped(Stop),
}

/// The example `x` of module `c`, run: the function it calls, the arguments it gives (`None` for one
/// the line does not write), and what the call comes to.
pub fn run_one<'p>(p: &'p Program, suite: &'p Suite, c: &'p Checked, x: &'p Example) -> Option<(usize, Vec<Value>, Came)> {
    let ExprKind::Call(callee, args) = &x.call.kind else { return None };
    let Ref::Fn(_, name) = c.typed.refs.get(callee.id)? else { return None };
    let f = c.decls.fns.iter().position(|i| i.name.text == *name)?;
    let info = &c.decls.fns[f];
    let mut it = Interp::new(p, suite);
    let mut vals = vec![Value::None; info.params.len()];
    for a in args {
        let Arg::Named(n, _, e) = a else { continue };
        let Some(k) = info.params.iter().position(|q| q.name.is(n)) else { continue };
        match it.constant(c, e, &info.params[k].slot.ty) {
            Ok(v) => vals[k] = v,
            Err(s) => return Some((f, vals, Came::Stopped(s))),
        }
    }
    if let Err(fail) = it.check_entry(c, f, &vals) {
        return Some((f, vals, Came::Stopped(Stop::Failed(fail))));
    }
    let came = match it.call(c, f, vals.clone(), 0) {
        Ok(v) => Came::Answer(v),
        Err(s) => Came::Stopped(s),
    };
    Some((f, vals, came))
}

/// Where a failure happened and what it says, as a note.
fn where_note(c: &Checked, f: &Failure) -> Text {
    let (line, msg) = (f.line, &f.message);
    let (ja, en) = (ritsu_base::text::spaced(msg, ritsu_base::text::Lang::Ja), ritsu_base::text::spaced(msg, ritsu_base::text::Lang::En));
    if f.file == c.path {
        tr!("{line} 行目: {ja}", "Line {line}: {en}")
    } else {
        let file = &f.file;
        tr!("{file} の {line} 行目: {ja}", "{file}, line {line}: {en}")
    }
}

/// The fields of two records of one type that differ, each said with its two values; nothing for
/// what is not a record.
fn differ(p: &Program, c: &Checked, got: &Value, want: &Value, t: &crate::ty::Ty) -> Vec<Text> {
    let (Value::Record(g), Value::Record(w), crate::ty::Ty::Record(id)) = (got, want, t.present()) else { return Vec::new() };
    let fields = p.fields(id);
    g.iter()
        .zip(w)
        .filter(|((_, a), (_, b))| a != b)
        .map(|((n, a), (_, b))| {
            let ft = fields.iter().find(|(f, _)| f.text == *n).map(|(_, s)| s.ty.clone()).unwrap_or(crate::ty::Ty::Error);
            let (gs, ws) = (p.show(c, a, &ft), p.show(c, b, &ft));
            tr!("違うのはフィールド `{n}` です。答えは {gs}、書いてあるのは {ws} です。", "The field `{n}` differs: the answer has {gs}, and {ws} is written.")
        })
        .collect()
}

/// E601 for every example of the module that does not hold.
pub fn check(c: &Rc<Checked>, suite: &Suite) -> Vec<Diag> {
    let p = Program::of(c);
    let mut diags = Vec::new();
    for x in &c.module.examples {
        let Some((f, _, came)) = run_one(&p, suite, c, x) else { continue };
        let ret = &c.decls.fns[f].ret.ty;
        let mut it = Interp::new(&p, suite);
        let at = |msg: Text| Diag::at("E601", &c.path, x.line, x.call.span.col, msg).source(&c.src);
        let d = match (&x.expect, came) {
            (Expect::Value(want), came) => {
                let want_v = match it.constant(c, want, ret) {
                    Ok(v) => v,
                    Err(Stop::Failed(fl)) => {
                        diags.push(at(tr!("書いてある答えを求められません", "The answer written cannot be worked out")).note(where_note(c, &fl)));
                        continue;
                    }
                    Err(Stop::Diverged(_)) => continue,
                };
                let ws = p.show(c, &want_v, ret);
                match came {
                    Came::Answer(v) if v == want_v => None,
                    Came::Answer(v) => {
                        let gs = p.show(c, &v, ret);
                        let mut d = at(tr!("例の答えは {gs} です。書いてある答えは {ws} です", "The example answers {gs}, and the answer written is {ws}"));
                        for n in differ(&p, c, &v, &want_v, ret) {
                            d = d.note(n);
                        }
                        Some(d)
                    }
                    Came::Stopped(Stop::Failed(fl)) => {
                        let k = fl.kind.word();
                        Some(at(tr!("例は `{k}` で失敗します。書いてある答えは {ws} です", "The example fails with `{k}`, and the answer written is {ws}")).note(where_note(c, &fl)))
                    }
                    Came::Stopped(Stop::Diverged(dv)) => {
                        let n = dv.depth;
                        Some(at(tr!("例の評価が止まりません。関数の呼び出しの深さが {n} を超えました", "The example does not stop: its calls of functions go deeper than {n}")))
                    }
                }
            }
            (Expect::Fails(kind, _), came) => {
                let want = Kind::of(*kind).word();
                match came {
                    Came::Stopped(Stop::Failed(fl)) if fl.kind.word() == want => None,
                    Came::Stopped(Stop::Failed(fl)) => {
                        let k = fl.kind.word();
                        Some(at(tr!("例は `{k}` で失敗します。書いてあるのは `{want}` での失敗です", "The example fails with `{k}`, and it is written to fail with `{want}`")).note(where_note(c, &fl)))
                    }
                    Came::Answer(v) => {
                        let gs = p.show(c, &v, ret);
                        Some(at(tr!("例の答えは {gs} です。書いてあるのは `{want}` での失敗です", "The example answers {gs}, and it is written to fail with `{want}`")))
                    }
                    Came::Stopped(Stop::Diverged(dv)) => {
                        let n = dv.depth;
                        Some(at(tr!("例の評価が止まりません。関数の呼び出しの深さが {n} を超えました", "The example does not stop: its calls of functions go deeper than {n}")))
                    }
                }
            }
        };
        if let Some(d) = d {
            diags.push(d.note(tr!(
                "`check` は、例の行の関数を、ホストが呼ぶときと同じように参照インタプリタで流し、答えを比べます。関数か、例に書いた答えのどちらかを直してください。",
                "`check` runs the function of an example in the reference interpreter, as a host would call it, and compares the answer: correct the function, or the answer the example writes."
            )));
        }
    }
    diags
}
