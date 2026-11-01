//! The names of a book that a client spells as one identifier, or cannot take as they are (W108
//! when the book is checked, E064 when a target is built; DESIGN 16).
//!
//! Each client writes the book's names as identifiers in its own spelling. Go writes a name as
//! an exported one (`sale` is `Sale`, `引当` is `X引当`); Rust writes a type and a variant in
//! PascalCase (`take_back` is `TakeBack`), and a field or a method with `_` after a keyword a raw
//! identifier cannot be and after `_` (`self_`); Python writes `_` after a keyword and after the
//! names its methods use (`class_`); PostgreSQL names a function `<transfer>_<operation>` and
//! `balance_<account>`. So two names can be one identifier in one place of the code, a name can
//! be one the client uses there itself (`balance`, `expire`), and Python changes a name that
//! begins with `__` inside a class, and the class of a transfer whose name begins with `_`.
//!
//! None of it is renamed (rulec's DESIGN 15.263): a call is written with the book's names in
//! every language. Each form breaks some of the targets (the clients of a language, the clients
//! that have the name themselves, or PostgreSQL's), and never every target that writes the names,
//! so `check` warns (W108), and the build of a target it breaks stops (E064), as with the other
//! limits of a target (E060–E063, DESIGN 3.1).

use crate::client::{go, python, rust};
use crate::diag::{self, Diag, DiagExt};
use crate::model::{Book, REASONS, Ty};
use crate::postgres;
use crate::target::{Backend, Lang, Target};
use ritsu_base::text::Text;

/// A name the book writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Name {
    Transfer(usize),
    Account(usize),
    /// a parameter of a transfer
    Param(usize, usize),
    /// a parameter of an account
    AccountParam(usize, usize),
    /// a bound's reason, by the account and the bound (0 the lower, 1 the upper) that first gives it
    Reason(usize, usize),
}

impl Name {
    pub fn text<'b>(&self, b: &'b Book) -> &'b str {
        match *self {
            Name::Transfer(k) => &b.transfers[k].name,
            Name::Account(k) => &b.accounts[k].name,
            Name::Param(k, i) => &b.transfers[k].params[i].name,
            Name::AccountParam(k, i) => &b.accounts[k].params[i],
            Name::Reason(k, w) => &bound(b, k, w).refusal,
        }
    }

    /// Where the book writes it: the line and the column of the name (of the account, for the
    /// parameter of an account, which the model keeps no place of).
    fn place(&self, b: &Book) -> (usize, usize) {
        match *self {
            Name::Transfer(k) => (b.transfers[k].line, b.transfers[k].col),
            Name::Account(k) | Name::AccountParam(k, _) => (b.accounts[k].line, b.accounts[k].col),
            Name::Param(k, i) => (b.transfers[k].params[i].line, b.transfers[k].params[i].col),
            Name::Reason(k, w) => (bound(b, k, w).line, bound(b, k, w).col),
        }
    }

    /// `振替 \`sale\``, `the transfer \`sale\``.
    fn what(&self, b: &Book) -> Text {
        let n = self.text(b);
        match *self {
            Name::Transfer(_) => tr!("振替 `{n}`", "the transfer `{n}`"),
            Name::Account(_) => tr!("勘定 `{n}`", "the account `{n}`"),
            Name::Param(k, _) => tr!("振替 `{}` の引数 `{n}`", "the parameter `{n}` of the transfer `{}`", b.transfers[k].name),
            Name::AccountParam(k, _) => tr!("勘定 `{}` の引数 `{n}`", "the parameter `{n}` of the account `{}`", b.accounts[k].name),
            Name::Reason(..) => tr!("拒否の理由 `{n}`", "the reason `{n}`"),
        }
    }
}

fn bound(b: &Book, k: usize, w: usize) -> &crate::model::Bound {
    let a = &b.accounts[k];
    if w == 0 { a.lower.as_ref() } else { a.upper.as_ref() }.expect("a reason is a bound's")
}

/// Two names of the same kind and owner, `振替 \`sale\` と \`Sale\``; else each with its kind.
fn pair(b: &Book, x: Name, y: Name) -> Text {
    let (m, n) = (x.text(b), y.text(b));
    match (x, y) {
        (Name::Transfer(_), Name::Transfer(_)) => tr!("振替 `{m}` と `{n}`", "the transfers `{m}` and `{n}`"),
        (Name::Account(_), Name::Account(_)) => tr!("勘定 `{m}` と `{n}`", "the accounts `{m}` and `{n}`"),
        (Name::Param(k, _), Name::Param(j, _)) if k == j => tr!("振替 `{}` の引数 `{m}` と `{n}`", "the parameters `{m}` and `{n}` of the transfer `{}`", b.transfers[k].name),
        (Name::AccountParam(k, _), Name::AccountParam(j, _)) if k == j => {
            tr!("勘定 `{}` の引数 `{m}` と `{n}`", "the parameters `{m}` and `{n}` of the account `{}`", b.accounts[k].name)
        }
        (Name::Reason(..), Name::Reason(..)) => tr!("拒否の理由 `{m}` と `{n}`", "the reasons `{m}` and `{n}`"),
        _ => {
            let (p, q) = (x.what(b), y.what(b));
            Text::new(format!("{} と{}", p.ja, q.ja), format!("{} and {}", p.en, q.en))
        }
    }
}

/// What a finding is of: two names that are one identifier, a name that is one the client uses
/// itself, or a name Python changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Subject {
    Two(Name, Name),
    Own(Name),
    Mangled(Name),
}

