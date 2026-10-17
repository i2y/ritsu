//! `chobo build --target mysql`: the tables of a book on MySQL, TiDB and Vitess, and the SQL the
//! clients of the MySQL family send (DESIGN 4.5).
//!
//! The three share no code a database runs for the caller (a hosted Vitess takes no stored routine,
//! TiDB has none), so the clients run a call: each call is one transaction of the caller's
//! database, with its accounts locked in the order of their IDs. What a call checks is SQL written
//! here. A bound is about one row (DESIGN 0.1), so whether a move breaks it can be worked out from
//! that row and the arguments: the locking read of the accounts with bounds answers, for each row,
//! the first move whose bound the row would break, and the writes work the new balances out
//! themselves. A client fills in the values (IDs as hex, amounts as digits, the caller's strings as
//! the hex of their UTF-8) and picks the next statement from what the last one answered.
//!
//! An account without bounds is never locked: no check reads it, so what a call moves into or out
//! of it is a row of the entries, and its balance is its row plus the entries `expire()` has not
//! yet added to it. A call waits only for the calls that move the same bounded account.

use crate::diag::{self, Diag, DiagExt};
use crate::ids;
use crate::interp::{AccountId, Call, Hold, Op, Outcome, State};
use crate::model::*;
use serde_json::{Value, json};

/// MySQL keeps a name to 64 characters (TiDB and Vitess the same).
pub const NAME_MAX: usize = 64;

/// The tables of a book, each named `<book>_<table>`: a hosted Vitess makes no database of its
/// own, so a book's tables sit beside the caller's, its name in front.
pub const TABLES: [&str; 3] = ["accounts", "calls", "entries"];

/// The checks of the accounts, named `<book>_<check>`: MySQL wants a check's name to be one of its
/// kind in the whole database.
pub const CHECKS: [&str; 2] = ["within_lower", "within_upper"];

/// How a transaction starts: TiDB reads `/*T! … */` and takes the transaction pessimistically
/// whatever `tidb_txn_mode` says; MySQL and Vitess read a comment.
pub const BEGIN: &str = "begin /*T! pessimistic */";

/// The most holds and entries one statement of `expire()` takes.
pub const EXPIRE_MAX: usize = 1000;

/// A name as MySQL quotes it.
pub fn ident(s: &str) -> String {
    format!("`{}`", s.replace('`', "``"))
}

pub fn table(book: &Book, which: &str) -> String {
    ident(&format!("{}_{which}", book.name))
}

