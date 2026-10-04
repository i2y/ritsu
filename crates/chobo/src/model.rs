//! The book with its names and units resolved (PLAN B4). Every error of how the book is
//! written is found here; what only shows when the book is called is `check`'s.

use crate::diag::{self, Diag, DiagExt};
use ritsu_base::text::Text;
use crate::parse::{self, AmountExpr, ArgExpr, PendingKind};
use std::collections::BTreeMap;

/// The largest amount, and the largest bound either way: what PostgreSQL's `bigint` holds.
pub const MAX: i128 = i64::MAX as i128;
/// The longest expiry: TigerBeetle's timeout is 32 bits of seconds.
pub const MAX_EXPIRY: u64 = u32::MAX as u64;

/// The reasons chobo gives itself (DESIGN 2.7). A bound may not take one of them.
pub const REASONS: &[&str] =
    &["key_conflict", "already_refused", "same_account", "no_such_hold", "already_posted", "already_voided", "expired", "over_hold"];

#[derive(Clone, Debug)]
pub struct Book {
    pub name: String,
    /// where the name is written, on the `book` line
    pub line: usize,
    pub col: usize,
    pub version: u32,
    pub description: Option<String>,
    pub units: Vec<Unit>,
    pub accounts: Vec<AccountKind>,
    pub transfers: Vec<TransferKind>,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub name: String,
    pub scale: u32,
    /// Whether an amount of money is with tax or without, when the unit says (`unit 円 incl_tax`).
    pub tax: Option<ritsu_units::Tax>,
    /// The unit as ritsu's languages share it (ritsu's DESIGN 5.4): money in a currency of the
    /// table, a quantity of the table, or a count that has a name and nothing else.
    pub ty: ritsu_units::Unit,
    pub line: usize,
    pub col: usize,
}

impl Unit {
    /// The unit as the book writes it, with its tax when it says one (`円 incl_tax`): what a
    /// page shows of an account's unit.
    pub fn shown(&self) -> String {
        match self.tax {
            Some(t) => format!("{} {}", self.name, t.word()),
            None => self.name.clone(),
        }
    }
}

/// The unit of ritsu a unit of chobo is (ritsu's DESIGN 5.4). A name in the table of currencies
/// is money: counted in whole units with scale 0, in hundredths with scale 2 (`USD scale 2` is
/// `money[USDc]`), but yen only in whole yen (its hundredth is `銭`). A unit of the table's
/// quantities (`g`, `kg`, `L`) with scale 0 is that unit. Anything else is a count with its name
/// only, the same as itself and nothing else, so that seats are not counted as items. Err when
/// a tax is written on a unit that is not money (E014).
pub fn unit_type(name: &str, scale: u32, tax: Option<ritsu_units::Tax>) -> Result<ritsu_units::Unit, ()> {
    use ritsu_units::{Rat, Unit as U, table};
    let money = match (table::money(name), scale) {
        (Some(_), 0) => U::money(name, tax),
        (Some((_, f)), 2) if f == Rat::int(1) && name != "円" && name != "JPY" => U::money(&format!("{name}c"), tax),
        _ => None,
    };
    if let Some(m) = money {
        return Ok(m);
    }
    if tax.is_some() {
        return Err(());
    }
    // a quantity moves between accounts as an amount; a temperature or a level of sound is
    // ordered and does not add up, so it is no amount, and is a count of its own name here
    use ritsu_units::Dim;
    Ok(match table::unit(name) {
        Some((dim @ (Dim::Mass | Dim::Length | Dim::Area | Dim::Volume | Dim::Duration), _)) if scale == 0 => U { dim, unit: name.to_string(), tax: None, step: None },
        _ => U::count(name),
    })
}

#[derive(Clone, Debug)]
pub struct Bound {
    pub value: i128,
    pub refusal: String,
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug)]
pub struct AccountKind {
    pub name: String,
    pub params: Vec<String>,
    pub unit: usize,
    pub outside: bool,
    pub lower: Option<Bound>,
    pub upper: Option<Bound>,
    pub description: Option<String>,
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Str,
    Amount(usize),
}

