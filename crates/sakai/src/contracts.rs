//! OpenAPI and AsyncAPI documents (DESIGN 15): which files are ones, what they hold, and the
//! `$ref`s between them. A file of the scope ending in `.yaml`, `.yml` or `.json` whose top
//! mapping has the key `openapi`, `asyncapi` or `swagger` is a document; a file such a document
//! reaches by `$ref` is a part of one (a schema kept in a file of its own). Both are artifacts,
//! and each belongs to one context.
//!
//! The YAML and the JSON are read by ritsu's reader (`ritsu_base::yaml`). What is sakai's is what
//! the documents mean: where the schemas, the channels, the messages and the operations are, the
//! values of an enum, and where a `$ref` lands (a file, and a JSON Pointer in it).

use crate::diag::{self, Diag};
use crate::model::Model;
use crate::owners::Artifact;
use crate::paths;
use crate::naming::Name;
use ritsu_base::document::{self, Documents};
use ritsu_base::text::Text;
use ritsu_base::yaml::{self, Node, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The kind of a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    OpenApi,
    AsyncApi,
}

impl Kind {
    /// The word a `.ctx` writes it with, and the summary counts it under.
    pub fn word(self) -> &'static str {
        match self {
            Kind::OpenApi => "openapi",
            Kind::AsyncApi => "asyncapi",
        }
    }

    /// The name people know it by.
    pub fn title(self) -> &'static str {
        match self {
            Kind::OpenApi => "OpenAPI",
            Kind::AsyncApi => "AsyncAPI",
        }
    }

    pub fn from_word(w: &str) -> Option<Kind> {
        match w {
            "openapi" => Some(Kind::OpenApi),
            "asyncapi" => Some(Kind::AsyncApi),
            _ => None,
        }
    }

    /// The tool its references start with (ritsu's DESIGN 6.2): `openapi "…"`, `asyncapi "…"`.
    pub fn tool(self) -> crate::naming::Tool {
        match self {
            Kind::OpenApi => crate::naming::Tool::Openapi,
            Kind::AsyncApi => crate::naming::Tool::Asyncapi,
        }
    }
}

/// Whether a file may be a document, by its extension.
pub fn may_be(path: &str) -> bool {
    path.ends_with(".yaml") || path.ends_with(".yml") || path.ends_with(".json")
}

/// What the top mapping of a text says it is, at a first look: `openapi` and `swagger` an OpenAPI
/// document, `asyncapi` an AsyncAPI document. The whole text is read later; this only finds the
/// documents among the scope's files of YAML and JSON.
pub fn sniff(text: &str, json: bool) -> Option<Kind> {
    let key = |k: &str| match k {
        "openapi" | "swagger" => Some(Kind::OpenApi),
        "asyncapi" => Some(Kind::AsyncApi),
        _ => None,
    };
    if json || text.trim_start_matches('\u{FEFF}').trim_start().starts_with('{') {
        return top_json_keys(text).into_iter().find_map(|k| key(&k));
    }
    for l in text.lines() {
        let l = l.trim_start_matches('\u{FEFF}');
        if l.starts_with(' ') || l.starts_with('\t') || l.starts_with('#') {
            continue;
        }
        let k = l.split(':').next().unwrap_or("").trim().trim_matches(['"', '\'']);
        if l.contains(':')
            && let Some(kind) = key(k)
        {
            return Some(kind);
        }
    }
    None
}

/// The keys of the top object of a JSON text (or of a flow mapping that starts a YAML one), read
/// without reading the values.
fn top_json_keys(text: &str) -> Vec<String> {
    let cs: Vec<char> = text.chars().collect();
    let mut keys = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    let mut expect_key = false;
    while i < cs.len() {
        let c = cs[i];
        match c {
            '"' | '\'' => {
                let q = c;
                let mut j = i + 1;
                let mut s = String::new();
                while j < cs.len() && cs[j] != q {
                    if cs[j] == '\\' && q == '"' {
                        j += 1;
                    }
                    if j < cs.len() {
                        s.push(cs[j]);
                    }
                    j += 1;
                }
                if depth == 1 && expect_key {
                    keys.push(s);
                    expect_key = false;
                }
                i = j + 1;
                continue;
            }
            '{' | '[' => {
                depth += 1;
                expect_key = depth == 1 && c == '{';
            }
            '}' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 1 => expect_key = true,
            _ => {}
        }
        if keys.len() > 200 {
            break;
        }
        i += 1;
    }
    keys
}

