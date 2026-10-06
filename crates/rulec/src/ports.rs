//! ritsu's ports, as rulec answers them (ritsu's DESIGN 3.2): [`Engine`] implements the port of
//! rules (`ritsu_ports::Rules`), and gives what a rule file holds and what it names outside
//! itself (`Items`, `References`). A program that holds rulec as a library — dandori, yuen, sakai,
//! ritsu's checks across languages — reads a rule through these, with the units the JSON of
//! `rulec api` only describes in words, instead of running rulec and reading its JSON.
//!
//! Nothing here decides anything rulec's check does not: the facts are read off the checked rule
//! by the same helpers `api`, `certificate` and `schema` write their JSON from, and the page is
//! the one `doc` draws.

use crate::ast::{Item as AstItem, Name, RuleFile, SourceKind};
use crate::codegen::pub_name_of;
use crate::i18n::{self, Lang as RLang};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use ritsu_base::naming::{Name as Naming, Tool};
use ritsu_base::text::{Lang, Text};
use ritsu_ports::{Answer, Column, ColumnType, DaySet, Item, Precondition, Reference, RuleError, RuleFacts, Said, Value, Values};
use std::path::Path;

/// rulec, as the ports reach it. It keeps each rule it has checked, by its path and its text, so
/// that a program asking one rule many times (an evaluation per call of a workflow) checks it
/// once; and the report `rulec check` prints of it, so that `ritsu check`, which prints that report
/// and then asks the rule's facts for the workflows that use it, checks it once too.
#[derive(Default)]
pub struct Engine {
    checked: std::sync::Mutex<std::collections::HashMap<(String, String), Checked2>>,
    /// By the file (its path with its links followed, so two spellings of one path are one file)
    /// and its text: the path the report was made with, its language, and the report.
    reports: std::sync::Mutex<std::collections::HashMap<(std::path::PathBuf, String), (String, RLang, std::sync::Arc<crate::Report>)>>,
    /// How many times a rule's text has been checked whole (what [`Engine::checks`] says).
    checks: std::sync::atomic::AtomicUsize,
    /// The port of dates a rule's `range from koyomi` is read through (§15.174); None where no
    /// koyomi is joined.
    dates: Option<crate::days::Port>,
}

/// The file a path names, its links followed and `..` folded: how the reports are kept.
fn the_file(path: &str) -> std::path::PathBuf {
    ritsu_base::fs::canonicalize(path).unwrap_or_else(|_| std::path::PathBuf::from(path))
}

/// A rule read and checked, or what check said of it.
type Checked2 = std::sync::Arc<Result<(RuleFile, Checked), Vec<Said>>>;

fn rlang(l: Lang) -> RLang {
    match l {
        Lang::Ja => RLang::Ja,
        Lang::En => RLang::En,
    }
}

/// The file's text, as the caller named it.
fn read(rule: &Path) -> Result<(String, String), Vec<Said>> {
    let path = rule.to_string_lossy().to_string();
    match ritsu_base::fs::read_to_string(rule) {
        Ok(src) => Ok((path, src)),
        Err(e) => Err(vec![Said::unreadable(&path, &e.to_string())]),
    }
}

/// What check says of a rule that does not pass, each error in both languages.
fn refused(path: &str, src: &str) -> Vec<Said> {
    let ja = i18n::with(RLang::Ja, || crate::report(src, path).diags);
    let en = i18n::with(RLang::En, || crate::report(src, path).diags);
    ja.iter()
        .zip(&en)
        .filter(|(d, _)| d.severity == crate::diag::Severity::Error)
        .map(|(j, e)| Said { code: j.code.to_string(), file: path.to_string(), line: j.line(), message: Text::new(j.title.clone(), e.title.clone()) })
        .collect()
}

/// The rule, when it passes the whole of check (as `api` and `certificate` ask of it).
fn checked(path: &str, src: &str) -> Result<(RuleFile, Checked), Vec<Said>> {
    let rep = crate::report(src, path);
    if crate::has_error(&rep.diags) {
        return Err(refused(path, src));
    }
    crate::prepare(src, path).map_err(|_| refused(path, src))
}

/// A value's type as the port hands it over: a number with its unit and its range on the wire.
fn column_type(c: &Checked, name: &str, ty: &Ty) -> ColumnType {
    match ty {
        Ty::Bool => ColumnType::Bool,
        Ty::Str => ColumnType::Str,
        Ty::Date => ColumnType::Date,
        Ty::Enum(e) => ColumnType::Enum(e.clone()),
        Ty::Opt(t) => ColumnType::Opt(Box::new(column_type(c, name, t))),
        _ => {
            let sc = c.wire_scale(name);
            let step = matches!(ty, Ty::Rate).then(|| Rat::new(1, sc));
            let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
            ColumnType::Num {
                written: ty.to_string(),
                unit: ty.unit(step),
                min: lo.map(|v| crate::types::wire_int(v, sc)),
                max: hi.map(|v| crate::types::wire_int(v, sc)),
            }
        }
    }
}

