//! The Go clients: a package each, `book.go` with the book's part and `runtime.go` with what
//! every book shares. The code is written as gofmt writes it (C8 checks it with `gofmt -l`).

use super::*;
use crate::postgres;

const TB_RUNTIME: &str = include_str!("runtime/tigerbeetle.go");
const PG_RUNTIME: &str = include_str!("runtime/postgres.go");

use ritsu_emit::ident::is_ascii_ident as ascii_ident;
use ritsu_emit::words::go::KEYWORDS;

/// A name of the book as Go exports it: `sku` is `Sku`, and a name that has no capital letter,
/// as Japanese has none, gets an `X` in front (`X引当`).
pub use ritsu_emit::ident::go_exported as go_name;

/// The package: the book's name when it is an ASCII identifier, else the file's name
/// (`inventory.ja.book` is `inventory_ja`), else `book` (PLAN 0.3).
pub fn package(book: &Book, stem: &str) -> String {
    let file = stem.replace(['.', '-'], "_");
    for c in [book.name.to_ascii_lowercase(), file.to_ascii_lowercase()] {
        if ascii_ident(&c) && !KEYWORDS.contains(&c.as_str()) && c != "_" {
            return c;
        }
    }
    "book".into()
}

/// The fields of the book value, one per transfer kind, kept apart from `Balance` and `Expire`
/// (the PostgreSQL client's method), so that the two clients name them alike.
pub fn members(book: &Book) -> Vec<String> {
    unique(book.transfers.iter().map(|t| go_name(&t.name)).collect(), &["Balance", "Expire"])
}

/// The fields of a struct of the parameters `which` of `t`.
pub fn fields(t: &TransferKind, which: &[usize]) -> Vec<String> {
    unique(which.iter().map(|i| go_name(&t.params[*i].name)).collect(), &[])
}

/// The fields of an account kind's arguments.
pub fn account_fields(a: &AccountKind) -> Vec<String> {
    unique(a.params.iter().map(|p| go_name(p)).collect(), &[])
}

/// The methods of `Balances`, one per account kind.
pub fn balance_methods(book: &Book) -> Vec<String> {
    unique(book.accounts.iter().map(|a| go_name(&a.name)).collect(), &[])
}

/// A string as a Go literal.
use ritsu_emit::lit::go as q;

/// Struct fields as gofmt aligns them: the types in one column, counted in runes.
fn struct_fields(rows: &[(String, String)]) -> String {
    let w = rows.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(0);
    rows.iter().map(|(n, t)| format!("\t{n}{}{t}\n", " ".repeat(w - n.chars().count() + 1))).collect()
}

fn struct_type(doc: &str, name: &str, rows: &[(String, String)]) -> String {
    format!("// {name} {doc}\ntype {name} struct {{\n{}}}\n\n", struct_fields(rows))
}

fn moves_text(book: &Book, t: &TransferKind) -> String {
    let moves: Vec<String> = t.moves.iter().map(|m| format!("{} from {} to {}", book.amount_text(t, m), book.ref_text(t, &m.from), book.ref_text(t, &m.to))).collect();
    let keys: Vec<&str> = t.key.iter().map(|i| t.params[*i].name.as_str()).collect();
    format!("{}; once per {}", moves.join(", then "), keys.join(", "))
}

fn arg_types(book: &Book) -> String {
    let mut o = String::new();
    for t in &book.transfers {
        let g = go_name(&t.name);
        let ty = |i: &usize| if matches!(t.params[*i].ty, Ty::Amount(_)) { "int64" } else { "string" };
        let all: Vec<usize> = (0..t.params.len()).collect();
        let rows: Vec<(String, String)> = fields(t, &all).into_iter().zip(all.iter().map(ty)).map(|(n, t)| (n, t.to_string())).collect();
        o.push_str(&struct_type(&format!("are the arguments of {}: {}.", t.name, moves_text(book, t)), &format!("{g}Args"), &rows));
        if t.is_pending() {
            let keys = postgres::key_params(t);
            let rows: Vec<(String, String)> = fields(t, &keys).into_iter().zip(keys.iter().map(ty)).map(|(n, t)| (n, t.to_string())).collect();
            o.push_str(&struct_type(&format!("is the key of a hold of {}.", t.name), &format!("{g}Key"), &rows));
            let ap = t.amount_params();
            if !ap.is_empty() {
                let rows: Vec<(String, String)> = fields(t, &ap).into_iter().map(|n| (n, "int64".to_string())).collect();
                o.push_str(&struct_type(&format!("are what a post of {} posts, when not all of it.", t.name), &format!("{g}Amounts"), &rows));
            }
        }
    }
    for a in &book.accounts {
        if !a.params.is_empty() {
            let rows: Vec<(String, String)> = account_fields(a).into_iter().map(|n| (n, "string".to_string())).collect();
            o.push_str(&struct_type(&format!("are the arguments of the account {}.", a.name), &format!("{}Account", go_name(&a.name)), &rows));
        }
    }
    o
}

