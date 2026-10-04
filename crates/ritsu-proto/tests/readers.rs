//! ritsu-proto reads everything the three readers it took the place of read (PLAN C.9, D.10):
//! sakai's, rulec's and dandori's. Every `.proto` of the three crates and the examples their own
//! tests used (`tests/fixtures/`) is read, put in each language's terms, and held to what that
//! language's own reader gave for it before it read with ritsu-proto: `tests/golden/sakai.txt`,
//! `rulec.txt` and `dandori.txt`. The three readers were compared here, live, until each language
//! read with ritsu-proto (sakai in C.9, rulec and dandori in D.10); the goldens are what is left
//! of that comparison (DESIGN 3.3). A change to what ritsu-proto reads shows in them.

use ritsu_base::text::{Lang, Text};
use ritsu_proto::validate::Rules;
use ritsu_proto::{Issue, Label, Protos, Resolved, Type, WELL_KNOWN};
use std::collections::BTreeMap;
use std::path::Path;

/// The three crates, and this crate's copies of the examples the readers' own tests used.
const ROOTS: [&str; 4] = ["../rulec", "../dandori", "../sakai", "tests/fixtures"];

/// The files no reader reads whole: a statement left open (sakai's E106 mutant, in English and in Japanese, and dandori's
/// file that is not a `.proto`), and a proto2 `group`. ritsu-proto says where the file is wrong
/// (rulec's reader passed over what it did not understand and gave what it found in them).
const REFUSED: [&str; 4] = ["../sakai/tests/mutants/E106_unreadable_proto/proto/shop/billing/v1/billing.proto", "../sakai/tests/mutants/E106_読めない_proto/proto/shop/billing/v1/billing.proto", "tests/fixtures/dandori/bad/specs/money.proto", "tests/fixtures/sakai/group.proto"];

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

/// What rulec takes from a file (its `proto::read`), a line an element: the package, the imports,
/// every enum and message by its own short name, and of a message the fields a path can follow —
/// not a `map`, not a proto2 `required`, a member of a `oneof` having presence of its own, its
/// `json_name` the first one written; of the rules, what is set.
fn rulec_lines(f: &ritsu_proto::ProtoFile) -> String {
    let short = |n: &str| n.rsplit('.').next().unwrap_or(n).to_string();
    let package = (!f.package.is_empty()).then(|| f.package.clone());
    let imports: Vec<String> = f.imports.iter().map(|i| i.path.clone()).collect();
    let mut out = format!("package {package:?} imports {imports:?}\n");
    for e in &f.enums {
        let vs: Vec<String> = e.values.iter().map(|v| format!("{}={}", v.name, v.number)).collect();
        out.push_str(&format!("enum {} {}\n", short(&e.name), vs.join(" ")));
    }
    for m in &f.messages {
        out.push_str(&format!("message {}\n", short(&m.name)));
        for x in m.fields.iter().filter(|x| !matches!(x.ty, Type::Map(..)) && x.label != Label::Required) {
            let ty = match &x.ty {
                Type::Scalar(t) | Type::Named(t) => t.clone(),
                Type::Map(..) => unreachable!(),
            };
            let mut w = vec![format!("  {} {ty}", x.name)];
            if x.label == Label::Repeated {
                w.push("repeated".into());
            }
            if x.label == Label::Optional || x.oneof.is_some() {
                w.push("optional".into());
            }
            let json_name = x.options.iter().find_map(|o| match &o.value {
                Some(ritsu_proto::Value::Str(s)) if o.name.split_whitespace().collect::<String>() == "json_name" => Some(s.clone()),
                _ => None,
            });
            if let Some(j) = &json_name {
                w.push(format!("json_name={j:?}"));
            }
            if let Some(o) = &x.oneof {
                w.push(format!("oneof={o}"));
            }
            w.extend(rules_set(&x.rules));
            out.push_str(&format!("{}\n", w.join(" ")));
        }
        let r = &m.rules;
        if !r.cel.is_empty() || !r.oneofs.is_empty() || r.disabled {
            out.push_str(&format!("  rules cel={:?} oneofs={:?} disabled={}\n", r.cel, r.oneofs.iter().map(|o| (o.fields.clone(), o.required)).collect::<Vec<_>>(), r.disabled));
        }
    }
    out
}

fn rules_set(r: &Rules) -> Vec<String> {
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
fn rulec_reads_what_it_read_before() {
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
            golden.push_str(&format!("== {at}\n{}", rulec_lines(&f)));
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
            golden.push_str(&format!("== {at}\nversion {v:?}\n"));
            for p in &pins {
                golden.push_str(&format!("pin {} {} {}\n", p.name, p.commit, p.digest));
            }
        } else {
            golden.push_str(&format!("== {at}\ndeps {:?}\n", ritsu_proto::buf::deps(&y)));
        }
    }
    assert_eq!(refused, REFUSED);
    assert!(n >= 80 && bufs.len() == 6, "{n} files, {} buf files", bufs.len());
    ritsu_testkit::golden("tests/golden/rulec.txt", &golden);
}

// ── dandori ─────────────────────────────────────────────────────────────

/// What dandori takes from a file and everything it imports (its `proto::load`), a line a fact:
/// the messages and the enums by their full names, each field with its JSON name, its type
/// resolved over the files the file can see, whether it is a list, whether it says it is set,
/// and its Protovalidate rules as written; the services with their options; the imports that
/// were not on the disk. dandori reads proto3 only; a type that resolves to nothing is the
/// trouble, unless an import was not read, when it stays as written.
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
fn dandori_reads_what_it_read_before() {
    let mut golden = String::new();
    let (mut read, mut refused) = (0, 0);
    for root in ROOTS {
        for file in protos(root) {
            let at = format!("{root}/{file}");
            let ours = match ritsu_proto::load_from(Path::new(root), &file, &[], &[(ritsu_proto::DANDORI_OPTIONS, DANDORI_OPTIONS)]) {
                Ok((ps, issues)) => as_dandori(&ps, &issues),
                Err((f, e)) => Err(format!("{f}:{}:{}: {}", e.line, e.col, e.message("dandori").get(Lang::En))),
            };
            match &ours {
                Ok(_) => read += 1,
                Err(_) => refused += 1,
            }
            golden.push_str(&format!("== {at}\n{}\n", show(&ours)));
        }
    }
    assert!(read >= 70, "{read} read, {refused} refused");
    ritsu_testkit::golden("tests/golden/dandori.txt", &golden);
}

/// dandori's options, which a `.proto` imports without the file (dandori's
/// `proto/dandori/v1/options.proto`).
const DANDORI_OPTIONS: &str = include_str!("../../dandori/proto/dandori/v1/options.proto");

fn show(r: &Result<String, String>) -> String {
    match r {
        Ok(s) => s.trim_end().to_string(),
        Err(e) => format!("error: {e}"),
    }
}
