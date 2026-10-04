//! ritsu's ports, as chobo answers them (ritsu's DESIGN 3.2): [`Engine`] implements the port of
//! books (`ritsu_ports::Books`) — the units, accounts and transfers of a book, the life of a hold
//! as a state machine, and a ledger run by the reference interpreter — and gives what a `.book`
//! holds (`Items`).

use crate::check::{self, Checked};
use crate::interp::{AccountId, Call, Op, Outcome, State, Val};
use crate::model::*;
use ritsu_base::naming::{Name as Naming, Tool};
use ritsu_base::text::Text;
use ritsu_ports::{Account, Balance, BookCall, BookFacts, BookOutcome, BookUnit, Bound as PBound, Found, Item, Ledger, Machine, MachineRow, Move as PMove, MoveAmount, MoveRef, Said, Transfer, TransferParam};
use std::path::Path;

/// chobo, as the ports reach it.
#[derive(Default)]
pub struct Engine;

/// The book, checked as `chobo check` checks it; else what check says.
fn checked(file: &Path) -> Result<(String, Book, Checked), Vec<Said>> {
    let path = file.to_string_lossy().to_string();
    let (src, c) = check::check_file(file).map_err(|e| vec![Said::unreadable(&path, &e)])?;
    let errors: Vec<Said> = c.diags.iter().filter(|d| d.is_error()).map(|d| Said { file: path.clone(), ..Said::of(d) }).collect();
    match &c.book {
        Some(b) if errors.is_empty() => Ok((src, b.clone(), c)),
        _ => Err(errors),
    }
}

/// The life of a hold of `t`, as a machine (`api`'s `machines`).
fn hold_machine(t: &TransferKind) -> Machine {
    let states: Vec<String> = crate::api::HOLD_STATES.iter().map(|s| s.to_string()).collect();
    let (events, rows) = crate::api::hold_table(t);
    Machine {
        name: t.name.clone(),
        carry_in: "state".into(),
        carry_out: "next_state".into(),
        state_enum: "state".into(),
        initial: 0,
        finals: vec![1, 2, 3],
        held: vec![],
        policy: "first".into(),
        axes: vec![ritsu_ports::Axis { column: "event".into(), coords: events.iter().map(|e| e.to_string()).collect() }, ritsu_ports::Axis { column: "state".into(), coords: states.clone() }],
        state_axis: Some(1),
        decides: vec!["next_state".into(), "refused".into(), "reason".into()],
        rows: rows
            .iter()
            .map(|r| MachineRow {
                row: r.row,
                accepts: vec![r.events.clone(), r.states.clone()],
                to: r.next.and_then(|n| states.iter().position(|s| s == n)),
                produces: vec![r.next.map(String::from), Some(r.refused.to_string()), Some(r.reason.to_string())],
            })
            .collect(),
        states,
    }
}

