//! OpenAPI and AsyncAPI documents and Cedar's files, as yuen reads them (DESIGN 3.6): with ritsu's
//! readers, not through a language's port, as yuen reads a `.proto` (3.4). They are standard
//! formats no language of ritsu owns.
//!
//! An element of a document (`openapi "api/orders.yaml" operation refundOrder`) is found where
//! ritsu-base's [`document`] says a reference lands. Its end is the element's value as the YAML or
//! JSON reader reads it, written as JSON with its keys in order, and after it the value of each
//! place its `$ref`s reach, so that a schema an operation takes changes the operation's end too,
//! and an element of the same document it does not reach changes nothing.
//!
//! A Cedar policy's end is the policy as `cedar format` lays it out, without its comments; an
//! action's or an entity type's, its declaration in the Cedar schema format, with the common
//! types it uses.

use crate::names::{Name, Tool};
use ritsu_base::cedar;
use ritsu_base::document::{self, Documents};
use ritsu_base::json::Json;
use ritsu_base::text::Text;
use ritsu_base::yaml::{self, Node, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// A document and every file its `$ref`s reach, each read once, by its path from the root.
#[derive(Default)]
pub struct Loaded {
    docs: BTreeMap<String, Node>,
}

impl Documents for Loaded {
    fn root(&self, file: &str) -> Option<&Node> {
        self.docs.get(file)
    }
}

/// A file a reference of `openapi`, `asyncapi` or `cedar` names, read.
pub enum Read {
    Doc(Loaded),
    Policies(cedar::PolicySet),
    Schema(cedar::Schema),
}

/// How many files the `$ref`s of one document may reach, so that a reference cannot read a whole
/// disk.
const MAX_FILES: usize = 1_000;

/// Where a text does not read, and why: `<line>:<column>: …`.
fn located(line: usize, col: usize, message: &Text) -> Text {
    tr!("{line}:{col}: {}", "{line}:{col}: {}", message.ja; message.en)
}

/// Read the file a reference names (`file`, from the root), as its tool reads it. An OpenAPI or
/// AsyncAPI document is read with the files its `$ref`s reach that are there and read; one of the
/// other kind is not, for its references to be written with the tool it is.
pub fn load(root: &Path, tool: Tool, file: &str) -> Result<Read, Text> {
    let text = ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(root, file)).map_err(|e| Text::same(e.to_string()))?;
    match tool {
        Tool::Cedar => {
            let at = |e: cedar::Error| located(e.line, e.col, &e.message);
            if file.ends_with(".cedar") {
                cedar::parse_policies(&text).map(Read::Policies).map_err(at)
            } else if file.ends_with(".cedarschema") {
                cedar::parse_schema(&text).map(Read::Schema).map_err(at)
            } else if file.ends_with(".cedarschema.json") {
                cedar::parse_schema_json(&text).map(Read::Schema).map_err(at)
            } else {
                Err(tr!(
                    "Cedar のファイルではありません。yuen が読むのは、ポリシー（.cedar）とスキーマ（.cedarschema、.cedarschema.json）です",
                    "it is not a file of Cedar's; yuen reads policies (.cedar) and schemas (.cedarschema, .cedarschema.json)"
                ))
            }
        }
        _ => {
            let top = yaml::read_file_text(file, &text).map_err(|e| located(e.line, e.col, &e.message))?;
            if let Some(k) = document::kind_of(&top)
                && k != tool
            {
                let (is, w) = (if k == Tool::Openapi { "OpenAPI" } else { "AsyncAPI" }, k.word());
                return Err(tr!("{is} の文書です。`{w} \"…\"` で指してください", "it is an {is} document, named with `{w} \"…\"`"));
            }
            let mut l = Loaded::default();
            l.docs.insert(file.to_string(), top);
            let mut todo = vec![file.to_string()];
            while let Some(f) = todo.pop() {
                let mut rs = Vec::new();
                document::refs_in(&l.docs[&f], "", &mut rs);
                for (_, _, _, w) in rs {
                    let Ok((t, _)) = document::target(&f, &w) else { continue };
                    if l.docs.contains_key(&t) || l.docs.len() >= MAX_FILES {
                        continue;
                    }
                    let Ok(src) = ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(root, &t)) else { continue };
                    if let Ok(node) = yaml::read_file_text(&t, &src) {
                        l.docs.insert(t.clone(), node);
                        todo.push(t);
                    }
                }
            }
            Ok(Read::Doc(l))
        }
    }
}

/// Why a reference has no end in a file that reads.
pub enum Miss {
    /// The file does not hold what it names.
    NotThere,
    /// A schema declares the name in more than one namespace: the namespaces.
    Twice(Vec<String>),
}

