//! `chobo build --target postgres`: one SQL file that makes a schema for the book, its tables
//! and a function for each transfer kind and operation (DESIGN 4.1, PLAN 0.3 and C2).
//!
//! A function does one call in one transaction, in the order DESIGN 2.3 gives: a move from an
//! account to itself, then the key, then the moves in the order they are written, each checked
//! against what the moves before it left. The rows of the accounts are locked in the order of
//! their IDs, so two calls never wait on each other in a circle.

use crate::diag::{self, DiagExt, Diag};
use ritsu_base::text::Text;
use crate::ids;
use crate::interp::{Call, Op, Val};
use crate::model::*;
use serde_json::{Value, json};

/// PostgreSQL keeps an identifier to 63 bytes, and cuts a longer one short without a word.
pub const NAME_MAX: usize = 63;

/// A name as SQL quotes it (`"在庫"`), and a string as SQL writes it (`'本店'`).
pub use ritsu_emit::lit::{sql as lit, sql_ident as ident};

/// `"在庫"."引当_hold"`.
pub fn qualified(book: &Book, name: &str) -> String {
    format!("{}.{}", ident(&book.name), ident(name))
}

pub fn op_function(t: &TransferKind, op: &str) -> String {
    format!("{}_{}", t.name, op)
}

pub fn balance_function(a: &AccountKind) -> String {
    format!("balance_{}", a.name)
}

/// The names of a function's parameters: the tenant's, then each of `names` with `p_` in
/// front. Every name of the body is a column, `p_…` or `v_…`, so a parameter never reads as a
/// column.
pub fn param_names(names: &[&str]) -> (String, Vec<String>) {
    let ps: Vec<String> = names.iter().map(|n| format!("p_{n}")).collect();
    let mut tenant = "p_tenant".to_string();
    while ps.contains(&tenant) {
        tenant.push('_');
    }
    (tenant, ps)
}

/// The parameters of a `post`, `void` or `status`: the key's, in the order they are declared.
pub fn key_params(t: &TransferKind) -> Vec<usize> {
    (0..t.params.len()).filter(|i| t.key.contains(i)).collect()
}

// ── E061 ──────────────────────────────────────────────────────────────────

fn too_long(name: &str, what: Text, line: usize, col: usize) -> Option<Diag> {
    let n = name.len();
    if n <= NAME_MAX {
        return None;
    }
    let msg = tr!(
        "{{what}} `{name}` は {n} バイトあります。PostgreSQL の名前は 63 バイトまでで、それより長い名前は黙って切り詰められます",
        "{{what}} `{name}` is {n} bytes long; PostgreSQL keeps a name to 63 bytes, and cuts a longer one short without a word"
    )
    .sub("what", &what);
    Some(diag::error("E061", line, col, msg).hint(tr!(
        "名前を短くします。日本語は一字 3 バイトなので、63 バイトは 21 字です（関数の名前は、後ろに付く `_hold` なども含めて数えます）",
        "Make the name shorter: 63 bytes are 63 ASCII letters, or 21 Japanese characters (a function's name counts what follows it too, `_hold` and the like)"
    )))
}

/// The names the SQL would make that PostgreSQL would cut short (E061): the schema (the book's
/// name), each function, and each parameter.
pub fn check_names(book: &Book) -> Vec<Diag> {
    let mut d = Vec::new();
    d.extend(too_long(&book.name, tr!("スキーマの名前（帳簿の名前）", "The schema's name (the book's)"), book.line, book.col));
    for t in &book.transfers {
        let mut ops: Vec<&str> = t.ops().to_vec();
        if t.is_pending() {
            ops.push("status");
        }
        for op in ops {
            d.extend(too_long(&op_function(t, op), tr!("関数の名前", "The function's name"), t.line, t.col));
        }
        for p in &t.params {
            d.extend(too_long(&format!("p_{}", p.name), tr!("関数の引数の名前", "The name of the function's parameter"), p.line, p.col));
        }
    }
    for a in &book.accounts {
        d.extend(too_long(&balance_function(a), tr!("関数の名前", "The function's name"), a.line, a.col));
        for p in &a.params {
            d.extend(too_long(&format!("p_{p}"), tr!("関数の引数の名前", "The name of the function's parameter"), a.line, a.col));
        }
    }
    crate::model::sort(&mut d);
    d.dedup_by(|a, b| a.line == b.line && a.col == b.col && a.message == b.message);
    d
}

// ── what a client sends ───────────────────────────────────────────────────

/// A value as the clients pass it and as `chobo run --show postgres` writes it: a string as it
/// is, an amount as its decimal digits, an amount not given as null.
fn param_json(v: Option<&Val>) -> Value {
    match v {
        Some(Val::Str(s)) => json!(s),
        Some(Val::Amt(a)) => json!(a.to_string()),
        None => Value::Null,
    }
}

fn placeholders(n: usize) -> String {
    (1..=n).map(|i| format!("${i}")).collect::<Vec<_>>().join(", ")
}

