//! ritsu-proto reads everything the three readers it takes the place of read (PLAN C.9): sakai's
//! (replaced in C.9), rulec's and dandori's (replaced in D.10). Each reader reads every `.proto`
//! of the three crates and the examples their own tests used (`tests/fixtures/`), and
//! ritsu-proto's reading of the same file, put in that reader's terms, is what the reader gives.
//!
//! rulec's and dandori's readers are run here, beside ritsu-proto. sakai's was, until sakai
//! read with ritsu-proto; what it gave then is `tests/golden/sakai.txt`, and ritsu-proto is held
//! to it. The other two goldens keep what rulec and dandori get, for D.10, when their readers go.

use ritsu_base::text::{Lang, Text};
use ritsu_proto::validate::Rules;
use ritsu_proto::{Issue, Label, Protos, Resolved, Type, WELL_KNOWN};
use std::collections::BTreeMap;
use std::path::Path;

use rulec::proto as rp;

/// The three crates, and this crate's copies of the examples the readers' own tests used.
const ROOTS: [&str; 4] = ["../rulec", "../dandori", "../sakai", "tests/fixtures"];

/// The files no reader reads whole: a statement left open (sakai's E106 mutant, and dandori's
/// file that is not a `.proto`), and a proto2 `group`. rulec's reader passes over what it does
/// not understand and gives what it found in them; ritsu-proto says where the file is wrong.
const REFUSED: [&str; 3] = ["../sakai/tests/mutants/E106_読めない_proto/proto/shop/billing/v1/billing.proto", "tests/fixtures/dandori/bad/specs/money.proto", "tests/fixtures/sakai/group.proto"];

/// Every `.proto` under a root, as paths from it, in path order; the tools a crate installs
/// (`tools/`) are not its files.
fn protos(root: &str) -> Vec<String> {
    let mut out = Vec::new();
    ritsu_base::paths::walk(Path::new(root), ".", &["tools".to_string()], &mut out);
    out.retain(|p| p.ends_with(".proto"));
    out
}

fn text(root: &str, file: &str) -> String {
    std::fs::read_to_string(Path::new(root).join(file)).unwrap()
}

// ── sakai ───────────────────────────────────────────────────────────────

/// An option as sakai printed it: its name and its value as written.
trait Written {
    fn written(&self) -> (String, String);
}

impl Written for ritsu_proto::Opt {
    fn written(&self) -> (String, String) {
        (self.name.clone(), self.text.clone())
    }
}

/// Why a file did not read, in sakai's words.
trait Said {
    fn said(&self) -> Text;
}

impl Said for ritsu_proto::ReadError {
    fn said(&self) -> Text {
        self.message("sakai")
    }
}

