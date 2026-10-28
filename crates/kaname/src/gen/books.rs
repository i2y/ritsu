//! The transfers a package's modules build, by book (DESIGN 6.3, 10.6): `glue/books/<book>` holds the
//! type of each operation built (its book, transfer and operation, its arguments, and the reasons it
//! can be refused with), written once for every module that builds one; `glue/books/<book>_apply`
//! holds the function that makes an operation through chobo's client of the book. The types read
//! nothing of chobo's; the function that applies them does, so a module that builds transfers does
//! not reach the book's driver.

use super::{Options, Target};
use crate::check::Checked;
use crate::ranges::SiteKind;
use crate::ty::TransferId;
use crate::value::Program;
use ritsu_ports::BookFacts;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The type of an operation of a transfer: `CaptureDo` (Go's exported: `X入金Do` for a name with no
/// capital).
pub fn op_type(t: Target, transfer: &str, op: &str) -> String {
    let n = format!("{}{}", ritsu_emit::ident::pascal(transfer), ritsu_emit::ident::pascal(op));
    match t {
        Target::Go => ritsu_emit::ident::go_exported(&n),
        _ => n,
    }
}

/// The Go package of the types of a book's transfers: the package of chobo's client of it.
pub fn go_package_of(p: &Program, book: &Path) -> String {
    p.books.get(book).map(|b| b.go.module.clone()).unwrap_or_else(|| "book".into())
}

/// The name of each parameter of a transfer as the target's code of it names it: the book's name in
/// TypeScript (the client's), the client's in Python and Go.
pub fn param_names(t: Target, book: &BookFacts, transfer: &str) -> Vec<String> {
    let Some(k) = book.transfers.iter().position(|x| x.name == transfer) else { return Vec::new() };
    let tr = &book.transfers[k];
    match t {
        Target::TypeScript => tr.params.iter().map(|p| p.name.clone()).collect(),
        Target::Python => book.python.transfers.get(k).map(|c| c.params.clone()).unwrap_or_default(),
        Target::Go => book.go.transfers.get(k).map(|c| c.params.clone()).unwrap_or_default(),
    }
}

/// The parameters of a `post` that may be left out (its amounts), by the target's names of them.
pub fn optional_amounts(book: &BookFacts, transfer: &str, op: &str) -> Vec<String> {
    if op != "post" {
        return Vec::new();
    }
    let Some(tr) = book.transfers.iter().find(|x| x.name == transfer) else { return Vec::new() };
    let names = param_names(Target::Go, book, transfer);
    tr.params.iter().zip(names).filter(|(p, _)| p.unit.is_some()).map(|(_, n)| n).collect()
}

/// The parameters an operation takes, by their places among the transfer's: every one for `do` and
/// `hold`, the key's and the amounts (which may be left out) for `post`, the key's for `void`.
pub(crate) fn op_params(book: &BookFacts, transfer: &str, op: &str) -> Vec<(usize, bool)> {
    let Some(tr) = book.transfers.iter().find(|x| x.name == transfer) else { return Vec::new() };
    tr.params
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let key = tr.key.contains(&p.name);
            match op {
                "do" | "hold" => Some((i, false)),
                "post" if key => Some((i, false)),
                "post" if p.unit.is_some() => Some((i, true)),
                "void" if key => Some((i, false)),
                _ => None,
            }
        })
        .collect()
}

