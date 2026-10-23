//! The Rust clients (DESIGN 14): a module each, `<module>.rs`, with the book's part (the types of
//! the arguments, the book value and its calls, the book's definitions) and what every book shares
//! (the runtime under `runtime/`, put in as it is), so that a crate takes the client with one `mod`
//! line and the crates its Cargo.toml names.
//!
//! The clients are async. TigerBeetle's own client for Rust is (it does not depend on a runtime),
//! and so is sqlx, which the PostgreSQL client and the client of the MySQL family call through, on
//! tokio. What a call is given goes through a trait of the runtime (`TigerBeetleClient`,
//! `Database`), which the drivers' own types implement, so that a caller hands over its pool, its
//! connection or its transaction, and a test wraps one to see what is sent, as the Go clients take
//! an interface. The module builds warning-free on the 2021 and the 2024 editions, clippy's lints
//! among them.

use super::*;
use super::xid;
use crate::postgres;

const TB_RUNTIME: &str = include_str!("runtime/tigerbeetle.rs");
const PG_RUNTIME: &str = include_str!("runtime/postgres.rs");
const MY_RUNTIME: &str = include_str!("runtime/mysql.rs");

use ritsu_emit::words::rust::KEYWORDS;

/// A crate a client needs, as the Cargo.toml of the crate it is in names it: the version is the
/// one its tests run (crates/chobo/tools/runner/rust/Cargo.lock).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crate {
    pub name: &'static str,
    pub version: &'static str,
    /// whether the crate's default features are left on
    pub default_features: bool,
    pub features: &'static [&'static str],
}

impl Crate {
    /// The line of a Cargo.toml's `[dependencies]`.
    pub fn line(&self) -> String {
        let mut parts = vec![format!("version = \"{}\"", self.version)];
        if !self.default_features {
            parts.push("default-features = false".into());
        }
        if !self.features.is_empty() {
            parts.push(format!("features = [{}]", self.features.iter().map(|f| format!("\"{f}\"")).collect::<Vec<_>>().join(", ")));
        }
        if parts.len() == 1 { format!("{} = \"{}\"", self.name, self.version) } else { format!("{} = {{ {} }}", self.name, parts.join(", ")) }
    }
}

const SQLX: &str = "0.9.0";
const TOKIO: &str = "1.54.1";
/// TigerBeetle's client for Rust, at the version of the replica the tests run (0.17.9): a client
/// newer than the cluster is refused, so this one is pinned.
pub const TIGERBEETLE: &str = "=0.17.9";

/// The crates the client of a database needs.
pub fn crates(b: crate::target::Backend) -> &'static [Crate] {
    use crate::target::Backend;
    const PG: &[Crate] = &[
        Crate { name: "sqlx", version: SQLX, default_features: false, features: &["runtime-tokio", "postgres"] },
        Crate { name: "tokio", version: TOKIO, default_features: true, features: &["sync", "time"] },
    ];
    const TB: &[Crate] = &[Crate { name: "tigerbeetle", version: TIGERBEETLE, default_features: true, features: &[] }];
    const MY: &[Crate] = &[
        Crate { name: "sqlx", version: SQLX, default_features: false, features: &["runtime-tokio", "mysql"] },
        Crate { name: "tokio", version: TOKIO, default_features: true, features: &["sync", "time"] },
    ];
    match b {
        Backend::Postgres => PG,
        Backend::TigerBeetle => TB,
        Backend::MySql => MY,
    }
}

/// The keywords a raw identifier (`r#type`) cannot be.
const NOT_RAW: [&str; 4] = ["self", "Self", "super", "crate"];

/// A name of the book as a Rust field, method or module takes it: as it is, `r#` and it when it is
/// a keyword (`r#type`), and with `_` after it when it is a keyword a raw identifier cannot be
/// (`self_`) or `_`, which names nothing.
pub fn ident(s: &str) -> String {
    if s == "_" || NOT_RAW.contains(&s) {
        format!("{s}_")
    } else if KEYWORDS.contains(&s) {
        format!("r#{s}")
    } else {
        s.to_string()
    }
}

/// Whether a Rust name may begin with `c`, a character a name of chobo's may hold: `_`, or a letter
/// that is XID_Start (xid.rs keeps out the letters that are not).
pub fn begins(c: char) -> bool {
    c == '_' || (c.is_alphabetic() && !has(xid::NOT_FIRST, c))
}

/// Whether a Rust name may hold `c` after its first, `c` a character a name of chobo's may hold:
/// `_`, or a letter or a number that is XID_Continue.
pub fn holds(c: char) -> bool {
    c == '_' || (c.is_alphanumeric() && !has(xid::NOT_AFTER, c))
}

/// Whether one of the ranges of `table` has `c`.
fn has(table: &[(u32, u32)], c: char) -> bool {
    let u = c as u32;
    table.binary_search_by(|&(lo, hi)| if hi < u { std::cmp::Ordering::Less } else if lo > u { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal }).is_ok()
}

/// A name of the book as a Rust type's name begins: `take_back` is `TakeBack`, `引当` is `引当`. One
/// whose first letter can only follow another (a digit, after `_1`) keeps a `_` in front, and
/// `Self` gets one after it.
pub fn type_base(s: &str) -> String {
    let p = ritsu_emit::ident::pascal(s);
    let p = if p.chars().next().is_some_and(begins) { p } else { format!("_{p}") };
    if KEYWORDS.contains(&p.as_str()) { format!("{p}_") } else { p }
}