/// `args.X注文` for each parameter of `which`.
fn field_values(t: &TransferKind, which: &[usize], obj: &str) -> Vec<String> {
    fields(t, which).into_iter().map(|f| format!("{obj}.{f}")).collect()
}

fn plan_literal(book: &Book) -> String {
    let p = plan(book);
    let mut o = String::from("var theUnits = []unitDef{\n");
    for u in &p.units {
        o.push_str(&format!("\t{{{}, {}, {}}},\n", q(&u.name), u.ledger, u.sink_code));
    }
    o.push_str("}\n\nvar theAccounts = []accountDef{\n");
    let bound = |b: &Option<(i128, String)>| match b {
        Some((v, r)) => format!("&boundDef{{{v}, {}}}", q(r)),
        None => "nil".into(),
    };
    let strs = |xs: &[String]| if xs.is_empty() { "nil".to_string() } else { format!("[]string{{{}}}", xs.iter().map(|x| q(x)).collect::<Vec<_>>().join(", ")) };
    for a in &p.accounts {
        o.push_str(&format!("\t{{{}, {}, {}, {}, {}, {}, {}}},\n", q(&a.name), strs(&a.params), q(&a.unit), a.code, a.flagged, bound(&a.lower), bound(&a.upper)));
    }
    o.push_str("}\n\nvar theTransfers = []transferDef{\n");
    let rf = |r: &RefPlan| {
        let args: Vec<String> = r
            .args
            .iter()
            .map(|a| match a {
                ArgPlan::Param(i) => format!("{{{i}, \"\"}}"),
                ArgPlan::Lit(s) => format!("{{-1, {}}}", q(s)),
            })
            .collect();
        let args = if args.is_empty() { "nil".to_string() } else { format!("[]argDef{{{}}}", args.join(", ")) };
        format!("refDef{{{}, {args}}}", q(&r.kind))
    };
    for t in &p.transfers {
        let ps: Vec<String> = t.params.iter().map(|x| format!("{{{}, {}}}", q(&x.name), x.amount)).collect();
        let key: Vec<String> = t.key.iter().map(|i| i.to_string()).collect();
        let moves: Vec<String> = t
            .moves
            .iter()
            .map(|m| {
                let amount = match &m.amount {
                    AmountPlan::Param(i) => format!("amountDef{{{i}, 0}}"),
                    AmountPlan::Lit(v) => format!("amountDef{{-1, {v}}}"),
                };
                let roles: Vec<String> = m.roles.iter().map(|r| q(r)).collect();
                format!("{{{amount}, {}, {}, []string{{{}}}}}", rf(&m.from), rf(&m.to), roles.join(", "))
            })
            .collect();
        o.push_str(&format!(
            "\t{{{}, []paramDef{{{}}}, []int{{{}}}, {}, {}, {}, {}, []moveDef{{{}}}}},\n",
            q(&t.name),
            ps.join(", "),
            key.join(", "),
            t.pending,
            t.timeout,
            t.code,
            q(&t.definition),
            moves.join(", ")
        ));
    }
    o.push_str(&format!("}}\n\nvar theBook = newBook({}, theUnits, theAccounts, theTransfers)\n", q(&book.name)));
    o
}

fn runtime(text: &str, pkg: &str) -> String {
    format!("{}\n{}", Comment::Slashes.line(&generated("chobo")), text.replace("{{PKG}}", pkg))
}