/// The SQL a client sends for one call, and its parameters (PLAN 0.3).
pub fn call(book: &Book, tenant: &str, c: &Call) -> (String, Vec<Value>) {
    let t = &book.transfers[c.kind];
    let mut params = vec![json!(tenant)];
    match c.op {
        Op::Do | Op::Hold => params.extend(c.args.iter().map(|v| param_json(v.as_ref()))),
        Op::Post | Op::Void => {
            params.extend(key_params(t).iter().map(|i| param_json(c.args[*i].as_ref())));
            if c.op == Op::Post {
                for i in t.amount_params() {
                    let v = c.amounts.as_ref().and_then(|a| a.get(&i)).map(|a| Val::Amt(*a));
                    params.push(param_json(v.as_ref()));
                }
            }
        }
    }
    let sql = format!("select * from {}({})", qualified(book, &op_function(t, c.op.name())), placeholders(params.len()));
    (sql, params)
}

/// The SQL a client sends to read the state of a hold, and the balance of an account.
pub fn status_sql(book: &Book, t: &TransferKind) -> String {
    format!("select {}({}) as state", qualified(book, &op_function(t, "status")), placeholders(1 + key_params(t).len()))
}

pub fn balance_sql(book: &Book, a: &AccountKind) -> String {
    format!("select * from {}({})", qualified(book, &balance_function(a)), placeholders(1 + a.params.len()))
}

pub fn expire_sql(book: &Book) -> String {
    format!("select {}.expire() as expired", ident(&book.name))
}

// ── the SQL ───────────────────────────────────────────────────────────────

/// The SQL of the book, or the names PostgreSQL would cut short.
pub fn build(book: &Book, origin: &crate::target::Origin) -> Result<String, Vec<Diag>> {
    let d = check_names(book);
    if !d.is_empty() {
        return Err(d);
    }
    let s = ident(&book.name);
    let mut o = crate::client::head(book, origin, crate::client::Comment::Dashes);
    o.push_str(&format!(
        "-- The schema and the functions of the book {} v{} on PostgreSQL.\n\
         -- It can run any number of times: it makes what is missing and replaces the functions.\n\
         -- Each function does one call in one transaction; READ COMMITTED is enough (DESIGN 4.1).\n\n",
        book.name, book.version
    ));
    o.push_str(&format!("create schema if not exists {s};\n\n"));
    o.push_str(&format!(
        "do $chobo$ begin\n  create type {s}.result as (result text, reason text);\nexception when duplicate_object then null;\nend $chobo$;\n\n\
         do $chobo$ begin\n  create type {s}.balance as (posted bigint, held_in bigint, held_out bigint);\nexception when duplicate_object then null;\nend $chobo$;\n\n"
    ));
    o.push_str(&tables(&s));
    o.push_str(&chobo_id(&s));
    for t in &book.transfers {
        if t.is_pending() {
            o.push_str(&move_or_hold(book, t, Op::Hold));
            o.push_str(&post(book, t));
            o.push_str(&void(book, t));
            o.push_str(&status(book, t));
        } else {
            o.push_str(&move_or_hold(book, t, Op::Do));
        }
    }
    for a in &book.accounts {
        o.push_str(&balance(book, a));
    }
    o.push_str(&expire(&s));
    Ok(o)
}

fn tables(s: &str) -> String {
    format!(
        "-- the accounts: one row each, made the first time a call names it. The two checks stop a write\n\
         -- that does not go through the functions; the functions check the bounds themselves.\n\
         create table if not exists {s}.accounts (\n  tenant text not null,\n  id uuid not null,\n  kind text not null,\n  args jsonb not null,\n  unit text not null,\n  lower_bound bigint,\n  upper_bound bigint,\n  posted bigint not null default 0,\n  held_in bigint not null default 0,\n  held_out bigint not null default 0,\n  primary key (tenant, id),\n  constraint within_lower check (lower_bound is null or posted - held_out >= least(lower_bound, 0)),\n  constraint within_upper check (upper_bound is null or posted + held_in <= greatest(upper_bound, 0))\n);\n\n\
         -- the holds: what each move holds, and how the hold ended\n\
         create table if not exists {s}.holds (\n  tenant text not null,\n  id uuid not null,\n  kind text not null,\n  key jsonb not null,\n  moves jsonb not null,\n  state text not null check (state in ('held', 'posted', 'voided', 'expired')),\n  created_at timestamptz not null,\n  deadline timestamptz,\n  posted jsonb,\n  primary key (tenant, id)\n);\n\
         create index if not exists holds_due on {s}.holds (deadline) where state = 'held';\n\n\
         -- the keys: what each call was called with, and what came of it\n\
         create table if not exists {s}.keys (\n  tenant text not null,\n  kind text not null,\n  op text not null,\n  key jsonb not null,\n  content jsonb not null,\n  definition text not null,\n  result text not null,\n  reason text,\n  at timestamptz not null,\n  primary key (tenant, kind, op, key)\n);\n\n\
         -- the entries: every change to an account, so that a balance can be worked out again\n\
         create table if not exists {s}.entries (\n  seq bigserial primary key,\n  tenant text not null,\n  at timestamptz not null,\n  kind text not null,\n  op text not null,\n  key jsonb not null,\n  account uuid not null,\n  d_posted bigint not null,\n  d_held_in bigint not null,\n  d_held_out bigint not null\n);\n\
         create index if not exists entries_account on {s}.entries (tenant, account);\n\n"
    )
}

