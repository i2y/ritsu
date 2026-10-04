//! The port of books (DESIGN 3.2, `Books`): what chobo knows of a book, the life of a hold as a
//! state machine, and a ledger to run operations on with chobo's reference interpreter.

use crate::{Found, Machine, Said};
use ritsu_base::text::Text;
use ritsu_units::Unit;
use std::path::Path;

/// What chobo knows of one book that passes its check.
#[derive(Clone, Debug, PartialEq)]
pub struct BookFacts {
    pub name: String,
    pub version: u32,
    pub sha256: String,
    pub units: Vec<BookUnit>,
    pub accounts: Vec<Account>,
    pub transfers: Vec<Transfer>,
}

impl BookFacts {
    /// The unit of ritsu a unit of the book is, by the book's name for it (the name an account
    /// and an amount parameter give).
    pub fn unit(&self, name: &str) -> Option<&Unit> {
        self.units.iter().find(|u| u.name == name).map(|u| &u.unit)
    }
}

/// A unit of the book: its name, how many places a stored amount counts past its point
/// (`unit USD scale 2` counts cents), and the unit it is as ritsu's languages share it (DESIGN
/// 5.4): money in a currency, with tax or without when the book says (`unit 円 incl_tax` is
/// `money[円, incl_tax]`, `unit USD scale 2` is `money[USDc]`), a quantity of the table
/// (`unit kg`), or a count that has a name and nothing else (`unit 個`, a `Dim::Count`).
#[derive(Clone, Debug, PartialEq)]
pub struct BookUnit {
    pub name: String,
    pub scale: u32,
    pub unit: Unit,
}

/// A kind of account: its name, the parameters that make one account of it, its unit, whether it
/// stands for the world outside the book, and its bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct Account {
    pub name: String,
    pub params: Vec<String>,
    pub unit: String,
    pub outside: bool,
    pub lower: Option<Bound>,
    pub upper: Option<Bound>,
}

/// A bound of an account, and the reason a move that would cross it is refused with.
#[derive(Clone, Debug, PartialEq)]
pub struct Bound {
    pub value: i128,
    pub refusal: String,
}

/// A kind of transfer: its parameters, its key, whether it is held first and for how long, its
/// moves, and for each operation the reasons it can be refused with.
#[derive(Clone, Debug, PartialEq)]
pub struct Transfer {
    pub name: String,
    pub params: Vec<TransferParam>,
    /// The parameters of the key, in the order of the key.
    pub key: Vec<String>,
    /// None for a transfer done at once; for a hold, when it expires.
    pub pending: Option<Expiry>,
    pub moves: Vec<Move>,
    /// Each operation (`do`, or `hold`, `post`, `void`), with the reasons the check found it can
    /// be refused with.
    pub refusals: Vec<(String, Vec<String>)>,
    /// The life of a hold of this kind (for a transfer that holds).
    pub machine: Option<Machine>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransferParam {
    pub name: String,
    /// The unit of an amount; None for a string.
    pub unit: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expiry {
    /// Seconds after the hold.
    After(u64),
    Never,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Move {
    pub amount: MoveAmount,
    pub from: MoveRef,
    pub to: MoveRef,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MoveAmount {
    Param(String),
    Literal(i128),
}

/// An account a move names: its kind, and for each of the kind's parameters a parameter of the
/// transfer (`Ok`) or a literal (`Err`).
#[derive(Clone, Debug, PartialEq)]
pub struct MoveRef {
    pub account: String,
    pub args: Vec<Result<String, String>>,
}

/// One operation on a transfer: `do`, or `hold`, `post`, `void`; the arguments by the transfer's
/// parameters (every one for `do` and `hold`, the key's for `post` and `void`), an amount as its
/// whole number, a string as itself; and for a `post`, the amounts to post when not all of it.
#[derive(Clone, Debug, PartialEq)]
pub struct BookCall {
    pub transfer: String,
    pub op: String,
    pub args: Vec<(String, Result<i128, String>)>,
    pub amounts: Option<Vec<(String, i128)>>,
}

/// What an operation came to.
#[derive(Clone, Debug, PartialEq)]
pub enum BookOutcome {
    Done,
    /// The key was used before with the same arguments: nothing happens again.
    DoneBefore,
    /// Refused, with the reason (a bound's, or chobo's own: `expired`, `already_posted`, …).
    Refused(String),
}

/// An account's balance: what is posted, and what holds hold coming in and going out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Balance {
    pub posted: i128,
    pub held_in: i128,
    pub held_out: i128,
}

/// A book with what has happened to it so far (chobo's reference interpreter).
pub trait Ledger {
    /// Do one operation. Err is a call that does not fit the book (no such transfer, an argument
    /// missing or of the wrong type), not a refusal.
    fn apply(&mut self, call: &BookCall) -> Result<BookOutcome, Text>;

    /// Let `seconds` pass: the holds that expire, each its transfer and the values of its key.
    fn pass(&mut self, seconds: u64) -> Vec<(String, Vec<String>)>;

    /// One account's balance, by its kind and the values of its parameters.
    fn balance(&self, account: &str, args: &[String]) -> Result<Balance, Text>;
}

/// What chobo answers for a book. `file` is the book, as the caller reaches it.
pub trait Books {
    /// What chobo knows of the book, when it passes check; else what check says.
    fn facts(&self, file: &Path) -> Result<BookFacts, Vec<Said>>;

    /// Which operations of `transfer` can be refused, and with which reasons, when every amount
    /// it is called with is in `amounts` (DESIGN 7.6, X4).
    fn refusals(&self, file: &Path, transfer: &str, amounts: (i128, i128)) -> Result<Found<Vec<(String, Vec<String>)>>, Vec<Said>>;

    /// The book with nothing in it yet, to run operations on.
    fn open(&self, file: &Path) -> Result<Box<dyn Ledger>, Vec<Said>>;
}
