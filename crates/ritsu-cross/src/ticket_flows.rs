//! What a ticket gives a workflow, held to what the workflow takes (ringi's DESIGN 9.3, X20). A
//! request starts a flow when a ticket ends (`on approved launch pay(…)`), asks one to answer a
//! stage (`agent <flow>, else …`) and asks one for a note of a stage (`note from <flow>`). The
//! request and the flow are written apart, and each passes its own check alone.
//!
//! ringi says what each flow is given (`Requests::ticket`): a `launch` gives what it writes; an
//! agent and a summary are given the ticket's ID (`request_id`) and every field of the form, of
//! which the flow takes those it declares. Each value is held to less where the flow is given it:
//! by the rows of the route that give the flow its stage, and by a `when` (of the `on` line, of the
//! stage) whose parts, joined with `and`, compare fields of the form with constants. dandori says
//! what the flow takes and answers (`Flows::declared`). Each value is a border:
//!
//! - everything the request can give is something the input takes: shown to hold.
//! - a value the input does not take (E210), with the value: a number or a day outside the input's
//!   range, a choice the input's enum does not have, another type, or no value at all where the
//!   input is not `T?`. An input a `launch` does not give, a value it gives that the flow has no
//!   input for, and an input of an agent's or a summary's flow that is neither `request_id` nor a
//!   field of the form, are E210 too.
//! - otherwise undecided (W210): a value ringi does not follow, a type that is not compared value
//!   by value, or a mismatch of values at a place whose condition ringi does not follow (a `when`
//!   with a part of another form, or one that reads a field the value is counted with), which may
//!   hold the value to less. Two types that differ are E210 whatever the condition.
//!
//! What an agent's flow answers is a border: ringi reads `verdict`, an enum whose values are `ok`
//! and `doubt`, and `reason`, a string (E210 for a flow that answers anything else). A summary's
//! flow answers at least one output, each of a type a note holds: text (a string, a date, a
//! timestamp, an enum), a whole number, a truth, rows (a list of records whose fields hold these),
//! or a `T?` of these (E210 for a record on its own, a list of what is not a record, `json`).
//!
//! A flow that does not pass dandori's check is dandori's to say, and a request that does not pass
//! is ringi's.

use crate::{Border, Borders, Came, Check, Side};
use ritsu_base::diag::Diag;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{Answer, Carried, Finding, FlowDeclared, FlowGiven, FlowPlace, Flows, Found, Ports, RequestTicket, Requests, Spelled, TicketFlow, day_text};
use ritsu_project::{Joined, Project};
use ritsu_units::{Dim, Unit, written};
use std::collections::BTreeMap;

/// The ends of a range of days as a line writes them: `>=2026-01-01 <=2026-12-31`.
fn ends<T: std::fmt::Display>(lo: Option<T>, hi: Option<T>) -> String {
    [lo.map(|v| format!(">={v}")), hi.map(|v| format!("<={v}"))].into_iter().flatten().collect::<Vec<_>>().join(" ")
}

fn with_ends(word: &str, range: String) -> String {
    if range.is_empty() { word.to_string() } else { format!("{word} {range}") }
}

/// What a value carries, for a message: the type, with its range as a sentence says it, each end in
/// its unit and its digits grouped (`money[JPY] >=1JPY <=500_000JPY`, ritsu's DESIGN 6.2 item 8),
/// as `fed by` says a place.
pub fn carried_text(c: &Carried) -> String {
    match c {
        Carried::Bool => "bool".into(),
        Carried::Str => "string".into(),
        Carried::Date { min, max } => with_ends("date", ends(min.map(day_text), max.map(day_text))),
        Carried::Days(set) => match (set.first(), set.last()) {
            (Some(a), Some(b)) => format!("date ({} days from {} to {})", set.len(), day_text(*a), day_text(*b)),
            _ => "date (no day)".into(),
        },
        Carried::Enum { name, values } => format!("enum {name} ({})", values.iter().map(|v| v.public.as_str()).collect::<Vec<_>>().join(", ")),
        Carried::Num { written: word, unit, min, max } => with_ends(word, written::between(*min, *max, unit.as_ref())),
        Carried::Opt(t) => format!("{}?", carried_text(t)),
        Carried::Other(w) => w.clone(),
    }
}

