//! The Python clients: one module each. The PostgreSQL one takes a DB-API connection
//! (psycopg 3's, or psycopg2's); the TigerBeetle one a `tigerbeetle.ClientSync`.

use super::*;
use crate::postgres;

const TB_RUNTIME: &str = include_str!("runtime/tigerbeetle.py");
const PG_RUNTIME: &str = include_str!("runtime/postgres.py");

use ritsu_emit::words::python::KEYWORDS;

/// A name of the book as Python can take it: a keyword, or a name the method's body uses itself,
/// gets `_` after it.
pub fn name(s: &str) -> String {
    ritsu_emit::ident::aside(s, |w| KEYWORDS.contains(&w) || ["self", "_str", "_amt", "_amounts"].contains(&w))
}

/// A string as a Python literal, in single quotes.
use ritsu_emit::lit::python as q;

/// The attributes of the book value, one per transfer kind, kept apart from `balance` and
/// `expire` (the PostgreSQL client's), so that the two clients name them alike.
pub fn members(book: &Book) -> Vec<String> {
    unique(book.transfers.iter().map(|t| name(&t.name)).collect(), &["balance", "expire"])
}

fn sig(t: &TransferKind, which: &[usize], optional: &[usize]) -> String {
    let mut ps: Vec<String> = which
        .iter()
        .map(|i| {
            let p = &t.params[*i];
            format!("{}: {}", name(&p.name), if matches!(p.ty, Ty::Amount(_)) { "int" } else { "str" })
        })
        .collect();
    ps.extend(optional.iter().map(|i| format!("{}: Optional[int] = None", name(&t.params[*i].name))));
    if ps.is_empty() { "self".into() } else { format!("self, *, {}", ps.join(", ")) }
}