fn chobo_id(s: &str) -> String {
    format!(
        "-- an ID as chobo and its TigerBeetle clients make it (DESIGN 4.3): the first 16 bytes of a SHA-256\n\
         -- over the parts, each with its UTF-8 length in front\n\
         create or replace function {s}.chobo_id(parts text[]) returns uuid\nlanguage sql immutable strict as $chobo$\n  \
         select case encode(x.h, 'hex')\n           when '00000000000000000000000000000000' then '00000000-0000-0000-0000-000000000001'::uuid\n           when 'ffffffffffffffffffffffffffffffff' then 'ffffffff-ffff-ffff-ffff-fffffffffffe'::uuid\n           else encode(x.h, 'hex')::uuid\n         end\n    \
         from (select substring(sha256(\n            int4send(7) || convert_to('{}', 'UTF8') ||\n            coalesce((select string_agg(int4send(octet_length(convert_to(u.p, 'UTF8'))) || convert_to(u.p, 'UTF8'), ''::bytea order by u.i)\n                        from unnest(parts) with ordinality as u (p, i)), ''::bytea)\n          ) from 1 for 16) as h) as x\n$chobo$;\n\n",
        ids::PREFIX
    )
}

/// What the SQL computes an account's ID from: `"在庫".chobo_id(array['account', '在庫', p_tenant, '在庫', p_sku])`.
fn account_id_sql(book: &Book, t: &TransferKind, r: &Ref, tenant: &str, ps: &[String]) -> String {
    let mut parts = vec![lit("account"), lit(&book.name), tenant.to_string(), lit(&book.accounts[r.kind].name)];
    parts.extend(r.args.iter().map(|a| arg_sql(a, ps)));
    let _ = t;
    format!("{}.chobo_id(array[{}])", ident(&book.name), parts.join(", "))
}

fn arg_sql(a: &Arg, ps: &[String]) -> String {
    match a {
        Arg::Param(i) => ident(&ps[*i]),
        Arg::Lit(s) => format!("{}::text", lit(s)),
    }
}

fn amount_sql(m: &Move, ps: &[String]) -> String {
    match &m.amount {
        Amount::Param(i) => ident(&ps[*i]),
        Amount::Lit(v) => v.to_string(),
    }
}

fn bound_sql(b: &Option<Bound>) -> String {
    match b {
        Some(b) => format!("{}::bigint", b.value),
        None => "null::bigint".into(),
    }
}

/// The ID of a hold: the ID of the first transfer of its chain (DESIGN 4.3).
fn hold_id_sql(book: &Book, t: &TransferKind, tenant: &str, ps: &[String]) -> String {
    let mut parts = vec![lit("transfer"), lit(&book.name), tenant.to_string(), lit(&t.name), lit("hold")];
    parts.extend(t.key.iter().map(|i| ident(&ps[*i])));
    parts.push(lit("0"));
    format!("{}.chobo_id(array[{}])", ident(&book.name), parts.join(", "))
}

fn key_sql(t: &TransferKind, ps: &[String]) -> String {
    format!("jsonb_build_array({})", t.key.iter().map(|i| ident(&ps[*i])).collect::<Vec<_>>().join(", "))
}

fn sql_type(book: &Book, p: &TParam) -> &'static str {
    let _ = book;
    match p.ty {
        Ty::Str => "text",
        Ty::Amount(_) => "bigint",
    }
}

/// The book's own lines for a transfer kind, as comments over its functions.
fn transfer_comment(book: &Book, t: &TransferKind) -> String {
    let mut o = String::new();
    let params: Vec<String> = t
        .params
        .iter()
        .map(|p| match p.ty {
            Ty::Str => format!("{}: string", p.name),
            Ty::Amount(u) => format!("{}: {}", p.name, book.units[u].name),
        })
        .collect();
    o.push_str(&format!("--   transfer {}({})\n", t.name, params.join(", ")));
    o.push_str(&format!("--     key {}\n", t.key.iter().map(|i| t.params[*i].name.as_str()).collect::<Vec<_>>().join(", ")));
    match t.pending {
        Some(Expiry::After(secs)) => o.push_str(&format!("--     pending expires after {}\n", crate::scenario::format_duration(secs))),
        Some(Expiry::Never) => o.push_str("--     pending never expires\n"),
        None => {}
    }
    for m in &t.moves {
        o.push_str(&format!("--     move {} from {} to {}\n", book.amount_text(t, m), book.ref_text(t, &m.from), book.ref_text(t, &m.to)));
    }
    o
}

fn result_sql(s: &str, result: &str, reason: Option<&str>) -> String {
    match reason {
        Some(r) => format!("return ({}, {})::{s}.result;", lit(result), lit(r)),
        None => format!("return ({}, null)::{s}.result;", lit(result)),
    }
}

