//! The Go that builds a gate's requests and asks Cedar (DESIGN 5.3–5.6): one package,
//! `authz/<package>/<package>.go` of a package `ritsu gen` writes, which imports the rules' and the
//! dates' packages from the same module (`<module>/rules/<package>`, `<module>/dates/<package>`).
//!
//! For each action, `<Action>Request(store, principal, input, now)` follows the same steps as the
//! Python's ([`super::python`]): who asks, the day, the input, the resource and the principal as
//! the `Store` holds them, then the values computed with the code rulec and koyomi generate; it
//! returns an `*Error` of a kind when it cannot (DESIGN 3.8). `Authorize<Action>(…)` asks Cedar —
//! cedar-go in the process, or Verified Permissions through the AWS SDK (`--authorizer avp`) — and
//! answers allowed or not, the policies that decided, and the context it gave Cedar. Each dates
//! package of koyomi's has a `Date` of its own, so a day is handed over as its year, month and day
//! (DESIGN 5.3).

use super::plan::{self, ActionPlan, Authorizer, Call, Import, Options, Plan, ValuePlan};
use crate::ast::Op;
use crate::model::*;
use ritsu_base::text::Lang;
use ritsu_emit::header::Comment;
use ritsu_emit::lit;
use std::collections::BTreeMap;

/// The package's path under the Go of a package: `authz/<package>/<package>.go`.
pub fn path(alias: &str) -> String {
    let pkg = ritsu_emit::ident::go_package(alias);
    format!("authz/{pkg}/{pkg}.go")
}

/// A string of Go.
fn s(text: &str) -> String {
    lit::go(text)
}

fn tr(o: &Options, ja: &str, en: &str) -> String {
    if o.lang == Lang::Ja { ja.to_string() } else { en.to_string() }
}

/// A name of the source as an exported name of Go: `refund_limit` is `RefundLimit`, `orderId`
/// is `OrderId`, a character Go does not take in a name is a break between parts.
pub fn exported(name: &str) -> String {
    let clean: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let p = ritsu_emit::ident::pascal(&clean);
    if p.is_empty() || p.starts_with(|c: char| c.is_ascii_digit()) { format!("X{p}") } else { p }
}

/// The names the package declares, kept apart from one another.
struct Names {
    principal: String,
    store: String,
    error: String,
    request: String,
    answer: String,
    entity: String,
    entity_ref: String,
    date: String,
    avp: String,
    records: BTreeMap<usize, String>,
    inputs: BTreeMap<usize, String>,
}

fn names(p: &Plan) -> Names {
    let g = p.g;
    let mut wanted: Vec<String> = Vec::new();
    let recorded: Vec<usize> = p.types_used().into_iter().filter(|&t| p.reads(t)).collect();
    for &t in &recorded {
        wanted.push(g.types[t].named.alias.clone());
    }
    for w in ["Principal", "Store", "Error", "Request", "Answer", "Entity", "EntityRef", "Date", "VerifiedPermissions"] {
        wanted.push(w.to_string());
    }
    for a in &g.actions {
        wanted.push(format!("{}Input", exported(&a.named.alias)));
    }
    let got = ritsu_emit::ident::unique(wanted, &[]);
    let mut it = got.into_iter();
    let mut records = BTreeMap::new();
    for &t in &recorded {
        records.insert(t, it.next().unwrap_or_default());
    }
    let mut fixed: Vec<String> = (0..9).map(|_| it.next().unwrap_or_default()).collect();
    let mut inputs = BTreeMap::new();
    for i in 0..g.actions.len() {
        inputs.insert(i, it.next().unwrap_or_default());
    }
    let avp = fixed.pop().unwrap_or_default();
    let date = fixed.pop().unwrap_or_default();
    let entity_ref = fixed.pop().unwrap_or_default();
    let entity = fixed.pop().unwrap_or_default();
    let answer = fixed.pop().unwrap_or_default();
    let request = fixed.pop().unwrap_or_default();
    let error = fixed.pop().unwrap_or_default();
    let store = fixed.pop().unwrap_or_default();
    let principal = fixed.pop().unwrap_or_default();
    Names { principal, store, error, request, answer, entity, entity_ref, date, avp, records, inputs }
}

/// The fields of a type's record: `Roles`, a field for each group it is a member of, and each
/// attribute's, kept apart.
struct Record {
    roles: Option<String>,
    groups: Vec<(usize, String)>,
    attrs: Vec<String>,
}

fn record(p: &Plan, t: usize) -> Record {
    let ty = &p.g.types[t];
    let mut wanted: Vec<String> = Vec::new();
    for f in &ty.attrs {
        wanted.push(exported(&f.named.alias));
    }
    if !ty.roles.is_empty() {
        wanted.push("Roles".to_string());
    }
    for &gt in p.groups(t) {
        wanted.push(format!("MemberOf{}", p.g.types[gt].named.alias));
    }
    let got = ritsu_emit::ident::unique(wanted, &[]);
    let attrs: Vec<String> = got[..ty.attrs.len()].to_vec();
    let mut rest = got[ty.attrs.len()..].iter();
    let roles = (!ty.roles.is_empty()).then(|| rest.next().cloned().unwrap_or_default());
    let groups = p.groups(t).iter().map(|&gt| (gt, rest.next().cloned().unwrap_or_default())).collect();
    Record { roles, groups, attrs }
}

/// The package alias a module of another language is imported as: `rulesrefundlimit`.
fn import_name(p: &Plan, ui: usize) -> String {
    match p.imports.get(&ui) {
        Some(Import::Rule(r)) => format!("rules{}", r.go.module),
        Some(Import::Dates(d)) => format!("dates{}", ritsu_emit::ident::go_package(&d.alias)),
        Some(Import::Calendar(c)) => format!("dates{}", ritsu_emit::ident::go_package(&c.alias)),
        None => "_".to_string(),
    }
}

/// A comment of several lines, each at most about 100 columns, at `indent`: a line breaks at a
/// space, or after a Japanese full stop or comma (a character that is not ASCII counted two
/// columns wide).
fn doc(text: &str, indent: &str) -> String {
    let width = |s: &str| s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum::<usize>();
    let mut out = String::new();
    for para in text.split("\n\n") {
        if !out.is_empty() {
            out.push_str(&format!("{indent}//\n"));
        }
        // the pieces, each with whether a space goes before it
        let mut pieces: Vec<(bool, String)> = Vec::new();
        for (k, word) in ritsu_emit::header::one_line(para).split(' ').enumerate() {
            let mut cur = String::new();
            let mut spaced = k > 0;
            for c in word.chars() {
                cur.push(c);
                if c == '。' || c == '、' {
                    pieces.push((spaced, std::mem::take(&mut cur)));
                    spaced = false;
                }
            }
            if !cur.is_empty() {
                pieces.push((spaced, cur));
            }
        }
        let mut line = String::new();
        for (spaced, piece) in pieces {
            let gap = if spaced && !line.is_empty() { 1 } else { 0 };
            if !line.is_empty() && width(&line) + gap + width(&piece) > 96 {
                out.push_str(&format!("{indent}// {line}\n"));
                line.clear();
            } else if gap == 1 {
                line.push(' ');
            }
            line.push_str(&piece);
        }
        if !line.is_empty() {
            out.push_str(&format!("{indent}// {line}\n"));
        }
    }
    out
}

fn field_type(n: &Names, f: &Field) -> String {
    let base = match &f.ty {
        FieldType::Bool => "bool".to_string(),
        FieldType::Enum(_) | FieldType::Entity(_) => "string".to_string(),
        FieldType::Num { .. } => "int64".to_string(),
        FieldType::Date { .. } => n.date.clone(),
    };
    if f.optional { format!("*{base}") } else { base }
}

fn field_comment(g: &Gate, f: &Field, lang: Lang) -> String {
    let note = plan::field_note(g, f, lang);
    if note.is_empty() { String::new() } else { format!(" // {}", ritsu_emit::header::one_line(&note)) }
}

/// The variable a record of an owner's type is read into: `pUser`, `rOrder`.
fn var(owner: Owner, alias: &str) -> String {
    format!("{}{alias}", if owner == Owner::Principal { "p" } else { "r" })
}

