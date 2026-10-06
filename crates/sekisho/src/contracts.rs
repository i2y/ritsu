//! The operations an action guards (DESIGN 2.6): what a `guards` line names is an operation of the
//! contract (E202), the argument `from` reads the resource's id from is a parameter of its path or
//! query (E204), every `input` is one of its parameters or a field of its body, of the type and
//! within the range the operation takes, read as dandori reads a task's parameters against an
//! operation (E203), and no operation is guarded by two actions (E205).
//!
//! Each operation found is named by its reference (ritsu's DESIGN 6.2), built as a
//! [`ritsu_base::naming::Name`] with the contract's path from the root: `openapi "api/orders.json"
//! operation refundOrder` (the `operationId`, else the method and the path), `asyncapi "…" operation
//! <key>`, `proto "…" service S method M`, `chobo "…" transfer T operation O`. The diagnostics say
//! it so, the model keeps it ([`Guard::reference`]), and `@guards`, `sekisho api` and the port
//! `Gates` write it. A contract outside the root has no reference: E201.
//!
//! OpenAPI and AsyncAPI documents are read with ritsu-base's reader ([`ritsu_base::openapi`]),
//! `.proto` files with ritsu-proto's, and a book's transfers through chobo's port.

use crate::diag::Diag;
use crate::model::*;
use ritsu_base::naming::{Name, Tool, word_or_quote};
use ritsu_base::openapi::{self, Bound, Document, Number, Place, Schema};
use ritsu_base::paths::Shown;
use ritsu_base::text::Text;
use std::collections::BTreeMap;

/// A contract a `use` line reads.
enum Contract {
    Doc(Document),
    Proto(ritsu_proto::ProtoFile),
    Book(ritsu_ports::BookFacts),
}

/// One operation a `guards` line finds.
enum Found<'a> {
    Op(&'a openapi::Operation),
    Method { file: &'a ritsu_proto::ProtoFile, method: &'a ritsu_proto::Method },
    Transfer,
}

fn at(code: &'static str, g: &Gate, line: usize, message: Text) -> Diag {
    Diag::at(code, &g.file, line, 1, message).source(&g.src)
}

/// The pairs of a reference, without its tool and path: `operation refundOrder`, `service Orders
/// method Refund`.
fn pairs(items: &[(&str, &str)]) -> String {
    items.iter().map(|(k, n)| format!("{k} {}", word_or_quote(n))).collect::<Vec<_>>().join(" ")
}

/// How the root is found, for a contract outside it.
fn root_note() -> Text {
    tr!(
        "action が守る操作は、ルートからのパスで参照します。ルートは、最初に渡したパスの上で .git を持つ一番近いディレクトリです（無ければ、渡したファイルのあるディレクトリ。--root で替えられます）。",
        "An operation an action guards is named by a reference whose path is from the root. The root is the nearest directory above the first path given that has a .git (else the directory of the file given); --root changes it."
    )
}

