//! The Python that builds a gate's requests and asks Cedar (DESIGN 5.3–5.6): one module,
//! `authz/<alias>.py` of a package `ritsu gen` writes, which imports the rules' and the dates'
//! modules from the package's `rules/` and `dates/`.
//!
//! For each action, `<action>_request(store, principal, [resource,] [input,] now)` reads the
//! principal and the resource from the `Store` the service implements, holds every value to the
//! range or the values the gate declares, computes the values of the context with the code rulec
//! and koyomi generate, and returns the request with the entities Cedar reads; it raises a
//! `SekishoError` of a kind (`principal`, `resource`, `input`, `today`, `rule`, `date`) when it
//! cannot (DESIGN 3.8). `authorize_<action>(…)` asks Cedar — cedarpy in the process, or Verified
//! Permissions through boto3 (`--authorizer avp`) — and answers allowed or not, the policies that
//! decided (their `@id`, or the ids Verified Permissions gave them), and the context it gave Cedar;
//! an error is a deny, with the error in the answer. Nothing the caller sends is put in the context
//! but the arguments of the operation the gate declares (DESIGN 3.6).

use super::plan::{self, ActionPlan, Authorizer, Call, Import, Options, Plan, ValuePlan};
use crate::ast::Op;
use crate::model::*;
use ritsu_base::text::Lang;
use ritsu_emit::header::Comment;
use ritsu_emit::lit;
use std::collections::BTreeMap;

/// The module's path under the package: `authz/<alias>.py`.
pub fn path(alias: &str) -> String {
    format!("authz/{alias}.py")
}

/// A string of Python.
fn s(text: &str) -> String {
    lit::python(text)
}

/// Text in the language of the comments.
fn tr(o: &Options, ja: &str, en: &str) -> String {
    if o.lang == Lang::Ja { ja.to_string() } else { en.to_string() }
}

/// The names the module declares, kept apart from one another and from Python's words.
struct Names {
    principal: String,
    store: String,
    error: String,
    request: String,
    answer: String,
    role: String,
    avp: String,
    /// Each type's record, by index.
    records: BTreeMap<usize, String>,
    /// Each enum's `Literal`, by index.
    enums: BTreeMap<usize, String>,
    /// Each action's input, by index.
    inputs: BTreeMap<usize, String>,
}

fn names(p: &Plan) -> Names {
    let g = p.g;
    let mut wanted: Vec<String> = Vec::new();
    let types = p.types_used();
    let recorded: Vec<usize> = types.iter().copied().filter(|&t| p.reads(t)).collect();
    for &t in &recorded {
        wanted.push(g.types[t].named.alias.clone());
    }
    for e in &g.enums {
        wanted.push(plan::pascal(&e.named.alias));
    }
    for w in ["Principal", "Store", "SekishoError", "Request", "Answer", "Role", "VerifiedPermissions"] {
        wanted.push(w.to_string());
    }
    for a in &g.actions {
        wanted.push(format!("{}Input", plan::pascal(&a.named.alias)));
    }
    let taken: Vec<&str> = ritsu_emit::words::python::KEYWORDS.iter().chain(["Literal", "Protocol", "TypedDict", "NotRequired", "dataclass", "date", "datetime", "timedelta", "timezone", "cedarpy", "json"].iter()).copied().collect();
    let got = ritsu_emit::ident::unique(wanted, &taken);
    let mut it = got.into_iter();
    let mut records = BTreeMap::new();
    for &t in &recorded {
        records.insert(t, it.next().unwrap_or_default());
    }
    let mut enums = BTreeMap::new();
    for i in 0..g.enums.len() {
        enums.insert(i, it.next().unwrap_or_default());
    }
    let mut fixed: Vec<String> = (0..7).map(|_| it.next().unwrap_or_default()).collect();
    let mut inputs = BTreeMap::new();
    for i in 0..g.actions.len() {
        inputs.insert(i, it.next().unwrap_or_default());
    }
    let avp = fixed.pop().unwrap_or_default();
    let role = fixed.pop().unwrap_or_default();
    let answer = fixed.pop().unwrap_or_default();
    let request = fixed.pop().unwrap_or_default();
    let error = fixed.pop().unwrap_or_default();
    let store = fixed.pop().unwrap_or_default();
    let principal = fixed.pop().unwrap_or_default();
    Names { principal, store, error, request, answer, role, avp, records, enums, inputs }
}

/// The fields of a type's record: its roles, the groups it is a member of, and its attributes, each
/// with its name in the record. An attribute keeps its alias; `roles` and the groups' fields move
/// aside from one.
struct Record {
    roles: Option<String>,
    groups: Vec<(usize, String)>,
    attrs: Vec<String>,
}

fn record(p: &Plan, t: usize) -> Record {
    let ty = &p.g.types[t];
    let attrs: Vec<String> = ty.attrs.iter().map(|f| f.named.alias.clone()).collect();
    let taken = |n: &str| attrs.iter().any(|a| a == n) || ritsu_emit::words::python::KEYWORDS.contains(&n);
    let mut used: Vec<String> = Vec::new();
    let mut fresh = |n: String| -> String {
        let mut m = n.clone();
        while taken(&m) || used.contains(&m) {
            m.push('_');
        }
        used.push(m.clone());
        m
    };
    let roles = (!ty.roles.is_empty()).then(|| fresh("roles".to_string()));
    let groups = p.groups(t).iter().map(|&gt| (gt, fresh(format!("member_of_{}", plan::snake(&p.g.types[gt].named.alias))))).collect();
    Record { roles, groups, attrs }
}

