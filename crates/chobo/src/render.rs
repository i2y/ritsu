//! `chobo run --show postgres|tigerbeetle`: what a client sends for each operation of a
//! scenario (PLAN C1). The tests compare what the clients of the three languages really send
//! with this, field by field.
//!
//! A `post` or `void` depends on the hold as the client reads it first; an operation of a
//! `together` is shown from the state before the `together`, as `ids::scenario_chains` does.

use ritsu_base::text::Lang;
use crate::ids::{chain, chain_json, hold_chain_ids, id_hex};
use crate::interp::{Call, Op, State};
use crate::model::*;
use crate::postgres;
use crate::scenario::{self, Scenario, Step, pad, width};
use serde_json::{Value, json};

/// Each operation of the scenario, with the step (from 1) and the caller (from 1, in a
/// `together`), and what `sent(state, call)` makes of it.
fn each(book: &Book, s: &Scenario, mut sent: impl FnMut(&State, &Call) -> Value) -> Vec<(Call, Value)> {
    let mut state = State::default();
    let mut out = Vec::new();
    let head = |c: &Call, step: usize, caller: Option<usize>| {
        let mut v = json!({"step": step});
        if let Some(n) = caller {
            v["caller"] = json!(n);
        }
        v["op"] = json!(c.op.name());
        v["kind"] = json!(book.transfers[c.kind].name);
        v
    };
    for (i, st) in s.steps.iter().enumerate() {
        match st {
            Step::Call(c) => {
                let mut v = head(c, i + 1, None);
                merge(&mut v, sent(&state, c));
                out.push((c.clone(), v));
                let _ = state.apply(book, c);
            }
            Step::Pass(secs, _) => {
                state.pass(*secs);
            }
            Step::Together(cs) => {
                let before = state.clone();
                for (ci, ops) in cs.iter().enumerate() {
                    for c in ops {
                        let mut v = head(c, i + 1, Some(ci + 1));
                        merge(&mut v, sent(&before, c));
                        out.push((c.clone(), v));
                    }
                }
                for c in cs.iter().flatten() {
                    let _ = state.apply(book, c);
                }
            }
        }
    }
    out
}

fn merge(v: &mut Value, more: Value) {
    if let (Some(o), Value::Object(m)) = (v.as_object_mut(), more) {
        o.extend(m);
    }
}

/// What a PostgreSQL client sends: the SQL and its parameters (strings as they are, amounts as
/// their decimal digits, an amount not given as null).
pub fn postgres_json(book: &Book, tenant: &str, s: &Scenario) -> Value {
    Value::Array(postgres_ops(book, tenant, s).into_iter().map(|(_, v)| v).collect())
}

/// The probe a TigerBeetle client sends to read where a hold is (DESIGN 4.2): a void of the
/// hold and a transfer that cannot go through, linked. Its IDs are new each time.
pub fn state_probe(pending: u128) -> Value {
    let z = id_hex(0);
    json!({"accounts": [], "openings": [], "transfers": [
        {"role": "state_void", "move": 0, "id": "random", "debit_account_id": z, "credit_account_id": z, "amount": "0", "pending_id": id_hex(pending),
         "user_data_128": z, "user_data_64": 0, "user_data_32": 0, "timeout": 0, "ledger": 0, "code": 0, "flags": ["linked", "void_pending_transfer"]},
        {"role": "state_check", "move": 0, "id": "random", "debit_account_id": id_hex(1), "credit_account_id": id_hex(1), "amount": "0", "pending_id": z,
         "user_data_128": z, "user_data_64": 0, "user_data_32": 0, "timeout": 0, "ledger": 1, "code": 1, "flags": []},
    ]})
}

