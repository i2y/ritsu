//! The check: the errors of how the book is written (`model`), the findings that only show
//! when it is called (E012, W101–W104, each with the operations that get there), what is
//! never used or never matters (W105, W106), and the report of what each operation can be
//! refused with (DESIGN 3).

use crate::diag::{self, DiagExt, Diag, Show};
use ritsu_base::text::Lang;
use crate::interp::{Op, Outcome};
use crate::model::{self, *};
use crate::scenario;
use crate::witness::{self, Builder};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

pub struct Checked {
    pub book: Option<Book>,
    pub diags: Vec<Diag>,
    /// what each operation can be refused with; when the book has no errors
    pub report: Option<Report>,
}

pub struct Refusal {
    pub name: String,
    /// bound, key, account or hold
    pub because: &'static str,
    /// for a bound: the account kind, the account as the move writes it, the bound, the move
    pub account: Option<(usize, String, String, usize)>,
    pub spends_key: bool,
    pub example: Vec<Value>,
}

pub struct OpReport {
    pub kind: usize,
    pub op: Op,
    pub refusals: Vec<Refusal>,
}

pub struct KeyNote {
    pub kind: usize,
    /// a parameter outside the key, and a call that differs only in it, refused
    pub outside: Vec<(usize, Vec<Value>)>,
    /// a hold that ended, held again with its key: done_before
    pub again: Option<Vec<Value>>,
}

pub struct Report {
    pub ops: Vec<OpReport>,
    pub keys: Vec<KeyNote>,
}

pub fn check_file(path: &Path) -> Result<(String, Checked), String> {
    let src = ritsu_base::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let c = check_source(&src);
    Ok((src, c))
}

pub fn check_source(src: &str) -> Checked {
    let (book, mut diags) = model::load(src);
    let Some(book) = book else {
        return Checked { book: None, diags, report: None };
    };
    diags.extend(findings(&book));
    model::sort(&mut diags);
    let report = if diag::has_errors(&diags) { None } else { Some(report(&book)) };
    Checked { book: Some(book), diags, report }
}

fn bound_text(book: &Book, kind: usize, upper: bool) -> String {
    let a = &book.accounts[kind];
    let scale = book.units[a.unit].scale;
    if upper {
        format!("at most {}", format_amount(a.upper.as_ref().map(|b| b.value).unwrap_or(0), scale))
    } else {
        format!("at least {}", format_amount(a.lower.as_ref().map(|b| b.value).unwrap_or(0), scale))
    }
}