/// What the module is: its text, for a file that passed its check.
pub fn module(p: &Plan, o: &Options) -> String {
    let g = p.g;
    let n = names(p);
    let mut t = String::new();
    // the head
    let f = p.scope.file();
    let sha = ritsu_base::sha256::hex(f.src.as_bytes());
    let version = f.version.strip_prefix('v').unwrap_or(&f.version);
    let source = ritsu_emit::header::Source { path: &o.shown, kind: "gate", name: &f.name.text, version, sha256: &sha };
    t.push_str(&Comment::Hash.line(&ritsu_emit::header::generated("sekisho")));
    t.push_str(&Comment::Hash.line(source.line().get(o.lang)));
    // the docstring
    let mut doc: Vec<String> = Vec::new();
    if let Some((d, _)) = &f.description {
        doc.push(d.clone());
        doc.push(String::new());
    }
    doc.push(tr(
        o,
        "このモジュールが組み立てるリクエストだけが、このゲートへの尋ね方です。context に入るのは、操作の引数（ハンドラーが解析したもの）と、サービスのデータ（Store が読むもの）から ritsu の言語が計算した値だけで、呼ぶ側が送ってきた context は入りません。",
        "The requests this module builds are the only way the gate is meant to be asked: the context holds the arguments of the operation, as the handler parsed them, and what ritsu's languages compute from the service's own data, which the Store reads. Nothing in it is taken from a context the caller sends.",
    ));
    doc.push(String::new());
    doc.push(match o.authorizer {
        Authorizer::Cedar => tr(o, "Cedar には、このプロセスの中で cedarpy で尋ねます。ポリシーとスキーマは、このモジュールが持っています。", "Cedar is asked in this process, through cedarpy; the module holds the policies and the schema."),
        Authorizer::Avp => tr(
            o,
            "Cedar には、Amazon Verified Permissions で尋ねます（boto3）。ポリシーとスキーマは、gen --target cedar が書いたものを、ポリシーストアに置いてください。決めたポリシーは、ストアが付けた ID で返ります。",
            "Cedar is asked through Amazon Verified Permissions (boto3): put the policies and the schema gen --target cedar writes into the policy store. The policies that decide are the ids the store gave them.",
        ),
    });
    doc.push(tr(o, "規則と日付のモジュールは、パッケージの rules/ と dates/ から読みます（ritsu gen が書きます）。", "The modules of the rules and the dates are the package's own rules/ and dates/, which ritsu gen writes."));
    t.push_str(&py_doc(&doc, ""));
    t.push('\n');
    // the imports
    let has_date = g.types.iter().any(|ty| ty.attrs.iter().any(|f| matches!(f.ty, FieldType::Date { .. }))) || g.actions.iter().any(|a| a.inputs.iter().any(|f| matches!(f.ty, FieldType::Date { .. })));
    t.push_str("from __future__ import annotations\n\n");
    if o.authorizer == Authorizer::Avp {
        t.push_str("import json\n");
    }
    t.push_str("from dataclasses import dataclass\n");
    let today = p.today().is_some() && p.actions.iter().any(|a| a.reads_today);
    let mut dt = vec!["datetime"];
    if has_date || today {
        dt.insert(0, "date");
    }
    if today {
        dt.extend(["timedelta", "timezone"]);
    }
    t.push_str(&format!("from datetime import {}\n", dt.join(", ")));
    let mut typing = vec!["Literal", "Protocol", "TypedDict"];
    if g.actions.iter().any(|a| a.inputs.iter().any(|f| f.optional)) {
        typing.insert(1, "NotRequired");
    }
    if o.authorizer == Authorizer::Avp {
        typing.push("TYPE_CHECKING");
    }
    t.push_str(&format!("from typing import {}\n\n", typing.join(", ")));
    match o.authorizer {
        Authorizer::Cedar => t.push_str("import cedarpy\n"),
        Authorizer::Avp => t.push_str("if TYPE_CHECKING:\n    from mypy_boto3_verifiedpermissions import VerifiedPermissionsClient\n"),
    }
    let mut imports: Vec<String> = Vec::new();
    for (ui, imp) in &p.imports {
        imports.push(match imp {
            Import::Rule(r) => format!("from ..rules import {} as {}\n", r.python.module, module_var(p, *ui)),
            Import::Dates(d) => format!("from ..dates import {} as {}\n", d.alias, module_var(p, *ui)),
            Import::Calendar(c) => format!("from ..dates import {} as {}\n", c.alias, module_var(p, *ui)),
        });
    }
    imports.sort();
    if !imports.is_empty() {
        t.push('\n');
        for i in imports {
            t.push_str(&i);
        }
    }
    t.push_str("\n\n");
    // the types
    types(p, &n, o, &mut t);
    helpers(p, &n, o, &mut t);
    for a in &p.actions {
        action(p, &n, o, a, &mut t);
    }
    ask(p, &n, o, &mut t);
    while t.ends_with("\n\n") {
        t.pop();
    }
    t
}

/// The name a module of another language is imported as: `_rules_refund_limit`.
fn module_var(p: &Plan, ui: usize) -> String {
    match p.imports.get(&ui) {
        Some(Import::Rule(r)) => format!("_rules_{}", r.python.module),
        Some(Import::Dates(d)) => format!("_dates_{}", d.alias),
        Some(Import::Calendar(c)) => format!("_dates_{}", c.alias),
        None => "_".to_string(),
    }
}

/// A docstring of the lines, at `indent`: `\` and `"""` written so that the text is what Python reads.
fn py_doc(lines: &[String], indent: &str) -> String {
    let esc = |l: &str| l.replace('\\', "\\\\").replace("\"\"\"", "\\\"\\\"\\\"");
    let mut out = String::new();
    let body: Vec<String> = lines.iter().map(|l| esc(&ritsu_emit::header::one_line(l))).collect();
    if body.len() == 1 {
        let one = &body[0];
        let one = if one.ends_with('"') { format!("{one} ") } else { one.clone() };
        out.push_str(&format!("{indent}\"\"\"{one}\"\"\"\n"));
        return out;
    }
    out.push_str(&format!("{indent}\"\"\"{}\n", body[0]));
    for l in &body[1..] {
        if l.is_empty() {
            out.push('\n');
        } else {
            out.push_str(&format!("{indent}{l}\n"));
        }
    }
    out.push_str(&format!("{indent}\"\"\"\n"));
    out
}

/// A field's type in Python.
fn field_type(n: &Names, f: &Field) -> String {
    let base = match &f.ty {
        FieldType::Bool => "bool".to_string(),
        FieldType::Enum(e) => n.enums.get(e).cloned().unwrap_or_else(|| "str".into()),
        FieldType::Num { .. } => "int".to_string(),
        FieldType::Date { .. } => "date".to_string(),
        FieldType::Entity(_) => "str".to_string(),
    };
    if f.optional { format!("{base} | None") } else { base }
}

/// A comment after a field: `  # money[GBP, incl_tax], 1 to 10000`.
fn field_comment(g: &Gate, f: &Field, lang: Lang) -> String {
    let note = plan::field_note(g, f, lang);
    if note.is_empty() { String::new() } else { format!("  # {}", ritsu_emit::header::one_line(&note)) }
}

