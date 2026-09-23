//! Holding a projection to the contract the caller's object is already described by
//! (§15.125).
//!
//! An input may say where it comes from — `dest : prefecture  from order.shipping.prefecture`
//! — and a `shape` says which contract `order` is. Neither half changes a single check of the
//! table: what comes out of a projection is a scalar input like any other, and the region
//! analysis never learns that it was projected. What the two halves buy is that the glue
//! between the caller's object and the rule's flat inputs is **generated rather than written
//! by hand**, and that a field the contract renamed stops the build instead of the glue.
//!
//! Two kinds of contract are read, a `.proto` and a JSON Schema, and only far enough to say
//! what stands at a path. Like `import proto` (§15.59) they are read on every `check` and
//! carry no digest: a contract that moved is a change nobody has read, and the paths are what
//! say so.
//!
//! A path says **where** a value comes from; the contract also says **which values** can come
//! from there — the Protovalidate rules on a field, JSON Schema's `minimum`, `maxItems`, `enum`
//! and `required` — and the input says which it takes. The two are held together here too
//! (§15.132). Still no check of the table moves: what is compared is the contract with the
//! input's own declaration, so that whatever passes the contract's validation is something
//! the generated code does not refuse at the door (E122), and a row reached only by values the
//! contract never lets through is named (W123).

use crate::ast::{Cell, CmpOp, EnumSource, Item, Lit, ProjKind, Projection, RuleFile, ShapeDecl, VarDecl};
use crate::cel::Kind;
use crate::diag::{Diag, FixKind, WVal};
use crate::fourier;
use crate::json::Json;
use crate::num::Rat;
use crate::relation::{Atom, Extra, Formula, Lin, Rel, Relation, Term};
use crate::types::{Checked, Ty};
use std::collections::{BTreeMap, BTreeSet};

/// What a contract says stands at a path.
#[derive(Debug, Clone, PartialEq)]
pub enum At {
    Str,
    Int,
    /// A number the contract allows a fraction in. A rule's numbers are whole in their
    /// declared unit (§2.1), so this is named apart and refused where a whole one is wanted.
    Frac,
    Bool,
    /// A record with fields. Carries the names it has, so a message can list them.
    Object(Vec<String>),
    Array(Box<At>),
    /// A kind no input can be, named as the contract writes it: `bytes`, or a type the file
    /// does not declare.
    Other(String),
}

impl At {
    /// How it is named in a message.
    pub fn word(&self) -> String {
        match self {
            At::Str => "string".into(),
            At::Int => "integer".into(),
            At::Frac => "number".into(),
            At::Bool => "boolean".into(),
            At::Object(_) => "object".into(),
            At::Array(e) => format!("array of {}", e.word()),
            At::Other(t) => t.clone(),
        }
    }

    /// Whether a value of this kind can be read as a value of the rule's type. A contract
    /// says how a value travels, not what it means, so an enum and a date arrive as strings
    /// and money and quantities arrive as whole numbers in the unit the rule declares.
    pub fn fits(&self, ty: &Ty) -> bool {
        match (self, ty) {
            (_, Ty::Unknown) => true,
            (At::Bool, Ty::Bool) => true,
            (At::Str, Ty::Str | Ty::Date | Ty::Enum(_)) => true,
            (At::Int, Ty::Number | Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) => true,
            (At::Frac, Ty::Rate) => true,
            (a, Ty::Opt(inner)) => a.fits(inner),
            _ => false,
        }
    }
}

/// Where the resolution of a path stopped, when it did not finish.
pub struct Stuck {
    /// The path as far as it did resolve.
    pub reached: String,
    /// The names that were there, so the next attempt is a correction and not a guess.
    pub had: Vec<String>,
}

/// The contract of one shape, read once and asked many times.
pub struct Contract {
    kind: EnumSource,
    /// For a schema: the whole document, so `$ref` can be followed. For a `.proto`: unused.
    doc: Option<Json>,
    /// For a `.proto`: every message of the file, as (name, fields).
    msgs: Vec<crate::proto::Message>,
    /// For a `.proto`: every enum of the file. A field of one travels as its value's name, and
    /// a field left at the value numbered 0 is left out (§15.133).
    enums: Vec<crate::proto::Enum>,
    /// Where the root stands: a pointer into the schema, or a message name.
    at: String,
    /// For a schema: whether the keywords beside a `$ref` apply too (§15.140).
    siblings: bool,
}

/// Read the contract a `shape` names, or say why it cannot be read.
pub fn read(d: &ShapeDecl, text: &str) -> Result<Contract, String> {
    match d.source {
        EnumSource::JsonSchema => {
            let doc = crate::jsonschema::read(text, &d.file)?;
            let siblings = siblings_apply(&doc);
            Ok(Contract { kind: d.source, doc: Some(doc), msgs: Vec::new(), enums: Vec::new(), at: d.at.clone(), siblings })
        }
        EnumSource::Proto => {
            let msgs = crate::proto::messages(text);
            if !msgs.iter().any(|m| crate::proto::same_message(&m.name, &d.at)) {
                return Err(if msgs.is_empty() {
                    tr!("そのファイルにメッセージは一つもありません。", "That file declares no message at all.")
                } else {
                    tr!(
                        "そのファイルにあるメッセージ: {}",
                        "The messages in that file: {}",
                        msgs.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ")
                    )
                });
            }
            Ok(Contract { kind: d.source, doc: None, msgs, enums: crate::proto::enums(text), at: d.at.clone(), siblings: false })
        }
    }
}

impl Contract {
    /// The enum of the `.proto` a field's type names, if the file declares it.
    pub(crate) fn enum_of(&self, ty: &str) -> Option<&crate::proto::Enum> {
        self.enums.iter().find(|e| crate::proto::same_message(&e.name, ty))
    }

    /// What stands at a path from the root, or where the walk got stuck.
    pub fn at(&self, path: &[String]) -> Result<At, Stuck> {
        match self.kind {
            EnumSource::JsonSchema => self.schema_at(path),
            EnumSource::Proto => self.proto_at(path),
        }
    }

    fn schema_at(&self, path: &[String]) -> Result<At, Stuck> {
        let doc = self.doc.as_ref().expect("a schema contract carries its document");
        let mut node = at_pointer(doc, &self.at).ok_or_else(|| Stuck { reached: self.at.clone(), had: keys(doc) })?;
        let mut reached = String::new();
        for step in path {
            match self.property(node, step) {
                Some(v) => {
                    node = v;
                    reached = if reached.is_empty() { step.clone() } else { format!("{reached}.{step}") };
                }
                None => return Err(Stuck { reached, had: self.property_names(node) }),
            }
        }
        Ok(self.kind_at(node))
    }

    fn proto_at(&self, path: &[String]) -> Result<At, Stuck> {
        let mut msg = self.msgs.iter().find(|m| crate::proto::same_message(&m.name, &self.at));
        let mut reached = String::new();
        let mut last = None;
        for (i, step) in path.iter().enumerate() {
            let Some(m) = msg else {
                return Err(Stuck { reached, had: Vec::new() });
            };
            let Some(fd) = m.fields.iter().find(|x| x.name == *step) else {
                return Err(Stuck { reached, had: m.fields.iter().map(|x| x.name.clone()).collect() });
            };
            reached = if reached.is_empty() { step.clone() } else { format!("{reached}.{step}") };
            let scalar = crate::proto::scalar(&fd.ty).map(|s| match s {
                crate::proto::Scalar::Str => At::Str,
                crate::proto::Scalar::Int => At::Int,
                crate::proto::Scalar::Frac => At::Frac,
                crate::proto::Scalar::Bool => At::Bool,
            });
            msg = match &scalar {
                Some(_) => None,
                None => self.msgs.iter().find(|x| crate::proto::same_message(&x.name, &fd.ty)),
            };
            let here = match (&scalar, msg) {
                (Some(s), _) => s.clone(),
                (None, Some(m)) => At::Object(m.fields.iter().map(|x| x.name.clone()).collect()),
                // At the end of the path, a kind no input takes is named, so the finding says
                // what is there rather than that nothing is: an enum of the file, `bytes`, or a
                // type the file does not declare (§15.133).
                (None, None) if i + 1 == path.len() => {
                    At::Other(if self.enum_of(&fd.ty).is_some() { format!("enum {}", fd.ty) } else { fd.ty.clone() })
                }
                // A message the file does not hold: the path cannot go on.
                (None, None) => return Err(Stuck { reached, had: Vec::new() }),
            };
            last = Some(if fd.repeated { At::Array(Box::new(here)) } else { here });
            // Only the last step may be repeated; a path through a collection is a join.
            if fd.repeated && i + 1 < path.len() {
                return Err(Stuck { reached, had: Vec::new() });
            }
        }
        last.ok_or(Stuck { reached, had: Vec::new() })
    }
}

// --- The JSON Schema side, only as far as a path needs -----------------------------------

fn get<'a>(v: &'a Json, k: &str) -> Option<&'a Json> {
    match v {
        Json::Obj(m) => m.iter().find(|(n, _)| n.as_str() == k).map(|(_, v)| v),
        _ => None,
    }
}

fn obj<'a>(v: &'a Json, k: &str) -> Option<&'a Json> {
    get(v, k).filter(|x| matches!(x, Json::Obj(_)))
}

fn keys(v: &Json) -> Vec<String> {
    match v {
        Json::Obj(m) => m.iter().map(|(n, _)| n.clone()).collect(),
        _ => Vec::new(),
    }
}

/// A JSON Pointer into the document, `#/a/b` or `/a/b`.
fn at_pointer<'a>(doc: &'a Json, pointer: &str) -> Option<&'a Json> {
    let mut node = doc;
    for step in pointer.trim_start_matches('#').split('/').filter(|s| !s.is_empty()) {
        node = get(node, &step.replace("~1", "/").replace("~0", "~"))?;
    }
    Some(node)
}

/// Whether the keywords beside a `$ref` apply as well as the ones it leads to (§15.140). They
/// do from JSON Schema 2019-09 on, which OpenAPI 3.1 follows, and are ignored before that,
/// which OpenAPI 3.0 and draft-07 are. A document that does not say is read the older way:
/// ignoring keywords only reads the contract wider than it is.
fn siblings_apply(doc: &Json) -> bool {
    let schema = get(doc, "$schema").and_then(Json::as_str).unwrap_or("");
    let openapi = get(doc, "openapi").and_then(Json::as_str).unwrap_or("");
    schema.contains("2019-09") || schema.contains("2020-12") || openapi.starts_with("3.1")
}

/// How deep a walk through `$ref` and the combinators goes before it stops looking.
const DEPTH: usize = 24;

impl Contract {
    /// The schema a `$ref` leads to, when it points inside this document.
    fn target(&self, r: &Json) -> Option<&Json> {
        match r {
            Json::Str(p) if p.starts_with('#') => at_pointer(self.doc.as_ref()?, p),
            _ => None,
        }
    }

    /// Every schema that certainly applies to an instance of `v`: `v` itself, where its `$ref`
    /// leads, and the branches of its `allOf`, as far as they go. In a document that ignores
    /// the keywords beside a `$ref`, a schema with one is only where it leads.
    fn applying<'a>(&'a self, v: &'a Json) -> Vec<&'a Json> {
        let mut out: Vec<&Json> = Vec::new();
        let mut stack = vec![v];
        while let Some(x) = stack.pop() {
            if out.len() > DEPTH || out.iter().any(|y| std::ptr::eq(*y, x)) {
                continue;
            }
            if let Some(r) = get(x, "$ref") {
                if let Some(t) = self.target(r) {
                    stack.push(t);
                }
                if !self.siblings {
                    continue;
                }
            }
            out.push(x);
            if let Some(Json::Arr(bs)) = get(x, "allOf") {
                stack.extend(bs.iter().rev());
            }
        }
        out
    }

    /// The schema of the property `name` of an instance of `v`. Looked for where it certainly
    /// applies first, then in the branches of `anyOf`, `oneOf` and `if`: a property only one
    /// branch describes is still one an instance can have, and a path to it is no mistake.
    fn property<'a>(&'a self, v: &'a Json, name: &str) -> Option<&'a Json> {
        self.property_in(v, name, 0)
    }

    fn property_in<'a>(&'a self, v: &'a Json, name: &str, depth: usize) -> Option<&'a Json> {
        if depth > DEPTH {
            return None;
        }
        let all = self.applying(v);
        if let Some(p) = all.iter().find_map(|x| obj(x, "properties").and_then(|ps| get(ps, name))) {
            return Some(p);
        }
        all.iter().find_map(|x| self.branches(x).into_iter().find_map(|b| self.property_in(b, name, depth + 1)))
    }

    /// The branches an instance of `x` may take one of.
    fn branches<'a>(&'a self, x: &'a Json) -> Vec<&'a Json> {
        let mut out = Vec::new();
        for k in ["anyOf", "oneOf"] {
            if let Some(Json::Arr(bs)) = get(x, k) {
                out.extend(bs.iter());
            }
        }
        for k in ["then", "else"] {
            out.extend(get(x, k));
        }
        out
    }

    /// The names of the properties an instance of `v` can have, found the same way.
    fn property_names(&self, v: &Json) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut stack = vec![(v, 0usize)];
        while let Some((x, depth)) = stack.pop() {
            if depth > DEPTH {
                continue;
            }
            for a in self.applying(x) {
                for k in obj(a, "properties").map(keys).unwrap_or_default() {
                    if !out.contains(&k) {
                        out.push(k);
                    }
                }
                stack.extend(self.branches(a).into_iter().map(|b| (b, depth + 1)));
            }
        }
        out
    }

    /// The properties an instance of `v` certainly has: `required` wherever it certainly
    /// applies.
    fn required(&self, v: &Json) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for a in self.applying(v) {
            for r in required_of(a) {
                if !out.contains(&r) {
                    out.push(r);
                }
            }
        }
        out
    }

    /// The type an instance of `v` has, as far as the schema says, and whether it may be null
    /// as well: `"type": ["integer", "null"]`, or OpenAPI 3.0's `nullable`.
    fn schema_type(&self, v: &Json) -> (Option<String>, bool) {
        let all = self.applying(v);
        let nullable = all.iter().any(|x| matches!(get(x, "nullable"), Some(Json::Bool(true))));
        for x in &all {
            match get(x, "type") {
                Some(Json::Str(t)) => return (Some(t.clone()), nullable || t == "null"),
                Some(Json::Arr(ts)) => {
                    let names: Vec<&str> = ts.iter().filter_map(Json::as_str).collect();
                    let some: Vec<&str> = names.iter().copied().filter(|t| *t != "null").collect();
                    let null = names.len() != some.len();
                    return match some.as_slice() {
                        [] => (Some("null".into()), true),
                        [one] => (Some(one.to_string()), nullable || null),
                        many => (Some(many.join("|")), nullable || null),
                    };
                }
                _ => {}
            }
        }
        // No `type`: an `enum` of strings travels as a string, and anything else is read as an
        // object, the kind a path can go on through and the kind `fits` refuses for a scalar.
        if all.iter().any(|x| get(x, "enum").is_some()) {
            return (Some("string".into()), nullable);
        }
        (None, nullable)
    }

    /// What stands where `v` describes, in the words a path compares.
    fn kind_at(&self, v: &Json) -> At {
        match self.schema_type(v).0.as_deref() {
            Some("string") => At::Str,
            Some("integer") => At::Int,
            Some("number") => At::Frac,
            Some("boolean") => At::Bool,
            Some("array") => At::Array(Box::new(match self.applying(v).into_iter().find_map(|x| get(x, "items")) {
                Some(i) => self.kind_at(i),
                None => At::Object(Vec::new()),
            })),
            Some("object") | None => At::Object(self.property_names(v)),
            Some(other) => At::Other(other.to_string()),
        }
    }

    /// The items of the collection `v` describes.
    fn items<'a>(&'a self, v: &'a Json) -> Option<&'a Json> {
        self.applying(v).into_iter().find_map(|x| get(x, "items"))
    }
}

