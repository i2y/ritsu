//! The TypeScript clients: one file each, written so that Node runs it by stripping its types
//! (no `enum`, no `namespace` with code, no parameter properties).

use super::*;
use crate::mysql as my;
use crate::postgres;

const TB_RUNTIME: &str = include_str!("runtime/tigerbeetle.ts");
const PG_RUNTIME: &str = include_str!("runtime/postgres.ts");
const MY_RUNTIME: &str = include_str!("runtime/mysql.ts");

/// A string as a TypeScript literal.
use ritsu_emit::lit::json as q;

/// The members of the book value, one per transfer kind, kept apart from `balance` and `expire`
/// (the PostgreSQL client's), so that the two clients name them alike.
pub fn members(book: &Book) -> Vec<String> {
    unique(book.transfers.iter().map(|t| t.name.clone()).collect(), &["balance", "expire"])
}

fn reason_type(book: &Book) -> String {
    let rs: Vec<String> = reasons(book).iter().map(|r| q(r)).collect();
    format!(
        "/** Why a call of {} can be refused: the bounds' own reasons, then the ones chobo gives itself. */\nexport type Reason = {};\n\n\
         /** What a call answers. A refusal is an answer, not an error: nothing has changed, and the reason says why. */\nexport type Result = {{ result: \"done\" }} | {{ result: \"done_before\" }} | {{ result: \"refused\"; reason: Reason }};\n\n",
        book.name,
        rs.join(" | ")
    )
}

/// The fields of the parameters `which` of `t`: an amount is a `bigint`, or in a package of ritsu's
/// the package's type of its unit (`units.USDc`).
fn field_list(book: &Book, t: &TransferKind, which: &[usize], pkg: Option<&InPackage>) -> String {
    let fs: Vec<String> = which
        .iter()
        .map(|i| {
            let p = &t.params[*i];
            let ty = match (unit_type(book, &p.ty, pkg), p.ty) {
                (Some(u), _) => format!("units.{u}"),
                (None, Ty::Amount(_)) => "bigint".into(),
                (None, Ty::Str) => "string".into(),
            };
            format!("{}: {ty}", p.name)
        })
        .collect();
    format!("{{ {} }}", fs.join("; "))
}

/// The import of the package's types of the units (ritsu's DESIGN 9.3), for a client in a package
/// whose calls take an amount of money or of a quantity.
fn units_import(book: &Book, pkg: Option<&InPackage>) -> String {
    match pkg {
        Some(p) if !package_units(book).is_empty() => format!("import type * as units from \"{}\";\n", ritsu_emit::package::ts_path(&p.books_dir(), "units")),
        _ => String::new(),
    }
}

/// The types of the arguments: `入荷Args`, `引当Key`, `引当Amounts`, `在庫Account`.
fn arg_types(book: &Book, pkg: Option<&InPackage>) -> String {
    let mut o = String::new();
    for t in &book.transfers {
        let all: Vec<usize> = (0..t.params.len()).collect();
        let moves: Vec<String> = t.moves.iter().map(|m| format!("{} from {} to {}", book.amount_text(t, m), book.ref_text(t, &m.from), book.ref_text(t, &m.to))).collect();
        let keys: Vec<&str> = t.key.iter().map(|i| t.params[*i].name.as_str()).collect();
        o.push_str(&format!("/** {}: {}; once per {}. */\n", t.name, moves.join(", then "), keys.join(", ")));
        o.push_str(&format!("export type {}Args = {};\n", t.name, field_list(book, t, &all, pkg)));
        if t.is_pending() {
            o.push_str(&format!("export type {}Key = {};\n", t.name, field_list(book, t, &postgres::key_params(t), pkg)));
            let ap = t.amount_params();
            if !ap.is_empty() {
                o.push_str(&format!("export type {}Amounts = {};\n", t.name, field_list(book, t, &ap, pkg)));
            }
        }
    }
    for a in &book.accounts {
        if !a.params.is_empty() {
            let fs: Vec<String> = a.params.iter().map(|p| format!("{p}: string")).collect();
            o.push_str(&format!("export type {}Account = {{ {} }};\n", a.name, fs.join("; ")));
        }
    }
    o.push('\n');
    o
}

