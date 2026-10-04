//! The scenarios chobo writes for a book (DESIGN 6, PLAN B7): every bound just before, at
//! and past its limit, each key used twice, every way a hold ends, a refusal in the middle of
//! a transfer that moves more than once, and two callers after the last of something.
//!
//! Each one is built by calling the book in the reference interpreter (`witness::Builder`),
//! and kept only when the interpreter answers what its name says.

use ritsu_base::text::Text;
use crate::interp::{Call, Op, Outcome, Val};
use crate::model::*;
use crate::scenario::{self, Scenario, Step};
use crate::witness::{self, Builder};
use std::collections::BTreeMap;

/// The longest expiry in the book, and a minute more: a `pass` that ends every hold made.
pub fn pass_secs(book: &Book) -> u64 {
    book.transfers.iter().filter_map(|t| if let Some(Expiry::After(s)) = t.pending { Some(s) } else { None }).max().unwrap_or(0) + 60
}

fn op_name(t: &TransferKind) -> &'static str {
    if t.is_pending() { "hold" } else { "do" }
}

/// `売上.do`, or `売上.do move 2` when the kind moves more than once; in Japanese, with the
/// particle after it: `売上.do が`, `売上.do の 2 つ目の移動が` (a name is set off by a space,
/// a word is not).
fn at_move(t: &TransferKind, i: usize, particle: &str) -> Text {
    let (name, op, n) = (&t.name, op_name(t), i + 1);
    if t.moves.len() > 1 {
        tr!("{name}.{op} の {n} つ目の移動{particle}", "{name}.{op} move {n}")
    } else {
        Text { ja: format!("{name}.{op} {particle}"), en: format!("{name}.{op}") }
    }
}

/// A scenario and its title in both languages: the English title is its name.
type Titled = Vec<(Scenario, Text)>;

fn push(out: &mut Titled, b: &Builder, title: Text) {
    out.push((b.scenario(title.en.clone()), title));
}

/// The account of move `i` (its `from`, or its `to` when `to_side`) appears in no other move.
fn alone(t: &TransferKind, i: usize, to_side: bool) -> bool {
    let r = if to_side { &t.moves[i].to } else { &t.moves[i].from };
    t.moves.iter().enumerate().all(|(j, m)| j == i || (m.from != *r && m.to != *r))
}

pub fn generate(book: &Book) -> Vec<Scenario> {
    generate_titled(book).into_iter().map(|(s, _)| s).collect()
}

/// The scenarios with a title in each language: the English one is the scenario's name, and the
/// Japanese one is what `chobo doc --lang ja` shows.
pub fn generate_titled(book: &Book) -> Vec<(Scenario, Text)> {
    let mut out: Titled = Vec::new();
    for (k, _) in book.transfers.iter().enumerate() {
        bounds(book, k, &mut out);
    }
    for (k, _) in book.transfers.iter().enumerate() {
        keys(book, k, &mut out);
    }
    for (k, t) in book.transfers.iter().enumerate() {
        if t.is_pending() {
            holds(book, k, &mut out);
        }
    }
    for (k, t) in book.transfers.iter().enumerate() {
        if matches!(t.pending, Some(Expiry::After(_))) {
            passes(book, k, &mut out);
        }
    }
    for (k, t) in book.transfers.iter().enumerate() {
        if t.moves.len() > 1 {
            moves(book, k, &mut out);
        }
    }
    for (k, _) in book.transfers.iter().enumerate() {
        together(book, k, &mut out);
    }
    for (k, _) in book.transfers.iter().enumerate() {
        same(book, k, &mut out);
    }
    out
}

fn last_is(b: &Builder, want: &Outcome) -> bool {
    b.last() == Some(want)
}

fn refused_at(b: &Builder, reason: &str, i: usize, upper: bool) -> bool {
    matches!(b.last(), Some(Outcome::Refused(r)) if r.reason == reason && r.at.as_ref().is_some_and(|a| a.move_index == i && a.upper == upper))
}

