//! The clients chobo writes (DESIGN 4.3, 4.5, 14): for each of TypeScript, Python, Go and Rust,
//! one that calls the SQL functions of `chobo build --target postgres`, one that sends TigerBeetle
//! the chains of PLAN 0.3, and one that makes the calls on the tables of the MySQL family. The
//! three of a language have the same names, arguments and results, so that a caller moves from
//! one database to another without changing its code.
//!
//! The book's part of a client is written here, a language at a time; what every book shares
//! (IDs, chains, reading TigerBeetle's answers) is the runtime under `runtime/`, which each
//! client carries along, so that what it computes is the same text in every book.

pub mod go;
pub mod python;
pub mod rust;
pub mod typescript;
pub mod xid;

use crate::diag::{self, DiagExt, Diag, OpLine};
use crate::ids;
use crate::model::*;
use crate::witness::Builder;
use serde_json::json;
use std::collections::BTreeMap;

/// The most events one TigerBeetle request takes on every cluster: a replica started with
/// `--development` takes 253 (32 KiB), one without it 8189 (DESIGN 3.1, E060).
pub const REQUEST_MAX: usize = 253;

/// How many transfers the chain of a `do` or `hold` of `t` has: the most any of its
/// operations sends in one request (`post` and `void` leave out the probes).
pub fn chain_len(book: &Book, t: &TransferKind) -> usize {
    t.moves.iter().map(|m| ids::layout(book, m.from.kind, m.to.kind).len()).sum()
}

/// E060: an operation whose chain does not fit one TigerBeetle request. The accounts and the
/// openings a client makes sure of are sent in requests of their own, as many as they need.
pub fn check_requests(book: &Book) -> Vec<Diag> {
    let mut d = Vec::new();
    for (k, t) in book.transfers.iter().enumerate() {
        let n = chain_len(book, t);
        if n <= REQUEST_MAX {
            continue;
        }
        let max = REQUEST_MAX;
        let op = if t.is_pending() { "hold" } else { "do" };
        let name = &t.name;
        let moves = t.moves.len();
        let mut b = Builder::new(book);
        let c = b.first_call(k, &BTreeMap::new());
        let call = crate::scenario::call_text(book, &c);
        let line = OpLine {
            text: tr!("{call}  一つのリクエストで {n} 件の振替を送る", "{call}  sends {n} transfers in one request"),
            json: {
                let mut v = crate::scenario::call_to_json(book, &c);
                v["transfers"] = json!(n);
                v
            },
        };
        d.push(
            diag::error(
                "E060",
                t.line,
                t.col,
                tr!(
                    "{name}.{op} は {moves} 個の移動を、{n} 件の振替のチェーンとして TigerBeetle に一つのリクエストで送ります。--development で立てたレプリカは、一つのリクエストで {max} 件までしか受け取りません",
                    "{name}.{op} sends its {moves} moves as a chain of {n} TigerBeetle transfers, in one request; a replica started with --development takes at most {max} in one"
                ),
            )
            .with_ops(vec![line])
            .hint(tr!(
                "振替の種類を、移動の少ないいくつかの種類に分けます。境界のある勘定のあいだの移動は、一つで最大 6 件の振替になります",
                "Split the transfer kind into kinds with fewer moves; a move between bounded accounts takes up to 6 transfers"
            )),
        );
    }
    d
}

/// Every reason a call of the book can be refused with: the bounds' own names, in the order
/// the book gives them, then the ones chobo gives itself.
pub fn reasons(book: &Book) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in &book.accounts {
        for b in [&a.lower, &a.upper].into_iter().flatten() {
            if !out.contains(&b.refusal) {
                out.push(b.refusal.clone());
            }
        }
    }
    for r in REASONS {
        if !out.iter().any(|x| x == r) {
            out.push(r.to_string());
        }
    }
    out
}

/// The part of a client that differs from book to book, before a language writes it.
pub struct Plan {
    pub units: Vec<UnitPlan>,
    pub accounts: Vec<AccountPlan>,
    pub transfers: Vec<TransferPlan>,
}

pub struct UnitPlan {
    pub name: String,
    pub ledger: u32,
    /// the code of the unit's sink account
    pub sink_code: u16,
}

pub struct AccountPlan {
    pub name: String,
    pub params: Vec<String>,
    pub unit: String,
    pub code: u16,
    /// `debits_must_not_exceed_credits`: a lower bound of 0 or more
    pub flagged: bool,
    pub lower: Option<(i128, String)>,
    pub upper: Option<(i128, String)>,
}

pub enum ArgPlan {
    Param(usize),
    Lit(String),
}

pub enum AmountPlan {
    Param(usize),
    Lit(i128),
}

pub struct RefPlan {
    pub kind: String,
    pub args: Vec<ArgPlan>,
}

pub struct MovePlan {
    pub amount: AmountPlan,
    pub from: RefPlan,
    pub to: RefPlan,
    /// the transfers of the move, by their roles (PLAN 0.3)
    pub roles: Vec<&'static str>,
}

pub struct ParamPlan {
    pub name: String,
    pub amount: bool,
}

