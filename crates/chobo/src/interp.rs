//! The reference interpreter: what a book means (DESIGN 2). PostgreSQL and TigerBeetle are
//! matched against it, operation by operation.

use crate::model::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Val {
    Str(String),
    Amt(i128),
}

impl Val {
    pub fn text(&self) -> String {
        match self {
            Val::Str(s) => s.clone(),
            Val::Amt(v) => v.to_string(),
        }
    }
}

/// One account: its kind and the values of its arguments.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountId {
    pub kind: usize,
    pub args: Vec<String>,
}

impl AccountId {
    pub fn text(&self, book: &Book) -> String {
        let name = &book.accounts[self.kind].name;
        if self.args.is_empty() { name.clone() } else { format!("{name}({})", self.args.join(", ")) }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bal {
    pub posted: i128,
    pub held_in: i128,
    pub held_out: i128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op {
    Do,
    Hold,
    Post,
    Void,
}

impl Op {
    pub fn name(self) -> &'static str {
        match self {
            Op::Do => "do",
            Op::Hold => "hold",
            Op::Post => "post",
            Op::Void => "void",
        }
    }

    pub fn parse(s: &str) -> Option<Op> {
        match s {
            "do" => Some(Op::Do),
            "hold" => Some(Op::Hold),
            "post" => Some(Op::Post),
            "void" => Some(Op::Void),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HoldState {
    Held,
    Posted,
    Voided,
    Expired,
}

impl HoldState {
    pub fn name(self) -> &'static str {
        match self {
            HoldState::Held => "held",
            HoldState::Posted => "posted",
            HoldState::Voided => "voided",
            HoldState::Expired => "expired",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldMove {
    pub from: AccountId,
    pub to: AccountId,
    pub amount: i128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hold {
    pub moves: Vec<HeldMove>,
    pub state: HoldState,
    pub created: u64,
    /// when it expires, in seconds from the start; None for `never expires`
    pub deadline: Option<u64>,
    /// what was posted, move by move, once it is
    pub posted: Option<Vec<i128>>,
}

/// What a key was used for: a call that went through, with what it was called with, or a
/// call a bound refused (DESIGN 2.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyRec {
    Done(String),
    Refused,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// seconds from the start of the scenario
    pub now: u64,
    pub accounts: BTreeMap<AccountId, Bal>,
    /// by transfer kind and the values of its key
    pub holds: BTreeMap<(usize, Vec<String>), Hold>,
    /// the keys of `do` and `hold`, by transfer kind, operation and the values of the key
    pub keys: BTreeMap<(usize, Op, Vec<String>), KeyRec>,
}

/// Where a bound refused a call: which move, which account, which bound, and the account's
/// balance as that move found it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct At {
    /// from 0
    pub move_index: usize,
    pub account: AccountId,
    pub upper: bool,
    pub amount: i128,
    pub bal: Bal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub reason: String,
    pub at: Option<At>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Done,
    DoneBefore,
    Refused(Refusal),
}

impl Outcome {
    pub fn result(&self) -> &'static str {
        match self {
            Outcome::Done => "done",
            Outcome::DoneBefore => "done_before",
            Outcome::Refused(_) => "refused",
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Outcome::Refused(r) => Some(&r.reason),
            _ => None,
        }
    }

    fn refused(reason: &str) -> Outcome {
        Outcome::Refused(Refusal { reason: reason.to_string(), at: None })
    }
}

/// One call: an operation on a transfer kind. `args` is by the kind's parameters, every
/// one for `do` and `hold`, the key's for `post` and `void`. `amounts` is what to post, by
/// the amount parameter, when not all of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub op: Op,
    pub kind: usize,
    pub args: Vec<Option<Val>>,
    pub amounts: Option<BTreeMap<usize, i128>>,
}

impl Call {
    pub fn key(&self, t: &TransferKind) -> Vec<String> {
        t.key.iter().map(|i| self.args[*i].as_ref().map(|v| v.text()).unwrap_or_default()).collect()
    }

    fn str_arg(&self, i: usize) -> String {
        match &self.args[i] {
            Some(Val::Str(s)) => s.clone(),
            Some(v) => v.text(),
            None => String::new(),
        }
    }

    pub fn account(&self, r: &Ref) -> AccountId {
        let args = r
            .args
            .iter()
            .map(|a| match a {
                Arg::Param(i) => self.str_arg(*i),
                Arg::Lit(s) => s.clone(),
            })
            .collect();
        AccountId { kind: r.kind, args }
    }

    pub fn amount(&self, m: &Move) -> i128 {
        match &m.amount {
            Amount::Param(i) => match &self.args[*i] {
                Some(Val::Amt(v)) => *v,
                _ => 0,
            },
            Amount::Lit(v) => *v,
        }
    }

    /// Every account a `do` or `hold` names, in the order of its moves.
    pub fn accounts(&self, book: &Book) -> Vec<AccountId> {
        let t = &book.transfers[self.kind];
        let mut out = Vec::new();
        for m in &t.moves {
            for id in [self.account(&m.from), self.account(&m.to)] {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }
}

/// Is the call well formed for the book: the operation fits the kind, every argument it
/// needs is there with the right type, and the amounts are in range. An answer of Err is a
/// mistake in the scenario, not a refusal.
pub fn validate(book: &Book, c: &Call) -> Result<(), String> {
    let t = book.transfers.get(c.kind).ok_or("no such transfer kind")?;
    let fits = match c.op {
        Op::Do => !t.is_pending(),
        Op::Hold | Op::Post | Op::Void => t.is_pending(),
    };
    if !fits {
        return Err(format!("`{}` takes {}, not {}", t.name, t.ops().join(", "), c.op.name()));
    }
    if c.args.len() != t.params.len() {
        return Err("the arguments do not match the parameters".into());
    }
    let needed: Vec<usize> = match c.op {
        Op::Do | Op::Hold => (0..t.params.len()).collect(),
        Op::Post | Op::Void => t.key.clone(),
    };
    for (i, p) in t.params.iter().enumerate() {
        let want = needed.contains(&i);
        match (&c.args[i], want, p.ty) {
            (None, false, _) => {}
            (None, true, _) => return Err(format!("`{}` needs the argument `{}`", t.name, p.name)),
            (Some(_), false, _) => return Err(format!("`{}.{}` takes only the key; `{}` is not in it", t.name, c.op.name(), p.name)),
            (Some(Val::Str(_)), true, Ty::Str) => {}
            (Some(Val::Amt(v)), true, Ty::Amount(_)) => {
                if !(0..=MAX).contains(v) {
                    return Err(format!("`{}` is {v}; an amount is from 0 to 2^63 - 1", p.name));
                }
            }
            (Some(_), true, Ty::Str) => return Err(format!("`{}` is a string", p.name)),
            (Some(_), true, Ty::Amount(_)) => return Err(format!("`{}` is an amount, a whole number", p.name)),
        }
    }
    if let Some(a) = &c.amounts {
        if c.op != Op::Post {
            return Err("only `post` takes amounts".into());
        }
        let ap = t.amount_params();
        if a.len() != ap.len() || !ap.iter().all(|i| a.contains_key(i)) {
            let names: Vec<&str> = ap.iter().map(|i| t.params[*i].name.as_str()).collect();
            return Err(format!("`{}.post` takes every amount ({}) or none", t.name, names.join(", ")));
        }
        for (i, v) in a {
            if !(0..=MAX).contains(v) {
                return Err(format!("`{}` is {v}; an amount is from 0 to 2^63 - 1", t.params[*i].name));
            }
        }
    }
    Ok(())
}

fn in_range(b: &Bal) -> bool {
    let r = (i64::MIN as i128)..=(i64::MAX as i128);
    r.contains(&b.posted) && r.contains(&b.held_in) && r.contains(&b.held_out)
}

impl State {
    pub fn balance(&self, id: &AccountId) -> Bal {
        self.accounts.get(id).copied().unwrap_or_default()
    }

    pub fn status(&self, kind: usize, key: &[String]) -> Option<HoldState> {
        self.holds.get(&(kind, key.to_vec())).map(|h| h.state)
    }

    /// Carry out one call. Err is a mistake in the call (DESIGN 1.5: a failure, not a
    /// refusal), and then nothing has changed.
    pub fn apply(&mut self, book: &Book, c: &Call) -> Result<Outcome, String> {
        validate(book, c)?;
        match c.op {
            Op::Do | Op::Hold => self.move_or_hold(book, c),
            Op::Post => self.post(book, c),
            Op::Void => self.void(book, c),
        }
    }

    fn move_or_hold(&mut self, book: &Book, c: &Call) -> Result<Outcome, String> {
        let t = &book.transfers[c.kind];
        let moves: Vec<HeldMove> = t.moves.iter().map(|m| HeldMove { from: c.account(&m.from), to: c.account(&m.to), amount: c.amount(m) }).collect();
        // an account cannot be moved to itself; this is the call's own mistake, and costs no key
        if moves.iter().any(|m| m.from == m.to) {
            return Ok(Outcome::refused("same_account"));
        }
        let key = c.key(t);
        let slot = (c.kind, c.op, key.clone());
        let content = crate::ids::content_json(book, c, None);
        match self.keys.get(&slot) {
            Some(KeyRec::Refused) => return Ok(Outcome::refused("already_refused")),
            Some(KeyRec::Done(before)) => {
                return Ok(if *before == content { Outcome::DoneBefore } else { Outcome::refused("key_conflict") });
            }
            None => {}
        }
        // the moves in the order they are written, each checked against the balances the
        // moves before it left (DESIGN 2.2)
        let mut scratch: BTreeMap<AccountId, Bal> = BTreeMap::new();
        for (i, m) in moves.iter().enumerate() {
            for id in [&m.from, &m.to] {
                scratch.entry(id.clone()).or_insert_with(|| self.balance(id));
            }
            let fb = scratch[&m.from];
            if let Some(l) = &book.accounts[m.from.kind].lower {
                if fb.posted - fb.held_out - m.amount < l.value {
                    self.keys.insert(slot, KeyRec::Refused);
                    let at = At { move_index: i, account: m.from.clone(), upper: false, amount: m.amount, bal: fb };
                    return Ok(Outcome::Refused(Refusal { reason: l.refusal.clone(), at: Some(at) }));
                }
            }
            let tb = scratch[&m.to];
            if let Some(u) = &book.accounts[m.to.kind].upper {
                if tb.posted + tb.held_in + m.amount > u.value {
                    self.keys.insert(slot, KeyRec::Refused);
                    let at = At { move_index: i, account: m.to.clone(), upper: true, amount: m.amount, bal: tb };
                    return Ok(Outcome::Refused(Refusal { reason: u.refusal.clone(), at: Some(at) }));
                }
            }
            let f = scratch.get_mut(&m.from).unwrap();
            if c.op == Op::Hold {
                f.held_out += m.amount;
            } else {
                f.posted -= m.amount;
            }
            let to = scratch.get_mut(&m.to).unwrap();
            if c.op == Op::Hold {
                to.held_in += m.amount;
            } else {
                to.posted += m.amount;
            }
        }
        if let Some((id, _)) = scratch.iter().find(|(_, b)| !in_range(b)) {
            return Err(format!("the balance of {} would leave the range of a 64-bit integer", id.text(book)));
        }
        self.accounts.extend(scratch);
        if c.op == Op::Hold {
            let deadline = match t.pending {
                Some(Expiry::After(s)) => Some(self.now + s),
                _ => None,
            };
            self.holds.insert((c.kind, key), Hold { moves, state: HoldState::Held, created: self.now, deadline, posted: None });
        }
        self.keys.insert(slot, KeyRec::Done(content));
        Ok(Outcome::Done)
    }

    /// What a `post` moves, move by move: all of it, or what the amounts make of each move.
    pub fn post_amounts(book: &Book, c: &Call, hold: &Hold) -> Vec<i128> {
        let t = &book.transfers[c.kind];
        match &c.amounts {
            None => hold.moves.iter().map(|m| m.amount).collect(),
            Some(a) => t
                .moves
                .iter()
                .map(|m| match &m.amount {
                    Amount::Param(i) => a.get(i).copied().unwrap_or(0),
                    Amount::Lit(v) => *v,
                })
                .collect(),
        }
    }

    fn post(&mut self, book: &Book, c: &Call) -> Result<Outcome, String> {
        let t = &book.transfers[c.kind];
        let hk = (c.kind, c.key(t));
        let Some(hold) = self.holds.get(&hk) else {
            return Ok(Outcome::refused("no_such_hold"));
        };
        let amounts = Self::post_amounts(book, c, hold);
        match hold.state {
            HoldState::Held => {
                if hold.deadline.is_some_and(|d| self.now >= d) {
                    return Ok(Outcome::refused("expired"));
                }
                if amounts.iter().zip(&hold.moves).any(|(p, m)| *p > m.amount) {
                    return Ok(Outcome::refused("over_hold"));
                }
                let moves = hold.moves.clone();
                for (m, p) in moves.iter().zip(&amounts) {
                    let f = self.accounts.entry(m.from.clone()).or_default();
                    f.held_out -= m.amount;
                    f.posted -= p;
                    let to = self.accounts.entry(m.to.clone()).or_default();
                    to.held_in -= m.amount;
                    to.posted += p;
                }
                let h = self.holds.get_mut(&hk).unwrap();
                h.state = HoldState::Posted;
                h.posted = Some(amounts);
                Ok(Outcome::Done)
            }
            HoldState::Posted => Ok(if hold.posted.as_ref() == Some(&amounts) { Outcome::DoneBefore } else { Outcome::refused("key_conflict") }),
            HoldState::Voided => Ok(Outcome::refused("already_voided")),
            HoldState::Expired => Ok(Outcome::refused("expired")),
        }
    }

    fn void(&mut self, book: &Book, c: &Call) -> Result<Outcome, String> {
        let t = &book.transfers[c.kind];
        let hk = (c.kind, c.key(t));
        let Some(hold) = self.holds.get(&hk) else {
            return Ok(Outcome::refused("no_such_hold"));
        };
        match hold.state {
            HoldState::Held => {
                if hold.deadline.is_some_and(|d| self.now >= d) {
                    return Ok(Outcome::refused("expired"));
                }
                self.release(&hk);
                self.holds.get_mut(&hk).unwrap().state = HoldState::Voided;
                Ok(Outcome::Done)
            }
            HoldState::Posted => Ok(Outcome::refused("already_posted")),
            HoldState::Voided => Ok(Outcome::DoneBefore),
            HoldState::Expired => Ok(Outcome::refused("expired")),
        }
    }

    fn release(&mut self, hk: &(usize, Vec<String>)) {
        let moves = self.holds[hk].moves.clone();
        for m in &moves {
            self.accounts.entry(m.from.clone()).or_default().held_out -= m.amount;
            self.accounts.entry(m.to.clone()).or_default().held_in -= m.amount;
        }
    }

    /// Let time pass. A hold whose expiry the clock reaches expires then, and what it held
    /// goes back (DESIGN 2.5). Answers the holds that expired, in the order they did.
    pub fn pass(&mut self, secs: u64) -> Vec<(usize, Vec<String>)> {
        self.now += secs;
        let mut due: Vec<(u64, (usize, Vec<String>))> = self
            .holds
            .iter()
            .filter(|(_, h)| h.state == HoldState::Held && h.deadline.is_some_and(|d| d <= self.now))
            .map(|(k, h)| (h.deadline.unwrap(), k.clone()))
            .collect();
        due.sort();
        let mut out = Vec::new();
        for (_, hk) in due {
            self.release(&hk);
            self.holds.get_mut(&hk).unwrap().state = HoldState::Expired;
            out.push(hk);
        }
        out
    }
}
