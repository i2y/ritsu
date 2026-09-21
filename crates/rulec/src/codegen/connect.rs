//! The rule as one Connect service (§15.112): a `.proto` beside the module, and what stands
//! between the two — a conversion of twenty-five lines, and a server to try it with.
//!
//! `codegen/tool.rs` is the rule as a tool for an agent. This is the other caller: a service
//! another program calls, over a wire it already speaks. What is generated is three files and
//! no engine — the `.proto`, which is the whole of the contract; the implementation, which
//! converts a message into the module's arguments and its answer back; and a runner, so the
//! service is held to the reference evaluator over the same vectors as everything else.
//!
//! Three decisions shape it.
//!
//! **The dependency stays outside the decision.** The module `rulec gen` writes has no
//! imports at all, and nothing here changes that: `connectrpc` is imported by the service
//! file alone, which a reader can delete without touching the rule. That is also why the
//! service is generated against the *stubs* — `protoc` reads the `.proto` and writes the
//! client, the server base and the messages, and what is left for rulec to write is the
//! conversion.
//!
//! **The method has no side effects, and says so.** A rule is a pure function of its inputs,
//! so `idempotency_level = NO_SIDE_EFFECTS` is not a hint but a fact the checker already
//! proved: the same inputs give the same answer, forever, for one version of the table.
//! Connect lets such a method be called with GET, which is what makes an answer cacheable —
//! and `rulec test` drives both ways, because a transport that changed an answer would be
//! the one thing worth catching.
//!
//! **The rows that matched travel with the answer.** `trace` is the same list the record
//! function writes, so one call is one fixtures record and `--record` turns a running
//! service into the file `rulec replay` and `rulec diff` read (§10.3).

use super::{brand_of, pascal, pub_name, wire_of, Gen, Wire};
use crate::ast::EnumSource;
use crate::types::Ty;

/// The field number of `trace`. It sits far from the outputs so that an output added to a
/// table later takes the next small number and leaves the trace where it was.
const TRACE_FIELD: u32 = 100;

impl<'a> Gen<'a> {
    /// `rulec.<alias>.v<major>`.
    ///
    /// The major version alone: the package names the **wire**, and a rule's later versions
    /// answer the same question in the same shape. What a change to the shape does is exactly
    /// what `buf breaking` is there to say.
    pub fn proto_package(&self) -> String {
        format!("rulec.{}.{}", pub_name(&self.f.name), proto_version(&self.f.version))
    }

    /// `rulec/<alias>/v<major>/<alias>.proto`.
    ///
    /// The path spells the package, which is what buf's `PACKAGE_DIRECTORY_MATCH` asks for, so
    /// the file can be dropped into a buf module as it stands.
    pub fn proto_path(&self) -> String {
        let alias = pub_name(&self.f.name);
        format!("{}/{alias}.proto", self.proto_package().replace('.', "/"))
    }

    /// `<Alias>Service` — buf's `SERVICE_SUFFIX`.
    pub fn proto_service(&self) -> String {
        format!("{}Service", pascal(&pub_name(&self.f.name)))
    }

    /// The enum types that actually cross the wire, in the order they are met: the rule's
    /// inputs, then an element's fields, then its outputs. An enum a table uses only as an
    /// intermediate column is not on the wire and is not declared.
    fn wire_enums(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let push = |t: Ty, out: &mut Vec<String>| {
            let t = if let Ty::Opt(i) = &t { (**i).clone() } else { t };
            if let Ty::Enum(n) = t {
                if !out.contains(&n) {
                    out.push(n);
                }
            }
        };
        for i in &self.f.inputs {
            push(self.ty_of(&i.name.text), &mut out);
        }
        if let Some(el) = &self.f.elements {
            for fd in &el.fields {
                push(self.ty_of(&fd.name.text), &mut out);
            }
        }
        for o in &self.f.outputs {
            push(self.ty_of(&o.name.text), &mut out);
        }
        out
    }