/// E012 and W101–W106.
pub fn findings(book: &Book) -> Vec<Diag> {
    let mut d = Vec::new();
    let puts = |k: usize| book.transfers.iter().any(|t| t.moves.iter().any(|m| m.to.kind == k));
    let takes = |k: usize| book.transfers.iter().any(|t| t.moves.iter().any(|m| m.from.kind == k));

    // E012: a move from an account to the same account, whatever it is called with
    for (k, t) in book.transfers.iter().enumerate() {
        for (i, m) in t.moves.iter().enumerate() {
            if m.from != m.to {
                continue;
            }
            let Some(b) = witness::same_account(book, k, i) else { continue };
            let r = book.ref_text(t, &m.from);
            d.push(
                diag::error(
                    "E012",
                    m.line,
                    m.col,
                    tr!(
                        "この移動の元と先は、どちらも {r} です。元と先がいつも同じ勘定なので、`{}` はどう呼んでも same_account で断られます",
                        "this move goes from {r} to {r}: the same account whatever it is called with, so every call of `{}` is refused with same_account",
                        t.name
                    ),
                )
                .with_ops(b.op_lines())
                .hint(tr!("動かす先を別の勘定にするか、同じ種類の勘定なら引数を変えてください", "move to another account, or to the same kind with other arguments")),
            );
        }
    }

    // W101: put into, never taken out of
    let mut stuck = vec![false; book.accounts.len()];
    for (x, a) in book.accounts.iter().enumerate() {
        if a.outside || !puts(x) || takes(x) {
            continue;
        }
        let witness = book.transfers.iter().enumerate().find_map(|(k, t)| {
            if !t.moves.iter().any(|m| m.to.kind == x) {
                return None;
            }
            let mut b = Builder::new(book);
            b.passing_call(k, &BTreeMap::new(), 0).map(|_| b)
        });
        let Some(b) = witness else { continue };
        stuck[x] = true;
        let n = &a.name;
        d.push(
            diag::warning(
                "W101",
                a.line,
                a.col,
                tr!(
                    "勘定 `{n}` へ入れる移動はありますが、`{n}` から取る移動がどこにもありません。入ったものは出ていけず、残高はたまる一方です",
                    "moves put into `{n}`, and no move takes out of it anywhere: whatever goes in stays there"
                ),
            )
            .with_ops(b.op_lines())
            .hint(tr!(
                "`{n}` から取る振替を書いてください。外の世界を表す勘定なら `outside` を付けてください",
                "write a transfer that takes out of `{n}`, or mark it `outside` if it stands for the world outside the book"
            )),
        );
    }

    // W102: what a transfer takes from has nothing that puts into it
    for (k, t) in book.transfers.iter().enumerate() {
        for (i, m) in t.moves.iter().enumerate() {
            let a = &book.accounts[m.from.kind];
            let Some(l) = &a.lower else { continue };
            if l.value < 0 || puts(m.from.kind) || matches!(m.amount, Amount::Lit(0)) {
                continue;
            }
            let Some(b) = witness::bound_refusal(book, k, i, false) else { continue };
            let (n, tn, reason) = (&a.name, &t.name, &l.refusal);
            d.push(
                diag::warning(
                    "W102",
                    m.line,
                    m.col,
                    tr!(
                        "`{n}` へ入れる移動がどこにも無いので、`{tn}` は 0 より多く動かせば、いつも {reason} で断られます",
                        "nothing in the book puts into `{n}`, so `{tn}` is refused with {reason} whenever it moves more than 0"
                    ),
                )
                .with_ops(b.op_lines())
                .hint(tr!("`{n}` へ入れる振替を書いてください", "write a transfer that puts into `{n}`")),
            );
            break;
        }
    }

    // W103, W104: one move of a transfer counts on another
    for (k, t) in book.transfers.iter().enumerate() {
        let pending = t.is_pending();
        for (ti, tm) in t.moves.iter().enumerate() {
            for (pi, pm) in t.moves.iter().enumerate() {
                if ti == pi || tm.from != pm.to {
                    continue;
                }
                let r = &tm.from;
                let ra = &book.accounts[r.kind];
                let rt = book.ref_text(t, r);
                let (tn, pn) = (ti + 1, pi + 1);
                // the lower bound: the take must not count on the put
                if let Some(l) = &ra.lower {
                    if pending || ti < pi {
                        if let Some(b) = order_witness(book, k, ti, pi, false) {
                            let reason = &l.refusal;
                            let (code, msg, hint) = if pending {
                                (
                                    "W104",
                                    tr!(
                                        "仮押さえでは、{pn} つ目の移動が {rt} へ入れる額は入ってくる仮押さえになり、{tn} つ目の移動が {rt} から取るときに数えられません。{rt} にはじめから足りるだけの残高が無ければ、{reason} で断られます",
                                        "in a hold, what move {pn} puts into {rt} is held coming in, and does not count when move {tn} takes from {rt}: unless {rt} has enough already, the hold is refused with {reason}"
                                    ),
                                    tr!(
                                        "取る分をはじめから {rt} に入れておくか、確定のあとで取る別の振替にしてください",
                                        "have what it takes in {rt} beforehand, or take it in another transfer after the hold is posted"
                                    ),
                                )
                            } else {
                                (
                                    "W103",
                                    tr!(
                                        "{tn} つ目の移動が {rt} から取るのは、{pn} つ目の移動が {rt} へ入れるより前です。そのとき {rt} が足りないと、二つの移動を合わせれば足りる場合でも {reason} で断られます",
                                        "move {tn} takes from {rt} before move {pn} puts into it: when {rt} is short at that point, the call is refused with {reason}, even when the two moves together would leave enough"
                                    ),
                                    tr!("{rt} へ入れる移動を先に書いてください", "write the move that puts into {rt} first"),
                                )
                            };
                            d.push(diag::warning(code, tm.line, tm.col, msg).with_ops(b.op_lines()).hint(hint));
                        }
                    }
                }
                // the upper bound: the put must not count on the take
                if let Some(u) = &ra.upper {
                    if pending || pi < ti {
                        if let Some(b) = order_witness(book, k, ti, pi, true) {
                            let reason = &u.refusal;
                            let (code, msg, hint) = if pending {
                                (
                                    "W104",
                                    tr!(
                                        "仮押さえでは、{tn} つ目の移動が {rt} から取る額は出ていく仮押さえになり、{pn} つ目の移動が {rt} へ入れるときに空きとして数えられません。{rt} にはじめから空きが無ければ、{reason} で断られます",
                                        "in a hold, what move {tn} takes from {rt} is held going out, and makes no room when move {pn} puts into {rt}: unless {rt} has room already, the hold is refused with {reason}"
                                    ),
                                    tr!(
                                        "{rt} の空きをはじめから作っておくか、確定のあとで入れる別の振替にしてください",
                                        "make the room in {rt} beforehand, or put it in another transfer after the hold is posted"
                                    ),
                                )
                            } else {
                                (
                                    "W103",
                                    tr!(
                                        "{pn} つ目の移動が {rt} へ入れるのは、{tn} つ目の移動が {rt} から取るより前です。そのとき {rt} に空きが無いと、二つの移動を合わせれば上限に収まる場合でも {reason} で断られます",
                                        "move {pn} puts into {rt} before move {tn} takes from it: when {rt} has no room at that point, the call is refused with {reason}, even when the two moves together would stay within its bound"
                                    ),
                                    tr!("{rt} から取る移動を先に書いてください", "write the move that takes from {rt} first"),
                                )
                            };
                            d.push(diag::warning(code, pm.line, pm.col, msg).with_ops(b.op_lines()).hint(hint));
                        }
                    }
                }
            }
        }
    }

    // W105: never used
    let mut unused_account = vec![false; book.accounts.len()];
    for (u, unit) in book.units.iter().enumerate() {
        let used = book.accounts.iter().any(|a| a.unit == u) || book.transfers.iter().any(|t| t.params.iter().any(|p| p.ty == Ty::Amount(u)));
        if !used {
            let n = &unit.name;
            d.push(diag::warning("W105", unit.line, unit.col, tr!("単位 `{n}` はどこにも使われていません", "the unit `{n}` is not used anywhere")));
        }
    }
    for (x, a) in book.accounts.iter().enumerate() {
        if !puts(x) && !takes(x) {
            unused_account[x] = true;
            let n = &a.name;
            d.push(diag::warning("W105", a.line, a.col, tr!("勘定 `{n}` を使う振替がありません", "no transfer moves into or out of the account `{n}`")));
        }
    }
    for t in &book.transfers {
        for (i, p) in t.params.iter().enumerate() {
            let in_moves = t.moves.iter().any(|m| {
                m.amount == Amount::Param(i) || m.from.args.contains(&Arg::Param(i)) || m.to.args.contains(&Arg::Param(i))
            });
            if !t.key.contains(&i) && !in_moves {
                let n = &p.name;
                d.push(diag::warning("W105", p.line, p.col, tr!("引数 `{n}` はキーにも移動にも使われていません", "the parameter `{n}` is in neither the key nor any move")));
            }
        }
    }

    // W106: a bound that never matters
    for (x, a) in book.accounts.iter().enumerate() {
        if unused_account[x] {
            continue;
        }
        let scale = book.units[a.unit].scale;
        if let Some(l) = &a.lower {
            if !takes(x) && !stuck[x] {
                let (n, v) = (&a.name, format_amount(l.value, scale));
                d.push(diag::warning(
                    "W106",
                    l.line,
                    l.col,
                    tr!("`at least {v}` は効きません。`{n}` から取る移動がどこにも無いからです", "`at least {v}` never matters: nothing takes out of `{n}`"),
                ));
            }
        }
        if let Some(u) = &a.upper {
            if !puts(x) {
                let (n, v) = (&a.name, format_amount(u.value, scale));
                d.push(diag::warning(
                    "W106",
                    u.line,
                    u.col,
                    tr!("`at most {v}` は効きません。`{n}` へ入れる移動がどこにも無いからです", "`at most {v}` never matters: nothing puts into `{n}`"),
                ));
            }
        }
    }
    d
}

