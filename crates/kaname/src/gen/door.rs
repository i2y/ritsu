//! Rust's `glue::door::answer` (DESIGN 36.11): a call of a function a host calls, written as a line of
//! JSON (`{"module": …, "function": …, "in": {…}}`, each name as the module writes it), answered with
//! the line `kaname vectors` writes for that input without its `why` (`{"function":…,"in":…,"out":…}`,
//! or `"fails"` for a `KanameError`), or a refusal (`{"refused":"input","error":…}`) where the input is
//! no such call. The package's Wasm answers its `call` through it (`answer`, and `not_utf8` for bytes
//! that are not UTF-8; DESIGN 36.11), and the native harness of the tests calls it as it is (DESIGN
//! 36.9), so that both go through one reading and one writing.
//!
//! The values on the wire are those of `vectors` (DESIGN 9). The door reads them as any encoder of
//! JSON writes them (escapes, white space, `20.0` and `2e1` for 20, the later of two keys) and writes
//! `in` again from what it read, as `vectors` writes it. A string written with an escape that is no
//! Unicode scalar value (`\ud800`), which a value of Rust cannot hold, is answered with the entry's
//! failure of a string that is not well formed (DESIGN 25.8), before the ranges are looked at.

use super::harness::{Need, needs, one_key_of, one_of, sum_need};
use super::plan::{Boxed, Gen, check_name, field_name, fn_name};
use super::{Options, Target};
use crate::check::Checked;
use crate::ty::{Owner, RecId, Ty};
use crate::value::Program;
use ritsu_base::json::Json;
use ritsu_base::text::Lang;
use ritsu_emit::package::Place;
use ritsu_ports::ColumnType;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// How deep the door reads JSON at the least: deeper than any value a host writes by hand, and far
/// short of what would use up the stack of a Wasm (DESIGN 36.11).
pub const DEPTH: usize = 1024;