/// The methods of a transfer kind's calls, with the body each one has.
struct Method {
    doc: String,
    sig: String,
    body: String,
}

fn calls_type(t: &TransferKind, runtime: &str, methods: &[Method]) -> String {
    let g = go_name(&t.name);
    let mut o = format!("// {g}Calls are the operations of {}.\ntype {g}Calls struct{{ r *{runtime} }}\n\n", t.name);
    for m in methods {
        o.push_str(&format!("// {}\nfunc (c {g}Calls) {} {{\n{}}}\n\n", m.doc, m.sig, m.body));
    }
    o
}

fn book_struct(book: &Book, backend: &str, pg: bool) -> String {
    let mut rows: Vec<(String, String)> = book.transfers.iter().zip(members(book)).map(|(t, f)| (f, format!("{}Calls", go_name(&t.name)))).collect();
    rows.push(("Balance".into(), "Balances".into()));
    if pg {
        rows.push(("r".into(), "*pgRuntime".into()));
    }
    format!("// Book is the book {} (v{}), called on {backend}.\ntype Book struct {{\n{}}}\n\n", book.name, book.version, struct_fields(&rows))
}

fn header(book: &Book, origin: &Origin, what: &str, pkg: &str) -> String {
    let line = super::head(book, origin, Comment::Slashes);
    format!("{line}\n// Package {pkg} calls the book {} (v{}) {what}\npackage {pkg}\n\nimport \"context\"\n\n", book.name, book.version)
}

pub fn tigerbeetle(book: &Book, stem: &str, origin: &Origin) -> Vec<(String, String)> {
    let pkg = package(book, stem);
    let mut o = header(book, origin, "on TigerBeetle, through tigerbeetle-go v0.17.9 (DESIGN 4.2, 4.3).", &pkg);
    o.push_str(&arg_types(book));
    o.push_str(&book_struct(book, "TigerBeetle", false));
    let names = members(book);
    let inits: Vec<String> = book.transfers.iter().zip(&names).map(|(t, f)| format!("{f}: {}Calls{{r}}", go_name(&t.name))).collect();
    o.push_str(&format!(
        "// TigerBeetle is the book on TigerBeetle. A tenant is a set of balances of its own (DESIGN 4.3); \"\" for one.\nfunc TigerBeetle(c TigerBeetleClient, tenant string) *Book {{\n\tr := newTbRuntime(&theBook, c, tenant)\n\treturn &Book{{{}, Balance: Balances{{r}}}}\n}}\n\n",
        inits.join(", ")
    ));
    for t in &book.transfers {
        let g = go_name(&t.name);
        let all: Vec<usize> = (0..t.params.len()).collect();
        let kind = q(&t.name);
        let args = field_values(t, &all, "args").join(", ");
        let mut methods = Vec::new();
        if t.is_pending() {
            let key = field_values(t, &t.key, "key").join(", ");
            let ap = t.amount_params();
            methods.push(Method {
                doc: format!("Hold holds {}.", moves_text(book, t)),
                sig: format!("Hold(ctx context.Context, args {g}Args) (Result, error)"),
                body: format!("\treturn c.r.move(ctx, {kind}, \"hold\", []any{{{args}}})\n"),
            });
            if ap.is_empty() {
                methods.push(Method {
                    doc: "Post posts the hold of key.".into(),
                    sig: format!("Post(ctx context.Context, key {g}Key) (Result, error)"),
                    body: format!("\treturn c.r.end(ctx, {kind}, \"post\", []string{{{key}}}, nil)\n"),
                });
            } else {
                let am = field_values(t, &ap, "amounts").join(", ");
                methods.push(Method {
                    doc: "Post posts the hold of key: all of it when amounts is nil.".into(),
                    sig: format!("Post(ctx context.Context, key {g}Key, amounts *{g}Amounts) (Result, error)"),
                    body: format!("\tvar a []int64\n\tif amounts != nil {{\n\t\ta = []int64{{{am}}}\n\t}}\n\treturn c.r.end(ctx, {kind}, \"post\", []string{{{key}}}, a)\n"),
                });
            }
            methods.push(Method {
                doc: "Void voids the hold of key.".into(),
                sig: format!("Void(ctx context.Context, key {g}Key) (Result, error)"),
                body: format!("\treturn c.r.end(ctx, {kind}, \"void\", []string{{{key}}}, nil)\n"),
            });
            methods.push(Method {
                doc: "Status is where the hold of key is; \"\" when there is no such hold.".into(),
                sig: format!("Status(ctx context.Context, key {g}Key) (HoldState, error)"),
                body: format!("\treturn c.r.status(ctx, {kind}, []string{{{key}}})\n"),
            });
        } else {
            methods.push(Method {
                doc: format!("Do moves {}.", moves_text(book, t)),
                sig: format!("Do(ctx context.Context, args {g}Args) (Result, error)"),
                body: format!("\treturn c.r.move(ctx, {kind}, \"do\", []any{{{args}}})\n"),
            });
        }
        o.push_str(&calls_type(t, "tbRuntime", &methods));
    }
    o.push_str("// Balances are the balance of each account kind.\ntype Balances struct{ r *tbRuntime }\n\n");
    for (a, m) in book.accounts.iter().zip(balance_methods(book)) {
        if a.params.is_empty() {
            o.push_str(&format!(
                "// {m} is the balance of {}.\nfunc (b Balances) {m}(ctx context.Context) (Balance, error) {{\n\treturn b.r.balance(ctx, {}, nil)\n}}\n\n",
                a.name,
                q(&a.name)
            ));
        } else {
            let vals: Vec<String> = account_fields(a).into_iter().map(|f| format!("args.{f}")).collect();
            o.push_str(&format!(
                "// {m} is the balance of {}.\nfunc (b Balances) {m}(ctx context.Context, args {}Account) (Balance, error) {{\n\treturn b.r.balance(ctx, {}, []string{{{}}})\n}}\n\n",
                a.name,
                go_name(&a.name),
                q(&a.name),
                vals.join(", ")
            ));
        }
    }
    o.push_str(&plan_literal(book));
    vec![(format!("{pkg}/book.go"), o), (format!("{pkg}/runtime.go"), runtime(TB_RUNTIME, &pkg))]
}