fn book_interface(book: &Book, on: &str, expire: bool) -> String {
    let names = members(book);
    let mut o = format!("/** The book {} (v{}), called on {on}. */\nexport interface Book {{\n", book.name, book.version);
    for (t, n) in book.transfers.iter().zip(&names) {
        if t.is_pending() {
            let amounts = if t.amount_params().is_empty() { String::new() } else { format!(", amounts?: {}Amounts", t.name) };
            o.push_str(&format!(
                "  {n}: {{\n    hold(args: {0}Args): Promise<Result>;\n    post(key: {0}Key{amounts}): Promise<Result>;\n    void(key: {0}Key): Promise<Result>;\n    status(key: {0}Key): Promise<HoldState | null>;\n  }};\n",
                t.name
            ));
        } else {
            o.push_str(&format!("  {n}: {{ do(args: {}Args): Promise<Result> }};\n", t.name));
        }
    }
    let bs: Vec<String> = book.accounts.iter().map(|a| if a.params.is_empty() { format!("{}(): Promise<Balance>", a.name) } else { format!("{0}(args: {0}Account): Promise<Balance>", a.name) }).collect();
    o.push_str(&format!("  balance: {{ {} }};\n", bs.join("; ")));
    if expire {
        o.push_str("  /** Give back what the holds past their expiry hold; call it from a job. */\n  expire(): Promise<number>;\n");
    }
    o.push_str("}\n\n");
    o
}

/// The arguments as the runtime takes them: checked, and in the order asked for.
fn values(t: &TransferKind, which: &[usize], obj: &str) -> String {
    let vs: Vec<String> = which
        .iter()
        .map(|i| {
            let p = &t.params[*i];
            let f = if matches!(p.ty, Ty::Amount(_)) { "amt" } else { "str" };
            format!("{f}({}, {obj}.{})", q(&p.name), p.name)
        })
        .collect();
    vs.join(", ")
}

fn plan_literal(book: &Book) -> String {
    let p = plan(book);
    let mut o = format!("const BOOK: BookDef = {{\n  name: {},\n  units: {{\n", q(&book.name));
    for u in &p.units {
        o.push_str(&format!("    {}: {{ ledger: {}, sinkCode: {} }},\n", q(&u.name), u.ledger, u.sink_code));
    }
    o.push_str("  },\n  accounts: {\n");
    let bound = |b: &Option<(i128, String)>| match b {
        Some((v, r)) => format!("{{ value: {v}n, reason: {} }}", q(r)),
        None => "null".into(),
    };
    for a in &p.accounts {
        let ps: Vec<String> = a.params.iter().map(|x| q(x)).collect();
        o.push_str(&format!(
            "    {}: {{ params: [{}], unit: {}, code: {}, flagged: {}, lower: {}, upper: {} }},\n",
            q(&a.name),
            ps.join(", "),
            q(&a.unit),
            a.code,
            a.flagged,
            bound(&a.lower),
            bound(&a.upper)
        ));
    }
    o.push_str("  },\n  transfers: {\n");
    let rf = |r: &RefPlan| {
        let args: Vec<String> = r
            .args
            .iter()
            .map(|a| match a {
                ArgPlan::Param(i) => format!("{{ param: {i} }}"),
                ArgPlan::Lit(s) => format!("{{ literal: {} }}", q(s)),
            })
            .collect();
        format!("{{ kind: {}, args: [{}] }}", q(&r.kind), args.join(", "))
    };
    for t in &p.transfers {
        let ps: Vec<String> = t.params.iter().map(|x| format!("{{ name: {}, amount: {} }}", q(&x.name), x.amount)).collect();
        let key: Vec<String> = t.key.iter().map(|i| i.to_string()).collect();
        o.push_str(&format!(
            "    {}: {{\n      params: [{}],\n      key: [{}],\n      pending: {},\n      timeout: {},\n      code: {},\n      definition: {},\n      moves: [\n",
            q(&t.name),
            ps.join(", "),
            key.join(", "),
            t.pending,
            t.timeout,
            t.code,
            q(&t.definition)
        ));
        for m in &t.moves {
            let amount = match &m.amount {
                AmountPlan::Param(i) => format!("{{ param: {i} }}"),
                AmountPlan::Lit(v) => format!("{{ literal: {v}n }}"),
            };
            let roles: Vec<String> = m.roles.iter().map(|r| q(r)).collect();
            o.push_str(&format!("        {{ amount: {amount}, from: {}, to: {}, roles: [{}] }},\n", rf(&m.from), rf(&m.to), roles.join(", ")));
        }
        o.push_str("      ],\n    },\n");
    }
    o.push_str("  },\n};\n");
    o
}

fn split(runtime: &str) -> (&str, &str) {
    runtime.split_once("//@@BOOK@@\n").expect("the runtime has its //@@BOOK@@ line")
}

