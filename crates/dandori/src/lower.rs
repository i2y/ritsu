//! The first pass: read the rules through ritsu's port of rules, resolve every name, type every value,
//! and turn the statements into the checked tree of `model`. The run-dependent checks
//! (states of cases, assignment, exhaustiveness, exits) are the second pass, in `flow`.

use crate::apis::{ApiDoc, ApiKind, MadeTy};
use crate::diag::{Diag, Text};
use crate::model::*;
use crate::proto::ProtoFile;
use crate::rulec::{self, RType};
use crate::syntax::{self, Block, Call, Expr, MachineUse, Part, Program, RangeDecl, Span, StmtKind, TypeExpr};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn type_hint() -> Text {
    tr!(
        "型は int・string・bool・timestamp・date・json・money[円, incl_tax] のような単位・列挙・レコード・list[T]・T? のどれかです",
        "a type is int, string, bool, timestamp, date, json, a unit such as money[円, incl_tax], an enum, a record, list[T] or T?"
    )
}

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
    /// the dates files `use dates` reads, by the name it gives
    date_files: BTreeMap<String, ritsu_ports::DateFacts>,
    /// the books `use book` reads, by the name it gives
    book_ix: BTreeMap<String, usize>,
    /// the life of the holds of each transfer that holds, by book and transfer: the rule it is
    hold_ix: BTreeMap<(String, String), usize>,
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