fn column(c: &Checked, n: &Name) -> Column {
    let ty = c.ty_of(&n.text).unwrap_or(Ty::Unknown);
    Column { name: n.text.clone(), alias: pub_name_of(n), ty: column_type(c, &n.text, &ty) }
}

fn preconditions(f: &RuleFile, c: &Checked) -> Vec<Precondition> {
    crate::verify::preconditions(f, c)
        .into_iter()
        .map(|p| match p {
            crate::verify::Pre::Rel { left, op, right } => Precondition::Relation { left, op: op.to_string(), right },
            crate::verify::Pre::Sum { name, over, of, max } => Precondition::Sum { name, over, of, max },
            crate::verify::Pre::Length { sequence, max } => Precondition::Length { sequence, max },
            crate::verify::Pre::Days { input, file, date, days } => Precondition::Days { input, file, date, days: days.into_iter().collect() },
        })
        .collect()
}

impl Engine {
    pub fn new() -> Engine {
        Engine::default()
    }

    /// The engine with koyomi joined: a rule whose range is a date of a koyomi file reads its
    /// days through `dates` (§15.174).
    pub fn with_dates(dates: crate::days::Port) -> Engine {
        Engine { dates: Some(dates), ..Engine::default() }
    }

    /// Runs `f` with this engine's port of dates joined on the thread.
    fn joined<R>(&self, f: impl FnOnce() -> R) -> R {
        crate::days::with(self.dates.clone(), f)
    }

    /// The rule at `rule`, checked: read again when its text has changed. A report `check` already
    /// made of the same text says whether the rule passes, so it is not checked a second time.
    fn rule(&self, rule: &Path) -> Result<(String, String, Checked2), Vec<Said>> {
        self.joined(|| self.rule_here(rule))
    }

    fn rule_here(&self, rule: &Path) -> Result<(String, String, Checked2), Vec<Said>> {
        let (path, src) = read(rule)?;
        let key = (path.clone(), crate::sha256::hex(src.as_bytes()));
        let reported = self.reports.lock().unwrap_or_else(|e| e.into_inner()).get(&(the_file(&path), key.1.clone())).map(|(_, _, r)| r.clone());
        let mut cache = self.checked.lock().unwrap_or_else(|e| e.into_inner());
        let got = cache
            .entry(key)
            .or_insert_with(|| {
                std::sync::Arc::new(match reported {
                    Some(r) if crate::has_error(&r.diags) => Err(refused(&path, &src)),
                    Some(_) => crate::prepare(&src, &path).map_err(|_| refused(&path, &src)),
                    None => {
                        self.checks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        checked(&path, &src)
                    }
                })
            })
            .clone();
        Ok((path, src, got))
    }

    /// The report `rulec check` makes of a rule's text, in `lang`, naming the file `path` (its
    /// findings are written as they are found, so a report is kept for the language and the path
    /// it was made with; whether the rule passes, [`Engine::rule`] reads off any of them).
    fn report(&self, path: &str, src: &str, lang: RLang) -> std::sync::Arc<crate::Report> {
        let key = (the_file(path), crate::sha256::hex(src.as_bytes()));
        let mut reports = self.reports.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((p, l, r)) = reports.get(&key)
            && p == path
            && *l == lang
        {
            return r.clone();
        }
        self.checks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let r = std::sync::Arc::new(self.joined(|| i18n::with(lang, || crate::report(src, path))));
        reports.insert(key, (path.to_string(), lang, r.clone()));
        r
    }