/// A document, read.
#[derive(Clone, Debug)]
pub struct Doc {
    /// From the root.
    pub path: String,
    pub kind: Kind,
    /// The version of the specification (`3.1.0`); empty for a part of a document.
    pub spec: String,
    /// `info.title` and `info.version`.
    pub title: String,
    pub version: String,
    pub root: Node,
    /// Reached by `$ref`, with no `openapi` or `asyncapi` of its own.
    pub part: bool,
}

/// A `$ref`, and where it lands.
#[derive(Clone, Debug)]
pub struct Ref {
    /// The document it is written in, from the root.
    pub from: String,
    /// Where the `$ref` is in that document, as a JSON Pointer (`/channels/orderPlaced`).
    pub at: String,
    pub line: usize,
    pub col: usize,
    /// As written.
    pub written: String,
    /// The file and the JSON Pointer it lands on; None for a URL or a file not there.
    pub to: Option<(String, String)>,
}

/// The documents of a map and their `$ref`s.
#[derive(Clone, Debug, Default)]
pub struct Contracts {
    pub docs: BTreeMap<String, Doc>,
    pub refs: Vec<Ref>,
    /// The documents that could not be read (E108).
    pub unread: BTreeSet<String>,
}

/// Every `$ref` of a node, with the JSON Pointer of the mapping it is in: ritsu-base's, which yuen
/// follows too (`ritsu_base::document`).
fn refs_in(node: &Node, at: &str, out: &mut Vec<(String, usize, usize, String)>) {
    document::refs_in(node, at, out);
}

/// A key as a token of a JSON Pointer.
pub fn escape(key: &str) -> String {
    document::escape(key)
}

/// Where a `$ref` written in `from` lands: the file (from the root) and the JSON Pointer; Err with
/// the URL when it is one; Err(None) when the path leaves the root.
pub fn target(from: &str, written: &str) -> Result<(String, String), Option<String>> {
    document::target(from, written)
}

thread_local! {
    /// The documents read while who owns what is decided (to find the parts of documents), kept
    /// for the stage that reads them, so that a large document is read once in a check.
    static READ: std::cell::RefCell<BTreeMap<std::path::PathBuf, Result<Node, yaml::Error>>> = const { std::cell::RefCell::new(BTreeMap::new()) };
}

/// Forget the documents kept between the stages of a check (at its start and its end).
pub fn forget() {
    READ.with(|r| r.borrow_mut().clear());
}

/// A file's text read as YAML or JSON: what an earlier stage of the check kept, taken, or read now.
fn read_kept(m: &Model, p: &str, text: &str) -> Result<Node, yaml::Error> {
    let disk = paths::on_disk(&m.root, p);
    match READ.with(|r| r.borrow_mut().remove(&disk)) {
        Some(got) => got,
        None => yaml::read_file_text(p, text),
    }
}

/// Keep what a file read as, for the next stage of the check.
fn keep(m: &Model, p: &str, got: Result<Node, yaml::Error>) {
    READ.with(|r| r.borrow_mut().insert(paths::on_disk(&m.root, p), got));
}

/// The files of YAML or JSON in the scope that the documents reach by `$ref`, over any number of
/// steps, and that are no documents themselves: the parts of documents. Read before who owns what
/// is decided, so that each part belongs to a context too.
pub fn parts(m: &Model, docs: &[(String, Kind)]) -> Vec<(String, Kind)> {
    let mut seen: BTreeSet<String> = docs.iter().map(|(p, _)| p.clone()).collect();
    let mut todo: Vec<(String, Kind)> = docs.to_vec();
    let mut out = Vec::new();
    while let Some((p, kind)) = todo.pop() {
        let Ok(text) = ritsu_base::fs::read_to_string(paths::on_disk(&m.root, &p)) else { continue };
        let got = read_kept(m, &p, &text);
        let mut rs = Vec::new();
        if let Ok(node) = &got {
            refs_in(node, "", &mut rs);
        }
        keep(m, &p, got);
        for (_, _, _, w) in rs {
            let Ok((f, _)) = target(&p, &w) else { continue };
            if seen.contains(&f) || !may_be(&f) || !crate::owners::in_scope(m, &f) || !ritsu_base::fs::is_file(paths::on_disk(&m.root, &f)) {
                continue;
            }
            seen.insert(f.clone());
            out.push((f.clone(), kind));
            todo.push((f, kind));
        }
    }
    out.sort();
    out
}

