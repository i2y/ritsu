//! `chobo api`: how to call the book, what each operation can be refused with, and the life
//! of a hold as a state machine, as JSON (DESIGN 5). A tool that reads it, dandori among them,
//! needs nothing but this output.
//!
//! The machine is written with the keys dandori's `src/rulec.rs` reads from rulec's `api`
//! and `certificate` (`machine()`), so that the same reader takes a hold of chobo's.

use crate::check;
use crate::ids;
use crate::model::*;
use serde_json::{Value, json};

fn amount_json(t: &TransferKind, m: &Move) -> Value {
    match &m.amount {
        Amount::Param(i) => json!({"param": t.params[*i].name}),
        Amount::Lit(v) => json!({"literal": *v as i64}),
    }
}

fn ref_json(book: &Book, t: &TransferKind, r: &Ref) -> Value {
    let args: Vec<Value> = r
        .args
        .iter()
        .map(|a| match a {
            Arg::Param(i) => json!({"param": t.params[*i].name}),
            Arg::Lit(s) => json!({"literal": s}),
        })
        .collect();
    json!({"account": book.accounts[r.kind].name, "args": args})
}

fn bound_json(b: &Option<Bound>) -> Value {
    match b {
        Some(b) => json!({"value": b.value as i64, "refused_as": b.refusal}),
        None => Value::Null,
    }
}

/// The states of a hold, in order.
pub const HOLD_STATES: [&str; 4] = ["held", "posted", "voided", "expired"];

/// One row of the table a hold's life is: its number, the events and the states it takes, the
/// state it goes to (None: it stays), and whether the call is refused, and why.
pub struct HoldRow {
    pub row: usize,
    pub events: Vec<usize>,
    pub states: Vec<usize>,
    pub next: Option<&'static str>,
    pub refused: &'static str,
    pub reason: &'static str,
}

