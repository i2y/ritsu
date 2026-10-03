//! The first pass: read the rules through rulec, resolve every name, type every value,
//! and turn the statements into the checked tree of `model`. The run-dependent checks
//! (states of cases, assignment, exhaustiveness, exits) are the second pass, in `flow`.

use crate::apis::{ApiDoc, ApiKind, MadeTy};
use crate::diag::Diag;
use crate::model::*;
use crate::proto::ProtoFile;
use crate::rulec::{self, RType};
use crate::syntax::{self, Block, Call, Expr, MachineUse, Part, Program, RangeDecl, Span, StmtKind, TypeExpr};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const TYPE_HINT_EN: &str = "a type is int, string, bool, timestamp, json, a unit such as money[円, incl_tax], an enum, a record, list[T] or T?";
const TYPE_HINT_JA: &str = "型は int・string・bool・timestamp・json・money[円, incl_tax] のような単位・列挙・レコード・list[T]・T? のどれかです";

pub struct Lowerer<'a> {
    prog: &'a Program,
    /// the descriptions of the APIs the tasks call, by the name `use` gives them
    apis: BTreeMap<String, (crate::apis::ApiKind, crate::apis::Api)>,
    /// the directory of the `.flow`, which the paths of rules and child flows start from
    dir: std::path::PathBuf,
    pub diags: Vec<Diag>,
    m: Model,
    enum_ix: BTreeMap<String, EnumId>,
    record_ix: BTreeMap<String, RecordId>,
    /// the enums made from a `.proto`: a case's state may be one
    made_enums: BTreeSet<EnumId>,
    /// where a type made from a `.proto` was first named, by its name in the flow
    made_at: BTreeMap<String, Span>,
    /// the fields of made records whose type no file read has (an import was not read): the record,
    /// the field's key in JSON, and the type's name
    made_unread: Vec<(String, String, String)>,
    /// where each `use` names what it reads, by the name it gives
    use_spans: BTreeMap<String, Span>,
    /// the APIs whose missing `url` is already said
    url_said: BTreeSet<String>,
    rule_ix: BTreeMap<String, usize>,
    task_ix: BTreeMap<String, usize>,
    var_ty: BTreeMap<String, Ty>,
    site: usize,
    /// the loops around the statement being lowered: true for `for … in parallel`
    loops: Vec<bool>,
    /// how many `for … in parallel` are around it
    par_depth: usize,
    in_on_failure: bool,
    in_on_cancel: bool,
    /// a variable's type grew to take `none` in this pass over the names
    widened: bool,
}

fn e(code: &'static str, sp: Span, en: impl Into<String>, ja: impl Into<String>) -> Diag {
    Diag::error(code, sp.line, sp.col, en, ja)
}

/// The zero values protobuf's JSON leaves out of a rule's response: each field the rule always
/// answers, by its JSON key (`apis::fill` reads this form). A number is the string `"0"`, as the
/// 64-bit integer it is; an enum is the `.proto`'s name for its value 0: `<ENUM>_UNSPECIFIED`, which
/// the check of the answer refuses, or a value of the rule's when a contract puts one at 0 (`ACTIVE`).
fn rule_zeros(response: &[rulec::WireField]) -> serde_json::Value {
    let mut f = serde_json::Map::new();
    for w in response.iter().filter(|w| !w.optional) {
        let zero = match &w.kind {
            rulec::WireKind::Bool => serde_json::json!(false),
            rulec::WireKind::Int => serde_json::json!("0"),
            rulec::WireKind::Str => serde_json::json!(""),
            rulec::WireKind::Enum { zero, .. } => serde_json::json!(zero),
        };
        f.insert(w.json.clone(), zero);
    }
    if f.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::json!({ "f": f })
    }
}

pub fn lower(prog: &Program, file: &Path) -> (Option<Model>, Vec<Diag>) {
    let name = match &prog.name {
        Some((n, _)) => n.clone(),
        None => {
            return (
                None,
                vec![Diag::error("E001", 1, 1, "a .flow starts with `workflow <name> v<n>`", ".flow は `workflow <名前> v<番号>` で始めます")],
            )
        }
    };
    let mut lw = Lowerer {
        prog,
        apis: BTreeMap::new(),
        dir: file.parent().unwrap_or(Path::new(".")).to_path_buf(),
        diags: vec![],
        m: Model {
            name,
            version: prog.version,
            description: prog.description.clone().unwrap_or_default(),
            kind: prog.kind.clone(),
            source_file: file.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(),
            rules: vec![],
            enums: vec![],
            records: vec![],
            inputs: vec![],
            outputs: vec![],
            input_ranges: BTreeMap::new(),
            output_ranges: BTreeMap::new(),
            tasks: vec![],
            cases: vec![],
            vars: vec![],
            flow: vec![],
            on_failure: None,
            on_cancel: None,
            on_cancel_line: 0,
            monitors: BTreeMap::new(),
            service: None,
        },
        enum_ix: BTreeMap::new(),
        record_ix: BTreeMap::new(),
        made_enums: BTreeSet::new(),
        made_at: BTreeMap::new(),
        made_unread: Vec::new(),
        use_spans: BTreeMap::new(),
        url_said: BTreeSet::new(),
        rule_ix: BTreeMap::new(),
        task_ix: BTreeMap::new(),
        var_ty: BTreeMap::new(),
        site: 0,
        loops: vec![],
        par_depth: 0,
        in_on_failure: false,
        in_on_cancel: false,
        widened: false,
    };
    let base = file.parent().unwrap_or(Path::new("."));
    lw.rules(base);
    lw.apis(base);
    lw.local_types();
    lw.io();
    lw.tasks();
    lw.service();
    lw.cases();
    lw.variables();
    if let Some((b, _)) = &prog.flow {
        lw.m.flow = lw.block(b);
    } else {
        lw.diags.push(Diag::error("E009", 1, 1, "a workflow needs a `flow`", "ワークフローには `flow` が要ります"));
    }
    if let Some((b, _)) = &prog.on_failure {
        lw.in_on_failure = true;
        let t = lw.block(b);
        lw.m.on_failure = Some(t);
        lw.in_on_failure = false;
    }
    if let Some((b, sp)) = &prog.on_cancel {
        lw.m.on_cancel_line = sp.line;
        lw.in_on_cancel = true;
        let t = lw.block(b);
        lw.m.on_cancel = Some(t);
        lw.in_on_cancel = false;
    }
    lw.recursive_records();
    lw.unread_types();
    lw.same_generated_names();
    lw.made_enums_named_none();
    lw.parallel_scopes();
    lw.m.vars = lw.var_ty.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let ok = !crate::diag::has_errors(&lw.diags);
    (if ok { Some(lw.m) } else { None }, lw.diags)
}

impl<'a> Lowerer<'a> {
    fn push(&mut self, d: Diag) {
        self.diags.push(d);
    }

    fn next_site(&mut self) -> usize {
        self.site += 1;
        self.site
    }

    // -----------------------------------------------------------------------
    // Declarations

    /// Read the descriptions of the APIs (E016 when one cannot be read).
    fn apis(&mut self, base: &Path) {
        for u in &self.prog.apis {
            let (name, sp) = &u.name;
            if self.apis.contains_key(name) {
                self.push(e("E006", *sp, format!("the API `{name}` is used twice"), format!("API `{name}` が二度読み込まれています")));
                continue;
            }
            // `<name>.<type>` would not say which it means
            if self.rule_ix.contains_key(name) {
                self.push(e("E006", *sp, format!("`{name}` names both a rule and an API; give them different names"), format!("`{name}` は規則と API の両方の名前です。別の名前を付けてください")));
                continue;
            }
            self.use_spans.insert(name.clone(), *sp);
            match crate::apis::load(u.kind, &base.join(&u.path)) {
                Ok(doc) => {
                    self.apis.insert(name.clone(), (u.kind, crate::apis::Api { name: name.clone(), doc, url: u.url.clone() }));
                }
                Err(msg) => self.push(e("E016", *sp, format!("could not read the API `{}`", u.path), format!("API `{}` を読めませんでした", u.path)).note(msg.clone(), msg)),
            }
        }
    }

    /// The API a binding names, which must be of `kind`.
    fn api(&mut self, (name, sp): &syntax::Name, kind: crate::apis::ApiKind, what_en: &str, what_ja: &str) -> Option<crate::apis::Api> {
        match self.apis.get(name) {
            Some((k, a)) if *k == kind => Some(a.clone()),
            Some((k, _)) => {
                let k = k.word();
                self.push(e("E016", *sp, format!("`{name}` is described by `use {k}`, and {what_en}"), format!("`{name}` は `use {k}` で読んだものですが、{what_ja}")));
                None
            }
            None => {
                self.push(e("E002", *sp, format!("there is no API `{name}`; name one read by `use {}`", kind.word()), format!("API `{name}` はありません。`use {}` で読んだものを書きます", kind.word())));
                None
            }
        }
    }

    fn rules(&mut self, base: &Path) {
        for u in &self.prog.uses {
            let (name, sp) = &u.name;
            if self.rule_ix.contains_key(name) {
                self.push(e("E006", *sp, format!("the rule `{name}` is used twice"), format!("規則 `{name}` が二度読み込まれています")));
                continue;
            }
            let path = base.join(&u.path);
            let info = match rulec::load(&path) {
                Ok(i) => i,
                Err(msg) => {
                    self.push(
                        e("E005", *sp, format!("could not read the rule `{}`", u.path), format!("規則 `{}` を読めませんでした", u.path))
                            .note(msg.clone(), msg),
                    );
                    continue;
                }
            };
            // a rule that walks a list takes it as an input of elements, which no type of dandori's is, at
            // its service as in the code that goes with the workflow
            if let Some(list) = &info.walks {
                self.push(
                    e("E005", *sp, format!("could not read the rule `{}`", u.path), format!("規則 `{}` を読めませんでした", u.path)).note(
                        format!("the rule walks a list of elements (`{list}`), and dandori does not pass a rule a list yet"),
                        format!("この規則は要素の並び（`{list}`）をたどります。dandori はまだ、規則に並びを渡せません"),
                    ),
                );
                continue;
            }
            self.use_spans.insert(name.clone(), *sp);
            let ix = self.m.rules.len();
            for (en, values) in &info.enums {
                let q = format!("{name}.{en}");
                self.enum_ix.insert(q.clone(), self.m.enums.len());
                self.m.enums.push(EnumDef { name: q, values: values.clone() });
            }
            let fields: Vec<(String, Ty)> = info.outputs.iter().map(|c| (c.name.clone(), self.rty(name, &c.ty))).collect();
            let ranges = info.outputs.iter().filter_map(|c| Some((c.name.clone(), c.range()?))).collect();
            let rec = self.m.records.len();
            self.m.records.push(RecordDef { name: format!("{name}.outputs"), fields, ranges, origin: RecordOrigin::RuleOutputs(ix) });
            self.rule_ix.insert(name.clone(), ix);
            let connect = self.rule_connect(u, &info);
            self.m.rules.push(RuleUse {
                name: name.clone(),
                info,
                lambda: u.lambda.as_ref().map(|(f, _)| f.clone()),
                local: u.local,
                connect,
                connection: u.connection.as_ref().map(|(c, _)| c.clone()),
                outputs: rec,
                line: sp.line,
            });
        }
    }

