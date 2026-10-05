//! `chobo doc`: the book as the people who keep the accounts and run the operations read it
//! (DESIGN 7). The accounts and their bounds; how things move between them; each kind of transfer
//! with its moves, its key, the life of its holds, and what each operation can be refused with,
//! with the operations that get there; and the scenarios chobo makes from the book, with the
//! balances after every step.
//!
//! `markdown` writes Mermaid charts, which GitHub draws in a pull request. `html` writes one page
//! with charts chobo draws itself (`draw`), and a scenario a reader can step through. Neither
//! says how PostgreSQL or TigerBeetle keep the bounds: the reader is not the one who runs them.
//!
//! Every number is written in its unit's decimal places (`12.50` in a unit of scale 2), as the
//! book writes its literals, not in the smallest step a target stores.

use crate::check::Report;
use crate::diag::{Diag, Show};
use ritsu_base::text::{Lang, Text};
use crate::interp::{AccountId, At, Bal, Call, HoldState, Op, Outcome, State, Val};
use crate::model::*;
use crate::scenario::{self, Step, StepOut};
use serde_json::{Value, json};

/// What a page is drawn from: a book that passed the check, its report, and its warnings.
pub struct Input<'a> {
    pub book: &'a Book,
    /// the book's path as the page names it
    pub file: &'a str,
    pub src: &'a str,
    /// the warnings of the check (a book with errors is not drawn)
    pub diags: &'a [Diag],
    pub report: &'a Report,
    pub lang: Lang,
}

// ── numbers and names ─────────────────────────────────────────────────────

/// An amount in `unit`, with its decimal places.
pub fn amount(book: &Book, unit: usize, v: i128) -> String {
    format_amount(v, book.units[unit].scale)
}

/// An amount on an account of kind `kind`.
fn on(book: &Book, kind: usize, v: i128) -> String {
    amount(book, book.accounts[kind].unit, v)
}

/// `30 minutes`, `30 分`.
pub fn duration(secs: u64, lang: Lang) -> String {
    if lang == Lang::En {
        return scenario::format_duration(secs);
    }
    for (per, word) in [(86400, "日"), (3600, "時間"), (60, "分")] {
        if secs > 0 && secs.is_multiple_of(per) {
            return format!("{} {word}", secs / per);
        }
    }
    format!("{secs} 秒")
}

/// An account kind with its parameters: `stock(sku)`.
pub fn kind_text(a: &AccountKind) -> String {
    if a.params.is_empty() { a.name.clone() } else { format!("{}({})", a.name, a.params.join(", ")) }
}

fn arg_text(book: &Book, t: &TransferKind, i: usize, v: &Val) -> String {
    match (t.params[i].ty, v) {
        (Ty::Amount(u), Val::Amt(a)) => amount(book, u, *a),
        _ => v.text(),
    }
}

/// A call as a person reads it, its amounts in their units: `refund.hold(request: request-1, order: order-2, amount: 0.02)`.
pub fn call_text(book: &Book, c: &Call) -> String {
    let t = &book.transfers[c.kind];
    let mut parts: Vec<String> = Vec::new();
    for (i, p) in t.params.iter().enumerate() {
        if let Some(v) = &c.args[i] {
            parts.push(format!("{}: {}", p.name, arg_text(book, t, i, v)));
        }
    }
    if let Some(a) = &c.amounts {
        for (i, v) in a {
            parts.push(format!("{}: {}", t.params[*i].name, arg_text(book, t, *i, &Val::Amt(*v))));
        }
    }
    format!("{}.{}({})", t.name, c.op.name(), parts.join(", "))
}

/// Where a bound refused a call: the move, the amount, and the balance it found.
pub fn at_text(book: &Book, at: &At) -> Text {
    let n = at.move_index + 1;
    let acct = at.account.text(book);
    let k = at.account.kind;
    let (a, p, o, i) = (on(book, k, at.amount), on(book, k, at.bal.posted), on(book, k, at.bal.held_out), on(book, k, at.bal.held_in));
    if !at.upper {
        let mut t = tr!("{n} つ目の移動が {acct} から {a} を取ろうとしたときの残高は、確定 {p}、出ていく仮押さえ {o}", "move {n} takes {a} from {acct}: posted {p}, held out {o}");
        if at.bal.held_in != 0 {
            t.ja.push_str(&format!("、入ってくる仮押さえ {i}"));
            t.en.push_str(&format!(", held in {i}"));
        }
        t
    } else {
        let mut t = tr!("{n} つ目の移動が {acct} へ {a} を入れようとしたときの残高は、確定 {p}、入ってくる仮押さえ {i}", "move {n} puts {a} into {acct}: posted {p}, held in {i}");
        if at.bal.held_out != 0 {
            t.ja.push_str(&format!("、出ていく仮押さえ {o}"));
            t.en.push_str(&format!(", held out {o}"));
        }
        t
    }
}

/// What an operation answered: `done`, `refused: out_of_stock`.
pub fn outcome_text(o: &Outcome, lang: Lang) -> String {
    scenario::outcome_text(o).get(lang).to_string()
}

