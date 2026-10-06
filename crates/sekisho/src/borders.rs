//! The borders with the other languages (DESIGN 3.2, 3.3, 8.1): what a `.gate` reads of rulec,
//! koyomi and dandori, checked through their ports.
//!
//! - E201: a calendar or a book the file reads does not pass koyomi's or chobo's check, or cannot
//!   be read (the rules and the dates files are `names.rs`'s, which asks their facts first).
//! - E101, E102, E105 of what only the other language knows (a rule's output or input, a date of
//!   a dates file and its inputs): `names.rs` says them, and a file whose names hold has none. They
//!   are said here too, for a reading of the facts that would differ from the names', rather than
//!   leave the computed value out of the walk without a word.
//! - E206: a value given to a rule or a date can be outside the input's range, or can break a
//!   precondition of the rule (rulec answers, with an example); W201 when rulec cannot decide.
//! - E207: a calendar `today is open in` reads does not cover every day of `today`.
//! - E208: a workflow's `.flow` does not pass dandori's check, or cannot be read.
//!
//! What the languages answer is gathered as [`Known`], what the walk reads of each computed value.

use crate::diag::Diag;
use crate::model::*;
use crate::walk::{Domain, Kn, Known};
use ritsu_base::text::Text;
use ritsu_ports::{Answer, ColumnType, DateKind, Dates, Flows, Ports, Precondition, RuleFacts, Rules, Said, Value};
use std::collections::BTreeMap;
use std::rc::Rc;

/// The languages the borders are checked through; None for one that is not joined (the binary of
/// sekisho's own crate, which says so itself, E209).
#[derive(Clone, Default)]
pub struct Langs<'a> {
    pub rules: Option<&'a dyn Rules>,
    pub dates: Option<&'a dyn Dates>,
    pub books: Option<&'a dyn ritsu_ports::Books>,
    /// dandori, which says whether a `.flow` passes its check, with the ports it reads a flow with.
    pub flows: Option<(&'a dyn Flows, Ports)>,
}

fn at(code: &'static str, g: &Gate, line: usize, message: Text) -> Diag {
    Diag::at(code, &g.file, line, 1, message).source(&g.src)
}

/// What a language said of a file, one note a finding.
fn said_notes(mut d: Diag, said: &[Said]) -> Diag {
    for s in said {
        let place = match s.line {
            Some(l) => format!("{}:{l}", s.file),
            None => s.file.clone(),
        };
        let code = if s.code.is_empty() { String::new() } else { format!("[{}] ", s.code) };
        d = d.note(Text { ja: format!("{code}{place}: {}", s.message.ja), en: format!("{code}{place}: {}", s.message.en) });
    }
    d
}

/// What a value given to a rule or a date is, in this file: its kind, its unit and its range (on the
/// wire; days for a date).
#[derive(Clone, Debug, PartialEq)]
enum Given {
    Num { unit: Option<ritsu_units::Unit>, lo: i128, hi: i128, shown: String },
    Bool,
    Enum(usize),
    Date { lo: i64, hi: i64 },
    /// A word, which only an enum of the rule can make a value of.
    Word(String),
    /// What the file has no type for (an attribute none of the types has; `names.rs` says why).
    Unknown,
}

/// The types an attribute of an owner has, over the types of the action that the owner can be.
fn attr_given(g: &Gate, a: &Action, o: Owner, name: &str) -> Given {
    let types = match o {
        Owner::Principal => &a.principals,
        Owner::Resource => &a.resources,
    };
    let mut found: Option<Given> = None;
    for &t in types {
        if let Some((_, f)) = g.types[t].attr(name) {
            let gv = field_given(f, &format!("{}.{name}", o.word()));
            match &found {
                None => found = Some(gv),
                // two types with the attribute: the range that covers both
                Some(Given::Num { unit, lo, hi, shown }) => {
                    if let Given::Num { lo: l2, hi: h2, .. } = gv {
                        found = Some(Given::Num { unit: unit.clone(), lo: (*lo).min(l2), hi: (*hi).max(h2), shown: shown.clone() });
                    }
                }
                Some(Given::Date { lo, hi }) => {
                    if let Given::Date { lo: l2, hi: h2 } = gv {
                        found = Some(Given::Date { lo: (*lo).min(l2), hi: (*hi).max(h2) });
                    }
                }
                Some(_) => {}
            }
        }
    }
    found.unwrap_or(Given::Unknown)
}