fn types(p: &Plan, n: &Names, o: &Options, t: &mut String) {
    let g = p.g;
    // the roles and the enums
    if !g.roles.is_empty() {
        let vs: Vec<String> = g.roles.iter().map(|r| s(&r.named.alias)).collect();
        t.push_str(&format!("{} = Literal[{}]\n", n.role, vs.join(", ")));
    }
    for (i, e) in g.enums.iter().enumerate() {
        let vs: Vec<String> = e.values.iter().map(|v| s(&v.alias)).collect();
        t.push_str(&format!("{} = Literal[{}]\n", n.enums[&i], vs.join(", ")));
    }
    if !g.roles.is_empty() || !g.enums.is_empty() {
        t.push_str("\n\n");
    }
    // who asks
    let principals: Vec<usize> = p.types_used().into_iter().filter(|&x| g.types[x].kind != Kind::Resource).collect();
    let lits = |ts: &[usize]| ts.iter().map(|&x| s(&g.types[x].named.alias)).collect::<Vec<_>>().join(", ");
    t.push_str("@dataclass(frozen=True)\n");
    t.push_str(&format!("class {}:\n", n.principal));
    let mut d = vec![tr(o, "尋ねる者。サービスの認証が決めた型と ID です。", "Who asks, as the service's authentication found it: the type and the id.")];
    if !g.workflows.is_empty() {
        let ws: Vec<String> = g.workflows.iter().map(|w| w.named.alias.clone()).collect();
        d.push(tr(o, &format!("ワークフローは、名前を ID にします（{}）。", ws.join("、")), &format!("A workflow is asked as itself, its name the id ({}).", ws.join(", "))));
    }
    t.push_str(&py_doc(&[plan::sentences(&d)], "    "));
    t.push('\n');
    t.push_str(&format!("    type: Literal[{}]\n    id: str\n\n\n", if principals.is_empty() { s("") } else { lits(&principals) }));
    // the records
    for (&ti, name) in &n.records {
        let ty = &g.types[ti];
        let r = record(p, ti);
        t.push_str("@dataclass(frozen=True)\n");
        t.push_str(&format!("class {name}:\n"));
        let owner = if ty.kind == Kind::Resource { "resource" } else { "principal" };
        let mut d = vec![tr(o, &format!("{owner} {} について、ゲートが読むもの。", ty.named.alias), &format!("What the gate reads of the {owner} {}.", ty.named.alias))];
        if let Some(desc) = plan::type_description(p.scope, &ty.named.alias) {
            d.push(plan::sentence(&desc));
        }
        t.push_str(&py_doc(&[plan::sentences(&d)], "    "));
        t.push('\n');
        if let Some(roles) = &r.roles {
            let vs: Vec<String> = ty.roles.iter().map(|&x| g.roles[x].named.alias.clone()).collect();
            t.push_str(&format!("    {roles}: list[{}]  # {}\n", n.role, tr(o, &format!("{} のどれか", vs.join("、")), &format!("each one of {}", vs.join(", ")))));
        }
        for (gt, field) in &r.groups {
            let gn = &g.types[*gt].named.alias;
            t.push_str(&format!("    {field}: list[str]  # {}\n", tr(o, &format!("メンバーである {gn} の ID"), &format!("the ids of the {gn}s it is a member of"))));
        }
        for (i, f) in ty.attrs.iter().enumerate() {
            t.push_str(&format!("    {}: {}{}\n", r.attrs[i], field_type(n, f), field_comment(g, f, o.lang)));
        }
        t.push_str("\n\n");
    }
    // the store
    t.push_str(&format!("class {}(Protocol):\n", n.store));
    t.push_str(&py_doc(
        &[tr(
            o,
            "principal と resource について知っていることを、サービス自身のデータから読むところ。無ければ None を返します。",
            "Where the code reads what it knows of the principal and the resource: the service's own data. None for one it does not know.",
        )],
        "    ",
    ));
    if n.records.is_empty() {
        t.push_str("\n    pass\n");
    } else {
        t.push('\n');
        for (&ti, name) in &n.records {
            t.push_str(&format!("    def {}(self, id: str) -> {name} | None: ...\n", plan::snake(&g.types[ti].named.alias)));
        }
    }
    t.push_str("\n\n");
    // the inputs: the arguments the gate declares, and the resource's id (and its type, for an action
    // that takes more than one)
    for ap in &p.actions {
        let a = &g.actions[ap.index];
        let name = &n.inputs[&ap.index];
        let mut fields: Vec<(String, String, String)> = Vec::new();
        let rt = a.resources.iter().map(|&x| g.types[x].named.alias.clone()).collect::<Vec<_>>();
        let field = plan::resource_field(a);
        fields.push((field, "str".to_string(), format!("  # {}", tr(o, &format!("{} の ID", rt.join("、")), &format!("the id of the {}", rt.join(" or "))))));
        if a.resources.len() > 1 {
            fields.push(("resource_type".to_string(), format!("Literal[{}]", lits(&a.resources)), format!("  # {}", tr(o, "resource の型", "the type of the resource"))));
        }
        for f in &a.inputs {
            let ty = if f.optional { format!("NotRequired[{}]", field_type(n, &Field { optional: false, ..f.clone() })) } else { field_type(n, f) };
            fields.push((f.named.alias.clone(), ty, field_comment(g, f, o.lang)));
        }
        let operation = plan::guarded(p.g, a, o.lang);
        let d = match &operation {
            Some(op) => tr(o, &format!("操作（{op}）の引数。ハンドラーが解析したもので、操作はこれと同じ引数で行ってください。"), &format!("The arguments of the operation {op}, as the handler parsed them; the operation is to be done with the same.")),
            None => tr(o, &format!("{} のリクエストの引数。", a.named.alias), &format!("The arguments of the request of {}.", a.named.alias)),
        };
        let idents = fields.iter().all(|(k, _, _)| ritsu_emit::ident::is_ascii_ident(k) && !ritsu_emit::words::python::KEYWORDS.contains(&k.as_str()));
        if idents {
            t.push_str(&format!("class {name}(TypedDict):\n"));
            t.push_str(&py_doc(&[d], "    "));
            t.push('\n');
            for (k, ty, c) in &fields {
                t.push_str(&format!("    {k}: {ty}{c}\n"));
            }
        } else {
            t.push_str(&format!("# {}\n", ritsu_emit::header::one_line(&d)));
            let kv: Vec<String> = fields.iter().map(|(k, ty, _)| format!("{}: {ty}", s(k))).collect();
            t.push_str(&format!("{name} = TypedDict({}, {{{}}})\n", s(name), kv.join(", ")));
        }
        t.push_str("\n\n");
    }
    // the error, the request and the answer
    t.push_str(&format!("class {}(Exception):\n", n.error));
    t.push_str(&py_doc(
        &[tr(
            o,
            "ゲートに尋ねられなかった理由。このとき答えは「拒む」です。kind は principal、resource、input、today、rule、date（リクエストを組み立てられない）、cedar（Cedar がエラーを返したか、尋ねられなかった）のどれかです。",
            "Why the gate could not be asked, and the answer is a deny: kind is principal, resource, input, today, rule or date (the request could not be built), or cedar (Cedar answered with an error, or could not be asked).",
        )],
        "    ",
    ));
    t.push('\n');
    t.push_str("    def __init__(self, kind: Literal[\"principal\", \"resource\", \"input\", \"today\", \"rule\", \"date\", \"cedar\"], message: str) -> None:\n");
    t.push_str("        super().__init__(message)\n        self.kind = kind\n\n\n");
    t.push_str("@dataclass(frozen=True)\n");
    t.push_str(&format!("class {}:\n", n.request));
    t.push_str(&py_doc(&[tr(o, "Cedar に尋ねるもの。principal、action、resource、context と、ポリシーが読むエンティティ（Cedar の JSON の形）。", "What Cedar is asked: the principal, the action, the resource and the context, with the entities the policies read, in Cedar's JSON.")], "    "));
    t.push('\n');
    t.push_str("    principal: dict[str, str]\n    action: dict[str, str]\n    resource: dict[str, str]\n    context: dict[str, object]\n    entities: list[dict[str, object]]\n\n\n");
    t.push_str("@dataclass(frozen=True)\n");
    t.push_str(&format!("class {}:\n", n.answer));
    t.push_str(&py_doc(
        &[tr(
            o,
            "答え。許すか、決めたポリシー、Cedar に渡した context（判断の記録のため）、尋ねられなかったときはその理由。",
            "The answer: allowed or not, the policies that decided it, the context Cedar was given (for the log of decisions), and why, when the gate could not be asked.",
        )],
        "    ",
    ));
    t.push('\n');
    t.push_str(&format!("    allowed: bool\n    policies: list[str]\n    context: dict[str, object] | None = None\n    error: {} | None = None\n\n\n", n.error));
    if o.authorizer == Authorizer::Avp {
        t.push_str("@dataclass(frozen=True)\n");
        t.push_str(&format!("class {}:\n", n.avp));
        t.push_str(&py_doc(&[tr(o, "尋ねる先。Amazon Verified Permissions のクライアント（boto3 のもの）と、ゲートの Cedar を置いたポリシーストアの ID。", "Where the gate is asked: the client of Amazon Verified Permissions (boto3's), and the id of the policy store the gate's Cedar is in.")], "    "));
        t.push('\n');
        t.push_str("    client: VerifiedPermissionsClient\n    policy_store_id: str\n\n\n");
    }
}


/// What a field is held to, as a call of a helper of the module: `_int("input", "amount", x, 1, 10000)`.
fn check_call(g: &Gate, kind: &str, what: &str, f: &Field, value: &str) -> String {
    match &f.ty {
        FieldType::Num { lo, hi, .. } => format!("_int({}, {what}, {value}, {lo}, {hi})", s(kind)),
        FieldType::Enum(en) => {
            let vs: Vec<String> = g.enums[*en].values.iter().map(|v| s(&v.alias)).collect();
            format!("_enum({}, {what}, {value}, ({},))", s(kind), vs.join(", "))
        }
        FieldType::Date { lo, hi } => {
            let (ly, lm, ld) = plan::ymd(*lo);
            let (hy, hm, hd) = plan::ymd(*hi);
            format!("_date({}, {what}, {value}, date({ly}, {lm}, {ld}), date({hy}, {hm}, {hd}))", s(kind))
        }
        FieldType::Bool => format!("_bool({}, {what}, {value})", s(kind)),
        FieldType::Entity(_) => format!("_text({}, {what}, {value})", s(kind)),
    }
}