impl Subject {
    /// The name the diagnostic is at: the later of two.
    fn at(&self) -> Name {
        match *self {
            Subject::Two(_, y) => y,
            Subject::Own(n) | Subject::Mangled(n) => n,
        }
    }
}

/// What one language says of a finding: the targets it breaks, and the note.
#[derive(Clone, Debug)]
pub struct Said {
    pub lang: Lang,
    pub backends: Vec<Backend>,
    pub note: Text,
}

impl Said {
    /// The targets it breaks: the language's, on its databases. What PostgreSQL's SQL says breaks
    /// the `postgres` target and every client that calls that SQL.
    pub fn targets(&self) -> Vec<Target> {
        Target::ALL.into_iter().filter(|t| (self.lang == Lang::Sql || t.lang() == self.lang) && self.backends.contains(&t.backend())).collect()
    }
}

/// One finding: what it is of, and what each language it breaks in says.
#[derive(Clone, Debug)]
pub struct Found {
    pub subject: Subject,
    pub said: Vec<Said>,
}

impl Found {
    /// The targets it breaks, in their order.
    pub fn targets(&self) -> Vec<Target> {
        Target::ALL.into_iter().filter(|t| self.said.iter().any(|s| s.targets().contains(t))).collect()
    }
}

const ALL: &[Backend] = &[Backend::Postgres, Backend::TigerBeetle, Backend::MySql];
/// The databases whose clients have `expire`, and keep what they call through in `Book` (TigerBeetle
/// gives back the holds past their expiry itself).
const EXPIRE: &[Backend] = &[Backend::Postgres, Backend::MySql];

/// How a client spells a name of the book in one place of its code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    /// Go: an exported name (`go::go_name`)
    GoExported,
    /// Rust: a type's name or a variant's, in PascalCase (`rust::type_base`)
    RustType,
    /// Rust: a field, a method (`rust::ident`)
    RustIdent,
    /// Python: `_` after a keyword and the methods' own names (`python::name`)
    PyAside,
    /// TypeScript: as it is
    TsAsIs,
}

impl Rule {
    fn lang(self) -> Lang {
        match self {
            Rule::GoExported => Lang::Go,
            Rule::RustType | Rule::RustIdent => Lang::Rust,
            Rule::PyAside => Lang::Python,
            Rule::TsAsIs => Lang::TypeScript,
        }
    }

    /// How the client spells two names alike, so that both are `id`.
    fn why(self, id: &str) -> Text {
        match self {
            Rule::GoExported => tr!(
                "クライアントは、名前の頭の一字を大文字にして書く（大文字の無い字で始まる名前には、頭に `X` を付ける）ので、どちらも `{id}` です",
                "the client writes a name with its first letter in upper case (with an `X` before one whose first letter has none), so both are `{id}`"
            ),
            Rule::RustType => tr!(
                "クライアントは、型の名前と列挙の値を PascalCase で書く（`_` で区切った語の頭を大文字にして、`_` を除く）ので、どちらも `{id}` です",
                "the client writes the names of types and variants in PascalCase (each word between `_` begins in upper case, and the `_` go), so both are `{id}`"
            ),
            Rule::RustIdent => tr!(
                "クライアントは、生の識別子にできないキーワード（`self`、`Self`、`super`、`crate`）と `_` の後ろに `_` を付けて書くので、どちらも `{id}` です",
                "the client writes `_` after a keyword a raw identifier cannot be (`self`, `Self`, `super`, `crate`) and after `_`, so both are `{id}`"
            ),
            Rule::PyAside => tr!(
                "クライアントは、キーワードと、メソッドが自分で使う名前（`self`、`_str`、`_amt`、`_amounts`）の後ろに `_` を付けて書くので、どちらも `{id}` です",
                "the client writes `_` after a keyword, and after a name its methods use themselves (`self`, `_str`, `_amt`, `_amounts`), so both are `{id}`"
            ),
            Rule::TsAsIs => tr!("クライアントは名前をそのまま書くので、どちらも `{id}` です", "the client writes a name as it is, so both are `{id}`"),
        }
    }
}

/// Where in the code an identifier is.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Role {
    /// a type of the package or the module
    Type,
    /// a field of a struct (Go, Rust)
    Field(String),
    /// a method of a type
    Method(String),
    /// a variant of an enum (Rust's `Reason`)
    Variant(String),
    /// an attribute of an object (Python)
    Attr(String),
    /// a parameter of the methods of a class (Python): the class of a transfer, or the method of
    /// `_Balances` of an account
    Kwarg(String),
    /// a member of an interface (TypeScript's `Book`)
    Member(String),
}

/// A name the client gives something itself, where a name of the book can be the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Own {
    /// what reads the balances: Go's field `Balance`, Rust's method and TypeScript's member and
    /// Python's attribute `balance`
    Balance,
    /// the method that gives back the holds past their expiry
    Expire,
    /// Python's attribute `_r` of `_Balances`, what its methods call through
    Conn,
    /// Python's `__init__` of `_Balances`
    Init,
    /// a variant of Rust's `Reason` for a reason chobo gives itself
    Reason(&'static str),
}