fn field_given(f: &Field, shown: &str) -> Given {
    match &f.ty {
        FieldType::Num { unit, lo, hi, .. } => Given::Num { unit: Some(unit.clone()), lo: *lo, hi: *hi, shown: shown.to_string() },
        FieldType::Bool => Given::Bool,
        FieldType::Enum(e) => Given::Enum(*e),
        FieldType::Date { lo, hi } => Given::Date { lo: *lo, hi: *hi },
        FieldType::Entity(_) => Given::Unknown,
    }
}

fn given(g: &Gate, a: &Action, src: &Source) -> Given {
    match src {
        Source::Attr(o, n) => attr_given(g, a, *o, n),
        Source::Input(n) => a.input(n).map(|(_, f)| field_given(f, n)).unwrap_or(Given::Unknown),
        Source::Lit(Literal::Num(n)) => Given::Num { unit: None, lo: 0, hi: 0, shown: n.raw.clone() },
        Source::Lit(Literal::Bool(_)) => Given::Bool,
        Source::Lit(Literal::Date(d)) => Given::Date { lo: *d, hi: *d },
        Source::Lit(Literal::Word(w)) => Given::Word(w.clone()),
        Source::Today => match &g.today {
            Some(t) => Given::Date { lo: t.lo, hi: t.hi },
            None => Given::Unknown,
        },
    }
}

fn source_text(src: &Source) -> String {
    match src {
        Source::Attr(o, n) => format!("{}.{n}", o.word()),
        Source::Input(n) => n.clone(),
        Source::Lit(Literal::Num(n)) => n.raw.clone(),
        Source::Lit(Literal::Bool(b)) => b.to_string(),
        Source::Lit(Literal::Date(d)) => crate::types::day_text(*d),
        Source::Lit(Literal::Word(w)) => w.clone(),
        Source::Today => "today".into(),
    }
}

fn show_num(v: i128, unit: Option<&ritsu_units::Unit>) -> String {
    match unit {
        Some(u) => crate::cells::show(v, u),
        None => v.to_string(),
    }
}