// --- The check ---------------------------------------------------------------------------

/// Every projection of a rule, against the shape it starts at.
///
/// The path is resolved against the directory of the `.rule`, the way an enum import's is.
/// A shape whose file cannot be read is reported once, and the projections that start at it
/// are then left alone: one unreadable file should not print one error per input.
pub fn check(f: &RuleFile, c: &Checked, rule_path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    let dir = std::path::Path::new(rule_path).parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
    let mut read_ok = Vec::new();
    for d in &f.shapes {
        let p = dir.join(&d.file);
        let at = format!("{rule_path}:{}", d.span.line);
        match std::fs::read_to_string(&p) {
            Ok(text) => match read(d, &text) {
                Ok(con) => read_ok.push((d, con)),
                Err(why) => out.push(
                    Diag::error("E013", tr!("`{}` の中に `{}` はありません", "`{}` has nothing at `{}`", d.file, d.at))
                        .at(at)
                        .mark(d.span.clone(), "")
                        .note(why),
                ),
            },
            Err(_) => out.push(
                Diag::error("E013", tr!("`{}` を読めません", "Cannot read `{}`", d.file))
                    .at(at)
                    .mark(d.span.clone(), "")
                    .note(tr!(
                        "パスは規則ファイルのある場所からたどります（探した先: {}）。",
                        "The path is followed from the directory of the rule file (looked for: {}).",
                        p.display()
                    )),
            ),
        }
    }
    let mut doms = Vec::new();
    // What each contract says across the fields the rule reads from it (§15.140).
    let across: Vec<(String, Across)> = read_ok.iter().map(|(d, con)| (d.name.text.clone(), con.across(f, c, &d.name.text))).collect();
    for i in &f.inputs {
        let Some(pr) = &i.from else { continue };
        let ty = c.ty_of(&i.name.text).unwrap_or(Ty::Unknown);
        let shaped = fits_the_type(pr, &ty, rule_path, &i.name.text);
        let fits = shaped.is_empty();
        out.extend(shaped);
        let Some((_, con)) = read_ok.iter().find(|(d, _)| d.name.text == pr.root.text) else {
            // A root with no shape of its own, or one whose file did not read. The first is
            // an error the reader can act on; the second was already said once.
            if !f.shapes.iter().any(|d| d.name.text == pr.root.text) {
                out.push(no_such_root(f, pr, rule_path));
            }
            continue;
        };
        let resolved = resolves(con, pr, &ty, rule_path, &i.name.text);
        // What the contract lets through is only asked of a path that reached a value of the
        // input's kind: past an E120 or an E121 there is nothing to compare.
        if fits && resolved.is_empty() {
            let ac = across.iter().find(|(n, _)| *n == pr.root.text).map(|(_, a)| a);
            let (found, dom) = holds(f, con, pr, i, c, rule_path, ac);
            out.extend(found);
            if let Some(d) = dom {
                doms.push((i.name.text.clone(), d, pr));
            }
        }
        out.extend(resolved);
    }
    let w123 = dead_rows(f, c, &doms, rule_path);
    let dead: BTreeSet<(String, usize)> = w123.iter().filter_map(|d| Some((d.table.clone()?, d.row?))).collect();
    out.extend(w123);
    for ((d, con), (_, ac)) in read_ok.iter().zip(&across) {
        out.extend(broken_constraints(f, c, con, ac, rule_path));
        out.extend(unreachable_rows(f, c, ac, &d.name.text, &dead, rule_path));
    }
    for d in &f.shapes {
        if f.inputs.iter().any(|i| i.from.as_ref().is_some_and(|p| p.root.text == d.name.text)) {
            continue;
        }
        out.push(
            Diag::warning("W122", tr!("`shape {}` を使っている入力がありません", "No input is projected from `shape {}`", d.name.text))
                .at(format!("{rule_path}:{}", d.span.line))
                .mark(d.span.clone(), "")
                .note(tr!(
                    "`from {}.…` と書いた入力が一つもないので、この契約は読まれるだけで何も確かめていません。使わないなら消してください。",
                    "No input says `from {}.…`, so this contract is read and holds nothing. Delete it if it is not used.",
                    d.name.text
                )),
        );
    }
    out
}

/// E120 without reading anything: the shape of the projection against the input's own type.
fn fits_the_type(pr: &Projection, ty: &Ty, rule_path: &str, name: &str) -> Vec<Diag> {
    let want = match &pr.kind {
        ProjKind::Field => return Vec::new(),
        ProjKind::Any(..) | ProjKind::All(..) => Ty::Bool,
        ProjKind::Count(_) => Ty::Number,
    };
    if *ty == want || matches!(ty, Ty::Unknown) {
        return Vec::new();
    }
    vec![
        Diag::error("E120", tr!("`from {}` は {} を返します", "`from {}` yields {}", pr.kind.word(), want))
            .at(format!("{rule_path}:{}", pr.span.line))
            .mark(pr.span.clone(), tr!("入力 {name} は {ty}", "the input {name} is {ty}"))
            .note(match pr.kind {
                ProjKind::Count(_) => tr!(
                    "`count` は並びの件数なので、受ける入力は `number` です。範囲も要ります——数える上限は検査が量化する宇宙でもあるからです。",
                    "`count` is how many elements passed, so the input that takes it is a `number`. It needs a range too: the cap on the walk is the universe the checks quantify over."
                ),
                _ => tr!(
                    "`any` と `all` は当てはまるかどうかなので、受ける入力は `bool` です。値そのものが欲しいなら `from <shape の名前>.<フィールド>` を書いてください。",
                    "`any` and `all` say whether the elements passed, so the input that takes one is a `bool`. For the value itself, write `from <shape>.<field>`."
                ),
            }),
    ]
}

fn no_such_root(f: &RuleFile, pr: &Projection, rule_path: &str) -> Diag {
    let known: Vec<&str> = f.shapes.iter().map(|d| d.name.text.as_str()).collect();
    Diag::error("E121", tr!("`{}` という `shape` はありません", "There is no `shape` named `{}`", pr.root.text))
        .at(format!("{rule_path}:{}", pr.span.line))
        .mark(pr.root.span.clone(), "")
        .note(if known.is_empty() {
            tr!(
                "`from` のパスは宣言した `shape` から始まります。`shape {}(...) = jsonschema \"<ファイル>\" \"<ポインタ>\"` を足してください。",
                "A `from` path starts at a declared `shape`. Add `shape {}(...) = jsonschema \"<file>\" \"<pointer>\"`.",
                pr.root.text
            )
        } else {
            tr!("宣言されている `shape`: {}", "The shapes declared here: {}", known.join(", "))
        })
}

/// E121 and E120 against the contract: the path, and what stands at the end of it.
fn resolves(con: &Contract, pr: &Projection, ty: &Ty, rule_path: &str, name: &str) -> Vec<Diag> {
    let at = format!("{rule_path}:{}", pr.span.line);
    let steps: Vec<String> = pr.path.iter().map(|n| n.text.clone()).collect();
    let full = || format!("{}.{}", pr.root.text, steps.join("."));
    let stuck = |s: Stuck| {
        let where_ = if s.reached.is_empty() { pr.root.text.clone() } else { format!("{}.{}", pr.root.text, s.reached) };
        Diag::error("E121", tr!("`{}` というパスは契約にありません", "The contract has no path `{}`", full()))
            .at(at.clone())
            .mark(pr.span.clone(), "")
            .note(if s.had.is_empty() {
                tr!("`{where_}` までは届きました。その先にフィールドはありません。", "It resolved as far as `{where_}`, which has no fields under it.")
            } else {
                tr!(
                    "`{where_}` までは届きました。そこにあるフィールド: {}",
                    "It resolved as far as `{where_}`. The fields there: {}",
                    s.had.join(", ")
                )
            })
    };
    let here = match con.at(&steps) {
        Ok(a) => a,
        Err(s) => return vec![stuck(s)],
    };
    let mut out = Vec::new();
    match &pr.kind {
        ProjKind::Field => {
            if !here.fits(ty) {
                out.push(mismatch(&at, pr, &full(), &here, ty, name));
            }
        }
        _ => {
            let At::Array(elem) = &here else {
                out.push(
                    Diag::error("E120", tr!("`{}` は並びではありません", "`{}` is not a collection", full()))
                        .at(at.clone())
                        .mark(pr.span.clone(), tr!("契約では {}", "the contract says {}", here.word()))
                        .note(tr!(
                            "`any`・`all`・`count` は並びを一度だけ歩きます。値そのものが欲しいなら `from <shape の名前>.<フィールド>` を書いてください。",
                            "`any`, `all` and `count` walk a collection once. For the value itself, write `from <shape>.<field>`."
                        )),
                );
                return out;
            };
            if let Some((field, cell)) = pr.kind.test() {
                let At::Object(had) = elem.as_ref() else {
                    out.push(
                        Diag::error("E121", tr!("`{}` の要素にフィールドはありません", "An element of `{}` has no fields", full()))
                            .at(at.clone())
                            .mark(field.span.clone(), tr!("契約では {}", "the contract says {}", elem.word()))
                            .note(tr!("`where` は要素のフィールドを見ます。", "`where` tests a field of an element.")),
                    );
                    return out;
                };
                if !had.contains(&field.text) {
                    out.push(
                        Diag::error("E121", tr!("要素に `{}` というフィールドはありません", "An element has no field `{}`", field.text))
                            .at(at.clone())
                            .mark(field.span.clone(), "")
                            .note(tr!("要素にあるフィールド: {}", "The fields of an element: {}", had.join(", "))),
                    );
                } else if let Some(bad) = cell_clashes(con, &steps, &field.text, cell) {
                    out.push(
                        Diag::error("E120", tr!("`where` の値がフィールドの型と合いません", "The value of `where` does not fit the field"))
                            .at(at.clone())
                            .mark(field.span.clone(), tr!("契約では {}", "the contract says {}", bad))
                            .note(tr!(
                                "契約は値がどう運ばれるかを言います。列挙も日付も文字列で、金額と数量は整数で来ます。",
                                "A contract says how a value travels: an enum and a date arrive as strings, money and quantities as whole numbers."
                            )),
                    );
                }
            }
        }
    }
    out
}

fn mismatch(at: &str, pr: &Projection, full: &str, here: &At, ty: &Ty, name: &str) -> Diag {
    let d = Diag::error("E120", tr!("`{full}` の型が入力と合いません", "The type at `{full}` does not fit the input"))
        .at(at.to_string())
        .mark(pr.span.clone(), tr!("契約では {}、入力 {name} は {ty}", "the contract says {}, the input {name} is {ty}", here.word()))
        .note(tr!(
            "契約は値がどう運ばれるかを言い、規則はそれが何を意味するかを言います。列挙も日付も文字列で来て、金額と数量は宣言した単位の整数で来ます。",
            "A contract says how a value travels and the rule says what it means: an enum and a date arrive as strings, and money and a quantity as whole numbers in the unit the rule declares."
        ));
    match here {
        // What protojson carries is the `.proto` value's name, and taking it into the rule's
        // enum would need the two tied value by value, which `import proto` does and a path
        // does not (§15.133).
        At::Other(t) if t.starts_with("enum ") => d.note(tr!(
            "`.proto` の列挙は、値の名前（`ZONE_HONSHU` のような）で運ばれます。パスはそれを規則の列挙に渡しません。`where` で比べることはできます。",
            "A `.proto` enum travels as its value's name (such as `ZONE_HONSHU`), and a path does not hand that to the rule's enum. A `where` can compare it."
        )),
        _ => d,
    }
}

/// The word for the element field's kind, when a `where` cell cannot be a value of it.
fn cell_clashes(con: &Contract, steps: &[String], field: &str, cell: &Cell) -> Option<String> {
    let mut path = steps.to_vec();
    path.push(field.to_string());
    let At::Array(_) = con.at(steps).ok()? else { return None };
    // The element's own field: resolved by walking the collection's path and then the field,
    // which the contract readers already do by looking through the element.
    let k = con.elem_field(steps, field)?;
    let lits: Vec<&Lit> = match cell {
        Cell::Lit(l) => vec![l],
        Cell::Set(ls) | Cell::Not(ls) => ls.iter().collect(),
        Cell::Cmp(ops) => ops.iter().map(|(_, l)| l).collect(),
        _ => return None,
    };
    let ok = lits.iter().all(|l| match (l, &k) {
        (Lit::Str(_) | Lit::Word(_), At::Str) => true,
        (Lit::Date(..), At::Str) => true,
        (Lit::Num(_), At::Int | At::Frac) => true,
        (Lit::Word(w), At::Bool) => w == crate::kw::TRUE || w == crate::kw::FALSE,
        _ => false,
    });
    (!ok).then(|| k.word())
}

impl Contract {
    /// What stands at one field of an element of the collection at `steps`.
    fn elem_field(&self, steps: &[String], field: &str) -> Option<At> {
        match self.kind {
            EnumSource::JsonSchema => {
                let doc = self.doc.as_ref()?;
                let mut node = at_pointer(doc, &self.at)?;
                for s in steps {
                    node = self.property(node, s)?;
                }
                let f = self.property(self.items(node)?, field)?;
                Some(self.kind_at(f))
            }
            EnumSource::Proto => {
                let mut msg = self.msgs.iter().find(|m| crate::proto::same_message(&m.name, &self.at))?;
                for s in steps {
                    let fd = msg.fields.iter().find(|x| x.name == *s)?;
                    msg = self.msgs.iter().find(|x| crate::proto::same_message(&x.name, &fd.ty))?;
                }
                let fd = msg.fields.iter().find(|x| x.name == field)?;
                Some(match crate::proto::scalar(&fd.ty) {
                    Some(crate::proto::Scalar::Str) => At::Str,
                    Some(crate::proto::Scalar::Int) => At::Int,
                    Some(crate::proto::Scalar::Frac) => At::Frac,
                    Some(crate::proto::Scalar::Bool) => At::Bool,
                    // An enum travels as the name of its value (§15.133), which a `where`
                    // compares like any other string.
                    None if self.enum_of(&fd.ty).is_some() => At::Str,
                    None => match self.msgs.iter().find(|m| crate::proto::same_message(&m.name, &fd.ty)) {
                        Some(m) => At::Object(m.fields.iter().map(|x| x.name.clone()).collect()),
                        None => At::Other(fd.ty.clone()),
                    },
                })
            }
        }
    }
}

// --- What the contract lets through, held against what the input takes (§15.132) --------

/// Integers as closed intervals, sorted and apart; `None` is an open end.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Ints(Vec<(Option<i128>, Option<i128>)>);

impl Ints {
    fn between(lo: Option<i128>, hi: Option<i128>) -> Ints {
        match (lo, hi) {
            (Some(a), Some(b)) if a > b => Ints(Vec::new()),
            _ => Ints(vec![(lo, hi)]),
        }
    }