/// What sakai's reader gives for every `.proto` under a root: each file as it reads it alone,
/// then all of them read together as sakai reads a map's — what each import comes to, and what
/// the name of each type a field or a method names comes to. `$m` is the reader's module.
macro_rules! sakai_facts {
    ($m:ident, $root:expr) => {{
        let root: &str = $root;
        let files = protos(root);
        let mut out = String::new();
        for file in &files {
            out.push_str(&format!("== {file}\n"));
            match $m::read(file, &text(root, file)) {
                Ok(f) => {
                    out.push_str(&format!("syntax {}\npackage {}\n", f.syntax, f.package));
                    for i in &f.imports {
                        out.push_str(&format!("import {} {}:{} public={} weak={}\n", i.path, i.line, i.col, i.public, i.weak));
                    }
                    for m in &f.messages {
                        out.push_str(&format!("message {} {}\n", m.name, m.line));
                        for x in &m.fields {
                            out.push_str(&format!("  field {} {} {:?} {:?} json={:?} oneof={:?} line={}\n", x.name, x.number, x.label, x.ty, x.json_name, x.oneof, x.line));
                        }
                    }
                    for e in &f.enums {
                        out.push_str(&format!("enum {} {}\n", e.name, e.line));
                        for v in &e.values {
                            out.push_str(&format!("  value {} {} {}\n", v.name, v.number, v.line));
                        }
                    }
                    for s in &f.services {
                        out.push_str(&format!("service {} {}\n", s.name, s.line));
                        for o in &s.options {
                            let (n, v) = o.written();
                            out.push_str(&format!("  option {n} = {v}\n"));
                        }
                        for me in &s.methods {
                            out.push_str(&format!("  method {} {} {} {} {} {}\n", me.name, me.input, me.output, me.client_streaming, me.server_streaming, me.line));
                            for o in &me.options {
                                let (n, v) = o.written();
                                out.push_str(&format!("    option {n} = {v}\n"));
                            }
                        }
                    }
                }
                Err(e) => {
                    let t = e.said();
                    out.push_str(&format!("error {}:{} {} | {}\n", e.line, e.col, t.get(Lang::En), t.get(Lang::Ja)));
                }
            }
        }
        let (ps, issues) = $m::load(Path::new(root), &files, &[]);
        out.push_str("== together\n");
        for i in &issues {
            match i {
                $m::Issue::Unreadable { file, err } => out.push_str(&format!("unreadable {file} {}:{}\n", err.line, err.col)),
                $m::Issue::NotFound { file, import, tried } => out.push_str(&format!("not found {file} {} {}:{} tried {}\n", import.path, import.line, import.col, tried.join(" "))),
                $m::Issue::OutOfScope { file, import, at } => out.push_str(&format!("out of scope {file} {} at {at}\n", import.path)),
            }
        }
        let said = |r: $m::Resolved| match r {
            $m::Resolved::Found(s) => format!("{} ({}{})", s.full, s.file, if s.is_enum { ", enum" } else { "" }),
            $m::Resolved::Known(k) => format!("known {k}"),
            $m::Resolved::Unknown => "unknown".to_string(),
            $m::Resolved::Missing => "missing".to_string(),
        };
        for (file, f) in &ps.files {
            let vis: Vec<String> = ps.visible(file).into_iter().filter(|v| v != file).collect();
            out.push_str(&format!("{file} sees {}\n", vis.join(" ")));
            for m in &f.messages {
                for (field, r) in ps.field_types(file, m) {
                    out.push_str(&format!("  {}.{field}: {}\n", m.name, said(r)));
                }
            }
            for s in &f.services {
                for me in &s.methods {
                    let (i, o) = (ps.resolve(file, &f.package, &me.input), ps.resolve(file, &f.package, &me.output));
                    out.push_str(&format!("  {}.{}: {} -> {}\n", s.name, me.name, said(i), said(o)));
                }
            }
        }
        out
    }};
}

#[test]
fn sakai_reads_what_it_read_before() {
    let mut got = String::new();
    for root in ROOTS {
        got.push_str(&format!("#### {root}\n"));
        got.push_str(&sakai_facts!(ritsu_proto, root));
    }
    ritsu_testkit::golden("tests/golden/sakai.txt", &got);
}

// ── rulec ───────────────────────────────────────────────────────────────

fn rulec_rules(r: &Rules) -> rp::Rules {
    rp::Rules {
        required: r.required,
        ignore: r.ignore.clone(),
        int: rp::IntRules { konst: r.int.konst, gt: r.int.gt, gte: r.int.gte, lt: r.int.lt, lte: r.int.lte, in_: r.int.in_.clone(), not_in: r.int.not_in.clone() },
        min_items: r.min_items,
        max_items: r.max_items,
        str_const: r.str_const.clone(),
        str_in: r.str_in.clone(),
        str_not_in: r.str_not_in.clone(),
        str_min_len: r.str_min_len,
        cel: r.cel.clone(),
        unread: r.unread.clone(),
    }
}

