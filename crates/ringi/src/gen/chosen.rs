//! What `types/<name>.json` says of who answers each stage and of its deadlines (DESIGN 12.2): in
//! English and in Japanese, in the words of the kind's page, and with what a program reads beside
//! them, so that a server shows what changes between two versions of a kind from its two
//! descriptions alone.
//!
//! - Each stage's `approvers`: the cases it comes in, in the order of the route: when (null where
//!   it always comes), who answers, the ways to choose it names (by their ASCII names; of a rule's
//!   output, the ways its enum can name), what kind of
//!   stage it is (`who`, `quorum`, `circulate`, `note`, `rule_output`), and the shared route it is
//!   of (null for the request's own). A stage of a route's table comes in each row that has it; a
//!   stage of stage lines on each line of it; a stage of a shared route as the route has it, with
//!   what the request gives the route's inputs, and with the row of the table that chooses the
//!   route, when one does.
//! - Each stage's `deadlines`: the lines of its deadlines in the order written, its own where it
//!   writes them, else those of `every stage` (as the machine reads them, [`blocks_of`]), each with
//!   its kind (`due`, `remind`, `late`, `empty`), the days and the calendar of a due day and of a
//!   reminder, and a `late` and an `empty` line as written.
//! - Each way's `how`: who it chooses, as the page's "How the approvers are chosen" says it.
//! - `late_tables`: each table a late stage goes up by, its head and rows as the page shows them.
//!
//! [`blocks_of`]: crate::lower::terms::blocks_of

use crate::ast::{Deadline, Deadlines, Route, Span, Spec, Test, UseKind, Word};
use crate::doc::words::Say;
use crate::names::Scope;
use ritsu_base::json::Json;
use ritsu_base::text::Lang;

/// What [`of`] answers.
#[derive(Clone, Debug)]
pub struct Chosen {
    /// Each stage by its name as written: its `approvers` and its `deadlines`.
    pub stages: Vec<(String, Json, Json)>,
    /// Each way to choose by its name as written: its `how`.
    pub ways: Vec<(String, Json)>,
    /// `late_tables`.
    pub tables: Json,
}

/// A case a stage comes in, said in one language.
struct Case {
    stage: String,
    when: Option<String>,
    who: String,
    ways: Vec<String>,
    kind: &'static str,
    route: Option<String>,
}

/// The texts of the two languages, as an object of `en` and `ja`.
fn both(en: String, ja: String) -> Json {
    Json::obj([("en", Json::str(en)), ("ja", Json::str(ja))])
}

/// What the stages, the ways and the tables of the request of `s` say.
pub fn of(s: &Scope) -> Chosen {
    let (en, ja) = (Say::new(s, Lang::En), Say::new(s, Lang::Ja));
    let (ce, cj) = (cases(&en), cases(&ja));
    let (every, own) = crate::lower::terms::blocks_of(s);
    let mut stages = Vec::new();
    for n in s.stages().unwrap_or_default() {
        let approvers = Json::arr(ce.iter().zip(&cj).filter(|(e, _)| e.stage == n.text).map(|(e, j)| {
            Json::obj([
                ("when", match (&e.when, &j.when) {
                    (Some(a), Some(b)) => both(a.clone(), b.clone()),
                    _ => Json::Null,
                }),
                ("who", both(e.who.clone(), j.who.clone())),
                ("ways", Json::arr(e.ways.iter().map(Json::str))),
                ("kind", Json::str(e.kind)),
                ("route", e.route.as_deref().map(Json::str).unwrap_or(Json::Null)),
            ])
        }));
        let d: Option<&Deadlines> = own.iter().find(|(text, _, _)| *text == n.text).map(|(_, _, d)| *d).or(every);
        let deadlines = Json::arr(d.map(|d| d.lines.as_slice()).unwrap_or_default().iter().map(|l| deadline(l, &en, &ja)));
        stages.push((n.text.clone(), approvers, deadlines));
    }
    let mut ways: Vec<(String, Json)> = Vec::new();
    for w in s.who_all() {
        if !ways.iter().any(|(n, _)| *n == w.who.name.text) {
            ways.push((w.who.name.text.clone(), both(en.how(&w.who.how), ja.how(&w.who.how))));
        }
    }
    let mut tables = Vec::new();
    for f in en.files() {
        for t in &f.tables {
            let side = |say: &Say| {
                let named = |n: &crate::ast::Name| if n.alias.is_some() { n.text.clone() } else { format!("`{}`", n.text) };
                let mut head: Vec<String> = t.inputs.iter().map(named).collect();
                head.extend(t.outputs.iter().map(named));
                let ins: Vec<Vec<Word>> = t.inputs.iter().map(|n| vec![Word { text: n.text.clone(), span: n.span }]).collect();
                let rows = t.rows.iter().map(|r| {
                    let mut cells: Vec<String> = ins.iter().zip(&r.tests).map(|(c, (test, _))| crate::doc::request::way_or_test(say, c, test)).collect();
                    cells.extend(r.stages.iter().map(|(sp, _)| say.spec(sp)));
                    Json::arr(cells.into_iter().map(Json::str))
                });
                Json::obj([("head", Json::arr(head.into_iter().map(Json::str))), ("rows", Json::arr(rows))])
            };
            tables.push(Json::obj([("name", Json::str(t.name.ascii())), ("en", side(&en)), ("ja", side(&ja))]));
        }
    }
    Chosen { stages, ways, tables: Json::arr(tables) }
}

