//! `sakai vectors` (DESIGN 11.4): what the mappings of the anticorruption layers say, one line for
//! each value of each upstream enum a map maps, as JSON Lines. sakai does not read the code of a
//! layer (DESIGN 1.7); it gives the code's tests the values to hold it to, in the form the vectors
//! of rulec, koyomi, sekisho and kaname are in: the test reads a line, gives the code the upstream
//! value (and its number, for a `.proto`), and holds what the code answers to `to` or `refuse`.
//!
//! A value the map says nothing of is a line too, with `unmapped`: the value that says nothing is
//! set (`*_UNSPECIFIED`) is the usual one, and a layer's code is not silently let off it. Where the
//! mapping goes to a rule's enum, the line says which value of the rule's, and, when that enum is
//! the only input of the rule, what rulec answers for it, asked through the port of rules as any
//! other language is; sakai writes nothing of the rule's answer itself.

use crate::ast::{Role, Target, ValueTo};
use crate::check::Checked;
use crate::elements::At;
use crate::mapping;
use crate::model::RelK;
use crate::naming::Tool;
use crate::suite::Suite;
use ritsu_ports::{ColumnType, RuleError, Value};
use serde_json::{Value as Json, json};

/// The lines, in the order of the map: each context, each of its `upstream` lines, each mapped enum,
/// each value in the order of the upstream's enum.
pub fn lines(c: &Checked, suite: &Suite) -> Vec<Json> {
    let m = &c.model;
    let mut out = Vec::new();
    for (ci, x) in m.contexts.iter().enumerate() {
        for (ri, r) in x.rels.iter().enumerate() {
            let RelK::Upstream { roles, through, layer, enums, .. } = &r.kind else { continue };
            let y = &m.contexts[r.partner];
            // a conformist keeps the upstream's model and maps nothing
            if roles.iter().any(|(role, _)| *role == Role::Conformist) {
                continue;
            }
            for (ei, em) in enums.iter().enumerate() {
                let Some(from) = c.elements.get(At::From(ci, ri, ei)) else { continue };
                let Some(src) = mapping::source(&c.protos, &c.contracts, from) else { continue };
                let numbers = mapping::numbers(&c.protos, from);
                // where the mapping goes: a rule's enum, another enum of a document, or a name
                let to_name = c.elements.get(At::To(ci, ri, ei));
                let rule: Option<(String, String)> = match (&em.target, to_name) {
                    (Target::Element(_), Some(n)) if n.tool == Tool::Rulec => n.items.first().map(|(_, name)| (n.path.clone(), name.clone())),
                    _ => None,
                };
                let target_enum = match (&em.target, to_name) {
                    (Target::Name(n, _), _) => n.clone(),
                    (Target::Element(_), Some(n)) if rule.is_some() => n.items.first().map(|(_, name)| name.clone()).unwrap_or_default(),
                    (Target::Element(_), Some(n)) => mapping::source(&c.protos, &c.contracts, n).map(|s| s.full).unwrap_or_else(|| n.text()),
                    _ => String::new(),
                };
                // a rule that takes the upstream's enum in with `import proto` is the mapping, for the values
                // the lines leave out
                let imported = match (&rule, &src.proto) {
                    (Some((r, name)), Some((pf, pfull))) => mapping::rule_import(&m.root, &c.read, r, name, pf, pfull),
                    _ => None,
                };
                // the package the enum is in, of those the relationship goes through
                let via = through.iter().map(|(t, _)| t.as_str()).find(|t| src.full.starts_with(&format!("{t}."))).or(through.first().map(|(t, _)| t.as_str())).unwrap_or("");
                for (vname, _, unset) in &src.values {
                    let mut line = json!({
                        "context": x.name,
                        "upstream": y.name,
                        "through": via,
                        "layer": layer.iter().map(|l| l.path.clone()).collect::<Vec<_>>(),
                        "enum": em.from,
                        "value": vname,
                    });
                    if let Some(n) = numbers.as_ref().and_then(|ns| ns.iter().find(|(k, _)| k == vname)) {
                        line["number"] = json!(n.1);
                    }
                    let written = em.values.iter().find(|v| v.from == *vname);
                    let rule_value = match written.map(|v| &v.to) {
                        Some(ValueTo::Value(to, _)) => Some(to.clone()),
                        Some(ValueTo::Refuse(why, _)) => {
                            line["refuse"] = json!(why.as_ref().map(|s| s.value.clone()));
                            None
                        }
                        None => imported.as_ref().and_then(|taken| taken.iter().find(|(wire, _)| wire == vname).map(|(_, rv)| rv.clone())),
                    };
                    match (&rule_value, written) {
                        (Some(to), _) => {
                            let mut t = json!({"enum": target_enum, "value": to});
                            if let Some((path, _)) = &rule {
                                t["rule"] = json!(path);
                                if let Some(a) = answers(c, suite, path, &target_enum, to) {
                                    line["answers"] = a;
                                }
                            }
                            line["to"] = t;
                        }
                        (None, Some(_)) => {}
                        (None, None) => line["unmapped"] = json!(true),
                    }
                    if *unset {
                        line["unset"] = json!(true);
                    }
                    out.push(line);
                }
            }
        }
    }
    out
}

/// What rulec answers for the rule when its only input is the enum `en` and it is given the value:
/// the outputs by name. `{"refused": …}` where the rule's door refuses the value; None where the
/// enum is not the only input, or no rule is joined.
fn answers(c: &Checked, suite: &Suite, rule: &str, en: &str, value: &str) -> Option<Json> {
    let facts = c.read.facts.get(rule)?;
    let [input] = facts.inputs.as_slice() else { return None };
    if !facts.elements.is_empty() || !matches!(&input.ty, ColumnType::Enum { name, .. } if name == en) {
        return None;
    }
    let port = suite.rules.as_ref()?;
    let abs = crate::paths::on_disk(&c.model.root, rule);
    Some(match port.eval(&abs, &vec![(input.name.clone(), Value::Enum(value.to_string()))]) {
        Ok(outs) => Json::Object(outs.into_iter().map(|(n, v)| (n, wire(&v))).collect()),
        Err(RuleError::Input(why)) | Err(RuleError::Contradiction(why)) => json!({"refused": why.get(ritsu_base::text::Lang::En)}),
        Err(RuleError::Unread(_)) => return None,
    })
}

fn wire(v: &Value) -> Json {
    match v {
        Value::Bool(b) => json!(b),
        Value::Int(n) => json!(n.to_string().parse::<i64>().map(Json::from).unwrap_or_else(|_| Json::String(n.to_string()))),
        Value::Str(s) | Value::Date(s) | Value::Enum(s) => json!(s),
        Value::None => Json::Null,
        Value::List(_) => Json::Null,
    }
}
