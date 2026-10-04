//! The facts of a rule, of a dates file and of a book as JSON, for the bundle the playground reads
//! (crate::sources::Bundle): what rulec, koyomi and chobo answered through ritsu's ports, written
//! down by the tests that record the bundle and read back in the browser, where none of them runs.
//! The form is dandori's own; it holds every field of `ritsu_ports::RuleFacts`, `DateFacts` and
//! `BookFacts`, and a unit as rulec spells it (`money[JPY, incl_tax]`, `rate[step 0.1%]`), which
//! reads back as the same unit; a book's count of things with a name and nothing else
//! (`unit pcs`) is `{"count": "pcs"}`.

use ritsu_ports::{
    Account, Axis, BookClient, BookFacts, BookUnit, Bound, Call, CallEnum, ClientTransfer, Column, ColumnType, Connect, DateCalendar, DateFacts, DateFunction, DateInput, DateKind, EnumValue, Expiry, Machine,
    MachineRow, Move, MoveAmount, MoveRef, Param, Precondition, RuleEnum, RuleFacts, Transfer, TransferParam, WireEnum, WireField,
};
use serde_json::{json, Map, Value};

/// A whole number of the wire: a JSON number when it fits in 64 bits, else its digits as a string.
fn int(v: i128) -> Value {
    match i64::try_from(v) {
        Ok(n) => json!(n),
        Err(_) => json!(v.to_string()),
    }
}

fn opt_int(v: Option<i128>) -> Value {
    v.map(int).unwrap_or(Value::Null)
}

fn column_type(t: &ColumnType) -> Value {
    match t {
        ColumnType::Bool => json!("bool"),
        ColumnType::Str => json!("string"),
        ColumnType::Date => json!("date"),
        ColumnType::Enum(e) => json!({ "enum": e }),
        ColumnType::Num { written, unit, min, max } => json!({ "number": written, "unit": unit.as_ref().map(|u| u.to_string()), "min": opt_int(*min), "max": opt_int(*max) }),
        ColumnType::Opt(inner) => json!({ "optional": column_type(inner) }),
    }
}

fn column(c: &Column) -> Value {
    json!({ "name": c.name, "alias": c.alias, "type": column_type(&c.ty) })
}

fn columns(cs: &[Column]) -> Value {
    Value::Array(cs.iter().map(column).collect())
}

fn wire_field(f: &WireField) -> Value {
    json!({ "name": f.name, "field": f.field, "type": f.ty, "optional": f.optional, "enum": f.enumeration })
}

fn wire_fields(fs: &[WireField]) -> Value {
    Value::Array(fs.iter().map(wire_field).collect())
}

fn call(c: &Call) -> Value {
    let params = |ps: &[Param]| Value::Array(ps.iter().map(|p| json!({ "name": p.name, "alias": p.alias, "type": p.ty, "optional": p.optional })).collect());
    json!({
        "module": c.module,
        "function": c.function,
        "input_type": c.input_type,
        "params": params(&c.params),
        "outputs": params(&c.outputs),
        "enums": c.enums.iter().map(|e| json!({ "name": e.name, "alias": e.alias, "values": e.values })).collect::<Vec<_>>(),
    })
}

fn machine(m: &Machine) -> Value {
    json!({
        "name": m.name,
        "carry_in": m.carry_in,
        "carry_out": m.carry_out,
        "state_enum": m.state_enum,
        "states": m.states,
        "initial": m.initial,
        "finals": m.finals,
        "held": m.held,
        "policy": m.policy,
        "axes": m.axes.iter().map(|a| json!({ "column": a.column, "coords": a.coords })).collect::<Vec<_>>(),
        "state_axis": m.state_axis,
        "decides": m.decides,
        "rows": m.rows.iter().map(|r| json!({ "row": r.row, "accepts": r.accepts, "to": r.to, "produces": r.produces })).collect::<Vec<_>>(),
    })
}