/// The file `glue/door.rs` of a package of Rust with these modules (each with its place in the
/// package), its sentences in `lang`.
pub fn door(modules: &[(Rc<Checked>, String, Place)], lang: Lang) -> String {
    let head = ritsu_emit::header::Comment::Slashes.line(&ritsu_emit::header::generated("kaname"));
    let callable: Vec<&(Rc<Checked>, String, Place)> = modules.iter().filter(|(c, _, _)| c.decls.fns.iter().any(|f| !f.inner())).collect();
    let mut depth = DEPTH;
    let mut subs = String::new();
    let mut arms = String::new();
    for (i, (c, shown, place)) in callable.iter().enumerate() {
        let opts = Options { shown: shown.clone(), lang, go_module: String::new(), place: place.clone(), store: super::Store::default() };
        let (text, d) = module_door(c, &opts, i);
        depth = depth.max(d);
        subs.push_str(&text);
        arms.push_str(&format!("        {} => m{i}::answer(function, inn),\n", ritsu_emit::lit::rust(&module_key(c, place))));
    }
    let one = match callable.as_slice() {
        [(c, _, place)] => format!("None => {},", ritsu_emit::lit::rust(&module_key(c, place))),
        _ => "None => return refusal(&missing_key(\"module\")),".to_string(),
    };
    // a package whose modules have no function a host calls (each takes a function, or is generic):
    // what a call names is read as in any package, and no module answers it
    let dispatch = if arms.is_empty() {
        "    let _ = (function, inn);\n    refusal(&no_module(module))\n".to_string()
    } else {
        format!("    let answered = match module {{\n{arms}        _ => return refusal(&no_module(module)),\n    }};\n    match answered {{\n        Some(line) => line,\n        None => refusal(&no_function(module, function)),\n    }}\n")
    };
    let s = said(lang);
    let mut o = String::new();
    o.push_str(&head);
    o.push_str(&format!(
        "//! {}\n\n",
        match lang {
            Lang::Ja => "パッケージの kaname のモジュールの入口：`answer` は、JSON の一行に書いた呼び出しを読み、ホストが呼ぶ関数を呼んで、kaname vectors がその入力に書く行（`why` を除く）で答える。",
            Lang::En => "The door of the package's modules of kaname: `answer` reads a call written as a line of JSON, calls the function a host calls, and answers the line kaname vectors writes for that input, without its `why`.",
        }
    ));
    // the reader of JSON and the sentences are the same in every package, and a package's calls
    // read a part of them (a package of no dates reads no date)
    o.push_str("#![allow(dead_code, clippy::all)]\n\n");
    o.push_str(&format!("/// How deep the door reads the JSON of a call.\nconst DEPTH: usize = {depth};\n\n"));
    o.push_str(READER);
    o.push_str(&sentences(&s, lang));
    o.push_str(&format!(
        "/// The answer to a call written as a line of JSON: the line kaname vectors writes for that input,
/// without its `why`, or a refusal where the input is no call of a function a host calls.
pub fn answer(input: &str) -> String {{
    let j = match read(input) {{
        Ok(j) => j,
        Err(e) => return refusal(&e),
    }};
    let Json::Obj(top) = &j else {{
        return refusal(NOT_OBJECT);
    }};
    let module: &str = match get(top, \"module\") {{
        Some(Json::Str(s, false)) => s,
        Some(_) => return refusal(&not_string_key(\"module\")),
        {one}
    }};
    let function: &str = match get(top, \"function\") {{
        Some(Json::Str(s, false)) => s,
        Some(_) => return refusal(&not_string_key(\"function\")),
        None => return refusal(&missing_key(\"function\")),
    }};
    let inn: &[(::std::string::String, Json)] = match get(top, \"in\") {{
        Some(Json::Obj(fs)) => fs,
        Some(_) => return refusal(&not_object_key(\"in\")),
        None => return refusal(&missing_key(\"in\")),
    }};
{dispatch}}}

"
    ));
    o.push_str(&subs);
    super::module::tidy(Target::Rust, &o)
}

/// The name a call gives a module by: as the module writes it, and a dependency's after the names of
/// the dependencies on the way to it (`@pricing/fees`, ritsu's DESIGN 17.8).
pub fn module_key(c: &Checked, place: &Place) -> String {
    let mut key = String::new();
    for d in ritsu_emit::package::chain(&place.dir) {
        key.push('@');
        key.push_str(d);
        key.push('/');
    }
    key.push_str(&c.module.name.text);
    key
}

/// The JSON depth of the deepest call of a function of the module the entry reads to its end: a value
/// of a type that holds itself one past the depth a host may give (DESIGN 28.1), in the object of the
/// call and of its `in`.
fn deepest(p: &Program, c: &Checked) -> usize {
    let mut most = 0;
    for f in c.decls.fns.iter().filter(|f| !f.inner()) {
        for prm in &f.params {
            if let Some(v) = crate::vectors::nested_value(p, &prm.slot.ty, crate::interp::NESTED + 1) {
                most = most.max(json_depth(&p.to_json(&v, &prm.slot.ty)) + 2);
            }
        }
    }
    most
}

fn json_depth(j: &Json) -> usize {
    match j {
        Json::Arr(xs) => 1 + xs.iter().map(json_depth).max().unwrap_or(0),
        Json::Obj(fs) => 1 + fs.iter().map(|(_, v)| json_depth(v)).max().unwrap_or(0),
        _ => 0,
    }
}

/// The part of the door of one module: a module of Rust (`m<i>`) that reads the module's types as its
/// code names them, with a function for each function a host calls and the codecs of their types;
/// and the depth its calls ask of the reader.
fn module_door(c: &Rc<Checked>, opts: &Options, i: usize) -> (String, usize) {
    let g = Gen::new(c, Target::Rust, opts);
    let mut d = Door { g: &g, codecs: RefCell::new(BTreeMap::new()), defs: RefCell::new(Vec::new()) };
    let path = ritsu_emit::package::rust_path(&format!("{}glue/{}", opts.place.dir, c.module.name.ascii()));
    let mut fns = String::new();
    let mut arms = String::new();
    for (fi, info) in c.decls.fns.iter().enumerate() {
        if info.inner() {
            continue;
        }
        let f = format!("f{fi}");
        arms.push_str(&format!("            {} => {f}(inn),\n", ritsu_emit::lit::rust(&info.name.text)));
        fns.push_str(&d.function(&path, fi, &f));
    }
    let body = format!(
        "    /// The answer to a call of a function of the module, by its name; None where it has no such function
    /// a host calls.
    pub fn answer(function: &str, inn: &[(::std::string::String, super::Json)]) -> Option<::std::string::String> {{
        Some(match function {{
{arms}            _ => return None,
        }})
    }}

{fns}{}",
        d.defs.get_mut().join("")
    );
    // the module's types, which the codecs name bare (a module whose calls take and give none of
    // them reads none), and the other modules the body names
    let mut uses = vec![format!("#[allow(unused_imports)]\n    use {path}::*;")];
    uses.extend(super::module::rust_uses(&g, &body).into_iter().filter(|u| named_in(u, &body)));
    let text = format!("/// The calls of the module {}.\nmod m{i} {{\n{}\n\n{body}}}\n\n", c.module.name.text, uses.iter().map(|u| format!("    {u}")).collect::<Vec<_>>().join("\n"));
    (text, deepest(&g.p, c))
}

/// The codecs of one module's part of the door, each made once by its type.
struct Door<'g, 'a> {
    g: &'g Gen<'a>,
    codecs: RefCell<BTreeMap<String, String>>,
    defs: RefCell<Vec<String>>,
}

impl Door<'_, '_> {
    /// The function of the door for the function `fi` of the module at `path`, named `name`: it reads
    /// the arguments, writes `in` again from them, asks the module's check of the entry, calls the
    /// function and writes its answer.
    fn function(&mut self, path: &str, fi: usize, name: &str) -> String {
        let g = self.g;
        let c = &g.c;
        let info = &c.decls.fns[fi];
        let fun = ritsu_emit::lit::rust(&info.name.text);
        let module = ritsu_emit::lit::rust(&c.module.name.text);
        let fails = c.analysis.get().and_then(|a| a.fns.get(fi)).is_some_and(|x| !x.effect.fails.is_empty());
        let inn = if info.params.is_empty() { "_inn" } else { "inn" };
        let mut s = format!("    fn {name}({inn}: &[(::std::string::String, super::Json)]) -> ::std::string::String {{\n        let mut cx = super::Cx {{ fun: {fun}, bad: None }};\n");
        let mut args = Vec::new();
        let mut inline = String::from("        let mut given = ::std::string::String::from(\"{\");\n");
        for (k, p) in info.params.iter().enumerate() {
            let ty = g.boundary(&p.slot.ty);
            let dec = self.decoder(&p.slot.ty);
            let enc = self.encoder(&p.slot.ty);
            let wire = ritsu_emit::lit::rust(&p.name.text);
            s.push_str(&format!(
                "        let a{k}: {ty} = match {dec}(super::arg(inn, {wire}), {wire}, &mut cx) {{\n            Ok(v) => v,\n            Err(e) => return super::refused(&e),\n        }};\n"
            ));
            let key = ritsu_emit::lit::rust(&format!("{}{}:", if k > 0 { "," } else { "" }, ritsu_base::json::quote(&p.name.text)));
            inline.push_str(&format!("        given.push_str({key});\n        {enc}(&a{k}, &mut given);\n"));
            args.push(lend(g, &p.slot.ty, &format!("a{k}")));
        }
        inline.push_str("        given.push('}');\n");
        s.push_str(&inline);
        s.push_str(&format!("        if let Some((at, message)) = cx.bad.take() {{\n            return super::bad_line({fun}, &given, {module}, {}, &at, &message);\n        }}\n", info.line));
        let args = args.join(", ");
        let check = check_name(Target::Rust, c, fi);
        let f = fn_name(Target::Rust, c, fi);
        s.push_str(&format!("        if let Err(e) = {path}::{check}({args}) {{\n            return super::failed({fun}, &given, &e);\n        }}\n"));
        if fails {
            s.push_str(&format!("        let r = match {path}::{f}({args}) {{\n            Ok(r) => r,\n            Err(e) => return super::failed({fun}, &given, &e),\n        }};\n"));
        } else {
            s.push_str(&format!("        let r = {path}::{f}({args});\n"));
        }
        let enc = self.encoder(&info.ret.ty);
        s.push_str(&format!("        let mut out = ::std::string::String::new();\n        {enc}(&r, &mut out);\n        super::out_line({fun}, &given, &out)\n    }}\n\n"));
        s
    }

    /// A codec's function, made once by its key.
    fn codec(&self, key: String, make: impl FnOnce(&Self, &str) -> String) -> String {
        if let Some(n) = self.codecs.borrow().get(&key) {
            return n.clone();
        }
        let n = format!("{}{}", if key.starts_with("dec|") { "d" } else { "e" }, self.codecs.borrow().len() + 1);
        self.codecs.borrow_mut().insert(key, n.clone());
        let def = make(self, &n);
        self.defs.borrow_mut().push(def);
        n
    }

    // ── From the wire ────────────────────────────────────────────────────

    /// The function that reads a value of `t` off the wire: `fn d3(j: &Json, path: &str, cx: &mut Cx) ->
    /// Read<T>`, a refusal (the place and the sentence) where the JSON is no value of it.
    fn decoder(&self, t: &Ty) -> String {
        self.codec(format!("dec|{t:?}"), |d, n| d.decoder_def(t, n))
    }

    fn decoder_def(&self, t: &Ty, n: &str) -> String {
        let g = self.g;
        let ty = g.boundary(t);
        let body = match t {
            Ty::Num(nt) if !nt.is_rate() && nt.unit.type_name().is_some() => format!("Ok({}(super::int(j, path, cx)?))", g.unit_type(&nt.unit)),
            Ty::Num(_) => "super::int(j, path, cx)".to_string(),
            Ty::Bool => "super::boolean(j, path, cx)".to_string(),
            Ty::Str => "super::string_of(j, path, cx)".to_string(),
            Ty::Date => "super::date(j, path, cx)".to_string(),
            Ty::Opt(x) => {
                let dx = self.decoder(x);
                format!("match j {{\n    super::Json::Null | super::Json::Absent => Ok(None),\n    j => {dx}(j, path, cx).map(Some),\n}}")
            }
            Ty::List(x) => {
                let dx = self.decoder(x);
                format!("let xs = super::array(j, path, cx)?;\nlet mut out: {ty} = <{ty}>::new();\nfor (i, x) in xs.iter().enumerate() {{\n    out.push({dx}(x, &format!(\"{{path}}[{{i}}]\"), cx)?);\n}}\nOk(out)")
            }
            Ty::Dict(k, v) => {
                let (dk, dv) = (self.decoder(k), self.decoder(v));
                format!(
                    "let xs = super::array(j, path, cx)?;\nlet mut out: {ty} = <{ty}>::new();\nfor (i, x) in xs.iter().enumerate() {{\n    let at = format!(\"{{path}}[{{i}}]\");\n    let (a, b) = super::pair(x, &at, cx)?;\n    let key = {dk}(a, &format!(\"{{at}}[0]\"), cx)?;\n    let value = {dv}(b, &format!(\"{{at}}[1]\"), cx)?;\n    out.insert(key, value);\n}}\nOk(out)"
                )
            }
            Ty::Set(x) => {
                let dx = self.decoder(x);
                format!("let xs = super::array(j, path, cx)?;\nlet mut out: {ty} = <{ty}>::new();\nfor (i, x) in xs.iter().enumerate() {{\n    out.insert({dx}(x, &format!(\"{{path}}[{{i}}]\"), cx)?);\n}}\nOk(out)")
            }
            // a result from its one pair, `ok` or `err` (DESIGN 26.4)
            Ty::Result(a, e) => {
                let (da, de) = (self.decoder(a), self.decoder(e));
                let names = ["ok".to_string(), "err".to_string()];
                let (need_key, need) = (ritsu_emit::lit::rust(&one_key_of(g.lang, &names)), ritsu_emit::lit::rust(&sum_need(g.lang, &[], &names)));
                format!(
                    "match j {{\n    super::Json::Obj(fs) => match super::one_key(fs) {{\n        Some((\"ok\", x)) => Ok(Ok({da}(x, &format!(\"{{path}}.ok\"), cx)?)),\n        Some((\"err\", x)) => Ok(Err({de}(x, &format!(\"{{path}}.err\"), cx)?)),\n        _ => Err(super::keyed(j, path, cx, {need_key})),\n    }},\n    _ => Err(super::wrong(j, path, cx, {need})),\n}}"
                )
            }
            Ty::Enum(id) if g.p.is_sum(id) => self.sum_decoder(id),
            // a value of an enum by its name, one of the names it declares
            Ty::Enum(id) => {
                let mut arms = String::new();
                let names = g.enum_values(id);
                for v in &names {
                    arms.push_str(&format!("        {} => Ok({}),\n", ritsu_emit::lit::rust(v), g.member(id, v).print(Target::Rust)));
                }
                let need = ritsu_emit::lit::rust(&one_of(g.lang, &names));
                format!("match j {{\n    super::Json::Str(s, _) => match s.as_str() {{\n{arms}        _ => Err(super::wrong(j, path, cx, {need})),\n    }},\n    _ => Err(super::wrong(j, path, cx, {need})),\n}}")
            }
            Ty::Record(r @ RecId::Variant { value, .. }) => {
                let Some(id) = r.sum() else { return String::new() };
                let ds = self.decoder(&Ty::Enum(id.clone()));
                let w = g.variant_of(&id, value);
                let need = ritsu_emit::lit::rust(&one_key_of(g.lang, std::slice::from_ref(value)));
                format!("match {ds}(j, path, cx)? {{\n    {w}(x) => Ok(x),\n    _ => Err(super::keyed(j, path, cx, {need})),\n}}")
            }
            Ty::Record(rid @ RecId::Module { .. }) => {
                let holder = t.clone();
                let fields = self.fields_read(&holder, &g.p.fields(rid), "");
                format!("let fs = super::object(j, path, cx)?;\nOk({} {{\n{fields}}})", g.record_type_bare(rid))
            }
            Ty::Record(RecId::Entry(k, v)) => {
                let (dk, dv) = (self.decoder(k), self.decoder(v));
                let k = g.imp(super::plan::Import::Kaname);
                format!(
                    "let fs = super::object(j, path, cx)?;\nOk({k}::Entry {{\n    key: {dk}(super::field(fs, \"key\"), &format!(\"{{path}}.key\"), cx)?,\n    value: {dv}(super::field(fs, \"value\"), &format!(\"{{path}}.value\"), cx)?,\n}})"
                )
            }
            Ty::Record(RecId::Indexed(x)) => {
                let dx = self.decoder(x);
                let k = g.imp(super::plan::Import::Kaname);
                format!(
                    "let fs = super::object(j, path, cx)?;\nOk({k}::Indexed {{\n    index: super::int(super::field(fs, \"index\"), &format!(\"{{path}}.index\"), cx)?,\n    item: {dx}(super::field(fs, \"item\"), &format!(\"{{path}}.item\"), cx)?,\n}})"
                )
            }
            // a running total a host gives (a parameter of `running[T, W]`): the item, and the three
            // weights, in the order declared
            Ty::Record(RecId::Running(item, w)) => {
                let (di, dw) = (self.decoder(item), self.decoder(w));
                let k = g.imp(super::plan::Import::Kaname);
                let weight = |f: &str| format!("    {f}: {dw}(super::field(fs, \"{f}\"), &format!(\"{{path}}.{f}\"), cx)?,\n");
                format!(
                    "let fs = super::object(j, path, cx)?;\nOk({k}::Running {{\n    item: {di}(super::field(fs, \"item\"), &format!(\"{{path}}.item\"), cx)?,\n{}{}{}}})",
                    weight("before"),
                    weight("upto"),
                    weight("total")
                )
            }
            // an element of a list a rule walks (DESIGN 4.4): each field read in its order as kaname's
            // types hold it, then put as the rule's code holds it (a date as its count of days, a rate
            // in the rule's type of it), as kaname's code puts it where it calls the rule. A day of no
            // calendar (2026-13-01, 10000-01-01) has no count of days: the door fails `input` on it, as
            // the check of a date says one outside its range
            Ty::Record(RecId::Element(file, seq)) => {
                let Some(facts) = g.p.rules.get(file) else { return String::new() };
                let Some((_, _, cols)) = facts.elements.iter().find(|(x, _, _)| x == seq) else { return String::new() };
                let rule = g.imp(super::plan::Import::Rule(file.clone()));
                let fields = g.p.fields(&RecId::Element(file.clone(), seq.clone()));
                let mut lines = String::new();
                let mut parts = String::new();
                for (k, ((f, slot), col)) in fields.iter().zip(cols).enumerate() {
                    let d = self.decoder(&slot.ty);
                    let wire = ritsu_emit::lit::rust(&f.text);
                    let at = format!("&format!(\"{{path}}{}\")", rs_fmt_text(&format!(".{}", f.text)));
                    lines.push_str(&format!("let f{} = {d}(super::field(fs, {wire}), {at}, cx)?;\n", k + 1));
                    let mut inner = &col.ty;
                    while let ritsu_ports::ColumnType::Opt(x) = inner {
                        inner = x;
                    }
                    let rate = matches!(inner, ritsu_ports::ColumnType::Num { unit: Some(u), .. } if u.dim == ritsu_units::Dim::Rate);
                    let date = matches!(inner, ritsu_ports::ColumnType::Date { .. });
                    if date {
                        let rt = ritsu_emit::lit::rust(&super::harness::day_range(slot.range.as_ref()));
                        lines.push_str(&if matches!(col.ty, ritsu_ports::ColumnType::Opt(_)) {
                            format!("if let Some(d) = &f{} {{\n    super::day_held(d, {at}, {rt}, cx);\n}}\n", k + 1)
                        } else {
                            format!("super::day_held(&f{}, {at}, {rt}, cx);\n", k + 1)
                        });
                    }
                    let conv = |x: &str| if date { format!("{x}.days()") } else { format!("{rule}::Rate({x})") };
                    let v = format!("f{}", k + 1);
                    let put = match (date || rate, matches!(col.ty, ritsu_ports::ColumnType::Opt(_))) {
                        (false, _) => v,
                        (true, false) => conv(&v),
                        (true, true) if date => format!("{v}.map(|x| {})", conv("x")),
                        // the rate's type itself, which clippy holds a closure that only calls it to
                        (true, true) => format!("{v}.map({rule}::Rate)"),
                    };
                    parts.push_str(&format!("    {}: {put},\n", ritsu_emit::ident::rust_name(&col.alias)));
                }
                format!("let fs = super::object(j, path, cx)?;\n{lines}Ok({ty} {{\n{parts}}})")
            }
            // a value no host gives (a rule's outputs, a transfer: a function a host calls takes none,
            // E116): no call reads one
            _ => "let _ = j;\nErr(super::not_taken(path, cx))".to_string(),
        };
        format!("    fn {n}(j: &super::Json, path: &str, cx: &mut super::Cx) -> super::Read<{ty}> {{\n        {}\n    }}\n\n", indent(&body, 8))
    }

    /// The fields of a value of `holder` read off the object `fs` in the order they are declared (the
    /// order the check reads them in), each in its box; a field that may be none may be left out.
    fn fields_read(&self, holder: &Ty, fields: &[(crate::ast::Name, crate::ty::Slot)], tail: &str) -> String {
        let g = self.g;
        let mut out = String::new();
        for (f, slot) in fields {
            let df = self.decoder(&slot.ty);
            let wire = ritsu_emit::lit::rust(&f.text);
            // a key the object leaves out is read as absent: none, for a field that may be none
            let at = format!("super::field(fs, {wire})");
            let read = super::ex::Ex::atom(format!("{df}({at}, &format!(\"{{path}}{}\"), cx)?", rs_fmt_text(&format!("{tail}.{}", f.text))));
            let x = g.boxed_value(holder, &slot.ty, read);
            out.push_str(&format!("    {}: {},\n", field_name(Target::Rust, f.ascii()), x.print(Target::Rust)));
        }
        out
    }

    /// A sum off the wire (DESIGN 23.6): a value with no fields by its name, a value with fields as an
    /// object of one pair, its name and its fields (a value with no fields that way too).
    fn sum_decoder(&self, id: &crate::ty::EnumId) -> String {
        let g = self.g;
        let Owner::Module(file) = &id.owner else { return String::new() };
        let mut plain = String::new();
        let mut with = String::new();
        let mut names = Vec::new();
        let all = g.enum_values(id);
        for v in &all {
            let r = RecId::Variant { file: file.clone(), name: id.name.clone(), value: v.clone(), args: id.args.clone() };
            let holder = Ty::Record(r.clone());
            let fs = g.p.value_fields(id, v);
            let ctor = g.variant_of(id, v);
            let vt = g.record_type_bare(&r);
            let lit = ritsu_emit::lit::rust(v);
            if fs.is_empty() {
                names.push(v.clone());
                plain.push_str(&format!("        {lit} => Ok({ctor}({vt} {{}})),\n"));
                with.push_str(&format!("        Some(({lit}, _)) => Ok({ctor}({vt} {{}})),\n"));
                continue;
            }
            let fields = indent_lines(&self.fields_read(&holder, &fs, &format!(".{v}")), 12);
            with.push_str(&format!(
                "        Some(({lit}, x)) => {{\n            let fs = super::object(x, &format!(\"{{path}}.{}\"), cx)?;\n            Ok({ctor}({vt} {{\n{fields}            }}))\n        }}\n",
                rs_fmt_text(v)
            ));
        }
        let lit = |s: String| ritsu_emit::lit::rust(&s);
        // what the place must be, every name listed, said of a string that is no name of a value of
        // no fields too, as the harness says it; a sum whose every value has fields is no name alone
        let (need_key, need) = (lit(one_key_of(g.lang, &all)), lit(sum_need(g.lang, &names, &all)));
        let plain = if plain.is_empty() { String::new() } else { format!("    super::Json::Str(s, _) => match s.as_str() {{\n{plain}        _ => Err(super::wrong(j, path, cx, {need})),\n    }},\n") };
        format!("match j {{\n{plain}    super::Json::Obj(one) => match super::one_key(one) {{\n{with}        _ => Err(super::keyed(j, path, cx, {need_key})),\n    }},\n    _ => Err(super::wrong(j, path, cx, {need})),\n}}")
    }

    // ── To the wire ──────────────────────────────────────────────────────

    /// The function that writes a value of `t` on the wire: `fn e3(v: &T, o: &mut String)`.
    fn encoder(&self, t: &Ty) -> String {
        self.codec(format!("enc|{t:?}"), |d, n| d.encoder_def(t, n))
    }

    fn encoder_def(&self, t: &Ty, n: &str) -> String {
        let g = self.g;
        // what the encoder borrows: a `str` and a slice, which a `String` and a `Vec` lend (a rule's
        // answer of one output that is a string is that string)
        let ty = match t {
            Ty::Str => "str".to_string(),
            Ty::List(x) => format!("[{}]", g.boundary(x)),
            Ty::Record(RecId::Outputs(f)) if g.p.rules.get(f).is_some_and(|r| matches!(r.outputs.as_slice(), [one] if matches!(one.ty, ColumnType::Str))) => "str".to_string(),
            _ => g.boundary(t),
        };
        let body = match t {
            Ty::Num(nt) if !nt.is_rate() && nt.unit.type_name().is_some() => "super::int_out(v.0, o);".to_string(),
            Ty::Num(_) => "super::int_out(*v, o);".to_string(),
            Ty::Bool => "o.push_str(if *v { \"true\" } else { \"false\" });".to_string(),
            Ty::Str => "super::str_out(v, o);".to_string(),
            Ty::Date => "super::str_out(&v.to_string(), o);".to_string(),
            Ty::Opt(x) => {
                let ex = self.encoder(x);
                format!("match v {{\n    Some(x) => {ex}(x, o),\n    None => o.push_str(\"null\"),\n}}")
            }
            Ty::List(x) | Ty::Set(x) => {
                let ex = self.encoder(x);
                format!("o.push('[');\nfor (i, x) in v.iter().enumerate() {{\n    if i > 0 {{\n        o.push(',');\n    }}\n    {ex}(x, o);\n}}\no.push(']');")
            }
            Ty::Dict(k, x) => {
                let (ek, ex) = (self.encoder(k), self.encoder(x));
                format!("o.push('[');\nfor (i, (k, x)) in v.iter().enumerate() {{\n    if i > 0 {{\n        o.push(',');\n    }}\n    o.push('[');\n    {ek}(k, o);\n    o.push(',');\n    {ex}(x, o);\n    o.push(']');\n}}\no.push(']');")
            }
            Ty::Result(a, e) => {
                let (ea, ee) = (self.encoder(a), self.encoder(e));
                format!("match v {{\n    Ok(x) => {{\n        o.push_str(\"{{\\\"ok\\\":\");\n        {ea}(x, o);\n        o.push('}}');\n    }}\n    Err(x) => {{\n        o.push_str(\"{{\\\"err\\\":\");\n        {ee}(x, o);\n        o.push('}}');\n    }}\n}}")
            }
            Ty::Enum(id) if g.p.is_sum(id) => self.sum_encoder(id),
            Ty::Enum(id) => {
                let mut arms = String::new();
                for v in g.enum_values(id) {
                    arms.push_str(&format!("    {} => {},\n", g.member(id, &v).print(Target::Rust), ritsu_emit::lit::rust(&ritsu_base::json::quote(&v))));
                }
                format!("o.push_str(match *v {{\n{arms}}});")
            }
            Ty::Record(r @ RecId::Variant { value, .. }) => {
                let Some(id) = r.sum() else { return String::new() };
                let fs = g.p.value_fields(&id, value);
                if fs.is_empty() {
                    format!("let _ = v;\no.push_str({});", ritsu_emit::lit::rust(&ritsu_base::json::quote(value)))
                } else {
                    format!("o.push_str({});\n{}o.push_str(\"}}}}\");", ritsu_emit::lit::rust(&format!("{{{}:{{", ritsu_base::json::quote(value))), self.fields_write(t, &fs, "v"))
                }
            }
            Ty::Record(rid @ RecId::Module { .. }) => {
                let fs = g.p.fields(rid);
                format!("o.push('{{');\n{}o.push('}}');", self.fields_write(t, &fs, "v"))
            }
            Ty::Record(RecId::Entry(k, x)) => {
                let (ek, ex) = (self.encoder(k), self.encoder(x));
                format!("o.push_str(\"{{\\\"key\\\":\");\n{ek}(&v.key, o);\no.push_str(\",\\\"value\\\":\");\n{ex}(&v.value, o);\no.push('}}');")
            }
            Ty::Record(RecId::Indexed(x)) => {
                let ex = self.encoder(x);
                format!("o.push_str(\"{{\\\"index\\\":\");\nsuper::int_out(v.index, o);\no.push_str(\",\\\"item\\\":\");\n{ex}(&v.item, o);\no.push('}}');")
            }
            Ty::Record(RecId::Running(item, w)) => {
                let (ei, ew) = (self.encoder(item), self.encoder(w));
                format!(
                    "o.push_str(\"{{\\\"item\\\":\");\n{ei}(&v.item, o);\no.push_str(\",\\\"before\\\":\");\n{ew}(&v.before, o);\no.push_str(\",\\\"upto\\\":\");\n{ew}(&v.upto, o);\no.push_str(\",\\\"total\\\":\");\n{ew}(&v.total, o);\no.push('}}');"
                )
            }
            Ty::Record(RecId::Outputs(f)) => self.outputs_encoder(f),
            Ty::Record(RecId::Element(f, seq)) => self.element_encoder(f, seq),
            Ty::Transfer(id) => self.transfer_encoder(id),
            _ => "let _ = v;\no.push_str(\"null\");".to_string(),
        };
        format!("    fn {n}(v: &{ty}, o: &mut ::std::string::String) {{\n        {}\n    }}\n\n", indent(&body, 8))
    }

    /// The fields of a value of `holder` at `x`, each `"name":value` in the order they are declared,
    /// out of its box.
    fn fields_write(&self, holder: &Ty, fields: &[(crate::ast::Name, crate::ty::Slot)], x: &str) -> String {
        let g = self.g;
        let mut out = String::new();
        for (k, (f, slot)) in fields.iter().enumerate() {
            let key = format!("{}{}:", if k > 0 { "," } else { "" }, ritsu_base::json::quote(&f.text));
            out.push_str(&format!("o.push_str({});\n", ritsu_emit::lit::rust(&key)));
            let at = format!("{x}.{}", field_name(Target::Rust, f.ascii()));
            match (g.boxed(holder, &slot.ty), &slot.ty) {
                (Boxed::InOpt, Ty::Opt(inner)) => {
                    let e = self.encoder(inner);
                    out.push_str(&format!("match &{at} {{\n    Some(b) => {e}(b, o),\n    None => o.push_str(\"null\"),\n}}\n"));
                }
                (Boxed::InResult(..), Ty::Result(a, er)) => {
                    let (ea, ee) = (self.encoder(a), self.encoder(er));
                    out.push_str(&format!(
                        "match &{at} {{\n    Ok(x) => {{\n        o.push_str(\"{{\\\"ok\\\":\");\n        {ea}(x, o);\n        o.push('}}');\n    }}\n    Err(x) => {{\n        o.push_str(\"{{\\\"err\\\":\");\n        {ee}(x, o);\n        o.push('}}');\n    }}\n}}\n"
                    ));
                }
                _ => {
                    let e = self.encoder(&slot.ty);
                    out.push_str(&format!("{e}(&{at}, o);\n"));
                }
            }
        }
        out
    }

    /// A sum on the wire: a value with no fields by its name, a value with fields as an object of one
    /// pair.
    fn sum_encoder(&self, id: &crate::ty::EnumId) -> String {
        let g = self.g;
        let Owner::Module(file) = &id.owner else { return String::new() };
        let mut arms = String::new();
        for v in g.enum_values(id) {
            let r = RecId::Variant { file: file.clone(), name: id.name.clone(), value: v.clone(), args: id.args.clone() };
            let fs = g.p.value_fields(id, &v);
            let ctor = g.variant_of(id, &v);
            if fs.is_empty() {
                arms.push_str(&format!("    {ctor}(_) => o.push_str({}),\n", ritsu_emit::lit::rust(&ritsu_base::json::quote(&v))));
                continue;
            }
            let fields = self.fields_write(&Ty::Record(r), &fs, "x");
            arms.push_str(&format!("    {ctor}(x) => {{\n        o.push_str({});\n{}        o.push_str(\"}}}}\");\n    }}\n", ritsu_emit::lit::rust(&format!("{{{}:{{", ritsu_base::json::quote(&v))), indent_lines(&fields, 8)));
        }
        format!("match v {{\n{arms}}}")
    }

    /// A rule's outputs on the wire: an object of its columns, by their names (one output is the
    /// value itself in Rust's code).
    fn outputs_encoder(&self, f: &std::path::Path) -> String {
        let g = self.g;
        let Some(facts) = g.p.rules.get(f).cloned() else { return "let _ = v;\no.push_str(\"null\");".into() };
        let mut out = String::from("o.push('{');\n");
        for (k, col) in facts.outputs.iter().enumerate() {
            let key = format!("{}{}:", if k > 0 { "," } else { "" }, ritsu_base::json::quote(&col.name));
            out.push_str(&format!("o.push_str({});\n", ritsu_emit::lit::rust(&key)));
            let at = if facts.outputs.len() == 1 { Spot::Lent("v".to_string()) } else { Spot::Field(format!("v.{}", facts.rust.outputs.get(k).map(|o| o.alias.clone()).unwrap_or_default())) };
            out.push_str(&self.column_write(&facts, f, &col.ty, &at));
        }
        out.push_str("o.push('}');");
        out
    }

    /// An element of a list a rule walks on the wire (DESIGN 4.4), as `vectors` writes it: an object of
    /// its fields by the names the module reads them by, in their order, each as kaname's types hold it
    /// (a date as `YYYY-MM-DD`, not the count of days the rule's code holds it in).
    fn element_encoder(&self, f: &std::path::Path, seq: &str) -> String {
        let g = self.g;
        let Some(facts) = g.p.rules.get(f).cloned() else { return "let _ = v;\no.push_str(\"null\");".into() };
        let Some((_, _, cols)) = facts.elements.iter().find(|(x, _, _)| x == seq) else { return "let _ = v;\no.push_str(\"null\");".into() };
        let fields = g.p.fields(&RecId::Element(f.to_path_buf(), seq.to_string()));
        let mut out = String::from("o.push('{');\n");
        for (k, ((name, _), col)) in fields.iter().zip(cols).enumerate() {
            let key = format!("{}{}:", if k > 0 { "," } else { "" }, ritsu_base::json::quote(&name.text));
            out.push_str(&format!("o.push_str({});\n", ritsu_emit::lit::rust(&key)));
            let at = Spot::Field(format!("v.{}", ritsu_emit::ident::rust_name(&col.alias)));
            out.push_str(&self.column_write(&facts, f, &col.ty, &at));
        }
        out.push_str("o.push('}');");
        out
    }

    /// A rule's column at the place `at` on the wire.
    fn column_write(&self, facts: &ritsu_ports::RuleFacts, f: &std::path::Path, t: &ColumnType, at: &Spot) -> String {
        let g = self.g;
        match t {
            ColumnType::Opt(x) => {
                let inner = self.column_write(facts, f, x, &Spot::Lent("x".to_string()));
                format!("match {} {{\n    Some(x) => {{\n{}    }}\n    None => o.push_str(\"null\"),\n}}\n", at.lent(), indent_lines(&inner, 8))
            }
            ColumnType::Bool => format!("o.push_str(if {} {{ \"true\" }} else {{ \"false\" }});\n", at.value()),
            ColumnType::Str => format!("super::str_out({}, o);\n", at.lent()),
            ColumnType::Date { .. } => format!("super::str_out(&{}::date_of({}).to_string(), o);\n", g.imp(super::plan::Import::Kaname), at.value()),
            ColumnType::Enum { name, .. } => {
                let r = g.imp(super::plan::Import::Rule(f.to_path_buf()));
                let Some(e) = facts.rust.enums.iter().find(|e| e.name == *name) else { return "o.push_str(\"null\");\n".into() };
                let mut arms = String::new();
                for (v, m) in &e.values {
                    arms.push_str(&format!("    {r}::{}::{m} => {},\n", e.alias, ritsu_emit::lit::rust(&ritsu_base::json::quote(v))));
                }
                format!("o.push_str(match {} {{\n{arms}}});\n", at.value())
            }
            ColumnType::Num { unit: Some(u), .. } if u.dim == ritsu_units::Dim::Rate || u.type_name().is_some() => format!("super::int_out({}.0, o);\n", at.place()),
            ColumnType::Num { .. } => format!("super::int_out({}, o);\n", at.value()),
        }
    }

    /// A transfer on the wire, as `vectors` writes it: `{"book","transfer","op","args"}`, the arguments
    /// the operation takes (an amount `post` may leave out, where it is given).
    fn transfer_encoder(&self, id: &crate::ty::TransferId) -> String {
        let g = self.g;
        let Some(book) = g.p.books.get(&id.book).cloned() else { return "let _ = v;\no.push_str(\"null\");".into() };
        let Some(tr) = book.transfers.iter().find(|x| x.name == id.transfer).cloned() else { return "let _ = v;\no.push_str(\"null\");".into() };
        let names = super::books::param_names(Target::Rust, &book, &id.transfer);
        let head = format!(
            "{{\"book\":{},\"transfer\":{},\"op\":{},\"args\":{{",
            ritsu_base::json::quote(&book.name),
            ritsu_base::json::quote(&id.transfer),
            ritsu_base::json::quote(&id.op)
        );
        let mut out = String::from("let mut args: ::std::vec::Vec<::std::string::String> = ::std::vec::Vec::new();\n");
        for (i, optional) in super::books::op_params(&book, &id.transfer, &id.op) {
            let p = &tr.params[i];
            let key = ritsu_emit::lit::rust(&format!("{}:", ritsu_base::json::quote(&p.name)));
            let n = &names[i];
            let amount = p.unit.as_ref().and_then(|u| crate::names::book_unit(&book, u)).is_some_and(|u| u.type_name().is_some());
            let write = |x: &str| -> String {
                match (&p.unit, amount) {
                    (Some(_), true) => format!("args.push(format!(\"{{}}{{}}\", {key}, {x}.0));\n"),
                    (Some(_), false) => format!("args.push(format!(\"{{}}{{}}\", {key}, {x}));\n"),
                    (None, _) => format!("args.push(format!(\"{{}}{{}}\", {key}, super::json_str(&{x})));\n"),
                }
            };
            if optional {
                out.push_str(&format!("if let Some(x) = &v.{n} {{\n    {}}}\n", write("x")));
            } else {
                out.push_str(&write(&format!("v.{n}")));
            }
        }
        out.push_str(&format!("o.push_str({});\no.push_str(&args.join(\",\"));\no.push_str(\"}}}}\");", ritsu_emit::lit::rust(&head)));
        out
    }
}

/// Whether the body names what a `use` line brings in: `use crate::glue::kaname;` where the body
/// writes `kaname::`, and a line with `as` by its name.
fn named_in(u: &str, body: &str) -> bool {
    let line = u.trim_start_matches("use ").trim_end_matches(';');
    let name = line.rsplit([' ', ':']).next().unwrap_or(line);
    name == "*" || line.starts_with("std::collections::") || body.contains(&format!("{name}::"))
}

/// An argument of the door's call of a function of the module: as the function takes it, by value
/// where it is `Copy`, else lent (`&a0`, `a0.as_deref()`, `a0.as_ref()`).
fn lend(g: &Gen, t: &Ty, x: &str) -> String {
    if g.is_copy(t) {
        return x.to_string();
    }
    match t {
        Ty::Opt(inner) => match &**inner {
            Ty::Str | Ty::List(_) => format!("{x}.as_deref()"),
            _ => format!("{x}.as_ref()"),
        },
        _ => format!("&{x}"),
    }
}

/// Where the value of a rule's column is, as the door's code writes it: a field of a value the encoder
/// borrows (`v.price`), or a reference to it (`v`, the one output of a rule; `x`, what a column that
/// may be none holds).
enum Spot {
    Field(String),
    Lent(String),
}

impl Spot {
    /// The value itself, read where it is (a `Copy` one, or one matched on with no binding).
    fn value(&self) -> String {
        match self {
            Spot::Field(p) => p.clone(),
            Spot::Lent(r) => format!("*{r}"),
        }
    }

    /// A reference to it (`&v.note`, `x`).
    fn lent(&self) -> String {
        match self {
            Spot::Field(p) => format!("&{p}"),
            Spot::Lent(r) => r.clone(),
        }
    }

    /// What a field of it is read off (`v.price`, `x`: `x.0` reads through the reference).
    fn place(&self) -> &str {
        match self {
            Spot::Field(p) | Spot::Lent(p) => p,
        }
    }
}

/// A text as the inside of a literal of `format!`: its braces doubled, and what a literal of Rust
/// escapes escaped.
fn rs_fmt_text(s: &str) -> String {
    let lit = ritsu_emit::lit::rust(&s.replace('{', "{{").replace('}', "}}"));
    lit[1..lit.len() - 1].to_string()
}

/// Each line of `s` that is not empty with `n` more spaces before it.
fn indent_lines(s: &str, n: usize) -> String {
    let pad = " ".repeat(n);
    s.lines().map(|l| if l.is_empty() { "\n".to_string() } else { format!("{pad}{l}\n") }).collect()
}

/// Each line but the first of `s` with `n` more spaces before it.
fn indent(s: &str, n: usize) -> String {
    let pad = " ".repeat(n);
    let mut out = String::new();
    for (i, l) in s.lines().enumerate() {
        if i > 0 {
            out.push('\n');
            if !l.is_empty() {
                out.push_str(&pad);
            }
        }
        out.push_str(l);
    }
    if s.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// The sentences of the door's refusals, in the language of the package, and of the entry's failure
/// of a string that is not well formed (`{path}`, `{fun}`, `{name}`, `{module}`, `{given}` and
/// `{needed}` stand for what each says); and the words a refusal of JSON no value of its type says what
/// is there with (DESIGN 16.4), as the readers of the other targets say it.
struct Said {
    not_utf8: &'static str,
    not_object: &'static str,
    deep: &'static str,
    missing_key: &'static str,
    not_string_key: &'static str,
    not_object_key: &'static str,
    no_module: &'static str,
    no_function: &'static str,
    wrong: &'static str,
    missing: &'static str,
    outside: &'static str,
    not_taken: &'static str,
    bad: &'static str,
    /// A day of no calendar in a field of a rule's element, as the check of a date says one outside
    /// its range.
    day: &'static str,
    /// What is written before and after a value as JSON writes it (`null`, `2.5`, `"three"`).
    lit: (&'static str, &'static str),
    an_array: &'static str,
    an_object: &'static str,
    no_key: &'static str,
    /// Before and after the key of an object of one key, and the count of the keys of one of more.
    one_key: (&'static str, &'static str),
    keys: (&'static str, &'static str),
}

fn said(lang: Lang) -> Said {
    match lang {
        Lang::En => Said {
            not_utf8: "The input is not UTF-8",
            not_object: "The input is not a JSON object",
            deep: "The input nests deeper than {depth}",
            missing_key: "The input has no `{name}`",
            not_string_key: "The `{name}` of the input is not a string",
            not_object_key: "The `{name}` of the input is not a JSON object",
            no_module: "There is no module `{module}`",
            no_function: "The module `{module}` has no function `{name}` a host calls",
            wrong: "The argument `{path}` of `{fun}` is {given}, and must {needed}",
            missing: "The argument `{path}` of `{fun}` is missing, and must {needed}",
            outside: "The argument `{path}` of `{fun}` is outside int64",
            not_taken: "The argument `{path}` of `{fun}` is of a type no host gives",
            bad: "The argument `{path}` of `{fun}` is no well-formed string of Unicode",
            day: "The argument `{path}` of `{fun}` is {given}, outside its range {needed}",
            lit: ("", ""),
            an_array: "an array",
            an_object: "an object",
            no_key: "an object with no key",
            one_key: ("an object with the key ", ""),
            keys: ("an object with ", " keys"),
        },
        Lang::Ja => Said {
            not_utf8: "入力が UTF-8 ではありません",
            not_object: "入力が JSON のオブジェクトではありません",
            deep: "入力の入れ子が {depth} 段より深くなっています",
            missing_key: "入力に `{name}` がありません",
            not_string_key: "入力の `{name}` が文字列ではありません",
            not_object_key: "入力の `{name}` が JSON のオブジェクトではありません",
            no_module: "モジュール `{module}` はありません",
            no_function: "モジュール `{module}` に、ホストが呼ぶ関数 `{name}` はありません",
            wrong: "関数 `{fun}` の引数 `{path}` が{given}です。{needed}が要ります",
            missing: "関数 `{fun}` の引数 `{path}` がありません。{needed}が要ります",
            outside: "関数 `{fun}` の引数 `{path}` が int64 の外にあります",
            not_taken: "関数 `{fun}` の引数 `{path}` は、ホストが渡さない型です",
            bad: "関数 `{fun}` の引数 `{path}` は、Unicode の文字列として正しくありません",
            day: "関数 `{fun}` の引数 `{path}` の {given} は、範囲 {needed} の外です",
            lit: (" ", " "),
            an_array: "配列",
            an_object: "オブジェクト",
            no_key: "キーの無いオブジェクト",
            one_key: ("キー ", " のオブジェクト"),
            keys: ("キーを ", " 個持つオブジェクト"),
        },
    }
}

/// The functions of the door that make its sentences.
fn sentences(s: &Said, lang: Lang) -> String {
    // a sentence as the literal of `format!` over the names it holds
    let f = |text: &str| {
        let mut t = text.replace('{', "{{").replace('}', "}}");
        for n in ["path", "fun", "name", "module", "depth", "given", "needed"] {
            t = t.replace(&format!("{{{{{n}}}}}"), &format!("{{{n}}}"));
        }
        ritsu_emit::lit::rust(&t)
    };
    let lit = ritsu_emit::lit::rust;
    let mut o = String::new();
    o.push_str(&format!("const NOT_OBJECT: &str = {};\n\n", lit(s.not_object)));
    o.push_str(&format!(
        "/// The refusal of bytes that are not UTF-8, which the package's Wasm answers before it reads a call.\npub fn not_utf8() -> ::std::string::String {{\n    refusal({})\n}}\n\n",
        lit(s.not_utf8)
    ));
    o.push_str(&format!("fn too_deep() -> ::std::string::String {{\n    let depth = DEPTH;\n    format!({})\n}}\n\n", f(s.deep)));
    o.push_str(&format!("fn missing_key(name: &str) -> ::std::string::String {{\n    format!({})\n}}\n\n", f(s.missing_key)));
    o.push_str(&format!("fn not_string_key(name: &str) -> ::std::string::String {{\n    format!({})\n}}\n\n", f(s.not_string_key)));
    o.push_str(&format!("fn not_object_key(name: &str) -> ::std::string::String {{\n    format!({})\n}}\n\n", f(s.not_object_key)));
    o.push_str(&format!("fn no_module(module: &str) -> ::std::string::String {{\n    format!({})\n}}\n\n", f(s.no_module)));
    o.push_str(&format!("fn no_function(module: &str, name: &str) -> ::std::string::String {{\n    format!({})\n}}\n\n", f(s.no_function)));
    for (name, n) in [("NEED_STR", Need::Str), ("NEED_INT", Need::Int), ("NEED_BOOL", Need::Bool), ("NEED_DATE", Need::Date), ("NEED_ARRAY", Need::Array), ("NEED_OBJECT", Need::Object), ("NEED_PAIR", Need::Pair)] {
        o.push_str(&format!("const {name}: &str = {};\n", lit(needs(lang, n))));
    }
    o.push('\n');
    o.push_str(&format!(
        "/// What is at a place of a call, as the sentences say it.\nfn given(j: &Json) -> ::std::string::String {{\n    let lit = |s: &str| format!(\"{{}}{{s}}{{}}\", {}, {});\n    match j {{\n        Json::Null => lit(\"null\"),\n        Json::Bool(b) => lit(if *b {{ \"true\" }} else {{ \"false\" }}),\n        Json::Num(t) => lit(if t == \"-0\" {{ \"0\" }} else {{ t }}),\n        Json::Str(s, _) => lit(&quoted(s)),\n        Json::Arr(_) => {}.into(),\n        Json::Obj(_) | Json::Absent => {}.into(),\n    }}\n}}\n\n",
        lit(s.lit.0),
        lit(s.lit.1),
        lit(s.an_array),
        lit(s.an_object)
    ));
    o.push_str(&format!(
        "/// An object given for a value of a sum, as the sentences say it: by its keys.\nfn keyed_text(j: &Json) -> ::std::string::String {{\n    let Json::Obj(fs) = j else {{\n        return given(j);\n    }};\n    let keys: ::std::collections::BTreeSet<&str> = fs.iter().map(|(k, _)| k.as_str()).collect();\n    match keys.len() {{\n        0 => {}.into(),\n        1 => format!(\"{{}}{{}}{{}}\", {}, quoted(keys.first().copied().unwrap_or_default()), {}),\n        n => format!(\"{{}}{{n}}{{}}\", {}, {}),\n    }}\n}}\n\n",
        lit(s.no_key),
        lit(s.one_key.0),
        lit(s.one_key.1),
        lit(s.keys.0),
        lit(s.keys.1)
    ));
    o.push_str(&format!(
        "/// The refusal of JSON at `path` that is no value of its type: what is there, and what it must be.\nfn wrong(j: &Json, path: &str, cx: &Cx, needed: &str) -> Refused {{\n    let fun = cx.fun;\n    let s = match j {{\n        Json::Absent => format!({}),\n        _ => {{\n            let given = given(j);\n            format!({})\n        }}\n    }};\n    Refused {{ path: path.into(), error: crate::glue::kaname::spaced(&s) }}\n}}\n\n",
        f(s.missing),
        f(s.wrong)
    ));
    o.push_str(&format!(
        "/// The refusal of an object given for a value of a sum, said by its keys.\nfn keyed(j: &Json, path: &str, cx: &Cx, needed: &str) -> Refused {{\n    let fun = cx.fun;\n    let given = keyed_text(j);\n    Refused {{ path: path.into(), error: crate::glue::kaname::spaced(&format!({})) }}\n}}\n\n",
        f(s.wrong)
    ));
    o.push_str(&format!("fn outside(path: &str, cx: &Cx) -> Refused {{\n    let fun = cx.fun;\n    Refused {{ path: path.into(), error: format!({}) }}\n}}\n\n", f(s.outside)));
    o.push_str(&format!("fn not_taken(path: &str, cx: &Cx) -> Refused {{\n    let fun = cx.fun;\n    Refused {{ path: path.into(), error: format!({}) }}\n}}\n\n", f(s.not_taken)));
    o.push_str(&format!("fn bad_message(fun: &str, path: &str) -> ::std::string::String {{\n    format!({})\n}}\n\n", f(s.bad)));
    o.push_str(&format!("fn day_message(fun: &str, path: &str, given: &str, needed: &str) -> ::std::string::String {{\n    format!({})\n}}\n\n", f(s.day)));
    o
}

/// What every door has: the reader of JSON, the readers of the values of the wire and the writers of
/// the lines.
const READER: &str = r#"/// A JSON value as the door reads it: a number as it is written, a string with whether it was written
/// with an escape that is no Unicode scalar value (a lone surrogate, in its place U+FFFD); and the
/// value of a key an object leaves out, which no JSON writes.
pub enum Json {
    Null,
    Bool(bool),
    Num(::std::string::String),
    Str(::std::string::String, bool),
    Arr(::std::vec::Vec<Json>),
    Obj(::std::vec::Vec<(::std::string::String, Json)>),
    Absent,
}

/// A value read off the wire, or its refusal.
type Read<T> = ::std::result::Result<T, Refused>;

/// The refusal of JSON that is no value of the type of its place: the place, from the argument's
/// name (`submit.form.days`, `organization.people[0]`), and the sentence.
struct Refused {
    path: ::std::string::String,
    error: ::std::string::String,
}

/// What the door keeps as it reads the arguments of a call: the function's name, for its sentences,
/// and the first place, in the order the entry checks, where the door fails the call itself, with
/// its sentence: a string that is no well-formed Unicode, a day of no calendar in a field of a rule's
/// element.
struct Cx {
    fun: &'static str,
    bad: Option<(::std::string::String, ::std::string::String)>,
}

static ABSENT: Json = Json::Absent;

/// The one JSON value `text` is, read to the depth of `DEPTH`.
fn read(text: &str) -> ::std::result::Result<Json, ::std::string::String> {
    let b = text.as_bytes();
    let mut i = 0;
    let v = value(b, &mut i, 0)?;
    space(b, &mut i);
    if i == b.len() { Ok(v) } else { Err(NOT_OBJECT.into()) }
}

fn space(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
        *i += 1;
    }
}

fn value(b: &[u8], i: &mut usize, depth: usize) -> ::std::result::Result<Json, ::std::string::String> {
    space(b, i);
    let bad = || ::std::string::String::from(NOT_OBJECT);
    let c = *b.get(*i).ok_or_else(bad)?;
    if (c == b'{' || c == b'[') && depth >= DEPTH {
        return Err(too_deep());
    }
    match c {
        b'{' => {
            *i += 1;
            let mut pairs = ::std::vec::Vec::new();
            space(b, i);
            if b.get(*i) == Some(&b'}') {
                *i += 1;
                return Ok(Json::Obj(pairs));
            }
            loop {
                space(b, i);
                let (k, broken) = string(b, i).ok_or_else(bad)?;
                if broken {
                    return Err(bad());
                }
                space(b, i);
                if b.get(*i) != Some(&b':') {
                    return Err(bad());
                }
                *i += 1;
                let v = value(b, i, depth + 1)?;
                pairs.push((k, v));
                space(b, i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(b'}') => {
                        *i += 1;
                        return Ok(Json::Obj(pairs));
                    }
                    _ => return Err(bad()),
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut xs = ::std::vec::Vec::new();
            space(b, i);
            if b.get(*i) == Some(&b']') {
                *i += 1;
                return Ok(Json::Arr(xs));
            }
            loop {
                xs.push(value(b, i, depth + 1)?);
                space(b, i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(b']') => {
                        *i += 1;
                        return Ok(Json::Arr(xs));
                    }
                    _ => return Err(bad()),
                }
            }
        }
        b'"' => string(b, i).map(|(s, broken)| Json::Str(s, broken)).ok_or_else(bad),
        b't' => word(b, i, "true").map(|_| Json::Bool(true)).ok_or_else(bad),
        b'f' => word(b, i, "false").map(|_| Json::Bool(false)).ok_or_else(bad),
        b'n' => word(b, i, "null").map(|_| Json::Null).ok_or_else(bad),
        _ => number(b, i).map(Json::Num).ok_or_else(bad),
    }
}

fn word(b: &[u8], i: &mut usize, w: &str) -> Option<()> {
    if b[*i..].starts_with(w.as_bytes()) {
        *i += w.len();
        Some(())
    } else {
        None
    }
}

fn number(b: &[u8], i: &mut usize) -> Option<::std::string::String> {
    let start = *i;
    let digits = |i: &mut usize| {
        let s = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i > s
    };
    if b.get(*i) == Some(&b'-') {
        *i += 1;
    }
    if b.get(*i) == Some(&b'0') {
        *i += 1;
    } else if !digits(i) {
        return None;
    }
    if b.get(*i) == Some(&b'.') {
        *i += 1;
        if !digits(i) {
            return None;
        }
    }
    if matches!(b.get(*i), Some(b'e' | b'E')) {
        *i += 1;
        if matches!(b.get(*i), Some(b'+' | b'-')) {
            *i += 1;
        }
        if !digits(i) {
            return None;
        }
    }
    Some(::std::string::String::from_utf8_lossy(&b[start..*i]).into_owned())
}

/// A JSON string, every escape JSON has read, and whether one of them is no Unicode scalar value.
fn string(b: &[u8], i: &mut usize) -> Option<(::std::string::String, bool)> {
    if b.get(*i) != Some(&b'"') {
        return None;
    }
    *i += 1;
    let mut s: ::std::vec::Vec<u8> = ::std::vec::Vec::new();
    let mut broken = false;
    let hex = |j: usize| -> Option<u32> { ::std::str::from_utf8(b.get(j..j + 4)?).ok().and_then(|h| u32::from_str_radix(h, 16).ok()) };
    loop {
        let c = *b.get(*i)?;
        *i += 1;
        match c {
            b'"' => return ::std::string::String::from_utf8(s).ok().map(|s| (s, broken)),
            b'\\' => {
                let e = *b.get(*i)?;
                *i += 1;
                let ch = match e {
                    b'"' => '"',
                    b'\\' => '\\',
                    b'/' => '/',
                    b'b' => '\u{8}',
                    b'f' => '\u{c}',
                    b'n' => '\n',
                    b'r' => '\r',
                    b't' => '\t',
                    b'u' => {
                        let mut cp = hex(*i)?;
                        *i += 4;
                        let pair = (0xD800..0xDC00).contains(&cp) && b.get(*i) == Some(&b'\\') && b.get(*i + 1) == Some(&b'u');
                        if let Some(lo) = hex(*i + 2).filter(|l| pair && (0xDC00..0xE000).contains(l)) {
                            cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                            *i += 6;
                        }
                        match char::from_u32(cp) {
                            Some(ch) => ch,
                            None => {
                                broken = true;
                                '\u{FFFD}'
                            }
                        }
                    }
                    _ => return None,
                };
                let mut buf = [0u8; 4];
                s.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            }
            c if c < 0x20 => return None,
            c => s.push(c),
        }
    }
}

/// The value of a key of an object: the later of two (as JavaScript's `JSON.parse` reads them).
fn get<'a>(fs: &'a [(::std::string::String, Json)], key: &str) -> Option<&'a Json> {
    fs.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// The JSON of an argument of the call, absent where the call leaves it out.
fn arg<'a>(inn: &'a [(::std::string::String, Json)], name: &str) -> &'a Json {
    get(inn, name).unwrap_or(&ABSENT)
}

/// The JSON of a field of an object of the call, absent where the object leaves it out: none, for a
/// field that may be none.
fn field<'a>(fs: &'a [(::std::string::String, Json)], name: &str) -> &'a Json {
    get(fs, name).unwrap_or(&ABSENT)
}

/// The one key of an object and its value (the later, where it is written twice), where it has one.
fn one_key(fs: &[(::std::string::String, Json)]) -> Option<(&str, &Json)> {
    let (k, _) = fs.first()?;
    if fs.iter().all(|(x, _)| x == k) { fs.last().map(|(k, v)| (k.as_str(), v)) } else { None }
}

/// A string as the sentences show it: its first 40 characters, as JSON writes them.
fn quoted(s: &str) -> ::std::string::String {
    let mut t: ::std::string::String = s.chars().take(40).collect();
    if s.chars().nth(40).is_some() {
        t.push('\u{2026}');
    }
    json_str(&t)
}

/// A whole number of int64: written `20`, `20.0`, `2e1` or `2.0e1`, as any encoder of JSON may.
fn int(j: &Json, path: &str, cx: &mut Cx) -> Read<i64> {
    let Json::Num(text) = j else {
        return Err(wrong(j, path, cx, NEED_INT));
    };
    let (neg, rest) = match text.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, text.as_str()),
    };
    // an exponent longer than any int64 needs is as far as 10^18, either way
    let (mantissa, exp) = match rest.find(['e', 'E']) {
        Some(k) => {
            let e = &rest[k + 1..];
            let ds = e.trim_start_matches(['+', '-']);
            let n: i64 = if ds.len() > 18 { 1_000_000_000_000_000_000 } else { ds.parse().unwrap_or(0) };
            (&rest[..k], if e.starts_with('-') { -n } else { n })
        }
        None => (rest, 0),
    };
    let (whole, frac) = match mantissa.find('.') {
        Some(k) => (&mantissa[..k], &mantissa[k + 1..]),
        None => (mantissa, ""),
    };
    let mut digits: ::std::string::String = format!("{whole}{frac}");
    let mut exp = exp.saturating_sub(frac.len() as i64);
    // the digits past the point the exponent leaves: zeros, or it is no whole number
    while exp < 0 {
        match digits.pop() {
            Some('0') => exp += 1,
            Some(_) => return Err(wrong(j, path, cx, NEED_INT)),
            None => {
                exp = 0;
            }
        }
    }
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Ok(0);
    }
    if (digits.len() as i64).saturating_add(exp) > 19 {
        return Err(outside(path, cx));
    }
    let mut v: i128 = digits.parse::<i128>().map_err(|_| outside(path, cx))?;
    for _ in 0..exp {
        v *= 10;
    }
    if neg {
        v = -v;
    }
    i64::try_from(v).map_err(|_| outside(path, cx))
}

fn boolean(j: &Json, path: &str, cx: &mut Cx) -> Read<bool> {
    match j {
        Json::Bool(b) => Ok(*b),
        _ => Err(wrong(j, path, cx, NEED_BOOL)),
    }
}

/// A string; one with an escape that is no Unicode scalar value kept as the first such place.
fn string_of(j: &Json, path: &str, cx: &mut Cx) -> Read<::std::string::String> {
    match j {
        Json::Str(s, broken) => {
            if *broken && cx.bad.is_none() {
                cx.bad = Some((path.to_string(), bad_message(cx.fun, path)));
            }
            Ok(s.clone())
        }
        _ => Err(wrong(j, path, cx, NEED_STR)),
    }
}

/// A date written `YYYY-MM-DD` (four digits or more, then two and two), as a host builds one, of any
/// year, month and day: the entry holds it to the days of the calendar.
fn date(j: &Json, path: &str, cx: &mut Cx) -> Read<crate::glue::kaname::Date> {
    let Json::Str(s, false) = j else {
        return Err(wrong(j, path, cx, NEED_DATE));
    };
    let parts: ::std::vec::Vec<&str> = s.split('-').collect();
    let ok = parts.len() == 3 && parts[0].len() >= 4 && parts[1].len() == 2 && parts[2].len() == 2 && parts.iter().all(|p| p.bytes().all(|c| c.is_ascii_digit()));
    if !ok {
        return Err(wrong(j, path, cx, NEED_DATE));
    }
    let (Ok(year), Ok(month), Ok(day)) = (parts[0].parse::<i32>(), parts[1].parse::<u32>(), parts[2].parse::<u32>()) else {
        return Err(wrong(j, path, cx, NEED_DATE));
    };
    Ok(crate::glue::kaname::Date { year, month, day })
}

/// The date of a field of a rule's element, which the rule holds as a count of days: one that is no
/// day of 0001-01-01 to 9999-12-31 (2026-13-01, 10000-01-01) has none, and is kept as a place the
/// door fails, outside `needed`, the range of the field.
fn day_held(d: &crate::glue::kaname::Date, path: &str, needed: &str, cx: &mut Cx) {
    if !d.valid() && cx.bad.is_none() {
        cx.bad = Some((path.to_string(), day_message(cx.fun, path, &d.to_string(), needed)));
    }
}

fn array<'a>(j: &'a Json, path: &str, cx: &mut Cx) -> Read<&'a [Json]> {
    match j {
        Json::Arr(xs) => Ok(xs),
        _ => Err(wrong(j, path, cx, NEED_ARRAY)),
    }
}