fn helpers(p: &Plan, n: &Names, o: &Options, t: &mut String) {
    let g = p.g;
    let e = &n.error;
    t.push_str("def _uid(type: str, id: str) -> dict[str, str]:\n    return {\"type\": type, \"id\": id}\n\n\n");
    // the roles as entities
    if !g.roles.is_empty() {
        t.push_str("_ROLES: list[dict[str, object]] = [\n");
        let role_type = p.cedar_name(crate::cedar::ROLE);
        for r in &g.roles {
            let parents: Vec<String> = r.includes.iter().map(|&x| format!("_uid({}, {})", s(&role_type), s(&g.roles[x].named.alias))).collect();
            t.push_str(&format!("    {{\"uid\": _uid({}, {}), \"attrs\": {{}}, \"parents\": [{}]}},\n", s(&role_type), s(&r.named.alias), parents.join(", ")));
        }
        t.push_str("]\n\n\n");
    }
    // what a value is held to: its type, and its range or its values
    let kinds = "Literal[\"principal\", \"resource\", \"input\"]";
    let not_a = |what: &str| tr(o, &format!("{{what}} {{value!r}} は{what}ではありません"), &format!("{{what}} {{value!r}} is not {what}"));
    let outside = tr(o, "{what} {value!r} が {lo}〜{hi} の外です", "{what} {value!r} is outside {lo} to {hi}");
    t.push_str(&format!(
        "def _int(kind: {kinds}, what: str, value: object, lo: int, hi: int) -> int:\n    if isinstance(value, bool) or not isinstance(value, int):\n        raise {e}(kind, f{})\n    if not lo <= value <= hi:\n        raise {e}(kind, f{})\n    return value\n\n\n",
        s(&not_a(&tr(o, "整数", "a whole number"))),
        s(&outside)
    ));
    t.push_str(&format!("def _bool(kind: {kinds}, what: str, value: object) -> bool:\n    if not isinstance(value, bool):\n        raise {e}(kind, f{})\n    return value\n\n\n", s(&not_a(&tr(o, "真偽", "a bool")))));
    t.push_str(&format!("def _text(kind: {kinds}, what: str, value: object) -> str:\n    if not isinstance(value, str):\n        raise {e}(kind, f{})\n    return value\n\n\n", s(&not_a(&tr(o, "文字列", "a string")))));
    t.push_str(&format!(
        "def _enum(kind: {kinds}, what: str, value: object, values: tuple[str, ...]) -> str:\n    if not isinstance(value, str) or value not in values:\n        listed = \", \".join(values)\n        raise {e}(kind, f{})\n    return value\n\n\n",
        s(&tr(o, "{what} {value!r} は {listed} のどれでもありません", "{what} {value!r} is none of {listed}"))
    ));
    let has_date = g.types.iter().any(|ty| ty.attrs.iter().any(|f| matches!(f.ty, FieldType::Date { .. }))) || g.actions.iter().any(|a| a.inputs.iter().any(|f| matches!(f.ty, FieldType::Date { .. })));
    if has_date {
        t.push_str(&format!(
            "def _date(kind: {kinds}, what: str, value: object, lo: date, hi: date) -> date:\n    if isinstance(value, datetime) or not isinstance(value, date):\n        raise {e}(kind, f{})\n    if not lo <= value <= hi:\n        raise {e}(kind, f{})\n    return value\n\n\n",
            s(&not_a(&tr(o, "日付", "a date"))),
            s(&outside)
        ));
    }
    if p.actions.iter().any(|a| a.values.iter().any(|v| matches!(&v.call, Call::Rule { args, .. } if args.iter().any(|(c, s)| matches!(plan::strip(&c.ty), ritsu_ports::ColumnType::Date) && s.is_some())))) {
        t.push_str(&format!("def _days(day: date) -> int:\n{}    return (day - date(1970, 1, 1)).days\n\n\n", py_doc(&[tr(o, "rulec のコードが受け取る日：1970-01-01 からの日数。", "A day as rulec's code takes it: the days since 1970-01-01.")], "    ")));
    }
    if let Some(td) = p.today() {
        let (ly, lm, ld) = plan::ymd(td.lo);
        let (hy, hm, hd) = plan::ymd(td.hi);
        let off = td.offset;
        let shown = ritsu_base::naming::quote(&ritsu_emit::header::file_name(&g.file));
        let (lo, hi) = (ritsu_ports::day_text(td.lo), ritsu_ports::day_text(td.hi));
        let at = plan::offset_text(off);
        t.push_str("def _today(now: datetime | None) -> date:\n");
        t.push_str(&py_doc(
            &[tr(o, &format!("now の日（日は UTC{at} で変わる）。{shown} を確かめた範囲 {lo}..{hi} の外ならエラーです。now を渡さなければ、いまの時刻です。"), &format!("The day of now, the day changing at {at}; an error outside {lo}..{hi}, the range {shown} was checked over. Without now, the time now."))],
            "    ",
        ));
        t.push_str("    if now is None:\n        now = datetime.now(timezone.utc)\n");
        t.push_str("    off = now.utcoffset()\n");
        t.push_str(&format!("    if off is None:\n        raise {e}(\"today\", {})\n", s(&tr(o, "now にタイムゾーンがありません", "now has no time zone"))));
        t.push_str(&format!("    day = (now.replace(tzinfo=None) - off + timedelta(minutes={off})).date()\n"));
        t.push_str(&format!("    if not date({ly}, {lm}, {ld}) <= day <= date({hy}, {hm}, {hd}):\n"));
        let msg = tr(o, &format!("today {{day}} は、{shown} を確かめた範囲 {lo}..{hi} の外です"), &format!("today {{day}} is outside {lo}..{hi}, the range {shown} was checked over"));
        t.push_str(&format!("        raise {e}(\"today\", f{})\n    return day\n\n\n", s(&msg)));
    }
    // a read of each type, held to what the gate declares of the attributes the action reads, and
    // its entity
    for (&ti, rec_name) in &n.records {
        let ty = &g.types[ti];
        let r = record(p, ti);
        let kind = if ty.kind == Kind::Resource { "resource" } else { "principal" };
        let alias = &ty.named.alias;
        let fname = plan::snake(alias);
        t.push_str(&format!("def _read_{fname}(store: {}, id: str, reads: tuple[str, ...]) -> {rec_name}:\n", n.store));
        t.push_str(&py_doc(
            &[tr(o, &format!("{alias} を store から読み、reads に挙げた属性を、ゲートが宣言した型と範囲で確かめます。"), &format!("The {alias} the store holds, the attributes in reads held to the type and the range the gate declares."))],
            "    ",
        ));
        t.push_str("    try:\n");
        t.push_str(&format!("        found = store.{fname}(id)\n"));
        t.push_str("    except Exception as x:\n");
        t.push_str(&format!("        raise {e}({}, f{}) from x\n", s(kind), s(&tr(o, &format!("{alias} {{id}} を読めません: {{x}}"), &format!("cannot read the {alias} {{id}}: {{x}}")))));
        t.push_str("    if found is None:\n");
        t.push_str(&format!("        raise {e}({}, f{})\n", s(kind), s(&tr(o, &format!("{alias} {{id}} がありません"), &format!("no {alias} {{id}}")))));
        if let Some(roles) = &r.roles {
            let vs: Vec<String> = ty.roles.iter().map(|&x| s(&g.roles[x].named.alias)).collect();
            t.push_str(&format!("    for role in found.{roles}:\n        _enum({}, f\"{alias} {{id}}: role\", role, ({},))\n", s(kind), vs.join(", ")));
        }
        for (_, field) in &r.groups {
            t.push_str(&format!("    for group in found.{field}:\n        _text({}, f\"{alias} {{id}}: {field}\", group)\n", s(kind)));
        }
        for (i, f) in ty.attrs.iter().enumerate() {
            let field = &r.attrs[i];
            let what = format!("f\"{alias} {{id}}: {}\"", f.named.alias);
            let c = check_call(g, kind, &what, f, &format!("found.{field}"));
            let key = s(&f.named.alias);
            if f.optional {
                t.push_str(&format!("    if {key} in reads and found.{field} is not None:\n        {c}\n"));
            } else {
                t.push_str(&format!("    if {key} in reads:\n        {c}\n"));
            }
        }
        t.push_str("    return found\n\n\n");
        // the entity
        t.push_str(&format!("def _entity_{fname}(uid: dict[str, str], r: {rec_name}) -> dict[str, object]:\n"));
        t.push_str("    attrs: dict[str, object] = {}\n");
        for &i in p.cedar_attrs(ti) {
            let f = &ty.attrs[i];
            let field = &r.attrs[i];
            let value = match &f.ty {
                FieldType::Entity(et) => format!("{{\"__entity\": _uid({}, r.{field})}}", s(&p.cedar_type(*et))),
                _ => format!("r.{field}"),
            };
            if f.optional {
                t.push_str(&format!("    if r.{field} is not None:\n        attrs[{}] = {value}\n", s(&f.named.alias)));
            } else {
                t.push_str(&format!("    attrs[{}] = {value}\n", s(&f.named.alias)));
            }
        }
        let mut parents: Vec<String> = Vec::new();
        if let Some(roles) = &r.roles {
            parents.push(format!("[_uid({}, x) for x in r.{roles}]", s(&p.cedar_name(crate::cedar::ROLE))));
        }
        for (gt, field) in &r.groups {
            parents.push(format!("[_uid({}, x) for x in r.{field}]", s(&p.cedar_type(*gt))));
        }
        let parents = if parents.is_empty() { "[]".to_string() } else { parents.join(" + ") };
        t.push_str(&format!("    return {{\"uid\": uid, \"attrs\": attrs, \"parents\": {parents}}}\n\n\n"));
        // the entities its attributes point to, that Cedar holds no attribute of
        let pointed = pointed(p, ti);
        if !pointed.is_empty() {
            t.push_str(&format!("def _pointed_{fname}(r: {rec_name}) -> list[dict[str, str]]:\n"));
            t.push_str("    out: list[dict[str, str]] = []\n");
            for (et, i, optional) in pointed {
                let field = &r.attrs[i];
                if optional {
                    t.push_str(&format!("    if r.{field} is not None:\n        out.append(_uid({}, r.{field}))\n", s(&p.cedar_type(et))));
                } else {
                    t.push_str(&format!("    out.append(_uid({}, r.{field}))\n", s(&p.cedar_type(et))));
                }
            }
            t.push_str("    return out\n\n\n");
        }
    }
    // the request, its entities put together
    let roles = if g.roles.is_empty() { "" } else { "*_ROLES, " };
    t.push_str(&format!(
        "def _request(action: str, p_uid: dict[str, str], p_entity: dict[str, object], r_uid: dict[str, str], r_entity: dict[str, object], pointed: list[dict[str, str]], context: dict[str, object]) -> {}:\n",
        n.request
    ));
    t.push_str(&format!("    entities: list[dict[str, object]] = [{roles}p_entity, r_entity]\n"));
    t.push_str("    seen = [p_uid, r_uid]\n");
    t.push_str("    for uid in pointed:\n        if uid not in seen:\n            seen.append(uid)\n            entities.append({\"uid\": uid, \"attrs\": {}, \"parents\": []})\n");
    t.push_str(&format!("    return {}(p_uid, _uid({}, action), r_uid, context, entities)\n\n\n", n.request, s(&p.cedar_name("Action"))));
}