/// The checks of the contracts an action guards. `books` is chobo, when it is joined. What they
/// say, and each operation found, as the action and the guard (by index) and its reference.
pub fn check(g: &Gate, books: Option<&dyn ritsu_ports::Books>) -> (Vec<Diag>, Vec<(usize, usize, Name)>) {
    let mut diags = Vec::new();
    let mut references = Vec::new();
    // each contract read, with its path from the root
    let mut contracts: BTreeMap<usize, (Contract, String)> = BTreeMap::new();
    for (ui, u) in g.uses.iter().enumerate() {
        if !matches!(u.kind, UseKind::OpenApi | UseKind::AsyncApi | UseKind::Proto | UseKind::Book) {
            continue;
        }
        // the operations it holds are named from the root, so it has to be under it
        let Some(rel) = g.from_root(&u.file) else {
            diags.push(at("E201", g, u.line, tr!("`{}` はルートの外にあります", "`{}` is outside the root", u.path)).note(root_note()));
            continue;
        };
        let path = u.file.to_string_lossy().to_string();
        let unreadable = |why: Text| at("E201", g, u.line, tr!("`{}` を読めません", "`{}` cannot be read", u.path)).note(why);
        match u.kind {
            UseKind::OpenApi | UseKind::AsyncApi => {
                let text = match ritsu_base::fs::read_to_string(&u.file) {
                    Ok(t) => t,
                    Err(e) => {
                        diags.push(unreadable(Text::same(e.to_string())));
                        continue;
                    }
                };
                let load = |p: &str| ritsu_base::fs::read_to_string(std::path::Path::new(p)).ok();
                match openapi::read_with(&path, &text, &load) {
                    Ok(doc) => {
                        let want = if u.kind == UseKind::OpenApi { openapi::Kind::OpenApi } else { openapi::Kind::AsyncApi };
                        if doc.kind != want {
                            diags.push(at("E201", g, u.line, tr!("`{}` は {} の文書です。`use {}` で読んでください", "`{}` is an {} document; read it with `use {}`", u.path, doc.kind.title(), doc.kind.word())));
                            continue;
                        }
                        contracts.insert(ui, (Contract::Doc(doc), rel));
                    }
                    Err(p) => diags.push(unreadable(tr!("{}:{}: {}", "{}:{}: {}", p.line, p.col, p.message.ja; p.line, p.col, p.message.en))),
                }
            }
            UseKind::Proto => {
                let text = match ritsu_base::fs::read_to_string(&u.file) {
                    Ok(t) => t,
                    Err(e) => {
                        diags.push(unreadable(Text::same(e.to_string())));
                        continue;
                    }
                };
                match ritsu_proto::read(&path, &text) {
                    Ok(pf) => {
                        contracts.insert(ui, (Contract::Proto(pf), rel));
                    }
                    Err(e) => diags.push(unreadable(tr!("{}:{}: {}", "{}:{}: {}", e.line, e.col, e.message("sekisho").ja; e.line, e.col, e.message("sekisho").en))),
                }
            }
            UseKind::Book => {
                if let Some(b) = books.filter(|b| b.joined())
                    && let Ok(f) = b.facts(&u.file)
                {
                    contracts.insert(ui, (Contract::Book(f), rel));
                }
            }
            _ => {}
        }
    }

    // what a contract does not have, as yuen's E202 says it: the pairs of the reference written,
    // and the contract's path as the run writes it (the English starts as sekisho's E101 does)
    let shown = Shown::new(&g.root, &g.file);
    let has_no = |rel: &str, items: &[(&str, &str)]| {
        let (f, p) = (shown.path(rel), pairs(items));
        tr!("{f} に {p} はありません", "There is no {p} in {f}")
    };
    // each guard: the operation it names, and who guards it
    let mut guarded: BTreeMap<Name, usize> = BTreeMap::new();
    for (ai, a) in g.actions.iter().enumerate() {
        let mut found: Vec<(Name, Found)> = Vec::new();
        for (gi, gd) in a.guards.iter().enumerate() {
            let Some((c, rel)) = contracts.get(&gd.api) else { continue };
            let (reference, f) = match c {
                Contract::Doc(doc) => {
                    let tool = if doc.kind == openapi::Kind::OpenApi { Tool::Openapi } else { Tool::Asyncapi };
                    match doc.operation(&gd.operation) {
                        // an operation is named by its `operationId`, however the line writes it
                        Some(op) => (Name::file(tool, rel.clone()).with("operation", op.name()), Found::Op(op)),
                        None => {
                            let names: Vec<Text> = doc.operations.iter().take(12).map(|o| Text::same(o.name())).collect();
                            let l = Text::list(&names);
                            diags.push(at("E202", g, gd.line, has_no(rel, &[("operation", &gd.operation)])).note(tr!("操作は {} です。", "Its operations are {}.", l.ja; l.en)));
                            continue;
                        }
                    }
                }
                Contract::Proto(pf) => {
                    let (svc, m) = gd.operation.rsplit_once('/').unwrap_or(("", gd.operation.as_str()));
                    let svc_short = svc.rsplit('.').next().unwrap_or(svc);
                    let hit = pf.services.iter().find(|s| s.name == svc_short || pf.full(&s.name) == svc).and_then(|s| s.methods.iter().find(|x| x.name == m).map(|x| (s, x)));
                    match hit {
                        // a service by its name in the file's package
                        Some((s, x)) => (Name::file(Tool::Proto, rel.clone()).with("service", s.name.clone()).with("method", x.name.clone()), Found::Method { file: pf, method: x }),
                        None => {
                            let names: Vec<Text> = pf.services.iter().flat_map(|s| s.methods.iter().map(move |x| Text::same(format!("{}/{}", s.name, x.name)))).take(12).collect();
                            let l = Text::list(&names);
                            let items: Vec<(&str, &str)> = if svc.is_empty() { vec![("method", m)] } else { vec![("service", svc_short), ("method", m)] };
                            diags.push(at("E202", g, gd.line, has_no(rel, &items)).note(tr!("メソッドは {} です。", "Its methods are {}.", l.ja; l.en)));
                            continue;
                        }
                    }
                }
                Contract::Book(b) => {
                    let (t, op) = gd.operation.split_once('.').unwrap_or((gd.operation.as_str(), ""));
                    let ok = b.transfers.iter().any(|x| x.name == t && x.refusals.iter().any(|(o, _)| o == op));
                    if !ok {
                        let names: Vec<Text> = b.transfers.iter().flat_map(|x| x.refusals.iter().map(move |(o, _)| Text::same(format!("{}.{o}", x.name)))).take(12).collect();
                        let l = Text::list(&names);
                        diags.push(at("E202", g, gd.line, has_no(rel, &[("transfer", t), ("operation", op)])).note(tr!("振替の操作は {} です。", "Its transfers' operations are {}.", l.ja; l.en)));
                        continue;
                    }
                    (Name::file(Tool::Chobo, rel.clone()).with("transfer", t).with("operation", op), Found::Transfer)
                }
            };
            match guarded.get(&reference) {
                Some(&other) if other != ai => {
                    let (x, y, r) = (&g.actions[other].named.name, &a.named.name, reference.text());
                    diags.push(at("E205", g, gd.line, tr!("{r} を、`{x}` と `{y}` の二つの action が守ります", "Two actions, `{x}` and `{y}`, guard {r}")).note(tr!(
                        "どちらの判断で守るかが決まりません。一つの action にまとめてください。",
                        "Which of them decides is not settled: guard it with one action."
                    )));
                }
                _ => {
                    guarded.insert(reference.clone(), ai);
                }
            }
            references.push((ai, gi, reference.clone()));
            found.push((reference, f));
        }
        // `from`: a parameter of the path or the query of every operation
        if let Some((arg, line)) = &a.from {
            for (reference, f) in &found {
                let (ok, listed) = match f {
                    Found::Op(op) => {
                        let ok = op.param(arg).is_some_and(|p| matches!(p.place, Place::Path | Place::Query | Place::Channel));
                        let listed: Vec<Text> = op.params.iter().filter(|p| matches!(p.place, Place::Path | Place::Query | Place::Channel)).map(|p| Text::same(p.name.clone())).collect();
                        (ok, listed)
                    }
                    Found::Method { file, method } => {
                        let msg = message_of(file, &method.input);
                        let ok = msg.is_some_and(|m| m.fields.iter().any(|x| x.name == *arg || x.json() == *arg));
                        (ok, msg.map(|m| m.fields.iter().map(|x| Text::same(x.name.clone())).collect()).unwrap_or_default())
                    }
                    Found::Transfer => (true, vec![]),
                };
                if !ok {
                    let (l, r) = (Text::list(&listed), reference.text());
                    let mut d = at("E204", g, *line, tr!("`{arg}` は {r} のパスかクエリの引数にありません", "`{arg}` is not a parameter of the path or the query of {r}"));
                    if !listed.is_empty() {
                        d = d.note(tr!("引数は {} です。", "Its parameters are {}.", l.ja; l.en));
                    }
                    diags.push(d);
                }
            }
        }
        // each input: a parameter or a field of the body of every operation, of its type and range
        for inp in &a.inputs {
            for (reference, f) in &found {
                if let Some(d) = input_fits(g, inp, reference, f) {
                    diags.push(d);
                }
            }
        }
    }
    (diags, references)
}