/// Read every document of the map, and hold their versions and `$ref`s to what sakai reads
/// (E108, W104, E103).
pub fn load(m: &Model, arts: &[Artifact]) -> (Contracts, Vec<Diag>) {
    let mut c = Contracts::default();
    let mut diags = Vec::new();
    let src = |f: &str| ritsu_base::fs::read_to_string(paths::on_disk(&m.root, f)).unwrap_or_default();
    for a in arts {
        let Some((kind, part)) = a.contract else { continue };
        let text = src(&a.path);
        let sf = paths::shown(&a.path);
        let node = match read_kept(m, &a.path, &text) {
            Ok(n) => n,
            Err(e) => {
                let msg = &e.message;
                let t = if part { tr!("{sf}（{} の文書の一部）", "{sf} (a part of an {} document)", kind.title(); kind.title()) } else { tr!("{} の文書 {sf}", "The {} document {sf}", kind.title(); kind.title()) };
                diags.push(
                    diag::at("E108", &a.path, e.line, e.col, tr!("{} を読めません: {}", "{} cannot be read: {}", t.ja, msg.ja; t.en, msg.en))
                        .source(&text)
                        .note(reader_note()),
                );
                c.unread.insert(a.path.clone());
                continue;
            }
        };
        let text_of = |n: Option<&Node>| -> String {
            match n.map(|n| &n.value) {
                Some(Value::Str(s)) => s.clone(),
                Some(Value::Float(f)) => f.clone(),
                Some(Value::Int(i)) => i.to_string(),
                _ => String::new(),
            }
        };
        let (mut spec, mut title, mut version) = (String::new(), String::new(), String::new());
        if !part {
            let info = node.get("info");
            title = text_of(info.and_then(|i| i.get("title")));
            version = text_of(info.and_then(|i| i.get("version")));
            let (key, value) = if let Some((k, v)) = node.entry("openapi") {
                ("openapi", Some((k, v)))
            } else if let Some((k, v)) = node.entry("asyncapi") {
                ("asyncapi", Some((k, v)))
            } else {
                ("swagger", node.entry("swagger"))
            };
            spec = text_of(value.map(|(_, v)| v));
            let (line, col) = value.map(|(k, _)| (k.line, k.col)).unwrap_or((1, 1));
            let ok = match key {
                "openapi" => ["3.0.", "3.1.", "3.2."].iter().any(|p| spec.starts_with(p)) || matches!(spec.as_str(), "3.0" | "3.1" | "3.2"),
                "asyncapi" => ["3.0.", "3.1."].iter().any(|p| spec.starts_with(p)) || matches!(spec.as_str(), "3.0" | "3.1"),
                _ => false,
            };
            if !ok {
                let (what, note) = match key {
                    "swagger" => (
                        tr!("OpenAPI 2.0（Swagger）の文書", "an OpenAPI 2.0 (Swagger) document"),
                        tr!("OpenAPI 3 の文書に変換してください（Swagger Editor や swagger2openapi で変換できます）。sakai が読むのは OpenAPI 3.0、3.1、3.2 です。", "Convert it to OpenAPI 3 (Swagger Editor or swagger2openapi converts it); sakai reads OpenAPI 3.0, 3.1 and 3.2."),
                    ),
                    "asyncapi" if spec.starts_with('2') => (
                        tr!("AsyncAPI {spec} の文書", "an AsyncAPI {spec} document"),
                        tr!(
                            "AsyncAPI 3 に変換してください（`asyncapi convert {sf}`）。2.x の publish と subscribe は相手の側から見た言い方で、3.0 からアプリケーションがすること（send と receive）になりました。sakai が読むのは AsyncAPI 3.0 と 3.1 です。",
                            "Convert it to AsyncAPI 3 (`asyncapi convert {sf}`): the publish and subscribe of 2.x are said from the other side, and since 3.0 an operation says what the application does (send, receive). sakai reads AsyncAPI 3.0 and 3.1."
                        ),
                    ),
                    _ => (
                        tr!("{} {spec} の文書", "an {} {spec} document", kind.title(); kind.title()),
                        tr!("sakai が読むのは OpenAPI 3.0、3.1、3.2 と AsyncAPI 3.0、3.1 です。", "sakai reads OpenAPI 3.0, 3.1 and 3.2, and AsyncAPI 3.0 and 3.1."),
                    ),
                };
                diags.push(diag::at("E108", &a.path, line, col, tr!("{sf} は {}なので、sakai は読みません", "The file {sf} is {}, which sakai does not read", what.ja; what.en)).source(&text).note(note));
                c.unread.insert(a.path.clone());
                continue;
            }
        }
        let mut rs = Vec::new();
        refs_in(&node, "", &mut rs);
        for (at, line, col, written) in rs {
            let to = match target(&a.path, &written) {
                Ok(t) => Some(t),
                Err(Some(url)) => {
                    diags.push(
                        diag::at("W104", &a.path, line, col, tr!("{sf} の `$ref` は URL（{url}）なので、sakai は読まずに、範囲の外のものとして扱います", "The `$ref` of {sf} is a URL ({url}); sakai does not read it, and takes it as outside the scope"))
                            .source(&text)
                            .note(tr!(
                                "その先の型は、境界の検査からも外れます。ほかのコンテキストの文書なら、リポジトリの中のファイルを相対パスで指してください。",
                                "What it points at is left out of the checks of the boundaries too; for another context's document, point at its file in the repository by a relative path."
                            )),
                    );
                    None
                }
                Err(None) => {
                    diags.push(diag::at("E103", &a.path, line, col, tr!("{sf} の `$ref` \"{written}\" は、ルートの外を指しています", "The `$ref` \"{written}\" of {sf} points outside the root")).source(&text).note(crate::owners::scope_note(m)));
                    None
                }
            };
            c.refs.push(Ref { from: a.path.clone(), at, line, col, written, to });
        }
        c.docs.insert(a.path.clone(), Doc { path: a.path.clone(), kind, spec, title, version, root: node, part });
    }
    // Where each `$ref` lands: a file of the scope, and something in it.
    for r in &c.refs {
        let Some((f, ptr)) = &r.to else { continue };
        if c.unread.contains(f) || (c.docs.contains_key(f) && c.locate(f, ptr).is_some()) {
            continue;
        }
        // the source is read for the diagnostic only: a large document has thousands of `$ref`s
        let text = src(&r.from);
        let sf = paths::shown(&r.from);
        let w = &r.written;
        if !c.docs.contains_key(f) {
            if !ritsu_base::fs::is_file(paths::on_disk(&m.root, f)) {
                let sfile = paths::shown(f);
                diags.push(diag::at("E108", &r.from, r.line, r.col, tr!("{sf} の `$ref` \"{w}\" の先のファイル {sfile} がありません", "The `$ref` \"{w}\" of {sf} points at {sfile}, which is not there")).source(&text).note(reader_note()));
            } else if !crate::owners::in_scope(m, f) {
                let sfile = paths::shown(f);
                diags.push(diag::at("E103", &r.from, r.line, r.col, tr!("{sf} の `$ref` \"{w}\" は、地図の範囲の外の {sfile} を指しています", "The `$ref` \"{w}\" of {sf} points at {sfile}, which is outside the map's scope")).source(&text).note(crate::owners::scope_note(m)));
            }
            continue;
        }
        {
            let shown_ptr = if ptr.is_empty() { "#".to_string() } else { format!("#{ptr}") };
            diags.push(
                diag::at("E108", &r.from, r.line, r.col, tr!("{sf} の `$ref` \"{w}\" は、何も指していません（{} に {shown_ptr} がありません）", "The `$ref` \"{w}\" of {sf} points at nothing ({} has no {shown_ptr})", paths::shown(f); paths::shown(f)))
                    .source(&text)
                    .note(tr!("`$ref` の `#` のあとは JSON Pointer です（`#/components/schemas/Order`）。キーの `/` は `~1`、`~` は `~0` と書きます。", "After the `#` of a `$ref` is a JSON Pointer (`#/components/schemas/Order`); a `/` in a key is written `~1`, and a `~` is `~0`.")),
            );
        }
    }
    (c, diags)
}

