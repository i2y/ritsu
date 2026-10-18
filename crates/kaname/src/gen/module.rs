//! One module of kaname as a module of the target (DESIGN 10.1–10.3): its head, its imports, its
//! enums and records, and each function twice — the one a host calls, which holds what it is given
//! to what the parameters declare (DESIGN 4.4), and the one the module's own functions call, past
//! that check — with the checks and the orders the bodies ask for.

use super::ex::{Ex, Node, print};
use super::plan::{Fx, Gen, Import, Said, fails_name, field_name, fn_name, go_package, inner_name, member_name, param_names, type_name};
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
    };
    // the declarations and the functions first: they say what the imports are
    let parts: Vec<String> = vec![enums(&g), records(&g), functions(&g), outsides(&g), ranks(&g)];
    let body: String = parts.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join("\n");
    let text = format!("{}{}{}", head(&g, &alias), imports(&g, &body), body);
    let units = g.units.borrow().clone();
    (path, tidy(t, &text), units)
}

/// No two blank lines in a row, and one newline at the end.
fn tidy(t: Target, s: &str) -> String {
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
    let origin = Origin::at(&g.opts.shown, c.src.as_bytes());
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
            for (name, i) in used.iter() {
                let line = match i {
                    Import::Kaname => format!("import * as {name} from \"./kaname\";"),
                    Import::Units => format!("import type * as {name} from \"../units\";"),
                    Import::Datetime => continue,
                    Import::Rule(f) => {
                        let m = g.p.rules.get(f).map(|r| r.typescript.module.trim_end_matches(".ts").to_string()).unwrap_or_default();
                        format!("import * as {name} from \"../rules/{m}\";")
                    }
                    Import::Dates(f) => {
                        let a = g.p.dates.get(f).map(|d| d.alias.clone()).unwrap_or_default();
                        format!("import * as {name} from \"../dates/{a}\";")
                    }
                    Import::Module(f) => format!("import * as {name} from \"./{}\";", g.p.module(f).module.name.ascii()),
                    Import::Book(f) => {
                        let b = g.p.books.get(f).map(|b| b.name.clone()).unwrap_or_default();
                        format!("import type * as {name} from \"./books/{b}\";")
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
            if body.contains("@dataclass(frozen=True)") {
                std_lines.push("from dataclasses import dataclass".to_string());
            }
            let typing: Vec<&str> = ["Literal", "NoReturn"].into_iter().filter(|w| body.contains(&format!(" = {w}"))).collect();
            if !typing.is_empty() {
                std_lines.push(format!("from typing import {}", typing.join(", ")));
            }
            if !std_lines.is_empty() {
                out.push_str(&std_lines.join("\n"));
                out.push_str("\n\n");
            }
            for (name, i) in used.iter() {
                let line = match i {
                    Import::Kaname => "from . import kaname".to_string(),
                    Import::Units => format!("from .. import units as {name}"),
                    Import::Datetime => continue,
                    Import::Rule(f) => {
                        let m = g.p.rules.get(f).map(|r| r.python.module.clone()).unwrap_or_default();
                        format!("from ..rules import {m} as {name}")
                    }
                    Import::Dates(f) => {
                        let a = g.p.dates.get(f).map(|d| d.alias.clone()).unwrap_or_default();
                        format!("from ..dates import {a} as {name}")
                    }
                    Import::Module(f) => format!("from . import {} as {name}", g.p.module(f).module.name.ascii()),
                    Import::Book(f) => {
                        let b = g.p.books.get(f).map(|b| b.name.clone()).unwrap_or_default();
                        format!("from .books import {b} as {name}")
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
                        (format!("{m}/rules/{p}"), p)
                    }
                    Import::Dates(f) => {
                        let p = g.p.dates.get(f).map(|d| go_package(&d.alias)).unwrap_or_default();
                        (format!("{m}/dates/{p}"), p)
                    }
                    Import::Module(f) => {
                        let p = go_package(g.p.module(f).module.name.ascii());
                        (format!("{m}/glue/{p}"), p)
                    }
                    Import::Book(f) => {
                        let p = super::books::go_package_of(&g.p, f);
                        (format!("{m}/glue/books/{p}"), p)
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
    }
    out
}

/// The module's enums: TypeScript's frozen object and its union, Python's `enum.Enum`, Go's named
/// string and its constants. A value is its name (DESIGN 10.2).
fn enums(g: &Gen) -> String {
    let t = g.t;
    let mut out = String::new();
    for e in &g.c.module.enums {
        let alias = e.name.ascii();
        let ty = type_name(t, alias);
        let shown = &e.name.text;
        match t {
            Target::TypeScript => {
                out.push_str(&format!("/** The enum {shown}. */\nexport const {ty} = {{\n"));
                for v in &e.values {
                    out.push_str(&format!("  {}: {},\n", member_name(t, alias, v.name.ascii()), ritsu_emit::lit::json(&v.name.text)));
                }
                out.push_str(&format!("}} as const;\nexport type {ty} = (typeof {ty})[keyof typeof {ty}];\n\n"));
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
                let names: Vec<String> = e.values.iter().map(|v| member_name(t, alias, v.name.ascii())).collect();
                let w = names.iter().map(|n| n.chars().count()).max().unwrap_or(0);
                out.push_str("const (\n");
                for (n, v) in names.iter().zip(&e.values) {
                    out.push_str(&format!("\t{n}{}{ty} = {}\n", " ".repeat(w - n.chars().count() + 1), ritsu_emit::lit::go(&v.name.text)));
                }
                out.push_str(")\n\n");
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
        let ty = type_name(t, r.name.ascii());
        let shown = &r.name.text;
        let rows: Vec<(String, String)> = r.fields.iter().map(|f| (field_name(t, f.name.ascii()), g.boundary(&f.slot.ty))).collect();
        match t {
            Target::TypeScript => {
                out.push_str(&format!("/** The record {shown}. */\nexport interface {ty} {{\n"));
                for (n, fty) in &rows {
                    out.push_str(&format!("  readonly {n}: {fty};\n"));
                }
                out.push_str("}\n\n");
            }
            Target::Python => {
                out.push_str(&format!("\n@dataclass(frozen=True)\nclass {ty}:\n    \"\"\"The record {}.\"\"\"\n\n", doc_text(shown)));
                for (n, fty) in &rows {
                    out.push_str(&format!("    {n}: {fty}\n"));
                }
                out.push_str("\n\n");
            }
            Target::Go => {
                let w = rows.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(0);
                out.push_str(&format!("// {ty} is the record {shown}.\ntype {ty} struct {{\n"));
                for (n, fty) in &rows {
                    out.push_str(&format!("\t{n}{}{fty}\n", " ".repeat(w - n.chars().count() + 1)));
                }
                out.push_str("}\n\n");
            }
        }
        // the check of the fields, as a parameter reaches them (DESIGN 4.4)
        let rt = Ty::Record(RecId::Module { file: g.c.canon.clone(), name: r.name.text.clone() });
        if g.record_checkable(&rt, 0) {
            let mut body: Vec<Node> = Vec::new();
            for (k, f) in r.fields.iter().enumerate() {
                let Some(fun) = g.outside_fn(&f.slot, true) else { continue };
                let v = Ex::atom("v").dot(field_name(t, f.name.ascii()));
                let what = Ex::bin(super::ex::Op::Add, Ex::atom("what"), g.string(&format!(".{}", f.name.text)));
                let o = Ex::atom(format!("o{}", k + 1));
                body.extend(g.if_found(&o, Ex::call(fun, vec![v, what]), vec![Node::Line(g.ret(o.clone()))]));
            }
            body.push(Node::Line(g.ret(g.null())));
            let k = g.imp(Import::Kaname);
            let mut text = String::new();
            let (open, close) = match t {
                Target::TypeScript => (format!("/** Where a value of {shown} a parameter reaches is outside what its fields declare. */\nexport function _outside_{ty}(v: {ty}, what: string): {k}.Outside | null {{\n"), "}\n\n".to_string()),
                Target::Python => (format!("\ndef _outside_{ty}(v: {ty}, what: str) -> {k}.Outside | None:\n    \"\"\"Where a value of {} a parameter reaches is outside what its fields declare.\"\"\"\n", doc_text(shown)), "\n\n".to_string()),
                Target::Go => (format!("// KanameOutside is where a value of {shown} a parameter reaches is outside what its fields declare.\nfunc (v {ty}) KanameOutside(what string) *{k}.Outside {{\n"), "}\n\n".to_string()),
            };
            text.push_str(&open);
            print(&body, t, 1, &mut text);
            text.push_str(&close);
            out.push_str(&text);
        }
    }
    out
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
        let ptypes: Vec<String> = info.params.iter().map(|p| g.boundary(&p.slot.ty)).collect();
        let ret = g.boundary(&info.ret.ty);
        let body = Fx::new(g, f).body(&params);
        // the check of what a host gives (DESIGN 4.4)
        let mut entry: Vec<Node> = Vec::new();
        for (p, name) in info.params.iter().zip(&given) {
            let Some(fun) = g.outside_fn(&p.slot, true) else { continue };
            // where the argument is outside: `o_paid_on`, and in Go `oPaidOn`
            let o = Ex::atom(match t {
                Target::Go => format!("o{}", ritsu_emit::ident::pascal(name)),
                _ => format!("o_{name}"),
            });
            let fail = g.fail_outside("input", info.line, &Said::Entry { fun: info.name.text.clone() }, &o, g.counts(&p.slot, true));
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
        for super::plan::Outside { name, body, ty, .. } in &list[done..] {
            let k = g.imp(Import::Kaname);
            let mut nodes = body.clone();
            nodes.push(Node::Line(g.ret(g.null())));
            let (open, close) = match t {
                Target::TypeScript => (format!("function {name}(v: {ty}, what: string): {k}.Outside | null {{\n"), "}\n\n"),
                Target::Python => (format!("\ndef {name}(v: {ty}, what: str) -> {k}.Outside | None:\n"), "\n\n"),
                Target::Go => (format!("func {name}(v {ty}, what string) *{k}.Outside {{\n"), "}\n\n"),
            };
            out.push_str(&open);
            let mut s = String::new();
            print(&nodes, t, 1, &mut s);
            out.push_str(&s);
            out.push_str(close);
        }
        done = list.len();
    }
    out
}

/// The orders of the enums the bodies compare.
fn ranks(g: &Gen) -> String {
    let t = g.t;
    let mut out = String::new();
    let list: Vec<(EnumId, String)> = g.ranks.borrow().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
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
        }
        let _ = Owner::Module;
    }
    out
}