fn detail(book: &Book, o: &Outcome, lang: Lang) -> Option<String> {
    match o {
        Outcome::Refused(r) => r.at.as_ref().map(|a| at_text(book, a).get(lang).to_string()),
        _ => None,
    }
}

/// A balance: what is posted, and what is held going out or coming in when there is any.
pub fn balance_text(book: &Book, kind: usize, b: Bal, lang: Lang) -> String {
    let p = on(book, kind, b.posted);
    let mut held: Vec<String> = Vec::new();
    if b.held_out != 0 {
        let v = on(book, kind, b.held_out);
        held.push(tr!("出ていく仮押さえ {v}", "held out {v}").get(lang).to_string());
    }
    if b.held_in != 0 {
        let v = on(book, kind, b.held_in);
        held.push(tr!("入ってくる仮押さえ {v}", "held in {v}").get(lang).to_string());
    }
    match (held.is_empty(), lang) {
        (true, _) => p,
        (false, Lang::Ja) => format!("{p}（{}）", held.join("、")),
        (false, Lang::En) => format!("{p} ({})", held.join(", ")),
    }
}

/// `a`, `a と b`, `a、b と c`; `a`, `a and b`, `a, b and c`.
fn and_list(items: &[String], lang: Lang) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => match lang {
            Lang::Ja => format!("{} と {last}", init.join("、")),
            Lang::En => format!("{} and {last}", init.join(", ")),
        },
    }
}

/// `a `, `a、b のどれか` (what is followed by `が`); `a`, `a or b`.
fn or_list(items: &[String], lang: Lang) -> String {
    match items {
        [] => String::new(),
        [one] if lang == Lang::Ja => format!("{one} "),
        [one] => one.clone(),
        [init @ .., last] => match lang {
            Lang::Ja => format!("{}、{last} のどれか", init.join("、")),
            Lang::En => format!("{} or {last}", init.join(", ")),
        },
    }
}

fn code(s: &str) -> String {
    format!("`{s}`")
}

// ── what the page says about the book ─────────────────────────────────────

/// The bounds of an account kind, one line each, or that it is outside the book.
pub fn bounds_lines(book: &Book, k: usize, lang: Lang) -> Vec<String> {
    let a = &book.accounts[k];
    if a.outside {
        return vec![tr!("外の勘定。境界は無く、マイナスにもなる", "outside the book: no bounds, and it may go below 0").get(lang).to_string()];
    }
    let mut out = Vec::new();
    if let Some(l) = &a.lower {
        let (v, r) = (on(book, k, l.value), code(&l.refusal));
        out.push(tr!("{v} 以上。下回る振替は {r} で拒否される", "at least {v}; a transfer that would go below is refused with {r}").get(lang).to_string());
    }
    if let Some(u) = &a.upper {
        let (v, r) = (on(book, k, u.value), code(&u.refusal));
        out.push(tr!("{v} 以下。超える振替は {r} で拒否される", "at most {v}; a transfer that would go above is refused with {r}").get(lang).to_string());
    }
    out
}

/// One move as a sentence: `` `qty` from `suppliers` to `stock(sku)` ``.
pub fn move_text(book: &Book, t: &TransferKind, m: &Move, lang: Lang) -> String {
    let (a, f, to) = (code(&book.amount_text(t, m)), code(&book.ref_text(t, &m.from)), code(&book.ref_text(t, &m.to)));
    tr!("{f} から {to} へ {a}", "{a} from {f} to {to}").get(lang).to_string()
}

/// What a reason of refusal means for an operation of `t`, in a few words.
pub fn reason_meaning(book: &Book, t: &TransferKind, op: Op, r: &crate::check::Refusal, lang: Lang) -> String {
    if let Some((kind, written, bound, mv)) = &r.account {
        let a = &book.accounts[*kind];
        let w = code(written);
        let n = *mv;
        let several = t.moves.len() > 1;
        let text = if bound.starts_with("at most") {
            let v = on(book, *kind, a.upper.as_ref().map(|u| u.value).unwrap_or(0));
            if several { tr!("{n} つ目の移動で {w} が {v} を超える", "move {n} would take {w} above {v}") } else { tr!("{w} が {v} を超える", "{w} would go above {v}") }
        } else {
            let v = on(book, *kind, a.lower.as_ref().map(|l| l.value).unwrap_or(0));
            if several { tr!("{n} つ目の移動で {w} が {v} を下回る", "move {n} would take {w} below {v}") } else { tr!("{w} が {v} を下回る", "{w} would go below {v}") }
        };
        return text.get(lang).to_string();
    }
    let ps: Vec<String> = t.key.iter().map(|i| code(&t.params[*i].name)).collect();
    let key = and_list(&ps, lang);
    let text = match (r.name.as_str(), op) {
        ("key_conflict", Op::Post) => tr!("仮押さえは、違う額で確定済み", "the hold was posted before, for other amounts"),
        ("key_conflict", _) => tr!("{key} が同じで、ほかの引数が違う呼び出しが、前に済んでいる", "a call with the same {key} and other arguments came before"),
        ("already_refused", _) => tr!(
            "{key} が同じ呼び出しが、前に境界で拒否されている。境界で拒否されたキーは、あとで足りるようになっても通らない",
            "a call with the same {key} was refused by a bound before; a key a bound refused stays refused, even once there is enough"
        ),
        ("same_account", _) => tr!("移動の元と先が同じ勘定になる", "a move would go from an account to itself"),
        ("no_such_hold", _) => tr!("その {key} の仮押さえが無い", "there is no hold with that {key}"),
        ("already_posted", _) => tr!("仮押さえはもう確定している", "the hold is posted already"),
        ("already_voided", _) => tr!("仮押さえはもう取り消されている", "the hold is voided already"),
        ("expired", _) => tr!("仮押さえの期限が切れている", "the hold has expired"),
        ("over_hold", _) => tr!("押さえた額より多く確定しようとした", "more than the hold holds"),
        (other, _) => Text::same(other.to_string()),
    };
    text.get(lang).to_string()
}