impl Own {
    /// `\`Book\` には …があります` / `\`Book\` has …`.
    fn has(self, id: &str, owner: &str) -> Text {
        match self {
            Own::Balance => tr!("`{owner}` には、残高を読むための `{id}` があります", "`{owner}` has `{id}`, which reads the balances"),
            Own::Expire => tr!(
                "`{owner}` には、期限の切れた仮押さえを戻すメソッド `{id}` があります",
                "`{owner}` has the method `{id}`, which gives back the holds past their expiry"
            ),
            Own::Conn => tr!("`{owner}` には、メソッドが呼び出しに使うものを持つ属性 `{id}` があります", "`{owner}` has the attribute `{id}`, which holds what its methods call through"),
            Own::Init => tr!("`{owner}` には、それを作るメソッド `{id}` があります", "`{owner}` has the method `{id}`, which makes it"),
            Own::Reason(r) => tr!(
                "`{owner}` には、chobo が自分で返す理由 `{r}` の値 `{id}` があります",
                "`{owner}` has the variant `{id}`, of `{r}`, a reason chobo gives itself"
            ),
        }
    }
}

#[derive(Clone, Debug)]
enum From {
    Name(Name),
    Own(Own),
}

/// One identifier a client declares: what it is spelled from, how, where, and in the clients of
/// which databases.
#[derive(Clone, Debug)]
struct Decl {
    id: String,
    from: From,
    rule: Rule,
    /// the place two identifiers have to be apart in: a type's members, a package's types, …
    scope: String,
    role: Role,
    on: &'static [Backend],
}

fn decl(id: String, name: Name, rule: Rule, scope: impl Into<String>, role: Role) -> Decl {
    Decl { id, from: From::Name(name), rule, scope: scope.into(), role, on: ALL }
}

fn own(id: &str, own: Own, rule: Rule, scope: &str, role: Role, on: &'static [Backend]) -> Decl {
    Decl { id: id.to_string(), from: From::Own(own), rule, scope: scope.to_string(), role, on }
}

/// Go: the types of the package (`<Transfer>Args`, `…Key`, `…Amounts`, `…Calls`, `<Account>Account`),
/// the fields of `Book`, the methods of `Balances`, and the fields of each struct of arguments.
fn go_decls(b: &Book) -> Vec<Decl> {
    let r = Rule::GoExported;
    let mut d = Vec::new();
    for (k, t) in b.transfers.iter().enumerate() {
        let g = go::go_name(&t.name);
        let n = Name::Transfer(k);
        let mut types = vec!["Args"];
        if t.is_pending() {
            types.push("Key");
            if !t.amount_params().is_empty() {
                types.push("Amounts");
            }
        }
        types.push("Calls");
        for s in types {
            d.push(decl(format!("{g}{s}"), n, r, "package", Role::Type));
        }
        d.push(decl(g.clone(), n, r, "Book", Role::Field("Book".into())));
        for (i, p) in t.params.iter().enumerate() {
            d.push(decl(go::go_name(&p.name), Name::Param(k, i), r, format!("args {k}"), Role::Field(format!("{g}Args"))));
        }
    }
    for (k, a) in b.accounts.iter().enumerate() {
        let g = go::go_name(&a.name);
        if !a.params.is_empty() {
            d.push(decl(format!("{g}Account"), Name::Account(k), r, "package", Role::Type));
        }
        d.push(decl(g.clone(), Name::Account(k), r, "Balances", Role::Method("Balances".into())));
        for (i, p) in a.params.iter().enumerate() {
            d.push(decl(go::go_name(p), Name::AccountParam(k, i), r, format!("account {k}"), Role::Field(format!("{g}Account"))));
        }
    }
    d.push(own("Balance", Own::Balance, r, "Book", Role::Field("Book".into()), ALL));
    d.push(own("Expire", Own::Expire, r, "Book", Role::Method("Book".into()), EXPIRE));
    d
}

/// Rust: the types of the module, the methods of `Book` and `Balances`, the fields of each struct
/// of arguments, and the variants of `Reason`.
fn rust_decls(b: &Book) -> Vec<Decl> {
    let (ty, id) = (Rule::RustType, Rule::RustIdent);
    let mut d = Vec::new();
    for (k, t) in b.transfers.iter().enumerate() {
        let base = rust::type_base(&t.name);
        let n = Name::Transfer(k);
        let mut types = vec!["Args"];
        if t.is_pending() {
            types.push("Key");
            if !t.amount_params().is_empty() {
                types.push("Amounts");
            }
        }
        types.push("Calls");
        for s in types {
            d.push(decl(format!("{base}{s}"), n, ty, "module", Role::Type));
        }
        d.push(decl(rust::ident(&t.name), n, id, "Book", Role::Method("Book".into())));
        for (i, p) in t.params.iter().enumerate() {
            d.push(decl(rust::ident(&p.name), Name::Param(k, i), id, format!("args {k}"), Role::Field(format!("{base}Args"))));
        }
    }
    for (k, a) in b.accounts.iter().enumerate() {
        let base = rust::type_base(&a.name);
        if !a.params.is_empty() {
            d.push(decl(format!("{base}Account"), Name::Account(k), ty, "module", Role::Type));
        }
        d.push(decl(rust::ident(&a.name), Name::Account(k), id, "Balances", Role::Method("Balances".into())));
        for (i, p) in a.params.iter().enumerate() {
            d.push(decl(rust::ident(p), Name::AccountParam(k, i), id, format!("account {k}"), Role::Field(format!("{base}Account"))));
        }
    }
    for r in reasons(b) {
        d.push(decl(rust::type_base(r.text(b)), r, ty, "Reason", Role::Variant("Reason".into())));
    }
    for r in REASONS {
        d.push(own(&rust::type_base(r), Own::Reason(r), ty, "Reason", Role::Variant("Reason".into()), ALL));
    }
    d.push(own("balance", Own::Balance, id, "Book", Role::Method("Book".into()), ALL));
    d.push(own("expire", Own::Expire, id, "Book", Role::Method("Book".into()), EXPIRE));
    d
}