fn reader_note() -> Text {
    tr!(
        "sakai が読む YAML は、JSON と行き来できる YAML 1.2 です（RFC 9512 の 3.4 節。OpenAPI 3.2 と AsyncAPI 3.1 が勧めるもの）。その外の書き方（タグ、`?` のキー、二つ目の文書など）は読みません。",
        "The YAML sakai reads is the YAML 1.2 that goes to JSON and back (RFC 9512, section 3.4, as OpenAPI 3.2 and AsyncAPI 3.1 ask); what goes beyond it (tags, keys written with `?`, a second document) is not read."
    )
}

impl Documents for Contracts {
    fn root(&self, file: &str) -> Option<&Node> {
        self.docs.get(file).map(|d| &d.root)
    }
}

impl Contracts {
    /// The node a file and a JSON Pointer name, following the `$ref`s on the way (a channel that
    /// is a `$ref` to another file, then a message under it), and where it is: the file and the
    /// pointer it is at in the end.
    pub fn locate(&self, file: &str, pointer: &str) -> Option<(String, String, &Node)> {
        document::locate(self, file, pointer)
    }

    /// The line of the key a file and a pointer end on (the line of the document for the empty
    /// pointer), following the `$ref`s on the way.
    pub fn key_line(&self, file: &str, pointer: &str) -> Option<usize> {
        let tokens = yaml::pointer_tokens(pointer)?;
        let Some((last, init)) = tokens.split_last() else { return Some(1) };
        let parent: String = init.iter().map(|t| format!("/{}", escape(t))).collect();
        let (pf, pp, pnode) = self.locate(file, &parent)?;
        let (_, _, pnode) = self.definition(&pf, &pp, pnode);
        match &pnode.value {
            Value::Map(es) => es.iter().find(|(k, _)| k.name == *last).map(|(k, _)| k.line),
            Value::Seq(xs) => xs.get(last.parse::<usize>().ok()?).map(|x| x.line),
            _ => None,
        }
    }

