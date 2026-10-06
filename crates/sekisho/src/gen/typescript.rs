//! `sekisho gen --target typescript` (DESIGN 5.3–5.6): for each action, the function that builds
//! the request Cedar is asked (`<action>Request`) and the one that asks it (`authorize<Action>`),
//! in one module a gate, `typescript/authz/<alias>.ts`. The module reads the rules and the dates
//! the gate computes with from the package's `rules/` and `dates/` (what `ritsu gen` writes beside
//! it, ritsu's DESIGN 9.3), and holds the gate's Cedar: the policies by `@id` and the schema
//! (`--authorizer cedar`, asked of Cedar's own WebAssembly build, `@cedar-policy/cedar-wasm`), or
//! asks Amazon Verified Permissions through the AWS SDK (`--authorizer avp`).
//!
//! What the code reads, checks and refuses, and in which order, is [`crate::raw`]'s: the reference
//! evaluation there is what this code is held to (`tests/typescript.rs` runs both on the same data).

use crate::cedar::{self, Shape};
use crate::checks::Checked;
use crate::model::*;
use crate::names::Scope;
use crate::r#gen::Authorizer;
use crate::raw;
use crate::suite::Suite;
use crate::walk::{Domain, Kn};
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_emit::ident::pascal;
use ritsu_emit::lit::json as q;
use ritsu_ports::{ColumnType, DateKind, RuleFacts};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// The versions the code is written against, and its tests run with (`tools/runner-ts`'s lock).
pub const CEDAR_WASM: &str = "4.13.0";
pub const AWS_SDK: &str = "3.1146.0";

/// Where `gen` writes the module of a gate: as `ritsu gen` writes it into a package.
pub fn path(alias: &str) -> String {
    format!("typescript/authz/{alias}.ts")
}

/// The names the module uses and does not declare: the globals of JavaScript it calls, and the names
/// it imports. A name of the gate's that is one of them gets `_2` after it.
const GLOBALS: &[&str] = &[
    "Date", "Promise", "Error", "Number", "String", "Boolean", "BigInt", "JSON", "Array", "Object", "Record", "Set", "Map", "Math", "Symbol", "Function", "RegExp",
    "Readonly", "VerifiedPermissionsClient", "IsAuthorizedCommand",
];

/// The types the module declares for itself. When a name of the gate's is one of them, the gate's
/// keeps its name and the module's gets `_2` after it, as in the Python and the Go (DESIGN 5.3).
const OWN: [&str; 11] = ["Store", "Principal", "Role", "Uid", "Value", "Entity", "Request", "Answer", "SekishoError", "SekishoErrorKind", "VerifiedPermissions"];

/// `refund_order` → `refundOrder`.
pub fn camel(alias: &str) -> String {
    let p = pascal(alias);
    let mut cs = p.chars();
    match cs.next() {
        Some(c) => c.to_ascii_lowercase().to_string() + cs.as_str(),
        None => String::new(),
    }
}

/// The method of `Store` that reads an entity of a type: `user`, `refundRecord`.
pub fn store_method(type_alias: &str) -> String {
    let mut cs = type_alias.chars();
    match cs.next() {
        Some(c) => c.to_ascii_lowercase().to_string() + cs.as_str(),
        None => String::new(),
    }
}

/// The field of an entity's data that holds an attribute: its alias, moved aside from `roles` and
/// `member_of`.
pub fn field(attr_alias: &str) -> String {
    ritsu_emit::ident::aside(attr_alias, |s| s == "roles" || s == "member_of")
}

/// `RefundOrderInput`, `refundOrderRequest`, `authorizeRefundOrder`.
pub fn input_type(action_alias: &str) -> String {
    format!("{}Input", pascal(action_alias))
}
pub fn request_fn(action_alias: &str) -> String {
    format!("{}Request", camel(action_alias))
}
pub fn authorize_fn(action_alias: &str) -> String {
    format!("authorize{}", pascal(action_alias))
}

/// The names of the types the module declares: each entity type's data (by the type, for a type
/// the code reads), each enum, the module's own types, and each action's input (by the action).
pub struct Names {
    pub types: Vec<String>,
    pub enums: Vec<String>,
    pub store: String,
    pub principal: String,
    pub role: String,
    pub uid: String,
    pub value: String,
    pub entity: String,
    pub request: String,
    pub answer: String,
    pub error: String,
    pub error_kind: String,
    pub avp: String,
    pub inputs: Vec<String>,
}

/// The names, made as the Python and the Go make theirs: the gate's types and enums first, then the
/// module's own, then the inputs, each that comes out the same as one before it (or as a global)
/// with `_2` after it.
pub fn names(g: &Gate, shape: &Shape) -> Names {
    let read: Vec<usize> = (0..g.types.len()).filter(|&t| raw::reads(g, shape, t)).collect();
    let mut wanted: Vec<String> = read.iter().map(|&t| g.types[t].named.alias.clone()).collect();
    wanted.extend(g.enums.iter().map(|e| pascal(&e.named.alias)));
    wanted.extend(OWN.iter().map(|s| s.to_string()));
    wanted.extend(g.actions.iter().map(|a| input_type(&a.named.alias)));
    let mut got = ritsu_emit::ident::unique(wanted, GLOBALS).into_iter();
    let mut next = || got.next().unwrap_or_default();
    let mut types = vec![String::new(); g.types.len()];
    for &t in &read {
        types[t] = next();
    }
    let enums = g.enums.iter().map(|_| next()).collect();
    let [store, principal, role, uid, value, entity, request, answer, error, error_kind, avp] = OWN.map(|_| next());
    let inputs = g.actions.iter().map(|_| next()).collect();
    Names { types, enums, store, principal, role, uid, value, entity, request, answer, error, error_kind, avp, inputs }
}

/// The module of a gate that passes its check. `shown` is the path the head names; the rules,
/// the dates and the calendars are read through `suite`. Err when what another language says of a
/// file the gate reads is missing (it passed the check, so this is a bug).
pub fn module(scope: &Scope, checked: &Checked, suite: &Suite, shown: &str, lang: Lang, authorizer: Authorizer) -> Result<String, String> {
    let g = &checked.gate;
    let shape = cedar::shape(g, scope, checked);
    let files = cedar::files(scope, checked, shown, lang).map_err(|e| e.message.en)?;
    let w = Writer { g, shape: &shape, scope, checked, suite, lang, authorizer, names: names(g, &shape), out: String::new(), helpers: BTreeSet::new() };
    w.write(&cedar::head(scope, shown, lang), &files.policies_json, &files.schema_json)
}

struct Writer<'a> {
    g: &'a Gate,
    shape: &'a Shape,
    scope: &'a Scope,
    checked: &'a Checked,
    suite: &'a Suite,
    lang: Lang,
    authorizer: Authorizer,
    names: Names,
    out: String,
    /// The helpers the code calls, written at the end.
    helpers: BTreeSet<&'static str>,
}

/// A value of the data as the code holds it: an attribute of the principal's or the resource's
/// data, an input, today, or a constant.
enum Expr {
    Code(String),
    Absent,
}

impl<'a> Writer<'a> {
    fn t(&self, t: Text) -> String {
        t.get(self.lang).to_string()
    }

    fn line(&mut self, s: impl AsRef<str>) {
        self.out.push_str(s.as_ref());
        self.out.push('\n');
    }

    /// The comment lines of a text, wrapped at 100 columns.
    fn comment(&mut self, indent: &str, text: &str) {
        for l in wrap(text, 100 - indent.len() - 3) {
            self.line(format!("{indent}// {l}"));
        }
    }

    fn doc(&mut self, indent: &str, text: &str) {
        let lines = wrap(text, 100 - indent.len() - 4);
        if lines.len() == 1 && lines[0].len() + indent.len() + 7 <= 100 {
            self.line(format!("{indent}/** {} */", lines[0]));
            return;
        }
        self.line(format!("{indent}/**"));
        for l in lines {
            self.line(format!("{indent} * {l}"));
        }
        self.line(format!("{indent} */"));
    }

    fn ns(&self) -> String {
        format!("{}::", self.g.namespace)
    }

    /// The TypeScript type of a field's value.
    fn ty(&self, f: &Field) -> String {
        let base = match &f.ty {
            FieldType::Bool => "boolean".to_string(),
            FieldType::Enum(e) => self.names.enums[*e].clone(),
            FieldType::Num { .. } => "bigint".to_string(),
            FieldType::Date { .. } | FieldType::Entity(_) => "string".to_string(),
        };
        if f.optional { format!("{base} | null") } else { base }
    }