/// `"引当_hold"` and `"入荷_do"`.
fn move_or_hold(book: &Book, t: &TransferKind, op: Op) -> String {
    let s = ident(&book.name);
    let names: Vec<&str> = t.params.iter().map(|p| p.name.as_str()).collect();
    let (tenant, ps) = param_names(&names);
    let fname = qualified(book, &op_function(t, op.name()));
    let def = ids::definition(book, t);
    let opn = op.name();
    let mut o = String::new();
    o.push_str(&format!("-- {}.{opn}\n", t.name));
    o.push_str(&transfer_comment(book, t));
    let sig: Vec<String> = std::iter::once(format!("{tenant} text")).chain(t.params.iter().zip(&ps).map(|(p, n)| format!("{} {}", ident(n), sql_type(book, p)))).collect();
    o.push_str(&format!("create or replace function {fname}({})\nreturns {s}.result\nlanguage plpgsql as $chobo$\ndeclare\n", sig.join(", ")));
    o.push_str("  v_now timestamptz := clock_timestamp();\n");
    o.push_str(&format!("  v_key jsonb := {};\n", key_sql(t, &ps)));
    let content: Vec<String> = t.params.iter().zip(&ps).map(|(p, n)| format!("{}, {}", lit(&p.name), ident(n))).collect();
    o.push_str(&format!("  v_content jsonb := jsonb_build_object({});\n", content.join(", ")));
    o.push_str(&format!("  v_used {s}.keys;\n"));
    o.push_str("  v_ids uuid[];\n  v_posted bigint[];\n  v_held_in bigint[];\n  v_held_out bigint[];\n  v_from integer;\n  v_to integer;\n");
    for i in 1..=t.moves.len() {
        o.push_str(&format!("  v_from_{i} uuid;\n  v_to_{i} uuid;\n"));
    }
    o.push_str("begin\n");
    // the call's own mistakes
    let all: Vec<String> = std::iter::once(tenant.clone()).chain(ps.iter().map(|n| ident(n))).map(|n| format!("{n} is null")).collect();
    o.push_str(&format!(
        "  -- a mistake in the call is an error, not a refusal\n  if {} then\n    raise exception 'chobo: {}.{opn} takes every argument' using errcode = '22004';\n  end if;\n",
        all.join(" or "),
        t.name.replace('\'', "''")
    ));
    for i in t.amount_params() {
        let n = ident(&ps[i]);
        let pn = t.params[i].name.replace('\'', "''");
        o.push_str(&format!(
            "  if {n} < 0 then\n    raise exception 'chobo: {pn} is %, and an amount is from 0 to 2^63 - 1', {n} using errcode = '22003';\n  end if;\n"
        ));
    }
    for (i, m) in t.moves.iter().enumerate() {
        let k = i + 1;
        o.push_str(&format!("  v_from_{k} := {};\n", account_id_sql(book, t, &m.from, &tenant, &ps)));
        o.push_str(&format!("  v_to_{k} := {};\n", account_id_sql(book, t, &m.to, &tenant, &ps)));
    }
    // (0)
    let same: Vec<String> = (1..=t.moves.len()).map(|k| format!("v_from_{k} = v_to_{k}")).collect();
    o.push_str(&format!(
        "\n  -- (0) a move from an account to itself refuses the call before the key is used\n  if {} then\n    {}\n  end if;\n",
        same.join(" or "),
        result_sql(&s, "refused", Some("same_account"))
    ));
    // (1)
    let kind = lit(&t.name);
    let opl = lit(opn);
    let defl = lit(&def);
    o.push_str(&format!(
        "\n  -- (1) the key. A second call with it waits here for the first to commit, then answers from\n  -- what the first did\n  \
         insert into {s}.keys (tenant, kind, op, key, content, definition, result, at)\n  values ({tenant}, {kind}, {opl}, v_key, v_content, {defl}, 'done', v_now)\n  on conflict do nothing;\n  \
         if not found then\n    select * into v_used from {s}.keys k\n     where k.tenant = {tenant} and k.kind = {kind} and k.op = {opl} and k.key = v_key;\n    \
         if v_used.result = 'refused' then\n      {}\n    elsif v_used.content = v_content and v_used.definition = {defl} then\n      {}\n    else\n      {}\n    end if;\n  end if;\n",
        result_sql(&s, "refused", Some("already_refused")),
        result_sql(&s, "done_before", None),
        result_sql(&s, "refused", Some("key_conflict")),
    ));
    // (2)
    let mut rows = Vec::new();
    let mut bounds = Vec::new();
    for (i, m) in t.moves.iter().enumerate() {
        let k = i + 1;
        for (side, r) in [("from", &m.from), ("to", &m.to)] {
            let a = &book.accounts[r.kind];
            let args = if r.args.is_empty() { "'[]'::jsonb".to_string() } else { format!("jsonb_build_array({})", r.args.iter().map(|x| arg_sql(x, &ps)).collect::<Vec<_>>().join(", ")) };
            rows.push(format!(
                "      (v_{side}_{k}, {}, {args}, {}, {}, {})",
                lit(&a.name),
                lit(&book.units[a.unit].name),
                bound_sql(&a.lower),
                bound_sql(&a.upper)
            ));
            bounds.push(format!("(v_{side}_{k}, {}, {})", bound_sql(&a.lower), bound_sql(&a.upper)));
        }
    }
    let ids: Vec<String> = (1..=t.moves.len()).flat_map(|k| [format!("v_from_{k}"), format!("v_to_{k}")]).collect();
    o.push_str(&format!(
        "\n  -- (2) the accounts: made when they are missing, then locked, in the order of their IDs, and read\n  \
         insert into {s}.accounts (tenant, id, kind, args, unit, lower_bound, upper_bound)\n  \
         select distinct on (x.id) {tenant}, x.id, x.kind, x.args, x.unit, x.lower_bound, x.upper_bound\n    from (values\n{}\n    ) as x (id, kind, args, unit, lower_bound, upper_bound)\n   order by x.id\n  on conflict do nothing;\n  \
         select array_agg(a.id), array_agg(a.posted), array_agg(a.held_in), array_agg(a.held_out)\n    into v_ids, v_posted, v_held_in, v_held_out\n    from (select a.id, a.posted, a.held_in, a.held_out from {s}.accounts a\n           where a.tenant = {tenant} and a.id in ({})\n           order by a.id\n             for update) as a;\n  \
         if exists (select 1 from {s}.accounts a\n               join (values {}) as x (id, lower_bound, upper_bound) on a.id = x.id\n              where a.tenant = {tenant}\n                and (a.lower_bound is distinct from x.lower_bound or a.upper_bound is distinct from x.upper_bound)) then\n    \
         raise exception 'chobo: an account of {}.{opn} has other bounds in the database than in the book' using errcode = 'CB001';\n  end if;\n",
        rows.join(",\n"),
        ids.join(", "),
        bounds.join(", "),
        t.name.replace('\'', "''"),
    ));
    // (3) and (4)
    let refuse = |reason: &str| {
        format!(
            "    -- refused by a bound: nothing has moved, and the key stays used\n    update {s}.keys k set result = 'refused', reason = {0}\n     where k.tenant = {tenant} and k.kind = {kind} and k.op = {opl} and k.key = v_key;\n    {1}\n",
            lit(reason),
            result_sql(&s, "refused", Some(reason))
        )
    };
    o.push_str("\n  -- (3) the moves, in the order they are written, each checked against the locked rows as the\n  -- moves before it left them. Nothing is written until every move has gone through.\n");
    let (held, out_col, in_col) = if op == Op::Hold { (true, "v_held_out", "v_held_in") } else { (false, "v_posted", "v_posted") };
    for (i, m) in t.moves.iter().enumerate() {
        let k = i + 1;
        let amt = amount_sql(m, &ps);
        o.push_str(&format!("  -- move {} from {} to {}\n", book.amount_text(t, m), book.ref_text(t, &m.from), book.ref_text(t, &m.to)));
        o.push_str(&format!("  v_from := array_position(v_ids, v_from_{k});\n  v_to := array_position(v_ids, v_to_{k});\n"));
        if let Some(l) = &book.accounts[m.from.kind].lower {
            o.push_str(&format!("  if v_posted[v_from] - v_held_out[v_from] - {amt} < {} then\n{}  end if;\n", l.value, refuse(&l.refusal)));
        }
        if let Some(u) = &book.accounts[m.to.kind].upper {
            o.push_str(&format!("  if v_posted[v_to] + v_held_in[v_to] + {amt} > {} then\n{}  end if;\n", u.value, refuse(&u.refusal)));
        }
        let (take, put) = if held { ("+", "+") } else { ("-", "+") };
        o.push_str(&format!("  {out_col}[v_from] := {out_col}[v_from] {take} {amt};\n  {in_col}[v_to] := {in_col}[v_to] {put} {amt};\n"));
    }
    o.push_str(&format!(
        "\n  -- (4) every move went through: the rows take what the moves left\n  update {s}.accounts a set posted = x.posted, held_in = x.held_in, held_out = x.held_out\n    from unnest(v_ids, v_posted, v_held_in, v_held_out) as x (id, posted, held_in, held_out)\n   where a.tenant = {tenant} and a.id = x.id;\n"
    ));
    // (5)
    let mut entries = Vec::new();
    for (i, m) in t.moves.iter().enumerate() {
        let k = i + 1;
        let amt = amount_sql(m, &ps);
        if held {
            entries.push(format!("    ({tenant}, v_now, {kind}, {opl}, v_key, v_from_{k}, 0, 0, {amt})"));
            entries.push(format!("    ({tenant}, v_now, {kind}, {opl}, v_key, v_to_{k}, 0, {amt}, 0)"));
        } else {
            entries.push(format!("    ({tenant}, v_now, {kind}, {opl}, v_key, v_from_{k}, -{amt}, 0, 0)"));
            entries.push(format!("    ({tenant}, v_now, {kind}, {opl}, v_key, v_to_{k}, {amt}, 0, 0)"));
        }
    }
    o.push_str(&format!(
        "\n  -- (5) what moved\n  insert into {s}.entries (tenant, at, kind, op, key, account, d_posted, d_held_in, d_held_out) values\n{};\n",
        entries.join(",\n")
    ));
    if held {
        let moves: Vec<String> = t.moves.iter().enumerate().map(|(i, m)| format!("jsonb_build_object('from', v_from_{0}, 'to', v_to_{0}, 'amount', {1})", i + 1, amount_sql(m, &ps))).collect();
        let deadline = match t.pending {
            Some(Expiry::After(secs)) => format!("v_now + interval '{secs} seconds'"),
            _ => "null".into(),
        };
        o.push_str(&format!(
            "  insert into {s}.holds (tenant, id, kind, key, moves, state, created_at, deadline)\n  values ({tenant}, {}, {kind}, v_key,\n          jsonb_build_array({}),\n          'held', v_now, {deadline});\n",
            hold_id_sql(book, t, &tenant, &ps),
            moves.join(", ")
        ));
    }
    o.push_str(&format!("  {}\nend\n$chobo$;\n\n", result_sql(&s, "done", None)));
    o
}