/// Before, at and past each bound.
fn bounds(book: &Book, k: usize, out: &mut Titled) {
    let t = &book.transfers[k];
    for (i, m) in t.moves.iter().enumerate() {
        // the lower bound of what the move takes from
        if let Some(l) = &book.accounts[m.from.kind].lower {
            if alone(t, i, false) {
                let lv = l.value;
                let r = book.ref_text(t, &m.from);
                for delta in [1i128, 0, -1] {
                    let mut b = Builder::new(book);
                    let mut fixed = BTreeMap::new();
                    let target = match m.amount {
                        Amount::Param(p) => {
                            let target = lv.max(0) + 2;
                            let a = target - (lv + delta);
                            if a < 0 {
                                continue;
                            }
                            fixed.insert(p, Val::Amt(a));
                            target
                        }
                        Amount::Lit(a) => lv + delta + a,
                    };
                    if target < 0 {
                        continue;
                    }
                    let c = b.first_call(k, &fixed);
                    let x = c.account(&m.from);
                    if !b.fill(&x, target) || !b.prepare(&[], &c, Some(i), 0) || b.call(&c).is_none() {
                        continue;
                    }
                    // the numbers as the book writes them, in the unit's decimal places
                    let scale = book.unit_of(m.from.kind).scale;
                    let (left, lv) = (format_amount(lv + delta, scale), format_amount(lv, scale));
                    let (ga, wa) = (at_move(t, i, "が"), at_move(t, i, "は"));
                    let (gj, wj, ae) = (&ga.ja, &wa.ja, &ga.en);
                    let (ok, title) = match delta {
                        1 => (
                            last_is(&b, &Outcome::Done),
                            tr!("境界: {gj} {r} を {left} まで減らす（`at least {lv}` の 1 つ手前）", "bound: {ae} takes {r} to {left}, one above `at least {lv}`"),
                        ),
                        0 => (last_is(&b, &Outcome::Done), tr!("境界: {gj} {r} をちょうど {lv} まで減らす（`at least {lv}` ちょうど）", "bound: {ae} takes {r} to exactly {lv}, its `at least {lv}`")),
                        _ => (
                            refused_at(&b, &l.refusal, i, false),
                            tr!("境界: {wj} {r} を {left} まで減らすので断られる（`at least {lv}` を割る）", "bound: {ae} would take {r} to {left}, below `at least {lv}`"),
                        ),
                    };
                    if ok {
                        push(out, &b, title);
                    }
                }
            }
        }
        // the upper bound of what the move puts into
        if let Some(u) = &book.accounts[m.to.kind].upper {
            if alone(t, i, true) {
                let uv = u.value;
                let r = book.ref_text(t, &m.to);
                for delta in [-1i128, 0, 1] {
                    let mut b = Builder::new(book);
                    let mut fixed = BTreeMap::new();
                    let fill = match m.amount {
                        Amount::Param(p) => {
                            let (fill, a) = if uv >= 2 { (uv - 2, 2 + delta) } else { (0, uv + delta) };
                            if a < 0 {
                                continue;
                            }
                            fixed.insert(p, Val::Amt(a));
                            fill
                        }
                        Amount::Lit(a) => uv + delta - a,
                    };
                    if fill < 0 {
                        continue;
                    }
                    let c = b.first_call(k, &fixed);
                    let x = c.account(&m.to);
                    if !b.fill(&x, fill) || !b.prepare(&[], &c, Some(i), 0) || b.call(&c).is_none() {
                        continue;
                    }
                    let scale = book.unit_of(m.to.kind).scale;
                    let (to, uv) = (format_amount(uv + delta, scale), format_amount(uv, scale));
                    let (ga, wa) = (at_move(t, i, "が"), at_move(t, i, "は"));
                    let (gj, wj, ae) = (&ga.ja, &wa.ja, &ga.en);
                    let (ok, title) = match delta {
                        -1 => (
                            last_is(&b, &Outcome::Done),
                            tr!("境界: {gj} {r} を {to} まで増やす（`at most {uv}` の 1 つ手前）", "bound: {ae} fills {r} to {to}, one below `at most {uv}`"),
                        ),
                        0 => (last_is(&b, &Outcome::Done), tr!("境界: {gj} {r} をちょうど {uv} まで増やす（`at most {uv}` ちょうど）", "bound: {ae} fills {r} to exactly {uv}, its `at most {uv}`")),
                        _ => (
                            refused_at(&b, &u.refusal, i, true),
                            tr!("境界: {wj} {r} を {to} まで増やすので断られる（`at most {uv}` を超える）", "bound: {ae} would fill {r} to {to}, past `at most {uv}`"),
                        ),
                    };
                    if ok {
                        push(out, &b, title);
                    }
                }
            }
        }
    }
}