    /// What a field holds, for its comment.
    fn about(&self, f: &Field) -> String {
        let g = self.g;
        match &f.ty {
            FieldType::Bool => String::new(),
            FieldType::Enum(e) => {
                let vs: Vec<String> = g.enums[*e].values.iter().map(|v| v.alias.clone()).collect();
                self.t(tr!("{} のどれか", "One of {}", vs.join("、"); vs.join(", ")))
            }
            FieldType::Num { written, lo, hi, .. } => self.t(tr!("{written}、{lo}〜{hi}", "{written}, {lo} to {hi}")),
            FieldType::Date { lo, hi } => {
                let (l, h) = (ritsu_ports::day_text(*lo), ritsu_ports::day_text(*hi));
                self.t(tr!("YYYY-MM-DD、{l}〜{h}", "YYYY-MM-DD, {l} to {h}"))
            }
            FieldType::Entity(t) => {
                let n = &g.types[*t].named.alias;
                self.t(tr!("{n} の ID", "The id of a {n}"))
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // The module

    fn write(mut self, head: &str, policies_json: &str, schema_json: &str) -> Result<String, String> {
        let g = self.g;
        self.out.push_str(head);
        self.line("");
        if let Some((d, _)) = &self.scope.file().description {
            let d = d.clone();
            self.comment("", &d);
            self.line("//");
        }
        let about = self.t(tr!(
            "このモジュールが組み立てるリクエストだけが、このゲートの尋ね方である。context に入るのは、操作が頼まれたこと（ハンドラーが解析した input）と、ritsu の言語がサービス自身のデータ（Store が読む）から計算した値で、呼ぶ側が送ってきたものは何も入れない。操作は、ゲートに尋ねたのと同じ input で行う。Store から読んで許したあと、操作までにデータが変わりうるので、操作と同じトランザクションで読むか、操作の側でも確かめる。",
            "The requests this module builds are the only way the gate is meant to be asked: the context holds what the operation is asked to do (its input, as the handler parsed it) and what ritsu's languages compute from the service's own data, which the Store reads; nothing in it is taken from what the caller sends. Do the operation with the input the gate was asked with. The data can change between the read and the operation: read it in the operation's transaction, or check again in the operation."
        ));
        self.comment("", &about);
        self.line("");
        self.imports()?;
        self.types();
        self.consts(policies_json, schema_json);
        for ai in 0..g.actions.len() {
            self.action(ai)?;
        }
        self.ask();
        self.write_helpers();
        Ok(self.out)
    }

    fn imports(&mut self) -> Result<(), String> {
        match self.authorizer {
            Authorizer::Cedar => self.line("import * as cedar from \"@cedar-policy/cedar-wasm/nodejs\";"),
            Authorizer::Avp => self.line("import { IsAuthorizedCommand, type VerifiedPermissionsClient } from \"@aws-sdk/client-verifiedpermissions\";"),
        }
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for (ui, u) in self.g.uses.iter().enumerate() {
            let line = match u.kind {
                UseKind::Rule => {
                    let f = self.rule_facts(ui)?;
                    let m = f.typescript.module.trim_end_matches(".ts").to_string();
                    format!("import * as rule_{m} from \"../rules/{m}\";")
                }
                UseKind::Dates => {
                    let a = self.dates_facts(ui)?.alias.clone();
                    format!("import * as dates_{a} from \"../dates/{a}\";")
                }
                UseKind::Calendar => {
                    let a = self.calendar_alias(ui)?;
                    format!("import * as dates_{a} from \"../dates/{a}\";")
                }
                _ => continue,
            };
            // a file the gate reads only through a computed value it never computes is still read
            if seen.insert(line.clone()) {
                self.line(line);
            }
        }
        self.line("");
        Ok(())
    }

    fn rule_facts(&self, ui: usize) -> Result<&'a RuleFacts, String> {
        let u = &self.g.uses[ui];
        self.scope.rules.get(&u.name).map(|r| &r.facts).ok_or_else(|| format!("rulec said nothing of {}", u.path))
    }

    fn dates_facts(&self, ui: usize) -> Result<&'a ritsu_ports::DateFacts, String> {
        let u = &self.g.uses[ui];
        self.scope.dates.get(&u.name).map(|r| &r.facts).ok_or_else(|| format!("koyomi said nothing of {}", u.path))
    }

    fn calendar_alias(&self, ui: usize) -> Result<String, String> {
        let u = &self.g.uses[ui];
        let d = self.suite.dates.as_ref().ok_or_else(|| format!("koyomi is not joined for {}", u.path))?;
        d.calendar(&u.file).map(|c| c.alias).map_err(|_| format!("koyomi said nothing of {}", u.path))
    }

    fn types(&mut self) {
        let g = self.g;
        let shape = self.shape;
        // the roles, the enums
        if !g.roles.is_empty() {
            let rs: Vec<String> = g.roles.iter().map(|r| q(&r.named.alias)).collect();
            let doc = self.t(tr!("役割（ゲートが書いた別名）", "A role, by the alias the gate gives it"));
            self.doc("", &doc);
            self.line(format!("export type {} = {};", self.names.role, rs.join(" | ")));
        }
        for (ei, e) in g.enums.iter().enumerate() {
            let vs: Vec<String> = e.values.iter().map(|v| q(&v.alias)).collect();
            let doc = self.t(tr!("列挙 `{}` の値", "The values of the enum {}", e.named.name; e.named.name));
            self.doc("", &doc);
            self.line(format!("export type {} = {};", self.names.enums[ei], vs.join(" | ")));
        }
        // who asks
        let mut alts: Vec<String> = Vec::new();
        for t in g.types.iter().filter(|t| t.kind == Kind::Principal) {
            alts.push(format!("{{ type: {}; id: string }}", q(&t.named.alias)));
        }
        if let Some(wt) = g.workflow_type() {
            let ws: Vec<String> = g.workflows.iter().map(|w| q(&w.named.alias)).collect();
            let id = if ws.is_empty() { "string".to_string() } else { ws.join(" | ") };
            alts.push(format!("{{ type: {}; id: {id} }}", q(&g.types[wt].named.alias)));
        }
        let doc = self.t(tr!(
            "尋ねる principal：型と、認証が決めた ID（ワークフローは名前）",
            "Who asks: a principal's type, and its id as the authentication decided it (a workflow by its name)"
        ));
        self.doc("", &doc);
        self.line(format!("export type {} = {};", self.names.principal, if alts.is_empty() { "never".to_string() } else { alts.join(" | ") }));
        self.line("");
        // the data of each entity type the code reads
        for (ti, t) in g.types.iter().enumerate() {
            if !raw::reads(g, shape, ti) {
                continue;
            }
            let name = self.names.types[ti].clone();
            let kind = if t.kind == Kind::Principal { "principal" } else { "resource" };
            let desc = self.scope.files.iter().find_map(|f| f.principals.iter().chain(f.resources.iter()).find(|e| e.name.ascii() == t.named.alias)).and_then(|e| e.description.as_ref().map(|(d, _)| d.clone()));
            let doc = match desc {
                Some(d) => self.t(tr!("ゲートが {kind} {} について読むもの：{d}", "What the gate reads of a {kind} {}: {d}", t.named.alias; t.named.alias)),
                None => self.t(tr!("ゲートが {kind} {} について読むもの", "What the gate reads of a {kind} {}", t.named.alias; t.named.alias)),
            };
            self.doc("", &doc);
            self.line(format!("export interface {name} {{"));
            if !t.roles.is_empty() {
                let rs: Vec<&str> = t.roles.iter().map(|&r| g.roles[r].named.alias.as_str()).collect();
                let doc = self.t(tr!("直接持つ役割（{}）", "The roles it holds directly ({})", rs.join("、"); rs.join(", ")));
                self.doc("  ", &doc);
                self.line(format!("  roles: {}[];", self.names.role));
            }
            if !shape.types[ti].member_of.is_empty() {
                let gs: Vec<String> = shape.types[ti].member_of.iter().map(|&m| format!("{{ type: {}; id: string }}", q(&g.types[m].named.alias))).collect();
                let doc = self.t(tr!("メンバーであるグループ", "The groups it is a member of"));
                self.doc("  ", &doc);
                self.line(format!("  member_of: ({})[];", gs.join(" | ")));
            }
            for i in raw::store_attrs(g, shape, ti) {
                let f = &t.attrs[i];
                let about = self.about(f);
                if !about.is_empty() {
                    self.doc("  ", &about);
                }
                let opt = if f.optional { "?" } else { "" };
                self.line(format!("  {}{opt}: {};", field(&f.named.alias), self.ty(f)));
            }
            self.line("}");
        }
        // the store
        let doc = self.t(tr!(
            "サービス自身のデータ。principal と resource を ID で読む。無ければ undefined か null を返す",
            "The service's own data, which the code reads the principal and the resource from, by id; undefined or null when there is none"
        ));
        self.doc("", &doc);
        self.line(format!("export interface {} {{", self.names.store));
        for (ti, t) in g.types.iter().enumerate() {
            if raw::reads(g, shape, ti) {
                self.line(format!("  {}(id: string): Promise<{} | undefined | null>;", store_method(&t.named.alias), self.names.types[ti]));
            }
        }
        self.line("}");
        self.line("");
        // the inputs of each action
        for (ai, a) in g.actions.iter().enumerate() {
            let doc = match a.references().first() {
                Some((r, _)) => {
                    let r = r.text();
                    self.t(tr!("action {} の input：操作（{r}）が頼まれたこと。ハンドラーが解析したもの", "The input of the action {}: what the operation ({r}) is asked to do, as the handler parsed it", a.named.alias; a.named.alias))
                }
                None => self.t(tr!("action {} の input：リクエストの引数", "The input of the action {}: the arguments of the request", a.named.alias; a.named.alias)),
            };
            self.doc("", &doc);
            self.line(format!("export interface {} {{", self.names.inputs[ai]));
            let rf = raw::resource_field(a);
            let rdoc = self.t(tr!("操作が扱う resource の ID", "The id of the resource the operation acts on"));
            self.doc("  ", &rdoc);
            self.line(format!("  {}: string;", prop(&rf)));
            if a.resources.len() > 1 {
                let ts: Vec<String> = a.resources.iter().map(|&t| q(&g.types[t].named.alias)).collect();
                let tdoc = self.t(tr!("resource の型", "The resource's type"));
                self.doc("  ", &tdoc);
                self.line(format!("  resource_type: {};", ts.join(" | ")));
            }
            for f in &a.inputs {
                let about = self.about(f);
                if !about.is_empty() {
                    self.doc("  ", &about);
                }
                let opt = if f.optional { "?" } else { "" };
                self.line(format!("  {}{opt}: {};", prop(&f.named.alias), self.ty(f)));
            }
            self.line("}");
        }
        self.line("");
        // the error, the request, the answer
        let doc = self.t(tr!(
            "拒んだ理由の種類。Cedar に尋ねる前のもの：principal、resource、input（無い、範囲や列挙の外）、today（範囲の外の日）、rule と date（規則と日付の生成物のエラー）。尋ねたあとのもの：cedar（Cedar がエラーを言った、尋ねられなかった）",
            "Why the request was refused. Before Cedar is asked: the principal, the resource or the input (not there, or outside its range or its enum), today (a day outside the range the gate was checked over), or a rule or a date (an error of the code rulec or koyomi wrote). When it is asked: cedar (Cedar met an error, or could not be asked)"
        ));
        self.doc("", &doc);
        let (error, kind, uid, value, entity) = (self.names.error.clone(), self.names.error_kind.clone(), self.names.uid.clone(), self.names.value.clone(), self.names.entity.clone());
        self.line(format!("export type {kind} = \"principal\" | \"resource\" | \"input\" | \"today\" | \"rule\" | \"date\" | \"cedar\";"));
        self.line("");
        self.line(format!("export class {error} extends Error {{"));
        self.line(format!("  readonly kind: {kind};"));
        self.line(format!("  constructor(kind: {kind}, message: string) {{"));
        self.line("    super(message);");
        self.line(format!("    this.name = {};", q(&error)));
        self.line("    this.kind = kind;");
        self.line("  }");
        self.line("}");
        self.line("");
        let doc = self.t(tr!("エンティティの型と ID。Cedar がエンティティを指す形", "An entity's type and id, as Cedar names an entity"));
        self.doc("", &doc);
        self.line(format!("export interface {uid} {{"));
        self.line("  type: string;");
        self.line("  id: string;");
        self.line("}");
        self.line(format!("export type {value} = boolean | number | string | {{ __entity: {uid} }};"));
        self.line(format!("export interface {entity} {{"));
        self.line(format!("  uid: {uid};"));
        self.line(format!("  attrs: Record<string, {value}>;"));
        self.line(format!("  parents: {uid}[];"));
        self.line("}");
        let doc = self.t(tr!("Cedar に尋ねるリクエストと、Cedar が読むエンティティ", "A request to Cedar, with the entities it reads"));
        self.doc("", &doc);
        self.line(format!("export interface {} {{", self.names.request));
        self.line(format!("  principal: {uid};"));
        self.line(format!("  action: {uid};"));
        self.line(format!("  resource: {uid};"));
        self.line(format!("  context: Record<string, {value}>;"));
        self.line(format!("  entities: {entity}[];"));
        self.line("}");
        let doc = match self.authorizer {
            Authorizer::Cedar => self.t(tr!(
                "答え：許すか、決めたポリシー（@id を並べ替えたもの）、Cedar に渡した context（判断の記録のため）、拒んだ理由",
                "The answer: whether it is allowed, the policies that decided it (their @id, sorted), the context Cedar was given (for the log of decisions), and why it was refused before Cedar was asked"
            )),
            Authorizer::Avp => self.t(tr!(
                "答え：許すか、決めたポリシー（Verified Permissions のポリシーストアが付けた ID を並べ替えたもの）、渡した context（判断の記録のため）、拒んだ理由",
                "The answer: whether it is allowed, the policies that decided it (the ids the policy store of Verified Permissions gave them, sorted), the context it was given (for the log of decisions), and why it was refused before it was asked"
            )),
        };
        self.doc("", &doc);
        self.line(format!("export interface {} {{", self.names.answer));
        self.line("  allowed: boolean;");
        self.line("  policies: string[];");
        self.line(format!("  context?: Record<string, {value}>;"));
        self.line(format!("  error?: {error};"));
        self.line("}");
        if self.authorizer == Authorizer::Avp {
            let alias = &g.named.alias;
            let doc = self.t(tr!(
                "尋ねる先：Verified Permissions のクライアントと、このゲートのスキーマ（cedar/{alias}.cedarschema.json）とポリシー（cedar/{alias}.cedar の一つずつを、@id を名前にして）を置いたポリシーストア",
                "Where it is asked: the client of Amazon Verified Permissions, and the policy store that holds the gate's schema (cedar/{alias}.cedarschema.json) and its policies (each policy of cedar/{alias}.cedar, its @id as its name)"
            ));
            self.doc("", &doc);
            self.line(format!("export interface {} {{", self.names.avp));
            self.line("  client: VerifiedPermissionsClient;");
            self.line("  policyStoreId: string;");
            self.line("}");
        }
        self.line("");
    }

    fn consts(&mut self, policies_json: &str, schema_json: &str) {
        let g = self.g;
        let ns = self.ns();
        self.line(format!("const _NS = {};", q(&ns)));
        // the roles, as entities with the roles they include as their parents
        if !g.roles.is_empty() {
            self.line(format!("const _ROLES: {}[] = [", self.names.entity));
            for r in &g.roles {
                let parents: Vec<String> = r.includes.iter().map(|&i| format!("{{ type: {}, id: {} }}", q(&format!("{ns}{}", cedar::ROLE)), q(&g.roles[i].named.alias))).collect();
                self.line(format!("  {{ uid: {{ type: {}, id: {} }}, attrs: {{}}, parents: [{}] }},", q(&format!("{ns}{}", cedar::ROLE)), q(&r.named.alias), parents.join(", ")));
            }
            self.line("];");
        }
        for (ti, t) in g.types.iter().enumerate() {
            if !t.roles.is_empty() {
                let rs: Vec<String> = t.roles.iter().map(|&r| q(&g.roles[r].named.alias)).collect();
                self.line(format!("const _roles_{}: readonly string[] = [{}];", t.named.alias, rs.join(", ")));
            }
            let _ = ti;
        }
        for e in &g.enums {
            let vs: Vec<String> = e.values.iter().map(|v| q(&v.alias)).collect();
            self.line(format!("const _enum_{}: readonly string[] = [{}];", e.named.alias, vs.join(", ")));
        }
        if g.workflow_type().is_some() {
            let ws: Vec<String> = g.workflows.iter().map(|w| q(&w.named.alias)).collect();
            self.line(format!("const _WORKFLOWS: readonly string[] = [{}];", ws.join(", ")));
        }
        if self.authorizer == Authorizer::Cedar {
            let alias = &g.named.alias;
            let doc = self.t(tr!(
                "ポリシー。@id ごとに（cedar/{alias}.policies.json と同じ）。一つのテキストで渡すと、cedar-wasm はポリシーに policy0、policy1… と名前を付け、決めたポリシーがゲートの名前にならない",
                "The policies, each by its @id (cedar/{alias}.policies.json): given as one text, cedar-wasm would name them policy0, policy1, …, and the policies that decide would not be the gate's"
            ));
            self.doc("", &doc);
            self.line(format!("export const POLICIES: cedar.PolicySet = JSON.parse({});", q(policies_json.trim_end())));
            let doc = self.t(tr!("スキーマ（cedar/{alias}.cedarschema.json と同じ）。リクエストはこれで確かめる", "The schema (cedar/{alias}.cedarschema.json), which the request is held to"));
            self.doc("", &doc);
            self.line(format!("export const SCHEMA: cedar.Schema = JSON.parse({});", q(schema_json.trim_end())));
        }
        self.line("");
    }

    // -----------------------------------------------------------------------------------------
    // An action

    fn action(&mut self, ai: usize) -> Result<(), String> {
        let g = self.g;
        let shape = self.shape;
        let a = &g.actions[ai];
        let alias = a.named.alias.clone();
        let input_t = self.names.inputs[ai].clone();
        // the module's own types, by the names they came out with (names)
        let (store, principal, err) = (self.names.store.clone(), self.names.principal.clone(), self.names.error.clone());
        let guards: Vec<String> = a.references().iter().map(|(r, _)| r.text()).collect();
        let doc = if guards.is_empty() {
            self.t(tr!("action {alias} のリクエストを組み立てる。拒むときは {err} を投げる", "Builds the request of the action {alias}; throws a {err} to refuse it"))
        } else {
            let (ja, en) = (guards.join("、"), guards.join("; "));
            self.t(tr!("action {alias}（{ja} を守る）のリクエストを組み立てる。拒むときは {err} を投げる", "Builds the request of the action {alias}, which guards {en}; throws a {err} to refuse it"))
        };
        self.doc("", &doc);
        self.line(format!("export async function {}(store: {store}, principal: {principal}, input: {input_t}, now: Date = new Date()): Promise<{}> {{", request_fn(&alias), self.names.request));
        // 1. the principal's type
        let pts: Vec<String> = a.principals.iter().map(|&t| g.types[t].named.alias.clone()).collect();
        let cond: Vec<String> = pts.iter().map(|t| format!("principal.type !== {}", q(t))).collect();
        self.helpers.insert("fail");
        // what the type says cannot come still can from JavaScript, and is refused
        let uid = self.names.uid.clone();
        let m = self.t(tr!("{alias} を尋ねるのは {} で、${{(principal as {uid}).type}} ではありません", "{alias} is asked by {}, not by a ${{(principal as {uid}).type}}", pts.join("、"); pts.join(", ")));
        self.line(format!("  if ({}) _fail(\"principal\", `{m}`);", cond.join(" && ")));
        if let Some(wt) = g.workflow_type().filter(|t| a.principals.contains(t)) {
            let m = self.t(tr!("ゲートはワークフロー ${{principal.id}} を宣言していません", "the gate declares no workflow ${{principal.id}}"));
            self.line(format!("  if (principal.type === {} && !_WORKFLOWS.includes(principal.id)) _fail(\"principal\", `{m}`);", q(&g.types[wt].named.alias)));
        }
        // 2. today, held to its range by an action that computes a value from it (raw::reads_today)
        if g.today.is_some() && raw::reads_today(a) {
            self.helpers.insert("today");
            self.line("  const day = _today(now);");
        }
        // 3. the inputs
        for (i, f) in a.inputs.iter().enumerate() {
            let get = format!("input.{}", prop(&f.named.alias));
            let check = self.check(f, &get, "\"input\"", &q(&f.named.alias));
            self.line(format!("  const in{i} = {check};"));
        }
        let rf = raw::resource_field(a);
        self.helpers.insert("id");
        // a resource's id that is not there is the resource's fault, as is a type it does not take (the
        // Python says so too)
        self.line(format!("  const resourceId = _id(input.{}, \"resource\", {});", prop(&rf), q(&rf)));
        let rts: Vec<String> = a.resources.iter().map(|&t| g.types[t].named.alias.clone()).collect();
        if a.resources.len() > 1 {
            let cond: Vec<String> = rts.iter().map(|t| format!("input.resource_type !== {}", q(t))).collect();
            let m = self.t(tr!("{alias} の resource は {} で、${{String(input.resource_type)}} ではありません", "the resource of {alias} is a {}, not a ${{String(input.resource_type)}}", rts.join("、"); rts.join(", ")));
            self.line(format!("  if ({}) _fail(\"resource\", `{m}`);", cond.join(" && ")));
            self.line("  const resourceType: string = input.resource_type;");
        } else {
            self.line(format!("  const resourceType: string = {};", q(&rts[0])));
        }
        // 4. the resource, 5. the principal
        for &t in &a.resources {
            if raw::reads(g, shape, t) {
                let ty = self.names.types[t].clone();
                let m = store_method(&g.types[t].named.alias);
                self.helpers.insert("read");
                if a.resources.len() > 1 {
                    self.line(format!("  const r_{} = resourceType === {} ? _read_{}(await store.{m}(resourceId), \"resource\", resourceId) : undefined;", g.types[t].named.alias, q(&g.types[t].named.alias), g.types[t].named.alias));
                } else {
                    self.line(format!("  const r_{}: {ty} | undefined = _read_{}(await store.{m}(resourceId), \"resource\", resourceId);", g.types[t].named.alias, g.types[t].named.alias));
                }
            }
        }
        for &t in &a.principals {
            if raw::reads(g, shape, t) {
                let ty = self.names.types[t].clone();
                let m = store_method(&g.types[t].named.alias);
                self.line(format!(
                    "  const p_{0}: {ty} | undefined = principal.type === {1} ? _read_{0}(await store.{m}(principal.id), \"principal\", principal.id) : undefined;",
                    g.types[t].named.alias,
                    q(&g.types[t].named.alias)
                ));
            }
        }
        // 6. the context: the inputs Cedar reads, and every value computed
        self.line(format!("  const context: Record<string, {}> = {{}};", self.names.value));
        for &i in &shape.actions[ai].inputs {
            let f = &a.inputs[i];
            let v = cedar_value(f, &format!("in{i}"), &self.ns(), self.g);
            if f.optional {
                self.line(format!("  if (in{i} != null) context[{}] = {v};", q(&f.named.alias)));
            } else {
                self.line(format!("  context[{}] = {v};", q(&f.named.alias)));
            }
        }
        for ci in 0..a.computed.len() {
            self.computed(ai, ci)?;
        }
        // the entities
        self.line(format!("  const entities: {}[] = [];", self.names.entity));
        if !g.roles.is_empty() {
            self.line("  for (const e of _ROLES) _add(entities, e);");
        }
        self.helpers.insert("add");
        self.line("  _add(entities, { uid: { type: _NS + principal.type, id: principal.id }, attrs: {}, parents: [] });");
        self.line("  _add(entities, { uid: { type: _NS + resourceType, id: resourceId }, attrs: {}, parents: [] });");
        for &t in &a.principals {
            if raw::reads(g, shape, t) {
                let al = &g.types[t].named.alias;
                self.line(format!("  if (p_{al} !== undefined) _entity_{al}(entities, principal.id, p_{al});"));
            }
        }
        for &t in &a.resources {
            if raw::reads(g, shape, t) {
                let al = &g.types[t].named.alias;
                self.line(format!("  if (r_{al} !== undefined) _entity_{al}(entities, resourceId, r_{al});"));
            }
        }
        self.line(format!(
            "  return {{ principal: {{ type: _NS + principal.type, id: principal.id }}, action: {{ type: _NS + \"Action\", id: {} }}, resource: {{ type: _NS + resourceType, id: resourceId }}, context, entities }};",
            q(&alias)
        ));
        self.line("}");
        self.line("");
        // the question
        let (doc, params, args) = match self.authorizer {
            Authorizer::Cedar => (
                self.t(tr!(
                    "action {alias} が許されるかを、このプロセスの Cedar（cedar-wasm）に尋ねる。拒んだ理由（{err}）と Cedar のエラーは拒む答えになる。Store の失敗は、そのまま投げる",
                    "Asks Cedar, in this process (cedar-wasm), whether the action {alias} is allowed. A refusal (a {err}) and an error of Cedar's are a deny; a failure of the store is thrown as it is"
                )),
                format!("store: {store}, principal: {principal}, input: {input_t}, now: Date = new Date()"),
                "",
            ),
            Authorizer::Avp => (
                self.t(tr!(
                    "action {alias} が許されるかを、Amazon Verified Permissions に尋ねる。拒んだ理由（{err}）と、尋ねられないこと、評価のエラーは拒む答えになる。Store の失敗は、そのまま投げる",
                    "Asks Amazon Verified Permissions whether the action {alias} is allowed. A refusal (a {err}), a question that cannot be asked and an error of the evaluation are a deny; a failure of the store is thrown as it is"
                )),
                format!("avp: {}, store: {store}, principal: {principal}, input: {input_t}, now: Date = new Date()", self.names.avp),
                "avp, ",
            ),
        };
        self.doc("", &doc);
        self.line(format!("export async function {}({params}): Promise<{}> {{", authorize_fn(&alias), self.names.answer));
        self.line(format!("  let req: {};", self.names.request));
        self.line("  try {");
        self.line(format!("    req = await {}(store, principal, input, now);", request_fn(&alias)));
        self.line("  } catch (e) {");
        self.line(format!("    if (e instanceof {err}) return {{ allowed: false, policies: [], error: e }};"));
        self.line("    throw e;");
        self.line("  }");
        match self.authorizer {
            Authorizer::Cedar => self.line("  return _ask(req);"),
            Authorizer::Avp => self.line(format!("  return _ask({args}req);")),
        }
        self.line("}");
        self.line("");
        Ok(())
    }

    /// The expression that checks a value of the data and gives it, or refuses with the kind the
    /// expression `kind` gives.
    fn check(&mut self, f: &Field, get: &str, kind: &str, what: &str) -> String {
        let g = self.g;
        let inner = match &f.ty {
            FieldType::Bool => {
                self.helpers.insert("bool");
                format!("_bool({get}, {kind}, {what})")
            }
            FieldType::Enum(e) => {
                self.helpers.insert("enum");
                format!("_enum({get}, _enum_{}, {kind}, {what}) as {}", g.enums[*e].named.alias, self.names.enums[*e])
            }
            FieldType::Num { lo, hi, .. } => {
                self.helpers.insert("num");
                format!("_num({get}, {lo}n, {hi}n, {kind}, {what})")
            }
            FieldType::Date { lo, hi } => {
                self.helpers.insert("date");
                format!("_date({get}, {}, {}, {kind}, {what})", q(&ritsu_ports::day_text(*lo)), q(&ritsu_ports::day_text(*hi)))
            }
            FieldType::Entity(_) => {
                self.helpers.insert("id");
                format!("_id({get}, {kind}, {what})")
            }
        };
        if f.optional { format!("{get} == null ? undefined : {inner}") } else { inner }
    }

    /// The code of a computed value: what it is given, the call, and the value Cedar is given.
    fn computed(&mut self, ai: usize, ci: usize) -> Result<(), String> {
        let g = self.g;
        let a = &g.actions[ai];
        let cv = &a.computed[ci];
        let kn = self.checked.known.computed.get(&(ai, ci)).ok_or_else(|| format!("no language answered for {}", cv.named.alias))?;
        // what it reads of the principal and the resource, and the types that have it
        let sources: Vec<Source> = match &cv.how {
            How::Rule { args, .. } | How::Date { of: DateOf::Call { args, .. }, .. } => args.iter().map(|(_, s)| s.clone()).collect(),
            How::Date { of: DateOf::Attr(o, n), .. } => vec![Source::Attr(*o, n.clone())],
            How::Open { .. } => Vec::new(),
        };
        let reads = |o: Owner| sources.iter().any(|s| matches!(s, Source::Attr(x, _) if *x == o));
        let has = |o: Owner, t: usize| sources.iter().all(|s| match s {
            Source::Attr(x, n) if *x == o => g.types[t].attr(n).is_some(),
            _ => true,
        });
        let pts: Vec<Option<usize>> = if reads(Owner::Principal) { a.principals.iter().copied().filter(|&t| has(Owner::Principal, t)).map(Some).collect() } else { vec![None] };
        let rts: Vec<Option<usize>> = if reads(Owner::Resource) { a.resources.iter().copied().filter(|&t| has(Owner::Resource, t)).map(Some).collect() } else { vec![None] };
        let what = self.computed_comment(ai, ci);
        self.comment("  ", &what);
        for pt in &pts {
            for rt in &rts {
                let mut conds: Vec<String> = Vec::new();
                if let Some(t) = pt {
                    conds.push(format!("p_{} !== undefined", g.types[*t].named.alias));
                }
                if let Some(t) = rt {
                    conds.push(format!("r_{} !== undefined", g.types[*t].named.alias));
                }
                let indent = if conds.is_empty() { "  " } else { "    " };
                if !conds.is_empty() {
                    self.line(format!("  if ({}) {{", conds.join(" && ")));
                }
                let body = self.computation(ai, ci, kn, *pt, *rt)?;
                for l in body {
                    self.line(format!("{indent}{l}"));
                }
                if !conds.is_empty() {
                    self.line("  }");
                }
            }
        }
        Ok(())
    }

    /// What a computed value is, as its comment says it: what computes it, and from what.
    fn computed_comment(&self, ai: usize, ci: usize) -> String {
        let g = self.g;
        let a = &g.actions[ai];
        let cv = &a.computed[ci];
        let path = |u: usize| g.uses.get(u).map(|u| ritsu_base::naming::quote(&u.path)).unwrap_or_default();
        let given = |args: &[(String, Source)], sep: &str| -> String { args.iter().map(|(n, s)| format!("{n}: {}", source_text(s))).collect::<Vec<_>>().join(sep) };
        let (what, args): (String, Option<&[(String, Source)]>) = match &cv.how {
            How::Rule { rule, args, output } => (format!("rulec {} output {output}", path(*rule)), Some(args)),
            How::Date { op, of: DateOf::Call { dates, function, args } } => (format!("today {} koyomi {} date {function}", op.symbol(), path(*dates)), Some(args)),
            How::Date { op, of: DateOf::Attr(o, n) } => (format!("today {} {}.{n}", op.symbol(), o.word()), None),
            How::Open { calendar } => (format!("today is open in koyomi {}", path(*calendar)), None),
        };
        let alias = &cv.named.alias;
        match args {
            Some(args) => {
                let (ja, en) = (given(args, "、"), given(args, ", "));
                self.t(tr!("{alias} = {what}。{ja} から計算する", "{alias} = {what}, from {en}"))
            }
            None => format!("{alias} = {what}"),
        }
    }

    /// The value a source gives in the code: an input's const, an attribute of the data of the
    /// principal's or the resource's type, today, or a constant.
    fn source(&self, ai: usize, s: &Source, pt: Option<usize>, rt: Option<usize>) -> Expr {
        let g = self.g;
        let a = &g.actions[ai];
        match s {
            Source::Attr(o, n) => {
                let t = if *o == Owner::Principal { pt } else { rt };
                let Some(t) = t else { return Expr::Absent };
                let Some((_, f)) = g.types[t].attr(n) else { return Expr::Absent };
                let v = if *o == Owner::Principal { "p" } else { "r" };
                Expr::Code(format!("{v}_{}.{}", g.types[t].named.alias, field(&f.named.alias)))
            }
            Source::Input(n) => match a.input(n) {
                Some((i, _)) => Expr::Code(format!("in{i}")),
                None => Expr::Absent,
            },
            Source::Today => Expr::Code("day".to_string()),
            Source::Lit(_) => Expr::Absent,
        }
    }

    /// Whether a source may be absent in the code (an optional attribute or input).
    fn optional(&self, ai: usize, s: &Source, pt: Option<usize>, rt: Option<usize>) -> bool {
        let g = self.g;
        match s {
            Source::Attr(o, n) => {
                let t = if *o == Owner::Principal { pt } else { rt };
                t.and_then(|t| g.types[t].attr(n)).is_some_and(|(_, f)| f.optional)
            }
            Source::Input(n) => g.actions[ai].input(n).is_some_and(|(_, f)| f.optional),
            _ => false,
        }
    }

    fn computation(&mut self, ai: usize, ci: usize, kn: &Kn, pt: Option<usize>, rt: Option<usize>) -> Result<Vec<String>, String> {
        let g = self.g;
        let a = &g.actions[ai];
        let cv = &a.computed[ci];
        let key = q(&cv.named.alias);
        let mut out: Vec<String> = Vec::new();
        let mut guards: Vec<String> = Vec::new();
        match (&cv.how, kn) {
            (How::Rule { rule, args, output }, Kn::Rule { domain, .. }) => {
                let facts = self.rule_facts(*rule)?;
                let call = &facts.typescript;
                let m = format!("rule_{}", call.module.trim_end_matches(".ts"));
                let mut params: Vec<String> = Vec::new();
                for p in &call.params {
                    // the gate's argument for this input of the rule: by the rule's name or its alias
                    let col = facts.inputs.iter().find(|c| c.name == p.name);
                    let Some((_, src)) = args.iter().find(|(n, _)| *n == p.name || col.is_some_and(|c| c.alias == *n) || *n == p.alias) else {
                        params.push("null".to_string());
                        continue;
                    };
                    let is_date = col.is_some_and(|c| matches!(&c.ty, ColumnType::Date) || matches!(&c.ty, ColumnType::Opt(x) if **x == ColumnType::Date));
                    let enum_ty = call.enums.iter().find(|e| e.alias == p.ty || format!("{} | null", e.alias) == p.ty);
                    let v = match src {
                        Source::Lit(Literal::Num(num)) => {
                            let unit = col.and_then(|c| match &c.ty {
                                ColumnType::Num { unit, .. } => unit.clone(),
                                ColumnType::Opt(x) => match x.as_ref() {
                                    ColumnType::Num { unit, .. } => unit.clone(),
                                    _ => None,
                                },
                                _ => None,
                            });
                            let k = unit.and_then(|u| crate::types::count(num, &u).ok()).ok_or_else(|| format!("{} is not a number of {}", num.raw, p.name))?;
                            format!("{k}n")
                        }
                        Source::Lit(Literal::Bool(b)) => b.to_string(),
                        Source::Lit(Literal::Date(d)) => {
                            self.helpers.insert("days");
                            format!("_days({})", q(&ritsu_ports::day_text(*d)))
                        }
                        Source::Lit(Literal::Word(w)) => {
                            let e = enum_ty.ok_or_else(|| format!("{w} is not a value of an enum of {}", p.name))?;
                            let rv = facts.enums.iter().find(|x| x.alias == e.alias).and_then(|x| x.values.iter().find(|v| v.name == *w || v.alias == *w || v.public == *w)).ok_or_else(|| format!("{w} is not a value of {}", e.alias))?;
                            let member = e.values.iter().find(|(n, _)| *n == rv.name).map(|(_, m)| m.clone()).unwrap_or_default();
                            format!("{m}.{}.{member}", e.alias)
                        }
                        _ => {
                            let Expr::Code(code) = self.source(ai, src, pt, rt) else { return Ok(Vec::new()) };
                            if self.optional(ai, src, pt, rt) {
                                guards.push(format!("{code} != null"));
                            }
                            if is_date {
                                self.helpers.insert("days");
                                format!("_days({code})")
                            } else if let Some(e) = enum_ty {
                                // the gate's value, as the rule's member that names it
                                let table = self.enum_table(ai, src, facts, e)?;
                                self.helpers.insert("to");
                                format!("_to({table}, {code})")
                            } else if p.ty == "bigint" || p.ty == "boolean" || p.ty == "bigint | null" || p.ty == "boolean | null" {
                                code
                            } else {
                                format!("{code} as {m}.{}", p.ty.trim_end_matches(" | null"))
                            }
                        }
                    };
                    params.push(v);
                }
                let out_param = call.outputs.iter().find(|o| o.name == *output || o.alias == *output);
                let pick = if call.outputs.len() > 1 { format!(".{}", out_param.map(|o| o.alias.clone()).unwrap_or_default()) } else { String::new() };
                self.helpers.insert("rule");
                let invoke = format!("_rule(() => {m}.{}({}){pick})", call.function, params.join(", "));
                let value = match domain {
                    Domain::Bool => invoke,
                    Domain::Enum(vs) => {
                        // the rule's member as the public name Cedar is given
                        let e = out_param.and_then(|o| call.enums.iter().find(|e| e.alias == o.ty)).ok_or_else(|| format!("the output {output} has no enum"))?;
                        let pairs: Vec<String> = e
                            .values
                            .iter()
                            .filter_map(|(name, member)| {
                                let public = facts.enums.iter().find(|x| x.alias == e.alias)?.values.iter().find(|v| v.name == *name)?.public.clone();
                                vs.iter().any(|v| v.alias == public).then(|| format!("[{m}.{}.{member}]: {}", e.alias, q(&public)))
                            })
                            .collect();
                        self.helpers.insert("public");
                        format!("_public({{ {} }}, {invoke})", pairs.join(", "))
                    }
                };
                out.push(format!("context[{key}] = {value};"));
            }
            (How::Date { op, of: DateOf::Call { dates, function, args } }, Kn::Date { .. }) => {
                let facts = self.dates_facts(*dates)?;
                let func = facts.functions.iter().find(|f| f.name == *function || f.alias == *function).ok_or_else(|| format!("no date {function}"))?;
                let mut params: Vec<String> = Vec::new();
                for pname in &func.params {
                    let inp = facts.inputs.iter().find(|i| i.name == *pname);
                    let Some((_, src)) = args.iter().find(|(n, _)| *n == *pname || inp.is_some_and(|i| i.alias == *n)) else {
                        return Err(format!("the date {function} is not given {pname}"));
                    };
                    let kind = inp.map(|i| i.kind).unwrap_or(DateKind::Date);
                    let v = match src {
                        Source::Lit(Literal::Date(d)) => q(&ritsu_ports::day_text(*d)),
                        Source::Lit(Literal::Num(num)) if num.value.is_int() => num.value.num.to_string(),
                        Source::Lit(_) => return Err(format!("{pname} is given a constant koyomi does not take")),
                        _ => {
                            let Expr::Code(code) = self.source(ai, src, pt, rt) else { return Ok(Vec::new()) };
                            if self.optional(ai, src, pt, rt) {
                                guards.push(format!("{code} != null"));
                            }
                            if kind == DateKind::Int { format!("Number({code})") } else { code }
                        }
                    };
                    params.push(v);
                }
                self.helpers.insert("dated");
                out.push(format!("context[{key}] = day {} _dated(() => dates_{}.{}({}));", ts_op(*op), facts.alias, func.alias, params.join(", ")));
            }
            (How::Date { op, of: DateOf::Attr(o, n) }, Kn::Attr) => {
                let Expr::Code(code) = self.source(ai, &Source::Attr(*o, n.clone()), pt, rt) else { return Ok(Vec::new()) };
                if self.optional(ai, &Source::Attr(*o, n.clone()), pt, rt) {
                    guards.push(format!("{code} != null"));
                }
                out.push(format!("context[{key}] = day {} {code};", ts_op(*op)));
            }
            (How::Open { calendar }, Kn::Open { .. }) => {
                let a = self.calendar_alias(*calendar)?;
                self.helpers.insert("dated");
                out.push(format!("context[{key}] = _dated(() => dates_{a}.is_open(day));"));
            }
            _ => return Err(format!("no language answered for {}", cv.named.alias)),
        }
        if guards.is_empty() {
            Ok(out)
        } else {
            let mut wrapped = vec![format!("if ({}) {{", guards.join(" && "))];
            wrapped.extend(out.into_iter().map(|l| format!("  {l}")));
            wrapped.push("}".to_string());
            Ok(wrapped)
        }
    }

    /// A table from the gate's values of an enum to the members of a rule's enum, for an input
    /// of the rule given the gate's enum.
    fn enum_table(&mut self, ai: usize, src: &Source, facts: &RuleFacts, e: &ritsu_ports::CallEnum) -> Result<String, String> {
        let g = self.g;
        let a = &g.actions[ai];
        let field = match src {
            Source::Attr(o, n) => {
                let types = if *o == Owner::Principal { &a.principals } else { &a.resources };
                types.iter().find_map(|&t| g.types[t].attr(n).map(|(_, f)| f.clone()))
            }
            Source::Input(n) => a.input(n).map(|(_, f)| f.clone()),
            _ => None,
        };
        let Some(Field { ty: FieldType::Enum(ge), .. }) = field else { return Err("an enum input of a rule is given what is not an enum".into()) };
        let m = format!("rule_{}", facts.typescript.module.trim_end_matches(".ts"));
        let re = facts.enums.iter().find(|x| x.alias == e.alias).ok_or("no enum")?;
        let mut pairs = Vec::new();
        for v in &g.enums[ge].values {
            let rv = re.values.iter().find(|x| x.name == v.name || x.alias == v.name || x.public == v.name || x.name == v.alias || x.alias == v.alias || x.public == v.alias);
            if let Some(rv) = rv
                && let Some((_, member)) = e.values.iter().find(|(n, _)| *n == rv.name)
            {
                pairs.push(format!("{}: {m}.{}.{member}", q(&v.alias), e.alias));
            }
        }
        Ok(format!("{{ {} }}", pairs.join(", ")))
    }

    // -----------------------------------------------------------------------------------------
    // Asking, and the helpers

    fn ask(&mut self) {
        let (err, request, answer) = (self.names.error.clone(), self.names.request.clone(), self.names.answer.clone());
        match self.authorizer {
            Authorizer::Cedar => {
                let fail = self.t(tr!("Cedar が答えませんでした", "Cedar did not answer"));
                let errs = self.t(tr!("Cedar がポリシーの評価でエラーを言いました", "Cedar met an error evaluating a policy"));
                let doc = self.t(tr!(
                    "Cedar に尋ねる。Cedar のエラーは、拒む答え（種類は cedar）にする。評価の途中でエラーになったポリシーを Cedar は当てはまらなかったものとして扱い、forbid が効かずに許すことがあるからである",
                    "Asks Cedar. An error of Cedar's is a deny (of the kind cedar): Cedar takes a policy whose evaluation fails as one that does not apply, and a forbid that fails could let a permit allow"
                ));
                self.doc("", &doc);
                self.line(format!("function _ask(req: {request}): {answer} {{"));
                self.line("  const a = cedar.isAuthorized({ principal: req.principal, action: req.action, resource: req.resource, context: req.context, entities: req.entities, policies: POLICIES, schema: SCHEMA, validateRequest: true });");
                self.line(format!("  if (a.type !== \"success\") return {{ allowed: false, policies: [], context: req.context, error: new {err}(\"cedar\", `{fail}: ${{a.errors.map((e) => e.message).join(\"; \")}}`) }};"));
                self.line(format!("  if (a.response.diagnostics.errors.length > 0) return {{ allowed: false, policies: [], context: req.context, error: new {err}(\"cedar\", `{errs}: ${{a.response.diagnostics.errors.map((e) => e.error.message).join(\"; \")}}`) }};"));
                self.line("  return { allowed: a.response.decision === \"allow\", policies: [...a.response.diagnostics.reason].sort(), context: req.context };");
                self.line("}");
            }
            Authorizer::Avp => {
                let fail = self.t(tr!("Verified Permissions に尋ねられませんでした", "Verified Permissions could not be asked"));
                let errs = self.t(tr!("Verified Permissions がポリシーの評価でエラーを言いました", "Verified Permissions met an error evaluating a policy"));
                let doc = self.t(tr!(
                    "Verified Permissions に尋ねる。尋ねられないとき、評価でエラーを言ったときは、拒む答え（種類は cedar）にする。決めたポリシーは、ポリシーストアが付けた ID で返る",
                    "Asks Verified Permissions. When it cannot be asked, or says an evaluation failed, the answer is a deny (of the kind cedar). The policies that decide come back by the ids the policy store gave them"
                ));
                self.doc("", &doc);
                self.line(format!("async function _ask(avp: {}, req: {request}): Promise<{answer}> {{", self.names.avp));
                self.line("  let out;");
                self.line("  try {");
                self.line("    out = await avp.client.send(");
                self.line("      new IsAuthorizedCommand({");
                self.line("        policyStoreId: avp.policyStoreId,");
                self.line("        principal: { entityType: req.principal.type, entityId: req.principal.id },");
                self.line("        action: { actionType: req.action.type, actionId: req.action.id },");
                self.line("        resource: { entityType: req.resource.type, entityId: req.resource.id },");
                self.line("        context: { cedarJson: JSON.stringify(req.context) },");
                self.line("        entities: { cedarJson: JSON.stringify(req.entities) },");
                self.line("      }),");
                self.line("    );");
                self.line("  } catch (e) {");
                self.line(format!("    return {{ allowed: false, policies: [], context: req.context, error: new {err}(\"cedar\", `{fail}: ${{e instanceof Error ? e.message : String(e)}}`) }};"));
                self.line("  }");
                self.line(format!("  if (out.errors !== undefined && out.errors.length > 0) return {{ allowed: false, policies: [], context: req.context, error: new {err}(\"cedar\", `{errs}: ${{out.errors.map((e) => e.errorDescription).join(\"; \")}}`) }};"));
                self.line("  const policies = (out.determiningPolicies ?? []).map((p) => p.policyId ?? \"\").sort();");
                self.line("  return { allowed: out.decision === \"ALLOW\", policies, context: req.context };");
                self.line("}");
            }
        }
        self.line("");
    }

    fn write_helpers(&mut self) {
        let g = self.g;
        let shape = self.shape;
        let lang = self.lang;
        let msg = |t: Text| t.get(lang).to_string();
        // the module's own types, by the names they came out with (names)
        let (err, ek, ent, val) = (self.names.error.clone(), self.names.error_kind.clone(), self.names.entity.clone(), self.names.value.clone());
        // each entity type the code reads: its check, and the entity Cedar is given
        for (ti, t) in g.types.iter().enumerate() {
            if !raw::reads(g, shape, ti) {
                continue;
            }
            let al = t.named.alias.clone();
            let ty = self.names.types[ti].clone();
            let none = msg(tr!("Store に {al} ${{id}} がありません", "the store holds no {al} ${{id}}"));
            self.line(format!("function _read_{al}(e: {ty} | undefined | null, kind: {ek}, id: string): {ty} {{"));
            self.line(format!("  if (e == null) return _fail(kind, `{none}`);"));
            if !t.roles.is_empty() {
                self.helpers.insert("roles");
                self.line(format!("  _roles(e.roles, _roles_{al}, kind, `{al} ${{id}}`);"));
            }
            if !shape.types[ti].member_of.is_empty() {
                let gs: Vec<String> = shape.types[ti].member_of.iter().map(|&m| q(&g.types[m].named.alias)).collect();
                self.helpers.insert("groups");
                self.line(format!("  _groups(e.member_of, [{}], kind, `{al} ${{id}}`);", gs.join(", ")));
            }
            for i in raw::store_attrs(g, shape, ti) {
                let f = t.attrs[i].clone();
                let get = format!("e.{}", field(&f.named.alias));
                let mut check = self.check(&f, &get, "kind", &format!("`{al} ${{id}}: {}`", f.named.alias));
                // a statement needs no type
                if let Some(i) = check.rfind(") as ") {
                    check.truncate(i + 1);
                }
                self.line(format!("  {check};"));
            }
            self.line("  return e;");
            self.line("}");
            self.line("");
            // the entity, and those its attributes point to that hold no attribute in the schema
            let role_t = format!("{}{}", self.ns(), cedar::ROLE);
            self.line(format!("function _entity_{al}(entities: {ent}[], id: string, e: {ty}): void {{"));
            self.line(format!("  const attrs: Record<string, {val}> = {{}};"));
            let mut pointed: Vec<(String, String)> = Vec::new();
            for &i in &shape.types[ti].attrs {
                let f = &t.attrs[i];
                let v = cedar_value(f, &format!("e.{}", field(&f.named.alias)), &self.ns(), g);
                if f.optional {
                    self.line(format!("  if (e.{} != null) attrs[{}] = {v};", field(&f.named.alias), q(&f.named.alias)));
                } else {
                    self.line(format!("  attrs[{}] = {v};", q(&f.named.alias)));
                }
                if let FieldType::Entity(target) = f.ty
                    && shape.types[target].attrs.is_empty()
                {
                    pointed.push((format!("{}{}", self.ns(), g.types[target].named.alias), format!("e.{}", field(&f.named.alias))));
                }
            }
            let mut parents: Vec<String> = Vec::new();
            if !t.roles.is_empty() {
                parents.push(format!("...e.roles.map((r) => ({{ type: {}, id: r }}))", q(&role_t)));
            }
            if !shape.types[ti].member_of.is_empty() {
                parents.push("...e.member_of.map((m) => ({ type: _NS + m.type, id: m.id }))".to_string());
            }
            // the entity itself replaces the one with no attribute put first for it
            self.line(format!("  const i = entities.findIndex((x) => x.uid.type === {} && x.uid.id === id);", q(&format!("{}{al}", self.ns()))));
            self.line(format!("  const it: {ent} = {{ uid: {{ type: {}, id }}, attrs, parents: [{}] }};", q(&format!("{}{al}", self.ns())), parents.join(", ")));
            self.line("  if (i >= 0) entities[i] = it;");
            self.line("  else entities.push(it);");
            for (pt, get) in pointed {
                self.line(format!("  if ({get} != null) _add(entities, {{ uid: {{ type: {}, id: {get} }}, attrs: {{}}, parents: [] }});", q(&pt)));
            }
            self.line("}");
            self.line("");
        }
        let h = |s: &str| self.helpers.contains(s);
        let mut text = format!("function _fail(kind: {ek}, message: string): never {{\n  throw new {err}(kind, message);\n}}\n\n");
        if h("today") {
            let t = g.today.as_ref().expect("today");
            let (lo, hi) = (ritsu_ports::day_text(t.lo), ritsu_ports::day_text(t.hi));
            // the file as the Python and the Go say it
            let shown = ritsu_base::naming::quote(&ritsu_emit::header::file_name(&g.file));
            let m = msg(tr!("today ${{day}} は、{shown} を確かめた範囲 {lo}..{hi} の外です", "today ${{day}} is outside {lo}..{hi}, the range {shown} was checked over"));
            let sign = if t.offset < 0 { '-' } else { '+' };
            let off = format!("{sign}{:02}:{:02}", t.offset.abs() / 60, t.offset.abs() % 60);
            let doc = msg(tr!("now の日。日は {off} で変わる", "The day of now: the day changes at {off}"));
            let _ = writeln!(text, "/** {doc} */");
            let _ = write!(text, "function _today(now: Date): string {{\n  const day = new Date(now.getTime() + ({}) * 60000).toISOString().slice(0, 10);\n  if (day < {} || day > {}) _fail(\"today\", `{m}`);\n  return day;\n}}\n\n", t.offset, q(&lo), q(&hi));
        }
        if h("id") {
            let m = msg(tr!("${{what}} は ID（文字列）ではありません", "${{what}} is not an id (a string)"));
            let _ = write!(text, "function _id(v: unknown, kind: {ek}, what: string): string {{\n  if (typeof v !== \"string\") return _fail(kind, `{m}`);\n  return v;\n}}\n\n");
        }
        if h("bool") {
            let m = msg(tr!("${{what}} は真偽ではありません", "${{what}} is not a bool"));
            let _ = write!(text, "function _bool(v: unknown, kind: {ek}, what: string): boolean {{\n  if (typeof v !== \"boolean\") return _fail(kind, `{m}`);\n  return v;\n}}\n\n");
        }
        if h("num") {
            let m = msg(tr!("${{what}} ${{String(v)}} は ${{lo}}〜${{hi}} の整数（bigint）ではありません", "${{what}} ${{String(v)}} is not a whole number (a bigint) of ${{lo}} to ${{hi}}"));
            let _ = write!(text, "function _num(v: unknown, lo: bigint, hi: bigint, kind: {ek}, what: string): bigint {{\n  if (typeof v !== \"bigint\" || v < lo || v > hi) return _fail(kind, `{m}`);\n  return v;\n}}\n\n");
        }
        if h("enum") {
            let m = msg(tr!("${{what}} ${{String(v)}} は ${{values.join(\"、\")}} のどれでもありません", "${{what}} ${{String(v)}} is none of ${{values.join(\", \")}}"));
            let _ = write!(text, "function _enum(v: unknown, values: readonly string[], kind: {ek}, what: string): string {{\n  if (typeof v !== \"string\" || !values.includes(v)) return _fail(kind, `{m}`);\n  return v;\n}}\n\n");
        }
        if h("date") {
            let m = msg(tr!("${{what}} ${{String(v)}} は ${{lo}}〜${{hi}} の日（YYYY-MM-DD）ではありません", "${{what}} ${{String(v)}} is not a day (YYYY-MM-DD) of ${{lo}} to ${{hi}}"));
            let _ = write!(
                text,
                "function _date(v: unknown, lo: string, hi: string, kind: {ek}, what: string): string {{\n  if (typeof v !== \"string\" || !/^\\d{{4}}-\\d{{2}}-\\d{{2}}$/.test(v) || new Date(v + \"T00:00:00Z\").toISOString().slice(0, 10) !== v || v < lo || v > hi) return _fail(kind, `{m}`);\n  return v;\n}}\n\n"
            );
        }
        if h("roles") {
            let m = msg(tr!("${{what}} の役割 ${{r}} は、その型が持てる役割ではありません", "${{what}} holds ${{r}}, which is not a role of its type"));
            let n = msg(tr!("${{what}} の役割が並びではありません", "the roles of ${{what}} are not a list"));
            let _ = write!(text, "function _roles(v: unknown, roles: readonly string[], kind: {ek}, what: string): void {{\n  if (!Array.isArray(v)) return _fail(kind, `{n}`);\n  for (const r of v) if (typeof r !== \"string\" || !roles.includes(r)) _fail(kind, `{m}`);\n}}\n\n");
        }
        if h("groups") {
            let m = msg(tr!("${{what}} のグループが、メンバーになれる型（${{types.join(\"、\")}}）の ID ではありません", "a group of ${{what}} is not the id of a type it can be a member of (${{types.join(\", \")}})"));
            let _ = write!(
                text,
                "function _groups(v: unknown, types: readonly string[], kind: {ek}, what: string): void {{\n  if (!Array.isArray(v)) return _fail(kind, `{m}`);\n  for (const x of v as unknown[]) {{\n    const g = x as {{ type?: unknown; id?: unknown }} | null;\n    if (g == null || typeof g.type !== \"string\" || !types.includes(g.type) || typeof g.id !== \"string\") _fail(kind, `{m}`);\n  }}\n}}\n\n"
            );
        }
        if h("days") {
            let doc = msg(tr!("rulec の生成物が受け取る形の日（YYYY-MM-DD）：1970-01-01 からの日数", "A day (YYYY-MM-DD) as rulec's code takes it: the days since 1970-01-01"));
            let _ = write!(text, "/** {doc} */\nfunction _days(day: string): bigint {{\n  return BigInt(Date.parse(day + \"T00:00:00Z\") / 86400000);\n}}\n\n");
        }
        if h("to") {
            let m = msg(tr!("規則の列挙に ${{v}} がありません", "the rule's enum has no ${{v}}"));
            let _ = write!(text, "function _to<T>(table: Record<string, T>, v: string): T {{\n  const x = table[v];\n  if (x === undefined) return _fail(\"rule\", `{m}`);\n  return x;\n}}\n\n");
        }
        if h("rule") {
            let _ = write!(text, "function _rule<T>(f: () => T): T {{\n  try {{\n    return f();\n  }} catch (e) {{\n    return _fail(\"rule\", e instanceof Error ? e.message : String(e));\n  }}\n}}\n\n");
        }
        if h("public") {
            let m = msg(tr!("規則の答え ${{String(v)}} の公開名がありません", "the rule's answer ${{String(v)}} has no public name"));
            let _ = write!(text, "function _public(table: Record<string, string>, v: unknown): string {{\n  const x = table[String(v)];\n  if (x === undefined) return _fail(\"rule\", `{m}`);\n  return x;\n}}\n\n");
        }
        if h("dated") {
            let _ = write!(text, "function _dated<T>(f: () => T): T {{\n  try {{\n    return f();\n  }} catch (e) {{\n    return _fail(\"date\", e instanceof Error ? e.message : String(e));\n  }}\n}}\n\n");
        }
        if h("add") {
            let doc = msg(tr!("エンティティを入れる。同じ型と ID のものがあれば入れない", "Puts an entity in, unless one of its type and id is there"));
            let _ = write!(text, "/** {doc} */\nfunction _add(entities: {ent}[], e: {ent}): void {{\n  if (!entities.some((x) => x.uid.type === e.uid.type && x.uid.id === e.uid.id)) entities.push(e);\n}}\n\n");
        }
        self.out.push_str(text.trim_end());
        self.out.push('\n');
    }
}

/// A property of an object literal type: the name, quoted when it is not an identifier.
fn prop(name: &str) -> String {
    if ritsu_emit::ident::is_ascii_ident(name) { name.to_string() } else { q(name) }
}

/// The value Cedar is given for a field's value in the code: a number as a JSON number, an enum's
/// value and a bool as they are, an entity as `{ __entity }`.
fn cedar_value(f: &Field, get: &str, ns: &str, g: &Gate) -> String {
    match &f.ty {
        FieldType::Num { .. } => format!("Number({get})"),
        FieldType::Entity(t) => format!("{{ __entity: {{ type: {}, id: {get} }} }}", q(&format!("{ns}{}", g.types[*t].named.alias))),
        _ => get.to_string(),
    }
}

fn ts_op(op: crate::ast::Op) -> &'static str {
    use crate::ast::Op;
    match op {
        Op::Lt => "<",
        Op::Le => "<=",
        Op::Gt => ">",
        Op::Ge => ">=",
        Op::Is => "===",
    }
}

fn source_text(s: &Source) -> String {
    match s {
        Source::Attr(o, n) => format!("{}.{n}", o.word()),
        Source::Input(n) => n.clone(),
        Source::Lit(Literal::Num(n)) => n.raw.clone(),
        Source::Lit(Literal::Bool(b)) => b.to_string(),
        Source::Lit(Literal::Date(d)) => ritsu_ports::day_text(*d),
        Source::Lit(Literal::Word(w)) => w.clone(),
        Source::Today => "today".to_string(),
    }
}

/// Words of a text in lines of at most `width` characters.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in text.split(' ') {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
