//! IDs, and what a TigerBeetle client sends (DESIGN 4.2, 4.3; PLAN 0.3).
//!
//! An ID is the first 16 bytes of a SHA-256 over length-prefixed parts, so that the Rust
//! here, the three generated clients and PostgreSQL's `chobo_id` compute the same one from
//! the same key without ever storing it. SHA-256 is written out below so that chobo needs
//! nothing but serde_json.

use crate::interp::{AccountId, Call, Hold, Op, Val};
use crate::model::*;
use serde_json::{Value, json};

// ── SHA-256 (FIPS 180-4) ──────────────────────────────────────────────────

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    let mut w = [0u32; 64];
    for block in msg.chunks(64) {
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 32];
    for (i, x) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ── IDs ───────────────────────────────────────────────────────────────────

/// The version of the scheme, the first part of every hash.
pub const PREFIX: &str = "chobo/1";

/// The bytes of the parts: each one's UTF-8 length as 32 big-endian bits, then its UTF-8.
pub fn enc(parts: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    for p in std::iter::once(&PREFIX).chain(parts.iter()) {
        out.extend_from_slice(&(p.len() as u32).to_be_bytes());
        out.extend_from_slice(p.as_bytes());
    }
    out
}

pub fn h(parts: &[&str]) -> [u8; 32] {
    sha256(&enc(parts))
}

/// The first 16 bytes, big-endian; 0 and 2¹²⁸ − 1 are not IDs in TigerBeetle.
pub fn id(parts: &[&str]) -> u128 {
    let d = h(parts);
    let v = u128::from_be_bytes(d[..16].try_into().unwrap());
    match v {
        0 => 1,
        u128::MAX => u128::MAX - 1,
        v => v,
    }
}

pub fn id_hex(v: u128) -> String {
    format!("{v:032x}")
}

pub fn ledger(unit: &Unit) -> u32 {
    let d = h(&["ledger", &unit.name, &unit.scale.to_string()]);
    match u32::from_be_bytes(d[..4].try_into().unwrap()) {
        0 => 1,
        v => v,
    }
}

/// `what` is `account`, `transfer` or `sink`; `name` is the kind's name, or the unit's for a sink.
pub fn code(book: &str, what: &str, name: &str) -> u16 {
    let d = h(&["code", book, what, name]);
    match u16::from_be_bytes(d[..2].try_into().unwrap()) {
        0 => 1,
        v => v,
    }
}

pub fn account_id(book: &Book, tenant: &str, a: &AccountId) -> u128 {
    let mut parts: Vec<&str> = vec!["account", &book.name, tenant, &book.accounts[a.kind].name];
    parts.extend(a.args.iter().map(|s| s.as_str()));
    id(&parts)
}

pub fn floor_id(account: u128) -> u128 {
    id(&["floor", &id_hex(account)])
}

pub fn room_id(account: u128) -> u128 {
    id(&["room", &id_hex(account)])
}

pub fn sink_id(book: &Book, tenant: &str, unit: &Unit) -> u128 {
    id(&["sink", &book.name, tenant, &unit.name])
}

pub fn opening_id(account: u128) -> u128 {
    id(&["opening", &id_hex(account)])
}

pub fn kind_id(book: &Book, tenant: &str, kind: &str) -> u128 {
    id(&["kind", &book.name, tenant, kind])
}

pub fn transfer_id(book: &Book, tenant: &str, t: &TransferKind, op: Op, key: &[String], position: usize) -> u128 {
    let pos = position.to_string();
    let mut parts: Vec<&str> = vec!["transfer", &book.name, tenant, &t.name, op.name()];
    parts.extend(key.iter().map(|s| s.as_str()));
    parts.push(&pos);
    id(&parts)
}

// ── what a call is, as the hashes see it ─────────────────────────────────

/// A string as JavaScript's `JSON.stringify` writes it, which the three clients reproduce.
pub fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// What a call is called with (DESIGN 2.3): for `do` and `hold` every argument, as an object
/// in the order of the parameters; for `post` what it posts, move by move; for `void`
/// nothing. Written without spaces.
pub fn content_json(book: &Book, c: &Call, post: Option<&[i128]>) -> String {
    let t = &book.transfers[c.kind];
    match c.op {
        Op::Do | Op::Hold => {
            let fields: Vec<String> = t
                .params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let v = match &c.args[i] {
                        Some(Val::Str(s)) => json_string(s),
                        Some(Val::Amt(a)) => a.to_string(),
                        None => "null".into(),
                    };
                    format!("{}:{v}", json_string(&p.name))
                })
                .collect();
            format!("{{{}}}", fields.join(","))
        }
        Op::Post => format!("[{}]", post.unwrap_or(&[]).iter().map(|a| a.to_string()).collect::<Vec<_>>().join(",")),
        Op::Void => "null".into(),
    }
}