    /// Where the `$ref`s of a node lead in the end: the definition it stands for.
    pub fn definition<'a>(&'a self, file: &str, pointer: &str, node: &'a Node) -> (String, String, &'a Node) {
        document::definition(self, file, pointer, node)
    }

    /// Every element a file and a pointer reach: themselves, what their `$ref`s land on, and on
    /// from there (a channel's messages and their payloads), as (file, pointer).
    pub fn reach(&self, file: &str, pointer: &str) -> Vec<(String, String)> {
        document::reach(self, file, pointer)
    }

    /// The reference of what a file and a JSON Pointer name (ritsu's DESIGN 6.2): `openapi
    /// "payments/api.yaml" schema Charge`, `asyncapi "…" channel orderPlaced`, and with no kind of
    /// its own `pointer /servers/0`. The tool is the document's (a part of one, the tool of the
    /// document that reaches it).
    pub fn naming(&self, file: &str, pointer: &str) -> Name {
        let tool = self.docs.get(file).map(|d| d.kind.tool()).unwrap_or(crate::naming::Tool::Openapi);
        document::reference(self, tool, file, pointer)
    }

    /// The file and the JSON Pointer of the element a reference of `openapi` or `asyncapi` names
    /// (of the schema, for a value of its `enum`); None for another tool, a whole file, or what
    /// the documents do not hold.
    pub fn place(&self, n: &Name) -> Option<(String, String)> {
        if !matches!(n.tool, crate::naming::Tool::Openapi | crate::naming::Tool::Asyncapi) || n.items.is_empty() {
            return None;
        }
        let base = if n.kind() == Some("value") { Name { items: n.items[..n.items.len() - 1].to_vec(), ..n.clone() } } else { n.clone() };
        let e = document::find(self, &base)?;
        Some((e.file, e.pointer))
    }

    /// The elements of a kind named `name` in the documents `files` (DESIGN 15.4): `schema` and
    /// `enum` under `components/schemas` (an enum has `enum`), `message` under AsyncAPI's
    /// `components/messages` or a channel's `messages`, `channel` under AsyncAPI's `channels`, and
    /// `operation` an OpenAPI `operationId` or a key of AsyncAPI's `operations`. Each as (file,
    /// pointer).
    pub fn find(&self, files: &[String], kind: &str, name: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let e = escape(name);
        for f in files {
            let Some(d) = self.docs.get(f) else { continue };
            let try_ptr = |p: String, out: &mut Vec<(String, String)>| {
                if d.root.pointer(&p).is_some() && !out.contains(&(f.clone(), p.clone())) {
                    out.push((f.clone(), p));
                }
            };
            match kind {
                "schema" => try_ptr(format!("/components/schemas/{e}"), &mut out),
                "enum" => {
                    let p = format!("/components/schemas/{e}");
                    if self.enum_values(f, &p).is_some() {
                        try_ptr(p, &mut out);
                    }
                }
                "message" if d.kind == Kind::AsyncApi => {
                    let p = format!("/components/messages/{e}");
                    if d.root.pointer(&p).is_some() {
                        try_ptr(p, &mut out);
                    } else {
                        for (cid, _) in d.root.get("channels").and_then(Node::as_map).unwrap_or_default() {
                            try_ptr(format!("/channels/{}/messages/{e}", escape(&cid.name)), &mut out);
                        }
                    }
                }
                "channel" if d.kind == Kind::AsyncApi => try_ptr(format!("/channels/{e}"), &mut out),
                "operation" => match d.kind {
                    Kind::AsyncApi => try_ptr(format!("/operations/{e}"), &mut out),
                    Kind::OpenApi => {
                        for op in self.operations(f) {
                            if op.id == name {
                                try_ptr(op.pointer.clone(), &mut out);
                            }
                        }
                    }
                },
                _ => {}
            }
        }
        out
    }

    /// The values of the enum a file and a pointer name, with the definition they are read from
    /// (the file, the pointer and where its `enum` is). A `null` among them says no value is set.
    pub fn enum_values(&self, file: &str, pointer: &str) -> Option<EnumValues> {
        let (f, p, node) = self.locate(file, pointer)?;
        let (df, dp, def) = self.definition(&f, &p, node);
        let (k, list) = def.entry("enum")?;
        let xs = list.as_seq()?;
        let values = xs
            .iter()
            .map(|x| {
                let (name, absent) = match &x.value {
                    Value::Str(s) => (s.clone(), false),
                    Value::Null => ("null".to_string(), true),
                    _ => (x.to_json_text(), false),
                };
                EnumValue { name, line: x.line, absent }
            })
            .collect();
        Some(EnumValues { file: df, pointer: dp, line: k.line, values })
    }

    /// The operations of an OpenAPI document: under `paths` and `webhooks`, each method of a
    /// path item (and the `additionalOperations` of OpenAPI 3.2), with its `operationId`.
    pub fn operations(&self, file: &str) -> Vec<Operation> {
        if self.docs.get(file).is_none_or(|d| d.kind != Kind::OpenApi) {
            return Vec::new();
        }
        document::operations(self, file).into_iter().map(|o| Operation { id: o.id, method: o.method, path: o.path, pointer: o.pointer, line: o.line, webhook: o.webhook }).collect()
    }

    /// The channels of an AsyncAPI document: the key, the address, and where the key is.
    pub fn channels(&self, file: &str) -> Vec<Channel> {
        let mut out = Vec::new();
        let Some(d) = self.docs.get(file) else { return out };
        if d.kind != Kind::AsyncApi {
            return out;
        }
        for (ck, ch) in d.root.get("channels").and_then(Node::as_map).unwrap_or_default() {
            let base = format!("/channels/{}", escape(&ck.name));
            let (df, _, def) = self.definition(file, &base, ch);
            let address = def.get("address").and_then(Node::as_str).unwrap_or("").to_string();
            let elsewhere = (df != file).then_some(df);
            let messages = def.get("messages").and_then(Node::as_map).unwrap_or_default().iter().map(|(k, _)| k.name.clone()).collect();
            out.push(Channel { id: ck.name.clone(), address, line: ck.line, elsewhere, messages });
        }
        out
    }

    /// The operations of an AsyncAPI document: the key, `send` or `receive`, and the channel of
    /// the document it is on (the key under `channels`).
    pub fn actions(&self, file: &str) -> Vec<Action> {
        let mut out = Vec::new();
        let Some(d) = self.docs.get(file) else { return out };
        if d.kind != Kind::AsyncApi {
            return out;
        }
        for (ok, op) in d.root.get("operations").and_then(Node::as_map).unwrap_or_default() {
            let (_, _, def) = self.definition(file, &format!("/operations/{}", escape(&ok.name)), op);
            let action = def.get("action").and_then(Node::as_str).unwrap_or("").to_string();
            let channel_ref = def.get("channel").and_then(|c| c.get("$ref")).and_then(Node::as_str).unwrap_or("").to_string();
            let channel = match target(file, &channel_ref) {
                Ok((f, p)) if f == file => p.strip_prefix("/channels/").filter(|c| !c.contains('/')).map(|c| c.replace("~1", "/").replace("~0", "~")),
                _ => None,
            };
            out.push(Action { id: ok.name.clone(), action, channel, channel_ref, line: ok.line });
        }
        out
    }

    /// The documents a context's published language holds: the ones it lists, and the parts they
    /// reach that belong to the same context.
    pub fn published_files(&self, listed: &[String], owner_of: &dyn Fn(&str) -> Option<usize>, ctx: usize) -> BTreeSet<String> {
        let mut out: BTreeSet<String> = listed.iter().cloned().collect();
        let mut todo: Vec<String> = listed.to_vec();
        while let Some(f) = todo.pop() {
            for r in self.refs.iter().filter(|r| r.from == f) {
                if let Some((t, _)) = &r.to
                    && owner_of(t) == Some(ctx)
                    && out.insert(t.clone())
                {
                    todo.push(t.clone());
                }
            }
        }
        out
    }
}

