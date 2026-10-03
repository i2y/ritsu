//! What dandori reads from rulec. rulec is run as a process and its JSON is read, so
//! dandori depends on rulec's command line and rulec knows nothing of dandori.
//!
//! - `rulec schema`: the name and the JSON type of every input and output
//! - `rulec certificate`: the enums, the column types, and for a machine the table's
//!   axes and each row's accepted coordinates, what it produces and where it goes
//! - `rulec api`: the machine's summary and how the generated TypeScript and Python are called

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum RType {
    Bool,
    Str,
    Enum(String),
    /// A number with its unit as rulec spells it (`money[円, incl_tax]`)
    Num { unit: String, min: Option<i64>, max: Option<i64> },
}

#[derive(Clone, Debug)]
pub struct Column {
    pub name: String,
    pub alias: String,
    pub ty: RType,
}

#[derive(Clone, Debug)]
pub struct Axis {
    pub column: String,
    pub coords: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct MRow {
    pub row: usize,
    pub accepts: Vec<Vec<usize>>,
    /// None: the row keeps the state
    pub to: Option<usize>,
    pub produces: Vec<Option<String>>,
}

#[derive(Clone, Debug, Default)]
pub struct Memo(pub std::sync::Arc<std::sync::Mutex<std::collections::HashMap<(usize, Vec<(usize, usize)>), Vec<Outcome>>>>);

#[derive(Clone, Debug)]
pub struct Machine {
    pub name: String,
    pub carry_in: String,
    pub carry_out: String,
    pub state_enum: String,
    pub states: Vec<String>,
    pub initial: usize,
    pub finals: Vec<usize>,
    pub held: Vec<String>,
    pub policy: String,
    pub axes: Vec<Axis>,
    pub state_axis: usize,
    pub decides: Vec<String>,
    pub rows: Vec<MRow>,
    pub memo: Memo,
}

/// One way a call can come out: the row that answered, the state it leads to, and the
/// values the row writes (in the order of `Machine::decides`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Outcome {
    pub row: usize,
    pub next: usize,
    pub produces: Vec<Option<String>>,
}

#[derive(Clone, Debug)]
pub struct RuleInfo {
    pub rule: String,
    pub version: String,
    pub path: PathBuf,
    pub sha256: String,
    pub inputs: Vec<Column>,
    pub outputs: Vec<Column>,
    pub enums: Vec<(String, Vec<String>)>,
    pub machine: Option<Machine>,
    pub preconditions: Value,
    pub api: Value,
    /// the input that is the list a rule walks (`elements 明細(lines)` in the rule), whose
    /// elements dandori has no type for: such a rule is not one dandori calls (lower, E005)
    pub walks: Option<String>,
}