/// A name of the book as an SQL string. A name is letters, digits and `_` (syntax.rs), so a pair
/// of quotes is all it takes, whatever `sql_mode` says about backslashes.
pub fn name_lit(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// A string of the caller's as SQL: the hex of its UTF-8, read as utf8mb4. There is no quote and
/// no backslash to escape, whatever `sql_mode` says, and the statement's text is ASCII.
pub fn str_lit(s: &str) -> String {
    format!("_utf8mb4 X'{}'", ids::hex(s.as_bytes()))
}

pub fn id_lit(v: u128) -> String {
    format!("X'{}'", ids::id_hex(v))
}

/// The ID of a tenant: the first column of every table, so that a sharded Vitess can keep a
/// tenant's rows on one shard.
pub fn tenant_id(book: &Book, tenant: &str) -> u128 {
    ids::id(&["tenant", &book.name, tenant])
}

/// The ID of a call: the ID of the first transfer of its chain on TigerBeetle (DESIGN 4.3), so
/// that a hold has the ID it has on PostgreSQL and TigerBeetle.
pub fn call_id(book: &Book, tenant: &str, t: &TransferKind, op: Op, key: &[String]) -> u128 {
    ids::transfer_id(book, tenant, t, op, key, 0)
}

/// The ID under which `expire()` writes the entries of a hold it ends.
pub fn expire_id(hold: u128) -> u128 {
    ids::id(&["expire", &ids::id_hex(hold)])
}

/// Does the account kind have a bound: then its rows are locked and hold their balances.
pub fn bounded(a: &AccountKind) -> bool {
    a.lower.is_some() || a.upper.is_some()
}

/// A JSON array of strings, without spaces, as the clients write it.
pub fn json_strings(vs: &[String]) -> String {
    format!("[{}]", vs.iter().map(|v| ids::json_string(v)).collect::<Vec<_>>().join(","))
}

/// Put the values in a statement: each `{NAME}` of `template` by the value of NAME, in one pass
/// (a value is never looked into again).
pub fn fill(template: &str, values: &[(String, String)]) -> String {
    let mut out = String::with_capacity(template.len() + 64);
    let mut rest = template;
    while let Some(at) = rest.find('{') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        match after.find('}') {
            Some(end) if after[..end].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') && end > 0 => {
                let name = &after[..end];
                match values.iter().find(|(n, _)| n == name) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

// ── E062 ──────────────────────────────────────────────────────────────────

/// The names the SQL would make that MySQL cannot hold as written (E062): longer than 64
/// characters, or with a character beyond the Basic Multilingual Plane, which MySQL writes as `?`
/// with only a warning, so that two names can become one. Every name made is the book's name with
/// a table's or a check's after it.
pub fn check_names(book: &Book) -> Vec<Diag> {
    let mut d = Vec::new();
    let wide: Vec<char> = book.name.chars().filter(|c| (*c as u32) > 0xFFFF).collect();
    if let Some(c) = wide.first() {
        let name = &book.name;
        let cp = format!("U+{:04X}", *c as u32);
        d.push(
            diag::error(
                "E062",
                book.line,
                book.col,
                tr!(
                    "帳簿の名前 `{name}` に {c}（{cp}）があります。MySQL は名前に基本多言語面の外の文字を持てず、警告だけで `?` に書き換えるので、二つの名前が同じになりえます",
                    "The book's name `{name}` has {c} ({cp}); MySQL holds no character beyond the Basic Multilingual Plane in a name, and writes it as `?` with only a warning, so two names could become one"
                ),
            )
            .hint(tr!("帳簿の名前から、その字を外してください（テーブルと制約の名前は帳簿の名前から作ります）", "Take the character out of the book's name: the names of the tables and checks are made from it")),
        );
    }
    // the longest, the first of them when two are as long
    let longest = CHECKS.iter().chain(TABLES.iter()).map(|w| format!("{}_{w}", book.name)).fold(String::new(), |a, n| if n.chars().count() > a.chars().count() { n } else { a });
    let n = longest.chars().count();
    if n > NAME_MAX {
        let room = NAME_MAX - "_within_lower".len();
        d.push(
            diag::error(
                "E062",
                book.line,
                book.col,
                tr!(
                    "制約の名前 `{longest}` は {n} 字あります。MySQL の名前は 64 字までです",
                    "The check's name `{longest}` is {n} characters long; MySQL keeps a name to 64"
                ),
            )
            .hint(tr!("帳簿の名前を {room} 字までにしてください（後ろに `_within_lower` が付きます）", "Make the book's name at most {room} characters: `_within_lower` follows it")),
        );
    }
    d
}

// ── the tables ────────────────────────────────────────────────────────────

/// The tables of the book, or the names MySQL cannot hold. No comment holds a `;`: vtgate cuts
/// what it is sent at every one, comments and all.
pub fn build(book: &Book, origin: &crate::target::Origin) -> Result<String, Vec<Diag>> {
    let d = check_names(book);
    if !d.is_empty() {
        return Err(d);
    }
    let (a, c, e) = (table(book, "accounts"), table(book, "calls"), table(book, "entries"));
    let lower = ident(&format!("{}_within_lower", book.name));
    let upper = ident(&format!("{}_within_upper", book.name));
    let mut o = crate::client::head(book, origin, crate::client::Comment::Dashes);
    o.push_str(&format!(
        "-- The tables of the book {} v{} on MySQL 8.4 or later, TiDB 8.5 or later, and Vitess.\n\
         -- It can run any number of times: it makes what is missing. Nothing here runs a call: the\n\
         -- clients of `chobo build --target mysql-typescript`, `mysql-python` and `mysql-go` do, each\n\
         -- call one transaction, its accounts locked in the order of their IDs.\n\
         -- On TiDB, run it with tidb_enable_check_constraint ON: made without it, a table keeps no check.\n\n",
        book.name, book.version
    ));
    o.push_str(&format!(
        "-- the accounts: one row each, made the first time a call names it. An account with a bound\n\
         -- holds its balance here, and one without holds what expire() has added up of its entries. The\n\
         -- two checks stop a write that does not go through the clients, which check the bounds themselves.\n\
         create table if not exists {a} (\n  `tenant` binary(16) not null,\n  `id` binary(16) not null,\n  `tenant_name` text not null,\n  `kind` text not null,\n  `args` text not null,\n  `unit` text not null,\n  `lower_bound` bigint,\n  `upper_bound` bigint,\n  `posted` bigint not null default 0,\n  `held_in` bigint not null default 0,\n  `held_out` bigint not null default 0,\n  primary key (`tenant`, `id`),\n  constraint {lower} check (`lower_bound` is null or `posted` - `held_out` >= least(`lower_bound`, 0)),\n  constraint {upper} check (`upper_bound` is null or `posted` + `held_in` <= greatest(`upper_bound`, 0))\n) default charset = utf8mb4 collate = utf8mb4_bin;\n\n\
         -- the calls that used their keys: what each was called with and what came of it, and for a\n\
         -- hold, where it is too. A key and a tenant are compared as the bytes of their IDs, never as text.\n\
         create table if not exists {c} (\n  `tenant` binary(16) not null,\n  `id` binary(16) not null,\n  `tenant_name` text not null,\n  `kind` text not null,\n  `op` varchar(4) not null,\n  `key` text not null,\n  `content` text not null,\n  `definition` varchar(32) not null,\n  `result` varchar(8) not null,\n  `reason` text,\n  `at` datetime(6) not null,\n  `state` varchar(8),\n  `moves` text,\n  `deadline` datetime(6),\n  `posted` text,\n  `ended_definition` varchar(32),\n  `ended_at` datetime(6),\n  primary key (`tenant`, `id`),\n  key `due` (`state`, `deadline`)\n) default charset = utf8mb4 collate = utf8mb4_bin;\n\n\
         -- the entries: every change to an account, so that a balance can be worked out again. An\n\
         -- entry of an account with a bound is in its row at once (folded), and one of an account\n\
         -- without is added to its row by expire().\n\
         create table if not exists {e} (\n  `tenant` binary(16) not null,\n  `call_id` binary(16) not null,\n  `n` smallint not null,\n  `account` binary(16) not null,\n  `at` datetime(6) not null,\n  `kind` text not null,\n  `op` varchar(8) not null,\n  `key` text not null,\n  `d_posted` bigint not null,\n  `d_held_in` bigint not null,\n  `d_held_out` bigint not null,\n  `folded` tinyint not null,\n  primary key (`tenant`, `call_id`, `n`),\n  key `unfolded` (`folded`, `tenant`, `account`)\n) default charset = utf8mb4 collate = utf8mb4_bin;\n"
    ));
    Ok(o)
}

// ── the statements ────────────────────────────────────────────────────────

/// What every book's client sends, with `{NAME}` where a value goes.
pub struct Statements {
    pub begin: String,
    pub commit: String,
    pub rollback: String,
    /// the accounts a call names that a value of the book has not yet made sure of: read …
    pub read_accounts: String,
    /// … and made when missing, `{ROWS}` the `rows` of the account kinds
    pub make_accounts: String,
    /// by account kind: one row of `make_accounts`
    pub account_rows: Vec<String>,
    /// a call whose key was used: what it was called with
    pub read_call: String,
    /// a hold, locked
    pub hold_lock: String,
    pub status: String,
    pub balance: String,
    pub due: String,
    pub fold_list: String,
    pub fold_lock: String,
    pub fold_read: String,
    pub fold_apply: String,
    pub fold_mark: String,
    pub transfers: Vec<TransferSql>,
}

/// What a transfer kind's calls send.
pub struct TransferSql {
    pub key_insert: String,
    /// the locking read of the accounts with bounds; None when no move can break a bound (the
    /// update then locks the rows it writes)
    pub lock: Option<String>,
    /// what the locking read answers a row with, from 1: the reason of that bound, as SQL
    pub checks: Vec<(usize, String, String)>,
    pub refuse: String,
    pub entries: String,
    pub apply: Option<String>,
    /// for a pending kind: what `post`, `void` and `expire()` write, the hold's row first and then,
    /// when a move touches an account with a bound, the accounts' rows, each table in a statement
    /// of its own
    pub post_apply: Vec<String>,
    pub post_entries: String,
    pub void_apply: Vec<String>,
    pub void_entries: String,
    pub expire_apply: Vec<String>,
    pub expire_entries: String,
}

fn amt(m: &Move, k: usize) -> String {
    match &m.amount {
        Amount::Param(_) => format!("{{AMOUNT{k}}}"),
        Amount::Lit(v) => v.to_string(),
    }
}

fn case(alias: &str, id: &str, then: &str) -> String {
    format!("(case when {alias}`id` = {id} then {then} else 0 end)")
}

pub fn statements(book: &Book) -> Statements {
    let (a, c, e) = (table(book, "accounts"), table(book, "calls"), table(book, "entries"));
    let account_rows = book
        .accounts
        .iter()
        .map(|k| {
            let b = |x: &Option<Bound>| x.as_ref().map(|b| b.value.to_string()).unwrap_or_else(|| "null".into());
            format!("({{TENANT}}, {{ID}}, {{TENANT_NAME}}, {}, {{ARGS}}, {}, {}, {})", name_lit(&k.name), name_lit(&book.units[k.unit].name), b(&k.lower), b(&k.upper))
        })
        .collect();
    Statements {
        begin: BEGIN.into(),
        commit: "commit".into(),
        rollback: "rollback".into(),
        read_accounts: format!("select hex(`id`), cast(`lower_bound` as char), cast(`upper_bound` as char) from {a} where `tenant` = {{TENANT}} and `id` in ({{IDS}})"),
        make_accounts: format!("insert into {a} (`tenant`, `id`, `tenant_name`, `kind`, `args`, `unit`, `lower_bound`, `upper_bound`) values {{ROWS}} on duplicate key update `id` = `id`"),
        account_rows,
        read_call: format!("select `result`, `reason`, `content`, `definition` from {c} where `tenant` = {{TENANT}} and `id` = {{CALL}} for update"),
        hold_lock: format!("select `result`, `state`, `moves`, `posted`, `ended_definition`, `deadline` is not null and `deadline` <= utc_timestamp(6) from {c} where `tenant` = {{TENANT}} and `id` = {{HOLD}} for update"),
        status: format!("select `result`, `state`, `deadline` is not null and `deadline` <= utc_timestamp(6) from {c} where `tenant` = {{TENANT}} and `id` = {{HOLD}}"),
        balance: format!(
            "select cast(`a`.`posted` + coalesce(sum(`e`.`d_posted`), 0) as char), cast(`a`.`held_in` + coalesce(sum(`e`.`d_held_in`), 0) as char), cast(`a`.`held_out` + coalesce(sum(`e`.`d_held_out`), 0) as char) from {a} `a` left join {e} `e` on `e`.`folded` = 0 and `e`.`tenant` = `a`.`tenant` and `e`.`account` = `a`.`id` where `a`.`tenant` = {{TENANT}} and `a`.`id` = {{ID}} group by `a`.`posted`, `a`.`held_in`, `a`.`held_out`"
        ),
        due: format!("select hex(`tenant`), hex(`id`), `kind`, `key` from {c} where `state` = 'held' and `deadline` <= utc_timestamp(6) order by `deadline` limit {EXPIRE_MAX}"),
        fold_list: format!("select hex(`tenant`), hex(`account`) from {e} where `folded` = 0 group by `tenant`, `account` limit {EXPIRE_MAX}"),
        fold_lock: format!("select `posted` from {a} where `tenant` = {{TENANT}} and `id` = {{ID}} for update"),
        fold_read: format!("select hex(`call_id`), `n`, cast(`d_posted` as char), cast(`d_held_in` as char), cast(`d_held_out` as char) from {e} where `folded` = 0 and `tenant` = {{TENANT}} and `account` = {{ID}} order by `call_id`, `n` limit {EXPIRE_MAX} for update"),
        fold_apply: format!("update {a} set `posted` = `posted` + {{POSTED}}, `held_in` = `held_in` + {{HELD_IN}}, `held_out` = `held_out` + {{HELD_OUT}} where `tenant` = {{TENANT}} and `id` = {{ID}}"),
        fold_mark: format!("update {e} set `folded` = 1 where `tenant` = {{TENANT}} and (`call_id`, `n`) in ({{ROWS}})"),
        transfers: book.transfers.iter().map(|t| transfer_sql(book, t)).collect(),
    }
}

fn transfer_sql(book: &Book, t: &TransferKind) -> TransferSql {
    let (a, c, e) = (table(book, "accounts"), table(book, "calls"), table(book, "entries"));
    let def = ids::definition(book, t);
    let kind = name_lit(&t.name);
    let op = if t.is_pending() { Op::Hold } else { Op::Do };
    let held = op == Op::Hold;
    let bf = |m: &Move| bounded(&book.accounts[m.from.kind]);
    let bt = |m: &Move| bounded(&book.accounts[m.to.kind]);
    let any_bounded = t.moves.iter().any(|m| bf(m) || bt(m));

    // the call itself, made before anything is checked: a second call with the key waits here for
    // the first to commit
    let (hold_cols, hold_vals) = if held {
        let moves: Vec<String> = t.moves.iter().enumerate().map(|(i, m)| format!("{{\"from\":\"{{FROMX{0}}}\",\"to\":\"{{TOX{0}}}\",\"amount\":\"{1}\"}}", i + 1, amt(m, i + 1))).collect();
        let deadline = match t.pending {
            Some(Expiry::After(secs)) => format!("utc_timestamp(6) + interval {secs} second"),
            _ => "null".into(),
        };
        (", `state`, `moves`, `deadline`".to_string(), format!(", 'held', '[{}]', {deadline}", moves.join(",")))
    } else {
        (String::new(), String::new())
    };
    let key_insert = format!(
        "insert into {c} (`tenant`, `id`, `tenant_name`, `kind`, `op`, `key`, `content`, `definition`, `result`, `at`{hold_cols}) values ({{TENANT}}, {{CALL}}, {{TENANT_NAME}}, {kind}, '{}', {{KEY}}, {{CONTENT}}, '{def}', 'done', utc_timestamp(6){hold_vals})",
        op.name()
    );

    // the checks, in the order DESIGN 2.2 makes them: move by move, the account it takes from,
    // then the one it puts into, each against what the moves before it left of that row
    let mut whens = Vec::new();
    let mut checks = Vec::new();
    for (i, m) in t.moves.iter().enumerate() {
        let k = i + 1;
        // what the moves before this one did to a row of the kind `kind`, column by column
        let before = |kind: usize, col: &str| -> String {
            let mut s = String::new();
            for (j, p) in t.moves[..i].iter().enumerate() {
                let n = j + 1;
                let pa = amt(p, n);
                match (held, col) {
                    (false, "posted") => {
                        if p.from.kind == kind {
                            s.push_str(&format!(" - {}", case("", &format!("{{FROM{n}}}"), &pa)));
                        }
                        if p.to.kind == kind {
                            s.push_str(&format!(" + {}", case("", &format!("{{TO{n}}}"), &pa)));
                        }
                    }
                    (true, "held_out") if p.from.kind == kind => s.push_str(&format!(" + {}", case("", &format!("{{FROM{n}}}"), &pa))),
                    (true, "held_in") if p.to.kind == kind => s.push_str(&format!(" + {}", case("", &format!("{{TO{n}}}"), &pa))),
                    _ => {}
                }
            }
            s
        };
        let ma = amt(m, k);
        if let Some(l) = &book.accounts[m.from.kind].lower {
            let cond = if held {
                format!("`posted` - (`held_out`{}) - {ma} < {}", before(m.from.kind, "held_out"), l.value)
            } else {
                format!("`posted`{} - `held_out` - {ma} < {}", before(m.from.kind, "posted"), l.value)
            };
            let n = 2 * k - 1;
            whens.push(format!("when `id` = {{FROM{k}}} and {cond} then {n}"));
            checks.push((n, l.refusal.clone(), name_lit(&l.refusal)));
        }
        if let Some(u) = &book.accounts[m.to.kind].upper {
            let cond = if held {
                format!("`posted` + (`held_in`{}) + {ma} > {}", before(m.to.kind, "held_in"), u.value)
            } else {
                format!("`posted`{} + `held_in` + {ma} > {}", before(m.to.kind, "posted"), u.value)
            };
            let n = 2 * k;
            whens.push(format!("when `id` = {{TO{k}}} and {cond} then {n}"));
            checks.push((n, u.refusal.clone(), name_lit(&u.refusal)));
        }
    }
    // with no check to make, there is nothing to read: the update locks the rows, in the order of
    // their IDs, when it writes them
    let lock = (!whens.is_empty()).then(|| format!("select hex(`id`), case {} else 0 end from {a} where `tenant` = {{TENANT}} and `id` in ({{BOUNDED}}) order by `id` for update", whens.join(" ")));
    let refuse = format!(
        "update {c} set `result` = 'refused', `reason` = {{REASON}}, `state` = null, `moves` = null, `deadline` = null where `tenant` = {{TENANT}} and `id` = {{CALL}}"
    );

    // the entries: two a move, the account it takes from first; an account with a bound has it in
    // its row at once
    let entry_rows = |op: &str, call: &str, cols: &dyn Fn(usize, &Move, bool) -> (String, String, String)| -> String {
        let mut rows = Vec::new();
        for (i, m) in t.moves.iter().enumerate() {
            let k = i + 1;
            for (side, from) in [("FROM", true), ("TO", false)] {
                let n = 2 * k - usize::from(from);
                let folded = if from { bf(m) } else { bt(m) };
                let (dp, di, dout) = cols(k, m, from);
                rows.push(format!(
                    "({{TENANT}}, {call}, {n}, {{{side}{k}}}, utc_timestamp(6), {kind}, '{op}', {{KEY}}, {dp}, {di}, {dout}, {})",
                    u8::from(folded)
                ));
            }
        }
        format!("insert into {e} (`tenant`, `call_id`, `n`, `account`, `at`, `kind`, `op`, `key`, `d_posted`, `d_held_in`, `d_held_out`, `folded`) values {}", rows.join(", "))
    };
    let neg = |s: &str| format!("-{s}");
    let entries = if held {
        entry_rows("hold", "{CALL}", &|k, m, from| if from { ("0".into(), "0".into(), amt(m, k)) } else { ("0".into(), amt(m, k), "0".into()) })
    } else {
        entry_rows("do", "{CALL}", &|k, m, from| if from { (neg(&amt(m, k)), "0".into(), "0".into()) } else { (amt(m, k), "0".into(), "0".into()) })
    };

    // the new balances of the accounts with bounds, worked out by the update itself
    let sets = |alias: &str, terms: &[(&str, &str, String, String)]| -> Vec<String> {
        let mut out = Vec::new();
        for col in ["posted", "held_in", "held_out"] {
            let mine: Vec<String> = terms.iter().filter(|(c, ..)| *c == col).map(|(_, sign, id, v)| format!(" {sign} {}", case(alias, id, v))).collect();
            if !mine.is_empty() {
                out.push(format!("{alias}`{col}` = {alias}`{col}`{}", mine.concat()));
            }
        }
        out
    };
    let apply = any_bounded.then(|| {
        let mut terms: Vec<(&str, &str, String, String)> = Vec::new();
        for (i, m) in t.moves.iter().enumerate() {
            let k = i + 1;
            let ma = amt(m, k);
            if bf(m) {
                terms.push(if held { ("held_out", "+", format!("{{FROM{k}}}"), ma.clone()) } else { ("posted", "-", format!("{{FROM{k}}}"), ma.clone()) });
            }
            if bt(m) {
                terms.push(if held { ("held_in", "+", format!("{{TO{k}}}"), ma.clone()) } else { ("posted", "+", format!("{{TO{k}}}"), ma.clone()) });
            }
        }
        format!("update {a} set {} where `tenant` = {{TENANT}} and `id` in ({{BOUNDED}})", sets("", &terms).join(", "))
    });

    // the end of a hold: the hold's row says how it ended, and on the accounts with bounds what was
    // held comes off, what is posted goes on, and the rest goes back. One statement a table: an
    // update of two tables joined would be run by Vitess's vtgate as a read of their keys first,
    // which it plans from the columns its schema tracking knows, and sends the tuple it read to
    // every shard, an empty one too, which fails
    let end = |state: &str, posted: bool, extra: &str| -> Vec<String> {
        let mut terms: Vec<(&str, &str, String, String)> = Vec::new();
        for (i, m) in t.moves.iter().enumerate() {
            let k = i + 1;
            if bf(m) {
                if posted {
                    terms.push(("posted", "-", format!("{{FROM{k}}}"), format!("{{POSTED{k}}}")));
                }
                terms.push(("held_out", "-", format!("{{FROM{k}}}"), format!("{{HELD{k}}}")));
            }
            if bt(m) {
                if posted {
                    terms.push(("posted", "+", format!("{{TO{k}}}"), format!("{{POSTED{k}}}")));
                }
                terms.push(("held_in", "-", format!("{{TO{k}}}"), format!("{{HELD{k}}}")));
            }
        }
        let mut out = vec![format!("update {c} set `state` = '{state}'{}, `ended_at` = utc_timestamp(6) where `tenant` = {{TENANT}} and `id` = {{HOLD}}", extra.replace("{H}", ""))];
        if any_bounded {
            out.push(format!("update {a} set {} where `tenant` = {{TENANT}} and `id` in ({{BOUNDED}})", sets("", &terms).join(", ")));
        }
        out
    };
    let posted_json = format!("[{}]", (1..=t.moves.len()).map(|k| format!("\"{{POSTED{k}}}\"")).collect::<Vec<_>>().join(","));
    let post_apply = end("posted", true, &format!(", {{H}}`posted` = '{posted_json}', {{H}}`ended_definition` = '{def}'"));
    let void_apply = end("voided", false, &format!(", {{H}}`ended_definition` = '{def}'"));
    let expire_apply = end("expired", false, "");
    let post_entries = entry_rows("post", "{CALL}", &|k, _, from| {
        if from { (format!("-{{POSTED{k}}}"), "0".into(), format!("-{{HELD{k}}}")) } else { (format!("{{POSTED{k}}}"), format!("-{{HELD{k}}}"), "0".into()) }
    });
    let gone = |k: usize, from: bool| if from { ("0".to_string(), "0".to_string(), format!("-{{HELD{k}}}")) } else { ("0".to_string(), format!("-{{HELD{k}}}"), "0".to_string()) };
    let void_entries = entry_rows("void", "{CALL}", &|k, _, from| gone(k, from));
    let expire_entries = entry_rows("expire", "{CALL}", &|k, _, from| gone(k, from));
    TransferSql { key_insert, lock, checks, refuse, entries, apply, post_apply, post_entries, void_apply, void_entries, expire_apply, expire_entries }
}

// ── what a client sends, as `chobo run --show mysql` says it ─────────────

/// The values of a call that every one of its statements may take.
fn call_values(book: &Book, tenant: &str, c: &Call) -> Vec<(String, String)> {
    let t = &book.transfers[c.kind];
    let key = c.key(t);
    let mut v = vec![
        ("TENANT".to_string(), id_lit(tenant_id(book, tenant))),
        ("TENANT_NAME".to_string(), str_lit(tenant)),
        ("KEY".to_string(), str_lit(&json_strings(&key))),
    ];
    if matches!(c.op, Op::Do | Op::Hold) {
        v.push(("CALL".into(), id_lit(call_id(book, tenant, t, c.op, &key))));
        v.push(("CONTENT".into(), str_lit(&ids::content_json(book, c, None))));
        let mut bounded_ids: Vec<u128> = Vec::new();
        for (i, m) in t.moves.iter().enumerate() {
            let k = i + 1;
            let f = ids::account_id(book, tenant, &c.account(&m.from));
            let to = ids::account_id(book, tenant, &c.account(&m.to));
            v.push((format!("FROM{k}"), id_lit(f)));
            v.push((format!("TO{k}"), id_lit(to)));
            v.push((format!("FROMX{k}"), ids::id_hex(f)));
            v.push((format!("TOX{k}"), ids::id_hex(to)));
            v.push((format!("AMOUNT{k}"), c.amount(m).to_string()));
            if bounded(&book.accounts[m.from.kind]) {
                bounded_ids.push(f);
            }
            if bounded(&book.accounts[m.to.kind]) {
                bounded_ids.push(to);
            }
        }
        v.push(("BOUNDED".into(), id_list(bounded_ids)));
    }
    v
}

/// IDs as an SQL list: each once, in the order of their hex.
pub fn id_list(mut ids_: Vec<u128>) -> String {
    ids_.sort();
    ids_.dedup();
    ids_.iter().map(|i| id_lit(*i)).collect::<Vec<_>>().join(", ")
}

/// The values of the end of a hold (`post` or `void`), from the hold as the client reads it.
fn end_values(book: &Book, tenant: &str, c: &Call, hold: &Hold, posted: &[i128]) -> Vec<(String, String)> {
    let t = &book.transfers[c.kind];
    let key = c.key(t);
    let mut v = vec![
        ("TENANT".to_string(), id_lit(tenant_id(book, tenant))),
        ("KEY".to_string(), str_lit(&json_strings(&key))),
        ("HOLD".to_string(), id_lit(call_id(book, tenant, t, Op::Hold, &key))),
        ("CALL".to_string(), id_lit(call_id(book, tenant, t, c.op, &key))),
    ];
    let mut bounded_ids = Vec::new();
    for (i, (m, hm)) in t.moves.iter().zip(&hold.moves).enumerate() {
        let k = i + 1;
        let f = ids::account_id(book, tenant, &hm.from);
        let to = ids::account_id(book, tenant, &hm.to);
        v.push((format!("FROM{k}"), id_lit(f)));
        v.push((format!("TO{k}"), id_lit(to)));
        v.push((format!("HELD{k}"), hm.amount.to_string()));
        v.push((format!("POSTED{k}"), posted.get(i).copied().unwrap_or(0).to_string()));
        if bounded(&book.accounts[m.from.kind]) {
            bounded_ids.push(f);
        }
        if bounded(&book.accounts[m.to.kind]) {
            bounded_ids.push(to);
        }
    }
    v.push(("BOUNDED".into(), id_list(bounded_ids)));
    v
}

/// The accounts a `do` or `hold` makes sure of before it starts (a value of the book does it the
/// first time it names one), as hex.
fn accounts_of(book: &Book, tenant: &str, c: &Call) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in c.accounts(book) {
        let h = ids::id_hex(ids::account_id(book, tenant, &a));
        if !out.contains(&h) {
            out.push(h);
        }
    }
    out
}

/// What a client of the MySQL family sends for one call, from the state it finds, without a
/// transaction of the caller's and one statement at a time: the accounts it makes sure of
/// (`accounts`, the first time a value of the book names each), then its statements, of which
/// the first `head` do not depend on what it finds. `tails` are every way the statements after
/// the head can go, for an operation of a `together`, which can find another state than the one
/// before it.
pub fn one(book: &Book, tenant: &str, state: &State, c: &Call) -> Value {
    let st = statements(book);
    let t = &book.transfers[c.kind];
    let ts = &st.transfers[c.kind];
    match c.op {
        Op::Do | Op::Hold => {
            let ids_: Vec<(AccountId, AccountId)> = t.moves.iter().map(|m| (c.account(&m.from), c.account(&m.to))).collect();
            if ids_.iter().any(|(f, to)| f == to) {
                return json!({"accounts": [], "statements": [], "head": 0, "tails": [[]]});
            }
            let v = call_values(book, tenant, c);
            // what is sent whatever the call finds: the transaction and the call itself
            let head = vec![st.begin.clone(), fill(&ts.key_insert, &v)];
            // the key was used: what for, and nothing more
            let dup = vec![fill(&st.read_call, &v), st.rollback.clone()];
            let lock: Vec<String> = ts.lock.iter().map(|l| fill(l, &v)).collect();
            let done = {
                let mut d = lock.clone();
                d.push(fill(&ts.entries, &v));
                if let Some(ap) = &ts.apply {
                    d.push(fill(ap, &v));
                }
                d.push(st.commit.clone());
                d
            };
            let refused = |reason: &str| {
                let mut w = v.clone();
                w.push(("REASON".into(), name_lit(reason)));
                let mut r = lock.clone();
                r.push(fill(&ts.refuse, &w));
                r.push(st.commit.clone());
                r
            };
            let mut tails: Vec<Vec<String>> = vec![done.clone(), dup.clone()];
            for (_, reason, _) in &ts.checks {
                let r = refused(reason);
                if !tails.contains(&r) {
                    tails.push(r);
                }
            }
            let mut s2 = state.clone();
            let tail = match s2.apply(book, c).unwrap_or(Outcome::Done) {
                Outcome::Done => done,
                Outcome::DoneBefore => dup,
                Outcome::Refused(r) if r.reason == "key_conflict" || r.reason == "already_refused" => dup,
                Outcome::Refused(r) => refused(&r.reason),
            };
            let statements = [head.clone(), tail].concat();
            json!({"accounts": accounts_of(book, tenant, c), "statements": statements, "head": head.len(), "tails": tails})
        }
        Op::Post | Op::Void => {
            let key = c.key(t);
            let base = vec![
                ("TENANT".to_string(), id_lit(tenant_id(book, tenant))),
                ("HOLD".to_string(), id_lit(call_id(book, tenant, t, Op::Hold, &key))),
            ];
            let head = vec![st.begin.clone(), fill(&st.hold_lock, &base)];
            let quit = vec![st.rollback.clone()];
            let mut tails = vec![quit.clone()];
            let mut s2 = state.clone();
            let outcome = s2.apply(book, c).unwrap_or(Outcome::Done);
            let hold = state.holds.get(&(c.kind, key.clone()));
            let done = hold.map(|h| {
                let posted = if c.op == Op::Post { State::post_amounts(book, c, h) } else { vec![0; h.moves.len()] };
                let v = end_values(book, tenant, c, h, &posted);
                let (ap, en) = if c.op == Op::Post { (&ts.post_apply, &ts.post_entries) } else { (&ts.void_apply, &ts.void_entries) };
                let mut d: Vec<String> = ap.iter().map(|s| fill(s, &v)).collect();
                d.push(fill(en, &v));
                d.push(st.commit.clone());
                d
            });
            if let Some(d) = &done {
                tails.push(d.clone());
            }
            let statements = match (&outcome, &done) {
                (Outcome::Done, Some(d)) => [head.clone(), d.clone()].concat(),
                _ => [head.clone(), quit].concat(),
            };
            json!({"accounts": [], "statements": statements, "head": head.len(), "tails": tails})
        }
    }
}

/// The statements that make sure of the accounts, which the tests do not compare word for word
/// (a value of the book makes sure of an account the first time it names it): do they start like
/// that.
pub fn is_making_sure(book: &Book, sql: &str) -> bool {
    let st = statements(book);
    let read = st.read_accounts.split("{TENANT}").next().unwrap_or("");
    let make = st.make_accounts.split("{ROWS}").next().unwrap_or("");
    sql.starts_with(read) || sql.starts_with(make)
}

/// What the book's name makes the tables and checks named, for `chobo api`.
pub fn names(book: &Book) -> Value {
    json!({
        "tables": TABLES.iter().map(|w| (w.to_string(), json!(format!("{}_{w}", book.name)))).collect::<serde_json::Map<String, Value>>(),
        "checks": CHECKS.iter().map(|w| format!("{}_{w}", book.name)).collect::<Vec<_>>(),
        "begin": BEGIN,
    })
}

