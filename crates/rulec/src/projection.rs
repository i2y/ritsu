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

use crate::ast::{Cell, EnumSource, Lit, ProjKind, Projection, RuleFile, ShapeDecl};
use crate::diag::Diag;
use crate::json::Json;
use crate::types::{Checked, Ty};

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
    /// Where the root stands: a pointer into the schema, or a message name.
    at: String,
}

/// Read the contract a `shape` names, or say why it cannot be read.
pub fn read(d: &ShapeDecl, text: &str) -> Result<Contract, String> {
    match d.source {
        EnumSource::JsonSchema => {
            let doc = crate::jsonschema::read(text, &d.file)?;
            Ok(Contract { kind: d.source, doc: Some(doc), msgs: Vec::new(), at: d.at.clone() })
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
            Ok(Contract { kind: d.source, doc: None, msgs, at: d.at.clone() })
        }
    }
}

impl Contract {
    /// What stands at a path from the root, or where the walk got stuck.
    pub fn at(&self, path: &[String]) -> Result<At, Stuck> {
        match self.kind {
            EnumSource::JsonSchema => self.schema_at(path),
            EnumSource::Proto => self.proto_at(path),
        }
    }

    fn schema_at(&self, path: &[String]) -> Result<At, Stuck> {
        let doc = self.doc.as_ref().expect("a schema contract carries its document");
        let mut node = deref(doc, at_pointer(doc, &self.at).ok_or_else(|| Stuck { reached: self.at.clone(), had: keys(doc) })?);
        let mut reached = String::new();
        for step in path {
            let props = node.and_then(|n| obj(n, "properties"));
            let next = props.and_then(|p| get(p, step));
            match next {
                Some(v) => {
                    node = deref(doc, v);
                    reached = if reached.is_empty() { step.clone() } else { format!("{reached}.{step}") };
                }
                None => {
                    return Err(Stuck { reached, had: props.map(keys).unwrap_or_default() });
                }
            }
        }
        node.map(|n| kind_of_schema(doc, n)).ok_or(Stuck { reached, had: Vec::new() })
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
                // A message the file does not hold: the path cannot go on, and a leaf of an
                // unknown type is not claimed to be anything.
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

/// Follow `$ref` as long as it points inside this document. A `$ref` that leaves the
/// document is left as it is: nothing is fetched, and a path that runs into it gets stuck
/// rather than being waved through.
fn deref<'a>(doc: &'a Json, v: &'a Json) -> Option<&'a Json> {
    let mut node = v;
    for _ in 0..16 {
        match get(node, "$ref") {
            Some(Json::Str(p)) if p.starts_with('#') => node = at_pointer(doc, p)?,
            _ => return Some(node),
        }
    }
    None
}

fn kind_of_schema(doc: &Json, v: &Json) -> At {
    let ty = match get(v, "type") {
        Some(Json::Str(s)) => s.as_str(),
        // A schema with no `type` but with `properties` is an object, and one with `enum`
        // of strings travels as a string. Anything else is an object, which is the kind a
        // path can go on through and the kind `fits` refuses for every scalar.
        _ => {
            if get(v, "enum").is_some() {
                "string"
            } else {
                "object"
            }
        }
    };
    match ty {
        "string" => At::Str,
        "integer" => At::Int,
        "number" => At::Frac,
        "boolean" => At::Bool,
        "array" => At::Array(Box::new(
            get(v, "items").and_then(|i| deref(doc, i)).map(|i| kind_of_schema(doc, i)).unwrap_or(At::Object(Vec::new())),
        )),
        _ => At::Object(obj(v, "properties").map(keys).unwrap_or_default()),
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
    for i in &f.inputs {
        let Some(pr) = &i.from else { continue };
        let ty = c.ty_of(&i.name.text).unwrap_or(Ty::Unknown);
        out.extend(fits_the_type(pr, &ty, rule_path, &i.name.text));
        let Some((_, con)) = read_ok.iter().find(|(d, _)| d.name.text == pr.root.text) else {
            // A root with no shape of its own, or one whose file did not read. The first is
            // an error the reader can act on; the second was already said once.
            if !f.shapes.iter().any(|d| d.name.text == pr.root.text) {
                out.push(no_such_root(f, pr, rule_path));
            }
            continue;
        };
        out.extend(resolves(con, pr, &ty, rule_path, &i.name.text));
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
                    "`any` と `all` は当てはまるかどうかなので、受ける入力は `bool` です。値そのものが欲しいなら `from <形>.<欄>` を書いてください。",
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
                "`from` の道は宣言した `shape` から始まります。`shape {}(...) = jsonschema \"<ファイル>\" \"<ポインタ>\"` を足してください。",
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
        Diag::error("E121", tr!("`{}` という道は契約にありません", "The contract has no path `{}`", full()))
            .at(at.clone())
            .mark(pr.span.clone(), "")
            .note(if s.had.is_empty() {
                tr!("`{where_}` までは届きました。その先に欄はありません。", "It resolved as far as `{where_}`, which has no fields under it.")
            } else {
                tr!(
                    "`{where_}` までは届きました。そこにある欄: {}",
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
                            "`any`・`all`・`count` は並びを一度だけ歩きます。値そのものが欲しいなら `from <形>.<欄>` を書いてください。",
                            "`any`, `all` and `count` walk a collection once. For the value itself, write `from <shape>.<field>`."
                        )),
                );
                return out;
            };
            if let Some((field, cell)) = pr.kind.test() {
                let At::Object(had) = elem.as_ref() else {
                    out.push(
                        Diag::error("E121", tr!("`{}` の要素に欄はありません", "An element of `{}` has no fields", full()))
                            .at(at.clone())
                            .mark(field.span.clone(), tr!("契約では {}", "the contract says {}", elem.word()))
                            .note(tr!("`where` は要素の欄を見ます。", "`where` tests a field of an element.")),
                    );
                    return out;
                };
                if !had.contains(&field.text) {
                    out.push(
                        Diag::error("E121", tr!("要素に `{}` という欄はありません", "An element has no field `{}`", field.text))
                            .at(at.clone())
                            .mark(field.span.clone(), "")
                            .note(tr!("要素にある欄: {}", "The fields of an element: {}", had.join(", "))),
                    );
                } else if let Some(bad) = cell_clashes(con, &steps, &field.text, cell) {
                    out.push(
                        Diag::error("E120", tr!("`where` の値が欄の型と合いません", "The value of `where` does not fit the field"))
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
    Diag::error("E120", tr!("`{full}` の型が入力と合いません", "The type at `{full}` does not fit the input"))
        .at(at.to_string())
        .mark(pr.span.clone(), tr!("契約では {}、入力 {name} は {ty}", "the contract says {}, the input {name} is {ty}", here.word()))
        .note(tr!(
            "契約は値がどう運ばれるかを言い、規則はそれが何を意味するかを言います。列挙も日付も文字列で来て、金額と数量は宣言した単位の整数で来ます。",
            "A contract says how a value travels and the rule says what it means: an enum and a date arrive as strings, and money and a quantity as whole numbers in the unit the rule declares."
        ))
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
                let mut node = deref(doc, at_pointer(doc, &self.at)?)?;
                for s in steps {
                    node = deref(doc, get(obj(node, "properties")?, s)?)?;
                }
                let items = deref(doc, get(node, "items")?)?;
                let f = deref(doc, get(obj(items, "properties")?, field)?)?;
                Some(kind_of_schema(doc, f))
            }
            EnumSource::Proto => {
                let mut msg = self.msgs.iter().find(|m| crate::proto::same_message(&m.name, &self.at))?;
                for s in steps {
                    let fd = msg.fields.iter().find(|x| x.name == *s)?;
                    msg = self.msgs.iter().find(|x| crate::proto::same_message(&x.name, &fd.ty))?;
                }
                let fd = msg.fields.iter().find(|x| x.name == field)?;
                crate::proto::scalar(&fd.ty).map(|s| match s {
                    crate::proto::Scalar::Str => At::Str,
                    crate::proto::Scalar::Int => At::Int,
                    crate::proto::Scalar::Frac => At::Frac,
                    crate::proto::Scalar::Bool => At::Bool,
                })
            }
        }
    }
}