/// The attributes of a type that point to an entity Cedar holds no attribute of: the entity is
/// written beside the request, as the vectors write it. Each with the type, the attribute's index,
/// and whether it may be absent.
pub(super) fn pointed(p: &Plan, t: usize) -> Vec<(usize, usize, bool)> {
    let ty = &p.g.types[t];
    p.cedar_attrs(t)
        .iter()
        .filter_map(|&i| match ty.attrs[i].ty {
            FieldType::Entity(et) if p.cedar_attrs(et).is_empty() => Some((et, i, ty.attrs[i].optional)),
            _ => None,
        })
        .collect()
}

/// The variable a record of an owner's type is read into: `p_user`, `r_order`.
fn var(owner: Owner, alias: &str) -> String {
    format!("{}_{}", if owner == Owner::Principal { "p" } else { "r" }, plan::snake(alias))
}

/// The types of the other owner, grouped by what the code reads of an owner's type `t` in a request
/// of each: whether it reads the entity, and the attributes (their aliases). The groups in the order
/// of the types.
fn read_groups(p: &Plan, ai: usize, owner: Owner, t: usize) -> Vec<(Vec<usize>, bool, Vec<String>)> {
    let a = &p.g.actions[ai];
    let others: &[usize] = if owner == Owner::Principal { &a.resources } else { &a.principals };
    let mut out: Vec<(Vec<usize>, bool, Vec<String>)> = Vec::new();
    for &x in others {
        let (pt, rt) = if owner == Owner::Principal { (t, x) } else { (x, t) };
        let reads = p.reads_entity(ai, owner, pt, rt);
        let attrs: Vec<String> = if reads { p.read_attrs(ai, owner, pt, rt).iter().map(|&i| p.g.types[t].attrs[i].named.alias.clone()).collect() } else { Vec::new() };
        match out.iter_mut().find(|(_, r, at)| *r == reads && *at == attrs) {
            Some(gr) => gr.0.push(x),
            None => out.push((vec![x], reads, attrs)),
        }
    }
    out
}