/// What rulec's reader takes from a file, from what ritsu-proto read of it: the package, the
/// imports, every enum and message by its own short name, and of a message the fields a path
/// can follow (not a `map`; not a proto2 `required`, which rulec's reader does not take for a
/// field), a member of a `oneof` having presence of its own.
fn as_rulec(f: &ritsu_proto::ProtoFile) -> (Option<String>, Vec<String>, Vec<rp::Enum>, Vec<rp::Message>) {
    let short = |n: &str| n.rsplit('.').next().unwrap_or(n).to_string();
    let package = (!f.package.is_empty()).then(|| f.package.clone());
    let imports = f.imports.iter().map(|i| i.path.clone()).collect();
    let enums = f.enums.iter().map(|e| rp::Enum { name: short(&e.name), values: e.values.iter().map(|v| rp::Value { name: v.name.clone(), number: v.number }).collect() }).collect();
    let messages = f
        .messages
        .iter()
        .map(|m| rp::Message {
            name: short(&m.name),
            fields: m
                .fields
                .iter()
                .filter(|x| !matches!(x.ty, Type::Map(..)) && x.label != Label::Required)
                .map(|x| rp::Field {
                    name: x.name.clone(),
                    ty: match &x.ty {
                        Type::Scalar(s) | Type::Named(s) => s.clone(),
                        Type::Map(..) => unreachable!(),
                    },
                    repeated: x.label == Label::Repeated,
                    optional: x.label == Label::Optional || x.oneof.is_some(),
                    rules: rulec_rules(&x.rules),
                    json_name: x.options.iter().find_map(|o| match &o.value {
                        Some(ritsu_proto::Value::Str(s)) if o.name.split_whitespace().collect::<String>() == "json_name" => Some(s.clone()),
                        _ => None,
                    }),
                    oneof: x.oneof.clone(),
                })
                .collect(),
            rules: rp::MsgRules { cel: m.rules.cel.clone(), oneofs: m.rules.oneofs.iter().map(|o| rp::Oneof { fields: o.fields.clone(), required: o.required }).collect(), disabled: m.rules.disabled },
        })
        .collect();
    (package, imports, enums, messages)
}

/// What rulec takes from a file, a line an element; of the rules, what is set.
fn rulec_lines(package: &Option<String>, imports: &[String], enums: &[rp::Enum], messages: &[rp::Message]) -> String {
    let mut out = format!("package {package:?} imports {imports:?}\n");
    for e in enums {
        let vs: Vec<String> = e.values.iter().map(|v| format!("{}={}", v.name, v.number)).collect();
        out.push_str(&format!("enum {} {}\n", e.name, vs.join(" ")));
    }
    for m in messages {
        out.push_str(&format!("message {}\n", m.name));
        for f in &m.fields {
            let mut w = vec![format!("  {} {}", f.name, f.ty)];
            if f.repeated {
                w.push("repeated".into());
            }
            if f.optional {
                w.push("optional".into());
            }
            if let Some(j) = &f.json_name {
                w.push(format!("json_name={j:?}"));
            }
            if let Some(o) = &f.oneof {
                w.push(format!("oneof={o}"));
            }
            w.extend(rules_set(&f.rules));
            out.push_str(&format!("{}\n", w.join(" ")));
        }
        let r = &m.rules;
        if !r.cel.is_empty() || !r.oneofs.is_empty() || r.disabled {
            out.push_str(&format!("  rules cel={:?} oneofs={:?} disabled={}\n", r.cel, r.oneofs.iter().map(|o| (o.fields.clone(), o.required)).collect::<Vec<_>>(), r.disabled));
        }
    }
    out
}