fn message_of<'a>(f: &'a ritsu_proto::ProtoFile, written: &str) -> Option<&'a ritsu_proto::Message> {
    let short = written.trim_start_matches('.');
    let short = short.strip_prefix(&format!("{}.", f.package)).unwrap_or(short);
    f.message(short)
}

/// An end of a range a schema writes, as the integer it lets through: `exclusiveMinimum: 0` is 1.
fn int_end(b: &Bound, low: bool) -> Option<i128> {
    let v: ritsu_units::Rat = match &b.value {
        Number::Int(i) => ritsu_units::Rat::int(*i),
        Number::Decimal(s) => decimal(s)?,
    };
    // the least (most) integer at or past the end
    let (q, r) = (v.num.div_euclid(v.den), v.num.rem_euclid(v.den));
    let whole = r == 0;
    Some(match (low, whole, b.exclusive) {
        (true, true, false) => q,
        (true, true, true) => q + 1,
        (true, false, _) => q + 1,
        (false, true, false) => q,
        (false, true, true) => q - 1,
        (false, false, _) => q,
    })
}

/// A decimal as written (`12.5`, `-0.25`), without an exponent.
fn decimal(s: &str) -> Option<ritsu_units::Rat> {
    let (neg, d) = match s.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, s),
    };
    let (w, f) = d.split_once('.').unwrap_or((d, ""));
    if f.len() > 18 || !w.chars().chain(f.chars()).all(|c| c.is_ascii_digit()) || (w.is_empty() && f.is_empty()) {
        return None;
    }
    let n: i128 = format!("{w}{f}").parse().ok()?;
    ritsu_units::Rat::checked_new(if neg { -n } else { n }, 10i128.checked_pow(f.len() as u32)?)
}

