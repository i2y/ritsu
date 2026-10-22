//! `ritsu kaname api <file.kn>` (DESIGN 11.1): a module as the tools outside read it, one JSON
//! object. Its name, version and SHA-256; the files it reads; its enums and records; its functions
//! with their parameters and results (types, ranges and counts as written), their effects, the calls
//! they make into the other languages with what each check across the borders came to, and the
//! names the generated code gives them (DESIGN 10.1, 10.3); and each operation of a transfer it
//! builds, with the reasons it can be refused with at the amounts given (DESIGN 6.3).
//!
//! It reads a module whose names and types pass, whatever its analysis finds: what can fail where,
//! and why, is what an outside tool reads it for (DESIGN 4.2).

use crate::check::Checked;
use crate::ranges::{Outcome, Place, SiteKind};
use crate::ty::{Range, Slot, Ty};
use crate::value::Program;
use ritsu_base::json::Json;
use std::path::Path;

/// A type as the module writes it, a rate with its step: `rate[step 0.01%]`, `list[Line]`.
fn type_text(p: &Program, c: &Checked, t: &Ty) -> String {
    match t {
        Ty::Num(n) if n.is_rate() => match n.step {
            Some(s) => format!("rate[step {}]", crate::names::step_text(s, &n.unit)),
            None => "rate".into(),
        },
        Ty::List(x) => format!("list[{}]", type_text(p, c, x)),
        Ty::Dict(k, v) => format!("dict[{}, {}]", type_text(p, c, k), type_text(p, c, v)),
        Ty::Set(x) => format!("set[{}]", type_text(p, c, x)),
        Ty::Opt(x) => format!("{}?", type_text(p, c, x)),
        Ty::Fn(ps, r, eff) => crate::ty::fn_text(&ps.iter().map(|x| type_text(p, c, x)).collect::<Vec<_>>(), &type_text(p, c, r), eff.as_ref()),
        t => p.show_ty(c, t),
    }
}

/// `"private": true` for what is written `private` (DESIGN 28.2); nothing else.
fn private(o: &mut Vec<(String, Json)>, private: bool) {
    if private {
        o.push(("private".into(), Json::from(true)));
    }
}

/// A range as the module writes it, in its type's unit: `>=1GBPc <=100000000GBPc`; null for none. The
/// range of a list or a set is its elements', of a dict its values'.
fn range_json(r: &Option<Range>, t: &Ty) -> Json {
    let mut t = t;
    while let Ty::Opt(x) | Ty::List(x) | Ty::Set(x) | Ty::Dict(_, x) = t {
        t = x;
    }
    match r {
        Some(r) => Json::str(crate::value::range_text(r, t)),
        None => Json::Null,
    }
}

/// A place a value is declared at (a parameter, a field, a result): its type, range and count.
fn slot_json(p: &Program, c: &Checked, s: &Slot) -> Vec<(String, Json)> {
    vec![
        ("type".into(), Json::str(type_text(p, c, &s.ty))),
        ("range".into(), range_json(&s.range, &s.ty)),
        ("count".into(), range_json(&s.count, &Ty::Num(crate::ty::NumTy::number()))),
    ]
}

/// A name with its alias (the alias is the name where none is written).
fn named(n: &crate::ast::Name) -> Vec<(String, Json)> {
    let alias = n.alias.as_ref().map(|(a, _)| a.clone()).unwrap_or_else(|| n.text.clone());
    vec![("name".into(), Json::str(n.text.clone())), ("alias".into(), Json::str(alias))]
}

/// The alias of a name, as the generated code names it.
fn alias(n: &crate::ast::Name) -> String {
    n.alias.as_ref().map(|(a, _)| a.clone()).unwrap_or_else(|| n.text.clone())
}

/// What a place came to, as `api` and `--format json` say it.
fn outcome_word(o: &Outcome) -> &'static str {
    match o {
        Outcome::Shown => "shown",
        Outcome::Example(_) => "example",
        Outcome::Undecided => "undecided",
        Outcome::Unbounded => "unbounded",
        Outcome::Pending => "waiting",
    }
}