/// The facts as the bundle holds them.
pub fn facts_to_json(f: &RuleFacts) -> Value {
    let machine = f.machine.as_ref().map(machine);
    let preconditions: Vec<Value> = f
        .preconditions
        .iter()
        .map(|p| match p {
            Precondition::Relation { left, op, right } => json!({ "kind": "constraint", "left": left, "op": op, "right": right }),
            Precondition::Sum { name, over, of, max } => json!({ "kind": "sum", "name": name, "over": over, "of": of, "max": int(*max) }),
            Precondition::Length { sequence, max } => json!({ "kind": "length", "sequence": sequence, "max": int(*max) }),
            Precondition::Days { input, file, date, days } => json!({ "kind": "days", "input": input, "file": file, "date": date, "days": days.iter().map(|d| int(*d as i128)).collect::<Vec<_>>() }),
        })
        .collect();
    let connect = f.connect.as_ref().map(|c| {
        json!({
            "path": c.path,
            "json_names": c.json_names,
            "json_int64": c.json_int64,
            "request": wire_fields(&c.request),
            "response": wire_fields(&c.response),
            "elements": c.elements.as_ref().map(|e| wire_fields(e)),
            "enums": c.enums.iter().map(|e| json!({ "name": e.name, "alias": e.alias, "contract": e.contract, "unset": e.unset, "values": e.values })).collect::<Vec<_>>(),
        })
    });
    json!({
        "rule": f.rule,
        "alias": f.alias,
        "version": f.version,
        "sha256": f.sha256,
        "rulec": f.rulec,
        "inputs": columns(&f.inputs),
        "outputs": columns(&f.outputs),
        "elements": f.elements.as_ref().map(|(n, a, fs)| json!({ "name": n, "alias": a, "fields": columns(fs) })),
        "enums": f.enums.iter().map(|e| json!({ "name": e.name, "alias": e.alias, "values": e.values.iter().map(|v| json!({ "name": v.name, "alias": v.alias })).collect::<Vec<_>>() })).collect::<Vec<_>>(),
        "machine": machine,
        "preconditions": preconditions,
        "connect": connect,
        "typescript": call(&f.typescript),
        "python": call(&f.python),
        "go": call(&f.go),
    })
}

/// What koyomi knows of a dates file, as the bundle holds it.
pub fn dates_to_json(f: &DateFacts) -> Value {
    json!({
        "name": f.name,
        "alias": f.alias,
        "version": f.version,
        "sha256": f.sha256,
        "inputs": f.inputs.iter().map(|i| json!({ "name": i.name, "alias": i.alias, "kind": if i.kind == DateKind::Date { "date" } else { "int" }, "min": i.min, "max": i.max })).collect::<Vec<_>>(),
        "functions": f.functions.iter().map(|d| json!({ "name": d.name, "alias": d.alias, "params": d.params, "at": d.at })).collect::<Vec<_>>(),
        "calendar": f.calendar.as_ref().map(|c| json!({ "name": c.name, "data": [c.data.0, c.data.1], "offset": c.offset })),
        "claims": f.claims.iter().map(|(n, t)| json!([n, t])).collect::<Vec<_>>(),
    })
}

fn unit(u: &ritsu_units::Unit) -> Value {
    match &u.dim {
        ritsu_units::Dim::Count(n) => json!({ "count": n }),
        _ => json!(u.to_string()),
    }
}

fn client(c: &BookClient) -> Value {
    json!({ "module": c.module, "transfers": c.transfers.iter().map(|t| json!({ "member": t.member, "params": t.params })).collect::<Vec<_>>() })
}

/// What chobo knows of a book, as the bundle holds it.
pub fn book_to_json(f: &BookFacts) -> Value {
    let bound = |b: &Option<Bound>| b.as_ref().map(|b| json!({ "value": int(b.value), "refusal": b.refusal }));
    let at = |r: &MoveRef| json!({ "account": r.account, "args": r.args.iter().map(|a| match a { Ok(p) => json!({ "param": p }), Err(l) => json!({ "literal": l }) }).collect::<Vec<_>>() });
    let transfers: Vec<Value> = f
        .transfers
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "params": t.params.iter().map(|p| json!({ "name": p.name, "unit": p.unit })).collect::<Vec<_>>(),
                "key": t.key,
                "pending": t.pending.map(|e| match e { Expiry::After(s) => json!({ "after": s }), Expiry::Never => json!("never") }),
                "moves": t.moves.iter().map(|m| json!({
                    "amount": match &m.amount { MoveAmount::Param(p) => json!({ "param": p }), MoveAmount::Literal(v) => json!({ "literal": int(*v) }) },
                    "from": at(&m.from),
                    "to": at(&m.to),
                })).collect::<Vec<_>>(),
                "refusals": t.refusals.iter().map(|(op, rs)| json!([op, rs])).collect::<Vec<_>>(),
                "machine": t.machine.as_ref().map(machine),
            })
        })
        .collect();
    json!({
        "name": f.name,
        "version": f.version,
        "sha256": f.sha256,
        "units": f.units.iter().map(|u| json!({ "name": u.name, "scale": u.scale, "unit": unit(&u.unit) })).collect::<Vec<_>>(),
        "accounts": f.accounts.iter().map(|a| json!({ "name": a.name, "params": a.params, "unit": a.unit, "outside": a.outside, "lower": bound(&a.lower), "upper": bound(&a.upper) })).collect::<Vec<_>>(),
        "transfers": transfers,
        "typescript": client(&f.typescript),
        "python": client(&f.python),
        "go": client(&f.go),
    })
}