impl ritsu_ports::Books for Engine {
    fn facts(&self, file: &Path) -> Result<BookFacts, Vec<Said>> {
        let (src, book, c) = checked(file)?;
        let report = c.report.as_ref();
        let bound = |b: &Option<Bound>| b.as_ref().map(|b| PBound { value: b.value, refusal: b.refusal.clone() });
        Ok(BookFacts {
            name: book.name.clone(),
            version: book.version,
            sha256: crate::ids::hex(&crate::ids::sha256(src.as_bytes())),
            units: book.units.iter().map(|u| BookUnit { name: u.name.clone(), scale: u.scale, unit: u.ty.clone() }).collect(),
            accounts: book
                .accounts
                .iter()
                .map(|a| Account { name: a.name.clone(), params: a.params.clone(), unit: book.units[a.unit].name.clone(), outside: a.outside, lower: bound(&a.lower), upper: bound(&a.upper) })
                .collect(),
            transfers: book
                .transfers
                .iter()
                .enumerate()
                .map(|(k, t)| Transfer {
                    name: t.name.clone(),
                    params: t
                        .params
                        .iter()
                        .map(|p| TransferParam {
                            name: p.name.clone(),
                            unit: match p.ty {
                                Ty::Str => None,
                                Ty::Amount(u) => Some(book.units[u].name.clone()),
                            },
                        })
                        .collect(),
                    key: t.key.iter().map(|i| t.params[*i].name.clone()).collect(),
                    pending: t.pending.map(|e| match e {
                        Expiry::After(s) => ritsu_ports::Expiry::After(s),
                        Expiry::Never => ritsu_ports::Expiry::Never,
                    }),
                    moves: t
                        .moves
                        .iter()
                        .map(|m| {
                            let r = |r: &Ref| MoveRef {
                                account: book.accounts[r.kind].name.clone(),
                                args: r
                                    .args
                                    .iter()
                                    .map(|a| match a {
                                        Arg::Param(i) => Ok(t.params[*i].name.clone()),
                                        Arg::Lit(s) => Err(s.clone()),
                                    })
                                    .collect(),
                            };
                            PMove {
                                amount: match &m.amount {
                                    Amount::Param(i) => MoveAmount::Param(t.params[*i].name.clone()),
                                    Amount::Lit(v) => MoveAmount::Literal(*v),
                                },
                                from: r(&m.from),
                                to: r(&m.to),
                            }
                        })
                        .collect(),
                    refusals: report
                        .map(|rep| rep.ops.iter().filter(|o| o.kind == k).map(|o| (o.op.name().to_string(), o.refusals.iter().map(|r| r.name.clone()).collect())).collect())
                        .unwrap_or_default(),
                    machine: t.is_pending().then(|| hold_machine(t)),
                })
                .collect(),
        })
    }

    fn refusals(&self, file: &Path, transfer: &str, _amounts: (i128, i128)) -> Result<Found<Vec<(String, Vec<String>)>>, Vec<Said>> {
        checked(file)?;
        Ok(Found::Undecided(ritsu_base::tr!(
            "chobo の検査は、額を決まった値（1、2、3、5、10、100、1000）でしか試さないので、`{transfer}` の額を範囲に限った問いにはまだ答えられません",
            "chobo's check tries amounts of a few fixed values only (1, 2, 3, 5, 10, 100, 1000), so it cannot yet answer for `{transfer}` with its amounts held to a range"
        )))
    }

    fn open(&self, file: &Path) -> Result<Box<dyn Ledger>, Vec<Said>> {
        let (_, book, _) = checked(file)?;
        Ok(Box::new(Run { book, state: State::default() }))
    }
}

/// A book and what has happened to it.
struct Run {
    book: Book,
    state: State,
}

impl Ledger for Run {
    fn apply(&mut self, call: &BookCall) -> Result<BookOutcome, Text> {
        let book = &self.book;
        let kind = book.transfer(&call.transfer).ok_or_else(|| ritsu_base::tr!("振替 `{}` はありません", "there is no transfer `{}`", call.transfer))?;
        let op = Op::parse(&call.op).ok_or_else(|| ritsu_base::tr!("操作 `{}` はありません（do、hold、post、void のどれか）", "there is no operation `{}` (do, hold, post or void)", call.op))?;
        let t = &book.transfers[kind];
        for (name, _) in &call.args {
            if !t.params.iter().any(|p| p.name == *name) {
                return Err(ritsu_base::tr!("振替 `{}` に引数 `{name}` はありません", "the transfer `{}` takes no `{name}`", t.name));
            }
        }
        let args = t
            .params
            .iter()
            .map(|p| {
                call.args.iter().find(|(n, _)| *n == p.name).map(|(_, v)| match v {
                    Ok(n) => Val::Amt(*n),
                    Err(s) => Val::Str(s.clone()),
                })
            })
            .collect();
        let amounts = match &call.amounts {
            None => None,
            Some(a) => {
                let mut m = std::collections::BTreeMap::new();
                for (name, v) in a {
                    let i = t.params.iter().position(|p| p.name == *name).ok_or_else(|| ritsu_base::tr!("振替 `{}` に額 `{name}` はありません", "the transfer `{}` has no amount `{name}`", t.name))?;
                    m.insert(i, *v);
                }
                Some(m)
            }
        };
        let c = Call { op, kind, args, amounts };
        // what is wrong with a call is chobo's own words, which are English
        crate::interp::validate(book, &c).map_err(Text::same)?;
        let out = self.state.apply(book, &c).map_err(Text::same)?;
        Ok(match out {
            Outcome::Done => BookOutcome::Done,
            Outcome::DoneBefore => BookOutcome::DoneBefore,
            Outcome::Refused(r) => BookOutcome::Refused(r.reason),
        })
    }