/// What is wrong with an input against one operation (E203), if anything. `reference` is the
/// operation's.
fn input_fits(g: &Gate, inp: &Field, reference: &Name, f: &Found) -> Option<Diag> {
    let name = &inp.named.name;
    let line = inp.named.line;
    let r = reference.text();
    let e203 = |why: Text| at("E203", g, line, tr!("input `{name}` は {r} が受け取るものと合いません。{}", "The input `{name}` does not fit what {r} takes: {}", why.ja; why.en));
    match f {
        Found::Transfer => None,
        Found::Op(op) => {
            let (schema, required): (&Schema, bool) = match (op.param(name).or_else(|| op.param(&inp.named.alias)), op.field(name).or_else(|| op.field(&inp.named.alias))) {
                (Some(p), _) => (&p.schema, p.required || p.place == Place::Path),
                (None, Some(x)) => (&x.schema, x.required && op.body.as_ref().is_some_and(|b| b.required)),
                (None, None) => {
                    return Some(at("E203", g, line, tr!("input `{name}` は {r} の引数にも本文のフィールドにもありません", "The input `{name}` is neither a parameter nor a field of the body of {r}")).note(tr!(
                        "input には、操作が受け取る引数かフィールドを、同じ名前で書いてください。",
                        "Write under `input` what the operation takes, by the same name."
                    )));
                }
            };
            if !required && !inp.optional {
                return Some(e203(tr!("操作はこれを求めないので、無いことがあります。型に `?` を付けてください", "the operation does not require it, so it can be absent: write `?` after its type")));
            }
            schema_fits(g, inp, schema).map(e203)
        }
        Found::Method { file, method } => {
            let msg = message_of(file, &method.input)?;
            let Some(fld) = msg.fields.iter().find(|x| x.name == *name || x.json() == *name || x.name == inp.named.alias) else {
                return Some(at("E203", g, line, tr!("input `{name}` は {r} のリクエストのフィールドにありません", "The input `{name}` is not a field of the request of {r}")));
            };
            let ints = ["int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32", "fixed64", "sfixed32", "sfixed64"];
            let why = match (&inp.ty, &fld.ty) {
                (FieldType::Num { lo, hi, unit, .. }, ritsu_proto::Type::Scalar(s)) if ints.contains(&s.as_str()) => {
                    let r = &fld.rules.int;
                    let min = r.gte.or(r.gt.map(|x| x + 1)).or(r.konst);
                    let max = r.lte.or(r.lt.map(|x| x - 1)).or(r.konst);
                    range_fits(*lo, *hi, min, max, unit)
                }
                (FieldType::Num { .. }, ritsu_proto::Type::Scalar(s)) => Some(tr!("フィールドは `{s}` で、整数ではありません", "the field is `{s}`, not an integer")),
                (FieldType::Bool, ritsu_proto::Type::Scalar(s)) if s == "bool" => None,
                (FieldType::Enum(_), ritsu_proto::Type::Named(t)) if file.enumeration(t.trim_start_matches('.')).is_some() || file.enums.iter().any(|e| t.ends_with(&e.name)) => None,
                (FieldType::Date { .. }, _) => None,
                (_, t) => Some(tr!("フィールドの型 `{}` と合いません", "the field's type `{}` is not the input's", proto_type(t); proto_type(t))),
            };
            why.map(e203)
        }
    }
}

