//! One module of kaname as a module of the target (DESIGN 10.1–10.3): its head, its imports, its
//! enums and records, and each function twice — the one a host calls, which holds what it is given
//! to what the parameters declare (DESIGN 4.4), and the one the module's own functions call, past
//! that check — with the checks and the orders the bodies ask for.

use super::ex::{Ex, Node, print};
use super::plan::{Fx, Gen, Import, Reach, Said, check_name, fails_name, field_name, fn_name, generic_name, go_package, hidden, inner_name, member_name, param_names, type_name, variant_name};
use super::{Options, Target};
use crate::check::Checked;
use crate::ty::{EnumId, Owner, RecId, Ty};
use ritsu_emit::header::{Comment, Origin};
use ritsu_units::Unit;
use std::rc::Rc;

/// The module's file under the package's directory, its text, and the units whose types it names.
pub fn module(c: &Rc<Checked>, t: Target, opts: &Options) -> (String, String, Vec<Unit>) {
    let g = Gen::new(c, t, opts);
    let alias = c.module.name.ascii().to_string();
    let path = match t {
        Target::TypeScript => format!("glue/{alias}.ts"),
        Target::Python => format!("glue/{alias}.py"),
        Target::Go => {
            let p = go_package(&alias);
            format!("glue/{p}/{p}.go")
        }
        Target::Rust => format!("glue/{alias}.rs"),
    };
    // the declarations and the functions first: they say what the imports are
    let parts: Vec<String> = vec![typevars(&g), enums(&g), records(&g), functions(&g), outsides(&g), nests(&g), ranks(&g), eqs(&g), cmps(&g)];
    let body: String = parts.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join("\n");
    let text = format!("{}{}{}", head(&g, &alias), imports(&g, &body), body);
    let units = g.units.borrow().clone();
    (path, tidy(t, &text), units)
}