/// The operations that show move `take` of `k` refused because it counts on move `put`
/// (W103, W104): the account they share has nothing (lower bound) or no room (upper), and
/// the two moves together would fit. For an upper bound, the account is tried empty with
/// more than its bound, then full with 1.
fn order_witness(book: &Book, k: usize, take: usize, put: usize, upper: bool) -> Option<Builder<'_>> {
    let t = &book.transfers[k];
    let r = &t.moves[take].from;
    let ra = &book.accounts[r.kind];
    let tries: Vec<(i128, Option<i128>)> = if upper {
        let u = ra.upper.as_ref()?.value;
        vec![(u + 1, None), (1, Some(u))]
    } else {
        vec![((1 - ra.lower.as_ref()?.value).max(1), None)]
    };
    let at = if upper { put } else { take };
    let reason = if upper { &ra.upper.as_ref()?.refusal } else { &ra.lower.as_ref()?.refusal };
    'next: for (a, fill) in tries {
        let mut fixed: BTreeMap<usize, crate::interp::Val> = BTreeMap::new();
        for i in [take, put] {
            match t.moves[i].amount {
                Amount::Param(p) => {
                    fixed.insert(p, crate::interp::Val::Amt(a));
                }
                Amount::Lit(v) if v < a => continue 'next,
                Amount::Lit(_) => {}
            }
        }
        let mut b = Builder::new(book);
        let c = b.first_call(k, &fixed);
        let x = c.account(r);
        if let Some(f) = fill {
            if !b.fill(&x, f) {
                continue;
            }
        }
        if !b.prepare(&[], &c, Some(at), 0) {
            continue;
        }
        if let Some(Outcome::Refused(rf)) = b.call(&c) {
            if &rf.reason == reason && rf.at.as_ref().is_some_and(|a| a.move_index == at && a.upper == upper) {
                return Some(b);
            }
        }
    }
    None
}