/// The same key twice: with the same arguments, with others, and after a bound refused it.
fn keys(book: &Book, k: usize, out: &mut Titled) {
    let t = &book.transfers[k];
    let op = op_name(t);
    {
        let mut b = Builder::new(book);
        if let Some(c) = b.passing_call(k, &BTreeMap::new(), 0) {
            if b.call(&c) == Some(Outcome::DoneBefore) {
                let tn = &t.name;
                push(out, &b, tr!("キー: {tn}.{op} を同じ引数で二度呼ぶ", "key: {tn}.{op} twice with the same arguments"));
            }
        }
    }
    if let Some((b, param)) = witness::key_conflict(book, k) {
        let (tn, pn) = (&t.name, &t.params[param].name);
        push(out, &b, tr!("キー: {tn}.{op} を {pn} だけ変えてもう一度呼ぶ", "key: {tn}.{op} again with another {pn}"));
    }
    // refused by a bound, then the same call again, then once more after the account has
    // enough: the key stays spent
    'found: for (i, m) in t.moves.iter().enumerate() {
        for upper in [false, true] {
            let Some(mut b) = witness::bound_refusal(book, k, i, upper) else { continue };
            let Some(Step::Call(c)) = b.steps.last().cloned() else { continue };
            let reason = b.last().and_then(|o| o.reason()).unwrap_or_default().to_string();
            if !matches!(b.call(&c), Some(Outcome::Refused(r)) if r.reason == "already_refused") {
                continue;
            }
            if !upper {
                let x = c.account(&m.from);
                let l = book.accounts[m.from.kind].lower.as_ref().map(|l| l.value).unwrap_or(0);
                let bal = b.state.balance(&x);
                let need = l + c.amount(m) - (bal.posted - bal.held_out);
                if !b.put(&x, need, 0) {
                    continue;
                }
                if !matches!(b.call(&c), Some(Outcome::Refused(r)) if r.reason == "already_refused") {
                    continue;
                }
                let xr = book.ref_text(t, &m.from);
                let tn = &t.name;
                push(
                    out,
                    &b,
                    tr!(
                        "キー: {tn}.{op} が {reason} で断られたあと、同じ引数ですぐにもう一度呼び、{xr} が足りるようになってからもう一度呼ぶ",
                        "key: {tn}.{op} refused with {reason}, then again, and again once {xr} has enough"
                    ),
                );
            } else {
                let tn = &t.name;
                push(out, &b, tr!("キー: {tn}.{op} が {reason} で断られたあと、同じ引数でもう一度呼ぶ", "key: {tn}.{op} refused with {reason}, then again"));
            }
            break 'found;
        }
    }
}