fn object<'a>(j: &'a Json, path: &str, cx: &mut Cx) -> Read<&'a [(::std::string::String, Json)]> {
    match j {
        Json::Obj(fs) => Ok(fs),
        _ => Err(wrong(j, path, cx, NEED_OBJECT)),
    }
}

/// A pair of a dict on the wire, `[key, value]`.
fn pair<'a>(j: &'a Json, path: &str, cx: &mut Cx) -> Read<(&'a Json, &'a Json)> {
    match j {
        Json::Arr(xs) if xs.len() == 2 => Ok((&xs[0], &xs[1])),
        _ => Err(wrong(j, path, cx, NEED_PAIR)),
    }
}

/// A string as JSON writes it, as serde_json escapes it: `"`, `\`, and the control characters
/// (`\b`, `\t`, `\n`, `\f`, `\r` by name, the others `\u00xx`).
fn str_out(s: &str, o: &mut ::std::string::String) {
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\u{8}' => o.push_str("\\b"),
            '\t' => o.push_str("\\t"),
            '\n' => o.push_str("\\n"),
            '\u{c}' => o.push_str("\\f"),
            '\r' => o.push_str("\\r"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
}

fn json_str(s: &str) -> ::std::string::String {
    let mut o = ::std::string::String::new();
    str_out(s, &mut o);
    o
}

fn int_out(n: i64, o: &mut ::std::string::String) {
    o.push_str(&n.to_string());
}

/// The line of the entry's failure the door makes itself, at `path`: a string of the call that is no
/// well-formed Unicode (DESIGN 25.8), as the entries of TypeScript and Python say it, and a day of no
/// calendar in a field of a rule's element, as the check of a date says it.
fn bad_line(fun: &str, given: &str, module: &str, line: u32, path: &str, message: &str) -> ::std::string::String {
    let message = crate::glue::kaname::spaced(message);
    format!(
        "{{\"function\":{},\"in\":{given},\"fails\":{{\"kind\":\"input\",\"module\":{},\"line\":{line},\"message\":{},\"path\":{}}}}}",
        json_str(fun),
        json_str(module),
        json_str(&message),
        json_str(path)
    )
}

/// The refusal of a call that names no function or gives no arguments:
/// `{"refused":"input","error":"<sentence>"}`.
fn refusal(error: &str) -> ::std::string::String {
    format!("{{\"refused\":\"input\",\"error\":{}}}", json_str(error))
}

/// The refusal of an argument: `{"refused":"input","error":"<sentence>","path":"<place>"}`.
fn refused(r: &Refused) -> ::std::string::String {
    format!("{{\"refused\":\"input\",\"error\":{},\"path\":{}}}", json_str(&r.error), json_str(&r.path))
}

/// The line of an answer: `{"function":…,"in":…,"out":…}`.
fn out_line(fun: &str, given: &str, out: &str) -> ::std::string::String {
    format!("{{\"function\":{},\"in\":{given},\"out\":{out}}}", json_str(fun))
}

/// The line of a failure: `{"function":…,"in":…,"fails":{"kind","module","line","message"}}`, and
/// for the kind `input` the place, `"path"`.
fn failed(fun: &str, given: &str, e: &crate::glue::kaname::KanameError) -> ::std::string::String {
    let path = match &e.path {
        Some(p) => format!(",\"path\":{}", json_str(p)),
        None => ::std::string::String::new(),
    };
    format!(
        "{{\"function\":{},\"in\":{given},\"fails\":{{\"kind\":{},\"module\":{},\"line\":{},\"message\":{}{path}}}}}",
        json_str(fun),
        json_str(e.kind.as_str()),
        json_str(e.module),
        e.line,
        json_str(&e.message)
    )
}

"#;