    /// How many times the engine has checked a rule's text whole, for `check` or for a port's
    /// answer: once for each rule in a run that asks the same rule both ways (ritsu's DESIGN 6.1).
    pub fn checks(&self) -> usize {
        self.checks.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// `rulec check` of each file, as `ritsu check` prints it (ritsu's DESIGN 8.3): every finding,
    /// as the command prints it and as its `--format json` prints it, then what the command prints
    /// after them (`ok rules/送料.rule`). `files` are as the person gave them, from where the
    /// program runs; `root` is the project's, which the findings' files are written from.
    pub fn checked(&self, root: &Path, files: &[String], lang: Lang) -> Vec<ritsu_ports::Checked> {
        use ritsu_ports::{Checked, Finding, Part, Verdict};
        let rl = rlang(lang);
        files
            .iter()
            .map(|path| {
                let Ok(src) = ritsu_base::fs::read_to_string(path) else {
                    return Checked::unchecked(path, i18n::with(rl, || format!("{}\n", crate::tr!("error: `{path}` を読めません", "error: cannot read `{path}`"))));
                };
                let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
                let r = self.report(path, &src, rl);
                let file = ritsu_base::paths::from_root(root, Path::new(path));
                let mut parts: Vec<Part> = r
                    .diags
                    .iter()
                    .map(|d| {
                        let json = ritsu_base::json::parse(&crate::diag::render_json(d, path)).unwrap_or(ritsu_base::json::Json::Null);
                        Part::Finding(Finding {
                            code: d.code.to_string(),
                            severity: match d.severity {
                                crate::diag::Severity::Error => ritsu_base::diag::Severity::Error,
                                crate::diag::Severity::Warning => ritsu_base::diag::Severity::Warning,
                            },
                            file: file.clone(),
                            line: d.line(),
                            text: i18n::with(rl, || crate::findings_text(std::slice::from_ref(d), &lines)),
                            json,
                        })
                    })
                    .collect();
                parts.push(Part::Text(i18n::with(rl, || crate::check_tail(&r.shadow, &r.diags, path, 0))));
                // E129: no koyomi is joined to read the rule's range from (§15.174), which the
                // command exits 2 for.
                let verdict = if r.diags.iter().any(|d| d.code == "E129") {
                    Verdict::Unchecked
                } else if crate::has_error(&r.diags) {
                    Verdict::Fails
                } else {
                    Verdict::Passes
                };
                Checked { label: path.clone(), parts, verdict }
            })
            .collect()
    }

    /// The facts of a rule already read and checked.
    pub fn facts_of(&self, f: &RuleFile, c: &Checked, src: &str, path: &str) -> Result<RuleFacts, Vec<Said>> {
        let g = crate::codegen::Gen::new(f, c, src, path);
        let (typescript, python, go) = g.calls();
        let machine = match crate::cert::machine_facts(f, c) {
            None => None,
            Some(Ok(m)) => Some(m),
            Some(Err(_)) => {
                let why = |l| i18n::with(l, || crate::cert::machine_facts(f, c).and_then(|r| r.err()).unwrap_or_default());
                return Err(vec![Said {
                    code: String::new(),
                    file: path.to_string(),
                    line: f.machine.as_ref().map(|m| m.span.line),
                    message: Text::new(
                        format!("このステートマシンの遷移は、表の行から読めません: {}", why(RLang::Ja)),
                        format!("the moves of this machine cannot be read off a table's rows: {}", why(RLang::En)),
                    ),
                }]);
            }
        };
        Ok(RuleFacts {
            rule: f.name.text.clone(),
            alias: pub_name_of(&f.name),
            version: f.version.clone(),
            sha256: crate::sha256::hex(src.as_bytes()),
            rulec: env!("CARGO_PKG_VERSION").to_string(),
            inputs: f.inputs.iter().map(|i| column(c, &i.name)).collect(),
            outputs: f.outputs.iter().map(|o| column(c, &o.name)).collect(),
            elements: f.elements.as_ref().map(|el| (el.name.text.clone(), pub_name_of(&el.name), el.fields.iter().map(|fd| column(c, &fd.name)).collect())),
            enums: g.rule_enums(),
            machine,
            preconditions: preconditions(f, c),
            connect: Some(g.connect_facts()),
            typescript,
            python,
            go,
        })
    }
}

/// The value of an input on the wire, as rulec's evaluator holds it.
fn to_val(c: &Checked, name: &str, ty: &Ty, v: &Value) -> Result<crate::eval::Val, Text> {
    use crate::eval::Val;
    let wrong = || ritsu_base::tr!("`{name}` に {ty} として読めない値が渡されています", "`{name}` is given a value that is not a {ty}");
    match (ty, v) {
        (Ty::Opt(t), v) => to_val(c, name, t, v),
        (Ty::Bool, Value::Bool(b)) => Ok(Val::Bool(*b)),
        (Ty::Str, Value::Str(s)) => Ok(Val::Str(s.clone())),
        (Ty::Enum(e), Value::Enum(x)) => {
            if c.enums.get(e).is_some_and(|vs| vs.contains(x)) {
                Ok(Val::Enum(x.clone()))
            } else {
                Err(ritsu_base::tr!("`{name}` の `{x}` は {e} の値ではありません", "`{x}` of `{name}` is not a value of {e}"))
            }
        }
        (Ty::Date, Value::Date(d)) => {
            let p: Vec<&str> = d.split('-').collect();
            match (p.first().and_then(|y| y.parse().ok()), p.get(1).and_then(|m| m.parse().ok()), p.get(2).and_then(|x| x.parse().ok())) {
                (Some(y), Some(m), Some(dd)) if p.len() == 3 => Ok(Val::Date(y, m, dd)),
                _ => Err(wrong()),
            }
        }
        (t, Value::Int(n)) if t.is_numeric() => Ok(Val::Num(crate::types::from_wire(*n, c.wire_scale(name)))),
        _ => Err(wrong()),
    }
}

/// An output of the evaluator, on the wire: `none` of a value that may be absent is the absence.
pub(crate) fn from_val(c: &Checked, name: &str, v: &crate::eval::Val) -> Value {
    use crate::eval::Val;
    match v {
        Val::Enum(s) if s == crate::kw::NONE && matches!(c.ty_of(name), Some(Ty::Opt(_))) => Value::None,
        Val::Enum(s) => Value::Enum(s.clone()),
        Val::Num(r) => Value::Int(crate::types::wire_int(*r, c.wire_scale(name))),
        Val::Bool(b) => Value::Bool(*b),
        Val::Str(s) => Value::Str(s.clone()),
        Val::Date(y, m, d) => Value::Date(format!("{y:04}-{m:02}-{d:02}")),
        Val::Seq(xs) => Value::List(xs.iter().map(|x| x.iter().map(|(k, v)| (k.clone(), from_val(c, k, v))).collect()).collect()),
    }
}

/// Whether `l op r` holds between two rationals.
fn holds(l: Rat, op: &str, r: Rat) -> bool {
    use std::cmp::Ordering::*;
    match (op, l.cmp_to(r)) {
        ("<=", Less | Equal) | ("<", Less) | (">=", Greater | Equal) | (">", Greater) => true,
        _ => false,
    }
}

impl ritsu_ports::Rules for Engine {
    fn facts(&self, rule: &Path) -> Result<RuleFacts, Vec<Said>> {
        let (path, src, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        self.facts_of(f, c, &src, &path)
    }

    /// A relation between two inputs is decided over the box of their ranges: it holds for every
    /// pair exactly when it holds for the pair at the corner that tries it hardest, and that pair
    /// is the example when it does not. The bounds on a list's total and length are about a list
    /// whose length the question does not carry, and are not decided.
    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)], max_len: Option<i128>) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>> {
        let (_, _, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        let range = |name: &str| -> (Option<Rat>, Option<Rat>) {
            let sc = c.wire_scale(name);
            match ranges.iter().find(|(n, ..)| n == name) {
                Some((_, lo, hi)) => (lo.map(|v| crate::types::from_wire(v, sc)), hi.map(|v| crate::types::from_wire(v, sc))),
                None => c.ranges.get(name).copied().unwrap_or((None, None)),
            }
        };
        Ok(preconditions(f, c)
            .into_iter()
            .map(|p| {
                let a = match &p {
                    Precondition::Relation { left, op, right } => {
                        let ((llo, lhi), (rlo, rhi)) = (range(left), range(right));
                        // the corner: the left side as large as it gets against the right as small,
                        // for `<` and `<=`; the other way round for `>` and `>=`
                        let corner = if op.starts_with('<') { (lhi, rlo) } else { (llo, rhi) };
                        match corner {
                            (Some(l), Some(r)) if holds(l, op, r) => Answer::Holds,
                            (Some(l), Some(r)) => Answer::Fails(vec![
                                (left.clone(), Value::Int(crate::types::wire_int(l, c.wire_scale(left)))),
                                (right.clone(), Value::Int(crate::types::wire_int(r, c.wire_scale(right)))),
                            ]),
                            _ => Answer::Undecided(ritsu_base::tr!(
                                "`{left}` か `{right}` の範囲に上限か下限が無いので、`{left} {op} {right}` がどの値でも成り立つとは言えません",
                                "`{left}` or `{right}` has an open end, so `{left} {op} {right}` cannot be shown to hold for every value"
                            )),
                        }
                    }
                    // The total is largest with the list as long as it gets and every element at
                    // the top of its range (none when that top is below zero: the empty list's
                    // total, 0, is then the largest).
                    Precondition::Sum { name, over, of, max } => match (max_len, range(of).1) {
                        (Some(n), Some(hi)) => {
                            let top = crate::types::wire_int(hi, c.wire_scale(of));
                            let most = if top > 0 { top.saturating_mul(n) } else { 0 };
                            if most <= *max {
                                Answer::Holds
                            } else {
                                let one = vec![(of.clone(), Value::Int(top))];
                                Answer::Fails(vec![(over.clone(), Value::List(vec![one; n.clamp(0, 10_000) as usize]))])
                            }
                        }
                        (None, _) => Answer::Undecided(ritsu_base::tr!(
                            "`{name}` は `{over}` の合計の上限です。並びの長さの上限が分からないので、決められません",
                            "`{name}` bounds a total over `{over}`, and how long the list can be is not known, so it cannot be decided"
                        )),
                        (Some(_), None) => Answer::Undecided(ritsu_base::tr!(
                            "`{name}` は `{over}` の `{of}` の合計の上限です。`{of}` の範囲に上限が無いので、決められません",
                            "`{name}` bounds the total of `{of}` over `{over}`, and the range of `{of}` has no upper end, so it cannot be decided"
                        )),
                    },
                    Precondition::Length { sequence, max } => match max_len {
                        Some(n) if n <= *max => Answer::Holds,
                        Some(_) => Answer::Fails(vec![(sequence.clone(), Value::List(vec![Vec::new(); (*max + 1).clamp(0, 10_000) as usize]))]),
                        None => Answer::Undecided(ritsu_base::tr!(
                            "`{sequence}` の長さの上限です。並びの長さの上限が分からないので、決められません",
                            "it bounds the length of `{sequence}`, and how long the list can be is not known, so it cannot be decided"
                        )),
                    },
                    Precondition::Days { input, .. } => Answer::Undecided(ritsu_base::tr!(
                        "`{input}` は koyomi の日付がとる日だけをとります。問い合わせには範囲しか入らないので、渡す日がその日のどれかであることまでは示せません",
                        "`{input}` takes only the days of a koyomi date, and a range alone cannot show that the day given is one of them"
                    )),
                };
                (p, a)
            })
            .collect())
    }

    /// The rule checked again with the date input `input` taking only `days` (inside what the
    /// rule declares for it), as a rule whose range is a koyomi date is checked over its days
    /// (§15.174): its tables complete, without overlap, every row reached. The first error that
    /// says otherwise is the example; a rule that does not pass its own check is refused.
    fn checked_over(&self, rule: &Path, input: &str, days: &DaySet) -> Result<Answer<Text>, Vec<Said>> {
        let (path, src, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        if !f.inputs.iter().any(|i| i.name.text == input) || c.ty_of(input) != Some(Ty::Date) {
            return Ok(Answer::Undecided(ritsu_base::tr!("`{input}` は、この規則の日付の入力ではありません", "`{input}` is not a date input of this rule")));
        }
        let given: Vec<i64> = days.iter().copied().collect();
        let both = |l: RLang| self.joined(|| crate::days::over(input, given.clone(), || i18n::with(l, || crate::report(&src, &path).diags)));
        let (ja, en) = (both(RLang::Ja), both(RLang::En));
        let first = ja.iter().zip(&en).find(|(d, _)| d.severity == crate::diag::Severity::Error && matches!(d.code, "E101" | "E102" | "E105"));
        Ok(match first {
            None => Answer::Holds,
            Some((j, e)) => {
                let line = |d: &crate::diag::Diag| d.line().map(|l| format!(":{l}")).unwrap_or_default();
                Answer::Fails(Text::new(format!("[{}] {path}{}: {}", j.code, line(j), j.title), format!("[{}] {path}{}: {}", e.code, line(e), e.title)))
            }
        })
    }

    /// The output's range is the interval rulec holds it to (a table's rows, a derive's
    /// arithmetic); where every row that decides it writes a number, those numbers, each with an
    /// input from the rule's vectors (which take every row) that comes to it.
    fn output_values(&self, rule: &Path, output: &str) -> Result<ritsu_ports::Found<ritsu_ports::OutputValues>, Vec<Said>> {
        use crate::ast::{Lit, OutCell};
        let (_, _, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        if !f.outputs.iter().any(|o| o.name.text == output) || !c.ty_of(output).is_some_and(|t| t.is_numeric()) {
            return Ok(ritsu_ports::Found::Undecided(ritsu_base::tr!("`{output}` は、この規則の数の出力ではありません", "`{output}` is not a numeric output of this rule")));
        }
        let sc = c.wire_scale(output);
        let (lo, hi) = c.ranges.get(output).copied().unwrap_or((None, None));
        // every number the rows that decide the output write, if each writes one
        let ty = c.ty_of(output).unwrap_or(Ty::Unknown);
        let mut literal: Option<Vec<i128>> = Some(Vec::new());
        for set in &c.sets {
            let Some(oi) = set.table.outputs.iter().position(|o| o.name.text == output) else { continue };
            for row in &set.table.rows {
                match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => match crate::types::lit_value_in_pub(n, &ty) {
                        Some(v) => {
                            if let Some(vs) = literal.as_mut() {
                                vs.push(crate::types::wire_int(v, sc));
                            }
                        }
                        None => literal = None,
                    },
                    _ => literal = None,
                }
            }
        }
        let values = literal.filter(|v| !v.is_empty()).map(|mut v| {
            v.sort_unstable();
            v.dedup();
            v
        });
        let (min, max) = (lo.map(|v| crate::types::wire_int(v, sc)), hi.map(|v| crate::types::wire_int(v, sc)));
        // an input for each value, from the vectors
        let mut wanted: Vec<i128> = min.into_iter().chain(max).collect();
        wanted.extend(values.iter().flatten().copied());
        wanted.dedup();
        let mut examples: Vec<(i128, Values)> = Vec::new();
        let vectors = self.joined(|| crate::vectors::generate(f, c));
        for w in wanted {
            if examples.iter().any(|(v, _)| *v == w) {
                continue;
            }
            let hit = vectors.iter().find(|v| v.outputs.iter().any(|(n, x)| n == output && matches!(x, Some(crate::eval::Val::Num(r)) if crate::types::wire_int(*r, sc) == w)));
            if let Some(v) = hit {
                let inputs: Values = v.input.iter().map(|(n, x)| (n.clone(), from_val(c, n, x))).collect();
                examples.push((w, inputs));
            }
        }
        Ok(ritsu_ports::Found::Value(ritsu_ports::OutputValues { min, max, values, examples }))
    }

    /// The rule read again with each input held to its range, the rows some input may reach
    /// found through the sieve of the completeness proof, and an input for each of their values
    /// found in the vectors over the ranges (`crate::over`). Exact when the two meet.
    fn outputs_over(&self, rule: &Path, output: &str, ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<ritsu_ports::Found<Vec<(Value, Values)>>, Vec<Said>> {
        let (path, src, got) = self.rule(rule)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        Ok(self.joined(|| crate::over::outputs_over(f, c, &src, &path, output, ranges)))
    }

    fn date_range(&self, rule: &Path, input: &str) -> Result<(Option<i64>, Option<i64>), Vec<Said>> {
        let (_, _, got) = self.rule(rule)?;
        let (_, c) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        let (lo, hi) = c.ranges.get(input).copied().unwrap_or((None, None));
        let day = |r: Option<Rat>| r.map(|v| (v.num / v.den) as i64);
        Ok((day(lo), day(hi)))
    }

    fn eval(&self, rule: &Path, inputs: &Values) -> Result<Values, RuleError> {
        use crate::eval::Val;
        let (_, _, got) = self.rule(rule).map_err(RuleError::Unread)?;
        let (f, c) = got.as_ref().as_ref().map_err(|e| RuleError::Unread(e.clone()))?;
        let mut vals = std::collections::HashMap::new();
        for i in &f.inputs {
            let name = &i.name.text;
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            let v = inputs.iter().find(|(n, _)| n == name).map(|(_, v)| v).ok_or_else(|| RuleError::Input(ritsu_base::tr!("入力 `{name}` がありません", "the input `{name}` is missing")))?;
            // `none`, which the evaluator holds as the word itself
            if matches!((&ty, v), (Ty::Opt(_), Value::None)) {
                vals.insert(name.clone(), Val::Enum(crate::kw::NONE.into()));
                continue;
            }
            let val = to_val(c, name, &ty, v).map_err(RuleError::Input)?;
            if let (Val::Num(x), Some((lo, hi))) = (&val, c.ranges.get(name)) {
                let below = lo.is_some_and(|lo| x.cmp_to(lo) == std::cmp::Ordering::Less);
                let above = hi.is_some_and(|hi| x.cmp_to(hi) == std::cmp::Ordering::Greater);
                if below || above {
                    return Err(RuleError::Input(ritsu_base::tr!("入力 `{name}` が範囲の外です", "the input `{name}` is outside its range")));
                }
            }
            vals.insert(name.clone(), val);
        }
        if let Some(el) = &f.elements {
            let name = &el.name.text;
            let Some((_, Value::List(xs))) = inputs.iter().find(|(n, _)| n == name) else {
                return Err(RuleError::Input(ritsu_base::tr!("並び `{name}` がありません", "the list `{name}` is missing")));
            };
            let mut seq = Vec::new();
            for x in xs {
                let mut one = std::collections::BTreeMap::new();
                for fd in &el.fields {
                    let fname = &fd.name.text;
                    let ty = c.ty_of(fname).unwrap_or(Ty::Unknown);
                    let v = x.iter().find(|(n, _)| n == fname).map(|(_, v)| v).ok_or_else(|| RuleError::Input(ritsu_base::tr!("要素のフィールド `{fname}` がありません", "an element's field `{fname}` is missing")))?;
                    if matches!((&ty, v), (Ty::Opt(_), Value::None)) {
                        one.insert(fname.clone(), Val::Enum(crate::kw::NONE.into()));
                        continue;
                    }
                    one.insert(fname.clone(), to_val(c, fname, &ty, v).map_err(RuleError::Input)?);
                }
                seq.push(one);
            }
            vals.insert(name.clone(), Val::Seq(seq));
        }
        for k in &f.constraints {
            if let (Some(Val::Num(l)), Some(Val::Num(r))) = (vals.get(&k.left), vals.get(&k.right)) {
                if !holds(*l, k.op.word(), *r) {
                    return Err(RuleError::Input(ritsu_base::tr!(
                        "`{} {} {}` が成り立ちません",
                        "`{} {} {}` does not hold",
                        k.left,
                        k.op.word(),
                        k.right
                    )));
                }
            }
        }
        let (outs, _, _) = crate::eval::run_all(f, c, vals);
        outs.into_iter()
            .map(|(name, v)| match v {
                Some(v) => Ok((name.clone(), from_val(c, &name, &v))),
                None => Err(RuleError::Contradiction(ritsu_base::tr!("出力 `{name}` を決める行がありません", "no row decides the output `{name}`"))),
            })
            .collect()
    }

    fn doc(&self, rule: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>> {
        let (path, src) = read(rule)?;
        self.joined(|| i18n::with(rlang(lang), || {
            let (f, c) = checked(&path, &src)?;
            Ok(if html {
                let js = crate::codegen::Gen::new(&f, &c, &src, &path).javascript();
                crate::doc::render_html_named(&f, &c, &src, &path, shown, &js)
            } else {
                crate::doc::render_named(&f, &c, &src, &path, shown)
            })
        }))
    }
}

/// The last line of each thing a rule writes, past its first: a table's last row, a machine's
/// last line, a source's last pin.
fn table_end(t: &crate::ast::Table) -> usize {
    let mut end = t.span.line;
    for r in &t.rows {
        end = end.max(r.span.line);
        end = r.out_spans.iter().chain(&r.cell_spans).map(|s| s.line).fold(end, usize::max);
    }
    t.overrides.iter().map(|o| o.span.line).fold(end, usize::max)
}

fn machine_end(m: &crate::ast::MachineDecl) -> usize {
    let mut end = m.span.line;
    for l in [m.carry.as_ref().map(|c| c.2.line), m.initial.as_ref().map(|i| i.1.line), m.final_span.as_ref().map(|s| s.line), m.held_span.as_ref().map(|s| s.line)].into_iter().flatten() {
        end = end.max(l);
    }
    end = m.nevers.iter().map(|n| n.span.line).fold(end, usize::max);
    m.onces.iter().map(|o| o.span.line).fold(end, usize::max)
}

/// The path of a file a rule names, from the root: written from the rule's directory.
fn from_root(file: &str, written: &str) -> Option<String> {
    ritsu_base::paths::join(&ritsu_base::paths::parent(file), written).ok()
}

impl ritsu_ports::Sources for Engine {
    /// The sources the rule file declares itself (not one an `apply` brings in: the other file
    /// declares it, and its copies sit beside that file), each with its pins, when the rule
    /// passes the whole of check, which holds every copy to its pin (§15.68).
    fn sources(&self, file: &Path) -> Result<Vec<ritsu_ports::Source>, Vec<Said>> {
        let (_, _, got) = self.rule(file)?;
        let (f, _) = got.as_ref().as_ref().map_err(|e| e.clone())?;
        Ok(f.sources
            .iter()
            .filter(|s| s.base.is_none())
            .map(|s| ritsu_ports::Source {
                name: s.name.text.clone(),
                line: s.span.line,
                kind: match &s.kind {
                    SourceKind::Law { db, id, asof } => ritsu_ports::SourceKind::Law {
                        db: db.word().to_string(),
                        id: id.clone(),
                        asof: asof.clone(),
                        pins: s.pins.iter().map(|p| (p.fragment.clone(), p.hash.clone())).collect(),
                    },
                    SourceKind::File { path, url, hash } => ritsu_ports::SourceKind::File { path: path.clone(), url: url.clone(), pin: hash.clone() },
                },
            })
            .collect())
    }
}

impl ritsu_ports::Items for Engine {
    /// Each input, output, enum (and each of its values), table, clause, define, derive,
    /// machine and source the file writes itself (what an `apply` brings in is the other file's).
    /// The definition of each is its lines as `rulec fmt` writes them: the columns aligned, the
    /// symbols in ASCII, so that realigning a table changes nothing.
    fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        let (path, src) = read(&ritsu_base::paths::on_disk(root, file))?;
        let parsed = crate::parse::parse(&src, &path);
        let Some(f) = parsed.file else {
            return Err(refused(&path, &src));
        };
        let formatted: Vec<String> = crate::fmt::format(&src).lines().map(String::from).collect();
        let text = |from: usize, to: usize| -> String { formatted.get(from.saturating_sub(1)..to.min(formatted.len())).map(|ls| ls.join("\n")).unwrap_or_default() };
        let naming = |kind: &str, name: &str| Naming::file(Tool::Rulec, file).with(kind, name);
        let mut out = Vec::new();
        let one = |naming: Naming, from: usize, to: usize, out: &mut Vec<Item>| out.push(Item { naming, lines: (from, to), text: text(from, to) });
        for i in &f.inputs {
            one(naming("input", &i.name.text), i.span.line, i.span.line, &mut out);
        }
        for o in &f.outputs {
            one(naming("output", &o.name.text), o.span.line, o.span.line, &mut out);
        }
        for e in &f.enums {
            let line = e.span.line;
            out.push(Item { naming: naming("enum", &e.name.text), lines: (line, line), text: text(line, line) });
            for v in &e.values {
                let written = match &v.ascii {
                    Some(a) => format!("{}({a})", v.text),
                    None => v.text.clone(),
                };
                out.push(Item { naming: naming("enum", &e.name.text).with("value", &v.text), lines: (line, line), text: written });
            }
        }
        for it in &f.items {
            match it {
                AstItem::Table(t) => {
                    let Some(n) = &t.name else { continue };
                    if t.applied.is_some() {
                        continue;
                    }
                    one(naming(if t.clause { "clause" } else { "table" }, &n.text), t.span.line, table_end(t), &mut out);
                }
                AstItem::Define(d) => one(naming("define", &d.name.text), d.span.line, d.span.line, &mut out),
                AstItem::Derived(d) => one(naming("derive", &d.name.text), d.span.line, d.span.line, &mut out),
                AstItem::Agg(_) => {}
            }
        }
        if let Some(m) = &f.machine {
            one(naming("machine", &m.name.text), m.span.line, machine_end(m), &mut out);
        }
        for s in f.sources.iter().filter(|s| s.base.is_none()) {
            let end = s.pins.iter().map(|p| p.span.line).fold(s.span.line, usize::max);
            one(naming("source", &s.name.text), s.span.line, end, &mut out);
        }
        out.sort_by_key(|i| i.lines.0);
        Ok(out)
    }
}

impl ritsu_ports::References for Engine {
    /// The files a rule names: the `.proto` or JSON Schema an enum's values come from
    /// (`import proto`, `import jsonschema`), the contract a `shape` reads its inputs out of, a
    /// rule it applies (`apply`), a document a `source` copies (`source … file`), and the koyomi
    /// date an input takes its days from (`range from koyomi`, §15.174). An enum or a message of
    /// a `.proto` is named from the file's package.
    fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>> {
        let disk = ritsu_base::paths::on_disk(root, file);
        let (path, src) = read(&disk)?;
        let parsed = crate::parse::parse(&src, &path);
        let Some(f) = parsed.file else {
            return Err(refused(&path, &src));
        };
        // a `.proto`'s name for a message or an enum, from its package: `shop.v1.Order` is `Order`
        let local = |proto: &str, written: &str| -> String {
            let package = ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(root, proto)).ok().and_then(|s| crate::proto::read(proto, &s).ok()).and_then(|p| p.package);
            match package {
                Some(p) => written.strip_prefix(&format!("{p}.")).unwrap_or(written).to_string(),
                None => written.to_string(),
            }
        };
        let mut out = Vec::new();
        for im in &f.enum_imports {
            let Some(p) = from_root(file, &im.file) else { continue };
            let (target, how) = match im.kind {
                crate::ast::EnumSource::Proto => (Naming::file(Tool::Proto, &p).with("enum", local(&p, &im.source)), "import proto"),
                crate::ast::EnumSource::JsonSchema => (Naming::file(Tool::File, &p), "import jsonschema"),
            };
            out.push(Reference { line: im.span.line, target, how: how.into() });
        }
        for sh in &f.shapes {
            let Some(p) = from_root(file, &sh.file) else { continue };
            let target = match sh.source {
                crate::ast::EnumSource::Proto => Naming::file(Tool::Proto, &p).with("message", local(&p, &sh.at)),
                crate::ast::EnumSource::JsonSchema => Naming::file(Tool::File, &p),
            };
            out.push(Reference { line: sh.span.line, target, how: "shape".into() });
        }
        for a in &f.applies {
            if let Some(p) = from_root(file, &a.path) {
                out.push(Reference { line: a.span.line, target: Naming::file(Tool::Rulec, &p), how: "apply".into() });
            }
        }
        for s in f.sources.iter().filter(|s| s.base.is_none()) {
            if let SourceKind::File { path: written, .. } = &s.kind
                && let Some(p) = from_root(file, written)
            {
                out.push(Reference { line: s.span.line, target: Naming::file(Tool::File, &p), how: "source".into() });
            }
        }
        for i in &f.inputs {
            if let Some(d) = i.range.as_ref().and_then(|r| r.days.as_ref())
                && let Some(p) = from_root(file, &d.file)
            {
                out.push(Reference { line: d.span.line, target: Naming::file(Tool::Koyomi, &p).with("date", &d.date), how: "range from koyomi".into() });
            }
        }
        out.sort_by_key(|r| r.line);
        Ok(out)
    }
}
