//! The route (DESIGN 5) as far as ringi decides it alone: a quorum the approvers it counts can
//! reach (E304), a person chosen by an approver of an earlier stage, or by the requester, in a field
//! that one fills in (E305), what a shared route is given (E307), and the stages the ticket, the
//! claims, the examples and the scenarios name (E308). Whether the table takes every request once
//! and the conditions are whole is rulec's to say, through the lowered rule, in a later stage.

use crate::ast::*;
use crate::diag::Diag;
use crate::names::{self, Scope};
use crate::types::{Ty, Typer, agree, written};

pub fn check(s: &Scope, suite: &crate::suite::Suite, out: &mut Vec<Diag>) {
    quorums(s, out);
    chosen(s, out);
    shared_args(s, out);
    stage_refs(s, out);
    example_values(s, suite, out);
}

/// E601 at a cell of the examples whose value the form does not take, whether the route reads the
/// column or not (DESIGN 5.3): a number or a date outside its field's `range`, a number off its
/// unit's step, a text longer or shorter than its `count`, a key no row of its master has. A value
/// a `choice` does not have is E004 and a value of another type E009, as the types say
/// ([`crate::types`]); a cell said of already is not said again. Said here, before the route is
/// lowered, a column the route reads and one it does not are told the same thing at the same
/// place, and rulec is not given a row no request can be. What a field takes is decided where
/// every value of a form is read ([`crate::taken`]): a row has no day, so a row of a master of any
/// day is taken, and a person or a team of any version of the organization.
fn example_values(s: &Scope, suite: &crate::suite::Suite, out: &mut Vec<Diag>) {
    let Some(e) = &s.file.examples else { return };
    let field = |k: usize| e.inputs.get(k).filter(|c| c.len() == 1).and_then(|c| s.field(&c[0].text));
    // the calendars are read only where a column is a date of `open in`
    let calendars = if (0..e.inputs.len()).any(|k| field(k).is_some_and(|f| f.open_in.is_some())) { crate::engine::calendars(s, suite) } else { Vec::new() };
    let when = crate::taken::When { day: None, versions: s.org_data().map(|o| o.versions.iter().collect()), calendars: &calendars, rows: None };
    let said: Vec<(Option<usize>, Option<usize>)> = out.iter().filter(|d| d.file == s.path).map(|d| (d.line, d.col)).collect();
    for r in &e.rows {
        for (k, (v, _)) in r.values.iter().enumerate() {
            let Some(f) = field(k) else { continue };
            if said.contains(&(Some(v.span.line), Some(v.span.col))) {
                continue;
            }
            if let Some(why) = crate::taken::cell(s, f, v, &when) {
                out.push(s.err("E601", v.span, why).note(tr!(
                    "例の行は申請です。経路がその列を読むかどうかにかかわらず、フォームが取る値を書いてください。",
                    "A row of the examples is a request: write values the form takes, whether the route reads the column or not."
                )));
            }
        }
    }
}

/// Every spec of the file's own route, with where it is written: each stage line's, each cell of
/// the table, by row.
fn route_specs(f: &File) -> Vec<(&Spec, Span)> {
    match &f.route {
        Some(Route::Stages(lines)) => lines.iter().map(|l| (&l.spec, l.spec_span)).collect(),
        Some(Route::Table(t)) => t.rows.iter().flat_map(|r| r.stages.iter().map(|(sp, at)| (sp, *at))).collect(),
        _ => Vec::new(),
    }
}

/// E304: a quorum of more than one, of a way to choose that names one person; a share above 100%.
fn quorums(s: &Scope, out: &mut Vec<Diag>) {
    for (spec, at) in route_specs(s.file) {
        let Spec::Quorum(q, w, span) = spec else { continue };
        let at = if span.line > 0 { *span } else { at };
        match q {
            Quorum::Percent(n) if *n > 100 => {
                out.push(s.err("E304", at, tr!("`at least {n}%` は 100% を超えるので、届きません", "`at least {n}%` is more than 100%, and cannot be reached")));
            }
            Quorum::AtLeast(n) if *n > 1 => {
                if let Some(who) = s.who(&w.text)
                    && who.how.one_person()
                {
                    let x = &w.text;
                    out.push(s.err("E304", at, tr!("`{x}` は一人を決めるので、`at least {n} of {x}` には届きません", "`{x}` chooses one person, so `at least {n} of {x}` cannot be reached")).note(tr!(
                        "人数を数える議決は、`team legal` のようにチームで決める承認者に使ってください。",
                        "A quorum counts the members of a team: use it with a way to choose that names a team (`team legal`)."
                    )));
                }
            }
            _ => {}
        }
    }
}