    fn points(vs: &[i128]) -> Ints {
        Ints(vs.iter().map(|&x| (Some(x), Some(x))).collect()).normal()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn intersect(&self, o: &Ints) -> Ints {
        let mut out = Vec::new();
        for &(a1, b1) in &self.0 {
            for &(a2, b2) in &o.0 {
                let lo = match (a1, a2) {
                    (None, x) | (x, None) => x,
                    (Some(x), Some(y)) => Some(x.max(y)),
                };
                let hi = match (b1, b2) {
                    (None, x) | (x, None) => x,
                    (Some(x), Some(y)) => Some(x.min(y)),
                };
                out.push((lo, hi));
            }
        }
        Ints(out).normal()
    }

    fn union(&self, o: &Ints) -> Ints {
        let mut v = self.0.clone();
        v.extend(o.0.iter().copied());
        Ints(v).normal()
    }

    fn without(&self, x: i128) -> Ints {
        let mut out = Vec::new();
        for &(a, b) in &self.0 {
            if a.is_some_and(|a| x < a) || b.is_some_and(|b| x > b) {
                out.push((a, b));
                continue;
            }
            if a != Some(x) {
                out.push((a, Some(x - 1)));
            }
            if b != Some(x) {
                out.push((Some(x + 1), b));
            }
        }
        Ints(out).normal()
    }

    /// Sorted, the empty ones dropped, and merged where they meet.
    fn normal(self) -> Ints {
        let mut v: Vec<_> = self.0.into_iter().filter(|(a, b)| !matches!((a, b), (Some(a), Some(b)) if a > b)).collect();
        v.sort_by(|x, y| match (x.0, y.0) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, _) => std::cmp::Ordering::Less,
            (_, None) => std::cmp::Ordering::Greater,
            (Some(a), Some(b)) => a.cmp(&b),
        });
        let mut out: Vec<(Option<i128>, Option<i128>)> = Vec::new();
        for (a, b) in v {
            if let Some(last) = out.last_mut() {
                let meets = match (last.1, a) {
                    (None, _) | (_, None) => true,
                    (Some(h), Some(a)) => a <= h.saturating_add(1),
                };
                if meets {
                    last.1 = match (last.1, b) {
                        (None, _) | (_, None) => None,
                        (Some(x), Some(y)) => Some(x.max(y)),
                    };
                    continue;
                }
            }
            out.push((a, b));
        }
        Ints(out)
    }

    fn min(&self) -> Option<i128> {
        self.0.first().and_then(|(a, _)| *a)
    }

    fn max(&self) -> Option<i128> {
        self.0.last().and_then(|(_, b)| *b)
    }

    /// In words, for a message: `1〜50`, `0 以上`, `100、200`.
    fn describe(&self) -> String {
        self.describe_in("")
    }

    /// The same, with a counter word after each number in Japanese: `0 件以上`, `1〜50 件`.
    fn describe_in(&self, unit: &str) -> String {
        let u = if unit.is_empty() { String::new() } else { format!(" {unit}") };
        let one = |(a, b): &(Option<i128>, Option<i128>)| match (a, b) {
            (None, None) => tr!("どの整数でも", "any integer"),
            (Some(a), None) if unit.is_empty() => tr!("{a} 以上", "{a} or more"),
            (Some(a), None) => tr!("{a}{u}以上", "{a} or more"),
            (None, Some(b)) if unit.is_empty() => tr!("{b} 以下", "{b} or less"),
            (None, Some(b)) => tr!("{b}{u}以下", "{b} or less"),
            (Some(a), Some(b)) if a == b => tr!("{a}{u}", "{a}"),
            (Some(a), Some(b)) => tr!("{a}〜{b}{u}", "{a} to {b}"),
        };
        if self.0.is_empty() {
            return tr!("何も通さない", "nothing");
        }
        let mut parts: Vec<String> = self.0.iter().take(6).map(one).collect();
        if self.0.len() > 6 {
            parts.push("…".into());
        }
        parts.join(if crate::i18n::ja() { "、" } else { ", " })
    }
}

/// What passes where an input is read, when the contract names it: what W123 holds a row to.
pub(crate) enum Dom {
    /// Integers, and whether they count elements (said with 件 in Japanese).
    Ints(Ints, bool),
    Strs(Vec<String>),
}

impl Contract {
    /// The schema at the root and at each step of a path, `$ref` followed.
    fn schema_chain(&self, path: &[String]) -> Option<Vec<&Json>> {
        let doc = self.doc.as_ref()?;
        let mut node = at_pointer(doc, &self.at)?;
        let mut out = vec![node];
        for s in path {
            node = self.property(node, s)?;
            out.push(node);
        }
        Some(out)
    }

    /// The field at each step of a path.
    fn proto_chain(&self, path: &[String]) -> Option<Vec<&crate::proto::Field>> {
        let mut msg = self.msgs.iter().find(|m| crate::proto::same_message(&m.name, &self.at))?;
        let mut out = Vec::new();
        for (i, s) in path.iter().enumerate() {
            let fd = msg.fields.iter().find(|x| x.name == *s)?;
            out.push(fd);
            if i + 1 < path.len() {
                msg = self.msgs.iter().find(|x| crate::proto::same_message(&x.name, &fd.ty))?;
            }
        }
        Some(out)
    }
}

fn required_of(n: &Json) -> Vec<String> {
    match get(n, "required") {
        Some(Json::Arr(vs)) => vs.iter().filter_map(|v| if let Json::Str(s) = v { Some(s.clone()) } else { None }).collect(),
        _ => Vec::new(),
    }
}

/// A JSON number as the exact value written: `0.5` is 1/2, `1e3` is 1000.
fn rat_of(v: &Json) -> Option<Rat> {
    match v {
        Json::Int(n) => Some(Rat::int(*n)),
        Json::Frac(s) => {
            let (mant, exp) = match s.find(['e', 'E']) {
                Some(i) => (&s[..i], s[i + 1..].parse::<i32>().ok()?),
                None => (s.as_str(), 0),
            };
            let neg = mant.starts_with('-');
            let m = mant.trim_start_matches(['-', '+']);
            let (w, f) = m.split_once('.').unwrap_or((m, ""));
            let mut num: i128 = format!("{w}{f}").parse().ok()?;
            let mut den: i128 = 10i128.checked_pow(f.len() as u32)?;
            if exp > 0 {
                num = num.checked_mul(10i128.checked_pow(exp as u32)?)?;
            } else if exp < 0 {
                den = den.checked_mul(10i128.checked_pow(exp.unsigned_abs())?)?;
            }
            Some(Rat::new(if neg { -num } else { num }, den))
        }
        _ => None,
    }
}

fn floor(r: Rat) -> i128 {
    r.num.div_euclid(r.den)
}

fn ceil(r: Rat) -> i128 {
    -(-r.num).div_euclid(r.den)
}

/// The integers a schema lets through: `minimum`, `maximum`, both `exclusive` forms (the number
/// of 2019-09 on, and the flag of draft-04), `const` and `enum`.
fn schema_ints(n: &Json) -> Ints {
    let flag = |k: &str| matches!(get(n, k), Some(Json::Bool(true)));
    let mut lo = get(n, "minimum").and_then(rat_of).map(|m| if flag("exclusiveMinimum") { floor(m) + 1 } else { ceil(m) });
    let mut hi = get(n, "maximum").and_then(rat_of).map(|m| if flag("exclusiveMaximum") { ceil(m) - 1 } else { floor(m) });
    if let Some(m) = get(n, "exclusiveMinimum").and_then(rat_of) {
        let v = floor(m) + 1;
        lo = Some(lo.map_or(v, |x| x.max(v)));
    }
    if let Some(m) = get(n, "exclusiveMaximum").and_then(rat_of) {
        let v = ceil(m) - 1;
        hi = Some(hi.map_or(v, |x| x.min(v)));
    }
    let mut s = Ints::between(lo, hi);
    if let Some(k) = get(n, "const").and_then(rat_of) {
        s = s.intersect(&if k.is_int() { Ints::points(&[k.num]) } else { Ints(Vec::new()) });
    }
    if let Some(Json::Arr(vs)) = get(n, "enum") {
        let pts: Vec<i128> = vs.iter().filter_map(rat_of).filter(|r| r.is_int()).map(|r| r.num).collect();
        s = s.intersect(&Ints::points(&pts));
    }
    s
}

/// The strings a schema lets through, or `None` when it does not list them.
fn schema_strs(n: &Json) -> Option<Vec<String>> {
    let mut list: Option<Vec<String>> = match get(n, "enum") {
        Some(Json::Arr(vs)) => Some(vs.iter().filter_map(|v| if let Json::Str(s) = v { Some(s.clone()) } else { None }).collect()),
        _ => None,
    };
    if let Some(Json::Str(k)) = get(n, "const") {
        list = Some(match list {
            Some(l) => l.into_iter().filter(|x| x == k).collect(),
            None => vec![k.clone()],
        });
    }
    list
}

/// The keywords of a schema that narrow a value in a way this does not read. The combinators
/// are read, as the condition across the fields (§15.140).
fn schema_unread(n: &Json, strings: bool) -> Vec<String> {
    let mut ks: Vec<&str> = vec!["multipleOf"];
    if strings {
        ks.extend(["pattern", "format", "minLength", "maxLength"]);
    }
    ks.into_iter().filter(|k| get(n, k).is_some()).map(String::from).collect()
}

impl Contract {
    /// `schema_ints` over every schema that certainly applies.
    fn ints_at(&self, n: &Json) -> Ints {
        self.applying(n).into_iter().fold(Ints::between(None, None), |s, x| s.intersect(&schema_ints(x)))
    }

    /// `schema_strs` over every schema that certainly applies.
    fn strs_at(&self, n: &Json) -> Option<Vec<String>> {
        self.applying(n).into_iter().fold(None, |acc: Option<Vec<String>>, x| match (acc, schema_strs(x)) {
            (Some(a), Some(b)) => Some(a.into_iter().filter(|v| b.contains(v)).collect()),
            (a, b) => a.or(b),
        })
    }

    /// `schema_count` over every schema that certainly applies.
    fn count_at(&self, n: &Json, filtered: bool) -> Ints {
        self.applying(n).into_iter().fold(Ints::between(Some(0), None), |s, x| s.intersect(&schema_count(x, filtered)))
    }

    /// `schema_unread` over every schema that certainly applies.
    fn unread_at(&self, n: &Json, strings: bool) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for x in self.applying(n) {
            for k in schema_unread(x, strings) {
                if !out.contains(&k) {
                    out.push(k);
                }
            }
        }
        out
    }
}

fn schema_count(n: &Json, filtered: bool) -> Ints {
    let min = get(n, "minItems").and_then(rat_of).map(ceil).unwrap_or(0).max(0);
    let max = get(n, "maxItems").and_then(rat_of).map(floor);
    Ints::between(Some(if filtered { 0 } else { min }), max)
}

/// Whether Protovalidate lets an unset field of implicit presence through whatever the other
/// rules say: `IGNORE_IF_ZERO_VALUE`, and the two names it had before.
fn ignores_zero(r: &crate::proto::Rules) -> bool {
    r.ignore.as_deref().is_some_and(|g| g.starts_with("IGNORE_IF"))
}

/// The integers Protovalidate lets through a field. An unset field of implicit presence is its
/// zero value, and the rules are applied to it like to any other value — which is why a field
/// with no rules at all lets 0 through.
fn proto_ints(fd: &crate::proto::Field) -> Option<Ints> {
    let (tmin, tmax) = crate::proto::int_bounds(&fd.ty)?;
    let full = Ints::between(Some(tmin), Some(tmax));
    let r = &fd.rules;
    if r.ignore.as_deref() == Some("IGNORE_ALWAYS") {
        return Some(open_ends(full, tmin, tmax));
    }
    let i = &r.int;
    let lower = i.gt.map(|g| g + 1).or(i.gte);
    let upper = i.lt.map(|l| l - 1).or(i.lte);
    // Protovalidate reads a lower bound written above the upper one as the two ends of an
    // excluded range: `gt: 10, lt: 5` lets through what is above 10 or below 5.
    let reversed = matches!((i.gt.or(i.gte), i.lt.or(i.lte)), (Some(a), Some(b)) if a > b);
    let mut s = if reversed {
        Ints::between(None, upper).union(&Ints::between(lower, None))
    } else {
        Ints::between(lower, upper)
    }
    .intersect(&full);
    if let Some(k) = i.konst {
        s = s.intersect(&Ints::points(&[k]));
    }
    if !i.in_.is_empty() {
        s = s.intersect(&Ints::points(&i.in_));
    }
    for x in &i.not_in {
        s = s.without(*x);
    }
    // `required` rules 0 out only where the field has no presence of its own; on an `optional`
    // field it asks that the field be set, and a set 0 passes it (§15.140).
    if r.required && !fd.optional {
        s = s.without(0);
    }
    if !fd.optional && ignores_zero(r) {
        s = s.union(&Ints::points(&[0]));
    }
    Some(open_ends(s, tmin, tmax))
}

/// A bound that is only the kind's own limit is shown open: an `int64` with no rule reads as
/// "any integer", not as nineteen digits. The 0 an unsigned kind starts at stays, because it
/// says something.
fn open_ends(s: Ints, tmin: i128, tmax: i128) -> Ints {
    Ints(s.0.into_iter().map(|(a, b)| (a.filter(|&x| x != tmin || tmin == 0), b.filter(|&x| x != tmax))).collect())
}

fn proto_strs(fd: &crate::proto::Field) -> Option<Vec<String>> {
    let r = &fd.rules;
    if r.ignore.as_deref() == Some("IGNORE_ALWAYS") {
        return None;
    }
    let mut list = r.str_const.clone().map(|k| vec![k]);
    if !r.str_in.is_empty() {
        list = Some(match list {
            Some(l) => l.into_iter().filter(|x| r.str_in.contains(x)).collect(),
            None => r.str_in.clone(),
        });
    }
    let mut list = list?;
    list.retain(|x| !r.str_not_in.contains(x));
    // The zero value of a string is "", and IGNORE_IF_ZERO_VALUE lets it through unchecked.
    if !fd.optional && ignores_zero(r) && !list.iter().any(String::is_empty) {
        list.push(String::new());
    }
    Some(list)
}

fn proto_count(fd: &crate::proto::Field, filtered: bool) -> Ints {
    let r = &fd.rules;
    if r.ignore.as_deref() == Some("IGNORE_ALWAYS") {
        return Ints::between(Some(0), None);
    }
    let min = r.min_items.unwrap_or(0).max(if r.required { 1 } else { 0 });
    let s = Ints::between(Some(if filtered { 0 } else { min }), r.max_items);
    if ignores_zero(r) { s.union(&Ints::points(&[0])) } else { s }
}

/// The declared range as it was written: `>=1g <=40kg`.
fn range_text(i: &VarDecl) -> String {
    let Some(r) = &i.range else { return String::new() };
    r.bounds.iter().map(|(op, l)| format!("{}{}", op.word(), lit_text(l))).collect::<Vec<_>>().join(" ")
}

fn lit_text(l: &Lit) -> String {
    match l {
        Lit::Num(n) => n.raw.clone(),
        Lit::Word(w) => w.clone(),
        Lit::Str(s) => format!("\"{s}\""),
        Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
    }
}

fn quoted(vs: &[String]) -> String {
    vs.iter().map(|s| crate::json::quote(s)).collect::<Vec<_>>().join(", ")
}

/// The hint every E122 ends on: which way out, and that choosing is not the tool's.
fn two_ways(fix: Option<&str>, widen: String) -> String {
    match fix {
        Some(f) => tr!(
            "ヒント: その値が来ないはずなら、契約に {f} と書いて狭めてください。来るのなら、{widen}。どちらにするかは人が決めることです。",
            "hint: if that value cannot occur, narrow the contract with {f}. If it can, {widen}. Which of the two is a person's decision."
        ),
        None => tr!("ヒント: {widen}。", "hint: {widen}."),
    }
}

fn unread_note(unread: &[String]) -> Option<String> {
    (!unread.is_empty()).then(|| {
        tr!(
            "このフィールドには、読んでいない規則があります（{}）。それが値を絞っているなら、`gte`・`lte`・`in` のような標準の規則で書くと、ここで確かめられます。",
            "This field carries rules that were not read ({}). If they narrow the value, writing them as the standard rules — `gte`, `lte`, `in` — lets this check see it.",
            unread.join(", ")
        )
    })
}