/// What the package is: its text, for a file that passed its check.
pub fn module(p: &Plan, o: &Options) -> String {
    let g = p.g;
    let n = names(p);
    let f = p.scope.file();
    let pkg = ritsu_emit::ident::go_package(&g.named.alias);
    let mut t = String::new();
    let sha = ritsu_base::sha256::hex(f.src.as_bytes());
    let version = f.version.strip_prefix('v').unwrap_or(&f.version);
    let source = ritsu_emit::header::Source { path: &o.shown, kind: "gate", name: &f.name.text, version, sha256: &sha };
    t.push_str(&Comment::Slashes.line(&ritsu_emit::header::generated("sekisho")));
    t.push_str(&Comment::Slashes.line(source.line().get(o.lang)));
    t.push('\n');
    let mut d = tr(o, &format!("パッケージ {pkg} は、ゲート {} のリクエストを組み立て、Cedar に尋ねます。", g.named.alias), &format!("Package {pkg} builds the requests of the gate {} and asks Cedar.", g.named.alias));
    if let Some((desc, _)) = &f.description {
        d = plan::sentences(&[d, plan::sentence(desc)]);
    }
    d.push_str("\n\n");
    d.push_str(&tr(
        o,
        "このパッケージが組み立てるリクエストだけが、このゲートへの尋ね方です。context に入るのは、操作の引数（ハンドラーが解析したもの）と、サービスのデータ（Store が読むもの）から ritsu の言語が計算した値だけで、呼ぶ側が送ってきた context は入りません。",
        "The requests this package builds are the only way the gate is meant to be asked: the context holds the arguments of the operation, as the handler parsed them, and what ritsu's languages compute from the service's own data, which the Store reads. Nothing in it is taken from a context the caller sends.",
    ));
    d.push_str("\n\n");
    d.push_str(&match o.authorizer {
        Authorizer::Cedar => tr(o, "Cedar には、このプロセスの中で cedar-go で尋ねます。ポリシーは、このパッケージが持っています。", "Cedar is asked in this process, through cedar-go; the package holds the policies."),
        Authorizer::Avp => tr(
            o,
            "Cedar には、Amazon Verified Permissions で尋ねます（AWS SDK）。ポリシーとスキーマは、gen --target cedar が書いたものを、ポリシーストアに置いてください。決めたポリシーは、ストアが付けた ID で返ります。",
            "Cedar is asked through Amazon Verified Permissions (the AWS SDK): put the policies and the schema gen --target cedar writes into the policy store. The policies that decide are the ids the store gave them.",
        ),
    });
    let more = tr(o, &format!("規則と日付のパッケージは、同じモジュールの {}/rules と {}/dates から読みます（ritsu gen が書きます）。", o.go_module, o.go_module), &format!("The packages of the rules and the dates are the module's own {}/rules and {}/dates, which ritsu gen writes.", o.go_module, o.go_module));
    let (head, last) = d.rsplit_once("\n\n").map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or_default();
    d = format!("{head}\n\n{}", plan::sentences(&[last, more]));
    t.push_str(&doc(&d, ""));
    t.push_str(&format!("package {pkg}\n\n"));
    // the imports
    let mut std_imports: Vec<&str> = vec!["fmt", "sort"];
    if !g.actions.is_empty() || p.today().is_some() {
        std_imports.push("time");
    }
    match o.authorizer {
        Authorizer::Cedar => std_imports.push("encoding/json"),
        Authorizer::Avp => std_imports.extend(["context", "encoding/json"]),
    }
    std_imports.sort();
    t.push_str("import (\n");
    for i in &std_imports {
        t.push_str(&format!("\t{}\n", s(i)));
    }
    t.push('\n');
    match o.authorizer {
        Authorizer::Cedar => t.push_str("\t\"github.com/cedar-policy/cedar-go\"\n\t\"github.com/cedar-policy/cedar-go/types\"\n"),
        Authorizer::Avp => t.push_str("\t\"github.com/aws/aws-sdk-go-v2/aws\"\n\t\"github.com/aws/aws-sdk-go-v2/service/verifiedpermissions\"\n\tavptypes \"github.com/aws/aws-sdk-go-v2/service/verifiedpermissions/types\"\n"),
    }
    let mut ours: Vec<(String, String)> = Vec::new();
    for (ui, imp) in &p.imports {
        let path = match imp {
            Import::Rule(r) => format!("{}/rules/{}", o.go_module, r.go.module),
            Import::Dates(d) => format!("{}/dates/{}", o.go_module, ritsu_emit::ident::go_package(&d.alias)),
            Import::Calendar(c) => format!("{}/dates/{}", o.go_module, ritsu_emit::ident::go_package(&c.alias)),
        };
        ours.push((path.clone(), format!("\t{} {}\n", import_name(p, *ui), s(&path))));
    }
    ours.sort();
    ours.dedup();
    if !ours.is_empty() {
        t.push('\n');
        for (_, l) in ours {
            t.push_str(&l);
        }
    }
    t.push_str(")\n\n");
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



fn types(p: &Plan, n: &Names, o: &Options, t: &mut String) {
    let g = p.g;
    let principals: Vec<usize> = p.types_used().into_iter().filter(|&x| g.types[x].kind != Kind::Resource).collect();
    let names_of = |ts: &[usize]| ts.iter().map(|&x| g.types[x].named.alias.clone()).collect::<Vec<_>>();
    let mut d = tr(o, &format!("{} は、尋ねる者です。サービスの認証が決めた型（{}）と ID を持ちます。", n.principal, names_of(&principals).join("、")), &format!("{} is who asks, as the service's authentication found it: the type ({}) and the id.", n.principal, names_of(&principals).join(", ")));
    if !g.workflows.is_empty() {
        let ws: Vec<String> = g.workflows.iter().map(|w| w.named.alias.clone()).collect();
        d = plan::sentences(&[d, tr(o, &format!("ワークフローは、名前を ID にします（{}）。", ws.join("、")), &format!("A workflow is asked as itself, its name the id ({}).", ws.join(", ")))]);
    }
    t.push_str(&doc(&d, ""));
    t.push_str(&format!("type {} struct {{\n\tType string\n\tID   string\n}}\n\n", n.principal));
    t.push_str(&doc(&tr(o, &format!("{} は、ゲートの日付です。年、月、日を持ちます。", n.date), &format!("{} is a day of the gate's: its year, month and day.", n.date)), ""));
    t.push_str(&format!("type {} struct {{\n\tYear, Month, Day int\n}}\n\n", n.date));
    // the records
    for (&ti, name) in &n.records {
        let ty = &g.types[ti];
        let r = record(p, ti);
        let owner = if ty.kind == Kind::Resource { "resource" } else { "principal" };
        let mut d = tr(o, &format!("{name} は、{owner} {} について、ゲートが読むものです。", ty.named.alias), &format!("{name} is what the gate reads of the {owner} {}.", ty.named.alias));
        if let Some(desc) = plan::type_description(p.scope, &ty.named.alias) {
            d = plan::sentences(&[d, plan::sentence(&desc)]);
        }
        t.push_str(&doc(&d, ""));
        t.push_str(&format!("type {name} struct {{\n"));
        let mut lines: Vec<(String, String, String)> = Vec::new();
        if let Some(roles) = &r.roles {
            let vs: Vec<String> = ty.roles.iter().map(|&x| g.roles[x].named.alias.clone()).collect();
            lines.push((roles.clone(), "[]string".to_string(), format!(" // {}", tr(o, &format!("{} のどれか", vs.join("、")), &format!("each one of {}", vs.join(", "))))));
        }
        for (gt, field) in &r.groups {
            let gn = &g.types[*gt].named.alias;
            lines.push((field.clone(), "[]string".to_string(), format!(" // {}", tr(o, &format!("メンバーである {gn} の ID"), &format!("the ids of the {gn}s it is a member of")))));
        }
        for (i, f) in ty.attrs.iter().enumerate() {
            lines.push((r.attrs[i].clone(), field_type(n, f), field_comment(g, f, o.lang)));
        }
        aligned(&lines, t);
        t.push_str("}\n\n");
    }
    // the store
    t.push_str(&doc(&tr(o, &format!("{} は、principal と resource について知っていることを、サービス自身のデータから読むところです。無ければ nil を返します。", n.store), &format!("{} is where the code reads what it knows of the principal and the resource: the service's own data. A nil for one it does not know.", n.store)), ""));
    if n.records.is_empty() {
        t.push_str(&format!("type {} interface{{}}\n\n", n.store));
    } else {
        t.push_str(&format!("type {} interface {{\n", n.store));
        for (&ti, name) in &n.records {
            t.push_str(&format!("\t{}(id string) (*{name}, error)\n", g.types[ti].named.alias));
        }
        t.push_str("}\n\n");
    }
    // the inputs
    for ap in &p.actions {
        let a = &g.actions[ap.index];
        let name = &n.inputs[&ap.index];
        let operation = plan::guarded(p.g, a, o.lang);
        let d = match &operation {
            Some(op) => tr(o, &format!("{name} は、操作（{op}）の引数です。ハンドラーが解析したもので、操作はこれと同じ引数で行ってください。"), &format!("{name} is the arguments of the operation {op}, as the handler parsed them; the operation is to be done with the same.")),
            None => tr(o, &format!("{name} は、{} のリクエストの引数です。", a.named.alias), &format!("{name} is the arguments of the request of {}.", a.named.alias)),
        };
        t.push_str(&doc(&d, ""));
        t.push_str(&format!("type {name} struct {{\n"));
        let rts = names_of(&a.resources);
        let mut lines: Vec<(String, String, String)> = vec![(exported(&plan::resource_field(a)), "string".to_string(), format!(" // {}", tr(o, &format!("{} の ID", rts.join("、")), &format!("the id of the {}", rts.join(" or ")))))];
        if a.resources.len() > 1 {
            lines.push(("ResourceType".to_string(), "string".to_string(), format!(" // {}", tr(o, &format!("resource の型（{}）", rts.join("、")), &format!("the type of the resource ({})", rts.join(", "))))));
        }
        for f in &a.inputs {
            lines.push((exported(&f.named.alias), field_type(n, f), field_comment(g, f, o.lang)));
        }
        aligned(&lines, t);
        t.push_str("}\n\n");
    }
    // the error, the request and the answer
    t.push_str(&doc(
        &tr(
            o,
            &format!("{} は、ゲートに尋ねられなかった理由です。このとき答えは「拒む」です。Kind は principal、resource、input、today、rule、date（リクエストを組み立てられない）、cedar（Cedar がエラーを返したか、尋ねられなかった）のどれかです。", n.error),
            &format!("{} is why the gate could not be asked, and the answer is a deny: Kind is principal, resource, input, today, rule or date (the request could not be built), or cedar (Cedar answered with an error, or could not be asked).", n.error),
        ),
        "",
    ));
    t.push_str(&format!("type {} struct {{\n\tKind    string\n\tMessage string\n}}\n\nfunc (e *{}) Error() string {{ return e.Kind + \": \" + e.Message }}\n\n", n.error, n.error));
    t.push_str(&doc(&tr(o, &format!("{} は、Cedar が指すエンティティの型（名前空間つき）と ID です。", n.entity_ref), &format!("{} is an entity as Cedar names it: its type, with the namespace, and its id.", n.entity_ref)), ""));
    t.push_str(&format!("type {} struct {{\n\tType string\n\tID   string\n}}\n\n", n.entity_ref));
    t.push_str(&doc(&tr(o, &format!("{} は、ポリシーが読むエンティティです。属性（bool、int64、string か {}）と、親のエンティティを持ちます。", n.entity, n.entity_ref), &format!("{} is an entity the policies read: its attributes (a bool, an int64, a string or an {}) and the entities it is in.", n.entity, n.entity_ref)), ""));
    t.push_str(&format!("type {} struct {{\n\tUID     {}\n\tAttrs   map[string]any\n\tParents []{}\n}}\n\n", n.entity, n.entity_ref, n.entity_ref));
    t.push_str(&doc(&tr(o, &format!("{} は、Cedar に尋ねるものです。principal、action、resource、context と、ポリシーが読むエンティティを持ちます。", n.request), &format!("{} is what Cedar is asked: the principal, the action, the resource and the context, with the entities the policies read.", n.request)), ""));
    t.push_str(&format!("type {} struct {{\n\tPrincipal {}\n\tAction    {}\n\tResource  {}\n\tContext   map[string]any\n\tEntities  []{}\n}}\n\n", n.request, n.entity_ref, n.entity_ref, n.entity_ref, n.entity));
    t.push_str(&doc(&tr(o, &format!("{} は答えです。許すか、決めたポリシー、Cedar に渡した context（判断の記録のため）、尋ねられなかったときはその理由を持ちます。", n.answer), &format!("{} is the answer: allowed or not, the policies that decided it, the context Cedar was given (for the log of decisions), and why, when the gate could not be asked.", n.answer)), ""));
    t.push_str(&format!("type {} struct {{\n\tAllowed  bool\n\tPolicies []string\n\tContext  map[string]any\n\tErr      *{}\n}}\n\n", n.answer, n.error));
    if o.authorizer == Authorizer::Avp {
        t.push_str(&doc(&tr(o, &format!("{} は尋ねる先です。Amazon Verified Permissions のクライアント（AWS SDK の *verifiedpermissions.Client がこのメソッドを持ちます）と、ゲートの Cedar を置いたポリシーストアの ID を持ちます。", n.avp), &format!("{} is where the gate is asked: a client of Amazon Verified Permissions (the AWS SDK's *verifiedpermissions.Client has the method), and the id of the policy store the gate's Cedar is in.", n.avp)), ""));
        t.push_str(&format!("type {} struct {{\n\tClient interface {{\n\t\tIsAuthorized(ctx context.Context, params *verifiedpermissions.IsAuthorizedInput, optFns ...func(*verifiedpermissions.Options)) (*verifiedpermissions.IsAuthorizedOutput, error)\n\t}}\n\tPolicyStoreID string\n}}\n\n", n.avp));
    }
}

/// Fields of a struct, their types and comments aligned as gofmt aligns them: the types across
/// every field, the comments across each run of fields that have one.
fn aligned(lines: &[(String, String, String)], t: &mut String) {
    let w1 = lines.iter().map(|(a, _, _)| a.chars().count()).max().unwrap_or(0);
    // the width of the types in the run of commented lines each line is in
    let mut w2 = vec![0usize; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        if lines[i].2.is_empty() {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < lines.len() && !lines[j].2.is_empty() {
            j += 1;
        }
        let w = lines[i..j].iter().map(|(_, b, _)| b.chars().count()).max().unwrap_or(0);
        for x in &mut w2[i..j] {
            *x = w;
        }
        i = j;
    }
    for (k, (a, b, c)) in lines.iter().enumerate() {
        let pad1 = " ".repeat(w1 - a.chars().count());
        if c.is_empty() {
            t.push_str(&format!("\t{a}{pad1} {b}\n"));
        } else {
            let pad2 = " ".repeat(w2[k].saturating_sub(b.chars().count()));
            t.push_str(&format!("\t{a}{pad1} {b}{pad2}{c}\n"));
        }
    }
}

/// What a field is held to, as a statement: `if err := checkInt(…); err != nil { return req, err }`.
fn check(g: &Gate, kind: &str, what: &str, f: &Field, value: &str, indent: &str, ret: &str) -> Option<String> {
    let call = match &f.ty {
        FieldType::Num { lo, hi, .. } => format!("checkInt({}, {what}, {value}, {lo}, {hi})", s(kind)),
        FieldType::Enum(en) => {
            let vs: Vec<String> = g.enums[*en].values.iter().map(|v| s(&v.alias)).collect();
            format!("checkEnum({}, {what}, {value}, {})", s(kind), vs.join(", "))
        }
        FieldType::Date { lo, hi } => format!("checkDate({}, {what}, {value}, {lo}, {hi})", s(kind)),
        FieldType::Bool | FieldType::Entity(_) => return None,
    };
    Some(format!("{indent}if err := {call}; err != nil {{\n{indent}\treturn {ret}err\n{indent}}}\n"))
}

fn helpers(p: &Plan, n: &Names, o: &Options, t: &mut String) {
    let g = p.g;
    let e = &n.error;
    let er = &n.entity_ref;
    let en = &n.entity;
    let dt = &n.date;
    // the roles
    if !g.roles.is_empty() {
        let role_type = p.cedar_name(crate::cedar::ROLE);
        t.push_str(&doc(&tr(o, "roles は、ゲートの役割のエンティティです。親は、その役割が includes に書いた役割です。", "roles are the gate's roles as entities, each in the roles it includes."), ""));
        t.push_str(&format!("var roles = []{en}{{\n"));
        for r in &g.roles {
            let parents: Vec<String> = r.includes.iter().map(|&x| format!("{{{}, {}}}", s(&role_type), s(&g.roles[x].named.alias))).collect();
            t.push_str(&format!("\t{{UID: {er}{{{}, {}}}, Attrs: map[string]any{{}}, Parents: []{er}{{{}}}}},\n", s(&role_type), s(&r.named.alias), parents.join(", ")));
        }
        t.push_str("}\n\n");
    }
    let outside = tr(o, "%s %d が %d〜%d の外です", "%s %d is outside %d to %d");
    t.push_str(&format!("func checkInt(kind, what string, v, lo, hi int64) error {{\n\tif v < lo || v > hi {{\n\t\treturn &{e}{{kind, fmt.Sprintf({}, what, v, lo, hi)}}\n\t}}\n\treturn nil\n}}\n\n", s(&outside)));
    t.push_str(&format!(
        "func checkEnum(kind, what, v string, values ...string) error {{\n\tfor _, x := range values {{\n\t\tif v == x {{\n\t\t\treturn nil\n\t\t}}\n\t}}\n\treturn &{e}{{kind, fmt.Sprintf({}, what, v, values)}}\n}}\n\n",
        s(&tr(o, "%s %q は %q のどれでもありません", "%s %q is none of %q"))
    ));
    t.push_str(&doc(&tr(o, "days は、暦の日の、1970-01-01 からの日数です。", "days is the number of days since 1970-01-01 of a day of the calendar."), ""));
    t.push_str("func days(y, m, d int) int {\n\tif m <= 2 {\n\t\ty--\n\t}\n\tera := y / 400\n\tif y < 0 && y%400 != 0 {\n\t\tera--\n\t}\n\tyoe := y - era*400\n\tmp := (m + 9) % 12\n\tdoy := (153*mp+2)/5 + d - 1\n\tdoe := yoe*365 + yoe/4 - yoe/100 + doy\n\treturn era*146097 + doe - 719468\n}\n\n");
    t.push_str(&doc(&tr(o, "civil は、1970-01-01 からの日数の、暦の日です。", "civil is the day of the calendar of a number of days since 1970-01-01."), ""));
    t.push_str(&format!("func civil(z int) {dt} {{\n\tz += 719468\n\tera := z / 146097\n\tif z < 0 && z%146097 != 0 {{\n\t\tera--\n\t}}\n\tdoe := z - era*146097\n\tyoe := (doe - doe/1460 + doe/36524 - doe/146096) / 365\n\ty := yoe + era*400\n\tdoy := doe - (365*yoe + yoe/4 - yoe/100)\n\tmp := (5*doy + 2) / 153\n\td := doy - (153*mp+2)/5 + 1\n\tm := mp + 3\n\tif mp >= 10 {{\n\t\tm = mp - 9\n\t}}\n\tif m <= 2 {{\n\t\ty++\n\t}}\n\treturn {dt}{{y, m, d}}\n}}\n\n"));
    t.push_str(&format!(
        "func checkDate(kind, what string, v {dt}, lo, hi int) error {{\n\tz := days(v.Year, v.Month, v.Day)\n\tif civil(z) != v {{\n\t\treturn &{e}{{kind, fmt.Sprintf({}, what, v)}}\n\t}}\n\tif z < lo || z > hi {{\n\t\tl, h := civil(lo), civil(hi)\n\t\treturn &{e}{{kind, fmt.Sprintf({}, what, v.Year, v.Month, v.Day, l.Year, l.Month, l.Day, h.Year, h.Month, h.Day)}}\n\t}}\n\treturn nil\n}}\n\n",
        s(&tr(o, "%s %v は日付ではありません", "%s %v is not a day of the calendar")),
        s(&tr(o, "%s %04d-%02d-%02d が %04d-%02d-%02d〜%04d-%02d-%02d の外です", "%s %04d-%02d-%02d is outside %04d-%02d-%02d to %04d-%02d-%02d"))
    ));
    if let Some(td) = p.today() {
        let off = td.offset;
        let shown = ritsu_base::naming::quote(&ritsu_emit::header::file_name(&g.file));
        let (lo, hi) = (ritsu_ports::day_text(td.lo), ritsu_ports::day_text(td.hi));
        let at = plan::offset_text(off);
        let d = tr(o, &format!("today は now の日（1970-01-01 からの日数。日は UTC{at} で変わる）です。{shown} を確かめた範囲 {lo}..{hi} の外ならエラーです。"), &format!("today is the day of now, as the days since 1970-01-01, the day changing at {at}; an error outside {lo}..{hi}, the range {shown} was checked over."));
        t.push_str(&doc(&d, ""));
        t.push_str(&format!(
            "func today(now time.Time) (int, error) {{\n\ty, m, d := now.UTC().Add({off} * time.Minute).Date()\n\tz := days(y, int(m), d)\n\tif z < {} || z > {} {{\n\t\treturn 0, &{e}{{\"today\", fmt.Sprintf({}, y, int(m), d)}}\n\t}}\n\treturn z, nil\n}}\n\n",
            td.lo,
            td.hi,
            s(&tr(o, &format!("today %04d-%02d-%02d は、{shown} を確かめた範囲 {lo}..{hi} の外です"), &format!("today %04d-%02d-%02d is outside {lo}..{hi}, the range {shown} was checked over")))
        ));
    }
    // a read of each type, and its entity
    for (&ti, rec_name) in &n.records {
        let ty = &g.types[ti];
        let r = record(p, ti);
        let kind = if ty.kind == Kind::Resource { "resource" } else { "principal" };
        let alias = &ty.named.alias;
        t.push_str(&doc(&tr(o, &format!("read{alias} は、{alias} を store から読み、reads に挙げた属性を、ゲートが宣言した範囲で確かめます。"), &format!("read{alias} is the {alias} the store holds, the attributes in reads held to the range the gate declares.")), ""));
        t.push_str(&format!("func read{alias}(store {}, id string, reads ...string) (*{rec_name}, error) {{\n", n.store));
        t.push_str(&format!("\tfound, err := store.{alias}(id)\n\tif err != nil {{\n\t\treturn nil, &{e}{{{}, fmt.Sprintf({}, id, err)}}\n\t}}\n", s(kind), s(&tr(o, &format!("{alias} %s を読めません: %v"), &format!("cannot read the {alias} %s: %v")))));
        t.push_str(&format!("\tif found == nil {{\n\t\treturn nil, &{e}{{{}, {} + id}}\n\t}}\n", s(kind), s(&tr(o, &format!("{alias} がありません: "), &format!("no {alias} ")))));
        if let Some(roles) = &r.roles {
            let vs: Vec<String> = ty.roles.iter().map(|&x| s(&g.roles[x].named.alias)).collect();
            t.push_str(&format!("\tfor _, role := range found.{roles} {{\n\t\tif err := checkEnum({}, {}+id+\": role\", role, {}); err != nil {{\n\t\t\treturn nil, err\n\t\t}}\n\t}}\n", s(kind), s(&format!("{alias} ")), vs.join(", ")));
        }
        let checked: Vec<(usize, &Field)> = ty.attrs.iter().enumerate().filter(|(_, f)| matches!(f.ty, FieldType::Num { .. } | FieldType::Enum(_) | FieldType::Date { .. })).collect();
        if !checked.is_empty() {
            t.push_str("\tfor _, a := range reads {\n\t\tswitch a {\n");
            for (i, f) in checked {
                let field = &r.attrs[i];
                let what = format!("{}+id+{}", s(&format!("{alias} ")), s(&format!(": {}", f.named.alias)));
                t.push_str(&format!("\t\tcase {}:\n", s(&f.named.alias)));
                if f.optional {
                    t.push_str(&format!("\t\t\tif found.{field} != nil {{\n"));
                    if let Some(c) = check(g, kind, &what, f, &format!("*found.{field}"), "\t\t\t\t", "nil, ") {
                        t.push_str(&c);
                    }
                    t.push_str("\t\t\t}\n");
                } else if let Some(c) = check(g, kind, &what, f, &format!("found.{field}"), "\t\t\t", "nil, ") {
                    t.push_str(&c);
                }
            }
            t.push_str("\t\t}\n\t}\n");
        }
        t.push_str("\treturn found, nil\n}\n\n");
        // the entity
        t.push_str(&format!("func entity{alias}(uid {er}, r *{rec_name}) {en} {{\n"));
        t.push_str("\tattrs := map[string]any{}\n");
        for &i in p.cedar_attrs(ti) {
            let f = &ty.attrs[i];
            let field = &r.attrs[i];
            let deref = if f.optional { format!("*r.{field}") } else { format!("r.{field}") };
            let value = match &f.ty {
                FieldType::Entity(et) => format!("{er}{{{}, {deref}}}", s(&p.cedar_type(*et))),
                _ => deref,
            };
            if f.optional {
                t.push_str(&format!("\tif r.{field} != nil {{\n\t\tattrs[{}] = {value}\n\t}}\n", s(&f.named.alias)));
            } else {
                t.push_str(&format!("\tattrs[{}] = {value}\n", s(&f.named.alias)));
            }
        }
        t.push_str(&format!("\tparents := []{er}{{}}\n"));
        if let Some(roles) = &r.roles {
            t.push_str(&format!("\tfor _, x := range r.{roles} {{\n\t\tparents = append(parents, {er}{{{}, x}})\n\t}}\n", s(&p.cedar_name(crate::cedar::ROLE))));
        }
        for (gt, field) in &r.groups {
            t.push_str(&format!("\tfor _, x := range r.{field} {{\n\t\tparents = append(parents, {er}{{{}, x}})\n\t}}\n", s(&p.cedar_type(*gt))));
        }
        t.push_str(&format!("\treturn {en}{{UID: uid, Attrs: attrs, Parents: parents}}\n}}\n\n"));
        let pointed = super::python::pointed(p, ti);
        if !pointed.is_empty() {
            t.push_str(&format!("func pointed{alias}(r *{rec_name}) []{er} {{\n\tout := []{er}{{}}\n"));
            for (et, i, optional) in pointed {
                let field = &r.attrs[i];
                if optional {
                    t.push_str(&format!("\tif r.{field} != nil {{\n\t\tout = append(out, {er}{{{}, *r.{field}}})\n\t}}\n", s(&p.cedar_type(et))));
                } else {
                    t.push_str(&format!("\tout = append(out, {er}{{{}, r.{field}}})\n", s(&p.cedar_type(et))));
                }
            }
            t.push_str("\treturn out\n}\n\n");
        }
    }
    // the request
    let roles = if g.roles.is_empty() { format!("[]{en}{{}}") } else { format!("append([]{en}{{}}, roles...)") };
    t.push_str(&format!(
        "func request(action string, pEntity, rEntity {en}, pointed []{er}, ctx map[string]any) {} {{\n\tes := append({roles}, pEntity, rEntity)\n\tseen := []{er}{{pEntity.UID, rEntity.UID}}\n\tfor _, uid := range pointed {{\n\t\tknown := false\n\t\tfor _, x := range seen {{\n\t\t\tknown = known || x == uid\n\t\t}}\n\t\tif !known {{\n\t\t\tseen = append(seen, uid)\n\t\t\tes = append(es, {en}{{UID: uid, Attrs: map[string]any{{}}, Parents: []{er}{{}}}})\n\t\t}}\n\t}}\n\treturn {}{{Principal: pEntity.UID, Action: {er}{{{}, action}}, Resource: rEntity.UID, Context: ctx, Entities: es}}\n}}\n\n",
        n.request,
        n.request,
        s(&p.cedar_name("Action"))
    ));
}

/// The types of the other owner, grouped by what the code reads of the owner's type `t` in a
/// request of each (as the Python groups them).
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

/// The code that reads an owner's entity of type `t` and makes its entity for Cedar.
#[allow(clippy::too_many_arguments)]
fn read_entity(p: &Plan, n: &Names, ai: usize, owner: Owner, t: usize, indent: &str, other: &str, id: &str, out: &mut String, used: &mut bool) {
    let g = p.g;
    let pre = if owner == Owner::Principal { "p" } else { "r" };
    let alias = &g.types[t].named.alias;
    let er = &n.entity_ref;
    let en = &n.entity;
    let groups = read_groups(p, ai, owner, t);
    let body = |reads: bool, attrs: &[String], ind: &str, out: &mut String, used: &mut bool| {
        if reads {
            let v = var(owner, alias);
            let need: Vec<String> = attrs.iter().map(|x| s(x)).collect();
            let need = if need.is_empty() { String::new() } else { format!(", {}", need.join(", ")) };
            out.push_str(&format!("{ind}{v}, err = read{alias}(store, {id}{need})\n{ind}if err != nil {{\n{ind}\treturn req, err\n{ind}}}\n"));
            *used = true;
            out.push_str(&format!("{ind}{pre}Entity = entity{alias}({pre}UID, {v})\n"));
            if !super::python::pointed(p, t).is_empty() {
                out.push_str(&format!("{ind}{pre}Pointed = pointed{alias}({v})\n"));
            }
        } else {
            out.push_str(&format!("{ind}{pre}Entity = {en}{{UID: {pre}UID, Attrs: map[string]any{{}}, Parents: []{er}{{}}}}\n"));
        }
    };
    if groups.len() == 1 {
        body(groups[0].1, &groups[0].2, indent, out, used);
        return;
    }
    out.push_str(&format!("{indent}switch {other} {{\n"));
    for (k, (others, reads, attrs)) in groups.iter().enumerate() {
        if k + 1 == groups.len() {
            out.push_str(&format!("{indent}default:\n"));
        } else {
            let lits: Vec<String> = others.iter().map(|&x| s(&g.types[x].named.alias)).collect();
            out.push_str(&format!("{indent}case {}:\n", lits.join(", ")));
        }
        body(*reads, attrs, &format!("{indent}\t"), out, used);
    }
    out.push_str(&format!("{indent}}}\n"));
}

/// The action's functions.
fn action(p: &Plan, n: &Names, o: &Options, ap: &ActionPlan, t: &mut String) {
    let g = p.g;
    let ai = ap.index;
    let a = &g.actions[ai];
    let alias = &a.named.alias;
    let name = exported(alias);
    let e = &n.error;
    let er = &n.entity_ref;
    let en = &n.entity;
    let input = &n.inputs[&ai];
    let operation = plan::guarded(p.g, a, o.lang);
    let mut d = match &operation {
        Some(op) => tr(o, &format!("{name}Request は、action {alias} のリクエストを組み立てます。{op} を守ります。"), &format!("{name}Request builds the request for the action {alias}, which guards {op}.")),
        None => tr(o, &format!("{name}Request は、action {alias} のリクエストを組み立てます。"), &format!("{name}Request builds the request for the action {alias}.")),
    };
    if let Some(desc) = plan::action_description(p.scope, alias) {
        d = plan::sentences(&[d, plan::sentence(&desc)]);
    }
    d = plan::sentences(&[d, tr(o, &format!("組み立てられなければ *{e} を返します。"), &format!("It returns an *{e} when it cannot."))]);
    t.push_str(&doc(&d, ""));
    t.push_str(&format!("func {name}Request(store {}, p {}, in {input}, now time.Time) ({}, error) {{\n", n.store, n.principal, n.request));
    let mut b = String::new();
    let mut used_err = false;
    b.push_str(&format!("\tvar req {}\n", n.request));
    // 1. who asks
    let lits = |ts: &[usize]| ts.iter().map(|&x| s(&g.types[x].named.alias)).collect::<Vec<_>>().join(", ");
    b.push_str(&format!("\tswitch p.Type {{\n\tcase {}:\n\tdefault:\n\t\treturn req, &{e}{{\"principal\", fmt.Sprintf({}, p.Type)}}\n\t}}\n", lits(&a.principals), s(&tr(o, &format!("{alias} を %s が尋ねることはありません"), &format!("{alias} is not asked by a %s")))));
    if let Some(wt) = g.workflow_type().filter(|w| a.principals.contains(w)) {
        let ws: Vec<String> = g.workflows.iter().map(|w| s(&w.named.alias)).collect();
        b.push_str(&format!("\tif p.Type == {} && checkEnum(\"principal\", \"workflow\", p.ID, {}) != nil {{\n\t\treturn req, &{e}{{\"principal\", {} + p.ID}}\n\t}}\n", s(&g.types[wt].named.alias), ws.join(", "), s(&tr(o, "ワークフローがありません: ", "no workflow "))));
    }
    // 2. the day
    let day_at = b.len();
    // 3. the input
    for f in &a.inputs {
        let field = exported(&f.named.alias);
        let what = s(&f.named.alias);
        if f.optional {
            if let Some(c) = check(g, "input", &what, f, &format!("*in.{field}"), "\t\t", "req, ") {
                b.push_str(&format!("\tif in.{field} != nil {{\n{c}\t}}\n"));
            }
        } else if let Some(c) = check(g, "input", &what, f, &format!("in.{field}"), "\t", "req, ") {
            b.push_str(&c);
        }
    }
    // 4. the resource
    let multi = a.resources.len() > 1;
    let mut r_body = String::new();
    r_body.push_str(&format!("\trID := in.{}\n", exported(&plan::resource_field(a))));
    r_body.push_str(&format!("\tvar rUID {er}\n\tvar rEntity {en}\n\tvar rPointed []{er}\n"));
    let recorded_r: Vec<usize> = a.resources.iter().copied().filter(|&x| read_groups(p, ai, Owner::Resource, x).iter().any(|gr| gr.1)).collect();
    for &rt in &recorded_r {
        r_body.push_str(&format!("\tvar {} *{}\n", var(Owner::Resource, &g.types[rt].named.alias), n.records[&rt]));
    }
    if multi {
        r_body.push_str("\tswitch in.ResourceType {\n");
        for &rt in &a.resources {
            r_body.push_str(&format!("\tcase {}:\n", s(&g.types[rt].named.alias)));
            r_body.push_str(&format!("\t\trUID = {er}{{{}, rID}}\n", s(&p.cedar_type(rt))));
            read_entity(p, n, ai, Owner::Resource, rt, "\t\t", "p.Type", "rID", &mut r_body, &mut used_err);
        }
        r_body.push_str(&format!("\tdefault:\n\t\treturn req, &{e}{{\"resource\", fmt.Sprintf({}, in.ResourceType)}}\n\t}}\n", s(&tr(o, &format!("{alias} を %s に尋ねることはありません"), &format!("{alias} is not asked of a %s")))));
    } else {
        let rt = a.resources[0];
        r_body.push_str(&format!("\trUID = {er}{{{}, rID}}\n", s(&p.cedar_type(rt))));
        read_entity(p, n, ai, Owner::Resource, rt, "\t", "p.Type", "rID", &mut r_body, &mut used_err);
    }
    // 5. the principal
    let mut p_body = String::new();
    p_body.push_str(&format!("\tvar pUID {er}\n\tvar pEntity {en}\n\tvar pPointed []{er}\n"));
    let recorded_p: Vec<usize> = a.principals.iter().copied().filter(|&x| read_groups(p, ai, Owner::Principal, x).iter().any(|gr| gr.1)).collect();
    for &pt in &recorded_p {
        p_body.push_str(&format!("\tvar {} *{}\n", var(Owner::Principal, &g.types[pt].named.alias), n.records[&pt]));
    }
    p_body.push_str("\tswitch p.Type {\n");
    for &pt in &a.principals {
        p_body.push_str(&format!("\tcase {}:\n", s(&g.types[pt].named.alias)));
        p_body.push_str(&format!("\t\tpUID = {er}{{{}, p.ID}}\n", s(&p.cedar_type(pt))));
        read_entity(p, n, ai, Owner::Principal, pt, "\t\t", "in.ResourceType", "p.ID", &mut p_body, &mut used_err);
    }
    p_body.push_str("\t}\n");
    // 6. the context
    let mut c_body = String::new();
    c_body.push_str("\tctx := map[string]any{}\n");
    for &i in &p.shape.actions[ai].inputs {
        let f = &a.inputs[i];
        let field = exported(&f.named.alias);
        if f.optional {
            c_body.push_str(&format!("\tif in.{field} != nil {{\n\t\tctx[{}] = *in.{field}\n\t}}\n", s(&f.named.alias)));
        } else {
            c_body.push_str(&format!("\tctx[{}] = in.{field}\n", s(&f.named.alias)));
        }
    }
    for vp in &ap.values {
        value(p, n, a, vp, &mut c_body);
    }
    // the day, held to today's range by an action that computes a value from it: kept when a value
    // reads it
    let reads_day = c_body.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).any(|w| w == "day");
    let mut day_code = String::new();
    if p.today().is_some() && ap.reads_today {
        if reads_day {
            day_code.push_str("\tday, err := today(now)\n\tif err != nil {\n\t\treturn req, err\n\t}\n");
        } else {
            day_code.push_str("\tif _, err := today(now); err != nil {\n\t\treturn req, err\n\t}\n");
        }
    }
    b.insert_str(day_at, &day_code);
    let declared_err = p.today().is_some() && ap.reads_today && reads_day;
    if used_err && !declared_err {
        b.push_str("\tvar err error\n");
    }
    b.push_str(&r_body);
    b.push_str(&p_body);
    b.push_str(&c_body);
    b.push_str(&format!("\treturn request({}, pEntity, rEntity, append(pPointed, rPointed...), ctx), nil\n", s(alias)));
    t.push_str(&b);
    t.push_str("}\n\n");
    // authorize
    let how = match o.authorizer {
        Authorizer::Cedar => tr(o, "このプロセスの中の cedar-go", "cedar-go, in this process"),
        Authorizer::Avp => "Amazon Verified Permissions".to_string(),
    };
    t.push_str(&doc(&tr(o, &format!("Authorize{name} は、{alias} を許すかを Cedar（{how}）に尋ねます。エラーのときは、拒む答えを返します。"), &format!("Authorize{name} asks Cedar ({how}) whether {alias} is allowed. Any error is a deny.")), ""));
    match o.authorizer {
        Authorizer::Cedar => t.push_str(&format!("func Authorize{name}(store {}, p {}, in {input}, now time.Time) {} {{\n", n.store, n.principal, n.answer)),
        Authorizer::Avp => t.push_str(&format!("func Authorize{name}(ctx context.Context, avp {}, store {}, p {}, in {input}, now time.Time) {} {{\n", n.avp, n.store, n.principal, n.answer)),
    }
    t.push_str(&format!("\treq, err := {name}Request(store, p, in, now)\n\tif err != nil {{\n\t\tx, ok := err.(*{e})\n\t\tif !ok {{\n\t\t\tx = &{e}{{\"input\", err.Error()}}\n\t\t}}\n\t\treturn {}{{Policies: []string{{}}, Err: x}}\n\t}}\n", n.answer));
    match o.authorizer {
        Authorizer::Cedar => t.push_str("\treturn ask(req)\n}\n\n"),
        Authorizer::Avp => t.push_str("\treturn ask(ctx, avp, req)\n}\n\n"),
    }
}

/// The code of one value the action computes, put into `ctx` under its alias.
fn value(p: &Plan, n: &Names, a: &Action, vp: &ValuePlan, t: &mut String) {
    let g = p.g;
    let cv = &a.computed[vp.index];
    let e = &n.error;
    let key = s(&cv.named.alias);
    t.push_str(&format!("\t// {}\n", ritsu_emit::header::one_line(&plan::line_of(g, cv.named.line))));
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
                conds.push(format!("{} != nil", var(Owner::Principal, &g.types[*x].named.alias)));
            }
            if let Some(x) = rt {
                conds.push(format!("{} != nil", var(Owner::Resource, &g.types[*x].named.alias)));
            }
            let mut optional: Vec<String> = Vec::new();
            // a value the call reads: the attribute (dereferenced when it may be absent, which the
            // condition guards), the input, today
            let source = |src: &Source, optional: &mut Vec<String>| -> String {
                match src {
                    Source::Attr(owner, name) => {
                        let ty = match owner {
                            Owner::Principal => pt.unwrap_or(0),
                            Owner::Resource => rt.unwrap_or(0),
                        };
                        let r = record(p, ty);
                        let Some((i, f)) = g.types[ty].attr(name) else { return String::new() };
                        let expr = format!("{}.{}", var(*owner, &g.types[ty].named.alias), r.attrs[i]);
                        if f.optional {
                            optional.push(format!("{expr} != nil"));
                            format!("(*{expr})")
                        } else {
                            expr
                        }
                    }
                    Source::Input(name) => {
                        let Some((_, f)) = a.input(name) else { return String::new() };
                        let expr = format!("in.{}", exported(&f.named.alias));
                        if f.optional {
                            optional.push(format!("{expr} != nil"));
                            format!("(*{expr})")
                        } else {
                            expr
                        }
                    }
                    Source::Today => "civil(day)".to_string(),
                    Source::Lit(_) => String::new(),
                }
            };
            let mut body: Vec<String> = Vec::new();
            match &vp.call {
                Call::Rule { module, output, args, values } => {
                    let Some(facts) = p.rule(*module) else { continue };
                    let m = import_name(p, *module);
                    let go = &facts.go;
                    let mut fields: Vec<String> = Vec::new();
                    for (k, (col, src)) in args.iter().enumerate() {
                        let Some(src) = src else { continue };
                        let Some(param) = go.params.iter().find(|x| x.name == col.name) else { continue };
                        let pty = param.ty.trim_start_matches('*').to_string();
                        let raw = source(src, &mut optional);
                        let v = rule_arg(p, facts, &m, col, &pty, src, &raw);
                        if param.optional {
                            body.push(format!("a{k} := {v}"));
                            fields.push(format!("{}: &a{k}", param.alias));
                        } else {
                            fields.push(format!("{}: {v}", param.alias));
                        }
                    }
                    let call = format!("{m}.{}({m}.{}{{{}}})", go.function, go.input_type, fields.join(", "));
                    let failed = s(&format!("rulec {}: ", p.use_path(*module)));
                    if facts.outputs.len() > 1 {
                        let out_alias = go.outputs.iter().find(|x| x.name == output.name).map(|x| x.alias.clone()).unwrap_or_else(|| exported(&output.alias));
                        body.push(format!("out, err := {call}"));
                        body.push(format!("if err != nil {{\n\treturn req, &{e}{{\"rule\", {failed} + err.Error()}}\n}}"));
                        body.push(format!("v := out.{out_alias}"));
                    } else {
                        body.push(format!("v, err := {call}"));
                        body.push(format!("if err != nil {{\n\treturn req, &{e}{{\"rule\", {failed} + err.Error()}}\n}}"));
                    }
                    let optional_out = matches!(output.ty, ritsu_ports::ColumnType::Opt(_));
                    let got = if optional_out { "(*v)" } else { "v" };
                    let mut put_lines: Vec<String> = Vec::new();
                    let put = match (values, plan::strip(&output.ty)) {
                        (Some(vs), ritsu_ports::ColumnType::Enum(en)) => match go.enums.iter().find(|x| x.name == *en) {
                            Some(ce) => {
                                let pairs: Vec<String> = vs.iter().filter_map(|(nm, public)| ce.values.iter().find(|(x, _)| x == nm).map(|(_, mem)| format!("{m}.{mem}: {}", s(public)))).collect();
                                put_lines.push(format!("name, ok := map[{m}.{}]string{{{}}}[{got}]", ce.alias, pairs.join(", ")));
                                put_lines.push(format!("if !ok {{\n\treturn req, &{e}{{\"rule\", {}}}\n}}", s(&format!("rulec {}: not a value of the output", p.use_path(*module)))));
                                "name".to_string()
                            }
                            None => got.to_string(),
                        },
                        _ => got.to_string(),
                    };
                    put_lines.push(format!("ctx[{key}] = {put}"));
                    if optional_out {
                        let inner: Vec<String> = put_lines.iter().flat_map(|l| l.lines().map(|x| format!("\t{x}")).collect::<Vec<_>>()).collect();
                        body.push(format!("if v != nil {{\n{}\n}}", inner.join("\n")));
                    } else {
                        body.extend(put_lines);
                    }
                }
                Call::Date { module, function, op, args } => {
                    let m = import_name(p, *module);
                    let mut call_args: Vec<String> = Vec::new();
                    for (_, is_day, src) in args {
                        call_args.push(match src {
                            Source::Lit(Literal::Date(d)) => {
                                let (y, mo, dd) = plan::ymd(*d);
                                format!("{m}.Date{{Year: {y}, Month: {mo}, Day: {dd}}}")
                            }
                            Source::Lit(Literal::Num(num)) => num.value.num.to_string(),
                            other => {
                                let raw = source(other, &mut optional);
                                if *is_day {
                                    format!("{m}.Date{{Year: {raw}.Year, Month: {raw}.Month, Day: {raw}.Day}}")
                                } else {
                                    format!("int({raw})")
                                }
                            }
                        });
                    }
                    body.push(format!("v, err := {m}.{}({})", ritsu_emit::ident::pascal(function), call_args.join(", ")));
                    body.push(format!("if err != nil {{\n\treturn req, &{e}{{\"date\", {} + err.Error()}}\n}}", s(&format!("koyomi {}: ", p.use_path(*module)))));
                    body.push(format!("ctx[{key}] = {}", compare("day", *op, "days(v.Year, v.Month, v.Day)")));
                }
                Call::DateAttr { op, owner, attr } => {
                    let raw = source(&Source::Attr(*owner, attr.clone()), &mut optional);
                    body.push(format!("ctx[{key}] = {}", compare("day", *op, &format!("days({raw}.Year, {raw}.Month, {raw}.Day)"))));
                }
                Call::Open { module } => {
                    let m = import_name(p, *module);
                    body.push("d := civil(day)".to_string());
                    body.push(format!("v, err := {m}.IsOpen({m}.Date{{Year: d.Year, Month: d.Month, Day: d.Day}})"));
                    body.push(format!("if err != nil {{\n\treturn req, &{e}{{\"date\", {} + err.Error()}}\n}}", s(&format!("koyomi {}: ", p.use_path(*module)))));
                    body.push(format!("ctx[{key}] = v"));
                }
            }
            // in a block of its own, behind its branch and the values that may be absent
            let mut all = conds;
            all.extend(optional);
            let open = if all.is_empty() { "{".to_string() } else { format!("if {} {{", all.join(" && ")) };
            t.push_str(&format!("\t{open}\n"));
            for l in body.iter().flat_map(|b| b.lines().map(str::to_string).collect::<Vec<_>>()) {
                t.push_str(&format!("\t\t{l}\n"));
            }
            t.push_str("\t}\n");
        }
    }
}

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