/// The way to choose `w` names when it is `chosen by …`.
fn chosen_by<'b>(s: &Scope<'b>, w: &Word) -> Option<(bool, &'b Word)> {
    match &s.who(&w.text)?.how {
        How::ChosenBy { previous, field } => Some((*previous, field)),
        _ => None,
    }
}

/// E305: a person chosen by the approver before, where no stage before approves, or in a field the
/// requester fills in; a person chosen by the requester in a field approvers fill in.
fn chosen(s: &Scope, out: &mut Vec<Diag>) {
    let f = s.file;
    // the field each `chosen by` reads: who fills it in
    for w in &f.who {
        let How::ChosenBy { previous, field } = &w.how else { continue };
        let Some(fd) = s.field(&field.text) else { continue };
        let (n, x) = (&w.name.text, &field.text);
        if *previous && fd.approvers_only.is_none() {
            out.push(s.err("E305", field.span, tr!("`{n}` は前の承認者が `{x}` で指名しますが、`{x}` は申請者が入れるフィールドです", "`{n}` is chosen by the approver before, in `{x}`, and the requester fills in `{x}`")).note(tr!(
                "`{x}` に `approvers only` を書くか、申請者が指名するなら `chosen by requester in {x}` と書いてください。",
                "Write `approvers only` on `{x}`; or, for the requester to choose, write `chosen by requester in {x}`."
            )));
        }
        if !*previous && fd.approvers_only.is_some() {
            out.push(s.err("E305", field.span, tr!("`{n}` は申請者が `{x}` で指名しますが、`{x}` は承認者だけが入れるフィールドです", "`{n}` is chosen by the requester, in `{x}`, and only approvers fill in `{x}`")).note(tr!(
                "承認者が指名するなら `chosen by previous approver in {x}` と書き、申請者が指名するなら `{x}` の `approvers only` を消してください。",
                "For an approver to choose, write `chosen by previous approver in {x}`; for the requester to, take `approvers only` off `{x}`."
            )));
        }
    }
    // a stage chosen by the approver before needs a stage before it that approves
    let first_stage = |spec: &Spec| -> Option<String> {
        spec.who().into_iter().find(|w| chosen_by(s, w).is_some_and(|(previous, _)| previous)).map(|w| w.text.clone())
    };
    let none_before = |who: &str, stage: &str, at: Span| {
        s.err("E305", at, tr!("`{who}` は前の承認者が指名しますが、ステップ `{stage}` より前に、必ず承認するステップがありません", "`{who}` is chosen by the approver before, and no stage before `{stage}` always approves")).note(tr!(
            "前の承認者は、それより前にある条件の無いステップの承認者です。回覧（`circulate to`）と要約（`note from`）は承認しません。",
            "The approver before is the one of a stage before it with no condition; a stage that circulates (`circulate to`) or summarizes (`note from`) approves nothing."
        ))
    };
    match &f.route {
        Some(Route::Stages(lines)) => {
            for (i, l) in lines.iter().enumerate() {
                let Some(who) = first_stage(&l.spec) else { continue };
                if !lines[..i].iter().any(|x| x.when.is_none() && x.spec.approves()) {
                    out.push(none_before(&who, &l.name.text, l.spec_span));
                }
            }
        }
        Some(Route::Table(t)) if !t.chooses_route => {
            for r in &t.rows {
                for (k, (spec, at)) in r.stages.iter().enumerate() {
                    let Some(who) = first_stage(spec) else { continue };
                    if !r.stages[..k].iter().any(|(x, _)| x.approves()) {
                        let stage = t.stages.get(k).map(|n| n.text.clone()).unwrap_or_default();
                        out.push(none_before(&who, &stage, *at));
                    }
                }
            }
        }
        _ => {}
    }
}