// ── the operations of an example or a scenario ────────────────────────────

/// One call of a `together` as a page shows it: the call, what it answered, where a bound refused
/// it, and whether it was refused.
pub type CallView = (String, String, Option<String>, bool);

/// One step as a page shows it.
#[derive(Clone, Debug)]
pub struct StepView {
    /// the call, `pass 31 minutes`, or `together`
    pub label: String,
    /// what it answered, or which holds expired
    pub result: String,
    /// where a bound refused it
    pub detail: Option<String>,
    pub refused: bool,
    /// for `together`: each caller's calls, with their answers and details
    pub callers: Vec<Vec<CallView>>,
    /// the transfer kinds it called, and whether each went through
    pub kinds: Vec<(usize, bool)>,
}

fn call_view(book: &Book, c: &Call, o: &Outcome, lang: Lang) -> CallView {
    (call_text(book, c), outcome_text(o, lang), detail(book, o, lang), matches!(o, Outcome::Refused(_)))
}

pub fn step_view(book: &Book, st: &Step, out: &StepOut, lang: Lang) -> StepView {
    match (st, out) {
        (Step::Call(c), StepOut::Call(o)) => {
            let (label, result, detail, refused) = call_view(book, c, o, lang);
            StepView { label, result, detail, refused, callers: vec![], kinds: vec![(c.kind, !refused)] }
        }
        (Step::Pass(secs, _), StepOut::Pass(e)) => {
            let names: Vec<String> = e.iter().map(|(k, key)| scenario::hold_text(book, *k, key)).collect();
            let result = if names.is_empty() {
                String::new()
            } else {
                let names = names.join(", ");
                tr!("{names} が期限切れ", "{names} expired").get(lang).to_string()
            };
            let d = duration(*secs, lang);
            StepView { label: format!("pass {}", if lang == Lang::Ja { d } else { scenario::format_duration(*secs) }), result, detail: None, refused: false, callers: vec![], kinds: e.iter().map(|(k, _)| (*k, true)).collect() }
        }
        (Step::Together(cs), StepOut::Together(os)) => {
            let callers: Vec<Vec<CallView>> = cs.iter().zip(os).map(|(cc, oo)| cc.iter().zip(oo).map(|(c, o)| call_view(book, c, o, lang)).collect()).collect();
            let mut kinds = Vec::new();
            for (cc, oo) in cs.iter().zip(os) {
                for (c, o) in cc.iter().zip(oo) {
                    kinds.push((c.kind, !matches!(o, Outcome::Refused(_))));
                }
            }
            StepView { label: "together".into(), result: String::new(), detail: None, refused: false, callers, kinds }
        }
        _ => StepView { label: String::new(), result: String::new(), detail: None, refused: false, callers: vec![], kinds: vec![] },
    }
}

/// The lines of an example: each operation, numbered, with what it answered.
pub fn example_lines(book: &Book, example: &[Value], lang: Lang) -> Vec<String> {
    let Ok(s) = scenario::from_json(book, &json!({"steps": example})) else { return vec![] };
    let Ok(runs) = scenario::run(book, &s) else { return vec![] };
    let Some(run) = runs.first() else { return vec![] };
    let mut rows: Vec<(String, String)> = Vec::new();
    for (i, (st, out)) in s.steps.iter().zip(&run.steps).enumerate() {
        let v = step_view(book, st, out, lang);
        let n = i + 1;
        if v.callers.is_empty() {
            let mut r = v.result.clone();
            if let Some(d) = &v.detail {
                r = if lang == Lang::Ja { format!("{r}（{d}）") } else { format!("{r} ({d})") };
            }
            rows.push((format!("{n:>2}  {}", v.label), r));
        } else {
            rows.push((format!("{n:>2}  together"), String::new()));
            for (ci, calls) in v.callers.iter().enumerate() {
                for (call, result, _, _) in calls {
                    let who = tr!("呼び出し元 {}", "caller {}", ci + 1);
                    rows.push((format!("      {}: {call}", who.get(lang)), result.clone()));
                }
            }
        }
    }
    let w = rows.iter().map(|(a, _)| scenario::width(a)).max().unwrap_or(0);
    rows.iter().map(|(a, b)| if b.is_empty() { a.clone() } else { format!("{}  {b}", scenario::pad(a, w)) }).collect()
}