fn e(code: &'static str, sp: Span, message: Text) -> Diag {
    Diag::error(code, sp.line, sp.col, message)
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
                vec![Diag::error("E001", 1, 1, tr!(".flow は `workflow <名前> v<番号>` で始めます", "a .flow starts with `workflow <name> v<n>`"))],
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
            books: vec![],
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
        date_files: BTreeMap::new(),
        book_ix: BTreeMap::new(),
        hold_ix: BTreeMap::new(),
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
    lw.dates(base);
    lw.books(base);
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
        lw.diags.push(Diag::error("E009", 1, 1, tr!("ワークフローには `flow` が要ります", "a workflow needs a `flow`")));
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
    lw.names_around_rules();
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
                self.push(e("E006", *sp, tr!("API `{name}` が二度読み込まれています", "the API `{name}` is used twice")));
                continue;
            }
            // `<name>.<type>` would not say which it means
            if self.rule_ix.contains_key(name) {
                self.push(e("E006", *sp, tr!("`{name}` は規則と API の両方の名前です。別の名前を付けてください", "`{name}` names both a rule and an API; give them different names")));
                continue;
            }
            self.use_spans.insert(name.clone(), *sp);
            match crate::apis::load(u.kind, &base.join(&u.path)) {
                Ok(doc) => {
                    self.apis.insert(name.clone(), (u.kind, crate::apis::Api { name: name.clone(), doc, url: u.url.clone() }));
                }
                Err(msg) => self.push(e("E016", *sp, tr!("API `{}` を読めませんでした", "could not read the API `{}`", u.path)).note(Text::same(msg))),
            }
        }
    }

    /// The API a binding names, which must be of `kind`.
    fn api(&mut self, (name, sp): &syntax::Name, kind: crate::apis::ApiKind, what: Text) -> Option<crate::apis::Api> {
        match self.apis.get(name) {
            Some((k, a)) if *k == kind => Some(a.clone()),
            Some((k, _)) => {
                let k = k.word();
                self.push(e("E016", *sp, tr!("`{name}` は `use {k}` で読んだものですが、{}", "`{name}` is described by `use {k}`, and {}", what.ja; what.en)));
                None
            }
            None => {
                self.push(e("E002", *sp, tr!("API `{name}` はありません。`use {}` で読んだものを書きます", "there is no API `{name}`; name one read by `use {}`", kind.word())));
                None
            }
        }
    }

    fn rules(&mut self, base: &Path) {
        for u in &self.prog.uses {
            let (name, sp) = &u.name;
            if self.rule_ix.contains_key(name) {
                self.push(e("E006", *sp, tr!("規則 `{name}` が二度読み込まれています", "the rule `{name}` is used twice")));
                continue;
            }
            let path = base.join(&u.path);
            let info = match rulec::load(&path) {
                Ok(i) => i,
                Err(said) => {
                    // what rulec says of the rule instead, or what the port this dandori runs with says
                    let d = e("E005", *sp, tr!("規則 `{}` を読めませんでした", "could not read the rule `{}`", u.path));
                    self.push(crate::sources::said_notes(&said).into_iter().fold(d, Diag::note));
                    continue;
                }
            };
            // a rule that walks a list takes it as an input of elements, which no type of dandori's is, at
            // its service as in the code that goes with the workflow
            if let Some(list) = &info.walks {
                self.push(
                    e("E005", *sp, tr!("規則 `{}` を読めませんでした", "could not read the rule `{}`", u.path)).note(
                        tr!("この規則は要素の並び（`{list}`）をたどります。dandori はまだ、規則に並びを渡せません", "the rule walks a list of elements (`{list}`), and dandori does not pass a rule a list yet"),
                    ),
                );
                continue;
            }
            // a rule an input or an output of which may be `none`, which the code that goes with the
            // workflow neither passes to the rule's own nor reads from it yet
            if let Some(name) = info.optional.first() {
                self.push(
                    e("E005", *sp, tr!("規則 `{}` を読めませんでした", "could not read the rule `{}`", u.path)).note(
                        tr!("この規則の `{name}` は無いことがあります（`T?`）。dandori はまだ、無いことがある値を規則とやりとりできません", "the rule's `{name}` may be none (`T?`), and dandori does not pass a rule, or read from one, a value that may be absent yet"),
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
                kind: RuleKind::Rule,
            });
        }
    }

    /// Whether a name `use` gives is taken already: by a rule, a dates file or a book.
    fn used_name(&mut self, name: &str, sp: Span) -> bool {
        if self.rule_ix.contains_key(name) || self.date_files.contains_key(name) || self.book_ix.contains_key(name) {
            self.push(e("E006", sp, tr!("`{name}` が二度読み込まれています（規則と日付のファイルと帳簿は名前を共有します）", "`{name}` is used twice (rules, dates files and books share names)")));
            return true;
        }
        false
    }

    /// The dates files (`use dates terms from "…"`), read through the port of dates (koyomi's
    /// answer): each date of a file is called as a rule is, by `<file>.<date>`, and answers its day,
    /// and its time when the date turns into one (`at`). E005 when koyomi says the file does not pass
    /// its check, or this dandori reads no dates file.
    fn dates(&mut self, base: &Path) {
        for u in &self.prog.dates {
            let (name, sp) = &u.name;
            if self.used_name(name, *sp) {
                continue;
            }
            let path = base.join(&u.path);
            let facts = match crate::sources::dates(&path) {
                Ok(f) => f,
                Err(said) => {
                    let d = e("E005", *sp, tr!("日付のファイル `{}` を読めませんでした", "could not read the dates file `{}`", u.path));
                    self.push(crate::sources::said_notes_from(&said, base).into_iter().fold(d, Diag::note));
                    continue;
                }
            };
            self.use_spans.insert(name.clone(), *sp);
            for f in &facts.functions {
                let callee = format!("{name}.{}", f.name);
                let ix = self.m.rules.len();
                let params: Vec<(String, String, bool)> = f
                    .params
                    .iter()
                    .filter_map(|p| facts.inputs.iter().find(|i| i.name == *p))
                    .map(|i| (i.name.clone(), i.alias.clone(), i.kind == ritsu_ports::DateKind::Date))
                    .collect();
                let mut fields = vec![("day".to_string(), Ty::Date)];
                if f.at.is_some() {
                    fields.push(("at".to_string(), Ty::Timestamp));
                }
                let rec = self.m.records.len();
                self.m.records.push(RecordDef { name: callee.clone(), fields, ranges: BTreeMap::new(), origin: RecordOrigin::RuleOutputs(ix) });
                let number = ritsu_units::Unit::parse("number").ok();
                let column = |(n, a, day): &(String, String, bool)| rulec::Column {
                    name: n.clone(),
                    alias: a.clone(),
                    ty: if *day {
                        RType::Date
                    } else {
                        let i = facts.inputs.iter().find(|i| i.name == *n);
                        RType::Num { unit: number.clone().unwrap_or_else(|| ritsu_units::Unit::count("number")), min: i.map(|i| i.min), max: i.map(|i| i.max) }
                    },
                };
                let call = |module: String, function: String| ritsu_ports::Call {
                    module,
                    function,
                    input_type: String::new(),
                    params: params.iter().map(|(n, a, _)| ritsu_ports::Param { name: n.clone(), alias: a.clone(), ty: String::new(), optional: false }).collect(),
                    outputs: vec![],
                    enums: vec![],
                };
                // the names koyomi's generated code gives the file and the date (koyomi's `api`): the
                // alias in TypeScript and Python, and Go's package and exported name (ritsu-emit's)
                let info = rulec::RuleInfo {
                    rule: facts.name.clone(),
                    version: facts.version.clone(),
                    path: path.clone(),
                    sha256: facts.sha256.clone(),
                    inputs: params.iter().map(column).collect(),
                    outputs: vec![],
                    enums: vec![],
                    machine: None,
                    preconditions: vec![],
                    connect: None,
                    typescript: call(facts.alias.clone(), f.alias.clone()),
                    python: call(facts.alias.clone(), f.alias.clone()),
                    go: call(ritsu_emit::ident::go_package(&facts.alias), ritsu_emit::ident::pascal(&f.alias)),
                    walks: None,
                    optional: vec![],
                };
                self.rule_ix.insert(callee.clone(), ix);
                self.m.rules.push(RuleUse {
                    name: callee,
                    info,
                    lambda: u.lambda.as_ref().map(|(f, _)| f.clone()),
                    local: u.local,
                    connect: None,
                    connection: None,
                    outputs: rec,
                    line: sp.line,
                    kind: RuleKind::Date(DateCall {
                        file: name.clone(),
                        date: f.name.clone(),
                        file_alias: facts.alias.clone(),
                        alias: f.alias.clone(),
                        params: params.clone(),
                        at: f.at.is_some(),
                        offset: facts.calendar.as_ref().and_then(|c| c.offset),
                    }),
                });
            }
            self.date_files.insert(name.clone(), facts);
        }
    }

    /// The books (`use book stock from "…"`), read through the port of books (chobo's answer). Each
    /// transfer that holds gives a type, `<book>.<transfer>`: a record of the parameters of its key and
    /// its state, which a task that holds, posts or voids answers, and a case that follows the hold is
    /// held in; and the life of its holds, as a machine a case follows (chobo's, as a rule's is).
    fn books(&mut self, base: &Path) {
        for u in &self.prog.books {
            let (name, sp) = &u.name;
            if self.used_name(name, *sp) {
                continue;
            }
            let path = base.join(&u.path);
            let facts = match crate::sources::book(&path) {
                Ok(f) => f,
                Err(said) => {
                    let d = e("E005", *sp, tr!("帳簿 `{}` を読めませんでした", "could not read the book `{}`", u.path));
                    self.push(crate::sources::said_notes_from(&said, base).into_iter().fold(d, Diag::note));
                    continue;
                }
            };
            self.use_spans.insert(name.clone(), *sp);
            let bix = self.m.books.len();
            for t in &facts.transfers {
                let Some(mc) = &t.machine else { continue };
                let rname = format!("{name}.{}", t.name);
                let state = self.m.enums.len();
                self.enum_ix.insert(format!("{rname}.{}", mc.state_enum), state);
                self.m.enums.push(EnumDef { name: format!("{rname}.{}", mc.state_enum), values: mc.states.clone() });
                // the key's parameters, in the order the transfer declares them: what a post and a void take
                let mut fields: Vec<(String, Ty)> = t.params.iter().filter(|p| t.key.contains(&p.name)).map(|p| (p.name.clone(), Ty::Str)).collect();
                fields.push(("state".to_string(), Ty::Enum(state)));
                let ix = self.m.rules.len();
                let rec = self.m.records.len();
                self.record_ix.insert(rname.clone(), rec);
                self.m.records.push(RecordDef { name: rname.clone(), fields, ranges: BTreeMap::new(), origin: RecordOrigin::Hold { book: bix, transfer: t.name.clone() } });
                let none = ritsu_ports::Call { module: String::new(), function: String::new(), input_type: String::new(), params: vec![], outputs: vec![], enums: vec![] };
                let info = rulec::RuleInfo {
                    rule: facts.name.clone(),
                    version: format!("v{}", facts.version),
                    path: path.clone(),
                    sha256: facts.sha256.clone(),
                    inputs: vec![],
                    outputs: vec![],
                    enums: vec![(mc.state_enum.clone(), mc.states.clone())],
                    machine: Some(rulec::machine(mc.clone())),
                    preconditions: vec![],
                    connect: None,
                    typescript: none.clone(),
                    python: none.clone(),
                    go: none,
                    walks: None,
                    optional: vec![],
                };
                self.hold_ix.insert((name.clone(), t.name.clone()), ix);
                self.m.rules.push(RuleUse { name: rname, info, lambda: None, local: false, connect: None, connection: None, outputs: rec, line: sp.line, kind: RuleKind::Hold { book: bix, transfer: t.name.clone() } });
            }
            self.book_ix.insert(name.clone(), bix);
            self.m.books.push(BookUse { name: name.clone(), path, facts, lambda: u.lambda.as_ref().map(|(f, _)| f.clone()), line: sp.line });
        }
    }

    /// The type of a parameter of a book's transfer: a string, or an amount in a unit of the book,
    /// as dandori types it: a count with a name and nothing else (`unit pcs`) is a whole number,
    /// `int`, which dandori has no other spelling for; money and a quantity of the table are a number
    /// with that unit (`unit 円 incl_tax` is `money[円, incl_tax]`), which a value is given in as it
    /// is, and never converted (P1).
    fn book_param_ty(book: &BookUse, p: &ritsu_ports::TransferParam) -> Ty {
        match p.unit.as_deref().and_then(|u| book.facts.unit(u)) {
            None if p.unit.is_none() => Ty::Str,
            Some(u) if !matches!(u.dim, ritsu_units::Dim::Count(_) | ritsu_units::Dim::Number) => Ty::Num(u.clone()),
            _ => Ty::Int,
        }
    }

    /// What `connect` under `use rule` says: the service's URL, and what rulec says of its request
    /// and its response. The ways of calling that do not go together are E007, each at the second
    /// to say it.
    fn rule_connect(&mut self, u: &syntax::UseRule, info: &rulec::RuleInfo) -> Option<RuleConnect> {
        if let (Some((_, lsp)), Some((_, csp))) = (&u.lambda, &u.connect) {
            let (first, second) = if (lsp.line, lsp.col) < (csp.line, csp.col) { (lsp, csp) } else { (csp, lsp) };
            self.push(e(
                "E007",
                *second,
                tr!("この規則の呼び出し方はもう書かれています（{} 行目）。規則の呼び出し方は `lambda` か `connect` のどちらか一つです", "the rule is already called another way (line {}); a rule is called by `lambda` or by `connect`", first.line),
            ));
        }
        if let (Some((_, nsp)), None) = (&u.connection, &u.connect) {
            self.push(e(
                "E007",
                *nsp,
                tr!("`connection` は `connect` で呼ぶ規則に書きます。`lambda` の規則は、Step Functions が接続なしで呼びます", "`connection` is for a rule called by `connect`; Step Functions invokes a rule's `lambda` without one"),
            ));
        }
        let (url, csp) = u.connect.as_ref()?;
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            self.push(e(
                "E007",
                *csp,
                tr!("`connect` には、規則のサービスの場所を http:// か https:// で書きます", "`connect` says where the rule's service is, with http:// or https://"),
            ));
            return None;
        }
        match rulec::connect_shape(info) {
            Ok(shape) => {
                let zeros = rule_zeros(&shape.response);
                let base = url.trim_end_matches('/').to_string();
                Some(RuleConnect { url: format!("{base}{}", shape.path), base, request: shape.request, response: shape.response, zeros })
            }
            Err(why) => {
                // the rule is read, but its service is not: what `connect` calls cannot be said
                self.push(e("E005", u.name.1, tr!("規則 `{}` を読めませんでした", "could not read the rule `{}`", u.path)).note(why));
                None
            }
        }
    }

    fn rty(&self, rule: &str, t: &RType) -> Ty {
        match t {
            RType::Bool => Ty::Bool,
            RType::Str => Ty::Str,
            RType::Date => Ty::Date,
            RType::Enum(n) => Ty::Enum(*self.enum_ix.get(&format!("{rule}.{n}")).expect("rule enums are registered first")),
            // rulec's `number` has no unit: it is an integer like dandori's `int`
            RType::Num { unit, .. } if unit.dim == ritsu_units::Dim::Number => Ty::Int,
            RType::Num { unit, .. } => Ty::Num(unit.clone()),
        }
    }

    fn local_types(&mut self) {
        for en in &self.prog.enums {
            let (name, sp) = &en.name;
            if self.enum_ix.contains_key(name) || self.prog.records.iter().any(|r| r.name.0 == *name) {
                self.push(e("E006", *sp, tr!("`{name}` が二度宣言されています", "`{name}` is declared twice")));
                continue;
            }
            let mut values: Vec<String> = Vec::new();
            for (v, vsp) in &en.values {
                if v == "none" {
                    self.push(e("E006", *vsp, tr!("`none` は始まっていない案件を表す語なので、値には別の名前を付けてください", "`none` is kept for a case that has not started; name the value otherwise")));
                } else if values.contains(v) {
                    self.push(e("E006", *vsp, tr!("値 `{v}` が二度書かれています", "the value `{v}` is written twice")));
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
                self.push(e("E006", *sp, tr!("`{name}` が二度宣言されています", "`{name}` is declared twice")));
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
                    self.push(e("E006", f.name.1, tr!("フィールド `{}` が二度書かれています", "the field `{}` is written twice", f.name.0)));
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
            TypeExpr::Date(_) => Some(Ty::Date),
            TypeExpr::Json(_) => Some(Ty::Json),
            TypeExpr::List(inner, sp) => {
                let t = self.ty(inner)?;
                if matches!(t.inner(), Ty::List(_)) {
                    self.push(e("E003", *sp, tr!("リストのリストは書けません。内側のリストはレコードに入れてください", "a list of lists is not supported; put the inner list in a record")));
                    return None;
                }
                Some(Ty::List(Box::new(t)))
            }
            TypeExpr::Opt(inner, _) => Some(Ty::Opt(Box::new(self.ty(inner)?))),
            // a unit as rulec spells it, read by the one table of units (ritsu-units): `money[JPY,
            // incl_tax]`, `mass[kg]`, `rate[step 0.1%]`
            TypeExpr::Unit(u, sp) => match ritsu_units::Unit::parse(u) {
                Ok(unit) if !matches!(unit.dim, ritsu_units::Dim::Number | ritsu_units::Dim::Count(_)) => Some(Ty::Num(unit)),
                Ok(_) | Err(ritsu_units::Problem::Shape(_)) | Err(ritsu_units::Problem::Dimension(_)) => {
                    self.push(e("E002", *sp, tr!("型 `{u}` はありません", "there is no type `{u}`")).note(type_hint()));
                    None
                }
                Err(why) => {
                    self.push(e("E002", *sp, tr!("型 `{u}` はありません", "there is no type `{u}`")).note(why.text()));
                    None
                }
            },
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
                                let Text { en: what_en, ja: what_ja } = match kind {
                                    ApiKind::OpenApi => tr!("OpenAPI の記述", "an OpenAPI document"),
                                    _ => tr!("Smithy のモデル", "a Smithy model"),
                                };
                                let Text { en: shapes_en, ja: shapes_ja } = match kind {
                                    ApiKind::OpenApi => tr!("スキーマ", "schemas"),
                                    _ => Text::new("shape", "shapes"),
                                };
                                self.push(e("E002", sp, tr!("型 `{text}` はありません", "there is no type `{text}`")).note(
                                    tr!("型を作れるのは、`use proto` で読んだ `.proto` からです。`{api}` は {what_ja}で、その{shapes_ja}からは型を作りません", "types are made from a `.proto` read by `use proto`; `{api}` is {what_en}, and its {shapes_en} are not made into types"),
                                ));
                                None
                            }
                        };
                    }
                }
                let hint = if parts.len() == 2 && self.rule_ix.contains_key(&parts[0].0) {
                    let r = &self.m.rules[self.rule_ix[&parts[0].0]];
                    let names: Vec<String> = r.info.enums.iter().map(|(n, _)| n.clone()).collect();
                    tr!("`{}` の列挙は {} です", "the enums of `{}` are {}", parts[0].0, names.join("・"); parts[0].0, names.join(", "))
                } else {
                    type_hint()
                };
                self.push(e("E002", sp, tr!("型 `{text}` はありません", "there is no type `{text}`")).note(hint));
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // Types made from a `.proto` (DESIGN 1.12)

    /// What to say of the files a `.proto` imports and no file was read for, when the API is one.
    fn unread_note_of(api: &crate::apis::Api) -> Option<Text> {
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
                    tr!("`{text}` は `{api}` のサービスで、メッセージでも列挙でもありません", "`{text}` is a service of `{api}`, not a message or an enum"),
                ));
                return None;
            }
            let own = format!("{}.", pf.package);
            let mut names: Vec<String> = pf.messages.keys().chain(pf.enums.keys()).map(|f| if pf.package.is_empty() { f.clone() } else { f.strip_prefix(&own).unwrap_or(f).to_string() }).collect();
            names.sort();
            names.dedup();
            let more = names.len().saturating_sub(20);
            names.truncate(20);
            let mut list = Text::new(names.join("・"), names.join(", "));
            if more > 0 {
                list = list.then(&tr!("…ほか {more} 個", ", and {more} more"));
            }
            let mut d = e("E002", sp, tr!("型 `{text}` はありません", "there is no type `{text}`")).note(tr!("`{api}` のメッセージと列挙は {} です", "the messages and enums of `{api}` are {}", list.ja; list.en));
            // a file it may be in was not read
            if let Some(note) = crate::apis::unread_note(&pf) {
                d = d.note(note);
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
                tr!("`{name}` の値 `none` は、`match` では値が無いことを表す語として読まれます。代わりに自分で列挙を書いてください", "the value `none` of `{name}` would be taken for an absent value in a `match`; write an enum of your own for it"),
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
                tr!("`{record}` を作れません。フィールド `{field}` の型 `{ty}` が分かりません。レコードを自分で書き、`{field}` は `json` にしてください", "`{record}` cannot be made: its field `{field}` is of the type `{ty}`, which is not known; write the record yourself, with `json` for `{field}`"),
            );
            // the `.proto` the record is of, by the API it was made through
            if let Some(note) = record.split('.').next().and_then(|api| self.apis.get(api)).and_then(|(_, a)| match &a.doc {
                ApiDoc::Proto(pf) => crate::apis::unread_note(pf),
                _ => None,
            }) {
                d = d.note(note);
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
                    tr!("`{name}` は、フィールド `{through}` を通して自分自身を含んでいます。レコードは自分自身を含められないので、自分を含むところを `json` にしたレコードを自分で書いてください", "`{name}` contains itself, through `{through}`; a record cannot, so write the record yourself, with `json` where it holds itself"),
                ));
            } else {
                // the type of the field, where the flow wrote it
                let at = self.prog.records.iter().find(|d| d.name.0 == name).and_then(|d| d.fields.iter().find(|f| f.name.0 == through).map(|f| f.ty.span()).or(Some(d.name.1))).unwrap_or(Span { line: 1, col: 1 });
                self.push(e(
                    "E003",
                    at,
                    tr!("`{name}` は、フィールド `{through}` を通して自分自身を含んでいます。レコードは自分自身を含められないので、自分を含むフィールドの型は `json` にしてください", "`{name}` contains itself, through `{through}`; a record cannot, so give the field that holds itself the type `json`"),
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
                tr!("`{a}` と `{b}` は、生成するコードで同じ名前（`{generated}`）になります。`{rename}` の名前を変えてください", "`{a}` and `{b}` would have one name in the code dandori writes, `{generated}`; rename `{rename}`"),
            ));
        }
    }

    /// E006 for the names of the rules a flow calls that would clash in the code dandori writes
    /// (DESIGN 1.15). The code rulec generates for a rule goes by the rule's alias (its function and
    /// module, or its Go package) and its enums' aliases, and the files dandori writes around it
    /// import those names beside their own: each file's own are its `AROUND_RULES`. Two rules of one
    /// module would be written over each other. And every task and every rule called is an
    /// activity, by its name and `rule_<rule>`. Only the rules whose code goes with the workflow
    /// (called, and not at a Connect service) have their names imported.
    fn names_around_rules(&mut self) {
        let m = &self.m;
        let called: BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| if let TK::Call { callee: Callee::Rule(r), .. } = &s.kind { Some(*r) } else { None }).collect();
        let bundled: Vec<usize> = called.iter().copied().filter(|r| m.rules[*r].connect.is_none()).collect();
        let at = |n: &str| self.use_spans.get(n).copied().unwrap_or(Span { line: 1, col: 1 });
        let mut out: Vec<Diag> = Vec::new();
        let activities: BTreeMap<String, &str> = bundled.iter().map(|r| (crate::render::rule_activity(&m.rules[*r].name), m.rules[*r].name.as_str())).collect();
        let fix_rule = tr!("規則のファイルで規則の別名を変えてください（`rule <名前>(<別名>) v1`。名前はそのままで構いません）", "Change the rule's alias in its file (`rule <name>(<alias>) v1`); its name can stay as it is.");
        let fix_enum = tr!("規則のファイルで列挙の別名を変えてください（`enum <名前>(<別名>) = …`）", "Change the enum's alias in the rule's file (`enum <name>(<alias>) = …`).");
        // the names of rulec's code (a date of a dates file goes by koyomi's, which `use dates` checks)
        for r in bundled.iter().filter(|r| m.rules[**r].is_rule()) {
            let ru = &m.rules[*r];
            let (n, info) = (ru.name.as_str(), &ru.info);
            // each name of the rule's generated code, with the places it would clash and what it is
            let mut clash: Vec<(String, Option<String>, Vec<Text>)> = Vec::new();
            let mut add = |name: &str, en: Option<&str>, place: Text| match clash.iter_mut().find(|(x, e, _)| x == name && e.as_deref() == en) {
                Some((_, _, ps)) => ps.push(place),
                None => clash.push((name.to_string(), en.map(String::from), vec![place])),
            };
            if crate::temporal::AROUND_RULES.contains(&info.typescript.function.as_str()) {
                add(&info.typescript.function, None, tr!("Temporal の TypeScript と Argo の rules.ts", "rules.ts (Temporal's TypeScript, Argo)"));
            }
            // what the Python of rules.py and of the Lambda function imports: the function, and the enums it takes
            let py = &info.python;
            let mut py_names: Vec<(&str, Option<&str>)> = vec![(py.function.as_str(), None)];
            for p in &py.params {
                if let Some(e) = py.enums.iter().find(|e| e.alias == p.ty) {
                    if !py_names.iter().any(|(x, _)| *x == p.ty) {
                        py_names.push((p.ty.as_str(), Some(e.name.as_str())));
                    }
                }
            }
            for (name, en) in &py_names {
                if crate::temporal_py::AROUND_RULES.contains(name) {
                    add(name, *en, tr!("Temporal の Python と pydantic-graph の rules.py", "rules.py (Temporal's Python, pydantic-graph)"));
                }
                if crate::asl::AROUND_RULE.contains(name) {
                    add(name, *en, tr!("Step Functions と Lambda durable functions の Lambda の関数", "the Lambda function (Step Functions, Lambda durable functions)"));
                }
                if let Some(owner) = activities.get(*name) {
                    // an activity of rules.py would take the place of what it imports: the one to rename is the rule `use rule` named so
                    let act = name.to_string();
                    out.push(
                        e("E006", at(owner), tr!("規則 `{owner}` のアクティビティ `{act}` は、rulec が規則 `{n}` のために生成するコードの名前でもあり、rules.py で二つがぶつかります", "`{act}`, the activity of the rule `{owner}`, is also a name of the code rulec generates for the rule `{n}`, and the two clash in rules.py"))
                            .note(tr!("`use rule` で付けた名前 `{owner}` を変えてください", "Give the rule another name than `{owner}` in `use rule`.")),
                    );
                }
            }
            let go = &info.go;
            let takes_enum = go.params.iter().any(|p| go.enums.iter().any(|e| e.alias == p.ty));
            if crate::temporal_go::AROUND_RULES.contains(&go.module.as_str()) || (takes_enum && go.module == "ok") {
                add(&go.module, None, tr!("Temporal の Go の rules.go（Go のパッケージとして）", "rules.go (Temporal's Go), as a Go package"));
            }
            for (name, en, places) in clash {
                let ja: Vec<String> = places.iter().map(|p| p.ja.clone()).collect();
                let en_places: Vec<String> = places.iter().map(|p| p.en.clone()).collect();
                let (message, fix) = match en.as_deref() {
                    None => (tr!("規則 `{n}` は、rulec が生成するコードで `{name}` という名前になり、dandori がそのまわりに書くコードの名前とぶつかります", "the rule `{n}` is `{name}` in the code rulec generates, a name the code dandori writes around it uses already"), fix_rule.clone()),
                    Some(en) => (tr!("規則 `{n}` の列挙 `{en}` は、rulec が生成するコードで `{name}` という名前になり、dandori がそのまわりに書くコードの名前とぶつかります", "the enum `{en}` of the rule `{n}` is `{name}` in the code rulec generates, a name the code dandori writes around it uses already"), fix_enum.clone()),
                };
                out.push(e("E006", at(n), message).note(tr!("ぶつかるところ：{}", "Where: {}", ja.join("、"); en_places.join("; "))).note(fix));
            }
        }
        // two rules of one module: `rulec gen` writes them over each other
        for (i, a) in bundled.iter().enumerate() {
            for b in &bundled[..i] {
                let (ra, rb) = (&m.rules[*a], &m.rules[*b]);
                if ra.info.sha256 == rb.info.sha256 {
                    continue;
                }
                let ts = |x: &RuleUse| x.info.typescript.module.trim_end_matches(".ts").to_string();
                let module = [(ts(ra), ts(rb)), (ra.info.python.module.clone(), rb.info.python.module.clone()), (ra.info.go.module.clone(), rb.info.go.module.clone())].into_iter().find(|(x, y)| x == y).map(|(x, _)| x);
                if let Some(module) = module {
                    let (an, bn) = (ra.name.as_str(), rb.name.as_str());
                    out.push(
                        e("E006", at(an), tr!("規則 `{bn}` と `{an}` は別の規則ですが、rulec が生成するコードでは同じモジュール `{module}` になり、片方がもう片方を上書きします", "the rules `{bn}` and `{an}` are two rules, but one module, `{module}`, in the code rulec generates: one would be written over the other"))
                            .note(tr!("どちらかの規則の別名を変えてください", "Change the alias of one of them.")),
                    );
                }
            }
        }
        // every task, and every rule called, is an activity
        let mut acts: BTreeMap<String, &str> = BTreeMap::new();
        for r in &called {
            let n = m.rules[*r].name.as_str();
            let act = crate::render::rule_activity(n);
            match acts.get(&act) {
                Some(first) => out.push(e("E006", at(n), tr!("規則 `{first}` と `{n}` は、dandori が書くコードで同じアクティビティ `{act}` になります。どちらかの名前を変えてください", "the rules `{first}` and `{n}` would be one activity, `{act}`, in the code dandori writes; rename one"))),
                None => {
                    acts.insert(act, n);
                }
            }
        }
        for t in &self.prog.tasks {
            let (tn, sp) = (&t.name.0, t.name.1);
            let owner = acts.get(tn.as_str()).or_else(|| acts.get(&crate::render::ident(tn)));
            if let Some(n) = owner {
                let act = crate::render::rule_activity(n);
                out.push(
                    e("E006", sp, tr!("タスク `{tn}` と規則 `{n}` は、dandori が書くコードで同じアクティビティ `{act}` になります", "the task `{tn}` and the rule `{n}` would be one activity, `{act}`, in the code dandori writes"))
                        .note(tr!("タスクの名前を変えてください。`rule_` で始まる名前は、規則のアクティビティのものです", "Rename the task: a name that starts with `rule_` is a rule's activity.")),
                );
            }
        }
        for d in out {
            self.push(d);
        }
    }

    /// The `.flow` a task runs as its child, checked on its own (E015 when it cannot be).
    fn child_flow(&mut self, path: &str, sp: Span) -> Option<Box<ChildFlow>> {
        use crate::check::ChildTrouble;
        let (message, note) = match crate::check::child(&self.dir.join(path)) {
            Ok(model) => return Some(Box::new(ChildFlow { path: path.to_string(), model })),
            Err(ChildTrouble::Cycle(chain)) => {
                let names: Vec<String> = chain.iter().map(|p| p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()).collect();
                (
                    tr!("`{path}` はこのフローをまた走らせます（{}）。フローは自分を、直接にもほかのフローを通しても走らせられません", "`{path}` runs this flow again ({}); a flow cannot run itself, directly or through others", names.join(" → ")),
                    None,
                )
            }
            Err(ChildTrouble::Unreadable(msg)) => (tr!("`{path}` を読めませんでした", "could not read `{path}`"), Some(Text::same(msg))),
            Err(ChildTrouble::Refused(diags)) => (
                tr!("`{path}` は検査を通りません", "`{path}` does not pass check"),
                diags.first().map(|d| tr!("{} 行目：{}（{}）", "line {}: {} ({})", d.line, d.ja, d.code; d.line, d.en, d.code)),
            ),
        };
        let mut d = e("E015", sp, message);
        if let Some(note) = note {
            d = d.note(note);
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
            self.push(e("E003", r.span, tr!("範囲を書けるのは数（`int` か単位の付いた数）で、これは `{n}` です", "a range is for a number (`int` or a unit), and this is `{n}`")));
            return None;
        }
        let lo = match &r.lo {
            Some(b) => Some(self.bound(inner, b)?),
            None => None,
        };
        let hi = match &r.hi {
            Some(b) => Some(self.bound(inner, b)?),
            None => None,
        };
        let rg = Range { lo, hi };
        if let (Some(lo), Some(hi)) = (rg.lo, rg.hi) {
            if lo > hi {
                self.push(e("E003", r.span, tr!("`{}` に入る数はありません", "no number is in `{}`", rg.show())));
                return None;
            }
        }
        Some(rg)
    }

    /// One end of a range, counted in the unit of the number `ty`: a whole number as it is; a number
    /// with a unit, as rulec counts one (ritsu-units: `1kg` is 1000 in `mass[g]`, `5%` is 50 steps of
    /// `rate[step 0.1%]`, an amount in yen is the same in `money[円, incl_tax]` and in
    /// `money[JPY, excl_tax]`). E003 for a unit of another dimension, a unit on `int`, and an end that
    /// does not come to a whole number in the type's unit, which is what goes on the wire.
    fn bound(&mut self, ty: &Ty, b: &syntax::Bound) -> Option<i64> {
        let (value, unit, raw, sp) = match b {
            syntax::Bound::Int(n) => return Some(*n),
            syntax::Bound::Quantity { value, unit, raw, span } => (*value, unit.as_str(), raw.as_str(), *span),
        };
        let tn = self.m.ty_name(ty);
        let not_of = tr!("範囲の端 `{raw}` は `{tn}` の値ではありません", "the bound `{raw}` is not a value of `{tn}`");
        let Ty::Num(to) = ty else {
            self.push(e("E003", sp, not_of).note(tr!("`int` の端は、単位を付けずに書きます", "a bound of `int` is written without a unit")));
            return None;
        };
        let counted = match &to.dim {
            // a rate's end in percent, counted in its steps
            ritsu_units::Dim::Rate if unit == "%" => to.step.and_then(|step| value.checked_div(ritsu_units::Rat::int(100))?.checked_div(step)),
            ritsu_units::Dim::Money(_) => ritsu_units::Unit::money(unit, to.tax).and_then(|from| from.convert(value, to)),
            d => ritsu_units::Unit::quantity(d.word(), unit).and_then(|from| from.convert(value, to)),
        };
        let Some(counted) = counted else {
            self.push(e("E003", sp, not_of));
            return None;
        };
        match Some(counted).filter(|r| r.is_int()).and_then(|r| i64::try_from(r.num).ok()) {
            Some(n) => Some(n),
            None => {
                self.push(
                    e("E003", sp, tr!("範囲の端 `{raw}` を `{tn}` で数えると、整数になりません", "the bound `{raw}`, counted in `{tn}`, is not a whole number")).note(tr!(
                        "値は型の単位で数えた整数で運ぶので、端も換算して整数にならなければなりません",
                        "a value travels as a whole number in the unit of its type, so a bound must come to one too"
                    )),
                );
                None
            }
        }
    }

    fn io(&mut self) {
        let mut seen: Vec<String> = vec![];
        for f in &self.prog.inputs {
            if seen.contains(&f.name.0) {
                self.push(e("E006", f.name.1, tr!("入力 `{}` が二度書かれています", "the input `{}` is written twice", f.name.0)));
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
                self.push(e("E006", f.name.1, tr!("出力 `{}` が二度書かれています", "the output `{}` is written twice", f.name.0)));
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
                self.push(e("E006", *sp, tr!("`{name}` が二度宣言されています（タスクと規則は名前を共有します）", "`{name}` is declared twice (tasks and rules share names)")));
                continue;
            }
            let mut params = Vec::new();
            let mut param_ranges = BTreeMap::new();
            for p in &t.params {
                if params.iter().any(|(n, _): &(String, Ty)| *n == p.name.0) {
                    self.push(e("E006", p.name.1, tr!("引数 `{}` が二度書かれています", "the parameter `{}` is written twice", p.name.0)));
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
                    if let Some(api) = self.api(a, crate::apis::ApiKind::OpenApi, tr!("`http` で呼べるのは OpenAPI の記述（`use openapi`）の操作です", "`http` calls an operation of an OpenAPI document (`use openapi`)")) {
                        match crate::apis::openapi_op(&api, method, url) {
                            Ok(op) => {
                                if *form {
                                    self.push(e("E007", *bsp, tr!("本文の書き方は OpenAPI の記述が言うので、`form` は外してください", "the OpenAPI document says how the body is written; leave out `form`")));
                                }
                                match api.base_url() {
                                    Some(base) => spec_url = Some((format!("{base}{url}"), op.form)),
                                    None => self.push(e("E016", *bsp, tr!("`{}` はサーバーを言いません。`use openapi {}` の下に `url \"<場所>\"` を書いてください", "`{}` names no server; write `url \"<where it is>\"` under `use openapi {}`", a.0, a.0))),
                                }
                                described = Some((api.clone(), b.clone()));
                            }
                            Err(w) => self.push(e("E016", *bsp, w)),
                        }
                    }
                }
                Some((b @ syntax::Binding::Connect { api: a, method }, bsp)) => {
                    if let Some(api) = self.api(a, crate::apis::ApiKind::Proto, tr!("`connect` で呼べるのは `.proto`（`use proto`）のメソッドです", "`connect` calls a method of a `.proto` (`use proto`)")) {
                        // a `.proto` does not say where its service is; a flow that only takes its types needs no `url`
                        if api.url.is_none() && self.url_said.insert(a.0.clone()) {
                            let at = self.use_spans.get(&a.0).copied().unwrap_or(a.1);
                            let name = &a.0;
                            self.push(e(
                                "E016",
                                at,
                                tr!("`.proto` はサービスの場所を言いません。`use proto {name}` の下に `url \"<場所>\"` を書いてください", "a `.proto` does not say where the service is; write `url \"<where it is>\"` under `use proto {name}`"),
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
                            Err(w) => {
                                // the service may be in a file that was not read
                                let mut d = e("E016", *bsp, w);
                                if let Some(note) = Self::unread_note_of(&api) {
                                    d = d.note(note);
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
            // an operation of a book's transfer: the transfer and its parameters hold the task
            let book_op = match &t.binding {
                Some((syntax::Binding::Book { book, transfer, op }, bsp)) => self.book_task(t, book, transfer, op, *bsp, &params, result.as_ref()),
                _ => None,
            };
            // what a Jev task asks, from the type of its answer
            let jev = match &t.binding {
                Some((syntax::Binding::Jev(jd), bsp)) => Some(self.jev(t, jd, *bsp, result.as_ref())),
                _ => None,
            };
            if let (Some((_, _, csp)), None) = (&t.confidence, &jev) {
                self.push(e("E007", *csp, tr!("`confidence` は Jev の答えにどれだけの確信が要るかを書くところです。このタスクには `jev` がありません", "`confidence` says how sure Jev must be of its answer; this task has no `jev`")));
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
                // a book that could not be read, or an operation it has not, is said above
                syntax::Binding::Book { .. } => match &book_op {
                    Some(b) => Binding::Book(b.clone()),
                    None => Binding::Lambda(String::new()),
                },
            });
            if matches!(t.binding, Some((syntax::Binding::Book { .. }, _))) && book_op.is_none() {
                continue;
            }
            self.agent(t, result.as_ref(), result_range);
            // another `.flow` as the child: checked here, for its names and the contract it holds the task to
            let flow = t.flow.as_ref().and_then(|(path, fsp)| self.child_flow(path, *fsp));
            if t.flow.is_some() {
                if let Some((_, bsp)) = &t.binding {
                    self.push(e("E007", *bsp, tr!("ほかの `.flow` を走らせるタスクは、ほかに何も呼びません。`lambda`・`http`・`aws`・`agent`・`jev` は外してください", "a task that runs another `.flow` calls nothing else; leave out `lambda`, `http`, `aws`, `agent` and `jev`")));
                }
                if let Some((_, isp)) = &t.image {
                    self.push(e("E007", *isp, tr!("Argo では、ほかの `.flow` を走らせるタスクは、子の WorkflowTemplate からワークフローを作ります。`image` は外してください", "on Argo, a task that runs another `.flow` makes a workflow of the child's WorkflowTemplate; leave out `image`")));
                }
            }
            let child = t.flow.as_ref().or(t.workflow.as_ref()).or(t.state_machine.as_ref()).or(t.durable_function.as_ref()).or(t.argo_template.as_ref()).map(|(_, s)| *s);
            if let Some((syntax::Binding::Aws { service, .. }, bsp)) = &t.binding {
                if crate::aws::exception_prefix(service).is_none() {
                    self.push(e(
                        "E007",
                        *bsp,
                        tr!("Step Functions の AWS SDK 統合に、サービス `{service}` はありません。`dynamodb` や `secretsmanager` のように、Task の Resource での名前で書いてください", "Step Functions has no AWS SDK integration for the service `{service}`; write it as the Task's Resource names it, such as `dynamodb` or `secretsmanager`"),
                    ));
                }
            }
            let mut errors: Vec<ErrDef> = Vec::new();
            for er in &t.errors {
                let (en, esp) = &er.name;
                if en == "timeout" || en == "failure" {
                    self.push(e("E007", *esp, tr!("`{en}` は宣言しなくても使えます。タスク自身のエラーだけを宣言します", "`{en}` is always there; declare only the task's own errors")));
                    continue;
                }
                if errors.iter().any(|x| x.name == *en) {
                    self.push(e("E006", *esp, tr!("エラー `{en}` が二度書かれています", "the error `{en}` is written twice")));
                    continue;
                }
                if let Some(Binding::Book(b)) = &binding {
                    if er.status.is_some() || er.exception.is_some() {
                        self.push(e("E007", *esp, tr!("帳簿の操作のエラーは、帳簿が断る理由の名前そのものです。`= …` は外してください", "an error of a book's operation is the name of the reason the book refuses it with; leave out `= …`")));
                        continue;
                    }
                    let reasons = self.book_reasons(b);
                    if !reasons.contains(en) {
                        let (tn, op) = (&b.transfer, &b.op);
                        self.push(e("E016", *esp, tr!("`{en}` は、帳簿が `{tn}.{op}` を断る理由ではありません（{}）", "`{en}` is not a reason the book refuses `{tn}.{op}` with ({})", reasons.join("・"); reasons.join(", "))));
                        continue;
                    }
                    errors.push(ErrDef { name: en.clone(), status: None, exception: None });
                    continue;
                }
                if matches!(binding, Some(Binding::Agent { .. })) {
                    self.push(e(
                        "E007",
                        *esp,
                        tr!("エージェントのタスクにはエラーを宣言できません。モデルが応答を拒否したときや呼び出しが失敗したときは `failure`、時間を過ぎたときは `timeout` になります", "an agent declares no errors of its own: when the model refuses or the call fails it is `failure`, and past its time `timeout`"),
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
                                tr!("`{code}` は Connect のエラーコードではありません。コードは canceled・unknown・invalid_argument・deadline_exceeded・not_found・already_exists・permission_denied・resource_exhausted・failed_precondition・aborted・out_of_range・unimplemented・internal・unavailable・data_loss・unauthenticated です", "`{code}` is not a Connect error code; the codes are canceled, unknown, invalid_argument, deadline_exceeded, not_found, already_exists, permission_denied, resource_exhausted, failed_precondition, aborted, out_of_range, unimplemented, internal, unavailable, data_loss and unauthenticated"),
                            ));
                            continue;
                        }
                        _ => {
                            self.push(e("E007", *esp, tr!("Connect のエラーは、`{en} = not_found` のようにコードで書きます", "a Connect error is named by its code, as `{en} = not_found`")));
                            continue;
                        }
                    }
                } else {
                    (er.status, er.exception.clone())
                };
                match (&binding, status, &exception) {
                    (Some(Binding::Http { .. }), None, _) => {
                        self.push(e("E007", *esp, tr!("`{en}` が返ってくるときの HTTP ステータスを `{en} = 402` のように書きます", "give `{en}` the HTTP status it comes back with, as `{en} = 402`")));
                    }
                    // TypeSafe's API says an error by its status: 429 for the rate limit, 529 when it is overloaded
                    (Some(Binding::Jev(_)), None, _) => {
                        self.push(e(
                            "E007",
                            *esp,
                            tr!("Jev の API はエラーを HTTP ステータスで伝えます。`{en}` が返ってくるときのステータスを、`{en} = 429`（レート制限）や `{en} = 529`（過負荷）のように書きます", "Jev's API says an error by its HTTP status; give `{en}` the one it comes back with, as `{en} = 429` (the rate limit) or `{en} = 529` (overloaded)"),
                        ));
                    }
                    (Some(Binding::Jev(_)), Some(_), _) => {}
                    (Some(Binding::Http { .. }), Some(_), None) => {}
                    (Some(Binding::Aws { .. }), None, _) => {}
                    (Some(Binding::Aws { .. }), Some(_), _) => {
                        self.push(e("E007", *esp, tr!("AWS の API のエラーは、ステータスではなく `{en} = ConditionalCheckFailedException` のように例外の名前で書きます", "an AWS API's error is named by its exception, as `{en} = ConditionalCheckFailedException`, not by a status")));
                    }
                    (Some(Binding::Lambda(_)), None, None) => {}
                    (Some(Binding::Lambda(_)), _, _) => {
                        self.push(e("E007", *esp, tr!("Lambda のタスクのエラーは関数が投げるエラーの型の名前で表し、ステータスは書きません", "a Lambda task's error is named by the error type the function raises, without a status")));
                    }
                    (None, None, None) => {}
                    (None, _, _) => {
                        self.push(e("E007", *esp, tr!("`{en} = …` は HTTP や AWS の呼び出しでのエラーの表し方です。このタスクには `http` も `aws` もありません", "`{en} = …` says how an HTTP or AWS call names the error; this task has neither `http` nor `aws`")));
                    }
                    (Some(Binding::Http { .. }), Some(_), Some(_)) => {}
                    // refused above
                    (Some(Binding::Agent { .. }), _, _) | (Some(Binding::Book(_)), _, _) => {}
                }
                if let Some(st) = status {
                    if let Some(other) = errors.iter().find(|x| x.status == Some(st)) {
                        let other = other.name.clone();
                        self.push(e(
                            "E007",
                            *esp,
                            tr!("`{other}` と `{en}` がどちらも {st} で返ってきます。Step Functions はエラーをステータスで見分けるので、別々のステータスにしてください", "`{other}` and `{en}` both come back as {st}; Step Functions tells errors apart by status, so give each its own"),
                        ));
                        continue;
                    }
                }
                errors.push(ErrDef { name: en.clone(), status, exception });
            }
            // the error a less sure answer fails the call with is one of the task's own
            if let (Some((_, (fe, fsp), _)), Some(_)) = (&t.confidence, &jev) {
                if fe == "timeout" || fe == "failure" {
                    self.push(e("E007", *fsp, tr!("`{fe}` はいつもあるエラーです。確信度が足りない答えには、タスク自身のエラーの名前を付けてください", "`{fe}` is always there; name an error of the task's own for an answer that is not sure enough")));
                } else if errors.iter().any(|x| x.name == *fe) {
                    self.push(e("E006", *fsp, tr!("エラー `{fe}` が二度書かれています。`confidence … else {fe}` がそれを宣言します", "the error `{fe}` is written twice: `confidence … else {fe}` declares it")));
                } else {
                    errors.push(ErrDef { name: fe.clone(), status: None, exception: None });
                }
            }
            if let Some((syntax::Binding::Http { url, .. }, bsp)) = t.binding.as_ref().map(|(b, s)| (b, s)) {
                for ph in placeholders(url) {
                    if !params.iter().any(|(n, _)| *n == ph) {
                        self.push(e("E007", *bsp, tr!("URL に `{{{ph}}}` がありますが、タスクに引数 `{ph}` がありません", "the URL has `{{{ph}}}` but the task has no parameter `{ph}`")));
                    }
                }
            }
            if let Some(csp) = t.callback {
                let ok = match &binding {
                    None | Some(Binding::Lambda(_)) => true,
                    Some(Binding::Aws { service, action }) => service == "sqs" && action == "sendMessage",
                    Some(Binding::Http { .. }) | Some(Binding::Agent { .. }) | Some(Binding::Jev(_)) | Some(Binding::Book(_)) => false,
                };
                if !ok {
                    self.push(e(
                        "E007",
                        csp,
                        tr!("`callback` のタスクがトークンを渡せるのは `lambda` か `aws sqs:sendMessage` です", "a `callback` task hands its token over by `lambda` or by `aws sqs:sendMessage`"),
                    ));
                } else if matches!(binding, Some(Binding::Aws { .. })) {
                    match params.iter().find(|(p, _)| p == "MessageBody").map(|(_, t)| t.inner().clone()) {
                        Some(Ty::Record(_)) | Some(Ty::Json) => {}
                        _ => self.push(e(
                            "E007",
                            csp,
                            tr!("SQS を通すコールバックはトークンを `MessageBody` に入れるので、レコードか `json` の引数 `MessageBody` が要ります", "a callback through SQS puts its token into `MessageBody`, so the task needs a parameter `MessageBody` that is a record or `json`"),
                        )),
                    }
                }
                if let Some(c) = child {
                    self.push(e("E007", c, tr!("子ワークフローは終わったときに結果を返すので、`callback` のタスクにはなりません", "a child workflow answers when it ends; it is not a `callback` task")));
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
                        tr!("`event` のタスクは何も呼びません。値はワークフローに名前で送られてきます。呼び出し方の項目（と、先に ID を渡す `callback`）は外してください", "an `event` task calls nothing: its value is sent to the workflow by name; leave out the ways of calling it (and `callback`, whose task hands on an id first)"),
                    ));
                }
                if !params.is_empty() {
                    self.push(e("E007", *sp, tr!("`event` のタスクは何も送らないので、引数を取りません", "an `event` task sends nothing, so it takes no parameters")));
                }
                if let Some(r) = &t.retry {
                    self.push(e("E007", r.span, tr!("`event` のタスクには、リトライする呼び出しがありません。送る側が送り直します", "an `event` task calls nothing that could be tried again; whoever sends the event sends it again")));
                }
                if let Some(ksp) = t.key {
                    self.push(e("E007", ksp, tr!("`event` のタスクは何も送らないので、`key` は要りません", "an `event` task sends nothing, so it takes no `key`")));
                }
                if matches!(t.machine, Some((MachineUse::Starts { .. }, _)) | Some((MachineUse::Sends { .. }, _))) {
                    self.push(e("E007", esp, tr!("`event` のタスクが案件についてできるのは、状態を知らせる（`observes`）ことだけです。案件を始めたり、イベントを送ったりはできません", "an event can tell what a case is (`observes`), not start a case or send it an event")));
                }
            }
            if let Some(ksp) = t.key {
                if child.is_some() {
                    self.push(e("E007", ksp, tr!("子ワークフローはプラットフォームが呼び出しごとに一度だけ始めるので、`key` は使えません", "the platform starts a child workflow once for each call, so `key` does not apply to it")));
                }
                match (&binding, &t.key_param) {
                    (Some(Binding::Book(_)), _) => self.push(e("E007", ksp, tr!("帳簿の操作は、帳簿に書いたキーで一度だけ行われます。`key` は外してください", "an operation of a book is done once for the key the book gives it; leave out `key`"))),
                    (Some(Binding::Agent { .. }), _) => self.push(e("E007", ksp, tr!("エージェントは外部のデータを何も変えないので、`key` は要りません", "an agent changes nothing on the other side, so it takes no `key`"))),
                    (Some(Binding::Jev(_)), _) => self.push(e("E007", ksp, tr!("Jev は答えるだけで外部のデータを何も変えないので、`key` は要りません", "Jev only answers, and changes nothing on the other side, so it takes no `key`"))),
                    (Some(Binding::Aws { .. }), None) => self.push(e(
                        "E007",
                        ksp,
                        tr!("AWS の API は冪等キーを自分の引数の一つで受け取ります。`key ClientToken` のように `key <引数>` と書いてください", "an AWS API takes the idempotency key as one of its own parameters; write `key <parameter>`, as `key ClientToken`"),
                    )),
                    (Some(Binding::Aws { .. }), Some((kp, kpsp))) => {
                        if params.iter().any(|(p, _)| p == kp) {
                            self.push(e("E007", *kpsp, tr!("`{kp}` は `key` が埋めるので、引数からは外してください", "`{kp}` is filled by `key`; leave it out of the parameters")));
                        }
                    }
                    (_, Some((_, kpsp))) => self.push(e("E007", *kpsp, tr!("`key <引数>` は AWS の API のための書き方です。ここでは `key` だけを書きます", "`key <parameter>` is for an AWS API; write just `key`"))),
                    _ => {}
                }
            }
            let retry = t.retry.as_ref().map(|r| {
                for (on, osp) in &r.on {
                    if on != "timeout" && on != "failure" && !errors.iter().any(|x| x.name == *on) {
                        self.diags.push(e("E002", *osp, tr!("`{on}` は `{name}` のエラーではありません", "`{on}` is not an error of `{name}`")));
                    }
                    if jev.as_ref().and_then(|j| j.floor.as_ref()).is_some_and(|(_, fe)| fe == on) {
                        self.diags.push(e(
                            "E007",
                            *osp,
                            tr!("Jev は同じ入力にはほぼ同じように答えるので、尋ね直しても確信度は上がりません。`{on}` はリトライできません", "Jev answers the same input much the same way each time, so asking again is not surer; `{on}` is not retried"),
                        ));
                    }
                }
                Retry { times: r.times, every: r.every, backoff: r.backoff, on: r.on.iter().map(|x| x.0.clone()).collect() }
            });
            if let Some((ra, rsp)) = &t.refused_as {
                // a book's operation takes no `refused as` at all, which is said below
                if !errors.iter().any(|x| x.name == *ra) && !matches!(binding, Some(Binding::Book(_))) {
                    self.push(e("E007", *rsp, tr!("`{ra}` を `errors` にも書いてください", "declare `{ra}` in `errors` too")));
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
            if let (Some(Binding::Book(_)), Some((_, rsp))) = (&binding, &t.refused_as) {
                self.push(e("E007", *rsp, tr!("帳簿は、仮押さえの状態ごとに理由を付けて断ります。`refused as` は外し、理由を `errors` に書いてください", "a book refuses with a reason for each state of the hold; leave out `refused as`, and declare the reasons in `errors`")));
            } else if t.refused_as.is_some() && !matches!(machine, Some(TaskMachine::Sends { .. })) {
                self.push(e("E007", t.refused_as.as_ref().unwrap().1, tr!("`refused as` はイベントを `sends` するタスクに書きます", "`refused as` belongs to a task that `sends` an event")));
            }
            if machine.is_some() && result.is_none() {
                self.push(e("E008", *sp, tr!("`{name}` は案件を動かすので、案件のレコードを返します。`-> <レコード>` を書いてください", "`{name}` moves a case, so it answers with the case's record; write `-> <record>`")));
            }
            // what a book's operation does to the hold a case follows is the operation's own
            if let (Some(Binding::Book(b)), Some(mu)) = (&binding, &machine) {
                let hold = self.hold_ix.get(&(self.m.books[b.book].name.clone(), b.transfer.clone())).copied();
                let fits = match (b.op.as_str(), mu) {
                    ("hold", TaskMachine::Starts { rule, then }) => Some(*rule) == hold && then.is_empty(),
                    ("post", TaskMachine::Sends { event, .. }) => event == "post",
                    ("void", TaskMachine::Sends { event, .. }) => event == "void",
                    _ => false,
                };
                if !fits {
                    let (bn, tn, op) = (&self.m.books[b.book].name, &b.transfer, &b.op);
                    let want = match op.as_str() {
                        "hold" => format!("starts {bn}.{tn}"),
                        "post" => "sends post".to_string(),
                        "void" => "sends void".to_string(),
                        _ => String::new(),
                    };
                    let msp = t.machine.as_ref().map(|(_, s)| *s).unwrap_or(*sp);
                    let d = if want.is_empty() {
                        e("E008", msp, tr!("`{tn}.do` はすぐに確定する振替で、案件の始まりも終わりもしません", "`{tn}.do` is a transfer done at once, which neither starts nor ends a case"))
                    } else {
                        e("E008", msp, tr!("`{tn}.{op}` が仮押さえにすることは `{want}` です", "what `{tn}.{op}` does to the hold is `{want}`"))
                    };
                    self.push(d);
                }
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
                        for w in found {
                            let mut d = e("E016", Span { line: sp.line, col: 1 }, w);
                            // what the task is held to may name a type of a file that was not read
                            if let Some(note) = Self::unread_note_of(api) {
                                d = d.note(note);
                            }
                            self.push(d);
                        }
                    }
                    Err(w) => {
                        if !matches!(b, syntax::Binding::Http { .. } | syntax::Binding::Connect { .. }) {
                            self.push(e("E016", bsp, w));
                        }
                    }
                }
            }
        }
    }

    /// An operation of a book's transfer as a task's way of calling (`book stock.reserve.hold`):
    /// the book and the transfer it names (E002), an operation the transfer has (E007: `do` for a
    /// transfer done at once, `hold`, `post` and `void` for one that holds), and the task's
    /// parameters and answer held to the transfer (E016): `do` and `hold` take every parameter of
    /// the transfer, `post` and `void` those of the key (`post` all its amounts besides, or none, for
    /// all of it), each of the type the book gives it; `hold`, `post` and `void` answer the hold
    /// (`<book>.<transfer>`), or nothing, and `do` nothing.
    fn book_task(&mut self, t: &syntax::TaskDecl, book: &syntax::Name, transfer: &syntax::Name, op: &syntax::Name, bsp: Span, params: &[(String, Ty)], result: Option<&Ty>) -> Option<BookOp> {
        let Some(&bix) = self.book_ix.get(&book.0) else {
            self.push(e("E002", book.1, tr!("帳簿 `{}` はありません。`use book` で読んだものを書きます", "there is no book `{}`; name one read by `use book`", book.0)));
            return None;
        };
        let bu = self.m.books[bix].clone();
        let Some(tr) = bu.transfer(&transfer.0) else {
            let names: Vec<&str> = bu.facts.transfers.iter().map(|x| x.name.as_str()).collect();
            self.push(e("E002", transfer.1, tr!("帳簿 `{}` に振替 `{}` はありません（{}）", "the book `{}` has no transfer `{}` ({})", book.0, transfer.0, names.join("・"); book.0, transfer.0, names.join(", "))));
            return None;
        };
        let holds = tr.pending.is_some();
        let ops: &[&str] = if holds { &["hold", "post", "void"] } else { &["do"] };
        if !ops.contains(&op.0.as_str()) {
            let tn = &tr.name;
            let msg = if holds {
                tr!("`{tn}` は仮押さえにする振替で、操作は hold・post・void です", "`{tn}` is a transfer that holds first; its operations are hold, post and void")
            } else {
                tr!("`{tn}` はすぐに確定する振替で、操作は do です", "`{tn}` is a transfer done at once; its operation is do")
            };
            self.push(e("E007", op.1, msg));
            return None;
        }
        let tn = tr.name.clone();
        let opn = op.0.clone();
        let mut ok = true;
        // what the operation takes, each with its type
        let key: Vec<&ritsu_ports::TransferParam> = tr.params.iter().filter(|p| tr.key.contains(&p.name)).collect();
        let amounts: Vec<&ritsu_ports::TransferParam> = tr.params.iter().filter(|p| p.unit.is_some()).collect();
        let takes: Vec<&ritsu_ports::TransferParam> = match opn.as_str() {
            "do" | "hold" => tr.params.iter().collect(),
            "post" if amounts.iter().any(|a| params.iter().any(|(n, _)| *n == a.name)) => key.iter().chain(amounts.iter()).copied().collect(),
            _ => key.clone(),
        };
        let line = Span { line: t.name.1.line, col: 1 };
        for p in &takes {
            let want = Self::book_param_ty(&bu, p);
            match params.iter().find(|(n, _)| *n == p.name) {
                None => {
                    ok = false;
                    self.push(e("E016", line, tr!("`{tn}.{opn}` は `{}` を受け取ります。タスクの引数に書いてください", "`{tn}.{opn}` takes `{}`; give the task that parameter", p.name)));
                }
                Some((_, got)) if *got != want => {
                    ok = false;
                    let (g, w) = (self.m.ty_name(got), self.m.ty_name(&want));
                    self.push(e("E016", line, tr!("帳簿は `{}` を `{w}` で受け取りますが、タスクの引数は `{g}` です", "the book takes `{}` as `{w}`, but the task's parameter is `{g}`", p.name)));
                }
                _ => {}
            }
        }
        for (n, _) in params {
            if !takes.iter().any(|p| p.name == *n) {
                ok = false;
                self.push(e("E016", line, tr!("`{tn}.{opn}` は `{n}` を受け取りません", "`{tn}.{opn}` takes no `{n}`")));
            }
        }
        if opn == "post" && takes.len() > key.len() && amounts.iter().any(|a| !params.iter().any(|(n, _)| *n == a.name)) {
            // a post takes every amount, or none: a part of the hold, or all of it
            ok = false;
        }
        let hold_rec = self.record_ix.get(&format!("{}.{tn}", book.0)).copied();
        match (opn.as_str(), result) {
            (_, None) => {}
            ("do", Some(_)) => {
                ok = false;
                self.push(e("E016", line, tr!("`{tn}.do` は何も返しません。`->` は外してください", "`{tn}.do` answers nothing; leave out `->`")));
            }
            (_, Some(r)) if hold_rec.map(Ty::Record).as_ref() == Some(r) => {}
            (_, Some(r)) => {
                ok = false;
                let g = self.m.ty_name(r);
                let w = format!("{}.{tn}", book.0);
                self.push(e("E016", line, tr!("`{tn}.{opn}` が返すのは仮押さえ `{w}` です（`{g}` ではありません）", "`{tn}.{opn}` answers the hold, `{w}`, not `{g}`")));
            }
        }
        let _ = bsp;
        ok.then(|| BookOp { book: bix, transfer: tn, op: opn })
    }

    /// The reasons a book can refuse an operation with: its check's, and those chobo gives itself
    /// for every operation of its kind (DESIGN 2.7 of chobo's).
    fn book_reasons(&self, b: &BookOp) -> Vec<String> {
        let bu = &self.m.books[b.book];
        let mut out: Vec<String> = Vec::new();
        if let Some(tr) = bu.transfer(&b.transfer) {
            for (op, rs) in &tr.refusals {
                if *op == b.op {
                    out.extend(rs.iter().cloned());
                }
            }
        }
        // the bounds' own reasons can refuse a do and a hold, which move amounts
        if matches!(b.op.as_str(), "do" | "hold") {
            for a in &bu.facts.accounts {
                for bd in [&a.lower, &a.upper].into_iter().flatten() {
                    out.push(bd.refusal.clone());
                }
            }
        }
        let own: &[&str] = match b.op.as_str() {
            "do" | "hold" => &["key_conflict", "already_refused", "same_account"],
            "post" => &["no_such_hold", "already_voided", "expired", "over_hold", "key_conflict"],
            _ => &["no_such_hold", "already_posted", "expired"],
        };
        out.extend(own.iter().map(|s| s.to_string()));
        let mut seen = Vec::new();
        out.retain(|r| {
            if seen.contains(r) {
                false
            } else {
                seen.push(r.clone());
                true
            }
        });
        out
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
                    tr!("`{api}` は `use {k}` で読んだものですが、ワークフローが実装できるのは `.proto`（`use proto`）のサービスです", "`{api}` is read by `use {k}`, and a workflow implements a service of a `.proto` (`use proto`)"),
                ));
                return;
            }
            // a description that could not be read is said where it is read
            None if self.prog.apis.iter().any(|u| u.name.0 == api) => return,
            None => {
                self.push(e("E002", at, tr!("API `{api}` はありません。`use proto` で読んだものを書きます", "there is no API `{api}`; name one read by `use proto`")));
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
            let message = if names.is_empty() {
                tr!("`{api}` にサービス `{rel}` はありません（サービスはありません）", "`{api}` has no service `{rel}` (it has no services)")
            } else {
                tr!("`{api}` にサービス `{rel}` はありません（サービスは {}）", "`{api}` has no service `{rel}` (its services are {})", names.join("・"); names.join(", "))
            };
            let mut d = e("E017", at, message);
            // the service may be in a file that was not read
            if let Some(note) = crate::apis::unread_note(&pf) {
                d = d.note(note);
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
                tr!("Jev はいつも TypeSafe の API で呼びます。`url` は、エージェントの Open Responses の API がどのサーバーにあるかを書くところです", "Jev is always called at TypeSafe's API; `url` says which server an agent's Open Responses API is on"),
            ));
            return;
        }
        if let (Some((_, esp)), true) = (&t.effort, jev) {
            self.push(e("E007", *esp, tr!("Jev はすぐに答え、長く推論しません。`effort` はエージェントのモデルが推論にどれだけ力を入れるかを書くところです", "Jev answers at once and does not reason at length; `effort` says how hard an agent's model reasons")));
            return;
        }
        if jev {
            return;
        }
        if let (Some((_, usp)), false) = (&t.url, matches!(t.binding, Some((syntax::Binding::Agent { .. }, _)))) {
            self.push(e(
                "E007",
                *usp,
                tr!("`url` はエージェントの Open Responses の API がどのサーバーにあるかを書くところです。このタスクには `agent` がありません（HTTP のタスクの URL は `http` のあとに書きます）", "`url` says which server an agent's Open Responses API is on; this task has no `agent` (an HTTP task writes its URL after `http`)"),
            ));
        }
        if let (Some((_, esp)), false) = (&t.effort, matches!(t.binding, Some((syntax::Binding::Agent { .. }, _)))) {
            self.push(e(
                "E007",
                *esp,
                tr!("`effort` はエージェントのモデルが推論にどれだけ力を入れるかを書くところです。このタスクには `agent` がありません", "`effort` says how hard an agent's model reasons; this task has no `agent`"),
            ));
        }
        let (provider, bsp) = match (&t.binding, &t.model) {
            (Some((syntax::Binding::Agent { provider, .. }, bsp)), _) => (provider.clone(), *bsp),
            (_, Some((_, msp))) => {
                self.push(e("E007", *msp, tr!("`model` はエージェントや Jev が使うモデルを書くところです。このタスクには `agent` も `jev` もありません", "`model` says which model an agent or Jev uses; this task has neither `agent` nor `jev`")));
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
                    tr!("`{p}` は dandori の知らないエージェントです。`agent openai \"…\"`（`agent \"…\"` だけでも同じ）か `agent claude \"…\"` と書いてください", "`{p}` is not an agent dandori knows; write `agent openai \"…\"` (or just `agent \"…\"`) or `agent claude \"…\"`"),
                ));
                return;
            }
        };
        if let Some((u, usp)) = &t.url {
            if provider == Provider::Claude {
                self.push(e(
                    "E007",
                    *usp,
                    tr!("`url` は Open Responses（OpenAI の Responses API を、ほかのサーバーも同じ形で出すもの）のサーバーを書くところです。Claude のエージェントは Anthropic の Messages API で呼び、それは Open Responses ではありません", "`url` names a server of Open Responses (OpenAI's Responses API, as others serve it); a Claude agent is called on Anthropic's Messages API, which is not among them"),
                ));
            } else if !(u.starts_with("https://") || u.starts_with("http://")) {
                self.push(e("E007", *usp, tr!("`{u}` は http:// や https:// の URL ではありません", "`{u}` is not an http:// or https:// URL")));
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
                    tr!("`{level}` は {whose} が受け付けるエフォートではありません。{} のどれかを書いてください", "`{level}` is not an effort {whose} takes; write one of {}", levels.join("・"); levels.join(", ")),
                ));
            }
        }
        let Text { en: outputs_en, ja: outputs_ja } = match (provider, &t.url) {
            // with a space before the Japanese that follows an English word
            (Provider::OpenAi, None) => tr!("OpenAI の Structured Outputs ", "OpenAI's Structured Outputs"),
            (Provider::OpenAi, Some(_)) => tr!("Open Responses の構造化出力", "the structured outputs of Open Responses"),
            (Provider::Claude, _) => tr!("Claude の構造化出力", "Claude's structured outputs"),
        };
        if t.model.is_none() {
            let example = match provider {
                Provider::OpenAi => "gpt-5.4-mini",
                Provider::Claude => "claude-sonnet-5",
            };
            self.push(e(
                "E007",
                bsp,
                tr!("エージェントには、使うモデルを `model \"{example}\"` のように書いてください", "an agent needs the model it uses; write it as `model \"{example}\"`"),
            ));
        }
        if let Some((_, msp)) = &t.machine {
            self.push(e(
                "E007",
                *msp,
                tr!("エージェントは渡されたものを読んで応答するだけで、案件を始めたり動かしたり見たりはしません", "an agent reads what it is given and answers; it has no case on the other side to start, move or look at"),
            ));
        }
        let Some(r) = result else {
            self.push(e("E007", bsp, tr!("エージェントは応答を返します。応答の型を `-> <型>` と書いてください", "an agent answers; write the type of its answer as `-> <type>`")));
            return;
        };
        let Some(schema) = crate::render::answer_schema(&self.m, r, rg, provider) else {
            let message = if has_json(&self.m, r, &mut Vec::new()) {
                tr!("エージェントの応答は、{outputs_ja}で JSON Schema に合わせて返させます。`json` は Schema に書けないので、`json` を含まない型にしてください", "{outputs_en} hold an agent's answer to a JSON Schema, and `json` has none; give the answer a type without `json`")
            } else {
                tr!("エージェントの応答は、{outputs_ja}で JSON Schema に合わせて返させます。ほかのレコードを通して自分を含むレコードは Schema に書き切れないので、それを含まない型にしてください", "{outputs_en} hold an agent's answer to a JSON Schema written out in full, and a record that holds itself through others has no end; give the answer a type without it")
            };
            self.push(e("E007", bsp, message));
            return;
        };
        let mut over: Vec<Text> = Vec::new();
        match provider {
            // another server's limits are its own, and unknown here: its answer's check tells
            Provider::OpenAi if t.url.is_some() => {}
            Provider::OpenAi => {
                let (depth, props, values) = crate::render::schema_size(&schema);
                if depth > 10 {
                    over.push(tr!("オブジェクトのネストが {depth} 段（10 段まで）", "its objects nest {depth} deep (at most 10)"));
                }
                if props > 5000 {
                    over.push(tr!("プロパティが {props} 個（5,000 個まで）", "it has {props} properties (at most 5,000)"));
                }
                if values > 1000 {
                    over.push(tr!("列挙の値が {values} 個（1,000 個まで）", "it has {values} enum values (at most 1,000)"));
                }
            }
            Provider::Claude => {
                let unions = crate::render::schema_unions(&schema);
                if unions > crate::render::CLAUDE_UNIONS {
                    over.push(tr!("オプショナルな値（`T?`）が {unions} 個（{} 個まで）", "it has {unions} values that may be absent, each a choice with null (at most {})", crate::render::CLAUDE_UNIONS));
                }
            }
        }
        if !over.is_empty() {
            self.push(e(
                "E007",
                bsp,
                tr!("応答の JSON Schema（`{{\"answer\": …}}` に包んだもの）が、{outputs_ja}の受け付ける大きさを超えています。{}", "the answer's JSON Schema, in `{{\"answer\": …}}`, is larger than {outputs_en} take: {}", over.iter().map(|x| x.ja.clone()).collect::<Vec<_>>().join("、"); over.iter().map(|x| x.en.clone()).collect::<Vec<_>>().join(", ")),
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
                            tr!("Claude は列挙の値の大文字と小文字を変えて返すことがあり、dandori は大文字と小文字を区別せずに値を読みます。このため `{name}` の `{b}` と `{a}` は同じ値になります。大文字と小文字のほかにも違いのある名前にしてください", "Claude may answer an enum's value in another case, and dandori takes it as the value it matches without regard to case, so `{b}` and `{a}` of `{name}` would be the same; give them names that differ in more than case"),
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
                tr!("Jev には、尋ねるモデルを `model \"jev-1.13.0\"` のようにバージョンで書いてください", "Jev needs the model it asks; write the version, as `model \"jev-1.13.0\"`"),
            ));
        }
        if let Some((_, msp)) = &t.machine {
            self.push(e("E007", *msp, tr!("Jev は渡されたものを読んで答えるだけで、案件を始めたり動かしたり見たりはしません", "Jev reads what it is given and answers; it has no case on the other side to start, move or look at")));
        }
        const WHAT_EN: &str = "Jev answers a choice among an enum's values, a place on a scale of them (`score`), yes or no (`bool`), or a record of such answers; it writes no text";
        const WHAT_JA: &str = "Jev が答えるのは、列挙の値のどれか、列挙の値を低いものから並べた段階のどこか（`score`）、はいかいいえ（`bool`）と、それらを並べたレコードです。文章は書きません";
        let Some(r) = result else {
            self.push(e("E007", bsp, tr!("{WHAT_JA}。結果の型を `-> <型>` と書いてください", "{WHAT_EN}; write the type of its answer as `-> <type>`")));
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
                    tr!("結果はレコードなので、Jev にはフィールドごとに尋ねます。`jev` だけを書き、その下にフィールドごとに `<フィールド> \"<質問>\"` を書いてください", "the answer is a record, so Jev is asked a question for each of its fields: write `jev` alone, and under it `<field> \"<question>\"` for each"),
                ));
            }
            (None, Ty::Enum(_) | Ty::Bool) => {
                self.push(e("E007", bsp, tr!("Jev に尋ねることを、`jev \"<質問>\"` のように `jev` のあとに二重引用符で書いてください", "write the question Jev answers after `jev`, in double quotes: `jev \"<question>\"`")));
            }
            (None, Ty::Record(rid)) => {
                let rid = *rid;
                let rec = self.m.records[rid].clone();
                let mut seen: BTreeMap<String, Span> = BTreeMap::new();
                for (f, _) in &jd.fields {
                    if let Some(first) = seen.get(&f.0) {
                        self.push(e("E006", f.1, tr!("`{}` が二度尋ねられています（{} 行目）", "`{}` is asked twice (line {})", f.0, first.line)));
                        continue;
                    }
                    seen.insert(f.0.clone(), f.1);
                    if !rec.fields.iter().any(|(n, _)| *n == f.0) {
                        let fields = rec.fields.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
                        self.push(e("E002", f.1, tr!("`{}` は `{}` のフィールドではありません（フィールドは {}）", "`{}` is not a field of `{}`, whose fields are {}", f.0, rec.name, fields.join("・"); f.0, rec.name, fields.join(", "))));
                    }
                }
                // the questions first, in the order of the record's fields, then how sure of which
                for (fname, fty) in &rec.fields {
                    let Some((_, what)) = jd.fields.iter().find(|(n, _)| n.0 == *fname) else {
                        self.push(e(
                            "E007",
                            bsp,
                            tr!("`{}` のフィールド `{fname}` について Jev に尋ねることを、`jev` の下に書いてください（`{fname} \"<質問>\"` か `{fname} confidence of <フィールド>`）", "write under `jev` what Jev is asked for the field `{fname}` of `{}`: `{fname} \"<question>\"`, or `{fname} confidence of <field>`", rec.name),
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
                                self.push(e("E007", ask.span, tr!("{WHAT_JA}。フィールド `{fname}` は `{tn}` です", "{WHAT_EN}; the field `{fname}` is `{tn}`")));
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
                            tr!("Jev の確信度が入るのは、`rate[step 1%]` や `rate[step 0.01%]` のように、刻みで 100% を割り切れる率です。`{fname}` は `{tn}` です", "how sure Jev is goes into a rate with a step that goes into 100% a whole number of times, such as `rate[step 1%]` or `rate[step 0.01%]`; `{fname}` is `{tn}`"),
                        ));
                        continue;
                    };
                    match out.questions.iter().position(|q| q.field.as_deref() == Some(of.0.as_str())) {
                        Some(qi) => out.confidences.push((fname.clone(), qi, per)),
                        None => self.push(e(
                            "E007",
                            of.1,
                            tr!("`{}` については Jev に尋ねていないので、その答えの確信度はありません", "`{}` is not a field Jev is asked about, so there is no answer to be sure of", of.0),
                        )),
                    }
                }
                if out.questions.is_empty() && !jd.fields.is_empty() {
                    self.push(e("E007", bsp, tr!("{WHAT_JA}。`{}` のフィールドの一つは尋ねてください", "{WHAT_EN}; ask it about one field of `{}` at least", rec.name)));
                }
            }
            (_, other) => {
                let tn = self.m.ty_name(other);
                self.push(e("E007", bsp, tr!("{WHAT_JA}。結果は `{tn}` です", "{WHAT_EN}; the answer is `{tn}`")));
            }
        }
        if let Some((v, _, csp)) = &t.confidence {
            // yes or no: sure of the answer it takes by the probability of that answer, one half at least
            if !out.questions.is_empty() && out.questions.iter().all(|q| q.kind == QuestionKind::Noul) && *v <= 0.5 {
                self.push(e(
                    "E007",
                    *csp,
                    tr!("はいかいいえの答えには、Jev はいつも半分以上確かなので、`confidence {v}` で呼び出しが失敗することはありません。0.5 より大きい値を書いてください", "Jev is at least half sure of the answer it takes to a yes or no, so `confidence {v}` never fails the call; give more than 0.5"),
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
                    tr!("`{m}` はエイリアスで、ここを変えなくても Jev の新しいバージョンに移ります。確信度の意味はバージョンごとに違うので、確信度を合わせたバージョンを `model \"jev-1.13.0\"` のように書いてください", "`{m}` is an alias, which moves to a new version of Jev without a change here, and how sure one version is means something else to the next; name the version the confidence is set for, as `model \"jev-1.13.0\"`"),
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
                self.push(e("E006", *vsp, tr!("`{v}` の意味が二度書かれています（{} 行目）", "what `{v}` means is written twice (line {})", first.line)));
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
                        self.push(e("E002", *vsp, tr!("`{v}` は `{}` の値ではありません（値は {}）", "`{v}` is not a value of `{}`, whose values are {}", def.name, def.values.join("・"); def.name, def.values.join(", "))));
                        return None;
                    }
                }
                if let Some(ssp) = ask.score {
                    // the levels are the values, each with what it means, from the lowest
                    if !(2..=10).contains(&def.values.len()) {
                        self.push(e(
                            "E007",
                            ssp,
                            tr!("score の段階は 2 から 10 までで、`{}` の値は {} 個です", "a score has from 2 to 10 levels, and `{}` has {} values", def.name, def.values.len()),
                        ));
                        return None;
                    }
                    let missing: Vec<&String> = def.values.iter().filter(|v| meaning(v).is_none()).collect();
                    if !missing.is_empty() {
                        let names = missing.iter().map(|v| v.as_str()).collect::<Vec<_>>();
                        self.push(e(
                            "E007",
                            ask.span,
                            tr!("Jev は score の段階を、書いた意味だけで見分けます。`{}` のすべての値の意味を、低い段階から高い段階の順に下に書いてください。{} がありません", "Jev sees a score's levels only by what each means, so write under it what every value of `{}` means, from the lowest level to the highest; {} is missing", def.name, names.join("・"); def.name, names.join(", ")),
                        ));
                        return None;
                    }
                    let options = ask.criteria.iter().map(|((v, _), m)| (v.clone(), Some(m.clone()))).collect();
                    Some(Question { id, field: field.map(String::from), kind: QuestionKind::Score, instructions: ask.instructions.clone(), options })
                } else {
                    if def.values.len() > 255 {
                        self.push(e("E007", ask.span, tr!("Jev が選べるのは 255 個までで、`{}` の値は {} 個です", "Jev chooses among at most 255 options, and `{}` has {} values", def.name, def.values.len())));
                        return None;
                    }
                    let options = def.values.iter().map(|v| (v.clone(), meaning(v))).collect();
                    Some(Question { id, field: field.map(String::from), kind: QuestionKind::Choice, instructions: ask.instructions.clone(), options })
                }
            }
            Ty::Bool => {
                if let Some(ssp) = ask.score {
                    self.push(e("E007", ssp, tr!("score は、列挙の値を低いものから並べた段階のどこかを答えます。`bool` は、はいかいいえで尋ねます", "a score is a place on a scale of an enum's values; a `bool` is asked as yes or no")));
                    return None;
                }
                for ((v, vsp), _) in &ask.criteria {
                    if v != "true" && v != "false" {
                        self.push(e("E002", *vsp, tr!("答えははいかいいえなので、`{v}` ではなく `true` と `false` の意味を書きます", "the answer is yes or no, so write what `true` and `false` mean, not `{v}`")));
                        return None;
                    }
                }
                let options: Vec<(String, Option<String>)> = match (meaning("true"), meaning("false")) {
                    (Some(y), Some(n)) => vec![("true".into(), Some(y)), ("false".into(), Some(n))],
                    (None, None) => vec![],
                    _ => {
                        self.push(e("E007", ask.span, tr!("`true` と `false` の意味は、両方書くか、どちらも書かないかです", "write what both `true` and `false` mean, or neither")));
                        return None;
                    }
                };
                Some(Question { id, field: field.map(String::from), kind: QuestionKind::Noul, instructions: ask.instructions.clone(), options })
            }
            _ => None,
        }
    }

    /// `payment_intent.payment`: a rule that is used, and its machine's name; or `stock.reserve`, a
    /// book and its transfer that holds, whose holds' life is chobo's machine.
    fn machine_rule(&mut self, q: &[(String, Span)], sp: Span) -> Option<usize> {
        if q.len() != 2 {
            self.push(e("E002", sp, tr!("ステートマシンは <規則>.<ステートマシン> と書きます", "name a machine as <rule>.<machine>")));
            return None;
        }
        if let Some(&bix) = self.book_ix.get(&q[0].0) {
            if let Some(&r) = self.hold_ix.get(&(q[0].0.clone(), q[1].0.clone())) {
                return Some(r);
            }
            let bu = &self.m.books[bix];
            let holds: Vec<&str> = bu.facts.transfers.iter().filter(|t| t.pending.is_some()).map(|t| t.name.as_str()).collect();
            let what = if bu.transfer(&q[1].0).is_some() {
                tr!("`{}` はすぐに確定する振替で、仮押さえにしないので、案件はそれに従えません", "`{}` is a transfer done at once, which holds nothing a case could follow", q[1].0)
            } else {
                tr!("帳簿 `{}` に振替 `{}` はありません", "the book `{}` has no transfer `{}`", q[0].0, q[1].0)
            };
            let holds = if holds.is_empty() { tr!("この帳簿に仮押さえにする振替はありません", "the book has no transfer that holds") } else { tr!("仮押さえにする振替は {} です", "the transfers that hold are {}", holds.join("・"); holds.join(", ")) };
            self.push(e("E002", q[1].1, what).note(holds));
            return None;
        }
        let rix = match self.rule_ix.get(&q[0].0) {
            Some(r) => *r,
            None => {
                self.push(e("E002", q[0].1, tr!("規則 `{}` は読み込まれていません", "no rule `{}` is used", q[0].0)));
                return None;
            }
        };
        match &self.m.rules[rix].info.machine {
            Some(mc) if mc.name == q[1].0 => Some(rix),
            Some(mc) => {
                let n = mc.name.clone();
                self.push(e("E002", q[1].1, tr!("`{}` のステートマシンは `{n}` です", "the machine of `{}` is `{n}`", q[0].0)));
                None
            }
            None => {
                self.push(e("E002", q[0].1, tr!("`{}` にはステートマシンがありません", "`{}` has no machine", q[0].0)));
                None
            }
        }
    }

    fn cases(&mut self) {
        for c in &self.prog.cases {
            let (name, sp) = &c.name;
            if self.m.cases.iter().any(|x| x.name == *name) {
                self.push(e("E006", *sp, tr!("案件 `{name}` が二度宣言されています", "the case `{name}` is declared twice")));
                continue;
            }
            let record = match self.ty(&c.record) {
                Some(Ty::Record(r)) if matches!(self.m.records[r].origin, RecordOrigin::Local | RecordOrigin::Proto { .. } | RecordOrigin::Hold { .. }) => r,
                Some(_) => {
                    self.push(e(
                        "E008",
                        c.record.span(),
                        tr!("案件の型には、このファイルで宣言したレコードか、`.proto` から作ったレコードを使います", "a case is held in a record declared in this file or made from a `.proto`"),
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
                    tr!("`{}` の値（{}）は、`{machine}` の状態（{}）と違います", "the values of `{}` ({}) are not the states of `{machine}` ({})", en.name, en.values.join("・"), mc.states.join("・"); en.name, en.values.join(", "), mc.states.join(", ")),
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
                            self.push(e("E008", *fsp, tr!("`{f}` はステートマシンの状態の型ではありません", "`{f}` is not of the machine's state type")));
                        }
                        continue;
                    }
                    None => {
                        self.push(e("E002", *fsp, tr!("`{}` にフィールド `{f}` はありません", "`{}` has no field `{f}`", self.m.records[record].name)));
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
                                tr!("`{}` に、案件の状態を入れる `{}` 型のフィールドがありません", "`{}` has no field of type `{}` to hold the case's state", self.m.records[record].name, self.m.enums[state_enum].name),
                            ));
                            continue;
                        }
                        _ => {
                            self.push(e("E008", c.record.span(), tr!("状態を入れられるフィールドが二つ以上あります。`state <フィールド>` を書いてください", "more than one field could hold the state; write `state <field>`")));
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
                        tr!("`{inp}` はステートマシンの held ではありません（held は {}）", "`{inp}` is not held by the machine; its held inputs are {}", if mc.held.is_empty() { "ありません".to_string() } else { mc.held.join("・") }; if mc.held.is_empty() { "none".to_string() } else { mc.held.join(", ") }),
                    ));
                    continue;
                }
                if let Some(a) = mc.axis_of(inp) {
                    match mc.axes[a].coords.iter().position(|x| x == val) {
                        Some(ci) => held.push((a, ci)),
                        None => {
                            self.push(e("E003", *vsp, tr!("`{val}` は `{inp}` の値ではありません（{}）", "`{val}` is not a value of `{inp}` ({})", mc.axes[a].coords.join("・"); mc.axes[a].coords.join(", "))));
                            continue;
                        }
                    }
                }
                held_values.push((inp.clone(), val.clone()));
            }
            let hold = matches!(self.m.rules[rule].kind, RuleKind::Hold { .. });
            let mut external = Vec::new();
            if hold {
                // a hold expires on its own when the book says it does: the workflow need not say so
                for (ev, esp) in &c.external {
                    if ev != "expire" || mc.axes_with_value(ev).is_empty() {
                        self.push(e("E008", *esp, tr!("仮押さえに外で起きるのは、有効期限のある振替の `expire` だけです", "what happens to a hold on its own is `expire`, of a transfer whose holds expire")));
                    }
                }
                if let Some(a) = mc.axes_with_value("expire").first() {
                    external.push((*a, mc.axes[*a].coords.iter().position(|x| x == "expire").unwrap(), "expire".to_string()));
                }
                if let Some(((_, osp), _)) = &c.refused_when {
                    self.push(e("E008", *osp, tr!("仮押さえを断るのは帳簿で、理由は状態ごとに決まっています。`refused when` は外してください", "a hold is refused by the book, with a reason for each state; leave out `refused when`")));
                }
            }
            for (ev, esp) in c.external.iter().filter(|_| !hold) {
                let axes = mc.axes_with_value(ev);
                match axes.len() {
                    1 => external.push((axes[0], mc.axes[axes[0]].coords.iter().position(|x| x == ev).unwrap(), ev.clone())),
                    0 => self.push(e("E008", *esp, tr!("ステートマシンのどの入力にもイベント `{ev}` はありません", "no input of the machine has the event `{ev}`"))),
                    _ => self.push(e("E008", *esp, tr!("値 `{ev}` を持つ入力が二つ以上あります", "more than one input has the value `{ev}`"))),
                }
            }
            let refused_when = match &c.refused_when {
                // chobo's table says whether a call is refused, in `refused`
                _ if hold => mc.decides.iter().position(|d| d == "refused").map(|i| (i, "true".to_string())),
                Some(((o, osp), (v, _))) => match mc.decides.iter().position(|d| d == o) {
                    Some(i) => Some((i, v.clone())),
                    None => {
                        self.push(e("E008", *osp, tr!("ステートマシンの表は `{o}` を書きません（書くのは {}）", "the machine's table does not write `{o}`; it writes {}", mc.decides.join("・"); mc.decides.join(", "))));
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
                self.push(Diag::error("E006", c.line, 1, tr!("`{}` が入力と案件の両方にあります", "`{}` is both an input and a case", c.name)));
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
                e("E006", sp, tr!("`{n}` は入力か案件です。ここで値を入れる変数には別の名前を付けてください", "`{n}` is an input or a case; a variable set here needs a new name")),
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
                    e("E003", sp, tr!("`{n}` はほかの場所で `{a}`、ここで `{b}` です。名前の型は一つです", "`{n}` was `{a}` elsewhere and is `{b}` here; a name keeps one type")),
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
                        self.push(e("E003", call.callee.1, tr!("`{}` は何も返しません。`let` を付けずに呼んでください", "`{}` answers nothing to keep; call it without `let`", call.callee.0)));
                        return None;
                    }
                };
                if let Some(te) = ty {
                    let want = self.ty(te)?;
                    if !result.fits(&want) {
                        let (a, b) = (self.m.ty_name(&result), self.m.ty_name(&want));
                        self.push(e("E003", call.callee.1, tr!("`{}` が返すのは `{a}` で、`{b}` ではありません", "`{}` answers `{a}`, which is not `{b}`", call.callee.0)));
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
                        self.push(e("E003", *sp, tr!("レコードの型を `let {}: <レコード> = {{…}}` のように書いてください", "write the record's type: `let {}: <record> = {{…}}`", name.0)));
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
                    self.push(e("E009", call.callee.1, tr!("規則は結果を返すだけです。`let` で結果を受け取ってください", "a rule only answers; keep its answer with `let`")));
                    return None;
                }
                let hs = self.handlers(&callee, handlers);
                TK::Call { target: None, callee, args, handlers: hs }
            }
            StmtKind::CaseCall { case, call, handlers } => {
                let ci = match self.m.case_index(&case.0) {
                    Some(c) => c,
                    None => {
                        self.push(e("E002", case.1, tr!("案件 `{}` はありません", "there is no case `{}`", case.0)));
                        return None;
                    }
                };
                let (callee, args) = self.call(call)?;
                let ti = match callee {
                    Callee::Task(t) => t,
                    Callee::Rule(_) => {
                        self.push(e("E008", call.callee.1, tr!("案件を動かすのは `starts`・`sends`・`observes` を書いたタスクです。規則は `let` で呼びます", "a case moves by a task that `starts`, `sends` or `observes`; a rule is called with `let`")));
                        return None;
                    }
                };
                if self.par_depth > 0 {
                    self.push(e(
                        "E009",
                        case.1,
                        tr!("同時に回る `for … in parallel` の中からは、案件 `{}` を動かせません", "the case `{}` cannot be moved from inside `for … in parallel`, where the rounds run at the same time", case.0),
                    ));
                    return None;
                }
                let task = self.m.tasks[ti].clone();
                if task.result != Some(Ty::Record(self.m.cases[ci].record)) {
                    let a = task.result.as_ref().map(|r| self.m.ty_name(r)).unwrap_or_else(|| "nothing".into());
                    let b = self.m.records[self.m.cases[ci].record].name.clone();
                    self.push(e("E003", call.callee.1, tr!("`{}` が返すのは `{a}` ですが、案件 `{}` の型は `{b}` です", "`{}` returns `{a}`, but the case `{}` is held in `{b}`", task.name, case.0)));
                    return None;
                }
                match &task.machine {
                    None => {
                        self.push(e("E008", call.callee.1, tr!("`{}` には、案件に何をするかが書かれていません。`starts`・`sends`・`observes` のどれかを書いてください", "`{}` does not say what it does to a case; write `starts`, `sends` or `observes` under it", task.name)));
                        return None;
                    }
                    Some(TaskMachine::Starts { rule, .. }) if *rule != self.m.cases[ci].rule => {
                        self.push(e("E008", call.callee.1, tr!("`{}` が始めるのは別のステートマシンの案件です", "`{}` starts a case of another machine", task.name)));
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
                    self.push(e("E009", s.span, tr!("0 の待ちは何もしません", "a wait of zero does nothing")));
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
                        self.push(e("E003", list.span(), tr!("ここでは `{}` が無いことがあります。先に `none` と `some <名前>` で `match` してください", "`{}` may be absent here; `match` it with `none` and `some <name>` first", lx.show())));
                        return None;
                    }
                    other => {
                        let n = self.m.ty_name(&other);
                        self.push(e("E003", list.span(), tr!("`for` で回せるのはリストです。これは `{n}` です", "`for` goes through a list; this is `{n}`")));
                        return None;
                    }
                };
                if self.reserved(&var.0) {
                    self.push(e("E006", var.1, tr!("`{}` は入力か案件です。ループの変数には別の名前を付けてください", "`{}` is an input or a case; the loop's variable needs a new name", var.0)));
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
                            self.push(e("E009", s.span, tr!("`let {} = for …` の本体の最後の行には `yield <値>` が要ります", "`let {} = for …` needs `yield <value>` as the last line of its body", r.0)));
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
                self.push(e("E009", expr.span(), tr!("`yield` は `let <名前> = for …` の本体の最後の行に書きます", "`yield` is the last line of the body of `let <name> = for …`")));
                return None;
            }
            StmtKind::Pass => TK::Pass,
            StmtKind::Break => {
                match self.loops.last() {
                    None => {
                        self.push(e("E009", s.span, tr!("`break` は `repeat` か `for` の中に書きます", "`break` is written inside `repeat` or `for`")));
                        return None;
                    }
                    Some(true) => {
                        self.push(e("E009", s.span, tr!("`for … in parallel` のイテレーションは同時に走るので、`break` で抜けられません", "the rounds of `for … in parallel` run at the same time, so there is no `break` from them")));
                        return None;
                    }
                    Some(false) => {}
                }
                TK::Break
            }
            StmtKind::Succeed { fields } => {
                if self.in_on_failure {
                    self.push(e("E009", s.span, tr!("`on failure` はワークフローを失敗で終えます。`succeed` は書けません", "`on failure` ends the workflow as failed; it cannot `succeed`")));
                    return None;
                }
                if self.in_on_cancel {
                    self.push(e("E009", s.span, tr!("`on cancel` はワークフローをキャンセルされたとして終えます。`succeed` は書けません", "`on cancel` ends the workflow as cancelled; it cannot `succeed`")));
                    return None;
                }
                if self.par_depth > 0 {
                    self.push(e("E009", s.span, tr!("ほかのイテレーションがまだ動いていることがあるので、`for … in parallel` の中からは `succeed` できません", "the workflow cannot `succeed` from inside `for … in parallel`, where other rounds may still run")));
                    return None;
                }
                let mut out = Vec::new();
                for ((n, nsp), ex) in fields {
                    let oty = match self.m.outputs.iter().find(|(o, _)| o == n) {
                        Some((_, t)) => t.clone(),
                        None => {
                            self.push(e("E002", *nsp, tr!("出力 `{n}` はありません", "there is no output `{n}`")));
                            continue;
                        }
                    };
                    if out.iter().any(|(o, _): &(String, TExpr)| o == n) {
                        self.push(e("E006", *nsp, tr!("`{n}` が二度書かれています", "`{n}` is given twice")));
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
                    self.push(e("E004", s.span, tr!("`succeed` が {} を書いていません", "`succeed` does not give {}", missing.join("・"); missing.join(", "))));
                }
                TK::Succeed { fields: out }
            }
            StmtKind::Fail { error, cause, leaving } => {
                let mut left = Vec::new();
                for (l, lsp) in leaving {
                    match self.m.case_index(l) {
                        Some(c) => left.push(c),
                        None => self.push(e("E002", *lsp, tr!("案件 `{l}` はありません", "there is no case `{l}`"))),
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
                    tr!("`match` に渡せるのは列挙・bool・オプショナルな値（`T?`）です。これは `{n}` です", "`match` works on an enum, a bool, or a value that may be absent (`T?`); this is `{n}`"),
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
                    self.push(e("E003", *vsp, tr!("`some` を書けるのは、オプショナルな値（`T?`）で分けるときだけです", "`some` is written when matching a value that may be absent (`T?`)")));
                    continue;
                }
                if seen.iter().any(|x| x == "some") {
                    self.push(e("E011", a.span, tr!("`some` の分岐は上にもう書かれています", "`some` already has an arm above")));
                    continue;
                }
                if has_values {
                    self.push(e("E003", a.span, tr!("`some` は値があるときのすべてを表します。値を並べるか `some` を書くかのどちらかにしてください", "`some` stands for every value that is there; write either the values or `some`")));
                    continue;
                }
                seen.push("some".into());
                if self.reserved(v) {
                    self.push(e("E006", *vsp, tr!("`{v}` は入力か案件です。`some` には別の名前を付けてください", "`{v}` is an input or a case; `some` needs a new name")));
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
                    self.push(e("E011", *vsp, tr!("`{v}` の分岐は上にもう書かれています", "`{v}` already has an arm above")));
                    continue;
                }
                seen.push(v.clone());
                if v == "none" {
                    if is_case_state || optional {
                        none = true;
                    } else {
                        self.push(e("E003", *vsp, tr!("`none` を書けるのは、案件の状態か、オプショナルな値（`T?`）で分けるときだけです", "`none` is written when matching a case's state, or a value that may be absent (`T?`)")));
                    }
                    continue;
                }
                if optional && domain.is_empty() {
                    let n = self.m.ty_name(&ty);
                    self.push(e("E003", *vsp, tr!("`{}` は `{n}` です。分岐は `none` と `some <名前>` です", "`{}` is `{n}`; its arms are `none` and `some <name>`", te.show())));
                    continue;
                }
                if !domain.contains(v) {
                    let n = self.m.ty_name(ty.inner());
                    self.push(e("E003", *vsp, tr!("`{v}` は `{n}` の値ではありません（{}）", "`{v}` is not a value of `{n}` ({})", domain.join("・"); domain.join(", "))));
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
                    self.push(e("E011", *sp, tr!("`{n}` は上ですでに処理しています", "`{n}` is handled above already")));
                    continue;
                }
                seen.push(n.clone());
                match n.as_str() {
                    "timeout" => errs.push(HErr::Timeout),
                    "failure" => errs.push(HErr::Failure),
                    _ if declared.contains(n) => errs.push(HErr::Declared(n.clone())),
                    _ => {
                        let list = if declared.is_empty() { "timeout, failure".to_string() } else { format!("{}, timeout, failure", declared.join(", ")) };
                        self.push(e("E002", *sp, tr!("`{n}` はこの呼び出しが投げるエラーではありません（{list}）", "`{n}` is not an error this call can raise ({list})")));
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
                self.push(e("E011", h.span, tr!("この分岐は、すべてのエラーを処理する `on failure` のあとにあります", "this arm comes after `on failure`, which already takes every error")));
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
                    tr!("`{name}` はイベントを待ちますが、`for … in parallel` の中では待てません。どのイテレーションも同じイベントを待つことになり、どのイテレーションに宛てたものか分かりません", "`{name}` waits for an event, which cannot be waited for inside `for … in parallel`: the rounds would wait for the same one, and it would not say which round it is for"),
                ));
                return None;
            }
            (Callee::Task(*t), self.m.tasks[*t].params.clone())
        } else if let Some(r) = self.rule_ix.get(name) {
            let r = *r;
            let rn = self.m.rules[r].name.clone();
            let ps = self.m.rules[r].info.inputs.iter().map(|col| (col.name.clone(), self.rty(&rn, &col.ty))).collect();
            (Callee::Rule(r), ps)
        } else if let Some((file, facts)) = name.split_once('.').and_then(|(f, _)| self.date_files.get(f).map(|x| (f.to_string(), x))) {
            // a date that the dates file does not have: say which it has
            let date = &name[file.len() + 1..];
            let names: Vec<&str> = facts.functions.iter().map(|f| f.name.as_str()).collect();
            self.push(e("E002", *sp, tr!("日付のファイル `{file}` に日付 `{date}` はありません（{}）", "the dates file `{file}` has no date `{date}` ({})", names.join("・"); names.join(", "))));
            return None;
        } else {
            self.push(e("E002", *sp, tr!("タスクか規則 `{name}` はありません", "there is no task or rule `{name}`")));
            return None;
        };
        let mut args = Vec::new();
        let mut ok = true;
        for ((an, asp), ex) in &c.args {
            let pty = match params.iter().find(|(p, _)| p == an) {
                Some((_, t)) => t.clone(),
                None => {
                    let list: Vec<&str> = params.iter().map(|(p, _)| p.as_str()).collect();
                    self.push(e("E004", *asp, tr!("`{name}` に引数 `{an}` はありません（{}）", "`{name}` has no parameter `{an}` ({})", list.join("・"); list.join(", "))));
                    ok = false;
                    continue;
                }
            };
            if args.iter().any(|(a, _): &(String, TExpr)| a == an) {
                self.push(e("E004", *asp, tr!("`{an}` が二度書かれています", "`{an}` is given twice")));
                ok = false;
                continue;
            }
            // a day of a dates file takes a time too, read as the day it falls on in the calendar's offset
            let date_call = match &callee {
                Callee::Rule(r) => self.m.rules[*r].date().cloned(),
                _ => None,
            };
            if let (Some(dc), Ty::Date) = (&date_call, &pty) {
                if self.quiet_expr(ex, None) == Some(Ty::Timestamp) {
                    if dc.offset.is_none() {
                        let file = &dc.file;
                        self.push(e("E003", ex.span(), tr!("ここには `date` が要りますが、これは `timestamp` です", "expected `date` here, but this is `timestamp`")).note(tr!(
                            "`{file}` のカレンダーは UTC オフセットを言わないので、時刻がどの日にあたるかが決まりません。カレンダーに `offset +09:00` のように書くか、日付を渡してください",
                            "the calendar of `{file}` says no UTC offset, so which day a time falls on is not known; give the calendar an offset (`offset +09:00`), or pass a day"
                        )));
                        ok = false;
                        continue;
                    }
                    match self.expr(ex, Some(&Ty::Timestamp)) {
                        Some(te) => args.push((an.clone(), te)),
                        None => ok = false,
                    }
                    continue;
                }
            }
            match self.expr(ex, Some(&pty)) {
                Some(te) => args.push((an.clone(), te)),
                None => ok = false,
            }
        }
        // an optional parameter may be left out; it is then sent as null
        let missing: Vec<String> = params.iter().filter(|(p, t)| !matches!(t, Ty::Opt(_)) && !c.args.iter().any(|((a, _), _)| a == p)).map(|(p, _)| p.clone()).collect();
        if !missing.is_empty() {
            self.push(e("E004", *sp, tr!("`{name}` の呼び出しに {} がありません", "the call of `{name}` does not give {}", missing.join("・"); missing.join(", "))));
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
            Expr::Now(_) => TExpr::Now,
            Expr::Str(s, _) => TExpr::Str(s.clone()),
            Expr::Int(n, _) => TExpr::Int(*n),
            Expr::Bool(b, _) => TExpr::Bool(*b),
            Expr::Interp(parts, sp) => {
                let mut out = Vec::new();
                for p in parts {
                    match p {
                        Part::Lit(s) => out.push(IPart::Lit(s.clone())),
                        Part::Hole(path) if path.len() == 1 && path[0].0 == "now" => out.push(IPart::Hole(TExpr::Now)),
                        Part::Hole(path) => {
                            let x = self.path(path, None)?;
                            let t = x.ty();
                            if !matches!(t, Ty::Str | Ty::Int | Ty::Num(_) | Ty::Bool | Ty::Enum(_) | Ty::Timestamp | Ty::Date) {
                                let n = self.m.ty_name(&t);
                                let message = if matches!(t, Ty::Opt(_)) {
                                    tr!("`{}` は `{n}` で、値が無いことがあるので、そのままでは文字列に入れられません。先に `none` と `some <名前>` で `match` してください", "`{}` is `{n}` and may be absent, so it cannot be put in a string as it is; `match` it with `none` and `some <name>` first", x.show())
                                } else {
                                    tr!("`{}` は `{n}` なので、文字列に入れられません", "`{}` is `{n}`, which cannot be put in a string", x.show())
                                };
                                self.push(e("E003", *sp, message));
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
                                    self.push(e("E002", *fsp, tr!("`{rn}` にフィールド `{fname}` はありません（{}）", "`{rn}` has no field `{fname}` ({})", list.join("・"); list.join(", "))));
                                    return None;
                                }
                            };
                            if out.iter().any(|(n, _)| n == fname) {
                                self.push(e("E006", *fsp, tr!("`{fname}` が二度書かれています", "`{fname}` is given twice")));
                                return None;
                            }
                            let x = self.expr(fe, Some(&fty))?;
                            out.push((fname.clone(), x));
                        }
                        let missing: Vec<String> = decl.iter().filter(|(n, t)| !matches!(t, Ty::Opt(_)) && !out.iter().any(|(g, _)| g == n)).map(|(n, _)| n.clone()).collect();
                        if !missing.is_empty() {
                            self.push(e("E004", *sp, tr!("この `{rn}` に {} がありません", "this `{rn}` does not give {}", missing.join("・"); missing.join(", "))));
                            return None;
                        }
                        out.sort_by_key(|(n, _)| decl.iter().position(|(d, _)| d == n).unwrap_or(usize::MAX));
                        TExpr::Record { fields: out, ty: Ty::Record(r) }
                    }
                    Some(Ty::Json) => {
                        let mut out: Vec<(String, TExpr)> = Vec::new();
                        for ((fname, fsp), fe) in fields {
                            if out.iter().any(|(n, _)| n == fname) {
                                self.push(e("E006", *fsp, tr!("`{fname}` が二度書かれています", "`{fname}` is given twice")));
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
                            tr!("`{{…}}` で書いたレコードは、型が決まるところ（引数・出力・`let x: <レコード> = {{…}}`）に書きます", "a record written as `{{…}}` takes its type from where it goes: an argument, an output, or `let x: <record> = {{…}}`"),
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
                        self.push(e("E003", *sp, tr!("空のリストの型がここでは分かりません。リストを渡す先に書くか、`let x: list[T] = []` と書いてください", "the type of an empty list is not known here; write it where a list is expected, or as `let x: list[T] = []`")));
                        return None;
                    }
                };
                if matches!(elem.inner(), Ty::List(_)) {
                    self.push(e("E003", *sp, tr!("リストのリストは書けません。内側のリストはレコードに入れてください", "a list of lists is not supported; put the inner list in a record")));
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
                            self.push(e("E003", parts[0].1, tr!("`none` を渡せるのは、オプショナルな値（`T?`）のところだけです", "`none` is given only where a value may be absent (`T?`)")));
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
                let message = if matches!(got, Ty::Opt(_)) && got.inner().fits(want) {
                    tr!("ここには `{a}` が要りますが、これは `none` になりうる `{b}` です。先に `none` と `some <名前>` で `match` してください", "expected `{a}` here, but this is `{b}`, which may be absent; `match` it with `none` and `some <name>` first")
                } else {
                    tr!("ここには `{a}` が要りますが、これは `{b}` です", "expected `{a}` here, but this is `{b}`")
                };
                self.push(e("E003", ex.span(), message));
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
                                tr!("ここでは `{so_far}` が無いことがあります。先に `none` と `some <名前>` で `match` してください", "`{so_far}` may be absent here; `match` it with `none` and `some <name>` first"),
                            ));
                            return None;
                        }
                        Ty::Json => {
                            self.push(e("E003", *fsp2, tr!("`{so_far}` は `json` で、中を見ずにそのまま運ぶ値です", "`{so_far}` is `json`, which is carried as it is and not looked into")));
                            return None;
                        }
                        ref other => {
                            let n = self.m.ty_name(other);
                            self.push(e("E003", *fsp2, tr!("`{n}` にはフィールドがありません", "`{n}` has no fields")));
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
                            self.push(e("E002", *fsp2, tr!("`{rn}` にフィールド `{f}` はありません（{}）", "`{rn}` has no field `{f}` ({})", list.join("・"); list.join(", "))));
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
                let Text { en: hen, ja: hja } = match expected.map(|t| t.inner()) {
                    Some(Ty::Enum(id)) => tr!("`{}` の値は {} です", "the values of `{}` are {}", self.m.enums[*id].name, self.m.enums[*id].values.join("・"); self.m.enums[*id].name, self.m.enums[*id].values.join(", ")),
                    _ => tr!("値は変数・そのフィールド・文字列・数・true・false のどれかです", "a value is a variable, a field of one, a string, a number, true or false"),
                };
                self.push(e("E002", *fsp, tr!("変数や値 `{first}` はありません", "there is no variable or value `{first}`")).note(Text::new(hja, hen)));
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
                tr!("{list} は、この `for … in parallel` の中と外の両方で値を入れられています。イテレーションは同時に走り、それぞれが自分の変数を持つので、別の名前にしてください", "{list} is set both inside this `for … in parallel` and outside it; the rounds run at the same time and each keeps its own variables, so give them their own names"),
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