/// E122: what the contract lets through where an input is read, held against what the input
/// takes. Returns the findings, and what passes when the contract names it — the set W123
/// holds the rows to.
fn holds(f: &RuleFile, con: &Contract, pr: &Projection, input: &VarDecl, c: &Checked, rule_path: &str, ac: Option<&Across>) -> (Vec<Diag>, Option<Dom>) {
    let mut out = Vec::new();
    let name = input.name.text.as_str();
    let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
    let optional = matches!(ty, Ty::Opt(_));
    let inner = match &ty {
        Ty::Opt(i) => (**i).clone(),
        t => t.clone(),
    };
    let steps: Vec<String> = pr.path.iter().map(|n| n.text.clone()).collect();
    let full = format!("{}.{}", pr.root.text, steps.join("."));
    let at = format!("{rule_path}:{}", pr.span.line);
    let schema = con.kind == EnumSource::JsonSchema;
    let chain = if schema { con.schema_chain(&steps) } else { None };
    let fields = if schema { None } else { con.proto_chain(&steps) };

    // Whether the value is there at all. JSON Schema says with `required` which fields an
    // object may leave out; the function that reads the inputs out indexes straight into the
    // object, so one it leaves out fails there. An optional input reads it as none.
    if let (Some(chain), Some(_)) = (&chain, con.doc.as_ref()) {
        if !(matches!(pr.kind, ProjKind::Field) && optional) {
            for (i, step) in steps.iter().enumerate() {
                let req = con.required(chain[i]);
                if req.iter().any(|r| r == step) {
                    continue;
                }
                let parent = std::iter::once(pr.root.text.as_str()).chain(steps[..i].iter().map(String::as_str)).collect::<Vec<_>>().join(".");
                let mut want = req.clone();
                want.push(step.clone());
                let fix = format!("\"required\": [{}]", quoted(&want));
                let hint = if matches!(pr.kind, ProjKind::Field) {
                    tr!(
                        "ヒント: いつもあるはずなら、`{parent}` に {fix} と書いてください。無いことがあるなら、入力を `{inner}?` にしてください。無いときは none として読みます。",
                        "hint: if it is always there, write {fix} on `{parent}`. If it can be missing, make the input `{inner}?`; a missing value is then read as none."
                    )
                } else {
                    tr!("ヒント: `{parent}` に {fix} と書いてください。", "hint: write {fix} on `{parent}`.")
                };
                out.push(
                    Diag::error(
                        "E122",
                        tr!("契約では `{parent}.{step}` を省略できますが、規則はこの値を必ず読みます", "The contract lets `{parent}.{step}` be left out, and the rule always reads it"),
                    )
                    .at(at.clone())
                    .mark(pr.span.clone(), tr!("`{parent}` の `required` に `{step}` がありません", "`{step}` is not in the `required` of `{parent}`"))
                    .fix(FixKind::NarrowContract, fix)
                    .note(tr!(
                        "生成した `…_from` の関数は、無いフィールドを読むと落ちます（Python なら KeyError）。",
                        "The generated `…_from` function fails on a field that is not there (a KeyError in Python)."
                    ))
                    .note(hint),
                );
            }
            // A `where` reads one field of every element.
            if let Some((field, _)) = pr.kind.test() {
                if let Some(items) = chain.last().and_then(|n| con.items(n)) {
                    let req = con.required(items);
                    if !req.iter().any(|r| *r == field.text) {
                        let mut want = req.clone();
                        want.push(field.text.clone());
                        let fix = format!("\"required\": [{}]", quoted(&want));
                        out.push(
                            Diag::error(
                                "E122",
                                tr!(
                                    "契約では要素の `{}` を省略できますが、`where` はそれを必ず読みます",
                                    "The contract lets an element leave out `{}`, and the `where` always reads it",
                                    field.text
                                ),
                            )
                            .at(at.clone())
                            .mark(field.span.clone(), tr!("`{full}` の要素の `required` にありません", "not in the `required` of an element of `{full}`"))
                            .fix(FixKind::NarrowContract, fix.clone())
                            .note(tr!(
                                "生成した `…_from` の関数は、無いフィールドを読むと落ちます（Python なら KeyError）。",
                                "The generated `…_from` function fails on a field that is not there (a KeyError in Python)."
                            ))
                            .note(tr!("ヒント: `{full}` の要素に {fix} と書いてください。", "hint: write {fix} on an element of `{full}`.")),
                        );
                    }
                }
            }
        }
    }

    let leaf_schema = chain.as_ref().and_then(|ch| ch.last().copied());
    let leaf_proto = fields.as_ref().and_then(|fs| fs.last().copied());
    let mut dom = None;
    // A value the schema lets be null is one the function that reads it out cannot take,
    // unless the input may be missing (§15.140).
    if let (Some(n), ProjKind::Field, false) = (leaf_schema, &pr.kind, optional) {
        if con.schema_type(n).1 {
            out.push(null_passes(&at, pr, con, n, &full, &inner));
        }
    }
    match &pr.kind {
        ProjKind::Field => match &inner {
            Ty::Money { .. } | Ty::Qty { .. } | Ty::Number | Ty::Rate => {
                let Some((Some(lo), Some(hi))) = c.ranges.get(name).copied() else { return (out, dom) };
                let sc = c.wire_scale(name);
                let (rl, rh) = (crate::types::wire_int(lo, sc), crate::types::wire_int(hi, sc));
                // A value the contract carries as a fraction is not compared: the wire of a
                // rule's number is a whole number (§10.2), and a fraction there is already the
                // mismatch E120 is about.
                let seen = match (leaf_schema, leaf_proto, con.doc.as_ref()) {
                    (Some(n), _, Some(_)) if con.kind_at(n) == At::Int => Some((con.ints_at(n), con.unread_at(n, false), None)),
                    (_, Some(fd), _) => proto_ints(fd).map(|s| (s, fd.rules.unread.clone(), Some(fd.ty.rsplit('.').next().unwrap_or(&fd.ty).to_string()))),
                    _ => None,
                };
                // Under a step the contract lets be unset, the value arrives as its default
                // whatever the rules on it say: Protovalidate validates nothing under an unset
                // message, nor an unset `optional` field (§15.133).
                let unset = fields.as_deref().filter(|_| !optional).and_then(|fs| unset_step(fs, true));
                let refused = unset.filter(|_| !(rl <= 0 && 0 <= rh)).map(|(si, _)| defaulted(&at, pr, &steps, si, &full, "0", WVal::Int(0), name, &inner));
                let Some((seen, mut unread, kind)) = seen else {
                    out.extend(refused);
                    return (out, dom);
                };
                // What the rules across the fields leave of it (§15.140).
                let seen = match ac.and_then(|a| a.ints(name)) {
                    Some(s) => seen.intersect(&s),
                    None => seen,
                };
                if leaf_proto.is_some_and(|fd| con.cel_unread(fd, &steps)) {
                    unread.push("cel".into());
                }
                let below = seen.intersect(&Ints::between(None, Some(rl - 1)));
                let above = seen.intersect(&Ints::between(Some(rh + 1), None));
                if !below.is_empty() || !above.is_empty() {
                    let v = below.max().or(above.min()).unwrap_or(rl - 1);
                    let (a, b) = (seen.min().map_or(rl, |m| m.max(rl)), seen.max().map_or(rh, |m| m.min(rh)));
                    let fix = match &kind {
                        Some(k) => format!("[(buf.validate.field).{k} = {{gte: {a}, lte: {b}}}]"),
                        None => format!("\"minimum\": {a}, \"maximum\": {b}"),
                    };
                    let mut d = Diag::error("E122", tr!("契約は `{full}` に {v} を通しますが、規則はそれを断ります", "The contract lets `{full}` be {v}, which the rule refuses"))
                        .at(at.clone())
                        .win(name, WVal::Int(v))
                        .mark(
                            pr.span.clone(),
                            tr!("契約では {}、入力 {name} の範囲は {}", "the contract lets through {}; {name} takes {}", seen.describe(), range_text(input)),
                        )
                        .fix(FixKind::NarrowContract, fix.clone())
                        .note(refused_note());
                    if let Some(u) = unread_note(&unread) {
                        d = d.note(u);
                    }
                    out.push(d.note(two_ways(Some(&fix), tr!("規則の範囲を広げて、その値の答えを決めてください", "widen the rule's range and decide what it answers there"))));
                }
                out.extend(refused);
                let seen = if unset.is_some() { seen.union(&Ints::points(&[0])) } else { seen };
                dom = Some(Dom::Ints(seen, false));
            }
            Ty::Enum(e) => {
                let vals = c.enums.get(e).cloned().unwrap_or_default();
                let seen = match (leaf_schema, leaf_proto) {
                    (Some(n), _) => Some((con.strs_at(n), con.unread_at(n, true), false)),
                    (_, Some(fd)) => Some((proto_strs(fd), fd.rules.unread.clone(), true)),
                    _ => None,
                };
                let Some((list, mut unread, proto)) = seen else { return (out, dom) };
                // What the rules across the fields leave of it (§15.140). The "" an unset
                // message on the way reads as is said by its own finding below, not here.
                let unset_ahead = fields.as_deref().filter(|_| !optional).and_then(|fs| unset_step(fs, true)).is_some();
                let across = ac.and_then(|a| a.strs(name)).map(|mut r| {
                    if unset_ahead {
                        r.retain(|x| !x.is_empty());
                    }
                    r
                });
                let list = match (list, across) {
                    (Some(l), Some(r)) => Some(l.into_iter().filter(|x| r.contains(x)).collect()),
                    (None, Some(r)) => Some(r),
                    (l, None) => l,
                };
                if leaf_proto.is_some_and(|fd| con.cel_unread(fd, &steps)) {
                    unread.push("cel".into());
                }
                let keep: Vec<String> = match &list {
                    Some(l) => l.iter().filter(|x| vals.contains(x)).cloned().collect(),
                    None => vals.clone(),
                };
                let fix = if proto {
                    format!("[(buf.validate.field).string = {{in: [{}]}}]", quoted(&keep))
                } else {
                    format!("\"enum\": [{}]", quoted(&keep))
                };
                let label = tr!(
                    "契約では {}、列挙 {e} は {}",
                    "the contract lets through {}; enum {e} has {}",
                    list.as_ref().map(|l| quoted(l)).unwrap_or_else(|| tr!("どの文字列でも", "any string")),
                    quoted(&vals)
                );
                let widen = tr!("列挙に値を足して、その値の答えを決めてください", "add the value to the enum and decide what it answers there");
                let extra = list.as_ref().and_then(|l| l.iter().find(|x| !vals.contains(x)).cloned());
                let mut d = match (&list, &extra) {
                    (Some(_), Some(x)) => Some(
                        Diag::error("E122", tr!("契約は `{full}` に \"{x}\" を通しますが、列挙 {e} にその値はありません", "The contract lets `{full}` be \"{x}\", which enum {e} does not have"))
                            .win(name, WVal::Str(x.clone())),
                    ),
                    (None, _) => Some(Diag::error(
                        "E122",
                        tr!(
                            "契約は `{full}` にどの文字列でも通しますが、規則が受け付けるのは列挙 {e} の値だけです",
                            "The contract lets any string through at `{full}`, and the rule takes only the values of enum {e}"
                        ),
                    )),
                    _ => None,
                };
                if let Some(mut dd) = d.take() {
                    dd = dd.at(at.clone()).mark(pr.span.clone(), label).note(refused_note());
                    // Narrowing to none of the enum's values is no fix: an empty `in` is no rule.
                    dd = if keep.is_empty() {
                        let aliases: Vec<String> =
                            f.enums.iter().find(|d| d.name.text == *e).map(|d| d.values.iter().filter_map(|v| v.ascii.clone()).collect()).unwrap_or_default();
                        let spelled = list.as_ref().is_some_and(|l| !l.is_empty() && l.iter().all(|x| aliases.contains(x)));
                        let dd = dd.fix_kind(FixKind::None);
                        if spelled {
                            dd.note(tr!(
                                "契約の通す値は、列挙 {e} の別名です（{}）。運ばれるのは、規則に書いた値の名前（{}）のほうです。値の名前を契約の綴りにそろえるか、呼び出し側に名前を送らせてください。",
                                "The values the contract lets through are the aliases of enum {e} ({}), and what travels is the name the rule gives a value ({}). Name the values the way the contract spells them, or have the caller send the names.",
                                quoted(&aliases),
                                quoted(&vals)
                            ))
                        } else {
                            dd.note(tr!("契約の通す値に、列挙 {e} の値は一つもありません。", "None of the values the contract lets through is a value of enum {e}."))
                                .note(two_ways(None, widen))
                        }
                    } else {
                        dd.fix(FixKind::NarrowContract, fix.clone()).note(two_ways(Some(&fix), widen))
                    };
                    if let Some(u) = unread_note(&unread) {
                        dd = dd.note(u);
                    }
                    out.push(dd);
                }
                let unset = fields.as_deref().filter(|_| !optional).and_then(|fs| unset_step(fs, true));
                if let Some((si, _)) = unset.filter(|_| !vals.iter().any(String::is_empty)) {
                    out.push(defaulted(&at, pr, &steps, si, &full, "\"\"", WVal::Str(String::new()), name, &inner));
                }
                if let Some(mut l) = list {
                    if unset.is_some() && !l.iter().any(String::is_empty) {
                        l.push(String::new());
                    }
                    dom = Some(Dom::Strs(l));
                }
            }
            // A date travels as a string, and the dates themselves are not compared (§15.132).
            // The one string that is compared is "", which protojson leaves out and the function
            // reads back, and which is not a date (§15.133).
            Ty::Date => {
                let Some(fs) = fields.as_deref() else { return (out, dom) };
                if let Some((si, _)) = fs.last().and_then(|_| if optional { None } else { unset_step(fs, true) }) {
                    out.push(defaulted(&at, pr, &steps, si, &full, "\"\"", WVal::Str(String::new()), name, &inner));
                }
                if let Some(fd) = fs.last().filter(|fd| empty_passes(fd)) {
                    out.push(empty_date(&at, pr, fd, &full, name, optional));
                }
            }
            _ => {}
        },
        ProjKind::Count(t) => {
            let Some((Some(lo), Some(hi))) = c.ranges.get(name).copied() else { return (out, dom) };
            let (rl, rh) = (crate::types::wire_int(lo, 1), crate::types::wire_int(hi, 1));
            let filtered = t.is_some();
            let seen = match (leaf_schema, leaf_proto) {
                (Some(n), _) => Some((con.count_at(n, filtered), con.unread_at(n, false), false)),
                (_, Some(fd)) => Some((proto_count(fd, filtered), fd.rules.unread.clone(), true)),
                _ => None,
            };
            let Some((seen, mut unread, proto)) = seen else { return (out, dom) };
            let seen = match ac.filter(|_| !filtered).and_then(|a| a.ints(name)) {
                Some(s) => seen.intersect(&s),
                None => seen,
            };
            if leaf_proto.is_some_and(|fd| con.cel_unread(fd, &steps)) {
                unread.push("cel".into());
            }
            let below = seen.intersect(&Ints::between(None, Some(rl - 1)));
            let above = seen.intersect(&Ints::between(Some(rh + 1), None));
            if !below.is_empty() || !above.is_empty() {
                let v = below.max().or(above.min()).unwrap_or(rl - 1);
                let (a, b) = (seen.min().map_or(rl, |m| m.max(rl)), seen.max().map_or(rh, |m| m.min(rh)));
                let min_part = |p: &str| if a > 0 { format!("{p}: {a}, ") } else { String::new() };
                // How many elements a `where` picks out has no floor a contract can state.
                let fix = (!(filtered && !below.is_empty())).then(|| {
                    if proto {
                        format!("[(buf.validate.field).repeated = {{{}max_items: {b}}}]", min_part("min_items"))
                    } else {
                        format!("{}\"maxItems\": {b}", min_part("\"minItems\""))
                    }
                });
                let title = if filtered {
                    tr!(
                        "契約は `{full}` のうち `where` に当てはまる要素が {v} 件でも通しますが、規則はそれを断ります",
                        "The contract lets `{full}` hold {v} elements the `where` picks out, which the rule refuses"
                    )
                } else {
                    tr!("契約は `{full}` が {v} 件でも通しますが、規則はそれを断ります", "The contract lets `{full}` hold {v}, which the rule refuses")
                };
                let mut d = Diag::error("E122", title)
                    .at(at.clone())
                    .win(name, WVal::Int(v))
                    .mark(
                        pr.span.clone(),
                        tr!("契約では {}、入力 {name} の範囲は {}", "the contract lets through {}; {name} takes {}", seen.describe_in("件"), range_text(input)),
                    )
                    .note(refused_note());
                d = match &fix {
                    Some(f) => d.fix(FixKind::NarrowContract, f.clone()),
                    None => d.fix_kind(FixKind::None).note(tr!(
                        "`where` で絞った件数の下限は、契約には書けません。",
                        "A floor on how many elements a `where` picks out cannot be written in a contract."
                    )),
                };
                if let Some(u) = unread_note(&unread) {
                    d = d.note(u);
                }
                out.push(d.note(two_ways(fix.as_deref(), tr!("規則の範囲を広げて、その件数のときの答えを決めてください", "widen the rule's range and decide what it answers for that many"))));
            }
            let unset = fields.as_deref().and_then(|fs| unset_step(fs, false));
            // What a `where` picks out can be none at all, and the finding above says so already;
            // making the message `required` would not change it.
            if let Some((si, _)) = unset.filter(|_| rl > 0 && !filtered) {
                out.push(defaulted(&at, pr, &steps, si, &full, &tr!("0 件", "0 elements"), WVal::Int(0), name, &inner));
            }
            let seen = if unset.is_some() { seen.union(&Ints::points(&[0])) } else { seen };
            dom = Some(Dom::Ints(seen, true));
        }
        ProjKind::Any(..) | ProjKind::All(..) => {}
    }
    (out, dom)
}