/// One scenario as the page shows it: its title, the accounts it names, and each way it can come
/// out, a step at a time with the balances and holds after each.
pub struct ScenarioView {
    pub title: String,
    pub accounts: Vec<AccountId>,
    pub outcomes: Vec<Vec<(StepView, State)>>,
}

pub fn scenario_views(book: &Book, lang: Lang) -> Vec<ScenarioView> {
    let mut out = Vec::new();
    for (s, title) in crate::scenarios::generate_titled(book) {
        let Ok(traces) = scenario::trace(book, &s) else { continue };
        let outcomes = traces.iter().map(|tr| s.steps.iter().zip(&tr.steps).zip(&tr.states).map(|((st, o), state)| (step_view(book, st, o, lang), state.clone())).collect()).collect();
        out.push(ScenarioView { title: title.get(lang).to_string(), accounts: scenario::named_accounts(book, &s), outcomes });
    }
    out
}

/// The holds of a state, as a page names them, with their states.
pub fn holds_of(book: &Book, state: &State, lang: Lang) -> Vec<(String, String)> {
    state.holds.iter().map(|((k, key), h)| (scenario::hold_text(book, *k, key), scenario::state_text(h.state).get(lang).to_string())).collect()
}

// ── the life of a hold ────────────────────────────────────────────────────

/// What `post` and `void` answer in each state of a hold of `t` (DESIGN 2.4), for the table
/// under its chart: the state, then the two answers.
pub fn life_rows(t: &TransferKind, lang: Lang) -> Vec<[String; 3]> {
    let expiring = matches!(t.pending, Some(Expiry::After(_)));
    let mut rows = Vec::new();
    let mut held_post = tr!(
        "確定する。額を渡せばその額、渡さなければ全額で、残りは元に戻る。押さえた額を超えれば `over_hold` で拒否される",
        "posts it: the amounts given, or all of it, and the rest goes back; more than it holds is refused with `over_hold`"
    );
    let mut held_void = tr!("取り消す。押さえた量は元に戻る", "voids it: what it holds goes back");
    if let Some(Expiry::After(s)) = t.pending {
        let (dj, de) = (duration(s, Lang::Ja), duration(s, Lang::En));
        let extra = tr!("。押さえてから {dj}たっていれば `expired` で拒否される", "; once {de} have passed since it was held, refused with `expired`");
        held_post.ja.push_str(&extra.ja);
        held_post.en.push_str(&extra.en);
        held_void.ja.push_str(&extra.ja);
        held_void.en.push_str(&extra.en);
    }
    rows.push([scenario::state_text(HoldState::Held).get(lang).to_string(), held_post.get(lang).to_string(), held_void.get(lang).to_string()]);
    rows.push([
        scenario::state_text(HoldState::Posted).get(lang).to_string(),
        tr!("同じ額なら `done_before`、違う額なら `key_conflict` で拒否される", "`done_before` with the same amounts; refused with `key_conflict` with others").get(lang).to_string(),
        tr!("`already_posted` で拒否される", "refused with `already_posted`").get(lang).to_string(),
    ]);
    rows.push([
        scenario::state_text(HoldState::Voided).get(lang).to_string(),
        tr!("`already_voided` で拒否される", "refused with `already_voided`").get(lang).to_string(),
        "`done_before`".to_string(),
    ]);
    if expiring {
        let e = tr!("`expired` で拒否される", "refused with `expired`").get(lang).to_string();
        rows.push([scenario::state_text(HoldState::Expired).get(lang).to_string(), e.clone(), e]);
    }
    let none = tr!("`no_such_hold` で拒否される", "refused with `no_such_hold`").get(lang).to_string();
    rows.push([tr!("仮押さえが無い", "no hold with the key").get(lang).to_string(), none.clone(), none]);
    rows
}

/// How a hold of `t` ends on its own, if it does: `after 30 minutes`.
pub fn expiry_text(t: &TransferKind, lang: Lang) -> Option<String> {
    match t.pending {
        Some(Expiry::After(s)) => {
            let d = duration(s, lang);
            Some(tr!("{d}たつ", "after {d}").get(lang).to_string())
        }
        _ => None,
    }
}

// ── Markdown ──────────────────────────────────────────────────────────────

fn mermaid_label(lines: &[String]) -> String {
    let joined = lines.join("<br>");
    format!("\"{}\"", joined.replace('"', "#quot;"))
}