pub fn tigerbeetle(book: &Book, origin: &Origin, pkg: Option<&InPackage>) -> String {
    let (head, tail) = split(TB_RUNTIME);
    let mut o = super::head(book, origin, Comment::Slashes);
    o.push_str("// It calls the book on TigerBeetle through tigerbeetle-node 0.17.9.\n\n");
    o.push_str(head);
    o.push_str(&units_import(book, pkg));
    o.push('\n');
    o.push_str(&reason_type(book));
    o.push_str(&arg_types(book, pkg));
    o.push_str(&book_interface(book, "TigerBeetle", false));
    let names = members(book);
    o.push_str(&format!(
        "/** The book on TigerBeetle. A tenant is a set of balances of its own; \"\" when not given. */\nexport function tigerbeetle(client: TigerBeetleClient, options: {{ tenant?: string }} = {{}}): Book {{\n  const r = new Runtime(BOOK, client, options.tenant ?? \"\");\n  return {{\n"
    ));
    for (t, n) in book.transfers.iter().zip(&names) {
        let all: Vec<usize> = (0..t.params.len()).collect();
        let kind = q(&t.name);
        if t.is_pending() {
            let key = values(t, &t.key, "key");
            let ap = t.amount_params();
            let amounts = if ap.is_empty() { "null".to_string() } else { format!("amounts === undefined ? null : [{}]", values(t, &ap, "amounts")) };
            let post_params = if ap.is_empty() { "key" } else { "key, amounts" };
            o.push_str(&format!(
                "    {n}: {{\n      hold: (args) => r.move({kind}, \"hold\", [{}]),\n      post: ({post_params}) => r.end({kind}, \"post\", [{key}], {amounts}),\n      void: (key) => r.end({kind}, \"void\", [{key}], null),\n      status: (key) => r.status({kind}, [{key}]),\n    }},\n",
                values(t, &all, "args")
            ));
        } else {
            o.push_str(&format!("    {n}: {{ do: (args) => r.move({kind}, \"do\", [{}]) }},\n", values(t, &all, "args")));
        }
    }
    o.push_str("    balance: {\n");
    for a in &book.accounts {
        let args: Vec<String> = a.params.iter().map(|p| format!("str({}, args.{p})", q(p))).collect();
        let sig = if a.params.is_empty() { "()" } else { "(args)" };
        o.push_str(&format!("      {}: {sig} => r.balance({}, [{}]),\n", a.name, q(&a.name), args.join(", ")));
    }
    o.push_str("    },\n  };\n}\n\n");
    o.push_str(&plan_literal(book));
    o.push_str(tail);
    o
}

pub fn postgres(book: &Book, origin: &Origin, pkg: Option<&InPackage>) -> String {
    let (head, tail) = split(PG_RUNTIME);
    let mut o = super::head(book, origin, Comment::Slashes);
    o.push_str(&format!(
        "// It calls the SQL functions of `chobo build --target postgres` (schema {}) through a connection such as pg's.\n\n",
        postgres::ident(&book.name)
    ));
    o.push_str(head);
    let units = units_import(book, pkg);
    if !units.is_empty() {
        o.push_str(&units);
        o.push('\n');
    }
    o.push_str(&reason_type(book));
    o.push_str(&arg_types(book, pkg));
    o.push_str(&book_interface(book, "PostgreSQL", true));
    let names = members(book);
    o.push_str(
        "/** The book on PostgreSQL. A tenant is a set of balances of its own; \"\" when not given. Each call is a transaction of its own, unless `db` is in one of the caller's. */\nexport function postgres(db: Queryable, options: { tenant?: string } = {}): Book {\n  const r = new Runtime(db, options.tenant ?? \"\");\n  return {\n",
    );
    let sql = |t: &TransferKind, op: &str, n: usize| q(&format!("select * from {}({})", postgres::qualified(book, &postgres::op_function(t, op)), (1..=n).map(|i| format!("${i}")).collect::<Vec<_>>().join(", ")));
    for (t, n) in book.transfers.iter().zip(&names) {
        let all: Vec<usize> = (0..t.params.len()).collect();
        let keys = postgres::key_params(t);
        if t.is_pending() {
            let ap = t.amount_params();
            let key = values(t, &keys, "key");
            let amounts = if ap.is_empty() {
                String::new()
            } else {
                format!(", ...(amounts === undefined ? [{}] : [{}])", vec!["null"; ap.len()].join(", "), values(t, &ap, "amounts"))
            };
            let post_params = if ap.is_empty() { "key" } else { "key, amounts" };
            o.push_str(&format!(
                "    {n}: {{\n      hold: (args) => r.call({}, [r.tenant, {}]),\n      post: ({post_params}) => r.call({}, [r.tenant, {key}{amounts}]),\n      void: (key) => r.call({}, [r.tenant, {key}]),\n      status: (key) => r.status({}, [r.tenant, {key}]),\n    }},\n",
                sql(t, "hold", 1 + all.len()),
                values(t, &all, "args"),
                sql(t, "post", 1 + keys.len() + ap.len()),
                sql(t, "void", 1 + keys.len()),
                q(&postgres::status_sql(book, t)),
            ));
        } else {
            o.push_str(&format!("    {n}: {{ do: (args) => r.call({}, [r.tenant, {}]) }},\n", sql(t, "do", 1 + all.len()), values(t, &all, "args")));
        }
    }
    o.push_str("    balance: {\n");
    for a in &book.accounts {
        let mut args = vec!["r.tenant".to_string()];
        args.extend(a.params.iter().map(|p| format!("str({}, args.{p})", q(p))));
        let sig = if a.params.is_empty() { "()" } else { "(args)" };
        o.push_str(&format!("      {}: {sig} => r.balance({}, [{}]),\n", a.name, q(&postgres::balance_sql(book, a)), args.join(", ")));
    }
    o.push_str(&format!("    }},\n    expire: () => r.expire({}),\n  }};\n}}\n", q(&postgres::expire_sql(book))));
    o.push_str(tail);
    o
}