/// What `rulec <cmd>` prints for the rule, from the disk and a rulec process, or from what the
/// playground carries (crate::sources).
fn run(cmd: &str, path: &Path) -> Result<Value, String> {
    crate::sources::rulec(cmd, path)
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

pub fn load(path: &Path) -> Result<RuleInfo, String> {
    let schema = run("schema", path)?;
    let cert = run("certificate", path)?;
    let api = run("api", path)?;

    let enums: Vec<(String, Vec<String>)> = cert["enums"]
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_array().map(|a| a.iter().map(s).collect()).unwrap_or_default())).collect())
        .unwrap_or_default();
    let types: BTreeMap<String, String> =
        cert["types"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), s(v))).collect()).unwrap_or_default();

    let rtype = |name: &str, sch: &Value| -> RType {
        let text = types.get(name).cloned().unwrap_or_default();
        if enums.iter().any(|(n, _)| *n == text) {
            return RType::Enum(text);
        }
        match sch["type"].as_str() {
            Some("boolean") => RType::Bool,
            Some("integer") | Some("number") => {
                // a rate travels as a count of its steps, and the schema says how many make 100%
                let unit = match (text.as_str(), rate_scale(sch["description"].as_str().unwrap_or(""))) {
                    ("rate", Some(per)) => crate::model::rate_unit(per),
                    _ => normalize_unit(&text),
                };
                RType::Num { unit, min: sch["minimum"].as_i64(), max: sch["maximum"].as_i64() }
            }
            _ => RType::Str,
        }
    };

    // The order and the ASCII aliases come from `api`; the types from the certificate and the schema.
    let ts = &api["typescript"];
    let cols = |list: &Value, side: &str| -> Vec<Column> {
        list.as_array()
            .map(|a| {
                a.iter()
                    .map(|p| {
                        let name = s(&p["name"]);
                        let sch = &schema["properties"][side]["properties"][&name];
                        Column { alias: s(&p["alias"]), ty: rtype(&name, sch), name }
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let inputs = cols(&ts["params"], "in");
    let outputs = cols(&ts["outputs"], "out");
    // a rule that walks a list has it among its inputs, with the fields of one element
    let walks = ts["params"].as_array().and_then(|a| a.iter().find(|p| p.get("elements").is_some())).map(|p| s(&p["name"]));

    let machine = if api["machine"].is_object() { Some(machine(&api["machine"], &cert)?) } else { None };

    Ok(RuleInfo {
        rule: s(&api["rule"]),
        version: s(&api["version"]),
        path: path.to_path_buf(),
        sha256: s(&api["source_sha256"]),
        inputs,
        outputs,
        enums,
        machine,
        preconditions: api["preconditions"].clone(),
        api,
        walks,
    })
}

/// How many steps of a rate make 100%, from the description rulec gives the rate in its schema:
/// "the rate as a count of 1% steps (100% is 100)", or in Japanese "（100% なら 100）".
fn rate_scale(description: &str) -> Option<u64> {
    let rest = description.split("100% is ").nth(1).or_else(|| description.split("100% なら ").nth(1))?;
    rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok().filter(|n| *n > 0)
}

/// `MemberTier` → `MEMBER_TIER`: how a rulec that does not say the names of its service's enum
/// values (`connect.enums`, which rulec 0.22.0 does) spells an enum of the rule's own in the names
/// of its values (`MEMBER_TIER_GOLD`). Its rule is that rulec's: a capital that is not the first
/// letter and does not follow an underscore starts a new word.
pub fn upper_snake(camel: &str) -> String {
    let mut out = String::new();
    for (i, ch) in camel.chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 && !out.ends_with('_') {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

/// A field of a rule's Connect request or response, as the `.proto` rulec writes has it.
#[derive(Clone, Debug, PartialEq)]
pub struct WireField {
    /// the name the rule gives it: the key in the arguments of a call and in the record it answers
    pub name: String,
    /// its key in protobuf's JSON: the field's name in lowerCamelCase
    pub json: String,
    pub kind: WireKind,
    /// the rule may leave it out
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WireKind {
    Bool,
    /// a number: `int64`, which protobuf's JSON writes as a decimal string
    Int,
    Str,
    /// an enum: each of its values as the rule names it and as the `.proto` does (`next_day`,
    /// `CARRIER_NEXTDAY`), and the `.proto`'s name for its value 0, which a message holds when the
    /// field is left out. That is `CARRIER_UNSPECIFIED`, which is no value of the rule's, unless the
    /// enum is a contract's that puts a value of its own at 0 (`ACTIVE`): then it is among `values`.
    Enum { zero: String, values: Vec<(String, String)> },
}

impl WireKind {
    /// The rule's value at 0, when value 0 of the enum is one of the rule's (`有効` for `ACTIVE = 0`):
    /// what the service leaves out of its answer, and what a field left out is read as. None for an
    /// enum whose value 0 says that it is not set, and for what is not an enum.
    pub fn zero_value(&self) -> Option<&str> {
        match self {
            WireKind::Enum { zero, values } => values.iter().find(|(_, proto)| proto == zero).map(|(rule, _)| rule.as_str()),
            _ => None,
        }
    }
}

/// What `rulec api` says of a rule's Connect service (`connect`, from rulec's DESIGN 15.112): the
/// path to POST to, and the fields of the request and the response.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectShape {
    /// `/rulec.urgency.v1.UrgencyService/Decide`
    pub path: String,
    pub request: Vec<WireField>,
    pub response: Vec<WireField>,
}

/// The values of one enum on the wire, as `connect.enums` names them: value 0's `.proto` name, and
/// each value of the rule's with its `.proto` name.
type Named = (String, Vec<(String, String)>);

/// What `connect.enums` says of each enum that crosses the wire, by the rule's name for it. Every
/// value of the rule's has its `.proto` name there, and so has value 0: `unset` names it when it says
/// that the field is not set, and else it is the value of the rule's whose number is 0.
fn named_enums(enums: &Value, py: &Value) -> Result<BTreeMap<String, Named>, (String, String)> {
    let mut out = BTreeMap::new();
    for e in enums.as_array().into_iter().flatten() {
        let name = s(&e["name"]);
        let values: Vec<(String, String, Option<i64>)> = e["values"].as_array().into_iter().flatten().map(|v| (s(&v["name"]), s(&v["alias"]), v["number"].as_i64())).collect();
        let zero = match e["unset"].as_str() {
            Some(unset) => unset.to_string(),
            None => values.iter().find(|(_, _, n)| *n == Some(0)).map(|(_, proto, _)| proto.clone()).ok_or_else(|| {
                (
                    format!("`rulec api` does not say what the rule's service calls value 0 of the enum `{name}`, which a field it leaves out of its answer holds"),
                    format!("`rulec api` が、規則のサービスが列挙 `{name}` の 0 番の値を何と呼ぶかを言いません。サービスがレスポンスから省いたフィールドは、その値になります"),
                )
            })?,
        };
        let theirs = py["enums"].as_array().and_then(|es| es.iter().find(|x| s(&x["name"]) == name));
        for v in theirs.and_then(|x| x["values"].as_array()).into_iter().flatten().map(|v| s(&v["name"])) {
            if !values.iter().any(|(n, _, _)| *n == v) {
                return Err((
                    format!("`rulec api` does not say what the rule's service calls the value `{v}` of the enum `{name}`"),
                    format!("`rulec api` が、規則のサービスが列挙 `{name}` の値 `{v}` を何と呼ぶかを言いません"),
                ));
            }
        }
        out.insert(name, (zero, values.into_iter().map(|(n, proto, _)| (n, proto)).collect()));
    }
    Ok(out)
}

/// The Connect service of a rule, from the JSON of `rulec api`. Err, in words (English, Japanese)
/// that say why `connect` cannot call it, when `api` says nothing of one (a rulec without it) or says
/// it in a way dandori does not read: field names other than lowerCamelCase, 64-bit integers other
/// than as strings, a type it has no name for.
///
/// The names the service gives the values of an enum are read from `connect.enums`, which rulec
/// 0.22.0 prints. A rulec that does not (0.21.2 and before) leaves them to be built: an enum of the
/// rule's own is spelled as that rulec spells it, its name in capitals and the value's alias
/// (`CARRIER_NEXTDAY`), with value 0 `CARRIER_UNSPECIFIED`. An enum that is a contract's
/// (`import proto`) is not built, since the contract may name its values otherwise and put a value
/// of its own at 0, and a name that is not the contract's is read by the service as value 0
/// (DESIGN 1.13): calling such a rule needs a rulec that says the names. That rulec's output tells
/// the two apart by the type a field has, the `.proto` enum's name, against the rule's alias for
/// the enum, which differ for a contract's.
pub fn connect_shape(api: &Value) -> Result<ConnectShape, (String, String)> {
    let c = &api["connect"];
    let path = s(&c["path"]);
    if !c.is_object() || path.is_empty() {
        return Err(("`rulec api` says nothing of the rule's Connect service, which `connect` calls".into(), "`rulec api` が規則の Connect のサービスについて何も言わないので、`connect` で呼べません".into()));
    }
    if c["json_names"].as_str().is_some_and(|n| n != "lowerCamelCase") {
        let n = s(&c["json_names"]);
        return Err((
            format!("the rule's service writes its field names as `{n}`, and dandori reads only lowerCamelCase, so `connect` cannot call it"),
            format!("規則のサービスは、フィールド名を `{n}` で書きます。dandori が読めるのは lowerCamelCase だけなので、`connect` で呼べません"),
        ));
    }
    if c["json_int64"].as_str().is_some_and(|n| n != "string") {
        let n = s(&c["json_int64"]);
        return Err((
            format!("the rule's service writes 64-bit integers as `{n}`, and dandori reads only strings, so `connect` cannot call it"),
            format!("規則のサービスは、64 ビットの整数を `{n}` で書きます。dandori が読めるのは文字列だけなので、`connect` で呼べません"),
        ));
    }
    let py = &api["python"];
    let named = match c.get("enums") {
        Some(enums) => Some(named_enums(enums, py)?),
        None => None,
    };
    let unread = |name: &str, ty: &str| -> (String, String) {
        (
            format!("the field `{name}` of the rule's service is of the type `{ty}`, which dandori does not read, so `connect` cannot call it"),
            format!("規則のサービスのフィールド `{name}` の型 `{ty}` を dandori は読めないので、`connect` で呼べません"),
        )
    };
    let field = |f: &Value, side: &str| -> Result<WireField, (String, String)> {
        let (name, ty) = (s(&f["name"]), s(&f["type"]));
        // what the rule says of the same name: whether it may be left out, and which enum it is
        let own = py[side].as_array().and_then(|a| a.iter().find(|p| s(&p["name"]) == name));
        let optional = match f["optional"].as_bool() {
            Some(o) => o,
            None => own.is_some_and(|p| p["optional"].as_bool() == Some(true)),
        };
        let kind = match ty.as_str() {
            "bool" => WireKind::Bool,
            "string" => WireKind::Str,
            "int32" | "int64" | "uint32" | "uint64" | "sint32" | "sint64" | "fixed32" | "fixed64" | "sfixed32" | "sfixed64" => WireKind::Int,
            // an enum, by the names `connect.enums` gives its values
            _ if named.is_some() => {
                let enums = named.as_ref().expect("said");
                let (zero, values) = f["enum"].as_str().and_then(|e| enums.get(e)).cloned().ok_or_else(|| unread(&name, &ty))?;
                WireKind::Enum { zero, values }
            }
            // an enum, from a rulec that does not say the names: the `.proto`'s name for the enum is the last
            // part of the type (`Carrier`), and the rule's alias for it the type of the same name in `python`
            _ => {
                let alias = own.map(|p| s(&p["type"])).unwrap_or_default();
                let theirs = py["enums"].as_array().and_then(|es| es.iter().find(|e| s(&e["alias"]) == alias)).ok_or_else(|| unread(&name, &ty))?;
                let last = ty.rsplit('.').next().unwrap_or(&ty);
                if last != alias {
                    let e = s(&theirs["name"]);
                    return Err((
                        format!("the enum `{e}` of the rule is a contract's (`{ty}`), and this rulec's `rulec api` does not say what the rule's service calls its values; `connect` needs rulec 0.22.0 or later, whose `rulec api` says them (`connect.enums`)"),
                        format!("規則の列挙 `{e}` は契約（`{ty}`）から取り込んだものですが、この rulec の `rulec api` は、規則のサービスがその値を何と呼ぶかを言いません。`connect` で呼ぶには、rulec 0.22.0 以降が要ります（`rulec api` の `connect.enums` がそれを言います）"),
                    ));
                }
                let prefix = upper_snake(last);
                WireKind::Enum {
                    zero: format!("{prefix}_UNSPECIFIED"),
                    values: theirs["values"].as_array().into_iter().flatten().map(|v| (s(&v["name"]), format!("{prefix}_{}", s(&v["alias"]).to_uppercase()))).collect(),
                }
            }
        };
        Ok(WireField { json: crate::proto::json_name(&s(&f["field"])), name, kind, optional })
    };
    let fields = |list: &str, side: &str| -> Result<Vec<WireField>, (String, String)> { c[list].as_array().unwrap_or(&vec![]).iter().map(|f| field(f, side)).collect() };
    Ok(ConnectShape { path, request: fields("request_fields", "params")?, response: fields("response_fields", "outputs")? })
}

pub fn normalize_unit(t: &str) -> String {
    // `money[円,incl_tax]` and `money[円, incl_tax]` are the same unit
    let mut out = String::new();
    for part in t.split(',') {
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(part.trim());
    }
    out
}

fn machine(m: &Value, cert: &Value) -> Result<Machine, String> {
    let cm = &cert["machine"];
    let table = s(&cm["table"]);
    let t = cert["tables"]
        .as_array()
        .and_then(|ts| ts.iter().find(|t| s(&t["table"]) == table))
        .ok_or_else(|| format!("the certificate has no table `{table}` for the machine"))?;
    let axes: Vec<Axis> = t["axes"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|a| Axis { column: s(&a["column"]), coords: a["coords"].as_array().map(|c| c.iter().map(s).collect()).unwrap_or_default() })
        .collect();
    let mut to: BTreeMap<usize, Option<usize>> = BTreeMap::new();
    for r in cm["rows"].as_array().unwrap_or(&vec![]) {
        let row = r["row"].as_u64().unwrap_or(0) as usize;
        let dest = if r["stay"].as_bool() == Some(true) { None } else { r["to"].as_u64().map(|x| x as usize) };
        to.insert(row, dest);
    }
    let rows: Vec<MRow> = t["rows"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|r| {
            let row = r["row"].as_u64().unwrap_or(0) as usize;
            MRow {
                row,
                accepts: r["accepts"]
                    .as_array()
                    .map(|a| a.iter().map(|x| x.as_array().map(|y| y.iter().filter_map(|z| z.as_u64().map(|q| q as usize)).collect()).unwrap_or_default()).collect())
                    .unwrap_or_default(),
                to: to.get(&row).cloned().flatten(),
                produces: r["produces"].as_array().map(|a| a.iter().map(|x| x.as_str().map(|q| q.to_string())).collect()).unwrap_or_default(),
            }
        })
        .collect();
    let states: Vec<String> = m["states"].as_array().map(|a| a.iter().map(s).collect()).unwrap_or_default();
    let idx = |name: &str| states.iter().position(|x| x == name);
    Ok(Machine {
        name: s(&m["name"]),
        carry_in: s(&m["carry"]["input"]),
        carry_out: s(&m["carry"]["output"]),
        state_enum: s(&m["carry"]["enum"]),
        initial: idx(&s(&m["initial"])).unwrap_or(0),
        finals: m["final"].as_array().map(|a| a.iter().filter_map(|x| idx(x.as_str().unwrap_or(""))).collect()).unwrap_or_default(),
        held: m["held"].as_array().map(|a| a.iter().map(s).collect()).unwrap_or_default(),
        policy: s(&t["policy"]),
        state_axis: cm["axis"].as_u64().unwrap_or(0) as usize,
        decides: t["decides"].as_array().map(|a| a.iter().map(s).collect()).unwrap_or_default(),
        axes,
        rows,
        states,
        memo: Memo::default(),
    })
}

impl Machine {
    pub fn state_index(&self, name: &str) -> Option<usize> {
        self.states.iter().position(|x| x == name)
    }

    pub fn is_final(&self, st: usize) -> bool {
        self.finals.contains(&st)
    }

    pub fn axis_of(&self, column: &str) -> Option<usize> {
        self.axes.iter().position(|a| a.column == column)
    }

    /// The axes, other than the state's, on which `value` is a coordinate.
    pub fn axes_with_value(&self, value: &str) -> Vec<usize> {
        (0..self.axes.len()).filter(|&a| a != self.state_axis && self.axes[a].coords.iter().any(|c| c == value)).collect()
    }

    /// The ways a call can come out from `state`, with some axes fixed (the event, the held
    /// inputs) and every other axis free. The free axes are walked coordinate by
    /// coordinate, so a row that some earlier row always takes over under `policy first`
    /// is not counted. Past a budget it falls back to every row that accepts the fixed
    /// coordinates, which is wider and therefore still safe for what it is used for.
    pub fn outcomes(&self, state: usize, fixed: &[(usize, usize)]) -> Vec<Outcome> {
        let mut key_fixed = fixed.to_vec();
        key_fixed.sort();
        let key = (state, key_fixed);
        if let Some(v) = self.memo.0.lock().unwrap().get(&key) {
            return v.clone();
        }
        let v = self.outcomes_uncached(state, fixed);
        self.memo.0.lock().unwrap().insert(key, v.clone());
        v
    }

    fn outcomes_uncached(&self, state: usize, fixed: &[(usize, usize)]) -> Vec<Outcome> {
        let state_coord = match self.axes.get(self.state_axis).and_then(|a| a.coords.iter().position(|c| *c == self.states[state])) {
            Some(c) => c,
            None => return vec![],
        };
        let mut pinned: Vec<Option<usize>> = vec![None; self.axes.len()];
        pinned[self.state_axis] = Some(state_coord);
        for (a, c) in fixed {
            pinned[*a] = Some(*c);
        }
        let free: Vec<usize> = (0..self.axes.len()).filter(|a| pinned[*a].is_none()).collect();
        let total: usize = free.iter().map(|a| self.axes[*a].coords.len().max(1)).product();
        let mut found: BTreeMap<usize, Outcome> = BTreeMap::new();
        let outcome_of = |r: &MRow| Outcome { row: r.row, next: r.to.unwrap_or(state), produces: r.produces.clone() };
        if total > 200_000 {
            for r in &self.rows {
                let ok = (0..self.axes.len()).all(|a| match pinned[a] {
                    Some(c) => r.accepts.get(a).map(|v| v.contains(&c)).unwrap_or(false),
                    None => true,
                });
                if ok {
                    found.insert(r.row, outcome_of(r));
                }
            }
            return found.into_values().collect();
        }
        let mut point: Vec<usize> = pinned.iter().map(|p| p.unwrap_or(0)).collect();
        let mut counter = vec![0usize; free.len()];
        loop {
            for (k, a) in free.iter().enumerate() {
                point[*a] = counter[k];
            }
            let mut hits = self.rows.iter().filter(|r| (0..self.axes.len()).all(|a| r.accepts.get(a).map(|v| v.contains(&point[a])).unwrap_or(false)));
            if self.policy == "first" || self.policy == "unique" {
                if let Some(r) = hits.next() {
                    found.insert(r.row, outcome_of(r));
                }
            } else {
                for r in hits {
                    found.insert(r.row, outcome_of(r));
                }
            }
            // next point
            let mut k = 0;
            loop {
                if k == free.len() {
                    return found.into_values().collect();
                }
                counter[k] += 1;
                if counter[k] < self.axes[free[k]].coords.len() {
                    break;
                }
                counter[k] = 0;
                k += 1;
            }
        }
    }
}

impl RuleInfo {
    pub fn enum_values(&self, name: &str) -> Option<&Vec<String>> {
        self.enums.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// What `rulec api` says of the account fee of tests/fixtures/rules, whose enum is a contract's
    /// (`import proto "account.proto" Status`) that gives its values no prefix and puts a value of
    /// its own at 0 (`enum Status { ACTIVE = 0; CLOSED = 1; }`): the keys dandori reads, as a rulec
    /// that says the names of its service's enum values prints them.
    fn account() -> Value {
        json!({
            "connect": {
                "path": "/rulec.account_fee.v1.AccountFeeService/Decide",
                "json_names": "lowerCamelCase",
                "json_int64": "string",
                "request_fields": [
                    { "name": "状態", "field": "state", "type": "bank.v1.Status", "optional": false, "enum": "口座の状態" },
                    { "name": "残高", "field": "balance", "type": "int64", "optional": false },
                ],
                "element_fields": null,
                "response_fields": [
                    { "name": "手数料", "field": "fee", "type": "int64", "optional": false },
                    { "name": "次の状態", "field": "next_state", "type": "bank.v1.Status", "optional": false, "enum": "口座の状態" },
                ],
                "enums": [{
                    "name": "口座の状態", "alias": "bank.v1.Status",
                    "contract": { "file": "account.proto", "proto": "proto/bank/v1/account.proto" },
                    "unset": null,
                    "values": [{ "name": "有効", "alias": "ACTIVE", "number": 0 }, { "name": "解約", "alias": "CLOSED", "number": 1 }],
                }],
            },
            "python": {
                "params": [{ "name": "状態", "alias": "state", "type": "State", "optional": false }, { "name": "残高", "alias": "balance", "type": "Yen", "optional": false }],
                "outputs": [{ "name": "手数料", "alias": "fee", "type": "Yen", "optional": false }, { "name": "次の状態", "alias": "next_state", "type": "State", "optional": false }],
                "enums": [{ "name": "口座の状態", "alias": "State", "values": [{ "name": "有効", "alias": "ACTIVE" }, { "name": "解約", "alias": "CLOSED" }] }],
            },
        })
    }

    /// The same rule as a rulec that does not say the names prints it: no `enums`, no `optional` and
    /// no `enum` on a field. `ty` is the enum's type, which it prints with the package
    /// (`bank.v1.Status`) when it finds the `.proto` and without it when it does not (it looks where
    /// it is run, and `rulec gen` where the rule is).
    fn account_unnamed(ty: &str) -> Value {
        let mut v = account();
        let c = v["connect"].as_object_mut().unwrap();
        c.remove("enums");
        c.remove("element_fields");
        for list in ["request_fields", "response_fields"] {
            for f in c[list].as_array_mut().unwrap() {
                let f = f.as_object_mut().unwrap();
                f.remove("optional");
                if f.remove("enum").is_some() {
                    f.insert("type".into(), json!(ty));
                }
            }
        }
        v
    }

    /// The urgency rule of the examples, whose enum is its own (`enum carrier = standard | next_day`),
    /// as a rulec that does not say the names prints it.
    fn urgency_unnamed() -> Value {
        json!({
            "connect": {
                "path": "/rulec.urgency.v1.UrgencyService/Decide",
                "json_names": "lowerCamelCase",
                "json_int64": "string",
                "request_fields": [{ "name": "member", "field": "member", "type": "bool" }, { "name": "amount", "field": "amount", "type": "int64" }],
                "response_fields": [{ "name": "urgent", "field": "urgent", "type": "bool" }, { "name": "carrier", "field": "carrier", "type": "Carrier" }],
            },
            "python": {
                "params": [{ "name": "member", "alias": "member", "type": "bool", "optional": false }, { "name": "amount", "alias": "amount", "type": "JPYInclTax", "optional": false }],
                "outputs": [{ "name": "urgent", "alias": "urgent", "type": "bool", "optional": false }, { "name": "carrier", "alias": "carrier", "type": "Carrier", "optional": false }],
                "enums": [{ "name": "carrier", "alias": "Carrier", "values": [{ "name": "standard", "alias": "STANDARD" }, { "name": "next_day", "alias": "NEXTDAY" }] }],
            },
        })
    }

    #[test]
    fn the_names_of_an_enums_values_are_read_from_rulec_api() {
        let shape = connect_shape(&account()).unwrap();
        assert_eq!(shape.path, "/rulec.account_fee.v1.AccountFeeService/Decide");
        let state = WireKind::Enum { zero: "ACTIVE".into(), values: vec![("有効".into(), "ACTIVE".into()), ("解約".into(), "CLOSED".into())] };
        assert_eq!(shape.request[0], WireField { name: "状態".into(), json: "state".into(), kind: state.clone(), optional: false });
        assert_eq!(shape.response[1], WireField { name: "次の状態".into(), json: "nextState".into(), kind: state, optional: false });
        assert_eq!(shape.response[1].kind.zero_value(), Some("有効"));
        assert_eq!(shape.response[0].kind, WireKind::Int);

        // value 0 that says the field is not set is `unset`, and is no value of the rule's
        let mut tier = account();
        tier["connect"]["enums"][0] = json!({ "name": "口座の状態", "alias": "shop.v1.MemberTier", "unset": "MEMBER_TIER_UNSPECIFIED", "values": [{ "name": "有効", "alias": "MEMBER_TIER_BASIC", "number": 1 }, { "name": "解約", "alias": "MEMBER_TIER_GOLD", "number": 2 }] });
        let shape = connect_shape(&tier).unwrap();
        assert_eq!(shape.request[0].kind, WireKind::Enum { zero: "MEMBER_TIER_UNSPECIFIED".into(), values: vec![("有効".into(), "MEMBER_TIER_BASIC".into()), ("解約".into(), "MEMBER_TIER_GOLD".into())] });
        assert_eq!(shape.request[0].kind.zero_value(), None);

        // whether a field may be left out is the field's own word
        let mut optional = account();
        optional["connect"]["response_fields"][0]["optional"] = json!(true);
        assert!(connect_shape(&optional).unwrap().response[0].optional);
    }

    #[test]
    fn an_enum_rulec_api_does_not_name_every_value_of_is_not_called() {
        // value 0 not named, neither as unset nor as a value
        let mut no_zero = account();
        no_zero["connect"]["enums"][0]["values"][0]["number"] = json!(2);
        let (en, ja) = connect_shape(&no_zero).unwrap_err();
        assert!(en.contains("value 0 of the enum `口座の状態`"), "{en}");
        assert!(ja.contains("列挙 `口座の状態` の 0 番の値"), "{ja}");
        // a value of the rule's without its name on the wire
        let mut short = account();
        short["connect"]["enums"][0]["values"].as_array_mut().unwrap().pop();
        let (en, _) = connect_shape(&short).unwrap_err();
        assert!(en.contains("the value `解約` of the enum `口座の状態`"), "{en}");
        // a field of an enum `enums` does not have
        let mut other = account();
        other["connect"]["request_fields"][0]["enum"] = json!("ほかの列挙");
        let (en, _) = connect_shape(&other).unwrap_err();
        assert!(en.contains("the field `状態` of the rule's service is of the type `bank.v1.Status`"), "{en}");
    }

    #[test]
    fn a_rulec_that_does_not_name_them_has_a_rules_own_enum_spelled_as_it_spells_it() {
        let shape = connect_shape(&urgency_unnamed()).unwrap();
        assert_eq!(shape.response[1].kind, WireKind::Enum { zero: "CARRIER_UNSPECIFIED".into(), values: vec![("standard".into(), "CARRIER_STANDARD".into()), ("next_day".into(), "CARRIER_NEXTDAY".into())] });
        assert_eq!(shape.request.iter().map(|f| (f.json.as_str(), f.optional)).collect::<Vec<_>>(), [("member", false), ("amount", false)]);
    }

    #[test]
    fn a_rulec_that_does_not_name_them_cannot_have_a_contracts_enum_called() {
        // with the package or without it: the `.proto`'s name for the enum is not the rule's alias for it
        for ty in ["bank.v1.Status", "Status"] {
            let (en, ja) = connect_shape(&account_unnamed(ty)).unwrap_err();
            assert_eq!(
                en,
                format!("the enum `口座の状態` of the rule is a contract's (`{ty}`), and this rulec's `rulec api` does not say what the rule's service calls its values; `connect` needs rulec 0.22.0 or later, whose `rulec api` says them (`connect.enums`)")
            );
            assert_eq!(ja, format!("規則の列挙 `口座の状態` は契約（`{ty}`）から取り込んだものですが、この rulec の `rulec api` は、規則のサービスがその値を何と呼ぶかを言いません。`connect` で呼ぶには、rulec 0.22.0 以降が要ります（`rulec api` の `connect.enums` がそれを言います）"));
        }
    }

    #[test]
    fn upper_snake_is_rulecs_spelling() {
        assert_eq!(upper_snake("Carrier"), "CARRIER");
        assert_eq!(upper_snake("MemberTier"), "MEMBER_TIER");
        assert_eq!(upper_snake("CaptureMethod"), "CAPTURE_METHOD");
    }
}