/// The lines of an account's box: its name and parameters, its unit, and its bounds.
pub fn node_lines(book: &Book, k: usize, lang: Lang) -> Vec<String> {
    let a = &book.accounts[k];
    let unit = &book.units[a.unit].shown();
    let mut lines = vec![kind_text(a)];
    if a.outside {
        lines.push(tr!("{unit}・外の勘定", "{unit} · outside").get(lang).to_string());
        return lines;
    }
    lines.push(unit.clone());
    if let Some(l) = &a.lower {
        let (v, r) = (on(book, k, l.value), &l.refusal);
        lines.push(tr!("{v} 以上（{r}）", "at least {v} ({r})").get(lang).to_string());
    }
    if let Some(u) = &a.upper {
        let (v, r) = (on(book, k, u.value), &u.refusal);
        lines.push(tr!("{v} 以下（{r}）", "at most {v} ({r})").get(lang).to_string());
    }
    lines
}

/// The label of a move's arrow: the transfer, and the amount when the transfer moves more than once.
pub fn edge_label(book: &Book, t: &TransferKind, m: &Move, lang: Lang) -> String {
    if t.moves.len() == 1 {
        return t.name.clone();
    }
    let (name, a) = (&t.name, book.amount_text(t, m));
    tr!("{name}（{a}）", "{name} ({a})").get(lang).to_string()
}

/// The flow between the accounts, as a Mermaid flowchart.
pub fn flow_mermaid(book: &Book, lang: Lang) -> String {
    let mut o = String::from("```mermaid\nflowchart LR\n");
    for (k, a) in book.accounts.iter().enumerate() {
        let label = mermaid_label(&node_lines(book, k, lang));
        if a.outside {
            o.push_str(&format!("    a{k}([{label}])\n"));
        } else {
            o.push_str(&format!("    a{k}[{label}]\n"));
        }
    }
    for t in &book.transfers {
        for m in &t.moves {
            let label = mermaid_label(&[edge_label(book, t, m, lang)]);
            let arrow = if t.is_pending() { "-.->" } else { "-->" };
            o.push_str(&format!("    a{} {arrow}|{label}| a{}\n", m.from.kind, m.to.kind));
        }
    }
    o.push_str("```\n");
    o
}

/// The life of a hold of `t`, as a Mermaid state diagram.
pub fn life_mermaid(t: &TransferKind, lang: Lang) -> String {
    let mut o = String::from("```mermaid\nstateDiagram-v2\n    direction LR\n");
    let expiring = matches!(t.pending, Some(Expiry::After(_)));
    let mut states = vec![HoldState::Held, HoldState::Posted, HoldState::Voided];
    if expiring {
        states.push(HoldState::Expired);
    }
    for s in &states {
        o.push_str(&format!("    state \"{}\" as {}\n", scenario::state_text(*s).get(lang), s.name()));
    }
    o.push_str(&format!("    [*] --> held : {}.hold\n", t.name));
    o.push_str(&format!("    held --> posted : {}\n", tr!("確定（post）", "post").get(lang)));
    o.push_str(&format!("    held --> voided : {}\n", tr!("取消（void）", "void").get(lang)));
    if let Some(e) = expiry_text(t, lang) {
        o.push_str(&format!("    held --> expired : {e}\n"));
    }
    for s in &states[1..] {
        o.push_str(&format!("    {} --> [*]\n", s.name()));
    }
    o.push_str("```\n");
    o
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', "<br>")
}

fn md_table(head: &[String], rows: &[Vec<String>]) -> String {
    let mut o = format!("| {} |\n|{}\n", head.iter().map(|h| md_cell(h)).collect::<Vec<_>>().join(" | "), "---|".repeat(head.len()));
    for r in rows {
        o.push_str(&format!("| {} |\n", r.iter().map(|c| md_cell(c)).collect::<Vec<_>>().join(" | ")));
    }
    o
}

/// The text of the page's warnings: each diagnostic as `chobo check` prints it.
fn warnings_md(i: &Input) -> String {
    if i.diags.is_empty() {
        return String::new();
    }
    let mut o = format!("## {}\n\n", tr!("検査の警告", "What the check warns about").get(i.lang));
    o.push_str(tr!("`chobo check` が言うこと。どれも、ここに書いた操作の列で起きる。\n\n", "What `chobo check` says; each comes with the operations that get there.\n\n").get(i.lang));
    for d in i.diags {
        o.push_str(&format!("```text\n{}```\n\n", d.shown(i.file, i.src, i.lang)));
    }
    o
}

/// The intro: what the page is, and how to read a balance and a transfer.
pub fn intro(i: &Input) -> String {
    let file = i.file;
    let mut o = tr!("`chobo doc` が `{file}` から作ったページ。", "`{file}`, as `chobo doc` writes it. ").get(i.lang).to_string();
    o.push_str(
        tr!(
            "勘定ごとに残高を持ち、残高は入った量から出た量を引いたもの。どの振替も勘定の下限と上限を守り、守れない振替は、その境界に付けた名前で拒否されて、どの移動も行われない。振替には、すぐに動かすもの（`do`）と、動かす量をまず押さえるもの（`hold`）がある。押さえた分は、あとで確定される（`post`。全部か一部）か、取り消される（`void`）か、有効期限で切れる。",
            "Each account keeps a balance: what came in, less what went out. Every transfer keeps the bounds of the accounts: one that would break a bound is refused with the name the book gives that bound, and nothing of it moves. A transfer either moves at once (`do`), or first holds what it moves (`hold`); a hold is then posted (`post`, all of it or part), voided (`void`), or expires."
        )
        .get(i.lang),
    );
    o
}