fn quote_book(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A transfer kind written the one way: its parameters, key, how a hold ends, and moves.
/// What it says in `description` is left out: it changes nothing a transfer does.
pub fn transfer_text(book: &Book, t: &TransferKind) -> String {
    let ty = |p: &TParam| match p.ty {
        Ty::Str => "string".to_string(),
        Ty::Amount(u) => book.units[u].name.clone(),
    };
    let params: Vec<String> = t.params.iter().map(|p| format!("{}: {}", p.name, ty(p))).collect();
    let mut lines = vec![format!("transfer {}({})", t.name, params.join(", "))];
    lines.push(format!("key {}", t.key.iter().map(|i| t.params[*i].name.clone()).collect::<Vec<_>>().join(", ")));
    match t.pending {
        Some(Expiry::After(s)) => lines.push(format!("pending expires after {s} seconds")),
        Some(Expiry::Never) => lines.push("pending never expires".into()),
        None => {}
    }
    let rf = |r: &Ref| {
        let a = &book.accounts[r.kind];
        if r.args.is_empty() {
            return a.name.clone();
        }
        let args: Vec<String> = r
            .args
            .iter()
            .map(|x| match x {
                Arg::Param(i) => t.params[*i].name.clone(),
                Arg::Lit(s) => quote_book(s),
            })
            .collect();
        format!("{}({})", a.name, args.join(", "))
    };
    for m in &t.moves {
        let amount = match &m.amount {
            Amount::Param(i) => t.params[*i].name.clone(),
            Amount::Lit(v) => v.to_string(),
        };
        lines.push(format!("move {amount} from {} to {}", rf(&m.from), rf(&m.to)));
    }
    lines.join("\n")
}

/// An account kind written the one way: its parameters, unit and bounds. The names of the
/// refusals are left out: they change nothing that is stored.
pub fn account_text(book: &Book, a: &AccountKind) -> String {
    let u = &book.units[a.unit];
    let mut s = format!("account {}", a.name);
    if !a.params.is_empty() {
        s.push_str(&format!("({})", a.params.iter().map(|p| format!("{p}: string")).collect::<Vec<_>>().join(", ")));
    }
    s.push_str(&format!(" : {} scale {}", u.name, u.scale));
    if a.outside {
        s.push_str(" outside");
    }
    if let Some(l) = &a.lower {
        s.push_str(&format!(" at least {}", l.value));
    }
    if let Some(up) = &a.upper {
        s.push_str(&format!(" at most {}", up.value));
    }
    s
}

/// The account kinds a transfer kind's moves touch, in the order they first appear.
pub fn touched(t: &TransferKind) -> Vec<usize> {
    let mut out = Vec::new();
    for m in &t.moves {
        for k in [m.from.kind, m.to.kind] {
            if !out.contains(&k) {
                out.push(k);
            }
        }
    }
    out
}

/// The hash of a transfer kind's definition and of the bounds of the accounts it touches,
/// as 32 hex digits. It goes into every content ID, so that a retry made after the book
/// changed does not run against the old chain (DESIGN 4.2).
pub fn definition(book: &Book, t: &TransferKind) -> String {
    let mut parts: Vec<String> = vec!["definition".into(), transfer_text(book, t)];
    parts.extend(touched(t).into_iter().map(|k| account_text(book, &book.accounts[k])));
    let refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
    hex(&h(&refs)[..16])
}

pub fn content_id(book: &Book, tenant: &str, c: &Call, post: Option<&[i128]>) -> u128 {
    let t = &book.transfers[c.kind];
    let def = definition(book, t);
    let content = content_json(book, c, post);
    id(&["content", &book.name, tenant, &t.name, c.op.name(), &def, &content])
}

// ── the chain ─────────────────────────────────────────────────────────────

/// The roles of the transfers in a chain, with their `user_data_32` (PLAN 0.3).
pub const ROLES: &[(&str, u32)] =
    &[("main", 1), ("probe", 2), ("probe_void", 3), ("floor_out", 4), ("room_back", 5), ("room_in", 6), ("floor_back", 7), ("opening", 8)];

pub fn role_code(role: &str) -> u32 {
    ROLES.iter().find(|(r, _)| *r == role).map(|(_, c)| *c).unwrap_or(0)
}

#[derive(Clone, Debug, PartialEq)]
pub struct TbAccount {
    /// account, floor, room or sink
    pub role: &'static str,
    /// the account of the book it is, or belongs to; the unit, for a sink
    pub of: String,
    pub id: u128,
    pub ledger: u32,
    pub code: u16,
    pub flags: Vec<&'static str>,
    pub user_data_128: u128,
    pub user_data_32: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TbTransfer {
    pub role: &'static str,
    /// the move it belongs to, from 1; 0 for an opening
    pub move_no: usize,
    pub id: u128,
    pub debit: u128,
    pub credit: u128,
    pub amount: u128,
    pub pending_id: u128,
    pub user_data_128: u128,
    pub user_data_32: u32,
    pub timeout: u32,
    pub ledger: u32,
    pub code: u16,
    pub flags: Vec<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Chain {
    /// what the client makes sure exists before it sends the chain
    pub accounts: Vec<TbAccount>,
    /// the first transfer into each floor and room account
    pub openings: Vec<TbTransfer>,
    pub transfers: Vec<TbTransfer>,
}

/// The transfers of one move of a `do` or `hold`, with what each one is for. The amounts
/// are filled in by the caller.
pub fn layout(book: &Book, from: usize, to: usize) -> Vec<&'static str> {
    let fa = &book.accounts[from];
    let ta = &book.accounts[to];
    let mut roles = vec!["main"];
    if let Some(l) = &fa.lower {
        if l.value > 0 {
            roles.push("probe");
            roles.push("probe_void");
        }
        if l.value < 0 {
            roles.push("floor_out");
        }
    }
    if fa.upper.is_some() {
        roles.push("room_back");
    }
    if ta.upper.is_some() {
        roles.push("room_in");
    }
    if ta.lower.as_ref().is_some_and(|l| l.value < 0) {
        roles.push("floor_back");
    }
    roles
}

pub fn flagged(a: &AccountKind) -> bool {
    !a.outside && a.lower.as_ref().is_some_and(|l| l.value >= 0)
}

/// What a TigerBeetle client sends for one call (PLAN 0.3), or None when it sends nothing:
/// a move from an account to itself (`same_account`), or a `post` or `void` with no hold
/// behind it (`no_such_hold`). `hold` is the hold a `post` or `void` is about, as the client
/// reads it first.
pub fn chain(book: &Book, tenant: &str, c: &Call, hold: Option<&Hold>) -> Option<Chain> {
    let t = &book.transfers[c.kind];
    let key = c.key(t);
    let tcode = code(&book.name, "transfer", &t.name);
    let timeout = match t.pending {
        Some(Expiry::After(s)) if c.op == Op::Hold => s as u32,
        _ => 0,
    };
    match c.op {
        Op::Do | Op::Hold => {
            let ids: Vec<(AccountId, AccountId, i128)> = t.moves.iter().map(|m| (c.account(&m.from), c.account(&m.to), c.amount(m))).collect();
            if ids.iter().any(|(f, to, _)| f == to) {
                return None;
            }
            let content = content_id(book, tenant, c, None);
            let mut ch = Chain::default();
            let add_account = |ch: &mut Chain, acc: TbAccount| {
                if !ch.accounts.iter().any(|x| x.id == acc.id) {
                    ch.accounts.push(acc);
                }
            };
            for (m, (f, to, amount)) in t.moves.iter().zip(&ids) {
                let fa = &book.accounts[m.from.kind];
                let ta = &book.accounts[m.to.kind];
                let unit = book.unit_of(m.from.kind);
                let led = ledger(unit);
                let sink = sink_id(book, tenant, unit);
                let fid = account_id(book, tenant, f);
                let tid = account_id(book, tenant, to);
                for (acc, kind, aid) in [(f, fa, fid), (to, ta, tid)] {
                    add_account(
                        &mut ch,
                        TbAccount {
                            role: "account",
                            of: acc.text(book),
                            id: aid,
                            ledger: led,
                            code: code(&book.name, "account", &kind.name),
                            flags: if flagged(kind) { vec!["debits_must_not_exceed_credits"] } else { vec![] },
                            user_data_128: kind_id(book, tenant, &kind.name),
                            user_data_32: 0,
                        },
                    );
                }
                let roles = layout(book, m.from.kind, m.to.kind);
                let extra = |role: &str| -> Option<(&'static str, u128, &AccountKind, &AccountId, i128)> {
                    match role {
                        "floor_out" => Some(("floor", fid, fa, f, -fa.lower.as_ref().unwrap().value)),
                        "room_back" => Some(("room", fid, fa, f, fa.upper.as_ref().unwrap().value)),
                        "room_in" => Some(("room", tid, ta, to, ta.upper.as_ref().unwrap().value)),
                        "floor_back" => Some(("floor", tid, ta, to, -ta.lower.as_ref().unwrap().value)),
                        _ => None,
                    }
                };
                if roles.len() > 1 {
                    add_account(
                        &mut ch,
                        TbAccount {
                            role: "sink",
                            of: unit.name.clone(),
                            id: sink,
                            ledger: led,
                            code: code(&book.name, "sink", &unit.name),
                            flags: vec![],
                            user_data_128: 0,
                            user_data_32: 3,
                        },
                    );
                }
                for role in &roles {
                    if let Some((what, owner, kind, acc, opening)) = extra(role) {
                        let xid = if what == "floor" { floor_id(owner) } else { room_id(owner) };
                        if !ch.accounts.iter().any(|x| x.id == xid) {
                            add_account(
                                &mut ch,
                                TbAccount {
                                    role: what,
                                    of: acc.text(book),
                                    id: xid,
                                    ledger: led,
                                    code: code(&book.name, "account", &kind.name),
                                    flags: vec!["debits_must_not_exceed_credits"],
                                    user_data_128: kind_id(book, tenant, &kind.name),
                                    user_data_32: if what == "floor" { 1 } else { 2 },
                                },
                            );
                            ch.openings.push(TbTransfer {
                                role: "opening",
                                move_no: 0,
                                id: opening_id(xid),
                                debit: sink,
                                credit: xid,
                                amount: opening as u128,
                                pending_id: 0,
                                user_data_128: 0,
                                user_data_32: role_code("opening"),
                                timeout: 0,
                                ledger: led,
                                code: code(&book.name, "account", &kind.name),
                                flags: vec![],
                            });
                        }
                    }
                }
                let move_no = ch.transfers.iter().map(|x| x.move_no).max().unwrap_or(0) + 1;
                let a = *amount as u128;
                let pending = if c.op == Op::Hold { vec!["pending"] } else { vec![] };
                for role in roles {
                    let (debit, credit, amt, flags, tmo, pid, led2, code2): (u128, u128, u128, Vec<&'static str>, u32, u128, u32, u16) = match role {
                        "main" => (fid, tid, a, pending.clone(), timeout, 0, led, tcode),
                        "probe" => (fid, sink, fa.lower.as_ref().unwrap().value as u128, vec!["pending"], 0, 0, led, tcode),
                        "probe_void" => {
                            let probe = ch.transfers.last().map(|p| p.id).unwrap_or(0);
                            (0, 0, 0, vec!["void_pending_transfer"], 0, probe, 0, 0)
                        }
                        "floor_out" => (floor_id(fid), sink, a, pending.clone(), timeout, 0, led, tcode),
                        "room_back" => (sink, room_id(fid), a, pending.clone(), timeout, 0, led, tcode),
                        "room_in" => (room_id(tid), sink, a, pending.clone(), timeout, 0, led, tcode),
                        "floor_back" => (sink, floor_id(tid), a, pending.clone(), timeout, 0, led, tcode),
                        _ => unreachable!(),
                    };
                    let position = ch.transfers.len();
                    ch.transfers.push(TbTransfer {
                        role,
                        move_no,
                        id: transfer_id(book, tenant, t, c.op, &key, position),
                        debit,
                        credit,
                        amount: amt,
                        pending_id: pid,
                        user_data_128: content,
                        user_data_32: role_code(role),
                        timeout: tmo,
                        ledger: led2,
                        code: code2,
                        flags,
                    });
                }
            }
            link(&mut ch.transfers);
            Some(ch)
        }
        Op::Post | Op::Void => {
            let hold = hold?;
            // the hold's own chain, to find the ID of each of its transfers
            let hold_call = Call { op: Op::Hold, kind: c.kind, args: c.args.clone(), amounts: None };
            let held = hold_layout(book, tenant, &hold_call, hold);
            let posted: Option<Vec<i128>> = if c.op == Op::Post { Some(crate::interp::State::post_amounts(book, c, hold)) } else { None };
            let content = content_id(book, tenant, c, posted.as_deref());
            let mut ch = Chain::default();
            for (role, move_no, pid) in held {
                if role == "probe" || role == "probe_void" {
                    continue;
                }
                let amount = match (&c.op, &c.amounts, &posted) {
                    (Op::Void, _, _) => 0,
                    (Op::Post, None, _) => u128::MAX,
                    (Op::Post, Some(_), Some(p)) => p[move_no - 1] as u128,
                    _ => 0,
                };
                let position = ch.transfers.len();
                ch.transfers.push(TbTransfer {
                    role,
                    move_no,
                    id: transfer_id(book, tenant, t, c.op, &key, position),
                    debit: 0,
                    credit: 0,
                    amount,
                    pending_id: pid,
                    user_data_128: content,
                    user_data_32: role_code(role),
                    timeout: 0,
                    ledger: 0,
                    code: 0,
                    flags: vec![if c.op == Op::Post { "post_pending_transfer" } else { "void_pending_transfer" }],
                });
            }
            link(&mut ch.transfers);
            Some(ch)
        }
    }
}

/// The roles, moves and IDs of a hold's chain, from its key.
fn hold_layout(book: &Book, tenant: &str, hold_call: &Call, _hold: &Hold) -> Vec<(&'static str, usize, u128)> {
    let t = &book.transfers[hold_call.kind];
    hold_chain_ids(book, tenant, t, &hold_call.key(t))
}

/// The role, move (from 1) and ID of each transfer of the chain that holds for `t` with `key`:
/// what a `post` or `void` posts or voids, and what a client reads before it sends one.
pub fn hold_chain_ids(book: &Book, tenant: &str, t: &TransferKind, key: &[String]) -> Vec<(&'static str, usize, u128)> {
    let mut out = Vec::new();
    for (i, m) in t.moves.iter().enumerate() {
        for role in layout(book, m.from.kind, m.to.kind) {
            let position = out.len();
            out.push((role, i + 1, transfer_id(book, tenant, t, Op::Hold, key, position)));
        }
    }
    out
}

fn link(ts: &mut [TbTransfer]) {
    let n = ts.len();
    for (i, x) in ts.iter_mut().enumerate() {
        if i + 1 < n {
            x.flags.insert(0, "linked");
        }
    }
}

pub fn chain_json(ch: &Chain) -> Value {
    let acc = |a: &TbAccount| {
        json!({
            "role": a.role, "of": a.of, "id": id_hex(a.id), "ledger": a.ledger, "code": a.code, "flags": a.flags,
            "user_data_128": id_hex(a.user_data_128), "user_data_64": 0, "user_data_32": a.user_data_32,
        })
    };
    let tr = |x: &TbTransfer| {
        json!({
            "role": x.role, "move": x.move_no, "id": id_hex(x.id),
            "debit_account_id": id_hex(x.debit), "credit_account_id": id_hex(x.credit),
            "amount": x.amount.to_string(), "pending_id": id_hex(x.pending_id),
            "user_data_128": id_hex(x.user_data_128), "user_data_64": 0, "user_data_32": x.user_data_32,
            "timeout": x.timeout, "ledger": x.ledger, "code": x.code, "flags": x.flags,
        })
    };
    json!({
        "accounts": ch.accounts.iter().map(acc).collect::<Vec<_>>(),
        "openings": ch.openings.iter().map(tr).collect::<Vec<_>>(),
        "transfers": ch.transfers.iter().map(tr).collect::<Vec<_>>(),
    })
}

/// What a TigerBeetle client sends for every operation of a scenario, step by step: the
/// accounts it makes sure of, the openings, and the chain; null where it sends nothing. The
/// operations of a `together` are taken in caller order, each from the state before it.
pub fn scenario_chains(book: &Book, tenant: &str, s: &crate::scenario::Scenario) -> Value {
    use crate::scenario::Step;
    let mut state = crate::interp::State::default();
    let mut out = Vec::new();
    let one = |state: &crate::interp::State, c: &Call| -> Value {
        let t = &book.transfers[c.kind];
        let hold = state.holds.get(&(c.kind, c.key(t)));
        let sent = chain(book, tenant, c, hold).map(|ch| chain_json(&ch)).unwrap_or(Value::Null);
        json!({"op": c.op.name(), "kind": t.name, "sent": sent})
    };
    for (i, st) in s.steps.iter().enumerate() {
        match st {
            Step::Call(c) => {
                let mut v = one(&state, c);
                v["step"] = json!(i + 1);
                out.push(v);
                let _ = state.apply(book, c);
            }
            Step::Pass(secs, _) => {
                state.pass(*secs);
            }
            Step::Together(cs) => {
                let before = state.clone();
                for (ci, ops) in cs.iter().enumerate() {
                    for c in ops {
                        let mut v = one(&before, c);
                        v["step"] = json!(i + 1);
                        v["caller"] = json!(ci + 1);
                        out.push(v);
                    }
                }
                for c in cs.iter().flatten() {
                    let _ = state.apply(book, c);
                }
            }
        }
    }
    Value::Array(out)
}
