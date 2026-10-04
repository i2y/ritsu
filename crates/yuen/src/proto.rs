//! A `.proto` as yuen reads it (DESIGN 3.4): with ritsu's one reader of `.proto` files
//! (ritsu-proto), the file and every file it imports, each import looked for as dandori and buf
//! look for one. The end of a service, a method, a message, a field, an enum or a value is a
//! text of a fixed shape, without the comments and the spacing, so that rewording a comment
//! marks no link: one member a line, the type of each field by its full name, and with a method
//! and a field every message and enum their types reach, in the order of their full names. An
//! import that is not found is written by its name where its types would be.

use crate::names::Name;
use ritsu_proto::{Label, Message, Opt, Protos, Resolved, Symbol, Type};
use std::collections::BTreeMap;
use std::path::Path;

/// The file and what it imports, read; or the file that does not read, and why.
pub fn load(root: &Path, file: &str) -> Result<Protos, (String, ritsu_base::text::Text)> {
    ritsu_proto::load_from(root, file, &[], &[]).map(|(ps, _)| ps).map_err(|(f, e)| (f, e.message("yuen")))
}

fn opts(os: &[Opt]) -> String {
    if os.is_empty() {
        return String::new();
    }
    let all: Vec<String> = os.iter().map(|o| format!("{} = {}", o.name, o.text)).collect();
    format!(" [{}]", all.join(", "))
}

/// A type as the text writes it: a scalar as it is, a message or an enum by its full name, one
/// that cannot be found as written.
fn type_name(ps: &Protos, file: &str, scope: &str, ty: &Type) -> String {
    match ty {
        Type::Scalar(s) => s.clone(),
        Type::Named(n) => match ps.resolve(file, scope, n) {
            Resolved::Found(s) => s.full,
            Resolved::Known(k) => k,
            Resolved::Unknown | Resolved::Missing => n.clone(),
        },
        Type::Map(k, v) => format!("map<{k}, {}>", type_name(ps, file, scope, v)),
    }
}

fn field_line(ps: &Protos, file: &str, scope: &str, f: &ritsu_proto::Field) -> String {
    let label = match f.label {
        Label::None => "",
        Label::Optional => "optional ",
        Label::Required => "required ",
        Label::Repeated => "repeated ",
    };
    let oneof = f.oneof.as_ref().map(|o| format!("oneof {o} ")).unwrap_or_default();
    format!("{oneof}{label}{} {} = {}{}", type_name(ps, file, scope, &f.ty), f.name, f.number, opts(&f.options))
}

/// A message or an enum, a member a line.
fn symbol_text(ps: &Protos, s: &Symbol) -> String {
    let f = &ps.files[&s.file];
    if s.is_enum {
        let e = f.enumeration(&s.name).expect("a symbol is in its file");
        let mut out = format!("enum {}{}\n", s.full, opts(&e.options));
        for v in &e.values {
            out.push_str(&format!("  {} = {}{}\n", v.name, v.number, opts(&v.options)));
        }
        out
    } else {
        let m = f.message(&s.name).expect("a symbol is in its file");
        message_text(ps, &s.file, &s.full, m)
    }
}

fn message_text(ps: &Protos, file: &str, full: &str, m: &Message) -> String {
    let mut out = format!("message {full}\n");
    for o in &m.options {
        out.push_str(&format!("  option {} = {}\n", o.name, o.text));
    }
    for f in &m.fields {
        out.push_str(&format!("  {}\n", field_line(ps, file, full, f)));
    }
    out
}

/// What the types `start` reach, `start` among them, each once, in the order of their full names.
fn reached(ps: &Protos, start: &[Symbol]) -> String {
    let mut all: BTreeMap<(String, String), Symbol> = BTreeMap::new();
    for s in ps.reach(start) {
        all.insert((s.full.clone(), s.file.clone()), s);
    }
    all.values().map(|s| symbol_text(ps, s)).collect()
}

fn found(ps: &Protos, file: &str, scope: &str, written: &str) -> Option<Symbol> {
    match ps.resolve(file, scope, written) {
        Resolved::Found(s) => Some(s),
        _ => None,
    }
}

fn method_text(ps: &Protos, file: &str, service: &str, m: &ritsu_proto::Method) -> (String, Vec<Symbol>) {
    let pkg = &ps.files[file].package;
    let ty = |w: &str| match ps.resolve(file, pkg, w) {
        Resolved::Found(s) => s.full,
        Resolved::Known(k) => k,
        _ => w.to_string(),
    };
    let stream = |b: bool| if b { "stream " } else { "" };
    let mut out = format!("rpc {}.{}({}{}) returns ({}{})\n", ps.files[file].full(service), m.name, stream(m.client_streaming), ty(&m.input), stream(m.server_streaming), ty(&m.output));
    for o in &m.options {
        out.push_str(&format!("  option {} = {}\n", o.name, o.text));
    }
    let types = [&m.input, &m.output].into_iter().filter_map(|w| found(ps, file, pkg, w)).collect();
    (out, types)
}