/// Whether two numbers are counted the same way, as `fed by` reads it: the same unit, step and tax;
/// a count with a name is a number; a spelling the table of units does not have is the same as
/// itself.
fn same_unit(a: (&str, Option<&Unit>), b: (&str, Option<&Unit>)) -> bool {
    match (a.1, b.1) {
        (Some(x), Some(y)) => match (&x.dim, &y.dim) {
            (Dim::Count(m), Dim::Count(n)) => m == n,
            (Dim::Count(_), Dim::Number) | (Dim::Number, Dim::Count(_)) => true,
            _ => x.same(y),
        },
        (None, None) => a.0 == b.0,
        _ => false,
    }
}

/// An example of what an input does not take.
#[derive(Clone, Debug, PartialEq)]
pub struct Against {
    /// The value, as a message shows it; None where no one value shows it.
    pub value: Option<String>,
    /// What is wrong, in words.
    pub why: Text,
    /// True where the types differ: nothing that holds the value to less changes it.
    pub whatever: bool,
}

impl Against {
    /// A value the input does not take.
    fn value(value: Option<String>, why: Text) -> Against {
        Against { value, why, whatever: false }
    }

    /// Two types that differ.
    fn types(why: Text) -> Against {
        Against { value: None, why, whatever: true }
    }
}

/// Whether a value that carries `given` is one an input that carries `takes` takes, whatever of
/// `given` comes. A `T?` takes what `T` does. A choice is given by its name, which a string takes.
pub fn fits(given: &Carried, takes: &Carried) -> Answer<Against> {
    let (g, t) = (given.present().0, takes.present().0);
    let (gt, tt) = (carried_text(g), carried_text(t));
    let differ = || Answer::Fails(Against::types(tr!("渡す値は {gt}、flow が受け取るのは {tt} で、型が違います", "the request gives {gt}, and the flow takes {tt}: the types differ")));
    // an end of the given range outside the taken one: the nearest value past it, said as the
    // request writes it (`gives`), and the end the flow takes as the flow writes it (`takes`)
    let outside = |gmin: Option<i128>, gmax: Option<i128>, tmin: Option<i128>, tmax: Option<i128>, gives: &dyn Fn(i128) -> String, takes: &dyn Fn(i128) -> String| -> Option<Against> {
        if let Some(lo) = tmin {
            match gmin {
                Some(v) if v >= lo => {}
                Some(v) => return Some(Against::value(Some(gives(v)), tr!("flow が受け取るのは {} 以上です", "the flow takes {} or more", takes(lo); takes(lo)))),
                None => {
                    return Some(Against::value(Some(gives(lo - 1)), tr!("渡す値には下の端が無く、flow が受け取るのは {} 以上です", "nothing holds the value from below, and the flow takes {} or more", takes(lo); takes(lo))));
                }
            }
        }
        if let Some(hi) = tmax {
            match gmax {
                Some(v) if v <= hi => {}
                Some(v) => return Some(Against::value(Some(gives(v)), tr!("flow が受け取るのは {} 以下です", "the flow takes {} or less", takes(hi); takes(hi)))),
                None => {
                    return Some(Against::value(Some(gives(hi + 1)), tr!("渡す値には上の端が無く、flow が受け取るのは {} 以下です", "nothing holds the value from above, and the flow takes {} or less", takes(hi); takes(hi))));
                }
            }
        }
        None
    };
    match (g, t) {
        (Carried::Other(w), _) | (_, Carried::Other(w)) => Answer::Undecided(tr!("{w} は、値を比べません", "{w} is not compared value by value")),
        (Carried::Bool, Carried::Bool) | (Carried::Str | Carried::Enum { .. }, Carried::Str) => Answer::Holds,
        (Carried::Str, Carried::Enum { .. }) => Answer::Fails(Against::types(tr!("渡す値はどの文字列にもなり、flow が受け取るのは列挙の値だけです（{tt}）", "the request gives any string, and the flow takes only the values of the enum ({tt})"))),
        (Carried::Enum { values: gv, .. }, Carried::Enum { values: tv, .. }) => match gv.iter().find(|v| !tv.iter().any(|x: &Spelled| x.same(v))) {
            Some(v) => Answer::Fails(Against::value(Some(v.public.clone()), tr!("flow の列挙にその値はありません（{tt}）", "the flow's enum has no such value ({tt})"))),
            None => Answer::Holds,
        },
        (Carried::Num { written: gw, unit: gu, min: gmin, max: gmax }, Carried::Num { written: tw, unit: tu, min: tmin, max: tmax }) => {
            if !same_unit((gw, gu.as_ref()), (tw, tu.as_ref())) {
                return Answer::Fails(Against::types(tr!("渡す値の単位は {gw}、flow が受け取る単位は {tw} です", "the request counts in {gw}, and the flow in {tw}")));
            }
            // each number in its own unit, its digits grouped (ritsu's DESIGN 6.2 item 8)
            match outside(*gmin, *gmax, *tmin, *tmax, &|v| written::value(v, gu.as_ref()), &|v| written::value(v, tu.as_ref())) {
                Some(a) => Answer::Fails(a),
                None => Answer::Holds,
            }
        }
        (Carried::Date { min: gmin, max: gmax }, Carried::Date { min: tmin, max: tmax }) => {
            let day = |v: i128| day_text(v as i64);
            match outside(gmin.map(i128::from), gmax.map(i128::from), tmin.map(i128::from), tmax.map(i128::from), &day, &day) {
                Some(a) => Answer::Fails(a),
                None => Answer::Holds,
            }
        }
        (Carried::Date { min: Some(a), max: Some(b) }, Carried::Days(set)) => match (*a..=*b).find(|d| !set.contains(d)) {
            Some(d) => Answer::Fails(Against::value(Some(day_text(d)), tr!("flow が受け取る日ではありません", "it is no day the flow takes"))),
            None => Answer::Holds,
        },
        (Carried::Date { .. }, Carried::Days(_)) => Answer::Fails(Against::value(None, tr!("渡す日には端が無く、flow が受け取るのは決まった日だけです（{tt}）", "nothing holds the day at an end, and the flow takes certain days only ({tt})"))),
        (Carried::Days(gs), Carried::Days(_) | Carried::Date { .. }) => {
            let taken = |d: &i64| match t {
                Carried::Days(ts) => ts.contains(d),
                Carried::Date { min, max } => min.is_none_or(|m| *d >= m) && max.is_none_or(|m| *d <= m),
                _ => true,
            };
            match gs.iter().find(|d| !taken(d)) {
                Some(d) => Answer::Fails(Against::value(Some(day_text(*d)), tr!("flow が受け取る日ではありません", "it is no day the flow takes"))),
                None => Answer::Holds,
            }
        }
        _ => differ(),
    }
}