fn values(t: &TransferKind, which: &[usize]) -> String {
    which
        .iter()
        .map(|i| {
            let p = &t.params[*i];
            let f = if matches!(p.ty, Ty::Amount(_)) { "_amt" } else { "_str" };
            format!("{f}({}, {})", q(&p.name), name(&p.name))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn amounts(t: &TransferKind) -> String {
    let ap = t.amount_params();
    let names: Vec<String> = ap.iter().map(|i| q(&t.params[*i].name)).collect();
    let vals: Vec<String> = ap.iter().map(|i| name(&t.params[*i].name)).collect();
    format!("_amounts([{}], [{}])", names.join(", "), vals.join(", "))
}

fn doc(book: &Book, t: &TransferKind) -> String {
    let moves: Vec<String> = t.moves.iter().map(|m| format!("{} from {} to {}", book.amount_text(t, m), book.ref_text(t, &m.from), book.ref_text(t, &m.to))).collect();
    let keys: Vec<&str> = t.key.iter().map(|i| t.params[*i].name.as_str()).collect();
    format!("    \"\"\"{}: {}; once per {}.\"\"\"\n", t.name, moves.join(", then "), keys.join(", "))
}

fn class_name(t: &TransferKind) -> String {
    format!("_{}Transfer", t.name)
}

fn plan_literal(book: &Book) -> String {
    let p = plan(book);
    let mut o = format!("_BOOK = {{\n    'name': {},\n    'units': {{\n", q(&book.name));
    for u in &p.units {
        o.push_str(&format!("        {}: {{'ledger': {}, 'sink_code': {}}},\n", q(&u.name), u.ledger, u.sink_code));
    }
    o.push_str("    },\n    'accounts': {\n");
    let bound = |b: &Option<(i128, String)>| match b {
        Some((v, r)) => format!("{{'value': {v}, 'reason': {}}}", q(r)),
        None => "None".into(),
    };
    let tf = |b: bool| if b { "True" } else { "False" };
    for a in &p.accounts {
        let ps: Vec<String> = a.params.iter().map(|x| q(x)).collect();
        o.push_str(&format!(
            "        {}: {{'params': [{}], 'unit': {}, 'code': {}, 'flagged': {}, 'lower': {}, 'upper': {}}},\n",
            q(&a.name),
            ps.join(", "),
            q(&a.unit),
            a.code,
            tf(a.flagged),
            bound(&a.lower),
            bound(&a.upper)
        ));
    }
    o.push_str("    },\n    'transfers': {\n");
    let rf = |r: &RefPlan| {
        let args: Vec<String> = r
            .args
            .iter()
            .map(|a| match a {
                ArgPlan::Param(i) => format!("{{'param': {i}}}"),
                ArgPlan::Lit(s) => format!("{{'literal': {}}}", q(s)),
            })
            .collect();
        format!("{{'kind': {}, 'args': [{}]}}", q(&r.kind), args.join(", "))
    };
    for t in &p.transfers {
        let ps: Vec<String> = t.params.iter().map(|x| format!("{{'name': {}, 'amount': {}}}", q(&x.name), tf(x.amount))).collect();
        let key: Vec<String> = t.key.iter().map(|i| i.to_string()).collect();
        o.push_str(&format!(
            "        {}: {{\n            'params': [{}],\n            'key': [{}],\n            'pending': {},\n            'timeout': {},\n            'code': {},\n            'definition': {},\n            'moves': [\n",
            q(&t.name),
            ps.join(", "),
            key.join(", "),
            tf(t.pending),
            t.timeout,
            t.code,
            q(&t.definition)
        ));
        for m in &t.moves {
            let amount = match &m.amount {
                AmountPlan::Param(i) => format!("{{'param': {i}}}"),
                AmountPlan::Lit(v) => format!("{{'literal': {v}}}"),
            };
            let roles: Vec<String> = m.roles.iter().map(|r| q(r)).collect();
            o.push_str(&format!("                {{'amount': {amount}, 'from': {}, 'to': {}, 'roles': [{}]}},\n", rf(&m.from), rf(&m.to), roles.join(", ")));
        }
        o.push_str("            ],\n        },\n");
    }
    o.push_str("    },\n}\n");
    o
}

fn split(runtime: &str) -> (&str, &str) {
    runtime.split_once("#@@BOOK@@\n").expect("the runtime has its #@@BOOK@@ line")
}

fn header(book: &Book, origin: &Origin, what: &str) -> String {
    format!("{}\"\"\"The client of the book {} (v{}).\n\n{what}\n\"\"\"\n\n", super::head(book, origin, Comment::Hash), book.name, book.version)
}

fn balances(book: &Book, call: impl Fn(&AccountKind, String) -> String) -> String {
    let mut o = String::from("\n\nclass _Balances:\n    \"\"\"The balance of each account kind.\"\"\"\n\n    def __init__(self, r: Any) -> None:\n        self._r = r\n");
    for a in &book.accounts {
        let ps: Vec<String> = a.params.iter().map(|p| format!("{}: str", name(p))).collect();
        let s = if ps.is_empty() { "self".to_string() } else { format!("self, *, {}", ps.join(", ")) };
        let vals: Vec<String> = a.params.iter().map(|p| format!("_str({}, {})", q(p), name(p))).collect();
        o.push_str(&format!("\n    def {}({s}) -> Balance:\n        return {}\n", name(&a.name), call(a, vals.join(", "))));
    }
    o
}

pub fn tigerbeetle(book: &Book, origin: &Origin) -> String {
    let (head, tail) = split(TB_RUNTIME);
    let mut o = header(book, origin, "It calls the book on TigerBeetle through the tigerbeetle package 0.17.9 (DESIGN 4.2, 4.3).");
    o.push_str(head.trim_end());
    o.push('\n');
    for t in &book.transfers {
        let all: Vec<usize> = (0..t.params.len()).collect();
        let kind = q(&t.name);
        o.push_str(&format!("\n\nclass {}:\n{}\n    def __init__(self, r: Any) -> None:\n        self._r = r\n", class_name(t), doc(book, t)));
        if t.is_pending() {
            let key = values(t, &t.key);
            let keys = postgres::key_params(t);
            o.push_str(&format!("\n    def hold({}) -> Result:\n        return self._r.move({kind}, 'hold', [{}])\n", sig(t, &all, &[]), values(t, &all)));
            let ap = t.amount_params();
            let am = if ap.is_empty() { "None".to_string() } else { amounts(t) };
            o.push_str(&format!("\n    def post({}) -> Result:\n        return self._r.end({kind}, 'post', [{key}], {am})\n", sig(t, &keys, &ap)));
            o.push_str(&format!("\n    def void({}) -> Result:\n        return self._r.end({kind}, 'void', [{key}], None)\n", sig(t, &keys, &[])));
            o.push_str(&format!("\n    def status({}) -> Optional[str]:\n        return self._r.status({kind}, [{key}])\n", sig(t, &keys, &[])));
        } else {
            o.push_str(&format!("\n    def do({}) -> Result:\n        return self._r.move({kind}, 'do', [{}])\n", sig(t, &all, &[]), values(t, &all)));
        }
    }
    o.push_str(&balances(book, |a, vals| format!("self._r.balance({}, [{vals}])", q(&a.name))));
    o.push_str(&format!("\n\nclass Book:\n    \"\"\"The book {} (v{}), called on TigerBeetle.\"\"\"\n\n    def __init__(self, r: Any) -> None:\n", book.name, book.version));
    for (t, n) in book.transfers.iter().zip(members(book)) {
        o.push_str(&format!("        self.{n} = {}(r)\n", class_name(t)));
    }
    o.push_str("        self.balance = _Balances(r)\n");
    o.push_str(
        "\n\ndef tigerbeetle(client: Any, tenant: str = \"\") -> Book:\n    \"\"\"The book on TigerBeetle, through a tigerbeetle.ClientSync. A tenant is a set of balances of\n    its own (DESIGN 4.3).\"\"\"\n    return Book(_TigerBeetle(_BOOK, client, tenant))\n\n\n",
    );
    o.push_str(&plan_literal(book));
    o.push_str("\n\n");
    o.push_str(tail);
    // what the calls run on, by its type, so that what they answer is typed too
    o.replace("(self, r: Any) -> None:", "(self, r: _TigerBeetle) -> None:")
}

pub fn postgres(book: &Book, origin: &Origin) -> String {
    let (head, tail) = split(PG_RUNTIME);
    let mut o = header(
        book,
        origin,
        &format!("It calls the SQL functions of `chobo build --target postgres` (schema {}) through a DB-API connection (DESIGN 4.1, 4.3).", postgres::ident(&book.name)),
    );
    o.push_str(head.trim_end());
    o.push('\n');
    let sql = |t: &TransferKind, op: &str, n: usize| q(&format!("select * from {}({})", postgres::qualified(book, &postgres::op_function(t, op)), vec!["%s"; n].join(", ")));
    for t in &book.transfers {
        let all: Vec<usize> = (0..t.params.len()).collect();
        let keys = postgres::key_params(t);
        o.push_str(&format!("\n\nclass {}:\n{}\n    def __init__(self, r: Any) -> None:\n        self._r = r\n", class_name(t), doc(book, t)));
        if t.is_pending() {
            let ap = t.amount_params();
            let key = values(t, &keys);
            o.push_str(&format!(
                "\n    def hold({}) -> Result:\n        return self._r.call({}, [self._r.tenant, {}])\n",
                sig(t, &all, &[]),
                sql(t, "hold", 1 + all.len()),
                values(t, &all)
            ));
            let am = if ap.is_empty() { String::new() } else { format!(", *{}", amounts(t)) };
            o.push_str(&format!(
                "\n    def post({}) -> Result:\n        return self._r.call({}, [self._r.tenant, {key}{am}])\n",
                sig(t, &keys, &ap),
                sql(t, "post", 1 + keys.len() + ap.len())
            ));
            o.push_str(&format!("\n    def void({}) -> Result:\n        return self._r.call({}, [self._r.tenant, {key}])\n", sig(t, &keys, &[]), sql(t, "void", 1 + keys.len())));
            let status = format!("select {}({}) as state", postgres::qualified(book, &postgres::op_function(t, "status")), vec!["%s"; 1 + keys.len()].join(", "));
            o.push_str(&format!("\n    def status({}) -> Optional[str]:\n        return self._r.status({}, [self._r.tenant, {key}])\n", sig(t, &keys, &[]), q(&status)));
        } else {
            o.push_str(&format!(
                "\n    def do({}) -> Result:\n        return self._r.call({}, [self._r.tenant, {}])\n",
                sig(t, &all, &[]),
                sql(t, "do", 1 + all.len()),
                values(t, &all)
            ));
        }
    }
    o.push_str(&balances(book, |a, vals| {
        let s = format!("select * from {}({})", postgres::qualified(book, &postgres::balance_function(a)), vec!["%s"; 1 + a.params.len()].join(", "));
        let vals = if vals.is_empty() { String::new() } else { format!(", {vals}") };
        format!("self._r.balance({}, [self._r.tenant{vals}])", q(&s))
    }));
    o.push_str(&format!(
        "\n\nclass Book:\n    \"\"\"The book {} (v{}), called on PostgreSQL.\"\"\"\n\n    def __init__(self, r: Any) -> None:\n        self._r = r\n",
        book.name, book.version
    ));
    for (t, n) in book.transfers.iter().zip(members(book)) {
        o.push_str(&format!("        self.{n} = {}(r)\n", class_name(t)));
    }
    o.push_str("        self.balance = _Balances(r)\n");
    o.push_str(&format!(
        "\n    def expire(self) -> int:\n        \"\"\"Give back what the holds past their expiry hold; call it from a job (DESIGN 4.1).\"\"\"\n        return self._r.expire({})\n",
        q(&postgres::expire_sql(book))
    ));
    o.push_str(
        "\n\ndef postgres(conn: Any, tenant: str = \"\") -> Book:\n    \"\"\"The book on PostgreSQL, through a DB-API connection. A tenant is a set of balances of its\n    own (DESIGN 4.3). The client neither commits nor rolls back: with autocommit on, each call\n    is a transaction of its own; with it off, the call is part of the caller's transaction.\"\"\"\n    return Book(_Postgres(conn, tenant))\n\n\n",
    );
    o.push_str(tail);
    // what the calls run on, by its type, so that what they answer is typed too
    o.replace("(self, r: Any) -> None:", "(self, r: _Postgres) -> None:")
}