/// The events of the holds of `t`, and DESIGN 2.4's table as rows (the rows of an event the
/// transfer does not have left out): what `machine` writes as JSON, and ritsu's port of books
/// hands over (`crate::ports`).
pub fn hold_table(t: &TransferKind) -> (Vec<&'static str>, Vec<HoldRow>) {
    let expires = matches!(t.pending, Some(Expiry::After(_)));
    let events: Vec<&'static str> = if expires { vec!["post", "void", "expire"] } else { vec!["post", "void"] };
    let ev = |e: &str| events.iter().position(|x| *x == e);
    // (event, states, next, refused, reason): DESIGN 2.4's table, row by row
    let table: [(&str, &[usize], Option<&'static str>, &'static str, &'static str); 9] = [
        ("post", &[0], Some("posted"), "false", "none"),
        ("void", &[0], Some("voided"), "false", "none"),
        ("expire", &[0], Some("expired"), "false", "none"),
        ("post", &[1], None, "false", "none"),
        ("void", &[1], None, "true", "already_posted"),
        ("post", &[2], None, "true", "already_voided"),
        ("void", &[2], None, "false", "none"),
        ("post|void", &[3], None, "true", "expired"),
        ("expire", &[1, 2, 3], None, "false", "none"),
    ];
    let mut rows = Vec::new();
    for (event, st, next, refused, reason) in table {
        let evs: Vec<usize> = event.split('|').filter_map(ev).collect();
        if evs.is_empty() {
            continue;
        }
        rows.push(HoldRow { row: rows.len() + 1, events: evs, states: st.to_vec(), next, refused, reason });
    }
    (events, rows)
}

/// The life of the holds of `t` as a state machine, in the shape dandori reads (PLAN B11).
pub fn machine(t: &TransferKind) -> Value {
    let expires = matches!(t.pending, Some(Expiry::After(_)));
    let states = HOLD_STATES;
    let (events, table) = hold_table(t);
    let mut rows = Vec::new();
    let mut moves = Vec::new();
    for r in table {
        let n = r.row;
        rows.push(json!({"row": n, "accepts": [r.events, r.states], "produces": [r.next, r.refused, r.reason]}));
        moves.push(match r.next {
            Some(s) => json!({"row": n, "to": states.iter().position(|x| *x == s).unwrap()}),
            None => json!({"row": n, "stay": true}),
        });
    }
    json!({
        "name": t.name,
        "over": t.name,
        "carry": {"input": "state", "output": "next_state", "enum": "state"},
        "held": [],
        "never": [],
        "once": [],
        "states": states,
        "initial": "held",
        "final": ["posted", "voided", "expired"],
        "events": events,
        "external": if expires { vec!["expire"] } else { vec![] },
        "certificate": {
            "enums": {
                "state": states,
                "event": events,
                "reason": ["none", "already_posted", "already_voided", "expired"],
            },
            "types": {"state": "state", "event": "event", "next_state": "state", "refused": "bool", "reason": "reason"},
            "machine": {
                "table": t.name,
                "carry": {"input": "state", "output": "next_state"},
                "axis": 1,
                "states": states,
                "initial": 0,
                "finals": [1, 2, 3],
                "rows": moves,
            },
            "tables": [{
                "table": t.name,
                "policy": "first",
                "axes": [{"column": "event", "coords": events}, {"column": "state", "coords": states}],
                "decides": ["next_state", "refused", "reason"],
                "rows": rows,
            }],
        },
    })
}

/// How IDs are made (DESIGN 4.3), for a tool that works them out itself.
fn ids_json() -> Value {
    json!({
        "hash": "sha256",
        "parts": format!("each part as the 32-bit big-endian length of its UTF-8, then the UTF-8; the first part is \"{}\"", ids::PREFIX),
        "id": "the first 16 bytes of the hash, as a big-endian 128-bit integer; 0 becomes 1 and 2^128 - 1 becomes 2^128 - 2",
        "ledger": "the first 4 bytes, big-endian; 0 becomes 1",
        "code": "the first 2 bytes, big-endian; 0 becomes 1",
        "roles": {
            "account": ["book", "tenant", "account kind", "the argument values, in the order of the parameters"],
            "floor": ["the account's id, as 32 lowercase hex digits"],
            "room": ["the account's id, as 32 lowercase hex digits"],
            "sink": ["book", "tenant", "unit"],
            "opening": ["the floor or room account's id, as 32 lowercase hex digits"],
            "kind": ["book", "tenant", "account kind"],
            "transfer": ["book", "tenant", "transfer kind", "operation", "the key values, in the order of the key", "the place in the chain, from 0, in decimal"],
            "content": ["book", "tenant", "transfer kind", "operation", "definition", "the content as JSON without spaces"],
            "definition": ["the transfer kind written the one way", "each account kind its moves touch, written the one way"],
            "ledger": ["unit", "scale, in decimal"],
            "code": ["book", "account, transfer or sink", "the kind's name, or the unit's for a sink"],
        },
    })
}

/// How each target names what it writes (DESIGN 4.3, 5): for a tool that calls a book through a
/// client chobo built, or through the SQL, without reading the code. `stem` is the book's file
/// name without `.book`, which the Go package falls back on.
pub fn targets(book: &Book, stem: &str) -> Value {
    use crate::client::{self, go, python};
    use crate::postgres as pg;
    let key_names = |t: &TransferKind, which: &[usize]| which.iter().map(|i| t.params[*i].name.clone()).collect::<Vec<_>>();
    // the SQL
    let mut functions = Vec::new();
    for t in &book.transfers {
        let all: Vec<usize> = (0..t.params.len()).collect();
        let keys = pg::key_params(t);
        let mut ops: Vec<(&str, Vec<usize>, Vec<usize>)> = Vec::new();
        if t.is_pending() {
            ops.push(("hold", all.clone(), vec![]));
            ops.push(("post", keys.clone(), t.amount_params()));
            ops.push(("void", keys.clone(), vec![]));
            ops.push(("status", keys.clone(), vec![]));
        } else {
            ops.push(("do", all.clone(), vec![]));
        }
        let names: Vec<&str> = t.params.iter().map(|p| p.name.as_str()).collect();
        let (tenant, pnames) = pg::param_names(&names);
        for (op, ps, defaults) in ops {
            let mut params = vec![tenant.clone()];
            params.extend(ps.iter().chain(&defaults).map(|i| pnames[*i].clone()));
            let mut f = json!({"kind": t.name, "op": op, "name": pg::op_function(t, op), "params": params});
            if !defaults.is_empty() {
                f["default_null"] = json!(defaults.iter().map(|i| pnames[*i].clone()).collect::<Vec<_>>());
            }
            functions.push(f);
        }
    }
    let balances: Vec<Value> = book
        .accounts
        .iter()
        .map(|a| {
            let names: Vec<&str> = a.params.iter().map(|p| p.as_str()).collect();
            let (tenant, pnames) = pg::param_names(&names);
            let mut params = vec![tenant];
            params.extend(pnames);
            json!({"account": a.name, "name": pg::balance_function(a), "params": params})
        })
        .collect();
    // the clients
    let ts_members = client::typescript::members(book);
    let ts: Vec<Value> = book
        .transfers
        .iter()
        .zip(&ts_members)
        .map(|(t, m)| {
            let mut v = json!({"kind": t.name, "member": m, "args": format!("{}Args", t.name)});
            if t.is_pending() {
                v["key"] = json!(format!("{}Key", t.name));
                if !t.amount_params().is_empty() {
                    v["amounts"] = json!(format!("{}Amounts", t.name));
                }
            }
            v
        })
        .collect();
    let py: Vec<Value> = book
        .transfers
        .iter()
        .zip(python::members(book))
        .map(|(t, m)| {
            let params: serde_json::Map<String, Value> = t.params.iter().map(|p| (p.name.clone(), json!(python::name(&p.name)))).collect();
            json!({"kind": t.name, "member": m, "params": params})
        })
        .collect();
    let gos: Vec<Value> = book
        .transfers
        .iter()
        .zip(go::members(book))
        .map(|(t, m)| {
            let all: Vec<usize> = (0..t.params.len()).collect();
            let fields: serde_json::Map<String, Value> = key_names(t, &all).into_iter().zip(go::fields(t, &all)).map(|(n, f)| (n, json!(f))).collect();
            let g = go::go_name(&t.name);
            let mut v = json!({"kind": t.name, "member": m, "args": format!("{g}Args"), "fields": fields});
            if t.is_pending() {
                v["key"] = json!(format!("{g}Key"));
                if !t.amount_params().is_empty() {
                    v["amounts"] = json!(format!("{g}Amounts"));
                }
            }
            v
        })
        .collect();
    let go_balances: Vec<Value> = book
        .accounts
        .iter()
        .zip(go::balance_methods(book))
        .map(|(a, m)| {
            let mut v = json!({"account": a.name, "method": m});
            if !a.params.is_empty() {
                let fields: serde_json::Map<String, Value> = a.params.iter().cloned().zip(go::account_fields(a)).map(|(n, f)| (n, json!(f))).collect();
                v["args"] = json!(format!("{}Account", go::go_name(&a.name)));
                v["fields"] = Value::Object(fields);
            }
            v
        })
        .collect();
    // the chains
    let mut chains = Vec::new();
    for t in &book.transfers {
        let mut hold = Vec::new();
        for (i, m) in t.moves.iter().enumerate() {
            for role in ids::layout(book, m.from.kind, m.to.kind) {
                hold.push(json!({"move": i + 1, "role": role}));
            }
        }
        let ends: Vec<Value> = hold.iter().filter(|x| x["role"] != "probe" && x["role"] != "probe_void").cloned().collect();
        if t.is_pending() {
            chains.push(json!({"kind": t.name, "op": "hold", "transfers": hold}));
            chains.push(json!({"kind": t.name, "op": "post", "transfers": ends}));
            chains.push(json!({"kind": t.name, "op": "void", "transfers": ends}));
        } else {
            chains.push(json!({"kind": t.name, "op": "do", "transfers": hold}));
        }
    }
    json!({
        "postgres": {"schema": book.name, "functions": functions, "balances": balances, "expire": "expire"},
        "typescript": {"file": format!("{}.ts", book.name), "factories": {"postgres": "postgres", "tigerbeetle": "tigerbeetle"}, "transfers": ts, "balance": "balance"},
        "python": {"module": book.name, "factories": {"postgres": "postgres", "tigerbeetle": "tigerbeetle"}, "transfers": py, "balance": "balance"},
        "go": {"package": go::package(book, stem), "factories": {"postgres": "Postgres", "tigerbeetle": "TigerBeetle"}, "transfers": gos, "balances": go_balances},
        "tigerbeetle": {"request_max": client::REQUEST_MAX, "chains": chains},
    })
}

pub fn api(book: &Book, src: &str, rep: &check::Report, stem: &str) -> Value {
    let units: Vec<Value> = book.units.iter().map(|u| json!({"name": u.name, "scale": u.scale, "ledger": ids::ledger(u)})).collect();
    let accounts: Vec<Value> = book
        .accounts
        .iter()
        .map(|a| {
            json!({
                "name": a.name,
                "params": a.params,
                "unit": book.units[a.unit].name,
                "outside": a.outside,
                "lower": bound_json(&a.lower),
                "upper": bound_json(&a.upper),
                "description": a.description,
                "code": ids::code(&book.name, "account", &a.name),
            })
        })
        .collect();
    let all = check::report_json(book, rep);
    let transfers: Vec<Value> = book
        .transfers
        .iter()
        .map(|t| {
            let params: Vec<Value> = t
                .params
                .iter()
                .map(|p| match p.ty {
                    Ty::Str => json!({"name": p.name, "type": "string"}),
                    Ty::Amount(u) => json!({"name": p.name, "type": "amount", "unit": book.units[u].name}),
                })
                .collect();
            let pending = match t.pending {
                Some(Expiry::After(s)) => json!({"expires_after_seconds": s}),
                Some(Expiry::Never) => json!({"never_expires": true}),
                None => Value::Null,
            };
            let moves: Vec<Value> =
                t.moves.iter().map(|m| json!({"amount": amount_json(t, m), "from": ref_json(book, t, &m.from), "to": ref_json(book, t, &m.to)})).collect();
            let mut ops = serde_json::Map::new();
            for e in all.as_array().into_iter().flatten() {
                if e["kind"] == json!(t.name) {
                    let mut o = json!({"refusals": e["refusals"]});
                    if let Some(k) = e.get("key") {
                        o["key"] = k.clone();
                    }
                    ops.insert(e["op"].as_str().unwrap_or("").to_string(), o);
                }
            }
            json!({
                "name": t.name,
                "description": t.description,
                "params": params,
                "key": t.key.iter().map(|i| t.params[*i].name.clone()).collect::<Vec<_>>(),
                "pending": pending,
                "moves": moves,
                "code": ids::code(&book.name, "transfer", &t.name),
                "definition": ids::definition(book, t),
                "operations": ops,
            })
        })
        .collect();
    let machines: Vec<Value> = book.transfers.iter().filter(|t| t.is_pending()).map(machine).collect();
    json!({
        "v": 1,
        "book": book.name,
        "version": book.version,
        "source_sha256": ids::hex(&ids::sha256(src.as_bytes())),
        "units": units,
        "accounts": accounts,
        "transfers": transfers,
        "machines": machines,
        "ids": ids_json(),
        "targets": targets(book, stem),
    })
}