    /// Where an enum's value set is declared, when that is a `.proto` outside this rule
    /// (§15.59): the file as the rule wrote it, the enum's name there, and the package that
    /// file declares. The generated `.proto` imports it rather than declaring the set twice.
    fn foreign(&self, ty: &str) -> Option<(String, String, Option<String>)> {
        let im = self
            .f
            .enum_imports
            .iter()
            .find(|p| p.kind == EnumSource::Proto && p.target.text == ty)?;
        let dir = std::path::Path::new(&self.path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let pkg = std::fs::read_to_string(dir.join(&im.file)).ok().and_then(|s| crate::proto::package(&s));
        Some((im.file.clone(), im.source.clone(), pkg))
    }

    /// The type of one value in the `.proto`.
    fn proto_ty(&self, name: &str, ty: &Ty) -> String {
        match ty {
            Ty::Opt(t) => self.proto_ty(name, t),
            Ty::Enum(n) => match self.foreign(n) {
                Some((_, sel, Some(pkg))) => format!("{pkg}.{sel}"),
                Some((_, sel, None)) => sel,
                None => self.enum_names.get(n).cloned().unwrap_or_else(|| pascal(n)),
            },
            Ty::Bool => "bool".into(),
            // A date is `YYYY-MM-DD` on this wire as on every other of rulec's (§10.2), and a
            // string is the one type that says so without a dependency on `google.type`.
            Ty::Date | Ty::Str => "string".into(),
            _ => "int64".into(),
        }
    }

    /// The comment beside a field: the rule's own name for it, what the number means, and the
    /// range the generated guard will enforce.
    fn proto_note(&self, name: &str, ty: &Ty) -> String {
        let mut s = name.to_string();
        let inner = if let Ty::Opt(t) = ty { t.as_ref() } else { ty };
        let unit = crate::verify::wire_unit(name, inner, self.c);
        if !unit.is_empty() {
            s.push_str(&tr!("：{unit}", ": {unit}"));
            let sc = self.c.wire_scale(name);
            if let (Some(lo), Some(hi)) = self.c.ranges.get(name).copied().unwrap_or((None, None)) {
                s.push_str(&tr!(
                    "。{} から {} まで",
                    ". {} to {}",
                    crate::types::wire_int(lo, sc),
                    crate::types::wire_int(hi, sc)
                ));
            }
        }
        if matches!(inner, Ty::Date) {
            s.push_str(&tr!("：日付。YYYY-MM-DD", ": a date, YYYY-MM-DD"));
        }
        if matches!(ty, Ty::Opt(_)) {
            s.push_str(&tr!("。無い場合は入れない", ". Leave it unset for none"));
        }
        s
    }

    /// One field line of a message.
    ///
    /// The comment is left out when it would only repeat the field's own name, which is what
    /// happens to every field of a rule written in English that carries no unit. A comment
    /// that says nothing teaches the reader to skip the ones that do.
    fn proto_field(&self, name: &str, alias: &str, ty: &Ty, n: u32) -> String {
        let label = if matches!(ty, Ty::Opt(_)) { "optional " } else { "" };
        let note = self.proto_note(name, ty);
        let comment = if note.eq_ignore_ascii_case(alias) { String::new() } else { format!("  // {note}\n") };
        format!("{comment}  {label}{} {alias} = {n};\n", self.proto_ty(name, ty))
    }

    /// The first line of the header alone: what these two files say about themselves.
    ///
    /// They belong to the directory rather than to a rule, so the line naming a rule and its
    /// digest has no place in them — and a header that named one would make the file depend
    /// on which rule was generated last.
    fn plain_header(&self) -> String {
        self.header("#").lines().next().unwrap_or_default().to_string()
    }

    /// `proto/buf.yaml`: the generated tree **is** a buf module.
    pub fn buf_yaml(&self) -> String {
        format!(
            "{}\nversion: v2\nlint:\n  use:\n    - STANDARD\nbreaking:\n  use:\n    - FILE\n",
            self.plain_header()
        )
    }

    /// `proto/buf.gen.yaml`: the stubs, the way connect-python's own documentation asks for
    /// them.
    ///
    /// `buf generate` from this directory writes the messages, their types and the client and
    /// server bases into `../python`, where the generated service imports them. `protoc` does
    /// the same and is in the service's own docstring, for a build that has no buf.
    pub fn buf_gen_yaml(&self) -> String {
        let note = tr!(
            "# 手元だけで作るなら `uv add --dev protoc-gen-py protoc-gen-connectrpc` を入れて、\n  \
             # この二行を `local: protoc-gen-py` と `local: protoc-gen-connectrpc` にする。\n  \
             # そのときは py の側に `strategy: all` も足すこと。規則ごとにディレクトリが分かれるので、\n  \
             # 既定（directory）だとプラグインがディレクトリごとに呼ばれ、同じ __init__.py を何度も書く。",
            "# To generate entirely locally, `uv add --dev protoc-gen-py protoc-gen-connectrpc`\n  \
             # and make these two `local: protoc-gen-py` and `local: protoc-gen-connectrpc`.\n  \
             # Add `strategy: all` to the py one when you do: there is one directory per rule, and\n  \
             # under the default (directory) buf calls the plugin once per directory, which writes\n  \
             # the shared __init__.py again each time."
        );
        format!(
            "{}\n# cd generated/proto && buf generate\nversion: v2\nplugins:\n  \
             {note}\n  \
             - remote: buf.build/bufbuild/py\n    out: ../python/stubs\n  \
             - remote: buf.build/connectrpc/py\n    out: ../python/stubs\n",
            self.plain_header()
        )
    }

    /// `proto/<package as a path>/<alias>.proto`: the whole of what a caller has to agree to.
    pub fn proto(&self) -> String {
        let mut o = self.header("//");
        o.push_str("\nsyntax = \"proto3\";\n\n");
        o.push_str(&format!("package {};\n", self.proto_package()));

        // An enum whose values are declared elsewhere is imported, not copied: the rule cites
        // that file and `rulec check` holds the two together (E032), so the service speaks the
        // contract's own enum rather than a second one that means the same thing.
        let mut imports: Vec<String> = Vec::new();
        for ty in self.wire_enums() {
            if let Some((file, _, _)) = self.foreign(&ty) {
                if !imports.contains(&file) {
                    imports.push(file);
                }
            }
        }
        if !imports.is_empty() {
            o.push('\n');
            for f in &imports {
                o.push_str(&format!("import \"{f}\";\n"));
            }
        }

        for ty in self.wire_enums() {
            if self.foreign(&ty).is_some() {
                continue;
            }
            let name = self.enum_names.get(&ty).cloned().unwrap_or_else(|| pascal(&ty));
            let prefix = crate::proto::upper_snake(&name);
            let head = if ty.eq_ignore_ascii_case(&name) { String::new() } else { format!("// {ty}\n") };
            o.push_str(&format!("\n{head}enum {name} {{\n"));
            // Proto3's zero value is "not set", and it is not one of the rule's values: an
            // input that arrives as 0 has named nothing, and the service refuses it the way
            // the module refuses anything else outside the declared domain.
            o.push_str(&format!("  {prefix}_UNSPECIFIED = 0;\n"));
            for (i, v) in self.c.enums.get(&ty).cloned().unwrap_or_default().iter().enumerate() {
                let al = self.value_names.get(v).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.to_uppercase());
                let note = if v.eq_ignore_ascii_case(&al) { String::new() } else { format!("  // {v}") };
                o.push_str(&format!("  {prefix}_{al} = {};{note}\n", i + 1));
            }
            o.push_str("}\n");
        }

        o.push_str(&tr!(
            "\n// 当てはまった行一つ：表の名前、1 から数えた行番号、ラベルがあればそれ。\n",
            "\n// A row that matched: the table's name, its 1-based row number, and its label when it has one.\n"
        ));
        o.push_str("message Fired {\n  string table = 1;\n  int32 row = 2;\n  string label = 3;\n}\n");

        // The sequence a walk reads is a repeated message of the element's own fields (§15.56).
        if let Some(el) = &self.f.elements {
            o.push_str(&tr!("\n// {} の一件。\n", "\n// One {}.\n", el.name.text));
            o.push_str("message Element {\n");
            for (n, fd) in el.fields.iter().enumerate() {
                o.push_str(&self.proto_field(&fd.name.text, &pub_name(&fd.name), &self.ty_of(&fd.name.text), n as u32 + 1));
            }
            o.push_str("}\n");
        }

        o.push_str(&tr!("\n// 規則 {} v{} の入力。\n", "\n// The inputs of rule {} v{}.\n", self.f.name.text, self.f.version));
        o.push_str("message DecideRequest {\n");
        let mut n = 0u32;
        for i in &self.f.inputs {
            n += 1;
            o.push_str(&self.proto_field(&i.name.text, &pub_name(&i.name), &self.ty_of(&i.name.text), n));
        }
        if let Some(el) = &self.f.elements {
            n += 1;
            o.push_str(&format!(
                "  // {}\n  repeated Element {} = {n};\n",
                el.name.text,
                pub_name(&el.name)
            ));
        }
        o.push_str("}\n");

        o.push_str(&tr!(
            "\n// 規則 {} v{} が決めたことと、決めた行。\n",
            "\n// What rule {} v{} decided, and the rows that decided it.\n",
            self.f.name.text,
            self.f.version
        ));
        o.push_str("message DecideResponse {\n");
        for (k, out) in self.f.outputs.iter().enumerate() {
            o.push_str(&self.proto_field(&out.name.text, &pub_name(&out.name), &self.ty_of(&out.name.text), k as u32 + 1));
        }
        o.push_str(&tr!(
            "\n  // 当てはまった行。表ごとに一つ、順番に。\n",
            "\n  // The rows that matched, one per table, in order.\n"
        ));
        o.push_str(&format!("  repeated Fired trace = {TRACE_FIELD};\n}}\n"));

        let ins: Vec<String> = self.f.inputs.iter().map(|i| i.name.text.clone()).collect();
        let outs: Vec<String> = self.f.outputs.iter().map(|x| x.name.text.clone()).collect();
        let sep = if crate::i18n::ja() { "・" } else { ", " };
        o.push_str(&tr!(
            "\n// 規則 {} v{}。\nservice {} {{\n  // {} から {} を決める。\n  // 規則は純関数なので、この手続きには副作用が無く、GET でも呼べる。\n",
            "\n// Rule {} v{}.\nservice {} {{\n  // Decides {4} from {3}.\n  // The rule is a pure function, so this method has no side effects and can be\n  // called with GET.\n",
            self.f.name.text,
            self.f.version,
            self.proto_service(),
            ins.join(sep),
            outs.join(sep)
        ));
        o.push_str("  rpc Decide(DecideRequest) returns (DecideResponse) {\n    option idempotency_level = NO_SIDE_EFFECTS;\n  }\n}\n");
        o
    }
}