type R<T> = Result<T, String>;

fn obj<'a>(v: &'a Value, what: &str) -> R<&'a Map<String, Value>> {
    v.as_object().ok_or_else(|| format!("{what} is not an object"))
}

fn text(v: &Value, k: &str) -> R<String> {
    v[k].as_str().map(String::from).ok_or_else(|| format!("`{k}` is not text"))
}

fn opt_text(v: &Value, k: &str) -> R<Option<String>> {
    match &v[k] {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(format!("`{k}` is neither text nor null")),
    }
}

fn flag(v: &Value, k: &str) -> R<bool> {
    v[k].as_bool().ok_or_else(|| format!("`{k}` is not true or false"))
}

fn arr<'a>(v: &'a Value, k: &str) -> R<&'a Vec<Value>> {
    v[k].as_array().ok_or_else(|| format!("`{k}` is not a list"))
}

fn texts(v: &Value, k: &str) -> R<Vec<String>> {
    arr(v, k)?.iter().map(|x| x.as_str().map(String::from).ok_or_else(|| format!("`{k}` holds what is not text"))).collect()
}

fn index(v: &Value) -> R<usize> {
    v.as_u64().map(|n| n as usize).ok_or_else(|| format!("{v} is not an index"))
}

fn indexes(v: &Value, k: &str) -> R<Vec<usize>> {
    arr(v, k)?.iter().map(index).collect()
}

fn read_int(v: &Value) -> R<i128> {
    match v {
        Value::Number(n) => n.as_i64().map(i128::from).ok_or_else(|| format!("{n} is not a whole number")),
        Value::String(s) => s.parse().map_err(|_| format!("{s} is not a whole number")),
        _ => Err(format!("{v} is not a whole number")),
    }
}

fn read_opt_int(v: &Value) -> R<Option<i128>> {
    match v {
        Value::Null => Ok(None),
        x => read_int(x).map(Some),
    }
}

fn read_column_type(v: &Value) -> R<ColumnType> {
    match v {
        Value::String(s) if s == "bool" => Ok(ColumnType::Bool),
        Value::String(s) if s == "string" => Ok(ColumnType::Str),
        Value::String(s) if s == "date" => Ok(ColumnType::Date),
        Value::Object(o) if o.contains_key("enum") => Ok(ColumnType::Enum(text(v, "enum")?)),
        Value::Object(o) if o.contains_key("optional") => Ok(ColumnType::Opt(Box::new(read_column_type(&v["optional"])?))),
        Value::Object(o) if o.contains_key("number") => {
            let unit = match opt_text(v, "unit")? {
                Some(u) => Some(ritsu_units::Unit::parse(&u).map_err(|e| format!("the unit `{u}`: {}", e.text().en))?),
                None => None,
            };
            Ok(ColumnType::Num { written: text(v, "number")?, unit, min: read_opt_int(&v["min"])?, max: read_opt_int(&v["max"])? })
        }
        _ => Err(format!("{v} is not a type")),
    }
}

fn read_column(v: &Value) -> R<Column> {
    Ok(Column { name: text(v, "name")?, alias: text(v, "alias")?, ty: read_column_type(&v["type"])? })
}

fn read_columns(v: &Value, k: &str) -> R<Vec<Column>> {
    arr(v, k)?.iter().map(read_column).collect()
}

fn read_wire_field(v: &Value) -> R<WireField> {
    Ok(WireField { name: text(v, "name")?, field: text(v, "field")?, ty: text(v, "type")?, optional: flag(v, "optional")?, enumeration: opt_text(v, "enum")? })
}