/// The action's functions.
fn action(p: &Plan, n: &Names, o: &Options, ap: &ActionPlan, t: &mut String) {
    let g = p.g;
    let ai = ap.index;
    let a = &g.actions[ai];
    let alias = &a.named.alias;
    let e = &n.error;
    let params = [format!("store: {}", n.store), format!("principal: {}", n.principal), format!("input: {}", n.inputs[&ai]), "now: datetime | None = None".to_string()];
    t.push_str(&format!("def {alias}_request({}) -> {}:\n", params.join(", "), n.request));
    let operation = plan::guarded(p.g, a, o.lang);
    let mut d = vec![match &operation {
        Some(op) => tr(o, &format!("action {alias} のリクエスト。{op} を守ります。"), &format!("The request for the action {alias}, which guards {op}.")),
        None => tr(o, &format!("action {alias} のリクエスト。"), &format!("The request for the action {alias}.")),
    }];
    if let Some(desc) = plan::action_description(p.scope, alias) {
        d.push(plan::sentence(&desc));
    }
    d.push(tr(o, &format!("組み立てられなければ {e} を投げます。"), &format!("Raises {e} when it cannot be built.")));
    t.push_str(&py_doc(&[plan::sentences(&d)], "    "));
    let lits = |ts: &[usize]| ts.iter().map(|&x| s(&g.types[x].named.alias)).collect::<Vec<_>>().join(", ");
    // 1. who asks: a type the action takes, a workflow the gate declares
    t.push_str(&format!("    if principal.type not in ({},):\n", lits(&a.principals)));
    t.push_str(&format!("        raise {e}(\"principal\", f{})\n", s(&tr(o, &format!("{alias} を {{principal.type}} が尋ねることはありません"), &format!("{alias} is not asked by a {{principal.type}}")))));
    if let Some(wt) = g.workflow_type().filter(|w| a.principals.contains(w)) {
        let ws: Vec<String> = g.workflows.iter().map(|w| s(&w.named.alias)).collect();
        t.push_str(&format!("    if principal.type == {} and principal.id not in ({},):\n", s(&g.types[wt].named.alias), ws.join(", ")));
        t.push_str(&format!("        raise {e}(\"principal\", f{})\n", s(&tr(o, "ワークフロー {principal.id} はありません", "no workflow {principal.id}"))));
    }
    // 2. the day, held to today's range by an action that computes a value from it
    if p.today().is_some() && ap.reads_today {
        t.push_str("    day = _today(now)\n");
    }
    // 3. the input
    for f in &a.inputs {
        let key = s(&f.named.alias);
        let v = format!("in_{}", f.named.alias);
        if f.optional {
            t.push_str(&format!("    {v} = input.get({key})\n"));
            t.push_str(&format!("    if {v} is not None:\n        {v} = {}\n", check_call(g, "input", &key, f, &v)));
        } else {
            t.push_str(&format!("    {v} = {}\n", check_call(g, "input", &key, f, &format!("input.get({key})"))));
        }
    }
    // 4. the resource: its id, its type, and what the action reads of it
    let field = plan::resource_field(a);
    t.push_str(&format!("    r_id = _text(\"resource\", {}, input.get({}))\n", s(&field), s(&field)));
    let multi = a.resources.len() > 1;
    if multi {
        t.push_str(&format!("    r_type = _enum(\"resource\", \"resource_type\", input.get(\"resource_type\"), ({},))\n", lits(&a.resources)));
    }
    t.push_str("    r_uid: dict[str, str]\n    r_entity: dict[str, object]\n    r_pointed: list[dict[str, str]] = []\n");
    for &rt in a.resources.iter().filter(|&&x| p.reads(x)) {
        t.push_str(&format!("    {}: {} | None = None\n", var(Owner::Resource, &g.types[rt].named.alias), n.records[&rt]));
    }
    let mut first = true;
    for &rt in &a.resources {
        let indent = if multi { "        " } else { "    " };
        if multi {
            t.push_str(&format!("    {} r_type == {}:\n", if first { "if" } else { "elif" }, s(&g.types[rt].named.alias)));
        }
        first = false;
        t.push_str(&format!("{indent}r_uid = _uid({}, r_id)\n", s(&p.cedar_type(rt))));
        read_entity(p, ai, Owner::Resource, rt, indent, "principal.type", "r_id", t);
    }
    if multi {
        t.push_str(&format!("    else:\n        raise {e}(\"resource\", f{})\n", s(&tr(o, &format!("{alias} を {{r_type}} に尋ねることはありません"), &format!("{alias} is not asked of a {{r_type}}")))));
    }
    // 5. the principal: what the action reads of it
    t.push_str("    p_uid: dict[str, str]\n    p_entity: dict[str, object]\n    p_pointed: list[dict[str, str]] = []\n");
    for &pt in a.principals.iter().filter(|&&x| p.reads(x)) {
        t.push_str(&format!("    {}: {} | None = None\n", var(Owner::Principal, &g.types[pt].named.alias), n.records[&pt]));
    }
    let r_type = if multi { "r_type".to_string() } else { s(&g.types[a.resources[0]].named.alias) };
    let mut first = true;
    for &pt in &a.principals {
        t.push_str(&format!("    {} principal.type == {}:\n", if first { "if" } else { "elif" }, s(&g.types[pt].named.alias)));
        first = false;
        t.push_str(&format!("        p_uid = _uid({}, principal.id)\n", s(&p.cedar_type(pt))));
        read_entity(p, ai, Owner::Principal, pt, "        ", &r_type, "principal.id", t);
    }
    t.push_str(&format!("    else:\n        raise {e}(\"principal\", f{})\n", s(&tr(o, &format!("{alias} を {{principal.type}} が尋ねることはありません"), &format!("{alias} is not asked by a {{principal.type}}")))));
    // 6. the context: the inputs the policies read, then the values the action computes
    t.push_str("    context: dict[str, object] = {}\n");
    for &i in &p.shape.actions[ai].inputs {
        let f = &a.inputs[i];
        let v = format!("in_{}", f.named.alias);
        if f.optional {
            t.push_str(&format!("    if {v} is not None:\n        context[{}] = {v}\n", s(&f.named.alias)));
        } else {
            t.push_str(&format!("    context[{}] = {v}\n", s(&f.named.alias)));
        }
    }
    for vp in &ap.values {
        value(p, n, a, vp, t);
    }
    t.push_str(&format!("    return _request({}, p_uid, p_entity, r_uid, r_entity, p_pointed + r_pointed, context)\n\n\n", s(alias)));
    // authorize
    let mut call_params: Vec<String> = params.to_vec();
    if o.authorizer == Authorizer::Avp {
        call_params.insert(0, format!("avp: {}", n.avp));
    }
    t.push_str(&format!("def authorize_{alias}({}) -> {}:\n", call_params.join(", "), n.answer));
    let how = match o.authorizer {
        Authorizer::Cedar => tr(o, "このプロセスの中の cedarpy", "cedarpy, in this process"),
        Authorizer::Avp => "Amazon Verified Permissions".to_string(),
    };
    t.push_str(&py_doc(&[tr(o, &format!("{alias} を許すかを Cedar（{how}）に尋ねます。エラーのときは、拒む答えを返します。"), &format!("Asks Cedar ({how}) whether {alias} is allowed. Any error is a deny."))], "    "));
    t.push_str("    try:\n");
    t.push_str(&format!("        req = {alias}_request(store, principal, input, now)\n"));
    t.push_str(&format!("    except {e} as x:\n        return {}(False, [], None, x)\n", n.answer));
    match o.authorizer {
        Authorizer::Cedar => t.push_str("    return _ask(req)\n\n\n"),
        Authorizer::Avp => t.push_str("    return _ask(avp, req)\n\n\n"),
    }
}


/// The code that reads an owner's entity of type `t` (when the action reads it, by the type of the
/// other owner, `other`) and makes its entity for Cedar; `id` is its id.
#[allow(clippy::too_many_arguments)]
fn read_entity(p: &Plan, ai: usize, owner: Owner, t: usize, indent: &str, other: &str, id: &str, t_out: &mut String) {
    let g = p.g;
    let pre = if owner == Owner::Principal { "p" } else { "r" };
    let groups = read_groups(p, ai, owner, t);
    let others_lits = |ts: &[usize]| ts.iter().map(|&x| s(&g.types[x].named.alias)).collect::<Vec<_>>().join(", ");
    let body = |reads: bool, attrs: &[String], ind: &str, out: &mut String| {
        if reads {
            let v = var(owner, &g.types[t].named.alias);
            let f = plan::snake(&g.types[t].named.alias);
            let need: Vec<String> = attrs.iter().map(|x| s(x)).collect();
            let need = if need.is_empty() { "()".to_string() } else { format!("({},)", need.join(", ")) };
            out.push_str(&format!("{ind}{v} = _read_{f}(store, {id}, {need})\n"));
            out.push_str(&format!("{ind}{pre}_entity = _entity_{f}({pre}_uid, {v})\n"));
            if !pointed(p, t).is_empty() {
                out.push_str(&format!("{ind}{pre}_pointed = _pointed_{f}({v})\n"));
            }
        } else {
            out.push_str(&format!("{ind}{pre}_entity = {{\"uid\": {pre}_uid, \"attrs\": {{}}, \"parents\": []}}\n"));
        }
    };
    if groups.len() == 1 {
        body(groups[0].1, &groups[0].2, indent, t_out);
        return;
    }
    for (k, (others, reads, attrs)) in groups.iter().enumerate() {
        if k + 1 == groups.len() {
            t_out.push_str(&format!("{indent}else:\n"));
        } else {
            t_out.push_str(&format!("{indent}{} {other} in ({},):\n", if k == 0 { "if" } else { "elif" }, others_lits(others)));
        }
        body(*reads, attrs, &format!("{indent}    "), t_out);
    }
}

