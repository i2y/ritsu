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
use crate::diag::{Diag, FixKind, WVal};
use crate::json::Json;
use crate::num::Rat;
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
    let mut doms = Vec::new();
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
            let (found, dom) = holds(con, pr, i, c, rule_path);
            out.extend(found);
            if let Some(d) = dom {
                doms.push((i.name.text.clone(), d, pr));
            }
        }
        out.extend(resolved);
    }
    out.extend(dead_rows(f, c, &doms, rule_path));
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
        let mut node = deref(doc, at_pointer(doc, &self.at)?)?;
        let mut out = vec![node];
        for s in path {
            node = deref(doc, get(obj(node, "properties")?, s)?)?;
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

/// The keywords of a schema that narrow a value in a way this does not read.
fn schema_unread(n: &Json, strings: bool) -> Vec<String> {
    let mut ks: Vec<&str> = vec!["allOf", "anyOf", "oneOf", "not", "if", "multipleOf"];
    if strings {
        ks.extend(["pattern", "format", "minLength", "maxLength"]);
    }
    ks.into_iter().filter(|k| get(n, k).is_some()).map(String::from).collect()
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
    if r.required {
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
fn holds(con: &Contract, pr: &Projection, input: &VarDecl, c: &Checked, rule_path: &str) -> (Vec<Diag>, Option<Dom>) {
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
    if let (Some(chain), Some(doc)) = (&chain, con.doc.as_ref()) {
        if !(matches!(pr.kind, ProjKind::Field) && optional) {
            for (i, step) in steps.iter().enumerate() {
                let req = required_of(chain[i]);
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
                if let Some(items) = chain.last().and_then(|n| get(n, "items")).and_then(|i| deref(doc, i)) {
                    let req = required_of(items);
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
                    (Some(n), _, Some(doc)) if kind_of_schema(doc, n) == At::Int => Some((schema_ints(n), schema_unread(n, false), None)),
                    (_, Some(fd), _) => proto_ints(fd).map(|s| (s, fd.rules.unread.clone(), Some(fd.ty.rsplit('.').next().unwrap_or(&fd.ty).to_string()))),
                    _ => None,
                };
                let Some((seen, unread, kind)) = seen else { return (out, dom) };
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
                dom = Some(Dom::Ints(seen, false));
            }
            Ty::Enum(e) => {
                let vals = c.enums.get(e).cloned().unwrap_or_default();
                let seen = match (leaf_schema, leaf_proto) {
                    (Some(n), _) => Some((schema_strs(n), schema_unread(n, true), false)),
                    (_, Some(fd)) => Some((proto_strs(fd), fd.rules.unread.clone(), true)),
                    _ => None,
                };
                let Some((list, unread, proto)) = seen else { return (out, dom) };
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
                    dd = dd.at(at.clone()).mark(pr.span.clone(), label).fix(FixKind::NarrowContract, fix.clone()).note(refused_note());
                    if let Some(u) = unread_note(&unread) {
                        dd = dd.note(u);
                    }
                    out.push(dd.note(two_ways(Some(&fix), widen)));
                }
                if let Some(l) = list {
                    dom = Some(Dom::Strs(l));
                }
            }
            _ => {}
        },
        ProjKind::Count(t) => {
            let Some((Some(lo), Some(hi))) = c.ranges.get(name).copied() else { return (out, dom) };
            let (rl, rh) = (crate::types::wire_int(lo, 1), crate::types::wire_int(hi, 1));
            let filtered = t.is_some();
            let seen = match (leaf_schema, leaf_proto) {
                (Some(n), _) => Some((schema_count(n, filtered), schema_unread(n, false), false)),
                (_, Some(fd)) => Some((proto_count(fd, filtered), fd.rules.unread.clone(), true)),
                _ => None,
            };
            let Some((seen, unread, proto)) = seen else { return (out, dom) };
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
            dom = Some(Dom::Ints(seen, true));
        }
        ProjKind::Any(..) | ProjKind::All(..) => {}
    }
    (out, dom)
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