/// The module: the book's name in lower case when it is an ASCII identifier, else the file's name
/// (`inventory.ja.book` is `inventory_ja`), else `book`, as the Go package (a module named in
/// Japanese takes a `#[path]` in Rust). A keyword, and a name of a crate the client reads, are
/// passed over.
pub fn module(book: &Book, stem: &str) -> String {
    let file = stem.replace(['.', '-'], "_");
    for c in [book.name.to_ascii_lowercase(), file.to_ascii_lowercase()] {
        let crates = ["std", "core", "alloc", "sqlx", "tokio", "tigerbeetle"];
        if ritsu_emit::ident::is_ascii_ident(&c) && !KEYWORDS.contains(&c.as_str()) && !crates.contains(&c.as_str()) && c != "_" {
            return c;
        }
    }
    "book".into()
}

/// The methods of the book value, one per transfer kind, kept apart from `balance` and `expire`.
pub fn members(book: &Book) -> Vec<String> {
    unique(book.transfers.iter().map(|t| ident(&t.name)).collect(), &["balance", "expire"])
}

/// The fields of a struct of the parameters `which` of `t`.
pub fn fields(t: &TransferKind, which: &[usize]) -> Vec<String> {
    unique(which.iter().map(|i| ident(&t.params[*i].name)).collect(), &[])
}

/// The fields of an account kind's arguments.
pub fn account_fields(a: &AccountKind) -> Vec<String> {
    unique(a.params.iter().map(|p| ident(p)).collect(), &[])
}

/// The methods of `Balances`, one per account kind.
pub fn balance_methods(book: &Book) -> Vec<String> {
    unique(book.accounts.iter().map(|a| ident(&a.name)).collect(), &[])
}

/// What the types of a transfer kind's arguments begin with (`ReserveArgs`, `ReserveKey`,
/// `ReserveAmounts`, `ReserveCalls`), one per transfer kind.
pub fn transfer_types(book: &Book) -> Vec<String> {
    unique(book.transfers.iter().map(|t| type_base(&t.name)).collect(), &[])
}

/// What the type of an account kind's arguments begins with (`StockAccount`).
pub fn account_types(book: &Book) -> Vec<String> {
    unique(book.accounts.iter().map(|a| type_base(&a.name)).collect(), &[])
}

/// The variants of `Reason`: the ones chobo gives itself have their names (`KeyConflict`, …), and
/// the bounds' own, first in the enum, the names left over.
pub fn reason_variants(book: &Book) -> Vec<(String, String)> {
    let own: Vec<String> = REASONS.iter().map(|r| type_base(r)).collect();
    let taken: Vec<&str> = own.iter().map(String::as_str).collect();
    let all = reasons(book);
    let bounds: Vec<&String> = all.iter().filter(|r| !REASONS.contains(&r.as_str())).collect();
    let mut out: Vec<(String, String)> = bounds.iter().zip(unique(bounds.iter().map(|r| type_base(r)).collect(), &taken)).map(|(r, v)| (r.to_string(), v)).collect();
    out.extend(REASONS.iter().zip(own).map(|(r, v)| (r.to_string(), v)));
    out
}

// ── E063 ──────────────────────────────────────────────────────────────────

/// The names Rust cannot take as identifiers (E063): one with a character an identifier does not
/// have where the name has it. A name of chobo is a letter or `_`, then letters, digits and `_`, as
/// Unicode counts them (syntax.rs); a Rust identifier is XID_Start or `_`, then XID_Continue, which
/// leave out digits that are not decimal (`²`, `½`), and letters and marks that only follow another
/// (`ْ`). Every name the client writes as an identifier is held to it: the transfers', the
/// accounts', the parameters', and the bounds' reasons (the variants of `Reason`).
pub fn check_names(book: &Book) -> Vec<Diag> {
    let mut d = Vec::new();
    for t in &book.transfers {
        d.extend(not_an_ident(&t.name, tr!("振替の名前", "The transfer's name"), t.line, t.col));
        for p in &t.params {
            d.extend(not_an_ident(&p.name, tr!("引数の名前", "The parameter's name"), p.line, p.col));
        }
    }
    for a in &book.accounts {
        d.extend(not_an_ident(&a.name, tr!("勘定の名前", "The account's name"), a.line, a.col));
        for p in &a.params {
            d.extend(not_an_ident(p, tr!("勘定の引数の名前", "The name of the account's parameter"), a.line, a.col));
        }
        for b in [&a.lower, &a.upper].into_iter().flatten() {
            d.extend(not_an_ident(&b.refusal, tr!("拒否の理由", "The reason"), b.line, b.col));
        }
    }
    crate::model::sort(&mut d);
    d.dedup_by(|a, b| a.line == b.line && a.col == b.col && a.message == b.message);
    d
}