/// The checks of the borders, and what the languages answered.
pub fn check(g: &Gate, langs: &Langs) -> (Vec<Diag>, Known) {
    let mut diags = Vec::new();
    let mut known = Known::default();
    // each file read once: its facts, or what its language said
    let mut rules: BTreeMap<usize, Option<RuleFacts>> = BTreeMap::new();
    let mut dates: BTreeMap<usize, Option<ritsu_ports::DateFacts>> = BTreeMap::new();
    let mut cals: BTreeMap<usize, Option<ritsu_ports::CalendarFacts>> = BTreeMap::new();
    for (ui, u) in g.uses.iter().enumerate() {
        let unreadable = |said: Vec<Said>, lang: &str| {
            let d = at("E201", g, u.line, tr!("`{}` は {lang} の検査を通らないか、読めません", "`{}` does not pass {lang}'s check, or cannot be read", u.path));
            said_notes(d, &said).note(tr!("そのファイルを {lang} の検査が通るように直してください。", "Fix the file until {lang}'s check passes."))
        };
        match u.kind {
            UseKind::Rule => {
                let Some(r) = langs.rules.filter(|r| r.joined()) else { continue };
                match r.facts(&u.file) {
                    Ok(f) => {
                        rules.insert(ui, Some(f));
                    }
                    Err(said) => {
                        diags.push(unreadable(said, "rulec"));
                        rules.insert(ui, None);
                    }
                }
            }
            UseKind::Dates => {
                let Some(d) = langs.dates.filter(|d| d.joined()) else { continue };
                match d.facts(&u.file) {
                    Ok(f) => {
                        dates.insert(ui, Some(f));
                    }
                    Err(said) => {
                        diags.push(unreadable(said, "koyomi"));
                        dates.insert(ui, None);
                    }
                }
            }
            UseKind::Calendar => {
                let Some(d) = langs.dates.filter(|d| d.joined()) else { continue };
                match d.calendar(&u.file) {
                    Ok(f) => {
                        cals.insert(ui, Some(f));
                    }
                    Err(said) => {
                        diags.push(unreadable(said, "koyomi"));
                        cals.insert(ui, None);
                    }
                }
            }
            UseKind::Book => {
                let Some(b) = langs.books.filter(|b| b.joined()) else { continue };
                if let Err(said) = b.facts(&u.file) {
                    diags.push(unreadable(said, "chobo"));
                }
            }
            UseKind::OpenApi | UseKind::Proto | UseKind::AsyncApi | UseKind::Gate => {}
        }
    }

    for (ai, a) in g.actions.iter().enumerate() {
        for (ci, c) in a.computed.iter().enumerate() {
            match &c.how {
                How::Rule { rule, args, output } => {
                    let Some(Some(facts)) = rules.get(rule) else { continue };
                    if let Some(kn) = rule_value(g, a, c, *rule, facts, args, output, langs, &mut diags) {
                        known.computed.insert((ai, ci), kn);
                    }
                }
                How::Date { of: DateOf::Call { dates: du, function, args }, .. } => {
                    let Some(Some(facts)) = dates.get(du) else { continue };
                    if let Some(kn) = date_value(g, a, c, *du, facts, function, args, &mut diags) {
                        known.computed.insert((ai, ci), kn);
                    }
                }
                How::Date { of: DateOf::Attr(o, n), .. } => match attr_given(g, a, *o, n) {
                    Given::Date { .. } => {
                        known.computed.insert((ai, ci), Kn::Attr);
                    }
                    Given::Unknown => {}
                    _ => diags.push(at("E102", g, c.named.line, tr!("`{}.{n}` は日付ではないので、today と比べられません", "`{}.{n}` is not a date, and cannot be compared with today", o.word(); o.word()))),
                },
                How::Open { calendar } => {
                    let Some(Some(cal)) = cals.get(calendar) else { continue };
                    if let Some(t) = &g.today
                        && (cal.data.0 > t.lo || cal.data.1 < t.hi)
                    {
                        let (dl, dh) = (crate::types::day_text(cal.data.0), crate::types::day_text(cal.data.1));
                        let (tl, th) = (crate::types::day_text(t.lo), crate::types::day_text(t.hi));
                        let u = &g.uses[*calendar];
                        diags.push(
                            at("E207", g, c.named.line, tr!("カレンダー `{}` のデータは {dl}〜{dh} の日しか知らず、today の {tl}〜{th} を覆いません", "The calendar `{}` knows the days {dl}..{dh} only, which do not cover today's {tl}..{th}", u.path))
                                .note(tr!(
                                    "today の範囲をデータの範囲の中に狭めるか、新しいデータが出てからカレンダーのコピーを取り直してください。",
                                    "Narrow today's range to the data's, or take the calendar's copy again when newer data is out."
                                )),
                        );
                        continue;
                    }
                    known.computed.insert((ai, ci), Kn::Open { data: cal.data, closed: Rc::new(cal.closed.clone()) });
                }
            }
        }
    }

    // the workflows: each `.flow` passes dandori's check
    if let Some((flows, ports)) = &langs.flows {
        for w in &g.workflows {
            if let Err(said) = flows.crossings(&w.file, ports) {
                let d = at("E208", g, w.named.line, tr!("`{}` は dandori の検査を通らないか、読めません", "`{}` does not pass dandori's check, or cannot be read", w.flow));
                diags.push(said_notes(d, &said).note(tr!("ワークフローを dandori の検査が通るように直してください。", "Fix the workflow until dandori's check passes.")));
            }
        }
    }
    (diags, known)
}