// ── the report ────────────────────────────────────────────────────────────

pub fn report(book: &Book) -> Report {
    let mut ops = Vec::new();
    let mut keys = Vec::new();
    for (k, t) in book.transfers.iter().enumerate() {
        for op in t.ops() {
            let op = Op::parse(op).unwrap();
            let mut refusals: Vec<Refusal> = Vec::new();
            match op {
                Op::Do | Op::Hold => {
                    for (i, m) in t.moves.iter().enumerate() {
                        for upper in [false, true] {
                            let (side, bound) = if upper { (&m.to, &book.accounts[m.to.kind].upper) } else { (&m.from, &book.accounts[m.from.kind].lower) };
                            let Some(bound) = bound else { continue };
                            if refusals.iter().any(|r| r.name == bound.refusal && r.account.as_ref().is_some_and(|a| a.0 == side.kind && a.2 == bound_text(book, side.kind, upper))) {
                                continue;
                            }
                            let Some(b) = witness::bound_refusal(book, k, i, upper) else { continue };
                            refusals.push(Refusal {
                                name: bound.refusal.clone(),
                                because: "bound",
                                account: Some((side.kind, book.ref_text(t, side), bound_text(book, side.kind, upper), i + 1)),
                                spends_key: true,
                                example: b.example_json(),
                            });
                        }
                    }
                    let any_bound = !refusals.is_empty();
                    if let Some((b, _)) = witness::key_conflict(book, k) {
                        refusals.push(Refusal { name: "key_conflict".into(), because: "key", account: None, spends_key: false, example: b.example_json() });
                    }
                    if any_bound {
                        if let Some(b) = witness::already_refused(book, k) {
                            refusals.push(Refusal { name: "already_refused".into(), because: "key", account: None, spends_key: false, example: b.example_json() });
                        }
                    }
                    if let Some(b) = (0..t.moves.len()).find_map(|i| witness::same_account(book, k, i)) {
                        refusals.push(Refusal { name: "same_account".into(), because: "account", account: None, spends_key: false, example: b.example_json() });
                    }
                }
                Op::Post | Op::Void => {
                    let reasons: &[&str] = if op == Op::Post {
                        &["key_conflict", "already_voided", "expired", "over_hold", "no_such_hold"]
                    } else {
                        &["already_posted", "expired", "no_such_hold"]
                    };
                    for reason in reasons {
                        if let Some(b) = witness::hold_refusal(book, k, op, reason) {
                            let because = if *reason == "key_conflict" { "key" } else { "hold" };
                            refusals.push(Refusal { name: reason.to_string(), because, account: None, spends_key: false, example: b.example_json() });
                        }
                    }
                }
            }
            ops.push(OpReport { kind: k, op, refusals });
        }
        let outside: Vec<(usize, Vec<Value>)> =
            (0..t.params.len()).filter(|i| !t.key.contains(i)).filter_map(|i| witness::key_conflict_on(book, k, i).map(|b| (i, b.example_json()))).collect();
        let again = if t.is_pending() { witness::hold_again(book, k).map(|b| b.example_json()) } else { None };
        keys.push(KeyNote { kind: k, outside, again });
    }
    Report { ops, keys }
}