/// The part of the MySQL client that differs from book to book: the plan of the TigerBeetle
/// client's shape, with the book's SQL in it.
fn mysql_literal(book: &Book) -> String {
    let st = my::statements(book);
    let mut o = format!("const BOOK: BookDef = {{\n  name: {},\n  accounts: {{\n", q(&book.name));
    let bound = |b: &Option<Bound>| b.as_ref().map(|b| format!("{}n", b.value)).unwrap_or_else(|| "null".into());
    for (a, row) in book.accounts.iter().zip(&st.account_rows) {
        let ps: Vec<String> = a.params.iter().map(|x| q(x)).collect();
        o.push_str(&format!(
            "    {}: {{ params: [{}], bounded: {}, lower: {}, upper: {}, row: {} }},\n",
            q(&a.name),
            ps.join(", "),
            my::bounded(a),
            bound(&a.lower),
            bound(&a.upper),
            q(row)
        ));
    }
    o.push_str("  },\n  transfers: {\n");
    let rf = |t: &TransferKind, r: &Ref| {
        let args: Vec<String> = r
            .args
            .iter()
            .map(|a| match a {
                Arg::Param(i) => format!("{{ param: {i} }}"),
                Arg::Lit(s) => format!("{{ literal: {} }}", q(s)),
            })
            .collect();
        let _ = t;
        format!("{{ kind: {}, args: [{}] }}", q(&book.accounts[r.kind].name), args.join(", "))
    };
    for (t, ts) in book.transfers.iter().zip(&st.transfers) {
        let ps: Vec<String> = t.params.iter().map(|x| format!("{{ name: {}, amount: {} }}", q(&x.name), matches!(x.ty, Ty::Amount(_)))).collect();
        let key: Vec<String> = t.key.iter().map(|i| i.to_string()).collect();
        o.push_str(&format!(
            "    {}: {{\n      params: [{}],\n      key: [{}],\n      pending: {},\n      definition: {},\n      moves: [\n",
            q(&t.name),
            ps.join(", "),
            key.join(", "),
            t.is_pending(),
            q(&ids::definition(book, t))
        ));
        for m in &t.moves {
            let amount = match &m.amount {
                Amount::Param(i) => format!("{{ param: {i} }}"),
                Amount::Lit(v) => format!("{{ literal: {v}n }}"),
            };
            o.push_str(&format!("        {{ amount: {amount}, from: {}, to: {} }},\n", rf(t, &m.from), rf(t, &m.to)));
        }
        let opt = |s: &Option<String>| s.as_ref().map(|x| q(x)).unwrap_or_else(|| "null".into());
        let pend = |s: &str| if t.is_pending() { q(s) } else { q("") };
        let pends = |v: &[String]| if t.is_pending() { format!("[{}]", v.iter().map(|s| q(s)).collect::<Vec<_>>().join(", ")) } else { "[]".into() };
        let checks: Vec<String> = ts.checks.iter().map(|(n, r, sql)| format!("{{ n: {n}, reason: {}, sql: {} }}", q(r), q(sql))).collect();
        o.push_str(&format!(
            "      ],\n      sql: {{\n        key: {},\n        lock: {},\n        checks: [{}],\n        refuse: {},\n        entries: {},\n        apply: {},\n        postApply: {},\n        postEntries: {},\n        voidApply: {},\n        voidEntries: {},\n        expireApply: {},\n        expireEntries: {},\n      }},\n    }},\n",
            q(&ts.key_insert),
            opt(&ts.lock),
            checks.join(", "),
            q(&ts.refuse),
            q(&ts.entries),
            opt(&ts.apply),
            pends(&ts.post_apply),
            pend(&ts.post_entries),
            pends(&ts.void_apply),
            pend(&ts.void_entries),
            pends(&ts.expire_apply),
            pend(&ts.expire_entries),
        ));
    }
    o.push_str("  },\n  sql: {\n");
    for (name, sql) in [
        ("begin", &st.begin),
        ("readAccounts", &st.read_accounts),
        ("makeAccounts", &st.make_accounts),
        ("readCall", &st.read_call),
        ("holdLock", &st.hold_lock),
        ("status", &st.status),
        ("balance", &st.balance),
        ("due", &st.due),
        ("foldList", &st.fold_list),
        ("foldLock", &st.fold_lock),
        ("foldRead", &st.fold_read),
        ("foldApply", &st.fold_apply),
        ("foldMark", &st.fold_mark),
    ] {
        o.push_str(&format!("    {name}: {},\n", q(sql)));
    }
    o.push_str("  },\n};\n");
    o
}