/// E063 for `name`, when it has a character Rust takes in no identifier where the name has it.
fn not_an_ident(name: &str, what: ritsu_base::text::Text, line: usize, col: usize) -> Option<Diag> {
    let (at, c) = name.chars().enumerate().find(|(i, c)| !(if *i == 0 { begins(*c) } else { holds(*c) }))?;
    let cp = format!("U+{:04X}", c as u32);
    let msg = if at == 0 {
        tr!(
            "{{what}} `{name}` は {c}（{cp}）で始まります。Rust の識別子の先頭には、この字を使えません",
            "{{what}} `{name}` begins with {c} ({cp}), which no Rust identifier begins with"
        )
    } else {
        tr!("{{what}} `{name}` に {c}（{cp}）があります。Rust の識別子には、この字を使えません", "{{what}} `{name}` has {c} ({cp}), which no Rust identifier holds")
    }
    .sub("what", &what);
    Some(diag::error("E063", line, col, msg).hint(tr!(
        "名前からその字を外すか、ほかの字に替えてください。Rust の識別子は、文字か `_` で始まり、そのあとに文字と十進の数字と `_` が続きます（たとえば `²` は `2` にしてください）",
        "Take the character out of the name, or write another in its place: a Rust identifier is a letter or `_`, then letters, decimal digits and `_` (write `²` as `2`, for one)"
    )))
}

// ── the book's part ───────────────────────────────────────────────────────

/// A string as a Rust literal: JSON's escapes, which Rust reads as the same string.
use ritsu_emit::lit::json as q;

/// A Rust raw string: as many `#` as the text needs, so that SQL is written as it is.
fn raw(s: &str) -> String {
    let mut n = 0;
    while s.contains(&format!("\"{}", "#".repeat(n))) {
        n += 1;
    }
    let h = "#".repeat(n);
    format!("r{h}\"{s}\"{h}")
}

/// Text in a doc comment: on one line.
fn doc(s: &str) -> String {
    ritsu_emit::header::one_line(s).into_owned()
}

fn moves_text(book: &Book, t: &TransferKind) -> String {
    let moves: Vec<String> = t.moves.iter().map(|m| format!("{} from {} to {}", book.amount_text(t, m), book.ref_text(t, &m.from), book.ref_text(t, &m.to))).collect();
    let keys: Vec<&str> = t.key.iter().map(|i| t.params[*i].name.as_str()).collect();
    format!("{}; once per {}", moves.join(", then "), keys.join(", "))
}

/// The type of an amount: an `i64`, or in a package of ritsu's the package's type of its unit
/// (`crate::units::USDc`), which the client reads with `i64::from`.
fn amount_type(book: &Book, ty: &Ty, in_pkg: Option<&InPackage>) -> String {
    match unit_type(book, ty, in_pkg) {
        Some(u) => format!("crate::units::{u}"),
        None => "i64".into(),
    }
}

/// An amount field as the runtime takes it.
fn amount_value(book: &Book, ty: &Ty, obj: &str, field: &str, in_pkg: Option<&InPackage>) -> String {
    if unit_type(book, ty, in_pkg).is_some() { format!("i64::from({obj}.{field})") } else { format!("{obj}.{field}") }
}

/// A struct of fields, with a lifetime when a field is a string.
fn struct_type(doc_line: &str, name: &str, rows: &[(String, String)], derive: &str) -> String {
    let lt = if rows.iter().any(|(_, t)| t.starts_with('&')) { "<'a>" } else { "" };
    let mut o = format!("/// {}\n#[derive({derive})]\npub struct {name}{lt} {{\n", doc(doc_line));
    for (f, t) in rows {
        o.push_str(&format!("    pub {f}: {t},\n"));
    }
    o.push_str("}\n\n");
    o
}

/// Whether a type of the arguments has a lifetime: a struct of strings has one.
fn has_lifetime(t: &TransferKind, which: &[usize]) -> bool {
    which.iter().any(|i| matches!(t.params[*i].ty, Ty::Str))
}

fn lt_of(t: &TransferKind, which: &[usize]) -> &'static str {
    if has_lifetime(t, which) { "<'_>" } else { "" }
}

fn arg_types(book: &Book, in_pkg: Option<&InPackage>) -> String {
    let derive = "Clone, Copy, Debug, PartialEq, Eq";
    let mut o = String::new();
    for (t, base) in book.transfers.iter().zip(transfer_types(book)) {
        let ty = |i: &usize| match t.params[*i].ty {
            Ty::Str => "&'a str".to_string(),
            ref a => amount_type(book, a, in_pkg),
        };
        let all: Vec<usize> = (0..t.params.len()).collect();
        let rows: Vec<(String, String)> = fields(t, &all).into_iter().zip(all.iter().map(ty)).collect();
        o.push_str(&struct_type(&format!("The arguments of {}: {}.", t.name, moves_text(book, t)), &format!("{base}Args"), &rows, derive));
        if t.is_pending() {
            let keys = postgres::key_params(t);
            let rows: Vec<(String, String)> = fields(t, &keys).into_iter().zip(keys.iter().map(ty)).collect();
            o.push_str(&struct_type(&format!("The key of a hold of {}.", t.name), &format!("{base}Key"), &rows, derive));
            let ap = t.amount_params();
            if !ap.is_empty() {
                let rows: Vec<(String, String)> = fields(t, &ap).into_iter().zip(ap.iter().map(ty)).collect();
                o.push_str(&struct_type(&format!("What a post of {} posts, when not all of it.", t.name), &format!("{base}Amounts"), &rows, derive));
            }
        }
    }
    for (a, base) in book.accounts.iter().zip(account_types(book)) {
        if !a.params.is_empty() {
            let rows: Vec<(String, String)> = account_fields(a).into_iter().map(|n| (n, "&'a str".to_string())).collect();
            o.push_str(&struct_type(&format!("The arguments of the account {}.", a.name), &format!("{base}Account"), &rows, derive));
        }
    }
    o
}