/// The parts every function of a hold starts with: the signature over the key's parameters
/// (and the amounts, for `post`), the key, and the hold, locked.
fn hold_head(book: &Book, t: &TransferKind, op: &str, amounts: bool) -> (String, String, Vec<String>) {
    let s = ident(&book.name);
    let names: Vec<&str> = t.params.iter().map(|p| p.name.as_str()).collect();
    let (tenant, ps) = param_names(&names);
    let fname = qualified(book, &op_function(t, op));
    let mut sig = vec![format!("{tenant} text")];
    for i in key_params(t) {
        sig.push(format!("{} text", ident(&ps[i])));
    }
    if amounts {
        for i in t.amount_params() {
            sig.push(format!("{} bigint default null", ident(&ps[i])));
        }
    }
    let mut o = String::new();
    o.push_str(&format!("-- {}.{op}\n", t.name));
    o.push_str(&transfer_comment(book, t));
    o.push_str(&format!("create or replace function {fname}({})\nreturns {s}.result\nlanguage plpgsql as $chobo$\ndeclare\n", sig.join(", ")));
    o.push_str("  v_now timestamptz := clock_timestamp();\n");
    o.push_str(&format!("  v_key jsonb := {};\n", key_sql(t, &ps)));
    o.push_str(&format!("  v_hold {s}.holds;\n  v_used {s}.keys;\n  v_m jsonb;\n  v_i integer;\n"));
    (o, tenant, ps)
}