/// The values of an enum, and where they are.
#[derive(Clone, Debug)]
pub struct EnumValues {
    pub file: String,
    pub pointer: String,
    /// The line of its `enum`.
    pub line: usize,
    pub values: Vec<EnumValue>,
}

#[derive(Clone, Debug)]
pub struct EnumValue {
    /// The string on the wire (a number or a bool as JSON writes it).
    pub name: String,
    pub line: usize,
    /// `null`: no value is set.
    pub absent: bool,
}

#[derive(Clone, Debug)]
pub struct Operation {
    pub id: String,
    pub method: String,
    pub path: String,
    pub pointer: String,
    pub line: usize,
    pub webhook: bool,
}

impl Operation {
    /// What `open host service` names it by: its `operationId`, or with none its method and
    /// path (`"GET /orders/{id}"`).
    pub fn name(&self) -> String {
        if self.id.is_empty() { format!("{} {}", self.method, self.path) } else { self.id.clone() }
    }
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub id: String,
    pub address: String,
    pub line: usize,
    /// The file a channel that is a `$ref` to another document is in.
    pub elsewhere: Option<String>,
    pub messages: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Action {
    pub id: String,
    pub action: String,
    /// The key under the document's `channels` it is on.
    pub channel: Option<String>,
    pub channel_ref: String,
    pub line: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_look_finds_the_documents() {
        assert_eq!(sniff("openapi: 3.1.0\ninfo: {}\n", false), Some(Kind::OpenApi));
        assert_eq!(sniff("# c\n'asyncapi': 3.0.0\n", false), Some(Kind::AsyncApi));
        assert_eq!(sniff("name: x\nopenapi_like: 1\n", false), None);
        assert_eq!(sniff("{\"dependencies\": {\"openapi\": \"1\"}, \"name\": \"x\"}", true), None);
        assert_eq!(sniff("{\"info\": {}, \"openapi\": \"3.2.0\"}", true), Some(Kind::OpenApi));
    }

    #[test]
    fn the_refs_under_names_and_under_data() {
        let doc = yaml::read_yaml("paths:\n  /x:\n    get:\n      responses:\n        default:\n          $ref: 'e.yaml#/Error'\ncomponents:\n  schemas:\n    A:\n      properties:\n        example:\n          $ref: 'b.yaml#/B'\n        $ref:\n          type: string\n      example:\n        $ref: 'not/a/ref.yaml'\n      x-note:\n        $ref: 'nor/this.yaml'\n  messages:\n    M:\n      headers:\n        $ref: 'h.yaml#/H'\n").unwrap();
        let mut out = Vec::new();
        refs_in(&doc, "", &mut out);
        let got: Vec<(String, String)> = out.into_iter().map(|(at, _, _, w)| (at, w)).collect();
        let want = [("/paths/~1x/get/responses/default", "e.yaml#/Error"), ("/components/schemas/A/properties/example", "b.yaml#/B"), ("/components/messages/M/headers", "h.yaml#/H")];
        assert_eq!(got, want.map(|(a, b)| (a.to_string(), b.to_string())));
    }

    #[test]
    fn where_a_ref_lands() {
        assert_eq!(target("a/b.yaml", "#/components/schemas/X"), Ok(("a/b.yaml".into(), "/components/schemas/X".into())));
        assert_eq!(target("a/b.yaml", "../c.yaml#/channels/x"), Ok(("c.yaml".into(), "/channels/x".into())));
        assert_eq!(target("a/b.yaml", "https://example.com/x.yaml#/a"), Err(Some("https://example.com/x.yaml".into())));
        assert_eq!(target("a/b.yaml", "c.yaml#/paths/~1orders%7Bid%7D"), Ok(("a/c.yaml".into(), "/paths/~1orders{id}".into())));
    }
}