fn reason_type(book: &Book) -> String {
    let vs = reason_variants(book);
    let mut o = format!("/// Why a call of {} can be refused: the bounds' own reasons, then the ones chobo gives itself.\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum Reason {{\n", doc(&book.name));
    for (r, v) in &vs {
        o.push_str(&format!("    /// `{}`\n    {v},\n", doc(r)));
    }
    o.push_str("}\n\nimpl Reason {\n    /// The reason as the book and chobo name it.\n    pub fn as_str(self) -> &'static str {\n        match self {\n");
    for (r, v) in &vs {
        o.push_str(&format!("            Reason::{v} => {},\n", q(r)));
    }
    o.push_str("        }\n    }\n\n    /// The reason a database answers by its name.\n    fn from_name(name: &str) -> Option<Reason> {\n        match name {\n");
    for (r, v) in &vs {
        o.push_str(&format!("            {} => Some(Reason::{v}),\n", q(r)));
    }
    o.push_str("            _ => None,\n        }\n    }\n}\n\nimpl ::std::fmt::Display for Reason {\n    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {\n        f.write_str(self.as_str())\n    }\n}\n\n");
    o
}

/// The head of the module: what it is made from, what it calls, the crates it needs, and the lints
/// its names would raise (a name of the book is written as the book writes it).
fn header(book: &Book, origin: &Origin, what: &str, backend: crate::target::Backend) -> String {
    let mut o = head(book, origin, Comment::Slashes);
    o.push_str(&format!("//! The book {} (v{}) {what}\n//!\n//! It needs these crates in the Cargo.toml of the crate it is in (the versions its tests run):\n//!\n//! ```toml\n", doc(&book.name), book.version));
    for c in crates(backend) {
        o.push_str(&format!("//! {}\n", c.line()));
    }
    o.push_str("//! ```\n#![allow(non_snake_case, non_camel_case_types, dead_code)]\n\n");
    o
}

/// A method of a calls type: its doc, signature and body.
struct Method {
    doc: String,
    sig: String,
    body: String,
}

/// The book value, its constructor, and a method per transfer kind and account kind.
struct Shape<'b> {
    book: &'b Book,
    /// the generic parameter of the book value and its bound: `C: TigerBeetleClient`
    param: &'static str,
    bound: &'static str,
    on: &'static str,
}

impl Shape<'_> {
    fn book_type(&self, factory: &str) -> String {
        let (book, p, b) = (self.book, self.param, self.bound);
        let mut o = format!("/// The book {} (v{}), called on {}. Make one with [`{factory}`].\npub struct Book<{p}> {{\n    r: Runtime<{p}>,\n}}\n\n", doc(&book.name), book.version, self.on);
        o.push_str(&format!("impl<{p}: {b}> Book<{p}> {{\n"));
        for ((t, m), base) in book.transfers.iter().zip(members(book)).zip(transfer_types(book)) {
            o.push_str(&format!("    /// {}: {}.\n    pub fn {m}(&self) -> {base}Calls<'_, {p}> {{\n        {base}Calls {{ r: &self.r }}\n    }}\n\n", doc(&t.name), doc(&moves_text(book, t))));
        }
        o.push_str(&format!("    /// The balance of each account kind.\n    pub fn balance(&self) -> Balances<'_, {p}> {{\n        Balances {{ r: &self.r }}\n    }}\n"));
        o
    }

    fn calls_type(&self, t: &TransferKind, base: &str, methods: &[Method]) -> String {
        let (p, b) = (self.param, self.bound);
        let mut o = format!("/// The operations of {}.\npub struct {base}Calls<'b, {p}> {{\n    r: &'b Runtime<{p}>,\n}}\n\nimpl<{p}: {b}> {base}Calls<'_, {p}> {{\n", doc(&t.name));
        for (i, m) in methods.iter().enumerate() {
            if i > 0 {
                o.push('\n');
            }
            o.push_str(&format!("    /// {}\n    pub async fn {} {{\n{}    }}\n", doc(&m.doc), m.sig, m.body));
        }
        o.push_str("}\n\n");
        o
    }

    /// `Balances`, with a method per account kind whose body `body` writes from the account and
    /// the fields of its arguments.
    fn balances(&self, body: impl Fn(usize, &AccountKind, &[String]) -> String) -> String {
        let (book, p, b) = (self.book, self.param, self.bound);
        let mut o = format!("/// The balance of each account kind.\npub struct Balances<'b, {p}> {{\n    r: &'b Runtime<{p}>,\n}}\n\nimpl<{p}: {b}> Balances<'_, {p}> {{\n");
        for (k, ((a, m), base)) in book.accounts.iter().zip(balance_methods(book)).zip(account_types(book)).enumerate() {
            if k > 0 {
                o.push('\n');
            }
            let fs = account_fields(a);
            let sig = if a.params.is_empty() { format!("{m}(&self)") } else { format!("{m}(&self, args: {base}Account<'_>)") };
            o.push_str(&format!("    /// The balance of {}.\n    pub async fn {sig} -> Result<Balance, Error> {{\n        {}\n    }}\n", doc(&a.name), body(k, a, &fs)));
        }
        o.push_str("}\n\n");
        o
    }
}