/// The operations of a book's transfers the modules build, each with the reasons it can be refused
/// with: those the checks across the languages found at every place that builds it (ritsu-cross's
/// X4, DESIGN 6.3), or, where one place is not decided and for `post` and `void`, every reason chobo's
/// check finds for the operation.
pub fn built(modules: &[Rc<Checked>]) -> BTreeMap<PathBuf, Vec<(TransferId, Vec<String>)>> {
    // each operation with the reasons found so far: None once a place that builds it is not decided
    type Found = Vec<(TransferId, Option<Vec<String>>)>;
    let mut out: BTreeMap<PathBuf, Found> = BTreeMap::new();
    let mut facts: BTreeMap<PathBuf, BookFacts> = BTreeMap::new();
    for c in modules {
        let p = Program::of(c);
        let Some(a) = c.analysis.get() else { continue };
        for f in &a.fns {
            for site in &f.sites {
                let SiteKind::Transfer { id, .. } = &site.kind else { continue };
                if let Some(b) = p.books.get(&id.book) {
                    facts.entry(id.book.clone()).or_insert_with(|| (**b).clone());
                }
                let reasons = if id.op == "do" || id.op == "hold" { f.refusals.iter().find(|r| r.span == site.span).and_then(|r| r.reasons.clone()) } else { None };
                let ops = out.entry(id.book.clone()).or_default();
                match ops.iter_mut().find(|(x, _)| x == id) {
                    Some((_, have)) => {
                        *have = match (have.take(), reasons) {
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
                    None => ops.push((id.clone(), reasons)),
                }
            }
        }
    }
    out.into_iter()
        .map(|(book, ops)| {
            let b = facts.get(&book);
            let ops = ops
                .into_iter()
                .map(|(id, reasons)| {
                    let all = || b.and_then(|b| b.transfers.iter().find(|t| t.name == id.transfer)).and_then(|t| t.refusals.iter().find(|(o, _)| *o == id.op)).map(|(_, rs)| rs.clone()).unwrap_or_default();
                    let reasons = reasons.unwrap_or_else(all);
                    (id, reasons)
                })
                .collect();
            (book, ops)
        })
        .collect()
}

/// The files of the types of one book's transfers, and of the function that applies them, under the
/// package's directory: `glue/books/<book>.ts` and `glue/books/<book>_apply.ts`, and the same in
/// Python and Go.
pub fn files(t: Target, book: &BookFacts, ops: &[(TransferId, Vec<String>)], opts: &Options) -> Vec<(String, String)> {
    let head = |c: ritsu_emit::header::Comment| c.line(&ritsu_emit::header::generated("kaname"));
    let name = &book.name;
    // where the files are, in the package: glue/books/, under the book's module's directory
    let here = format!("{}glue/books/", opts.place.dir);
    let units_used = ops.iter().any(|(id, _)| op_params(book, &id.transfer, &id.op).iter().any(|(i, _)| book.transfers.iter().find(|x| x.name == id.transfer).is_some_and(|x| x.params[*i].unit.as_ref().and_then(|u| crate::names::book_unit(book, u)).is_some_and(|u| u.type_name().is_some()))));
    let amount_type = |unit: &str| -> String {
        match crate::names::book_unit(book, unit).and_then(|u| u.type_name()) {
            Some(n) => match t {
                Target::Python => format!("_units.{n}"),
                _ => format!("units.{n}"),
            },
            None => match t {
                Target::TypeScript => "bigint".into(),
                Target::Python => "int".into(),
                Target::Go => "int64".into(),
            },
        }
    };
    let what_types = format!("The transfers of the book {name} that the package's modules of kaname build, each a value the host makes with the book's client (kaname's DESIGN 6.3): its arguments, and the reasons it can be refused with.");
    let what_apply = format!("Makes a transfer of the book {name} a module of kaname built, through chobo's client of the book: the client's answer, as it is.");
    match t {
        Target::TypeScript => {
            let mut types = format!("{}// {what_types}\n\n", head(ritsu_emit::header::Comment::Slashes));
            if units_used {
                types.push_str(&format!("import type * as units from \"{}\";\n\n", ritsu_emit::package::ts_path(&here, "units")));
            }
            let mut union = Vec::new();
            for (id, reasons) in ops {
                let ty = op_type(t, &id.transfer, &id.op);
                let tr = book.transfers.iter().find(|x| x.name == id.transfer);
                let args: Vec<String> = op_params(book, &id.transfer, &id.op)
                    .iter()
                    .map(|(i, optional)| {
                        let p = &tr.expect("built from the book").params[*i];
                        let ty = p.unit.as_ref().map(|u| amount_type(u)).unwrap_or_else(|| "string".into());
                        format!("readonly {}{}: {ty}", ritsu_emit::lit::json(&p.name), if *optional { "?" } else { "" })
                    })
                    .collect();
                let q = ritsu_emit::lit::json;
                types.push_str(&format!(
                    "/** {}.{}: {} */\nexport interface {ty} {{\n  readonly book: {};\n  readonly transfer: {};\n  readonly op: {};\n  readonly args: {{ {} }};\n}}\n",
                    id.transfer,
                    id.op,
                    if id.op == "do" || id.op == "hold" { "the reasons it can be refused with, at the amounts the modules build it with, are its type's `Refusal`" } else { "the reasons are chobo's" },
                    q(name),
                    q(&id.transfer),
                    q(&id.op),
                    args.join("; ")
                ));
                let rs = if reasons.is_empty() { "never".to_string() } else { reasons.iter().map(|r| q(r)).collect::<Vec<_>>().join(" | ") };
                types.push_str(&format!("/** The reasons {}.{} can be refused with. */\nexport type {ty}Refusal = {rs};\n\n", id.transfer, id.op));
                union.push(ty);
            }
            let call = format!("{}Call", ritsu_emit::ident::pascal(name));
            types.push_str(&format!("/** Every transfer of {name} the modules build. */\nexport type {call} = {};\n", union.join(" | ")));
            // the function that applies them
            let module = &book.typescript.module;
            let client = ritsu_emit::package::ts_path(&here, &format!("{}books/{module}", opts.place.dir));
            let mut apply = format!("{}// {what_apply}\n\nimport type {{ Book, Result }} from \"{client}\";\nimport type * as calls from \"./{name}\";\n\n", head(ritsu_emit::header::Comment::Slashes));
            let f = format!("apply{}", ritsu_emit::ident::pascal(name));
            apply.push_str(&format!("/** Makes `call` through the client `book`: what the client answers. */\nexport async function {f}(book: Book, call: calls.{call}): Promise<Result> {{\n"));
            for (id, _) in ops {
                let k = book.transfers.iter().position(|x| x.name == id.transfer).unwrap_or(0);
                let tr = &book.transfers[k];
                let member = book.typescript.transfers.get(k).map(|c| c.member.clone()).unwrap_or_else(|| id.transfer.clone());
                let key = |p: &str| format!("{}: call.args[{}]", ritsu_emit::lit::json(p), ritsu_emit::lit::json(p));
                let keys: Vec<String> = tr.key.iter().map(|p| key(p)).collect();
                let body = match id.op.as_str() {
                    "do" | "hold" => format!("book[{}].{}(call.args)", ritsu_emit::lit::json(&member), id.op),
                    "post" => {
                        let amounts: Vec<&String> = tr.params.iter().filter(|p| p.unit.is_some()).map(|p| &p.name).collect();
                        let first = amounts.first().map(|a| format!("call.args[{}]", ritsu_emit::lit::json(a))).unwrap_or_else(|| "undefined".into());
                        let am: Vec<String> = amounts.iter().map(|a| format!("{}: call.args[{}]!", ritsu_emit::lit::json(a), ritsu_emit::lit::json(a))).collect();
                        format!("book[{}].post({{ {} }}, {first} === undefined ? undefined : {{ {} }})", ritsu_emit::lit::json(&member), keys.join(", "), am.join(", "))
                    }
                    _ => format!("book[{}].void({{ {} }})", ritsu_emit::lit::json(&member), keys.join(", ")),
                };
                apply.push_str(&format!("  if (call.transfer === {} && call.op === {}) return {body};\n", ritsu_emit::lit::json(&id.transfer), ritsu_emit::lit::json(&id.op)));
            }
            apply.push_str("  throw new Error(\"kaname: no such transfer\");\n}\n");
            vec![(format!("glue/books/{name}.ts"), types), (format!("glue/books/{name}_apply.ts"), apply)]
        }
        Target::Python => {
            let mut types = format!("{}\"\"\"{what_types}\"\"\"\n\nfrom __future__ import annotations\n\nfrom dataclasses import dataclass\nfrom typing import ClassVar, Literal, NoReturn, Union\n\n", head(ritsu_emit::header::Comment::Hash));
            if units_used {
                types.push_str(&format!("from {} import units as _units\n", ritsu_emit::package::py_from(&here, "units").0));
            }
            types.push('\n');
            let names = |tr: &str| param_names(Target::Python, book, tr);
            let mut union = Vec::new();
            for (id, reasons) in ops {
                let ty = op_type(t, &id.transfer, &id.op);
                let tr = book.transfers.iter().find(|x| x.name == id.transfer).expect("built from the book");
                let pn = names(&id.transfer);
                types.push_str(&format!("\n@dataclass(frozen=True)\nclass {ty}:\n    \"\"\"{}.{} of the book {name}.\"\"\"\n\n", id.transfer, id.op));
                types.push_str(&format!("    book: ClassVar[str] = {}\n    transfer: ClassVar[str] = {}\n    op: ClassVar[str] = {}\n", ritsu_emit::lit::json(name), ritsu_emit::lit::json(&id.transfer), ritsu_emit::lit::json(&id.op)));
                for (i, optional) in op_params(book, &id.transfer, &id.op) {
                    let p = &tr.params[i];
                    let pty = p.unit.as_ref().map(|u| amount_type(u)).unwrap_or_else(|| "str".into());
                    if optional {
                        types.push_str(&format!("    {}: {pty} | None = None\n", pn[i]));
                    } else {
                        types.push_str(&format!("    {}: {pty}\n", pn[i]));
                    }
                }
                let rs = if reasons.is_empty() { "NoReturn".to_string() } else { format!("Literal[{}]", reasons.iter().map(|r| ritsu_emit::lit::json(r)).collect::<Vec<_>>().join(", ")) };
                types.push_str(&format!("\n\n# The reasons {}.{} can be refused with.\n{ty}Refusal = {rs}\n\n", id.transfer, id.op));
                union.push(ty);
            }
            let call = format!("{}Call", ritsu_emit::ident::pascal(name));
            types.push_str(&format!("\n# Every transfer of {name} the modules build.\n{call} = Union[{}]\n", union.join(", ")));
            // the names of typing the types use: no reason to refuse with is NoReturn
            let typing: Vec<&str> = ["ClassVar", "Literal", "NoReturn", "Union"].into_iter().filter(|w| *w == "ClassVar" || *w == "Union" || types.contains(&format!(" = {w}"))).collect();
            let types = types.replacen("from typing import ClassVar, Literal, NoReturn, Union\n", &format!("from typing import {}\n", typing.join(", ")), 1);
            let (client, module) = ritsu_emit::package::py_from(&here, &format!("{}books/{}", opts.place.dir, book.python.module));
            let mut apply = format!(
                "{}\"\"\"{what_apply}\"\"\"\n\nfrom __future__ import annotations\n\nfrom {client} import {module} as _client\nfrom . import {name} as _calls\n\n\ndef apply_{name}(book: _client.Book, call: _calls.{call}) -> _client.Result:\n    \"\"\"Makes `call` through the client `book`: what the client answers.\"\"\"\n",
                head(ritsu_emit::header::Comment::Hash)
            );
            for (id, _) in ops {
                let k = book.transfers.iter().position(|x| x.name == id.transfer).unwrap_or(0);
                let tr = &book.transfers[k];
                let member = book.python.transfers.get(k).map(|c| c.member.clone()).unwrap_or_else(|| id.transfer.clone());
                let pn = names(&id.transfer);
                let kw: Vec<String> = op_params(book, &id.transfer, &id.op).iter().map(|(i, _)| format!("{}=call.{}", pn[*i], pn[*i])).collect();
                let _ = tr;
                apply.push_str(&format!("    if isinstance(call, _calls.{}):\n        return book.{member}.{}({})\n", op_type(t, &id.transfer, &id.op), id.op, kw.join(", ")));
            }
            apply.push_str("    raise ValueError(\"kaname: no such transfer\")\n");
            vec![(format!("glue/books/{name}.py"), types), (format!("glue/books/{name}_apply.py"), apply)]
        }
        Target::Go => {
            let pkg = &book.go.module;
            let mut body: Vec<String> = Vec::new();
            let names = |tr: &str| param_names(Target::Go, book, tr);
            let mut union = Vec::new();
            for (id, reasons) in ops {
                let ty = op_type(t, &id.transfer, &id.op);
                let tr = book.transfers.iter().find(|x| x.name == id.transfer).expect("built from the book");
                let pn = names(&id.transfer);
                let rows: Vec<(String, String)> = op_params(book, &id.transfer, &id.op)
                    .iter()
                    .map(|(i, optional)| {
                        let p = &tr.params[*i];
                        let pty = p.unit.as_ref().map(|u| amount_type(u)).unwrap_or_else(|| "string".into());
                        (pn[*i].clone(), if *optional { format!("*{pty}") } else { pty })
                    })
                    .collect();
                let w = rows.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(0);
                let fields: String = rows.iter().map(|(n, ty)| format!("\t{n}{}{ty}\n", " ".repeat(w - n.chars().count() + 1))).collect();
                body.push(format!("// {ty} is {}.{} of the book {name}.\ntype {ty} struct {{\n{fields}}}\n", id.transfer, id.op));
                body.push(format!("func ({ty}) kanameCall() {{}}\n"));
                // the reasons
                let rt = format!("{ty}Refusal");
                let mut consts = format!("// {rt} is a reason {}.{} can be refused with.\ntype {rt} string\n", id.transfer, id.op);
                if !reasons.is_empty() {
                    let cn: Vec<String> = reasons.iter().map(|r| format!("{rt}{}", ritsu_emit::ident::go_exported(&ritsu_emit::ident::pascal(r)))).collect();
                    let w = cn.iter().map(|n| n.chars().count()).max().unwrap_or(0);
                    consts.push_str("\nconst (\n");
                    for (n, r) in cn.iter().zip(reasons) {
                        consts.push_str(&format!("\t{n}{}{rt} = {}\n", " ".repeat(w - n.chars().count() + 1), ritsu_emit::lit::go(r)));
                    }
                    consts.push_str(")\n");
                }
                body.push(consts);
                union.push(ty);
            }
            let head_go = head(ritsu_emit::header::Comment::Slashes);
            let imports = if units_used { format!("import \"{}/units\"\n\n", opts.go_module) } else { String::new() };
            let types = format!(
                "{head_go}\n// Package {pkg}: {what_types}\npackage {pkg}\n\n{imports}// Call is a transfer of {name} the modules build: one of {}.\ntype Call interface{{ kanameCall() }}\n\n{}",
                union.join(", "),
                body.join("\n")
            );
            let apkg = format!("{pkg}apply");
            let mut apply = format!(
                "{head_go}\n// Package {apkg}: {what_apply}\npackage {apkg}\n\nimport (\n\t\"context\"\n\t\"fmt\"\n\n\tclient \"{client}\"\n\tcalls \"{calls}\"\n)\n\n// Apply makes call through the client book: what the client answers.\nfunc Apply(ctx context.Context, book *client.Book, call calls.Call) (client.Result, error) {{\n\tswitch c := call.(type) {{\n",
                client = ritsu_emit::package::go_path(&opts.go_module, &format!("{}books/{pkg}", opts.place.dir)),
                calls = ritsu_emit::package::go_path(&opts.go_module, &format!("{}glue/books/{pkg}", opts.place.dir)),
            );
            for (id, _) in ops {
                let k = book.transfers.iter().position(|x| x.name == id.transfer).unwrap_or(0);
                let tr = &book.transfers[k];
                let member = book.go.transfers.get(k).map(|c| c.member.clone()).unwrap_or_default();
                let pn = names(&id.transfer);
                let g = ritsu_emit::ident::go_exported(&id.transfer);
                let lit = |which: &[usize], deref: bool| -> String {
                    which.iter().map(|i| format!("{}: {}c.{}", pn[*i], if deref { "*" } else { "" }, pn[*i])).collect::<Vec<_>>().join(", ")
                };
                let all: Vec<usize> = (0..tr.params.len()).collect();
                let keys: Vec<usize> = tr.params.iter().enumerate().filter(|(_, p)| tr.key.contains(&p.name)).map(|(i, _)| i).collect();
                let amounts: Vec<usize> = tr.params.iter().enumerate().filter(|(_, p)| p.unit.is_some()).map(|(i, _)| i).collect();
                let call = match id.op.as_str() {
                    "do" => format!("return book.{member}.Do(ctx, client.{g}Args{{{}}})", lit(&all, false)),
                    "hold" => format!("return book.{member}.Hold(ctx, client.{g}Args{{{}}})", lit(&all, false)),
                    "post" => {
                        let first = amounts.first().map(|i| pn[*i].clone()).unwrap_or_default();
                        format!(
                            "if c.{first} == nil {{\n\t\t\treturn book.{member}.Post(ctx, client.{g}Key{{{}}}, nil)\n\t\t}}\n\t\treturn book.{member}.Post(ctx, client.{g}Key{{{}}}, &client.{g}Amounts{{{}}})",
                            lit(&keys, false),
                            lit(&keys, false),
                            lit(&amounts, true)
                        )
                    }
                    _ => format!("return book.{member}.Void(ctx, client.{g}Key{{{}}})", lit(&keys, false)),
                };
                apply.push_str(&format!("\tcase calls.{}:\n\t\t{call}\n", op_type(t, &id.transfer, &id.op)));
            }
            apply.push_str("\t}\n\treturn client.Result{}, fmt.Errorf(\"kaname: no such transfer %T\", call)\n}\n");
            vec![(format!("glue/books/{pkg}/{pkg}.go"), types), (format!("glue/books/{apkg}/{apkg}.go"), apply)]
        }
    }
}