/// A value as JSON, the keys of each mapping in the order of their UTF-8 bytes.
fn json(n: &Node) -> Json {
    match &n.value {
        Value::Null => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::Int(i) => Json::Int(*i),
        Value::Float(f) => Json::Frac(f.clone()),
        Value::Str(s) => Json::Str(s.clone()),
        Value::Seq(xs) => Json::Arr(xs.iter().map(json).collect()),
        Value::Map(es) => {
            let mut kv: Vec<(String, Json)> = es.iter().map(|(k, v)| (k.name.clone(), json(v))).collect();
            kv.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            Json::Obj(kv)
        }
    }
}

/// A place and its value: `#<pointer>` (`<file>#<pointer>` in another file than the one the
/// reference names) on a line, and the value as JSON.
fn section(named: &str, file: &str, pointer: &str, mark: &str, node: &Node) -> String {
    let place = if file == named { format!("#{pointer}") } else { format!("{file}#{pointer}") };
    format!("{place}{mark}\n{}\n", json(node).pretty())
}

/// The end of what a reference names in a file that reads (DESIGN 3.6).
pub fn end_text(read: &Read, n: &Name) -> Result<String, Miss> {
    match read {
        Read::Doc(l) => doc_end(l, n).ok_or(Miss::NotThere),
        Read::Policies(set) => {
            let [(k, id)] = n.items.as_slice() else { return Err(Miss::NotThere) };
            let p = set.policies.iter().find(|p| k == "policy" && p.id == *id).ok_or(Miss::NotThere)?;
            let t = cedar::write_policy(p).unwrap_or_else(|_| cedar::policy_to_json(p).pretty());
            Ok(if t.ends_with('\n') { t } else { t + "\n" })
        }
        Read::Schema(s) => schema_end(s, n),
    }
}

fn doc_end(l: &Loaded, n: &Name) -> Option<String> {
    let e = document::find(l, n)?;
    let named = n.path.as_str();
    let (lf, lp, node) = document::locate(l, &e.file, &e.pointer)?;
    if let Some(v) = &e.value {
        let x = node.as_seq()?.iter().find(|x| document::value_name(x) == *v)?;
        return Some(section(named, &lf, &lp, "", x));
    }
    let mark = if e.required == Some(true) { " (required)" } else { "" };
    let mut out = section(named, &lf, &lp, mark, node);
    // where to start: the element, and for an operation the path item's parameters and servers
    let mut starts = vec![(e.file.clone(), e.pointer.clone())];
    if n.tool == Tool::Openapi
        && let Some(op) = document::operations(l, &e.file).into_iter().find(|o| o.pointer == e.pointer)
    {
        for key in ["parameters", "servers"] {
            let p = format!("{}/{key}", op.item);
            if document::locate(l, &e.file, &p).is_some() {
                starts.push((e.file.clone(), p));
            }
        }
    }
    let mut places: BTreeSet<(String, String)> = BTreeSet::new();
    for (f, p) in &starts {
        for (rf, rp) in document::reach(l, f, p) {
            if let Some((xf, xp, _)) = document::locate(l, &rf, &rp) {
                places.insert((xf, xp));
            }
        }
    }
    places.remove(&(lf.clone(), lp.clone()));
    for (f, p) in places {
        if let Some((_, _, x)) = document::locate(l, &f, &p) {
            out.push_str(&section(named, &f, &p, "", x));
        }
    }
    Some(out)
}

fn ns_name(ns: &cedar::Namespace) -> String {
    ns.name.as_ref().map(|n| n.to_string()).unwrap_or_default()
}

/// The common types a type uses, over any number of steps, as (namespace, type), in the order
/// first met.
fn common_types(s: &cedar::Schema, ns: usize, ty: &cedar::Type, out: &mut Vec<(usize, usize)>) {
    match ty {
        cedar::Type::Set(t) => common_types(s, ns, t, out),
        cedar::Type::Record(r) => {
            for a in &r.attrs {
                common_types(s, ns, &a.ty, out);
            }
        }
        cedar::Type::EntityOrCommon(name) | cedar::Type::CommonRef(name) => {
            let (want_ns, id) = (name.path.join("::"), name.id.as_str());
            // a name without a namespace is the declaring namespace's, else the one outside any
            let tries: Vec<usize> = if name.path.is_empty() {
                let mut t = vec![ns];
                t.extend(s.namespaces.iter().position(|x| x.name.is_none()));
                t
            } else {
                s.namespaces.iter().enumerate().filter(|(_, x)| ns_name(x) == want_ns).map(|(i, _)| i).collect()
            };
            for i in tries {
                if let Some(c) = s.namespaces[i].common_types.iter().position(|c| c.name == id) {
                    if !out.contains(&(i, c)) {
                        out.push((i, c));
                        common_types(s, i, &s.namespaces[i].common_types[c].ty, out);
                    }
                    break;
                }
            }
        }
        _ => {}
    }
}

