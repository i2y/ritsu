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
use std::path::{Path, PathBuf};

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
    /// the names the flow sets (a `let`, a `for`'s variable and its result, a `match`'s `some`), and
    /// the lines of what sets each: a name here is a variable, whether its type is known or not
    set_lines: BTreeMap<String, BTreeSet<usize>>,
    /// what a declaration whose line has an error leaves out (DESIGN 30), and the line: an input
    /// or a case by its name, an output, a task, a parameter (`<task>.<parameter>`), an error of a
    /// task (`<task>!<error>`)
    left_out: BTreeMap<String, BTreeSet<usize>>,
    /// what a line would say that follows from an error said at another line (DESIGN 30): the
    /// diagnostic, and the lines whose error says it already. Said only when none of those lines has
    /// an error (`followers`).
    followers: Vec<(Diag, BTreeSet<usize>)>,
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
    /// where each variable is first set, for what is said of its name
    var_at: BTreeMap<String, Span>,
    site: usize,
    /// the loops around the statement being lowered: true for `for … in parallel`
    loops: Vec<bool>,
    /// how many `for … in parallel` are around it
    par_depth: usize,
    in_on_failure: bool,
    in_on_cancel: bool,
    /// a variable's type grew to take `none` in this pass over the names
    widened: bool,
    /// the whole numbers written where a rate counts steps other than 1%, by where each is written,
    /// with the rate's unit: W106 says them once their statement is lowered (`numbers`)
    stepped: BTreeMap<(usize, usize), ritsu_units::Unit>,
}

fn e(code: &'static str, sp: Span, message: Text) -> Diag {
    Diag::error(code, sp.line, sp.col, message)
}

/// Why a number written with a unit does not count in the unit of a type.
enum Miscount {
    /// a unit of another dimension or currency, or a rate written in another sign than `%`
    Other,
    /// one that counts, but not to a whole number of the type's unit
    Fraction,
    /// one that counts to a whole number past 64 bits
    Large,
}

/// A number written with a unit (`value` of `unit`), counted in the unit `to` as rulec counts one
/// (ritsu-units: `1kg` is 1000 in `mass[g]`, `5%` is 50 steps of `rate[step 0.1%]`, an amount in yen
/// is the same in `money[円, incl_tax]` and in `money[JPY, excl_tax]`): the whole number that goes on
/// the wire. A bound of a range and a value written where its type is known count alike (DESIGN 1.3,
/// 40).
fn counted(to: &ritsu_units::Unit, value: ritsu_units::Rat, unit: &str) -> Result<i64, Miscount> {
    let counted = match &to.dim {
        // a rate in percent, counted in its steps
        ritsu_units::Dim::Rate if unit == "%" => to.step.and_then(|step| value.checked_div(ritsu_units::Rat::int(100))?.checked_div(step)),
        ritsu_units::Dim::Money(_) => ritsu_units::Unit::money(unit, to.tax).and_then(|from| from.convert(value, to)),
        d => ritsu_units::Unit::quantity(d.word(), unit).and_then(|from| from.convert(value, to)),
    };
    let counted = counted.ok_or(Miscount::Other)?;
    if !counted.is_int() {
        return Err(Miscount::Fraction);
    }
    i64::try_from(counted.num).map_err(|_| Miscount::Large)
}

/// What a number of a statement is written for, as W106 says it and writes the number with its unit.
enum Place {
    /// an argument of a call, or a field of a record: `fee: 0.5%`
    Field(String),
    /// an output of `succeed`: `fee = 0.5%`
    Output(String),
    /// the value of a `let`
    Let(String),
    /// an item of a list, or what a `yield` adds to the list a `for` makes, by the list's place
    Item(String),
    /// an item of the list a `for` goes through, by the loop's variable
    Over(String),
}

impl Place {
    /// The place an item of a list written here is.
    fn item(&self) -> Place {
        match self {
            Place::Over(v) => Place::Over(v.clone()),
            Place::Field(n) | Place::Output(n) | Place::Let(n) | Place::Item(n) => Place::Item(n.clone()),
        }
    }
}

/// A whole number on a rate counted in steps other than 1%, as W106 says it: one step and the rate
/// the number is, as the checks write a rate (`0.1%`, `0.5%`).
struct InSteps {
    step: String,
    value: String,
}

/// What a whole number `n` written for a value of the unit `u` counts, when `u` is a rate counted in
/// steps other than 1% (W106): `5` of `rate[step 0.1%]` is 0.5%. None for a rate in steps of 1%,
/// where the number is the percentage, for money and a quantity, whose type names the unit the
/// number counts (`money[USDc]`, `mass[g]`), and for 0, which is 0% whatever it counts.
fn in_steps(u: &ritsu_units::Unit, n: i64) -> Option<InSteps> {
    let step = u.step?;
    if u.dim != ritsu_units::Dim::Rate || step == ritsu_units::Rat::new(1, 100) || n == 0 {
        return None;
    }
    Some(InSteps { step: ritsu_units::written::value(1, Some(u)), value: ritsu_units::written::value(i128::from(n), Some(u)) })
}

/// A value of a machine's table on an axis of numbers, as rulec writes one (`0.5%`, `300JPY`, `3`),
/// counted in the unit of the input's type `t` as a number written in a `.flow` is ([`counted`]):
/// what a number `held` names is found by (DESIGN 40). None for a value that is no number of `t`.
fn coord_number(x: &str, t: &Ty) -> Option<i64> {
    let (value, unit) = syntax::number_text(x)?;
    match t {
        Ty::Num(to) if !unit.is_empty() => counted(to, value, &unit).ok(),
        Ty::Int if unit.is_empty() && value.is_int() => i64::try_from(value.num).ok(),
        _ => None,
    }
}

/// Whether `n`, a number of the type `t` counted in its unit, is between the ends of a coordinate
/// (each written as rulec writes a value, None where the input's range leaves it open). An end that
/// is no number of `t` takes nothing.
fn held_between((lo, hi): &(Option<String>, Option<String>), n: i64, t: &Ty) -> bool {
    let end = |x: &Option<String>| x.as_ref().map(|x| coord_number(x, t));
    end(lo).is_none_or(|lo| lo.is_some_and(|lo| lo <= n)) && end(hi).is_none_or(|hi| hi.is_some_and(|hi| n <= hi))
}

/// What E003 says of a value `held` gives the input `inp` that is none of the axis's: for an axis of
/// numbers, the values its sets run over; else the values the table names.
fn not_held(text: &str, inp: &str, axis: &crate::rulec::Axis) -> Text {
    let sets: Vec<&(Option<String>, Option<String>)> = axis.ends.iter().flatten().collect();
    let (lo, hi) = (sets.first().and_then(|e| e.0.clone()), sets.last().and_then(|e| e.1.clone()));
    match (sets.is_empty(), lo, hi) {
        (true, _, _) => tr!("`{text}` は `{inp}` の値ではありません（値は {}）", "`{text}` is not a value of `{inp}` ({})", axis.coords.join("・"); axis.coords.join(", ")),
        (false, Some(lo), Some(hi)) => tr!("`{text}` は `{inp}` の値ではありません（値は {lo} から {hi} まで）", "`{text}` is not a value of `{inp}` (its values run from {lo} to {hi})"),
        (false, Some(lo), None) => tr!("`{text}` は `{inp}` の値ではありません（値は {lo} 以上）", "`{text}` is not a value of `{inp}` (its values are {lo} or more)"),
        (false, None, Some(hi)) => tr!("`{text}` は `{inp}` の値ではありません（値は {hi} 以下）", "`{text}` is not a value of `{inp}` (its values are {hi} or less)"),
        (false, None, None) => tr!("`{text}` は `{inp}` の値ではありません", "`{text}` is not a value of `{inp}`"),
    }
}

/// `d`, with the note for a path that starts with `@` where no module of ritsu's is above the
/// `.flow`, so that it is a plain path, when nothing is at `path` (ritsu's DESIGN 17.6).
fn outside(d: Diag, base: &Path, written: &str, path: &Path) -> Diag {
    match ritsu_base::modules::outside_note_for(base, written) {
        Some(n) if !ritsu_base::fs::exists(path) => d.note(n),
        _ => d,
    }
}

/// W902: a call that goes without encryption to a host that is not this machine (DESIGN 1.18), at
/// `at`, said as `said`; `under` is where `plaintext` goes.
fn plain_call(at: Span, said: Text, under: Text) -> Diag {
    let Text { en, ja } = under;
    Diag::warning("W902", at.line, at.col, said)
        .note(tr!("途中のネットワークにいる人は、リクエストとレスポンスと、ヘッダーの鍵を読んだり書き換えたりできます。", "Whoever is on the network between can read and change the requests, the answers, and any key in the headers."))
        .note(tr!("https:// にしてください。ほかの仕組み（サービスメッシュ、プライベートな接続など）で守っているなら、{ja}に `plaintext \"<理由>\"` と書いてください。", "Use https://. If the connection is protected another way (a service mesh, a private link), say so {en} with `plaintext \"<why>\"`."))
}

/// E005 at a call of a rule without outputs (DESIGN 10), at the name called: rulec's `enums` file,
/// which declares enums for rules to read, and from which nothing is generated to call. The note
/// says so, and names one of its enums as the type it can be.
fn nothing_to_call(r: &RuleUse, at: Span) -> Diag {
    let name = &r.name;
    let d = e("E005", at, tr!("規則 `{name}` には出力が無いので、呼べません", "the rule `{name}` has no outputs, so it cannot be called"));
    match r.info.enums.first() {
        Some((enum_name, _)) => d.note(tr!(
            "出力の無い規則は `enums` のファイルで、中にあるのは規則が読む列挙だけです。rulec は、このファイルから呼べる関数を生成しません。列挙は `{name}.{enum_name}` のように型として使ってください",
            "a rule with no outputs is an `enums` file, which holds only enums for rules to read, and rulec generates no function from it to call; use its enums as types, as in `{name}.{enum_name}`"
        )),
        None => d.note(tr!(
            "出力の無い規則は `enums` のファイルで、中にあるのは規則が読む列挙だけです。rulec は、このファイルから呼べる関数を生成しません",
            "a rule with no outputs is an `enums` file, which holds only enums for rules to read, and rulec generates no function from it to call"
        )),
    }
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
                vec![Diag::error("E001", 1, 1, tr!(".flow のファイルは `workflow <名前> v<番号>` で始めてください", "a .flow starts with `workflow <name> v<n>`"))],
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
            source_path: file.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(),
            source_sha256: String::new(),
            fed: vec![],
            package: None,
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
            input_secrets: BTreeMap::new(),
            output_secrets: BTreeMap::new(),
            input_lines: BTreeMap::new(),
            output_lines: BTreeMap::new(),
            history_encrypted: prog.history_encrypted.map(|s| s.line),
            period: None,
            moment_texts: vec![],
            source_reached: file.to_string_lossy().to_string(),
            spellings: Vec::new(),
            written: BTreeMap::new(),
            uses: {
                let mut uses: Vec<crate::model::UseLine> = Vec::new();
                for u in &prog.uses {
                    uses.push(crate::model::UseLine { kind: "rule", name: u.name.0.clone(), path: u.path.clone(), line: u.name.1.line, url: None, plaintext: u.plaintext.as_ref().map(|(w, _)| w.clone()) });
                }
                for u in &prog.dates {
                    uses.push(crate::model::UseLine { kind: "dates", name: u.name.0.clone(), path: u.path.clone(), line: u.name.1.line, url: None, plaintext: None });
                }
                for u in &prog.books {
                    uses.push(crate::model::UseLine { kind: "book", name: u.name.0.clone(), path: u.path.clone(), line: u.name.1.line, url: None, plaintext: None });
                }
                for u in &prog.apis {
                    uses.push(crate::model::UseLine { kind: u.kind.word(), name: u.name.0.clone(), path: u.path.clone(), line: u.name.1.line, url: u.url.clone(), plaintext: u.plaintext.as_ref().map(|(w, _)| w.clone()) });
                }
                uses.sort_by_key(|u| u.line);
                uses
            },
        },
        enum_ix: BTreeMap::new(),
        record_ix: BTreeMap::new(),
        made_enums: BTreeSet::new(),
        set_lines: BTreeMap::new(),
        left_out: BTreeMap::new(),
        followers: Vec::new(),
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
        var_at: BTreeMap::new(),
        site: 0,
        loops: vec![],
        par_depth: 0,
        in_on_failure: false,
        in_on_cancel: false,
        widened: false,
        stepped: BTreeMap::new(),
    };
    let base = file.parent().unwrap_or(Path::new("."));
    lw.rules(base);
    lw.dates(base);
    lw.books(base);
    lw.apis(base);
    lw.import_enums(base);
    lw.local_types();
    lw.period();
    lw.io();
    lw.tasks();
    lw.plaintexts();
    lw.service();
    lw.cases();
    lw.left_out_of_declarations();
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
    lw.names_around_rules();
    lw.made_enums_named_none();
    lw.parallel_scopes();
    lw.m.vars = lw.var_ty.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    // what follows from an error said at another line in the file is not said again (DESIGN 30)
    lw.followers_said();
    // what a `use` that could not be read leaves unsaid (DESIGN 27): its E005 or E016 is said once, at its
    // line, and no line that writes what it would have given says that it is not there
    let unread = lw.unread_names();
    if !unread.is_empty() {
        let quiet = unread_lines(prog, &unread, &lw.m, &lw.var_ty);
        lw.diags.retain(|d| !quiet.contains(&d.line));
    }
    let ok = !crate::diag::has_errors(&lw.diags);
    if ok {
        crate::periods::insert(&mut lw.m);
        let places = crate::spellings::Places { prog, made_at: &lw.made_at, use_spans: &lw.use_spans };
        lw.m.spellings = crate::spellings::find(&lw.m, &places);
    }
    (if ok { Some(lw.m) } else { None }, lw.diags)
}

impl<'a> Lowerer<'a> {
    fn push(&mut self, d: Diag) {
        self.diags.push(d);
    }

    /// The fields of a record by name, for the list E002 gives of the fields there are. A record of
    /// the `.flow`'s own is listed as it is written: a field whose type could not be read (a name
    /// from a `use` that could not be read, DESIGN 27, or a type that is not there) is there all the
    /// same.
    fn field_names(&self, rec: usize) -> Vec<String> {
        match self.written_record(rec) {
            Some(w) => {
                let mut out: Vec<String> = Vec::new();
                for f in &w.fields {
                    if !out.contains(&f.name.0) {
                        out.push(f.name.0.clone());
                    }
                }
                out
            }
            None => self.m.records[rec].fields.iter().map(|(n, _)| n.clone()).collect(),
        }
    }

    /// Whether `field` is written in a record of the `.flow`'s own, though the record has no field
    /// of that name: its type could not be read. What is wrong is said at the line of the field (or
    /// of the `use` that could not be read), and a line that writes the field says nothing more.
    fn unread_field(&self, rec: usize, field: &str) -> bool {
        self.written_record(rec).is_some_and(|w| w.fields.iter().any(|f| f.name.0 == field))
    }