/// E307: what a shared route is given, against its `inputs`.
fn shared_args(s: &Scope, out: &mut Vec<Diag>) {
    match &s.file.route {
        Some(Route::Shared { alias, args, line }) => {
            let Some(shared) = s.ringi(&alias.text, UseKind::Route) else { return };
            let a = &alias.text;
            let names: Vec<String> = shared.inputs.iter().map(|i| i.name.text.clone()).collect();
            let mut scratch = Vec::new();
            for (k, arg) in args.iter().enumerate() {
                let n = &arg.name.text;
                if args[..k].iter().any(|x| x.name.text == arg.name.text) {
                    out.push(s.err("E307", arg.name.span, tr!("`{n}` を二度渡しています", "`{n}` is given twice")));
                    continue;
                }
                let Some(input) = shared.inputs.iter().find(|i| i.name.text == arg.name.text) else {
                    let mut d = s.err("E307", arg.name.span, tr!("共有の経路 `{a}` は `{n}` を受け取りません", "The shared route `{a}` does not take `{n}`"));
                    if let Some(note) = names::declared(tr!("入力", "inputs"), &names) {
                        d = d.note(note);
                    }
                    out.push(d);
                    continue;
                };
                let want = Ty::of_field(input);
                let got = Typer::new(s, &mut scratch).against(&arg.value, &want);
                if !agree(&got, &want) {
                    let (w, gs, ws) = (written(s.src, arg.value.span), got.shown(), want.shown());
                    out.push(s.err("E307", arg.value.span, tr!("`{n}` は `{ws}` を受け取りますが、渡した `{w}` は `{gs}` です", "`{n}` takes `{ws}`, and `{w}`, what it is given, is `{gs}`")));
                }
            }
            out.extend(scratch);
            for input in shared.inputs.iter().filter(|i| i.optional.is_none() && i.computed.is_none()) {
                if !args.iter().any(|x| x.name.text == input.name.text) {
                    let n = &input.name.text;
                    out.push(s.err("E307", Span { line: *line, col: alias.span.col, end: alias.span.end }, tr!("共有の経路 `{a}` に `{n}` を渡していません", "The shared route `{a}` is not given `{n}`")).note(tr!(
                        "`route {a}({n}: …)` のように渡してください。",
                        "Give it, as in `route {a}({n}: …)`."
                    )));
                }
            }
        }
        Some(Route::Table(t)) if t.chooses_route => {
            for r in &t.rows {
                for (spec, at) in &r.stages {
                    let Spec::Route(w) = spec else { continue };
                    let Some(shared) = s.ringi(&w.text, UseKind::Route) else { continue };
                    if let Some(input) = shared.inputs.iter().find(|i| i.optional.is_none() && i.computed.is_none()) {
                        let (a, n) = (&w.text, &input.name.text);
                        out.push(s.err("E307", *at, tr!("共有の経路 `{a}` は `{n}` を受け取りますが、表のセルからは何も渡せません", "The shared route `{a}` takes `{n}`, and a cell of the table gives it nothing")).note(tr!(
                            "入力を受け取る共有の経路は、`route {a}({n}: …)` のように一行で使ってください。",
                            "Use a shared route that takes inputs on a line of its own, as in `route {a}({n}: …)`."
                        )));
                    }
                }
            }
        }
        _ => {}
    }
}

/// E308: a stage the route does not have, where the ticket, a claim, the examples or a scenario
/// names one. One name is said once, where the file first writes it, with the other lines that
/// write it in a note (ritsu's DESIGN 8.3; `diag::sort`).
fn stage_refs(s: &Scope, out: &mut Vec<Diag>) {
    let Some(stages) = s.stages() else { return };
    let names: Vec<String> = stages.iter().map(|n| n.text.clone()).collect();
    let mut check = |w: &Word| {
        if !names.contains(&w.text) {
            let n = &w.text;
            let mut d = s.err("E308", w.span, tr!("経路に `{n}` というステップはありません", "The route has no stage `{n}`")).same_in("stages", n.clone());
            if let Some(note) = names::declared(tr!("ステップ", "stages"), &names) {
                d = d.note(note);
            }
            out.push(d);
        }
    };
    let f = s.file;
    if let Some(t) = &f.ticket {
        for l in &t.lines {
            if let TicketLine::Withdraw { until: Some(w), .. } = l {
                check(w);
            }
        }
    }
    for c in &f.claims {
        match &c.body {
            ClaimBody::OnlyWhen { stage, .. } | ClaimBody::Possible { stage, .. } => check(stage),
            _ => {}
        }
    }
    if let Some(e) = &f.examples {
        for w in &e.stages {
            check(w);
        }
    }
    for sc in &f.scenarios {
        for ev in &sc.events {
            if let Some(w) = &ev.stage {
                check(w);
            }
        }
    }
}