/// `1` → `v1`. The major version, as a proto package component.
fn proto_version(v: &str) -> String {
    let major: String = v.split('.').next().unwrap_or("1").chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if major.is_empty() {
        "v1".to_string()
    } else {
        format!("v{}", major.to_lowercase())
    }
}

impl<'a> Gen<'a> {
    /// The Python module path of the generated stubs.
    ///
    /// They land in a `stubs/` package of their own beside the module rather than loose in
    /// the same directory, because the plugin writes an `__init__.py` at the root of wherever
    /// it generates — and an `__init__.py` over rulec's own flat modules would make the
    /// directory a package, which is not what it is.
    fn stub_pkg(&self) -> String {
        format!("stubs.{}", self.proto_package())
    }

    /// The dict that turns one proto enum value into the module's own, and its reverse when
    /// the enum is also an output. A value's number is its position, and for an imported
    /// enum it is the number the contract gave it.
    fn enum_maps(&self) -> String {
        let mut o = String::new();
        let outs: Vec<String> = self
            .f
            .outputs
            .iter()
            .filter_map(|x| match self.ty_of(&x.name.text) {
                Ty::Enum(n) => Some(n),
                Ty::Opt(t) => match *t {
                    Ty::Enum(n) => Some(n),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        for ty in self.wire_enums() {
            let cls = self.enum_names.get(&ty).cloned().unwrap_or_else(|| pascal(&ty));
            let (qual, prefix): (String, String) = match self.foreign(&ty) {
                // The values are the imported file's, so they are read from the module protoc
                // wrote for **that** file, not from this rule's.
                Some((file, sel, _)) => {
                    let module = ext_module(&file);
                    let name = if module.contains('.') { module.replace('.', "_") } else { module };
                    (format!("{name}.{sel}"), crate::proto::upper_snake(&sel))
                }
                None => (format!("pb.{cls}"), crate::proto::upper_snake(&cls)),
            };
            let values = self.c.enums.get(&ty).cloned().unwrap_or_default();
            let pairs: Vec<String> = values
                .iter()
                .map(|v| {
                    let al = self.value_names.get(v).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.to_uppercase());
                    format!("    {qual}.{al}: m.{cls}.{al},\n")
                })
                .collect();
            o.push_str(&format!("ENUM_{}: dict[{qual}, m.{cls}] = {{\n{}}}\n\n", crate::proto::upper_snake(&cls), pairs.concat()));
            if outs.contains(&ty) {
                o.push_str(&format!(
                    "_PB_{0}: dict[m.{cls}, {qual}] = {{v: k for k, v in ENUM_{0}.items()}}\n\n",
                    crate::proto::upper_snake(&cls)
                ));
            }
        }
        o
    }

    /// One input, read off the request message as the module's argument.
    fn from_request(&self, name: &str, alias: &str, ty: &Ty) -> String {
        let src = format!("request.{alias}");
        match ty {
            Ty::Opt(inner) => {
                format!("None if not request.has_field({alias:?}) else {}", self.from_request(name, alias, inner))
            }
            Ty::Enum(n) => {
                let cls = self.enum_names.get(n).cloned().unwrap_or_else(|| pascal(n));
                format!("_member({name:?}, {cls:?}, ENUM_{}, {src})", crate::proto::upper_snake(&cls))
            }
            Ty::Date => format!("_ord({name:?}, {src})"),
            Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("m.{}({src})", brand_of(ty)),
            _ => src,
        }
    }

    /// One output, as a keyword of the response message. An absent optional is `None`, which
    /// is what protobuf-py reads as "not set".
    fn into_response(&self, alias: &str, ty: &Ty, expr: &str) -> String {
        let one = |body: String| format!("        {alias}={body},\n");
        match ty {
            Ty::Opt(inner) => {
                let inner_expr = self.into_response(alias, inner, expr);
                let body = inner_expr.trim().trim_start_matches(&format!("{alias}=")).trim_end_matches(',').to_string();
                one(format!("None if {expr} is None else {body}"))
            }
            Ty::Enum(n) => {
                let cls = self.enum_names.get(n).cloned().unwrap_or_else(|| pascal(n));
                one(format!("_PB_{}[{expr}]", crate::proto::upper_snake(&cls)))
            }
            Ty::Date => one(format!("_civil({expr})")),
            _ => one(expr.to_string()),
        }
    }

    /// Whether any value on the wire is a date, in either direction.
    fn has_date(&self, dir: Dir) -> bool {
        let ins = self
            .f
            .inputs
            .iter()
            .map(|i| i.name.text.clone())
            .chain(self.f.elements.iter().flat_map(|el| el.fields.iter().map(|fd| fd.name.text.clone())));
        let outs = self.f.outputs.iter().map(|o| o.name.text.clone());
        let names: Vec<String> = match dir {
            Dir::In => ins.collect(),
            Dir::Out => outs.collect(),
        };
        names.iter().any(|n| matches!(wire_of(&self.ty_of(n)), Wire::Date))
    }

    /// `python/<alias>_service.py`: the rule behind a Connect endpoint.
    pub fn py_connect(&self) -> String {
        let alias = pub_name(&self.f.name);
        let svc = self.proto_service();
        let mut args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("        {},\n", self.from_request(&i.name.text, &pub_name(&i.name), &self.ty_of(&i.name.text))))
            .collect();
        if let Some(el) = &self.f.elements {
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| self.from_request(&fd.name.text, &pub_name(&fd.name), &self.ty_of(&fd.name.text)).replace("request.", "e."))
                .collect();
            args.push(format!(
                "        [m.Element({}) for e in request.{}],\n",
                fields.join(", "),
                pub_name(&el.name)
            ));
        }
        // A tuple of one is written on its line, comma and all; the formatter would fold a
        // three-line one back anyway (§15.105).
        let args: String = if args.len() == 1 {
            format!("    args = ({},)\n", args[0].trim().trim_end_matches(','))
        } else {
            format!("    args = (\n{}    )\n", args.concat())
        };
        let outs: String = if self.f.outputs.len() == 1 {
            let o = &self.f.outputs[0];
            self.into_response(&pub_name(&o.name), &self.ty_of(&o.name.text), "out")
        } else {
            self.f
                .outputs
                .iter()
                .map(|o| {
                    let a = pub_name(&o.name);
                    self.into_response(&a, &self.ty_of(&o.name.text), &format!("out.{a}"))
                })
                .collect()
        };
        // The stubs of an imported enum live in the module protoc wrote for **its** file.
        let mut ext: Vec<String> = Vec::new();
        for ty in self.wire_enums() {
            if let Some((file, _, _)) = self.foreign(&ty) {
                let module = ext_module(&file);
                // A module under a directory is imported under a flat name; a flat one needs
                // no second spelling.
                let line = match module.contains('.') {
                    true => format!("import {module} as {}\n", module.replace('.', "_")),
                    false => format!("import {module}\n"),
                };
                if !ext.contains(&line) {
                    ext.push(line);
                }
            }
        }
        let doc = tr!(
            "規則 {} v{} を、Connect のサービス一つとして出す。\n\n    uvicorn {alias}_service:app --port 8080       # ASGI\n    gunicorn '{alias}_service:wsgi_app'           # WSGI\n    python3 {alias}_service.py --http 127.0.0.1:8080 [--record calls.jsonl]   # 標準ライブラリだけで試す\n\nTLS と認証は前に置くこと。このサーバは自分では持たない。\n\n呼ぶ側と交わすのは隣の `.proto` だけで、その stub は buf が書く:\n\n    uv add connectrpc\n    cd ../proto && buf generate\n\n決めているのは隣のモジュールで、そちらは何にも依存しない。ここにあるのはワイヤだけである。\n返す値には当てはまった行が付く。一回の呼び出しが記録一件で、--record を付ければ\nその一行がファイルに溜まり、rulec replay と rulec diff がそのまま読む。",
            "Rule {} v{} as one Connect service.\n\n    uvicorn {alias}_service:app --port 8080       # ASGI\n    gunicorn '{alias}_service:wsgi_app'           # WSGI\n    python3 {alias}_service.py --http 127.0.0.1:8080 [--record calls.jsonl]   # the standard library alone\n\nPut TLS and authentication in front: this server carries neither.\n\nThe only thing a caller agrees to is the `.proto` beside it, whose stubs buf writes:\n\n    uv add connectrpc\n    cd ../proto && buf generate\n\nThe deciding is the module next to this file, which depends on nothing; what is here is the\nwire. The answer carries the rows that matched, so one call is one record: with --record that\nline is appended to a file, and rulec replay and rulec diff read it as it stands.",
            self.f.name.text,
            self.f.version,
            alias = alias
        );
        PY_SERVICE
            .replace("@HEADER@", self.header("#").trim_end())
            .replace("@DOC@", &doc)
            .replace("@ALIAS@", &alias)
            .replace("@CLASS@", &pascal(&alias))
            .replace("@SVC@", &svc)
            .replace("@PKG@", &self.stub_pkg())
            .replace("@EXT@", &ext.concat())
            .replace("@SHA@", &self.src_hash)
            .replace(
                "@MEMBER@",
                &if self.wire_enums().is_empty() {
                    String::new()
                } else {
                    PY_SERVICE_MEMBER.replace("@MAPS@", self.enum_maps().trim_end())
                },
            )
            .replace(
                "@IMPORTS@",
                &{
                    let mut v: Vec<&str> = Vec::new();
                    if !self.wire_enums().is_empty() {
                        v.push("import enum\n");
                    }
                    if self.has_date(Dir::In) || self.has_date(Dir::Out) {
                        v.push("import datetime\n");
                    }
                    let mut s: Vec<&str> = v.clone();
                    s.sort_unstable();
                    s.concat()
                },
            )
            .replace("@TYPEVAR@", if self.wire_enums().is_empty() { "" } else { ", Any, TypeVar" })
            .replace("@MAPPING@", if self.wire_enums().is_empty() { "" } else { ", Mapping" })
            .replace("@ARGS@", &args)
            .replace("@OUTS@", &outs)
            .replace(
                "@DATES@",
                if self.has_date(Dir::In) || self.has_date(Dir::Out) { PY_SERVICE_DATES } else { "" },
            )
            .replace("@D_SHA@", &tr!("どの版の表が答えたか。答えを保つ呼び出し側のために、返す見出しに入れる。", "Which version of the table answered; it goes in a response header for a caller that keeps the answer."))
            .replace("@D_MEMBER@", &tr!("列挙の値一つ。契約に無い番号は、呼び出し側の契約違反である。", "One value of an enum. A number the contract does not have is a contract violation by the caller."))
            .replace("@M_MEMBER@", &tr!("{{ty}} の値ではありません: {{v}}", "not a value of {{ty}}: {{v}}"))
            .replace("@D_ORD@", &tr!("日付は YYYY-MM-DD の文字列で受け、日数に直す。", "A date arrives as a YYYY-MM-DD string and is read as a day number."))
            .replace("@M_ORD@", &tr!("日付は YYYY-MM-DD で: {{v!r}}", "not a YYYY-MM-DD date: {{v!r}}"))
            .replace("@D_CIVIL@", &tr!("日数を YYYY-MM-DD に戻す。", "A day number as YYYY-MM-DD."))
            .replace("@D_ASYNC@", &tr!("ASGI の側。規則は純関数なので待つものが無く、中身は下の同期の側と同じ一本である。", "The ASGI side. A rule is a pure function with nothing to await, so this and the sync class below are two doors on one body."))
            .replace("@D_SYNC@", &tr!("WSGI の側。", "The WSGI side."))
            .replace("@D_CALL@", &tr!("一回の呼び出しが一回の判断で、二つの間に残るものは無い。", "One call is one decision, and nothing is kept between two of them."))
            .replace("@D_DECIDE@", &tr!("受け取った件に規則を当て、決めた値と決めた行を返す。", "Apply the rule to the case that arrived, and answer with what it decided and the rows that decided it."))
            .replace("@D_INVALID@", &tr!("宣言した入力の外は、呼び出し側の契約違反である（§8.1）。", "Outside the declared input domain is a contract violation by the caller (§8.1)."))
            .replace("@D_INTERNAL@", &tr!("静的に証明できなかった重なりに、実際に当たった（W114）。どちらの行を採るかは表が決めることで、\n        # 呼び出し側には直せない。だから 500 で、気づかれるべきものとして返す。", "An overlap the checker could not settle statically was actually hit (W114). Which row wins is\n        # the table's to decide and the caller cannot fix it, so it answers 500: something to notice."))
            .replace("@D_APP@", &tr!("ASGI アプリとしてのサービス。uvicorn・hypercorn・daphne のどれでも動く。", "The service as an ASGI application, for uvicorn, hypercorn or daphne."))
            .replace("@D_WSGI@", &tr!("同じサービスの WSGI 版。gunicorn・uWSGI のような同期のサーバ向け。", "The same service as a WSGI application, for a synchronous server such as gunicorn or uWSGI."))
            .replace("@D_SERVER@", &tr!("標準ライブラリだけで、WSGI の側を立てる。何も入れずに試すための道で、本番は\n    uvicorn か gunicorn に上の app を渡すこと。番号に 0 を渡すと空いているものが取られる。", "The WSGI side, served by the standard library alone: the way to try it with nothing\n    installed. In production, hand `app` to uvicorn or `wsgi_app` to gunicorn. A port of 0\n    takes any free one."))
            .replace("@D_QUIET@", &tr!("アクセスログは出さない。通信の話であって、この規則が言うことではない。", "No access log: that is the transport talking, not this rule."))
            .replace("@D_MAIN@", &tr!("引数を読んで待ち受ける。", "Read the arguments and listen."))
            .replace("@D_LIMITED@", &tr!(
                "本文の終わりで止まる入力。\n\n    PEP 3333 は、本文の長さで終わる入力をアプリに渡すことをサーバに勧めているが、標準ライブラリの\n    サーバはそうしない。wsgi.input はソケットそのもので、本文の先を読もうとすると、呼び出し側が\n    次を送るまで止まる。呼び出し側は答えを待っているので、何も来ない。本番の WSGI サーバはどれも\n    長さで切っている。標準ライブラリのものを同じ振る舞いにするのが、この一枚である。",
                "The request body, and not one byte past it.\n\n    PEP 3333 says a server *should* hand the application an input stream that ends at\n    CONTENT_LENGTH, and the standard library's server does not: its wsgi.input is the socket, so\n    a read past the body waits for the caller to send more. The caller is waiting for the answer,\n    so nothing comes. Every production WSGI server limits the stream; this is what makes the\n    standard library's behave like them."))
            .replace("@D_LIMIT@", &tr!("その入力を渡すだけの包み。", "The wrapper that hands that input over."))
    }
}