/// `a `, `a、b のどれか`, `a、b、c のどれか` (what is followed by `が`); `a`, `a or b`, `a, b or c`.
fn or_list(names: &[String], lang: Lang) -> String {
    match names {
        [] => String::new(),
        [one] if lang == Lang::Ja => format!("{one} "),
        [one] => one.clone(),
        [init @ .., last] => {
            if lang == Lang::Ja {
                format!("{}、{last} のどれか", init.join("、"))
            } else {
                format!("{} or {last}", init.join(", "))
            }
        }
    }
}

fn refusal_text(r: &Refusal, lang: Lang) -> String {
    match &r.account {
        Some((_, rf, bound, _)) => {
            if lang == Lang::Ja {
                format!("{}（{rf} {bound}）", r.name)
            } else {
                format!("{} ({rf} {bound})", r.name)
            }
        }
        None => r.name.clone(),
    }
}

/// The report as `chobo check` prints it: the reasons by name, and how each key works.
pub fn render_report(book: &Book, rep: &Report, lang: Lang) -> String {
    let mut out = String::new();
    let heads: Vec<String> = rep.ops.iter().map(|o| format!("{}.{}", book.transfers[o.kind].name, o.op.name())).collect();
    let w = heads.iter().map(|h| scenario::width(h)).max().unwrap_or(0);
    for (o, head) in rep.ops.iter().zip(&heads) {
        let names: Vec<String> = o.refusals.iter().map(|r| refusal_text(r, lang)).collect();
        let list = if names.is_empty() { tr!("なし", "none").get(lang).to_string() } else { names.join(if lang == Lang::Ja { "、" } else { ", " }) };
        let label = tr!("断られうる理由", "may be refused").get(lang).to_string();
        out.push_str(&format!("  {}  {label}: {list}\n", scenario::pad(head, w)));
    }
    for kn in &rep.keys {
        let t = &book.transfers[kn.kind];
        let params: Vec<&str> = t.key.iter().map(|i| t.params[*i].name.as_str()).collect();
        let (name, ps) = (&t.name, params.join(", "));
        // `order ごとに`, `delivery と sku の組ごとに`
        let per = match params.as_slice() {
            [init @ .., last] if !init.is_empty() => format!("{} と {last} の組ごとに", init.join("、")),
            _ => format!("{ps} ごとに"),
        };
        let mut line = tr!("キー: {name} は、{per}一度だけ動きます", "key: {name} once per {ps}");
        if !kn.outside.is_empty() {
            let others: Vec<String> = kn.outside.iter().map(|(i, _)| t.params[*i].name.clone()).collect();
            let (oj, oe) = (or_list(&others, Lang::Ja), or_list(&others, Lang::En));
            line.ja.push_str(&format!("。キーが同じで {oj}が違う二度目の呼び出しは、key_conflict で断られます"));
            line.en.push_str(&format!("; a second call that differs only in {oe} is refused with key_conflict"));
        }
        if kn.again.is_some() {
            line.ja.push_str("。仮押さえが終わったあとに同じキーでもう一度押さえると、done_before が返り、何も押さえません");
            line.en.push_str("; holding again with the same key after the hold has ended answers done_before and holds nothing");
        }
        line.ja.push('。');
        out.push_str(&format!("  {}\n", line.get(lang)));
    }
    out
}