/// A value given to an input of a rule, as the rule's Go takes it (`pty`, its type there).
fn rule_arg(p: &Plan, facts: &ritsu_ports::RuleFacts, m: &str, col: &ritsu_ports::Column, pty: &str, src: &Source, raw: &str) -> String {
    use ritsu_ports::ColumnType as C;
    let g = p.g;
    let typed = |v: String| if pty == "int64" { format!("int64({v})") } else { format!("{m}.{pty}({v})") };
    match (plan::strip(&col.ty), src) {
        (C::Date, Source::Lit(Literal::Date(d))) => format!("int64({d})"),
        (C::Date, Source::Today) => "int64(day)".to_string(),
        (C::Date, _) => format!("int64(days({raw}.Year, {raw}.Month, {raw}.Day))"),
        (C::Bool, Source::Lit(Literal::Bool(b))) => b.to_string(),
        (C::Bool, _) => raw.to_string(),
        (C::Num { unit, .. }, Source::Lit(Literal::Num(num))) => {
            let v = unit.as_ref().and_then(|u| crate::types::count(num, u).ok()).unwrap_or(num.value.num);
            typed(v.to_string())
        }
        (C::Num { .. }, _) => typed(raw.to_string()),
        (C::Enum(en), src) => {
            let Some(ce) = facts.go.enums.iter().find(|x| x.name == *en) else { return raw.to_string() };
            let Some(re) = facts.enums.iter().find(|x| x.name == *en) else { return raw.to_string() };
            let member_of = |word: &str| -> Option<String> {
                let v = re.values.iter().find(|v| v.name == word || v.alias == word || v.public == word)?;
                ce.values.iter().find(|(nm, _)| *nm == v.name).map(|(_, mem)| format!("{m}.{mem}"))
            };
            match src {
                Source::Lit(Literal::Word(w)) => member_of(w).unwrap_or_else(|| raw.to_string()),
                Source::Attr(..) | Source::Input(..) => {
                    let ge = match src {
                        Source::Attr(_, name) => g.types.iter().find_map(|t| t.attr(name).map(|(_, f)| f.ty.clone())),
                        Source::Input(name) => g.actions.iter().find_map(|a| a.input(name).map(|(_, f)| f.ty.clone())),
                        _ => None,
                    };
                    let pairs: Vec<String> = match ge {
                        Some(FieldType::Enum(ge)) => g.enums[ge]
                            .values
                            .iter()
                            .filter_map(|gv| re.values.iter().find(|v| gv.is(&v.name) || gv.is(&v.alias) || gv.is(&v.public)).and_then(|v| member_of(&v.name)).map(|mem| format!("{}: {mem}", s(&gv.alias))))
                            .collect(),
                        _ => Vec::new(),
                    };
                    format!("map[string]{m}.{}{{{}}}[{raw}]", ce.alias, pairs.join(", "))
                }
                _ => raw.to_string(),
            }
        }
        _ => raw.to_string(),
    }
}