    /// The record of the `.flow`'s own as it is written, for a record of the model.
    fn written_record(&self, rec: usize) -> Option<&'a syntax::RecordDecl> {
        let r = &self.m.records[rec];
        match r.origin {
            RecordOrigin::Local => self.prog.records.iter().find(|x| x.name.0 == r.name),
            _ => None,
        }
    }

    /// The names a `use` or an `import enum` gives that could not be read: a rule, a dates file, a
    /// book, an enum (E005), a `.proto` or an OpenAPI document (E016). A name another `use` gives
    /// all the same (one given twice, E006) is read: what is written with it is checked as that one.
    fn unread_names(&self) -> BTreeSet<String> {
        let read = |n: &str| self.rule_ix.contains_key(n) || self.date_files.contains_key(n) || self.book_ix.contains_key(n) || self.apis.contains_key(n);
        let mut out = BTreeSet::new();
        let used = self.prog.uses.iter().map(|u| &u.name).chain(self.prog.dates.iter().map(|u| &u.name)).chain(self.prog.books.iter().map(|u| &u.name)).chain(self.prog.apis.iter().map(|u| &u.name));
        out.extend(used.filter(|n| !read(&n.0)).map(|n| n.0.clone()));
        for im in &self.prog.imports {
            let name = im.rename.as_ref().map_or(&im.enum_name.0, |r| &r.0);
            if !self.enum_ix.contains_key(name) && !read(name) {
                out.insert(name.clone());
            }
        }
        out
    }

    /// The file a line of the `.flow` names, from the directory `base` of the `.flow` (ritsu's
    /// DESIGN 17.6). None when the module of ritsu's the `.flow` is in refuses the path, or the path
    /// is a dependency's file that is not there, which is said with `code`, in the words every
    /// language says it in.
    fn follow(&mut self, base: &Path, written: &str, code: &'static str, sp: Span) -> Option<PathBuf> {
        match crate::sources::follow(base, written) {
            Err(r) => {
                self.push(e(code, sp, r.text()));
                None
            }
            Ok(p) => match ritsu_base::modules::missing(&p).filter(|_| !ritsu_base::fs::exists(&p)) {
                Some(t) => {
                    self.push(e(code, sp, t));
                    None
                }
                None => Some(p),
            },
        }
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
            let Some(path) = self.follow(base, &u.path, "E016", *sp) else { continue };
            match crate::apis::load(u.kind, &path) {
                Ok(doc) => {
                    self.apis.insert(name.clone(), (u.kind, crate::apis::Api { name: name.clone(), doc, url: u.url.clone() }));
                }
                Err(why) => self.push(outside(e("E016", *sp, tr!("API `{}` を読めませんでした", "could not read the API `{}`", u.path)).note(why), base, &u.path, &path)),
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
                self.push(e("E002", *sp, tr!("API `{name}` はありません。`use {}` で読んだものを書いてください", "there is no API `{name}`; name one read by `use {}`", kind.word())));
                None
            }
        }
    }

    /// The enums the `import enum` lines bring in (DESIGN 1.21): each is asked of rulec through the
    /// port, which answers for an `enums` file as for a rule, and becomes an enum of the flow named
    /// as the line says, with the values of the file (rulec's names of them) and, where the file
    /// writes them in order, the order. From here on it is one the flow writes with `enum`: nothing
    /// after this knows where it came from. The file that cannot be read, or is a rule, is E005; an
    /// enum the file does not have is E002; a name the flow gives twice is E006 (`local_types`
    /// says it for an `enum` of the same name).
    fn import_enums(&mut self, base: &Path) {
        for im in &self.prog.imports {
            let (word, wsp) = &im.enum_name;
            let Some(path) = self.follow(base, &im.path, "E005", im.path_at) else { continue };
            let info = match rulec::load(&path) {
                Ok(i) => i,
                Err(said) => {
                    let d = e("E005", im.path_at, tr!("`enums` ファイル `{}` を読めませんでした", "could not read the `enums` file `{}`", im.path));
                    self.push(outside(crate::sources::said_notes(&said).into_iter().fold(d, Diag::note), base, &im.path, &path));
                    continue;
                }
            };
            if !info.outputs.is_empty() {
                self.push(e("E005", im.path_at, tr!("`{}` は `enums` ファイルではなく、規則です", "`{}` is a rule, not an `enums` file", im.path)).note(tr!(
                    "`import enum` が読めるのは、`enums <名前> v1` で始まるファイルの列挙です。規則の出力にある列挙は、`use rule` で読み、`<規則>.<列挙>` と書きます。",
                    "`import enum` reads the enums of a file that starts `enums <name> v1`; the enum of a rule's output is read with `use rule` and written `<rule>.<enum>`."
                )));
                continue;
            }
            let Some((en, values)) = info.enums.iter().find(|(n, _)| n == word) else {
                let have: Vec<String> = info.enums.iter().map(|(n, _)| format!("`{n}`")).collect();
                self.push(e("E002", *wsp, tr!("`{}` に列挙 `{word}` はありません", "`{}` has no enum `{word}`", im.path)).note(tr!(
                    "そのファイルにある列挙: {}。",
                    "The enums in that file: {}.",
                    have.join("、");
                    have.join(", ")
                )));
                continue;
            };
            let (name, sp) = im.rename.clone().unwrap_or_else(|| (en.clone(), *wsp));
            if self.enum_ix.contains_key(&name) || self.prog.records.iter().any(|r| r.name.0 == name) {
                self.push(e("E006", sp, tr!("`{name}` が二度宣言されています", "`{name}` is declared twice")).note(tr!(
                    "`import enum` の列挙は、`-> <名前>` でこの flow での名前を付け替えられます。",
                    "An enum of an `import enum` can be given another name in this flow with `-> <name>`."
                )));
                continue;
            }
            self.enum_ix.insert(name.clone(), self.m.enums.len());
            self.made_at.entry(name.clone()).or_insert(sp);
            let declared = Some(self.declared(&path, en, None));
            self.m.enums.push(EnumDef { name, values: values.clone(), ordered: info.ordered.contains(en), from: Some((im.path.clone(), en.clone())), declared });
        }
    }

    fn rules(&mut self, base: &Path) {
        for u in &self.prog.uses {
            let (name, sp) = &u.name;
            if self.rule_ix.contains_key(name) {
                self.push(e("E006", *sp, tr!("規則 `{name}` が二度読み込まれています", "the rule `{name}` is used twice")));
                continue;
            }
            let Some(path) = self.follow(base, &u.path, "E005", *sp) else { continue };
            let info = match rulec::load(&path) {
                Ok(i) => i,
                Err(said) => {
                    // what rulec says of the rule instead, or what the port this dandori runs with says
                    let d = e("E005", *sp, tr!("規則 `{}` を読めませんでした", "could not read the rule `{}`", u.path));
                    self.push(outside(crate::sources::said_notes(&said).into_iter().fold(d, Diag::note), base, &u.path, &path));
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
                // an enum the rule reads whole from an `enums` file is that file's; one it declares
                // itself is its own file's (an `enums` file used as a rule declares its own)
                let declared = match info.origins.get(en) {
                    Some((file, there)) => self.declared(file, there, None),
                    None => self.declared(&path, en, (!info.outputs.is_empty()).then(|| name.clone())),
                };
                self.enum_ix.insert(q.clone(), self.m.enums.len());
                self.m.enums.push(EnumDef { name: q, values: values.clone(), ordered: info.ordered.contains(en), from: None, declared: Some(declared) });
            }
            let fields: Vec<(String, Ty)> = info.outputs.iter().map(|c| (c.name.clone(), self.rty(name, &c.ty))).collect();
            let ranges = info.outputs.iter().filter_map(|c| Some((c.name.clone(), c.range()?))).collect();
            let rec = self.m.records.len();
            self.m.records.push(RecordDef { name: format!("{name}.outputs"), fields, ranges, secrets: BTreeMap::new(), origin: RecordOrigin::RuleOutputs(ix) });
            self.rule_ix.insert(name.clone(), ix);
            let connect = self.rule_connect(u, &info);
            // the `enums` files the rule reads enums from, by the modules rulec generates for each,
            // asked of the port as a rule is (DESIGN 28.8)
            let mut enums_modules: Vec<rulec::EnumsModule> = Vec::new();
            for (file, _) in info.origins.values() {
                if let Ok(owner) = rulec::load(file) {
                    // one file reached by two spellings of its path is one module
                    if !enums_modules.iter().any(|m| m.python == owner.python.module && m.go == owner.go.module) {
                        enums_modules.push(rulec::EnumsModule { python: owner.python.module.clone(), go: owner.go.module.clone(), file: file.clone() });
                    }
                }
            }
            enums_modules.sort();
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
                enums_modules,
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
            let Some(path) = self.follow(base, &u.path, "E005", *sp) else { continue };
            let facts = match crate::sources::dates(&path) {
                Ok(f) => f,
                Err(said) => {
                    let d = e("E005", *sp, tr!("日付のファイル `{}` を読めませんでした", "could not read the dates file `{}`", u.path));
                    self.push(outside(crate::sources::said_notes(&said).into_iter().fold(d, Diag::note), base, &u.path, &path));
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
                self.m.records.push(RecordDef { name: callee.clone(), fields, ranges: BTreeMap::new(), secrets: BTreeMap::new(), origin: RecordOrigin::RuleOutputs(ix) });
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
                    ordered: vec![],
                    origins: BTreeMap::new(),
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
                        minutes: f.at,
                        offset: facts.calendar.as_ref().and_then(|c| c.offset),
                    }),
                    enums_modules: vec![],
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
            let Some(path) = self.follow(base, &u.path, "E005", *sp) else { continue };
            let facts = match crate::sources::book(&path) {
                Ok(f) => f,
                Err(said) => {
                    let d = e("E005", *sp, tr!("帳簿 `{}` を読めませんでした", "could not read the book `{}`", u.path));
                    self.push(outside(crate::sources::said_notes(&said).into_iter().fold(d, Diag::note), base, &u.path, &path));
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
                self.m.enums.push(EnumDef { name: format!("{rname}.{}", mc.state_enum), values: mc.states.clone(), ordered: false, from: None, declared: None });
                // the key's parameters, in the order the transfer declares them: what a post and a void take
                let mut fields: Vec<(String, Ty)> = t.params.iter().filter(|p| t.key.contains(&p.name)).map(|p| (p.name.clone(), Ty::Str)).collect();
                fields.push(("state".to_string(), Ty::Enum(state)));
                let ix = self.m.rules.len();
                let rec = self.m.records.len();
                self.record_ix.insert(rname.clone(), rec);
                self.m.records.push(RecordDef { name: rname.clone(), fields, ranges: BTreeMap::new(), secrets: BTreeMap::new(), origin: RecordOrigin::Hold { book: bix, transfer: t.name.clone() } });
                let none = ritsu_ports::Call { module: String::new(), function: String::new(), input_type: String::new(), params: vec![], outputs: vec![], enums: vec![] };
                let info = rulec::RuleInfo {
                    rule: facts.name.clone(),
                    version: format!("v{}", facts.version),
                    path: path.clone(),
                    sha256: facts.sha256.clone(),
                    inputs: vec![],
                    outputs: vec![],
                    enums: vec![(mc.state_enum.clone(), mc.states.clone())],
                    ordered: vec![],
                    origins: BTreeMap::new(),
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
                self.m.rules.push(RuleUse { name: rname, info, lambda: None, local: false, connect: None, connection: None, outputs: rec, line: sp.line, kind: RuleKind::Hold { book: bix, transfer: t.name.clone() }, enums_modules: vec![] });
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
    /// to say it. A rule without outputs (an `enums` file) has no service: nothing calls it, since a
    /// call of it is E005 where it is called, as it is for every other way of calling (DESIGN 10).
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
                tr!("`connection` を書けるのは、`connect` で呼ぶ規則だけです。`lambda` の規則は、Step Functions が接続なしで呼びます", "`connection` is for a rule called by `connect`; Step Functions invokes a rule's `lambda` without one"),
            ));
        }
        let (url, csp) = u.connect.as_ref()?;
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            self.push(e(
                "E007",
                *csp,
                tr!("`connect` には、規則のサービスの場所を http:// か https:// で書いてください", "`connect` says where the rule's service is, with http:// or https://"),
            ));
            return None;
        }
        if info.outputs.is_empty() {
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
            RType::Enum { name: n, .. } => Ty::Enum(*self.enum_ix.get(&format!("{rule}.{n}")).expect("rule enums are registered first")),
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
            self.m.enums.push(EnumDef { name: name.clone(), values, ordered: false, from: None, declared: None });
        }
        // records: names first, so that a field can have a record type declared further down
        for r in &self.prog.records {
            let (name, sp) = &r.name;
            if self.record_ix.contains_key(name) {
                self.push(e("E006", *sp, tr!("`{name}` が二度宣言されています", "`{name}` is declared twice")));
                continue;
            }
            self.record_ix.insert(name.clone(), self.m.records.len());
            self.m.records.push(RecordDef { name: name.clone(), fields: vec![], ranges: BTreeMap::new(), secrets: BTreeMap::new(), origin: RecordOrigin::Local });
        }
        for r in &self.prog.records {
            let ix = match self.record_ix.get(&r.name.0) {
                Some(i) => *i,
                None => continue,
            };
            let mut fields = Vec::new();
            let mut ranges = BTreeMap::new();
            let mut secrets = BTreeMap::new();
            for f in &r.fields {
                if fields.iter().any(|(n, _): &(String, Ty)| *n == f.name.0) {
                    self.push(e("E006", f.name.1, tr!("フィールド `{}` が二度書かれています", "the field `{}` is written twice", f.name.0)));
                    continue;
                }
                if let Some(t) = self.ty(&f.ty) {
                    if let Some(rg) = self.range(&t, f.range.as_ref()) {
                        ranges.insert(f.name.0.clone(), rg);
                    }
                    if let Some(ssp) = f.secret {
                        secrets.insert(f.name.0.clone(), self.written_mark(ssp));
                    }
                    fields.push((f.name.0.clone(), t));
                }
            }
            self.m.records[ix].fields = fields;
            self.m.records[ix].ranges = ranges;
            self.m.records[ix].secrets = secrets;
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
                // `rate[%]`: a rate's brackets hold its step, not a unit
                Err(ritsu_units::Problem::Shape(_)) if u.starts_with("rate[") => {
                    self.push(e("E002", *sp, tr!("型 `{u}` はありません", "there is no type `{u}`")).same_in("type", u.clone()).note(tr!(
                        "率の型には、`rate[step 1%]` のように刻みを書いてください",
                        "the type of a rate writes its step, as in `rate[step 1%]`"
                    )));
                    None
                }
                Ok(_) | Err(ritsu_units::Problem::Shape(_)) | Err(ritsu_units::Problem::Dimension(_)) => {
                    self.push(e("E002", *sp, tr!("型 `{u}` はありません", "there is no type `{u}`")).same_in("type", u.clone()).note(type_hint()));
                    None
                }
                Err(why) => {
                    self.push(e("E002", *sp, tr!("型 `{u}` はありません", "there is no type `{u}`")).same_in("type", u.clone()).note(why.text()));
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
                                self.push(e("E002", sp, tr!("型 `{text}` はありません", "there is no type `{text}`")).same_in("type", text.clone()).note(
                                    tr!("型を作れるのは、`use proto` で読んだ `.proto` からだけです。`{api}` は {what_ja}で、dandori はその{shapes_ja}から型を作りません", "types are made from a `.proto` read by `use proto`; `{api}` is {what_en}, and its {shapes_en} are not made into types"),
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
                self.push(e("E002", sp, tr!("型 `{text}` はありません", "there is no type `{text}`")).same_in("type", text.clone()).note(hint));
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
            let mut d = e("E002", sp, tr!("型 `{text}` はありません", "there is no type `{text}`")).same_in("type", text.to_string()).note(tr!("`{api}` のメッセージと列挙は {} です", "the messages and enums of `{api}` are {}", list.ja; list.en));
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
            MadeTy::Date => Ty::Date,
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
        self.m.enums.push(EnumDef { name, values, ordered: false, from: None, declared: None });
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
        self.m.records.push(RecordDef { name, fields: vec![], ranges: BTreeMap::new(), secrets: BTreeMap::new(), origin: RecordOrigin::Proto { api: api.to_string() } });
        let mut fields = Vec::new();
        let mut ranges = BTreeMap::new();
        let mut secrets = BTreeMap::new();
        for f in crate::apis::made_fields(pf, full) {
            if let Some(r) = &f.redact {
                secrets.insert(f.name.clone(), self.proto_mark(r));
            }
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
        self.m.records[ix].secrets = secrets;
        ix
    }

    /// A path dandori reaches, as a message says it: from the `.flow`'s directory, where it is under it.
    fn shown_path(&self, p: &Path) -> String {
        p.strip_prefix(&self.dir).unwrap_or(p).to_string_lossy().to_string()
    }

    /// When two types that are not one hold two enums: a note that says where each enum is
    /// declared, and, when the two have the same values, how to make them one type (DESIGN 28).
    /// None when they are no enums, or one is.
    fn enums_apart(&self, a: &Ty, b: &Ty) -> Option<Text> {
        let (x, y) = (a.enum_inside()?, b.enum_inside()?);
        if self.m.same_enum(x, y) {
            return None;
        }
        let whence = |ix: EnumId| -> Option<Text> {
            let en = &self.m.enums[ix];
            let n = &en.name;
            Some(match &en.declared {
                Some(Declared { shown, name, rule: Some(r), .. }) => tr!("`{n}` は、規則 `{r}`（`{shown}`）が自分で宣言した列挙 `{name}` です", "`{n}` is the enum `{name}` that the rule `{r}` (`{shown}`) declares itself"),
                Some(Declared { shown, name, rule: None, .. }) => tr!("`{n}` は、`enums` ファイル `{shown}` の列挙 `{name}` です", "`{n}` is the enum `{name}` of the `enums` file `{shown}`"),
                None if self.prog.enums.iter().any(|d| d.name.0 == *n) => tr!("`{n}` は、このフローが宣言した列挙です", "`{n}` is an enum this flow declares"),
                None => return None,
            })
        };
        let (wx, wy) = (whence(x)?, whence(y)?);
        let same = {
            let (mut p, mut q) = (self.m.enums[x].values.clone(), self.m.enums[y].values.clone());
            p.sort();
            q.sort();
            p == q
        };
        let advice = if same {
            tr!(
                "別の場所で宣言した列挙は、値が同じでも別の型です。一つの型にするには、列挙を `enums` ファイルに置き、規則とフローのそれぞれで `import enum` で読んでください。",
                "Enums declared at two places are two types, even with the same values; to make them one, put the enum in an `enums` file and read it with `import enum` in each rule and in the flow."
            )
        } else {
            Text::same(String::new())
        };
        let ja = format!("{}。{}。{}", wx.ja, wy.ja, advice.ja);
        let en = format!("{}; {}. {}", wx.en, wy.en, advice.en);
        Some(Text::new(ja.trim_end(), en.trim_end()))
    }

    /// Where an enum of a file dandori reads is declared (`EnumDef::declared`): the file made
    /// canonical, the file from the `.flow`'s directory, and the enum's name there.
    fn declared(&self, file: &Path, name: &str, rule: Option<String>) -> Declared {
        Declared {
            file: ritsu_base::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf()),
            shown: ritsu_base::paths::seen_from(Path::new(&self.m.source_reached), file),
            name: name.to_string(),
            rule,
        }
    }

    /// The mark of a property of an OpenAPI document (DESIGN 1.18): its line, read from the document
    /// as written (ritsu-base's reader keeps the lines), and its JSON Pointer.
    fn doc_mark(&self, file: &Path, d: &crate::apis::DocMark) -> SecretMark {
        let line = crate::sources::read(file).ok().and_then(|text| ritsu_base::yaml::read_json(&text).ok()).and_then(|n| n.pointer(&d.pointer).map(|x| x.line)).unwrap_or(1);
        let detail = (!d.mark.detail.is_empty()).then(|| Text::same(d.mark.detail.clone()));
        SecretMark { file: file.to_string_lossy().to_string(), line, mark: d.mark.keyword.to_string(), at: format!("{}#{}", self.shown_path(file), d.pointer), detail }
    }

    /// The mark of a field of a `.proto` that `debug_redact` marks secret (DESIGN 1.18).
    fn proto_mark(&self, r: &crate::proto::Redact) -> SecretMark {
        let at = format!("{}:{}", self.shown_path(&r.file), r.line);
        let detail = r.by.as_ref().map(|(value, vf, vl)| {
            let vat = format!("{}:{vl}", self.shown_path(vf));
            tr!("値 {value} は {vat} で `debug_redact = true` を付けたもの", "the value {value} is marked `debug_redact = true` at {vat}")
        });
        SecretMark { file: r.file.to_string_lossy().to_string(), line: r.line, mark: r.mark.clone(), at, detail }
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
                        e("E006", at(owner), tr!("規則 `{owner}` のアクティビティ `{act}` は、rulec が規則 `{n}` のために生成するコードの名前でもあるので、rules.py で二つがぶつかります", "`{act}`, the activity of the rule `{owner}`, is also a name of the code rulec generates for the rule `{n}`, and the two clash in rules.py"))
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
                    None => (tr!("規則 `{n}` は、rulec が生成するコードで `{name}` という名前になり、dandori がそのまわりに生成するコードの名前とぶつかります", "the rule `{n}` is `{name}` in the code rulec generates, a name the code dandori writes around it uses already"), fix_rule.clone()),
                    Some(en) => (tr!("規則 `{n}` の列挙 `{en}` は、rulec が生成するコードで `{name}` という名前になり、dandori がそのまわりに生成するコードの名前とぶつかります", "the enum `{en}` of the rule `{n}` is `{name}` in the code rulec generates, a name the code dandori writes around it uses already"), fix_enum.clone()),
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
                Some(first) => out.push(e("E006", at(n), tr!("規則 `{first}` と `{n}` は、dandori が生成するコードで同じアクティビティ `{act}` になります。どちらかの名前を変えてください", "the rules `{first}` and `{n}` would be one activity, `{act}`, in the code dandori writes; rename one"))),
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
                    e("E006", sp, tr!("タスク `{tn}` と規則 `{n}` は、dandori が生成するコードで同じアクティビティ `{act}` になります", "the task `{tn}` and the rule `{n}` would be one activity, `{act}`, in the code dandori writes"))
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
        let dir = self.dir.clone();
        let child = self.follow(&dir, path, "E015", sp)?;
        let (message, note) = match crate::check::child(&child) {
            Ok(model) => return Some(Box::new(ChildFlow { path: path.to_string(), model })),
            Err(ChildTrouble::Cycle(chain)) => {
                let names: Vec<String> = chain.iter().map(|p| p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()).collect();
                (
                    tr!("`{path}` はこのフローをまた走らせます（{}）。フローは、直接にもほかのフローを通しても、自分自身を走らせられません", "`{path}` runs this flow again ({}); a flow cannot run itself, directly or through others", names.join(" → ")),
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
        self.push(outside(d, &dir, path, &child));
        None
    }

    /// The range written after a type, which only a number, a value of an ordered enum, a date or a
    /// time can have (or a `?` or a list of them).
    fn range(&mut self, t: &Ty, r: Option<&RangeDecl>) -> Option<Range> {
        let r = r?;
        let mut inner = t;
        while let Ty::Opt(x) | Ty::List(x) = inner {
            inner = x;
        }
        if !matches!(inner, Ty::Int | Ty::Num(_) | Ty::Enum(_) | Ty::Date | Ty::Timestamp) {
            let n = self.m.ty_name(t);
            self.push(e("E003", r.span, tr!("範囲を書けるのは、数（`int` か単位の付いた数）、順序のある列挙、日付と時刻だけです。これは `{n}` です", "a range is for a number (`int` or a unit), an ordered enum, a date or a time, and this is `{n}`")));
            return None;
        }
        if let (Some((_, sp)), false) = (&r.offset, matches!(inner, Ty::Timestamp)) {
            self.push(e("E003", *sp, tr!("`offset` を書けるのは、時刻の範囲と `now range` だけです。日付は時刻を持たないので、オフセットで読み替えません", "`offset` is written only after a range of times and in `now range`: a date has no time of day for an offset to read")));
            return None;
        }
        match inner {
            Ty::Enum(en) => return self.enum_range(*en, r),
            Ty::Date => return self.day_range(r),
            Ty::Timestamp => return self.moment_range(r).map(|(rg, _)| rg),
            _ => {}
        }
        let lo = match &r.lo {
            Some(b) => Some(self.bound(inner, b, ">=")?),
            None => None,
        };
        let hi = match &r.hi {
            Some(b) => Some(self.bound(inner, b, "<=")?),
            None => None,
        };
        let rg = Range { lo, hi };
        if let (Some(lo), Some(hi)) = (rg.lo, rg.hi) {
            if lo > hi {
                let said = number_range_text(inner, &rg).unwrap_or_else(|| rg.show());
                self.push(e("E003", r.span, tr!("`{said}` に入る数はありません", "no number is in `{said}`")));
                return None;
            }
        }
        Some(rg)
    }

    /// `range >=basic <=gold` on a place of the ordered enum `en`, as a rule writes one on its input
    /// (rulec's §15.210): the places of the values at its ends in the enum's order, an end left out
    /// the first or the last value. E003 for an enum with no order (a `.flow`'s own, a `.proto`'s,
    /// a rule's declared with `|`), an end that is none of its values, and a range no value is in.
    fn enum_range(&mut self, en: EnumId, r: &RangeDecl) -> Option<Range> {
        let def = &self.m.enums[en];
        let (n, values, ordered) = (def.name.clone(), def.values.clone(), def.ordered);
        if !ordered {
            self.push(e("E003", r.span, tr!("列挙 `{n}` には順序が無いので、範囲は書けません", "the enum `{n}` has no order, so it takes no range")).note(tr!(
                "範囲を書けるのは、規則が値を `<` で順に並べた列挙です（`enum tier = basic < silver < gold`）",
                "a range is for an enum a rule declares in order, with `<` (`enum tier = basic < silver < gold`)"
            )));
            return None;
        }
        let order = values.join(" < ");
        let mut at = |b: &syntax::Bound| -> Option<usize> {
            let (word, sp) = match b {
                syntax::Bound::Value { name, span } => match values.iter().position(|v| v == name) {
                    Some(i) => return Some(i),
                    None => (name.clone(), *span),
                },
                syntax::Bound::Int(k, _) => (k.to_string(), r.span),
                syntax::Bound::Quantity { raw, span, .. } => (raw.clone(), *span),
                syntax::Bound::Date { text, span } => (text.clone(), *span),
            };
            self.push(e("E003", sp, tr!("範囲の端 `{word}` は `{n}` の値ではありません", "the bound `{word}` is not a value of `{n}`")).note(tr!("`{n}` の値は、順に {order} です", "the values of `{n}` are, in order, {order}")));
            None
        };
        let lo = match &r.lo {
            Some(b) => at(b)?,
            None => 0,
        };
        let hi = match &r.hi {
            Some(b) => at(b)?,
            None => values.len().saturating_sub(1),
        };
        if lo > hi {
            let text = format!(">={} <={}", values[lo], values[hi]);
            self.push(e("E003", r.span, tr!("`{text}` に入る `{n}` の値はありません", "no value of `{n}` is in `{text}`")).note(tr!("`{n}` の値は、順に {order} です", "the values of `{n}` are, in order, {order}")));
            return None;
        }
        Some(Range { lo: Some(lo as i64), hi: Some(hi as i64) })
    }

    /// One end of a range, counted in the unit of the number `ty`: a whole number as it is; a number
    /// with a unit, as rulec counts one ([`counted`]). E003 for a unit of another dimension, a unit
    /// on `int`, and an end that does not come to a whole number in the type's unit, which is what
    /// goes on the wire. W106 for a whole number on a rate counted in steps other than 1%: `<=500`
    /// of `rate[step 0.1%]` is 50%, and `<=50%` says so (`op`, the end's `>=` or `<=`, writes it).
    fn bound(&mut self, ty: &Ty, b: &syntax::Bound, op: &str) -> Option<i64> {
        let (value, unit, raw, sp) = match b {
            syntax::Bound::Int(n, sp) => {
                if let Ty::Num(u) = ty {
                    if let Some(rate) = in_steps(u, *n) {
                        let d = Diag::warning("W106", sp.line, sp.col, tr!(
                            "範囲の端に書いた `{n}` は、{} の刻みの数として読みます。{} です",
                            "`{n}`, written as an end of the range, counts steps of {}: it is {}",
                            rate.step, rate.value; rate.step, rate.value
                        ))
                        .note(tr!(
                            "読む人に率が分かるよう、`{op}{}` と書いてください。範囲は変わりません",
                            "Write it `{op}{}`, so that a reader sees the rate; the range is the same",
                            rate.value; rate.value
                        ));
                        self.push(d);
                    }
                }
                return Some(*n);
            }
            syntax::Bound::Quantity { value, unit, raw, span } => (*value, unit.as_str(), raw.as_str(), *span),
            // a value of an enum, or a day, where a number goes
            syntax::Bound::Value { name, span } | syntax::Bound::Date { text: name, span } => {
                let tn = self.m.ty_name(ty);
                self.push(e("E003", *span, tr!("範囲の端 `{name}` は `{tn}` の値ではありません", "the bound `{name}` is not a value of `{tn}`")));
                return None;
            }
        };
        let tn = self.m.ty_name(ty);
        let not_of = tr!("範囲の端 `{raw}` は `{tn}` の値ではありません", "the bound `{raw}` is not a value of `{tn}`");
        let Ty::Num(to) = ty else {
            self.push(e("E003", sp, not_of).note(tr!("`int` の端は、単位を付けずに書いてください", "a bound of `int` is written without a unit")));
            return None;
        };
        match counted(to, value, unit) {
            Ok(n) => Some(n),
            Err(Miscount::Other) => {
                self.push(e("E003", sp, not_of));
                None
            }
            Err(Miscount::Fraction) => {
                self.push(
                    e("E003", sp, tr!("範囲の端 `{raw}` を `{tn}` で数えると、整数になりません", "the bound `{raw}`, counted in `{tn}`, is not a whole number")).note(tr!(
                        "値は型の単位で数えた整数として受け渡すので、範囲の端も、その単位に換算して整数にならなければなりません",
                        "a value travels as a whole number in the unit of its type, so a bound must come to one too"
                    )),
                );
                None
            }
            Err(Miscount::Large) => {
                self.push(e("E003", sp, tr!("範囲の端 `{raw}` を `{tn}` で数えると、64 ビットの整数に収まりません", "the bound `{raw}`, counted in `{tn}`, does not fit a whole number of 64 bits")));
                None
            }
        }
    }

    /// A number with a unit written as a value (`fee: 0.5%`, DESIGN 40), counted in the unit of the
    /// place's type, `expected`, as a bound of a range is ([`counted`]): the whole number that goes on
    /// the wire, as if the `.flow` wrote it. E003 where the type is not known (a `let` that writes no
    /// type, a `match`, a value of `json`), for a type that is no number with a unit (`int`, `string`
    /// …), for a unit of another dimension or currency, and for a number that does not come to a
    /// whole number of the type's unit.
    fn quantity(&mut self, value: ritsu_units::Rat, unit: &str, raw: &str, sp: Span, expected: Option<&Ty>) -> Option<i64> {
        let Some(place) = expected.map(Ty::inner) else {
            self.push(
                e(
                    "E003",
                    sp,
                    tr!(
                        "単位の付いた数 `{raw}` は、型の分かる所（呼び出しの引数、`succeed` の出力、型の分かるフィールド・要素・変数）に書いてください",
                        "a number with a unit, `{raw}`, goes where its type is known: an argument of a call, an output of `succeed`, or a field, an item or a variable whose type is known"
                    ),
                )
                .note(tr!(
                    "値は型の単位で数えた整数として受け渡すので、型が分からないと、`{raw}` をどの整数として渡すかが決まりません",
                    "a value travels as a whole number in the unit of its type, so where the type is not known, nothing says what whole number `{raw}` is"
                )),
            );
            return None;
        };
        let tn = self.m.ty_name(place);
        let not_of = tr!("`{raw}` は `{tn}` の値ではありません", "`{raw}` is not a value of `{tn}`");
        let Ty::Num(to) = place else {
            let d = e("E003", sp, not_of);
            self.push(match place {
                Ty::Int => d.note(tr!("`int` の値は、単位を付けずに書いてください", "a value of `int` is written without a unit")),
                Ty::Json => d.note(tr!(
                    "`json` の値には単位が無いので、`{raw}` をどの整数として渡すかが決まりません。受け渡す整数を書いてください",
                    "a value of `json` has no unit, so nothing says what whole number `{raw}` is; write the whole number that is passed"
                )),
                _ => d,
            });
            return None;
        };
        match counted(to, value, unit) {
            Ok(n) => Some(n),
            Err(Miscount::Other) => {
                self.push(e("E003", sp, not_of));
                None
            }
            Err(Miscount::Fraction) => {
                self.push(e("E003", sp, tr!("`{raw}` を `{tn}` で数えると、整数になりません", "`{raw}`, counted in `{tn}`, is not a whole number")).note(tr!(
                    "値は型の単位で数えた整数として受け渡すので、単位を付けて書いた数も、その単位に換算して整数にならなければなりません",
                    "a value travels as a whole number in the unit of its type, so a number written with a unit must come to one too"
                )));
                None
            }
            Err(Miscount::Large) => {
                self.push(e("E003", sp, tr!("`{raw}` を `{tn}` で数えると、64 ビットの整数に収まりません", "`{raw}`, counted in `{tn}`, does not fit a whole number of 64 bits")));
                None
            }
        }
    }

    /// `range >=2026-01-01 <=2027-12-31` on a date: its days, counted from 1970-01-01 as koyomi,
    /// rulec and kaname count them. E003 for an end that is no day, a day the calendar does not
    /// have, and a range no day is in.
    fn day_range(&mut self, r: &RangeDecl) -> Option<Range> {
        let lo = match &r.lo {
            Some(b) => Some(self.day(b, r.span)?),
            None => None,
        };
        let hi = match &r.hi {
            Some(b) => Some(self.day(b, r.span)?),
            None => None,
        };
        let rg = Range { lo, hi };
        if let (Some(l), Some(h)) = (lo, hi) {
            if l > h {
                let text = show_ends(rg.lo.map(day_text), rg.hi.map(day_text));
                self.push(e("E003", r.span, tr!("`{text}` に入る日はありません", "no day is in `{text}`")));
                return None;
            }
        }
        Some(rg)
    }

    /// One end of a range of days or of times: the day, counted from 1970-01-01.
    fn day(&mut self, b: &syntax::Bound, at: Span) -> Option<i64> {
        let (word, sp) = match b {
            syntax::Bound::Date { text, span } => match ritsu_ports::day_of(text) {
                Some(n) => return Some(n),
                None => {
                    self.push(e("E003", *span, tr!("`{text}` という日はありません", "there is no day `{text}`")));
                    return None;
                }
            },
            syntax::Bound::Int(n, _) => (n.to_string(), at),
            syntax::Bound::Quantity { raw, span, .. } => (raw.clone(), *span),
            syntax::Bound::Value { name, span } => (name.clone(), *span),
        };
        self.push(e("E003", sp, tr!("日付と時刻の範囲の端は、日付で書いてください（`>=2026-01-01`）。`{word}` は日付ではありません", "a bound of a range of dates or of times is a day (`>=2026-01-01`), and `{word}` is none")));
        None
    }

    /// `range >=2026-01-01 <=2027-12-31` on a time, or `now range …`: the moments from the start of
    /// the first day to the end of the last, in UTC or in the offset written after the ends, as
    /// seconds since 1970-01-01T00:00:00Z; and the range as the `.flow` writes it. E003 as for a
    /// range of days, and for an offset koyomi would not read (`±HH:MM`, at most 23:59).
    fn moment_range(&mut self, r: &RangeDecl) -> Option<(Range, String)> {
        let days = self.day_range(r)?;
        let offset = match &r.offset {
            Some((raw, sp)) => match offset_minutes(raw) {
                Some(o) => o,
                None => {
                    self.push(e("E003", *sp, tr!("`{raw}` というオフセットはありません。`+09:00` のように `±HH:MM` で書いてください（23:59 まで）", "there is no offset `{raw}`: write it `±HH:MM`, as `+09:00`, up to 23:59")));
                    return None;
                }
            },
            None => 0,
        };
        let shift = i64::from(offset) * 60;
        let rg = Range { lo: days.lo.map(|d| d * 86_400 - shift), hi: days.hi.map(|d| (d + 1) * 86_400 - shift - 1) };
        let mut text = show_ends(days.lo.map(day_text), days.hi.map(day_text));
        if let Some((raw, _)) = &r.offset {
            text = format!("{text} offset {raw}");
            if !self.m.moment_texts.iter().any(|(x, _)| *x == rg) {
                self.m.moment_texts.push((rg, text.clone()));
            }
        }
        Some((rg, text))
    }

    /// `now range …` under `workflow`: the period the workflow runs in (DESIGN 20).
    fn period(&mut self) {
        let Some(r) = self.prog.now_range.clone() else { return };
        if let Some((range, text)) = self.moment_range(&r) {
            self.m.period = Some(Period { range, text, line: r.span.line });
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
                if let Some(ssp) = f.secret {
                    let mark = self.written_mark(ssp);
                    self.m.input_secrets.insert(f.name.0.clone(), mark);
                }
                self.m.input_lines.insert(f.name.0.clone(), f.name.1.line);
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
                if let Some(ssp) = f.secret {
                    let mark = self.written_mark(ssp);
                    self.m.output_secrets.insert(f.name.0.clone(), mark);
                }
                self.m.output_lines.insert(f.name.0.clone(), f.name.1.line);
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
            let mut param_secrets: BTreeMap<String, Vec<(String, SecretMark)>> = BTreeMap::new();
            for p in &t.params {
                if params.iter().any(|(n, _): &(String, Ty)| *n == p.name.0) {
                    self.push(e("E006", p.name.1, tr!("引数 `{}` が二度書かれています", "the parameter `{}` is written twice", p.name.0)));
                    continue;
                }
                if let Some(ty) = self.ty(&p.ty) {
                    if let Some(rg) = self.range(&ty, p.range.as_ref()) {
                        param_ranges.insert(p.name.0.clone(), rg);
                    }
                    if let Some(ssp) = p.secret {
                        let mark = self.written_mark(ssp);
                        param_secrets.entry(p.name.0.clone()).or_default().push((String::new(), mark));
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
            let mut result_secrets: Vec<(String, SecretMark)> = t.result_secret.map(|ssp| (String::new(), self.written_mark(ssp))).into_iter().collect();
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
                                    self.push(e("E007", *bsp, tr!("本文の形式は OpenAPI の記述で決まるので、`form` は外してください", "the OpenAPI document says how the body is written; leave out `form`")));
                                }
                                match api.base_url() {
                                    Some(base) => spec_url = Some((format!("{base}{url}"), op.form)),
                                    None => self.push(e("E016", *bsp, tr!("`{}` にはサーバーが書かれていません。`use openapi {}` の下に `url \"<場所>\"` を書いてください", "`{}` names no server; write `url \"<where it is>\"` under `use openapi {}`", a.0, a.0))),
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
                                tr!("`.proto` にはサービスの場所が書かれていません。`use proto {name}` の下に `url \"<場所>\"` を書いてください", "a `.proto` does not say where the service is; write `url \"<where it is>\"` under `use proto {name}`"),
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
                Some((syntax::Binding::Jev(jd), bsp)) => Some(self.jev(t, jd, *bsp, result.as_ref(), result_range)),
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
                    self.push(e("E007", *esp, tr!("`{en}` は宣言しなくても使えます。タスク自身のエラーだけを宣言してください", "`{en}` is always there; declare only the task's own errors")));
                    continue;
                }
                if errors.iter().any(|x| x.name == *en) {
                    self.push(e("E006", *esp, tr!("エラー `{en}` が二度書かれています", "the error `{en}` is written twice")));
                    continue;
                }
                if let Some(Binding::Book(b)) = &binding {
                    if er.status.is_some() || er.exception.is_some() {
                        self.push(e("E007", *esp, tr!("帳簿の操作のエラーは、帳簿が拒否する理由の名前そのものです。`= …` は外してください", "an error of a book's operation is the name of the reason the book refuses it with; leave out `= …`")));
                        continue;
                    }
                    let reasons = self.book_reasons(b);
                    if !reasons.contains(en) {
                        let (tn, op) = (&b.transfer, &b.op);
                        self.push(e("E016", *esp, tr!("`{en}` は、帳簿が `{tn}.{op}` を拒否する理由ではありません（理由は {}）", "`{en}` is not a reason the book refuses `{tn}.{op}` with ({})", reasons.join("・"); reasons.join(", "))));
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
                            self.push(e("E007", *esp, tr!("Connect のエラーは、`{en} = not_found` のようにコードで書いてください", "a Connect error is named by its code, as `{en} = not_found`")));
                            continue;
                        }
                    }
                } else {
                    (er.status, er.exception.clone())
                };
                match (&binding, status, &exception) {
                    (Some(Binding::Http { .. }), None, _) => {
                        self.push(e("E007", *esp, tr!("`{en}` が返ってくるときの HTTP ステータスを、`{en} = 402` のように書いてください", "give `{en}` the HTTP status it comes back with, as `{en} = 402`")));
                    }
                    // TypeSafe's API says an error by its status: 429 for the rate limit, 529 when it is overloaded
                    (Some(Binding::Jev(j)), None, _) if j.api == DecisionApi::SystemOne && j.url.is_none() => {
                        self.push(e(
                            "E007",
                            *esp,
                            tr!("Jev の API はエラーを HTTP ステータスで伝えます。`{en}` が返ってくるときのステータスを、`{en} = 429`（レート制限）や `{en} = 529`（過負荷）のように書いてください", "Jev's API says an error by its HTTP status; give `{en}` the one it comes back with, as `{en} = 429` (the rate limit) or `{en} = 529` (overloaded)"),
                        ));
                    }
                    // so do the other servers of decisions: OpenAI's, and those `url` names
                    (Some(Binding::Jev(_)), None, _) => {
                        self.push(e(
                            "E007",
                            *esp,
                            tr!("判断の API はエラーを HTTP ステータスで伝えます。`{en}` が返ってくるときのステータスを、`{en} = 429`（レート制限）のように書いてください", "the API of decisions says an error by its HTTP status; give `{en}` the one it comes back with, as `{en} = 429` (the rate limit)"),
                        ));
                    }
                    (Some(Binding::Jev(_)), Some(_), _) => {}
                    (Some(Binding::Http { .. }), Some(_), None) => {}
                    (Some(Binding::Aws { .. }), None, _) => {}
                    (Some(Binding::Aws { .. }), Some(_), _) => {
                        self.push(e("E007", *esp, tr!("AWS の API のエラーは、ステータスではなく `{en} = ConditionalCheckFailedException` のように例外の名前で書いてください", "an AWS API's error is named by its exception, as `{en} = ConditionalCheckFailedException`, not by a status")));
                    }
                    (Some(Binding::Lambda(_)), None, None) => {}
                    (Some(Binding::Lambda(_)), _, _) => {
                        self.push(e("E007", *esp, tr!("Lambda のタスクのエラーは、関数が投げるエラーの型の名前で表します。ステータスは外してください", "a Lambda task's error is named by the error type the function raises, without a status")));
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
            // the error a refused question fails the call with: the Decisions API's alone refuses
            if let Some(((re, rsp), _)) = &t.refusal {
                match &jev {
                    Some(j) if j.api == DecisionApi::Decisions => {
                        if re == "timeout" || re == "failure" {
                            self.push(e("E007", *rsp, tr!("`{re}` はいつもあるエラーです。モデルが答えなかったときには、タスク自身のエラーの名前を付けてください", "`{re}` is always there; name an error of the task's own for a question the model refuses")));
                        } else if errors.iter().any(|x| x.name == *re) {
                            self.push(e("E006", *rsp, tr!("エラー `{re}` が二度書かれています。`refusal else {re}` がそれを宣言します", "the error `{re}` is written twice: `refusal else {re}` declares it")));
                        } else {
                            errors.push(ErrDef { name: re.clone(), status: None, exception: None });
                        }
                    }
                    Some(_) => self.push(e(
                        "E007",
                        *rsp,
                        tr!("System One の API（TypeSafe の Jev や、`url` のサーバー）は、どの質問にも答えを返し、答えないことはありません。`refusal` は `jev openai` のタスクに書くところです", "the System One API (TypeSafe's Jev, and the servers `url` names) answers every question and refuses none; `refusal` is for a `jev openai` task"),
                    )),
                    None => self.push(e("E007", *rsp, tr!("`refusal` は、OpenAI の Decisions API が質問に答えなかったときのエラーを書くところです。このタスクには `jev openai` がありません", "`refusal` says the error of a question OpenAI's Decisions API refuses; this task has no `jev openai`"))),
                }
            } else if jev.as_ref().is_some_and(|j| j.api == DecisionApi::Decisions) {
                // without it, a refusal fails the call with an error of dandori's that `on failure` takes,
                // and that no retry takes, as it does not take the task's own
                errors.push(ErrDef { name: REFUSED.to_string(), status: None, exception: None });
            }
            if let Some((syntax::Binding::Http { url, api, .. }, bsp)) = t.binding.as_ref().map(|(b, s)| (b, s)) {
                for ph in placeholders(url) {
                    // a description's `{item-id}` takes the parameter `item_id` (DESIGN 43)
                    if !params.iter().any(|(n, _)| *n == ph || (api.is_some() && crate::apis::same_name(crate::model::Place::Path, &ph, n))) {
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
                // what `timeout until` waits until is the one parameter an event takes
                if params.iter().any(|(p, _)| !t.deadline.as_ref().is_some_and(|(d, _)| d == p)) {
                    self.push(e("E007", *sp, tr!("`event` のタスクは何も送らないので、`timeout until` に書く時刻のほかに引数を取りません", "an `event` task sends nothing, so it takes no parameters but the time its `timeout until` names")));
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
            // `timeout until <param>`: the wait for the answer ends at the time the call gives the parameter
            if let Some((dp, dsp)) = &t.deadline {
                if t.event.is_none() && t.callback.is_none() {
                    self.push(e(
                        "E007",
                        *dsp,
                        tr!("`timeout until` を書けるのは、答えを待つタスク（`event` か `callback`）だけです。ほかのタスクの `timeout` は、`timeout 30 seconds` のように長さで書いてください", "`timeout until` belongs to a task that waits for an answer (`event` or `callback`); give any other task's `timeout` as a length, as `timeout 30 seconds`"),
                    ));
                }
                match params.iter().find(|(p, _)| p == dp) {
                    None => self.push(e("E002", *dsp, tr!("`timeout until` の `{dp}` は、タスク `{name}` の引数にありません", "`{dp}` of `timeout until` is not a parameter of `{name}`"))),
                    Some((_, ty)) if *ty != Ty::Timestamp => {
                        let shown = self.m.ty_name(ty);
                        self.push(e("E003", *dsp, tr!("`timeout until` が待つ `{dp}` は `timestamp` にしてください。いまは `{shown}` です", "`{dp}`, the time `timeout until` waits until, must be a `timestamp`, not `{shown}`")));
                    }
                    Some(_) => {}
                }
                if t.callback.is_some() && matches!(binding, Some(Binding::Aws { .. })) {
                    self.push(e(
                        "E007",
                        *dsp,
                        tr!("`aws` のタスクの引数は API の引数なので、`timeout until` の時刻を引数にできません。ID は `lambda` で渡してください", "the parameters of an `aws` task are the API's own, so none can be the time of `timeout until`; hand the id over by `lambda`"),
                    ));
                }
                // a retry after the time has passed only times out again
                if let Some(r) = t.retry.as_ref().filter(|r| t.callback.is_some() && (r.on.is_empty() || r.on.iter().any(|(x, _)| x == "timeout"))) {
                    self.push(e(
                        "E007",
                        r.span,
                        tr!("`timeout until` のタスクは、期限を過ぎてからリトライしても、すぐにまたタイムアウトします。`retry … on failure` のように、リトライするものを `timeout` 抜きで書いてください", "a task with `timeout until` only times out again when it is tried after its time has passed; name what `retry` retries without `timeout`, as `retry … on failure`"),
                    ));
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
                    (_, Some((_, kpsp))) => self.push(e("E007", *kpsp, tr!("`key <引数>` は AWS の API のための書き方です。ここでは `key` だけを書いてください", "`key <parameter>` is for an AWS API; write just `key`"))),
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
                    if jev.as_ref().and_then(|j| j.refusal.as_ref()).is_some_and(|re| re == on) {
                        self.diags.push(e(
                            "E007",
                            *osp,
                            tr!("答えなかった質問は、同じ入力で尋ね直しても答えないことが多いので、`{on}` はリトライできません（確信度の下限と同じです）", "a question the model refused is mostly refused again when asked again with the same input, so `{on}` is not retried, as a floor of how sure is not"),
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
                self.push(e("E007", *rsp, tr!("帳簿は、仮押さえの状態ごとに理由を付けて拒否します。`refused as` は外し、理由を `errors` に書いてください", "a book refuses with a reason for each state of the hold; leave out `refused as`, and declare the reasons in `errors`")));
            } else if t.refused_as.is_some() && !matches!(machine, Some(TaskMachine::Sends { .. })) {
                self.push(e("E007", t.refused_as.as_ref().unwrap().1, tr!("`refused as` を書けるのは、イベントを送る（`sends`）タスクだけです", "`refused as` belongs to a task that `sends` an event")));
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
                        e("E008", msp, tr!("`{tn}.do` はすぐに確定する振替で、案件を始めることも終えることもしません", "`{tn}.do` is a transfer done at once, which neither starts nor ends a case"))
                    } else {
                        e("E008", msp, tr!("`{tn}.{op}` が仮押さえに対してするのは `{want}` です", "what `{tn}.{op}` does to the hold is `{want}`"))
                    };
                    self.push(d);
                }
            }
            let discloses = self.discloses(t, &params);
            // the description of the API whose operation the task calls, as dandori reaches it
            let api_file = match &t.binding {
                Some((syntax::Binding::Http { api: Some(a), .. }, _)) | Some((syntax::Binding::Connect { api: a, .. }, _)) => {
                    self.prog.apis.iter().find(|u| u.name.0 == a.0).map(|u| crate::sources::reach(&self.dir, &u.path).to_string_lossy().to_string())
                }
                _ => None,
            };
            self.plaintext_task(t);
            // where the arguments of an `http` task go, as OpenAPI's defaults say; a task of an
            // operation of a description takes the description's below (DESIGN 43)
            let wire = match &binding {
                Some(Binding::Http { method, url, .. }) => crate::wire::defaults(method, url, &params),
                _ => vec![],
            };
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
                deadline: t.deadline.as_ref().map(|x| x.0.clone()),
                key: t.key.is_some(),
                key_param: t.key_param.as_ref().map(|x| x.0.clone()),
                idempotent: t.idempotent,
                machine,
                refused_as: t.refused_as.as_ref().map(|x| x.0.clone()),
                callback: t.callback.is_some(),
                event: t.event.is_some(),
                flow,
                connect,
                request_days: None,
                answer_zeros: None,
                timestamps: None,
                param_secrets,
                result_secrets: std::mem::take(&mut result_secrets),
                discloses,
                api_file,
                wire,
                line: sp.line,
            });
            let task = self.m.tasks.last().expect("just pushed");
            let held = task.flow.as_ref().map(|c| crate::contract::check(&self.m, task, &c.model)).unwrap_or_default();
            self.diags.extend(held);
            // an `http` task no description holds: each argument written where it goes, as OpenAPI's
            // defaults write it (DESIGN 43)
            if let Some((syntax::Binding::Http { api: None, form, .. }, bsp)) = &t.binding {
                let task = self.m.tasks.last().expect("just pushed");
                let mut found = Vec::new();
                for w in &task.wire {
                    if let Some((_, pt)) = task.params.iter().find(|(p, _)| *p == w.param) {
                        if let Some(Text { ja, en }) = crate::wire::unwritable(&self.m, w, pt, *form) {
                            found.push(tr!("引数 `{}` を送れません。{ja}", "the parameter `{}` cannot be sent: {en}", w.param));
                        }
                    }
                }
                if let Some(c) = crate::wire::clash(&self.m, task, *form) {
                    found.push(crate::wire::clash_words(&c));
                }
                for w in found {
                    self.push(e("E003", *bsp, w));
                }
            }
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
                        // where each argument goes, as the description says (DESIGN 43)
                        if let syntax::Binding::Http { url, .. } = b {
                            let task = self.m.tasks.last().expect("just pushed");
                            let (wire, mut found) = op.placement(&self.m, task, &placeholders(url));
                            let task = self.m.tasks.last_mut().expect("just pushed");
                            task.wire = wire;
                            let task = self.m.tasks.last().expect("just pushed");
                            if let Some(c) = crate::wire::clash(&self.m, task, op.form) {
                                found.push(crate::wire::clash_words(&c));
                            }
                            for w in found {
                                self.push(e("E016", Span { line: sp.line, col: 1 }, w));
                            }
                        }
                        // the properties of the operation that mark what the task gives it and reads from it secret
                        if let syntax::Binding::Http { method, url, .. } = b {
                            let task = self.m.tasks.last().expect("just pushed");
                            let (params, answer) = crate::apis::openapi_marks(api, method, url, &self.m, task);
                            let file = self.prog.apis.iter().find(|u| u.name.0 == api.name).map(|u| crate::sources::reach(&self.dir, &u.path)).unwrap_or_default();
                            let params: Vec<(String, String, SecretMark)> = params.into_iter().map(|(p, d)| (p, d.path.clone(), self.doc_mark(&file, &d))).collect();
                            let answer: Vec<(String, SecretMark)> = answer.into_iter().map(|d| (d.path.clone(), self.doc_mark(&file, &d))).collect();
                            let task = self.m.tasks.last_mut().expect("just pushed");
                            for (p, path, mark) in params {
                                task.param_secrets.entry(p).or_default().push((path, mark));
                            }
                            task.result_secrets.extend(answer);
                        }
                        // and the fields of the `.proto`'s request and response, for a `connect` task
                        if let syntax::Binding::Connect { method, .. } = b {
                            let task = self.m.tasks.last().expect("just pushed");
                            let (params, answer) = crate::apis::proto_marks(api, method, &self.m, task);
                            let params: Vec<(String, String, SecretMark)> = params.into_iter().map(|(p, path, r)| (p, path, self.proto_mark(&r))).collect();
                            let answer: Vec<(String, SecretMark)> = answer.into_iter().map(|(path, r)| (path, self.proto_mark(&r))).collect();
                            let task = self.m.tasks.last_mut().expect("just pushed");
                            for (p, path, mark) in params {
                                task.param_secrets.entry(p).or_default().push((path, mark));
                            }
                            task.result_secrets.extend(answer);
                        }
                        // where a `connect` task's request holds a `google.type.Date`, which its `date` goes to as `{year, month, day}`
                        if let syntax::Binding::Connect { .. } = b {
                            let task = self.m.tasks.last_mut().expect("just pushed");
                            task.request_days = op.request_days();
                        }
                        // the timestamps of what an `aws` task sends, with the forms the model sends them in
                        if let syntax::Binding::Aws { .. } = b {
                            let task = self.m.tasks.last_mut().expect("just pushed");
                            let params: Vec<String> = task.params.iter().map(|(p, _)| p.clone()).collect();
                            task.timestamps = op.timestamps(&params);
                        }
                        let task = self.m.tasks.last().expect("just pushed");
                        let found = op.check(&self.m, task);
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

    /// The mark `secret` written on the line of `sp` of this `.flow` (DESIGN 1.18).
    fn written_mark(&self, sp: Span) -> SecretMark {
        SecretMark { file: self.m.source_reached.clone(), line: sp.line, mark: "secret".into(), at: format!("{}:{}", self.m.source_file, sp.line), detail: None }
    }

    /// `discloses <param>, … "<why>"` under a task: each parameter it names, with why (E007 for a
    /// name the task has no parameter of, and for an empty why).
    fn discloses(&mut self, t: &syntax::TaskDecl, params: &[(String, Ty)]) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (names, why, sp) in &t.discloses {
            if why.trim().is_empty() {
                self.push(e("E007", *sp, tr!("`discloses` には、秘密の値をそこへ送る理由を書いてください（空の文字列です）", "give `discloses` the reason the task sends the secrets there; it is an empty string")));
            }
            for (p, psp) in names {
                if !params.iter().any(|(n, _)| n == p) {
                    let task = &t.name.0;
                    self.push(e("E007", *psp, tr!("`discloses` が書く `{p}` は、`{task}` の引数ではありません", "`discloses` names `{p}`, which is not a parameter of `{task}`")));
                    continue;
                }
                out.push((p.clone(), why.clone()));
            }
        }
        out
    }

    /// `plaintext` under a task (E007 when the task has no URL of its own, and for an empty why),
    /// and W902 for a URL the task calls without encryption, to a host that is not this machine:
    /// an `http` task's own URL, an agent's `url`, a decision task's `url` (DESIGN 1.18). The URL of
    /// an API's operation is the `use`'s, and said there (`plaintexts`).
    fn plaintext_task(&mut self, t: &syntax::TaskDecl) {
        let name = &t.name.0;
        // what calls: "task", "agent" or "decision"
        let own = match (&t.binding, &t.url) {
            (Some((syntax::Binding::Http { url, api: None, .. }, bsp)), _) => Some((url.clone(), *bsp, "task")),
            (Some((syntax::Binding::Agent { .. }, _)), Some((url, usp))) => Some((url.clone(), *usp, "agent")),
            (Some((syntax::Binding::Jev(_), _)), Some((url, usp))) => Some((url.clone(), *usp, "decision")),
            _ => None,
        };
        if let Some((why, psp)) = &t.plaintext {
            if own.is_none() {
                let mut d = e("E007", *psp, tr!("`plaintext` は、URL に暗号化せずに送ることを意図していると書くところですが、`{name}` は自分の URL を持ちません", "`plaintext` says that a URL is reached without encryption on purpose, and `{name}` has no URL of its own"));
                if let Some((syntax::Binding::Http { api: Some(a), .. }, _)) | Some((syntax::Binding::Connect { api: a, .. }, _)) = &t.binding {
                    let a = &a.0;
                    d = d.note(tr!("`{name}` が送る先は、`{a}` を読む `use` の URL です。`plaintext` はその `use` の下に書いてください。", "`{name}` sends to the URL of the `use` that reads `{a}`; write `plaintext` under that `use`."));
                }
                self.push(d);
            } else if why.trim().is_empty() {
                self.push(e("E007", *psp, tr!("`plaintext` には、暗号化しない理由を書いてください（空の文字列です）", "give `plaintext` the reason the connection is not encrypted; it is an empty string")));
            }
        }
        let Some((url, at, what)) = own else { return };
        if t.plaintext.is_some() {
            return;
        }
        if let Some((_, host)) = crate::secrets::plain_url(&url) {
            let said = match what {
                "agent" => tr!("エージェント `{name}` は、{host} に暗号化しない HTTP でリクエストを送ります", "the agent `{name}` sends its requests to {host} over plain HTTP"),
                "decision" => tr!("判断のタスク `{name}` は、{host} に暗号化しない HTTP でリクエストを送ります", "the decision task `{name}` sends its requests to {host} over plain HTTP"),
                _ => tr!("タスク `{name}` は、{host} に暗号化しない HTTP でリクエストを送ります", "the task `{name}` sends its requests to {host} over plain HTTP"),
            };
            self.push(plain_call(at, said, tr!("タスクの下", "under the task")));
        }
    }

    /// `plaintext` under `use rule … connect` and `use openapi|proto` (E007 where there is no URL,
    /// and for an empty why), and W902 for the service of a rule and the API that the tasks call
    /// without encryption, to a host that is not this machine (DESIGN 1.18): the URL under `use`,
    /// else an OpenAPI document's first server. Said at the `use`, once for every task that calls it.
    fn plaintexts(&mut self) {
        let no_why = || tr!("`plaintext` には、暗号化しない理由を書いてください（空の文字列です）", "give `plaintext` the reason the connection is not encrypted; it is an empty string");
        for u in &self.prog.uses {
            let rule = &u.name.0;
            match (&u.plaintext, &u.connect) {
                (Some((_, psp)), None) => self.diags.push(e("E007", *psp, tr!("`plaintext` は、規則のサービスに暗号化せずに送ることを意図していると書くところですが、規則 `{rule}` には `connect` がありません", "`plaintext` says that a rule's service is reached without encryption on purpose, and the rule `{rule}` has no `connect`"))),
                (Some((why, psp)), Some(_)) if why.trim().is_empty() => self.diags.push(e("E007", *psp, no_why())),
                (None, Some((url, csp))) => {
                    if let Some((_, host)) = crate::secrets::plain_url(url) {
                        self.diags.push(plain_call(*csp, tr!("規則 `{rule}` のサービスは、{host} に暗号化しない HTTP で呼ばれます", "the service of the rule `{rule}` is called at {host} over plain HTTP"), tr!("`use rule` の下", "under `use rule`")));
                    }
                }
                _ => {}
            }
        }
        for u in &self.prog.apis {
            let api = &u.name.0;
            let word = u.kind.word();
            let base = self.apis.get(api).and_then(|(_, a)| if u.kind == ApiKind::Smithy { None } else { a.base_url() });
            match (&u.plaintext, &base) {
                (Some((_, psp)), None) => {
                    self.diags.push(e("E007", *psp, tr!("`plaintext` は、API に暗号化せずに送ることを意図していると書くところですが、`{api}` には送る先の URL がありません", "`plaintext` says that an API is reached without encryption on purpose, and `{api}` has no URL to reach")));
                    continue;
                }
                (Some((why, psp)), Some(_)) => {
                    if why.trim().is_empty() {
                        self.diags.push(e("E007", *psp, no_why()));
                    }
                    continue;
                }
                (None, None) => continue,
                (None, Some(_)) => {}
            }
            let called = self.prog.tasks.iter().any(|t| matches!(&t.binding, Some((syntax::Binding::Http { api: Some(a), .. }, _)) | Some((syntax::Binding::Connect { api: a, .. }, _)) if a.0 == *api));
            let (Some(base), true) = (base, called) else { continue };
            if let Some((_, host)) = crate::secrets::plain_url(&base) {
                let at = u.url_at.unwrap_or(u.name.1);
                let said = if u.url.is_some() {
                    tr!("`{api}` を呼ぶタスクは、{host} に暗号化しない HTTP でリクエストを送ります", "the tasks that call `{api}` send their requests to {host} over plain HTTP")
                } else {
                    let file = &u.path;
                    tr!("`{api}` を呼ぶタスクは、{host}（`{file}` の最初のサーバー）に暗号化しない HTTP でリクエストを送ります", "the tasks that call `{api}` send their requests to {host}, the first server of `{file}`, over plain HTTP")
                };
                self.diags.push(plain_call(at, said, tr!("`use {word} {api}` の下", "under `use {word} {api}`")));
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
            self.push(e("E002", book.1, tr!("帳簿 `{}` はありません。`use book` で読んだものを書いてください", "there is no book `{}`; name one read by `use book`", book.0)));
            return None;
        };
        let bu = self.m.books[bix].clone();
        let Some(tr) = bu.transfer(&transfer.0) else {
            let names: Vec<&str> = bu.facts.transfers.iter().map(|x| x.name.as_str()).collect();
            self.push(e("E002", transfer.1, tr!("帳簿 `{}` に振替 `{}` はありません（あるのは {}）", "the book `{}` has no transfer `{}` ({})", book.0, transfer.0, names.join("・"); book.0, transfer.0, names.join(", "))));
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
                Some((_, got)) if !self.m.same_ty(got, &want) => {
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
                self.push(e("E002", at, tr!("API `{api}` はありません。`use proto` で読んだものを書いてください", "there is no API `{api}`; name one read by `use proto`")));
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
                tr!("`{api}` にサービス `{rel}` はありません（`{api}` にはサービスが一つもありません）", "`{api}` has no service `{rel}` (it has no services)")
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
        // a server of the same API as TypeSafe's (System One) or OpenAI's (Decisions)
        if let (Some((u, usp)), true) = (&t.url, jev) {
            if !(u.starts_with("https://") || u.starts_with("http://")) {
                self.push(e("E007", *usp, tr!("`{u}` は http:// や https:// の URL ではありません", "`{u}` is not an http:// or https:// URL")));
                return;
            }
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
                tr!("`url` は、エージェントの Open Responses の API や、判断のモデル（`jev`）の API がどのサーバーにあるかを書くところです。このタスクには `agent` も `jev` もありません（HTTP のタスクの URL は `http` のあとに書いてください）", "`url` says which server an agent's Open Responses API, or a decision model's (`jev`) API, is on; this task has neither `agent` nor `jev` (an HTTP task writes its URL after `http`)"),
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
    fn jev(&mut self, t: &syntax::TaskDecl, jd: &syntax::JevDecl, bsp: Span, result: Option<&Ty>, result_range: Option<Range>) -> Jev {
        let model = t.model.as_ref().map(|x| x.0.clone()).unwrap_or_default();
        let floor = t.confidence.as_ref().map(|(v, e, _)| (*v, e.0.clone()));
        // whose API: TypeSafe's System One (which other servers speak too), or OpenAI's Decisions
        let api = match &jd.provider {
            None => DecisionApi::SystemOne,
            Some((p, _)) if p == "typesafe" => DecisionApi::SystemOne,
            Some((p, _)) if p == "openai" => DecisionApi::Decisions,
            Some((p, psp)) => {
                self.push(e(
                    "E007",
                    *psp,
                    tr!("`{p}` は dandori の知らない判断の API です。`jev \"…\"`（TypeSafe の Jev が答える System One の API。`jev typesafe \"…\"` と書いても同じ）か `jev openai \"…\"`（OpenAI の Decisions API）と書いてください", "`{p}` is not an API of decisions dandori knows; write `jev \"…\"` (the System One API that TypeSafe's Jev answers on, also written `jev typesafe \"…\"`) or `jev openai \"…\"` (OpenAI's Decisions API)"),
                ));
                DecisionApi::SystemOne
            }
        };
        let url = t.url.as_ref().map(|x| x.0.clone());
        let refusal = t.refusal.as_ref().map(|((e, _), _)| e.clone());
        let mut out = Jev { api, url: url.clone(), model, questions: vec![], confidences: vec![], floor, refusal };
        if t.model.is_none() {
            let Text { en, ja } = match (api, &url) {
                (DecisionApi::SystemOne, None) => tr!("Jev には、尋ねるモデルを `model \"jev-1.13.0\"` のようにバージョンで書いてください", "Jev needs the model it asks; write the version, as `model \"jev-1.13.0\"`"),
                (DecisionApi::SystemOne, Some(_)) => tr!("System One の API のサーバーには、尋ねるモデルを `model \"tev1:0.8b\"` のように、名前とタグで書いてください", "the server of the System One API needs the model it asks; write its name and tag, as `model \"tev1:0.8b\"`"),
                (DecisionApi::Decisions, _) => tr!("Decisions API には、尋ねるモデルを `model \"gpt-6-luna\"` のように書いてください", "the Decisions API needs the model it asks; write it, as `model \"gpt-6-luna\"`"),
            };
            self.push(e("E007", bsp, Text { en, ja }));
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
                if let Some(q) = self.jev_question(ask, r, result_range, None, api, url.is_none() && api == DecisionApi::SystemOne) {
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
                                if let Some(q) = self.jev_question(ask, fty, rec.ranges.get(fname).copied(), Some(fname), api, url.is_none() && api == DecisionApi::SystemOne) {
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
            let moves = match (api, &url) {
                (DecisionApi::SystemOne, None) if m == "jev-latest" || m == "jev-preview" => Some(tr!(
                    "`{m}` はエイリアスで、ここを変えなくても Jev の新しいバージョンに移ります。確信度の意味はバージョンごとに違うので、確信度を合わせたバージョンを `model \"jev-1.13.0\"` のように書いてください",
                    "`{m}` is an alias, which moves to a new version of Jev without a change here, and how sure one version is means something else to the next; name the version the confidence is set for, as `model \"jev-1.13.0\"`"
                )),
                // Ollama's name without a tag is `:latest`, which a pull of the model moves to its newest
                (DecisionApi::SystemOne, Some(_)) if !m.contains(':') => Some(tr!(
                    "`{m}` にはタグが無いので、サーバーがモデルを取り直すと、ここを変えなくても新しいモデルに移ります（Ollama の名前の付け方では `:latest` と同じです）。確信度の意味はモデルごとに違うので、確信度を合わせたモデルを `model \"tev1:0.8b\"` のようにタグまで書いてください",
                    "`{m}` has no tag, so it moves to a newer model when the server pulls the model again, with no change here (as Ollama names models, it is `:latest`); how sure one model is means something else to the next, so name the model the confidence is set for with its tag, as `model \"tev1:0.8b\"`"
                )),
                (DecisionApi::SystemOne, Some(_)) if m.ends_with(":latest") => Some(tr!(
                    "`{m}` のタグ `latest` は、サーバーがモデルを取り直すと、ここを変えなくても新しいモデルに移ります（Ollama の名前の付け方）。確信度の意味はモデルごとに違うので、確信度を合わせたモデルを `model \"tev1:0.8b\"` のようにタグまで書いてください",
                    "the tag `latest` of `{m}` moves to a newer model when the server pulls the model again, with no change here (how Ollama names models); how sure one model is means something else to the next, so name the model the confidence is set for with its tag, as `model \"tev1:0.8b\"`"
                )),
                // OpenAI names no version of the model the Decisions API runs
                (DecisionApi::Decisions, _) => Some(tr!(
                    "OpenAI の Decisions API には、モデルのバージョンを固定する名前がありません（`{m}`）。モデルが替わると、ここを変えなくても確信度の意味が変わることがあるので、確信度の下限と確信度を受け取るフィールドを、ときどき実際の答えで確かめ直してください",
                    "OpenAI's Decisions API names no version of its model (`{m}`), so how sure an answer is may come to mean something else when the model changes, with no change here; check the floor of how sure, and the fields that take it, against real answers from time to time"
                )),
                _ => None,
            };
            if let Some(text) = moves {
                self.diags.push(Diag::warning("W032", msp.line, msp.col, text));
            }
        }
        out
    }

    /// One question of a Jev task, whose answer is of type `ty` (an enum or `bool`) in a place with
    /// the range `rg`: an enum held to a range is asked among the values the range lets in, as the
    /// answer is checked against them when it comes.
    fn jev_question(&mut self, ask: &syntax::JevAsk, ty: &Ty, rg: Option<Range>, field: Option<&str>, api: DecisionApi, typesafe: bool) -> Option<Question> {
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
                let mut def = self.m.enums[*en].clone();
                for ((v, vsp), _) in &ask.criteria {
                    if !def.values.contains(v) {
                        self.push(e("E002", *vsp, tr!("`{v}` は `{}` の値ではありません（値は {}）", "`{v}` is not a value of `{}`, whose values are {}", def.name, def.values.join("・"); def.name, def.values.join(", "))));
                        return None;
                    }
                }
                // an enum held to a range: Jev is asked among the values it lets in, which are what
                // the checks below count, and a meaning written for another is never asked for
                if rg.is_some() {
                    let takes = self.m.values_in(*en, rg);
                    let w = ends_text(&takes);
                    if let Some(((v, vsp), _)) = ask.criteria.iter().find(|((v, _), _)| !takes.contains(v)) {
                        self.push(e("E007", *vsp, tr!("`{v}` は範囲 `{w}` の外の値なので、Jev には尋ねません。この意味は外してください", "`{v}` is outside the range `{w}`, so Jev is not asked about it; leave its meaning out")));
                        return None;
                    }
                    def = EnumDef { name: format!("{} range {w}", def.name), values: takes, ordered: def.ordered, from: None, declared: None };
                }
                if let Some(ssp) = ask.score {
                    // the levels are the values, each with what it means, from the lowest; TypeSafe's
                    // API takes 2 to 10, and the others say no more than 2 at least (Ollama 26 at most)
                    if typesafe && !(2..=10).contains(&def.values.len()) {
                        self.push(e(
                            "E007",
                            ssp,
                            tr!("score の段階は 2 から 10 までで、`{}` の値は {} 個です", "a score has from 2 to 10 levels, and `{}` has {} values", def.name, def.values.len()),
                        ));
                        return None;
                    }
                    if def.values.len() < 2 {
                        self.push(e("E007", ssp, tr!("score の段階は二つ以上で、`{}` の値は一つだけです", "a score has two levels at least, and `{}` has one value only", def.name)));
                        return None;
                    }
                    let missing: Vec<&String> = def.values.iter().filter(|v| meaning(v).is_none()).collect();
                    if !missing.is_empty() {
                        let names = missing.iter().map(|v| v.as_str()).collect::<Vec<_>>();
                        self.push(e(
                            "E007",
                            ask.span,
                            tr!("Jev は score の段階を、書いた意味だけで見分けます。`{}` のすべての値の意味を、低い段階から高い段階の順に下に書いてください。{} の意味がありません", "Jev sees a score's levels only by what each means, so write under it what every value of `{}` means, from the lowest level to the highest; {} is missing", def.name, names.join("・"); def.name, names.join(", ")),
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
                    if def.values.len() < 2 {
                        self.push(e("E007", ask.span, tr!("choice の選択肢は二つ以上で、`{}` の値は一つだけです", "a choice has two options at least, and `{}` has one value only", def.name)));
                        return None;
                    }
                    let options = def.values.iter().map(|v| (v.clone(), meaning(v))).collect();
                    Some(Question { id, field: field.map(String::from), kind: QuestionKind::Choice, instructions: ask.instructions.clone(), options })
                }
            }
            Ty::Bool => {
                if let Some(ssp) = ask.score {
                    self.push(e("E007", ssp, tr!("score で尋ねられるのは、列挙の値を低いものから並べた段階のどこかです。`bool` は、`score` を付けずに、はいかいいえで尋ねてください", "a score is a place on a scale of an enum's values; a `bool` is asked as yes or no")));
                    return None;
                }
                for ((v, vsp), _) in &ask.criteria {
                    if v != "true" && v != "false" {
                        self.push(e("E002", *vsp, tr!("答えははいかいいえなので、`{v}` ではなく `true` と `false` の意味を書いてください", "the answer is yes or no, so write what `true` and `false` mean, not `{v}`")));
                        return None;
                    }
                }
                let options: Vec<(String, Option<String>)> = match (meaning("true"), meaning("false")) {
                    (Some(_), Some(_)) if api == DecisionApi::Decisions => {
                        self.push(e(
                            "E007",
                            ask.span,
                            tr!("Decisions API の predicate は、はいといいえの意味を受け取りません。`true` と `false` の意味を外し、尋ねることの中に書いてください", "a predicate of the Decisions API takes no meanings of yes and no; leave out what `true` and `false` mean, and say it in the question"),
                        ));
                        return None;
                    }
                    (Some(y), Some(n)) => vec![("true".into(), Some(y)), ("false".into(), Some(n))],
                    (None, None) => vec![],
                    _ => {
                        self.push(e("E007", ask.span, tr!("`true` と `false` の意味は、両方書くか、どちらも書かないかにしてください", "write what both `true` and `false` mean, or neither")));
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
            self.push(e("E002", sp, tr!("ステートマシンは <規則>.<ステートマシン> と書いてください", "name a machine as <rule>.<machine>")));
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
                        tr!("案件の型には、このファイルで宣言したレコードか、`.proto` から作ったレコードを使ってください", "a case is held in a record declared in this file or made from a `.proto`"),
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
            let rule_name = self.m.rules[rule].name.clone();
            for ((inp, isp), val) in &c.held {
                if !mc.held.contains(inp) {
                    self.push(e(
                        "E008",
                        *isp,
                        tr!("`{inp}` は、ステートマシンの `held` の入力ではありません（`held` の入力は {}）", "`{inp}` is not held by the machine; its held inputs are {}", if mc.held.is_empty() { "ありません".to_string() } else { mc.held.join("・") }; if mc.held.is_empty() { "none".to_string() } else { mc.held.join(", ") }),
                    ));
                    continue;
                }
                // the input's type, as the rule declares it: a number held to it counts in its unit,
                // as a value of a call does (DESIGN 40)
                let ty = self.m.rules[rule].info.inputs.iter().find(|col| col.name == *inp).map(|col| self.rty(&rule_name, &col.ty));
                let number = match val {
                    syntax::Held::Word(..) => None,
                    syntax::Held::Int(n, sp) => {
                        // a whole number where a rate counts steps other than 1% (W106)
                        if let Some(rate) = ty.as_ref().and_then(|t| match t.inner() {
                            Ty::Num(u) => in_steps(u, *n),
                            _ => None,
                        }) {
                            let (step, value) = (&rate.step, &rate.value);
                            let d = Diag::warning("W106", sp.line, sp.col, tr!(
                                "`{inp}` に書いた `{n}` は、{step} の刻みの数として読みます。{value} です",
                                "`{n}`, written for `{inp}`, counts steps of {step}: it is {value}"
                            ))
                            .note(tr!(
                                "読む人に率が分かるよう、`held {inp} = {value}` と書いてください。案件が持つ値は変わりません",
                                "Write it `held {inp} = {value}`, so that a reader sees the rate; the case holds the same value"
                            ));
                            self.push(d);
                        }
                        Some(*n)
                    }
                    syntax::Held::Quantity { value, unit, raw, span } => match self.quantity(*value, unit, raw, *span, ty.as_ref()) {
                        Some(n) => Some(n),
                        None => continue,
                    },
                };
                let text = val.text();
                if let Some(a) = mc.axis_of(inp) {
                    let axis = &mc.axes[a];
                    // a number is the value of the table that is the same number of the input's unit
                    // (`0.5%` and `5` of `rate[step 0.1%]` are the table's `0.5%`), or one of the set a
                    // coordinate stands for, between its ends (`0.3%` is the table's `0.1%` where that
                    // stands for 0.1% to 0.4%); a word is the value written alike
                    let at = match (number, ty.as_ref().map(Ty::inner)) {
                        (Some(n), Some(t @ (Ty::Num(_) | Ty::Int))) => axis.coords.iter().position(|x| coord_number(x, t) == Some(n)).or_else(|| axis.ends.iter().position(|ends| ends.as_ref().is_some_and(|ends| held_between(ends, n, t)))),
                        _ => axis.coords.iter().position(|x| *x == text),
                    };
                    match at {
                        Some(ci) => held.push((a, ci)),
                        None => {
                            self.push(e("E003", val.span(), not_held(&text, inp, axis)));
                            continue;
                        }
                    }
                }
                held_values.push((inp.clone(), text));
            }
            let hold = matches!(self.m.rules[rule].kind, RuleKind::Hold { .. });
            let mut external = Vec::new();
            if hold {
                // a hold expires on its own when the book says it does: the workflow need not say so
                for (ev, esp) in &c.external {
                    if ev != "expire" || mc.axes_with_value(ev).is_empty() {
                        self.push(e("E008", *esp, tr!("仮押さえにワークフローの外で起きることは、有効期限のある振替の `expire` だけです", "what happens to a hold on its own is `expire`, of a transfer whose holds expire")));
                    }
                }
                if let Some(a) = mc.axes_with_value("expire").first() {
                    external.push((*a, mc.axes[*a].coords.iter().position(|x| x == "expire").unwrap(), "expire".to_string()));
                }
                if let Some(((_, osp), _)) = &c.refused_when {
                    self.push(e("E008", *osp, tr!("仮押さえを拒否するのは帳簿で、理由は状態ごとに決まっています。`refused when` は外してください", "a hold is refused by the book, with a reason for each state; leave out `refused when`")));
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
                        self.push(e("E008", *osp, tr!("ステートマシンの表は `{o}` を出力しません（出力するのは {}）", "the machine's table does not write `{o}`; it writes {}", mc.decides.join("・"); mc.decides.join(", "))));
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
        for (b, _) in [&self.prog.flow, &self.prog.on_failure, &self.prog.on_cancel].into_iter().flatten() {
            set_lines(b, &mut self.set_lines);
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
        self.var_at.entry(n.to_string()).or_insert(sp);
        match self.var_ty.get(n).cloned() {
            // a value that fits the name's type is set to it; a name set to both `T` and `T?` is `T?`
            Some(prev) if self.m.fits(&t, &prev) => {}
            Some(prev) if self.m.fits(&prev, &t) && matches!(t, Ty::Opt(_)) => {
                self.var_ty.insert(n.to_string(), t);
                self.widened = true;
            }
            Some(prev) if !self.m.same_ty(&prev, &t) => {
                let (a, b) = (self.m.ty_name(&prev), self.m.ty_name(&t));
                let d = e("E003", sp, tr!("`{n}` は、ほかの場所では `{a}`、ここでは `{b}` です。一つの名前に型は一つだけです", "`{n}` was `{a}` elsewhere and is `{b}` here; a name keeps one type"));
                problems.insert((sp.line, sp.col), self.enums_apart(&prev, &t).into_iter().fold(d, Diag::note));
            }
            Some(_) => {}
            None => {
                self.var_ty.insert(n.to_string(), t);
            }
        }
    }

    /// Run `f` for its answer only: what it would say is dropped.
    fn quiet<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let (n, k) = (self.diags.len(), self.followers.len());
        let out = f(self);
        self.diags.truncate(n);
        self.followers.truncate(k);
        out
    }

    /// Hold back what a line would say that follows from an error at one of `lines` (DESIGN 30).
    /// What a declaration with an error leaves out of the model (DESIGN 30), each with the lines
    /// of its declaration: an input (`input:<name>`), an output, a task, a parameter
    /// (`param:<task>.<name>`), an error of a task (`error:<task>!<name>`) and a case.
    fn left_out_of_declarations(&mut self) {
        let mut out: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
        for (kind, fields, have) in [("input", &self.prog.inputs, &self.m.inputs), ("output", &self.prog.outputs, &self.m.outputs)] {
            for f in fields.iter().filter(|f| !have.iter().any(|(n, _)| *n == f.name.0)) {
                out.insert(format!("{kind}:{}", f.name.0), BTreeSet::from([f.name.1.line]));
            }
        }
        for t in &self.prog.tasks {
            let Some(&ti) = self.task_ix.get(&t.name.0) else {
                if !self.rule_ix.contains_key(&t.name.0) {
                    out.insert(format!("task:{}", t.name.0), task_lines(t));
                }
                continue;
            };
            let task = &self.m.tasks[ti];
            for p in t.params.iter().filter(|p| !task.params.iter().any(|(n, _)| *n == p.name.0)) {
                out.insert(format!("param:{}.{}", t.name.0, p.name.0), BTreeSet::from([p.name.1.line]));
            }
            for er in t.errors.iter().filter(|er| !matches!(er.name.0.as_str(), "timeout" | "failure") && !task.errors.iter().any(|x| x.name == er.name.0)) {
                out.insert(format!("error:{}!{}", t.name.0, er.name.0), BTreeSet::from([er.name.1.line]));
            }
        }
        for c in self.prog.cases.iter().filter(|c| !self.m.cases.iter().any(|x| x.name == c.name.0)) {
            let mut lines = BTreeSet::from([c.name.1.line, c.record.span().line]);
            lines.extend(c.machine.iter().chain(&c.external).chain(c.state_field.iter()).map(|n| n.1.line));
            lines.extend(c.held.iter().flat_map(|(a, b)| [a.1.line, b.span().line]));
            out.insert(format!("case:{}", c.name.0), lines);
        }
        self.left_out = out;
    }

    /// Whether a name is a word for a value rather than a variable: no variable, input or case has
    /// it, whether its type is known or not.
    fn bare_word(&self, n: &str) -> bool {
        n != "none" && !self.var_ty.contains_key(n) && !self.set_lines.contains_key(n) && !["input", "case"].iter().any(|k| self.left_out.contains_key(&format!("{k}:{n}")))
    }

    /// The enum that bare words are values of where nothing says which (DESIGN 29): the one enum of
    /// those the flow sees, two names of one enum counted once (DESIGN 28), that has every word read
    /// together (one word, or the words of a list). It goes by a name the flow declares or imports,
    /// else by the first of its names. Else what E002 says, and where: a word no enum has, at that
    /// word; words no one enum has all of; words two enums or more have, with how to say which.
    fn enum_of(&self, words: &[&str], list: bool) -> Result<EnumId, NoEnum> {
        let has = |e: EnumId, w: &str| self.m.enums[e].values.iter().any(|v| v == w);
        let types: Vec<EnumId> = (0..self.m.enums.len()).filter(|e| self.m.enum_class(*e) == *e).collect();
        let named = |c: EnumId| (0..self.m.enums.len()).filter(|x| self.m.enum_class(*x) == c).find(|x| !self.m.enums[*x].name.contains('.')).unwrap_or(c);
        let found: Vec<EnumId> = types.iter().copied().filter(|e| words.iter().all(|w| has(*e, w))).collect();
        let code = |xs: &[&str]| xs.iter().map(|w| format!("`{w}`")).collect::<Vec<_>>();
        match found.as_slice() {
            [one] => Ok(named(*one)),
            [] => {
                if let Some(at) = words.iter().position(|w| !types.iter().any(|e| has(*e, w))) {
                    // a word no enum has, said where it is written; in a list, the values of the
                    // enums the other words are values of, to show the one meant
                    let w = words[at];
                    let mut note = if list {
                        tr!("このフローから見える列挙に、値 `{w}` はありません", "no enum this flow sees has the value `{w}`")
                    } else {
                        tr!(
                            "値は、変数・そのフィールド・文字列・数・true・false・列挙の値のどれかです。このフローから見える列挙に、値 `{w}` はありません",
                            "a value is a variable, a field of one, a string, a number, true, false or a value of an enum, and no enum this flow sees has the value `{w}`"
                        )
                    };
                    let others: Vec<&str> = words.iter().copied().filter(|x| *x != w).collect();
                    if list && !others.is_empty() {
                        for e in types.iter().copied().filter(|e| others.iter().all(|x| has(*e, x))) {
                            let d = &self.m.enums[named(e)];
                            note.ja.push_str(&format!("。`{}` の値は {} です", d.name, d.values.join("・")));
                            note.en.push_str(&format!("; the values of `{}` are {}", d.name, d.values.join(", ")));
                        }
                    }
                    return Err(NoEnum { at, says: tr!("変数や値 `{w}` はありません", "there is no variable or value `{w}`"), note });
                }
                // every word is a value of an enum, and no one enum has them all
                let all = code(words);
                let (ja_all, en_all) = (all.join("・"), all.join(", "));
                let mut note = tr!("並びの値は、一つの型の値です", "the values of a list are of one type");
                for e in types.iter().copied().filter(|e| words.iter().any(|w| has(*e, w))) {
                    let n = &self.m.enums[named(e)].name;
                    let (got, lack): (Vec<&str>, Vec<&str>) = words.iter().copied().partition(|w| has(e, w));
                    let (got, lack) = (code(&got), code(&lack));
                    note.ja.push_str(&format!("。`{n}` には {} がありますが、{} はありません", got.join("・"), lack.join("・")));
                    note.en.push_str(&format!("; `{n}` has {} but not {}", got.join(", "), lack.join(", ")));
                }
                Err(NoEnum { at: 0, says: tr!("このフローから見える列挙に、{ja_all} のすべてを値に持つものはありません", "no enum this flow sees has all of {en_all} as values"), note })
            }
            several => {
                let names: Vec<String> = several.iter().map(|e| format!("`{}`", self.m.enums[named(*e)].name)).collect();
                let first = &self.m.enums[named(several[0])].name;
                let (ja_names, en_names) = (
                    match names.split_last() {
                        Some((last, rest)) if rest.len() == 1 => format!("{} と {last}", rest[0]),
                        _ => names.join("・"),
                    },
                    match names.split_last() {
                        Some((last, rest)) if !rest.is_empty() => format!("{} and of {last}", rest.join(", of ")),
                        _ => names.join(""),
                    },
                );
                let all = words.join(", ");
                let which = if several.len() == 2 { "どちら" } else { "どれ" };
                let ws = code(words);
                let (ja_ws, en_ws) = (ws.join("・"), ws.join(", "));
                let (is, a_value, are) = if words.len() == 1 { ("is", "a value", "is a value") } else { ("are", "values", "are values") };
                let (example, ja_where, en_where) = if list {
                    (format!("let xs : list[{first}] = [{all}]"), "並びに型を書いてください", "write the type of the list")
                } else {
                    (format!("let x : {first} = {all}"), "値を入れるところに型を書いてください", "write the type where the value goes")
                };
                Err(NoEnum {
                    at: 0,
                    says: tr!("{ja_ws} がどの列挙の値か、ここでは決まりません", "which enum {en_ws} {are} of is not known here"),
                    note: tr!("{ja_ws} は {ja_names} の{which}の値でもあります。{ja_where}（`{example}`）", "{en_ws} {is} {a_value} of {en_names} alike; {en_where} (`{example}`)"),
                })
            }
        }
    }

    /// Say `d`, unless it follows from what a declaration with an error left out (`key` in
    /// `left_out`): then it is held back, to be said only when that declaration says nothing.
    fn say_unless_left_out(&mut self, d: Diag, key: &str) {
        match self.left_out.get(key).cloned() {
            Some(lines) => self.hold_back(d, lines),
            None => self.push(d),
        }
    }

    fn hold_back(&mut self, d: Diag, lines: BTreeSet<usize>) {
        self.followers.push((d, lines));
    }

    /// Say what follows from an error at another line only when that line says none: a follower is
    /// said already when one of its lines has an error, or holds a follower that is said already.
    /// What is left is said as it is, so that a file never passes for what was held back.
    fn followers_said(&mut self) {
        let mut said: BTreeSet<usize> = self.diags.iter().filter(|d| d.severity == crate::diag::Severity::Error).map(|d| d.line).collect();
        let mut left = std::mem::take(&mut self.followers);
        loop {
            let before = left.len();
            left.retain(|(d, lines)| {
                if lines.iter().any(|l| said.contains(l)) {
                    said.insert(d.line);
                    false
                } else {
                    true
                }
            });
            if left.len() == before {
                break;
            }
        }
        self.diags.extend(left.into_iter().map(|(d, _)| d));
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
                    if !self.m.fits(&result, &want) {
                        let (a, b) = (self.m.ty_name(&result), self.m.ty_name(&want));
                        let d = e("E003", call.callee.1, tr!("`{}` が返すのは `{a}` で、`{b}` ではありません", "`{}` answers `{a}`, which is not `{b}`", call.callee.0));
                        let d = self.enums_apart(&result, &want).into_iter().fold(d, Diag::note);
                        self.push(d);
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
                        let d = e("E002", case.1, tr!("案件 `{}` はありません", "there is no case `{}`", case.0));
                        self.say_unless_left_out(d, &format!("case:{}", case.0));
                        return None;
                    }
                };
                let (callee, args) = self.call(call)?;
                let ti = match callee {
                    Callee::Task(t) => t,
                    Callee::Rule(_) => {
                        self.push(e("E008", call.callee.1, tr!("案件を動かすのは `starts`・`sends`・`observes` を書いたタスクです。規則は `let` で呼んでください", "a case moves by a task that `starts`, `sends` or `observes`; a rule is called with `let`")));
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
                    self.push(e("E009", s.span, tr!("0 秒の待ちは何もしません", "a wait of zero does nothing")));
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
                self.push(e("E009", expr.span(), tr!("`yield` は `let <名前> = for …` の本体の最後の行に書いてください", "`yield` is the last line of the body of `let <name> = for …`")));
                return None;
            }
            StmtKind::Pass => TK::Pass,
            StmtKind::Break => {
                match self.loops.last() {
                    None => {
                        self.push(e("E009", s.span, tr!("`break` は `repeat` か `for` の中に書いてください", "`break` is written inside `repeat` or `for`")));
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
                            let d = e("E002", *nsp, tr!("出力 `{n}` はありません", "there is no output `{n}`"));
                            self.say_unless_left_out(d, &format!("output:{n}"));
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
                        None => {
                            let d = e("E002", *lsp, tr!("案件 `{l}` はありません", "there is no case `{l}`"));
                            self.say_unless_left_out(d, &format!("case:{l}"));
                        }
                    }
                }
                let cause = match cause {
                    Some(c) => Some(self.expr(c, Some(&Ty::Str))?),
                    None => None,
                };
                TK::Fail { error: error.0.clone(), cause, leaving: left }
            }
        };
        self.numbers(s, site);
        Some(TStmt { kind, line, site })
    }

    /// What the numbers of a lowered statement say beyond their values (DESIGN 40): a number written
    /// with a unit is kept as it is written, by its place in the statement (`Model::written`), for
    /// what the checks and the pages quote of it; a whole number written where a rate counts steps
    /// other than 1% is said, with the place it is written for (W106).
    fn numbers(&mut self, s: &syntax::Stmt, site: usize) {
        let mut places: Vec<(String, Place, &Expr)> = Vec::new();
        match &s.kind {
            StmtKind::Let { call, .. } | StmtKind::Call { call, .. } | StmtKind::CaseCall { call, .. } => {
                places.extend(call.args.iter().map(|((a, _), x)| (a.clone(), Place::Field(a.clone()), x)));
            }
            StmtKind::Succeed { fields } => places.extend(fields.iter().map(|((f, _), x)| (f.clone(), Place::Output(f.clone()), x))),
            StmtKind::Assign { name, expr, .. } => places.push((String::new(), Place::Let(name.0.clone()), expr)),
            StmtKind::For { var, list, result, body, .. } => {
                places.push(("in".to_string(), Place::Over(var.0.clone()), list));
                if let (Some((r, _)), Some(StmtKind::Yield { expr })) = (result, body.last().map(|x| &x.kind)) {
                    places.push((String::new(), Place::Item(r.0.clone()), expr));
                }
            }
            _ => {}
        }
        for (path, place, x) in places {
            self.number_at(site, &path, &place, x);
        }
    }

    /// [`Lowerer::numbers`], of the value `x` at the place `path` of the statement `site`.
    fn number_at(&mut self, site: usize, path: &str, place: &Place, x: &Expr) {
        match x {
            Expr::Quantity { raw, .. } => {
                self.m.written.insert((site, path.to_string()), raw.clone());
            }
            Expr::Int(n, sp) => {
                let Some(rate) = self.stepped.get(&(sp.line, sp.col)).and_then(|u| in_steps(u, *n)) else { return };
                let (step, value) = (&rate.step, &rate.value);
                let said = match place {
                    Place::Item(name) => tr!(
                        "`{name}` の要素に書いた `{n}` は、{step} の刻みの数として読みます。{value} です",
                        "`{n}`, written as an item of `{name}`, counts steps of {step}: it is {value}"
                    ),
                    Place::Over(var) => tr!(
                        "`for {var}` が回るリストに書いた `{n}` は、{step} の刻みの数として読みます。{value} です",
                        "`{n}`, written in the list `for {var}` goes through, counts steps of {step}: it is {value}"
                    ),
                    Place::Field(name) | Place::Output(name) | Place::Let(name) => tr!(
                        "`{name}` に書いた `{n}` は、{step} の刻みの数として読みます。{value} です",
                        "`{n}`, written for `{name}`, counts steps of {step}: it is {value}"
                    ),
                };
                let form = match place {
                    Place::Field(name) => format!("{name}: {value}"),
                    Place::Output(name) => format!("{name} = {value}"),
                    Place::Let(_) | Place::Item(_) | Place::Over(_) => value.clone(),
                };
                let d = Diag::warning("W106", sp.line, sp.col, said).note(tr!(
                    "読む人に率が分かるよう、`{form}` と書いてください。生成するコードが渡す数（{n}）は変わりません",
                    "Write it `{form}`, so that a reader sees the rate; the generated code passes the same {n}"
                ));
                self.push(d);
            }
            Expr::Record(fields, _) => {
                for ((f, _), v) in fields {
                    self.number_at(site, &field_place(path, f), &Place::Field(f.clone()), v);
                }
            }
            Expr::List(items, _) => {
                let item = place.item();
                for (i, v) in items.iter().enumerate() {
                    self.number_at(site, &item_place(path, i), &item, v);
                }
            }
            _ => {}
        }
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
                    self.push(e("E003", *vsp, tr!("`{v}` は `{n}` の値ではありません（値は {}）", "`{v}` is not a value of `{n}` ({})", domain.join("・"); domain.join(", "))));
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
                        let d = e("E002", *sp, tr!("`{n}` はこの呼び出しが投げるエラーではありません（投げるのは {list}）", "`{n}` is not an error this call can raise ({list})"));
                        let key = match callee {
                            Callee::Task(t) => format!("error:{}!{n}", self.m.tasks[*t].name),
                            Callee::Rule(_) => String::new(),
                        };
                        self.say_unless_left_out(d, &key);
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
            // a rule without outputs answers nothing, and nothing is generated to call, however
            // `use rule` says to call it; its enums are types all the same (DESIGN 10)
            if self.m.rules[r].answers_nothing() {
                let d = nothing_to_call(&self.m.rules[r], *sp);
                self.push(d);
                return None;
            }
            let rn = self.m.rules[r].name.clone();
            let ps = self.m.rules[r].info.inputs.iter().map(|col| (col.name.clone(), self.rty(&rn, &col.ty))).collect();
            (Callee::Rule(r), ps)
        } else if let Some((file, facts)) = name.split_once('.').and_then(|(f, _)| self.date_files.get(f).map(|x| (f.to_string(), x))) {
            // a date that the dates file does not have: say which it has
            let date = &name[file.len() + 1..];
            let names: Vec<&str> = facts.functions.iter().map(|f| f.name.as_str()).collect();
            self.push(e("E002", *sp, tr!("日付のファイル `{file}` に日付 `{date}` はありません（あるのは {}）", "the dates file `{file}` has no date `{date}` ({})", names.join("・"); names.join(", "))).same_in("task", name.clone()));
            return None;
        } else {
            let d = e("E002", *sp, tr!("タスクか規則 `{name}` はありません", "there is no task or rule `{name}`")).same_in("task", name.clone());
            // a task whose declaration has an error says it there (DESIGN 30)
            match self.left_out.get(&format!("task:{name}")).cloned() {
                Some(lines) => self.hold_back(d, lines),
                None => self.push(d),
            }
            return None;
        };
        let mut args = Vec::new();
        let mut ok = true;
        for ((an, asp), ex) in &c.args {
            let pty = match params.iter().find(|(p, _)| p == an) {
                Some((_, t)) => t.clone(),
                None => {
                    let list: Vec<&str> = params.iter().map(|(p, _)| p.as_str()).collect();
                    let d = e("E004", *asp, tr!("`{name}` に引数 `{an}` はありません（あるのは {}）", "`{name}` has no parameter `{an}` ({})", list.join("・"); list.join(", ")));
                    // a parameter whose line has an error says it there (DESIGN 30)
                    match self.left_out.get(&format!("param:{name}.{an}")).cloned() {
                        Some(lines) => self.hold_back(d, lines),
                        None => self.push(d),
                    }
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
                            "`{file}` のカレンダーには UTC オフセットが書かれていないので、時刻がどの日にあたるかが決まりません。カレンダーに `offset +09:00` のように書くか、日付を渡してください",
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
            Expr::Int(n, sp) => {
                // a whole number where a rate counts steps other than 1%: W106 says it with the
                // place's name, once the statement is lowered (`numbers`)
                if let Some(Ty::Num(u)) = expected.map(Ty::inner) {
                    if in_steps(u, *n).is_some() {
                        self.stepped.insert((sp.line, sp.col), u.clone());
                    }
                }
                TExpr::Int(*n)
            }
            Expr::Quantity { value, unit, raw, span } => TExpr::Int(self.quantity(*value, unit, raw, *span, expected)?),
            Expr::Bool(b, _) => TExpr::Bool(*b),
            Expr::Interp(parts, sp) => {
                let mut out = Vec::new();
                for p in parts {
                    match p {
                        Part::Lit(s) => out.push(IPart::Lit(s.clone())),
                        Part::Hole(path) if path.len() == 1 && path[0].0 == "now" => out.push(IPart::Hole(TExpr::Now)),
                        // `{…}` puts a variable in, or a field of one; a word that is always the same
                        // is written in the string itself, so a word here is never a value of an
                        // enum (DESIGN 29)
                        Part::Hole(path) if path.len() == 1 && self.bare_word(&path[0].0) => {
                            let w = &path[0].0;
                            self.push(e("E002", path[0].1, tr!("変数 `{w}` はありません", "there is no variable `{w}`")).note(tr!(
                                "文字列の `{{…}}` に入れられるのは、変数かそのフィールドです。決まった語は、文字列にそのまま書いてください",
                                "`{{…}}` in a string puts in a variable or a field of one; a word that is always the same is written in the string as it is"
                            )));
                            return None;
                        }
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
                        // a field whose type could not be read says nothing, and the rest is checked
                        let mut unread = false;
                        for ((fname, fsp), fe) in fields {
                            let fty = match decl.iter().find(|(n, _)| n == fname) {
                                Some((_, t)) => t.clone(),
                                None => {
                                    if self.unread_field(r, fname) {
                                        unread = true;
                                        continue;
                                    }
                                    let list = self.field_names(r);
                                    self.push(e("E002", *fsp, tr!("`{rn}` にフィールド `{fname}` はありません（あるのは {}）", "`{rn}` has no field `{fname}` ({})", list.join("・"); list.join(", "))));
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
                        if unread {
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
                            tr!("`{{…}}` で書いたレコードは、型が決まるところ（引数・出力・`let x: <レコード> = {{…}}`）に書いてください", "a record written as `{{…}}` takes its type from where it goes: an argument, an output, or `let x: <record> = {{…}}`"),
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
                // a list that starts with a bare word, where nothing says its type: the first item
                // that is no bare word says it, as a first item does; else its words are values of
                // the one enum that has them all (DESIGN 29)
                if let (None, Some(Expr::Path(p))) = (&elem, items.first()) {
                    if p.len() == 1 && self.bare_word(&p[0].0) {
                        let is_bare = |x: &Expr| matches!(x, Expr::Path(q) if q.len() == 1 && self.bare_word(&q[0].0));
                        let bare: Vec<(&str, Span)> = items.iter().filter_map(|x| match x {
                            Expr::Path(q) if is_bare(x) => Some((q[0].0.as_str(), q[0].1)),
                            _ => None,
                        }).collect();
                        let others: Vec<&Expr> = items.iter().filter(|x| !is_bare(x)).collect();
                        if let Some(first) = others.first().copied() {
                            match others.iter().find_map(|x| self.quiet_expr(x, None)) {
                                Some(t) => elem = Some(t),
                                // no item that is no bare word has a type: what is wrong with the
                                // first is said, and the words wait for it
                                None => {
                                    self.expr(first, None);
                                    return None;
                                }
                            }
                        } else {
                            let words: Vec<&str> = bare.iter().map(|(w, _)| *w).collect();
                            match self.enum_of(&words, true) {
                                Ok(id) => elem = Some(Ty::Enum(id)),
                                Err(no) => {
                                    self.push(e("E002", bare[no.at].1, no.says).note(no.note));
                                    return None;
                                }
                            }
                        }
                    }
                }
                // a list whose type the place does not say takes it from an item, and a number with a
                // unit says no type of its own: it is read in the type the other items say
                // (`[fee, 0.5%]`), and where none says one, its type is not known (DESIGN 40)
                if elem.is_none() && matches!(items.first(), Some(Expr::Quantity { .. })) {
                    elem = items.iter().filter(|x| !matches!(x, Expr::Quantity { .. })).find_map(|x| self.quiet_expr(x, None));
                }
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
            let fits = self.m.fits(&got, want) || (matches!(te, TExpr::Int(_)) && matches!(want.inner(), Ty::Num(_)));
            if !fits {
                let (a, b) = (self.m.ty_name(want), self.m.ty_name(&got));
                let message = if matches!(got, Ty::Opt(_)) && self.m.fits(got.inner(), want) {
                    tr!("ここには `{a}` が要りますが、これは `none` になりうる `{b}` です。先に `none` と `some <名前>` で `match` してください", "expected `{a}` here, but this is `{b}`, which may be absent; `match` it with `none` and `some <name>` first")
                } else {
                    tr!("ここには `{a}` が要りますが、これは `{b}` です", "expected `{a}` here, but this is `{b}`")
                };
                let d = self.enums_apart(want, &got).into_iter().fold(e("E003", ex.span(), message), Diag::note);
                self.push(d);
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
                            self.push(e("E003", *fsp2, tr!("`{so_far}` は `json` で、中を見ずにそのまま受け渡す値です", "`{so_far}` is `json`, which is carried as it is and not looked into")));
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
                            // the answer of a rule without outputs, whose call is E005 already: that
                            // says what is wrong, and a field of nothing would say it again (DESIGN 10)
                            if let RecordOrigin::RuleOutputs(r) = self.m.records[rec].origin {
                                if self.m.rules[r].answers_nothing() {
                                    return None;
                                }
                            }
                            if self.unread_field(rec, f) {
                                return None;
                            }
                            let rn = self.m.records[rec].name.clone();
                            let list = self.field_names(rec);
                            self.push(e("E002", *fsp2, tr!("`{rn}` にフィールド `{f}` はありません（あるのは {}）", "`{rn}` has no field `{f}` ({})", list.join("・"); list.join(", "))));
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
                // a name the file declares or sets, whose type is not known: what is wrong is said at
                // the line that declares or sets it, and here only when that line says nothing
                // (DESIGN 30)
                if let Some(lines) = ["input", "case"].iter().find_map(|k| self.left_out.get(&format!("{k}:{first}")).cloned()) {
                    let d = e("E003", *fsp, tr!("`{first}` の型が、ここでは分かりません", "the type of `{first}` is not known here"));
                    self.hold_back(d, lines);
                    return None;
                }
                if let Some(lines) = self.set_lines.get(first).cloned() {
                    let at = lines.iter().next().copied().unwrap_or(fsp.line);
                    let d = e("E003", *fsp, tr!("変数 `{first}` の型が、ここでは分かりません", "the type of the variable `{first}` is not known here"))
                        .note(tr!("変数の型は、値を入れるところ（{at} 行）で決まります", "a variable takes its type from what is set to it (line {at})"));
                    self.hold_back(d, lines);
                    return None;
                }
                // a bare word where nothing says which enum: a value of the one enum that has it (DESIGN 29)
                if parts.len() == 1 && matches!(expected.map(|t| t.inner()), None | Some(Ty::Json)) {
                    return match self.enum_of(&[first.as_str()], false) {
                        Ok(id) => Some(TExpr::Enum(first.clone(), id)),
                        Err(no) => {
                            self.push(e("E002", *fsp, no.says).same_in("value", first.clone()).note(no.note));
                            None
                        }
                    };
                }
                let Text { en: hen, ja: hja } = match expected.map(|t| t.inner()) {
                    Some(Ty::Enum(id)) => tr!("`{}` の値は {} です", "the values of `{}` are {}", self.m.enums[*id].name, self.m.enums[*id].values.join("・"); self.m.enums[*id].name, self.m.enums[*id].values.join(", ")),
                    _ => tr!("値は変数・そのフィールド・文字列・数・true・false のどれかです", "a value is a variable, a field of one, a string, a number, true or false"),
                };
                self.push(e("E002", *fsp, tr!("変数や値 `{first}` はありません", "there is no variable or value `{first}`")).same_in("value", first.clone()).note(Text::new(hja, hen)));
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

/// An offset as koyomi reads one, `±HH:MM` with HH at most 23 and MM at most 59: minutes east of
/// UTC.
fn offset_minutes(raw: &str) -> Option<i32> {
    let b = raw.as_bytes();
    if b.len() != 6 || !(b[0] == b'+' || b[0] == b'-') || b[3] != b':' || ![1, 2, 4, 5].iter().all(|&k| b[k].is_ascii_digit()) {
        return None;
    }
    let (h, m): (i32, i32) = (raw[1..3].parse().ok()?, raw[4..6].parse().ok()?);
    if h > 23 || m > 59 {
        return None;
    }
    Some(if b[0] == b'-' { -(h * 60 + m) } else { h * 60 + m })
}

/// The names a block sets, each with the lines of what sets it (DESIGN 30): a `let` or an
/// assignment, a `for`'s variable (its line and its list) and its result (its line and what its
/// rounds yield), the name a `match` arm gives the value (`some x`, the `match` line and its value).
fn set_lines(b: &Block, out: &mut BTreeMap<String, BTreeSet<usize>>) {
    for s in b {
        let line = s.span.line;
        let mut put = |n: &str, lines: BTreeSet<usize>| out.entry(n.to_string()).or_default().extend(lines.into_iter().chain([line]));
        match &s.kind {
            StmtKind::Let { name, call, handlers, .. } => {
                put(&name.0, call_lines(call));
                for h in handlers {
                    set_lines(&h.body, out);
                }
            }
            StmtKind::Assign { name, expr, .. } => put(&name.0, expr_lines(expr)),
            StmtKind::Call { handlers, .. } | StmtKind::CaseCall { handlers, .. } => {
                for h in handlers {
                    set_lines(&h.body, out);
                }
            }
            StmtKind::Match { expr, arms } => {
                let lines = expr_lines(expr);
                for a in arms {
                    if let Some((v, vsp)) = &a.some {
                        out.entry(v.clone()).or_default().extend(lines.iter().copied().chain([line, vsp.line]));
                    }
                    set_lines(&a.body, out);
                }
            }
            StmtKind::Repeat { body, .. } => set_lines(body, out),
            StmtKind::For { var, list, body, result, .. } => {
                put(&var.0, expr_lines(list));
                if let Some((r, _)) = result {
                    let mut lines = BTreeSet::new();
                    yield_lines(body, &mut lines);
                    out.entry(r.0.clone()).or_default().extend(lines.into_iter().chain([line]));
                }
                set_lines(body, out);
            }
            _ => {}
        }
    }
}

/// The lines of a task's declaration: from its name to the last line that says something of it.
fn task_lines(t: &syntax::TaskDecl) -> BTreeSet<usize> {
    let mut last = t.name.1.line;
    let mut see = |sp: Span| last = last.max(sp.line);
    t.params.iter().for_each(|p| see(p.name.1));
    t.result.iter().for_each(|r| see(r.span()));
    t.errors.iter().for_each(|er| see(er.name.1));
    for sp in [t.binding.as_ref().map(|b| b.1), t.machine.as_ref().map(|m| m.1), t.flow.as_ref().map(|f| f.1), t.model.as_ref().map(|x| x.1), t.url.as_ref().map(|x| x.1), t.effort.as_ref().map(|x| x.1), t.workflow.as_ref().map(|x| x.1), t.state_machine.as_ref().map(|x| x.1), t.durable_function.as_ref().map(|x| x.1), t.image.as_ref().map(|x| x.1), t.argo_template.as_ref().map(|x| x.1), t.plaintext.as_ref().map(|x| x.1), t.key, t.callback, t.event].into_iter().flatten() {
        see(sp);
    }
    for n in [&t.deadline, &t.key_param, &t.refused_as].into_iter().flatten() {
        see(n.1);
    }
    (t.name.1.line..=last).collect()
}

/// The lines of what the rounds of a `for` yield, but for the `for`s inside it, which yield their own.
fn yield_lines(b: &Block, out: &mut BTreeSet<usize>) {
    for s in b {
        match &s.kind {
            StmtKind::Yield { expr } => out.extend(expr_lines(expr).into_iter().chain([s.span.line])),
            StmtKind::Match { arms, .. } => arms.iter().for_each(|a| yield_lines(&a.body, out)),
            StmtKind::Let { handlers, .. } | StmtKind::Call { handlers, .. } | StmtKind::CaseCall { handlers, .. } => handlers.iter().for_each(|h| yield_lines(&h.body, out)),
            StmtKind::Repeat { body, .. } => yield_lines(body, out),
            _ => {}
        }
    }
}

/// The lines a value is written on.
fn expr_lines(e: &Expr) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    match e {
        Expr::Path(p) => out.extend(p.iter().map(|n| n.1.line)),
        Expr::Str(_, sp) | Expr::Int(_, sp) | Expr::Bool(_, sp) | Expr::Now(sp) | Expr::Quantity { span: sp, .. } => {
            out.insert(sp.line);
        }
        Expr::Interp(parts, sp) => {
            out.insert(sp.line);
            for p in parts {
                if let Part::Hole(names) = p {
                    out.extend(names.iter().map(|n| n.1.line));
                }
            }
        }
        Expr::Record(fields, sp) => {
            out.insert(sp.line);
            for (n, v) in fields {
                out.insert(n.1.line);
                out.extend(expr_lines(v));
            }
        }
        Expr::List(items, sp) => {
            out.insert(sp.line);
            for v in items {
                out.extend(expr_lines(v));
            }
        }
    }
    out
}

/// The lines a call is written on.
fn call_lines(c: &Call) -> BTreeSet<usize> {
    let mut out = BTreeSet::from([c.callee.1.line]);
    for (n, v) in &c.args {
        out.insert(n.1.line);
        out.extend(expr_lines(v));
    }
    out
}

/// The lines that say nothing because a `use` could not be read (DESIGN 27). What is wrong is said
/// once, at the line of the `use` (E005, E016); the names it would have given are in the file all
/// the same, and are not said to be missing wherever they are written. A line is quiet when it
/// writes one of `unread` (a rule, a dates file, a book, an enum of an `import enum`, a `.proto` or
/// an OpenAPI document) or a name made from one: a variable set from it (but for one whose type is
/// written, and read: what is written with it is checked against that type), and every other line
/// that sets such a variable, an input or an output of its type, a task with a parameter or an
/// answer of it or that runs an operation of its book or its service, a field of its type read as
/// the field of a value (`parcel.zone`; in a record written out, that field says nothing and the
/// rest is checked), a case of its record or its machine. So are the values of a `match` on one, the
/// errors a call of one is handled for, the `starts` line of a task that starts its machine, and what
/// a `jev` task asks of an answer or a field of an answer of its type. Everything else of the file is
/// checked as it is, the bodies of those `match`es and handlers too.
fn unread_lines(prog: &Program, unread: &BTreeSet<String>, m: &Model, known: &BTreeMap<String, Ty>) -> BTreeSet<usize> {
    let mut u = Unread { names: unread.clone(), fields: BTreeSet::new(), m, known };
    let mut lines: BTreeSet<usize> = BTreeSet::new();
    // a name made from one leads to more, each read before or after it is set
    loop {
        let before = (u.names.len(), u.fields.len());
        for r in &prog.records {
            for f in &r.fields {
                if u.ty(&f.ty) {
                    lines.insert(f.name.1.line);
                    u.fields.insert((r.name.0.clone(), f.name.0.clone()));
                }
            }
        }
        for f in prog.inputs.iter().chain(&prog.outputs) {
            if u.ty(&f.ty) {
                lines.insert(f.name.1.line);
                u.names.insert(f.name.0.clone());
            }
        }
        for t in &prog.tasks {
            let mut quiet: Vec<usize> = t.params.iter().filter(|p| u.ty(&p.ty)).map(|p| p.name.1.line).collect();
            quiet.extend(t.result.iter().filter(|r| u.ty(r)).map(|r| r.span().line));
            // an operation of a book, or of a `.proto` or an OpenAPI document, that could not be read
            let operation_of = match &t.binding {
                Some((syntax::Binding::Book { book: n, .. } | syntax::Binding::Connect { api: n, .. } | syntax::Binding::Http { api: Some(n), .. }, sp)) => Some((n, sp)),
                _ => None,
            };
            if let Some((_, sp)) = operation_of.filter(|(n, _)| u.names.contains(&n.0)) {
                quiet.push(sp.line);
            }
            // the machine of a rule that could not be read (`starts payment_intent.payment`): the
            // line says nothing, and the rest of the task is checked
            if let Some((syntax::MachineUse::Starts { machine, .. }, sp)) = &t.machine {
                if machine.first().is_some_and(|n| u.names.contains(&n.0)) {
                    lines.insert(sp.line);
                }
            }
            // what Jev is asked of an answer whose type could not be read, or of a field of it whose
            // type could not be read: the question, its values, how sure it is of it, and the `jev`
            // line, which holds the answer to what it asks
            if let Some((syntax::Binding::Jev(j), sp)) = &t.binding {
                let whole = t.result.as_ref().is_some_and(|r| u.ty(r));
                let rec = match &t.result {
                    Some(TypeExpr::Named(parts)) if parts.len() == 1 => Some(parts[0].0.clone()),
                    _ => None,
                };
                let unread_field = |n: &str| whole || rec.as_ref().is_some_and(|r| u.fields.contains(&(r.clone(), n.to_string())));
                let ask_lines = |a: &syntax::JevAsk| std::iter::once(a.span.line).chain(a.criteria.iter().map(|(v, _)| v.1.line)).collect::<Vec<_>>();
                let mut asked: Vec<usize> = Vec::new();
                if let Some(a) = j.ask.as_ref().filter(|_| whole) {
                    asked.extend(ask_lines(a));
                }
                for (n, f) in &j.fields {
                    match f {
                        syntax::JevField::Ask(a) if unread_field(&n.0) => {
                            asked.push(n.1.line);
                            asked.extend(ask_lines(a));
                        }
                        syntax::JevField::Confidence(of) if unread_field(&of.0) => {
                            asked.push(n.1.line);
                            asked.push(of.1.line);
                        }
                        _ => {}
                    }
                }
                if !asked.is_empty() {
                    lines.extend(asked);
                    lines.insert(sp.line);
                }
            }
            if !quiet.is_empty() {
                lines.extend(quiet);
                lines.insert(t.name.1.line);
                u.names.insert(t.name.0.clone());
            }
        }
        for c in &prog.cases {
            let named = |n: &syntax::Name| u.names.contains(&n.0);
            if u.ty(&c.record) || c.machine.first().is_some_and(named) || c.held.iter().any(|(a, _)| named(a)) {
                lines.insert(c.name.1.line);
                u.names.insert(c.name.0.clone());
            }
        }
        for (b, _) in prog.flow.iter().chain(&prog.on_failure).chain(&prog.on_cancel) {
            u.block(b, &mut lines);
        }
        if (u.names.len(), u.fields.len()) == before {
            break;
        }
    }
    lines
}

/// Why bare words read where nothing says their type are no value of one enum
/// (`Lowerer::enum_of`): the word to say it at, by its place among the words, what E002 says, and
/// its note.
struct NoEnum {
    at: usize,
    says: Text,
    note: Text,
}

/// What [`unread_lines`] has found of the names that could not be read: the names (the ones the
/// `use` lines give, and each variable, input, output, task and case made from one), and the fields
/// of the records whose type is one, by record.
struct Unread<'a> {
    names: BTreeSet<String>,
    fields: BTreeSet<(String, String)>,
    /// the records of the model, which a field of a value whose type is read is followed through
    m: &'a Model,
    /// the inputs, the cases and the variables whose type is read
    known: &'a BTreeMap<String, Ty>,
}

impl Unread<'_> {
    /// The statements of a block: the lines that write one, and the names they set.
    fn block(&mut self, b: &Block, lines: &mut BTreeSet<usize>) {
        for s in b {
            let line = s.span.line;
            match &s.kind {
                // a variable whose type is written, and read, is not made from one: what is written
                // with it is checked against that type. One whose type is not written, and that
                // another line sets from one, has no type to check this line's value against
                StmtKind::Let { name, ty, call, handlers } => {
                    let unread_ty = ty.as_ref().map(|t| self.ty(t));
                    if unread_ty == Some(true) || self.call(call) || (unread_ty.is_none() && self.made(&name.0)) {
                        lines.insert(line);
                        lines.extend(handlers.iter().map(|h| h.span.line));
                        if unread_ty != Some(false) {
                            self.names.insert(name.0.clone());
                        }
                    }
                    for h in handlers {
                        self.block(&h.body, lines);
                    }
                }
                StmtKind::Assign { name, ty, expr } => {
                    let unread_ty = ty.as_ref().map(|t| self.ty(t));
                    if unread_ty == Some(true) || self.expr(expr) || (unread_ty.is_none() && self.made(&name.0)) {
                        lines.insert(line);
                        if unread_ty != Some(false) {
                            self.names.insert(name.0.clone());
                        }
                    }
                }
                StmtKind::Call { call, handlers } | StmtKind::CaseCall { call, handlers, .. } => {
                    let case = match &s.kind {
                        StmtKind::CaseCall { case, .. } => self.names.contains(&case.0),
                        _ => false,
                    };
                    if case || self.call(call) {
                        lines.insert(line);
                        lines.extend(handlers.iter().map(|h| h.span.line));
                    }
                    for h in handlers {
                        self.block(&h.body, lines);
                    }
                }
                StmtKind::Match { expr, arms } => {
                    let quiet = self.expr(expr);
                    if quiet {
                        lines.insert(line);
                    }
                    for a in arms {
                        if quiet {
                            lines.insert(a.span.line);
                            self.names.extend(a.some.iter().map(|v| v.0.clone()));
                        }
                        self.block(&a.body, lines);
                    }
                }
                StmtKind::WaitUntil { at } | StmtKind::Yield { expr: at } => {
                    if self.expr(at) {
                        lines.insert(line);
                    }
                }
                StmtKind::Repeat { body, .. } => self.block(body, lines),
                StmtKind::For { var, list, body, result, .. } => {
                    let quiet = self.expr(list);
                    if quiet {
                        lines.insert(line);
                        self.names.insert(var.0.clone());
                    }
                    self.block(body, lines);
                    if let Some((r, rty)) = result {
                        let yields = body.iter().any(|x| matches!(&x.kind, StmtKind::Yield { expr } if self.expr(expr)));
                        let unread_ty = rty.as_ref().map(|t| self.ty(t));
                        if quiet || yields || unread_ty == Some(true) {
                            lines.insert(line);
                            if unread_ty != Some(false) {
                                self.names.insert(r.0.clone());
                            }
                        }
                    }
                }
                StmtKind::Succeed { fields } => {
                    if fields.iter().any(|(n, v)| self.names.contains(&n.0) || self.expr(v)) {
                        lines.insert(line);
                    }
                }
                StmtKind::Fail { cause, leaving, .. } => {
                    if cause.as_ref().is_some_and(|c| self.expr(c)) || leaving.iter().any(|n| self.names.contains(&n.0)) {
                        lines.insert(line);
                    }
                }
                StmtKind::Wait { .. } | StmtKind::Break | StmtKind::Pass => {}
            }
        }
    }

    /// Whether a variable is made from one, and has no type that is read: a line sets it from one
    /// (`let s = r.next_stage`), so a value another line sets it to (`let s = received`) has no type
    /// to be checked against.
    fn made(&self, name: &str) -> bool {
        self.names.contains(name) && !self.known.contains_key(name)
    }

    /// Whether a type names one: `terms`, `refund_limit.band`, `list[zone]`.
    fn ty(&self, t: &TypeExpr) -> bool {
        match t {
            TypeExpr::Named(parts) => self.names.contains(&parts[0].0) || self.names.contains(&parts.iter().map(|p| p.0.as_str()).collect::<Vec<_>>().join(".")),
            TypeExpr::List(inner, _) | TypeExpr::Opt(inner, _) => self.ty(inner),
            _ => false,
        }
    }

    /// Whether a path names one: by its first word, or by a field of one after it. Of a value whose
    /// type is read, the fields are followed through its records: `order.item` is one only when the
    /// record of `order` has `item` of a type that is one. Of any other name, nothing: a variable
    /// whose type is not known for an error at the line that sets it says nothing where it is read
    /// all the same (`Lowerer::followers`, DESIGN 30), and a name nothing sets is not there.
    fn path(&self, p: &[syntax::Name]) -> bool {
        let Some(first) = p.first() else { return false };
        if self.names.contains(&first.0) {
            return true;
        }
        let Some(mut t) = self.known.get(&first.0) else {
            return false;
        };
        for n in &p[1..] {
            let Ty::Record(r) = t.inner() else { return false };
            let rec = &self.m.records[*r];
            match rec.fields.iter().find(|(f, _)| *f == n.0) {
                Some((_, ft)) => t = ft,
                None => return self.fields.contains(&(rec.name.clone(), n.0.clone())),
            }
        }
        false
    }

    /// Whether a value names one, in a string's `{…}` too. A record written out with a field whose
    /// type is one is not: that field says nothing (`Lowerer::unread_field`), and the rest is
    /// checked.
    fn expr(&self, e: &Expr) -> bool {
        match e {
            Expr::Path(p) => self.path(p),
            Expr::Interp(parts, _) => parts.iter().any(|x| matches!(x, Part::Hole(p) if self.path(p))),
            Expr::Record(fs, _) => fs.iter().any(|(_, v)| self.expr(v)),
            Expr::List(xs, _) => xs.iter().any(|x| self.expr(x)),
            Expr::Str(..) | Expr::Int(..) | Expr::Bool(..) | Expr::Now(_) | Expr::Quantity { .. } => false,
        }
    }

    /// Whether a call names one: what it calls (`terms.due`, a task), or a value it passes.
    fn call(&self, c: &Call) -> bool {
        let callee = &c.callee.0;
        self.names.contains(callee) || callee.split('.').next().is_some_and(|f| self.names.contains(f)) || c.args.iter().any(|(_, v)| self.expr(v))
    }
}
