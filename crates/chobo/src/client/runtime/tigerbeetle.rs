// ── What follows is the same in every book chobo writes for TigerBeetle ──────
//
// A call is one chain of transfers in one request: TigerBeetle takes the chain whole or not at
// all. The bounds TigerBeetle cannot keep with an account's flags are kept by more transfers in
// the chain (a probe for a lower bound above 0, the room and floor accounts for an upper bound and
// a lower bound below 0), laid out as PLAN 0.3 says.

use ::std::future::Future;
use ::tigerbeetle as tb;

/// What a client needs of TigerBeetle: the `Client` of the tigerbeetle crate has it, and so do a
/// reference to one and an `Arc` of one.
pub trait TigerBeetleClient: Send + Sync {
    fn create_accounts(&self, accounts: &[tb::Account]) -> impl Future<Output = Result<Vec<tb::CreateAccountResult>, tb::Error>> + Send;
    fn create_transfers(&self, transfers: &[tb::Transfer]) -> impl Future<Output = Result<Vec<tb::CreateTransferResult>, tb::Error>> + Send;
    fn lookup_accounts(&self, ids: &[u128]) -> impl Future<Output = Result<Vec<tb::Account>, tb::Error>> + Send;
    fn lookup_transfers(&self, ids: &[u128]) -> impl Future<Output = Result<Vec<tb::Transfer>, tb::Error>> + Send;
}

impl TigerBeetleClient for tb::Client {
    fn create_accounts(&self, accounts: &[tb::Account]) -> impl Future<Output = Result<Vec<tb::CreateAccountResult>, tb::Error>> + Send {
        tb::Client::create_accounts(self, accounts)
    }
    fn create_transfers(&self, transfers: &[tb::Transfer]) -> impl Future<Output = Result<Vec<tb::CreateTransferResult>, tb::Error>> + Send {
        tb::Client::create_transfers(self, transfers)
    }
    fn lookup_accounts(&self, ids: &[u128]) -> impl Future<Output = Result<Vec<tb::Account>, tb::Error>> + Send {
        tb::Client::lookup_accounts(self, ids)
    }
    fn lookup_transfers(&self, ids: &[u128]) -> impl Future<Output = Result<Vec<tb::Transfer>, tb::Error>> + Send {
        tb::Client::lookup_transfers(self, ids)
    }
}

impl<T: TigerBeetleClient> TigerBeetleClient for &T {
    fn create_accounts(&self, accounts: &[tb::Account]) -> impl Future<Output = Result<Vec<tb::CreateAccountResult>, tb::Error>> + Send {
        (**self).create_accounts(accounts)
    }
    fn create_transfers(&self, transfers: &[tb::Transfer]) -> impl Future<Output = Result<Vec<tb::CreateTransferResult>, tb::Error>> + Send {
        (**self).create_transfers(transfers)
    }
    fn lookup_accounts(&self, ids: &[u128]) -> impl Future<Output = Result<Vec<tb::Account>, tb::Error>> + Send {
        (**self).lookup_accounts(ids)
    }
    fn lookup_transfers(&self, ids: &[u128]) -> impl Future<Output = Result<Vec<tb::Transfer>, tb::Error>> + Send {
        (**self).lookup_transfers(ids)
    }
}

impl<T: TigerBeetleClient> TigerBeetleClient for ::std::sync::Arc<T> {
    fn create_accounts(&self, accounts: &[tb::Account]) -> impl Future<Output = Result<Vec<tb::CreateAccountResult>, tb::Error>> + Send {
        (**self).create_accounts(accounts)
    }
    fn create_transfers(&self, transfers: &[tb::Transfer]) -> impl Future<Output = Result<Vec<tb::CreateTransferResult>, tb::Error>> + Send {
        (**self).create_transfers(transfers)
    }
    fn lookup_accounts(&self, ids: &[u128]) -> impl Future<Output = Result<Vec<tb::Account>, tb::Error>> + Send {
        (**self).lookup_accounts(ids)
    }
    fn lookup_transfers(&self, ids: &[u128]) -> impl Future<Output = Result<Vec<tb::Transfer>, tb::Error>> + Send {
        (**self).lookup_transfers(ids)
    }
}