pub fn mysql(book: &Book, origin: &Origin, pkg: Option<&InPackage>) -> String {
    let (head, tail) = split(MY_RUNTIME);
    let mut o = super::head(book, origin, Comment::Slashes);
    o.push_str(&format!(
        "// It runs the calls on the tables of `chobo build --target mysql` ({}) through mysql2: MySQL, TiDB or Vitess.\n\n",
        my::TABLES.iter().map(|w| format!("{}_{w}", book.name)).collect::<Vec<_>>().join(", ")
    ));
    o.push_str(head);
    o.push_str(&units_import(book, pkg));
    o.push('\n');
    o.push_str(&reason_type(book));
    o.push_str(&arg_types(book, pkg));
    o.push_str(&book_interface(book, "MySQL, TiDB or Vitess", true));
    let names = members(book);
    o.push_str(
        "/** The book on MySQL, TiDB or Vitess. A tenant is a set of balances of its own; \"\" when not given. Given a pool, each call is a transaction of its own. Given a connection, a call made while it is in a transaction (one the caller began, or any with autocommit off) is part of it, and the caller commits it; else the call is a transaction of its own on that connection. With `batch`, the statements a call sends in a row go in one request, which the pool or connection has to allow (`multipleStatements`). */\nexport function mysql(db: Queryable | PoolLike, options: { tenant?: string; batch?: boolean } = {}): Book {\n  const r = new Runtime(BOOK, db, options.tenant ?? \"\", options.batch ?? false);\n  return {\n",
    );
    for (t, n) in book.transfers.iter().zip(&names) {
        let all: Vec<usize> = (0..t.params.len()).collect();
        let kind = q(&t.name);
        if t.is_pending() {
            let key = values(t, &t.key, "key");
            let ap = t.amount_params();
            let amounts = if ap.is_empty() { "null".to_string() } else { format!("amounts === undefined ? null : [{}]", values(t, &ap, "amounts")) };
            let post_params = if ap.is_empty() { "key" } else { "key, amounts" };
            o.push_str(&format!(
                "    {n}: {{\n      hold: (args) => r.move({kind}, \"hold\", [{}]),\n      post: ({post_params}) => r.end({kind}, \"post\", [{key}], {amounts}),\n      void: (key) => r.end({kind}, \"void\", [{key}], null),\n      status: (key) => r.status({kind}, [{key}]),\n    }},\n",
                values(t, &all, "args")
            ));
        } else {
            o.push_str(&format!("    {n}: {{ do: (args) => r.move({kind}, \"do\", [{}]) }},\n", values(t, &all, "args")));
        }
    }
    o.push_str("    balance: {\n");
    for a in &book.accounts {
        let args: Vec<String> = a.params.iter().map(|p| format!("str({}, args.{p})", q(p))).collect();
        let sig = if a.params.is_empty() { "()" } else { "(args)" };
        o.push_str(&format!("      {}: {sig} => r.balance({}, [{}]),\n", a.name, q(&a.name), args.join(", ")));
    }
    o.push_str("    },\n    expire: () => r.expire(),\n  };\n}\n\n");
    o.push_str(&mysql_literal(book));
    o.push_str(tail);
    o
}