/// Every way a hold ends, and the calls that come too late or too often.
fn holds(book: &Book, k: usize, out: &mut Titled) {
    let t = &book.transfers[k];
    let name = &t.name;
    let try_one = |out: &mut Titled, label: Text, f: &dyn Fn(&mut Builder, &Call) -> Option<bool>| {
        let mut b = Builder::new(book);
        let Some(c) = witness::held(&mut b, k) else { return };
        if f(&mut b, &c) == Some(true) {
            push(out, &b, label);
        }
    };
    let is = |o: Option<Outcome>, want: &str| o.is_some_and(|o| match &o {
        Outcome::Refused(r) => r.reason == want,
        Outcome::Done => want == "done",
        Outcome::DoneBefore => want == "done_before",
    });
    try_one(out, tr!("仮押さえ: {name} を全額で確定する", "hold: {name} posted in full"), &|b, c| {
        let p = b.follow(c, Op::Post, None);
        Some(is(b.call(&p), "done"))
    });
    if !t.amount_params().is_empty() {
        try_one(out, tr!("仮押さえ: {name} を一部だけ確定する", "hold: {name} posted in part"), &|b, c| {
            let p = b.follow(c, Op::Post, witness::amounts_of(book, c, |_| 1));
            Some(is(b.call(&p), "done"))
        });
    }
    try_one(out, tr!("仮押さえ: {name} を取り消す", "hold: {name} voided"), &|b, c| {
        let v = b.follow(c, Op::Void, None);
        Some(is(b.call(&v), "done"))
    });
    try_one(out, tr!("仮押さえ: {name} を確定してから取り消そうとする", "hold: {name} posted, then voided"), &|b, c| {
        let p = b.follow(c, Op::Post, None);
        let v = b.follow(c, Op::Void, None);
        Some(is(b.call(&p), "done") && is(b.call(&v), "already_posted"))
    });
    try_one(out, tr!("仮押さえ: {name} を取り消してから確定しようとする", "hold: {name} voided, then posted"), &|b, c| {
        let v = b.follow(c, Op::Void, None);
        let p = b.follow(c, Op::Post, None);
        Some(is(b.call(&v), "done") && is(b.call(&p), "already_voided"))
    });
    if !t.amount_params().is_empty() {
        try_one(out, tr!("仮押さえ: {name} を押さえた額より多く確定しようとし、押さえた額で確定する", "hold: {name} posted for more than it holds, then for what it holds"), &|b, c| {
            let over = b.follow(c, Op::Post, witness::amounts_of(book, c, |h| h + 1));
            let right = b.follow(c, Op::Post, witness::amounts_of(book, c, |h| h));
            Some(is(b.call(&over), "over_hold") && is(b.call(&right), "done"))
        });
    }
    {
        // the hold that goes through, found on a copy; the post that comes before it changes nothing
        let mut trial = Builder::new(book);
        if let Some(c) = trial.passing_call(k, &BTreeMap::new(), 0) {
            let mut b = Builder::new(book);
            let p = b.follow(&c, Op::Post, None);
            if is(b.call(&p), "no_such_hold") && b.prepare(&[], &c, None, 0) && is(b.call(&c), "done") && is(b.call(&p), "done") {
                push(out, &b, tr!("仮押さえ: {name} を押さえる前に確定しようとし、押さえてから確定する", "hold: {name} posted before it is held, then held and posted"));
            }
        }
    }
    try_one(out, tr!("仮押さえ: {name} を確定したあと、同じ額でもう一度、違う額でもう一度確定する", "hold: {name} posted twice, with the same amounts and with others"), &|b, c| {
        let full = b.follow(c, Op::Post, None);
        let mut ok = is(b.call(&full), "done") && is(b.call(&full), "done_before");
        if let Some(same) = witness::amounts_of(book, c, |h| h) {
            let same = b.follow(c, Op::Post, Some(same));
            let other = b.follow(c, Op::Post, witness::amounts_of(book, c, |h| h - 1));
            ok = ok && is(b.call(&same), "done_before") && is(b.call(&other), "key_conflict");
        }
        Some(ok)
    });
}

/// A hold that expires: posting and voiding it after, and holding again with its key.
fn passes(book: &Book, k: usize, out: &mut Titled) {
    let t = &book.transfers[k];
    let secs = pass_secs(book);
    {
        let mut b = Builder::new(book);
        if let Some(c) = witness::held(&mut b, k) {
            b.pass(secs);
            let p = b.follow(&c, Op::Post, None);
            let v = b.follow(&c, Op::Void, None);
            let expired = |o: Option<Outcome>| matches!(o, Some(Outcome::Refused(r)) if r.reason == "expired");
            if expired(b.call(&p)) && expired(b.call(&v)) {
                let tn = &t.name;
                push(out, &b, tr!("期限切れ: {tn} が期限切れになってから、確定と取消を試す", "pass: {tn} expires, then is posted and voided"));
            }
        }
    }
    {
        let mut b = Builder::new(book);
        if let Some(c) = witness::held(&mut b, k) {
            b.pass(secs);
            if b.call(&c) == Some(Outcome::DoneBefore) {
                let tn = &t.name;
                push(out, &b, tr!("期限切れ: {tn} が期限切れになってから、同じキーで押さえ直す", "pass: {tn} held again with the same key after it expired"));
            }
        }
    }
}

/// A transfer that moves more than once, refused at one move after the moves before it
/// went: nothing of it stays.
fn moves(book: &Book, k: usize, out: &mut Titled) {
    let t = &book.transfers[k];
    for (i, m) in t.moves.iter().enumerate() {
        for upper in [false, true] {
            let has = if upper { book.accounts[m.to.kind].upper.is_some() } else { book.accounts[m.from.kind].lower.is_some() };
            if !has {
                continue;
            }
            let Some(b) = witness::bound_refusal(book, k, i, upper) else { continue };
            // the moves before it went through in the trial (the account it is refused on
            // had what the moves before took into account), and the call changed nothing
            let Some(Step::Call(_)) = b.steps.last() else { continue };
            let mut before = b.clone();
            before.steps.pop();
            before.outs.pop();
            let mut st = b.state.clone();
            st.keys.clear();
            let mut st0 = before.state.clone();
            st0.keys.clear();
            if st != st0 {
                continue;
            }
            let x = book.ref_text(t, if upper { &m.to } else { &m.from });
            let (tn, op, n) = (&t.name, op_name(t), i + 1);
            push(out, &b, tr!("移動: {tn}.{op} が {n} つ目の移動（{x}）で断られ、どの移動も行われない", "moves: {tn}.{op} refused at move {n} ({x}), and no move is made"));
            break;
        }
    }
}