/// E122 for a default: a `.proto` step that may be unset, under which the value read arrives
/// as its default with no rule of the contract applied to it (§15.133).
#[allow(clippy::too_many_arguments)]
fn defaulted(at: &str, pr: &Projection, steps: &[String], si: usize, full: &str, zero: &str, w: WVal, name: &str, inner: &Ty) -> Diag {
    let step = std::iter::once(pr.root.text.as_str()).chain(steps[..=si].iter().map(String::as_str)).collect::<Vec<_>>().join(".");
    let fix = "[(buf.validate.field).required = true]".to_string();
    let why = if si + 1 < steps.len() {
        tr!(
            "`{step}` が無いとき、Protovalidate はその中の規則を検証しません。生成した `…_from` の関数は、無いフィールドを既定値として読みます。",
            "Protovalidate validates nothing inside `{step}` when it is not set, and the generated `…_from` function reads a field that is not there as its default."
        )
    } else {
        tr!(
            "`{step}` は `optional` なので、無いときは Protovalidate がその規則を検証しません。生成した `…_from` の関数は、無いフィールドを既定値として読みます。",
            "`{step}` is `optional`, so Protovalidate applies no rule to it when it is not set, and the generated `…_from` function reads it as its default."
        )
    };
    Diag::error(
        "E122",
        tr!(
            "契約では `{step}` を省略でき、そのとき `{full}` は {zero} として届きますが、規則はそれを断ります",
            "The contract lets `{step}` be left out, and `{full}` then arrives as {zero}, which the rule refuses"
        ),
    )
    .at(at.to_string())
    .win(name, w)
    .mark(pr.span.clone(), tr!("`{step}` に `required` がありません", "`{step}` is not `required`"))
    .fix(FixKind::NarrowContract, fix.clone())
    .note(why)
    .note(tr!(
        "ヒント: いつもあるはずなら、`{step}` に {fix} と書いてください。無いことがあるなら、入力を `{inner}?` にしてください。無いときは none として読みます。",
        "hint: if it is always there, write {fix} on `{step}`. If it can be missing, make the input `{inner}?`; a missing value is then read as none."
    ))
}

/// Whether Protovalidate lets "" through a string field: nothing on it rules the empty string
/// out, or it is told to skip the rules for the zero value. `required` rules it out only where
/// the field has no presence of its own; on an `optional` one it asks only that the field be
/// set. A rule this does not read (a `pattern`, say) is read as not there, as everywhere else
/// (§15.132).
fn empty_passes(fd: &crate::proto::Field) -> bool {
    let r = &fd.rules;
    if r.ignore.as_deref() == Some("IGNORE_ALWAYS") || ignores_zero(r) {
        return true;
    }
    if (r.required && !fd.optional) || r.str_min_len.is_some_and(|n| n >= 1) || r.str_not_in.iter().any(String::is_empty) {
        return false;
    }
    if let Some(k) = &r.str_const {
        return k.is_empty();
    }
    r.str_in.is_empty() || r.str_in.iter().any(String::is_empty)
}

/// E122 for a date read from a `.proto` string that may be "": not a date, and the function
/// that reads it out cannot turn it into one (§15.133).
fn empty_date(at: &str, pr: &Projection, fd: &crate::proto::Field, full: &str, name: &str, optional: bool) -> Diag {
    // On an `optional` field, `required` asks only that it be set, and a set "" passes it.
    let fix = if fd.optional { "[(buf.validate.field).string.min_len = 1]" } else { "[(buf.validate.field).required = true]" }.to_string();
    let mut d = Diag::error("E122", tr!("契約は `{full}` に \"\" を通しますが、\"\" は日付ではありません", "The contract lets `{full}` be \"\", which is not a date"))
        .at(at.to_string())
        .win(name, WVal::Str(String::new()))
        .mark(pr.span.clone(), tr!("`{}` は \"\" を断りません", "nothing on `{}` rules \"\" out", fd.name))
        .fix(FixKind::NarrowContract, fix.clone())
        .note(if fd.optional {
            tr!(
                "`{}` に \"\" を入れて送ると、Protovalidate はそれを通し、生成した `…_from` の関数はそれを日付として読めずに落ちます。",
                "A message that sets `{}` to \"\" passes Protovalidate, and the generated `…_from` function fails on it, since it cannot read it as a date.",
                fd.name
            )
        } else {
            tr!(
                "protojson は空の文字列を出さず、生成した `…_from` の関数は無い `{}` を \"\" として読みます。\"\" は日付として読めず、関数はそこで落ちます。",
                "protojson leaves an empty string out, and the generated `…_from` function reads a missing `{}` as \"\" — which it cannot read as a date, and fails there.",
                fd.name
            )
        });
    if let Some(u) = unread_note(&fd.rules.unread) {
        d = d.note(u);
    }
    d.note(if optional && !fd.optional {
        tr!(
            "ヒント: いつもあるはずなら、`{full}` に {fix} と書いてください。無いことがあるなら、`.proto` でそのフィールドに `optional` を付けてください。付けたフィールドが無いときは none として読みます。",
            "hint: if it is always there, write {fix} on `{full}`. If it can be missing, mark the field `optional` in the `.proto`; an unset one is then read as none."
        )
    } else {
        tr!(
            "ヒント: `{full}` に {fix} と書いてください。Protovalidate が \"\" を断るようになります。",
            "hint: write {fix} on `{full}`, and Protovalidate refuses \"\"."
        )
    })
}

/// E122 for null: the schema lets the value be null, and an input that is not optional does
/// not take it (§15.140).
fn null_passes(at: &str, pr: &Projection, con: &Contract, n: &Json, full: &str, inner: &Ty) -> Diag {
    let ty = con.schema_type(n).0.unwrap_or_default();
    let by_keyword = con.applying(n).iter().any(|x| matches!(get(x, "nullable"), Some(Json::Bool(true))));
    let fix = if by_keyword { "\"nullable\": false".to_string() } else { format!("\"type\": \"{ty}\"") };
    Diag::error("E122", tr!("契約は `{full}` に null を通しますが、規則はそれを断ります", "The contract lets `{full}` be null, which the rule refuses"))
        .at(at.to_string())
        .mark(
            pr.span.clone(),
            if by_keyword { tr!("`nullable` が付いています", "it is marked `nullable`") } else { tr!("型に \"null\" があります", "its type has \"null\" in it") },
        )
        .fix(FixKind::NarrowContract, fix.clone())
        .note(tr!(
            "生成した `…_from` の関数は、省略できない入力に null が来ると落ちます。",
            "The generated `…_from` function fails when null comes for an input that is not optional."
        ))
        .note(tr!(
            "ヒント: null が来ないはずなら、`{full}` を {fix} と書いてください。来るのなら、入力を `{inner}?` にしてください。null は none として読みます。",
            "hint: if null cannot come, write {fix} on `{full}`. If it can, make the input `{inner}?`: null is then read as none."
        ))
}

impl Contract {
    /// Whether a CEL expression on the field at `path` has a part that could not be read.
    fn cel_unread(&self, fd: &crate::proto::Field, path: &[String]) -> bool {
        let kind = |p: &[String]| self.proto_kind(p);
        fd.rules.cel.iter().any(|e| crate::cel::read_src(e, path, &kind).has_unknown())
    }
}

fn refused_note() -> String {
    tr!(
        "契約の検証を通っても、この値では生成コードが入口で断ります。API なら要求が誤りとして返り、Kafka の消費側なら処理が止まるか DLQ に回ります。",
        "A value that passes the contract's validation is still refused at the door of the generated code: an API answers the request with an error, and a Kafka consumer stops or sends the message to the DLQ."
    )
}

/// The wire integers a numeric cell admits, at the scale the input travels at.
fn cell_ints(cell: &Cell, ty: &Ty, sc: i128) -> Option<Ints> {
    let at = |l: &Lit| match l {
        Lit::Num(n) => crate::types::lit_value_in_pub(n, ty).map(|v| v.mul(Rat::int(sc))),
        _ => None,
    };
    match cell {
        Cell::DontCare => Some(Ints::between(None, None)),
        Cell::Lit(l) => {
            let v = at(l)?;
            Some(if v.is_int() { Ints::points(&[v.num]) } else { Ints(Vec::new()) })
        }
        Cell::Cmp(ops) => {
            let mut s = Ints::between(None, None);
            for (op, l) in ops {
                let v = at(l)?;
                s = s.intersect(&match op {
                    CmpOp::Ge => Ints::between(Some(ceil(v)), None),
                    CmpOp::Gt => Ints::between(Some(floor(v) + 1), None),
                    CmpOp::Le => Ints::between(None, Some(floor(v))),
                    CmpOp::Lt => Ints::between(None, Some(ceil(v) - 1)),
                });
            }
            Some(s)
        }
        _ => None,
    }
}

/// The enum values a cell admits, groups opened up.
fn cell_values(cell: &Cell, c: &Checked, all: &[String]) -> Option<Vec<String>> {
    let open = |ls: &[Lit]| -> Vec<String> {
        ls.iter()
            .filter_map(|l| if let Lit::Word(w) = l { Some(w.clone()) } else { None })
            .flat_map(|w| match c.groups.get(&w) {
                Some((_, members)) => members.clone(),
                None => vec![w],
            })
            .collect()
    };
    match cell {
        Cell::DontCare => Some(all.to_vec()),
        Cell::Lit(l) => Some(open(std::slice::from_ref(l))),
        Cell::Set(ls) => Some(open(ls)),
        Cell::Not(ls) => {
            let out = open(ls);
            Some(all.iter().filter(|v| !out.contains(v)).cloned().collect())
        }
        _ => None,
    }
}

/// W123: a row whose cell on a projected input admits nothing the contract lets through. The
/// cell is on the input itself, so the row can only be reached by a value that did not pass
/// the contract's validation; a cell on anything derived from it is not judged.
fn dead_rows(f: &RuleFile, c: &Checked, doms: &[(String, Dom, &Projection)], rule_path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        if t.applied.is_some() {
            continue;
        }
        let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        for (ci, (col, _)) in t.inputs.iter().enumerate() {
            let Some((_, dom, pr)) = doms.iter().find(|(n, _, _)| n == col) else { continue };
            let ty = match c.ty_of(col) {
                Some(Ty::Opt(i)) => *i,
                Some(t) => t,
                None => continue,
            };
            let full = format!("{}.{}", pr.root.text, pr.path.iter().map(|n| n.text.as_str()).collect::<Vec<_>>().join("."));
            for row in &t.rows {
                let Some(cell) = row.cells.get(ci) else { continue };
                let (dead, desc) = match dom {
                    Dom::Ints(s, count) => match cell_ints(cell, &ty, c.wire_scale(col)) {
                        Some(ci) => (ci.intersect(s).is_empty(), if *count { s.describe_in("件") } else { s.describe() }),
                        None => continue,
                    },
                    Dom::Strs(l) => {
                        let all = match &ty {
                            Ty::Enum(e) => c.enums.get(e).cloned().unwrap_or_default(),
                            _ => continue,
                        };
                        match cell_values(cell, c, &all) {
                            Some(vs) => (!vs.iter().any(|v| l.contains(v)), quoted(l)),
                            None => continue,
                        }
                    }
                };
                if !dead {
                    continue;
                }
                let (kind, rn) = if t.clause {
                    (tr!("節", "clause"), tr!("節 {tname}", "clause {tname}"))
                } else {
                    let base = tr!("行{}", "row {}", row.index);
                    (
                        tr!("表", "table"),
                        match &row.label {
                            Some(l) => tr!("{base}（{}）", "{base} ({})", l.text),
                            None => base,
                        },
                    )
                };
                let span = row.cell_spans.get(ci).cloned().unwrap_or_else(|| row.span.clone());
                out.push(
                    Diag::warning("W123", tr!("{rn} は、契約が通さない値でしか当たりません", "{rn} is reached only by values the contract does not let through"))
                        .at(format!("{rule_path}:{} {kind} {tname}", row.span.line))
                        .table(tname.clone())
                        .row(row.index)
                        .rowref(tname.clone(), row.index)
                        .key(format!("W123\u{1}{tname}\u{1}{col}\u{1}{}", crate::region::row_key(row)))
                        .fix_kind(FixKind::None)
                        .mark(span, tr!("`{full}` は契約では {desc}", "the contract lets `{full}` be {desc}"))
                        .note(tr!(
                            "`{full}` から来る値は契約の検証を通ったものだけなので、この行に当たる要求やメッセージは来ません。",
                            "What comes from `{full}` has passed the contract's validation, so no request or message reaches this row."
                        ))
                        .note(tr!(
                            "ヒント: 契約がこの先も広がらないなら、この行を消して、入力の範囲を契約に合わせてください。広がる予定があって残しているのなら、このままで構いません。",
                            "hint: if the contract will not widen, delete the row and bring the input's range in line with the contract. If it is kept for a widening that is planned, leave it."
                        )),
                );
            }
        }
    }
    out
}

// --- What the contract says across its fields (§15.140) ----------------------------------

/// A contract's condition over the fields a rule reads, and the names its terms go by in the
/// systems the rule's side is added to (§15.140).
pub(crate) struct Across {
    pub rel: Relation,
    /// The term each input reads, by input name: an input that is not optional and that the
    /// contract gives the kind the rule reads it as — a whole number, a string for an enum, a
    /// boolean, or the number of elements of a collection.
    terms: BTreeMap<String, Term>,
    names: BTreeMap<Term, String>,
    /// Whether some rule of the contract could not be read and was taken as true.
    pub unread: bool,
}

impl Across {
    /// The name a term goes by in a linear system: the input that reads it, or a name no input
    /// can have.
    fn name(&self, t: &Term) -> String {
        self.names.get(t).cloned().unwrap_or_else(|| format!("\u{1}{}", t.word()))
    }