/// What one value a flow is given comes to.
#[derive(Clone, Debug, PartialEq)]
pub enum Given {
    Held,
    /// A value the input does not take (E210).
    Outside(Against),
    /// It can have no value, and the input is not `T?` (E210): an example, in words.
    Lacking(Text),
    /// Undecided (W210): why.
    Undecided(Text),
}

/// X20 for one value: what the request gives, held to what the input takes. A mismatch at a place
/// whose condition ringi does not follow is undecided: the condition may hold the value to less.
/// Two types that differ are a mismatch whatever the condition.
pub fn judge_given(g: &FlowGiven, takes: &Carried) -> Given {
    let optional = takes.present().1;
    let less = |what: Text| match &g.may_be_less {
        Some(why) => Given::Undecided(tr!("{}。ただし、{}が値をもっと絞るかもしれず、ringi はそれを追いません", "{}; but {} may hold the value to less, which ringi does not follow", what.ja, why.ja; what.en, why.en)),
        None => unreachable!("asked only where the value may be less"),
    };
    // whether it has a value
    match (&g.none, optional) {
        (_, true) | (Found::Value(None), _) => {}
        (Found::Value(Some(example)), false) if g.may_be_less.is_some() => return less(example.said.clone()),
        (Found::Value(Some(example)), false) => return Given::Lacking(example.said.clone()),
        (Found::Undecided(why), false) => return Given::Undecided(why.clone()),
    }
    let carried = match &g.carried {
        Found::Value(c) => c,
        Found::Undecided(why) => return Given::Undecided(why.clone()),
    };
    match fits(carried, takes) {
        Answer::Holds => Given::Held,
        Answer::Undecided(why) => Given::Undecided(why),
        Answer::Fails(a) if g.may_be_less.is_some() && !a.whatever => {
            let what = match &a.value {
                Some(v) => tr!("{v} を渡すことがあるように見えます（{}）", "it looks as if it can give {v} ({})", a.why.ja; a.why.en),
                None => a.why.clone(),
            };
            less(what)
        }
        Answer::Fails(a) => Given::Outside(a),
    }
}