/// The cases each stage comes in, in the order of the route, said in the language of `say`.
fn cases(say: &Say) -> Vec<Case> {
    let s = say.s;
    let mut out = Vec::new();
    match &s.file.route {
        None => {}
        Some(Route::Stages(lines)) => {
            for l in lines {
                let when = l.when.as_ref().map(|c| say.when(say.cond(c, None)));
                out.push(case(say, &l.name.text, when, &l.spec, None));
            }
        }
        Some(Route::Table(t)) => {
            for r in &t.rows {
                let when = row_when(say, &t.inputs, &r.tests);
                if t.chooses_route {
                    for (sp, _) in &r.stages {
                        if let Spec::Route(alias) = sp
                            && let Some(f) = s.ringi(&alias.text, UseKind::Route)
                        {
                            shared(say, f, &[], when.clone(), &mut out);
                        }
                    }
                    continue;
                }
                for (n, (sp, _)) in t.stages.iter().zip(&r.stages) {
                    if !matches!(sp, Spec::Empty) {
                        out.push(case(say, &n.text, when.clone(), sp, None));
                    }
                }
            }
        }
        Some(Route::Shared { alias, args, .. }) => {
            if let Some(f) = s.ringi(&alias.text, UseKind::Route) {
                shared(say, f, args, None, &mut out);
            }
        }
    }
    out
}

/// The cases of the stages of the shared route `f`, given `args`, after the condition `before`
/// (the row of a table that chooses it).
fn shared(say: &Say, f: &crate::ast::File, args: &[crate::ast::Arg], before: Option<String>, out: &mut Vec<Case>) {
    let Some(Route::Stages(lines)) = &f.route else { return };
    for l in lines {
        let own = l.when.as_ref().map(|c| say.when(crate::doc::request::shared_cond(say, f, c, args)));
        let when = match (before.clone(), own) {
            (Some(a), Some(b)) => Some(say.t(format!("{a}, and {b}"), format!("{a}、かつ{b}"))),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };
        out.push(case(say, &l.name.text, when, &l.spec, Some(f.name.ascii().to_string())));
    }
}

fn case(say: &Say, stage: &str, when: Option<String>, sp: &Spec, route: Option<String>) -> Case {
    let all = say.s.who_all();
    let ascii = |text: &str| all.iter().find(|x| x.who.name.text == text).map_or_else(|| text.to_string(), |x| x.who.name.ascii().to_string());
    // a rule's output: the ways the rule's enum can name
    let ways = match sp {
        Spec::RuleOutput(p) => say.rule_way_names(p).iter().map(|n| ascii(n)).collect(),
        _ => sp.who().iter().map(|w| ascii(&w.text)).collect(),
    };
    let kind = match sp {
        Spec::Quorum(..) => "quorum",
        Spec::Circulate(_) => "circulate",
        Spec::Note(_) => "note",
        Spec::RuleOutput(_) => "rule_output",
        _ => "who",
    };
    Case { stage: stage.to_string(), when, who: say.spec(sp), ways, kind, route }
}

/// When a row of a route's table holds, its columns of any value left out: null for a row of no
/// other column.
fn row_when(say: &Say, inputs: &[Vec<Word>], tests: &[(Test, Span)]) -> Option<String> {
    let parts: Vec<String> = inputs
        .iter()
        .zip(tests)
        .filter(|(_, (t, _))| !matches!(t, Test::Any))
        .map(|(c, (t, _))| {
            let (col, test) = (say.column(c), say.test(c, t));
            say.t(format!("“{col}” is {test}"), format!("「{col}」が{test}"))
        })
        .collect();
    if parts.is_empty() {
        return None;
    }
    Some(say.when(say.t(parts.join(", and "), parts.join("、かつ"))))
}

/// A line of a stage's deadlines.
fn deadline(l: &Deadline, en: &Say, ja: &Say) -> Json {
    let cal = |c: &Option<Word>| c.as_ref().map(|w| Json::str(&w.text)).unwrap_or(Json::Null);
    let (kind, days, calendar, written) = match l {
        Deadline::Due { days, calendar, .. } => ("due", Json::int(i128::from(*days)), cal(calendar), Json::Null),
        Deadline::Remind { days, calendar, .. } => ("remind", Json::int(i128::from(*days)), cal(calendar), Json::Null),
        Deadline::Late { first, then, .. } => ("late", Json::Null, Json::Null, Json::str(crate::ast::Late::line(first, then.as_ref()))),
        Deadline::Empty { what, .. } => ("empty", Json::Null, Json::Null, Json::str(what.written())),
    };
    Json::obj([("kind", Json::str(kind)), ("days", days), ("calendar", calendar), ("written", written), ("en", Json::str(en.deadline(l))), ("ja", Json::str(ja.deadline(l)))])
}