/// What a TigerBeetle client sends for one call, from the state it finds: for a `post` or
/// `void`, the hold's transfers it reads first (`lookup`); then the accounts and openings it
/// makes sure of and the chain (`sent`), or null when it sends nothing.
pub fn tigerbeetle_one(book: &Book, tenant: &str, state: &State, c: &Call) -> Value {
    let t = &book.transfers[c.kind];
    match c.op {
        Op::Do | Op::Hold => json!({"sent": chain(book, tenant, c, None).map(|ch| chain_json(&ch)).unwrap_or(Value::Null)}),
        Op::Post | Op::Void => {
            let key = c.key(t);
            let held = hold_chain_ids(book, tenant, t, &key);
            let mains: Vec<u128> = held.iter().filter(|(r, _, _)| *r == "main").map(|(_, _, id)| *id).collect();
            let lookup: Vec<String> = mains.iter().map(|id| id_hex(*id)).collect();
            let Some(hold) = state.holds.get(&(c.kind, key)) else {
                return json!({"lookup": lookup, "sent": Value::Null});
            };
            if c.op == Op::Post && c.amounts.is_some() {
                let posted = State::post_amounts(book, c, hold);
                if posted.iter().zip(&hold.moves).any(|(p, m)| *p > m.amount) {
                    return json!({"lookup": lookup, "sent": state_probe(mains[0])});
                }
            }
            json!({"lookup": lookup, "sent": chain(book, tenant, c, Some(hold)).map(|ch| chain_json(&ch)).unwrap_or(Value::Null)})
        }
    }
}

pub fn tigerbeetle_json(book: &Book, tenant: &str, s: &Scenario) -> Value {
    Value::Array(tigerbeetle_ops(book, tenant, s).into_iter().map(|(_, v)| v).collect())
}

fn postgres_ops(book: &Book, tenant: &str, s: &Scenario) -> Vec<(Call, Value)> {
    each(book, s, |_, c| {
        let (sql, params) = postgres::call(book, tenant, c);
        json!({"sql": sql, "params": params})
    })
}

fn tigerbeetle_ops(book: &Book, tenant: &str, s: &Scenario) -> Vec<(Call, Value)> {
    each(book, s, |state, c| tigerbeetle_one(book, tenant, state, c))
}

// ── as a person reads it ──────────────────────────────────────────────────

/// A value as SQL writes it, for the text of `--show postgres`.
fn sql_value(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::String(s) if s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty() => s.clone(),
        Value::String(s) => postgres::lit(s),
        other => other.to_string(),
    }
}

fn op_line(book: &Book, c: &Call, v: &Value) -> String {
    let step = v["step"].as_u64().unwrap_or(0);
    match v["caller"].as_u64() {
        Some(n) => format!("{step:>3}  together, caller {n}: {}", scenario::call_text(book, c)),
        None => format!("{step:>3}  {}", scenario::call_text(book, c)),
    }
}

/// `--show postgres` as text: each operation and the SQL it runs, with its values in place.
pub fn postgres_text(book: &Book, file: &str, tenant: &str, s: &Scenario, lang: Lang) -> String {
    let mut o = header(file, s, lang, "PostgreSQL", tenant);
    for (c, v) in postgres_ops(book, tenant, s) {
        o.push_str(&op_line(book, &c, &v));
        o.push('\n');
        let mut sql = v["sql"].as_str().unwrap_or("").to_string();
        let params = v["params"].as_array().cloned().unwrap_or_default();
        for (i, p) in params.iter().enumerate().rev() {
            sql = sql.replace(&format!("${}", i + 1), &sql_value(p));
        }
        o.push_str(&format!("       {sql}\n"));
    }
    o
}

fn header(file: &str, s: &Scenario, lang: Lang, target: &str, tenant: &str) -> String {
    let name = if s.name.is_empty() { String::new() } else { format!(" ({})", s.name) };
    let n = s.steps.len();
    let t = crate::ids::json_string(tenant);
    format!("{file}{name}: {}\n", tr!("{n} ステップ、{target} に送るもの（テナント {t}）", "{n} steps, as sent to {target} (tenant {t})").get(lang))
}