fn lock_hold_accounts(s: &str, tenant: &str) -> String {
    format!(
        "  -- the accounts of the hold, locked in the order of their IDs\n  perform 1 from {s}.accounts a\n   where a.tenant = {tenant}\n     and a.id in (select (m ->> e.side)::uuid from jsonb_array_elements(v_hold.moves) as m, (values ('from'), ('to')) as e (side))\n   order by a.id\n     for update;\n"
    )
}

fn post(book: &Book, t: &TransferKind) -> String {
    let s = ident(&book.name);
    let (mut o, tenant, ps) = hold_head(book, t, "post", true);
    let def = lit(&ids::definition(book, t));
    let kind = lit(&t.name);
    o.push_str("  v_posted jsonb;\n  v_held bigint;\n  v_post bigint;\nbegin\n");
    let mut nulls: Vec<String> = std::iter::once(tenant.clone()).chain(key_params(t).iter().map(|i| ident(&ps[*i]))).map(|n| format!("{n} is null")).collect();
    nulls.dedup();
    o.push_str(&format!(
        "  -- a mistake in the call is an error, not a refusal\n  if {} then\n    raise exception 'chobo: {}.post takes every argument of the key' using errcode = '22004';\n  end if;\n",
        nulls.join(" or "),
        t.name.replace('\'', "''")
    ));
    let ap = t.amount_params();
    if !ap.is_empty() {
        let given: Vec<String> = ap.iter().map(|i| format!("{} is null", ident(&ps[*i]))).collect();
        let all_null = given.join(" and ");
        let any_null = given.join(" or ");
        o.push_str(&format!(
            "  if ({any_null}) and not ({all_null}) then\n    raise exception 'chobo: {}.post takes every amount or none' using errcode = '22004';\n  end if;\n",
            t.name.replace('\'', "''")
        ));
        for i in &ap {
            let n = ident(&ps[*i]);
            let pn = t.params[*i].name.replace('\'', "''");
            o.push_str(&format!("  if {n} < 0 then\n    raise exception 'chobo: {pn} is %, and an amount is from 0 to 2^63 - 1', {n} using errcode = '22003';\n  end if;\n"));
        }
    }
    o.push_str(&format!(
        "\n  select * into v_hold from {s}.holds h\n   where h.tenant = {tenant} and h.id = {}\n     for update;\n  if not found then\n    {}\n  end if;\n",
        hold_id_sql(book, t, &tenant, &ps),
        result_sql(&s, "refused", Some("no_such_hold"))
    ));
    // what is posted
    let full = "(select jsonb_agg(m -> 'amount' order by i) from jsonb_array_elements(v_hold.moves) with ordinality as e (m, i))";
    if ap.is_empty() {
        o.push_str(&format!("\n  -- what is posted, move by move: all that is held\n  v_posted := {full};\n"));
    } else {
        let parts: Vec<String> = t.moves.iter().map(|m| amount_sql(m, &ps)).collect();
        o.push_str(&format!(
            "\n  -- what is posted, move by move: all that is held, or what the amounts make of each move\n  if {} is null then\n    v_posted := {full};\n  else\n    v_posted := jsonb_build_array({});\n  end if;\n",
            ident(&ps[ap[0]]),
            parts.join(", ")
        ));
    }
    o.push_str(&format!(
        "\n  if v_hold.state = 'posted' then\n    select * into v_used from {s}.keys k\n     where k.tenant = {tenant} and k.kind = {kind} and k.op = 'post' and k.key = v_key;\n    \
         if v_used.content = v_posted and v_used.definition = {def} then\n      {}\n    end if;\n    {}\n  elsif v_hold.state = 'voided' then\n    {}\n  \
         elsif v_hold.state = 'expired' or v_now >= v_hold.deadline then\n    {}\n  end if;\n",
        result_sql(&s, "done_before", None),
        result_sql(&s, "refused", Some("key_conflict")),
        result_sql(&s, "refused", Some("already_voided")),
        result_sql(&s, "refused", Some("expired")),
    ));
    o.push_str(&format!(
        "  for v_i in 0 .. jsonb_array_length(v_hold.moves) - 1 loop\n    if (v_posted ->> v_i)::bigint > (v_hold.moves -> v_i ->> 'amount')::bigint then\n      {}\n    end if;\n  end loop;\n\n",
        result_sql(&s, "refused", Some("over_hold"))
    ));
    o.push_str(&lock_hold_accounts(&s, &tenant));
    o.push_str(&format!(
        "  -- move by move: what was held comes off, what is posted goes on, and the rest goes back\n  for v_i in 0 .. jsonb_array_length(v_hold.moves) - 1 loop\n    v_m := v_hold.moves -> v_i;\n    v_held := (v_m ->> 'amount')::bigint;\n    v_post := (v_posted ->> v_i)::bigint;\n    \
         update {s}.accounts a set held_out = a.held_out - v_held, posted = a.posted - v_post\n     where a.tenant = {tenant} and a.id = (v_m ->> 'from')::uuid;\n    \
         update {s}.accounts a set held_in = a.held_in - v_held, posted = a.posted + v_post\n     where a.tenant = {tenant} and a.id = (v_m ->> 'to')::uuid;\n    \
         insert into {s}.entries (tenant, at, kind, op, key, account, d_posted, d_held_in, d_held_out) values\n      ({tenant}, v_now, {kind}, 'post', v_key, (v_m ->> 'from')::uuid, -v_post, 0, -v_held),\n      ({tenant}, v_now, {kind}, 'post', v_key, (v_m ->> 'to')::uuid, v_post, -v_held, 0);\n  end loop;\n  \
         update {s}.holds h set state = 'posted', posted = v_posted where h.tenant = {tenant} and h.id = v_hold.id;\n  \
         insert into {s}.keys (tenant, kind, op, key, content, definition, result, at)\n  values ({tenant}, {kind}, 'post', v_key, v_posted, {def}, 'done', v_now)\n  on conflict do nothing;\n  {}\nend\n$chobo$;\n\n",
        result_sql(&s, "done", None)
    ));
    o
}