fn rules_set(r: &rp::Rules) -> Vec<String> {
    let mut w = Vec::new();
    let mut opt = |name: &str, v: Option<String>| {
        if let Some(v) = v {
            w.push(format!("{name}={v}"));
        }
    };
    opt("ignore", r.ignore.clone());
    opt("const", r.int.konst.map(|x| x.to_string()));
    opt("gt", r.int.gt.map(|x| x.to_string()));
    opt("gte", r.int.gte.map(|x| x.to_string()));
    opt("lt", r.int.lt.map(|x| x.to_string()));
    opt("lte", r.int.lte.map(|x| x.to_string()));
    opt("min_items", r.min_items.map(|x| x.to_string()));
    opt("max_items", r.max_items.map(|x| x.to_string()));
    opt("str_const", r.str_const.as_ref().map(|x| format!("{x:?}")));
    opt("str_min_len", r.str_min_len.map(|x| x.to_string()));
    let lists = [("in", format!("{:?}", r.int.in_), r.int.in_.is_empty()), ("not_in", format!("{:?}", r.int.not_in), r.int.not_in.is_empty()), ("str_in", format!("{:?}", r.str_in), r.str_in.is_empty()), ("str_not_in", format!("{:?}", r.str_not_in), r.str_not_in.is_empty()), ("cel", format!("{:?}", r.cel), r.cel.is_empty()), ("unread", format!("{:?}", r.unread), r.unread.is_empty())];
    if r.required {
        w.insert(0, "required".into());
    }
    for (name, v, empty) in lists {
        if !empty {
            w.push(format!("{name}={v}"));
        }
    }
    w
}