/// Python: the attributes of `Book`, the methods of `_Balances`, and the parameters of the methods
/// of each transfer's class and of each account's balance.
fn python_decls(b: &Book) -> Vec<Decl> {
    let r = Rule::PyAside;
    let mut d = Vec::new();
    for (k, t) in b.transfers.iter().enumerate() {
        d.push(decl(python::name(&t.name), Name::Transfer(k), r, "Book", Role::Attr("Book".into())));
        for (i, p) in t.params.iter().enumerate() {
            d.push(decl(python::name(&p.name), Name::Param(k, i), r, format!("args {k}"), Role::Kwarg(format!("_{}Transfer", t.name))));
        }
    }
    for (k, a) in b.accounts.iter().enumerate() {
        let m = python::name(&a.name);
        d.push(decl(m.clone(), Name::Account(k), r, "_Balances", Role::Method("_Balances".into())));
        for (i, p) in a.params.iter().enumerate() {
            d.push(decl(python::name(p), Name::AccountParam(k, i), r, format!("account {k}"), Role::Kwarg(m.clone())));
        }
    }
    d.push(own("balance", Own::Balance, r, "Book", Role::Attr("Book".into()), ALL));
    d.push(own("expire", Own::Expire, r, "Book", Role::Method("Book".into()), EXPIRE));
    d.push(own("_r", Own::Conn, r, "_Balances", Role::Attr("_Balances".into()), ALL));
    d.push(own("__init__", Own::Init, r, "_Balances", Role::Method("_Balances".into()), ALL));
    d
}

/// TypeScript writes every name as it is, so only the members of `Book` the client names itself
/// can meet one.
fn typescript_decls(b: &Book) -> Vec<Decl> {
    let r = Rule::TsAsIs;
    let mut d: Vec<Decl> = b.transfers.iter().enumerate().map(|(k, t)| decl(t.name.clone(), Name::Transfer(k), r, "Book", Role::Member("Book".into()))).collect();
    d.push(own("balance", Own::Balance, r, "Book", Role::Member("Book".into()), ALL));
    d.push(own("expire", Own::Expire, r, "Book", Role::Method("Book".into()), EXPIRE));
    d
}

/// Each reason the bounds give, where it is first given.
fn reasons(b: &Book) -> Vec<Name> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for (k, a) in b.accounts.iter().enumerate() {
        for (w, x) in [&a.lower, &a.upper].into_iter().enumerate() {
            if let Some(x) = x
                && !seen.contains(&x.refusal.as_str())
            {
                seen.push(&x.refusal);
                out.push(Name::Reason(k, w));
            }
        }
    }
    out
}

/// The names that are one identifier in one language, by how the client spells them: the
/// identifier they are spelled as (`Sale`), each place it is declared twice (`SaleArgs` among the
/// types, `Sale` among the fields of `Book`), and the client's own name it meets, if one.
struct Hit {
    subject: Subject,
    rule: Rule,
    base: String,
    roles: Vec<(Role, String)>,
    own: Option<(Own, Role)>,
    on: Vec<Backend>,
}

/// Two names, the one the book writes first first.
fn ordered(b: &Book, m: Name, n: Name) -> (Name, Name) {
    if (m.place(b), m) <= (n.place(b), n) { (m, n) } else { (n, m) }
}

/// A name as `rule` spells it.
fn spelled(b: &Book, rule: Rule, n: Name) -> String {
    let s = n.text(b);
    match rule {
        Rule::GoExported => go::go_name(s),
        Rule::RustType => rust::type_base(s),
        Rule::RustIdent => rust::ident(s),
        Rule::PyAside => python::name(s),
        Rule::TsAsIs => s.to_string(),
    }
}

/// Every two declarations of one scope with one identifier, by what they are of.
fn hits(b: &Book, decls: &[Decl]) -> Vec<Hit> {
    let mut out: Vec<Hit> = Vec::new();
    for (i, x) in decls.iter().enumerate() {
        for y in &decls[i + 1..] {
            if x.scope != y.scope || x.id != y.id {
                continue;
            }
            let on: Vec<Backend> = x.on.iter().copied().filter(|b| y.on.contains(b)).collect();
            let (subject, own, role) = match (&x.from, &y.from) {
                (From::Name(m), From::Name(n)) => {
                    let (m, n) = ordered(b, *m, *n);
                    (Subject::Two(m, n), None, x.role.clone())
                }
                (From::Name(n), From::Own(o)) => (Subject::Own(*n), Some((*o, y.role.clone())), x.role.clone()),
                (From::Own(o), From::Name(n)) => (Subject::Own(*n), Some((*o, x.role.clone())), y.role.clone()),
                (From::Own(_), From::Own(_)) => continue,
            };
            let at = (role, x.id.clone());
            match out.iter_mut().find(|h| h.subject == subject && h.rule == x.rule) {
                Some(h) => {
                    if !h.roles.contains(&at) {
                        h.roles.push(at);
                    }
                }
                None => {
                    let n = match subject {
                        Subject::Two(n, _) | Subject::Own(n) | Subject::Mangled(n) => n,
                    };
                    out.push(Hit { subject, rule: x.rule, base: spelled(b, x.rule, n), roles: vec![at], own, on });
                }
            }
        }
    }
    out
}