/// The code of one value the action computes, put into `context` under its alias.
fn value(p: &Plan, n: &Names, a: &Action, vp: &ValuePlan, t: &mut String) {
    let g = p.g;
    let cv = &a.computed[vp.index];
    let e = &n.error;
    let key = s(&cv.named.alias);
    let res = format!("v_{}", cv.named.alias);
    // what the comment says: how the gate writes it
    let said = plan::line_of(g, cv.named.line);
    t.push_str(&format!("    # {}\n", ritsu_emit::header::one_line(&said)));
    // the branches: for each type of an owner it reads, the record it reads
    let ps: Vec<Option<usize>> = match &vp.principals {
        Some(ts) => ts.iter().map(|&x| Some(x)).collect(),
        None => vec![None],
    };
    let rs: Vec<Option<usize>> = match &vp.resources {
        Some(ts) => ts.iter().map(|&x| Some(x)).collect(),
        None => vec![None],
    };
    for pt in &ps {
        for rt in &rs {
            let mut conds: Vec<String> = Vec::new();
            if let Some(x) = pt {
                conds.push(format!("{} is not None", var(Owner::Principal, &g.types[*x].named.alias)));
            }
            if let Some(x) = rt {
                conds.push(format!("{} is not None", var(Owner::Resource, &g.types[*x].named.alias)));
            }
            let mut body = String::new();
            let mut binds: Vec<String> = Vec::new();
            let mut optional: Vec<String> = Vec::new();
            let source = |src: &Source, binds: &mut Vec<String>, optional: &mut Vec<String>| -> String {
                match src {
                    Source::Attr(owner, name) => {
                        let ty = match owner {
                            Owner::Principal => pt.unwrap_or(0),
                            Owner::Resource => rt.unwrap_or(0),
                        };
                        let r = record(p, ty);
                        let (i, f) = g.types[ty].attr(name).unwrap_or((0, &g.types[ty].attrs[0]));
                        let expr = format!("{}.{}", var(*owner, &g.types[ty].named.alias), r.attrs[i]);
                        if f.optional {
                            let local = format!("a{}", binds.len());
                            binds.push(format!("{local} = {expr}"));
                            optional.push(format!("{local} is not None"));
                            local
                        } else {
                            expr
                        }
                    }
                    Source::Input(name) => {
                        let f = a.input(name).map(|(_, f)| f);
                        let local = format!("in_{}", f.map(|f| f.named.alias.clone()).unwrap_or_else(|| name.clone()));
                        if f.is_some_and(|f| f.optional) {
                            optional.push(format!("{local} is not None"));
                        }
                        local
                    }
                    Source::Today => "day".to_string(),
                    Source::Lit(_) => String::new(),
                }
            };
            let computed: String = match &vp.call {
                Call::Rule { module, output, args, values } => {
                    let Some(facts) = p.rule(*module) else { continue };
                    let m = module_var(p, *module);
                    let py = &facts.python;
                    let mut call_args: Vec<String> = Vec::new();
                    for (col, src) in args {
                        let param = py.params.iter().find(|x| x.name == col.name);
                        let pty = param.map(|x| x.ty.trim_end_matches(" | None").to_string()).unwrap_or_else(|| "int".into());
                        let Some(src) = src else {
                            call_args.push("None".into());
                            continue;
                        };
                        let raw = source(src, &mut binds, &mut optional);
                        call_args.push(rule_arg(p, facts, &m, col, &pty, src, &raw));
                    }
                    let mut expr = format!("{m}.{}({})", py.function, call_args.join(", "));
                    if facts.outputs.len() > 1 {
                        let out_alias = py.outputs.iter().find(|x| x.name == output.name).map(|x| x.alias.clone()).unwrap_or_else(|| output.alias.clone());
                        expr = format!("{expr}.{out_alias}");
                    }
                    let msg = format!("f{}", s(&format!("rulec {}: {{x}}", p.use_path(*module))));
                    let _ = &msg;
                    let optional_out = matches!(output.ty, ritsu_ports::ColumnType::Opt(_));
                    let put = match values {
                        Some(vs) => {
                            let ty_name = match plan::strip(&output.ty) {
                                ritsu_ports::ColumnType::Enum(en) => py.enums.iter().find(|x| x.name == *en).map(|x| (x.alias.clone(), x.values.clone())),
                                _ => None,
                            };
                            match ty_name {
                                Some((class, members)) => {
                                    let pairs: Vec<String> = vs
                                        .iter()
                                        .filter_map(|(name, public)| members.iter().find(|(x, _)| x == name).map(|(_, mem)| format!("{m}.{class}.{mem}: {}", s(public))))
                                        .collect();
                                    format!("{{{}}}[{res}]", pairs.join(", "))
                                }
                                None => res.clone(),
                            }
                        }
                        None => res.clone(),
                    };
                    if optional_out {
                        body.push_str(&format!("try:\n    {res} = {expr}\n    if {res} is not None:\n        context[{key}] = {put}\nexcept Exception as x:\n    raise {e}(\"rule\", {msg}) from x\n"));
                    } else {
                        body.push_str(&format!("try:\n    {res} = {expr}\n    context[{key}] = {put}\nexcept Exception as x:\n    raise {e}(\"rule\", {msg}) from x\n"));
                    }
                    String::new()
                }
                Call::Date { module, function, op, args } => {
                    let m = module_var(p, *module);
                    let mut call_args: Vec<String> = Vec::new();
                    for (_, is_day, src) in args {
                        call_args.push(match src {
                            Source::Lit(Literal::Date(d)) => {
                                let (y, mo, dd) = plan::ymd(*d);
                                format!("date({y}, {mo}, {dd})")
                            }
                            Source::Lit(Literal::Num(num)) => {
                                let _ = is_day;
                                num.value.num.to_string()
                            }
                            other => source(other, &mut binds, &mut optional),
                        });
                    }
                    let msg = format!("f{}", s(&format!("koyomi {}: {{x}}", p.use_path(*module))));
                    body.push_str(&format!("try:\n    {res} = {m}.{function}({})\nexcept Exception as x:\n    raise {e}(\"date\", {msg}) from x\n", call_args.join(", ")));
                    body.push_str(&format!("context[{key}] = {}\n", compare("day", *op, &res)));
                    String::new()
                }
                Call::DateAttr { op, owner, attr } => {
                    let raw = source(&Source::Attr(*owner, attr.clone()), &mut binds, &mut optional);
                    body.push_str(&format!("context[{key}] = {}\n", compare("day", *op, &raw)));
                    String::new()
                }
                Call::Open { module } => {
                    let m = module_var(p, *module);
                    let msg = format!("f{}", s(&format!("koyomi {}: {{x}}", p.use_path(*module))));
                    body.push_str(&format!("try:\n    context[{key}] = {m}.is_open(day)\nexcept Exception as x:\n    raise {e}(\"date\", {msg}) from x\n"));
                    String::new()
                }
            };
            let _ = computed;
            // the body, behind its branch and the values that may be absent
            let mut lines: Vec<String> = Vec::new();
            let mut depth = 1;
            if !conds.is_empty() {
                lines.push(format!("{}if {}:", "    ".repeat(depth), conds.join(" and ")));
                depth += 1;
            }
            for b in &binds {
                lines.push(format!("{}{b}", "    ".repeat(depth)));
            }
            if !optional.is_empty() {
                lines.push(format!("{}if {}:", "    ".repeat(depth), optional.join(" and ")));
                depth += 1;
            }
            for l in body.lines() {
                lines.push(format!("{}{l}", "    ".repeat(depth)));
            }
            for l in lines {
                t.push_str(&l);
                t.push('\n');
            }
        }
    }
}