#[test]
fn rulec_reads_nothing_ritsu_proto_does_not() {
    let mut failures = Vec::new();
    let mut refused = Vec::new();
    let mut golden = String::new();
    let mut n = 0;
    for root in ROOTS {
        for file in protos(root) {
            let src = text(root, &file);
            let at = format!("{root}/{file}");
            let Ok(f) = ritsu_proto::read(&file, &src) else {
                refused.push(at);
                continue;
            };
            let (package, imports, enums, messages) = as_rulec(&f);
            // rulec's reader finds `package` only at the start of a line; ritsu-proto finds it
            // wherever the statement is (`syntax = "proto3"; package a.v1;`).
            if rp::package(&src).is_some_and(|p| Some(p) != package) {
                failures.push(format!("{at}: package {:?}, ritsu-proto {package:?}", rp::package(&src)));
            }
            if rp::imports(&src) != imports {
                failures.push(format!("{at}: imports {:?}, ritsu-proto {imports:?}", rp::imports(&src)));
            }
            if rp::enums(&src) != enums {
                failures.push(format!("{at}: enums\n{:#?}\nritsu-proto\n{enums:#?}", rp::enums(&src)));
            }
            if rp::messages(&src) != messages {
                failures.push(format!("{at}: messages\n{:#?}\nritsu-proto\n{messages:#?}", rp::messages(&src)));
            }
            golden.push_str(&format!("== {at}\n{}", rulec_lines(&package, &imports, &enums, &messages)));
            n += 1;
        }
    }
    // A module's buf.yaml and buf.lock: dandori's, and the ones rulec's tests wrote.
    let mut bufs = vec![("../dandori".to_string(), "proto/buf.yaml".to_string())];
    let mut more = Vec::new();
    ritsu_base::paths::walk(Path::new("tests/fixtures/rulec/buf"), ".", &[], &mut more);
    bufs.extend(more.into_iter().map(|f| ("tests/fixtures/rulec/buf".to_string(), f)));
    for (root, file) in &bufs {
        let y = text(root, file);
        let at = format!("{root}/{file}");
        if file.ends_with(".lock") {
            let (v, pins) = ritsu_proto::buf::lock(&y);
            let pins: Vec<rp::Pin> = pins.into_iter().map(|p| rp::Pin { name: p.name, commit: p.commit, digest: p.digest }).collect();
            if rp::buf_lock(&y) != (v.clone(), pins.clone()) {
                failures.push(format!("{at}: {:?}, ritsu-proto {:?}", rp::buf_lock(&y), (v.clone(), pins.clone())));
            }
            golden.push_str(&format!("== {at}\nversion {v:?}\n"));
            for p in &pins {
                golden.push_str(&format!("pin {} {} {}\n", p.name, p.commit, p.digest));
            }
        } else {
            let deps = ritsu_proto::buf::deps(&y);
            if rp::buf_deps(&y) != deps {
                failures.push(format!("{at}: {:?}, ritsu-proto {deps:?}", rp::buf_deps(&y)));
            }
            golden.push_str(&format!("== {at}\ndeps {deps:?}\n"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    assert_eq!(refused, REFUSED);
    assert!(n >= 80 && bufs.len() == 6, "{n} files, {} buf files", bufs.len());
    ritsu_testkit::golden("tests/golden/rulec.txt", &golden);
}

// ── dandori ─────────────────────────────────────────────────────────────

/// What dandori's reader gives, a line a fact: the messages and the enums by their full names,
/// each field with its JSON name, its type resolved, whether it is a list, whether it says it is
/// set, and its Protovalidate rules as written; the services with their options; the imports
/// that were not on the disk.
fn dandori_facts(f: &dandori::proto::ProtoFile) -> String {
    let mut out = format!("package {}\n", f.package);
    for (name, fields) in &f.messages {
        out.push_str(&format!("message {name}\n"));
        for x in fields {
            out.push_str(&format!("  field {} json={} ty={:?} repeated={} presence={} rules={}\n", x.name, x.json, x.ty, x.repeated, x.presence, x.rules));
        }
    }
    for (name, values) in &f.enums {
        out.push_str(&format!("enum {name} = {}\n", values.join(" ")));
    }
    for s in &f.services {
        out.push_str(&format!("service {} imports_options={}\n", s.name, s.imports_options));
        for (k, v) in &s.options {
            out.push_str(&format!("  option {k} = {v}\n"));
        }
        for m in &s.methods {
            out.push_str(&format!("  method {} in={} out={} streams={}\n", m.name, m.input, m.output, m.streams));
            for (k, v) in &m.options {
                out.push_str(&format!("    option {k} = {v}\n"));
            }
        }
    }
    out.push_str(&format!("unread {}\n", f.unread.join(" ")));
    out
}

/// The same facts from what ritsu-proto read of a file and everything it imports: what dandori
/// takes from the reader when it reads with it (D.10). dandori reads proto3 only; a type that
/// resolves to nothing is the trouble, unless an import was not read, when it stays as written.
fn as_dandori(ps: &Protos, issues: &[Issue]) -> Result<String, String> {
    for f in &ps.order {
        let pf = &ps.files[f];
        if pf.syntax_line.is_some() && pf.syntax != "proto3" {
            return Err(format!("{f}: `{}` is not read; dandori reads proto3", pf.syntax));
        }
    }
    let mut unread: Vec<String> = Vec::new();
    for i in issues {
        if let Issue::NotFound { import, .. } = i
            && !unread.contains(&import.path)
        {
            unread.push(import.path.clone());
        }
    }
    let lenient = !unread.is_empty();
    let resolve = |file: &str, scope: &str, name: &str| -> Result<String, String> {
        match ps.resolve(file, scope, name) {
            Resolved::Found(s) => Ok(s.full),
            Resolved::Known(k) if WELL_KNOWN.contains(&k.as_str()) => Ok(k),
            _ if lenient => Ok(name.trim_start_matches('.').to_string()),
            _ if name.starts_with('.') => Err(format!("there is no type `{name}`")),
            _ => Err(format!("there is no type `{name}` (in `{scope}`)")),
        }
    };
    let mut enums: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut messages: BTreeMap<String, (&str, &ritsu_proto::Message)> = BTreeMap::new();
    for f in &ps.order {
        let pf = &ps.files[f];
        for e in &pf.enums {
            enums.insert(pf.full(&e.name), e.values.iter().map(|v| v.name.clone()).collect());
        }
        for m in &pf.messages {
            messages.insert(pf.full(&m.name), (f.as_str(), m));
        }
    }
    let mut out = format!("package {}\n", ps.files[&ps.order[0]].package);
    for (full, (file, m)) in &messages {
        out.push_str(&format!("message {full}\n"));
        for x in &m.fields {
            let named = |n: &str| resolve(file, full, n);
            let (ty, resolved) = match &x.ty {
                Type::Scalar(s) => (format!("Scalar({s:?})"), None),
                Type::Named(n) => {
                    let r = named(n)?;
                    (format!("Named({r:?})"), Some(r))
                }
                Type::Map(k, v) => {
                    let v = match v.as_ref() {
                        Type::Scalar(s) => format!("Scalar({s:?})"),
                        Type::Named(n) => format!("Named({:?})", named(n)?),
                        Type::Map(..) => unreachable!(),
                    };
                    (format!("Map(Scalar({k:?}), {v})"), None)
                }
            };
            let repeated = x.label == Label::Repeated && !matches!(x.ty, Type::Map(..));
            let presence = x.label == Label::Optional || x.oneof.is_some() || resolved.is_some_and(|r| !repeated && !enums.contains_key(&r) && r != "google.protobuf.NullValue");
            let rules = ritsu_proto::value::tree(&x.options).get("buf.validate.field").cloned().unwrap_or(ritsu_base::json::Json::Null);
            out.push_str(&format!("  field {} json={} ty={ty} repeated={repeated} presence={presence} rules={}\n", x.name, x.json(), rules.compact()));
        }
    }
    for (name, values) in &enums {
        out.push_str(&format!("enum {name} = {}\n", values.join(" ")));
    }
    let sorted = |opts: &[ritsu_proto::Opt]| -> BTreeMap<String, String> {
        match ritsu_proto::value::tree(opts) {
            ritsu_base::json::Json::Obj(kv) => kv.into_iter().map(|(k, v)| (k, v.compact())).collect(),
            _ => BTreeMap::new(),
        }
    };
    for f in &ps.order {
        let pf = &ps.files[f];
        let imports_options = pf.imports.iter().any(|i| i.path == ritsu_proto::DANDORI_OPTIONS);
        for s in &pf.services {
            out.push_str(&format!("service {} imports_options={imports_options}\n", pf.full(&s.name)));
            for (k, v) in sorted(&s.options) {
                out.push_str(&format!("  option {k} = {v}\n"));
            }
            for m in &s.methods {
                let (i, o) = (resolve(f, &pf.package, &m.input)?, resolve(f, &pf.package, &m.output)?);
                out.push_str(&format!("  method {} in={i} out={o} streams={}\n", m.name, m.client_streaming || m.server_streaming));
                for (k, v) in sorted(&m.options) {
                    out.push_str(&format!("    option {k} = {v}\n"));
                }
            }
        }
    }
    out.push_str(&format!("unread {}\n", unread.join(" ")));
    Ok(out)
}

#[test]
fn dandori_reads_nothing_ritsu_proto_does_not() {
    let mut failures = Vec::new();
    let mut golden = String::new();
    let (mut read, mut refused) = (0, 0);
    for root in ROOTS {
        for file in protos(root) {
            let at = format!("{root}/{file}");
            let theirs = dandori::proto::load(&Path::new(root).join(&file)).map(|f| dandori_facts(&f));
            let ours = match ritsu_proto::load_from(Path::new(root), &file, &[], &[(ritsu_proto::DANDORI_OPTIONS, dandori::proto::DANDORI_OPTIONS)]) {
                Ok((ps, issues)) => as_dandori(&ps, &issues),
                Err((f, e)) => Err(format!("{f}:{}:{}: {}", e.line, e.col, e.message("dandori").get(Lang::En))),
            };
            match (&theirs, &ours) {
                (Ok(a), Ok(b)) if a == b => read += 1,
                // Both refuse it. A type that is nowhere is said the same way; the rest each
                // reader says in its own words.
                (Err(a), Err(b)) if !a.contains("there is no type") || a == b => refused += 1,
                _ => failures.push(format!("{at}:\n--- dandori\n{}\n--- ritsu-proto\n{}", show(&theirs), show(&ours))),
            }
            golden.push_str(&format!("== {at}\n{}\n", show(&ours)));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    assert!(read >= 70, "{read} read, {refused} refused");
    ritsu_testkit::golden("tests/golden/dandori.txt", &golden);
}

fn show(r: &Result<String, String>) -> String {
    match r {
        Ok(s) => s.trim_end().to_string(),
        Err(e) => format!("error: {e}"),
    }
}