/// Two callers after the last of something, at the same time.
fn together(book: &Book, k: usize, out: &mut Titled) {
    let t = &book.transfers[k];
    for (i, m) in t.moves.iter().enumerate() {
        for upper in [false, true] {
            let r = if upper { &m.to } else { &m.from };
            let bound = if upper { book.accounts[r.kind].upper.as_ref() } else { book.accounts[r.kind].lower.as_ref() };
            let Some(bound) = bound else { continue };
            if !alone(t, i, upper) {
                continue;
            }
            // the two calls share the account and differ in the key
            let shared: Vec<usize> = r.args.iter().filter_map(|a| if let Arg::Param(p) = a { Some(*p) } else { None }).collect();
            if t.key.iter().all(|p| shared.contains(p)) {
                continue;
            }
            let mut b = Builder::new(book);
            let mut fixed = BTreeMap::new();
            let a = match m.amount {
                Amount::Param(p) => {
                    let a = if upper { 1 } else { (-bound.value).max(1) };
                    fixed.insert(p, Val::Amt(a));
                    a
                }
                Amount::Lit(a) => a,
            };
            let c1 = b.first_call(k, &fixed);
            let mut fixed2 = fixed.clone();
            for p in &shared {
                if let Some(v) = &c1.args[*p] {
                    fixed2.insert(*p, v.clone());
                }
            }
            let c2 = b.first_call(k, &fixed2);
            let x = c1.account(r);
            // room for one call, and not for two
            let ok = if upper {
                bound.value - a >= 0 && b.fill(&x, bound.value - a)
            } else {
                let target = bound.value + a;
                let avail0 = -bound.value;
                (target > 0 && b.fill(&x, target)) || (target <= 0 && avail0 >= a && avail0 < 2 * a)
            };
            if !ok || !b.prepare(&[], &c1, Some(i), 0) || !b.prepare(std::slice::from_ref(&c1), &c2, Some(i), 0) {
                continue;
            }
            b.together(vec![vec![c1.clone()], vec![c2.clone()]]);
            let s = b.scenario(String::new());
            let Ok(runs) = scenario::run(book, &s) else { continue };
            let mut ends: Vec<(String, String)> = runs
                .iter()
                .filter_map(|run| match run.steps.last() {
                    Some(scenario::StepOut::Together(os)) => Some((os[0][0].result().to_string() + os[0][0].reason().unwrap_or(""), os[1][0].result().to_string() + os[1][0].reason().unwrap_or(""))),
                    _ => None,
                })
                .collect();
            ends.sort();
            ends.dedup();
            let refused = format!("refused{}", bound.refusal);
            if ends != vec![("done".to_string(), refused.clone()), (refused, "done".to_string())] {
                continue;
            }
            let xr = book.ref_text(t, r);
            let (tn, op) = (&t.name, op_name(t));
            let a = format_amount(a, book.unit_of(r.kind).scale);
            let title = if upper {
                tr!("同時: 二つの呼び出し元が {tn}.{op} で {xr} の最後の空き {a} を取り合う", "together: two callers fill the last {a} of room in {xr} with {tn}.{op}")
            } else {
                tr!("同時: 二つの呼び出し元が {tn}.{op} で {xr} の最後の {a} を取り合う", "together: two callers take the last {a} of {xr} with {tn}.{op}")
            };
            out.push((Scenario { name: title.en.clone(), steps: s.steps }, title));
        }
    }
}

/// A move from an account to itself.
fn same(book: &Book, k: usize, out: &mut Titled) {
    let t = &book.transfers[k];
    for (i, m) in t.moves.iter().enumerate() {
        if m.from.kind != m.to.kind {
            continue;
        }
        if let Some(b) = witness::same_account(book, k, i) {
            let (r, r2) = (book.ref_text(t, &m.from), book.ref_text(t, &m.to));
            let at = at_move(t, i, "が");
            let (gj, ae) = (&at.ja, &at.en);
            push(out, &b, tr!("同じ勘定: {gj} {r} から {r2} へ動かそうとし、元と先が同じ勘定になる", "same_account: {ae} moves {r} to {r2}, the same account"));
            break;
        }
    }
}