/// Which way a value crosses the wire.
#[derive(Clone, Copy)]
enum Dir {
    In,
    Out,
}

/// `api/v1/order.proto` → `api.v1.order_pb`, the module protobuf-py's plugin writes for it.
fn ext_module(file: &str) -> String {
    file.trim_end_matches(".proto").replace(['/', '\\'], ".") + "_pb"
}

/// The enum tables and the one reader of them, for a rule that has an enum on the wire.
const PY_SERVICE_MEMBER: &str = r#"
E = TypeVar("E", bound=enum.Enum)

@MAPS@


def _member(name: str, ty: str, table: Mapping[Any, E], v: int) -> E:
    """@D_MEMBER@"""
    try:
        return table[v]
    except KeyError:
        raise ConnectError(Code.INVALID_ARGUMENT, f"{name}: @M_MEMBER@") from None
"#;

const PY_SERVICE_DATES: &str = r#"

def _ord(name: str, v: str) -> int:
    """@D_ORD@"""
    try:
        y, mo, d = (int(x) for x in v.split("-"))
        return (datetime.date(y, mo, d) - datetime.date(1970, 1, 1)).days
    except ValueError:
        raise ConnectError(Code.INVALID_ARGUMENT, f"{name}: @M_ORD@") from None


def _civil(n: int) -> str:
    """@D_CIVIL@"""
    return (datetime.date(1970, 1, 1) + datetime.timedelta(days=n)).isoformat()