/// What an agent's flow answers, held to what ringi reads: None where it answers `verdict`, an enum
/// whose values are `ok` and `doubt`, and `reason`, a string; else what is wrong, in words.
pub fn judge_answer(outputs: &[FlowPlace]) -> Option<Text> {
    let verdict = match outputs.iter().find(|o| o.name == "verdict") {
        None => Some(tr!("出力 `verdict` がありません", "it has no output `verdict`")),
        Some(o) => match &o.carried {
            Carried::Enum { values, .. } if values.len() == 2 && ["ok", "doubt"].iter().all(|w| values.iter().any(|v| v.is(w))) => None,
            c => {
                let ty = carried_text(c);
                Some(tr!("出力 `verdict` が {ty} です", "its output `verdict` is {ty}"))
            }
        },
    };
    let reason = match outputs.iter().find(|o| o.name == "reason") {
        None => Some(tr!("出力 `reason` がありません", "it has no output `reason`")),
        Some(o) => match &o.carried {
            Carried::Str => None,
            c => {
                let ty = carried_text(c);
                Some(tr!("出力 `reason` が {ty} です", "its output `reason` is {ty}"))
            }
        },
    };
    match (verdict, reason) {
        (None, None) => None,
        (Some(a), None) | (None, Some(a)) => Some(a),
        (Some(a), Some(b)) => Some(tr!("{}。{}", "{}, and {}", a.ja, b.ja; a.en, b.en)),
    }
}

/// X20 over the project, through the ports `joined` holds.
pub(crate) fn check(project: &Project, joined: &Joined, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    check_with(project, &*joined.requests(), &*joined.dandori, &joined.ports(), lang, borders)
}

/// [`check`], with each port given: what ringi says of the tickets, and what dandori says a flow
/// declares, read with the rules, the dates files and the books of `ports`.
pub fn check_with(project: &Project, requests: &dyn Requests, flows: &dyn Flows, ports: &Ports, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in project.of(Tool::Ringi) {
        // by the path the file's own check was run with, which ringi keeps the check under
        let disk = std::path::Path::new(&f.shown);
        let Ok(Found::Value(t)) = requests.ticket(disk) else { continue };
        if t.flows.is_empty() {
            continue;
        }
        let src = ritsu_base::fs::read_to_string(disk).unwrap_or_default();
        for fl in &t.flows {
            let Ok(Found::Value(declared)) = flows.declared(&fl.flow, ports) else { continue };
            let one = One { project, file: f, src: &src, t: &t, fl, declared: &declared, flow: crate::ticket_gates::partner(project, &fl.flow) };
            for (b, d) in one.all() {
                borders.add(b);
                out.extend(d.map(|d| Finding::of(&d, Some(f.rel.clone()), lang)));
            }
        }
    }
    out
}

/// One flow a request starts or asks.
struct One<'a> {
    project: &'a Project,
    file: &'a ritsu_project::File,
    src: &'a str,
    t: &'a RequestTicket,
    fl: &'a TicketFlow,
    declared: &'a FlowDeclared,
    /// The flow's file, as the project names it.
    flow: String,
}