fn read_wire_fields(v: &Value, k: &str) -> R<Vec<WireField>> {
    arr(v, k)?.iter().map(read_wire_field).collect()
}

fn pair(v: &Value) -> R<(String, String)> {
    match v.as_array().map(|a| a.as_slice()) {
        Some([Value::String(a), Value::String(b)]) => Ok((a.clone(), b.clone())),
        _ => Err(format!("{v} is not a pair of texts")),
    }
}

fn read_call(v: &Value) -> R<Call> {
    let params = |k: &str| -> R<Vec<Param>> { arr(v, k)?.iter().map(|p| Ok(Param { name: text(p, "name")?, alias: text(p, "alias")?, ty: text(p, "type")?, optional: flag(p, "optional")? })).collect() };
    let enums = arr(v, "enums")?
        .iter()
        .map(|e| Ok(CallEnum { name: text(e, "name")?, alias: text(e, "alias")?, values: arr(e, "values")?.iter().map(pair).collect::<R<_>>()? }))
        .collect::<R<_>>()?;
    Ok(Call { module: text(v, "module")?, function: text(v, "function")?, input_type: text(v, "input_type")?, params: params("params")?, outputs: params("outputs")?, enums })
}

fn read_machine(v: &Value) -> R<Machine> {
    obj(v, "the machine")?;
    let axes = arr(v, "axes")?.iter().map(|a| Ok(Axis { column: text(a, "column")?, coords: texts(a, "coords")? })).collect::<R<_>>()?;
    let rows = arr(v, "rows")?
        .iter()
        .map(|r| {
            Ok(MachineRow {
                row: index(&r["row"])?,
                accepts: arr(r, "accepts")?.iter().map(|a| a.as_array().ok_or("an axis of `accepts` is not a list".to_string()).and_then(|xs| xs.iter().map(index).collect())).collect::<R<_>>()?,
                to: match &r["to"] {
                    Value::Null => None,
                    x => Some(index(x)?),
                },
                produces: arr(r, "produces")?.iter().map(|x| x.as_str().map(String::from)).collect(),
            })
        })
        .collect::<R<_>>()?;
    Ok(Machine {
        name: text(v, "name")?,
        carry_in: text(v, "carry_in")?,
        carry_out: text(v, "carry_out")?,
        state_enum: text(v, "state_enum")?,
        states: texts(v, "states")?,
        initial: index(&v["initial"])?,
        finals: indexes(v, "finals")?,
        held: texts(v, "held")?,
        policy: text(v, "policy")?,
        axes,
        state_axis: match &v["state_axis"] {
            Value::Null => None,
            x => Some(index(x)?),
        },
        decides: texts(v, "decides")?,
        rows,
    })
}

fn read_connect(v: &Value) -> R<Connect> {
    let enums = arr(v, "enums")?
        .iter()
        .map(|e| {
            let values = arr(e, "values")?
                .iter()
                .map(|x| match x.as_array().map(|a| a.as_slice()) {
                    Some([Value::String(n), Value::String(a), num]) => Ok((n.clone(), a.clone(), num.as_i64().ok_or_else(|| format!("{num} is not a number"))?)),
                    _ => Err(format!("{x} is not a value on the wire")),
                })
                .collect::<R<_>>()?;
            let contract = match &e["contract"] {
                Value::Null => None,
                x => Some(pair(x)?),
            };
            Ok(WireEnum { name: text(e, "name")?, alias: text(e, "alias")?, contract, unset: opt_text(e, "unset")?, values })
        })
        .collect::<R<_>>()?;
    Ok(Connect {
        path: text(v, "path")?,
        json_names: text(v, "json_names")?,
        json_int64: text(v, "json_int64")?,
        request: read_wire_fields(v, "request")?,
        response: read_wire_fields(v, "response")?,
        elements: match &v["elements"] {
            Value::Null => None,
            _ => Some(read_wire_fields(v, "elements")?),
        },
        enums,
    })
}