    /// What `connect` under `use rule` says: the service's URL, and what `rulec api` says of its
    /// request and its response. The ways of calling that do not go together are E007, each at the
    /// second to say it.
    fn rule_connect(&mut self, u: &syntax::UseRule, info: &rulec::RuleInfo) -> Option<RuleConnect> {
        if let (Some((_, lsp)), Some((_, csp))) = (&u.lambda, &u.connect) {
            let (first, second) = if (lsp.line, lsp.col) < (csp.line, csp.col) { (lsp, csp) } else { (csp, lsp) };
            self.push(e(
                "E007",
                *second,
                format!("the rule is already called another way (line {}); a rule is called by `lambda` or by `connect`", first.line),
                format!("この規則の呼び出し方はもう書かれています（{} 行目）。規則の呼び出し方は `lambda` か `connect` のどちらか一つです", first.line),
            ));
        }
        if let (Some((_, nsp)), None) = (&u.connection, &u.connect) {
            self.push(e(
                "E007",
                *nsp,
                "`connection` is for a rule called by `connect`; Step Functions invokes a rule's `lambda` without one",
                "`connection` は `connect` で呼ぶ規則に書きます。`lambda` の規則は、Step Functions が接続なしで呼びます",
            ));
        }
        let (url, csp) = u.connect.as_ref()?;
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            self.push(e(
                "E007",
                *csp,
                "`connect` says where the rule's service is, with http:// or https://",
                "`connect` には、規則のサービスの場所を http:// か https:// で書きます",
            ));
            return None;
        }
        match rulec::connect_shape(&info.api) {
            Ok(shape) => {
                let zeros = rule_zeros(&shape.response);
                let base = url.trim_end_matches('/').to_string();
                Some(RuleConnect { url: format!("{base}{}", shape.path), base, request: shape.request, response: shape.response, zeros })
            }
            Err((en, ja)) => {
                // the rule is read, but its service is not: what `connect` calls cannot be said
                self.push(e("E005", u.name.1, format!("could not read the rule `{}`", u.path), format!("規則 `{}` を読めませんでした", u.path)).note(en, ja));
                None
            }
        }
    }

    fn rty(&self, rule: &str, t: &RType) -> Ty {
        match t {
            RType::Bool => Ty::Bool,
            RType::Str => Ty::Str,
            RType::Enum(n) => Ty::Enum(*self.enum_ix.get(&format!("{rule}.{n}")).expect("rule enums are registered first")),
            // rulec's `number` has no unit: it is an integer like dandori's `int`
            RType::Num { unit, .. } if unit == "number" => Ty::Int,
            RType::Num { unit, .. } => Ty::Num(unit.clone()),
        }
    }

    fn local_types(&mut self) {
        for en in &self.prog.enums {
            let (name, sp) = &en.name;
            if self.enum_ix.contains_key(name) || self.prog.records.iter().any(|r| r.name.0 == *name) {
                self.push(e("E006", *sp, format!("`{name}` is declared twice"), format!("`{name}` が二度宣言されています")));
                continue;
            }
            let mut values: Vec<String> = Vec::new();
            for (v, vsp) in &en.values {
                if v == "none" {
                    self.push(e("E006", *vsp, "`none` is kept for a case that has not started; name the value otherwise", "`none` は始まっていない案件を表す語なので、値には別の名前を付けてください"));
                } else if values.contains(v) {
                    self.push(e("E006", *vsp, format!("the value `{v}` is written twice"), format!("値 `{v}` が二度書かれています")));
                } else {
                    values.push(v.clone());
                }
            }
            self.enum_ix.insert(name.clone(), self.m.enums.len());
            self.m.enums.push(EnumDef { name: name.clone(), values });
        }
        // records: names first, so that a field can have a record type declared further down
        for r in &self.prog.records {
            let (name, sp) = &r.name;
            if self.record_ix.contains_key(name) {
                self.push(e("E006", *sp, format!("`{name}` is declared twice"), format!("`{name}` が二度宣言されています")));
                continue;
            }
            self.record_ix.insert(name.clone(), self.m.records.len());
            self.m.records.push(RecordDef { name: name.clone(), fields: vec![], ranges: BTreeMap::new(), origin: RecordOrigin::Local });
        }
        for r in &self.prog.records {
            let ix = match self.record_ix.get(&r.name.0) {
                Some(i) => *i,
                None => continue,
            };
            let mut fields = Vec::new();
            let mut ranges = BTreeMap::new();
            for f in &r.fields {
                if fields.iter().any(|(n, _): &(String, Ty)| *n == f.name.0) {
                    self.push(e("E006", f.name.1, format!("the field `{}` is written twice", f.name.0), format!("フィールド `{}` が二度書かれています", f.name.0)));
                    continue;
                }
                if let Some(t) = self.ty(&f.ty) {
                    if let Some(rg) = self.range(&t, f.range.as_ref()) {
                        ranges.insert(f.name.0.clone(), rg);
                    }
                    fields.push((f.name.0.clone(), t));
                }
            }
            self.m.records[ix].fields = fields;
            self.m.records[ix].ranges = ranges;
        }
    }

    fn ty(&mut self, te: &TypeExpr) -> Option<Ty> {
        match te {
            TypeExpr::Int(_) => Some(Ty::Int),
            TypeExpr::Str(_) => Some(Ty::Str),
            TypeExpr::Bool(_) => Some(Ty::Bool),
            TypeExpr::Timestamp(_) => Some(Ty::Timestamp),
            TypeExpr::Json(_) => Some(Ty::Json),
            TypeExpr::List(inner, sp) => {
                let t = self.ty(inner)?;
                if matches!(t.inner(), Ty::List(_)) {
                    self.push(e("E003", *sp, "a list of lists is not supported; put the inner list in a record", "リストのリストは書けません。内側のリストはレコードに入れてください"));
                    return None;
                }
                Some(Ty::List(Box::new(t)))
            }
            TypeExpr::Opt(inner, _) => Some(Ty::Opt(Box::new(self.ty(inner)?))),
            TypeExpr::Unit(u, sp) => {
                let kind = u.split('[').next().unwrap_or("");
                if !syntax::UNIT_KINDS.contains(&kind) {
                    self.push(e("E002", *sp, format!("there is no type `{u}`"), format!("型 `{u}` はありません")).note(TYPE_HINT_EN, TYPE_HINT_JA));
                    return None;
                }
                // a rate by how many of its steps make the whole, as the rules' rates are spelled
                let unit = rulec::normalize_unit(u);
                Some(Ty::Num(rate_per(&unit).map(rate_unit).unwrap_or(unit)))
            }
            TypeExpr::Named(parts) => {
                let text = parts.iter().map(|p| p.0.as_str()).collect::<Vec<_>>().join(".");
                if let Some(i) = self.enum_ix.get(&text) {
                    return Some(Ty::Enum(*i));
                }
                if let Some(i) = self.record_ix.get(&text) {
                    return Some(Ty::Record(*i));
                }
                let sp = parts[0].1;
                // `<API>.<name>`: a message or an enum of the `.proto` that `use proto` read
                if parts.len() >= 2 {
                    if let Some((kind, _)) = self.apis.get(&parts[0].0) {
                        let (kind, api) = (*kind, parts[0].0.clone());
                        return match kind {
                            ApiKind::Proto => self.made_type(&api, &parts[1..], &text, sp),
                            _ => {
                                let (what_en, what_ja) = match kind {
                                    ApiKind::OpenApi => ("an OpenAPI document", "OpenAPI の記述"),
                                    _ => ("a Smithy model", "Smithy のモデル"),
                                };
                                let (shapes_en, shapes_ja) = match kind {
                                    ApiKind::OpenApi => ("schemas", "スキーマ"),
                                    _ => ("shapes", "shape"),
                                };
                                self.push(e("E002", sp, format!("there is no type `{text}`"), format!("型 `{text}` はありません")).note(
                                    format!("types are made from a `.proto` read by `use proto`; `{api}` is {what_en}, and its {shapes_en} are not made into types"),
                                    format!("型を作れるのは、`use proto` で読んだ `.proto` からです。`{api}` は {what_ja}で、その{shapes_ja}からは型を作りません"),
                                ));
                                None
                            }
                        };
                    }
                }
                let hint_en;
                let hint_ja;
                if parts.len() == 2 && self.rule_ix.contains_key(&parts[0].0) {
                    let r = &self.m.rules[self.rule_ix[&parts[0].0]];
                    let names: Vec<String> = r.info.enums.iter().map(|(n, _)| n.clone()).collect();
                    hint_en = format!("the enums of `{}` are {}", parts[0].0, names.join(", "));
                    hint_ja = format!("`{}` の列挙は {} です", parts[0].0, names.join("・"));
                } else {
                    hint_en = TYPE_HINT_EN.to_string();
                    hint_ja = TYPE_HINT_JA.to_string();
                }
                self.push(e("E002", sp, format!("there is no type `{text}`"), format!("型 `{text}` はありません")).note(hint_en, hint_ja));
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // Types made from a `.proto` (DESIGN 1.12)

    /// What to say of the files a `.proto` imports and no file was read for, when the API is one.
    fn unread_note_of(api: &crate::apis::Api) -> Option<(String, String)> {
        match &api.doc {
            ApiDoc::Proto(pf) => crate::apis::unread_note(pf),
            _ => None,
        }
    }

    /// The `.proto` the API `api` read, which is one.
    fn proto_of(&self, api: &str) -> std::sync::Arc<ProtoFile> {
        match self.apis.get(api) {
            Some((_, crate::apis::Api { doc: ApiDoc::Proto(pf), .. })) => pf.clone(),
            _ => unreachable!("an API read by `use proto`"),
        }
    }

    /// The type a flow names `<api>.<rel>`: a message or an enum of the `.proto` the API read,
    /// named from its package, or from the root (`common.v1.Money`) when it is of another
    /// package. E002 when the `.proto` has no such name, with the names it has.
    fn made_type(&mut self, api: &str, rel: &[syntax::Name], text: &str, sp: Span) -> Option<Ty> {
        let pf = self.proto_of(api);
        let rel_text = rel.iter().map(|p| p.0.as_str()).collect::<Vec<_>>().join(".");
        let candidates: Vec<String> = if pf.package.is_empty() { vec![rel_text.clone()] } else { vec![format!("{}.{rel_text}", pf.package), rel_text.clone()] };
        let named = |c: &String| pf.messages.contains_key(c) || pf.enums.contains_key(c) || crate::proto::WELL_KNOWN.contains(&c.as_str());
        let Some(full) = candidates.iter().find(|c| named(c)).cloned() else {
            if pf.services.iter().any(|sv| candidates.contains(&sv.name)) {
                self.push(e(
                    "E002",
                    sp,
                    format!("`{text}` is a service of `{api}`, not a message or an enum"),
                    format!("`{text}` は `{api}` のサービスで、メッセージでも列挙でもありません"),
                ));
                return None;
            }
            let own = format!("{}.", pf.package);
            let mut names: Vec<String> = pf.messages.keys().chain(pf.enums.keys()).map(|f| if pf.package.is_empty() { f.clone() } else { f.strip_prefix(&own).unwrap_or(f).to_string() }).collect();
            names.sort();
            names.dedup();
            let more = names.len().saturating_sub(20);
            names.truncate(20);
            let (mut en, mut ja) = (names.join(", "), names.join("・"));
            if more > 0 {
                en.push_str(&format!(", and {more} more"));
                ja.push_str(&format!("…ほか {more} 個"));
            }
            let mut d = e("E002", sp, format!("there is no type `{text}`"), format!("型 `{text}` はありません")).note(format!("the messages and enums of `{api}` are {en}"), format!("`{api}` のメッセージと列挙は {ja} です"));
            // a file it may be in was not read
            if let Some((nen, nja)) = crate::apis::unread_note(&pf) {
                d = d.note(nen, nja);
            }
            self.push(d);
            return None;
        };
        let made = crate::apis::made_named(&pf, &full);
        Some(self.made_ty(api, &pf, &made, sp))
    }

    /// The type of what a `.proto` says, with the messages and enums it names made along the way.
    fn made_ty(&mut self, api: &str, pf: &ProtoFile, t: &MadeTy, sp: Span) -> Ty {
        match t {
            MadeTy::Int(_) => Ty::Int,
            MadeTy::Str => Ty::Str,
            MadeTy::Bool => Ty::Bool,
            MadeTy::Timestamp => Ty::Timestamp,
            MadeTy::Json => Ty::Json,
            MadeTy::Message(full) => Ty::Record(self.made_record(api, pf, full, sp)),
            MadeTy::Enum(full) => Ty::Enum(self.made_enum(api, pf, full, sp)),
            MadeTy::List(x) => Ty::List(Box::new(self.made_ty(api, pf, x, sp))),
            MadeTy::Opt(x) => Ty::Opt(Box::new(self.made_ty(api, pf, x, sp))),
            // said when the lowering is done (`unread_types`); `json` takes the place of what is not known
            MadeTy::Unread(_) => Ty::Json,
        }
    }

    /// How a flow names a type of the `.proto`: `warehouse.ReserveResponse` for one of the file's
    /// own package, and from the root (`warehouse.common.v1.Money`) for one of another.
    fn made_name(api: &str, pf: &ProtoFile, full: &str) -> String {
        match full.strip_prefix(&format!("{}.", pf.package)) {
            Some(rest) if !pf.package.is_empty() => format!("{api}.{rest}"),
            _ => format!("{api}.{full}"),
        }
    }

    fn made_enum(&mut self, api: &str, pf: &ProtoFile, full: &str, sp: Span) -> EnumId {
        let name = Self::made_name(api, pf, full);
        if let Some(i) = self.enum_ix.get(&name) {
            return *i;
        }
        let values = crate::apis::made_enum_values(pf, full);
        let ix = self.m.enums.len();
        self.enum_ix.insert(name.clone(), ix);
        self.made_enums.insert(ix);
        self.made_at.entry(name.clone()).or_insert(sp);
        self.m.enums.push(EnumDef { name, values });
        ix
    }

    /// E006 for a made enum with a value `none`, which a `match` would read as the arm of an absent
    /// value. It is said when the lowering is done, at the place the enum was first named: what
    /// is lowered only to find a variable's type says nothing of what it finds.
    fn made_enums_named_none(&mut self) {
        let named: Vec<(String, Span)> = self
            .made_enums
            .iter()
            .filter(|ix| self.m.enums[**ix].values.iter().any(|v| v == "none"))
            .map(|ix| (self.m.enums[*ix].name.clone(), self.made_at.get(&self.m.enums[*ix].name).copied().unwrap_or(Span { line: 1, col: 1 })))
            .collect();
        for (name, sp) in named {
            self.push(e(
                "E006",
                sp,
                format!("the value `none` of `{name}` would be taken for an absent value in a `match`; write an enum of your own for it"),
                format!("`{name}` の値 `none` は、`match` では値が無いことを表す語として読まれます。代わりに自分で列挙を書いてください"),
            ));
        }
    }

    fn made_record(&mut self, api: &str, pf: &ProtoFile, full: &str, sp: Span) -> RecordId {
        let name = Self::made_name(api, pf, full);
        if let Some(i) = self.record_ix.get(&name) {
            return *i;
        }
        // named before its fields are made, so that a message that holds itself stops here
        let ix = self.m.records.len();
        self.record_ix.insert(name.clone(), ix);
        self.made_at.entry(name.clone()).or_insert(sp);
        self.m.records.push(RecordDef { name, fields: vec![], ranges: BTreeMap::new(), origin: RecordOrigin::Proto { api: api.to_string() } });
        let mut fields = Vec::new();
        let mut ranges = BTreeMap::new();
        for f in crate::apis::made_fields(pf, full) {
            fn range_of(t: &MadeTy) -> Option<Range> {
                match t {
                    MadeTy::Int(r) => *r,
                    MadeTy::List(x) | MadeTy::Opt(x) => range_of(x),
                    _ => None,
                }
            }
            fn unread_in(t: &MadeTy) -> Option<&String> {
                match t {
                    MadeTy::Unread(n) => Some(n),
                    MadeTy::List(x) | MadeTy::Opt(x) => unread_in(x),
                    _ => None,
                }
            }
            if let Some(u) = unread_in(&f.ty) {
                let record = Self::made_name(api, pf, full);
                self.made_unread.push((record, f.name.clone(), u.clone()));
            }
            let t = self.made_ty(api, pf, &f.ty, sp);
            if let Some(r) = range_of(&f.ty) {
                ranges.insert(f.name.clone(), r);
            }
            fields.push((f.name, t));
        }
        self.m.records[ix].fields = fields;
        self.m.records[ix].ranges = ranges;
        ix
    }

    /// E002 for a made record with a field whose type no file read has, which an import that was not
    /// read may hold: the record cannot be made from what is known of it. Said when the lowering is
    /// done, at the place the record was first named, since what is lowered only to find a
    /// variable's type says nothing of what it finds. The way out is a record written by hand, with
    /// `json` for the field.
    fn unread_types(&mut self) {
        let found = std::mem::take(&mut self.made_unread);
        for (record, field, ty) in found {
            let at = self.made_at.get(&record).copied().unwrap_or(Span { line: 1, col: 1 });
            let mut d = e(
                "E002",
                at,
                format!("`{record}` cannot be made: its field `{field}` is of the type `{ty}`, which is not known; write the record yourself, with `json` for `{field}`"),
                format!("`{record}` を作れません。フィールド `{field}` の型 `{ty}` が分かりません。レコードを自分で書き、`{field}` は `json` にしてください"),
            );
            // the `.proto` the record is of, by the API it was made through
            if let Some((nen, nja)) = record.split('.').next().and_then(|api| self.apis.get(api)).and_then(|(_, a)| match &a.doc {
                ApiDoc::Proto(pf) => crate::apis::unread_note(pf),
                _ => None,
            }) {
                d = d.note(nen, nja);
            }
            self.push(d);
        }
    }

    /// E003 for a record that holds itself, through a field, a list, a value that may be absent, or
    /// other records (a tree, or two that hold each other): one a flow declares and one made from a
    /// message of a `.proto` alike. What a value of it is stays finite, but the scenarios and some
    /// of the generators go on for ever on a type that holds itself, and Step Functions has no
    /// function to check one with. The way out is `json` where it holds itself, in a record written
    /// by hand.
    fn recursive_records(&mut self) {
        fn records_in(t: &Ty, out: &mut Vec<RecordId>) {
            match t {
                Ty::Record(r) => out.push(*r),
                Ty::List(x) | Ty::Opt(x) => records_in(x, out),
                _ => {}
            }
        }
        let n = self.m.records.len();
        // each record's fields, with the records they name
        let edges: Vec<Vec<(String, RecordId)>> = self
            .m
            .records
            .iter()
            .map(|r| {
                r.fields
                    .iter()
                    .flat_map(|(f, t)| {
                        let mut named = Vec::new();
                        records_in(t, &mut named);
                        named.into_iter().map(move |q| (f.clone(), q))
                    })
                    .collect()
            })
            .collect();
        let reach = |from: RecordId| -> BTreeSet<RecordId> {
            let mut seen = BTreeSet::new();
            let mut todo = vec![from];
            while let Some(r) = todo.pop() {
                for (_, q) in &edges[r] {
                    if seen.insert(*q) {
                        todo.push(*q);
                    }
                }
            }
            seen
        };
        let reaches: Vec<BTreeSet<RecordId>> = (0..n).map(reach).collect();
        let mut said: BTreeSet<RecordId> = BTreeSet::new();
        for r in 0..n {
            // what a rule answers holds no record
            if matches!(self.m.records[r].origin, RecordOrigin::RuleOutputs(_)) || !reaches[r].contains(&r) || said.contains(&r) {
                continue;
            }
            // the records that hold each other with this one are said once
            for q in 0..n {
                if q == r || (reaches[r].contains(&q) && reaches[q].contains(&r)) {
                    said.insert(q);
                }
            }
            let name = self.m.records[r].name.clone();
            let through = edges[r].iter().find(|(_, q)| *q == r || reaches[*q].contains(&r)).map(|(f, _)| f.clone()).unwrap_or_default();
            if let RecordOrigin::Proto { .. } = self.m.records[r].origin {
                let at = self.made_at.get(&name).copied().unwrap_or(Span { line: 1, col: 1 });
                self.push(e(
                    "E003",
                    at,
                    format!("`{name}` contains itself, through `{through}`; a record cannot, so write the record yourself, with `json` where it holds itself"),
                    format!("`{name}` は、フィールド `{through}` を通して自分自身を含んでいます。レコードは自分自身を含められないので、自分を含むところを `json` にしたレコードを自分で書いてください"),
                ));
            } else {
                // the type of the field, where the flow wrote it
                let at = self.prog.records.iter().find(|d| d.name.0 == name).and_then(|d| d.fields.iter().find(|f| f.name.0 == through).map(|f| f.ty.span()).or(Some(d.name.1))).unwrap_or(Span { line: 1, col: 1 });
                self.push(e(
                    "E003",
                    at,
                    format!("`{name}` contains itself, through `{through}`; a record cannot, so give the field that holds itself the type `json`"),
                    format!("`{name}` は、フィールド `{through}` を通して自分自身を含んでいます。レコードは自分自身を含められないので、自分を含むフィールドの型は `json` にしてください"),
                ));
            }
        }
    }

    /// E006 for two types that have one name in the code dandori writes: a type's name there is
    /// its name here with the dots turned to underscores, so `warehouse.Stock` and a `record
    /// warehouse_Stock` are one, in TypeScript and in Python alike.
    fn same_generated_names(&mut self) {
        let mut seen: BTreeMap<(bool, String), String> = BTreeMap::new();
        let mut clashes: Vec<(String, String, String)> = Vec::new();
        let names: Vec<String> = self.m.enums.iter().map(|x| x.name.clone()).chain(self.m.records.iter().map(|x| x.name.clone())).collect();
        for n in names {
            for (python, generated) in [(false, crate::temporal::type_name(&n)), (true, crate::temporal_py::type_name(&n))] {
                match seen.get(&(python, generated.clone())) {
                    Some(first) if *first != n => {
                        if !clashes.iter().any(|(a, b, _)| *a == *first && *b == n) {
                            clashes.push((first.clone(), n.clone(), generated));
                        }
                    }
                    Some(_) => {}
                    None => {
                        seen.insert((python, generated), n.clone());
                    }
                }
            }
        }
        for (a, b, generated) in clashes {
            // the one to rename is the one this file declares, else the one named later
            let declared = |x: &str| self.prog.enums.iter().map(|d| &d.name).chain(self.prog.records.iter().map(|d| &d.name)).find(|(n, _)| n == x).map(|(_, sp)| *sp);
            let (rename, at) = match (declared(&a), declared(&b)) {
                (_, Some(sp)) => (b.clone(), sp),
                (Some(sp), None) => (a.clone(), sp),
                (None, None) => {
                    let sp = self.made_at.get(&b).or_else(|| self.made_at.get(&a)).or_else(|| self.use_spans.get(b.split('.').next().unwrap_or(&b))).copied().unwrap_or(Span { line: 1, col: 1 });
                    (b.clone(), sp)
                }
            };
            self.push(e(
                "E006",
                at,
                format!("`{a}` and `{b}` would have one name in the code dandori writes, `{generated}`; rename `{rename}`"),
                format!("`{a}` と `{b}` は、生成するコードで同じ名前（`{generated}`）になります。`{rename}` の名前を変えてください"),
            ));
        }
    }

    /// The `.flow` a task runs as its child, checked on its own (E015 when it cannot be).
    fn child_flow(&mut self, path: &str, sp: Span) -> Option<Box<ChildFlow>> {
        use crate::check::ChildTrouble;
        let (en, ja, note) = match crate::check::child(&self.dir.join(path)) {
            Ok(model) => return Some(Box::new(ChildFlow { path: path.to_string(), model })),
            Err(ChildTrouble::Cycle(chain)) => {
                let names: Vec<String> = chain.iter().map(|p| p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()).collect();
                (
                    format!("`{path}` runs this flow again ({}); a flow cannot run itself, directly or through others", names.join(" → ")),
                    format!("`{path}` はこのフローをまた走らせます（{}）。フローは自分を、直接にもほかのフローを通しても走らせられません", names.join(" → ")),
                    None,
                )
            }
            Err(ChildTrouble::Unreadable(msg)) => (format!("could not read `{path}`"), format!("`{path}` を読めませんでした"), Some((msg.clone(), msg))),
            Err(ChildTrouble::Refused(diags)) => (
                format!("`{path}` does not pass check"),
                format!("`{path}` は検査を通りません"),
                diags.first().map(|d| (format!("line {}: {} ({})", d.line, d.en, d.code), format!("{} 行目：{}（{}）", d.line, d.ja, d.code))),
            ),
        };
        let mut d = e("E015", sp, en, ja);
        if let Some((nen, nja)) = note {
            d = d.note(nen, nja);
        }
        self.push(d);
        None
    }

    /// The range written after a type, which only a number can have (or a `?` or a list of numbers).
    fn range(&mut self, t: &Ty, r: Option<&RangeDecl>) -> Option<Range> {
        let r = r?;
        let mut inner = t;
        while let Ty::Opt(x) | Ty::List(x) = inner {
            inner = x;
        }
        if !matches!(inner, Ty::Int | Ty::Num(_)) {
            let n = self.m.ty_name(t);
            self.push(e("E003", r.span, format!("a range is for a number (`int` or a unit), and this is `{n}`"), format!("範囲を書けるのは数（`int` か単位の付いた数）で、これは `{n}` です")));
            return None;
        }
        let rg = Range { lo: r.lo, hi: r.hi };
        if let (Some(lo), Some(hi)) = (rg.lo, rg.hi) {
            if lo > hi {
                self.push(e("E003", r.span, format!("no number is in `{}`", rg.show()), format!("`{}` に入る数はありません", rg.show())));
                return None;
            }
        }
        Some(rg)
    }

    fn io(&mut self) {
        let mut seen: Vec<String> = vec![];
        for f in &self.prog.inputs {
            if seen.contains(&f.name.0) {
                self.push(e("E006", f.name.1, format!("the input `{}` is written twice", f.name.0), format!("入力 `{}` が二度書かれています", f.name.0)));
                continue;
            }
            seen.push(f.name.0.clone());
            if let Some(t) = self.ty(&f.ty) {
                if let Some(rg) = self.range(&t, f.range.as_ref()) {
                    self.m.input_ranges.insert(f.name.0.clone(), rg);
                }
                self.m.inputs.push((f.name.0.clone(), t));
            }
        }
        let mut seen: Vec<String> = vec![];
        for f in &self.prog.outputs {
            if seen.contains(&f.name.0) {
                self.push(e("E006", f.name.1, format!("the output `{}` is written twice", f.name.0), format!("出力 `{}` が二度書かれています", f.name.0)));
                continue;
            }
            seen.push(f.name.0.clone());
            if let Some(t) = self.ty(&f.ty) {
                if let Some(rg) = self.range(&t, f.range.as_ref()) {
                    self.m.output_ranges.insert(f.name.0.clone(), rg);
                }
                self.m.outputs.push((f.name.0.clone(), t));
            }
        }
    }

    fn tasks(&mut self) {
        for t in &self.prog.tasks {
            let (name, sp) = &t.name;
            if self.task_ix.contains_key(name) || self.rule_ix.contains_key(name) {
                self.push(e("E006", *sp, format!("`{name}` is declared twice (tasks and rules share names)"), format!("`{name}` が二度宣言されています（タスクと規則は名前を共有します）")));
                continue;
            }
            let mut params = Vec::new();
            let mut param_ranges = BTreeMap::new();
            for p in &t.params {
                if params.iter().any(|(n, _): &(String, Ty)| *n == p.name.0) {
                    self.push(e("E006", p.name.1, format!("the parameter `{}` is written twice", p.name.0), format!("引数 `{}` が二度書かれています", p.name.0)));
                    continue;
                }
                if let Some(ty) = self.ty(&p.ty) {
                    if let Some(rg) = self.range(&ty, p.range.as_ref()) {
                        param_ranges.insert(p.name.0.clone(), rg);
                    }
                    params.push((p.name.0.clone(), ty));
                }
            }
            let result = match &t.result {
                Some(te) => match self.ty(te) {
                    Some(r) => Some(r),
                    None => continue,
                },
                None => None,
            };
            let result_range = result.as_ref().and_then(|r| self.range(r, t.result_range.as_ref()));
            // an operation of an API's description: its URL and body come from there
            let mut described: Option<(crate::apis::Api, syntax::Binding)> = None;
            let mut connect = None;
            let mut spec_url = None;
            match &t.binding {
                Some((b @ syntax::Binding::Http { method, url, form, api: Some(a) }, bsp)) => {
                    if let Some(api) = self.api(a, crate::apis::ApiKind::OpenApi, "`http` calls an operation of an OpenAPI document (`use openapi`)", "`http` で呼べるのは OpenAPI の記述（`use openapi`）の操作です") {
                        match crate::apis::openapi_op(&api, method, url) {
                            Ok(op) => {
                                if *form {
                                    self.push(e("E007", *bsp, "the OpenAPI document says how the body is written; leave out `form`", "本文の書き方は OpenAPI の記述が言うので、`form` は外してください"));
                                }
                                match api.base_url() {
                                    Some(base) => spec_url = Some((format!("{base}{url}"), op.form)),
                                    None => self.push(e("E016", *bsp, format!("`{}` names no server; write `url \"<where it is>\"` under `use openapi {}`", a.0, a.0), format!("`{}` はサーバーを言いません。`use openapi {}` の下に `url \"<場所>\"` を書いてください", a.0, a.0))),
                                }
                                described = Some((api.clone(), b.clone()));
                            }
                            Err((en, ja)) => self.push(e("E016", *bsp, en, ja)),
                        }
                    }
                }
                Some((b @ syntax::Binding::Connect { api: a, method }, bsp)) => {
                    if let Some(api) = self.api(a, crate::apis::ApiKind::Proto, "`connect` calls a method of a `.proto` (`use proto`)", "`connect` で呼べるのは `.proto`（`use proto`）のメソッドです") {
                        // a `.proto` does not say where its service is; a flow that only takes its types needs no `url`
                        if api.url.is_none() && self.url_said.insert(a.0.clone()) {
                            let at = self.use_spans.get(&a.0).copied().unwrap_or(a.1);
                            let name = &a.0;
                            self.push(e(
                                "E016",
                                at,
                                format!("a `.proto` does not say where the service is; write `url \"<where it is>\"` under `use proto {name}`"),
                                format!("`.proto` はサービスの場所を言いません。`use proto {name}` の下に `url \"<場所>\"` を書いてください"),
                            ));
                        }
                        match crate::apis::proto_op(&api, method) {
                            Ok(op) => {
                                connect = op.zeros();
                                if let (Some(base), Some(proc_)) = (api.url.as_deref(), &op.procedure) {
                                    spec_url = Some((format!("{}/{proc_}", base.trim_end_matches('/')), false));
                                }
                                described = Some((api.clone(), b.clone()));
                            }
                            Err((en, ja)) => {
                                // the service may be in a file that was not read
                                let mut d = e("E016", *bsp, en, ja);
                                if let Some((nen, nja)) = Self::unread_note_of(&api) {
                                    d = d.note(nen, nja);
                                }
                                self.push(d);
                            }
                        }
                    }
                }
                Some((b @ syntax::Binding::Aws { service, .. }, _)) => {
                    if let Some((crate::apis::ApiKind::Smithy, api)) = self.apis.get(service) {
                        described = Some((api.clone(), b.clone()));
                    }
                }
                _ => {}
            }
            // what a Jev task asks, from the type of its answer
            let jev = match &t.binding {
                Some((syntax::Binding::Jev(jd), bsp)) => Some(self.jev(t, jd, *bsp, result.as_ref())),
                _ => None,
            };
            if let (Some((_, _, csp)), None) = (&t.confidence, &jev) {
                self.push(e("E007", *csp, "`confidence` says how sure Jev must be of its answer; this task has no `jev`", "`confidence` は Jev の答えにどれだけの確信が要るかを書くところです。このタスクには `jev` がありません"));
            }
            let binding = t.binding.as_ref().map(|(b, _)| match b {
                syntax::Binding::Lambda(f) => Binding::Lambda(f.clone()),
                syntax::Binding::Http { method, url, form, api: None } => Binding::Http { method: method.clone(), url: url.clone(), form: *form },
                syntax::Binding::Http { method, url, api: Some(_), .. } => {
                    let (url, form) = spec_url.clone().unwrap_or((url.clone(), false));
                    Binding::Http { method: method.clone(), url, form }
                }
                syntax::Binding::Connect { .. } => Binding::Http { method: "POST".into(), url: spec_url.clone().map(|(u, _)| u).unwrap_or_default(), form: false },
                syntax::Binding::Aws { service, action } => Binding::Aws { service: service.clone(), action: action.clone() },
                syntax::Binding::Agent { provider, instructions } => Binding::Agent {
                    // an unknown provider is refused in `agent` below
                    provider: match provider.as_ref().map(|x| x.0.as_str()) {
                        Some("claude") => Provider::Claude,
                        _ => Provider::OpenAi,
                    },
                    instructions: instructions.clone(),
                    model: t.model.as_ref().map(|x| x.0.clone()).unwrap_or_default(),
                    url: t.url.as_ref().map(|x| x.0.clone()),
                    effort: t.effort.as_ref().map(|x| x.0.clone()),
                },
                syntax::Binding::Jev(_) => Binding::Jev(jev.clone().expect("lowered above")),
            });
            self.agent(t, result.as_ref(), result_range);
            // another `.flow` as the child: checked here, for its names and the contract it holds the task to
            let flow = t.flow.as_ref().and_then(|(path, fsp)| self.child_flow(path, *fsp));
            if t.flow.is_some() {
                if let Some((_, bsp)) = &t.binding {
                    self.push(e("E007", *bsp, "a task that runs another `.flow` calls nothing else; leave out `lambda`, `http`, `aws`, `agent` and `jev`", "ほかの `.flow` を走らせるタスクは、ほかに何も呼びません。`lambda`・`http`・`aws`・`agent`・`jev` は外してください"));
                }
                if let Some((_, isp)) = &t.image {
                    self.push(e("E007", *isp, "on Argo, a task that runs another `.flow` makes a workflow of the child's WorkflowTemplate; leave out `image`", "Argo では、ほかの `.flow` を走らせるタスクは、子の WorkflowTemplate からワークフローを作ります。`image` は外してください"));
                }
            }
            let child = t.flow.as_ref().or(t.workflow.as_ref()).or(t.state_machine.as_ref()).or(t.durable_function.as_ref()).or(t.argo_template.as_ref()).map(|(_, s)| *s);
            if let Some((syntax::Binding::Aws { service, .. }, bsp)) = &t.binding {
                if crate::aws::exception_prefix(service).is_none() {
                    self.push(e(
                        "E007",
                        *bsp,
                        format!("Step Functions has no AWS SDK integration for the service `{service}`; write it as the Task's Resource names it, such as `dynamodb` or `secretsmanager`"),
                        format!("Step Functions の AWS SDK 統合に、サービス `{service}` はありません。`dynamodb` や `secretsmanager` のように、Task の Resource での名前で書いてください"),
                    ));
                }
            }
            let mut errors: Vec<ErrDef> = Vec::new();
            for er in &t.errors {
                let (en, esp) = &er.name;
                if en == "timeout" || en == "failure" {
                    self.push(e("E007", *esp, format!("`{en}` is always there; declare only the task's own errors"), format!("`{en}` は宣言しなくても使えます。タスク自身のエラーだけを宣言します")));
                    continue;
                }
                if errors.iter().any(|x| x.name == *en) {
                    self.push(e("E006", *esp, format!("the error `{en}` is written twice"), format!("エラー `{en}` が二度書かれています")));
                    continue;
                }
                if matches!(binding, Some(Binding::Agent { .. })) {
                    self.push(e(
                        "E007",
                        *esp,
                        "an agent declares no errors of its own: when the model refuses or the call fails it is `failure`, and past its time `timeout`",
                        "エージェントのタスクにはエラーを宣言できません。モデルが応答を拒否したときや呼び出しが失敗したときは `failure`、時間を過ぎたときは `timeout` になります",
                    ));
                    continue;
                }
                // a Connect error is named by its code, and comes back with the code's HTTP status
                let (status, exception) = if matches!(t.binding, Some((syntax::Binding::Connect { .. }, _))) {
                    match (er.status, er.exception.as_deref().map(|c| (c, crate::apis::connect_status(c)))) {
                        (None, Some((code, Some(st)))) => (Some(st), Some(code.to_string())),
                        (None, Some((code, None))) => {
                            self.push(e(
                                "E007",
                                *esp,
                                format!("`{code}` is not a Connect error code; the codes are canceled, unknown, invalid_argument, deadline_exceeded, not_found, already_exists, permission_denied, resource_exhausted, failed_precondition, aborted, out_of_range, unimplemented, internal, unavailable, data_loss and unauthenticated"),
                                format!("`{code}` は Connect のエラーコードではありません。コードは canceled・unknown・invalid_argument・deadline_exceeded・not_found・already_exists・permission_denied・resource_exhausted・failed_precondition・aborted・out_of_range・unimplemented・internal・unavailable・data_loss・unauthenticated です"),
                            ));
                            continue;
                        }
                        _ => {
                            self.push(e("E007", *esp, format!("a Connect error is named by its code, as `{en} = not_found`"), format!("Connect のエラーは、`{en} = not_found` のようにコードで書きます")));
                            continue;
                        }
                    }
                } else {
                    (er.status, er.exception.clone())
                };
                match (&binding, status, &exception) {
                    (Some(Binding::Http { .. }), None, _) => {
                        self.push(e("E007", *esp, format!("give `{en}` the HTTP status it comes back with, as `{en} = 402`"), format!("`{en}` が返ってくるときの HTTP ステータスを `{en} = 402` のように書きます")));
                    }
                    // TypeSafe's API says an error by its status: 429 for the rate limit, 529 when it is overloaded
                    (Some(Binding::Jev(_)), None, _) => {
                        self.push(e(
                            "E007",
                            *esp,
                            format!("Jev's API says an error by its HTTP status; give `{en}` the one it comes back with, as `{en} = 429` (the rate limit) or `{en} = 529` (overloaded)"),
                            format!("Jev の API はエラーを HTTP ステータスで伝えます。`{en}` が返ってくるときのステータスを、`{en} = 429`（レート制限）や `{en} = 529`（過負荷）のように書きます"),
                        ));
                    }
                    (Some(Binding::Jev(_)), Some(_), _) => {}
                    (Some(Binding::Http { .. }), Some(_), None) => {}
                    (Some(Binding::Aws { .. }), None, _) => {}
                    (Some(Binding::Aws { .. }), Some(_), _) => {
                        self.push(e("E007", *esp, format!("an AWS API's error is named by its exception, as `{en} = ConditionalCheckFailedException`, not by a status"), format!("AWS の API のエラーは、ステータスではなく `{en} = ConditionalCheckFailedException` のように例外の名前で書きます")));
                    }
                    (Some(Binding::Lambda(_)), None, None) => {}
                    (Some(Binding::Lambda(_)), _, _) => {
                        self.push(e("E007", *esp, "a Lambda task's error is named by the error type the function raises, without a status", "Lambda のタスクのエラーは関数が投げるエラーの型の名前で表し、ステータスは書きません"));
                    }
                    (None, None, None) => {}
                    (None, _, _) => {
                        self.push(e("E007", *esp, format!("`{en} = …` says how an HTTP or AWS call names the error; this task has neither `http` nor `aws`"), format!("`{en} = …` は HTTP や AWS の呼び出しでのエラーの表し方です。このタスクには `http` も `aws` もありません")));
                    }
                    (Some(Binding::Http { .. }), Some(_), Some(_)) => {}
                    // refused above
                    (Some(Binding::Agent { .. }), _, _) => {}
                }
                if let Some(st) = status {
                    if let Some(other) = errors.iter().find(|x| x.status == Some(st)) {
                        let other = other.name.clone();
                        self.push(e(
                            "E007",
                            *esp,
                            format!("`{other}` and `{en}` both come back as {st}; Step Functions tells errors apart by status, so give each its own"),
                            format!("`{other}` と `{en}` がどちらも {st} で返ってきます。Step Functions はエラーをステータスで見分けるので、別々のステータスにしてください"),
                        ));
                        continue;
                    }
                }
                errors.push(ErrDef { name: en.clone(), status, exception });
            }
            // the error a less sure answer fails the call with is one of the task's own
            if let (Some((_, (fe, fsp), _)), Some(_)) = (&t.confidence, &jev) {
                if fe == "timeout" || fe == "failure" {
                    self.push(e("E007", *fsp, format!("`{fe}` is always there; name an error of the task's own for an answer that is not sure enough"), format!("`{fe}` はいつもあるエラーです。確信度が足りない答えには、タスク自身のエラーの名前を付けてください")));
                } else if errors.iter().any(|x| x.name == *fe) {
                    self.push(e("E006", *fsp, format!("the error `{fe}` is written twice: `confidence … else {fe}` declares it"), format!("エラー `{fe}` が二度書かれています。`confidence … else {fe}` がそれを宣言します")));
                } else {
                    errors.push(ErrDef { name: fe.clone(), status: None, exception: None });
                }
            }
            if let Some((syntax::Binding::Http { url, .. }, bsp)) = t.binding.as_ref().map(|(b, s)| (b, s)) {
                for ph in placeholders(url) {
                    if !params.iter().any(|(n, _)| *n == ph) {
                        self.push(e("E007", *bsp, format!("the URL has `{{{ph}}}` but the task has no parameter `{ph}`"), format!("URL に `{{{ph}}}` がありますが、タスクに引数 `{ph}` がありません")));
                    }
                }
            }
            if let Some(csp) = t.callback {
                let ok = match &binding {
                    None | Some(Binding::Lambda(_)) => true,
                    Some(Binding::Aws { service, action }) => service == "sqs" && action == "sendMessage",
                    Some(Binding::Http { .. }) | Some(Binding::Agent { .. }) | Some(Binding::Jev(_)) => false,
                };
                if !ok {
                    self.push(e(
                        "E007",
                        csp,
                        "a `callback` task hands its token over by `lambda` or by `aws sqs:sendMessage`",
                        "`callback` のタスクがトークンを渡せるのは `lambda` か `aws sqs:sendMessage` です",
                    ));
                } else if matches!(binding, Some(Binding::Aws { .. })) {
                    match params.iter().find(|(p, _)| p == "MessageBody").map(|(_, t)| t.inner().clone()) {
                        Some(Ty::Record(_)) | Some(Ty::Json) => {}
                        _ => self.push(e(
                            "E007",
                            csp,
                            "a callback through SQS puts its token into `MessageBody`, so the task needs a parameter `MessageBody` that is a record or `json`",
                            "SQS を通すコールバックはトークンを `MessageBody` に入れるので、レコードか `json` の引数 `MessageBody` が要ります",
                        )),
                    }
                }
                if let Some(c) = child {
                    self.push(e("E007", c, "a child workflow answers when it ends; it is not a `callback` task", "子ワークフローは終わったときに結果を返すので、`callback` のタスクにはなりません"));
                }
            }
            if let Some(esp) = t.event {
                // nothing is called: a value comes to the workflow, sent to it by name
                // at the clause that calls, where there is one to point at
                let calls = t.binding.as_ref().map(|(_, s)| *s).or(child).or(t.image.as_ref().map(|(_, s)| *s)).or(t.callback);
                if calls.is_some() || t.queue.is_some() || t.connection.is_some() {
                    self.push(e(
                        "E007",
                        calls.unwrap_or(esp),
                        "an `event` task calls nothing: its value is sent to the workflow by name; leave out the ways of calling it (and `callback`, whose task hands on an id first)",
                        "`event` のタスクは何も呼びません。値はワークフローに名前で送られてきます。呼び出し方の項目（と、先に ID を渡す `callback`）は外してください",
                    ));
                }
                if !params.is_empty() {
                    self.push(e("E007", *sp, "an `event` task sends nothing, so it takes no parameters", "`event` のタスクは何も送らないので、引数を取りません"));
                }
                if let Some(r) = &t.retry {
                    self.push(e("E007", r.span, "an `event` task calls nothing that could be tried again; whoever sends the event sends it again", "`event` のタスクには、リトライする呼び出しがありません。送る側が送り直します"));
                }
                if let Some(ksp) = t.key {
                    self.push(e("E007", ksp, "an `event` task sends nothing, so it takes no `key`", "`event` のタスクは何も送らないので、`key` は要りません"));
                }
                if matches!(t.machine, Some((MachineUse::Starts { .. }, _)) | Some((MachineUse::Sends { .. }, _))) {
                    self.push(e("E007", esp, "an event can tell what a case is (`observes`), not start a case or send it an event", "`event` のタスクが案件についてできるのは、状態を知らせる（`observes`）ことだけです。案件を始めたり、イベントを送ったりはできません"));
                }
            }
            if let Some(ksp) = t.key {
                if child.is_some() {
                    self.push(e("E007", ksp, "the platform starts a child workflow once for each call, so `key` does not apply to it", "子ワークフローはプラットフォームが呼び出しごとに一度だけ始めるので、`key` は使えません"));
                }
                match (&binding, &t.key_param) {
                    (Some(Binding::Agent { .. }), _) => self.push(e("E007", ksp, "an agent changes nothing on the other side, so it takes no `key`", "エージェントは外部のデータを何も変えないので、`key` は要りません")),
                    (Some(Binding::Jev(_)), _) => self.push(e("E007", ksp, "Jev only answers, and changes nothing on the other side, so it takes no `key`", "Jev は答えるだけで外部のデータを何も変えないので、`key` は要りません")),
                    (Some(Binding::Aws { .. }), None) => self.push(e(
                        "E007",
                        ksp,
                        "an AWS API takes the idempotency key as one of its own parameters; write `key <parameter>`, as `key ClientToken`",
                        "AWS の API は冪等キーを自分の引数の一つで受け取ります。`key ClientToken` のように `key <引数>` と書いてください",
                    )),
                    (Some(Binding::Aws { .. }), Some((kp, kpsp))) => {
                        if params.iter().any(|(p, _)| p == kp) {
                            self.push(e("E007", *kpsp, format!("`{kp}` is filled by `key`; leave it out of the parameters"), format!("`{kp}` は `key` が埋めるので、引数からは外してください")));
                        }
                    }
                    (_, Some((_, kpsp))) => self.push(e("E007", *kpsp, "`key <parameter>` is for an AWS API; write just `key`", "`key <引数>` は AWS の API のための書き方です。ここでは `key` だけを書きます")),
                    _ => {}
                }
            }
            let retry = t.retry.as_ref().map(|r| {
                for (on, osp) in &r.on {
                    if on != "timeout" && on != "failure" && !errors.iter().any(|x| x.name == *on) {
                        self.diags.push(e("E002", *osp, format!("`{on}` is not an error of `{name}`"), format!("`{on}` は `{name}` のエラーではありません")));
                    }
                    if jev.as_ref().and_then(|j| j.floor.as_ref()).is_some_and(|(_, fe)| fe == on) {
                        self.diags.push(e(
                            "E007",
                            *osp,
                            format!("Jev answers the same input much the same way each time, so asking again is not surer; `{on}` is not retried"),
                            format!("Jev は同じ入力にはほぼ同じように答えるので、尋ね直しても確信度は上がりません。`{on}` はリトライできません"),
                        ));
                    }
                }
                Retry { times: r.times, every: r.every, backoff: r.backoff, on: r.on.iter().map(|x| x.0.clone()).collect() }
            });
            if let Some((ra, rsp)) = &t.refused_as {
                if !errors.iter().any(|x| x.name == *ra) {
                    self.push(e("E007", *rsp, format!("declare `{ra}` in `errors` too"), format!("`{ra}` を `errors` にも書いてください")));
                }
            }
            let machine = t.machine.as_ref().and_then(|(mu, msp)| match mu {
                MachineUse::Starts { machine, then } => {
                    let rule = self.machine_rule(machine, *msp)?;
                    Some(TaskMachine::Starts { rule, then: then.iter().map(|x| x.0.clone()).collect() })
                }
                MachineUse::Sends { event, column } => Some(TaskMachine::Sends { event: event.0.clone(), column: column.as_ref().map(|c| c.0.clone()) }),
                MachineUse::Observes => Some(TaskMachine::Observes),
            });
            if t.refused_as.is_some() && !matches!(machine, Some(TaskMachine::Sends { .. })) {
                self.push(e("E007", t.refused_as.as_ref().unwrap().1, "`refused as` belongs to a task that `sends` an event", "`refused as` はイベントを `sends` するタスクに書きます"));
            }
            if machine.is_some() && result.is_none() {
                self.push(e("E008", *sp, format!("`{name}` moves a case, so it answers with the case's record; write `-> <record>`"), format!("`{name}` は案件を動かすので、案件のレコードを返します。`-> <レコード>` を書いてください")));
            }
            self.task_ix.insert(name.clone(), self.m.tasks.len());
            self.m.tasks.push(TaskDef {
                name: name.clone(),
                params,
                param_ranges,
                result,
                result_range,
                binding,
                connection: t.connection.clone(),
                // a `.flow` child runs on the queue of its own worker, unless the task names the type it runs as
                queue: t.queue.clone().or_else(|| flow.as_ref().filter(|_| t.workflow.is_none()).map(|c| crate::temporal::workflow_type(&c.model))),
                workflow: t.workflow.as_ref().map(|x| x.0.clone()).or_else(|| flow.as_ref().map(|c| crate::temporal::workflow_type(&c.model))),
                state_machine: t.state_machine.as_ref().map(|x| x.0.clone()),
                durable_function: t.durable_function.as_ref().map(|x| x.0.clone()),
                image: t.image.as_ref().map(|x| x.0.clone()),
                argo_template: t.argo_template.as_ref().map(|x| x.0.clone()).or_else(|| flow.as_ref().map(|c| crate::argo::workflow_name(&c.model))),
                errors,
                retry,
                timeout: t.timeout,
                key: t.key.is_some(),
                key_param: t.key_param.as_ref().map(|x| x.0.clone()),
                idempotent: t.idempotent,
                machine,
                refused_as: t.refused_as.as_ref().map(|x| x.0.clone()),
                callback: t.callback.is_some(),
                event: t.event.is_some(),
                flow,
                connect,
                answer_zeros: None,
                line: sp.line,
            });
            let task = self.m.tasks.last().expect("just pushed");
            let held = task.flow.as_ref().map(|c| crate::contract::check(&self.m, task, &c.model)).unwrap_or_default();
            self.diags.extend(held);
            // the operation of the API's description the task calls: what it takes, answers and fails with
            if let Some((api, b)) = &described {
                let op = match b {
                    syntax::Binding::Http { method, url, .. } => crate::apis::openapi_op(api, method, url),
                    syntax::Binding::Connect { method, .. } => crate::apis::proto_op(api, method),
                    syntax::Binding::Aws { action, .. } => crate::apis::smithy_op(api, action),
                    _ => unreachable!("an API's operation"),
                };
                let bsp = t.binding.as_ref().map(|(_, s)| *s).unwrap_or(*sp);
                match op {
                    Ok(op) => {
                        let task = self.m.tasks.last().expect("just pushed");
                        let path_params = match b {
                            syntax::Binding::Http { url, .. } => placeholders(url),
                            _ => vec![],
                        };
                        let found = op.check(&self.m, task, &path_params);
                        for (en, ja) in found {
                            let mut d = e("E016", Span { line: sp.line, col: 1 }, en, ja);
                            // what the task is held to may name a type of a file that was not read
                            if let Some((nen, nja)) = Self::unread_note_of(api) {
                                d = d.note(nen, nja);
                            }
                            self.push(d);
                        }
                    }
                    Err((en, ja)) => {
                        if !matches!(b, syntax::Binding::Http { .. } | syntax::Binding::Connect { .. }) {
                            self.push(e("E016", bsp, en, ja));
                        }
                    }
                }
            }
        }
    }

    /// The service of a `.proto` the workflow implements (`implements`, DESIGN 1.14): its methods and
    /// dandori's marks on them, the zero values protobuf's JSON leaves out of the request that starts a
    /// run, and of the values the service's methods send the `event` and `callback` tasks. Here is
    /// said only what keeps the service from being found; how the workflow fits it is held to it once
    /// the flow is lowered (`service::check`, E017), when the names the flow fails with are known.
    fn service(&mut self) {
        let Some(parts) = self.prog.implements.clone() else { return };
        let (api, at) = (parts[0].0.clone(), parts[0].1);
        let rel = parts[1..].iter().map(|p| p.0.as_str()).collect::<Vec<_>>().join(".");
        let pf = match self.apis.get(&api) {
            Some((ApiKind::Proto, _)) => self.proto_of(&api),
            Some((kind, _)) => {
                let k = kind.word();
                self.push(e(
                    "E017",
                    at,
                    format!("`{api}` is read by `use {k}`, and a workflow implements a service of a `.proto` (`use proto`)"),
                    format!("`{api}` は `use {k}` で読んだものですが、ワークフローが実装できるのは `.proto`（`use proto`）のサービスです"),
                ));
                return;
            }
            // a description that could not be read is said where it is read
            None if self.prog.apis.iter().any(|u| u.name.0 == api) => return,
            None => {
                self.push(e("E002", at, format!("there is no API `{api}`; name one read by `use proto`"), format!("API `{api}` はありません。`use proto` で読んだものを書きます")));
                return;
            }
        };
        // by its full name, from the package, or by its own name when that is all that is written
        let own = format!("{}.{rel}", pf.package);
        let found = pf.services.iter().find(|x| x.name == rel).or_else(|| pf.services.iter().find(|x| x.name == own)).or_else(|| {
            if rel.contains('.') {
                return None;
            }
            pf.services.iter().find(|x| x.name.rsplit('.').next() == Some(rel.as_str()))
        });
        let Some(svc) = found.cloned() else {
            let pkg = format!("{}.", pf.package);
            let names: Vec<String> = pf.services.iter().map(|x| if pf.package.is_empty() { x.name.clone() } else { x.name.strip_prefix(&pkg).unwrap_or(&x.name).to_string() }).collect();
            let (en, ja) = if names.is_empty() {
                (format!("`{api}` has no service `{rel}` (it has no services)"), format!("`{api}` にサービス `{rel}` はありません（サービスはありません）"))
            } else {
                (format!("`{api}` has no service `{rel}` (its services are {})", names.join(", ")), format!("`{api}` にサービス `{rel}` はありません（サービスは {}）", names.join("・")))
            };
            let mut d = e("E017", at, en, ja);
            // the service may be in a file that was not read
            if let Some((nen, nja)) = crate::apis::unread_note(&pf) {
                d = d.note(nen, nja);
            }
            self.push(d);
            return;
        };
        let methods: Vec<MethodUse> = svc
            .methods
            .iter()
            .map(|mt| {
                let o = &mt.options;
                let task = |k: &str| crate::proto::strings_of(&o[k]["task"]).into_iter().next().unwrap_or_default();
                let mut marks = Vec::new();
                if let Some(v) = o.get("dandori.v1.start") {
                    marks.push(Mark::Start { fails: crate::proto::strings_of(&v["fails"]) });
                }
                if o.contains_key("dandori.v1.event") {
                    marks.push(Mark::Event { task: task("dandori.v1.event") });
                }
                if o.contains_key("dandori.v1.answer") {
                    marks.push(Mark::Answer { task: task("dandori.v1.answer") });
                }
                if o.contains_key("dandori.v1.status") {
                    marks.push(Mark::Status);
                }
                MethodUse { name: mt.name.clone(), request: mt.input.clone(), response: mt.output.clone(), streams: mt.streams, marks }
            })
            .collect();
        // the zero values of what comes in: the start's request, and the values of the events and the callbacks' answers
        let starts: Vec<&MethodUse> = methods.iter().filter(|x| x.starts()).collect();
        let input_zeros = match starts.as_slice() {
            [one] => crate::apis::zeros_of(&pf, &one.request),
            _ => serde_json::json!({}),
        };
        for mt in &methods {
            for k in &mt.marks {
                let (task, event) = match k {
                    Mark::Event { task } => (task, true),
                    Mark::Answer { task } => (task, false),
                    _ => continue,
                };
                if let Some(t) = self.task_ix.get(task).copied() {
                    let td = &mut self.m.tasks[t];
                    if (event && td.event) || (!event && td.callback) {
                        td.answer_zeros = Some(crate::apis::zeros_of(&pf, &mt.request));
                    }
                }
            }
        }
        let file = self.prog.apis.iter().find(|u| u.name.0 == api).map(|u| u.path.clone()).unwrap_or_default();
        let doc = self.apis[&api].1.clone();
        self.m.service = Some(ServiceUse { api, file, name: svc.name.clone(), line: at.line, col: at.col, doc, methods, input_zeros });
    }

    /// What an agent task must have, and what it cannot: a provider dandori knows, a model, an
    /// answer whose type the provider's structured outputs can hold the model to, and nothing that
    /// moves a case.
    fn agent(&mut self, t: &syntax::TaskDecl, result: Option<&Ty>, rg: Option<Range>) {
        let jev = matches!(t.binding, Some((syntax::Binding::Jev(_), _)));
        if let (Some((_, usp)), true) = (&t.url, jev) {
            self.push(e(
                "E007",
                *usp,
                "Jev is always called at TypeSafe's API; `url` says which server an agent's Open Responses API is on",
                "Jev はいつも TypeSafe の API で呼びます。`url` は、エージェントの Open Responses の API がどのサーバーにあるかを書くところです",
            ));
            return;
        }
        if let (Some((_, esp)), true) = (&t.effort, jev) {
            self.push(e("E007", *esp, "Jev answers at once and does not reason at length; `effort` says how hard an agent's model reasons", "Jev はすぐに答え、長く推論しません。`effort` はエージェントのモデルが推論にどれだけ力を入れるかを書くところです"));
            return;
        }
        if jev {
            return;
        }
        if let (Some((_, usp)), false) = (&t.url, matches!(t.binding, Some((syntax::Binding::Agent { .. }, _)))) {
            self.push(e(
                "E007",
                *usp,
                "`url` says which server an agent's Open Responses API is on; this task has no `agent` (an HTTP task writes its URL after `http`)",
                "`url` はエージェントの Open Responses の API がどのサーバーにあるかを書くところです。このタスクには `agent` がありません（HTTP のタスクの URL は `http` のあとに書きます）",
            ));
        }
        if let (Some((_, esp)), false) = (&t.effort, matches!(t.binding, Some((syntax::Binding::Agent { .. }, _)))) {
            self.push(e(
                "E007",
                *esp,
                "`effort` says how hard an agent's model reasons; this task has no `agent`",
                "`effort` はエージェントのモデルが推論にどれだけ力を入れるかを書くところです。このタスクには `agent` がありません",
            ));
        }
        let (provider, bsp) = match (&t.binding, &t.model) {
            (Some((syntax::Binding::Agent { provider, .. }, bsp)), _) => (provider.clone(), *bsp),
            (_, Some((_, msp))) => {
                self.push(e("E007", *msp, "`model` says which model an agent or Jev uses; this task has neither `agent` nor `jev`", "`model` はエージェントや Jev が使うモデルを書くところです。このタスクには `agent` も `jev` もありません"));
                return;
            }
            _ => return,
        };
        let provider = match provider {
            None => Provider::OpenAi,
            Some((p, _)) if p == "openai" => Provider::OpenAi,
            Some((p, _)) if p == "claude" => Provider::Claude,
            Some((p, psp)) => {
                self.push(e(
                    "E007",
                    psp,
                    format!("`{p}` is not an agent dandori knows; write `agent openai \"…\"` (or just `agent \"…\"`) or `agent claude \"…\"`"),
                    format!("`{p}` は dandori の知らないエージェントです。`agent openai \"…\"`（`agent \"…\"` だけでも同じ）か `agent claude \"…\"` と書いてください"),
                ));
                return;
            }
        };
        if let Some((u, usp)) = &t.url {
            if provider == Provider::Claude {
                self.push(e(
                    "E007",
                    *usp,
                    "`url` names a server of Open Responses (OpenAI's Responses API, as others serve it); a Claude agent is called on Anthropic's Messages API, which is not among them",
                    "`url` は Open Responses（OpenAI の Responses API を、ほかのサーバーも同じ形で出すもの）のサーバーを書くところです。Claude のエージェントは Anthropic の Messages API で呼び、それは Open Responses ではありません",
                ));
            } else if !(u.starts_with("https://") || u.starts_with("http://")) {
                self.push(e("E007", *usp, format!("`{u}` is not an http:// or https:// URL"), format!("`{u}` は http:// や https:// の URL ではありません")));
            }
        }
        if let Some((level, esp)) = &t.effort {
            // the levels each provider's API takes: OpenAI's `reasoning.effort` (which Open
            // Responses has too) and Claude's `output_config.effort`
            let (levels, whose): (&[&str], &str) = match provider {
                Provider::OpenAi => (&["none", "minimal", "low", "medium", "high", "xhigh", "max"], if t.url.is_some() { "Open Responses" } else { "OpenAI" }),
                Provider::Claude => (&["low", "medium", "high", "xhigh", "max"], "Claude"),
            };
            if !levels.contains(&level.as_str()) {
                self.push(e(
                    "E007",
                    *esp,
                    format!("`{level}` is not an effort {whose} takes; write one of {}", levels.join(", ")),
                    format!("`{level}` は {whose} が受け付けるエフォートではありません。{} のどれかを書いてください", levels.join("・")),
                ));
            }
        }
        let (outputs_en, outputs_ja) = match (provider, &t.url) {
            // with a space before the Japanese that follows an English word
            (Provider::OpenAi, None) => ("OpenAI's Structured Outputs", "OpenAI の Structured Outputs "),
            (Provider::OpenAi, Some(_)) => ("the structured outputs of Open Responses", "Open Responses の構造化出力"),
            (Provider::Claude, _) => ("Claude's structured outputs", "Claude の構造化出力"),
        };
        if t.model.is_none() {
            let example = match provider {
                Provider::OpenAi => "gpt-5.4-mini",
                Provider::Claude => "claude-sonnet-5",
            };
            self.push(e(
                "E007",
                bsp,
                format!("an agent needs the model it uses; write it as `model \"{example}\"`"),
                format!("エージェントには、使うモデルを `model \"{example}\"` のように書いてください"),
            ));
        }
        if let Some((_, msp)) = &t.machine {
            self.push(e(
                "E007",
                *msp,
                "an agent reads what it is given and answers; it has no case on the other side to start, move or look at",
                "エージェントは渡されたものを読んで応答するだけで、案件を始めたり動かしたり見たりはしません",
            ));
        }
        let Some(r) = result else {
            self.push(e("E007", bsp, "an agent answers; write the type of its answer as `-> <type>`", "エージェントは応答を返します。応答の型を `-> <型>` と書いてください"));
            return;
        };
        let Some(schema) = crate::render::answer_schema(&self.m, r, rg, provider) else {
            let (en, ja) = if has_json(&self.m, r, &mut Vec::new()) {
                (
                    format!("{outputs_en} hold an agent's answer to a JSON Schema, and `json` has none; give the answer a type without `json`"),
                    format!("エージェントの応答は、{outputs_ja}で JSON Schema に合わせて返させます。`json` は Schema に書けないので、`json` を含まない型にしてください"),
                )
            } else {
                (
                    format!("{outputs_en} hold an agent's answer to a JSON Schema written out in full, and a record that holds itself through others has no end; give the answer a type without it"),
                    format!("エージェントの応答は、{outputs_ja}で JSON Schema に合わせて返させます。ほかのレコードを通して自分を含むレコードは Schema に書き切れないので、それを含まない型にしてください"),
                )
            };
            self.push(e("E007", bsp, en, ja));
            return;
        };
        let mut over: Vec<(String, String)> = Vec::new();
        match provider {
            // another server's limits are its own, and unknown here: its answer's check tells
            Provider::OpenAi if t.url.is_some() => {}
            Provider::OpenAi => {
                let (depth, props, values) = crate::render::schema_size(&schema);
                if depth > 10 {
                    over.push((format!("its objects nest {depth} deep (at most 10)"), format!("オブジェクトのネストが {depth} 段（10 段まで）")));
                }
                if props > 5000 {
                    over.push((format!("it has {props} properties (at most 5,000)"), format!("プロパティが {props} 個（5,000 個まで）")));
                }
                if values > 1000 {
                    over.push((format!("it has {values} enum values (at most 1,000)"), format!("列挙の値が {values} 個（1,000 個まで）")));
                }
            }
            Provider::Claude => {
                let unions = crate::render::schema_unions(&schema);
                if unions > crate::render::CLAUDE_UNIONS {
                    over.push((
                        format!("it has {unions} values that may be absent, each a choice with null (at most {})", crate::render::CLAUDE_UNIONS),
                        format!("オプショナルな値（`T?`）が {unions} 個（{} 個まで）", crate::render::CLAUDE_UNIONS),
                    ));
                }
            }
        }
        if !over.is_empty() {
            self.push(e(
                "E007",
                bsp,
                format!("the answer's JSON Schema, in `{{\"answer\": …}}`, is larger than {outputs_en} take: {}", over.iter().map(|x| x.0.clone()).collect::<Vec<_>>().join(", ")),
                format!("応答の JSON Schema（`{{\"answer\": …}}` に包んだもの）が、{outputs_ja}の受け付ける大きさを超えています。{}", over.iter().map(|x| x.1.clone()).collect::<Vec<_>>().join("、")),
            ));
        }
        if provider == Provider::Claude {
            // Claude may answer an enum's value in another case, which dandori takes as the value
            // it differs from only in case; two values that differ only in case would be one
            for en in crate::render::enums_of(&self.m, r) {
                let name = self.m.enums[en].name.clone();
                let values = self.m.enums[en].values.clone();
                for (i, a) in values.iter().enumerate() {
                    if let Some(b) = values[..i].iter().find(|b| b.to_lowercase() == a.to_lowercase()) {
                        self.push(e(
                            "E007",
                            bsp,
                            format!("Claude may answer an enum's value in another case, and dandori takes it as the value it matches without regard to case, so `{b}` and `{a}` of `{name}` would be the same; give them names that differ in more than case"),
                            format!("Claude は列挙の値の大文字と小文字を変えて返すことがあり、dandori は大文字と小文字を区別せずに値を読みます。このため `{name}` の `{b}` と `{a}` は同じ値になります。大文字と小文字のほかにも違いのある名前にしてください"),
                        ));
                    }
                }
            }
        }
    }

    /// What a Jev task asks, from the type of its answer: an enum is a choice among its values
    /// (`score`: a place on a scale of them), `bool` yes or no, and a record a question for each
    /// field, or the field that takes how sure Jev is of another's answer. Jev writes no text, so
    /// an answer that is not one of these is refused (E007).
    fn jev(&mut self, t: &syntax::TaskDecl, jd: &syntax::JevDecl, bsp: Span, result: Option<&Ty>) -> Jev {
        let model = t.model.as_ref().map(|x| x.0.clone()).unwrap_or_default();
        let floor = t.confidence.as_ref().map(|(v, e, _)| (*v, e.0.clone()));
        let mut out = Jev { model, questions: vec![], confidences: vec![], floor };
        if t.model.is_none() {
            self.push(e(
                "E007",
                bsp,
                "Jev needs the model it asks; write the version, as `model \"jev-1.13.0\"`",
                "Jev には、尋ねるモデルを `model \"jev-1.13.0\"` のようにバージョンで書いてください",
            ));
        }
        if let Some((_, msp)) = &t.machine {
            self.push(e("E007", *msp, "Jev reads what it is given and answers; it has no case on the other side to start, move or look at", "Jev は渡されたものを読んで答えるだけで、案件を始めたり動かしたり見たりはしません"));
        }
        const WHAT_EN: &str = "Jev answers a choice among an enum's values, a place on a scale of them (`score`), yes or no (`bool`), or a record of such answers; it writes no text";
        const WHAT_JA: &str = "Jev が答えるのは、列挙の値のどれか、列挙の値を低いものから並べた段階のどこか（`score`）、はいかいいえ（`bool`）と、それらを並べたレコードです。文章は書きません";
        let Some(r) = result else {
            self.push(e("E007", bsp, format!("{WHAT_EN}; write the type of its answer as `-> <type>`"), format!("{WHAT_JA}。結果の型を `-> <型>` と書いてください")));
            return out;
        };
        match (&jd.ask, r) {
            (Some(ask), Ty::Enum(_) | Ty::Bool) => {
                if let Some(q) = self.jev_question(ask, r, None) {
                    out.questions.push(q);
                }
            }
            (Some(ask), Ty::Record(_)) => {
                self.push(e(
                    "E007",
                    ask.span,
                    "the answer is a record, so Jev is asked a question for each of its fields: write `jev` alone, and under it `<field> \"<question>\"` for each",
                    "結果はレコードなので、Jev にはフィールドごとに尋ねます。`jev` だけを書き、その下にフィールドごとに `<フィールド> \"<質問>\"` を書いてください",
                ));
            }
            (None, Ty::Enum(_) | Ty::Bool) => {
                self.push(e("E007", bsp, "write the question Jev answers after `jev`, in double quotes: `jev \"<question>\"`", "Jev に尋ねることを、`jev \"<質問>\"` のように `jev` のあとに二重引用符で書いてください"));
            }
            (None, Ty::Record(rid)) => {
                let rid = *rid;
                let rec = self.m.records[rid].clone();
                let mut seen: BTreeMap<String, Span> = BTreeMap::new();
                for (f, _) in &jd.fields {
                    if let Some(first) = seen.get(&f.0) {
                        self.push(e("E006", f.1, format!("`{}` is asked twice (line {})", f.0, first.line), format!("`{}` が二度尋ねられています（{} 行目）", f.0, first.line)));
                        continue;
                    }
                    seen.insert(f.0.clone(), f.1);
                    if !rec.fields.iter().any(|(n, _)| *n == f.0) {
                        let fields = rec.fields.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
                        self.push(e("E002", f.1, format!("`{}` is not a field of `{}`, whose fields are {}", f.0, rec.name, fields.join(", ")), format!("`{}` は `{}` のフィールドではありません（フィールドは {}）", f.0, rec.name, fields.join("・"))));
                    }
                }
                // the questions first, in the order of the record's fields, then how sure of which
                for (fname, fty) in &rec.fields {
                    let Some((_, what)) = jd.fields.iter().find(|(n, _)| n.0 == *fname) else {
                        self.push(e(
                            "E007",
                            bsp,
                            format!("write under `jev` what Jev is asked for the field `{fname}` of `{}`: `{fname} \"<question>\"`, or `{fname} confidence of <field>`", rec.name),
                            format!("`{}` のフィールド `{fname}` について Jev に尋ねることを、`jev` の下に書いてください（`{fname} \"<質問>\"` か `{fname} confidence of <フィールド>`）", rec.name),
                        ));
                        continue;
                    };
                    if let syntax::JevField::Ask(ask) = what {
                        match fty {
                            Ty::Enum(_) | Ty::Bool => {
                                if let Some(q) = self.jev_question(ask, fty, Some(fname)) {
                                    out.questions.push(q);
                                }
                            }
                            other => {
                                let tn = self.m.ty_name(other);
                                self.push(e("E007", ask.span, format!("{WHAT_EN}; the field `{fname}` is `{tn}`"), format!("{WHAT_JA}。フィールド `{fname}` は `{tn}` です")));
                            }
                        }
                    }
                }
                for (fname, fty) in &rec.fields {
                    let Some((fsp, syntax::JevField::Confidence(of))) = jd.fields.iter().find(|(n, _)| n.0 == *fname).map(|(n, w)| (n.1, w)) else { continue };
                    let per = match fty {
                        Ty::Num(u) => rate_per(u),
                        _ => None,
                    };
                    let Some(per) = per else {
                        let tn = self.m.ty_name(fty);
                        self.push(e(
                            "E007",
                            fsp,
                            format!("how sure Jev is goes into a rate with a step that goes into 100% a whole number of times, such as `rate[step 1%]` or `rate[step 0.01%]`; `{fname}` is `{tn}`"),
                            format!("Jev の確信度が入るのは、`rate[step 1%]` や `rate[step 0.01%]` のように、刻みで 100% を割り切れる率です。`{fname}` は `{tn}` です"),
                        ));
                        continue;
                    };
                    match out.questions.iter().position(|q| q.field.as_deref() == Some(of.0.as_str())) {
                        Some(qi) => out.confidences.push((fname.clone(), qi, per)),
                        None => self.push(e(
                            "E007",
                            of.1,
                            format!("`{}` is not a field Jev is asked about, so there is no answer to be sure of", of.0),
                            format!("`{}` については Jev に尋ねていないので、その答えの確信度はありません", of.0),
                        )),
                    }
                }
                if out.questions.is_empty() && !jd.fields.is_empty() {
                    self.push(e("E007", bsp, format!("{WHAT_EN}; ask it about one field of `{}` at least", rec.name), format!("{WHAT_JA}。`{}` のフィールドの一つは尋ねてください", rec.name)));
                }
            }
            (_, other) => {
                let tn = self.m.ty_name(other);
                self.push(e("E007", bsp, format!("{WHAT_EN}; the answer is `{tn}`"), format!("{WHAT_JA}。結果は `{tn}` です")));
            }
        }
        if let Some((v, _, csp)) = &t.confidence {
            // yes or no: sure of the answer it takes by the probability of that answer, one half at least
            if !out.questions.is_empty() && out.questions.iter().all(|q| q.kind == QuestionKind::Noul) && *v <= 0.5 {
                self.push(e(
                    "E007",
                    *csp,
                    format!("Jev is at least half sure of the answer it takes to a yes or no, so `confidence {v}` never fails the call; give more than 0.5"),
                    format!("はいかいいえの答えには、Jev はいつも半分以上確かなので、`confidence {v}` で呼び出しが失敗することはありません。0.5 より大きい値を書いてください"),
                ));
            }
        }
        // a floor, or how sure it is, means one version's answers; an alias moves to the next
        if let (true, Some((m, msp))) = (out.uses_confidence(), &t.model) {
            if m == "jev-latest" || m == "jev-preview" {
                self.diags.push(Diag::warning(
                    "W032",
                    msp.line,
                    msp.col,
                    format!("`{m}` is an alias, which moves to a new version of Jev without a change here, and how sure one version is means something else to the next; name the version the confidence is set for, as `model \"jev-1.13.0\"`"),
                    format!("`{m}` はエイリアスで、ここを変えなくても Jev の新しいバージョンに移ります。確信度の意味はバージョンごとに違うので、確信度を合わせたバージョンを `model \"jev-1.13.0\"` のように書いてください"),
                ));
            }
        }
        out
    }

    /// One question of a Jev task, whose answer is of type `ty` (an enum or `bool`).
    fn jev_question(&mut self, ask: &syntax::JevAsk, ty: &Ty, field: Option<&str>) -> Option<Question> {
        let id = field.unwrap_or("answer").to_string();
        let mut seen: BTreeMap<String, Span> = BTreeMap::new();
        for (v, vsp) in ask.criteria.iter().map(|(n, _)| n) {
            if let Some(first) = seen.get(v) {
                self.push(e("E006", *vsp, format!("what `{v}` means is written twice (line {})", first.line), format!("`{v}` の意味が二度書かれています（{} 行目）", first.line)));
                return None;
            }
            seen.insert(v.clone(), *vsp);
        }
        let meaning = |v: &str| ask.criteria.iter().find(|((n, _), _)| n == v).map(|(_, m)| m.clone());
        match ty {
            Ty::Enum(en) => {
                let def = self.m.enums[*en].clone();
                for ((v, vsp), _) in &ask.criteria {
                    if !def.values.contains(v) {
                        self.push(e("E002", *vsp, format!("`{v}` is not a value of `{}`, whose values are {}", def.name, def.values.join(", ")), format!("`{v}` は `{}` の値ではありません（値は {}）", def.name, def.values.join("・"))));
                        return None;
                    }
                }
                if let Some(ssp) = ask.score {
                    // the levels are the values, each with what it means, from the lowest
                    if !(2..=10).contains(&def.values.len()) {
                        self.push(e(
                            "E007",
                            ssp,
                            format!("a score has from 2 to 10 levels, and `{}` has {} values", def.name, def.values.len()),
                            format!("score の段階は 2 から 10 までで、`{}` の値は {} 個です", def.name, def.values.len()),
                        ));
                        return None;
                    }
                    let missing: Vec<&String> = def.values.iter().filter(|v| meaning(v).is_none()).collect();
                    if !missing.is_empty() {
                        let names = missing.iter().map(|v| v.as_str()).collect::<Vec<_>>();
                        self.push(e(
                            "E007",
                            ask.span,
                            format!("Jev sees a score's levels only by what each means, so write under it what every value of `{}` means, from the lowest level to the highest; {} is missing", def.name, names.join(", ")),
                            format!("Jev は score の段階を、書いた意味だけで見分けます。`{}` のすべての値の意味を、低い段階から高い段階の順に下に書いてください。{} がありません", def.name, names.join("・")),
                        ));
                        return None;
                    }
                    let options = ask.criteria.iter().map(|((v, _), m)| (v.clone(), Some(m.clone()))).collect();
                    Some(Question { id, field: field.map(String::from), kind: QuestionKind::Score, instructions: ask.instructions.clone(), options })
                } else {
                    if def.values.len() > 255 {
                        self.push(e("E007", ask.span, format!("Jev chooses among at most 255 options, and `{}` has {} values", def.name, def.values.len()), format!("Jev が選べるのは 255 個までで、`{}` の値は {} 個です", def.name, def.values.len())));
                        return None;
                    }
                    let options = def.values.iter().map(|v| (v.clone(), meaning(v))).collect();
                    Some(Question { id, field: field.map(String::from), kind: QuestionKind::Choice, instructions: ask.instructions.clone(), options })
                }
            }
            Ty::Bool => {
                if let Some(ssp) = ask.score {
                    self.push(e("E007", ssp, "a score is a place on a scale of an enum's values; a `bool` is asked as yes or no", "score は、列挙の値を低いものから並べた段階のどこかを答えます。`bool` は、はいかいいえで尋ねます"));
                    return None;
                }
                for ((v, vsp), _) in &ask.criteria {
                    if v != "true" && v != "false" {
                        self.push(e("E002", *vsp, format!("the answer is yes or no, so write what `true` and `false` mean, not `{v}`"), format!("答えははいかいいえなので、`{v}` ではなく `true` と `false` の意味を書きます")));
                        return None;
                    }
                }
                let options: Vec<(String, Option<String>)> = match (meaning("true"), meaning("false")) {
                    (Some(y), Some(n)) => vec![("true".into(), Some(y)), ("false".into(), Some(n))],
                    (None, None) => vec![],
                    _ => {
                        self.push(e("E007", ask.span, "write what both `true` and `false` mean, or neither", "`true` と `false` の意味は、両方書くか、どちらも書かないかです"));
                        return None;
                    }
                };
                Some(Question { id, field: field.map(String::from), kind: QuestionKind::Noul, instructions: ask.instructions.clone(), options })
            }
            _ => None,
        }
    }

    /// `payment_intent.payment`: a rule that is used, and its machine's name.
    fn machine_rule(&mut self, q: &[(String, Span)], sp: Span) -> Option<usize> {
        if q.len() != 2 {
            self.push(e("E002", sp, "name a machine as <rule>.<machine>", "ステートマシンは <規則>.<ステートマシン> と書きます"));
            return None;
        }
        let rix = match self.rule_ix.get(&q[0].0) {
            Some(r) => *r,
            None => {
                self.push(e("E002", q[0].1, format!("no rule `{}` is used", q[0].0), format!("規則 `{}` は読み込まれていません", q[0].0)));
                return None;
            }
        };
        match &self.m.rules[rix].info.machine {
            Some(mc) if mc.name == q[1].0 => Some(rix),
            Some(mc) => {
                let n = mc.name.clone();
                self.push(e("E002", q[1].1, format!("the machine of `{}` is `{n}`", q[0].0), format!("`{}` のステートマシンは `{n}` です", q[0].0)));
                None
            }
            None => {
                self.push(e("E002", q[0].1, format!("`{}` has no machine", q[0].0), format!("`{}` にはステートマシンがありません", q[0].0)));
                None
            }
        }
    }

    fn cases(&mut self) {
        for c in &self.prog.cases {
            let (name, sp) = &c.name;
            if self.m.cases.iter().any(|x| x.name == *name) {
                self.push(e("E006", *sp, format!("the case `{name}` is declared twice"), format!("案件 `{name}` が二度宣言されています")));
                continue;
            }
            let record = match self.ty(&c.record) {
                Some(Ty::Record(r)) if matches!(self.m.records[r].origin, RecordOrigin::Local | RecordOrigin::Proto { .. }) => r,
                Some(_) => {
                    self.push(e(
                        "E008",
                        c.record.span(),
                        "a case is held in a record declared in this file or made from a `.proto`",
                        "案件の型には、このファイルで宣言したレコードか、`.proto` から作ったレコードを使います",
                    ));
                    continue;
                }
                None => continue,
            };
            let rule = match self.machine_rule(&c.machine, c.machine[0].1) {
                Some(r) => r,
                None => continue,
            };
            let mc = self.m.rules[rule].info.machine.clone().unwrap();
            let state_enum = *self.enum_ix.get(&format!("{}.{}", self.m.rules[rule].name, mc.state_enum)).unwrap();
            let fields = self.m.records[record].fields.clone();
            // the machine's own state enum, or an enum made from a `.proto` whose values are the machine's states
            let sorted = |v: &[String]| {
                let mut v = v.to_vec();
                v.sort();
                v
            };
            let is_state = |lw: &Self, t: &Ty| match t {
                Ty::Enum(x) => *x == state_enum || (lw.made_enums.contains(x) && sorted(&lw.m.enums[*x].values) == sorted(&mc.states)),
                _ => false,
            };
            // an enum made from a `.proto`, which says its values are not the machine's states
            let not_states = |lw: &mut Self, sp: Span, t: &Ty| {
                let Ty::Enum(x) = t else { return false };
                if !lw.made_enums.contains(x) {
                    return false;
                }
                let en = &lw.m.enums[*x];
                let machine = format!("{}.{}", lw.m.rules[rule].name, mc.name);
                let d = e(
                    "E008",
                    sp,
                    format!("the values of `{}` ({}) are not the states of `{machine}` ({})", en.name, en.values.join(", "), mc.states.join(", ")),
                    format!("`{}` の値（{}）は、`{machine}` の状態（{}）と違います", en.name, en.values.join("・"), mc.states.join("・")),
                );
                lw.push(d);
                true
            };
            let state_field = match &c.state_field {
                Some((f, fsp)) => match fields.iter().find(|(n, _)| n == f) {
                    Some((_, t)) if is_state(self, t) => f.clone(),
                    Some((_, t)) => {
                        let t = t.clone();
                        if !not_states(self, *fsp, &t) {
                            self.push(e("E008", *fsp, format!("`{f}` is not of the machine's state type"), format!("`{f}` はステートマシンの状態の型ではありません")));
                        }
                        continue;
                    }
                    None => {
                        self.push(e("E002", *fsp, format!("`{}` has no field `{f}`", self.m.records[record].name), format!("`{}` にフィールド `{f}` はありません", self.m.records[record].name)));
                        continue;
                    }
                },
                None => {
                    let cands: Vec<&String> = fields.iter().filter(|(_, t)| is_state(self, t)).map(|(n, _)| n).collect();
                    match cands.len() {
                        1 => cands[0].clone(),
                        0 => {
                            // one field of a made enum that is not the states says so
                            let made: Vec<Ty> = fields.iter().filter(|(_, t)| matches!(t, Ty::Enum(x) if self.made_enums.contains(x))).map(|(_, t)| t.clone()).collect();
                            if made.len() == 1 && not_states(self, c.record.span(), &made[0]) {
                                continue;
                            }
                            self.push(e(
                                "E008",
                                c.record.span(),
                                format!("`{}` has no field of type `{}` to hold the case's state", self.m.records[record].name, self.m.enums[state_enum].name),
                                format!("`{}` に、案件の状態を入れる `{}` 型のフィールドがありません", self.m.records[record].name, self.m.enums[state_enum].name),
                            ));
                            continue;
                        }
                        _ => {
                            self.push(e("E008", c.record.span(), "more than one field could hold the state; write `state <field>`", "状態を入れられるフィールドが二つ以上あります。`state <フィールド>` を書いてください"));
                            continue;
                        }
                    }
                }
            };
            let mut held = Vec::new();
            let mut held_values = Vec::new();
            for ((inp, isp), (val, vsp)) in &c.held {
                if !mc.held.contains(inp) {
                    self.push(e(
                        "E008",
                        *isp,
                        format!("`{inp}` is not held by the machine; its held inputs are {}", if mc.held.is_empty() { "none".to_string() } else { mc.held.join(", ") }),
                        format!("`{inp}` はステートマシンの held ではありません（held は {}）", if mc.held.is_empty() { "ありません".to_string() } else { mc.held.join("・") }),
                    ));
                    continue;
                }
                if let Some(a) = mc.axis_of(inp) {
                    match mc.axes[a].coords.iter().position(|x| x == val) {
                        Some(ci) => held.push((a, ci)),
                        None => {
                            self.push(e("E003", *vsp, format!("`{val}` is not a value of `{inp}` ({})", mc.axes[a].coords.join(", ")), format!("`{val}` は `{inp}` の値ではありません（{}）", mc.axes[a].coords.join("・"))));
                            continue;
                        }
                    }
                }
                held_values.push((inp.clone(), val.clone()));
            }
            let mut external = Vec::new();
            for (ev, esp) in &c.external {
                let axes = mc.axes_with_value(ev);
                match axes.len() {
                    1 => external.push((axes[0], mc.axes[axes[0]].coords.iter().position(|x| x == ev).unwrap(), ev.clone())),
                    0 => self.push(e("E008", *esp, format!("no input of the machine has the event `{ev}`"), format!("ステートマシンのどの入力にもイベント `{ev}` はありません"))),
                    _ => self.push(e("E008", *esp, format!("more than one input has the value `{ev}`"), format!("値 `{ev}` を持つ入力が二つ以上あります"))),
                }
            }
            let refused_when = match &c.refused_when {
                Some(((o, osp), (v, _))) => match mc.decides.iter().position(|d| d == o) {
                    Some(i) => Some((i, v.clone())),
                    None => {
                        self.push(e("E008", *osp, format!("the machine's table does not write `{o}`; it writes {}", mc.decides.join(", ")), format!("ステートマシンの表は `{o}` を書きません（書くのは {}）", mc.decides.join("・"))));
                        None
                    }
                },
                None => None,
            };
            self.m.cases.push(CaseDef { name: name.clone(), record, rule, state_field, held, held_values, external, refused_when, line: sp.line });
        }
    }

    /// Every variable and its type, before the statements are read: inputs, cases, and every
    /// name a statement sets — `let`, `for`, `some`. A value's type can depend on another
    /// variable's, so the names are read again until no new one turns up.
    fn variables(&mut self) {
        for (n, t) in self.m.inputs.clone() {
            self.var_ty.insert(n, t);
        }
        for c in self.m.cases.clone() {
            if self.var_ty.contains_key(&c.name) {
                self.push(Diag::error("E006", c.line, 1, format!("`{}` is both an input and a case", c.name), format!("`{}` が入力と案件の両方にあります", c.name)));
            }
            self.var_ty.insert(c.name.clone(), Ty::Record(c.record));
        }
        let mut problems: BTreeMap<(usize, usize), Diag> = BTreeMap::new();
        loop {
            let before = self.var_ty.len();
            self.widened = false;
            let blocks: Vec<Block> = [&self.prog.flow, &self.prog.on_failure, &self.prog.on_cancel].iter().filter_map(|b| b.as_ref().map(|(b, _)| b.clone())).collect();
            for b in &blocks {
                self.scan_vars(b, &mut problems);
            }
            if self.var_ty.len() == before && !self.widened {
                break;
            }
            problems.clear();
        }
        for (_, d) in problems {
            self.push(d);
        }
    }

    fn reserved(&self, n: &str) -> bool {
        self.m.cases.iter().any(|c| c.name == n) || self.m.inputs.iter().any(|(i, _)| *i == n)
    }

    fn declare(&mut self, n: &str, sp: Span, t: Ty, problems: &mut BTreeMap<(usize, usize), Diag>) {
        if self.reserved(n) {
            problems.insert(
                (sp.line, sp.col),
                e("E006", sp, format!("`{n}` is an input or a case; a variable set here needs a new name"), format!("`{n}` は入力か案件です。ここで値を入れる変数には別の名前を付けてください")),
            );
            return;
        }
        match self.var_ty.get(n).cloned() {
            // a value that fits the name's type is set to it; a name set to both `T` and `T?` is `T?`
            Some(prev) if t.fits(&prev) => {}
            Some(prev) if prev.fits(&t) && matches!(t, Ty::Opt(_)) => {
                self.var_ty.insert(n.to_string(), t);
                self.widened = true;
            }
            Some(prev) if prev != t => {
                let (a, b) = (self.m.ty_name(&prev), self.m.ty_name(&t));
                problems.insert(
                    (sp.line, sp.col),
                    e("E003", sp, format!("`{n}` was `{a}` elsewhere and is `{b}` here; a name keeps one type"), format!("`{n}` はほかの場所で `{a}`、ここで `{b}` です。名前の型は一つです")),
                );
            }
            Some(_) => {}
            None => {
                self.var_ty.insert(n.to_string(), t);
            }
        }
    }

    /// Run `f` for its answer only: what it would say is dropped.
    fn quiet<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let n = self.diags.len();
        let out = f(self);
        self.diags.truncate(n);
        out
    }

    fn quiet_ty(&mut self, te: &TypeExpr) -> Option<Ty> {
        self.quiet(|x| x.ty(te))
    }

    fn quiet_expr(&mut self, ex: &Expr, want: Option<&Ty>) -> Option<Ty> {
        self.quiet(|x| x.expr(ex, want).map(|t| t.ty()))
    }

    fn scan_vars(&mut self, b: &Block, problems: &mut BTreeMap<(usize, usize), Diag>) {
        for s in b {
            match &s.kind {
                StmtKind::Let { name, ty, call, handlers } => {
                    let t = match ty {
                        Some(te) => self.quiet_ty(te),
                        None => self.callee_result(call),
                    };
                    if let Some(t) = t {
                        self.declare(&name.0, name.1, t, problems);
                    }
                    for h in handlers {
                        self.scan_vars(&h.body, problems);
                    }
                }
                StmtKind::Assign { name, ty, expr } => {
                    let t = match ty {
                        Some(te) => self.quiet_ty(te),
                        None => self.quiet_expr(expr, None),
                    };
                    if let Some(t) = t {
                        self.declare(&name.0, name.1, t, problems);
                    }
                }
                StmtKind::Call { handlers, .. } | StmtKind::CaseCall { handlers, .. } => {
                    for h in handlers {
                        self.scan_vars(&h.body, problems);
                    }
                }
                StmtKind::Match { expr, arms } => {
                    for a in arms {
                        if let Some((v, vsp)) = &a.some {
                            if let Some(Ty::Opt(inner)) = self.quiet_expr(expr, None) {
                                self.declare(v, *vsp, *inner, problems);
                            }
                        }
                        self.scan_vars(&a.body, problems);
                    }
                }
                StmtKind::Repeat { body, .. } => self.scan_vars(body, problems),
                StmtKind::For { var, list, body, result, .. } => {
                    if let Some(Ty::List(elem)) = self.quiet_expr(list, None) {
                        self.declare(&var.0, var.1, *elem, problems);
                    }
                    self.scan_vars(body, problems);
                    if let Some((r, rty)) = result {
                        let t = match rty {
                            Some(te) => self.quiet_ty(te),
                            None => match body.last().map(|x| &x.kind) {
                                Some(StmtKind::Yield { expr }) => self.quiet_expr(expr, None).map(|t| Ty::List(Box::new(t))),
                                _ => None,
                            },
                        };
                        if let Some(t) = t {
                            self.declare(&r.0, r.1, t, problems);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn callee_result(&self, call: &Call) -> Option<Ty> {
        if let Some(t) = self.task_ix.get(&call.callee.0) {
            return self.m.tasks[*t].result.clone();
        }
        if let Some(r) = self.rule_ix.get(&call.callee.0) {
            return Some(Ty::Record(self.m.rules[*r].outputs));
        }
        None
    }

    // -----------------------------------------------------------------------
    // Statements

    fn block(&mut self, b: &Block) -> Vec<TStmt> {
        let mut out = Vec::new();
        for s in b {
            if let Some(t) = self.stmt(s) {
                out.push(t);
            }
        }
        out
    }

    fn stmt(&mut self, s: &syntax::Stmt) -> Option<TStmt> {
        let site = self.next_site();
        let line = s.span.line;
        let kind = match &s.kind {
            StmtKind::Let { name, ty, call, handlers } => {
                let (callee, args) = self.call(call)?;
                let result = match &callee {
                    Callee::Task(t) => self.m.tasks[*t].result.clone(),
                    Callee::Rule(r) => Some(Ty::Record(self.m.rules[*r].outputs)),
                };
                let result = match result {
                    Some(r) => r,
                    None => {
                        self.push(e("E003", call.callee.1, format!("`{}` answers nothing to keep; call it without `let`", call.callee.0), format!("`{}` は何も返しません。`let` を付けずに呼んでください", call.callee.0)));
                        return None;
                    }
                };
                if let Some(te) = ty {
                    let want = self.ty(te)?;
                    if !result.fits(&want) {
                        let (a, b) = (self.m.ty_name(&result), self.m.ty_name(&want));
                        self.push(e("E003", call.callee.1, format!("`{}` answers `{a}`, which is not `{b}`", call.callee.0), format!("`{}` が返すのは `{a}` で、`{b}` ではありません", call.callee.0)));
                        return None;
                    }
                }
                let hs = self.handlers(&callee, handlers);
                if !self.var_ty.contains_key(&name.0) || self.reserved(&name.0) {
                    return None;
                }
                TK::Call { target: Some(Target::Let(name.0.clone())), callee, args, handlers: hs }
            }
            StmtKind::Assign { name, ty, expr } => {
                let want = match ty {
                    Some(te) => Some(self.ty(te)?),
                    None => self.var_ty.get(&name.0).cloned(),
                };
                if want.is_none() {
                    if let Expr::Record(_, sp) = expr {
                        self.push(e("E003", *sp, format!("write the record's type: `let {}: <record> = {{…}}`", name.0), format!("レコードの型を `let {}: <レコード> = {{…}}` のように書いてください", name.0)));
                        return None;
                    }
                }
                let x = self.expr(expr, want.as_ref())?;
                if self.reserved(&name.0) || !self.var_ty.contains_key(&name.0) {
                    return None;
                }
                TK::Assign { name: name.0.clone(), expr: x }
            }
            StmtKind::Call { call, handlers } => {
                let (callee, args) = self.call(call)?;
                if let Callee::Rule(_) = callee {
                    self.push(e("E009", call.callee.1, "a rule only answers; keep its answer with `let`", "規則は結果を返すだけです。`let` で結果を受け取ってください"));
                    return None;
                }
                let hs = self.handlers(&callee, handlers);
                TK::Call { target: None, callee, args, handlers: hs }
            }
            StmtKind::CaseCall { case, call, handlers } => {
                let ci = match self.m.case_index(&case.0) {
                    Some(c) => c,
                    None => {
                        self.push(e("E002", case.1, format!("there is no case `{}`", case.0), format!("案件 `{}` はありません", case.0)));
                        return None;
                    }
                };
                let (callee, args) = self.call(call)?;
                let ti = match callee {
                    Callee::Task(t) => t,
                    Callee::Rule(_) => {
                        self.push(e("E008", call.callee.1, "a case moves by a task that `starts`, `sends` or `observes`; a rule is called with `let`", "案件を動かすのは `starts`・`sends`・`observes` を書いたタスクです。規則は `let` で呼びます"));
                        return None;
                    }
                };
                if self.par_depth > 0 {
                    self.push(e(
                        "E009",
                        case.1,
                        format!("the case `{}` cannot be moved from inside `for … in parallel`, where the rounds run at the same time", case.0),
                        format!("同時に回る `for … in parallel` の中からは、案件 `{}` を動かせません", case.0),
                    ));
                    return None;
                }
                let task = self.m.tasks[ti].clone();
                if task.result != Some(Ty::Record(self.m.cases[ci].record)) {
                    let a = task.result.as_ref().map(|r| self.m.ty_name(r)).unwrap_or_else(|| "nothing".into());
                    let b = self.m.records[self.m.cases[ci].record].name.clone();
                    self.push(e("E003", call.callee.1, format!("`{}` returns `{a}`, but the case `{}` is held in `{b}`", task.name, case.0), format!("`{}` が返すのは `{a}` ですが、案件 `{}` の型は `{b}` です", task.name, case.0)));
                    return None;
                }
                match &task.machine {
                    None => {
                        self.push(e("E008", call.callee.1, format!("`{}` does not say what it does to a case; write `starts`, `sends` or `observes` under it", task.name), format!("`{}` には、案件に何をするかが書かれていません。`starts`・`sends`・`observes` のどれかを書いてください", task.name)));
                        return None;
                    }
                    Some(TaskMachine::Starts { rule, .. }) if *rule != self.m.cases[ci].rule => {
                        self.push(e("E008", call.callee.1, format!("`{}` starts a case of another machine", task.name), format!("`{}` が始めるのは別のステートマシンの案件です", task.name)));
                        return None;
                    }
                    _ => {}
                }
                let hs = self.handlers(&callee, handlers);
                TK::Call { target: Some(Target::Case(ci)), callee, args, handlers: hs }
            }
            StmtKind::Match { expr, arms } => self.lower_match(expr, arms)?,
            StmtKind::Wait { seconds } => {
                if *seconds == 0 {
                    self.push(e("E009", s.span, "a wait of zero does nothing", "0 の待ちは何もしません"));
                }
                TK::Wait { seconds: *seconds }
            }
            StmtKind::WaitUntil { at } => {
                let te = self.expr(at, Some(&Ty::Timestamp))?;
                TK::WaitUntil { at: te }
            }
            StmtKind::Repeat { times, body } => {
                self.loops.push(false);
                let b = self.block(body);
                self.loops.pop();
                TK::Repeat { times: *times, body: b }
            }
            StmtKind::For { var, list, max, parallel, body, result } => {
                let lx = self.expr(list, None)?;
                let elem = match lx.ty() {
                    Ty::List(t) => *t,
                    Ty::Opt(_) => {
                        self.push(e("E003", list.span(), format!("`{}` may be absent here; `match` it with `none` and `some <name>` first", lx.show()), format!("ここでは `{}` が無いことがあります。先に `none` と `some <名前>` で `match` してください", lx.show())));
                        return None;
                    }
                    other => {
                        let n = self.m.ty_name(&other);
                        self.push(e("E003", list.span(), format!("`for` goes through a list; this is `{n}`"), format!("`for` で回せるのはリストです。これは `{n}` です")));
                        return None;
                    }
                };
                if self.reserved(&var.0) {
                    self.push(e("E006", var.1, format!("`{}` is an input or a case; the loop's variable needs a new name", var.0), format!("`{}` は入力か案件です。ループの変数には別の名前を付けてください", var.0)));
                    return None;
                }
                if self.var_ty.get(&var.0) != Some(&elem) {
                    return None;
                }
                let mut body = body.clone();
                let yielded = match result {
                    Some((r, _)) => match body.last().map(|x| x.kind.clone()) {
                        Some(StmtKind::Yield { expr }) => {
                            body.pop();
                            Some((r.clone(), expr))
                        }
                        _ => {
                            self.push(e("E009", s.span, format!("`let {} = for …` needs `yield <value>` as the last line of its body", r.0), format!("`let {} = for …` の本体の最後の行には `yield <値>` が要ります", r.0)));
                            return None;
                        }
                    },
                    None => None,
                };
                self.loops.push(parallel.is_some());
                if parallel.is_some() {
                    self.par_depth += 1;
                }
                let b = self.block(&body);
                let result = match yielded {
                    Some((r, ex)) => {
                        let want = match self.var_ty.get(&r.0) {
                            Some(Ty::List(t)) => Some((**t).clone()),
                            _ => None,
                        };
                        let x = self.expr(&ex, want.as_ref());
                        match x {
                            Some(x) => Some((r.0.clone(), x)),
                            None => {
                                self.loops.pop();
                                if parallel.is_some() {
                                    self.par_depth -= 1;
                                }
                                return None;
                            }
                        }
                    }
                    None => None,
                };
                if parallel.is_some() {
                    self.par_depth -= 1;
                }
                self.loops.pop();
                if let Some((r, _)) = &result {
                    if self.reserved(r) || !self.var_ty.contains_key(r) {
                        return None;
                    }
                }
                TK::For { var: var.0.clone(), list: lx, max: *max, parallel: *parallel, body: b, result, locals: vec![] }
            }
            StmtKind::Yield { expr } => {
                self.push(e("E009", expr.span(), "`yield` is the last line of the body of `let <name> = for …`", "`yield` は `let <名前> = for …` の本体の最後の行に書きます"));
                return None;
            }
            StmtKind::Pass => TK::Pass,
            StmtKind::Break => {
                match self.loops.last() {
                    None => {
                        self.push(e("E009", s.span, "`break` is written inside `repeat` or `for`", "`break` は `repeat` か `for` の中に書きます"));
                        return None;
                    }
                    Some(true) => {
                        self.push(e("E009", s.span, "the rounds of `for … in parallel` run at the same time, so there is no `break` from them", "`for … in parallel` のイテレーションは同時に走るので、`break` で抜けられません"));
                        return None;
                    }
                    Some(false) => {}
                }
                TK::Break
            }
            StmtKind::Succeed { fields } => {
                if self.in_on_failure {
                    self.push(e("E009", s.span, "`on failure` ends the workflow as failed; it cannot `succeed`", "`on failure` はワークフローを失敗で終えます。`succeed` は書けません"));
                    return None;
                }
                if self.in_on_cancel {
                    self.push(e("E009", s.span, "`on cancel` ends the workflow as cancelled; it cannot `succeed`", "`on cancel` はワークフローをキャンセルされたとして終えます。`succeed` は書けません"));
                    return None;
                }
                if self.par_depth > 0 {
                    self.push(e("E009", s.span, "the workflow cannot `succeed` from inside `for … in parallel`, where other rounds may still run", "ほかのイテレーションがまだ動いていることがあるので、`for … in parallel` の中からは `succeed` できません"));
                    return None;
                }
                let mut out = Vec::new();
                for ((n, nsp), ex) in fields {
                    let oty = match self.m.outputs.iter().find(|(o, _)| o == n) {
                        Some((_, t)) => t.clone(),
                        None => {
                            self.push(e("E002", *nsp, format!("there is no output `{n}`"), format!("出力 `{n}` はありません")));
                            continue;
                        }
                    };
                    if out.iter().any(|(o, _): &(String, TExpr)| o == n) {
                        self.push(e("E006", *nsp, format!("`{n}` is given twice"), format!("`{n}` が二度書かれています")));
                        continue;
                    }
                    if let Some(te) = self.expr(ex, Some(&oty)) {
                        out.push((n.clone(), te));
                    }
                }
                let missing: Vec<String> = self
                    .m
                    .outputs
                    .iter()
                    .filter(|(o, t)| !matches!(t, Ty::Opt(_)) && !out.iter().any(|(g, _)| g == o) && !fields.iter().any(|((f, _), _)| f == o))
                    .map(|(o, _)| o.clone())
                    .collect();
                if !missing.is_empty() {
                    self.push(e("E004", s.span, format!("`succeed` does not give {}", missing.join(", ")), format!("`succeed` が {} を書いていません", missing.join("・"))));
                }
                TK::Succeed { fields: out }
            }
            StmtKind::Fail { error, cause, leaving } => {
                let mut left = Vec::new();
                for (l, lsp) in leaving {
                    match self.m.case_index(l) {
                        Some(c) => left.push(c),
                        None => self.push(e("E002", *lsp, format!("there is no case `{l}`"), format!("案件 `{l}` はありません"))),
                    }
                }
                let cause = match cause {
                    Some(c) => Some(self.expr(c, Some(&Ty::Str))?),
                    None => None,
                };
                TK::Fail { error: error.0.clone(), cause, leaving: left }
            }
        };
        Some(TStmt { kind, line, site })
    }

    fn lower_match(&mut self, expr: &Expr, arms: &[syntax::Arm]) -> Option<TK> {
        let te = self.expr(expr, None)?;
        let ty = te.ty();
        let (domain, optional): (Vec<String>, bool) = match &ty {
            Ty::Enum(e) => (self.m.enums[*e].values.clone(), false),
            Ty::Bool => (vec!["true".into(), "false".into()], false),
            Ty::Opt(inner) => match &**inner {
                Ty::Enum(e) => (self.m.enums[*e].values.clone(), true),
                Ty::Bool => (vec!["true".into(), "false".into()], true),
                _ => (vec![], true),
            },
            other => {
                let n = self.m.ty_name(other);
                self.push(e(
                    "E009",
                    expr.span(),
                    format!("`match` works on an enum, a bool, or a value that may be absent (`T?`); this is `{n}`"),
                    format!("`match` に渡せるのは列挙・bool・オプショナルな値（`T?`）です。これは `{n}` です"),
                ));
                return None;
            }
        };
        let is_case_state = match &te {
            TExpr::Var { name, fields, .. } => fields.len() == 1 && self.m.case_index(name).map(|c| self.m.cases[c].state_field == fields[0]).unwrap_or(false),
            _ => false,
        };
        let has_some = arms.iter().any(|a| a.some.is_some());
        let has_values = arms.iter().any(|a| a.values.iter().any(|(v, _)| v != "none"));
        let mut seen: Vec<String> = Vec::new();
        let mut tarms = Vec::new();
        for a in arms {
            if let Some((v, vsp)) = &a.some {
                if !optional {
                    self.push(e("E003", *vsp, "`some` is written when matching a value that may be absent (`T?`)", "`some` を書けるのは、オプショナルな値（`T?`）で分けるときだけです"));
                    continue;
                }
                if seen.iter().any(|x| x == "some") {
                    self.push(e("E011", a.span, "`some` already has an arm above", "`some` の分岐は上にもう書かれています"));
                    continue;
                }
                if has_values {
                    self.push(e("E003", a.span, "`some` stands for every value that is there; write either the values or `some`", "`some` は値があるときのすべてを表します。値を並べるか `some` を書くかのどちらかにしてください"));
                    continue;
                }
                seen.push("some".into());
                if self.reserved(v) {
                    self.push(e("E006", *vsp, format!("`{v}` is an input or a case; `some` needs a new name"), format!("`{v}` は入力か案件です。`some` には別の名前を付けてください")));
                    continue;
                }
                let body = self.block(&a.body);
                tarms.push(TArm { values: vec![], none: false, some: Some(v.clone()), body, line: a.span.line });
                continue;
            }
            let mut values = Vec::new();
            let mut none = false;
            for (v, vsp) in &a.values {
                if seen.contains(v) {
                    self.push(e("E011", *vsp, format!("`{v}` already has an arm above"), format!("`{v}` の分岐は上にもう書かれています")));
                    continue;
                }
                seen.push(v.clone());
                if v == "none" {
                    if is_case_state || optional {
                        none = true;
                    } else {
                        self.push(e("E003", *vsp, "`none` is written when matching a case's state, or a value that may be absent (`T?`)", "`none` を書けるのは、案件の状態か、オプショナルな値（`T?`）で分けるときだけです"));
                    }
                    continue;
                }
                if optional && domain.is_empty() {
                    let n = self.m.ty_name(&ty);
                    self.push(e("E003", *vsp, format!("`{}` is `{n}`; its arms are `none` and `some <name>`", te.show()), format!("`{}` は `{n}` です。分岐は `none` と `some <名前>` です", te.show())));
                    continue;
                }
                if !domain.contains(v) {
                    let n = self.m.ty_name(ty.inner());
                    self.push(e("E003", *vsp, format!("`{v}` is not a value of `{n}` ({})", domain.join(", ")), format!("`{v}` は `{n}` の値ではありません（{}）", domain.join("・"))));
                    continue;
                }
                values.push(v.clone());
            }
            let _ = has_some;
            let body = self.block(&a.body);
            tarms.push(TArm { values, none, some: None, body, line: a.span.line });
        }
        Some(TK::Match { expr: te, arms: tarms })
    }

    fn handlers(&mut self, callee: &Callee, hs: &[syntax::Handler]) -> Vec<THandler> {
        let declared: Vec<String> = match callee {
            Callee::Task(t) => self.m.tasks[*t].errors.iter().map(|x| x.name.clone()).collect(),
            Callee::Rule(_) => vec![],
        };
        let mut seen: Vec<String> = Vec::new();
        let mut out = Vec::new();
        for h in hs {
            let mut errs = Vec::new();
            for (n, sp) in &h.errors {
                if seen.contains(n) {
                    self.push(e("E011", *sp, format!("`{n}` is handled above already"), format!("`{n}` は上ですでに処理しています")));
                    continue;
                }
                seen.push(n.clone());
                match n.as_str() {
                    "timeout" => errs.push(HErr::Timeout),
                    "failure" => errs.push(HErr::Failure),
                    _ if declared.contains(n) => errs.push(HErr::Declared(n.clone())),
                    _ => {
                        let list = if declared.is_empty() { "timeout, failure".to_string() } else { format!("{}, timeout, failure", declared.join(", ")) };
                        self.push(e("E002", *sp, format!("`{n}` is not an error this call can raise ({list})"), format!("`{n}` はこの呼び出しが投げるエラーではありません（{list}）")));
                    }
                }
            }
            let body = self.block(&h.body);
            out.push(THandler { errors: errs, body, line: h.span.line });
        }
        // `failure` catches everything the arms before it did not; an arm after it never runs
        let mut after_failure = false;
        for (i, h) in hs.iter().enumerate() {
            if after_failure {
                self.push(e("E011", h.span, "this arm comes after `on failure`, which already takes every error", "この分岐は、すべてのエラーを処理する `on failure` のあとにあります"));
            }
            if out.get(i).map(|x| x.errors.contains(&HErr::Failure)).unwrap_or(false) {
                after_failure = true;
            }
        }
        out
    }

    fn call(&mut self, c: &Call) -> Option<(Callee, Vec<(String, TExpr)>)> {
        let (name, sp) = &c.callee;
        let (callee, params): (Callee, Vec<(String, Ty)>) = if let Some(t) = self.task_ix.get(name) {
            if self.m.tasks[*t].event && self.par_depth > 0 {
                self.push(e(
                    "E009",
                    *sp,
                    format!("`{name}` waits for an event, which cannot be waited for inside `for … in parallel`: the rounds would wait for the same one, and it would not say which round it is for"),
                    format!("`{name}` はイベントを待ちますが、`for … in parallel` の中では待てません。どのイテレーションも同じイベントを待つことになり、どのイテレーションに宛てたものか分かりません"),
                ));
                return None;
            }
            (Callee::Task(*t), self.m.tasks[*t].params.clone())
        } else if let Some(r) = self.rule_ix.get(name) {
            let r = *r;
            let rn = self.m.rules[r].name.clone();
            let ps = self.m.rules[r].info.inputs.iter().map(|col| (col.name.clone(), self.rty(&rn, &col.ty))).collect();
            (Callee::Rule(r), ps)
        } else {
            self.push(e("E002", *sp, format!("there is no task or rule `{name}`"), format!("タスクか規則 `{name}` はありません")));
            return None;
        };
        let mut args = Vec::new();
        let mut ok = true;
        for ((an, asp), ex) in &c.args {
            let pty = match params.iter().find(|(p, _)| p == an) {
                Some((_, t)) => t.clone(),
                None => {
                    let list: Vec<&str> = params.iter().map(|(p, _)| p.as_str()).collect();
                    self.push(e("E004", *asp, format!("`{name}` has no parameter `{an}` ({})", list.join(", ")), format!("`{name}` に引数 `{an}` はありません（{}）", list.join("・"))));
                    ok = false;
                    continue;
                }
            };
            if args.iter().any(|(a, _): &(String, TExpr)| a == an) {
                self.push(e("E004", *asp, format!("`{an}` is given twice"), format!("`{an}` が二度書かれています")));
                ok = false;
                continue;
            }
            match self.expr(ex, Some(&pty)) {
                Some(te) => args.push((an.clone(), te)),
                None => ok = false,
            }
        }
        // an optional parameter may be left out; it is then sent as null
        let missing: Vec<String> = params.iter().filter(|(p, t)| !matches!(t, Ty::Opt(_)) && !c.args.iter().any(|((a, _), _)| a == p)).map(|(p, _)| p.clone()).collect();
        if !missing.is_empty() {
            self.push(e("E004", *sp, format!("the call of `{name}` does not give {}", missing.join(", ")), format!("`{name}` の呼び出しに {} がありません", missing.join("・"))));
            ok = false;
        }
        if !ok {
            return None;
        }
        for (p, t) in &params {
            if matches!(t, Ty::Opt(_)) && !args.iter().any(|(a, _)| a == p) {
                args.push((p.clone(), TExpr::None(t.clone())));
            }
        }
        // keep the parameters' order, so that the generated code reads like the declaration
        args.sort_by_key(|(a, _)| params.iter().position(|(p, _)| p == a).unwrap_or(usize::MAX));
        Some((callee, args))
    }

    fn expr(&mut self, ex: &Expr, expected: Option<&Ty>) -> Option<TExpr> {
        let te = match ex {
            Expr::Str(s, _) => TExpr::Str(s.clone()),
            Expr::Int(n, _) => TExpr::Int(*n),
            Expr::Bool(b, _) => TExpr::Bool(*b),
            Expr::Interp(parts, sp) => {
                let mut out = Vec::new();
                for p in parts {
                    match p {
                        Part::Lit(s) => out.push(IPart::Lit(s.clone())),
                        Part::Hole(path) => {
                            let x = self.path(path, None)?;
                            let t = x.ty();
                            if !matches!(t, Ty::Str | Ty::Int | Ty::Num(_) | Ty::Bool | Ty::Enum(_) | Ty::Timestamp) {
                                let n = self.m.ty_name(&t);
                                let (en, ja) = if matches!(t, Ty::Opt(_)) {
                                    (
                                        format!("`{}` is `{n}` and may be absent, so it cannot be put in a string as it is; `match` it with `none` and `some <name>` first", x.show()),
                                        format!("`{}` は `{n}` で、値が無いことがあるので、そのままでは文字列に入れられません。先に `none` と `some <名前>` で `match` してください", x.show()),
                                    )
                                } else {
                                    (format!("`{}` is `{n}`, which cannot be put in a string", x.show()), format!("`{}` は `{n}` なので、文字列に入れられません", x.show()))
                                };
                                self.push(e("E003", *sp, en, ja));
                                return None;
                            }
                            out.push(IPart::Hole(x));
                        }
                    }
                }
                TExpr::Interp(out)
            }
            Expr::Record(fields, sp) => {
                let want = expected.map(|t| t.inner().clone());
                match want {
                    Some(Ty::Record(r)) => {
                        let decl = self.m.records[r].fields.clone();
                        let rn = self.m.records[r].name.clone();
                        let mut out: Vec<(String, TExpr)> = Vec::new();
                        for ((fname, fsp), fe) in fields {
                            let fty = match decl.iter().find(|(n, _)| n == fname) {
                                Some((_, t)) => t.clone(),
                                None => {
                                    let list: Vec<String> = decl.iter().map(|(n, _)| n.clone()).collect();
                                    self.push(e("E002", *fsp, format!("`{rn}` has no field `{fname}` ({})", list.join(", ")), format!("`{rn}` にフィールド `{fname}` はありません（{}）", list.join("・"))));
                                    return None;
                                }
                            };
                            if out.iter().any(|(n, _)| n == fname) {
                                self.push(e("E006", *fsp, format!("`{fname}` is given twice"), format!("`{fname}` が二度書かれています")));
                                return None;
                            }
                            let x = self.expr(fe, Some(&fty))?;
                            out.push((fname.clone(), x));
                        }
                        let missing: Vec<String> = decl.iter().filter(|(n, t)| !matches!(t, Ty::Opt(_)) && !out.iter().any(|(g, _)| g == n)).map(|(n, _)| n.clone()).collect();
                        if !missing.is_empty() {
                            self.push(e("E004", *sp, format!("this `{rn}` does not give {}", missing.join(", ")), format!("この `{rn}` に {} がありません", missing.join("・"))));
                            return None;
                        }
                        out.sort_by_key(|(n, _)| decl.iter().position(|(d, _)| d == n).unwrap_or(usize::MAX));
                        TExpr::Record { fields: out, ty: Ty::Record(r) }
                    }
                    Some(Ty::Json) => {
                        let mut out: Vec<(String, TExpr)> = Vec::new();
                        for ((fname, fsp), fe) in fields {
                            if out.iter().any(|(n, _)| n == fname) {
                                self.push(e("E006", *fsp, format!("`{fname}` is given twice"), format!("`{fname}` が二度書かれています")));
                                return None;
                            }
                            let x = self.expr(fe, Some(&Ty::Json))?;
                            out.push((fname.clone(), x));
                        }
                        TExpr::Record { fields: out, ty: Ty::Json }
                    }
                    _ => {
                        self.push(e(
                            "E003",
                            *sp,
                            "a record written as `{…}` takes its type from where it goes: an argument, an output, or `let x: <record> = {…}`",
                            "`{…}` で書いたレコードは、型が決まるところ（引数・出力・`let x: <レコード> = {…}`）に書きます",
                        ));
                        return None;
                    }
                }
            }
            Expr::List(items, sp) => {
                let want_elem = match expected.map(|t| t.inner()) {
                    Some(Ty::List(t)) => Some((**t).clone()),
                    Some(Ty::Json) => Some(Ty::Json),
                    _ => None,
                };
                let mut elem = want_elem;
                let mut out = Vec::new();
                for it in items {
                    let x = self.expr(it, elem.as_ref())?;
                    if elem.is_none() {
                        elem = Some(x.ty());
                    }
                    out.push(x);
                }
                let elem = match elem {
                    Some(t) => t,
                    None => {
                        self.push(e("E003", *sp, "the type of an empty list is not known here; write it where a list is expected, or as `let x: list[T] = []`", "空のリストの型がここでは分かりません。リストを渡す先に書くか、`let x: list[T] = []` と書いてください"));
                        return None;
                    }
                };
                if matches!(elem.inner(), Ty::List(_)) {
                    self.push(e("E003", *sp, "a list of lists is not supported; put the inner list in a record", "リストのリストは書けません。内側のリストはレコードに入れてください"));
                    return None;
                }
                TExpr::List { items: out, ty: Ty::List(Box::new(elem)) }
            }
            Expr::Path(parts) => {
                if parts.len() == 1 && parts[0].0 == "none" && !self.var_ty.contains_key("none") {
                    match expected {
                        Some(t @ Ty::Opt(_)) => return Some(TExpr::None(t.clone())),
                        Some(Ty::Json) => return Some(TExpr::None(Ty::Opt(Box::new(Ty::Json)))),
                        _ => {
                            self.push(e("E003", parts[0].1, "`none` is given only where a value may be absent (`T?`)", "`none` を渡せるのは、オプショナルな値（`T?`）のところだけです"));
                            return None;
                        }
                    }
                }
                self.path(parts, expected)?
            }
        };
        if let Some(want) = expected {
            let got = te.ty();
            let fits = got.fits(want) || (matches!(te, TExpr::Int(_)) && matches!(want.inner(), Ty::Num(_)));
            if !fits {
                let (a, b) = (self.m.ty_name(want), self.m.ty_name(&got));
                let (en, ja) = if matches!(got, Ty::Opt(_)) && got.inner().fits(want) {
                    (
                        format!("expected `{a}` here, but this is `{b}`, which may be absent; `match` it with `none` and `some <name>` first"),
                        format!("ここには `{a}` が要りますが、これは `none` になりうる `{b}` です。先に `none` と `some <名前>` で `match` してください"),
                    )
                } else {
                    (format!("expected `{a}` here, but this is `{b}`"), format!("ここには `{a}` が要りますが、これは `{b}` です"))
                };
                self.push(e("E003", ex.span(), en, ja));
                return None;
            }
        }
        Some(te)
    }

    /// A variable and its fields: `pi.status`. A bare name that is no variable may be a value of the enum expected here.
    fn path(&mut self, parts: &[(String, Span)], expected: Option<&Ty>) -> Option<TExpr> {
        let (first, fsp) = &parts[0];
        match self.var_ty.get(first).cloned() {
            Some(mut t) => {
                let mut fields = Vec::new();
                for (f, fsp2) in &parts[1..] {
                    let so_far = std::iter::once(first.clone()).chain(fields.iter().cloned()).collect::<Vec<String>>().join(".");
                    let rec = match t {
                        Ty::Record(r) => r,
                        Ty::Opt(ref inner) if matches!(**inner, Ty::Record(_)) => {
                            self.push(e(
                                "E003",
                                *fsp2,
                                format!("`{so_far}` may be absent here; `match` it with `none` and `some <name>` first"),
                                format!("ここでは `{so_far}` が無いことがあります。先に `none` と `some <名前>` で `match` してください"),
                            ));
                            return None;
                        }
                        Ty::Json => {
                            self.push(e("E003", *fsp2, format!("`{so_far}` is `json`, which is carried as it is and not looked into"), format!("`{so_far}` は `json` で、中を見ずにそのまま運ぶ値です")));
                            return None;
                        }
                        ref other => {
                            let n = self.m.ty_name(other);
                            self.push(e("E003", *fsp2, format!("`{n}` has no fields"), format!("`{n}` にはフィールドがありません")));
                            return None;
                        }
                    };
                    match self.m.field_ty(rec, f) {
                        Some(ft) => {
                            t = ft.clone();
                            fields.push(f.clone());
                        }
                        None => {
                            let rn = self.m.records[rec].name.clone();
                            let list: Vec<String> = self.m.records[rec].fields.iter().map(|(n, _)| n.clone()).collect();
                            self.push(e("E002", *fsp2, format!("`{rn}` has no field `{f}` ({})", list.join(", ")), format!("`{rn}` にフィールド `{f}` はありません（{}）", list.join("・"))));
                            return None;
                        }
                    }
                }
                Some(TExpr::Var { name: first.clone(), fields, ty: t })
            }
            None => {
                if parts.len() == 1 {
                    if let Some(Ty::Enum(id)) = expected.map(|t| t.inner()) {
                        if self.m.enums[*id].values.contains(first) {
                            return Some(TExpr::Enum(first.clone(), *id));
                        }
                    }
                }
                let (hen, hja) = match expected.map(|t| t.inner()) {
                    Some(Ty::Enum(id)) => (
                        format!("the values of `{}` are {}", self.m.enums[*id].name, self.m.enums[*id].values.join(", ")),
                        format!("`{}` の値は {} です", self.m.enums[*id].name, self.m.enums[*id].values.join("・")),
                    ),
                    _ => ("a value is a variable, a field of one, a string, a number, true or false".to_string(), "値は変数・そのフィールド・文字列・数・true・false のどれかです".to_string()),
                };
                self.push(e("E002", *fsp, format!("there is no variable or value `{first}`"), format!("変数や値 `{first}` はありません")).note(hen, hja));
                None
            }
        }
    }

    /// The variables a `for … in parallel` round sets are its own. A name set inside such a
    /// round and also outside it would be one variable shared by rounds that run at the
    /// same time; that is refused. `locals` gets the names each round keeps.
    fn parallel_scopes(&mut self) {
        // for every name: the scopes that set it (the path of parallel loops around, by site)
        let mut sets: BTreeMap<String, BTreeSet<Vec<usize>>> = BTreeMap::new();
        fn put(sets: &mut BTreeMap<String, BTreeSet<Vec<usize>>>, n: &str, sc: &[usize]) {
            sets.entry(n.to_string()).or_default().insert(sc.to_vec());
        }
        fn visit(ss: &[TStmt], scope: &mut Vec<usize>, sets: &mut BTreeMap<String, BTreeSet<Vec<usize>>>) {
            for s in ss {
                match &s.kind {
                    TK::Call { target, handlers, .. } => {
                        if let Some(Target::Let(v)) = target {
                            put(sets, v, scope);
                        }
                        for h in handlers {
                            visit(&h.body, scope, sets);
                        }
                    }
                    TK::Assign { name, .. } => put(sets, name, scope),
                    TK::Match { arms, .. } => {
                        for a in arms {
                            if let Some(v) = &a.some {
                                put(sets, v, scope);
                            }
                            visit(&a.body, scope, sets);
                        }
                    }
                    TK::Repeat { body, .. } => visit(body, scope, sets),
                    TK::For { var, parallel, body, result, .. } => {
                        if let Some((r, _)) = result {
                            put(sets, r, scope);
                        }
                        if parallel.is_some() {
                            scope.push(s.site);
                            put(sets, var, scope);
                            visit(body, scope, sets);
                            scope.pop();
                        } else {
                            put(sets, var, scope);
                            visit(body, scope, sets);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut scope = Vec::new();
        visit(&self.m.flow, &mut scope, &mut sets);
        if let Some(f) = &self.m.on_failure {
            visit(f, &mut scope, &mut sets);
        }
        if let Some(f) = &self.m.on_cancel {
            visit(f, &mut scope, &mut sets);
        }
        // a name may be set in sibling rounds of different loops, but never in two scopes one of which holds the other
        let mut bad: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        for (n, scs) in &sets {
            let list: Vec<&Vec<usize>> = scs.iter().collect();
            for i in 0..list.len() {
                for j in 0..list.len() {
                    if i != j && list[j].len() > list[i].len() && list[j].starts_with(list[i]) {
                        bad.entry(*list[j].last().unwrap()).or_default().push(n.clone());
                    }
                }
            }
        }
        let mut lines: BTreeMap<usize, usize> = BTreeMap::new();
        Model::walk(&self.m.flow, &mut |s| {
            lines.insert(s.site, s.line);
        });
        for f in [&self.m.on_failure, &self.m.on_cancel].into_iter().flatten() {
            Model::walk(f, &mut |s| {
                lines.insert(s.site, s.line);
            });
        }
        for (site, mut names) in bad {
            names.sort();
            names.dedup();
            let line = lines.get(&site).cloned().unwrap_or(1);
            let list = names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ");
            self.push(Diag::error(
                "E009",
                line,
                1,
                format!("{list} is set both inside this `for … in parallel` and outside it; the rounds run at the same time and each keeps its own variables, so give them their own names"),
                format!("{list} は、この `for … in parallel` の中と外の両方で値を入れられています。イテレーションは同時に走り、それぞれが自分の変数を持つので、別の名前にしてください"),
            ));
        }
        // each parallel loop's own names
        let mut own: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        for (n, scs) in &sets {
            for sc in scs {
                if let Some(site) = sc.last() {
                    own.entry(*site).or_default().push(n.clone());
                }
            }
        }
        fn fill(ss: &mut [TStmt], own: &BTreeMap<usize, Vec<String>>) {
            for s in ss {
                let site = s.site;
                match &mut s.kind {
                    TK::Call { handlers, .. } => handlers.iter_mut().for_each(|h| fill(&mut h.body, own)),
                    TK::Match { arms, .. } => arms.iter_mut().for_each(|a| fill(&mut a.body, own)),
                    TK::Repeat { body, .. } => fill(body, own),
                    TK::For { body, locals, parallel, .. } => {
                        if parallel.is_some() {
                            *locals = own.get(&site).cloned().unwrap_or_default();
                        }
                        fill(body, own);
                    }
                    _ => {}
                }
            }
        }
        let mut flow = std::mem::take(&mut self.m.flow);
        fill(&mut flow, &own);
        self.m.flow = flow;
        if let Some(mut f) = self.m.on_failure.take() {
            fill(&mut f, &own);
            self.m.on_failure = Some(f);
        }
        if let Some(mut f) = self.m.on_cancel.take() {
            fill(&mut f, &own);
            self.m.on_cancel = Some(f);
        }
    }
}

/// `{id}` in a URL
pub fn placeholders(url: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = url;
    while let Some(i) = rest.find('{') {
        if let Some(j) = rest[i..].find('}') {
            out.push(rest[i + 1..i + j].to_string());
            rest = &rest[i + j + 1..];
        } else {
            break;
        }
    }
    out
}

/// Whether a value of type `t` can hold `json` somewhere inside.
fn has_json(m: &Model, t: &Ty, within: &mut Vec<RecordId>) -> bool {
    match t {
        Ty::Json => true,
        Ty::List(x) | Ty::Opt(x) => has_json(m, x, within),
        Ty::Record(r) => {
            if within.contains(r) {
                return false;
            }
            within.push(*r);
            let found = m.records[*r].fields.iter().any(|(_, ft)| has_json(m, ft, within));
            within.pop();
            found
        }
        _ => false,
    }
}