/// What a call answers. A refusal is an answer, not an error: nothing has changed, and the reason
/// says why.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    Done,
    DoneBefore,
    Refused(Reason),
}

impl Outcome {
    /// `done`, `done_before` or `refused`, as chobo writes a result.
    pub fn result(self) -> &'static str {
        match self {
            Outcome::Done => "done",
            Outcome::DoneBefore => "done_before",
            Outcome::Refused(_) => "refused",
        }
    }

    /// Why the call was refused; None when it was not.
    pub fn reason(self) -> Option<Reason> {
        match self {
            Outcome::Refused(r) => Some(r),
            _ => None,
        }
    }
}

impl ::std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Outcome::Refused(r) => write!(f, "refused: {r}"),
            other => f.write_str(other.result()),
        }
    }
}

/// Where a hold is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HoldState {
    Held,
    Posted,
    Voided,
    Expired,
}

impl HoldState {
    /// `held`, `posted`, `voided` or `expired`.
    pub fn as_str(self) -> &'static str {
        match self {
            HoldState::Held => "held",
            HoldState::Posted => "posted",
            HoldState::Voided => "voided",
            HoldState::Expired => "expired",
        }
    }
}

impl ::std::fmt::Display for HoldState {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An account's balance: what is posted, what holds put in, and what holds take out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Balance {
    pub posted: i64,
    pub held_in: i64,
    pub held_out: i64,
}

/// Why a call has no answer: an argument the book does not take, TigerBeetle's error for the
/// whole request (none of its events was processed), or something TigerBeetle has or answers that
/// the book does not explain (its accounts or bounds changed since they were made, part of a hold,
/// a balance past a 64-bit integer).
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Argument(String),
    TigerBeetle(tb::Error),
    Answer(String),
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Error::Argument(m) | Error::Answer(m) => f.write_str(m),
            Error::TigerBeetle(e) => write!(f, "chobo: TigerBeetle: {e}"),
        }
    }
}

impl ::std::error::Error for Error {
    fn source(&self) -> Option<&(dyn ::std::error::Error + 'static)> {
        match self {
            Error::TigerBeetle(e) => Some(e),
            _ => None,
        }
    }
}

impl From<tb::Error> for Error {
    fn from(e: tb::Error) -> Error {
        Error::TigerBeetle(e)
    }
}

// The book's definitions, which chobo writes as constants above.

struct UnitDef {
    name: &'static str,
    ledger: u32,
    sink_code: u16,
}

struct BoundDef {
    value: i64,
    reason: Reason,
}

struct AccountDef {
    name: &'static str,
    unit: usize,
    code: u16,
    flagged: bool,
    lower: Option<BoundDef>,
    upper: Option<BoundDef>,
}

enum ArgDef {
    Param(usize),
    Lit(&'static str),
}

struct RefDef {
    kind: usize,
    args: &'static [ArgDef],
}

enum AmountDef {
    Param(usize),
    Lit(i64),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Main,
    Probe,
    ProbeVoid,
    FloorOut,
    RoomBack,
    RoomIn,
    FloorBack,
}

impl Role {
    fn code(self) -> u32 {
        match self {
            Role::Main => 1,
            Role::Probe => 2,
            Role::ProbeVoid => 3,
            Role::FloorOut => 4,
            Role::RoomBack => 5,
            Role::RoomIn => 6,
            Role::FloorBack => 7,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Role::Main => "main",
            Role::Probe => "probe",
            Role::ProbeVoid => "probe_void",
            Role::FloorOut => "floor_out",
            Role::RoomBack => "room_back",
            Role::RoomIn => "room_in",
            Role::FloorBack => "floor_back",
        }
    }
}

struct MoveDef {
    amount: AmountDef,
    from: RefDef,
    to: RefDef,
    roles: &'static [Role],
}

struct ParamDef {
    name: &'static str,
    amount: bool,
}

struct TransferDef {
    name: &'static str,
    params: &'static [ParamDef],
    key: &'static [usize],
    timeout: u32,
    code: u16,
    definition: &'static str,
    moves: &'static [MoveDef],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Do,
    Hold,
    Post,
    Void,
}

impl Op {
    fn name(self) -> &'static str {
        match self {
            Op::Do => "do",
            Op::Hold => "hold",
            Op::Post => "post",
            Op::Void => "void",
        }
    }
}