    fn pass(&mut self, seconds: u64) -> Vec<(String, Vec<String>)> {
        self.state.pass(seconds).into_iter().map(|(k, key)| (self.book.transfers[k].name.clone(), key)).collect()
    }

    fn balance(&self, account: &str, args: &[String]) -> Result<Balance, Text> {
        let kind = self.book.account(account).ok_or_else(|| ritsu_base::tr!("勘定 `{account}` はありません", "there is no account `{account}`"))?;
        let b = self.state.balance(&AccountId { kind, args: args.to_vec() });
        Ok(Balance { posted: b.posted, held_in: b.held_in, held_out: b.held_out })
    }
}

/// A value as the definition of an item writes it (yuen's DESIGN 3.2): the keys in the order of
/// their bytes, two spaces of indent, one newline at the end.
fn definition(v: &serde_json::Value) -> String {
    fn sorted(v: &serde_json::Value) -> serde_json::Value {
        match v {
            serde_json::Value::Object(m) => {
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                serde_json::Value::Object(keys.into_iter().map(|k| (k.clone(), sorted(&m[k]))).collect())
            }
            serde_json::Value::Array(a) => serde_json::Value::Array(a.iter().map(sorted).collect()),
            other => other.clone(),
        }
    }
    serde_json::to_string_pretty(&sorted(v)).unwrap_or_default() + "\n"
}

fn without(v: &serde_json::Value, keys: &[&str]) -> serde_json::Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        for k in keys {
            o.remove(*k);
        }
    }
    v
}

impl ritsu_ports::Items for Engine {
    /// Each unit, account and transfer of a book. The definition of each is what `chobo api` says
    /// of it without its name and the numbers chobo makes from the name (yuen's DESIGN 3.2): a
    /// unit's scale; an account's parameters, unit, bounds and description; a transfer's
    /// parameters, key, hold and moves, with each account its moves touch.
    fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        let disk = ritsu_base::paths::on_disk(root, file);
        let (src, book, c) = checked(&disk)?;
        let stem = disk.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let api = crate::api::api(&book, &src, c.report.as_ref().expect("a book that passes check has a report"), &stem);
        let naming = |kind: &str, name: &str| Naming::file(Tool::Chobo, file).with(kind, name);
        let mut out = Vec::new();
        for (u, j) in book.units.iter().zip(api["units"].as_array().into_iter().flatten()) {
            out.push(Item { naming: naming("unit", &u.name), lines: (u.line, u.line), text: definition(&without(j, &["name", "ledger"])) });
        }
        let accounts: Vec<serde_json::Value> = api["accounts"].as_array().cloned().unwrap_or_default();
        for (a, j) in book.accounts.iter().zip(&accounts) {
            let end = [a.lower.as_ref().map(|b| b.line), a.upper.as_ref().map(|b| b.line)].into_iter().flatten().fold(a.line, usize::max);
            out.push(Item { naming: naming("account", &a.name), lines: (a.line, end), text: definition(&without(j, &["name", "code"])) });
        }
        for (t, j) in book.transfers.iter().zip(api["transfers"].as_array().into_iter().flatten()) {
            let mut v = without(j, &["name", "code", "definition", "operations"]);
            let mut touched = serde_json::Map::new();
            for m in &t.moves {
                for r in [&m.from, &m.to] {
                    let name = &book.accounts[r.kind].name;
                    touched.insert(name.clone(), without(&accounts[r.kind], &["name", "code"]));
                }
            }
            v["accounts"] = serde_json::Value::Object(touched);
            let end = t.moves.iter().map(|m| m.line).fold(t.line.max(t.key_line), usize::max);
            out.push(Item { naming: naming("transfer", &t.name), lines: (t.line, end), text: definition(&v) });
        }
        out.sort_by_key(|i| i.lines.0);
        Ok(out)
    }
}