/// A place a check across the borders decides, and what it came to.
fn crossing_json(pl: &Place) -> Option<Json> {
    let (check, about) = crate::ranges::crossing_of(&pl.what)?;
    Some(Json::obj([
        ("line", Json::int(pl.span.line as i128)),
        ("col", Json::int(pl.span.col as i128)),
        ("check", Json::str(check.check())),
        ("about", Json::str(about.to_string())),
        ("outcome", Json::str(outcome_word(&pl.outcome))),
        ("code", Json::opt_str(pl.across.as_ref().map(|(code, _)| code.clone()))),
    ]))
}

/// The `api` of a module whose names and types pass (`file` from `root`).
pub fn json(c: &std::rc::Rc<Checked>, root: &Path) -> Json {
    let p = Program::of(c);
    let file = ritsu_base::paths::from_root(root, Path::new(&c.path)).unwrap_or_else(|| c.path.clone());
    let analysis = c.analysis.get();
    let m = &c.module;
    let mut module = named(&m.name);
    module.push(("version".into(), Json::str(m.version.clone())));
    module.push(("description".into(), Json::opt_str(m.description.as_ref().map(|(d, _)| d.clone()))));
    module.push(("sha256".into(), Json::str(ritsu_base::sha256::hex(c.src.as_bytes()))));
    let uses = m.uses.iter().zip(&c.uses).map(|(u, t)| {
        let reached = ritsu_base::paths::from_root(root, t.canon()).unwrap_or_else(|| u.path.clone());
        Json::obj([("kind", Json::str(u.kind.word())), ("name", Json::str(u.name.clone())), ("path", Json::str(u.path.clone())), ("file", Json::str(reached)), ("line", Json::int(u.line as i128))])
    });
    let enums = m.enums.iter().enumerate().map(|(ei, e)| {
        let info = &c.decls.enums[ei];
        let sum = info.is_sum();
        let mut o = named(&e.name);
        private(&mut o, e.private);
        type_params(&mut o, &info.tparams);
        o.push(("ordered".into(), Json::from(e.ordered)));
        o.push((
            "values".into(),
            Json::arr(e.values.iter().zip(&info.fields).map(|(v, fs)| {
                let mut x = named(&v.name);
                x.push(("label".into(), Json::opt_str(v.label.clone())));
                // a value of an enum whose values have fields: its fields, and its type in the
                // generated code (DESIGN 23.8)
                if sum {
                    x.push((
                        "fields".into(),
                        Json::arr(fs.iter().map(|f| {
                            let mut y = named(&f.name);
                            y.extend(slot_json(&p, c, &f.slot));
                            y.push(("line".into(), Json::int(f.line as i128)));
                            Json::Obj(y)
                        })),
                    ));
                    x.push(("generated".into(), generated_value_type(&alias(&e.name), &alias(&v.name), e.private)));
                }
                Json::Obj(x)
            })),
        ));
        o.push(("line".into(), Json::int(e.line as i128)));
        o.push(("generated".into(), generated_type(&alias(&e.name), e.private)));
        Json::Obj(o)
    });
    let records = c.decls.records.iter().map(|r| {
        let mut o = named(&r.name);
        private(&mut o, r.private);
        type_params(&mut o, &r.tparams);
        o.push((
            "fields".into(),
            Json::arr(r.fields.iter().map(|f| {
                let mut x = named(&f.name);
                x.extend(slot_json(&p, c, &f.slot));
                x.push(("line".into(), Json::int(f.line as i128)));
                Json::Obj(x)
            })),
        ));
        o.push(("line".into(), Json::int(r.line as i128)));
        o.push(("generated".into(), generated_type(&alias(&r.name), r.private)));
        Json::Obj(o)
    });
    let functions = c.decls.fns.iter().enumerate().map(|(f, info)| {
        let decl = &m.fns[f];
        let facts = analysis.and_then(|a| a.fns.get(f));
        let mut o = named(&info.name);
        // a function no host calls (DESIGN 21.3): a generic one with its type parameters, one that takes
        // a function, one written `private`
        private(&mut o, info.private);
        type_params(&mut o, &info.tparams);
        if info.inner() {
            o.push(("inner".into(), Json::from(true)));
        }
        o.push(("line".into(), Json::int(info.line as i128)));
        o.push(("description".into(), Json::opt_str(decl.description.as_ref().map(|(d, _)| d.clone()))));
        o.push((
            "params".into(),
            Json::arr(info.params.iter().map(|prm| {
                let mut x = named(&prm.name);
                x.extend(slot_json(&p, c, &prm.slot));
                Json::Obj(x)
            })),
        ));
        o.push(("result".into(), Json::Obj(slot_json(&p, c, &info.ret))));
        o.push(("effect".into(), facts.map(|x| crate::effects::effect_json(info, &x.effect)).unwrap_or(Json::Null)));
        let written = decl.effect.as_ref().map(|w| {
            let ks: std::collections::BTreeSet<crate::ast::FailKind> = w.fails.iter().map(|(k, _)| *k).collect();
            Json::obj([("total", Json::from(w.total)), ("fails", Json::arr(ks.iter().map(|k| Json::str(k.word())))), ("diverges", Json::from(w.diverges))])
        });
        o.push(("written".into(), written.unwrap_or(Json::Null)));
        // the calls into the other languages, each with the checks of its places
        let crossings = facts.map(|x| {
            Json::arr(x.sites.iter().map(|site| {
                let (calls, callee) = match &site.kind {
                    SiteKind::Rule { name, .. } => ("rule", name.clone()),
                    SiteKind::Date { name, .. } => ("date", name.clone()),
                    SiteKind::Transfer { name, .. } => ("transfer", name.clone()),
                };
                let mine = |pl: &&Place| pl.expr == site.expr || site.given.iter().any(|g| g.expr == pl.expr);
                Json::obj([
                    ("line", Json::int(site.span.line as i128)),
                    ("col", Json::int(site.span.col as i128)),
                    ("calls", Json::str(calls)),
                    ("callee", Json::str(callee)),
                    ("checks", Json::arr(x.places.iter().filter(mine).filter_map(crossing_json))),
                ])
            }))
        });
        o.push(("crossings".into(), crossings.unwrap_or(Json::Null)));
        // the places kaname holds itself that are not shown
        let places = facts.map(|x| {
            Json::arr(x.places.iter().filter(|pl| pl.what.across().is_none() && pl.failing()).map(|pl| {
                Json::obj([
                    ("line", Json::int(pl.span.line as i128)),
                    ("col", Json::int(pl.span.col as i128)),
                    ("kind", Json::str(pl.kind.word())),
                    ("code", Json::opt_str(crate::ranges::code_of(pl).map(str::to_string))),
                    ("outcome", Json::str(outcome_word(&pl.outcome))),
                ])
            }))
        });
        o.push(("places".into(), places.unwrap_or(Json::Null)));
        let calls = facts.map(|x| Json::arr(x.calls.iter().map(|k| Json::obj([("line", Json::int(k.line as i128)), ("function", Json::str(k.function.clone()))]))));
        o.push(("calls".into(), calls.unwrap_or(Json::Null)));
        // an inner function's one function, which the generated code calls (DESIGN 21.3, 26.7)
        let a = if info.inner() { crate::r#gen::plan::inner_name(crate::r#gen::Target::TypeScript, c, f) } else { alias(&info.name) };
        let go = if info.inner() { crate::r#gen::plan::generic_name(crate::r#gen::Target::Go, c, f) } else { crate::r#gen::plan::fn_name(crate::r#gen::Target::Go, c, f) };
        o.push(("generated".into(), Json::obj([("typescript", Json::str(a.clone())), ("python", Json::str(a)), ("go", Json::str(go))])));
        Json::Obj(o)
    });
    // each operation of a transfer the module builds, with the reasons it can be refused with: the
    // reasons of every place that builds it, or none known where one is undecided
    let mut ops: Vec<Op> = Vec::new();
    if let Some(a) = analysis {
        for (f, x) in a.fns.iter().enumerate() {
            let _ = f;
            for site in &x.sites {
                let SiteKind::Transfer { id, name } = &site.kind else { continue };
                let reasons = x.refusals.iter().find(|r| r.span == site.span).map(|r| r.reasons.clone());
                let shown = format!("{}.{}", id.transfer, id.op);
                match ops.iter_mut().find(|(n, b, _, _, _)| *n == shown && *b == id.book) {
                    Some(op) => {
                        op.4 = match (op.4.take(), reasons.flatten()) {
                            (Some(mut a), Some(b)) => {
                                for r in b {
                                    if !a.contains(&r) {
                                        a.push(r);
                                    }
                                }
                                Some(a)
                            }
                            _ => None,
                        }
                    }
                    None => ops.push((shown, id.book.clone(), name.clone(), id.op.clone(), reasons.flatten())),
                }
            }
        }
    }
    let transfers = ops.into_iter().map(|(op, book, written, kind, reasons)| {
        let book_file = ritsu_base::paths::from_root(root, &book).unwrap_or_else(|| book.to_string_lossy().to_string());
        let refusable = kind == "do" || kind == "hold";
        Json::obj([
            ("type", Json::str(written)),
            ("book", Json::str(book_file)),
            ("operation", Json::str(op)),
            ("refusals", if refusable { reasons.map(|rs| Json::arr(rs.into_iter().map(Json::str))).unwrap_or(Json::Null) } else { Json::Null }),
        ])
    });
    let a = alias(&m.name);
    let pkg = ritsu_emit::ident::go_package(&a);
    Json::obj([
        ("kaname", Json::str(env!("CARGO_PKG_VERSION"))),
        ("file", Json::str(file)),
        ("module", Json::Obj(module)),
        ("generated", Json::obj([("typescript", Json::str(format!("glue/{a}.ts"))), ("python", Json::str(format!("glue/{a}.py"))), ("go", Json::str(format!("glue/{pkg}/{pkg}.go")))])),
        ("uses", Json::arr(uses)),
        ("enums", Json::arr(enums)),
        ("records", Json::arr(records)),
        ("functions", Json::arr(functions)),
        ("transfers", Json::arr(transfers)),
    ])
}

/// An operation of a transfer a module builds: as the book names it, the book on the disk, as the
/// module writes it, the operation, and the reasons it can be refused with (None where one place
/// that builds it does not know them).
type Op = (String, std::path::PathBuf, String, String, Option<Vec<String>>);

/// The type parameters of a generic declaration (DESIGN 26.1), each its name and alias and whether it
/// is held to keys; nothing for a declaration that has none.
fn type_params(o: &mut Vec<(String, Json)>, ps: &[crate::names::TParamInfo]) {
    if ps.is_empty() {
        return;
    }
    o.push((
        "type_params".into(),
        Json::arr(ps.iter().map(|p| {
            let mut x = named(&p.name);
            x.push(("key".into(), Json::from(p.key)));
            Json::Obj(x)
        })),
    ));
}

/// The names the type of a value of an enum whose values have fields has in the generated code
/// (`OrderPlaced`, DESIGN 23.8), with `_` before them for a `private` enum (DESIGN 28.2).
fn generated_value_type(enum_alias: &str, value_alias: &str, private: bool) -> Json {
    Json::obj(crate::r#gen::Target::ALL.iter().map(|t| (t.key(), Json::str(crate::r#gen::plan::hidden(crate::r#gen::plan::variant_name(*t, enum_alias, value_alias), private)))))
}

/// The names a record's or an enum's type has in the generated code.
fn generated_type(alias: &str, private: bool) -> Json {
    let t = ritsu_emit::ident::pascal(alias);
    let h = |n: String| crate::r#gen::plan::hidden(n, private);
    Json::obj([("typescript", Json::str(h(t.clone()))), ("python", Json::str(h(t.clone()))), ("go", Json::str(h(ritsu_emit::ident::go_exported(&t))))])
}