pub fn markdown(i: &Input) -> String {
    let (book, lang) = (i.book, i.lang);
    let mut o = String::new();
    let file = i.file;
    o.push_str(&format!(
        "{}\n\n",
        match lang {
            Lang::Ja => format!("<!-- `chobo doc {file} --lang ja` の出力です。手で編集しないでください。 -->"),
            Lang::En => format!("<!-- The output of `chobo doc {file}`. Do not edit by hand. -->"),
        }
    ));
    o.push_str(&format!("# {} v{}\n\n", book.name, book.version));
    if let Some(d) = &book.description {
        o.push_str(&format!("{d}\n\n"));
    }
    o.push_str(&intro(i));
    o.push_str("\n\n");
    o.push_str(&warnings_md(i));

    // the accounts
    o.push_str(&format!("## {}\n\n", tr!("勘定", "Accounts").get(lang)));
    let head: Vec<String> = [tr!("勘定", "Account"), tr!("分け方", "One for each"), tr!("単位", "Unit"), tr!("境界", "Bounds"), tr!("説明", "What it is")].iter().map(|t| t.get(lang).to_string()).collect();
    let rows: Vec<Vec<String>> = book
        .accounts
        .iter()
        .enumerate()
        .map(|(k, a)| {
            let per = if a.params.is_empty() {
                tr!("一つだけ", "one account").get(lang).to_string()
            } else {
                let ps: Vec<String> = a.params.iter().map(|p| code(p)).collect();
                let ps = and_list(&ps, lang);
                tr!("{ps} ごと", "{ps}").get(lang).to_string()
            };
            vec![code(&a.name), per, book.units[a.unit].shown(), bounds_lines(book, k, lang).join("\n"), a.description.clone().unwrap_or_default()]
        })
        .collect();
    o.push_str(&md_table(&head, &rows));
    o.push('\n');

    // the flow
    o.push_str(&format!("## {}\n\n", tr!("勘定のあいだの流れ", "How things move").get(lang)));
    o.push_str(&flow_mermaid(book, lang));
    o.push('\n');
    o.push_str(
        tr!(
            "四角は勘定で、矢印は振替の移動。角の丸い四角は外の勘定で、境界を持たない。破線の矢印は仮押さえの振替で、まず押さえ、確定したときに動く。\n\n",
            "A box is an account, and an arrow a move of a transfer. A rounded box is an account outside the book, which has no bounds. A dashed arrow belongs to a transfer that holds first, and moves when the hold is posted.\n\n"
        )
        .get(lang),
    );

    // the transfers
    o.push_str(&format!("## {}\n\n", tr!("振替", "Transfers").get(lang)));
    for (k, t) in book.transfers.iter().enumerate() {
        o.push_str(&transfer_md(i, k, t));
    }

    // the scenarios
    o.push_str(&scenarios_md(i));
    o
}

/// The facts of a transfer: its moves, its key, and how its holds end.
pub fn transfer_facts(i: &Input, k: usize) -> Vec<String> {
    let (book, lang) = (i.book, i.lang);
    let t = &book.transfers[k];
    let mut facts = Vec::new();
    if t.moves.len() == 1 {
        let m = move_text(book, t, &t.moves[0], lang);
        facts.push(tr!("移動: {m}。", "Moves {m}.").get(lang).to_string());
    } else {
        let list: Vec<String> = t.moves.iter().enumerate().map(|(n, m)| format!("{}. {}", n + 1, move_text(book, t, m, lang))).collect();
        let n = t.moves.len();
        let head = tr!("移動は {n} つ。書いた順に行い、どれか一つでも拒否されたら、どれも行わない:", "{n} moves, made in this order, all or none:").get(lang).to_string();
        facts.push(format!("{head}\n{}", list.join("\n")));
    }
    let ps: Vec<String> = t.key.iter().map(|p| code(&t.params[*p].name)).collect();
    let key = and_list(&ps, lang);
    // `` `order` ごとに ``, `` `delivery` と `sku` の組ごとに ``
    let per = if ps.len() > 1 { format!("{} の組", and_list(&ps, Lang::Ja)) } else { format!("{} ", and_list(&ps, Lang::Ja)) };
    let mut kt = tr!("キー: {per}ごとに一度だけ動く。同じ呼び出しの二度目は何もせず、`done_before` を返す", "Key: once per {key}. The same call again does nothing, and answers `done_before`");
    if let Some(kn) = i.report.keys.iter().find(|n| n.kind == k) {
        if !kn.outside.is_empty() {
            let others: Vec<String> = kn.outside.iter().map(|(p, _)| code(&t.params[*p].name)).collect();
            let (oj, oe) = (or_list(&others, Lang::Ja), or_list(&others, Lang::En));
            kt.ja.push_str(&format!("。キーが同じで {oj}が違う二度目の呼び出しは、`key_conflict` で拒否される"));
            kt.en.push_str(&format!("; one that differs only in {oe} is refused with `key_conflict`"));
        }
        if kn.again.is_some() {
            kt.ja.push_str("。仮押さえが終わったあとに同じキーでもう一度押さえると、`done_before` を返し、何も押さえない");
            kt.en.push_str("; holding again with the same key after the hold has ended answers `done_before`, and holds nothing");
        }
    }
    kt.ja.push('。');
    kt.en.push('.');
    facts.push(kt.get(lang).to_string());
    match t.pending {
        Some(Expiry::After(s)) => {
            let d = duration(s, lang);
            facts.push(
                tr!(
                    "仮押さえ: まず押さえる。確定と取消は呼ぶ側がする。押さえてから {d}で期限が切れ、押さえた量は元に戻る。",
                    "Holds first: the caller posts or voids the hold. It expires {d} after it was made, and what it holds goes back."
                )
                .get(lang)
                .to_string(),
            );
        }
        Some(Expiry::Never) => {
            facts.push(
                tr!(
                    "仮押さえ: まず押さえる。確定と取消は呼ぶ側がする。期限は無く、どちらかが来るまで押さえたまま。",
                    "Holds first: the caller posts or voids the hold. It never expires, and stays held until one of them comes."
                )
                .get(lang)
                .to_string(),
            );
        }
        None => facts.push(tr!("すぐに動かす（`do`）。", "Moves at once (`do`).").get(lang).to_string()),
    }
    facts
}