/// Why a naming of a `.proto` has no end: the element is not in the file.
pub struct NotThere;

/// The end of what a naming of a `.proto` names (DESIGN 3.4), the file read and its imports in
/// `ps`.
pub fn end_text(ps: &Protos, n: &Name) -> Result<String, NotThere> {
    let file = n.path.as_str();
    let f = ps.files.get(file).ok_or(NotThere)?;
    let pairs: Vec<(&str, &str)> = n.items.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    match pairs.as_slice() {
        [("service", s)] => {
            let svc = f.service(s).ok_or(NotThere)?;
            let mut out = format!("service {}\n", f.full(s));
            for o in &svc.options {
                out.push_str(&format!("  option {} = {}\n", o.name, o.text));
            }
            let mut types = Vec::new();
            for m in &svc.methods {
                let (t, ts) = method_text(ps, file, s, m);
                out.push_str(&t);
                types.extend(ts);
            }
            out.push_str(&reached(ps, &types));
            Ok(out)
        }
        [("service", s), ("method", m)] => {
            let svc = f.service(s).ok_or(NotThere)?;
            let me = svc.methods.iter().find(|x| x.name == *m).ok_or(NotThere)?;
            let (mut out, types) = method_text(ps, file, s, me);
            out.push_str(&reached(ps, &types));
            Ok(out)
        }
        [("message", m)] => {
            let msg = f.message(m).ok_or(NotThere)?;
            Ok(message_text(ps, file, &f.full(m), msg))
        }
        [("message", m), ("field", fl)] => {
            let msg = f.message(m).ok_or(NotThere)?;
            let field = msg.fields.iter().find(|x| x.name == *fl).ok_or(NotThere)?;
            let scope = f.full(m);
            let mut out = format!("{}.{}\n  {}\n", scope, field.name, field_line(ps, file, &scope, field));
            let named = match &field.ty {
                Type::Named(t) => Some(t.clone()),
                Type::Map(_, v) => match v.as_ref() {
                    Type::Named(t) => Some(t.clone()),
                    _ => None,
                },
                Type::Scalar(_) => None,
            };
            let start: Vec<Symbol> = named.and_then(|t| found(ps, file, &scope, &t)).into_iter().collect();
            out.push_str(&reached(ps, &start));
            Ok(out)
        }
        [("enum", e)] => {
            let en = f.enumeration(e).ok_or(NotThere)?;
            Ok(symbol_text(ps, &Symbol { full: f.full(e), file: file.to_string(), name: en.name.clone(), is_enum: true }))
        }
        [("enum", e), ("value", v)] => {
            let en = f.enumeration(e).ok_or(NotThere)?;
            let val = en.values.iter().find(|x| x.name == *v).ok_or(NotThere)?;
            Ok(format!("{}.{} = {}{}\n", f.full(e), val.name, val.number, opts(&val.options)))
        }
        _ => Err(NotThere),
    }
}

/// Every element of a kind in a file, as namings: `service` (each `service`), `method` (each
/// method of each service, or of the one named), `message`, `field`, `enum`, `value`.
pub fn gather(ps: &Protos, n: &Name, kind: &str) -> Vec<Name> {
    let Some(f) = ps.files.get(&n.path) else { return vec![] };
    let file = Name { tool: n.tool, path: n.path.clone(), items: vec![] };
    let under = n.items.first().map(|(_, v)| v.as_str());
    let mut out = Vec::new();
    match kind {
        "service" => out.extend(f.services.iter().map(|s| file.clone().with("service", &s.name))),
        "method" => {
            for s in f.services.iter().filter(|s| under.is_none_or(|u| u == s.name)) {
                out.extend(s.methods.iter().map(|m| file.clone().with("service", &s.name).with("method", &m.name)));
            }
        }
        "message" => out.extend(f.messages.iter().map(|m| file.clone().with("message", &m.name))),
        "field" => {
            for m in f.messages.iter().filter(|m| under.is_none_or(|u| u == m.name)) {
                out.extend(m.fields.iter().map(|x| file.clone().with("message", &m.name).with("field", &x.name)));
            }
        }
        "enum" => out.extend(f.enums.iter().map(|e| file.clone().with("enum", &e.name))),
        "value" => {
            for e in f.enums.iter().filter(|e| under.is_none_or(|u| u == e.name)) {
                out.extend(e.values.iter().map(|v| file.clone().with("enum", &e.name).with("value", &v.name)));
            }
        }
        _ => {}
    }
    out
}