impl One<'_> {
    fn at(&self, code: &'static str, message: Text) -> Diag {
        Diag::at(code, &self.file.shown, self.fl.line, 0, message).source(self.src).rel(&self.file.rel)
    }

    fn border(&self, about: String, to: Option<usize>, came: Came, code: Option<&str>) -> Border {
        Border::new(Check::TicketFlow, about, Side::at(Tool::Ringi, &self.file.rel, self.fl.line), Some(Side::reached(self.project, Tool::Dandori, &self.fl.flow, to.filter(|_| ritsu_base::paths::from_root(&self.project.root, &self.fl.flow).is_some()))), came, code)
    }

    /// Where the flow is given its values, for a note: None for a `launch`, whose line says it.
    fn asked(&self) -> Option<Text> {
        let at = self.fl.at.first().cloned().unwrap_or_default();
        match self.fl.kind.as_str() {
            "agent" => Some(tr!(
                "決め方 `{at}` のステップで、ringi は flow にチケットの ID（`request_id`）と、flow が入力に宣言したフォームのフィールドを渡します。",
                "At a stage of the way `{at}`, ringi gives the flow the ticket's ID (`request_id`) and the fields of the form the flow declares as its inputs."
            )),
            "summary" => Some(tr!(
                "ステップ `{at}` で、ringi は flow にチケットの ID（`request_id`）と、flow が入力に宣言したフォームのフィールドを渡します。",
                "At the stage `{at}`, ringi gives the flow the ticket's ID (`request_id`) and the fields of the form the flow declares as its inputs."
            )),
            _ => None,
        }
    }

    fn all(&self) -> Vec<(Border, Option<Diag>)> {
        let launch = self.fl.kind == "launch";
        let mut out = Vec::new();
        // each input of the flow, in the order the flow writes them
        for i in &self.declared.inputs {
            match self.fl.given.iter().find(|g| g.name == i.name) {
                Some(g) => out.push(self.given(g, i)),
                None if launch && i.optional => {}
                None => out.push(self.not_given(i)),
            }
        }
        // what a `launch` writes that the flow has no input for
        if launch {
            for g in self.fl.given.iter().filter(|g| !self.declared.inputs.iter().any(|i| i.name == g.name)) {
                out.push(self.no_input(g));
            }
        }
        match self.fl.kind.as_str() {
            "agent" => out.push(self.answer()),
            "summary" => out.push(self.note()),
            _ => {}
        }
        out
    }

    /// One value the flow is given, against the input that takes it.
    fn given(&self, g: &FlowGiven, i: &FlowPlace) -> (Border, Option<Diag>) {
        let (request, alias, input, flow, written) = (&self.t.named, &self.fl.alias, &i.name, &self.flow, &g.written);
        let about = format!("{alias}({input})");
        let takes = carried_text(&i.carried);
        let came = judge_given(g, &i.carried);
        // what the request gives, and where it is held to less than the form
        let gives = match (&g.carried, &g.narrowed) {
            (Found::Value(c), Some(rows)) => {
                let c = carried_text(c);
                Some(tr!("`{written}` は、{}では {c} です。", "`{written}` is {c} {}.", rows.said.ja; rows.said.en))
            }
            (Found::Value(c), None) => {
                let c = carried_text(c);
                Some(tr!("`{written}` は {c} です。", "`{written}` is {c}."))
            }
            (Found::Undecided(_), _) => None,
        };
        let line = i.line;
        let widen = tr!(
            "渡す値を flow が受け取る範囲に収めるか、\"{flow}\" の入力 `{input}`（{line} 行目）を広げてください。",
            "Hold the value to what the flow takes, or widen the input `{input}` of \"{flow}\" (line {line})."
        );
        let with = |mut d: Diag, notes: Vec<Option<Text>>| {
            for n in notes.into_iter().flatten() {
                d = d.note(n);
            }
            d
        };
        match came {
            Given::Held => (self.border(about, Some(i.line), Came::Held, None), None),
            Given::Outside(a) => {
                let head = match &a.value {
                    Some(v) => tr!(
                        "`{request}` は flow `{alias}` の入力 `{input}` に {v} を渡すことがありますが、入力が受け取るのは {takes} です",
                        "`{request}` can give the flow `{alias}` {v} as its input `{input}`, which takes {takes}"
                    ),
                    None => tr!(
                        "`{request}` が flow `{alias}` の入力 `{input}` に渡す値は、入力が受け取る {takes} に合いません",
                        "What `{request}` gives the flow `{alias}` as its input `{input}` is not what the input takes, {takes}"
                    ),
                };
                let why = a.why.clone();
                // two types that differ are not brought together by a narrower value
                let fix = match (a.whatever, self.fl.kind.as_str()) {
                    (false, _) => widen,
                    (true, "launch") => tr!(
                        "呼び出しで入力の型に合う値を渡すか、\"{flow}\" の入力 `{input}`（{line} 行目）の型を変えてください。",
                        "Give the input a value of its type in the call, or change the type of the input `{input}` in \"{flow}\" (line {line})."
                    ),
                    (true, _) => tr!(
                        "フォームのフィールドと \"{flow}\" の入力 `{input}`（{line} 行目）を同じ型にしてください。",
                        "Give the field of the form and the input `{input}` of \"{flow}\" (line {line}) the same type."
                    ),
                };
                let d = with(self.at("E210", head), vec![gives, Some(tr!("{}。", "{}.", why.ja; capital(&why.en))), self.asked(), Some(fix)]);
                (self.border(about, Some(i.line), Came::Failed, Some("E210")), Some(d))
            }
            Given::Lacking(example) => {
                let ty = &i.ty;
                let d = with(
                    self.at("E210", tr!(
                        "`{request}` は flow `{alias}` の入力 `{input}` に値を渡さないことがありますが、入力は `{ty}` で、値が要ります",
                        "`{request}` can give the flow `{alias}` no value as its input `{input}`, which is `{ty}` and takes one"
                    )),
                    vec![
                        Some(tr!("{}。", "{}.", example.ja; capital(&example.en))),
                        self.asked(),
                        Some(tr!(
                            "\"{flow}\" の入力を `{input} : {ty}?` にするか（{line} 行目）、flow に渡す所ではいつも値があるようにしてください。",
                            "Write the input as `{input} : {ty}?` in \"{flow}\" (line {line}), or have the value there wherever the flow is given it."
                        )),
                    ],
                );
                (self.border(about, Some(i.line), Came::Failed, Some("E210")), Some(d))
            }
            Given::Undecided(why) => {
                let d = with(
                    self.at("W210", tr!(
                        "`{request}` が flow `{alias}` の入力 `{input}` に渡す値を、入力が受け取るかを決められません",
                        "Whether the input `{input}` of the flow `{alias}` takes what `{request}` gives it cannot be decided"
                    )),
                    vec![
                        Some(tr!("決められない理由: {}。", "Why it cannot be decided: {}.", why.ja; why.en)),
                        gives,
                        Some(tr!("入力 `{input}` が受け取るのは {takes} です（\"{flow}\" の {line} 行目）。", "The input `{input}` takes {takes} (line {line} of \"{flow}\").")),
                        self.asked(),
                        // what ringi follows, where a condition it does not follow is why
                        g.may_be_less.as_ref().map(|_| {
                            tr!(
                                "ringi が値を絞って読むのは、経路の表の行と、フォームのフィールドを定数と比べる `when`（`amount <= 500_000JPY`、`and` でつないだものも）です。",
                                "ringi holds a value to less by the rows of the route's table, and by a `when` that compares fields of the form with constants (`amount <= 500_000JPY`, and such comparisons joined with `and`)."
                            )
                        }),
                    ],
                );
                (self.border(about, Some(i.line), Came::Undecided, Some("W210")), Some(d))
            }
        }
    }

    /// An input of the flow nothing is given for.
    fn not_given(&self, i: &FlowPlace) -> (Border, Option<Diag>) {
        let (request, alias, input, flow, ty, line) = (&self.t.named, &self.fl.alias, &i.name, &self.flow, &i.ty, i.line);
        let about = format!("{alias}({input})");
        let d = if self.fl.kind == "launch" {
            self.at("E210", tr!(
                "`{request}` は flow `{alias}` を、入力 `{input}`（`{ty}`）を渡さずに始めます",
                "`{request}` starts the flow `{alias}` without its input `{input}` (`{ty}`)"
            ))
            .note(tr!(
                "呼び出しで渡すか（`{input}: …`）、\"{flow}\" の入力を `{input} : {ty}?` にしてください（{line} 行目）。",
                "Give it in the call (`{input}: …`), or write the input as `{input} : {ty}?` in \"{flow}\" (line {line})."
            ))
        } else {
            let d = self.at("E210", tr!(
                "flow `{alias}` の入力 `{input}` は、`request_id` でも `{request}` のフォームのフィールドでもありません",
                "The input `{input}` of the flow `{alias}` is neither `request_id` nor a field of the form of `{request}`"
            ));
            let d = match self.asked() {
                Some(n) => d.note(n),
                None => d,
            };
            d.note(tr!("ringi には、この入力に渡す値がありません。", "ringi has nothing to give this input."))
                .note(tr!(
                    "\"{flow}\" の入力の名前をフォームのフィールドの名前に合わせるか（{line} 行目）、その入力を外してください。",
                    "Name the input of \"{flow}\" after a field of the form (line {line}), or take the input out."
                ))
        };
        (self.border(about, Some(i.line), Came::Failed, Some("E210")), Some(d))
    }

    /// A value a `launch` writes that the flow has no input for.
    fn no_input(&self, g: &FlowGiven) -> (Border, Option<Diag>) {
        let (request, alias, name, flow) = (&self.t.named, &self.fl.alias, &g.name, &self.flow);
        let inputs = Text::list(&self.declared.inputs.iter().map(|i| Text::same(format!("`{}`", i.name))).collect::<Vec<_>>());
        let d = self
            .at("E210", tr!(
                "`{request}` は flow `{alias}` に `{name}` を渡しますが、flow にその名前の入力はありません",
                "`{request}` gives the flow `{alias}` `{name}`, and the flow has no input of that name"
            ))
            .note(if self.declared.inputs.is_empty() {
                tr!("\"{flow}\" に入力はありません。", "\"{flow}\" has no input.")
            } else {
                tr!("\"{flow}\" の入力: {}。", "The inputs of \"{flow}\": {}.", inputs.ja; inputs.en)
            })
            .note(tr!("呼び出しから `{name}` を外すか、flow に入力を宣言してください。", "Take `{name}` out of the call, or declare the input in the flow."));
        (self.border(format!("{alias}({name})"), None, Came::Failed, Some("E210")), Some(d))
    }

    /// What an agent's flow answers.
    fn answer(&self) -> (Border, Option<Diag>) {
        let (request, alias, flow) = (&self.t.named, &self.fl.alias, &self.flow);
        let way = self.fl.at.first().cloned().unwrap_or_default();
        let about = format!("{alias} -> verdict, reason");
        let to = self.declared.outputs.iter().find(|o| o.name == "verdict").or(self.declared.outputs.first()).map(|o| o.line);
        match judge_answer(&self.declared.outputs) {
            None => (self.border(about, to, Came::Held, None), None),
            Some(wrong) => {
                let d = self
                    .at("E210", tr!(
                        "flow `{alias}` は `{request}` の決め方 `{way}` のステップに答えますが、{}",
                        "The flow `{alias}` answers the stages of the way `{way}` of `{request}`, and {}",
                        wrong.ja; wrong.en
                    ))
                    .note(tr!(
                        "ringi は、エージェントの答えを二つの出力から読みます。`verdict`（値が `ok` と `doubt` の列挙）と、`reason`（string）です。",
                        "ringi reads an agent's answer from two outputs: `verdict`, an enum whose values are `ok` and `doubt`, and `reason`, a string."
                    ))
                    .note(tr!(
                        "\"{flow}\" に、`enum Check = ok | doubt` と、出力 `verdict : Check`、`reason : string` を書いてください。",
                        "Write `enum Check = ok | doubt` and the outputs `verdict : Check` and `reason : string` in \"{flow}\"."
                    ));
                (self.border(about, to, Came::Failed, Some("E210")), Some(d))
            }
        }
    }

    /// What a summary's flow answers.
    fn note(&self) -> (Border, Option<Diag>) {
        let (request, alias, flow) = (&self.t.named, &self.fl.alias, &self.flow);
        let stage = self.fl.at.first().cloned().unwrap_or_default();
        let about = format!("{alias} -> note");
        if let Some(o) = self.declared.outputs.first() {
            // each output is a value a note holds (DESIGN 7.1 of ringi): text, a whole number, a
            // truth, rows, or nothing
            let records = records_of(&ritsu_base::fs::read_to_string(&self.fl.flow).unwrap_or_default());
            let wrong: Vec<(&FlowPlace, Text)> = self.declared.outputs.iter().filter_map(|o| noted(&o.ty, Some(&o.carried), &records, 0).err().map(|why| (o, why))).collect();
            let Some((first, why)) = wrong.first() else {
                return (self.border(about, Some(o.line), Came::Held, None), None);
            };
            let (name, ty) = (&first.name, &first.ty);
            let mut d = self
                .at("E210", tr!(
                    "flow `{alias}` は `{request}` のステップ `{stage}` の要約を作りますが、出力 `{name}` は `{ty}` で、要約は{}を持てません",
                    "The flow `{alias}` writes the note of the stage `{stage}` of `{request}`, and its output `{name}` is `{ty}`: a note does not hold {}",
                    why.ja; why.en
                ))
                .note(tr!(
                    "要約が持てる値は、文（string、date、timestamp、列挙）、整数（int と単位の付いた数。単位で数えた整数で運びます）、真偽（bool）、行（レコードの list。レコードのフィールドもこれらの値）、それらの `T?`（値が無ければ無い値）です。",
                    "A note holds text (a string, a date, a timestamp, an enum), a whole number (an int, or a number with a unit, carried as the whole number counted in its unit), a truth (a bool), rows (a list of records whose fields hold these too), and a `T?` of these (nothing where it has no value)."
                ));
            for (o, why) in wrong.iter().skip(1) {
                let (n, t) = (&o.name, &o.ty);
                d = d.note(tr!("出力 `{n}` も `{t}` で、要約は{}を持てません。", "The output `{n}` is `{t}` too: a note does not hold {}.", why.ja; why.en));
            }
            let d = d.note(tr!(
                "\"{flow}\" の出力を、要約が持てる型にしてください（一つのレコードは、一行の `list[…]` にします）。",
                "Give the outputs of \"{flow}\" a type a note holds (a record as the one row of a `list[…]`)."
            ));
            return (self.border(about, Some(first.line), Came::Failed, Some("E210")), Some(d));
        }
        let d = self
            .at("E210", tr!(
                "flow `{alias}` は `{request}` のステップ `{stage}` の要約を作りますが、出力がありません",
                "The flow `{alias}` writes the note of the stage `{stage}` of `{request}`, and it has no output"
            ))
            .note(tr!("ringi は、flow の出力を承認カードに見せます。", "ringi shows the flow's outputs on the approval card."))
            .note(tr!("\"{flow}\" に、見せる出力を一つ以上書いてください（`outputs`）。", "Write at least one output to show in \"{flow}\" (`outputs`)."));
        (self.border(about, None, Came::Failed, Some("E210")), Some(d))
    }
}