/// The facts the bundle holds, read back.
pub fn facts_from_json(v: &Value) -> R<RuleFacts> {
    obj(v, "the facts")?;
    let elements = match &v["elements"] {
        Value::Null => None,
        e => Some((text(e, "name")?, text(e, "alias")?, read_columns(e, "fields")?)),
    };
    let enums = arr(v, "enums")?
        .iter()
        .map(|e| Ok(RuleEnum { name: text(e, "name")?, alias: text(e, "alias")?, values: arr(e, "values")?.iter().map(|x| Ok(EnumValue { name: text(x, "name")?, alias: text(x, "alias")? })).collect::<R<_>>()? }))
        .collect::<R<_>>()?;
    let preconditions = arr(v, "preconditions")?
        .iter()
        .map(|p| match text(p, "kind")?.as_str() {
            "constraint" => Ok(Precondition::Relation { left: text(p, "left")?, op: text(p, "op")?, right: text(p, "right")? }),
            "sum" => Ok(Precondition::Sum { name: text(p, "name")?, over: text(p, "over")?, of: text(p, "of")?, max: read_int(&p["max"])? }),
            "length" => Ok(Precondition::Length { sequence: text(p, "sequence")?, max: read_int(&p["max"])? }),
            "days" => Ok(Precondition::Days {
                input: text(p, "input")?,
                file: text(p, "file")?,
                date: text(p, "date")?,
                days: p["days"].as_array().map(|ds| ds.iter().filter_map(|d| read_int(d).ok()).map(|d| d as i64).collect()).unwrap_or_default(),
            }),
            k => Err(format!("`{k}` is not a kind of precondition")),
        })
        .collect::<R<_>>()?;
    Ok(RuleFacts {
        rule: text(v, "rule")?,
        alias: text(v, "alias")?,
        version: text(v, "version")?,
        sha256: text(v, "sha256")?,
        rulec: text(v, "rulec")?,
        inputs: read_columns(v, "inputs")?,
        outputs: read_columns(v, "outputs")?,
        elements,
        enums,
        machine: match &v["machine"] {
            Value::Null => None,
            m => Some(read_machine(m)?),
        },
        preconditions,
        connect: match &v["connect"] {
            Value::Null => None,
            c => Some(read_connect(c)?),
        },
        typescript: read_call(&v["typescript"])?,
        python: read_call(&v["python"])?,
        go: read_call(&v["go"])?,
    })
}

fn whole(v: &Value, k: &str) -> R<i64> {
    v[k].as_i64().ok_or_else(|| format!("`{k}` is not a whole number"))
}

/// The facts of a dates file the bundle holds, read back.
pub fn dates_from_json(v: &Value) -> R<DateFacts> {
    obj(v, "the facts of the dates file")?;
    let inputs = arr(v, "inputs")?
        .iter()
        .map(|i| {
            let kind = match text(i, "kind")?.as_str() {
                "date" => DateKind::Date,
                "int" => DateKind::Int,
                k => return Err(format!("`{k}` is not a kind of input")),
            };
            Ok(DateInput { name: text(i, "name")?, alias: text(i, "alias")?, kind, min: whole(i, "min")?, max: whole(i, "max")? })
        })
        .collect::<R<_>>()?;
    let functions = arr(v, "functions")?
        .iter()
        .map(|d| {
            let at = match &d["at"] {
                Value::Null => None,
                x => Some(x.as_u64().ok_or_else(|| format!("{x} is not a time of day"))? as u32),
            };
            Ok(DateFunction { name: text(d, "name")?, alias: text(d, "alias")?, params: texts(d, "params")?, at })
        })
        .collect::<R<_>>()?;
    let calendar = match &v["calendar"] {
        Value::Null => None,
        c => {
            let data = match c["data"].as_array().map(|a| a.as_slice()) {
                Some([a, b]) => (a.as_i64().ok_or("the calendar's data is not two days")?, b.as_i64().ok_or("the calendar's data is not two days")?),
                _ => return Err("the calendar's data is not two days".into()),
            };
            let offset = match &c["offset"] {
                Value::Null => None,
                x => Some(x.as_i64().ok_or_else(|| format!("{x} is not an offset"))? as i32),
            };
            Some(DateCalendar { name: text(c, "name")?, data, offset })
        }
    };
    Ok(DateFacts {
        name: text(v, "name")?,
        alias: text(v, "alias")?,
        version: text(v, "version")?,
        sha256: text(v, "sha256")?,
        inputs,
        functions,
        calendar,
        claims: arr(v, "claims")?.iter().map(pair).collect::<R<_>>()?,
    })
}