/// `--show tigerbeetle` as text: for each operation, the accounts and openings it makes sure
/// of and its chain, transfer by transfer, with the accounts named as the book names them.
/// Rows as columns, each as wide as its widest cell (counted as a terminal shows it).
fn table(rows: &[Vec<String>], indent: &str) -> String {
    let n = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let widths: Vec<usize> = (0..n).map(|c| rows.iter().map(|r| r.get(c).map(|x| width(x)).unwrap_or(0)).max().unwrap_or(0)).collect();
    let mut o = String::new();
    for r in rows {
        let cells: Vec<String> = r.iter().enumerate().map(|(c, x)| pad(x, widths[c])).collect();
        o.push_str(&format!("{indent}{}\n", cells.join("  ").trim_end()));
    }
    o
}

pub fn tigerbeetle_text(book: &Book, file: &str, tenant: &str, s: &Scenario, lang: Lang) -> String {
    let ops = tigerbeetle_ops(book, tenant, s);
    let mut o = header(file, s, lang, "TigerBeetle", tenant);
    // what each ID is, as a person reads it
    let mut names: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for (_, v) in &ops {
        for a in v["sent"]["accounts"].as_array().into_iter().flatten() {
            let what = match a["role"].as_str() {
                Some("account") => a["of"].as_str().unwrap_or("").to_string(),
                Some(r) => format!("{r} {}", a["of"].as_str().unwrap_or("")),
                None => String::new(),
            };
            names.insert(a["id"].as_str().unwrap_or("").to_string(), what);
        }
    }
    let name = |id: &Value| names.get(id.as_str().unwrap_or("")).cloned().unwrap_or_else(|| id.as_str().unwrap_or("").to_string());
    for (c, v) in &ops {
        o.push_str(&op_line(book, c, v));
        o.push('\n');
        if let Some(l) = v["lookup"].as_array() {
            let ids: Vec<&str> = l.iter().filter_map(|x| x.as_str()).collect();
            o.push_str(&format!("       {} {}\n", tr!("先に読む仮押さえ:", "reads the hold first:").get(lang), ids.join(", ")));
        }
        let sent = &v["sent"];
        if sent.is_null() {
            o.push_str(&format!("       {}\n", tr!("何も送らない", "sends nothing").get(lang)));
            continue;
        }
        let accounts: Vec<Vec<String>> = sent["accounts"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|a| {
                let flags: Vec<&str> = a["flags"].as_array().into_iter().flatten().filter_map(|f| f.as_str()).collect();
                vec![a["role"].as_str().unwrap_or("").into(), a["of"].as_str().unwrap_or("").into(), a["id"].as_str().unwrap_or("").into(), flags.join(" ")]
            })
            .collect();
        let openings: Vec<Vec<String>> = sent["openings"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| vec![format!("{} → {}", name(&x["debit_account_id"]), name(&x["credit_account_id"])), x["amount"].as_str().unwrap_or("").into(), x["id"].as_str().unwrap_or("").into()])
            .collect();
        let transfers: Vec<Vec<String>> = sent["transfers"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(i, x)| {
                let what = if x["pending_id"].as_str().is_some_and(|p| p != id_hex(0)) {
                    format!("pending {}", x["pending_id"].as_str().unwrap_or(""))
                } else {
                    format!("{} → {}", name(&x["debit_account_id"]), name(&x["credit_account_id"]))
                };
                let mut flags: Vec<String> = x["flags"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(String::from)).collect();
                if x["timeout"].as_u64().is_some_and(|t| t > 0) {
                    flags.push(format!("timeout {}", x["timeout"]));
                }
                vec![i.to_string(), x["role"].as_str().unwrap_or("").into(), what, x["amount"].as_str().unwrap_or("").into(), x["id"].as_str().unwrap_or("").into(), flags.join(" ")]
            })
            .collect();
        for (title, rows) in [(tr!("勘定:", "accounts:"), accounts), (tr!("開始の振替:", "openings:"), openings), (tr!("チェーン:", "chain:"), transfers)] {
            if rows.is_empty() {
                continue;
            }
            o.push_str(&format!("       {}\n", title.get(lang)));
            o.push_str(&table(&rows, "         "));
        }
    }
    o
}