#[derive(Clone, Debug)]
pub struct TParam {
    pub name: String,
    pub ty: Ty,
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expiry {
    /// seconds
    After(u64),
    Never,
}

#[derive(Clone, Debug)]
pub struct TransferKind {
    pub name: String,
    pub params: Vec<TParam>,
    /// the parameters of the key, by their place in `params`, in the order of the key line
    pub key: Vec<usize>,
    pub pending: Option<Expiry>,
    pub moves: Vec<Move>,
    pub description: Option<String>,
    pub line: usize,
    pub col: usize,
    pub key_line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Amount {
    Param(usize),
    Lit(i128),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    Param(usize),
    Lit(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ref {
    pub kind: usize,
    pub args: Vec<Arg>,
}

#[derive(Clone, Debug)]
pub struct Move {
    pub amount: Amount,
    pub from: Ref,
    pub to: Ref,
    pub line: usize,
    pub col: usize,
}

impl Book {
    pub fn unit_of(&self, account: usize) -> &Unit {
        &self.units[self.accounts[account].unit]
    }

    pub fn account(&self, name: &str) -> Option<usize> {
        self.accounts.iter().position(|a| a.name == name)
    }

    pub fn transfer(&self, name: &str) -> Option<usize> {
        self.transfers.iter().position(|t| t.name == name)
    }

    /// A move's account as written in the book: `在庫(sku)`, `在庫("A-1")`, `客`.
    pub fn ref_text(&self, t: &TransferKind, r: &Ref) -> String {
        let a = &self.accounts[r.kind];
        if r.args.is_empty() {
            return a.name.clone();
        }
        let args: Vec<String> = r
            .args
            .iter()
            .map(|x| match x {
                Arg::Param(i) => t.params[*i].name.clone(),
                Arg::Lit(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            })
            .collect();
        format!("{}({})", a.name, args.join(", "))
    }

    /// A move's amount as written in the book.
    pub fn amount_text(&self, t: &TransferKind, m: &Move) -> String {
        match &m.amount {
            Amount::Param(i) => t.params[*i].name.clone(),
            Amount::Lit(v) => format_amount(*v, self.unit_of(m.from.kind).scale),
        }
    }
}

impl TransferKind {
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// The parameters that are amounts, in declaration order.
    pub fn amount_params(&self) -> Vec<usize> {
        (0..self.params.len()).filter(|i| matches!(self.params[*i].ty, Ty::Amount(_))).collect()
    }

    /// The operations a call can make: `do`, or `hold`, `post` and `void`.
    pub fn ops(&self) -> &'static [&'static str] {
        if self.is_pending() { &["hold", "post", "void"] } else { &["do"] }
    }
}

/// `1250` in a unit of scale 2 is `12.50`.
pub fn format_amount(v: i128, scale: u32) -> String {
    if scale == 0 {
        return v.to_string();
    }
    let neg = v < 0;
    let a = v.unsigned_abs();
    let p = 10u128.pow(scale);
    format!("{}{}.{:0width$}", if neg { "-" } else { "" }, a / p, a % p, width = scale as usize)
}

/// A number as written, in the smallest step of `unit` (with `scale` decimal places).
/// Err says why it does not fit: too many decimal places, or out of range.
pub fn parse_amount(text: &str, unit: &str, scale: u32) -> Result<i128, Text> {
    let (neg, body) = match text.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, text),
    };
    let (int, frac) = match body.split_once('.') {
        Some((i, f)) => (i, f),
        None => (body, ""),
    };
    if frac.len() > scale as usize {
        let n = frac.len();
        return Err(if scale == 0 {
            tr!("単位 `{unit}` には小数を書けません（`{text}`）", "the unit `{unit}` takes no decimal places (`{text}`)")
        } else {
            tr!(
                "`{text}` の小数は {n} 桁ですが、単位 `{unit}` で書けるのは {scale} 桁までです",
                "`{text}` has {n} decimal places, and the unit `{unit}` takes {scale}"
            )
        });
    }
    let out_of_range = || {
        tr!(
            "`{text}` は書ける範囲を超えています。いちばん小さい単位で数えて、−(2⁶³ − 1) から 2⁶³ − 1 までです",
            "`{text}` is out of range: from −(2⁶³ − 1) to 2⁶³ − 1, counted in the unit's smallest step"
        )
    };
    let mut v: i128 = 0;
    for d in int.chars().chain(frac.chars()).chain(std::iter::repeat_n('0', scale as usize - frac.len())) {
        v = v * 10 + (d as i128 - '0' as i128);
        if v > MAX {
            return Err(out_of_range());
        }
    }
    Ok(if neg { -v } else { v })
}

/// Resolve the names and units of a parsed book. Every error found is returned; the book
/// only when there are none.
pub fn resolve(b: &parse::Book) -> (Option<Book>, Vec<Diag>) {
    let mut d: Vec<Diag> = Vec::new();

    // units
    let mut units: Vec<Unit> = Vec::new();
    let mut unit_ix: BTreeMap<String, usize> = BTreeMap::new();
    for u in &b.units {
        if let Some(&i) = unit_ix.get(&u.name.text) {
            let (n, l) = (&u.name.text, units[i].line);
            d.push(diag::error("E003", u.name.span.line, u.name.span.col, tr!("単位 `{n}` は {l} 行目で宣言済みです", "the unit `{n}` is already declared on line {l}")));
            continue;
        }
        let scale = match &u.scale {
            None => 0,
            Some((s, sp)) => match s.parse::<u32>() {
                Ok(n) if n <= 18 => n,
                _ => {
                    d.push(diag::error("E001", sp.line, sp.col, tr!("`scale` は 0 から 18 までの整数です", "`scale` is a whole number from 0 to 18")));
                    0
                }
            },
        };
        let tax = u.tax.as_ref().and_then(|(w, _)| ritsu_units::Tax::parse(w));
        let ty = match unit_type(&u.name.text, scale, tax) {
            Ok(t) => t,
            Err(()) => {
                let (n, (w, sp)) = (&u.name.text, u.tax.as_ref().expect("a tax was written"));
                d.push(diag::error("E014", sp.line, sp.col, tr!("単位 `{n}` はお金の単位ではないので、`{w}` を書けません", "the unit `{n}` is not money, so it takes no `{w}`")).hint(tr!(
                    "`{w}` を外してください。税込か税抜かを書けるのは、通貨の名前の単位（円と JPY は scale 0、ほかの通貨は scale 0 か 2）だけです",
                    "take `{w}` off: only a unit named for a currency says whether it is with tax or without (yen and JPY with scale 0, any other currency with scale 0 or 2)"
                )));
                ritsu_units::Unit::count(&u.name.text)
            }
        };
        unit_ix.insert(u.name.text.clone(), units.len());
        units.push(Unit { name: u.name.text.clone(), scale, tax, ty, line: u.line, col: u.name.span.col });
    }

    // account kinds
    let mut accounts: Vec<AccountKind> = Vec::new();
    let mut acct_ix: BTreeMap<String, usize> = BTreeMap::new();
    for a in &b.accounts {
        let name = &a.name.text;
        if let Some(&i) = acct_ix.get(name) {
            let l = accounts[i].line;
            d.push(diag::error("E003", a.name.span.line, a.name.span.col, tr!("勘定 `{name}` は {l} 行目で宣言済みです", "the account `{name}` is already declared on line {l}")));
            continue;
        }
        let mut params: Vec<String> = Vec::new();
        match &a.params {
            Some(ps) if ps.is_empty() => d.push(diag::error(
                "E001",
                a.name.span.line,
                a.name.span.col,
                tr!("引数の無い勘定は、括弧を付けずに書いてください（`account {name} : …`）", "an account without parameters is written without parentheses (`account {name} : …`)"),
            )),
            Some(ps) => {
                for p in ps {
                    let pn = &p.name.text;
                    if params.contains(pn) {
                        d.push(diag::error("E003", p.name.span.line, p.name.span.col, tr!("引数 `{pn}` が二度あります", "the parameter `{pn}` appears twice")));
                    }
                    let ty = &p.ty.text;
                    if ty != "string" {
                        if unit_ix.contains_key(ty) {
                            d.push(diag::error(
                                "E005",
                                p.ty.span.line,
                                p.ty.span.col,
                                tr!(
                                    "勘定の引数の型は string だけです。`{ty}` は単位なので、勘定の引数には使えません",
                                    "an account's parameters are strings; `{ty}` is a unit and cannot be one"
                                ),
                            ));
                        } else {
                            d.push(diag::error("E002", p.ty.span.line, p.ty.span.col, tr!("型 `{ty}` がありません。勘定の引数の型は string です", "there is no type `{ty}`; an account's parameters are strings")));
                        }
                    }
                    params.push(pn.clone());
                }
            }
            None => {}
        }
        let unit = match unit_ix.get(&a.unit.text) {
            Some(&u) => u,
            None => {
                let un = &a.unit.text;
                d.push(diag::error("E002", a.unit.span.line, a.unit.span.col, tr!("単位 `{un}` が宣言されていません", "no unit `{un}` is declared")));
                usize::MAX
            }
        };
        let mut description = None;
        for (i, (s, sp)) in a.descriptions.iter().enumerate() {
            if i == 0 {
                description = Some(s.clone());
            } else {
                d.push(diag::error("E003", sp.line, sp.col, tr!("`description` の行が二つあります", "a second `description` line")));
            }
        }
        let scale = if unit == usize::MAX { 0 } else { units[unit].scale };
        let unit_name = if unit == usize::MAX { a.unit.text.clone() } else { units[unit].name.clone() };
        let mut lower: Option<Bound> = None;
        let mut upper: Option<Bound> = None;
        for bl in &a.bounds {
            let word = if bl.upper { "at most" } else { "at least" };
            let slot = if bl.upper { &mut upper } else { &mut lower };
            if slot.is_some() {
                d.push(diag::error("E003", bl.span.line, bl.span.col, tr!("`{word}` の行が二つあります", "a second `{word}` line")));
                continue;
            }
            let value = match parse_amount(&bl.value, &unit_name, scale) {
                Ok(v) => v,
                Err(t) => {
                    d.push(diag::error("E011", bl.value_span.line, bl.value_span.col, t));
                    continue;
                }
            };
            let refusal = match &bl.refusal {
                None => {
                    d.push(diag::error(
                        "E023",
                        bl.span.line,
                        bl.span.col,
                        tr!(
                            "境界に `refused as <理由>` がありません。この境界で断ったときに返す理由の名前を付けてください",
                            "the bound has no `refused as <reason>`: name the reason a call refused here is given"
                        ),
                    ));
                    String::new()
                }
                Some(n) if REASONS.contains(&n.text.as_str()) => {
                    let r = &n.text;
                    d.push(diag::error(
                        "E023",
                        n.span.line,
                        n.span.col,
                        tr!(
                            "`{r}` は chobo が自分で返す理由の名前なので、境界の理由には使えません",
                            "`{r}` is a reason chobo gives itself and cannot name a bound's refusal"
                        ),
                    ));
                    String::new()
                }
                Some(n) => n.text.clone(),
            };
            *slot = Some(Bound { value, refusal, line: bl.span.line, col: bl.span.col });
        }
        let outside = a.outside.is_some();
        if outside {
            for bl in &a.bounds {
                d.push(diag::error(
                    "E021",
                    bl.span.line,
                    bl.span.col,
                    tr!(
                        "外の勘定 `{name}` には境界を書けません。外の勘定は外の世界を表し、マイナスにもなります",
                        "the outside account `{name}` cannot have a bound: it stands for the world outside the book and may go below 0"
                    ),
                ));
            }
        } else if a.bounds.is_empty() {
            d.push(diag::error(
                "E020",
                a.name.span.line,
                a.name.span.col,
                tr!(
                    "勘定 `{name}` に境界がありません。`at least` か `at most` を書いてください。外の世界を表す勘定なら `outside` を付けてください",
                    "the account `{name}` has no bound; give it `at least` or `at most`, or mark it `outside` if it stands for the world outside the book"
                ),
            ));
        }
        if let Some(u) = &upper {
            if u.value < 0 {
                let v = format_amount(u.value, scale);
                d.push(diag::error(
                    "E022",
                    u.line,
                    u.col,
                    tr!(
                        "上限 {v} は 0 より小さくできません。勘定は残高 0 から始まるので、0 より小さい上限では何も入れられません",
                        "the upper bound {v} cannot be below 0: an account starts at 0, and below 0 nothing could ever be put into it"
                    ),
                ));
            }
        }
        if let (Some(l), Some(u)) = (&lower, &upper) {
            if l.value > u.value {
                let (lv, uv) = (format_amount(l.value, scale), format_amount(u.value, scale));
                d.push(diag::error(
                    "E022",
                    l.line,
                    l.col,
                    tr!("下限 {lv} が上限 {uv} より大きく、残高の取りうる値がありません", "the lower bound {lv} is above the upper bound {uv}, so no balance is allowed"),
                ));
            }
        }
        acct_ix.insert(name.clone(), accounts.len());
        accounts.push(AccountKind {
            name: name.clone(),
            params,
            unit,
            outside,
            lower,
            upper,
            description,
            line: a.line,
            col: a.name.span.col,
        });
    }

    // transfer kinds
    let mut transfers: Vec<TransferKind> = Vec::new();
    let mut tr_ix: BTreeMap<String, usize> = BTreeMap::new();
    for t in &b.transfers {
        let tname = &t.name.text;
        if let Some(&i) = tr_ix.get(tname) {
            let l = transfers[i].line;
            d.push(diag::error("E003", t.name.span.line, t.name.span.col, tr!("振替 `{tname}` は {l} 行目で宣言済みです", "the transfer `{tname}` is already declared on line {l}")));
            continue;
        }
        let mut params: Vec<TParam> = Vec::new();
        for p in &t.params {
            let pn = &p.name.text;
            if params.iter().any(|x| &x.name == pn) {
                d.push(diag::error("E003", p.name.span.line, p.name.span.col, tr!("引数 `{pn}` が二度あります", "the parameter `{pn}` appears twice")));
                continue;
            }
            let ty = if p.ty.text == "string" {
                Ty::Str
            } else if let Some(&u) = unit_ix.get(&p.ty.text) {
                Ty::Amount(u)
            } else {
                let tn = &p.ty.text;
                d.push(diag::error(
                    "E002",
                    p.ty.span.line,
                    p.ty.span.col,
                    tr!("型 `{tn}` がありません。型は string か、宣言した単位の名前です", "there is no type `{tn}`; a type is string or a declared unit"),
                ));
                Ty::Str
            };
            params.push(TParam { name: pn.clone(), ty, line: p.name.span.line, col: p.name.span.col });
        }
        let param = |name: &str| params.iter().position(|p| p.name == name);

        let mut description = None;
        for (i, (s, sp)) in t.descriptions.iter().enumerate() {
            if i == 0 {
                description = Some(s.clone());
            } else {
                d.push(diag::error("E003", sp.line, sp.col, tr!("`description` の行が二つあります", "a second `description` line")));
            }
        }

        // the key
        let mut key: Vec<usize> = Vec::new();
        let mut key_line = t.line;
        match t.keys.as_slice() {
            [] => d.push(diag::error(
                "E030",
                t.name.span.line,
                t.name.span.col,
                tr!(
                    "振替 `{tname}` に `key` の行がありません。どの振替にも冪等のキーが要ります。同じ注文で一度だけ動かすなら `key 注文` のように、一度だけにしたい引数を並べてください",
                    "the transfer `{tname}` has no `key` line; every transfer needs an idempotency key. List the parameters it happens once for, like `key order`"
                ),
            )),
            [k, rest @ ..] => {
                key_line = k.span.line;
                for extra in rest {
                    d.push(diag::error("E003", extra.span.line, extra.span.col, tr!("`key` の行が二つあります。キーは一行にまとめて書いてください", "a second `key` line; write the whole key on one line")));
                }
                for n in &k.names {
                    let x = &n.text;
                    match param(x) {
                        None => d.push(diag::error("E030", n.span.line, n.span.col, tr!("キーの `{x}` は振替 `{tname}` の引数にありません", "`{x}` in the key is not a parameter of `{tname}`"))),
                        Some(i) if key.contains(&i) => d.push(diag::error("E003", n.span.line, n.span.col, tr!("キーに `{x}` が二度あります", "`{x}` appears twice in the key"))),
                        Some(i) => {
                            if matches!(params[i].ty, Ty::Amount(_)) {
                                d.push(diag::error(
                                    "E031",
                                    n.span.line,
                                    n.span.col,
                                    tr!(
                                        "額の引数 `{x}` はキーに入れられません。入れると、額だけが違う二つの呼び出しが、別々の振替として両方通ってしまいます",
                                        "the amount `{x}` cannot be part of the key: two calls that differ only in the amount would both go through, as two transfers"
                                    ),
                                ));
                            }
                            key.push(i);
                        }
                    }
                }
            }
        }

        // pending
        let mut pending: Option<Expiry> = None;
        for (i, p) in t.pendings.iter().enumerate() {
            if i > 0 {
                d.push(diag::error("E003", p.span.line, p.span.col, tr!("`pending` の行が二つあります", "a second `pending` line")));
                continue;
            }
            match &p.kind {
                PendingKind::Missing => d.push(diag::error(
                    "E040",
                    p.span.line,
                    p.span.col,
                    tr!(
                        "`pending` に終わり方がありません。`pending expires after 30 minutes` か `pending never expires` と書いてください",
                        "`pending` does not say how a hold ends; write `pending expires after 30 minutes` or `pending never expires`"
                    ),
                )),
                PendingKind::Never => pending = Some(Expiry::Never),
                PendingKind::After { n, n_span, unit } => {
                    let per = crate::syntax::duration(&unit.text).unwrap_or(1);
                    let secs = n.parse::<u64>().ok().and_then(|x| x.checked_mul(per));
                    match secs {
                        Some(s) if (1..=MAX_EXPIRY).contains(&s) => pending = Some(Expiry::After(s)),
                        Some(s) => d.push(diag::error(
                            "E041",
                            n_span.line,
                            n_span.col,
                            tr!(
                                "有効期限は 1 秒から 4294967295 秒（約 136 年）までです。ここでは {s} 秒です",
                                "the expiry must be from 1 second to 4294967295 seconds (about 136 years); this is {s} seconds"
                            ),
                        )),
                        None => d.push(diag::error(
                            "E041",
                            n_span.line,
                            n_span.col,
                            tr!(
                                "有効期限は 1 秒から 4294967295 秒（約 136 年）までの整数で書いてください",
                                "the expiry is a whole number, from 1 second to 4294967295 seconds (about 136 years)"
                            ),
                        )),
                    }
                }
            }
        }

        // moves
        if t.moves.is_empty() {
            d.push(diag::error(
                "E013",
                t.name.span.line,
                t.name.span.col,
                tr!(
                    "振替 `{tname}` に `move` の行がありません。`move <額> from <勘定> to <勘定>` で、どこからどこへ動かすかを書いてください",
                    "the transfer `{tname}` has no `move` line; write where it moves from and to with `move <amount> from <account> to <account>`"
                ),
            ));
        }
        let mut moves: Vec<Move> = Vec::new();
        for m in &t.moves {
            let mut ok = true;
            let resolve_ref = |r: &parse::AccountRef, d: &mut Vec<Diag>| -> Option<Ref> {
                let an = &r.name.text;
                let Some(&k) = acct_ix.get(an) else {
                    d.push(diag::error("E002", r.name.span.line, r.name.span.col, tr!("勘定 `{an}` が宣言されていません", "no account `{an}` is declared")));
                    return None;
                };
                let want = accounts[k].params.len();
                let given = r.args.as_ref().map(|a| a.len()).unwrap_or(0);
                if want != given || (want == 0 && r.args.is_some()) {
                    let args = |n: usize| if n == 1 { "1 argument".to_string() } else { format!("{n} arguments") };
                    let (wa, ga) = (args(want), given);
                    let msg = if want == 0 {
                        tr!("勘定 `{an}` は引数を取りません。括弧を書かずに `{an}` と書いてください", "the account `{an}` takes no arguments; write `{an}` without parentheses")
                    } else if given == 0 {
                        tr!("勘定 `{an}` は引数を {want} つ取りますが、渡していません", "the account `{an}` takes {wa}, and is given none")
                    } else {
                        tr!("勘定 `{an}` は引数を {want} つ取りますが、渡しているのは {given} つです", "the account `{an}` takes {wa}, and is given {ga}")
                    };
                    d.push(diag::error("E004", r.span.line, r.span.col, msg));
                    return None;
                }
                let mut args = Vec::new();
                let mut good = true;
                for x in r.args.iter().flatten() {
                    match x {
                        ArgExpr::Str(s, _) => args.push(Arg::Lit(s.clone())),
                        ArgExpr::Name(n) => {
                            let pn = &n.text;
                            match param(pn) {
                                None => {
                                    d.push(diag::error("E002", n.span.line, n.span.col, tr!("振替 `{tname}` に引数 `{pn}` がありません", "the transfer `{tname}` has no parameter `{pn}`")));
                                    good = false;
                                }
                                Some(i) => {
                                    if matches!(params[i].ty, Ty::Amount(_)) {
                                        d.push(diag::error(
                                            "E005",
                                            n.span.line,
                                            n.span.col,
                                            tr!(
                                                "額の引数 `{pn}` は勘定の引数に使えません。勘定の引数は string です",
                                                "`{pn}` is an amount and cannot be an account's argument; an account's arguments are strings"
                                            ),
                                        ));
                                        good = false;
                                    }
                                    args.push(Arg::Param(i));
                                }
                            }
                        }
                    }
                }
                if good { Some(Ref { kind: k, args }) } else { None }
            };
            let from = resolve_ref(&m.from, &mut d);
            let to = resolve_ref(&m.to, &mut d);
            if from.is_none() || to.is_none() {
                ok = false;
            }
            // the unit of the move: the accounts', when they are known
            let unit_of = |r: &Option<Ref>| r.as_ref().map(|r| accounts[r.kind].unit).filter(|u| *u != usize::MAX);
            let (fu, tu) = (unit_of(&from), unit_of(&to));
            let move_unit = fu.or(tu);
            let amount = match &m.amount {
                AmountExpr::Name(n) => {
                    let pn = &n.text;
                    match param(pn) {
                        None => {
                            d.push(diag::error("E002", n.span.line, n.span.col, tr!("振替 `{tname}` に引数 `{pn}` がありません", "the transfer `{tname}` has no parameter `{pn}`")));
                            ok = false;
                            None
                        }
                        Some(i) => match params[i].ty {
                            Ty::Str => {
                                d.push(diag::error(
                                    "E005",
                                    n.span.line,
                                    n.span.col,
                                    tr!(
                                        "string の引数 `{pn}` は移動の額に使えません。額には、単位を型にした引数か数を書いてください",
                                        "`{pn}` is a string and cannot be the amount of a move; the amount is a parameter whose type is a unit, or a number"
                                    ),
                                ));
                                ok = false;
                                None
                            }
                            Ty::Amount(u) => Some((Amount::Param(i), Some(u))),
                        },
                    }
                }
                AmountExpr::Num(text, sp) => {
                    let scale = move_unit.map(|u| units[u].scale).unwrap_or(0);
                    let uname = move_unit.map(|u| units[u].name.clone()).unwrap_or_default();
                    if text.starts_with('-') {
                        d.push(diag::error("E011", sp.line, sp.col, tr!("移動の額に負の数は書けません。向きを変えるなら from と to を入れ替えてください", "the amount of a move cannot be negative; swap `from` and `to` to move the other way")));
                        ok = false;
                        None
                    } else {
                        match parse_amount(text, &uname, scale) {
                            Ok(v) => Some((Amount::Lit(v), None)),
                            Err(t) => {
                                d.push(diag::error("E011", sp.line, sp.col, t));
                                ok = false;
                                None
                            }
                        }
                    }
                }
            };
            if let (Some(fu), Some(tu)) = (fu, tu) {
                let au = amount.as_ref().and_then(|(_, u)| *u);
                if fu != tu || au.is_some_and(|a| a != fu) {
                    let fa = &m.from.name.text;
                    let ta = &m.to.name.text;
                    let (fun, tun) = (&units[fu].name, &units[tu].name);
                    let mut msg = tr!(
                        "この移動の単位が合いません。元の勘定 `{fa}` は `{fun}`、先の勘定 `{ta}` は `{tun}`",
                        "the units of this move do not match: the account it moves from, `{fa}`, is in `{fun}`, the one it moves to, `{ta}`, in `{tun}`"
                    );
                    if let (Some(au), AmountExpr::Name(n)) = (au, &m.amount) {
                        let (an, aun) = (&n.text, &units[au].name);
                        msg.ja.push_str(&format!("、額の `{an}` は `{aun}` です"));
                        msg.en.push_str(&format!(", and the amount `{an}` in `{aun}`"));
                    } else {
                        msg.ja.push_str(" です");
                    }
                    msg.ja.push_str("。別の単位へ替えるなら、単位ごとに外の勘定を置き、二つの移動で書いてください");
                    msg.en.push_str("; to exchange one unit for another, write two moves, each through an outside account of its unit");
                    d.push(diag::error("E010", m.span.line, m.span.col, msg));
                    ok = false;
                }
            }
            if ok {
                if let (Some(from), Some(to), Some((amount, _))) = (from, to, amount) {
                    moves.push(Move { amount, from, to, line: m.span.line, col: m.span.col });
                }
            }
        }
        tr_ix.insert(tname.clone(), transfers.len());
        transfers.push(TransferKind {
            name: tname.clone(),
            params,
            key,
            pending,
            moves,
            description,
            line: t.line,
            col: t.name.span.col,
            key_line,
        });
    }

    if d.is_empty() {
        let book = Book { name: b.name.text.clone(), line: b.name.span.line, col: b.name.span.col, version: b.version, description: b.description.clone(), units, accounts, transfers };
        (Some(book), d)
    } else {
        (None, d)
    }
}

/// Lex, parse and resolve. The diagnostics are those of the first stage that finds any.
pub fn load(src: &str) -> (Option<Book>, Vec<Diag>) {
    let (lines, mut d) = crate::syntax::lex(src);
    let (ast, pd) = parse::parse(&lines);
    d.extend(pd);
    if !d.is_empty() {
        sort(&mut d);
        return (None, d);
    }
    let Some(ast) = ast else { return (None, d) };
    let (book, mut d) = resolve(&ast);
    sort(&mut d);
    (book, d)
}

pub fn sort(d: &mut [Diag]) {
    d.sort_by(|a, b| (a.line, a.col, a.code).cmp(&(b.line, b.col, b.code)));
}