fn read_unit(v: &Value) -> R<ritsu_units::Unit> {
    match v {
        Value::Object(o) if o.contains_key("count") => Ok(ritsu_units::Unit::count(&text(v, "count")?)),
        Value::String(u) => ritsu_units::Unit::parse(u).map_err(|e| format!("the unit `{u}`: {}", e.text().en)),
        _ => Err(format!("{v} is not a unit")),
    }
}

fn read_client(v: &Value) -> R<BookClient> {
    let transfers = arr(v, "transfers")?.iter().map(|t| Ok(ClientTransfer { member: text(t, "member")?, params: texts(t, "params")? })).collect::<R<_>>()?;
    Ok(BookClient { module: text(v, "module")?, transfers })
}

/// The facts of a book the bundle holds, read back.
pub fn book_from_json(v: &Value) -> R<BookFacts> {
    obj(v, "the facts of the book")?;
    let bound = |b: &Value| -> R<Option<Bound>> {
        match b {
            Value::Null => Ok(None),
            x => Ok(Some(Bound { value: read_int(&x["value"])?, refusal: text(x, "refusal")? })),
        }
    };
    let at = |r: &Value| -> R<MoveRef> {
        let args = arr(r, "args")?
            .iter()
            .map(|a| match (&a["param"], &a["literal"]) {
                (Value::String(p), _) => Ok(Ok(p.clone())),
                (_, Value::String(l)) => Ok(Err(l.clone())),
                _ => Err(format!("{a} is neither a parameter nor a literal")),
            })
            .collect::<R<_>>()?;
        Ok(MoveRef { account: text(r, "account")?, args })
    };
    let units = arr(v, "units")?
        .iter()
        .map(|u| Ok(BookUnit { name: text(u, "name")?, scale: u["scale"].as_u64().ok_or("a unit's scale is not a whole number")? as u32, unit: read_unit(&u["unit"])? }))
        .collect::<R<_>>()?;
    let accounts = arr(v, "accounts")?
        .iter()
        .map(|a| Ok(Account { name: text(a, "name")?, params: texts(a, "params")?, unit: text(a, "unit")?, outside: flag(a, "outside")?, lower: bound(&a["lower"])?, upper: bound(&a["upper"])? }))
        .collect::<R<_>>()?;
    let transfers = arr(v, "transfers")?
        .iter()
        .map(|t| {
            let params = arr(t, "params")?.iter().map(|p| Ok(TransferParam { name: text(p, "name")?, unit: opt_text(p, "unit")? })).collect::<R<_>>()?;
            let pending = match &t["pending"] {
                Value::Null => None,
                Value::String(s) if s == "never" => Some(Expiry::Never),
                x => Some(Expiry::After(x["after"].as_u64().ok_or_else(|| format!("{x} is not an expiry"))?)),
            };
            let moves = arr(t, "moves")?
                .iter()
                .map(|m| {
                    let amount = match (&m["amount"]["param"], &m["amount"]["literal"]) {
                        (Value::String(p), _) => MoveAmount::Param(p.clone()),
                        (_, Value::Null) => return Err(format!("{} is not an amount", m["amount"])),
                        (_, l) => MoveAmount::Literal(read_int(l)?),
                    };
                    Ok(Move { amount, from: at(&m["from"])?, to: at(&m["to"])? })
                })
                .collect::<R<_>>()?;
            let refusals = arr(t, "refusals")?
                .iter()
                .map(|r| match r.as_array().map(|a| a.as_slice()) {
                    Some([Value::String(op), rs]) => Ok((op.clone(), rs.as_array().ok_or("the reasons are not a list")?.iter().map(|x| x.as_str().map(String::from).ok_or("a reason is not text".to_string())).collect::<R<_>>()?)),
                    _ => Err(format!("{r} is not an operation and its reasons")),
                })
                .collect::<R<_>>()?;
            let machine = match &t["machine"] {
                Value::Null => None,
                x => Some(read_machine(x)?),
            };
            Ok(Transfer { name: text(t, "name")?, params, key: texts(t, "key")?, pending, moves, refusals, machine })
        })
        .collect::<R<_>>()?;
    Ok(BookFacts {
        name: text(v, "name")?,
        version: v["version"].as_u64().ok_or("the book's version is not a whole number")? as u32,
        sha256: text(v, "sha256")?,
        units,
        accounts,
        transfers,
        typescript: read_client(&v["typescript"])?,
        python: read_client(&v["python"])?,
        go: read_client(&v["go"])?,
    })
}