fn void(book: &Book, t: &TransferKind) -> String {
    let s = ident(&book.name);
    let (mut o, tenant, ps) = hold_head(book, t, "void", false);
    let def = lit(&ids::definition(book, t));
    let kind = lit(&t.name);
    o.push_str("  v_held bigint;\nbegin\n");
    let nulls: Vec<String> = std::iter::once(tenant.clone()).chain(key_params(t).iter().map(|i| ident(&ps[*i]))).map(|n| format!("{n} is null")).collect();
    o.push_str(&format!(
        "  -- a mistake in the call is an error, not a refusal\n  if {} then\n    raise exception 'chobo: {}.void takes every argument of the key' using errcode = '22004';\n  end if;\n",
        nulls.join(" or "),
        t.name.replace('\'', "''")
    ));
    o.push_str(&format!(
        "\n  select * into v_hold from {s}.holds h\n   where h.tenant = {tenant} and h.id = {}\n     for update;\n  if not found then\n    {}\n  end if;\n",
        hold_id_sql(book, t, &tenant, &ps),
        result_sql(&s, "refused", Some("no_such_hold"))
    ));
    o.push_str(&format!(
        "\n  if v_hold.state = 'posted' then\n    {}\n  elsif v_hold.state = 'voided' then\n    select * into v_used from {s}.keys k\n     where k.tenant = {tenant} and k.kind = {kind} and k.op = 'void' and k.key = v_key;\n    \
         if v_used.definition = {def} then\n      {}\n    end if;\n    {}\n  elsif v_hold.state = 'expired' or v_now >= v_hold.deadline then\n    {}\n  end if;\n\n",
        result_sql(&s, "refused", Some("already_posted")),
        result_sql(&s, "done_before", None),
        result_sql(&s, "refused", Some("key_conflict")),
        result_sql(&s, "refused", Some("expired")),
    ));
    o.push_str(&lock_hold_accounts(&s, &tenant));
    o.push_str(&format!(
        "  -- move by move: what was held goes back\n  for v_i in 0 .. jsonb_array_length(v_hold.moves) - 1 loop\n    v_m := v_hold.moves -> v_i;\n    v_held := (v_m ->> 'amount')::bigint;\n    \
         update {s}.accounts a set held_out = a.held_out - v_held\n     where a.tenant = {tenant} and a.id = (v_m ->> 'from')::uuid;\n    \
         update {s}.accounts a set held_in = a.held_in - v_held\n     where a.tenant = {tenant} and a.id = (v_m ->> 'to')::uuid;\n    \
         insert into {s}.entries (tenant, at, kind, op, key, account, d_posted, d_held_in, d_held_out) values\n      ({tenant}, v_now, {kind}, 'void', v_key, (v_m ->> 'from')::uuid, 0, 0, -v_held),\n      ({tenant}, v_now, {kind}, 'void', v_key, (v_m ->> 'to')::uuid, 0, -v_held, 0);\n  end loop;\n  \
         update {s}.holds h set state = 'voided' where h.tenant = {tenant} and h.id = v_hold.id;\n  \
         insert into {s}.keys (tenant, kind, op, key, content, definition, result, at)\n  values ({tenant}, {kind}, 'void', v_key, 'null'::jsonb, {def}, 'done', v_now)\n  on conflict do nothing;\n  {}\nend\n$chobo$;\n\n",
        result_sql(&s, "done", None)
    ));
    o
}