/// A Go raw string: the SQL never has a backquote.
fn raw(s: &str) -> String {
    format!("`{s}`")
}

pub fn postgres(book: &Book, stem: &str, origin: &Origin) -> Vec<(String, String)> {
    let pkg = package(book, stem);
    let mut o = header(
        book,
        origin,
        &format!("on PostgreSQL, through the SQL functions of `chobo build --target postgres` (schema {}) and pgx v5 (DESIGN 4.1, 4.3).", postgres::ident(&book.name)),
        &pkg,
    );
    o.push_str(&arg_types(book));
    o.push_str(&book_struct(book, "PostgreSQL", true));
    let names = members(book);
    let inits: Vec<String> = book.transfers.iter().zip(&names).map(|(t, f)| format!("{f}: {}Calls{{r}}", go_name(&t.name))).collect();
    o.push_str(&format!(
        "// Postgres is the book on PostgreSQL. A tenant is a set of balances of its own (DESIGN 4.3); \"\" for one. Each call is a\n// transaction of its own, unless q is a transaction of the caller's.\nfunc Postgres(q Querier, tenant string) *Book {{\n\tr := &pgRuntime{{q: q, tenant: tenant}}\n\treturn &Book{{{}, Balance: Balances{{r}}, r: r}}\n}}\n\n",
        inits.join(", ")
    ));
    o.push_str(&format!(
        "// Expire gives back what the holds past their expiry hold; call it from a job (DESIGN 4.1).\nfunc (b *Book) Expire(ctx context.Context) (int, error) {{\n\treturn b.r.expire(ctx, {})\n}}\n\n",
        raw(&postgres::expire_sql(book))
    ));
    let sql = |t: &TransferKind, op: &str, n: usize| raw(&format!("select * from {}({})", postgres::qualified(book, &postgres::op_function(t, op)), (1..=n).map(|i| format!("${i}")).collect::<Vec<_>>().join(", ")));
    for t in &book.transfers {
        let g = go_name(&t.name);
        let all: Vec<usize> = (0..t.params.len()).collect();
        let keys = postgres::key_params(t);
        let args = field_values(t, &all, "args").join(", ");
        let mut methods = Vec::new();
        if t.is_pending() {
            let key = field_values(t, &keys, "key").join(", ");
            let ap = t.amount_params();
            methods.push(Method {
                doc: format!("Hold holds {}.", moves_text(book, t)),
                sig: format!("Hold(ctx context.Context, args {g}Args) (Result, error)"),
                body: format!("\treturn c.r.call(ctx, {}, []any{{c.r.tenant, {args}}})\n", sql(t, "hold", 1 + all.len())),
            });
            if ap.is_empty() {
                methods.push(Method {
                    doc: "Post posts the hold of key.".into(),
                    sig: format!("Post(ctx context.Context, key {g}Key) (Result, error)"),
                    body: format!("\treturn c.r.call(ctx, {}, []any{{c.r.tenant, {key}}})\n", sql(t, "post", 1 + keys.len())),
                });
            } else {
                let am = field_values(t, &ap, "amounts");
                let nulls = vec!["nil"; ap.len()].join(", ");
                let mut body = format!("\targs := []any{{c.r.tenant, {key}, {nulls}}}\n\tif amounts != nil {{\n");
                for (j, v) in am.iter().enumerate() {
                    body.push_str(&format!("\t\targs[{}] = {v}\n", 1 + keys.len() + j));
                }
                body.push_str(&format!("\t}}\n\treturn c.r.call(ctx, {}, args)\n", sql(t, "post", 1 + keys.len() + ap.len())));
                methods.push(Method {
                    doc: "Post posts the hold of key: all of it when amounts is nil.".into(),
                    sig: format!("Post(ctx context.Context, key {g}Key, amounts *{g}Amounts) (Result, error)"),
                    body,
                });
            }
            methods.push(Method {
                doc: "Void voids the hold of key.".into(),
                sig: format!("Void(ctx context.Context, key {g}Key) (Result, error)"),
                body: format!("\treturn c.r.call(ctx, {}, []any{{c.r.tenant, {key}}})\n", sql(t, "void", 1 + keys.len())),
            });
            methods.push(Method {
                doc: "Status is where the hold of key is; \"\" when there is no such hold.".into(),
                sig: format!("Status(ctx context.Context, key {g}Key) (HoldState, error)"),
                body: format!("\treturn c.r.status(ctx, {}, []any{{c.r.tenant, {key}}})\n", raw(&postgres::status_sql(book, t))),
            });
        } else {
            methods.push(Method {
                doc: format!("Do moves {}.", moves_text(book, t)),
                sig: format!("Do(ctx context.Context, args {g}Args) (Result, error)"),
                body: format!("\treturn c.r.call(ctx, {}, []any{{c.r.tenant, {args}}})\n", sql(t, "do", 1 + all.len())),
            });
        }
        o.push_str(&calls_type(t, "pgRuntime", &methods));
    }
    o.push_str("// Balances are the balance of each account kind.\ntype Balances struct{ r *pgRuntime }\n\n");
    for (a, m) in book.accounts.iter().zip(balance_methods(book)) {
        let s = raw(&postgres::balance_sql(book, a));
        if a.params.is_empty() {
            o.push_str(&format!(
                "// {m} is the balance of {}.\nfunc (b Balances) {m}(ctx context.Context) (Balance, error) {{\n\treturn b.r.balance(ctx, {s}, []any{{b.r.tenant}})\n}}\n\n",
                a.name
            ));
        } else {
            let vals: Vec<String> = account_fields(a).into_iter().map(|f| format!("args.{f}")).collect();
            o.push_str(&format!(
                "// {m} is the balance of {}.\nfunc (b Balances) {m}(ctx context.Context, args {}Account) (Balance, error) {{\n\treturn b.r.balance(ctx, {s}, []any{{b.r.tenant, {}}})\n}}\n\n",
                a.name,
                go_name(&a.name),
                vals.join(", ")
            ));
        }
    }
    // the last blank line is gofmt's to drop
    while o.ends_with("\n\n") {
        o.pop();
    }
    vec![(format!("{pkg}/book.go"), o), (format!("{pkg}/runtime.go"), runtime(PG_RUNTIME, &pkg))]
}