/// An argument of a call, in the order of the parameters.
#[derive(Clone, Copy)]
enum Value<'a> {
    Str(&'a str),
    Amt(i64),
}

impl Value<'_> {
    fn text(self) -> String {
        match self {
            Value::Str(s) => s.to_string(),
            Value::Amt(a) => a.to_string(),
        }
    }
}

/// The transfer that opens a floor or a room account carries this in `user_data_32`.
const OPENING: u32 = 8;

/// The most events one request takes on every cluster (a replica started with --development).
const REQUEST_MAX: usize = 253;

/// An ID: the first 16 bytes of a SHA-256 over the parts, each with its UTF-8 length in front, as
/// a big-endian integer; 0 and 2^128 - 1 are not IDs in TigerBeetle.
fn ch_id(parts: &[&str]) -> u128 {
    let mut bytes = Vec::new();
    for p in ::std::iter::once("chobo/1").chain(parts.iter().copied()) {
        bytes.extend_from_slice(&(p.len() as u32).to_be_bytes());
        bytes.extend_from_slice(p.as_bytes());
    }
    let d = sha256(&bytes);
    let mut first = [0u8; 16];
    first.copy_from_slice(&d[..16]);
    match u128::from_be_bytes(first) {
        0 => 1,
        u128::MAX => u128::MAX - 1,
        v => v,
    }
}