/// The places as a sentence names them: `型 \`SaleArgs\`・\`SaleCalls\` と、\`Book\` のフィールド \`Sale\``.
fn roles_text(roles: &[(Role, String)]) -> Text {
    let mut parts: Vec<Text> = Vec::new();
    let types: Vec<String> = roles.iter().filter(|(r, _)| *r == Role::Type).map(|(_, id)| format!("`{id}`")).collect();
    if !types.is_empty() {
        let en = if types.len() == 1 { format!("the type {}", types[0]) } else { format!("the types {}", and(&types)) };
        parts.push(Text::new(format!("型 {}", types.join("・")), en));
    }
    for (r, id) in roles {
        parts.push(match r {
            Role::Type => continue,
            Role::Field(o) => tr!("`{o}` のフィールド `{id}`", "the field `{id}` of `{o}`"),
            Role::Method(o) => tr!("`{o}` のメソッド `{id}`", "the method `{id}` of `{o}`"),
            Role::Variant(o) => tr!("列挙 `{o}` の値 `{id}`", "the variant `{id}` of `{o}`"),
            Role::Attr(o) => tr!("`{o}` の属性 `{id}`", "the attribute `{id}` of `{o}`"),
            Role::Kwarg(o) => tr!("`{o}` のメソッドの引数 `{id}`", "the parameter `{id}` of the methods of `{o}`"),
            Role::Member(o) => tr!("`{o}` のメンバ `{id}`", "the member `{id}` of `{o}`"),
        });
    }
    let ja: Vec<&str> = parts.iter().map(|p| p.ja.as_str()).collect();
    let en: Vec<String> = parts.iter().map(|p| p.en.clone()).collect();
    // a list of places that has a comma in it has one after it too
    let en = if en.len() > 1 { format!("{},", and_list(&en)) } else { and_list(&en) };
    Text::new(ja.join(" と、"), en)
}