pub struct TransferPlan {
    pub name: String,
    pub params: Vec<ParamPlan>,
    /// the parameters of the key, by their place, in the order of the key line
    pub key: Vec<usize>,
    pub pending: bool,
    /// the expiry in seconds; 0 when it never expires, or does not hold
    pub timeout: u64,
    pub code: u16,
    pub definition: String,
    pub moves: Vec<MovePlan>,
}

impl TransferPlan {
    /// The parameters that are amounts, by their place.
    pub fn amounts(&self) -> Vec<usize> {
        (0..self.params.len()).filter(|i| self.params[*i].amount).collect()
    }

    /// The parameters of the key, by their place, in the order they are declared: how a
    /// `post`, `void` and `status` take them.
    pub fn key_declared(&self) -> Vec<usize> {
        (0..self.params.len()).filter(|i| self.key.contains(i)).collect()
    }
}

pub fn plan(book: &Book) -> Plan {
    let units = book.units.iter().map(|u| UnitPlan { name: u.name.clone(), ledger: ids::ledger(u), sink_code: ids::code(&book.name, "sink", &u.name) }).collect();
    let accounts = book
        .accounts
        .iter()
        .map(|a| AccountPlan {
            name: a.name.clone(),
            params: a.params.clone(),
            unit: book.units[a.unit].name.clone(),
            code: ids::code(&book.name, "account", &a.name),
            flagged: ids::flagged(a),
            lower: a.lower.as_ref().map(|b| (b.value, b.refusal.clone())),
            upper: a.upper.as_ref().map(|b| (b.value, b.refusal.clone())),
        })
        .collect();
    let rf = |r: &Ref| RefPlan {
        kind: book.accounts[r.kind].name.clone(),
        args: r
            .args
            .iter()
            .map(|a| match a {
                Arg::Param(i) => ArgPlan::Param(*i),
                Arg::Lit(s) => ArgPlan::Lit(s.clone()),
            })
            .collect(),
    };
    let transfers = book
        .transfers
        .iter()
        .map(|t| TransferPlan {
            name: t.name.clone(),
            params: t.params.iter().map(|p| ParamPlan { name: p.name.clone(), amount: matches!(p.ty, Ty::Amount(_)) }).collect(),
            key: t.key.clone(),
            pending: t.is_pending(),
            timeout: match t.pending {
                Some(Expiry::After(s)) => s,
                _ => 0,
            },
            code: ids::code(&book.name, "transfer", &t.name),
            definition: ids::definition(book, t),
            moves: t
                .moves
                .iter()
                .map(|m| MovePlan {
                    amount: match &m.amount {
                        Amount::Param(i) => AmountPlan::Param(*i),
                        Amount::Lit(v) => AmountPlan::Lit(*v),
                    },
                    from: rf(&m.from),
                    to: rf(&m.to),
                    roles: ids::layout(book, m.from.kind, m.to.kind),
                })
                .collect(),
        })
        .collect();
    Plan { units, accounts, transfers }
}

/// A client written into a package of ritsu's (ritsu's DESIGN 9.3, DESIGN 4.3): the amounts of
/// money and of the quantities its calls take are the package's types of their units, which its
/// `units` module declares (TypeScript's `units.ts` and Python's `units.py` beside the package's
/// `books/`, Go's package at `<module>/units`), the same types the package's rules take and return.
/// An amount of one unit is then not given where the book takes another, by the type checker of the
/// language the caller writes. A count stays a plain integer.
#[derive(Clone, Debug, Default)]
pub struct InPackage {
    /// The Go import path of the package's directory.
    pub module: String,
}

/// The name of the type an amount of `ty` has in a client in a package: its unit's type in the
/// package's `units` module ([`ritsu_units::Unit::type_name`]). None outside a package, and for a
/// count.
pub fn unit_type(book: &Book, ty: &Ty, pkg: Option<&InPackage>) -> Option<String> {
    match (pkg, ty) {
        (Some(_), Ty::Amount(u)) => book.units[*u].ty.type_name(),
        _ => None,
    }
}

/// The units the amounts of the book's calls are in, of those a package of ritsu's has a type of
/// (money and the quantities, as the book writes them), for the package to declare them in its
/// `units` module.
pub fn package_units(book: &Book) -> Vec<ritsu_units::Unit> {
    let mut out: Vec<ritsu_units::Unit> = Vec::new();
    for t in &book.transfers {
        for p in &t.params {
            if let Ty::Amount(u) = p.ty {
                let unit = &book.units[u].ty;
                if unit.type_name().is_some() && !out.contains(unit) {
                    out.push(unit.clone());
                }
            }
        }
    }
    out
}

/// The first line of every file chobo writes for a book.
/// The head every file chobo writes from a book begins with (ritsu's DESIGN 9.2): ritsu's version,
/// and the `.book` with the digest of its bytes, as `origin` names it (the file's name, or its path
/// from a project's root under `ritsu gen`), each line a comment of `c`.
pub fn head(book: &Book, origin: &Origin, c: Comment) -> String {
    let version = book.version.to_string();
    let source = origin.source("book", &book.name, &version);
    format!("{}{}", c.line(&generated("chobo")), c.line(source.line().get(ritsu_base::text::Lang::En)))
}

pub use ritsu_emit::header::{Comment, Origin, generated};

/// Names made unique: the second of two that come out the same gets `_2`, the third `_3`.
pub use ritsu_emit::ident::unique;