fn status(book: &Book, t: &TransferKind) -> String {
    let s = ident(&book.name);
    let names: Vec<&str> = t.params.iter().map(|p| p.name.as_str()).collect();
    let (tenant, ps) = param_names(&names);
    let mut sig = vec![format!("{tenant} text")];
    for i in key_params(t) {
        sig.push(format!("{} text", ident(&ps[i])));
    }
    format!(
        "-- {0}.status: held, posted, voided or expired; null when there is no such hold. A hold past its\n\
         -- expiry is expired here even before expire() gives back what it held.\n\
         create or replace function {1}({2})\nreturns text\nlanguage sql as $chobo$\n  \
         select case when h.state = 'held' and h.deadline <= clock_timestamp() then 'expired' else h.state end\n    from {s}.holds h\n   where h.tenant = {tenant} and h.id = {3}\n$chobo$;\n\n",
        t.name,
        qualified(book, &op_function(t, "status")),
        sig.join(", "),
        hold_id_sql(book, t, &tenant, &ps),
    )
}

fn balance(book: &Book, a: &AccountKind) -> String {
    let s = ident(&book.name);
    let names: Vec<&str> = a.params.iter().map(|p| p.as_str()).collect();
    let (tenant, ps) = param_names(&names);
    let mut sig = vec![format!("{tenant} text")];
    sig.extend(ps.iter().map(|n| format!("{} text", ident(n))));
    let mut parts = vec![lit("account"), lit(&book.name), tenant.clone(), lit(&a.name)];
    parts.extend(ps.iter().map(|n| ident(n)));
    format!(
        "-- the balance of {0}: posted, held in and held out; 0 for an account no call has named\n\
         create or replace function {1}({2})\nreturns {s}.balance\nlanguage sql as $chobo$\n  \
         select coalesce(\n    (select row(a.posted, a.held_in, a.held_out)::{s}.balance\n       from {s}.accounts a\n      where a.tenant = {tenant} and a.id = {s}.chobo_id(array[{3}])),\n    row(0, 0, 0)::{s}.balance)\n$chobo$;\n\n",
        a.name,
        qualified(book, &balance_function(a)),
        sig.join(", "),
        parts.join(", "),
    )
}

fn expire(s: &str) -> String {
    format!(
        "-- expire(): gives back what the holds past their expiry held, and marks them expired. PostgreSQL\n\
         -- has no clock of its own: call it from pg_cron or a job of the caller's. It takes at most `p_max`\n\
         -- holds, skips the ones a call is working on, and answers how many it ended.\n\
         create or replace function {s}.expire(p_max integer default 1000)\nreturns integer\nlanguage plpgsql as $chobo$\ndeclare\n  v_now timestamptz := clock_timestamp();\n  v_due {s}.holds[];\n  v_hold {s}.holds;\n  v_m jsonb;\n  v_held bigint;\nbegin\n  \
         select array_agg(h order by h.tenant, h.id) into v_due\n    from (select * from {s}.holds h\n           where h.state = 'held' and h.deadline <= v_now\n           order by h.deadline, h.tenant, h.id\n           limit p_max\n             for update skip locked) as h;\n  \
         if v_due is null then\n    return 0;\n  end if;\n  \
         -- their accounts, locked in the order of their IDs, as every call locks them\n  perform 1 from {s}.accounts a\n   where (a.tenant, a.id) in (select h.tenant, (m ->> e.side)::uuid\n                                from unnest(v_due) as h, jsonb_array_elements(h.moves) as m, (values ('from'), ('to')) as e (side))\n   order by a.tenant, a.id\n     for update;\n  \
         foreach v_hold in array v_due loop\n    for v_m in select x from jsonb_array_elements(v_hold.moves) as x loop\n      v_held := (v_m ->> 'amount')::bigint;\n      \
         update {s}.accounts a set held_out = a.held_out - v_held\n       where a.tenant = v_hold.tenant and a.id = (v_m ->> 'from')::uuid;\n      \
         update {s}.accounts a set held_in = a.held_in - v_held\n       where a.tenant = v_hold.tenant and a.id = (v_m ->> 'to')::uuid;\n      \
         insert into {s}.entries (tenant, at, kind, op, key, account, d_posted, d_held_in, d_held_out) values\n        (v_hold.tenant, v_now, v_hold.kind, 'expire', v_hold.key, (v_m ->> 'from')::uuid, 0, 0, -v_held),\n        (v_hold.tenant, v_now, v_hold.kind, 'expire', v_hold.key, (v_m ->> 'to')::uuid, 0, -v_held, 0);\n    end loop;\n    \
         update {s}.holds h set state = 'expired' where h.tenant = v_hold.tenant and h.id = v_hold.id;\n  end loop;\n  return cardinality(v_due);\nend\n$chobo$;\n"
    )
}