    fn term(&self, input: &str) -> Option<&Term> {
        self.terms.get(input)
    }

    /// The whole values an input can take across the contract, when the condition narrows
    /// them at all.
    fn ints(&self, input: &str) -> Option<Ints> {
        let t = self.term(input)?;
        let spans = self.rel.span(t, &Extra::default(), &|t| self.name(t))?;
        let mut out = Ints(Vec::new());
        for (lo, hi, holes) in spans {
            let mut s = Ints::between(lo, hi);
            for h in holes {
                s = s.without(h);
            }
            out = out.union(&s);
        }
        Some(out)
    }

    /// The strings an input can be across the contract, when every case lists them.
    fn strs(&self, input: &str) -> Option<Vec<String>> {
        self.rel.strings(self.term(input)?, &|t| self.name(t))
    }
}

fn step(at: &[String], name: &str) -> Vec<String> {
    let mut p = at.to_vec();
    p.push(name.to_string());
    p
}

/// A boolean schema: `true` lets everything through, `false` nothing. Anything else is not a
/// schema this reads.
fn schema_bool(v: &Json) -> Formula {
    match v {
        Json::Bool(true) => Formula::True,
        Json::Bool(false) => Formula::False,
        _ => Formula::unknown(),
    }
}

/// Whether a `type` keyword lets a value of this JSON type through.
fn type_allows(x: &Json, ty: &str) -> bool {
    let ok = |t: &str| t == ty || (ty == "integer" && t == "number");
    match x {
        Json::Str(t) => ok(t),
        Json::Arr(ts) => ts.iter().filter_map(Json::as_str).any(ok),
        _ => true,
    }
}

impl Contract {
    /// The condition the contract of `shape` places on what the rule reads from it.
    pub(crate) fn across(&self, f: &RuleFile, c: &Checked, shape: &str) -> Across {
        let mut terms = BTreeMap::new();
        let mut names = BTreeMap::new();
        for i in &f.inputs {
            let Some(pr) = &i.from else { continue };
            if pr.root.text != shape {
                continue;
            }
            let ty = c.ty_of(&i.name.text).unwrap_or(Ty::Unknown);
            let path: Vec<String> = pr.path.iter().map(|n| n.text.clone()).collect();
            let t = match (&pr.kind, &ty, self.kind_of(&path)) {
                (ProjKind::Field, Ty::Money { .. } | Ty::Qty { .. } | Ty::Number | Ty::Rate, Some(Kind::Int { .. })) => Term::Field(path),
                (ProjKind::Field, Ty::Enum(_), Some(Kind::Str { .. })) => Term::Field(path),
                (ProjKind::Field, Ty::Bool, Some(Kind::Bool { .. })) => Term::Field(path),
                (ProjKind::Count(None), _, Some(Kind::List)) => Term::Size(path),
                _ => continue,
            };
            if names.contains_key(&t) {
                continue;
            }
            names.insert(t.clone(), i.name.text.clone());
            terms.insert(i.name.text.clone(), t);
        }
        let reads: Vec<Term> = terms.values().cloned().collect();
        let cond = match self.kind {
            EnumSource::JsonSchema => self.schema_condition(&reads),
            EnumSource::Proto => self.proto_condition(&reads),
        };
        Across { rel: Relation::of(&cond), terms, names, unread: cond.has_unknown() }
    }

    /// What a field is, for reading a condition about it.
    fn kind_of(&self, path: &[String]) -> Option<Kind> {
        match self.kind {
            EnumSource::JsonSchema => self.schema_kind(path),
            EnumSource::Proto => self.proto_kind(path),
        }
    }

    fn root_message(&self) -> Option<&crate::proto::Message> {
        self.msgs.iter().find(|m| crate::proto::same_message(&m.name, &self.at))
    }

    fn proto_kind(&self, path: &[String]) -> Option<Kind> {
        let mut msg = self.root_message()?;
        for (i, s) in path.iter().enumerate() {
            let fd = msg.fields.iter().find(|x| x.name == *s)?;
            if i + 1 == path.len() {
                return Some(self.field_kind(fd));
            }
            if fd.repeated {
                return None;
            }
            msg = self.msgs.iter().find(|m| crate::proto::same_message(&m.name, &fd.ty))?;
        }
        Some(Kind::Msg)
    }

    fn field_kind(&self, fd: &crate::proto::Field) -> Kind {
        if fd.repeated {
            return Kind::List;
        }
        let presence = fd.optional;
        match crate::proto::scalar(&fd.ty) {
            Some(crate::proto::Scalar::Int) => Kind::Int { presence },
            Some(crate::proto::Scalar::Str) => Kind::Str { presence },
            Some(crate::proto::Scalar::Bool) => Kind::Bool { presence },
            Some(crate::proto::Scalar::Frac) => Kind::Other,
            // An enum is its number in CEL.
            None if self.enum_of(&fd.ty).is_some() => Kind::Int { presence },
            None if self.msgs.iter().any(|m| crate::proto::same_message(&m.name, &fd.ty)) => Kind::Msg,
            None => Kind::Other,
        }
    }

    fn proto_condition(&self, reads: &[Term]) -> Formula {
        match self.root_message() {
            Some(m) => self.proto_message(m, &[], reads, 0),
            None => Formula::True,
        }
    }

    /// What validation asks of the message `m` at `at`: its own CEL, every field's rules, and
    /// that at most one member of each `oneof` is set.
    fn proto_message(&self, m: &crate::proto::Message, at: &[String], reads: &[Term], depth: usize) -> Formula {
        let kind = |p: &[String]| self.proto_kind(p);
        let mut parts: Vec<Formula> = m.rules.cel.iter().map(|e| crate::cel::read_src(e, at, &kind)).collect();
        for fd in &m.fields {
            parts.push(self.proto_field(fd, &step(at, &fd.name), reads, depth));
        }
        let mut seen = BTreeSet::new();
        for x in &parts {
            x.terms(&mut seen);
        }
        seen.extend(reads.iter().cloned());
        for o in &m.rules.oneofs {
            parts.push(self.one_set(m, at, o, &seen));
        }
        Formula::and(parts)
    }

    /// What validation asks of the field at `p`, and what the wire itself guarantees of it.
    fn proto_field(&self, fd: &crate::proto::Field, p: &[String], reads: &[Term], depth: usize) -> Formula {
        let kind = |q: &[String]| self.proto_kind(q);
        let r = &fd.rules;
        let skip = r.ignore.as_deref() == Some("IGNORE_ALWAYS");
        let k = |n: i128| Lin::con(Rat::int(n));
        let mut always = Vec::new();
        let mut rules = Vec::new();
        let zero = match self.field_kind(fd) {
            Kind::List => {
                let n = Lin::term(Term::Size(p.to_vec()));
                always.push(Formula::cmp(&k(0), Rel::Le, &n));
                if let Some(m) = r.min_items {
                    rules.push(Formula::cmp(&k(m), Rel::Le, &n));
                }
                if let Some(m) = r.max_items {
                    rules.push(Formula::cmp(&n, Rel::Le, &k(m)));
                }
                if r.required {
                    rules.push(Formula::cmp(&k(1), Rel::Le, &n));
                }
                Formula::cmp(&n, Rel::Eq, &k(0))
            }
            Kind::Int { .. } => {
                let v = Lin::term(Term::Field(p.to_vec()));
                if let Some((lo, hi)) = crate::proto::int_bounds(&fd.ty) {
                    always.push(Formula::cmp(&k(lo), Rel::Le, &v));
                    always.push(Formula::cmp(&v, Rel::Le, &k(hi)));
                }
                let i = &r.int;
                let lower = i.gt.map(|g| g + 1).or(i.gte).map(|a| Formula::cmp(&k(a), Rel::Le, &v));
                let upper = i.lt.map(|l| l - 1).or(i.lte).map(|b| Formula::cmp(&v, Rel::Le, &k(b)));
                // A lower bound above the upper one is the two ends of a range left out.
                let reversed = matches!((i.gt.or(i.gte), i.lt.or(i.lte)), (Some(a), Some(b)) if a > b);
                match (lower, upper) {
                    (Some(l), Some(h)) if reversed => rules.push(Formula::or(vec![l, h])),
                    (l, h) => rules.extend(l.into_iter().chain(h)),
                }
                if let Some(c) = i.konst {
                    rules.push(Formula::cmp(&v, Rel::Eq, &k(c)));
                }
                if !i.in_.is_empty() {
                    rules.push(Formula::or(i.in_.iter().map(|x| Formula::cmp(&v, Rel::Eq, &k(*x))).collect()));
                }
                rules.extend(i.not_in.iter().map(|x| Formula::cmp(&v, Rel::Ne, &k(*x))));
                let zero = Formula::cmp(&v, Rel::Eq, &k(0));
                if r.required && !fd.optional {
                    rules.push(zero.not());
                }
                zero
            }
            Kind::Str { .. } => {
                let t = Term::Field(p.to_vec());
                let set = |vs: Vec<String>, yes: bool| Formula::atom(Atom::Str(t.clone(), vs, yes));
                if let Some(c) = &r.str_const {
                    rules.push(set(vec![c.clone()], true));
                }
                if !r.str_in.is_empty() {
                    rules.push(set(r.str_in.clone(), true));
                }
                if !r.str_not_in.is_empty() {
                    rules.push(set(r.str_not_in.clone(), false));
                }
                if r.str_min_len.is_some_and(|n| n >= 1) {
                    rules.push(set(vec![String::new()], false));
                }
                let zero = set(vec![String::new()], true);
                if r.required && !fd.optional {
                    rules.push(zero.not());
                }
                zero
            }
            Kind::Bool { .. } => {
                let zero = Formula::atom(Atom::Bool(Term::Field(p.to_vec()), false));
                if r.required && !fd.optional {
                    rules.push(zero.not());
                }
                zero
            }
            Kind::Msg => {
                if skip {
                    return Formula::True;
                }
                // A message on the way to something read: what it asks of itself, when set.
                let mut inner: Vec<Formula> = r.cel.iter().map(|e| crate::cel::read_src(e, p, &kind)).collect();
                let on_path = reads.iter().any(|x| x.path().len() > p.len() && x.under(p));
                if let (true, Some(m)) = (on_path && depth < DEPTH, self.msgs.iter().find(|m| crate::proto::same_message(&m.name, &fd.ty))) {
                    inner.push(self.proto_message(m, p, reads, depth + 1));
                }
                let inner = Formula::and(inner);
                if r.required {
                    return inner;
                }
                // Unset, nothing under it is validated and every field there reads as its
                // default.
                let mut under = BTreeSet::new();
                inner.terms(&mut under);
                under.extend(reads.iter().cloned());
                return Formula::or(vec![inner, self.defaults(&under, p)]);
            }
            Kind::Other => Formula::True,
        };
        rules.extend(r.cel.iter().map(|e| crate::cel::read_src(e, p, &kind)));
        if skip {
            return Formula::and(always);
        }
        let rules = Formula::and(rules);
        // Where validation may skip the rules, the default gets through whatever they say.
        let gated = if (fd.optional && !r.required) || (!fd.optional && ignores_zero(r)) { Formula::or(vec![rules, zero]) } else { rules };
        Formula::and(vec![Formula::and(always), gated])
    }

    /// That every term under `p` holds its default, which is what an unset message reads as.
    fn defaults(&self, terms: &BTreeSet<Term>, p: &[String]) -> Formula {
        let zero = Lin::con(Rat::int(0));
        Formula::and(
            terms
                .iter()
                .filter(|t| t.path().len() > p.len() && t.under(p))
                .filter_map(|t| match t {
                    Term::Size(_) => Some(Formula::cmp(&Lin::term(t.clone()), Rel::Eq, &zero)),
                    Term::Field(q) => match self.proto_kind(q)? {
                        Kind::Int { .. } => Some(Formula::cmp(&Lin::term(t.clone()), Rel::Eq, &zero)),
                        Kind::Str { .. } => Some(Formula::atom(Atom::Str(t.clone(), vec![String::new()], true))),
                        Kind::Bool { .. } => Some(Formula::atom(Atom::Bool(t.clone(), false))),
                        _ => None,
                    },
                })
                .collect(),
        )
    }