/// The column of a rule's input or output that `word` names: by its name or its alias.
fn column<'a>(cols: &'a [ritsu_ports::Column], word: &str) -> Option<&'a ritsu_ports::Column> {
    cols.iter().find(|c| c.name == word || c.alias == word)
}

#[allow(clippy::too_many_arguments)]
fn rule_value(g: &Gate, a: &Action, c: &Computed, rule: usize, facts: &RuleFacts, args: &[(String, Source)], output: &str, langs: &Langs, diags: &mut Vec<Diag>) -> Option<Kn> {
    let u = &g.uses[rule];
    let line = c.named.line;
    let before = diags.len();
    // the output: an enum or a bool of the rule
    let domain = match column(&facts.outputs, output) {
        None => {
            let outs: Vec<Text> = facts.outputs.iter().map(|o| Text::same(o.name.clone())).collect();
            let l = Text::list(&outs);
            diags.push(at("E101", g, line, tr!("規則 `{}` に出力 `{output}` はありません", "The rule `{}` has no output `{output}`", facts.rule; facts.rule)).note(tr!("出力は {} です。", "Its outputs are {}.", l.ja; l.en)));
            None
        }
        Some(col) => match &col.ty {
            ColumnType::Bool => Some(Domain::Bool),
            // each value by the rule's name of it, and by its public name: the alias the `.rule` writes
            // in parentheses (`上限まで(within_limit)`), what Cedar is given and what rulec answers
            // `outputs_over` with
            ColumnType::Enum(e) => facts.enums.iter().find(|x| x.name == *e).map(|x| Domain::Enum(x.values.iter().map(|v| Named::new(&v.name, &v.public, 0)).collect())),
            ColumnType::Opt(inner) if matches!(**inner, ColumnType::Bool | ColumnType::Enum(_)) => {
                diags.push(at("E105", g, line, tr!("規則 `{}` の出力 `{output}` は値が無いことがあり、条件に使えません", "The output `{output}` of the rule `{}` may have no value, and a condition cannot read it", facts.rule; facts.rule)));
                None
            }
            _ => {
                diags.push(at("E105", g, line, tr!("規則 `{}` の出力 `{output}` は列挙でも真偽でもないので、計算した値にできません", "The output `{output}` of the rule `{}` is neither an enum nor a bool, so it cannot be a computed value", facts.rule; facts.rule)).note(tr!(
                    "列挙か真偽の出力を使ってください。数を区間に分ける出力は、まだ書けません。",
                    "Use an output that is an enum or a bool; a numeric output cannot be used yet."
                )));
                None
            }
        },
    };
    // the inputs: each given, each of the type and unit, each within the range
    let mut ranges: Vec<(String, Option<i128>, Option<i128>)> = Vec::new();
    let mut inputs: Vec<(String, Option<ritsu_units::Unit>)> = Vec::new();
    for (name, src) in args {
        let Some(col) = column(&facts.inputs, name) else {
            let ins: Vec<Text> = facts.inputs.iter().map(|i| Text::same(i.name.clone())).collect();
            let l = Text::list(&ins);
            diags.push(at("E101", g, line, tr!("規則 `{}` に入力 `{name}` はありません", "The rule `{}` has no input `{name}`", facts.rule; facts.rule)).note(tr!("入力は {} です。", "Its inputs are {}.", l.ja; l.en)));
            continue;
        };
        let (ty, optional) = match &col.ty {
            ColumnType::Opt(inner) => (inner.as_ref().clone(), true),
            t => (t.clone(), false),
        };
        let _ = optional;
        let gv = given(g, a, src);
        let shown = source_text(src);
        let mismatch = |what: Text| at("E102", g, line, tr!("規則 `{}` の入力 `{name}` に `{shown}` は渡せません。{}", "`{shown}` cannot be given to the input `{name}` of the rule `{}`: {}", facts.rule, what.ja; facts.rule, what.en));
        match (&ty, &gv) {
            (_, Given::Unknown) => {}
            (ColumnType::Num { unit: Some(want), min, max, written }, Given::Num { unit: Some(have), lo, hi, .. }) => {
                if !want.same(have) {
                    diags.push(mismatch(tr!("単位が {have} で、入力は {written} です", "it is {have}, and the input is {written}")));
                    continue;
                }
                inputs.push((col.name.clone(), Some(want.clone())));
                ranges.push((col.name.clone(), Some(*lo), Some(*hi)));
                outside(g, line, &facts.rule, name, &shown, (*lo, *hi), (*min, *max), Some(want), diags);
            }
            (ColumnType::Num { unit: Some(want), min, max, .. }, Given::Num { unit: None, .. }) => {
                let Source::Lit(Literal::Num(n)) = src else { continue };
                match crate::types::count(n, want) {
                    Ok(k) => {
                        inputs.push((col.name.clone(), Some(want.clone())));
                        ranges.push((col.name.clone(), Some(k), Some(k)));
                        outside(g, line, &facts.rule, name, &shown, (k, k), (*min, *max), Some(want), diags);
                    }
                    Err(m) => diags.push(at(m.code(), g, line, tr!("`{shown}` は規則 `{}` の入力 `{name}`（{want}）の値になりません", "`{shown}` is not a value of the input `{name}` of the rule `{}` ({want})", facts.rule; facts.rule))),
                }
            }
            (ColumnType::Num { unit: None, min, max, .. }, Given::Num { lo, hi, unit, .. }) => {
                inputs.push((col.name.clone(), unit.clone()));
                ranges.push((col.name.clone(), Some(*lo), Some(*hi)));
                outside(g, line, &facts.rule, name, &shown, (*lo, *hi), (*min, *max), unit.as_ref(), diags);
            }
            (ColumnType::Bool, Given::Bool) => inputs.push((col.name.clone(), None)),
            (ColumnType::Date, Given::Date { lo, hi }) => {
                inputs.push((col.name.clone(), None));
                // rulec takes the range of a date as day numbers
                ranges.push((col.name.clone(), Some(*lo as i128), Some(*hi as i128)));
                if let Some(r) = langs.rules
                    && let Ok((min, max)) = r.date_range(&u.file, &col.name)
                    && (min.is_some_and(|m| *lo < m) || max.is_some_and(|m| *hi > m))
                {
                    let (x, y) = (min.map(crate::types::day_text).unwrap_or_default(), max.map(crate::types::day_text).unwrap_or_default());
                    diags.push(at("E206", g, line, tr!("`{shown}` は規則 `{}` の入力 `{name}` の範囲（{x}〜{y}）の外になりえます", "`{shown}` can be outside the range of the input `{name}` of the rule `{}` ({x}..{y})", facts.rule; facts.rule)));
                }
            }
            (ColumnType::Enum(e), Given::Enum(ge)) => {
                let rv: Vec<String> = facts.enums.iter().find(|x| x.name == *e).map(|x| x.values.iter().flat_map(|v| [v.name.clone(), v.alias.clone(), v.public.clone()]).collect()).unwrap_or_default();
                let missing: Vec<Text> = g.enums[*ge].values.iter().filter(|v| !rv.contains(&v.name) && !rv.contains(&v.alias)).map(|v| Text::same(v.name.clone())).collect();
                if missing.is_empty() {
                    inputs.push((col.name.clone(), None));
                } else {
                    let l = Text::list(&missing);
                    diags.push(mismatch(tr!("{} は規則の列挙 `{e}` の値にありません", "{} are not values of the rule's enum `{e}`", l.ja; l.en)));
                }
            }
            (ColumnType::Enum(e), Given::Word(w)) => {
                let ok = facts.enums.iter().find(|x| x.name == *e).is_some_and(|x| x.values.iter().any(|v| v.name == *w || v.alias == *w || v.public == *w));
                if ok {
                    inputs.push((col.name.clone(), None));
                } else {
                    diags.push(at("E101", g, line, tr!("`{w}` は規則 `{}` の列挙 `{e}` の値ではありません", "`{w}` is not a value of the enum `{e}` of the rule `{}`", facts.rule; facts.rule)));
                }
            }
            (want, _) => diags.push(mismatch(tr!("型が違います（入力は {}）", "it is not of the input's type ({})", column_type(want); column_type(want)))),
        }
    }
    // every input given
    let missing: Vec<Text> = facts.inputs.iter().filter(|i| !args.iter().any(|(n, _)| *n == i.name || *n == i.alias)).map(|i| Text::same(i.name.clone())).collect();
    if !missing.is_empty() {
        let l = Text::list(&missing);
        diags.push(at("E105", g, line, tr!("規則 `{}` の入力 {} が渡されていません", "The rule `{}` is not given its input {}", facts.rule, l.ja; facts.rule, l.en)).note(tr!(
            "どの入力にも、principal か resource の属性、input、定数のどれかを渡してください。",
            "Give every input an attribute of the principal or the resource, an input, or a constant."
        )));
    }
    if diags.len() > before {
        return None;
    }
    // the preconditions of the rule, over the ranges given (a precondition that can break leaves
    // the value out of the walk, as a range that can be crossed does)
    let mut broken = false;
    if let Some(r) = langs.rules
        && let Ok(answers) = r.preconditions_hold(&u.file, &ranges, None)
    {
        for (p, ans) in answers {
            match ans {
                Answer::Holds => {}
                Answer::Fails(vals) => {
                    broken = true;
                    let ex = values_text(&vals, facts);
                    diags.push(at("E206", g, line, tr!("規則 `{}` に渡す値が、前提 {} を破りえます", "The values given to the rule `{}` can break its precondition {}", facts.rule, precondition_text(&p); facts.rule, precondition_text(&p))).note(tr!("例：{}。", "For example: {}.", ex.ja; ex.en)).note(tr!(
                        "渡す属性か input の範囲を、前提を守る範囲に狭めてください。",
                        "Narrow the ranges of what is given to ones that keep the precondition."
                    )));
                }
                Answer::Undecided(why) => {
                    diags.push(at("W201", g, line, tr!("規則 `{}` の前提 {} を守るかを決められません", "Cannot decide whether the values given to the rule `{}` keep its precondition {}", facts.rule, precondition_text(&p); facts.rule, precondition_text(&p))).note(why).note(tr!(
                        "生成したコードは、走らせたときに前提を確かめ、破れば拒みます。",
                        "The generated code checks it when it runs, and denies the request when it breaks."
                    )));
                }
            }
        }
    }
    if broken {
        return None;
    }
    // the values of each enum input, as rulec's evaluator takes them, with the words that name them
    let enums = facts
        .inputs
        .iter()
        .filter_map(|i| {
            let e = match &i.ty {
                ColumnType::Enum(e) => e,
                ColumnType::Opt(x) => match x.as_ref() {
                    ColumnType::Enum(e) => e,
                    _ => return None,
                },
                _ => return None,
            };
            let values = facts.enums.iter().find(|x| x.name == *e)?.values.iter().map(|v| (v.name.clone(), vec![v.name.clone(), v.alias.clone(), v.public.clone()])).collect();
            Some((i.name.clone(), values))
        })
        .collect();
    domain.map(|domain| Kn::Rule { file: u.file.clone(), output: column(&facts.outputs, output).map(|c| c.name.clone()).unwrap_or_default(), domain, inputs, enums })
}