"#;

const PY_SERVICE: &str = r#"@HEADER@
"""@DOC@
"""

from __future__ import annotations

@IMPORTS@import io
import sys
from typing import TYPE_CHECKING@TYPEVAR@

from connectrpc.code import Code
from connectrpc.errors import ConnectError

import @ALIAS@ as m
@EXT@from @PKG@ import @ALIAS@_pb as pb
from @PKG@.@ALIAS@_connect import (
    @SVC@,
    @SVC@ASGIApplication,
    @SVC@Sync,
    @SVC@WSGIApplication,
)

if TYPE_CHECKING:
    from collections.abc import Iterable@MAPPING@
    from typing import IO
    from wsgiref.simple_server import WSGIServer
    from wsgiref.types import StartResponse, WSGIApplication, WSGIEnvironment

    from connectrpc.request import RequestContext

# @D_SHA@
SOURCE_SHA256 = "@SHA@"
@MEMBER@@DATES@

def decide(
    request: pb.DecideRequest,
    ctx: RequestContext[pb.DecideRequest, pb.DecideResponse],
    record: str | None = None,
) -> pb.DecideResponse:
    """@D_DECIDE@"""
@ARGS@    try:
        out, trace = m.@ALIAS@_traced(*args)
    except m.RuleInputError as e:
        # @D_INVALID@
        raise ConnectError(Code.INVALID_ARGUMENT, str(e)) from None
    except m.RuleContradictionError as e:
        # @D_INTERNAL@
        raise ConnectError(Code.INTERNAL, str(e)) from None
    ctx.response_headers["rulec-source-sha256"] = SOURCE_SHA256
    if record is not None:
        with open(record, "a", encoding="utf-8") as f:
            f.write(m.@ALIAS@_record(*args, out, trace) + "\n")
    return pb.DecideResponse(
@OUTS@        trace=[pb.Fired(table=f.table, row=f.row, label=f.label) for f in trace],
    )


class @CLASS@(@SVC@):
    """@D_ASYNC@"""

    def __init__(self, record: str | None = None) -> None:
        self.record = record

    async def decide(
        self, request: pb.DecideRequest, ctx: RequestContext[pb.DecideRequest, pb.DecideResponse]
    ) -> pb.DecideResponse:
        """@D_CALL@"""
        return decide(request, ctx, self.record)


class @CLASS@Sync(@SVC@Sync):
    """@D_SYNC@"""

    def __init__(self, record: str | None = None) -> None:
        self.record = record

    def decide(
        self, request: pb.DecideRequest, ctx: RequestContext[pb.DecideRequest, pb.DecideResponse]
    ) -> pb.DecideResponse:
        """@D_CALL@"""
        return decide(request, ctx, self.record)


def application(record: str | None = None) -> @SVC@ASGIApplication:
    """@D_APP@"""
    return @SVC@ASGIApplication(@CLASS@(record))