    /// That at most one of a group of fields is set, said of their values: all but one of them
    /// hold their defaults. With `required`, and fields that have no presence of their own,
    /// one of them also holds something else; a field that does have presence may be set to
    /// its default, and then nothing more is said.
    fn one_set(&self, m: &crate::proto::Message, at: &[String], o: &crate::proto::Oneof, seen: &BTreeSet<Term>) -> Formula {
        let zero = Lin::con(Rat::int(0));
        let mut unset = Vec::new();
        let mut bare = true;
        for name in &o.fields {
            let Some(fd) = m.fields.iter().find(|x| x.name == *name) else { return Formula::True };
            let p = step(at, name);
            let kind = self.field_kind(fd);
            unset.push(match kind {
                Kind::Int { .. } => Formula::cmp(&Lin::term(Term::Field(p)), Rel::Eq, &zero),
                Kind::Str { .. } => Formula::atom(Atom::Str(Term::Field(p), vec![String::new()], true)),
                Kind::Bool { .. } => Formula::atom(Atom::Bool(Term::Field(p), false)),
                Kind::List => Formula::cmp(&Lin::term(Term::Size(p)), Rel::Eq, &zero),
                Kind::Msg => self.defaults(seen, &p),
                Kind::Other => Formula::True,
            });
            bare &= !fd.optional && !matches!(kind, Kind::Msg | Kind::Other);
        }
        if unset.len() < 2 {
            return Formula::True;
        }
        let mut parts = vec![Formula::or(
            (0..unset.len())
                .map(|i| Formula::and(unset.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, f)| f.clone()).collect()))
                .collect(),
        )];
        if o.required && bare {
            parts.push(Formula::or(unset.iter().map(Formula::not).collect()));
        }
        Formula::and(parts)
    }

    fn schema_root(&self) -> Option<&Json> {
        at_pointer(self.doc.as_ref()?, &self.at)
    }

    fn schema_node(&self, path: &[String]) -> Option<&Json> {
        let mut node = self.schema_root()?;
        for s in path {
            node = self.property(node, s)?;
        }
        Some(node)
    }

    /// Whether every instance has a value at `path`: each step `required` where it certainly
    /// applies. A keyword about a property that may be absent says nothing for certain — it
    /// holds of an absent one whatever it says — so only these are read as conditions.
    fn present(&self, path: &[String]) -> bool {
        let Some(mut node) = self.schema_root() else { return false };
        for s in path {
            if !self.required(node).contains(s) {
                return false;
            }
            match self.property(node, s) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    fn schema_kind(&self, path: &[String]) -> Option<Kind> {
        let node = self.schema_node(path)?;
        let (ty, nullable) = self.schema_type(node);
        if nullable {
            return None;
        }
        Some(match ty.as_deref() {
            Some("integer") => Kind::Int { presence: false },
            Some("string") => Kind::Str { presence: false },
            Some("boolean") => Kind::Bool { presence: false },
            Some("array") => Kind::List,
            Some("object") => Kind::Msg,
            None if path.is_empty() || !self.property_names(node).is_empty() => Kind::Msg,
            _ => return None,
        })
    }

    fn schema_condition(&self, reads: &[Term]) -> Formula {
        match self.schema_root() {
            Some(root) => self.schema_object(root, &[], reads, 0),
            None => Formula::True,
        }
    }

    /// What the schema `v` asks of the object at `at`: the root, or an object on the way to
    /// something the rule reads.
    fn schema_object(&self, v: &Json, at: &[String], reads: &[Term], depth: usize) -> Formula {
        if depth > DEPTH {
            return Formula::unknown();
        }
        let Json::Obj(kv) = v else { return schema_bool(v) };
        let mut parts = Vec::new();
        if let Some(r) = kv.get("$ref") {
            let to = self.target(r).map_or_else(Formula::unknown, |t| self.schema_object(t, at, reads, depth + 1));
            if !self.siblings {
                return to;
            }
            parts.push(to);
        }
        let each = |xs: &Json| -> Vec<Formula> {
            match xs {
                Json::Arr(bs) => bs.iter().map(|b| self.schema_object(b, at, reads, depth + 1)).collect(),
                _ => vec![Formula::unknown()],
            }
        };
        for (key, x) in kv {
            parts.push(match key.as_str() {
                "properties" => {
                    let Json::Obj(ps) = x else { continue };
                    Formula::and(
                        ps.iter()
                            .map(|(name, sub)| {
                                let q = step(at, name);
                                if !self.present(&q) {
                                    return Formula::unknown();
                                }
                                match self.schema_kind(&q) {
                                    Some(Kind::Msg) if reads.iter().any(|t| t.path().len() > q.len() && t.under(&q)) => {
                                        self.schema_object(sub, &q, reads, depth + 1)
                                    }
                                    Some(Kind::Msg | Kind::Other) | None => Formula::unknown(),
                                    Some(k) => self.schema_value(sub, &q, k, depth + 1),
                                }
                            })
                            .collect(),
                    )
                }
                "required" => {
                    let Json::Arr(ns) = x else { continue };
                    Formula::and(
                        ns.iter()
                            .filter_map(Json::as_str)
                            .map(|n| if self.present(&step(at, n)) { Formula::True } else { Formula::unknown() })
                            .collect(),
                    )
                }
                "allOf" => Formula::and(each(x)),
                "anyOf" | "oneOf" => Formula::or(each(x)),
                "not" => self.schema_object(x, at, reads, depth + 1).not(),
                "if" => {
                    let c = self.schema_object(x, at, reads, depth + 1);
                    let then = kv.get("then").map_or(Formula::True, |y| self.schema_object(y, at, reads, depth + 1));
                    let other = kv.get("else").map_or(Formula::True, |y| self.schema_object(y, at, reads, depth + 1));
                    Formula::or(vec![Formula::and(vec![c.clone(), then]), Formula::and(vec![c.not(), other])])
                }
                "type" => {
                    if type_allows(x, "object") {
                        Formula::True
                    } else {
                        Formula::False
                    }
                }
                "minProperties" | "maxProperties" | "dependentRequired" | "dependentSchemas" | "dependencies" | "additionalProperties"
                | "patternProperties" | "propertyNames" | "unevaluatedProperties" | "const" | "enum" => Formula::unknown(),
                // `then` and `else` go with `if`; the rest are annotations, or keywords about
                // values of another type.
                _ => continue,
            });
        }
        Formula::and(parts)
    }

    /// What the schema `v` asks of the value at `q`, which every instance has and which is of
    /// `kind`. A keyword about another type holds of it whatever it says.
    fn schema_value(&self, v: &Json, q: &[String], kind: Kind, depth: usize) -> Formula {
        if depth > DEPTH {
            return Formula::unknown();
        }
        let Json::Obj(kv) = v else { return schema_bool(v) };
        let t = Term::Field(q.to_vec());
        let val = Lin::term(t.clone());
        let n = Lin::term(Term::Size(q.to_vec()));
        let con = |r: Rat| Lin::con(r);
        let flag = |k: &str| matches!(kv.get(k), Some(Json::Bool(true)));
        let mut parts = Vec::new();
        if let Some(r) = kv.get("$ref") {
            let to = self.target(r).map_or_else(Formula::unknown, |x| self.schema_value(x, q, kind, depth + 1));
            if !self.siblings {
                return to;
            }
            parts.push(to);
        }
        let each = |xs: &Json| -> Vec<Formula> {
            match xs {
                Json::Arr(bs) => bs.iter().map(|b| self.schema_value(b, q, kind, depth + 1)).collect(),
                _ => vec![Formula::unknown()],
            }
        };
        let word = match kind {
            Kind::Int { .. } => "integer",
            Kind::Str { .. } => "string",
            Kind::Bool { .. } => "boolean",
            Kind::List => "array",
            Kind::Msg | Kind::Other => "object",
        };
        for (key, x) in kv {
            parts.push(match (key.as_str(), kind) {
                ("allOf", _) => Formula::and(each(x)),
                ("anyOf" | "oneOf", _) => Formula::or(each(x)),
                ("not", _) => self.schema_value(x, q, kind, depth + 1).not(),
                ("if", _) => {
                    let c = self.schema_value(x, q, kind, depth + 1);
                    let then = kv.get("then").map_or(Formula::True, |y| self.schema_value(y, q, kind, depth + 1));
                    let other = kv.get("else").map_or(Formula::True, |y| self.schema_value(y, q, kind, depth + 1));
                    Formula::or(vec![Formula::and(vec![c.clone(), then]), Formula::and(vec![c.not(), other])])
                }
                ("type", _) => {
                    if type_allows(x, word) {
                        Formula::True
                    } else {
                        Formula::False
                    }
                }
                ("minimum", Kind::Int { .. }) => match rat_of(x) {
                    Some(m) => Formula::cmp(&con(m), if flag("exclusiveMinimum") { Rel::Lt } else { Rel::Le }, &val),
                    None => Formula::unknown(),
                },
                ("maximum", Kind::Int { .. }) => match rat_of(x) {
                    Some(m) => Formula::cmp(&val, if flag("exclusiveMaximum") { Rel::Lt } else { Rel::Le }, &con(m)),
                    None => Formula::unknown(),
                },
                // The number form of 2019-09 on; the flag of draft-04 went with the bound above.
                ("exclusiveMinimum", Kind::Int { .. }) => match rat_of(x) {
                    Some(m) => Formula::cmp(&con(m), Rel::Lt, &val),
                    None => continue,
                },
                ("exclusiveMaximum", Kind::Int { .. }) => match rat_of(x) {
                    Some(m) => Formula::cmp(&val, Rel::Lt, &con(m)),
                    None => continue,
                },
                ("const", Kind::Int { .. }) => match rat_of(x) {
                    Some(m) => Formula::cmp(&val, Rel::Eq, &con(m)),
                    None => Formula::False,
                },
                ("enum", Kind::Int { .. }) => match x {
                    Json::Arr(vs) => {
                        Formula::or(vs.iter().filter_map(rat_of).filter(|m| m.is_int()).map(|m| Formula::cmp(&val, Rel::Eq, &con(m))).collect())
                    }
                    _ => Formula::unknown(),
                },
                ("multipleOf", Kind::Int { .. }) => Formula::unknown(),
                ("const", Kind::Str { .. }) => match x {
                    Json::Str(s) => Formula::atom(Atom::Str(t.clone(), vec![s.clone()], true)),
                    _ => Formula::False,
                },
                ("enum", Kind::Str { .. }) => match x {
                    Json::Arr(vs) => Formula::atom(Atom::Str(t.clone(), vs.iter().filter_map(Json::as_str).map(String::from).collect(), true)),
                    _ => Formula::unknown(),
                },
                ("minLength", Kind::Str { .. }) => match rat_of(x) {
                    Some(m) if m.cmp_to(Rat::int(1)).is_ge() => {
                        let some = Formula::atom(Atom::Str(t.clone(), vec![String::new()], false));
                        if m.cmp_to(Rat::int(1)).is_gt() { Formula::and(vec![some, Formula::unknown()]) } else { some }
                    }
                    _ => continue,
                },
                ("maxLength" | "pattern" | "format" | "contentEncoding" | "contentMediaType" | "contentSchema", Kind::Str { .. }) => Formula::unknown(),
                ("const", Kind::Bool { .. }) => match x {
                    Json::Bool(b) => Formula::atom(Atom::Bool(t.clone(), *b)),
                    _ => Formula::False,
                },
                ("enum", Kind::Bool { .. }) => match x {
                    Json::Arr(vs) => Formula::or(
                        vs.iter()
                            .filter_map(|v| if let Json::Bool(b) = v { Some(Formula::atom(Atom::Bool(t.clone(), *b))) } else { None })
                            .collect(),
                    ),
                    _ => Formula::unknown(),
                },
                ("minItems", Kind::List) => match rat_of(x) {
                    Some(m) => Formula::cmp(&con(m), Rel::Le, &n),
                    None => Formula::unknown(),
                },
                ("maxItems", Kind::List) => match rat_of(x) {
                    Some(m) => Formula::cmp(&n, Rel::Le, &con(m)),
                    None => Formula::unknown(),
                },
                ("items" | "prefixItems" | "additionalItems" | "contains" | "minContains" | "maxContains" | "uniqueItems" | "unevaluatedItems" | "const" | "enum", Kind::List) => {
                    Formula::unknown()
                }
                _ => continue,
            });
        }
        Formula::and(parts)
    }
}

/// The declared ranges of the inputs a relation names, at the scale they travel at: what a
/// value the generated code does not refuse at the door stays inside.
fn wire_ranges(ac: &Across, c: &Checked) -> Vec<fourier::Ineq> {
    let mut out = Vec::new();
    for name in ac.terms.keys() {
        let Some((lo, hi)) = c.ranges.get(name).copied() else { continue };
        let sc = Rat::int(c.wire_scale(name));
        let v = fourier::Lin::var(name);
        if let Some(lo) = lo {
            out.push(v.clone().plus(&fourier::Lin::con(lo.mul(sc).mul(Rat::int(-1)))).ge(false).tag(fourier::Origin::Range { name: name.clone(), hi: false }));
        }
        if let Some(hi) = hi {
            out.push(v.plus(&fourier::Lin::con(hi.mul(sc).mul(Rat::int(-1)))).le(false).tag(fourier::Origin::Range { name: name.clone(), hi: true }));
        }
    }
    out
}

/// A `constraint` between two inputs the relation names, at the scale they travel at; negated
/// when `broken`.
fn wire_constraint(k: &crate::ast::Constraint, i: usize, c: &Checked, broken: bool) -> fourier::Ineq {
    let (sl, sr) = (c.wire_scale(&k.left), c.wire_scale(&k.right));
    let d = fourier::Lin::var(&k.left).scale(Rat::new(1, sl)).plus(&fourier::Lin::var(&k.right).scale(Rat::new(-1, sr)));
    fourier::cmp(d, k.op, broken).tag(fourier::Origin::Constraint(i))
}

/// A numeric cell as inequalities on the input at the scale it travels at.
fn cell_ineqs(cell: &Cell, ty: &Ty, sc: i128, name: &str, row: usize) -> Option<Vec<fourier::Ineq>> {
    let at = |l: &Lit| match l {
        Lit::Num(n) => crate::types::lit_value_in_pub(n, ty).map(|v| v.mul(Rat::int(sc))),
        _ => None,
    };
    let tag = |part| fourier::Origin::Cell { row, col: name.to_string(), part };
    let d = |k: Rat| fourier::Lin::var(name).plus(&fourier::Lin::con(k.mul(Rat::int(-1))));
    match cell {
        Cell::DontCare => Some(Vec::new()),
        Cell::Lit(l) => {
            let k = at(l)?;
            Some(vec![d(k).le(false).tag(tag(0)), d(k).ge(false).tag(tag(1))])
        }
        Cell::Cmp(ops) => {
            let mut out = Vec::new();
            for (i, (op, l)) in ops.iter().enumerate() {
                out.push(fourier::cmp(d(at(l)?), *op, false).tag(tag(i)));
            }
            Some(out)
        }
        _ => None,
    }
}

/// The path a projection reads, as a message writes it: `注文.shipping.zone`.
fn full_path(pr: &Projection) -> String {
    std::iter::once(pr.root.text.as_str()).chain(pr.path.iter().map(|n| n.text.as_str())).collect::<Vec<_>>().join(".")
}

/// E123: a `constraint` between two inputs of one contract that the contract does not keep. A
/// message it lets through, with both values inside the rule's ranges, breaks the constraint,
/// and the generated code refuses it at the door (§15.140).
fn broken_constraints(f: &RuleFile, c: &Checked, con: &Contract, ac: &Across, rule_path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    let proj = |n: &str| f.inputs.iter().find(|i| i.name.text == n).and_then(|i| i.from.as_ref());
    for (idx, k) in f.constraints.iter().enumerate() {
        let (Some(lt), Some(rt)) = (ac.term(&k.left), ac.term(&k.right)) else { continue };
        let (Some(lp), Some(rp)) = (proj(&k.left), proj(&k.right)) else { continue };
        let mut ineqs = wire_ranges(ac, c);
        ineqs.push(wire_constraint(k, idx, c, true));
        let found = ac.rel.find(&Extra { ineqs, ..Extra::default() }, &|t| ac.name(t));
        let point = match found {
            crate::relation::Found::None(_) => continue,
            crate::relation::Found::Point(at, ..) => at.get(&k.left).zip(at.get(&k.right)).map(|(a, b)| (a.num / a.den, b.num / b.den)),
            crate::relation::Found::Unknown => None,
        };
        let (lw, rw) = (full_path(lp), full_path(rp));
        let rel = format!("{} {} {}", k.left, k.op.word(), k.right);
        let mut d = Diag::error(
            "E123",
            tr!(
                "契約は `{lw}` と `{rw}` について、`constraint {rel}` を破る組み合わせも通します",
                "The contract lets `{lw}` and `{rw}` through together where `constraint {rel}` does not hold"
            ),
        )
        .at(tr!("{rule_path}:{} 制約", "{rule_path}:{} constraint", k.span.line))
        .mark(k.span.clone(), tr!("契約はこの関係を約束していません", "the contract does not promise this"));
        match point {
            Some((a, b)) => {
                d = d.win(&k.left, WVal::Int(a)).win(&k.right, WVal::Int(b)).note(tr!(
                    "たとえば `{lw}` が {a}、`{rw}` が {b} の要求は、契約の検証を通り、`constraint` を満たしません。",
                    "A request with `{lw}` at {a} and `{rw}` at {b}, for one, passes the contract's validation and breaks the `constraint`."
                ));
            }
            None => {
                d = d.note(tr!(
                    "契約がこの組み合わせを断ることを示せませんでした（例は作れませんでした）。",
                    "It could not be shown that the contract refuses such a combination, and no example could be built either."
                ));
            }
        }
        d = d.note(refused_note());
        let fix = match con.kind {
            EnumSource::Proto => {
                let (sl, sr) = (c.wire_scale(&k.left), c.wire_scale(&k.right));
                let common = crate::types::lcm_i128(sl, sr);
                let side = |t: &Term, s: i128| {
                    let base = format!("this.{}", t.path().join("."));
                    let base = if let Term::Size(_) = t { format!("size({base})") } else { base };
                    if common / s == 1 { base } else { format!("{base} * {}", common / s) }
                };
                let alias = |n: &str| f.inputs.iter().find(|i| i.name.text == n).and_then(|i| i.name.ascii.clone()).unwrap_or_else(|| n.to_string());
                let op = match k.op {
                    CmpOp::Le => "le",
                    CmpOp::Lt => "lt",
                    CmpOp::Ge => "ge",
                    CmpOp::Gt => "gt",
                };
                Some(format!(
                    "option (buf.validate.message).cel = {{id: \"{}_{op}_{}\", expression: \"{} {} {}\"}};",
                    alias(&k.left),
                    alias(&k.right),
                    side(lt, sl),
                    k.op.word(),
                    side(rt, sr)
                ))
            }
            EnumSource::JsonSchema => None,
        };
        let widen = tr!(
            "`constraint` を外して、その組み合わせのときの答えを表で決めてください",
            "take the `constraint` off and decide in the tables what the rule answers for that combination"
        );
        d = match &fix {
            Some(x) => {
                let msg = con.at.rsplit('.').next().unwrap_or(&con.at).to_string();
                d.fix(FixKind::NarrowContract, x.clone()).note(tr!(
                    "ヒント: その組み合わせが来ないはずなら、メッセージ `{msg}` に {x} と書いて、契約で約束してください。来るのなら、{widen}。どちらにするかは人が決めることです。",
                    "hint: if that combination cannot occur, promise it in the contract: write {x} on the message `{msg}`. If it can, {widen}. Which of the two is a person's decision."
                ))
            }
            None => d.fix_kind(FixKind::None).note(tr!(
                "JSON Schema には、二つのフィールドの値を比べる書き方がありません。",
                "JSON Schema has no way to compare the values of two fields."
            ))
            .note(two_ways(None, widen)),
        };
        if ac.unread {
            d = d.note(tr!(
                "契約には読めなかった規則があり、ここでは無いものとして扱っています。",
                "The contract has rules that could not be read, and they are treated here as not there."
            ));
        }
        out.push(d);
    }
    out
}

/// W124: a row whose cells on the inputs of one contract ask for a combination the contract
/// never lets through, though each cell alone asks for values it does (§15.140). The cells on
/// other columns are left out, which only makes the row look reachable from more places.
fn unreachable_rows(f: &RuleFile, c: &Checked, ac: &Across, shape: &str, dead: &BTreeSet<(String, usize)>, rule_path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    if ac.rel.is_trivial() {
        return out;
    }
    let name = |t: &Term| ac.name(t);
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        if t.applied.is_some() {
            continue;
        }
        let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        for (ri, row) in t.rows.iter().enumerate() {
            if dead.contains(&(tname.clone(), row.index)) {
                continue;
            }
            let mut x = Extra::default();
            let mut cols: Vec<String> = Vec::new();
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let (Some(term), Some(cell)) = (ac.term(col), row.cells.get(ci)) else { continue };
                if matches!(cell, Cell::DontCare) {
                    continue;
                }
                let ty = c.ty_of(col).unwrap_or(Ty::Unknown);
                let took = match &ty {
                    Ty::Enum(e) => {
                        let all = c.enums.get(e).cloned().unwrap_or_default();
                        cell_values(cell, c, &all).map(|vs| x.strs.push((term.clone(), vs))).is_some()
                    }
                    Ty::Bool => match cell {
                        Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE || w == crate::kw::FALSE => {
                            x.bools.push((term.clone(), w == crate::kw::TRUE));
                            true
                        }
                        _ => false,
                    },
                    _ => cell_ineqs(cell, &ty, c.wire_scale(col), col, ri).map(|qs| x.ineqs.extend(qs)).is_some(),
                };
                if took {
                    cols.push(col.clone());
                }
            }
            // One column alone is W123's to judge; across fields there have to be two.
            if cols.len() < 2 {
                continue;
            }
            x.ineqs.extend(wire_ranges(ac, c));
            for (i, k) in f.constraints.iter().enumerate() {
                if ac.term(&k.left).is_some() && ac.term(&k.right).is_some() {
                    x.ineqs.push(wire_constraint(k, i, c, false));
                }
            }
            // A row the rule's own ranges and constraints already rule out is not the
            // contract's doing.
            if Relation::any().refute(&x, &name).is_some() || ac.rel.refute(&x, &name).is_none() {
                continue;
            }
            let (kind, rn) = if t.clause {
                (tr!("節", "clause"), tr!("節 {tname}", "clause {tname}"))
            } else {
                let base = tr!("行{}", "row {}", row.index);
                (
                    tr!("表", "table"),
                    match &row.label {
                        Some(l) => tr!("{base}（{}）", "{base} ({})", l.text),
                        None => base,
                    },
                )
            };
            let paths: Vec<String> = cols
                .iter()
                .filter_map(|col| f.inputs.iter().find(|i| i.name.text == *col).and_then(|i| i.from.as_ref()).map(|p| format!("`{}`", full_path(p))))
                .collect();
            let joined = paths.join(if crate::i18n::ja() { " と " } else { " and " });
            let mut d = Diag::warning(
                "W124",
                tr!("{rn} は、契約が通さない組み合わせでしか当たりません", "{rn} is reached only by a combination the contract does not let through"),
            )
            .at(format!("{rule_path}:{} {kind} {tname}", row.span.line))
            .table(tname.clone())
            .row(row.index)
            .rowref(tname.clone(), row.index)
            .key(format!("W124\u{1}{tname}\u{1}{shape}\u{1}{}", crate::region::row_key(row)))
            .fix_kind(FixKind::None)
            .mark(row.span.clone(), tr!("契約は {joined} をこの組み合わせでは通しません", "the contract never lets {joined} through in this combination"))
            .note(tr!(
                "セルを一つずつ見れば契約の通す値ですが、契約はフィールドのあいだにも条件を置いていて、この行の求める組み合わせはそれを満たしません。この行に当たる要求やメッセージは来ません。",
                "Each cell alone asks for values the contract lets through, but the contract also relates the fields, and the combination this row asks for does not satisfy it. No request or message reaches the row."
            ))
            .note(tr!(
                "ヒント: 契約がこの先も変わらないなら、この行を消してください。変わる予定があって残しているのなら、このままで構いません。",
                "hint: if the contract will not change, delete the row. If it is kept for a change that is planned, leave it."
            ));
            if ac.unread {
                d = d.note(tr!(
                    "契約には読めなかった規則もあります。それを無いものとしても、この行には当たりません。",
                    "The contract also has rules that could not be read. Even with them taken as not there, nothing reaches this row."
                ));
            }
            out.push(d);
        }
    }
    out
}