/// SHA-256 (FIPS 180-4).
fn sha256(bytes: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74,
        0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d,
        0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e,
        0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut message = bytes.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((bytes.len() as u64).wrapping_mul(8)).to_be_bytes());
    for chunk in message.chunks(64) {
        let mut w = [0u32; 64];
        for (word, four) in w.iter_mut().zip(chunk.chunks(4)) {
            *word = u32::from_be_bytes([four[0], four[1], four[2], four[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for (k, wi) in K.iter().zip(w.iter()) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(*k).wrapping_add(*wi);
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
    for (four, word) in out.chunks_mut(4).zip(h) {
        four.copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// An ID as 32 lowercase hex digits.
fn hex(v: u128) -> String {
    format!("{v:032x}")
}

/// A string as JavaScript's JSON.stringify writes it (PLAN 0.3).
fn json_string(s: &str) -> String {
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

/// What a do or hold is called with, as the IDs hash it: the arguments in the order of the
/// parameters.
fn content(t: &TransferDef, v: &[Value<'_>]) -> String {
    let fields: Vec<String> = t
        .params
        .iter()
        .zip(v)
        .map(|(p, x)| match x {
            Value::Amt(a) if p.amount => format!("{}:{a}", json_string(p.name)),
            other => format!("{}:{}", json_string(p.name), json_string(&other.text())),
        })
        .collect();
    format!("{{{}}}", fields.join(","))
}

fn tb_account(id: u128, ledger: u32, code: u16, flagged: bool, user_data_128: u128, user_data_32: u32) -> tb::Account {
    let flags = if flagged { tb::AccountFlags::DebitsMustNotExceedCredits } else { tb::AccountFlags::default() };
    tb::Account { id, user_data_128, user_data_32, ledger, code, flags, ..Default::default() }
}

/// An amount as TigerBeetle takes it.
fn amount(v: i64) -> u128 {
    v as u64 as u128
}

/// A transfer of a chain, with the move (from 0) and the role it is there for.
struct Planned {
    transfer: tb::Transfer,
    role: Role,
    move_no: usize,
}

/// Every transfer of the chain but the last is linked to the next.
fn link(chain: &mut [Planned]) {
    let n = chain.len();
    for x in chain.iter_mut().take(n.saturating_sub(1)) {
        x.transfer.flags = x.transfer.flags | tb::TransferFlags::Linked;
    }
}

/// An account a call names: its ID, and its kind.
struct Placed {
    id: u128,
    kind: usize,
}

struct Runtime<C> {
    client: C,
    tenant: String,
    /// the accounts and openings this value has made sure of: TigerBeetle never removes them
    made: ::std::sync::Mutex<::std::collections::HashSet<u128>>,
}

impl<C: TigerBeetleClient> Runtime<C> {
    fn new(client: C, tenant: &str) -> Runtime<C> {
        Runtime { client, tenant: tenant.to_string(), made: ::std::sync::Mutex::new(::std::collections::HashSet::new()) }
    }

    fn made(&self) -> ::std::sync::MutexGuard<'_, ::std::collections::HashSet<u128>> {
        self.made.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn place(&self, r: &RefDef, v: &[Value<'_>]) -> Placed {
        let mut parts: Vec<String> = vec!["account".into(), BOOK.into(), self.tenant.clone(), ACCOUNTS[r.kind].name.into()];
        for a in r.args {
            parts.push(match a {
                ArgDef::Param(i) => v[*i].text(),
                ArgDef::Lit(s) => s.to_string(),
            });
        }
        let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
        Placed { id: ch_id(&parts), kind: r.kind }
    }

    fn kind_id(&self, kind: usize) -> u128 {
        ch_id(&["kind", BOOK, &self.tenant, ACCOUNTS[kind].name])
    }

    fn transfer_id(&self, t: &TransferDef, op: Op, key: &[String], position: usize) -> u128 {
        let position = position.to_string();
        let mut parts: Vec<&str> = vec!["transfer", BOOK, &self.tenant, t.name, op.name()];
        parts.extend(key.iter().map(String::as_str));
        parts.push(&position);
        ch_id(&parts)
    }

    /// do and hold: `v` has every argument, in the order of the parameters.
    async fn move_(&self, t: &TransferDef, op: Op, v: &[Value<'_>]) -> Result<Outcome, Error> {
        for (p, x) in t.params.iter().zip(v) {
            match x {
                Value::Amt(a) if *a < 0 => return Err(Error::Argument(format!("chobo: {}.{}: {} is {a}, and an amount is from 0 to 2^63 - 1", t.name, op.name(), p.name))),
                _ => {}
            }
        }
        let places: Vec<(Placed, Placed)> = t.moves.iter().map(|m| (self.place(&m.from, v), self.place(&m.to, v))).collect();
        // a move from an account to itself: refused before anything is sent, and the key is not used
        if places.iter().any(|(from, to)| from.id == to.id) {
            return Ok(Outcome::Refused(Reason::SameAccount));
        }
        let key: Vec<String> = t.key.iter().map(|k| v[*k].text()).collect();
        let content_id = ch_id(&["content", BOOK, &self.tenant, t.name, op.name(), t.definition, &content(t, v)]);
        let mut accounts: Vec<tb::Account> = Vec::new();
        let mut openings: Vec<tb::Transfer> = Vec::new();
        let mut chain: Vec<Planned> = Vec::new();
        fn add(accounts: &mut Vec<tb::Account>, a: tb::Account) {
            if !accounts.iter().any(|x| x.id == a.id) {
                accounts.push(a);
            }
        }
        for (i, (m, (from, to))) in t.moves.iter().zip(&places).enumerate() {
            let (fd, td) = (&ACCOUNTS[from.kind], &ACCOUNTS[to.kind]);
            let unit = &UNITS[fd.unit];
            let ledger = unit.ledger;
            let sink = ch_id(&["sink", BOOK, &self.tenant, unit.name]);
            add(&mut accounts, tb_account(from.id, ledger, fd.code, fd.flagged, self.kind_id(from.kind), 0));
            add(&mut accounts, tb_account(to.id, ledger, td.code, td.flagged, self.kind_id(to.kind), 0));
            if m.roles.len() > 1 {
                add(&mut accounts, tb_account(sink, ledger, unit.sink_code, false, 0, 3));
            }
            // the floor and room accounts the move's bounds need, each with the transfer that opens it
            for role in m.roles {
                let bound = |b: &Option<BoundDef>| b.as_ref().map(|b| b.value).unwrap_or(0);
                let (what, owner, open) = match role {
                    Role::FloorOut => ("floor", from, -bound(&fd.lower)),
                    Role::RoomBack => ("room", from, bound(&fd.upper)),
                    Role::RoomIn => ("room", to, bound(&td.upper)),
                    Role::FloorBack => ("floor", to, -bound(&td.lower)),
                    _ => continue,
                };
                let xid = ch_id(&[what, &hex(owner.id)]);
                if accounts.iter().any(|a| a.id == xid) {
                    continue;
                }
                let od = &ACCOUNTS[owner.kind];
                accounts.push(tb_account(xid, ledger, od.code, true, self.kind_id(owner.kind), if what == "floor" { 1 } else { 2 }));
                openings.push(tb::Transfer {
                    id: ch_id(&["opening", &hex(xid)]),
                    debit_account_id: sink,
                    credit_account_id: xid,
                    amount: amount(open),
                    user_data_32: OPENING,
                    ledger,
                    code: od.code,
                    ..Default::default()
                });
            }
            let amt = match m.amount {
                AmountDef::Param(p) => match v[p] {
                    Value::Amt(a) => a,
                    Value::Str(_) => 0,
                },
                AmountDef::Lit(a) => a,
            };
            let (flags, timeout) = if op == Op::Hold { (tb::TransferFlags::Pending, t.timeout) } else { (tb::TransferFlags::default(), 0) };
            for role in m.roles {
                let mut x = tb::Transfer { id: self.transfer_id(t, op, &key, chain.len()), user_data_128: content_id, user_data_32: role.code(), ledger, code: t.code, ..Default::default() };
                match role {
                    Role::Main => {
                        (x.debit_account_id, x.credit_account_id, x.amount, x.flags, x.timeout) = (from.id, to.id, amount(amt), flags, timeout);
                    }
                    Role::Probe => {
                        (x.debit_account_id, x.credit_account_id, x.amount, x.flags) = (from.id, sink, amount(fd.lower.as_ref().map(|b| b.value).unwrap_or(0)), tb::TransferFlags::Pending);
                    }
                    Role::ProbeVoid => {
                        x.pending_id = chain.last().map(|p| p.transfer.id).unwrap_or(0);
                        (x.flags, x.ledger, x.code) = (tb::TransferFlags::VoidPendingTransfer, 0, 0);
                    }
                    Role::FloorOut => {
                        (x.debit_account_id, x.credit_account_id, x.amount, x.flags, x.timeout) = (ch_id(&["floor", &hex(from.id)]), sink, amount(amt), flags, timeout);
                    }
                    Role::RoomBack => {
                        (x.debit_account_id, x.credit_account_id, x.amount, x.flags, x.timeout) = (sink, ch_id(&["room", &hex(from.id)]), amount(amt), flags, timeout);
                    }
                    Role::RoomIn => {
                        (x.debit_account_id, x.credit_account_id, x.amount, x.flags, x.timeout) = (ch_id(&["room", &hex(to.id)]), sink, amount(amt), flags, timeout);
                    }
                    Role::FloorBack => {
                        (x.debit_account_id, x.credit_account_id, x.amount, x.flags, x.timeout) = (sink, ch_id(&["floor", &hex(to.id)]), amount(amt), flags, timeout);
                    }
                }
                chain.push(Planned { transfer: x, role: *role, move_no: i });
            }
        }
        link(&mut chain);
        self.ensure(&accounts, &openings).await?;
        self.send(t, op, &chain).await
    }

    /// Make sure the accounts and the openings are there before a chain needs them: TigerBeetle
    /// refuses a transfer to an account it does not have, and never takes its ID again.
    async fn ensure(&self, accounts: &[tb::Account], openings: &[tb::Transfer]) -> Result<(), Error> {
        let new_accounts: Vec<tb::Account> = {
            let made = self.made();
            accounts.iter().filter(|a| !made.contains(&a.id)).copied().collect()
        };
        for batch in new_accounts.chunks(REQUEST_MAX) {
            let results = self.client.create_accounts(batch).await?;
            for (a, r) in batch.iter().zip(&results) {
                if !matches!(r.status, tb::CreateAccountStatus::Created | tb::CreateAccountStatus::Exists) {
                    return Err(Error::Answer(format!(
                        "chobo: TigerBeetle has the account {} made another way ({:?}): the book's accounts have changed since it was made",
                        hex(a.id),
                        r.status
                    )));
                }
            }
            self.made().extend(batch.iter().map(|a| a.id));
        }
        let new_openings: Vec<tb::Transfer> = {
            let made = self.made();
            openings.iter().filter(|x| !made.contains(&x.id)).copied().collect()
        };
        for batch in new_openings.chunks(REQUEST_MAX) {
            let results = self.client.create_transfers(batch).await?;
            for (x, r) in batch.iter().zip(&results) {
                match r.status {
                    tb::CreateTransferStatus::Created | tb::CreateTransferStatus::Exists => {}
                    tb::CreateTransferStatus::ExistsWithDifferentAmount => {
                        return Err(Error::Answer(format!("chobo: the account {} was opened with another bound: the book's bounds have changed since it was made", hex(x.credit_account_id))));
                    }
                    other => return Err(Error::Answer(format!("chobo: TigerBeetle answered {other:?} for the opening {}", hex(x.id)))),
                }
            }
            self.made().extend(batch.iter().map(|x| x.id));
        }
        Ok(())
    }

    async fn send(&self, t: &TransferDef, op: Op, chain: &[Planned]) -> Result<Outcome, Error> {
        let transfers: Vec<tb::Transfer> = chain.iter().map(|p| p.transfer).collect();
        let results = self.client.create_transfers(&transfers).await?;
        self.read(t, op, chain, &results)
    }

    /// What a chain's results answer: the first that is neither created nor linked_event_failed
    /// decides.
    fn read(&self, t: &TransferDef, op: Op, chain: &[Planned], results: &[tb::CreateTransferResult]) -> Result<Outcome, Error> {
        use tb::CreateTransferStatus as S;
        for (i, (p, r)) in chain.iter().zip(results).enumerate() {
            let refused = |reason| Ok(Outcome::Refused(reason));
            match r.status {
                S::Created | S::LinkedEventFailed => continue,
                // the first transfer there already: the same call, made before; a later one: a
                // chain of another shape
                S::Exists if i == 0 => return Ok(Outcome::DoneBefore),
                S::Exists => return refused(Reason::KeyConflict),
                S::IdAlreadyFailed => return refused(Reason::AlreadyRefused),
                S::ExceedsCredits => {
                    let m = &t.moves[p.move_no];
                    let bound = if p.role == Role::RoomIn { &ACCOUNTS[m.to.kind].upper } else { &ACCOUNTS[m.from.kind].lower };
                    if let Some(b) = bound {
                        return refused(b.reason);
                    }
                }
                S::AccountsMustBeDifferent => return refused(Reason::SameAccount),
                S::PendingTransferAlreadyPosted => return refused(Reason::AlreadyPosted),
                S::PendingTransferAlreadyVoided => return refused(Reason::AlreadyVoided),
                S::PendingTransferExpired => return refused(Reason::Expired),
                S::ExceedsPendingTransferAmount => return refused(Reason::OverHold),
                S::ExistsWithDifferentFlags
                | S::ExistsWithDifferentPendingId
                | S::ExistsWithDifferentTimeout
                | S::ExistsWithDifferentDebitAccountId
                | S::ExistsWithDifferentCreditAccountId
                | S::ExistsWithDifferentAmount
                | S::ExistsWithDifferentUserData128
                | S::ExistsWithDifferentUserData64
                | S::ExistsWithDifferentUserData32
                | S::ExistsWithDifferentLedger
                | S::ExistsWithDifferentCode => return refused(Reason::KeyConflict),
                _ => {}
            }
            return Err(Error::Answer(format!("chobo: TigerBeetle answered {:?} for transfer {i} ({}) of {}.{}", r.status, p.role.name(), t.name, op.name())));
        }
        Ok(Outcome::Done)
    }

    /// The transfers of the chain that held for `key`: their roles, moves and IDs.
    fn held(&self, t: &TransferDef, key: &[String]) -> Vec<(Role, usize, u128)> {
        let mut out = Vec::new();
        for (i, m) in t.moves.iter().enumerate() {
            for role in m.roles {
                let id = self.transfer_id(t, Op::Hold, key, out.len());
                out.push((*role, i, id));
            }
        }
        out
    }

    /// post and void: `key` in the order of the key; `amounts` in the order of the amounts, or
    /// None for all of it.
    async fn end(&self, t: &TransferDef, op: Op, key: &[&str], amounts: Option<&[i64]>) -> Result<Outcome, Error> {
        for a in amounts.unwrap_or(&[]) {
            if *a < 0 {
                return Err(Error::Argument(format!("chobo: {}.post: an amount is {a}, and an amount is from 0 to 2^63 - 1", t.name)));
            }
        }
        let key: Vec<String> = key.iter().map(|k| k.to_string()).collect();
        let held = self.held(t, &key);
        let mains: Vec<(Role, usize, u128)> = held.iter().copied().filter(|(r, _, _)| *r == Role::Main).collect();
        let ids: Vec<u128> = mains.iter().map(|(_, _, id)| *id).collect();
        // read the hold first: a post or void of a hold TigerBeetle does not have would use up its
        // ID for good
        let found = self.client.lookup_transfers(&ids).await?;
        if found.is_empty() {
            return Ok(Outcome::Refused(Reason::NoSuchHold));
        }
        if found.len() != mains.len() {
            return Err(Error::Answer(format!("chobo: TigerBeetle has only part of the hold {}({})", t.name, key.join(", "))));
        }
        let holding: Vec<i64> = mains.iter().map(|(_, _, id)| found.iter().find(|x| x.id == *id).map(|x| x.amount as i64).unwrap_or(0)).collect();
        let mut posted: Vec<i64> = Vec::new();
        if op == Op::Post {
            match amounts {
                None => posted = holding.clone(),
                Some(amounts) => {
                    let amount_params: Vec<usize> = (0..t.params.len()).filter(|i| t.params[*i].amount).collect();
                    for m in t.moves {
                        posted.push(match m.amount {
                            AmountDef::Param(p) => amount_params.iter().position(|x| *x == p).map(|j| amounts[j]).unwrap_or(0),
                            AmountDef::Lit(a) => a,
                        });
                    }
                    // TigerBeetle checks the amount before it checks whether the hold has ended:
                    // ask where the hold is instead
                    if posted.iter().zip(&holding).any(|(p, h)| p > h) {
                        return Ok(Outcome::Refused(match self.state(mains[0].2).await? {
                            Some(HoldState::Held) => Reason::OverHold,
                            Some(HoldState::Posted) => Reason::KeyConflict,
                            Some(HoldState::Voided) => Reason::AlreadyVoided,
                            Some(HoldState::Expired) => Reason::Expired,
                            None => Reason::NoSuchHold,
                        }));
                    }
                }
            }
        }
        let what = if op == Op::Post { format!("[{}]", posted.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",")) } else { "null".to_string() };
        let content_id = ch_id(&["content", BOOK, &self.tenant, t.name, op.name(), t.definition, &what]);
        let mut chain: Vec<Planned> = Vec::new();
        for (role, move_no, id) in held {
            if role == Role::Probe || role == Role::ProbeVoid {
                continue;
            }
            let mut x = tb::Transfer { id: self.transfer_id(t, op, &key, chain.len()), pending_id: id, user_data_128: content_id, user_data_32: role.code(), ..Default::default() };
            match (op, amounts) {
                (Op::Void, _) => x.flags = tb::TransferFlags::VoidPendingTransfer,
                (_, None) => (x.flags, x.amount) = (tb::TransferFlags::PostPendingTransfer, u128::MAX),
                _ => (x.flags, x.amount) = (tb::TransferFlags::PostPendingTransfer, amount(posted[move_no])),
            }
            chain.push(Planned { transfer: x, role, move_no });
        }
        link(&mut chain);
        self.send(t, op, &chain).await
    }

    /// Where the hold of the pending transfer `id` is, as TigerBeetle would answer a void of it
    /// now: a void and a transfer that cannot go through, linked, so that nothing is kept.
    async fn state(&self, id: u128) -> Result<Option<HoldState>, Error> {
        use tb::CreateTransferStatus as S;
        let results = self
            .client
            .create_transfers(&[
                tb::Transfer { id: tb::id(), pending_id: id, flags: tb::TransferFlags::Linked | tb::TransferFlags::VoidPendingTransfer, ..Default::default() },
                tb::Transfer { id: tb::id(), debit_account_id: 1, credit_account_id: 1, ledger: 1, code: 1, ..Default::default() },
            ])
            .await?;
        let (first, second) = match results.as_slice() {
            [a, b] => (a.status, b.status),
            _ => return Err(Error::Answer(format!("chobo: TigerBeetle answered {} results for 2 transfers when asked where a hold is", results.len()))),
        };
        match first {
            S::LinkedEventFailed if second == S::AccountsMustBeDifferent => Ok(Some(HoldState::Held)),
            S::PendingTransferAlreadyPosted => Ok(Some(HoldState::Posted)),
            S::PendingTransferAlreadyVoided => Ok(Some(HoldState::Voided)),
            S::PendingTransferExpired => Ok(Some(HoldState::Expired)),
            S::PendingTransferNotFound => Ok(None),
            _ => Err(Error::Answer(format!("chobo: TigerBeetle answered {first:?}, {second:?} when asked where a hold is"))),
        }
    }

    async fn status(&self, t: &TransferDef, key: &[&str]) -> Result<Option<HoldState>, Error> {
        let key: Vec<String> = key.iter().map(|k| k.to_string()).collect();
        let first = self.transfer_id(t, Op::Hold, &key, 0);
        self.state(first).await
    }

    async fn balance(&self, kind: usize, args: &[&str]) -> Result<Balance, Error> {
        let mut parts: Vec<&str> = vec!["account", BOOK, &self.tenant, ACCOUNTS[kind].name];
        parts.extend(args.iter().copied());
        let found = self.client.lookup_accounts(&[ch_id(&parts)]).await?;
        let Some(a) = found.first() else { return Ok(Balance::default()) };
        let past = || Error::Answer(format!("chobo: the balance of {}({}) is past a 64-bit integer", ACCOUNTS[kind].name, args.join(", ")));
        let posted = difference(a.credits_posted, a.debits_posted).ok_or_else(past)?;
        let held_in = i64::try_from(a.credits_pending).map_err(|_| past())?;
        let held_out = i64::try_from(a.debits_pending).map_err(|_| past())?;
        Ok(Balance { posted, held_in, held_out })
    }
}

/// `plus - minus`, when it is a 64-bit integer.
fn difference(plus: u128, minus: u128) -> Option<i64> {
    if plus >= minus {
        i64::try_from(plus - minus).ok()
    } else {
        i64::try_from(-i128::try_from(minus - plus).ok()?).ok()
    }
}