def wsgi_application(record: str | None = None) -> @SVC@WSGIApplication:
    """@D_WSGI@"""
    return @SVC@WSGIApplication(@CLASS@Sync(record))


# uvicorn @ALIAS@_service:app --port 8080
app = application()

# gunicorn '@ALIAS@_service:wsgi_app'
wsgi_app = wsgi_application()


class _Limited(io.RawIOBase):
    """@D_LIMITED@"""

    def __init__(self, inner: IO[bytes], left: int) -> None:
        self._inner, self._left = inner, left

    def read(self, size: int | None = -1) -> bytes:
        if self._left <= 0:
            return b""
        if size is None or size < 0:
            size = self._left
        b = self._inner.read(min(size, self._left))
        self._left -= len(b)
        return b

    def readline(self, size: int | None = -1) -> bytes:
        return self.read(size)


def _limit(wsgi: WSGIApplication) -> WSGIApplication:
    """@D_LIMIT@"""

    def wrapped(environ: WSGIEnvironment, start_response: StartResponse) -> Iterable[bytes]:
        environ["wsgi.input"] = _Limited(environ["wsgi.input"], int(environ.get("CONTENT_LENGTH") or 0))
        return wsgi(environ, start_response)

    return wrapped


def server(host: str, port: int, record: str | None = None) -> WSGIServer:
    """@D_SERVER@"""
    from wsgiref.simple_server import WSGIRequestHandler, make_server

    class Quiet(WSGIRequestHandler):
        def log_message(self, *a: object) -> None:
            """@D_QUIET@"""

    return make_server(host, port, _limit(wsgi_application(record)), handler_class=Quiet)


def main(argv: list[str]) -> None:
    """@D_MAIN@"""
    at, record, i = "127.0.0.1:8080", None, 0
    while i < len(argv):
        if argv[i] == "--http" and i + 1 < len(argv):
            at, i = argv[i + 1], i + 2
        elif argv[i] == "--record" and i + 1 < len(argv):
            record, i = argv[i + 1], i + 2
        else:
            raise SystemExit(f"unknown argument: {argv[i]}")
    host, _, port = at.rpartition(":")
    host = host or "127.0.0.1"
    srv = server(host, int(port), record)
    # A port of 0 means any free one, so where it is listening is printed as one line.
    print(f"http://{host}:{srv.server_address[1]}", flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main(sys.argv[1:])
"#;

impl<'a> Gen<'a> {
    /// One value of the request message, built from a line of the vectors.
    fn to_request(&self, name: &str, ty: &Ty) -> String {
        let src = format!("d[{name:?}]");
        match ty {
            Ty::Opt(inner) => self.to_request(name, inner),
            Ty::Enum(n) => {
                let cls = self.enum_names.get(n).cloned().unwrap_or_else(|| pascal(n));
                format!("_PB_{}[m.{cls}({src})]", crate::proto::upper_snake(&cls))
            }
            Ty::Bool => format!("bool({src})"),
            Ty::Date | Ty::Str => format!("str({src})"),
            _ => format!("int({src})"),
        }
    }

    /// One value of the answer, read back as the module's own, so that the record this
    /// prints is the record the module would have written.
    fn from_response(&self, alias: &str, ty: &Ty) -> String {
        let src = format!("res.{alias}");
        match ty {
            Ty::Opt(inner) => format!("(None if not res.has_field({alias:?}) else {})", self.from_response(alias, inner)),
            Ty::Enum(n) => {
                let cls = self.enum_names.get(n).cloned().unwrap_or_else(|| pascal(n));
                format!("ENUM_{}[{src}]", crate::proto::upper_snake(&cls))
            }
            Ty::Date => format!("_ord({src})"),
            Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("m.{}({src})", brand_of(ty)),
            _ => src,
        }
    }

    /// `python/<alias>_connect_runner.py`: the same vectors as the runner beside it, reached
    /// over the wire.
    ///
    /// It exists so that the service is inside the word "proved" like everything else `gen`
    /// writes: `rulec test` puts every vector through it and holds the record it prints to
    /// the reference evaluator, byte for byte (§15.112).
    pub fn py_connect_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let svc = self.proto_service();
        let mut kw = String::new();
        for i in &self.f.inputs {
            let a = pub_name(&i.name);
            let ty = self.ty_of(&i.name.text);
            let one = self.to_request(&i.name.text, &ty);
            if matches!(ty, Ty::Opt(_)) {
                kw.push_str(&format!("    if d[{:?}] is not None:\n        kw[{a:?}] = {one}\n", i.name.text));
            } else {
                kw.push_str(&format!("    kw[{a:?}] = {one}\n"));
            }
        }
        if let Some(el) = &self.f.elements {
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| format!("{}={}", pub_name(&fd.name), self.to_request(&fd.name.text, &self.ty_of(&fd.name.text)).replace("d[", "e[")))
                .collect();
            kw.push_str(&format!(
                "    kw[{:?}] = [pb.Element({}) for e in d[{:?}]]\n",
                pub_name(&el.name),
                fields.join(", "),
                el.name.text
            ));
        }
        // The arguments are read from the same line the request was built from, because the
        // record names its inputs as the rule does and the message names them by their alias.
        let mut args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| self.py_read(&self.ty_of(&i.name.text), format!("d[{:?}]", i.name.text)))
            .collect();
        let mut prelude = String::new();
        if let Some(el) = &self.f.elements {
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| self.py_read(&self.ty_of(&fd.name.text), format!("e[{:?}]", fd.name.text)))
                .collect();
            prelude = format!("            rows = [m.Element({}) for e in d[{:?}]]\n", fields.join(", "), el.name.text);
            args.push("rows".to_string());
        }
        let out = if self.f.outputs.len() == 1 {
            let o = &self.f.outputs[0];
            self.from_response(&pub_name(&o.name), &self.ty_of(&o.name.text))
        } else {
            format!(
                "m.Output({})",
                self.f
                    .outputs
                    .iter()
                    .map(|o| self.from_response(&pub_name(&o.name), &self.ty_of(&o.name.text)))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let tables: Vec<String> = self
            .wire_enums()
            .iter()
            .map(|ty| format!("ENUM_{}", crate::proto::upper_snake(self.enum_names.get(ty).unwrap_or(ty))))
            .collect();
        // `app` is the ASGI application and `server` the standard library's WSGI one: this
        // runner drives both, because both are generated and a door nobody drove would be a
        // claim nobody checked.
        let mut names: Vec<String> = tables.clone();
        names.push("app".into());
        names.push("server".into());
        names.sort();
        let imports = format!("from {alias}_service import {}", names.join(", "));
        let reverse: String = tables
            .iter()
            .map(|t| format!("_PB_{} = {{v: k for k, v in {t}.items()}}\n", t.trim_start_matches("ENUM_")))
            .collect();
        let doc = tr!(
            "隣の {alias}_runner.py と同じベクタを、Connect 越しに通す。\n\n    python3 {alias}_connect_runner.py [--wsgi] [--get] [--at http://host:port] < vectors.jsonl\n\n--at が無ければ、生成したサービスを空いている番号で立ててそれを呼ぶ。既定は ASGI（uvicorn）で、\n--wsgi なら標準ライブラリのサーバに立てる。出す記録は返ってきたものから組み立てるので、\n隣の runner と食い違えば、それはワイヤが作った差である。--get は同じ呼び出しを GET で行う。\n規則は純関数で、手続きは副作用が無いと宣言してあるから。",
            "The same vectors as {alias}_runner.py beside it, put through Connect.\n\n    python3 {alias}_connect_runner.py [--wsgi] [--get] [--at http://host:port] < vectors.jsonl\n\nWith no --at it stands the generated service up on a free port and calls that: ASGI under\nuvicorn by default, or the standard library's WSGI server with --wsgi. The record it prints is\nbuilt from what came back, so a difference between this and the runner beside it is a\ndifference the wire made. --get makes the same call with GET, which the method allows because\nthe rule is a pure function.",
            alias = alias
        );
        PY_CONNECT_RUNNER
            .replace("@HEADER@", self.header("#").trim_end())
            .replace("@DOC@", &doc)
            .replace("@ALIAS@", &alias)
            .replace("@SVC@", &svc)
            .replace("@PKG@", &self.stub_pkg())
            .replace("@IMPORTS@", &imports)
            // Whatever these two blocks hold, what stands before `def _request` is two blank
            // lines and no more: an empty placeholder used to leave four.
            .replace(
                "@PRE@",
                &{
                    let mut pre = String::new();
                    if !reverse.trim().is_empty() {
                        pre.push('\n');
                        pre.push_str(reverse.trim_end());
                        pre.push('\n');
                    }
                    if self.has_date(Dir::In) || self.has_date(Dir::Out) {
                        pre.push_str(PY_RUNNER_DATES);
                    }
                    pre
                },
            )
            .replace("@KW@", kw.trim_end())
            .replace("@PRELUDE@", &prelude)
            .replace("@STDLIB@", if self.has_date(Dir::In) || self.has_date(Dir::Out) { "import datetime\n" } else { "" })
            // One argument keeps the comma a tuple of one needs; two or more must not have it.
            .replace("@ARGS@", &if args.len() == 1 { format!("{},", args[0]) } else { args.join(", ") })
            .replace("@OUT@", &out)
            .replace("@D_REQUEST@", &tr!("ベクタ一行を、要求のメッセージにする。", "One line of the vectors as the request message."))
            .replace("@D_SERVE@", &tr!("WSGI の側を、標準ライブラリのサーバで、この同じプロセスの空いている番号に立てる。", "The WSGI side, on a free port in this same process, served by the standard library."))
            .replace("@D_ASGI@", &tr!("ASGI の側を uvicorn で立てる。ソケットはこちらで作って渡すので、どの番号になったかが分かる。", "The ASGI side, under uvicorn. The socket is bound here and handed over, so which port it took is known."))
    }
}