fn column_type(t: &ColumnType) -> String {
    match t {
        ColumnType::Bool => "bool".into(),
        ColumnType::Str => "string".into(),
        ColumnType::Date => "date".into(),
        ColumnType::Enum(e) => e.clone(),
        ColumnType::Num { written, .. } => written.clone(),
        ColumnType::Opt(x) => format!("{}?", column_type(x)),
    }
}

fn precondition_text(p: &Precondition) -> String {
    match p {
        Precondition::Relation { left, op, right } => format!("`{left} {op} {right}`"),
        Precondition::Sum { name, over, of, max } => format!("`{name}` ({of} over {over} <= {max})"),
        Precondition::Length { sequence, max } => format!("`{sequence}` (<= {max})"),
        Precondition::Days { input, date, .. } => format!("`{input}` ({date})"),
    }
}

/// An input as rulec gives it, `low = 100, high = 0` (`、` between them in Japanese); a date, which
/// rulec gives as its day number, as the day.
fn values_text(vals: &[(String, Value)], facts: &RuleFacts) -> Text {
    let is_date = |n: &str| column(&facts.inputs, n).is_some_and(|c| matches!(&c.ty, ColumnType::Date) || matches!(&c.ty, ColumnType::Opt(x) if **x == ColumnType::Date));
    let parts: Vec<String> = vals
        .iter()
        .map(|(n, v)| {
            let v = match v {
                Value::Bool(b) => b.to_string(),
                Value::Int(i) if is_date(n) => crate::types::day_text(*i as i64),
                Value::Int(i) => i.to_string(),
                Value::Str(s) | Value::Date(s) | Value::Enum(s) => s.clone(),
                Value::List(_) => "[…]".into(),
                Value::None => "none".into(),
            };
            format!("{n} = {v}")
        })
        .collect();
    Text { ja: parts.join("、"), en: parts.join(", ") }
}