/// The rows of a transfer's table of refusals: the operation, the reason, and what it means.
pub fn refusal_rows(i: &Input, k: usize) -> Vec<[String; 3]> {
    let (book, lang) = (i.book, i.lang);
    let t = &book.transfers[k];
    let mut rows = Vec::new();
    for op in i.report.ops.iter().filter(|o| o.kind == k) {
        let name = format!("{}.{}", t.name, op.op.name());
        if op.refusals.is_empty() {
            rows.push([code(&name), "—".into(), tr!("拒否されない", "never refused").get(lang).to_string()]);
        }
        for r in &op.refusals {
            rows.push([code(&name), code(&r.name), reason_meaning(book, t, op.op, r, lang)]);
        }
    }
    rows
}

fn transfer_md(i: &Input, k: usize, t: &TransferKind) -> String {
    let lang = i.lang;
    let mut o = format!("### {}\n\n", t.name);
    if let Some(d) = &t.description {
        o.push_str(&format!("{d}\n\n"));
    }
    for f in transfer_facts(i, k) {
        let mut lines = f.lines();
        o.push_str(&format!("- {}\n", lines.next().unwrap_or("")));
        for l in lines {
            o.push_str(&format!("    {l}\n"));
        }
    }
    o.push('\n');
    if t.is_pending() {
        o.push_str(&life_mermaid(t, lang));
        o.push('\n');
        let head: Vec<String> = [tr!("仮押さえの状態", "The hold"), tr!("post（確定）", "post"), tr!("void（取消）", "void")].iter().map(|x| x.get(lang).to_string()).collect();
        let rows: Vec<Vec<String>> = life_rows(t, lang).into_iter().map(|r| r.to_vec()).collect();
        o.push_str(&md_table(&head, &rows));
        o.push('\n');
    }
    let head: Vec<String> = [tr!("操作", "Operation"), tr!("拒否されうる理由", "May be refused with"), tr!("いつ", "When")].iter().map(|x| x.get(lang).to_string()).collect();
    let rows: Vec<Vec<String>> = refusal_rows(i, k).into_iter().map(|r| r.to_vec()).collect();
    o.push_str(&md_table(&head, &rows));
    o.push('\n');
    let examples: Vec<(String, Vec<String>)> = examples_of(i, k);
    if !examples.is_empty() {
        o.push_str(&format!("<details><summary>{}</summary>\n\n", tr!("拒否される例", "How each refusal comes about").get(lang)));
        for (head, lines) in examples {
            o.push_str(&format!("#### {head}\n\n```text\n{}\n```\n\n", lines.join("\n")));
        }
        o.push_str("</details>\n\n");
    }
    o
}

/// The example of every refusal of every operation of transfer `k`: `reserve.hold: out_of_stock`
/// and the operations that get there.
pub fn examples_of(i: &Input, k: usize) -> Vec<(String, Vec<String>)> {
    let t = &i.book.transfers[k];
    let mut out = Vec::new();
    for op in i.report.ops.iter().filter(|o| o.kind == k) {
        for r in &op.refusals {
            let lines = example_lines(i.book, &r.example, i.lang);
            if !lines.is_empty() {
                out.push((format!("{}.{}: {}", t.name, op.op.name(), r.name), lines));
            }
        }
    }
    out
}

