//! The facts of a rule as JSON, for the bundle the playground reads (crate::sources::Bundle): what
//! rulec answered through ritsu's port of rules, written down by the tests that record the bundle
//! and read back in the browser, where there is no rulec. The form is dandori's own; it holds every
//! field of `ritsu_ports::RuleFacts`, and a unit as rulec spells it (`money[JPY, incl_tax]`,
//! `rate[step 0.1%]`), which reads back as the same unit.

use ritsu_ports::{Axis, Call, CallEnum, Column, ColumnType, Connect, EnumValue, Machine, MachineRow, Param, Precondition, RuleEnum, RuleFacts, WireEnum, WireField};
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

/// The facts as the bundle holds them.
pub fn facts_to_json(f: &RuleFacts) -> Value {
    let machine = f.machine.as_ref().map(|m| {
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
    });
    let preconditions: Vec<Value> = f
        .preconditions
        .iter()
        .map(|p| match p {
            Precondition::Relation { left, op, right } => json!({ "kind": "constraint", "left": left, "op": op, "right": right }),
            Precondition::Sum { name, over, of, max } => json!({ "kind": "sum", "name": name, "over": over, "of": of, "max": int(*max) }),
            Precondition::Length { sequence, max } => json!({ "kind": "length", "sequence": sequence, "max": int(*max) }),
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
