//! The contract between a task and the `.flow` it runs as its child (`flow "<path>"`), E015:
//! the task passes each input the child needs, of a type and in a range the input takes; the
//! child's outputs are what the task's answer reads; and each error the task declares is one
//! the child fails with. The two files declare their records and enums each on their own, so
//! types are compared by their shape: a record by its fields, an enum by its values.

use crate::diag::Diag;
use crate::model::*;

/// A difference, in English and in Japanese.
type Words = (String, String);

pub fn check(pm: &Model, task: &TaskDef, cm: &Model) -> Vec<Diag> {
    let mut out = Vec::new();
    let child = &cm.name;
    let mut push = |(en, ja): Words| out.push(Diag::error("E015", task.line, 1, en, ja));
    // what the task passes: every input the child needs, of a type the input takes
    for (p, pt) in &task.params {
        let Some((_, it)) = cm.inputs.iter().find(|(i, _)| i == p) else {
            let names: Vec<&str> = cm.inputs.iter().map(|(i, _)| i.as_str()).collect();
            push((format!("`{child}` has no input `{p}` (its inputs are {})", names.join(", ")), format!("`{child}` に入力 `{p}` はありません（入力は {}）", names.join("・"))));
            continue;
        };
        if let Err((en, ja)) = fits(pm, pt, task.param_ranges.get(p).copied(), cm, it, cm.input_ranges.get(p).copied(), &mut vec![]) {
            push((format!("the parameter `{p}` is not what `{child}` takes as its input `{p}`: {en}"), format!("引数 `{p}` は、`{child}` が入力 `{p}` として受け取るものと合いません。{ja}")));
        }
    }
    for (i, it) in &cm.inputs {
        if !matches!(it, Ty::Opt(_)) && !task.params.iter().any(|(p, _)| p == i) {
            push((format!("`{child}` needs the input `{i}`, which `{}` does not pass", task.name), format!("`{child}` には入力 `{i}` が要りますが、`{}` はそれを渡しません", task.name)));
        }
    }
    // what the child answers: its outputs, which the task reads as a record
    match &task.result {
        None | Some(Ty::Json) => {}
        Some(Ty::Record(r)) => {
            let rd = &pm.records[*r];
            for (f, ft) in &rd.fields {
                match cm.outputs.iter().find(|(o, _)| o == f) {
                    Some((_, ot)) => {
                        if let Err((en, ja)) = fits(cm, ot, cm.output_ranges.get(f).copied(), pm, ft, rd.ranges.get(f).copied(), &mut vec![]) {
                            push((format!("`{child}` answers `{f}` with what the field `{f}` of `{}` does not take: {en}", rd.name), format!("`{child}` が返す `{f}` は、`{}` のフィールド `{f}` と合いません。{ja}", rd.name)));
                        }
                    }
                    None if matches!(ft, Ty::Opt(_)) => {}
                    None => push((format!("`{child}` has no output `{f}`, which `{}` reads", rd.name), format!("`{child}` に出力 `{f}` はありませんが、`{}` はそれを読みます", rd.name))),
                }
            }
        }
        Some(t) => push((
            format!("`{child}` answers with its outputs, a record, and the answer of `{}` is `{}`", task.name, pm.ty_name(t)),
            format!("`{child}` は出力をレコードとして返しますが、`{}` の結果の型は `{}` です", task.name, pm.ty_name(t)),
        )),
    }
    // the errors: each is one the child fails with
    let fails = fail_names(cm);
    for er in &task.errors {
        if !fails.contains(&er.name) {
            push((format!("`{child}` never fails with `{}`", er.name), format!("`{child}` が `{}` で失敗することはありません", er.name)));
        }
    }
    out
}

/// The names a flow's `fail`s end it with: the child's, here; a service's `fails` are held to them too.
pub(crate) fn fail_names(m: &Model) -> Vec<String> {
    m.all_stmts()
        .into_iter()
        .filter_map(|s| match &s.kind {
            TK::Fail { error, .. } => Some(error.clone()),
            _ => None,
        })
        .collect()
}

/// Whether a value of `a` (of `am`, its numbers in `arg`) is one that `b` (of `bm`, its numbers in
/// `brg`) takes. `seen` holds the pairs of records being compared, for records that hold themselves.
fn fits(am: &Model, a: &Ty, arg: Option<Range>, bm: &Model, b: &Ty, brg: Option<Range>, seen: &mut Vec<(RecordId, RecordId)>) -> Result<(), Words> {
    let (an, bn) = (am.ty_name(a), bm.ty_name(b));
    let differ = || (format!("`{an}` is not `{bn}`"), format!("`{an}` は `{bn}` ではありません"));
    match (a, b) {
        (_, Ty::Json) => Ok(()),
        (Ty::Opt(x), Ty::Opt(y)) => fits(am, x, arg, bm, y, brg, seen),
        (Ty::Opt(_), _) => Err((format!("`{an}` may be absent, and `{bn}` may not"), format!("`{an}` は無いことがありますが、`{bn}` は無いことがありません"))),
        (x, Ty::Opt(y)) => fits(am, x, arg, bm, y, brg, seen),
        (Ty::List(x), Ty::List(y)) => fits(am, x, arg, bm, y, brg, seen),
        (Ty::Int, Ty::Int) => within(arg, brg),
        (Ty::Num(u), Ty::Num(v)) if u == v => within(arg, brg),
        (Ty::Str, Ty::Str) | (Ty::Bool, Ty::Bool) | (Ty::Timestamp, Ty::Timestamp) => Ok(()),
        (Ty::Enum(x), Ty::Enum(y)) => match am.enums[*x].values.iter().find(|v| !bm.enums[*y].values.contains(v)) {
            Some(v) => Err((format!("`{v}` of `{an}` is not a value of `{bn}`"), format!("`{an}` の `{v}` は `{bn}` の値にありません"))),
            None => Ok(()),
        },
        (Ty::Record(x), Ty::Record(y)) => {
            if seen.contains(&(*x, *y)) {
                return Ok(());
            }
            seen.push((*x, *y));
            let (ad, bd) = (&am.records[*x], &bm.records[*y]);
            for (f, ft) in &bd.fields {
                let r = match ad.fields.iter().find(|(g, _)| g == f) {
                    Some((_, gt)) => fits(am, gt, ad.ranges.get(f).copied(), bm, ft, bd.ranges.get(f).copied(), seen),
                    None if matches!(ft, Ty::Opt(_)) => Ok(()),
                    None => Err((format!("`{an}` has no field `{f}`"), format!("`{an}` にフィールド `{f}` がありません"))),
                };
                r.map_err(|(en, ja)| (format!("in the field `{f}` of `{bn}`, {en}"), format!("`{bn}` のフィールド `{f}` で、{ja}")))?;
            }
            seen.pop();
            Ok(())
        }
        _ => Err(differ()),
    }
}

/// Whether numbers in `have` are all in `want`, when `want` is a range at all.
fn within(have: Option<Range>, want: Option<Range>) -> Result<(), Words> {
    let Some(w) = want else { return Ok(()) };
    match have {
        Some(h) if h.within(&w) => Ok(()),
        Some(h) => Err((format!("it is `{}`, and `{}` is taken", h.show(), w.show()), format!("`{}` で、受け取るのは `{}` です", h.show(), w.show()))),
        None => Err((format!("it has no range, and `{}` is taken", w.show()), format!("範囲が書かれていませんが、受け取るのは `{}` です", w.show()))),
    }
}
