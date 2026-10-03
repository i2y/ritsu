//! What dandori reads of a rule: the facts rulec knows of it, handed over as types through ritsu's
//! port of rules (`ritsu_ports::Rules`, ritsu's DESIGN 3.2). dandori does not hold rulec; whoever
//! runs it hands it the port (crate::sources): `ritsu dandori` hands it rulec itself, the
//! playground the facts it recorded, and the dandori binary of this crate one that reads no rule.
//!
//! - the inputs and the outputs: their names, ASCII aliases, types with their units (a rate with
//!   the step its integer counts), and their ranges as the integers that go on the wire
//! - the enums, and for a machine the deciding table's axes, each row's accepted coordinates, what
//!   it writes and where it goes, and the inputs a case holds
//! - the preconditions, the rule's Connect service, and how the generated TypeScript, Python and Go
//!   are called

use crate::diag::Text;
use ritsu_ports::{Call, ColumnType, Precondition, RuleFacts, Said};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum RType {
    Bool,
    Str,
    Enum(String),
    /// A number with its unit as dandori spells it (`money[円, incl_tax]`, `rate[step 1%]`)
    Num { unit: String, min: Option<i64>, max: Option<i64> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub name: String,
    pub alias: String,
    pub ty: RType,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub column: String,
    pub coords: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
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
    /// The axis that is the state; None when the deciding table does not read the state, and every
    /// state goes by the same rows.
    pub state_axis: Option<usize>,
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
    /// what the generated code holds a caller to beyond the shape of each input (rulec's §15.116);
    /// read, and not yet checked at the call (ritsu's DESIGN 7.4)
    pub preconditions: Vec<Precondition>,
    /// the rule's Connect service, as rulec says it: what `connect` calls (`connect_shape`)
    pub connect: Option<ritsu_ports::Connect>,
    /// how the generated code is called in the three languages the code dandori writes calls it from
    pub typescript: Call,
    pub python: Call,
    pub go: Call,
    /// the input that is the list a rule walks (`elements 明細(lines)` in the rule), whose
    /// elements dandori has no type for: such a rule is not one dandori calls (lower, E005)
    pub walks: Option<String>,
    /// the inputs and the outputs that may be `none` (`T?`), which the code dandori writes around
    /// the rule's own neither passes nor reads yet: such a rule is not one dandori calls either
    pub optional: Vec<String>,
}

/// What rulec knows of the rule, read through the port the program that runs dandori handed it; or
/// what rulec (or the port) says instead.
pub fn load(path: &Path) -> Result<RuleInfo, Vec<Said>> {
    crate::sources::rule(path).map(|f| RuleInfo::from_facts(path, f))
}

impl RuleInfo {
    /// The rule as dandori holds it, from the facts the port hands over.
    pub fn from_facts(path: &Path, f: RuleFacts) -> RuleInfo {
        let enums: Vec<(String, Vec<String>)> = f.enums.iter().map(|e| (e.name.clone(), e.values.iter().map(|v| v.name.clone()).collect())).collect();
        let column = |c: &ritsu_ports::Column| Column { name: c.name.clone(), alias: c.alias.clone(), ty: rtype(&c.ty) };
        RuleInfo {
            rule: f.rule,
            version: f.version,
            path: path.to_path_buf(),
            sha256: f.sha256,
            inputs: f.inputs.iter().map(column).collect(),
            outputs: f.outputs.iter().map(column).collect(),
            enums,
            machine: f.machine.map(machine),
            preconditions: f.preconditions,
            connect: f.connect,
            typescript: f.typescript,
            python: f.python,
            go: f.go,
            walks: f.elements.map(|(name, _, _)| name),
            optional: f.inputs.iter().chain(&f.outputs).filter(|c| matches!(c.ty, ColumnType::Opt(_))).map(|c| c.name.clone()).collect(),
        }
    }

    pub fn enum_values(&self, name: &str) -> Option<&Vec<String>> {
        self.enums.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
}

/// A column's type as dandori types it. A day travels as the string `YYYY-MM-DD`. A value that may be
/// `none` is typed as the value (`RuleInfo::optional` names it, and lower refuses the rule).
fn rtype(t: &ColumnType) -> RType {
    match t {
        ColumnType::Bool => RType::Bool,
        ColumnType::Str | ColumnType::Date => RType::Str,
        ColumnType::Enum(e) => RType::Enum(e.clone()),
        ColumnType::Opt(inner) => rtype(inner),
        ColumnType::Num { written, unit, min, max } => {
            // a rate by how many of its steps make the whole, as a `.flow` spells it; another unit as rulec writes it
            let spelled = match unit.as_ref().and_then(|u| u.step.filter(|_| written == "rate")) {
                Some(step) if step.num == 1 && step.den > 0 => crate::model::rate_unit(step.den as u64),
                _ => normalize_unit(written),
            };
            RType::Num { unit: spelled, min: min.and_then(|v| i64::try_from(v).ok()), max: max.and_then(|v| i64::try_from(v).ok()) }
        }
    }
}

fn machine(m: ritsu_ports::Machine) -> Machine {
    Machine {
        name: m.name,
        carry_in: m.carry_in,
        carry_out: m.carry_out,
        state_enum: m.state_enum,
        states: m.states,
        initial: m.initial,
        finals: m.finals,
        held: m.held,
        policy: m.policy,
        axes: m.axes.into_iter().map(|a| Axis { column: a.column, coords: a.coords }).collect(),
        state_axis: m.state_axis,
        decides: m.decides,
        rows: m.rows.into_iter().map(|r| MRow { row: r.row, accepts: r.accepts, to: r.to, produces: r.produces }).collect(),
        memo: Memo::default(),
    }
}

/// What a rule's Connect service is to the code that calls it: the field of the request or the
/// response, by the rule's name, its JSON key, what it holds, and whether the rule may leave it out.
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

/// What rulec says of a rule's Connect service (rulec's §15.112): the path to POST to, and the
/// fields of the request and the response.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectShape {
    /// `/rulec.urgency.v1.UrgencyService/Decide`
    pub path: String,
    pub request: Vec<WireField>,
    pub response: Vec<WireField>,
}

/// The values of one enum on the wire: value 0's `.proto` name, and each value of the rule's with
/// its `.proto` name.
type Named = (String, Vec<(String, String)>);

/// What the service calls the values of each enum that crosses the wire, by the rule's name for it.
/// Every value of the rule's has its `.proto` name there, and so has value 0: `unset` names it when
/// it says that the field is not set, and else it is the value of the rule's whose number is 0.
fn named_enums(c: &ritsu_ports::Connect, py: &Call) -> Result<BTreeMap<String, Named>, Text> {
    let mut out = BTreeMap::new();
    for e in &c.enums {
        let name = &e.name;
        let zero = match &e.unset {
            Some(unset) => unset.clone(),
            None => e.values.iter().find(|(_, _, n)| *n == 0).map(|(_, proto, _)| proto.clone()).ok_or_else(|| {
                tr!(
                    "rulec が、規則のサービスが列挙 `{name}` の 0 番の値を何と呼ぶかを言いません。サービスがレスポンスから省いたフィールドは、その値になります",
                    "rulec does not say what the rule's service calls value 0 of the enum `{name}`, which a field it leaves out of its answer holds"
                )
            })?,
        };
        let theirs = py.enums.iter().find(|x| x.name == *name);
        for (v, _) in theirs.map(|x| x.values.as_slice()).unwrap_or_default() {
            if !e.values.iter().any(|(n, _, _)| n == v) {
                return Err(tr!("rulec が、規則のサービスが列挙 `{name}` の値 `{v}` を何と呼ぶかを言いません", "rulec does not say what the rule's service calls the value `{v}` of the enum `{name}`"));
            }
        }
        out.insert(name.clone(), (zero, e.values.iter().map(|(n, proto, _)| (n.clone(), proto.clone())).collect()));
    }
    Ok(out)
}

/// The Connect service of a rule, as rulec says it. Err, in words that say why `connect` cannot
/// call it, when rulec says nothing of one or says it in a way dandori does not read: field names
/// other than lowerCamelCase, 64-bit integers other than as strings, a type it has no name for.
///
/// The names the service gives the values of an enum are rulec's (`connect.enums` in `rulec api`):
/// rulec held the rule's enum to the contract it takes it from, so the names are the contract's
/// even where it gives its values no prefix and puts a value of its own at 0 (DESIGN 1.13).
pub fn connect_shape(info: &RuleInfo) -> Result<ConnectShape, Text> {
    let Some(c) = info.connect.as_ref().filter(|c| !c.path.is_empty()) else {
        return Err(tr!("rulec が規則の Connect のサービスについて何も言わないので、`connect` で呼べません", "rulec says nothing of the rule's Connect service, which `connect` calls"));
    };
    if c.json_names != "lowerCamelCase" {
        let n = &c.json_names;
        return Err(tr!(
            "規則のサービスは、フィールド名を `{n}` で書きます。dandori が読めるのは lowerCamelCase だけなので、`connect` で呼べません",
            "the rule's service writes its field names as `{n}`, and dandori reads only lowerCamelCase, so `connect` cannot call it"
        ));
    }
    if c.json_int64 != "string" {
        let n = &c.json_int64;
        return Err(tr!(
            "規則のサービスは、64 ビットの整数を `{n}` で書きます。dandori が読めるのは文字列だけなので、`connect` で呼べません",
            "the rule's service writes 64-bit integers as `{n}`, and dandori reads only strings, so `connect` cannot call it"
        ));
    }
    let named = named_enums(c, &info.python)?;
    let field = |f: &ritsu_ports::WireField| -> Result<WireField, Text> {
        let (name, ty) = (&f.name, &f.ty);
        let kind = match ty.as_str() {
            "bool" => WireKind::Bool,
            "string" => WireKind::Str,
            "int32" | "int64" | "uint32" | "uint64" | "sint32" | "sint64" | "fixed32" | "fixed64" | "sfixed32" | "sfixed64" => WireKind::Int,
            // an enum, by the names the service gives its values
            _ => {
                let (zero, values) = f.enumeration.as_ref().and_then(|e| named.get(e)).cloned().ok_or_else(|| {
                    tr!("規則のサービスのフィールド `{name}` の型 `{ty}` を dandori は読めないので、`connect` で呼べません", "the field `{name}` of the rule's service is of the type `{ty}`, which dandori does not read, so `connect` cannot call it")
                })?;
                WireKind::Enum { zero, values }
            }
        };
        Ok(WireField { json: crate::proto::json_name(&f.field), name: name.clone(), kind, optional: f.optional })
    };
    let fields = |list: &[ritsu_ports::WireField]| -> Result<Vec<WireField>, Text> { list.iter().map(field).collect() };
    Ok(ConnectShape { path: c.path.clone(), request: fields(&c.request)?, response: fields(&c.response)? })
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

    /// Whether `a` is the state's axis.
    pub fn is_state_axis(&self, a: usize) -> bool {
        self.state_axis == Some(a)
    }

    /// The axes, other than the state's, on which `value` is a coordinate.
    pub fn axes_with_value(&self, value: &str) -> Vec<usize> {
        (0..self.axes.len()).filter(|&a| !self.is_state_axis(a) && self.axes[a].coords.iter().any(|c| c == value)).collect()
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
        let mut pinned: Vec<Option<usize>> = vec![None; self.axes.len()];
        // a table that reads the state takes only its rows for the state; one that does not, every row
        if let Some(sa) = self.state_axis {
            match self.axes.get(sa).and_then(|a| a.coords.iter().position(|c| *c == self.states[state])) {
                Some(c) => pinned[sa] = Some(c),
                None => return vec![],
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ritsu_ports::{CallEnum, Connect, Param, WireEnum};

    fn call(params: Vec<(&str, &str)>, enums: Vec<CallEnum>) -> Call {
        Call {
            module: "account_fee".into(),
            function: "account_fee".into(),
            input_type: String::new(),
            params: params.into_iter().map(|(n, a)| Param { name: n.into(), alias: a.into(), ty: String::new(), optional: false }).collect(),
            outputs: vec![],
            enums,
        }
    }

    /// The account fee of tests/fixtures/rules, whose enum is a contract's (`import proto
    /// "account.proto" Status`) that gives its values no prefix and puts a value of its own at 0
    /// (`enum Status { ACTIVE = 0; CLOSED = 1; }`): the facts `connect` reads.
    fn account() -> RuleInfo {
        let field = |name: &str, field: &str, ty: &str, en: Option<&str>| ritsu_ports::WireField { name: name.into(), field: field.into(), ty: ty.into(), optional: false, enumeration: en.map(String::from) };
        let python = call(vec![("状態", "state"), ("残高", "balance")], vec![CallEnum { name: "口座の状態".into(), alias: "State".into(), values: vec![("有効".into(), "State.ACTIVE".into()), ("解約".into(), "State.CLOSED".into())] }]);
        RuleInfo {
            rule: "口座手数料".into(),
            version: "v1".into(),
            path: PathBuf::from("account_fee.rule"),
            sha256: String::new(),
            inputs: vec![],
            outputs: vec![],
            enums: vec![],
            machine: None,
            preconditions: vec![],
            connect: Some(Connect {
                path: "/rulec.account_fee.v1.AccountFeeService/Decide".into(),
                json_names: "lowerCamelCase".into(),
                json_int64: "string".into(),
                request: vec![field("状態", "state", "bank.v1.Status", Some("口座の状態")), field("残高", "balance", "int64", None)],
                response: vec![field("手数料", "fee", "int64", None), field("次の状態", "next_state", "bank.v1.Status", Some("口座の状態"))],
                elements: None,
                enums: vec![WireEnum {
                    name: "口座の状態".into(),
                    alias: "bank.v1.Status".into(),
                    contract: Some(("account.proto".into(), "proto/bank/v1/account.proto".into())),
                    unset: None,
                    values: vec![("有効".into(), "ACTIVE".into(), 0), ("解約".into(), "CLOSED".into(), 1)],
                }],
            }),
            typescript: call(vec![], vec![]),
            python,
            go: call(vec![], vec![]),
            walks: None,
            optional: vec![],
        }
    }

    #[test]
    fn the_names_of_an_enums_values_are_rulecs() {
        let shape = connect_shape(&account()).unwrap();
        assert_eq!(shape.path, "/rulec.account_fee.v1.AccountFeeService/Decide");
        let state = WireKind::Enum { zero: "ACTIVE".into(), values: vec![("有効".into(), "ACTIVE".into()), ("解約".into(), "CLOSED".into())] };
        assert_eq!(shape.request[0], WireField { name: "状態".into(), json: "state".into(), kind: state.clone(), optional: false });
        assert_eq!(shape.response[1], WireField { name: "次の状態".into(), json: "nextState".into(), kind: state, optional: false });
        assert_eq!(shape.response[1].kind.zero_value(), Some("有効"));
        assert_eq!(shape.response[0].kind, WireKind::Int);

        // value 0 that says the field is not set is `unset`, and is no value of the rule's
        let mut tier = account();
        tier.connect.as_mut().unwrap().enums[0] = WireEnum {
            name: "口座の状態".into(),
            alias: "shop.v1.MemberTier".into(),
            contract: None,
            unset: Some("MEMBER_TIER_UNSPECIFIED".into()),
            values: vec![("有効".into(), "MEMBER_TIER_BASIC".into(), 1), ("解約".into(), "MEMBER_TIER_GOLD".into(), 2)],
        };
        let shape = connect_shape(&tier).unwrap();
        assert_eq!(shape.request[0].kind, WireKind::Enum { zero: "MEMBER_TIER_UNSPECIFIED".into(), values: vec![("有効".into(), "MEMBER_TIER_BASIC".into()), ("解約".into(), "MEMBER_TIER_GOLD".into())] });
        assert_eq!(shape.request[0].kind.zero_value(), None);

        // whether a field may be left out is the field's own word
        let mut optional = account();
        optional.connect.as_mut().unwrap().response[0].optional = true;
        assert!(connect_shape(&optional).unwrap().response[0].optional);
    }

    #[test]
    fn an_enum_rulec_does_not_name_every_value_of_is_not_called() {
        // value 0 not named, neither as unset nor as a value
        let mut no_zero = account();
        no_zero.connect.as_mut().unwrap().enums[0].values[0].2 = 2;
        let t = connect_shape(&no_zero).unwrap_err();
        assert!(t.en.contains("value 0 of the enum `口座の状態`"), "{}", t.en);
        assert!(t.ja.contains("列挙 `口座の状態` の 0 番の値"), "{}", t.ja);
        // a value of the rule's without its name on the wire
        let mut short = account();
        short.connect.as_mut().unwrap().enums[0].values.pop();
        assert!(connect_shape(&short).unwrap_err().en.contains("the value `解約` of the enum `口座の状態`"));
        // a field of an enum the service does not name
        let mut other = account();
        other.connect.as_mut().unwrap().request[0].enumeration = Some("ほかの列挙".into());
        assert!(connect_shape(&other).unwrap_err().en.contains("the field `状態` of the rule's service is of the type `bank.v1.Status`"));
    }

    /// A deciding table that does not read the state (rulec's certificate says no `axis`) takes
    /// every row from every state; one that does takes only the rows for the state.
    #[test]
    fn a_table_that_does_not_read_the_state_moves_every_state_alike() {
        let mut m = Machine {
            name: "m".into(),
            carry_in: "状態".into(),
            carry_out: "次の状態".into(),
            state_enum: "状態".into(),
            states: vec!["受付".into(), "確定".into(), "取消".into()],
            initial: 0,
            finals: vec![1, 2],
            held: vec![],
            policy: "first".into(),
            axes: vec![Axis { column: "操作".into(), coords: vec!["確定".into(), "取消".into()] }],
            state_axis: None,
            decides: vec![],
            rows: vec![MRow { row: 1, accepts: vec![vec![0]], to: Some(1), produces: vec![] }, MRow { row: 2, accepts: vec![vec![1]], to: Some(2), produces: vec![] }],
            memo: Memo::default(),
        };
        for st in 0..3 {
            assert_eq!(m.outcomes(st, &[(0, 0)]).iter().map(|o| o.next).collect::<Vec<_>>(), [1], "from {}", m.states[st]);
        }
        assert_eq!(m.axes_with_value("確定"), [0]);
        // the same rows, with the state read as an axis: only the states it lists move
        m.axes.insert(0, Axis { column: "状態".into(), coords: vec!["受付".into()] });
        m.state_axis = Some(0);
        m.rows = vec![MRow { row: 1, accepts: vec![vec![0], vec![0]], to: Some(1), produces: vec![] }];
        m.memo = Memo::default();
        assert_eq!(m.outcomes(0, &[(1, 0)]).len(), 1);
        assert!(m.outcomes(1, &[(1, 0)]).is_empty());
        assert_eq!(m.axes_with_value("受付"), Vec::<usize>::new());
    }
}
