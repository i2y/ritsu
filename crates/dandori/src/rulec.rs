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
    })
}

/// How many steps of a rate make 100%, from the description rulec gives the rate in its schema:
/// "the rate as a count of 1% steps (100% is 100)", or in Japanese "（100% なら 100）".
fn rate_scale(description: &str) -> Option<u64> {
    let rest = description.split("100% is ").nth(1).or_else(|| description.split("100% なら ").nth(1))?;
    rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok().filter(|n| *n > 0)
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