fn proto_type(t: &ritsu_proto::Type) -> String {
    match t {
        ritsu_proto::Type::Scalar(s) | ritsu_proto::Type::Named(s) => s.clone(),
        ritsu_proto::Type::Map(k, v) => format!("map<{k}, {}>", proto_type(v)),
    }
}

/// Whether the declared range lies within what the operation takes (`min`, `max`), as a task's does
/// in dandori; why not, when it does not.
fn range_fits(lo: i128, hi: i128, min: Option<i128>, max: Option<i128>, unit: &ritsu_units::Unit) -> Option<Text> {
    if min.is_some_and(|m| lo < m) || max.is_some_and(|m| hi > m) {
        let show = |v: Option<i128>| v.map(|v| crate::cells::show(v, unit)).unwrap_or_default();
        let (a, b, c, d) = (crate::cells::show(lo, unit), crate::cells::show(hi, unit), show(min), show(max));
        return Some(tr!("範囲が {a}〜{b} で、操作が受け取るのは {c}〜{d} です", "its range is {a}..{b}, and the operation takes {c}..{d}"));
    }
    None
}

/// What is wrong with an input's type against a JSON Schema, if anything.
fn schema_fits(g: &Gate, inp: &Field, s: &Schema) -> Option<Text> {
    let ty = s.ty.as_deref();
    match &inp.ty {
        FieldType::Num { lo, hi, unit, .. } => match ty {
            None | Some("integer") => range_fits(*lo, *hi, s.minimum.as_ref().and_then(|b| int_end(b, true)), s.maximum.as_ref().and_then(|b| int_end(b, false)), unit),
            Some("number") => Some(tr!("操作は小数も受け取りますが、input は整数です", "the operation takes a number with a fraction, and the input is a whole number")),
            Some(t) => Some(tr!("操作が受け取るのは {t} です", "the operation takes {t}")),
        },
        FieldType::Bool => match ty {
            None | Some("boolean") => None,
            Some(t) => Some(tr!("操作が受け取るのは {t} です", "the operation takes {t}")),
        },
        FieldType::Enum(e) => match (ty, &s.values) {
            (None | Some("string"), Some(vals)) => {
                let en = &g.enums[*e];
                let missing: Vec<Text> = vals.iter().filter(|v| !en.values.iter().any(|x| x.is(v))).map(|v| Text::same(v.clone())).collect();
                if missing.is_empty() {
                    None
                } else {
                    let l = Text::list(&missing);
                    Some(tr!("操作が受け取る {} は、列挙 `{}` の値にありません", "the operation takes {}, which the enum `{}` does not have", l.ja, en.named.name; l.en, en.named.name))
                }
            }
            (None | Some("string"), None) => Some(tr!("操作はどの文字列も受け取り、列挙の値に限りません", "the operation takes any string, not only the enum's values")),
            (Some(t), _) => Some(tr!("操作が受け取るのは {t} です", "the operation takes {t}")),
        },
        FieldType::Date { .. } => match (ty, s.format.as_deref()) {
            (None, _) | (Some("string"), Some("date")) => None,
            (Some(t), f) => Some(tr!("操作が受け取るのは {t}{} で、日付（format: date）ではありません", "the operation takes {t}{}, not a date (format: date)", f.map(|f| format!(" ({f})")).unwrap_or_default(); f.map(|f| format!(" ({f})")).unwrap_or_default())),
        },
        FieldType::Entity(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_end_is_the_integer_it_lets_through() {
        let b = |v: Number, exclusive: bool| Bound { value: v, exclusive };
        assert_eq!(int_end(&b(Number::Int(1), false), true), Some(1));
        assert_eq!(int_end(&b(Number::Int(0), true), true), Some(1));
        assert_eq!(int_end(&b(Number::Int(100), true), false), Some(99));
        assert_eq!(int_end(&b(Number::Decimal("0.5".into()), false), true), Some(1));
        assert_eq!(int_end(&b(Number::Decimal("99.5".into()), false), false), Some(99));
        assert_eq!(int_end(&b(Number::Decimal("-0.5".into()), false), true), Some(0));
    }
}