// --- Reading a `.proto` contract's JSON form (§15.133) ------------------------------------

/// What a field of a `.proto` reads as when protojson leaves it out: the zero value of its
/// kind, or an empty list for a `repeated`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zero {
    /// An integer, which protojson writes as a string when it is 64 bits wide.
    Int,
    Frac,
    Str,
    Bool,
    List,
    /// An enum: the name of its value numbered 0.
    Enum(String),
}

fn zero_of(con: &Contract, fd: &crate::proto::Field) -> Option<Zero> {
    if fd.repeated {
        return Some(Zero::List);
    }
    Some(match crate::proto::scalar(&fd.ty) {
        Some(crate::proto::Scalar::Int) => Zero::Int,
        Some(crate::proto::Scalar::Frac) => Zero::Frac,
        Some(crate::proto::Scalar::Str) => Zero::Str,
        Some(crate::proto::Scalar::Bool) => Zero::Bool,
        None => {
            let e = con.enum_of(&fd.ty)?;
            // proto3 puts 0 first; proto2's default is the first value written.
            let v = e.values.iter().find(|v| v.number == 0).or(e.values.first())?;
            Zero::Enum(v.name.clone())
        }
    })
}

/// How the function that reads the inputs out reads one projection from a `.proto`
/// contract's JSON form (§15.133). protojson writes a field under its JSON name and leaves out
/// a field at its default value, an empty `repeated`, and an unset message or `optional`
/// field; its readers take the name as written in the `.proto` as well.
#[derive(Debug, Clone)]
pub struct ProtoPath {
    /// Each step's JSON name and its name as the `.proto` writes it.
    pub steps: Vec<(String, String)>,
    /// What the last step reads as when it is not there.
    pub zero: Zero,
    /// The last step has presence of its own (`optional`): missing is unset, not zero.
    pub leaf_optional: bool,
    /// For a `where`: the element field's two names and what it reads as when missing.
    pub elem: Option<(String, String, Zero)>,
}

/// The `.proto` reads of every input projected from a `.proto` shape, by input name. The
/// contract is read from beside the rule, the way `check` read it; an input whose path does
/// not resolve has none, and `check` has already refused the rule for it.
pub fn proto_paths(f: &RuleFile, rule_path: &str) -> std::collections::HashMap<String, ProtoPath> {
    let mut out = std::collections::HashMap::new();
    let dir = std::path::Path::new(rule_path).parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
    for i in &f.inputs {
        let Some(pr) = &i.from else { continue };
        let Some(d) = f.shapes.iter().find(|d| d.name.text == pr.root.text && d.source == EnumSource::Proto) else { continue };
        let Ok(text) = std::fs::read_to_string(dir.join(&d.file)) else { continue };
        let Ok(con) = read(d, &text) else { continue };
        let steps: Vec<String> = pr.path.iter().map(|n| n.text.clone()).collect();
        let Some(chain) = con.proto_chain(&steps) else { continue };
        let Some(leaf) = chain.last() else { continue };
        let Some(zero) = zero_of(&con, leaf) else { continue };
        let elem = pr.kind.test().and_then(|(field, _)| {
            let m = con.msgs.iter().find(|m| crate::proto::same_message(&m.name, &leaf.ty))?;
            let fd = m.fields.iter().find(|x| x.name == field.text)?;
            Some((fd.json(), fd.name.clone(), zero_of(&con, fd)?))
        });
        out.insert(
            i.name.text.clone(),
            ProtoPath { steps: chain.iter().map(|fd| (fd.json(), fd.name.clone())).collect(), zero, leaf_optional: leaf.optional, elem },
        );
    }
    out
}

/// The first step a `.proto` contract lets be unset on the way to a value an input reads,
/// whose absence then reads as the value's default (§15.133): a message field that is not
/// `required`, or an `optional` field at the end that is not. Protovalidate validates
/// nothing under an unset message, so the rules on the value do not see that default.
fn unset_step<'c>(chain: &[&'c crate::proto::Field], to_leaf: bool) -> Option<(usize, &'c crate::proto::Field)> {
    let n = chain.len();
    chain.iter().enumerate().find_map(|(i, fd)| {
        let parent = i + 1 < n;
        let unset = if parent { !fd.rules.required } else { to_leaf && fd.optional && !fd.rules.required };
        unset.then_some((i, *fd))
    })
}

// --- What the certificate says about the contracts (§15.142) -----------------------------

/// One thing the rule's door asks of the values a contract feeds it: an end of an input's
/// declared range, a `constraint` between two of them, or that an enum input is one of its
/// values.
#[derive(Debug, Clone, PartialEq)]
pub enum Door {
    Range { input: String, hi: bool },
    Constraint(usize),
    Member { input: String },
}

/// The certificate's section on one contract: the condition it places on what the rule reads,
/// opened into cases, and for each thing the door asks, why every case keeps it. A door with
/// no proofs is one this could not show, which a re-checker says out loud.
pub struct ContractCert {
    pub shape: String,
    pub file: String,
    /// The digest of the contract's text, which ties the reading below to one file as the
    /// certificate's own digest ties it to one rule.
    pub sha256: String,
    /// Whether some rule of the contract could not be read and was taken as true: the claim is
    /// then about the contract read wider than it is, which is still a claim about the contract.
    pub unread: bool,
    /// The names of the values, by kind: the inputs the contract feeds first, then its other
    /// fields a condition mentions (`@` and the path).
    pub nums: Vec<String>,
    pub strs: Vec<String>,
    pub bools: Vec<String>,
    /// Each numeric input, with the scale it travels at and the path it is read from.
    pub inputs: Vec<(String, i128, String)>,
    pub atoms: Vec<Atom>,
    pub cases: Option<Vec<Vec<usize>>>,
    pub doors: Vec<(Door, Option<Vec<crate::relation::Proof>>)>,
    /// How each term is named here.
    pub names: BTreeMap<Term, String>,
}

/// The contracts' sections of a certificate: every shape an input reads from, with what its
/// condition says and why the rule's door keeps it (§15.142).
pub fn contract_certificates(f: &RuleFile, c: &Checked, rule_path: &str) -> Vec<ContractCert> {
    let dir = std::path::Path::new(rule_path).parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
    let mut out = Vec::new();
    for d in &f.shapes {
        let Ok(text) = std::fs::read_to_string(dir.join(&d.file)) else { continue };
        let Ok(con) = read(d, &text) else { continue };
        let ac = con.across(f, c, &d.name.text);
        if ac.terms.is_empty() {
            continue;
        }
        // The names of the terms: an input's own name where one reads it, `@path` otherwise.
        let mut names: BTreeMap<Term, String> = ac.names.clone();
        let mut kinds: BTreeMap<Term, u8> = BTreeMap::new();
        for a in &ac.rel.atoms {
            match a {
                Atom::Num(l, _) => l.terms.keys().for_each(|t| {
                    kinds.insert(t.clone(), 0);
                }),
                Atom::Str(t, ..) => {
                    kinds.insert(t.clone(), 1);
                }
                Atom::Bool(t, _) => {
                    kinds.insert(t.clone(), 2);
                }
                Atom::Unknown => {}
            }
        }
        for (input, t) in &ac.terms {
            let ty = c.ty_of(input).unwrap_or(Ty::Unknown);
            let k = match ty {
                Ty::Enum(_) => 1,
                Ty::Bool => 2,
                _ => 0,
            };
            kinds.insert(t.clone(), k);
        }
        for t in kinds.keys() {
            names.entry(t.clone()).or_insert_with(|| format!("@{}", t.word()));
        }
        let listed = |kind: u8| -> Vec<String> {
            let mut inputs: Vec<String> = kinds.iter().filter(|(t, k)| **k == kind && ac.names.contains_key(*t)).map(|(t, _)| names[t].clone()).collect();
            inputs.sort();
            let mut others: Vec<String> = kinds.iter().filter(|(t, k)| **k == kind && !ac.names.contains_key(*t)).map(|(t, _)| names[t].clone()).collect();
            others.sort();
            inputs.extend(others);
            inputs
        };
        let (nums, strs, bools) = (listed(0), listed(1), listed(2));
        let name = |t: &Term| ac.name(t);
        let mut inputs = Vec::new();
        let mut doors = Vec::new();
        for (input, t) in &ac.terms {
            let path = f.inputs.iter().find(|i| i.name.text == *input).and_then(|i| i.from.as_ref()).map(full_path).unwrap_or_default();
            match c.ty_of(input).unwrap_or(Ty::Unknown) {
                Ty::Enum(e) => {
                    let vals = c.enums.get(&e).cloned().unwrap_or_default();
                    doors.push((Door::Member { input: input.clone() }, ac.rel.within(t, &vals, &name)));
                }
                Ty::Bool => {}
                _ => {
                    let sc = c.wire_scale(input);
                    inputs.push((input.clone(), sc, path));
                    let Some((lo, hi)) = c.ranges.get(input).copied() else { continue };
                    let v = fourier::Lin::var(input);
                    let s = Rat::int(sc);
                    // The door's negation: a value below the low end, or above the high one.
                    if let Some(lo) = lo {
                        let neg = v.clone().plus(&fourier::Lin::con(lo.mul(s).mul(Rat::int(-1)))).le(true);
                        doors.push((Door::Range { input: input.clone(), hi: false }, ac.rel.refute(&Extra { ineqs: vec![neg], ..Extra::default() }, &name)));
                    }
                    if let Some(hi) = hi {
                        let neg = v.plus(&fourier::Lin::con(hi.mul(s).mul(Rat::int(-1)))).ge(true);
                        doors.push((Door::Range { input: input.clone(), hi: true }, ac.rel.refute(&Extra { ineqs: vec![neg], ..Extra::default() }, &name)));
                    }
                }
            }
        }
        for (i, k) in f.constraints.iter().enumerate() {
            if ac.term(&k.left).is_some() && ac.term(&k.right).is_some() {
                let neg = wire_constraint(k, i, c, true).tag(fourier::Origin::None);
                doors.push((Door::Constraint(i), ac.rel.refute(&Extra { ineqs: vec![neg], ..Extra::default() }, &name)));
            }
        }
        inputs.sort();
        out.push(ContractCert {
            shape: d.name.text.clone(),
            file: d.file.clone(),
            sha256: crate::sha256::hex(text.as_bytes()),
            unread: ac.unread,
            nums,
            strs,
            bools,
            inputs,
            atoms: ac.rel.atoms.clone(),
            cases: ac.rel.cases.clone(),
            doors,
            names,
        });
    }
    out
}
