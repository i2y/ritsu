//! The TypeScript clients: one file each, written so that Node runs it by stripping its types
//! (no `enum`, no `namespace` with code, no parameter properties).

use super::*;
use crate::postgres;

const TB_RUNTIME: &str = include_str!("runtime/tigerbeetle.ts");
const PG_RUNTIME: &str = include_str!("runtime/postgres.ts");

/// A string as a TypeScript literal.
fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// The members of the book value, one per transfer kind, kept apart from `balance` and `expire`
/// (the PostgreSQL client's), so that the two clients name them alike.
pub fn members(book: &Book) -> Vec<String> {
    unique(book.transfers.iter().map(|t| t.name.clone()).collect(), &["balance", "expire"])
}

fn reason_type(book: &Book) -> String {
    let rs: Vec<String> = reasons(book).iter().map(|r| q(r)).collect();
    format!(
        "/** Why a call of {} can be refused: the bounds' own reasons, then the ones chobo gives itself (DESIGN 2.7). */\nexport type Reason = {};\n\n\
         /** What a call answers. A refusal is an answer, not an error: nothing has changed, and the reason says why. */\nexport type Result = {{ result: \"done\" }} | {{ result: \"done_before\" }} | {{ result: \"refused\"; reason: Reason }};\n\n",
        book.name,
        rs.join(" | ")
    )
}

fn field_list(t: &TransferKind, which: &[usize]) -> String {
    let fs: Vec<String> = which
        .iter()
        .map(|i| {
            let p = &t.params[*i];
            format!("{}: {}", p.name, if matches!(p.ty, Ty::Amount(_)) { "bigint" } else { "string" })
        })
        .collect();
    format!("{{ {} }}", fs.join("; "))
}

/// The types of the arguments: `入荷Args`, `引当Key`, `引当Amounts`, `在庫Account`.
fn arg_types(book: &Book) -> String {
    let mut o = String::new();
    for t in &book.transfers {
        let all: Vec<usize> = (0..t.params.len()).collect();
        let moves: Vec<String> = t.moves.iter().map(|m| format!("{} from {} to {}", book.amount_text(t, m), book.ref_text(t, &m.from), book.ref_text(t, &m.to))).collect();
        let keys: Vec<&str> = t.key.iter().map(|i| t.params[*i].name.as_str()).collect();
        o.push_str(&format!("/** {}: {}; once per {}. */\n", t.name, moves.join(", then "), keys.join(", ")));
        o.push_str(&format!("export type {}Args = {};\n", t.name, field_list(t, &all)));
        if t.is_pending() {
            o.push_str(&format!("export type {}Key = {};\n", t.name, field_list(t, &postgres::key_params(t))));
            let ap = t.amount_params();
            if !ap.is_empty() {
                o.push_str(&format!("export type {}Amounts = {};\n", t.name, field_list(t, &ap)));
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

fn book_interface(book: &Book, pg: bool) -> String {
    let names = members(book);
    let mut o = format!("/** The book {} (v{}), called on {}. */\nexport interface Book {{\n", book.name, book.version, if pg { "PostgreSQL" } else { "TigerBeetle" });
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
    if pg {
        o.push_str("  /** Give back what the holds past their expiry hold; call it from a job (DESIGN 4.1). */\n  expire(): Promise<number>;\n");
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

pub fn tigerbeetle(book: &Book) -> String {
    let (head, tail) = split(TB_RUNTIME);
    let mut o = format!("// {}.\n", banner(book, "tigerbeetle-typescript"));
    o.push_str("// It calls the book on TigerBeetle through tigerbeetle-node 0.17.9 (DESIGN 4.2, 4.3).\n\n");
    o.push_str(head);
    o.push('\n');
    o.push_str(&reason_type(book));
    o.push_str(&arg_types(book));
    o.push_str(&book_interface(book, false));
    let names = members(book);
    o.push_str(&format!(
        "/** The book on TigerBeetle. A tenant is a set of balances of its own (DESIGN 4.3); \"\" when not given. */\nexport function tigerbeetle(client: TigerBeetleClient, options: {{ tenant?: string }} = {{}}): Book {{\n  const r = new Runtime(BOOK, client, options.tenant ?? \"\");\n  return {{\n"
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

pub fn postgres(book: &Book) -> String {
    let (head, tail) = split(PG_RUNTIME);
    let mut o = format!("// {}.\n", banner(book, "postgres-typescript"));
    o.push_str(&format!(
        "// It calls the SQL functions of `chobo build --target postgres` (schema {}) through a connection such as pg's (DESIGN 4.1, 4.3).\n\n",
        postgres::ident(&book.name)
    ));
    o.push_str(head);
    o.push_str(&reason_type(book));
    o.push_str(&arg_types(book));
    o.push_str(&book_interface(book, true));
    let names = members(book);
    o.push_str(
        "/** The book on PostgreSQL. A tenant is a set of balances of its own (DESIGN 4.3); \"\" when not given. Each call is a transaction of its own, unless `db` is in one of the caller's. */\nexport function postgres(db: Queryable, options: { tenant?: string } = {}): Book {\n  const r = new Runtime(db, options.tenant ?? \"\");\n  return {\n",
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