/// How the package asks Cedar.
fn ask(p: &Plan, n: &Names, o: &Options, t: &mut String) {
    let e = &n.error;
    let a = &n.answer;
    let er = &n.entity_ref;
    let en = &n.entity;
    match o.authorizer {
        Authorizer::Cedar => {
            t.push_str(&doc(&tr(o, "policies は、@id をキーにした JSON の形のポリシーです。一つのテキストで読むと、cedar-go はポリシーに policy0、policy1… と名前を付け、決めたポリシーがゲートの名前になりません。", "policies are the policies in Cedar's JSON keyed by their @id: read as one text, cedar-go names them policy0, policy1, …, and the policies that decide would not be named as the gate names them."), ""));
            t.push_str("var policies = func() *cedar.PolicySet {\n\tvar ps cedar.PolicySet\n\tif err := json.Unmarshal([]byte(policiesJSON), &ps); err != nil {\n\t\tpanic(err)\n\t}\n\treturn &ps\n}()\n\n");
            t.push_str(&format!("const policiesJSON = {}\n\n", s(&p.policies_json)));
            t.push_str(&format!("func cedarUID(r {er}) types.EntityUID {{\n\treturn types.NewEntityUID(types.EntityType(r.Type), types.String(r.ID))\n}}\n\n"));
            t.push_str(&format!("func cedarValue(v any) types.Value {{\n\tswitch x := v.(type) {{\n\tcase bool:\n\t\treturn types.Boolean(x)\n\tcase int64:\n\t\treturn types.Long(x)\n\tcase {er}:\n\t\treturn cedarUID(x)\n\tcase string:\n\t\treturn types.String(x)\n\t}}\n\treturn types.String(fmt.Sprint(v))\n}}\n\n"));
            t.push_str("func cedarRecord(m map[string]any) types.Record {\n\tr := types.RecordMap{}\n\tfor k, v := range m {\n\t\tr[types.String(k)] = cedarValue(v)\n\t}\n\treturn types.NewRecord(r)\n}\n\n");
            t.push_str(&format!("func ask(req {}) {a} {{\n\tes := types.EntityMap{{}}\n\tfor _, x := range req.Entities {{\n\t\tparents := []types.EntityUID{{}}\n\t\tfor _, p := range x.Parents {{\n\t\t\tparents = append(parents, cedarUID(p))\n\t\t}}\n\t\tes[cedarUID(x.UID)] = types.Entity{{UID: cedarUID(x.UID), Attributes: cedarRecord(x.Attrs), Parents: types.NewEntityUIDSet(parents...)}}\n\t}}\n", n.request));
            t.push_str("\td, diag := cedar.Authorize(policies, es, cedar.Request{Principal: cedarUID(req.Principal), Action: cedarUID(req.Action), Resource: cedarUID(req.Resource), Context: cedarRecord(req.Context)})\n");
            t.push_str(&format!("\tif len(diag.Errors) > 0 {{\n\t\treturn {a}{{Policies: []string{{}}, Context: req.Context, Err: &{e}{{\"cedar\", fmt.Sprint(diag.Errors)}}}}\n\t}}\n"));
            t.push_str("\tps := []string{}\n\tfor _, r := range diag.Reasons {\n\t\tps = append(ps, string(r.PolicyID))\n\t}\n\tsort.Strings(ps)\n");
            t.push_str(&format!("\treturn {a}{{Allowed: d == cedar.Allow, Policies: ps, Context: req.Context}}\n}}\n"));
        }
        Authorizer::Avp => {
            t.push_str(&format!("func cedarJSON(v any) any {{\n\tif r, ok := v.({er}); ok {{\n\t\treturn map[string]any{{\"__entity\": map[string]string{{\"type\": r.Type, \"id\": r.ID}}}}\n\t}}\n\treturn v\n}}\n\n"));
            t.push_str(&format!("func entitiesJSON(es []{en}) ([]byte, error) {{\n\tout := []map[string]any{{}}\n\tfor _, x := range es {{\n\t\tattrs := map[string]any{{}}\n\t\tfor k, v := range x.Attrs {{\n\t\t\tattrs[k] = cedarJSON(v)\n\t\t}}\n\t\tparents := []map[string]string{{}}\n\t\tfor _, p := range x.Parents {{\n\t\t\tparents = append(parents, map[string]string{{\"type\": p.Type, \"id\": p.ID}})\n\t\t}}\n\t\tout = append(out, map[string]any{{\"uid\": map[string]string{{\"type\": x.UID.Type, \"id\": x.UID.ID}}, \"attrs\": attrs, \"parents\": parents}})\n\t}}\n\treturn json.Marshal(out)\n}}\n\n"));
            t.push_str(&format!("func ask(ctx context.Context, avp {}, req {}) {a} {{\n", n.avp, n.request));
            t.push_str(&format!("\tfailed := func(err error) {a} {{\n\t\treturn {a}{{Policies: []string{{}}, Context: req.Context, Err: &{e}{{\"cedar\", err.Error()}}}}\n\t}}\n"));
            t.push_str("\tc, err := json.Marshal(req.Context)\n\tif err != nil {\n\t\treturn failed(err)\n\t}\n\tes, err := entitiesJSON(req.Entities)\n\tif err != nil {\n\t\treturn failed(err)\n\t}\n");
            t.push_str("\tout, err := avp.Client.IsAuthorized(ctx, &verifiedpermissions.IsAuthorizedInput{\n\t\tPolicyStoreId: aws.String(avp.PolicyStoreID),\n\t\tPrincipal:     &avptypes.EntityIdentifier{EntityType: aws.String(req.Principal.Type), EntityId: aws.String(req.Principal.ID)},\n\t\tAction:        &avptypes.ActionIdentifier{ActionType: aws.String(req.Action.Type), ActionId: aws.String(req.Action.ID)},\n\t\tResource:      &avptypes.EntityIdentifier{EntityType: aws.String(req.Resource.Type), EntityId: aws.String(req.Resource.ID)},\n\t\tContext:       &avptypes.ContextDefinitionMemberCedarJson{Value: string(c)},\n\t\tEntities:      &avptypes.EntitiesDefinitionMemberCedarJson{Value: string(es)},\n\t})\n\tif err != nil {\n\t\treturn failed(err)\n\t}\n");
            t.push_str(&format!("\tif len(out.Errors) > 0 {{\n\t\tsaid := []string{{}}\n\t\tfor _, x := range out.Errors {{\n\t\t\tsaid = append(said, aws.ToString(x.ErrorDescription))\n\t\t}}\n\t\treturn {a}{{Policies: []string{{}}, Context: req.Context, Err: &{e}{{\"cedar\", fmt.Sprint(said)}}}}\n\t}}\n"));
            t.push_str("\tps := []string{}\n\tfor _, x := range out.DeterminingPolicies {\n\t\tps = append(ps, aws.ToString(x.PolicyId))\n\t}\n\tsort.Strings(ps)\n");
            t.push_str(&format!("\treturn {a}{{Allowed: out.Decision == avptypes.DecisionAllow, Policies: ps, Context: req.Context}}\n}}\n"));
        }
    }
}