const PY_RUNNER_DATES: &str = r#"

def _ord(s: str) -> int:
    y, mo, d = (int(x) for x in s.split("-"))
    return (datetime.date(y, mo, d) - datetime.date(1970, 1, 1)).days
"#;


const PY_CONNECT_RUNNER: &str = r#"@HEADER@
"""@DOC@
"""

from __future__ import annotations

@STDLIB@import json
import sys
import threading
import time
from typing import Any

import @ALIAS@ as m
from @PKG@ import @ALIAS@_pb as pb
from @PKG@.@ALIAS@_connect import @SVC@ClientSync
@IMPORTS@
@PRE@

def _request(d: dict[str, Any]) -> pb.DecideRequest:
    """@D_REQUEST@"""
    kw: dict[str, Any] = {}
@KW@
    return pb.DecideRequest(**kw)


def _serve_asgi() -> str:
    """@D_ASGI@"""
    import socket

    import uvicorn

    sock = socket.socket()
    sock.bind(("127.0.0.1", 0))
    port = int(sock.getsockname()[1])
    srv = uvicorn.Server(uvicorn.Config(app, log_level="error"))
    threading.Thread(target=lambda: srv.run(sockets=[sock]), daemon=True).start()
    while not srv.started:
        time.sleep(0.01)
    return f"http://127.0.0.1:{port}"


def _serve_wsgi() -> str:
    """@D_SERVE@"""
    srv = server("127.0.0.1", 0)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return f"http://127.0.0.1:{srv.server_address[1]}"


def main(argv: list[str]) -> None:
    use_get = "--get" in argv
    wsgi = "--wsgi" in argv
    argv = [a for a in argv if a not in ("--get", "--wsgi", "--asgi")]
    at = argv[argv.index("--at") + 1] if "--at" in argv else (_serve_wsgi() if wsgi else _serve_asgi())
    with @SVC@ClientSync(at) as client:
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            d = json.loads(line)["in"]
@PRELUDE@            args = (@ARGS@)
            res = client.decide(_request(d), use_get=use_get)
            trace = [m.Fired(f.table, f.row, f.label) for f in res.trace]
            print(m.@ALIAS@_record(*args, @OUT@, trace))


if __name__ == "__main__":
    main(sys.argv[1:])
"#;