/// `a`, `a and b`, `a, b and c`.
fn and(xs: &[String]) -> String {
    match xs {
        [] => String::new(),
        [x] => x.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// Phrases with commas around them: `a, and b`.
fn and_list(xs: &[String]) -> String {
    match xs {
        [] => String::new(),
        [x] => x.clone(),
        [init @ .., last] => format!("{}, and {last}", init.join(", ")),
    }
}

fn lang_name(l: Lang) -> &'static str {
    match l {
        Lang::TypeScript => "TypeScript",
        Lang::Python => "Python",
        Lang::Go => "Go",
        Lang::Rust => "Rust",
        Lang::Sql => "PostgreSQL",
    }
}

/// `Go（\`postgres-go\`、\`tigerbeetle-go\`、\`mysql-go\`）: ` / `Go (\`postgres-go\`, …): `.
fn prefix(lang: Lang, targets: &[Target]) -> Text {
    let ts: Vec<String> = targets.iter().map(|t| format!("`{}`", t.name())).collect();
    let l = lang_name(lang);
    Text::new(format!("{l}（{}）: ", ts.join("、")), format!("{l} ({}): ", ts.join(", ")))
}

/// What one language says of a hit: how the client spells it, where it is, and how it breaks.
fn note_of(b: &Book, h: &Hit) -> Text {
    let lang = h.rule.lang();
    let module = match lang {
        Lang::Go => tr!("コンパイルできません", "the package does not compile"),
        Lang::Rust => tr!("コンパイルできません", "the module does not compile"),
        _ => tr!("型の検査を通りません", "the module does not type-check"),
    };
    match (&h.subject, &h.own) {
        (Subject::Two(x, y), None) => {
            let why = h.rule.why(&h.base);
            let breaks = match lang {
                Lang::Python => py_two(b, *x, *y, &h.roles[0]),
                _ => {
                    let roles = roles_text(&h.roles);
                    tr!("{} を二度宣言することになり、{}", "{} would be declared twice, and {}", roles.ja, module.ja; roles.en, module.en)
                }
            };
            tr!("{}。{}。", "{}: {}.", why.ja, breaks.ja; why.en, breaks.en)
        }
        (Subject::Own(n), Some((o, own_role))) => {
            let (role, id) = &h.roles[0];
            let owner = owner_of(own_role);
            let written = roles_text(&h.roles[..1]);
            let has = o.has(id, &owner);
            let what = n.what(b);
            let breaks = match (lang, o, role) {
                (Lang::Go, Own::Expire, _) => tr!(
                    "Go は一つの型に同じ名前のフィールドとメソッドを持てないので、コンパイルできません",
                    "Go takes no field and method of one name in one type, so the package does not compile"
                ),
                (Lang::Python, Own::Balance, _) => tr!(
                    "振替の属性を先に置き、そのあとで残高を読むためのものに置き換えるので、振替を呼べません",
                    "the attribute is set to the transfer first and then to what reads the balances, so no call reaches the transfer"
                ),
                (Lang::Python, Own::Expire, _) => tr!(
                    "振替の属性がメソッドを隠すので、`expire()` を呼ぶと止まります（`TypeError`）",
                    "the transfer's attribute hides the method, and a call of `expire()` stops (`TypeError`)"
                ),
                (Lang::Python, Own::Conn, _) => tr!(
                    "その属性が勘定のメソッドを隠すので、この勘定の残高を読むと止まります（`TypeError`）",
                    "the attribute hides the account's method, and reading this account's balance stops (`TypeError`)"
                ),
                (Lang::Python, Own::Init, _) => tr!(
                    "勘定のメソッドがそれを置き換えるので、`Book` を作れず、どの呼び出しも止まります（`TypeError`）",
                    "the account's method takes its place, so `Book` cannot be made, and no call goes through (`TypeError`)"
                ),
                (Lang::TypeScript, ..) => tr!(
                    "`{owner}` に同じ名前のメンバが二つでき、{}",
                    "`{owner}` would have two members of that name, and {}",
                    module.ja; module.en
                ),
                _ => tr!("二度宣言することになり、{}", "it would be declared twice, and {}", module.ja; module.en),
            };
            tr!(
                "クライアントは、{} を {} にしますが、{}。{}。",
                "the client writes {} as {}, and {}: {}.",
                what.ja, written.ja, has.ja, breaks.ja;
                what.en, written.en, has.en, breaks.en
            )
        }
        _ => unreachable!("a hit of two names has no own name, and one of a name has one"),
    }
}

/// How two names that are one identifier in Python break it: an attribute or a method that the
/// later replaces, or a method with two parameters of one name.
fn py_two(b: &Book, x: Name, y: Name, at: &(Role, String)) -> Text {
    let (role, id) = at;
    let (p, q) = (x.what(b), y.what(b));
    match (x, role) {
        (Name::Transfer(_), _) => tr!(
            "`Book` の属性 `{id}` は、後の{}が前の{}を置き換えるので、{}を呼ぶと{}が動きます",
            "in the attribute `{id}` of `Book`, the later {} takes the place of the earlier {}, so a call of {} moves {}",
            q.ja, p.ja, p.ja, q.ja;
            q.en, p.en, p.en, q.en
        ),
        (Name::Account(_), _) => tr!(
            "`_Balances` のメソッド `{id}` は、後の{}のものが前の{}のものを置き換えるので、{}の残高を読むと{}の残高が返ります",
            "in the method `{id}` of `_Balances`, the later {}'s takes the place of the earlier {}'s, so reading the balance of {} gives the one of {}",
            q.ja, p.ja, p.ja, q.ja;
            q.en, p.en, p.en, q.en
        ),
        (Name::AccountParam(..), Role::Kwarg(m)) => tr!(
            "`_Balances` のメソッド `{m}` に同じ名前の引数 `{id}` が二つでき、モジュールを読み込めません（`SyntaxError`）",
            "the method `{m}` of `_Balances` would have two parameters `{id}`, and the module does not load (`SyntaxError`)"
        ),
        (_, Role::Kwarg(o)) => tr!(
            "`{o}` のメソッドに同じ名前の引数 `{id}` が二つでき、モジュールを読み込めません（`SyntaxError`）",
            "a method of `{o}` would have two parameters `{id}`, and the module does not load (`SyntaxError`)"
        ),
        _ => unreachable!("Python's names are attributes of `Book`, methods of `_Balances` and parameters"),
    }
}

fn owner_of(r: &Role) -> String {
    match r {
        Role::Type => String::new(),
        Role::Field(o) | Role::Method(o) | Role::Variant(o) | Role::Attr(o) | Role::Kwarg(o) | Role::Member(o) => o.clone(),
    }
}

/// Python changes a name that begins with `__` and does not end with `__` inside a class, to
/// `_<class>__name` (name mangling): the class of a transfer (`_<transfer>Transfer`) whose name
/// begins with `_`, which `Book` makes, and an account, or a parameter, whose name begins with `__`.
fn python_mangled(b: &Book) -> Vec<Found> {
    let mangled = |s: &str| s.starts_with("__") && !s.ends_with("__");
    let said = |note: Text| vec![Said { lang: Lang::Python, backends: ALL.to_vec(), note }];
    let mut out = Vec::new();
    for (k, t) in b.transfers.iter().enumerate() {
        if t.name.starts_with('_') {
            let class = format!("_{}Transfer", t.name);
            out.push(Found {
                subject: Subject::Mangled(Name::Transfer(k)),
                said: said(tr!(
                    "クライアントは振替をクラス `{class}` にします。Python はクラスの中で、`__` で始まり `__` で終わらない名前を書き換える（名前修飾）ので、`Book` の中で書いた `{class}` は `_Book{class}` になります。`Book` を作れず、どの呼び出しも止まります（`NameError`）。",
                    "the client writes the transfer as the class `{class}`, and Python changes a name that begins with `__`, and does not end with it, inside a class (name mangling): `{class}` written in `Book` is `_Book{class}`, so `Book` cannot be made, and no call goes through (`NameError`)."
                )),
            });
        }
        if mangled(&t.name) {
            continue;
        }
        let class = format!("_{}Transfer", t.name);
        for (i, p) in t.params.iter().enumerate() {
            let n = python::name(&p.name);
            if mangled(&n) {
                let m = format!("_{}{n}", class.trim_start_matches('_'));
                out.push(Found {
                    subject: Subject::Mangled(Name::Param(k, i)),
                    said: said(tr!(
                        "Python はクラスの中で、`__` で始まり `__` で終わらない名前を書き換える（名前修飾）ので、`{class}` のメソッドの引数 `{n}` は `{m}` になり、`{n}=` で渡せません（`TypeError`）。",
                        "Python changes a name that begins with `__`, and does not end with it, inside a class (name mangling): the parameter `{n}` of the methods of `{class}` is `{m}`, which `{n}=` does not reach (`TypeError`)."
                    )),
                });
            }
        }
    }
    for (k, a) in b.accounts.iter().enumerate() {
        let n = python::name(&a.name);
        if mangled(&n) {
            out.push(Found {
                subject: Subject::Mangled(Name::Account(k)),
                said: said(tr!(
                    "Python はクラスの中で、`__` で始まり `__` で終わらない名前を書き換える（名前修飾）ので、`_Balances` のメソッド `{n}` は `_Balances{n}` になり、`book.balance.{n}(…)` で呼べません（`AttributeError`）。",
                    "Python changes a name that begins with `__`, and does not end with it, inside a class (name mangling): the method `{n}` of `_Balances` is `_Balances{n}`, which `book.balance.{n}(…)` does not reach (`AttributeError`)."
                )),
            });
        }
        for (i, p) in a.params.iter().enumerate() {
            let q = python::name(p);
            if mangled(&q) && !mangled(&n) {
                out.push(Found {
                    subject: Subject::Mangled(Name::AccountParam(k, i)),
                    said: said(tr!(
                        "Python はクラスの中で、`__` で始まり `__` で終わらない名前を書き換える（名前修飾）ので、`_Balances` のメソッド `{n}` の引数 `{q}` は `_Balances{q}` になり、`{q}=` で渡せません（`TypeError`）。",
                        "Python changes a name that begins with `__`, and does not end with it, inside a class (name mangling): the parameter `{q}` of the method `{n}` of `_Balances` is `_Balances{q}`, which `{q}=` does not reach (`TypeError`)."
                    )),
                });
            }
        }
    }
    out
}

/// The SQL types of a PostgreSQL function's parameters after the tenant's, and how many of the
/// last ones a call may leave out.
struct Function {
    name: String,
    of: Name,
    /// what it is, as a note says it: `振替 \`x\` の \`do\``
    what: Text,
    types: Vec<&'static str>,
    optional: usize,
}

fn pg_functions(b: &Book) -> Vec<Function> {
    let mut out = Vec::new();
    for (k, t) in b.transfers.iter().enumerate() {
        let ty = |i: usize| match t.params[i].ty {
            Ty::Str => "text",
            Ty::Amount(_) => "bigint",
        };
        let all: Vec<&'static str> = (0..t.params.len()).map(ty).collect();
        let keys: Vec<&'static str> = postgres::key_params(t).iter().map(|_| "text").collect();
        let mut ops: Vec<(&str, Vec<&'static str>, usize)> = Vec::new();
        if t.is_pending() {
            let ap = t.amount_params();
            let mut post = keys.clone();
            post.extend(ap.iter().map(|_| "bigint"));
            ops.push(("hold", all.clone(), 0));
            ops.push(("post", post, ap.len()));
            ops.push(("void", keys.clone(), 0));
            ops.push(("status", keys.clone(), 0));
        } else {
            ops.push(("do", all.clone(), 0));
        }
        for (op, types, optional) in ops {
            out.push(Function {
                name: postgres::op_function(t, op),
                of: Name::Transfer(k),
                what: tr!("振替 `{}` の `{op}` の関数", "the function of `{op}` of the transfer `{}`", t.name),
                types,
                optional,
            });
        }
    }
    for (k, a) in b.accounts.iter().enumerate() {
        out.push(Function {
            name: postgres::balance_function(a),
            of: Name::Account(k),
            what: tr!("勘定 `{}` の残高の関数", "the function of the balance of the account `{}`", a.name),
            types: a.params.iter().map(|_| "text").collect(),
            optional: 0,
        });
    }
    out
}

/// How many arguments a call of `f` gives, the tenant's among them: the fewest and the most.
fn arity(f: &Function) -> (usize, usize) {
    (1 + f.types.len() - f.optional, 1 + f.types.len())
}

/// PostgreSQL: two functions of one name that a call of the same number of arguments can reach.
/// PostgreSQL tells functions of one name apart by their parameters' types: with the same types,
/// the SQL does not load (`create or replace` cannot change what the first returns); with others,
/// a call whose arguments have no type (`unknown`, as node-postgres sends them) reaches the one
/// that takes only strings.
fn pg_found(b: &Book) -> Vec<Found> {
    let fs = pg_functions(b);
    let mut out: Vec<Found> = Vec::new();
    for (i, f) in fs.iter().enumerate() {
        for g in &fs[i + 1..] {
            if f.name != g.name {
                continue;
            }
            let ((a, z), (c, d)) = (arity(f), arity(g));
            let (n, to) = (a.max(c), z.min(d));
            if n > to {
                continue;
            }
            let fname = &f.name;
            let same = f.types == g.types;
            let then = if same {
                tr!(
                    "PostgreSQL は同じ名前の関数を引数の型で分けますが、この二つは引数の型も同じなので、SQL のファイルを読み込めません（二つ目で `cannot change return type of existing function`）",
                    "PostgreSQL tells functions of one name apart by the types of their parameters, and these two take the same types: the SQL file does not load (`cannot change return type of existing function` at the second)"
                )
            } else {
                tr!(
                    "PostgreSQL は同じ名前の関数を引数の型で分けますが、この二つはどちらも引数 {n} 個で呼べるので、型を付けずに送る呼び出し（TypeScript のクライアントの pg がそう送ります）は、文字列だけを取るほうの関数に届きます",
                    "PostgreSQL tells functions of one name apart by the types of their parameters, and both of these take {n} arguments: a call that sends them with no type (as pg, the TypeScript client's driver, does) reaches the one that takes only strings"
                )
            };
            let (m, n2) = ordered(b, f.of, g.of);
            let (x, y) = if m == f.of { (f, g) } else { (g, f) };
            let note = tr!(
                "SQL は、振替の操作を `<振替>_<操作>`、勘定の残高を `balance_<勘定>` という関数にするので、{}と{}は、どちらも `{fname}` です。{}。",
                "the SQL names the function of a transfer's operation `<transfer>_<operation>`, and the one of an account's balance `balance_<account>`, so {} and {} are both `{fname}`. {}.",
                x.what.ja, y.what.ja, then.ja;
                x.what.en, y.what.en, then.en
            );
            let subject = Subject::Two(m, n2);
            if out.iter().any(|o| o.subject == subject) {
                continue;
            }
            out.push(Found { subject, said: vec![Said { lang: Lang::Sql, backends: vec![Backend::Postgres], note }] });
        }
    }
    out
}

/// Every finding of a book that loads: in the clients, by language, then in PostgreSQL's SQL.
pub fn find(b: &Book) -> Vec<Found> {
    let mut out: Vec<Found> = Vec::new();
    for decls in [typescript_decls(b), python_decls(b), go_decls(b), rust_decls(b)] {
        for h in hits(b, &decls) {
            let note = note_of(b, &h);
            let said = Said { lang: h.rule.lang(), backends: h.on.clone(), note };
            match out.iter_mut().find(|f| f.subject == h.subject) {
                Some(f) => f.said.push(said),
                None => out.push(Found { subject: h.subject, said: vec![said] }),
            }
        }
    }
    for f in python_mangled(b).into_iter().chain(pg_found(b)) {
        match out.iter_mut().find(|o| o.subject == f.subject) {
            Some(o) => o.said.extend(f.said),
            None => out.push(f),
        }
    }
    out.sort_by_key(|f| {
        let (l, c) = f.subject.at().place(b);
        (l, c, f.subject)
    });
    out
}

/// The headline of a finding, in `place`: the clients of some languages (`Go・Rust のクライアント`),
/// or the code of a target; `many` when the English is plural.
fn headline(b: &Book, f: &Found, place: &Text, many: bool) -> Text {
    match f.subject {
        Subject::Two(x, y) if f.said.iter().all(|s| s.lang == Lang::Sql) => {
            let p = pair(b, x, y);
            tr!("{} は、PostgreSQL の SQL で同じ関数になります", "{} are one function in the SQL of PostgreSQL", p.ja; cap(&p.en))
        }
        Subject::Two(x, y) => {
            let p = pair(b, x, y);
            tr!("{} は、{}で同じ識別子になります", "{} are one identifier in {}", p.ja, place.ja; cap(&p.en), place.en)
        }
        Subject::Own(n) => {
            let w = n.what(b);
            let uses = if many { "use themselves" } else { "uses itself" };
            tr!("{} は、{}が自分で使う名前と同じになります", "{} is a name {} {uses}", w.ja, place.ja; cap(&w.en), place.en)
        }
        Subject::Mangled(n) => {
            let w = n.what(b);
            tr!("{} は、{}でそのままでは使えません", "{} cannot be used as it is in {}", w.ja, place.ja; cap(&w.en), place.en)
        }
    }
}

fn cap(s: &str) -> String {
    ritsu_base::text::capitalize(s)
}

/// `Go と Rust のクライアント` / `the Go and Rust clients`.
fn clients(langs: &[Lang]) -> Text {
    let names: Vec<String> = langs.iter().map(|l| lang_name(*l).to_string()).collect();
    let en = if names.len() == 1 { format!("the {} client", names[0]) } else { format!("the {} clients", and(&names)) };
    Text::new(format!("{} のクライアント", names.join("・")), en)
}

fn fix(f: &Found, at_build: bool) -> Text {
    let rename = match f.subject {
        Subject::Two(..) => tr!("どちらかの名前を変えてください", "rename one of the two"),
        _ => tr!("名前を変えてください", "rename it"),
    };
    let since = tr!(
        "すでに使っている帳簿なら、名前を変えたあと、使っているリビジョンと `chobo check --diff-base` で比べてください",
        "if the book is in use, check the renamed book against the revision in use with `chobo check --diff-base`"
    );
    if at_build {
        tr!("{}。{}。", "{}; {}.", rename.ja, since.ja; cap(&rename.en), since.en)
    } else {
        tr!(
            "使わないターゲットなら、このままで構いません。使うなら、{}。{}。",
            "For a target you do not build, leave it as it is. For one you do, {}; {}.",
            rename.ja, since.ja; rename.en, since.en
        )
    }
}

/// W108: each finding, at the later of its names, with a note for each language.
pub fn check(b: &Book) -> Vec<Diag> {
    find(b)
        .iter()
        .map(|f| {
            let langs: Vec<Lang> = [Lang::TypeScript, Lang::Python, Lang::Go, Lang::Rust].into_iter().filter(|l| f.said.iter().any(|s| s.lang == *l)).collect();
            let (line, col) = f.subject.at().place(b);
            let mut d = diag::warning("W108", line, col, headline(b, f, &clients(&langs), langs.len() > 1));
            for l in [Lang::TypeScript, Lang::Python, Lang::Go, Lang::Rust, Lang::Sql] {
                for s in f.said.iter().filter(|s| s.lang == l) {
                    d = d.note(join(&prefix(l, &s.targets()), &s.note));
                }
            }
            d.hint(fix(f, false))
        })
        .collect()
}

/// E064: the findings that break `target`, which its build does not write.
pub fn check_target(b: &Book, target: Target) -> Vec<Diag> {
    find(b)
        .iter()
        .filter(|f| f.targets().contains(&target))
        .map(|f| {
            let t = target.name();
            let place = tr!("`{t}` のコード", "the code of `{t}`");
            let (line, col) = f.subject.at().place(b);
            let mut d = diag::error("E064", line, col, headline(b, f, &place, false));
            for s in f.said.iter().filter(|s| s.targets().contains(&target)) {
                d = d.note(join(&prefix(s.lang, &s.targets()), &s.note));
            }
            d.hint(fix(f, true))
        })
        .collect()
}

fn join(a: &Text, b: &Text) -> Text {
    Text::new(format!("{}{}", a.ja, b.ja), format!("{}{}", a.en, b.en))
}