fn schema_end(s: &cedar::Schema, n: &Name) -> Result<String, Miss> {
    let [(kind, name)] = n.items.as_slice() else { return Err(Miss::NotThere) };
    let mut found: Vec<(usize, usize)> = Vec::new();
    for (i, ns) in s.namespaces.iter().enumerate() {
        match kind.as_str() {
            "action" => found.extend(ns.actions.iter().position(|a| a.name == *name).map(|j| (i, j))),
            "entity" => found.extend(ns.entity_types.iter().position(|e| e.name == *name).map(|j| (i, j))),
            _ => return Err(Miss::NotThere),
        }
    }
    let (ni, di) = match found.as_slice() {
        [] => return Err(Miss::NotThere),
        [one] => *one,
        more => return Err(Miss::Twice(more.iter().map(|(i, _)| ns_name(&s.namespaces[*i])).collect())),
    };
    let ns = &s.namespaces[ni];
    let mut uses = Vec::new();
    let (actions, entity_types) = if kind == "action" {
        let a = ns.actions[di].clone();
        if let Some(t) = &a.applies_to {
            common_types(s, ni, &t.context, &mut uses);
        }
        (vec![a], vec![])
    } else {
        let e = ns.entity_types[di].clone();
        if let cedar::EntityKind::Standard { shape, tags, .. } = &e.kind {
            common_types(s, ni, shape, &mut uses);
            if let Some(t) = tags {
                common_types(s, ni, t, &mut uses);
            }
        }
        (vec![], vec![e])
    };
    // one namespace for the declaration, and one for each other namespace whose types it uses
    let mut spaces: Vec<cedar::Namespace> = Vec::new();
    let empty = |x: &cedar::Namespace| cedar::Namespace { name: x.name.clone(), annotations: vec![], common_types: vec![], entity_types: vec![], actions: vec![], line: x.line, col: x.col };
    let mut own = empty(ns);
    own.actions = actions;
    own.entity_types = entity_types;
    for (i, c) in &uses {
        let ct = s.namespaces[*i].common_types[*c].clone();
        if *i == ni {
            own.common_types.push(ct);
        } else {
            match spaces.iter_mut().find(|x| x.name == s.namespaces[*i].name) {
                Some(x) => x.common_types.push(ct),
                None => {
                    let mut x = empty(&s.namespaces[*i]);
                    x.common_types.push(ct);
                    spaces.push(x);
                }
            }
        }
    }
    spaces.push(own);
    let one = cedar::Schema { namespaces: spaces };
    // what the Cedar format cannot write (an entity whose shape is no record), in the JSON one
    let t = cedar::write_schema(&one).unwrap_or_else(|_| cedar::schema_to_json(&one).pretty());
    Ok(if t.ends_with('\n') { t } else { t + "\n" })
}

/// Every element of `kind` in the file `n` names, under its pairs, as references (for a scope,
/// and the candidates of E202).
pub fn gather(read: &Read, n: &Name, kind: &str) -> Vec<Name> {
    let file = Name::file(n.tool, n.path.clone());
    match read {
        Read::Doc(l) => document::gather(l, n, kind),
        Read::Policies(set) if kind == "policy" && n.items.is_empty() => set.policies.iter().map(|p| file.clone().with("policy", p.id.clone())).collect(),
        Read::Schema(s) if n.items.is_empty() => {
            let mut names: Vec<String> = Vec::new();
            for ns in &s.namespaces {
                match kind {
                    "action" => names.extend(ns.actions.iter().map(|a| a.name.clone())),
                    "entity" => names.extend(ns.entity_types.iter().map(|e| e.name.clone())),
                    _ => {}
                }
            }
            let mut seen = BTreeSet::new();
            names.into_iter().filter(|x| seen.insert(x.clone())).map(|x| file.clone().with(kind, x)).collect()
        }
        _ => vec![],
    }
}

/// Whether a file of a directory a scope names is one of the tool's: a document whose top mapping
/// says it is the tool's (a part of a document is named one by one), a Cedar file by its name.
pub fn is_the_tools(root: &Path, tool: Tool, file: &str) -> bool {
    if !tool.extensions().iter().any(|x| file.ends_with(x)) {
        return false;
    }
    if tool == Tool::Cedar {
        return true;
    }
    let Ok(src) = ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(root, file)) else { return false };
    yaml::read_file_text(file, &src).ok().and_then(|top| document::kind_of(&top)) == Some(tool)
}