/// E206 when a range given can be outside the range of the input.
#[allow(clippy::too_many_arguments)]
fn outside(g: &Gate, line: usize, rule: &str, input: &str, shown: &str, have: (i128, i128), want: (Option<i128>, Option<i128>), unit: Option<&ritsu_units::Unit>, diags: &mut Vec<Diag>) {
    let low = want.0.filter(|m| have.0 < *m).map(|_| have.0);
    let high = want.1.filter(|m| have.1 > *m).map(|_| have.1);
    let Some(v) = low.or(high) else { return };
    let (x, y) = (want.0.map(|m| show_num(m, unit)).unwrap_or_default(), want.1.map(|m| show_num(m, unit)).unwrap_or_default());
    let ex = show_num(v, unit);
    diags.push(
        at("E206", g, line, tr!("`{shown}` は規則 `{rule}` の入力 `{input}` の範囲（{x}〜{y}）の外になりえます", "`{shown}` can be outside the range of the input `{input}` of the rule `{rule}` ({x}..{y})"))
            .note(tr!("例：`{shown}` が {ex} のとき。", "For example: `{shown}` at {ex}."))
            .note(tr!("渡す値の範囲を、入力の範囲の中に狭めてください。", "Narrow the range of what is given to the input's.")),
    );
}

#[allow(clippy::too_many_arguments)]
fn date_value(g: &Gate, a: &Action, c: &Computed, du: usize, facts: &ritsu_ports::DateFacts, function: &str, args: &[(String, Source)], diags: &mut Vec<Diag>) -> Option<Kn> {
    let u = &g.uses[du];
    let line = c.named.line;
    let before = diags.len();
    let Some(func) = facts.functions.iter().find(|f| f.name == function || f.alias == function) else {
        let fs: Vec<Text> = facts.functions.iter().map(|f| Text::same(f.name.clone())).collect();
        let l = Text::list(&fs);
        diags.push(at("E101", g, line, tr!("日付のファイル `{}` に日付 `{function}` はありません", "The dates file `{}` has no date `{function}`", u.path; u.path)).note(tr!("日付は {} です。", "Its dates are {}.", l.ja; l.en)));
        return None;
    };
    let mut inputs: Vec<(String, bool)> = Vec::new();
    for (name, src) in args {
        let Some(inp) = facts.inputs.iter().find(|i| i.name == *name || i.alias == *name) else {
            let ins: Vec<Text> = func.params.iter().map(|p| Text::same(p.clone())).collect();
            let l = Text::list(&ins);
            diags.push(at("E101", g, line, tr!("日付 `{}` に入力 `{name}` はありません", "The date `{}` has no input `{name}`", func.name; func.name)).note(tr!("入力は {} です。", "Its inputs are {}.", l.ja; l.en)));
            continue;
        };
        let shown = source_text(src);
        let gv = given(g, a, src);
        match (inp.kind, &gv) {
            (_, Given::Unknown) => {}
            (DateKind::Date, Given::Date { lo, hi }) => {
                inputs.push((inp.name.clone(), true));
                if *lo < inp.min || *hi > inp.max {
                    let ex = if *lo < inp.min { *lo } else { *hi };
                    let (x, y) = (crate::types::day_text(inp.min), crate::types::day_text(inp.max));
                    diags.push(
                        at("E206", g, line, tr!("`{shown}` は日付 `{}` の入力 `{name}` の範囲（{x}〜{y}）の外になりえます", "`{shown}` can be outside the range of the input `{name}` of the date `{}` ({x}..{y})", func.name; func.name))
                            .note(tr!("例：`{shown}` が {} のとき。", "For example: `{shown}` at {}.", crate::types::day_text(ex)))
                            .note(tr!("渡す値の範囲を、入力の範囲の中に狭めてください。", "Narrow the range of what is given to the input's.")),
                    );
                }
            }
            (DateKind::Int, Given::Num { lo, hi, unit, .. }) => {
                inputs.push((inp.name.clone(), false));
                let (lo, hi) = match src {
                    Source::Lit(Literal::Num(n)) if n.value.is_int() => (n.value.num, n.value.num),
                    _ => (*lo, *hi),
                };
                if lo < inp.min as i128 || hi > inp.max as i128 {
                    let ex = if lo < inp.min as i128 { lo } else { hi };
                    diags.push(
                        at("E206", g, line, tr!("`{shown}` は日付 `{}` の入力 `{name}` の範囲（{}〜{}）の外になりえます", "`{shown}` can be outside the range of the input `{name}` of the date `{}` ({}..{})", func.name, inp.min, inp.max; func.name, inp.min, inp.max))
                            .note(tr!("例：`{shown}` が {} のとき。", "For example: `{shown}` at {}.", show_num(ex, unit.as_ref()); show_num(ex, unit.as_ref())))
                            .note(tr!("渡す値の範囲を、入力の範囲の中に狭めてください。", "Narrow the range of what is given to the input's.")),
                    );
                }
            }
            (kind, _) => {
                let want = if kind == DateKind::Date { "date" } else { "number" };
                diags.push(at("E102", g, line, tr!("日付 `{}` の入力 `{name}` に `{shown}` は渡せません。入力は {want} です", "`{shown}` cannot be given to the input `{name}` of the date `{}`: the input is a {want}", func.name; func.name)));
            }
        }
    }
    let missing: Vec<Text> = func.params.iter().filter(|p| !args.iter().any(|(n, _)| facts.inputs.iter().any(|i| (i.name == *n || i.alias == *n) && i.name == **p))).map(|p| Text::same(p.clone())).collect();
    if !missing.is_empty() {
        let l = Text::list(&missing);
        diags.push(at("E105", g, line, tr!("日付 `{}` の入力 {} が渡されていません", "The date `{}` is not given its input {}", func.name, l.ja; func.name, l.en)));
    }
    if diags.len() > before {
        return None;
    }
    Some(Kn::Date { file: u.file.clone(), function: func.name.clone(), inputs })
}