/// No two blank lines in a row, and one newline at the end.
pub(super) fn tidy(t: Target, s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank = 0;
    let most = if t == Target::Python { 2 } else { 1 };
    for line in s.lines() {
        if line.trim().is_empty() {
            blank += 1;
            if blank > most {
                continue;
            }
            out.push('\n');
        } else {
            blank = 0;
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

/// The lints every file of Rust kaname writes allows (DESIGN 36.8): the names as the source writes
/// them; the shapes of the generated code, which are the other targets' (a range checked end by end,
/// a name given its value in each branch) and the 2021 edition's (no `if let` chains); and what the
/// source writes that clippy would say of it (a `match` that answers `true` and `false`, or `none`
/// and a value).
pub const RUST_ALLOW: &str = "#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals, clippy::too_many_arguments, clippy::type_complexity, clippy::large_enum_variant, clippy::result_large_err, clippy::eq_op, clippy::identity_op, clippy::erasing_op, clippy::if_same_then_else, clippy::nonminimal_bool, clippy::explicit_auto_deref, clippy::manual_range_contains, clippy::needless_late_init, clippy::match_like_matches_macro, clippy::manual_map, clippy::manual_filter, clippy::collapsible_if, clippy::collapsible_else_if, clippy::collapsible_match)]\n";

/// A text as Rust's doc comment of as many lines as it has: `///` before an item, `//!` at the top of
/// a module.
pub fn rs_doc(mark: &str, text: &str) -> String {
    Comment::Slashes.lines(text).lines().map(|l| format!("{mark}{}\n", &l[2..])).collect()
}

fn comment(t: Target) -> Comment {
    match t {
        Target::Python => Comment::Hash,
        _ => Comment::Slashes,
    }
}

/// The head of DESIGN 9.2, and what the module is.
fn head(g: &Gen, alias: &str) -> String {
    let c = &g.c;
    let t = g.t;
    let cm = comment(t);
    let origin = Origin::at(&g.opts.shown, c.src.as_bytes()).of(g.opts.place.module.clone());
    let version = c.module.version.trim_start_matches('v');
    let source = origin.source("module", &c.module.name.text, version).line();
    let mut s = cm.line(&ritsu_emit::header::generated("kaname"));
    s.push_str(&cm.line(source.get(g.lang)));
    let what = c.module.description.as_ref().map(|(d, _)| d.clone()).unwrap_or_default();
    match t {
        Target::TypeScript => {
            if !what.is_empty() {
                s.push_str(&cm.lines(&what));
            }
            s.push('\n');
        }
        Target::Python => {
            let doc = if what.is_empty() { format!("The module {} of kaname.", c.module.name.text) } else { what.clone() };
            s.push_str(&format!("\"\"\"{}\"\"\"\n\nfrom __future__ import annotations\n\n", doc_text(&doc)));
        }
        Target::Go => {
            let p = go_package(alias);
            s.push('\n');
            if what.is_empty() {
                s.push_str(&format!("// Package {p} is the module {} of kaname.\n", c.module.name.text));
            } else {
                s.push_str(&cm.lines(&format!("Package {p} is the module {} of kaname: {what}", c.module.name.text)));
            }
            s.push_str(&format!("package {p}\n\n"));
        }
        Target::Rust => {
            let doc = if what.is_empty() { format!("The module {} of kaname.", c.module.name.text) } else { format!("The module {} of kaname: {what}", c.module.name.text) };
            s.push_str(&rs_doc("//!", &doc));
            s.push('\n');
            s.push_str(RUST_ALLOW);
            s.push('\n');
        }
    }
    s
}

/// A text in a Python docstring: no `"""` and no backslash that would end or escape it.
fn doc_text(s: &str) -> String {
    s.replace('\\', "\\\\").replace("\"\"\"", "\\\"\\\"\\\"")
}

/// The imports the module's code uses.
fn imports(g: &Gen, body: &str) -> String {
    let t = g.t;
    let used = g.used.borrow();
    let mut out = String::new();
    match t {
        Target::TypeScript => {
            let at = |to: String| g.opts.place.ts_path(&g.opts.here(), &to);
            for (name, i) in used.iter() {
                let line = match i {
                    Import::Kaname => format!("import * as {name} from \"{}\";", at("glue/kaname".into())),
                    Import::Units => format!("import type * as {name} from \"{}\";", at("units".into())),
                    Import::Datetime => continue,
                    Import::Rule(f) => {
                        let m = g.p.rules.get(f).map(|r| r.typescript.module.trim_end_matches(".ts").to_string()).unwrap_or_default();
                        format!("import * as {name} from \"{}\";", at(format!("{}rules/{m}", g.opts.dir_of(f))))
                    }
                    Import::Dates(f) => {
                        let a = g.p.dates.get(f).map(|d| d.alias.clone()).unwrap_or_default();
                        format!("import * as {name} from \"{}\";", at(format!("{}dates/{a}", g.opts.dir_of(f))))
                    }
                    Import::Module(f) => format!("import * as {name} from \"{}\";", at(format!("{}glue/{}", g.opts.dir_of(f), g.p.module(f).module.name.ascii()))),
                    Import::Book(f) => {
                        let b = g.p.books.get(f).map(|b| b.name.clone()).unwrap_or_default();
                        format!("import type * as {name} from \"{}\";", at(format!("{}glue/books/{b}", g.opts.dir_of(f))))
                    }
                };
                out.push_str(&line);
                out.push('\n');
            }
            out.push_str(&format!("\nconst _module = {};\n\n", ritsu_emit::lit::json(&g.c.module.name.text)));
        }
        Target::Python => {
            // the standard library, as much of it as the code names
            let mut std_lines = Vec::new();
            if used.values().any(|i| *i == Import::Datetime) {
                std_lines.push("import datetime as _dt".to_string());
            }
            if body.contains("(enum.Enum):") {
                std_lines.push("import enum".to_string());
            }
            if body.contains("dataclasses.replace(") {
                std_lines.push("import dataclasses".to_string());
            }
            if body.contains("@dataclass(frozen=True)") {
                std_lines.push("from dataclasses import dataclass".to_string());
            }
            let mut typing: Vec<&str> = ["Literal", "NoReturn", "Union", "TypeVar"].into_iter().filter(|w| body.contains(&format!(" = {w}"))).collect();
            // a dict and a set (DESIGN 24.6), the key of an order of records, a generic declaration and
            // the order a generic function is given (DESIGN 26.7)
            for (w, mark) in [("AbstractSet", "AbstractSet["), ("Any", "[Any, ...]"), ("Any", "], Any]"), ("Mapping", "Mapping["), ("Generic", "(Generic["), ("Callable", "Callable[")] {
                if body.contains(mark) && !typing.contains(&w) {
                    typing.push(w);
                }
            }
            typing.sort();
            if !typing.is_empty() {
                std_lines.push(format!("from typing import {}", typing.join(", ")));
            }
            if !std_lines.is_empty() {
                out.push_str(&std_lines.join("\n"));
                out.push_str("\n\n");
            }
            let from = |to: String| {
                let (pkg, m) = g.opts.place.py_from(&g.opts.here(), &to);
                format!("from {pkg} import {m}")
            };
            for (name, i) in used.iter() {
                let line = match i {
                    Import::Kaname => from("glue/kaname".into()),
                    Import::Units => format!("{} as {name}", from("units".into())),
                    Import::Datetime => continue,
                    Import::Rule(f) => {
                        let m = g.p.rules.get(f).map(|r| r.python.module.clone()).unwrap_or_default();
                        format!("{} as {name}", from(format!("{}rules/{m}", g.opts.dir_of(f))))
                    }
                    Import::Dates(f) => {
                        let a = g.p.dates.get(f).map(|d| d.alias.clone()).unwrap_or_default();
                        format!("{} as {name}", from(format!("{}dates/{a}", g.opts.dir_of(f))))
                    }
                    Import::Module(f) => format!("{} as {name}", from(format!("{}glue/{}", g.opts.dir_of(f), g.p.module(f).module.name.ascii()))),
                    Import::Book(f) => {
                        let b = g.p.books.get(f).map(|b| b.name.clone()).unwrap_or_default();
                        format!("{} as {name}", from(format!("{}glue/books/{b}", g.opts.dir_of(f))))
                    }
                };
                out.push_str(&line);
                out.push('\n');
            }
            out.push_str(&format!("\n_MODULE = {}\n\n", ritsu_emit::lit::json(&g.c.module.name.text)));
        }
        Target::Go => {
            let m = &g.opts.go_module;
            let mut paths: Vec<(String, String)> = Vec::new();
            for (name, i) in used.iter() {
                let (path, pkg) = match i {
                    Import::Kaname => (format!("{m}/glue/kaname"), "kaname".to_string()),
                    Import::Units => (format!("{m}/units"), "units".to_string()),
                    Import::Datetime => continue,
                    Import::Rule(f) => {
                        let p = g.p.rules.get(f).map(|r| r.go.module.clone()).unwrap_or_default();
                        (g.opts.place.go_path(m, &format!("{}rules/{p}", g.opts.dir_of(f))), p)
                    }
                    Import::Dates(f) => {
                        let p = g.p.dates.get(f).map(|d| go_package(&d.alias)).unwrap_or_default();
                        (g.opts.place.go_path(m, &format!("{}dates/{p}", g.opts.dir_of(f))), p)
                    }
                    Import::Module(f) => {
                        let p = go_package(g.p.module(f).module.name.ascii());
                        (g.opts.place.go_path(m, &format!("{}glue/{p}", g.opts.dir_of(f))), p)
                    }
                    Import::Book(f) => {
                        let p = super::books::go_package_of(&g.p, f);
                        (g.opts.place.go_path(m, &format!("{}glue/books/{p}", g.opts.dir_of(f))), p)
                    }
                };
                let spec = if *name == pkg { format!("\"{path}\"") } else { format!("{name} \"{path}\"") };
                paths.push((path, spec));
            }
            paths.sort();
            if !paths.is_empty() {
                out.push_str("import (\n");
                for (_, spec) in &paths {
                    out.push_str(&format!("\t{spec}\n"));
                }
                out.push_str(")\n\n");
            }
            out.push_str(&format!("const _module = {}\n\n", ritsu_emit::lit::go(&g.c.module.name.text)));
        }
        Target::Rust => {
            let lines = rust_uses(g, body);
            if !lines.is_empty() {
                out.push_str(&lines.join("\n"));
                out.push_str("\n\n");
            }
            out.push_str(&format!("const _MODULE: &str = {};\n\n", ritsu_emit::lit::rust(&g.c.module.name.text)));
        }
    }
    out
}

/// Rust's `use` lines of what the code `body` (written by `g`) reads: each import by the name the code
/// reads it by, and the standard library's collections it names bare.
pub(super) fn rust_uses(g: &Gen, body: &str) -> Vec<String> {
    let used = g.used.borrow();
    let path = |to: String| ritsu_emit::package::rust_path(&to);
    let mut lines: Vec<String> = Vec::new();
    for (name, i) in used.iter() {
        let to = match i {
            Import::Kaname => path("glue/kaname".into()),
            Import::Units => path("units".into()),
            Import::Datetime => continue,
            Import::Rule(f) => {
                let m = g.p.rules.get(f).map(|r| r.rust.module.trim_end_matches(".rs").to_string()).unwrap_or_default();
                path(format!("{}rules/{m}", g.opts.dir_of(f)))
            }
            Import::Dates(f) => {
                let a = g.p.dates.get(f).map(|d| d.alias.clone()).unwrap_or_default();
                path(format!("{}dates/{a}", g.opts.dir_of(f)))
            }
            Import::Module(f) => path(format!("{}glue/{}", g.opts.dir_of(f), g.p.module(f).module.name.ascii())),
            Import::Book(f) => {
                let b = g.p.books.get(f).map(|b| b.rust.module.clone()).unwrap_or_default();
                path(format!("{}glue/books/{b}", g.opts.dir_of(f)))
            }
        };
        let last = to.rsplit("::").next().unwrap_or_default();
        lines.push(if last == name { format!("use {to};") } else { format!("use {to} as {name};") });
    }
    // the standard library's collections, where the code names them bare
    let colls: Vec<&str> = ["BTreeMap", "BTreeSet"].into_iter().filter(|w| !g.outer.contains(*w) && has_word(body, w)).collect();
    match colls.as_slice() {
        [] => {}
        [one] => lines.push(format!("use std::collections::{one};")),
        many => lines.push(format!("use std::collections::{{{}}};", many.join(", "))),
    }
    lines
}

/// Whether `w` stands in `text` as a word of its own, not after `::` (a path that names it).
fn has_word(text: &str, w: &str) -> bool {
    let mut from = 0;
    while let Some(i) = text[from..].find(w) {
        let at = from + i;
        let before = text[..at].chars().next_back();
        let after = text[at + w.len()..].chars().next();
        let word = |c: Option<char>| c.is_some_and(|c| c == '_' || c.is_alphanumeric());
        if !word(before) && !word(after) && !text[..at].ends_with("::") {
            return true;
        }
        from = at + w.len();
    }
    false
}

/// Python's type variables of the module's generic declarations (DESIGN 26.7), each once: a record, an
/// enum and a function that name one `T` share it.
fn typevars(g: &Gen) -> String {
    if g.t != Target::Python {
        return String::new();
    }
    let c = &g.c;
    let mut names: Vec<String> = Vec::new();
    let lists = c.decls.records.iter().map(|r| &r.tparams).chain(c.decls.enums.iter().filter(|e| e.is_sum()).map(|e| &e.tparams)).chain(c.decls.fns.iter().map(|f| &f.tparams));
    for ps in lists {
        for n in g.tparam_names(ps) {
            if !names.contains(&n) {
                names.push(n);
            }
        }
    }
    if names.is_empty() {
        return String::new();
    }
    let mut out = String::from("# The type parameters of the module's generic declarations (kaname's DESIGN 26.7).\n");
    for n in names {
        out.push_str(&format!("{n} = TypeVar({})\n", ritsu_emit::lit::json(&n)));
    }
    out.push_str("\n\n");
    out
}

/// The module's enums: TypeScript's frozen object and its union, Python's `enum.Enum`, Go's named
/// string and its constants. A value is its name (DESIGN 10.2). An enum whose values have fields is a
/// sum: a type for each value, and the enum the union of them (DESIGN 23.8).
fn enums(g: &Gen) -> String {
    let t = g.t;
    let mut out = String::new();
    for (ei, e) in g.c.module.enums.iter().enumerate() {
        if g.c.decls.enums[ei].is_sum() {
            out.push_str(&sum(g, ei));
            continue;
        }
        let alias = e.name.ascii();
        let ty = hidden(type_name(t, alias), e.private);
        let shown = &e.name.text;
        // a `private` enum is not exported (DESIGN 28.2)
        let export = if e.private { "" } else { "export " };
        match t {
            Target::TypeScript => {
                out.push_str(&format!("/** The enum {shown}. */\n{export}const {ty} = {{\n"));
                for v in &e.values {
                    out.push_str(&format!("  {}: {},\n", member_name(t, alias, v.name.ascii()), ritsu_emit::lit::json(&v.name.text)));
                }
                out.push_str(&format!("}} as const;\n{export}type {ty} = (typeof {ty})[keyof typeof {ty}];\n\n"));
            }
            Target::Python => {
                out.push_str(&format!("\nclass {ty}(enum.Enum):\n    \"\"\"The enum {}.\"\"\"\n\n", doc_text(shown)));
                for v in &e.values {
                    out.push_str(&format!("    {} = {}\n", member_name(t, alias, v.name.ascii()), ritsu_emit::lit::json(&v.name.text)));
                }
                out.push_str("\n\n");
            }
            Target::Go => {
                out.push_str(&format!("// {ty} is the enum {shown}.\ntype {ty} string\n\n"));
                let names: Vec<String> = e.values.iter().map(|v| hidden(member_name(t, alias, v.name.ascii()), e.private)).collect();
                let w = names.iter().map(|n| n.chars().count()).max().unwrap_or(0);
                out.push_str("const (\n");
                for (n, v) in names.iter().zip(&e.values) {
                    out.push_str(&format!("\t{n}{}{ty} = {}\n", " ".repeat(w - n.chars().count() + 1), ritsu_emit::lit::go(&v.name.text)));
                }
                out.push_str(")\n\n");
            }
            Target::Rust => {
                let vis = if e.private { "" } else { "pub " };
                let members: Vec<String> = e.values.iter().map(|v| member_name(t, alias, v.name.ascii())).collect();
                out.push_str(&format!("/// The enum {shown}.\n#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]\n{vis}enum {ty} {{\n"));
                for m in &members {
                    out.push_str(&format!("    {m},\n"));
                }
                out.push_str("}\n\n");
                // the names of the values, which a host reads and writes (a private enum is no host's)
                if !e.private {
                    out.push_str(&format!("impl {ty} {{\n    /// The value's name, as the module writes it.\n    pub fn as_str(self) -> &'static str {{\n        match self {{\n"));
                    for (m, v) in members.iter().zip(&e.values) {
                        out.push_str(&format!("            {ty}::{m} => {},\n", ritsu_emit::lit::rust(&v.name.text)));
                    }
                    out.push_str("        }\n    }\n\n    /// The value of a name; None for a name that is no value's.\n    pub fn parse(s: &str) -> Option<");
                    out.push_str(&format!("{ty}> {{\n        match s {{\n"));
                    for (m, v) in members.iter().zip(&e.values) {
                        out.push_str(&format!("            {} => Some({ty}::{m}),\n", ritsu_emit::lit::rust(&v.name.text)));
                    }
                    out.push_str("            _ => None,\n        }\n    }\n}\n\n");
                }
            }
        }
    }
    out
}

/// An enum whose values have fields (DESIGN 23.8): the type of each value, as a record's with
/// TypeScript's `kind` first; the enum the union of them (Go's interface, which each value's type
/// meets); and, where a parameter can reach a field outside what it declares, the check of a value.
///
/// A generic sum (DESIGN 26.7) declares its type parameters on the enum and on the type of each value
/// (Go's mark takes them, so that two of its types with other type arguments are not one interface);
/// a value of it is checked where a parameter reaches it, by the check of the type it is given there.
fn sum(g: &Gen, ei: usize) -> String {
    let e = &g.c.decls.enums[ei];
    g.enter(&e.tparams, &[]);
    let out = sum_of(g, ei);
    g.leave();
    out
}

fn sum_of(g: &Gen, ei: usize) -> String {
    let t = g.t;
    let e = &g.c.decls.enums[ei];
    let alias = e.name.ascii();
    let ty = hidden(type_name(t, alias), e.private);
    let export = if e.private { "" } else { "export " };
    let shown = &e.name.text;
    let rt = Ty::Enum(EnumId::plain(Owner::Module(g.c.canon.clone()), e.name.text.clone()));
    let generic = !e.tparams.is_empty();
    let checked = !generic && g.record_checkable(&rt, 0, Reach::Call);
    // the check of a host's argument, where the enum can hold a value of a type that holds itself: the
    // fields at every depth (DESIGN 28.1)
    let entered = !generic && g.p.reaches_recursive(&rt) && g.record_checkable(&rt, 0, Reach::Entry);
    let (decl, used) = (g.tparams_decl(&e.tparams), g.tparams_use(&e.tparams));
    let marks = g.tparam_names(&e.tparams).join(", ");
    let vts: Vec<String> = e.values.iter().map(|v| hidden(variant_name(t, alias, v.ascii()), e.private)).collect();
    let k = if checked || entered { g.imp(Import::Kaname) } else { String::new() };
    let mut out = String::new();
    // the check of the fields of each value, as a parameter reaches them (DESIGN 4.4): each value's
    // body, by the value
    let checks: Vec<Vec<Node>> = e
        .values
        .iter()
        .zip(&e.fields)
        .map(|(v, fs)| {
            let mut body = Vec::new();
            if !checked {
                return body;
            }
            for (i, f) in fs.iter().enumerate() {
                let Some(fun) = g.outside_fn(&f.slot, Reach::Call) else { continue };
                let (x, what) = field_arg(g, &variant_ty(g, e, v), f, &format!(".{}.{}", v.text, f.name.text));
                let o = Ex::atom(format!("o{}", i + 1));
                body.extend(g.if_found(&o, Ex::call(fun, vec![x, what]), vec![Node::Line(g.ret_found(o.clone()))]));
            }
            body
        })
        .collect();
    let enters: Vec<Vec<Node>> = e
        .values
        .iter()
        .zip(&e.fields)
        .map(|(v, fs)| {
            let mut body = Vec::new();
            if !entered {
                return body;
            }
            for (i, f) in fs.iter().enumerate() {
                let Some(fun) = g.outside_fn(&f.slot, Reach::Entry) else { continue };
                let (x, what) = field_arg(g, &variant_ty(g, e, v), f, &format!(".{}.{}", v.text, f.name.text));
                let o = Ex::atom(format!("o{}", i + 1));
                body.extend(g.if_found(&o, Ex::call(fun, vec![x, what]), vec![Node::Line(g.ret_found(o.clone()))]));
            }
            body
        })
        .collect();
    match t {
        Target::TypeScript => {
            out.push_str(&format!("/** The enum {shown}: a value of one of its values, each with its fields. */\n{export}type {ty}{decl} = {};\n\n", vts.iter().map(|v| format!("{v}{used}")).collect::<Vec<_>>().join(" | ")));
            for ((v, fs), vt) in e.values.iter().zip(&e.fields).zip(&vts) {
                out.push_str(&format!("/** The value {} of the enum {shown}. */\n{export}interface {vt}{decl} {{\n  readonly kind: {};\n", v.text, ritsu_emit::lit::json(&v.text)));
                for f in fs {
                    out.push_str(&format!("  readonly {}: {};\n", field_name(t, f.name.ascii()), g.boundary(&f.slot.ty)));
                }
                out.push_str("}\n\n");
            }
            if checked {
                let mut body: Vec<Node> = Vec::new();
                for ((v, c), _) in e.values.iter().zip(checks).zip(&vts) {
                    if !c.is_empty() {
                        body.push(Node::If(vec![(Ex::bin(super::ex::Op::Eq, Ex::atom("v").dot("kind"), g.string(&v.text)), c)], Vec::new()));
                    }
                }
                body.push(Node::Line(g.ret(g.null())));
                out.push_str(&format!("/** Where a value of {shown} a parameter reaches is outside what its fields declare. */\n{export}function _outside_{ty}(v: {ty}, what: string): {k}.Outside | null {{\n"));
                print(&body, t, 1, &mut out);
                out.push_str("}\n\n");
            }
            if entered {
                let mut body: Vec<Node> = Vec::new();
                for (v, c) in e.values.iter().zip(&enters) {
                    if !c.is_empty() {
                        body.push(Node::If(vec![(Ex::bin(super::ex::Op::Eq, Ex::atom("v").dot("kind"), g.string(&v.text)), c.clone())], Vec::new()));
                    }
                }
                body.push(Node::Line(g.ret(g.null())));
                out.push_str(&format!("/** Where a value of {shown} a host gives is outside what its fields declare, at every depth. */\n{export}function _enter_{ty}(v: {ty}, what: string): {k}.Outside | null {{\n"));
                print(&body, t, 1, &mut out);
                out.push_str("}\n\n");
            }
        }
        Target::Python => {
            let base = if generic { format!("(Generic[{decl}])") } else { String::new() };
            for ((v, fs), vt) in e.values.iter().zip(&e.fields).zip(&vts) {
                out.push_str(&format!("\n@dataclass(frozen=True)\nclass {vt}{base}:\n    \"\"\"The value {} of the enum {}.\"\"\"\n", doc_text(&v.text), doc_text(shown)));
                if !fs.is_empty() {
                    out.push('\n');
                }
                for f in fs {
                    out.push_str(&format!("    {}: {}\n", field_name(t, f.name.ascii()), g.boundary(&f.slot.ty)));
                }
                out.push_str("\n\n");
            }
            out.push_str(&format!("# The enum {shown}: a value of one of its values, each with its fields.\n{ty} = Union[{}]\n\n", vts.iter().map(|v| format!("{v}{used}")).collect::<Vec<_>>().join(", ")));
            if checked {
                let mut body: Vec<Node> = Vec::new();
                for ((_, c), vt) in e.values.iter().zip(checks).zip(&vts) {
                    if !c.is_empty() {
                        body.push(Node::If(vec![(Ex::call("isinstance", vec![Ex::atom("v"), Ex::atom(vt.clone())]), c)], Vec::new()));
                    }
                }
                body.push(Node::Line(g.ret(g.null())));
                out.push_str(&format!("\ndef _outside_{ty}(v: {ty}, what: str) -> {k}.Outside | None:\n    \"\"\"Where a value of {} a parameter reaches is outside what its fields declare.\"\"\"\n", doc_text(shown)));
                print(&body, t, 1, &mut out);
                out.push_str("\n\n");
            }
            if entered {
                let mut body: Vec<Node> = Vec::new();
                for (c, vt) in enters.iter().zip(&vts) {
                    if !c.is_empty() {
                        body.push(Node::If(vec![(Ex::call("isinstance", vec![Ex::atom("v"), Ex::atom(vt.clone())]), c.clone())], Vec::new()));
                    }
                }
                body.push(Node::Line(g.ret(g.null())));
                out.push_str(&format!("\ndef _enter_{ty}(v: {ty}, what: str) -> {k}.Outside | None:\n    \"\"\"Where a value of {} a host gives is outside what its fields declare, at every depth.\"\"\"\n", doc_text(shown)));
                print(&body, t, 1, &mut out);
                out.push_str("\n\n");
            }
        }
        Target::Rust => {
            let vis = if e.private { "" } else { "pub " };
            let members: Vec<String> = e.values.iter().map(|v| member_name(t, alias, v.ascii())).collect();
            let _ = used;
            // each value's struct takes the type parameters its fields name (DESIGN 36.2)
            let own: Vec<Vec<crate::names::TParamInfo>> = e.values.iter().map(|v| super::plan::variant_tparams(e, &v.text).into_iter().map(|i| e.tparams[i].clone()).collect()).collect();
            let derive = g.derives(&rt);
            out.push_str(&format!("/// The enum {shown}: a value of one of its values, each with its fields.\n{derive}\n{vis}enum {ty}{decl} {{\n"));
            for ((m, vt), ps) in members.iter().zip(&vts).zip(&own) {
                out.push_str(&format!("    {m}({vt}{}),\n", g.tparams_use(ps)));
            }
            out.push_str("}\n\n");
            for (((v, fs), vt), ps) in e.values.iter().zip(&e.fields).zip(&vts).zip(&own) {
                let holder = variant_ty(g, e, v);
                let decl = g.tparams_decl(ps);
                let derive = g.derives(&holder);
                if fs.is_empty() {
                    out.push_str(&format!("/// The value {} of the enum {shown}.\n{derive}\n{vis}struct {vt}{decl} {{}}\n\n", v.text));
                    continue;
                }
                out.push_str(&format!("/// The value {} of the enum {shown}.\n{derive}\n{vis}struct {vt}{decl} {{\n", v.text));
                for f in fs {
                    out.push_str(&format!("    pub {}: {},\n", field_name(t, f.name.ascii()), g.field_type(&holder, &f.slot.ty)));
                }
                out.push_str("}\n\n");
            }
            // the checks of the fields: each value's own, as Go's, and the enum's by its value
            // a `private` enum's checks are the module's alone, which may call them nowhere
            let dead = if e.private { "    #[allow(dead_code)]\n" } else { "" };
            let method = |name: &str, doc: &str, bodies: &[Vec<Node>]| -> String {
                let mut o = String::new();
                for (((v, vt), m), b) in e.values.iter().zip(&vts).zip(&members).zip(bodies) {
                    let _ = m;
                    let mut body = b.clone();
                    body.push(Node::Line(g.ret(g.null())));
                    let what = if b.is_empty() { "_what" } else { "what" };
                    o.push_str(&format!("impl {vt} {{\n    /// Where a value of {shown}.{} {doc}\n{dead}    pub fn {name}(&self, {what}: &str) -> Option<{k}::Outside> {{\n", v.text));
                    print(&super::ex::tail(body), t, 2, &mut o);
                    o.push_str("    }\n}\n\n");
                }
                o.push_str(&format!("impl {ty} {{\n    /// Where a value of {shown} {doc}\n{dead}    pub fn {name}(&self, what: &str) -> Option<{k}::Outside> {{\n        match self {{\n"));
                for m in &members {
                    o.push_str(&format!("            {ty}::{m}(v) => v.{name}(what),\n"));
                }
                o.push_str("        }\n    }\n}\n\n");
                o
            };
            if checked {
                out.push_str(&method("kaname_outside", "a parameter reaches is outside what its fields declare.", &checks));
            }
            if entered {
                out.push_str(&method("kaname_enter", "a host gives is outside what its fields declare, at every depth.", &enters));
            }
        }
        Target::Go => {
            let method = format!("is{ty}");
            let list = match vts.as_slice() {
                [one] => one.clone(),
                [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
                [] => String::new(),
            };
            out.push_str(&format!("// {ty} is the enum {shown}: a value of one of {list}.\ntype {ty}{decl} interface {{\n\t{method}({marks})\n"));
            if checked {
                out.push_str(&format!("\t// KanameOutside is where a value of {shown} a parameter reaches is outside what its fields declare.\n\tKanameOutside(what string) *{k}.Outside\n"));
            }
            if entered {
                out.push_str(&format!("\t// KanameEnter is where a value of {shown} a host gives is outside what its fields declare, at every depth.\n\tKanameEnter(what string) *{k}.Outside\n"));
            }
            out.push_str("}\n\n");
            for ((((v, fs), vt), c), en) in e.values.iter().zip(&e.fields).zip(&vts).zip(checks).zip(enters) {
                let rows: Vec<(String, String)> = fs.iter().map(|f| (field_name(t, f.name.ascii()), g.boundary(&f.slot.ty))).collect();
                if rows.is_empty() {
                    out.push_str(&format!("// {vt} is the value {} of the enum {shown}.\ntype {vt}{decl} struct{{}}\n\n", v.text));
                } else {
                    let w = rows.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(0);
                    out.push_str(&format!("// {vt} is the value {} of the enum {shown}.\ntype {vt}{decl} struct {{\n", v.text));
                    for (n, fty) in &rows {
                        out.push_str(&format!("\t{n}{}{fty}\n", " ".repeat(w - n.chars().count() + 1)));
                    }
                    out.push_str("}\n\n");
                }
                out.push_str(&format!("func ({vt}{used}) {method}({marks}) {{}}\n\n"));
                if checked {
                    if c.is_empty() {
                        out.push_str(&format!("// KanameOutside is nil: a value of {}.{} has no field a parameter can find outside what it declares.\nfunc ({vt}) KanameOutside(string) *{k}.Outside {{ return nil }}\n\n", shown, v.text));
                    } else {
                        let mut body = c;
                        body.push(Node::Line(g.ret(g.null())));
                        out.push_str(&format!("// KanameOutside is where a value of {}.{} a parameter reaches is outside what its fields declare.\nfunc (v {vt}) KanameOutside(what string) *{k}.Outside {{\n", shown, v.text));
                        print(&body, t, 1, &mut out);
                        out.push_str("}\n\n");
                    }
                }
                if entered {
                    if en.is_empty() {
                        out.push_str(&format!("// KanameEnter is nil: a value of {}.{} has no field a host can give outside what it declares.\nfunc ({vt}) KanameEnter(string) *{k}.Outside {{ return nil }}\n\n", shown, v.text));
                    } else {
                        let mut body = en;
                        body.push(Node::Line(g.ret(g.null())));
                        out.push_str(&format!("// KanameEnter is where a value of {}.{} a host gives is outside what its fields declare, at every depth.\nfunc (v {vt}) KanameEnter(what string) *{k}.Outside {{\n", shown, v.text));
                        print(&body, t, 1, &mut out);
                        out.push_str("}\n\n");
                    }
                }
            }
        }
    }
    out
}

/// The module's records: TypeScript's interfaces, Python's frozen dataclasses, Go's structs; and for
/// each one a parameter may reach, the check of what its fields declare.
fn records(g: &Gen) -> String {
    let t = g.t;
    let mut out = String::new();
    for r in &g.c.decls.records {
        // a generic record (DESIGN 26.7) declares its type parameters; a value of it is checked where a
        // parameter reaches it, by the check of the type it is given there
        g.enter(&r.tparams, &[]);
        let generic = !r.tparams.is_empty();
        let decl = g.tparams_decl(&r.tparams);
        let ty = hidden(type_name(t, r.name.ascii()), r.private);
        // a `private` record is not exported (DESIGN 28.2)
        let export = if r.private { "" } else { "export " };
        let shown = &r.name.text;
        let rows: Vec<(String, String)> = r.fields.iter().map(|f| (field_name(t, f.name.ascii()), g.boundary(&f.slot.ty))).collect();
        g.leave();
        match t {
            Target::TypeScript => {
                out.push_str(&format!("/** The record {shown}. */\n{export}interface {ty}{decl} {{\n"));
                for (n, fty) in &rows {
                    out.push_str(&format!("  readonly {n}: {fty};\n"));
                }
                out.push_str("}\n\n");
            }
            Target::Python => {
                let base = if generic { format!("(Generic[{decl}])") } else { String::new() };
                out.push_str(&format!("\n@dataclass(frozen=True)\nclass {ty}{base}:\n    \"\"\"The record {}.\"\"\"\n\n", doc_text(shown)));
                for (n, fty) in &rows {
                    out.push_str(&format!("    {n}: {fty}\n"));
                }
                out.push_str("\n\n");
            }
            Target::Go => {
                let w = rows.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(0);
                out.push_str(&format!("// {ty} is the record {shown}.\ntype {ty}{decl} struct {{\n"));
                for (n, fty) in &rows {
                    out.push_str(&format!("\t{n}{}{fty}\n", " ".repeat(w - n.chars().count() + 1)));
                }
                out.push_str("}\n\n");
            }
            Target::Rust => {
                let vis = if r.private { "" } else { "pub " };
                let holder = Ty::Record(RecId::record(g.c.canon.clone(), r.name.text.clone()));
                g.enter(&r.tparams, &[]);
                let fields: Vec<(String, String)> = r.fields.iter().map(|f| (field_name(t, f.name.ascii()), g.field_type(&holder, &f.slot.ty))).collect();
                g.leave();
                // ordered field by field, as a key of a dict or of a sort is (`interp.rs`'s `key_cmp`)
                let derive = g.derives(&holder);
                if fields.is_empty() {
                    out.push_str(&format!("/// The record {shown}.\n{derive}\n{vis}struct {ty}{decl} {{}}\n\n"));
                } else {
                    out.push_str(&format!("/// The record {shown}.\n{derive}\n{vis}struct {ty}{decl} {{\n"));
                    for (n, fty) in &fields {
                        out.push_str(&format!("    pub {n}: {fty},\n"));
                    }
                    out.push_str("}\n\n");
                }
            }
        }
        // the check of the fields, as a parameter reaches them (DESIGN 4.4)
        let rt = Ty::Record(RecId::record(g.c.canon.clone(), r.name.text.clone()));
        if !generic && g.record_checkable(&rt, 0, Reach::Call) {
            let mut body: Vec<Node> = Vec::new();
            for (k, f) in r.fields.iter().enumerate() {
                let Some(fun) = g.outside_fn(&f.slot, Reach::Call) else { continue };
                let (v, what) = field_arg(g, &rt, f, &format!(".{}", f.name.text));
                let o = Ex::atom(format!("o{}", k + 1));
                body.extend(g.if_found(&o, Ex::call(fun, vec![v, what]), vec![Node::Line(g.ret_found(o.clone()))]));
            }
            body.push(Node::Line(g.ret(g.null())));
            let k = g.imp(Import::Kaname);
            let mut text = String::new();
            let (open, close) = match t {
                Target::TypeScript => (format!("/** Where a value of {shown} a parameter reaches is outside what its fields declare. */\n{export}function _outside_{ty}(v: {ty}, what: string): {k}.Outside | null {{\n"), "}\n\n".to_string()),
                Target::Python => (format!("\ndef _outside_{ty}(v: {ty}, what: str) -> {k}.Outside | None:\n    \"\"\"Where a value of {} a parameter reaches is outside what its fields declare.\"\"\"\n", doc_text(shown)), "\n\n".to_string()),
                Target::Go => (format!("// KanameOutside is where a value of {shown} a parameter reaches is outside what its fields declare.\nfunc (v {ty}) KanameOutside(what string) *{k}.Outside {{\n"), "}\n\n".to_string()),
                // a `private` record's check is the module's alone, which may call it nowhere
                Target::Rust => (format!("impl {ty} {{\n    /// Where a value of {shown} a parameter reaches is outside what its fields declare.\n{}    pub fn kaname_outside(&self, what: &str) -> Option<{k}::Outside> {{\n", if r.private { "    #[allow(dead_code)]\n" } else { "" }), "    }\n}\n\n".to_string()),
            };
            text.push_str(&open);
            if t == Target::Rust {
                print(&super::ex::tail(body), t, 2, &mut text);
            } else {
                print(&body, t, 1, &mut text);
            }
            text.push_str(&close);
            out.push_str(&text);
        }
        // the check of a host's argument, where the record can hold a value of a type that holds
        // itself: the fields at every depth (DESIGN 28.1)
        if !generic && g.p.reaches_recursive(&rt) && g.record_checkable(&rt, 0, Reach::Entry) {
            let mut body: Vec<Node> = Vec::new();
            for (k, f) in r.fields.iter().enumerate() {
                let Some(fun) = g.outside_fn(&f.slot, Reach::Entry) else { continue };
                let (v, what) = field_arg(g, &rt, f, &format!(".{}", f.name.text));
                let o = Ex::atom(format!("o{}", k + 1));
                body.extend(g.if_found(&o, Ex::call(fun, vec![v, what]), vec![Node::Line(g.ret_found(o.clone()))]));
            }
            body.push(Node::Line(g.ret(g.null())));
            let k = g.imp(Import::Kaname);
            let (open, close) = match t {
                Target::TypeScript => (format!("/** Where a value of {shown} a host gives is outside what its fields declare, at every depth. */\n{export}function _enter_{ty}(v: {ty}, what: string): {k}.Outside | null {{\n"), "}\n\n".to_string()),
                Target::Python => (format!("\ndef _enter_{ty}(v: {ty}, what: str) -> {k}.Outside | None:\n    \"\"\"Where a value of {} a host gives is outside what its fields declare, at every depth.\"\"\"\n", doc_text(shown)), "\n\n".to_string()),
                Target::Go => (format!("// KanameEnter is where a value of {shown} a host gives is outside what its fields declare, at every depth.\nfunc (v {ty}) KanameEnter(what string) *{k}.Outside {{\n"), "}\n\n".to_string()),
                Target::Rust => (format!("impl {ty} {{\n    /// Where a value of {shown} a host gives is outside what its fields declare, at every depth.\n{}    pub fn kaname_enter(&self, what: &str) -> Option<{k}::Outside> {{\n", if r.private { "    #[allow(dead_code)]\n" } else { "" }), "    }\n}\n\n".to_string()),
            };
            out.push_str(&open);
            if t == Target::Rust {
                print(&super::ex::tail(body), t, 2, &mut out);
            } else {
                print(&body, t, 1, &mut out);
            }
            out.push_str(&close);
        }
    }
    out
}

/// The type of the value `v` of the sum `e` of the module, as a holder of fields.
fn variant_ty(g: &Gen, e: &crate::names::EnumInfo, v: &crate::ast::Name) -> Ty {
    Ty::Record(RecId::Variant { file: g.c.canon.clone(), name: e.name.text.clone(), value: v.text.clone(), args: Vec::new() })
}

/// A field `f` of the value a check of `holder` is given, as the check of its place takes it, and its
/// path: `v.lines` and `what + ".lines"`; Rust's of `self`, lent (`&self.lines`,
/// `&format!("{what}.lines")`).
fn field_arg(g: &Gen, holder: &Ty, f: &crate::names::FieldInfo, tail: &str) -> (Ex, Ex) {
    if g.t == Target::Rust {
        return (g.lent_field(&Ex::atom("self"), holder, f.name.ascii(), &f.slot.ty), g.field_path(tail));
    }
    (Ex::atom("v").dot(field_name(g.t, f.name.ascii())), Ex::bin(super::ex::Op::Add, Ex::atom("what"), g.string(tail)))
}

/// The kinds of failure a function's effect has, in their order.
fn kinds(g: &Gen, f: usize) -> Vec<&'static str> {
    g.c.analysis.get().and_then(|a| a.fns.get(f)).map(|x| x.effect.fails.iter().map(|k| k.word()).collect()).unwrap_or_default()
}

fn diverges(g: &Gen, f: usize) -> bool {
    g.c.analysis.get().and_then(|a| a.fns.get(f)).is_some_and(|x| x.effect.diverges)
}

/// Every function, the one a host calls and the one the module's functions call.
fn functions(g: &Gen) -> String {
    let t = g.t;
    let c = &g.c;
    let mut out = String::new();
    for (f, info) in c.decls.fns.iter().enumerate() {
        if info.inner() {
            out.push_str(&generic(g, f));
            continue;
        }
        let decl = &c.module.fns[f];
        let public = fn_name(t, c, f);
        let inner = inner_name(t, c, f);
        let ks = kinds(g, f);
        let div = diverges(g, f);
        let fails = !ks.is_empty();
        // the parameters' names in the code of the body (DESIGN 10.3), and in the function a host
        // calls: the same in TypeScript and Go, the aliases in Python, which a host calls by keyword
        let params = param_names(g, f);
        let given: Vec<String> = match t {
            Target::Python => info.params.iter().map(|p| p.name.ascii().to_string()).collect(),
            _ => params.clone(),
        };
        let ptypes: Vec<String> = info.params.iter().map(|p| if t == Target::Rust { g.param_rust(&p.slot.ty) } else { g.boundary(&p.slot.ty) }).collect();
        let ret = g.boundary(&info.ret.ty);
        let body = Fx::new(g, f).body(&params);
        let unread: Vec<String> = g.unread.borrow().clone();
        // the check of what a host gives (DESIGN 4.4), and the parameters it reads
        let mut entry: Vec<Node> = Vec::new();
        let mut checked_params: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for (p, name) in info.params.iter().zip(&given) {
            // how deep it nests the values of types that hold themselves, first: the check that
            // follows walks no deeper (DESIGN 28.1)
            if let Some(nf) = g.nest_fn(&p.slot.ty) {
                let n = Ex::atom(match t {
                    Target::Go => format!("n{}", ritsu_emit::ident::pascal(name)),
                    _ => format!("n_{}", name.trim_start_matches("r#")),
                });
                checked_params.insert(name.clone());
                let fail = g.fail_nested(info.line, &info.name.text, &p.name.text, &n);
                entry.extend(g.if_nested(&n, Ex::call(nf, vec![Ex::atom(name.clone()), Ex::atom("0")]), fail));
            }
            let Some(fun) = g.outside_fn(&p.slot, Reach::Entry) else { continue };
            // where the argument is outside: `o_paid_on`, and in Go `oPaidOn`
            let o = Ex::atom(match t {
                Target::Go => format!("o{}", ritsu_emit::ident::pascal(name)),
                _ => format!("o_{}", name.trim_start_matches("r#")),
            });
            checked_params.insert(name.clone());
            let fail = g.fail_outside("input", info.line, &Said::Entry { fun: info.name.text.clone() }, &o, &p.slot, Reach::Entry);
            entry.extend(g.if_found(&o, Ex::call(fun, vec![Ex::atom(name.clone()), g.string(&p.name.text)]), fail));
        }
        let description = decl.description.as_ref().map(|(d, _)| d.clone()).unwrap_or_default();
        let effect = effect_text(g, &ks, div);
        let args = params.join(", ");
        match t {
            Target::TypeScript => {
                let k = g.imp(Import::Kaname);
                let fty = fails_name(c, f);
                let union = if ks.is_empty() { "never".to_string() } else { ks.iter().map(|w| format!("\"{w}\"")).collect::<Vec<_>>().join(" | ") };
                out.push_str(&format!("/** The kinds of failure `{public}` can end in besides `input` (kaname's DESIGN 4.5). */\nexport type {fty} = {union};\n\n"));
                let sig: Vec<String> = params.iter().zip(&ptypes).map(|(n, ty)| format!("{n}: {ty}")).collect();
                out.push_str("/**\n");
                if !description.is_empty() {
                    out.push_str(&format!(" * {}\n *\n", description.replace("*/", "* /")));
                }
                out.push_str(&format!(" * {effect}\n * @throws {{{k}.KanameError}} of the kind `input` when an argument is outside what its parameter declares, and of a kind of `{fty}`.\n */\n"));
                out.push_str(&format!("export function {public}({}): {ret} {{\n", sig.join(", ")));
                let mut s = String::new();
                print(&entry, t, 1, &mut s);
                out.push_str(&s);
                out.push_str(&format!("  return {inner}({args});\n}}\n\n"));
                out.push_str(&format!("function {inner}({}): {ret} {{\n", sig.join(", ")));
                let mut s = String::new();
                print(&body, t, 1, &mut s);
                out.push_str(&s);
                out.push_str("}\n\n");
            }
            Target::Python => {
                let k = g.imp(Import::Kaname);
                let fty = fails_name(c, f);
                let lit = if ks.is_empty() { "NoReturn".to_string() } else { format!("Literal[{}]", ks.iter().map(|w| format!("\"{w}\"")).collect::<Vec<_>>().join(", ")) };
                out.push_str(&format!("\n# The kinds of failure `{public}` can end in besides `input` (kaname's DESIGN 4.5).\n{fty} = {lit}\n\n"));
                let star = |names: &[String]| -> String {
                    let sig: Vec<String> = names.iter().zip(&ptypes).map(|(n, ty)| format!("{n}: {ty}")).collect();
                    if sig.is_empty() { String::new() } else { format!("*, {}", sig.join(", ")) }
                };
                let doc = if description.is_empty() { effect.clone() } else { format!("{}\n\n    {effect}", doc_text(&description)) };
                out.push_str(&format!(
                    "\ndef {public}({}) -> {ret}:\n    \"\"\"{doc}\n\n    Raises:\n        {k}.KanameError: of the kind `input` when an argument is outside what its parameter declares, and of a kind of `{fty}`.\n    \"\"\"\n",
                    star(&given)
                ));
                let mut s = String::new();
                print(&entry, t, 1, &mut s);
                out.push_str(&s);
                let kw: Vec<String> = params.iter().zip(&given).map(|(n, a)| format!("{n}={a}")).collect();
                out.push_str(&format!("    return {inner}({})\n\n", kw.join(", ")));
                out.push_str(&format!("\ndef {inner}({}) -> {ret}:\n", star(&params)));
                let mut s = String::new();
                print(&body, t, 1, &mut s);
                out.push_str(&s);
                out.push_str("\n\n");
            }
            Target::Go => {
                let k = g.imp(Import::Kaname);
                let sig: Vec<String> = params.iter().zip(&ptypes).map(|(n, ty)| format!("{n} {ty}")).collect();
                let check = format!("Check{public}");
                // the check of what a host gives, alone (DESIGN 4.4)
                out.push_str(&format!("// {check} is whether the arguments are what {public} takes: a *{k}.KanameError of the kind input when one is outside what its parameter declares.\nfunc {check}({}) (err error) {{\n\tdefer {k}.Recover(&err)\n", sig.join(", ")));
                let mut s = String::new();
                print(&entry, t, 1, &mut s);
                out.push_str(&s);
                out.push_str("\treturn nil\n}\n\n");
                let doc = if description.is_empty() { String::new() } else { format!("// {public}: {}\n//\n", description.replace('\n', " ")) };
                if fails {
                    out.push_str(&format!("{doc}// {effect} It returns a *{k}.KanameError of the kind input when an argument is outside what its parameter declares, and of a kind of its effect.\nfunc {public}({}) (_ {ret}, err error) {{\n\tif err = {check}({args}); err != nil {{\n\t\treturn\n\t}}\n\tdefer {k}.Recover(&err)\n\treturn {inner}({args}), nil\n}}\n\n", sig.join(", ")));
                } else {
                    out.push_str(&format!("{doc}// {effect} It panics with a *{k}.KanameError of the kind input when an argument is outside what its parameter declares; {check} says so first.\nfunc {public}({}) {ret} {{\n\tif err := {check}({args}); err != nil {{\n\t\tpanic(err)\n\t}}\n\treturn {inner}({args})\n}}\n\n", sig.join(", ")));
                }
                out.push_str(&format!("func {inner}({}) {ret} {{\n", sig.join(", ")));
                let mut s = String::new();
                print(&body, t, 1, &mut s);
                out.push_str(&s);
                out.push_str("}\n\n");
            }
            Target::Rust => {
                let k = g.imp(Import::Kaname);
                let check = check_name(t, c, f);
                let err = format!("{k}::KanameError");
                let result = g.std_type("Result");
                let sig = |names: &dyn Fn(&String) -> String| -> String { params.iter().zip(&ptypes).map(|(n, ty)| format!("{}: {ty}", names(n))).collect::<Vec<_>>().join(", ") };
                // a parameter the code does not read is `_`
                let check_sig = sig(&|n| if checked_params.contains(n) { n.clone() } else { "_".into() });
                out.push_str(&format!("/// Whether the arguments are what `{public}` takes: a `KanameError` of the kind input when one is\n/// outside what its parameter declares.\npub fn {check}({check_sig}) -> {result}<(), {err}> {{\n"));
                let mut s = String::new();
                print(&entry, t, 1, &mut s);
                out.push_str(&s);
                out.push_str("    Ok(())\n}\n\n");
                if !description.is_empty() {
                    out.push_str(&rs_doc("///", &description));
                    out.push_str("///\n");
                }
                let rt = if fails { format!("{result}<{ret}, {err}>") } else { ret.clone() };
                if fails {
                    out.push_str(&format!("/// {effect} It returns a `KanameError` of the kind input when an argument is outside what its\n/// parameter declares, and of a kind of its effect.\npub fn {public}({}) -> {rt} {{\n    {check}({args})?;\n    {inner}({args})\n}}\n\n", sig(&|n| n.clone())));
                } else {
                    out.push_str(&format!("/// {effect} It panics when an argument is outside what its parameter declares; `{check}` says so\n/// first, with a `KanameError` of the kind input.\npub fn {public}({}) -> {rt} {{\n    if let Err(e) = {check}({args}) {{\n        panic!(\"{{e}}\");\n    }}\n    {inner}({args})\n}}\n\n", sig(&|n| n.clone())));
                }
                let inner_sig = sig(&|n| if unread.contains(n) { "_".into() } else { n.clone() });
                out.push_str(&format!("fn {inner}({inner_sig}) -> {rt} {{\n"));
                let mut s = String::new();
                print(&super::ex::tail(body), t, 1, &mut s);
                out.push_str(&s);
                out.push_str("}\n\n");
            }
        }
    }
    out
}

/// A generic function (DESIGN 21.3, 26.7): one function, which the generated code of every module calls
/// and no host (it does not check its arguments at an entry), with its type parameters, and before its
/// parameters the order of each type parameter held to keys: TypeScript's `kaname.Order<K>`, Python's
/// function of the key (a parameter that goes by its place, `/`), Go's `func(a, b K) int`.
fn generic(g: &Gen, f: usize) -> String {
    let t = g.t;
    let c = &g.c;
    let info = &c.decls.fns[f];
    let decl = &c.module.fns[f];
    let name = generic_name(t, c, f);
    let params = param_names(g, f);
    let tnames = g.tparam_names(&info.tparams);
    // the names of the orders, apart from the parameters' and the module's
    let mut taken: std::collections::BTreeSet<String> = params.iter().cloned().collect();
    let orders: Vec<Option<String>> = info
        .tparams
        .iter()
        .zip(&tnames)
        .map(|(p, n)| {
            if !p.key {
                return None;
            }
            let base = match t {
                Target::TypeScript => format!("order_{n}"),
                Target::Python => format!("key_{n}"),
                Target::Go => format!("order{}", ritsu_emit::ident::go_exported(&ritsu_emit::ident::pascal(n))),
                // a key's order is its type's `Ord` (DESIGN 36.2)
                Target::Rust => return None,
            };
            let mut o = base.clone();
            let mut i = 2;
            while taken.contains(&o) || g.outer.contains(&o) {
                o = if t == Target::Go { format!("{base}{i}") } else { format!("{base}_{i}") };
                i += 1;
            }
            taken.insert(o.clone());
            Some(o)
        })
        .collect();
    g.enter(&info.tparams, &orders);
    let ptypes: Vec<String> = info.params.iter().map(|p| if t == Target::Rust { g.param_rust(&p.slot.ty) } else { g.boundary(&p.slot.ty) }).collect();
    let ret = g.boundary(&info.ret.ty);
    let hidden: Vec<(String, String)> = orders
        .iter()
        .zip(&tnames)
        .filter_map(|(o, n)| {
            let o = o.clone()?;
            let ty = match t {
                Target::TypeScript => format!("{}.Order<{n}>", g.imp(Import::Kaname)),
                Target::Python => format!("Callable[[{n}], Any]"),
                Target::Go => format!("func(a, b {n}) int"),
                Target::Rust => String::new(),
            };
            Some((o, ty))
        })
        .collect();
    let hidden_names: Vec<String> = hidden.iter().map(|(n, _)| n.clone()).collect();
    let body = Fx::new(g, f).body_with(&params, &hidden_names);
    let unread: Vec<String> = g.unread.borrow().clone();
    let tdecl = g.tparams_decl(&info.tparams);
    let k = if t == Target::Rust && super::plan::rs_fails(&g.c, f) { g.imp(Import::Kaname) } else { String::new() };
    g.leave();
    let effect = effect_text(g, &kinds(g, f), diverges(g, f));
    let what = match g.lang {
        ritsu_base::text::Lang::Ja => "kaname の生成したコードが呼ぶ関数で、入口で引数を確かめません。",
        _ => "A function kaname's generated code calls: it does not check its arguments at an entry.",
    };
    let description = decl.description.as_ref().map(|(d, _)| d.clone()).unwrap_or_default();
    let mut out = String::new();
    match t {
        Target::TypeScript => {
            let sig: Vec<String> = hidden.iter().map(|(n, ty)| format!("{n}: {ty}")).chain(params.iter().zip(&ptypes).map(|(n, ty)| format!("{n}: {ty}"))).collect();
            out.push_str("/**\n");
            if !description.is_empty() {
                out.push_str(&format!(" * {}\n *\n", description.replace("*/", "* /")));
            }
            let export = if info.private { "" } else { "export " };
            out.push_str(&format!(" * {what} {effect}\n */\n{export}function {name}{tdecl}({}): {ret} {{\n", sig.join(", ")));
            print(&body, t, 1, &mut out);
            out.push_str("}\n\n");
        }
        Target::Python => {
            let mut sig: Vec<String> = hidden.iter().map(|(n, ty)| format!("{n}: {ty}")).collect();
            if !sig.is_empty() {
                sig.push("/".into());
            }
            if !params.is_empty() {
                sig.push("*".into());
                sig.extend(params.iter().zip(&ptypes).map(|(n, ty)| format!("{n}: {ty}")));
            }
            let doc = if description.is_empty() { format!("{what} {effect}") } else { format!("{}\n\n    {what} {effect}", doc_text(&description)) };
            out.push_str(&format!("\ndef {name}({}) -> {ret}:\n    \"\"\"{doc}\"\"\"\n", sig.join(", ")));
            print(&body, t, 1, &mut out);
            out.push_str("\n\n");
        }
        Target::Go => {
            let sig: Vec<String> = hidden.iter().map(|(n, ty)| format!("{n} {ty}")).chain(params.iter().zip(&ptypes).map(|(n, ty)| format!("{n} {ty}"))).collect();
            let lead = match g.lang {
                ritsu_base::text::Lang::Ja => format!("// {name} は、{what}"),
                _ => format!("// {name} is a function kaname's generated code calls: it does not check its arguments at an entry."),
            };
            if !description.is_empty() {
                out.push_str(&format!("// {name}: {}\n//\n", description.replace('\n', " ")));
            }
            out.push_str(&format!("{lead} {effect}\nfunc {name}{tdecl}({}) {ret} {{\n", sig.join(", ")));
            print(&body, t, 1, &mut out);
            out.push_str("}\n\n");
        }
        Target::Rust => {
            let sig: Vec<String> = params.iter().zip(&ptypes).map(|(n, ty)| format!("{}: {ty}", if unread.contains(n) { "_" } else { n.as_str() })).collect();
            if !description.is_empty() {
                out.push_str(&rs_doc("///", &description));
                out.push_str("///\n");
            }
            let vis = if info.private { "" } else { "pub " };
            let rt = if k.is_empty() { ret } else { format!("{}<{ret}, {k}::KanameError>", g.std_type("Result")) };
            out.push_str(&format!("/// {what} {effect}\n{vis}fn {name}{tdecl}({}) -> {rt} {{\n", sig.join(", ")));
            print(&super::ex::tail(body), t, 1, &mut out);
            out.push_str("}\n\n");
        }
    }
    out
}

/// What a function's effect says, in a comment (DESIGN 4.5).
fn effect_text(g: &Gen, ks: &[&str], div: bool) -> String {
    let kinds = ks.join(", ");
    let mut s = match (ks.is_empty(), g.lang) {
        (true, ritsu_base::text::Lang::Ja) => "エフェクト: total（どの入力にも答えます）。".to_string(),
        (true, _) => "Effect: total (it answers every input).".to_string(),
        (false, ritsu_base::text::Lang::Ja) => format!("エフェクト: fails {kinds}（走らせたときに確かめる場所があります）。"),
        (false, _) => format!("Effect: fails {kinds} (it checks some places when it runs)."),
    };
    if div {
        s.push_str(match g.lang {
            ritsu_base::text::Lang::Ja => " 再帰があり、止まらないことがあります。",
            _ => " It recurses, and may not stop.",
        });
    }
    s
}

/// The checks of what a place declares the bodies asked for.
fn outsides(g: &Gen) -> String {
    let t = g.t;
    let mut out = String::new();
    // a check can ask for another (a list of records): made until no more come
    let mut done = 0;
    loop {
        let list = g.outsides.borrow().clone();
        if done >= list.len() {
            break;
        }
        for super::plan::Outside { name, body, ty, tparams, .. } in &list[done..] {
            let k = g.imp(Import::Kaname);
            let mut nodes = body.clone();
            nodes.push(Node::Line(g.ret(g.null())));
            let (open, close) = match t {
                Target::TypeScript => (format!("function {name}{tparams}(v: {ty}, what: string): {k}.Outside | null {{\n"), "}\n\n"),
                Target::Python => (format!("\ndef {name}(v: {ty}, what: str) -> {k}.Outside | None:\n"), "\n\n"),
                Target::Go => (format!("func {name}{tparams}(v {ty}, what string) *{k}.Outside {{\n"), "}\n\n"),
                Target::Rust => (format!("fn {name}{tparams}(v: {ty}, what: &str) -> Option<{k}::Outside> {{\n"), "}\n\n"),
            };
            out.push_str(&open);
            let mut s = String::new();
            let nodes = if t == Target::Rust { super::ex::tail(nodes) } else { nodes };
            print(&nodes, t, 1, &mut s);
            out.push_str(&s);
            out.push_str(close);
        }
        done = list.len();
    }
    out
}

/// The functions that find a host's argument nested too deep (DESIGN 28.1), as the entries ask for
/// them: a function can ask for another (a list of records), so they are made until no more come.
fn nests(g: &Gen) -> String {
    let t = g.t;
    let mut out = String::new();
    let mut done = 0;
    loop {
        let list = g.nests.borrow().clone();
        if done >= list.len() {
            break;
        }
        for super::plan::Outside { name, body, ty, tparams, .. } in &list[done..] {
            let mut nodes = body.clone();
            nodes.push(Node::Line(g.ret(match t {
                Target::Go => g.string(""),
                _ => g.null(),
            })));
            let (open, close) = match t {
                Target::TypeScript => (format!("function {name}{tparams}(v: {ty}, above: number): string | null {{\n"), "}\n\n"),
                Target::Python => (format!("\ndef {name}(v: {ty}, above: int) -> str | None:\n"), "\n\n"),
                Target::Go => (format!("func {name}{tparams}(v {ty}, above int) string {{\n"), "}\n\n"),
                Target::Rust => (format!("fn {name}{tparams}(v: {ty}, above: i64) -> Option<&'static str> {{\n"), "}\n\n"),
            };
            out.push_str(&open);
            let mut s = String::new();
            let nodes = if t == Target::Rust { super::ex::tail(nodes) } else { nodes };
            print(&nodes, t, 1, &mut s);
            out.push_str(&s);
            out.push_str(close);
        }
        done = list.len();
    }
    out
}

/// Go's functions that tell whether two values of a type are the same, as the bodies ask for them
/// (DESIGN 23.8): a function can ask for another (a list of records), so they are made until no more
/// come.
fn eqs(g: &Gen) -> String {
    if g.t != Target::Go {
        return String::new();
    }
    let mut out = String::new();
    let mut done = 0;
    loop {
        let list = g.eqs.borrow().clone();
        if done >= list.len() {
            break;
        }
        for (_, _, def) in &list[done..] {
            out.push_str(def);
            out.push('\n');
        }
        done = list.len();
    }
    out
}

/// The orders of the records the bodies order by (a key of `sort_by`, DESIGN 24.6): TypeScript's and
/// Go's functions that compare two, Python's that give the key.
fn cmps(g: &Gen) -> String {
    let list = g.cmps.borrow().clone();
    list.iter().map(|(_, _, def)| format!("{def}\n")).collect()
}

/// The orders of the enums the bodies compare.
fn ranks(g: &Gen) -> String {
    let t = g.t;
    let mut out = String::new();
    let list: Vec<(EnumId, String)> = g.ranks.borrow().clone();
    for (id, name) in list {
        let ty = g.enum_type(&id);
        let members: Vec<String> = g.enum_values(&id).iter().map(|v| g.member(&id, v).print(t)).collect();
        match t {
            Target::TypeScript => out.push_str(&format!("function {name}(v: {ty}): number {{\n  return [{}].indexOf(v);\n}}\n\n", members.join(", "))),
            Target::Python => out.push_str(&format!("\ndef {name}(v: {ty}) -> int:\n    return [{}].index(v)\n\n", members.join(", "))),
            Target::Go => {
                out.push_str(&format!("func {name}(v {ty}) int {{\n\tswitch v {{\n"));
                for (i, m) in members.iter().enumerate() {
                    out.push_str(&format!("\tcase {m}:\n\t\treturn {i}\n"));
                }
                out.push_str("\t}\n\treturn -1\n}\n\n");
            }
            Target::Rust => {
                out.push_str(&format!("fn {name}(v: {ty}) -> i64 {{\n    match v {{\n"));
                for (i, m) in members.iter().enumerate() {
                    out.push_str(&format!("        {m} => {i},\n"));
                }
                out.push_str("    }\n}\n\n");
            }
        }
        let _ = Owner::Module;
    }
    out
}