/// `day <= v`, `day == v`.
fn compare(left: &str, op: Op, right: &str) -> String {
    let sym = match op {
        Op::Lt => "<",
        Op::Le => "<=",
        Op::Gt => ">",
        Op::Ge => ">=",
        Op::Is => "==",
    };
    format!("{left} {sym} {right}")
}

/// A value given to an input of a rule, as the rule's Python takes it (`pty`, its type there).
fn rule_arg(p: &Plan, facts: &ritsu_ports::RuleFacts, m: &str, col: &ritsu_ports::Column, pty: &str, src: &Source, raw: &str) -> String {
    use ritsu_ports::ColumnType as C;
    let g = p.g;
    match (plan::strip(&col.ty), src) {
        (C::Date, Source::Lit(Literal::Date(d))) => d.to_string(),
        (C::Date, _) => format!("_days({raw})"),
        (C::Bool, Source::Lit(Literal::Bool(b))) => if *b { "True".into() } else { "False".into() },
        (C::Bool, _) => raw.to_string(),
        (C::Num { unit, .. }, Source::Lit(Literal::Num(num))) => {
            let v = unit.as_ref().and_then(|u| crate::types::count(num, u).ok()).unwrap_or(num.value.num);
            if pty == "int" { v.to_string() } else { format!("{m}.{pty}({v})") }
        }
        (C::Num { .. }, _) => {
            if pty == "int" {
                raw.to_string()
            } else {
                format!("{m}.{pty}({raw})")
            }
        }
        (C::Enum(en), src) => {
            let Some(ce) = facts.python.enums.iter().find(|x| x.name == *en) else { return raw.to_string() };
            let Some(re) = facts.enums.iter().find(|x| x.name == *en) else { return raw.to_string() };
            // the rule's value a word names: its name, its alias in the code, its public name
            let member_of = |word: &str| -> Option<String> {
                let v = re.values.iter().find(|v| v.name == word || v.alias == word || v.public == word)?;
                ce.values.iter().find(|(nm, _)| *nm == v.name).map(|(_, mem)| format!("{m}.{}.{mem}", ce.alias))
            };
            match src {
                Source::Lit(Literal::Word(w)) => member_of(w).unwrap_or_else(|| raw.to_string()),
                Source::Attr(..) | Source::Input(..) => {
                    // the gate's enum: each of its values, the rule's member of the same value
                    let gate_enum = gate_enum_of(g, src);
                    let pairs: Vec<String> = match gate_enum {
                        Some(ge) => g.enums[ge]
                            .values
                            .iter()
                            .filter_map(|gv| re.values.iter().find(|v| gv.is(&v.name) || gv.is(&v.alias) || gv.is(&v.public)).and_then(|v| member_of(&v.name)).map(|mem| format!("{}: {mem}", s(&gv.alias))))
                            .collect(),
                        None => Vec::new(),
                    };
                    format!("{{{}}}[{raw}]", pairs.join(", "))
                }
                _ => raw.to_string(),
            }
        }
        _ => raw.to_string(),
    }
}

/// The enum of the gate a value given to a rule is of.
fn gate_enum_of(g: &Gate, src: &Source) -> Option<usize> {
    let ty = match src {
        Source::Attr(_, name) => g.types.iter().find_map(|t| t.attr(name).map(|(_, f)| f.ty.clone())),
        Source::Input(name) => g.actions.iter().find_map(|a| a.input(name).map(|(_, f)| f.ty.clone())),
        _ => None,
    };
    match ty {
        Some(FieldType::Enum(e)) => Some(e),
        _ => None,
    }
}

/// How the module asks Cedar.
fn ask(p: &Plan, n: &Names, o: &Options, t: &mut String) {
    let e = &n.error;
    let a = &n.answer;
    match o.authorizer {
        Authorizer::Cedar => {
            t.push_str(&format!(
                "# {}\n",
                tr(o, "ポリシーは @id をキーにした JSON の形で持ちます。一つのテキストで渡すと、cedarpy はポリシーに policy0、policy1… と名前を付け、決めたポリシーがゲートの名前になりません。", "The policies, in Cedar's JSON keyed by their @id: given as one text, cedarpy names them policy0, policy1, …, and the policies that decide would not be named as the gate names them.")
            ));
            t.push_str(&format!("_POLICIES = cedarpy.PolicySet.from_json_str({})\n", s(&p.policies_json)));
            t.push_str(&format!("_SCHEMA = cedarpy.Schema.from_json_str({})\n\n\n", s(&p.schema_json)));
            t.push_str(&format!("def _ask(req: {}) -> {a}:\n", n.request));
            t.push_str("    request = {\"principal\": req.principal, \"action\": req.action, \"resource\": req.resource, \"context\": req.context}\n");
            t.push_str("    try:\n        r = cedarpy.is_authorized(request, _POLICIES, req.entities, schema=_SCHEMA)\n");
            t.push_str(&format!("    except Exception as x:\n        return {a}(False, [], req.context, {e}(\"cedar\", str(x)))\n"));
            t.push_str(&format!("    if r.diagnostics.errors:\n        return {a}(False, [], req.context, {e}(\"cedar\", \"; \".join(str(x) for x in r.diagnostics.errors)))\n"));
            t.push_str(&format!("    return {a}(r.allowed, sorted(r.diagnostics.reasons), req.context)\n"));
        }
        Authorizer::Avp => {
            t.push_str(&format!("def _ask(avp: {}, req: {}) -> {a}:\n", n.avp, n.request));
            t.push_str("    try:\n");
            t.push_str("        r = avp.client.is_authorized(\n");
            t.push_str("            policyStoreId=avp.policy_store_id,\n");
            t.push_str("            principal={\"entityType\": req.principal[\"type\"], \"entityId\": req.principal[\"id\"]},\n");
            t.push_str("            action={\"actionType\": req.action[\"type\"], \"actionId\": req.action[\"id\"]},\n");
            t.push_str("            resource={\"entityType\": req.resource[\"type\"], \"entityId\": req.resource[\"id\"]},\n");
            t.push_str("            context={\"cedarJson\": json.dumps(req.context)},\n");
            t.push_str("            entities={\"cedarJson\": json.dumps(req.entities)},\n");
            t.push_str("        )\n");
            t.push_str(&format!("    except Exception as x:\n        return {a}(False, [], req.context, {e}(\"cedar\", str(x)))\n"));
            t.push_str(&format!("    if r[\"errors\"]:\n        return {a}(False, [], req.context, {e}(\"cedar\", \"; \".join(x[\"errorDescription\"] for x in r[\"errors\"])))\n"));
            t.push_str(&format!("    return {a}(r[\"decision\"] == \"ALLOW\", sorted(x[\"policyId\"] for x in r[\"determiningPolicies\"]), req.context)\n"));
        }
    }
}