impl<'a> Gen<'a> {
    /// The `connect` entry of `rulec api`: the wire a caller needs, without reading the
    /// `.proto` — the endpoint's path, the two message names, and what each field is called
    /// there.
    pub fn api_connect(&self) -> String {
        let alias = pub_name(&self.f.name);
        let svc = self.proto_service();
        let pkg = self.proto_package();
        let field = |name: &str, n: &crate::ast::Name, ty: &Ty| -> String {
            crate::json::Obj::new()
                .str("name", name)
                .str("field", &pub_name(n))
                .str("type", &self.proto_ty(name, ty))
                .finish()
        };
        let mut ins: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| field(&i.name.text, &i.name, &self.ty_of(&i.name.text)))
            .collect();
        if let Some(el) = &self.f.elements {
            ins.push(
                crate::json::Obj::new()
                    .str("name", &el.name.text)
                    .str("field", &pub_name(&el.name))
                    .str("type", "repeated Element")
                    .finish(),
            );
        }
        let outs: Vec<String> = self
            .f
            .outputs
            .iter()
            .map(|o| field(&o.name.text, &o.name, &self.ty_of(&o.name.text)))
            .collect();
        let python = crate::json::Obj::new()
            .str("module", &format!("{alias}_service.py"))
            // Two applications and the two classes behind them: the rule is a pure function,
            // so the async one and the sync one are two doors on one body.
            .str("class", &pascal(&alias))
            .str("sync_class", &format!("{}Sync", pascal(&alias)))
            .str("asgi", "app")
            .str("wsgi", "wsgi_app")
            .str("serve_asgi", &format!("uvicorn {alias}_service:app --port 8080"))
            .str("serve_wsgi", &format!("gunicorn '{alias}_service:wsgi_app'"))
            .str("client", &format!("{svc}ClientSync"))
            .str("runner", &format!("{alias}_connect_runner.py"))
            .raw("needs", crate::json::strs(&["connectrpc", "buf"]))
            .finish();
        crate::json::Obj::new()
            .str("proto", &format!("proto/{}", self.proto_path()))
            .str("package", &pkg)
            .str("service", &svc)
            .str("method", "Decide")
            .str("path", &format!("/{pkg}.{svc}/Decide"))
            .str("request", "DecideRequest")
            .str("response", "DecideResponse")
            .str("idempotency_level", "NO_SIDE_EFFECTS")
            .str("trace", "trace")
            .str("source_header", "rulec-source-sha256")
            // buf, configured by the two files beside the `.proto`. protoc has no path here:
            // the messages are protobuf-py's, whose plugin buf is the way to reach.
            .str("stubs", "cd proto && buf generate")
            .str("buf_yaml", "proto/buf.yaml")
            .str("buf_gen_yaml", "proto/buf.gen.yaml")
            // Protobuf's own JSON mapping, which is what a caller sees who speaks the wire
            // by hand rather than through a generated client: names in lowerCamelCase, and an
            // int64 as a string, because a double cannot hold one.
            .str("json_names", "lowerCamelCase")
            .str("json_int64", "string")
            .raw("request_fields", crate::json::arr(&ins))
            .raw("response_fields", crate::json::arr(&outs))
            .raw("python", python)
            .finish()
    }
}

/// `rulec adapter --template connect-python`: the adapter protocol in front of a Connect
/// service that is already running (§10.1, §15.112).
///
/// The counterpart of `verify::template`'s two, for the legacy implementation that is not a
/// process to start but an endpoint to call. Its messages are the other team's, so the two
/// places that cannot be written for them are marked: the request their method takes, and
/// where their answer is. Everything around those — the handshake, the ids, the refusal that
/// keeps an unanswerable record out of the denominator — is written.
pub fn template(f: &crate::ast::RuleFile) -> String {
    let ins: Vec<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
    let out = f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
    let sep = if crate::i18n::ja() { "・" } else { ", " };
    tr!(
        "# rulec のアダプタのテンプレート（規則 {}）。いま動いているのが Connect のサービスのとき。\n\
         #\n\
         #   rulec verify {}.rule --adapter python3 adapter.py https://pricing.internal\n\
         #\n\
         # 標準入出力で JSON Lines をやりとりし、受けた入力をそのサービスに投げる。\n\
         # 要るのは `uv add connectrpc` と、相手の `.proto` から作った stub。\n\
         # 埋めるのは二か所、相手のメッセージだけである。\n\
         import json\n\
         import sys\n\n\
         from connectrpc.errors import ConnectError\n\n\
         from their_pb2 import TheirRequest  # TODO: 相手の .proto から生成した stub\n\
         from their_connect import TheirServiceClientSync  # TODO: 同上\n\n\
         AT = sys.argv[1] if len(sys.argv) > 1 else \"http://127.0.0.1:8080\"\n\n\
         sys.stdin.readline()  # 握手\n\
         print(json.dumps({{\"ok\": True, \"impl\": f\"connect@{{AT}}\"}}), flush=True)\n\n\
         with TheirServiceClientSync(AT) as client:\n    \
             for line in sys.stdin:\n        \
                 line = line.strip()\n        \
                 if not line:\n            \
                     continue\n        \
                 req = json.loads(line)\n        \
                 d = req[\"in\"]  # 入力は {}。値は宣言した単位の整数、日付は YYYY-MM-DD、列挙はその名前\n\n        \
                 try:\n            \
                     # TODO: d から相手の要求を組み立てる\n            \
                     res = client.decide(TheirRequest())\n            \
                     # TODO: 相手の答えの中で {} に当たるところ\n            \
                     got = 0\n        \
                 except ConnectError as e:\n            \
                     # 答えられないと言った件は一致率の分母から外れる（docs/formats.md）\n            \
                     print(json.dumps({{\"id\": req[\"id\"], \"err\": str(e)}}, ensure_ascii=False), flush=True)\n            \
                     continue\n\n        \
                 print(json.dumps({{\"id\": req[\"id\"], \"out\": {{{:?}: got}}}}, ensure_ascii=False), flush=True)\n",
        "# rulec adapter template (rule {}), for a legacy implementation that is a Connect service.\n\
         #\n\
         #   rulec verify {}.rule --adapter python3 adapter.py https://pricing.internal\n\
         #\n\
         # It exchanges JSON Lines over stdin/stdout and puts each input to that service.\n\
         # It needs `uv add connectrpc` and the stubs of their own `.proto`.\n\
         # Two places are left to fill in, and both of them are the other side's messages.\n\
         import json\n\
         import sys\n\n\
         from connectrpc.errors import ConnectError\n\n\
         from their_pb2 import TheirRequest  # TODO: the stubs of their own .proto\n\
         from their_connect import TheirServiceClientSync  # TODO: the same\n\n\
         AT = sys.argv[1] if len(sys.argv) > 1 else \"http://127.0.0.1:8080\"\n\n\
         sys.stdin.readline()  # handshake\n\
         print(json.dumps({{\"ok\": True, \"impl\": f\"connect@{{AT}}\"}}), flush=True)\n\n\
         with TheirServiceClientSync(AT) as client:\n    \
             for line in sys.stdin:\n        \
                 line = line.strip()\n        \
                 if not line:\n            \
                     continue\n        \
                 req = json.loads(line)\n        \
                 d = req[\"in\"]  # inputs: {}. An integer in the declared unit, YYYY-MM-DD for a date, an enum by its name\n\n        \
                 try:\n            \
                     # TODO: build their request from d\n            \
                     res = client.decide(TheirRequest())\n            \
                     # TODO: where {} is in their answer\n            \
                     got = 0\n        \
                 except ConnectError as e:\n            \
                     # A record they say they cannot answer is left out of the match rate (docs/formats.md)\n            \
                     print(json.dumps({{\"id\": req[\"id\"], \"err\": str(e)}}, ensure_ascii=False), flush=True)\n            \
                     continue\n\n        \
                 print(json.dumps({{\"id\": req[\"id\"], \"out\": {{{:?}: got}}}}, ensure_ascii=False), flush=True)\n",
        f.name.text,
        pub_name(&f.name),
        ins.join(sep),
        out,
        out
    )
}