/// The records a flow declares, each with the types of its fields as written, read from its
/// source: what a note's output that is a list of records holds in each row.
fn records_of(src: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut at: Option<String> = None;
    for line in src.lines() {
        if let Some(rest) = line.strip_prefix("record ") {
            let name = rest.split_whitespace().next().unwrap_or_default().to_string();
            out.insert(name.clone(), Vec::new());
            at = Some(name);
            continue;
        }
        if !line.starts_with(' ') && !line.trim().is_empty() {
            at = None;
            continue;
        }
        let (Some(r), Some((field, ty))) = (&at, line.trim().split_once(':')) else { continue };
        if field.trim().is_empty() || field.trim().contains([' ', '"', '#']) {
            continue;
        }
        // the type, without what is written after it (`range …`, `fed by …`)
        let ty = ty.trim();
        let end = ty.find("  ").or_else(|| ty.find(" range")).or_else(|| ty.find(" fed ")).unwrap_or(ty.len());
        if let Some(fields) = out.get_mut(r) {
            fields.push(ty[..end].trim().to_string());
        }
    }
    out
}

/// Whether a note holds a value of the type `ty` (as the flow writes it; `carried` where dandori
/// says it): Err with what it is where it does not. A note holds text (a string, a date, a
/// timestamp, an enum), a whole number (an int, or a number with a unit, which dandori carries as
/// the whole number counted in its unit), a truth, rows (a list of records whose fields hold these
/// too), and a `T?` of these.
fn noted(ty: &str, carried: Option<&Carried>, records: &BTreeMap<String, Vec<String>>, depth: usize) -> Result<(), Text> {
    let ty = ty.trim();
    let ty = ty.strip_suffix('?').unwrap_or(ty).trim();
    let mut c = carried;
    while let Some(Carried::Opt(inner)) = c {
        c = Some(inner);
    }
    if matches!(c, Some(Carried::Str | Carried::Bool | Carried::Date { .. } | Carried::Days(_) | Carried::Enum { .. } | Carried::Num { .. })) {
        return Ok(());
    }
    if let Some(inner) = ty.strip_prefix("list[").and_then(|x| x.strip_suffix(']')) {
        let inner = inner.trim();
        return match records.get(inner) {
            Some(fields) if depth < 8 => {
                for f in fields {
                    noted(f, None, records, depth + 1).map_err(|w| tr!("フィールドに{}を持つレコードの並び", "a list of records with {} in a field", w.ja; w.en))?;
                }
                Ok(())
            }
            Some(_) => Ok(()),
            None => Err(tr!("レコードでない値の並び", "a list of what is not a record")),
        };
    }
    let base = ty.split(['[', ' ']).next().unwrap_or_default();
    match base {
        "string" | "bool" | "int" | "date" | "timestamp" => Ok(()),
        "json" => Err(tr!("形を決めない値（json）", "a value of no set shape (json)")),
        _ if records.contains_key(base) => Err(tr!("一つのレコード", "a record on its own")),
        // a message of a contract (`use proto`), as dandori says it
        _ if matches!(c, Some(Carried::Other(_))) && base.contains('.') => Err(tr!("契約のメッセージ", "a message of a contract")),
        // a number with a unit (`money[JPY]`), an enum
        _ => Ok(()),
    }
}

/// A sentence's first letter as a capital, where it is a letter of ASCII.
fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) if f.is_ascii_lowercase() => f.to_ascii_uppercase().to_string() + c.as_str(),
        _ => s.to_string(),
    }
}