/// The arguments of `which` as values of the runtime's: `Value::Str(args.order)`, `Value::Amt(args.qty)`.
fn values(book: &Book, t: &TransferKind, which: &[usize], obj: &str, in_pkg: Option<&InPackage>) -> String {
    fields(t, which)
        .iter()
        .zip(which)
        .map(|(f, i)| match t.params[*i].ty {
            Ty::Str => format!("Value::Str({obj}.{f})"),
            ref a => format!("Value::Amt({})", amount_value(book, a, obj, f, in_pkg)),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The key's strings, in the order of the key line (`t.key`), from a struct of the key.
fn key_strs(t: &TransferKind, obj: &str) -> String {
    let declared = postgres::key_params(t);
    let fs = fields(t, &declared);
    t.key.iter().map(|k| format!("{obj}.{}", fs[declared.iter().position(|d| d == k).unwrap_or(0)])).collect::<Vec<_>>().join(", ")
}

/// The amounts of a post, in the order of the amounts, from a struct of them.
fn amount_values(book: &Book, t: &TransferKind, obj: &str, in_pkg: Option<&InPackage>) -> String {
    let ap = t.amount_params();
    fields(t, &ap).iter().zip(&ap).map(|(f, i)| amount_value(book, &t.params[*i].ty, obj, f, in_pkg)).collect::<Vec<_>>().join(", ")
}

/// The operations of `t` on a runtime whose `move_`, `end` and `status` take the transfer's
/// definition: TigerBeetle's and the MySQL family's.
fn chain_methods(book: &Book, t: &TransferKind, k: usize, base: &str, in_pkg: Option<&InPackage>) -> Vec<Method> {
    let all: Vec<usize> = (0..t.params.len()).collect();
    let args = values(book, t, &all, "args", in_pkg);
    let alt = lt_of(t, &all);
    let mut ms = Vec::new();
    if t.is_pending() {
        let key = key_strs(t, "key");
        let klt = lt_of(t, &postgres::key_params(t));
        ms.push(Method {
            doc: format!("Holds {}.", moves_text(book, t)),
            sig: format!("hold(&self, args: {base}Args{alt}) -> Result<Outcome, Error>"),
            body: format!("        self.r.move_(&TRANSFERS[{k}], Op::Hold, &[{args}]).await\n"),
        });
        if t.amount_params().is_empty() {
            ms.push(Method {
                doc: "Posts the hold of key.".into(),
                sig: format!("post(&self, key: {base}Key{klt}) -> Result<Outcome, Error>"),
                body: format!("        self.r.end(&TRANSFERS[{k}], Op::Post, &[{key}], None).await\n"),
            });
        } else {
            ms.push(Method {
                doc: "Posts the hold of key: all of it when amounts is None.".into(),
                sig: format!("post(&self, key: {base}Key{klt}, amounts: Option<{base}Amounts>) -> Result<Outcome, Error>"),
                body: format!("        let amounts = amounts.map(|amounts| [{}]);\n        self.r.end(&TRANSFERS[{k}], Op::Post, &[{key}], amounts.as_ref().map(|a| a.as_slice())).await\n", amount_values(book, t, "amounts", in_pkg)),
            });
        }
        ms.push(Method {
            doc: "Voids the hold of key.".into(),
            sig: format!("void(&self, key: {base}Key{klt}) -> Result<Outcome, Error>"),
            body: format!("        self.r.end(&TRANSFERS[{k}], Op::Void, &[{key}], None).await\n"),
        });
        ms.push(Method {
            doc: "Where the hold of key is; None when there is no such hold.".into(),
            sig: format!("status(&self, key: {base}Key{klt}) -> Result<Option<HoldState>, Error>"),
            body: format!("        self.r.status(&TRANSFERS[{k}], &[{key}]).await\n"),
        });
    } else {
        ms.push(Method {
            doc: format!("Moves {}.", moves_text(book, t)),
            sig: format!("r#do(&self, args: {base}Args{alt}) -> Result<Outcome, Error>"),
            body: format!("        self.r.move_(&TRANSFERS[{k}], Op::Do, &[{args}]).await\n"),
        });
    }
    ms
}

fn ref_literal(r: &Ref) -> String {
    let args: Vec<String> = r
        .args
        .iter()
        .map(|a| match a {
            Arg::Param(i) => format!("ArgDef::Param({i})"),
            Arg::Lit(s) => format!("ArgDef::Lit({})", q(s)),
        })
        .collect();
    format!("RefDef {{ kind: {}, args: &[{}] }}", r.kind, args.join(", "))
}

fn amount_literal(m: &Move) -> String {
    match &m.amount {
        Amount::Param(i) => format!("AmountDef::Param({i})"),
        Amount::Lit(v) => format!("AmountDef::Lit({v})"),
    }
}

fn params_literal(t: &TransferKind) -> String {
    t.params.iter().map(|p| format!("ParamDef {{ name: {}, amount: {} }}", q(&p.name), matches!(p.ty, Ty::Amount(_)))).collect::<Vec<_>>().join(", ")
}

fn usizes(xs: &[usize]) -> String {
    xs.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
}

fn reason_of(book: &Book, r: &str) -> String {
    let v = reason_variants(book).into_iter().find(|(name, _)| name == r).map(|(_, v)| v).unwrap_or_default();
    format!("Reason::{v}")
}

fn tb_literal(book: &Book) -> String {
    let p = plan(book);
    let mut o = format!("const BOOK: &str = {};\n\nconst UNITS: &[UnitDef] = &[\n", q(&book.name));
    for u in &p.units {
        o.push_str(&format!("    UnitDef {{ name: {}, ledger: {}, sink_code: {} }},\n", q(&u.name), u.ledger, u.sink_code));
    }
    o.push_str("];\n\nconst ACCOUNTS: &[AccountDef] = &[\n");
    let bound = |b: &Option<(i128, String)>| match b {
        Some((v, r)) => format!("Some(BoundDef {{ value: {v}, reason: {} }})", reason_of(book, r)),
        None => "None".into(),
    };
    for (a, ap) in book.accounts.iter().zip(&p.accounts) {
        let unit = a.unit;
        o.push_str(&format!(
            "    AccountDef {{ name: {}, unit: {unit}, code: {}, flagged: {}, lower: {}, upper: {} }},\n",
            q(&ap.name),
            ap.code,
            ap.flagged,
            bound(&ap.lower),
            bound(&ap.upper)
        ));
    }
    o.push_str("];\n\nconst TRANSFERS: &[TransferDef] = &[\n");
    for (t, tp) in book.transfers.iter().zip(&p.transfers) {
        let moves: Vec<String> = t
            .moves
            .iter()
            .zip(&tp.moves)
            .map(|(m, mp)| {
                let roles: Vec<String> = mp
                    .roles
                    .iter()
                    .map(|r| {
                        let v = match *r {
                            "main" => "Main",
                            "probe" => "Probe",
                            "probe_void" => "ProbeVoid",
                            "floor_out" => "FloorOut",
                            "room_back" => "RoomBack",
                            "room_in" => "RoomIn",
                            _ => "FloorBack",
                        };
                        format!("Role::{v}")
                    })
                    .collect();
                format!("MoveDef {{ amount: {}, from: {}, to: {}, roles: &[{}] }}", amount_literal(m), ref_literal(&m.from), ref_literal(&m.to), roles.join(", "))
            })
            .collect();
        o.push_str(&format!(
            "    TransferDef {{\n        name: {},\n        params: &[{}],\n        key: &[{}],\n        timeout: {},\n        code: {},\n        definition: {},\n        moves: &[{}],\n    }},\n",
            q(&t.name),
            params_literal(t),
            usizes(&t.key),
            tp.timeout,
            tp.code,
            q(&tp.definition),
            moves.join(", ")
        ));
    }
    o.push_str("];\n\n");
    o
}

pub fn tigerbeetle(book: &Book, stem: &str, origin: &Origin, in_pkg: Option<&InPackage>) -> Vec<(String, String)> {
    let module = module(book, stem);
    let mut o = header(book, origin, &format!("on TigerBeetle, through the tigerbeetle crate {}.", TIGERBEETLE.trim_start_matches('=')), crate::target::Backend::TigerBeetle);
    o.push_str(&reason_type(book));
    o.push_str(&arg_types(book, in_pkg));
    let s = Shape { book, param: "C", bound: "TigerBeetleClient", on: "TigerBeetle" };
    o.push_str(
        "/// The book on TigerBeetle, through `client`: tigerbeetle's `Client`, a reference to one, or an `Arc` of\n/// one. A tenant is a set of balances of its own; \"\" for one.\npub fn tigerbeetle<C: TigerBeetleClient>(client: C, tenant: &str) -> Book<C> {\n    Book { r: Runtime::new(client, tenant) }\n}\n\n",
    );
    o.push_str(&s.book_type("tigerbeetle"));
    o.push_str("}\n\n");
    for (k, (t, base)) in book.transfers.iter().zip(transfer_types(book)).enumerate() {
        o.push_str(&s.calls_type(t, &base, &chain_methods(book, t, k, &base, in_pkg)));
    }
    o.push_str(&s.balances(|k, _, fs| {
        let args: Vec<String> = fs.iter().map(|f| format!("args.{f}")).collect();
        format!("self.r.balance({k}, &[{}]).await", args.join(", "))
    }));
    o.push_str(&tb_literal(book));
    o.push_str(TB_RUNTIME);
    vec![(format!("{module}.rs"), o)]
}

pub fn postgres(book: &Book, stem: &str, origin: &Origin, in_pkg: Option<&InPackage>) -> Vec<(String, String)> {
    let module = module(book, stem);
    let mut o = header(
        book,
        origin,
        &format!("on PostgreSQL, through the SQL functions of `chobo build --target postgres` (schema {}) and sqlx {SQLX}.", postgres::ident(&book.name)),
        crate::target::Backend::Postgres,
    );
    o.push_str(&reason_type(book));
    o.push_str(&arg_types(book, in_pkg));
    let s = Shape { book, param: "D", bound: "Database", on: "PostgreSQL" };
    o.push_str(
        "/// The book on PostgreSQL, through `db`: sqlx's `PgPool`, or a `tokio::sync::Mutex` of a connection of\n/// sqlx's (`&mut PgConnection`, a `Transaction`, a `PoolConnection`), or a reference to, or an `Arc` of,\n/// one of these. A tenant is a set of balances of its own; \"\" for one. Each call is a transaction of\n/// its own, unless the connection is in a transaction of the caller's.\npub fn postgres<D: Database>(db: D, tenant: &str) -> Book<D> {\n    Book { r: Runtime::new(db, tenant) }\n}\n\n",
    );
    o.push_str(&s.book_type("postgres"));
    o.push_str(&format!(
        "\n    /// Gives back what the holds past their expiry hold; call it from a job. Answers how many holds it\n    /// ended.\n    pub async fn expire(&self) -> Result<u64, Error> {{\n        self.r.expire({}).await\n    }}\n}}\n\n",
        raw(&postgres::expire_sql(book))
    ));
    let sql = |t: &TransferKind, op: &str, n: usize| raw(&format!("select * from {}({})", postgres::qualified(book, &postgres::op_function(t, op)), (1..=n).map(|i| format!("${i}")).collect::<Vec<_>>().join(", ")));
    let text = |obj: &str, f: &str| format!("Param::Text({obj}.{f}.to_string())");
    let pvals = |t: &TransferKind, which: &[usize], obj: &str| -> String {
        fields(t, which)
            .iter()
            .zip(which)
            .map(|(f, i)| match t.params[*i].ty {
                Ty::Str => text(obj, f),
                ref a => format!("Param::Int({})", amount_value(book, a, obj, f, in_pkg)),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    for (t, base) in book.transfers.iter().zip(transfer_types(book)) {
        let all: Vec<usize> = (0..t.params.len()).collect();
        let keys = postgres::key_params(t);
        let alt = lt_of(t, &all);
        let klt = lt_of(t, &keys);
        let mut ms = Vec::new();
        if t.is_pending() {
            ms.push(Method {
                doc: format!("Holds {}.", moves_text(book, t)),
                sig: format!("hold(&self, args: {base}Args{alt}) -> Result<Outcome, Error>"),
                body: format!("        self.r.call({}, vec![{}]).await\n", sql(t, "hold", 1 + all.len()), pvals(t, &all, "args")),
            });
            let ap = t.amount_params();
            if ap.is_empty() {
                ms.push(Method {
                    doc: "Posts the hold of key.".into(),
                    sig: format!("post(&self, key: {base}Key{klt}) -> Result<Outcome, Error>"),
                    body: format!("        self.r.call({}, vec![{}]).await\n", sql(t, "post", 1 + keys.len()), pvals(t, &keys, "key")),
                });
            } else {
                let am: Vec<String> = fields(t, &ap).iter().zip(&ap).map(|(f, i)| format!("Param::Int({})", amount_value(book, &t.params[*i].ty, "a", f, in_pkg))).collect();
                let nulls = vec!["Param::Null"; ap.len()].join(", ");
                ms.push(Method {
                    doc: "Posts the hold of key: all of it when amounts is None.".into(),
                    sig: format!("post(&self, key: {base}Key{klt}, amounts: Option<{base}Amounts>) -> Result<Outcome, Error>"),
                    body: format!(
                        "        let mut params = vec![{}];\n        match amounts {{\n            Some(a) => params.extend([{}]),\n            None => params.extend([{nulls}]),\n        }}\n        self.r.call({}, params).await\n",
                        pvals(t, &keys, "key"),
                        am.join(", "),
                        sql(t, "post", 1 + keys.len() + ap.len())
                    ),
                });
            }
            ms.push(Method {
                doc: "Voids the hold of key.".into(),
                sig: format!("void(&self, key: {base}Key{klt}) -> Result<Outcome, Error>"),
                body: format!("        self.r.call({}, vec![{}]).await\n", sql(t, "void", 1 + keys.len()), pvals(t, &keys, "key")),
            });
            ms.push(Method {
                doc: "Where the hold of key is; None when there is no such hold.".into(),
                sig: format!("status(&self, key: {base}Key{klt}) -> Result<Option<HoldState>, Error>"),
                body: format!("        self.r.status({}, vec![{}]).await\n", raw(&postgres::status_sql(book, t)), pvals(t, &keys, "key")),
            });
        } else {
            ms.push(Method {
                doc: format!("Moves {}.", moves_text(book, t)),
                sig: format!("r#do(&self, args: {base}Args{alt}) -> Result<Outcome, Error>"),
                body: format!("        self.r.call({}, vec![{}]).await\n", sql(t, "do", 1 + all.len()), pvals(t, &all, "args")),
            });
        }
        o.push_str(&s.calls_type(t, &base, &ms));
    }
    o.push_str(&s.balances(|_, a, fs| {
        let args: Vec<String> = fs.iter().map(|f| text("args", f)).collect();
        format!("self.r.balance({}, vec![{}]).await", raw(&postgres::balance_sql(book, a)), args.join(", "))
    }));
    o.push_str(&format!("const BOOK: &str = {};\n\n", q(&book.name)));
    o.push_str(PG_RUNTIME);
    vec![(format!("{module}.rs"), o)]
}

fn my_literal(book: &Book) -> String {
    let st = crate::mysql::statements(book);
    let mut o = format!("const BOOK: &str = {};\n\nconst ACCOUNTS: &[AccountDef] = &[\n", q(&book.name));
    let bound = |b: &Option<Bound>| b.as_ref().map(|b| format!("Some({})", b.value)).unwrap_or_else(|| "None".into());
    for (a, row) in book.accounts.iter().zip(&st.account_rows) {
        o.push_str(&format!("    AccountDef {{ name: {}, bounded: {}, lower: {}, upper: {}, row: {} }},\n", q(&a.name), crate::mysql::bounded(a), bound(&a.lower), bound(&a.upper), raw(row)));
    }
    o.push_str("];\n\nconst TRANSFERS: &[TransferDef] = &[\n");
    let opt = |s: &Option<String>| s.as_ref().map(|x| format!("Some({})", raw(x))).unwrap_or_else(|| "None".into());
    for (t, ts) in book.transfers.iter().zip(&st.transfers) {
        let moves: Vec<String> = t.moves.iter().map(|m| format!("MoveDef {{ amount: {}, from: {}, to: {} }}", amount_literal(m), ref_literal(&m.from), ref_literal(&m.to))).collect();
        let pends = |v: &[String]| if t.is_pending() { format!("&[{}]", v.iter().map(|s| raw(s)).collect::<Vec<_>>().join(", ")) } else { "&[]".into() };
        let pend = |s: &str| if t.is_pending() { raw(s) } else { q("") };
        let checks: Vec<String> = ts.checks.iter().map(|(n, r, sql)| format!("({n}, {}, {})", reason_of(book, r), raw(sql))).collect();
        o.push_str(&format!(
            "    TransferDef {{\n        name: {},\n        params: &[{}],\n        key: &[{}],\n        definition: {},\n        moves: &[{}],\n        sql: TransferSql {{\n            key: {},\n            lock: {},\n            checks: &[{}],\n            refuse: {},\n            entries: {},\n            apply: {},\n            post_apply: {},\n            post_entries: {},\n            void_apply: {},\n            void_entries: {},\n            expire_apply: {},\n            expire_entries: {},\n        }},\n    }},\n",
            q(&t.name),
            params_literal(t),
            usizes(&t.key),
            q(&ids::definition(book, t)),
            moves.join(", "),
            raw(&ts.key_insert),
            opt(&ts.lock),
            checks.join(", "),
            raw(&ts.refuse),
            raw(&ts.entries),
            opt(&ts.apply),
            pends(&ts.post_apply),
            pend(&ts.post_entries),
            pends(&ts.void_apply),
            pend(&ts.void_entries),
            pends(&ts.expire_apply),
            pend(&ts.expire_entries),
        ));
    }
    o.push_str("];\n\nconst SQL: BookSql = BookSql {\n");
    for (name, sql) in [
        ("begin", &st.begin),
        ("read_accounts", &st.read_accounts),
        ("make_accounts", &st.make_accounts),
        ("read_call", &st.read_call),
        ("hold_lock", &st.hold_lock),
        ("status", &st.status),
        ("balance", &st.balance),
        ("due", &st.due),
        ("fold_list", &st.fold_list),
        ("fold_lock", &st.fold_lock),
        ("fold_read", &st.fold_read),
        ("fold_apply", &st.fold_apply),
        ("fold_mark", &st.fold_mark),
    ] {
        o.push_str(&format!("    {name}: {},\n", raw(sql)));
    }
    o.push_str("};\n\n");
    o
}

/// The client of the MySQL family (DESIGN 4.5): sqlx's.
pub fn mysql(book: &Book, stem: &str, origin: &Origin, in_pkg: Option<&InPackage>) -> Vec<(String, String)> {
    let module = module(book, stem);
    let tables: Vec<String> = crate::mysql::TABLES.iter().map(|w| format!("{}_{w}", book.name)).collect();
    let mut o = header(
        book,
        origin,
        &format!("on MySQL, TiDB or Vitess, through the tables of `chobo build --target mysql` ({})\n//! and sqlx {SQLX}.", tables.join(", ")),
        crate::target::Backend::MySql,
    );
    o.push_str(&reason_type(book));
    o.push_str(&arg_types(book, in_pkg));
    let s = Shape { book, param: "D", bound: "Database", on: "MySQL, TiDB or Vitess" };
    o.push_str(
        "/// The book on MySQL, TiDB or Vitess, through `db`: sqlx's `MySqlPool`, or a `tokio::sync::Mutex` of a\n/// connection of sqlx's (`&mut MySqlConnection`, a `Transaction`, a `PoolConnection`), or a reference to,\n/// or an `Arc` of, one of these. A tenant is a set of balances of its own; \"\" for one. A call on a\n/// connection that is in a transaction of sqlx's (a `Transaction`) is part of it, and the caller commits\n/// it; else the call is a transaction of its own, made again when the database gives up on it. With\n/// `batch`, the statements a call sends in a row go in one request.\npub fn mysql<D: Database>(db: D, tenant: &str, batch: bool) -> Book<D> {\n    Book { r: Runtime::new(db, tenant, batch) }\n}\n\n",
    );
    o.push_str(&s.book_type("mysql"));
    o.push_str("\n    /// Gives back what the holds past their expiry hold, and adds up what the accounts without bounds\n    /// were given; call it from a job. Answers how many holds it ended.\n    pub async fn expire(&self) -> Result<u64, Error> {\n        self.r.expire().await\n    }\n}\n\n");
    for (k, (t, base)) in book.transfers.iter().zip(transfer_types(book)).enumerate() {
        o.push_str(&s.calls_type(t, &base, &chain_methods(book, t, k, &base, in_pkg)));
    }
    o.push_str(&s.balances(|k, _, fs| {
        let args: Vec<String> = fs.iter().map(|f| format!("args.{f}")).collect();
        format!("self.r.balance({k}, &[{}]).await", args.join(", "))
    }));
    o.push_str(&my_literal(book));
    o.push_str(MY_RUNTIME);
    vec![(format!("{module}.rs"), o)]
}