pub fn report_json(book: &Book, rep: &Report) -> Value {
    let mut entries = Vec::new();
    for o in &rep.ops {
        let t = &book.transfers[o.kind];
        let refusals: Vec<Value> = o
            .refusals
            .iter()
            .map(|r| {
                let mut v = json!({"name": r.name, "because": r.because, "spends_key": r.spends_key, "example": r.example});
                if let Some((kind, rf, bound, mv)) = &r.account {
                    v["account"] = json!(book.accounts[*kind].name);
                    v["as_written"] = json!(rf);
                    v["bound"] = json!(bound);
                    v["move"] = json!(mv);
                }
                v
            })
            .collect();
        let mut e = json!({"kind": t.name, "op": o.op.name(), "refusals": refusals});
        if matches!(o.op, Op::Do | Op::Hold) {
            if let Some(kn) = rep.keys.iter().find(|k| k.kind == o.kind) {
                let mut examples: Vec<Value> =
                    kn.outside.iter().map(|(i, ex)| json!({"about": "key_conflict", "param": t.params[*i].name, "operations": ex})).collect();
                if let Some(ex) = &kn.again {
                    examples.push(json!({"about": "done_before", "operations": ex}));
                }
                e["key"] = json!({"params": t.key.iter().map(|i| t.params[*i].name.clone()).collect::<Vec<_>>(), "examples": examples});
            }
        }
        entries.push(e);
    }
    Value::Array(entries)
}

/// The text `chobo check` prints for one file.
pub fn render(file: &str, src: &str, c: &Checked, lang: Lang) -> String {
    let mut out = String::new();
    for d in &c.diags {
        out.push_str(&d.shown(file, src, lang));
    }
    out.push_str(&render_tail(file, c, lang));
    out
}

/// What `chobo check` prints for one file after its diagnostics: the line that sums them up, and
/// the report of a book that passes.
pub fn render_tail(file: &str, c: &Checked, lang: Lang) -> String {
    let mut out = diag::summary(file, &c.diags, lang);
    out.push('\n');
    if let (Some(b), Some(r)) = (&c.book, &c.report) {
        out.push_str(&render_report(b, r, lang));
    }
    out
}

pub fn to_json(file: &str, src: &str, c: &Checked, lang: Lang) -> Value {
    json!({
        "file": file,
        "ok": !diag::has_errors(&c.diags),
        "diagnostics": c.diags.iter().map(|d| d.json_in(file, src, lang)).collect::<Vec<_>>(),
        "report": match (&c.book, &c.report) {
            (Some(b), Some(r)) => report_json(b, r),
            _ => Value::Null,
        },
    })
}