fn scenarios_md(i: &Input) -> String {
    let (book, lang) = (i.book, i.lang);
    let views = scenario_views(book, lang);
    let n = views.len();
    let mut o = format!("## {}\n\n", tr!("シナリオ", "Scenarios").get(lang));
    o.push_str(
        tr!(
            "`chobo scenarios` が帳簿から作ったシナリオ {n} 本。境界の手前・ちょうど・超える、同じキーの二度目、仮押さえの終わり方、二つの呼び出し元が同時に最後の一つを取りに来るもの、などがある。どれも参照インタプリタで流したもので、ステップごとに、そのあとの残高を載せている。残高は確定した量で、仮押さえがあれば、その量を括弧の中に添えている。\n\n",
            "{n} scenarios, which `chobo scenarios` makes from the book: among them each bound just before, at and past it, each key used twice, every way a hold ends, and two callers after the last of something at the same time. The reference interpreter ran each one; after each step come the balances it left. A balance is what is posted, with what is held in brackets.\n\n"
        )
        .get(lang),
    );
    for (si, v) in views.iter().enumerate() {
        o.push_str(&format!("<details><summary>{}. {}</summary>\n\n", si + 1, crate::draw::inline(&v.title)));
        let k = v.outcomes.len();
        if k > 1 {
            o.push_str(&format!("{}\n\n", tr!("同時の操作の順序によって、結果は {k} 通りある。", "It can come out {k} ways, by the order the operations at the same time go in.").get(lang)));
        }
        for (oi, steps) in v.outcomes.iter().enumerate() {
            if k > 1 {
                o.push_str(&format!("{}\n\n", tr!("結果 {}:", "Outcome {}:", oi + 1).get(lang)));
            }
            o.push_str(&steps_table_md(book, &v.accounts, steps, lang));
            o.push('\n');
        }
        o.push_str("</details>\n\n");
    }
    o
}

fn steps_table_md(book: &Book, accounts: &[AccountId], steps: &[(StepView, State)], lang: Lang) -> String {
    let mut head: Vec<String> = vec!["#".into(), tr!("操作", "Operation").get(lang).to_string(), tr!("結果", "Result").get(lang).to_string()];
    head.extend(accounts.iter().map(|a| a.text(book)));
    let mut rows = Vec::new();
    for (n, (v, state)) in steps.iter().enumerate() {
        let (label, result) = if v.callers.is_empty() {
            (v.label.clone(), v.result.clone())
        } else {
            let mut ls = vec!["together".to_string()];
            let mut rs = vec![String::new()];
            for (ci, calls) in v.callers.iter().enumerate() {
                for (call, result, _, _) in calls {
                    ls.push(format!("{}: {call}", tr!("呼び出し元 {}", "caller {}", ci + 1).get(lang)));
                    rs.push(result.clone());
                }
            }
            (ls.join("\n"), rs.join("\n"))
        };
        let mut row = vec![(n + 1).to_string(), label, result];
        row.extend(accounts.iter().map(|a| balance_text(book, a.kind, state.balance(a), lang)));
        rows.push(row);
    }
    md_table(&head, &rows)
}

/// `&`, `<`, `>` and `"` as HTML writes them.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ── HTML ──────────────────────────────────────────────────────────────────

/// The data the page's script steps through: each scenario, each way it comes out, each step with
/// the balances and holds after it. Every string is written as the page shows it.
pub fn page_data(i: &Input) -> Value {
    let (book, lang) = (i.book, i.lang);
    let views = scenario_views(book, lang);
    let scenarios: Vec<Value> = views
        .iter()
        .map(|v| {
            let outcomes: Vec<Value> = v
                .outcomes
                .iter()
                .map(|steps| {
                    let ss: Vec<Value> = steps
                        .iter()
                        .map(|(sv, state)| {
                            json!({
                                "label": sv.label,
                                "result": sv.result,
                                "detail": sv.detail,
                                "refused": sv.refused,
                                "callers": sv.callers.iter().map(|cs| cs.iter().map(|(c, r, d, no)| json!({"call": c, "result": r, "detail": d, "refused": no})).collect::<Vec<_>>()).collect::<Vec<_>>(),
                                "kinds": sv.kinds.iter().map(|(k, ok)| json!([k, ok])).collect::<Vec<_>>(),
                                "balances": v.accounts.iter().map(|a| {
                                    let b = state.balance(a);
                                    json!([on(book, a.kind, b.posted), on(book, a.kind, b.held_out), on(book, a.kind, b.held_in)])
                                }).collect::<Vec<_>>(),
                                "holds": holds_of(book, state, lang).into_iter().map(|(n, s)| json!([n, s])).collect::<Vec<_>>(),
                            })
                        })
                        .collect();
                    json!(ss)
                })
                .collect();
            let zero: Vec<Value> = v.accounts.iter().map(|a| json!([on(book, a.kind, 0), on(book, a.kind, 0), on(book, a.kind, 0)])).collect();
            json!({
                "title": v.title,
                "accounts": v.accounts.iter().map(|a| a.text(book)).collect::<Vec<_>>(),
                "zero": zero,
                "outcomes": outcomes,
            })
        })
        .collect();
    json!({"lang": if lang == Lang::Ja { "ja" } else { "en" }, "scenarios": scenarios})
}

pub fn html(i: &Input) -> String {
    crate::draw::page(i)
}
