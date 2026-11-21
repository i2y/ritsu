//! The code that joins a request to the machine of its tickets (DESIGN 12.1, 12.2): a module of
//! kaname's, laid in the request's lowered directory beside the files its form, route and deadlines
//! are lowered to (`held.lowered/held.kn`). It reads the machine's module `ticket` (the machine
//! itself, [`TICKET`], which the crate holds and a project lays once with the day numbers it reads,
//! [`DAYS`], DESIGN 12.3), the lowered `route.rule` and `terms.cal`, and writes what the request
//! says as the machine's data: the form the requester gives, the request with its ways to choose an
//! approver and the stages of its route (`spec`), how the route is read for the values of a form
//! (`formed`), the values a flow is given (`vals`), a form given again or edited (`reform`), who may
//! submit it, the deadlines of a stage (`clocks`), the business days of a pause, of the requester's
//! turn and of a reminder counted apart (`open_after`), and what the ticket does as it is submitted
//! and as it ends (`on_lines`). `start` and `next` are what a host calls (DESIGN 12.2).
//!
//! The business days are counted by files of koyomi's the glue writes beside itself, one for each
//! calendar a count is made in (`open.cal`, `open_<calendar>.cal`): from each day a stage can start
//! on, as the deadlines are counted (the window of `terms.cal`), as many business days as the
//! request counts at most there. A day outside them is refused (`data`).
//!
//! The module is written from the machine of the reference interpreter ([`Machine`]), from the
//! same lowered files, so that the two read a request alike; the vectors hold the code kaname
//! writes from it to what the reference answers (DESIGN 10.4).
//!
//! What the module does not lower yet is said, and nothing is written for the request.

mod form;
mod on;

use super::words;
use crate::ast::{AtTop, How, Late, NamedTable, Spec, TicketLine, WhenEmpty};
use crate::engine::Machine;
use crate::lower::Part;
use ritsu_base::text::Text;
use std::path::Path;

/// The machine of a ticket, the module `ticket` of kaname's (DESIGN 12.1, 12.3): the same for every
/// request, which a request's glue reads.
pub const TICKET: &str = include_str!("../ticket.kn");

/// The name of the machine's file, where a project lays it.
pub const TICKET_FILE: &str = "ticket.kn";

/// The day of a day's number, a file of koyomi's the machine reads (kaname adds no days to a date):
/// laid beside the machine.
pub const DAYS: &str = include_str!("../days.cal");

/// The name of the file of the day numbers, beside the machine.
pub const DAYS_FILE: &str = "days.cal";

/// The most input combinations a file of business days is checked over: the days it counts from,
/// times the counts.
const OPEN_BUDGET: i64 = 10_000_000;

/// The glue of a request: its file's name in the lowered directory, its text, the request it is of
/// (its ASCII name, which is the module's, and its version), which a package's list of the kinds of
/// request names (DESIGN 12.2), and the files it reads besides the lowered ones, laid beside it (each
/// name in the lowered directory, with its text).
#[derive(Clone, Debug)]
pub struct Glue {
    pub name: String,
    pub text: String,
    pub request: String,
    pub version: u32,
    pub files: Vec<(String, String)>,
}

/// The glue of the request the machine is of, reading the machine's module at `ticket` (where it is
/// laid, as the program reaches it). Err with what of the request it does not lower yet.
pub fn glue(m: &Machine, ticket: &Path) -> Result<Glue, Text> {
    let g = Gen { m, offset: m.offset };
    let name = m.s.file.name.ascii().to_string();
    // the files as the system opens them (ritsu's DESIGN 4.15)
    let dir = ritsu_base::paths::opened(&m.lowered.dir);
    let machine = ritsu_base::paths::between(&dir, &ritsu_base::paths::opened(ticket));
    let mut form = form::FormOf::of(m)?;
    let terms = g.terms(&mut form.aliases)?;
    let opens = g.opens(&terms, &mut form.aliases)?;
    let route = m.lowered.files.iter().find(|f| f.part == Part::Route);
    // the names of the `use` lines, which no value of the form is taken by
    let mut uses = vec!["machine".to_string()];
    if route.is_some() {
        uses.push("route".to_string());
    }
    uses.extend(form.use_names());
    uses.extend(terms.iter().map(|t| t.alias.clone()));
    uses.extend(opens.iter().filter(|o| o.text.is_some()).map(|o| o.alias.clone()));
    form.name_locals(&uses);
    let mut out = String::new();
    // a request named with a word kaname reserves is a module of another name, with the word as its
    // ASCII name, which the generated code takes (the crate's door of Rust knows it by the other)
    let module = if words::KANAME_RESERVED.contains(&name.as_str()) { format!("{name}_value({name})") } else { name.clone() };
    out.push_str(&format!("module {module} v{}\n", m.s.file.version));
    out.push_str(&format!(
        "description \"The ticket of the request {name}: its form, the way its route and deadlines are read, and what it does as it is submitted and as it ends\"\n\n"
    ));
    out.push_str(&format!("use module machine from \"{machine}\"\n"));
    if route.is_some() {
        out.push_str("use rule route from \"route.rule\"\n");
    }
    out.push_str(&form.uses());
    for t in &terms {
        out.push_str(&format!("use dates {} from \"{}\"\n", t.alias, t.file));
    }
    for o in opens.iter().filter(|o| o.text.is_some()) {
        out.push_str(&format!("use dates {} from \"{}\"\n", o.alias, o.file));
    }
    out.push('\n');
    out.push_str(&form.records());
    out.push_str(&g.spec()?);
    out.push_str("fn refused(text: string, text_ja: string, at: string?) -> machine.Refusal total\n  machine.Refusal { refused: machine.Kind.form, text: text, text_ja: text_ja, at: at }\n\n");
    out.push_str(&form.functions()?);
    out.push_str(&form.formed(route.is_some())?);
    out.push_str(&form.vals());
    out.push_str(&form.reform()?);
    out.push_str(&g.submitters());
    out.push_str(&g.clocks(&terms, &opens)?);
    out.push_str(&g.open_after(&opens));
    out.push_str(&on::lines(m, &form)?);
    // what the form works out for values given from outside the machine: the entry `worked`
    out.push_str(&form.worked_out()?);
    // the functions the values that may be empty are read through, made as the parts above read them
    out.push_str(&form.helpers());
    out.push_str(&g.start_and_next());
    let mut files: Vec<(String, String)> = opens.iter().filter_map(|o| o.text.clone().map(|t| (o.file.clone(), t))).collect();
    files.extend(form.files());
    Ok(Glue { name: format!("{name}.kn"), text: out, request: name.clone(), version: m.s.file.version, files })
}

/// The names the glue declares at the top of its module whatever the request is: the names of its
/// `use` lines, its records and its functions. A name the glue takes from the request in the same
/// place (the alias of a rule it reads) is none of them.
pub(super) const DECLARED: [&str; 37] = [
    "machine", "route", "values", "form_dates", "terms", "open", "Form", "Given", "Outside", "Worked", "Shown", "spec", "refused", "given", "worked", "asked", "shown", "form_of", "formed", "vals",
    "reform", "named", "name_if", "unclearable", "clearable", "lost", "submitted_by", "clocks", "open_after", "on_lines", "start", "next", "WorkedIn", "WorkedOut", "WorkedFields", "worked_out",
    "worked_none",
];

struct Gen<'x, 's, 'a> {
    m: &'x Machine<'s, 'a>,
    offset: i32,
}

/// What a request has that the glue does not lower yet.
fn not_yet(what: String) -> Text {
    Text::new(format!("申請の種類の {what} は、まだ kaname に下ろしません"), format!("the glue does not lower {what} yet"))
}

/// A file of the lowered deadlines, as the glue reads it.
struct Terms {
    /// The alias the glue reads it by (`terms`, `terms_1`).
    alias: String,
    /// Its name in the lowered directory (`terms.cal`).
    file: String,
    /// The calendar it counts in, by its alias as written.
    calendar: String,
    /// The day a stage starts, its input, and the days it takes.
    input: String,
    lo: i64,
    hi: i64,
    /// Each date: the request's name of it (`due_escalated`), the file's, and its time of day in
    /// minutes after midnight.
    dates: Vec<(String, String, u32)>,
}

/// A calendar business days are counted in: the function the glue counts them by
/// (`open_office`), the file of koyomi's it reads (none where no day is ever added to a day), the
/// days it counts from and the most it counts.
struct Open {
    /// The calendar's alias as written.
    calendar: String,
    /// The function of the glue (`open_office`).
    function: String,
    /// The alias the glue reads its file by (`open`, `open_1`), and the file's name.
    alias: String,
    file: String,
    lo: i64,
    hi: i64,
    most: u32,
    text: Option<String>,
}

/// The ways a table of `when late escalate by <table>` sends a late stage to, for each key a part
/// of it can have (the ways it names, the way it went to at the top, and the ways a table sent it
/// to before), as the machine reads them: `machine.Send { key, to }`, each way by its ASCII name.
fn sends(s: &crate::names::Scope, file: &crate::ast::File, t: &NamedTable, keys: &[String]) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    for k in keys {
        if !all.contains(k) {
            all.push(k.clone());
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < all.len() {
        let key = all[i].clone();
        let to = crate::org::listing::table_sends(s, file, t, &key);
        for w in &to {
            if !all.contains(w) {
                all.push(w.clone());
            }
        }
        if !to.is_empty() {
            let names: Vec<String> = to.iter().map(|w| quoted(&words::ascii_of_who(s, w))).collect();
            out.push(format!("machine.Send {{ key: {}, to: [{}] }}", quoted(&words::ascii_of_who(s, &key)), names.join(", ")));
        }
        i += 1;
    }
    out
}

impl Gen<'_, '_, '_> {
    /// A way to choose as the machine reads it (`machine.How.…`): the team, the person, the kind of
    /// team, the position, the master's row a field chooses, the field a person is chosen in, the
    /// flow of an agent and the way after `else`, each by the names the JSON takes.
    fn how(&self, how: &How, written: &str) -> Result<String, Text> {
        let (m, s) = (self.m, self.m.s);
        Ok(match how {
            How::ManagerOf => "machine.How.manager_of".to_string(),
            How::Team(t) => format!("machine.How.team {{ id: {} }}", quoted(&t.text)),
            How::Person(id, _) => format!("machine.How.person {{ id: {} }}", quoted(id)),
            How::HeadOf(kind) => format!("machine.How.head_of {{ team_kind: {} }}", quoted(m.names.team_kind(&kind.text))),
            How::Nearest(pos) => format!("machine.How.nearest {{ position: {} }}", quoted(&pos.text)),
            How::Lookup { path, by } if path.len() == 3 => {
                let shown = format!("{}.{}", path[0].text, path[1].text);
                let master = m.names.master(&shown).to_string();
                let column = m.names.master_field(&shown, &path[2].text).to_string();
                let by_ascii = s.field(&by.text).map(|f| f.name.ascii().to_string()).unwrap_or_else(|| by.text.clone());
                format!(
                    "machine.How.lookup {{ master: {}, field: {}, by: {}, shown: {}, column: {}, by_name: {} }}",
                    quoted(&master),
                    quoted(&column),
                    quoted(&by_ascii),
                    quoted(&shown),
                    quoted(&path[2].text),
                    quoted(&by.text)
                )
            }
            How::Lookup { .. } => return Err(not_yet(format!("the way to choose `{written}`, a lookup of no master's field"))),
            How::ChosenBy { previous, field } => {
                let ascii = s.field(&field.text).map(|f| f.name.ascii().to_string()).unwrap_or_else(|| field.text.clone());
                format!("machine.How.chosen_by {{ field: {}, shown: {}, previous: {previous} }}", quoted(&ascii), quoted(&field.text))
            }
            How::Agent { flow, otherwise } => {
                format!("machine.How.agent {{ flow: {}, otherwise: {} }}", quoted(m.names.used(&flow.text)), quoted(&words::ascii_of_who(s, &otherwise.text)))
            }
        })
    }

    /// The request as the machine reads it: its name and version, the UTC offset of its days, the
    /// days it can be used on, whether a requester may approve their own request, `same approver
    /// approves once`, the lines of its ticket, the order of its stages, the ways to choose an
    /// approver, each stage of the route with what its value names, the stages as the request writes
    /// them, and the offsets of the clocks that are not the request's (`ClockOffset`), as the
    /// reference reads them ([`Machine::clock_offset`]).
    fn spec(&self) -> Result<String, Text> {
        let (m, s) = (self.m, self.m.s);
        let f = s.file;
        let day = |d: Option<i64>| d.map(crate::lex::date_text).unwrap_or_else(|| "none".to_string());
        let (from, through) = (m.usable.and_then(|u| u.from.as_ref().map(|e| e.day)), m.usable.and_then(|u| u.through.as_ref().map(|e| e.day)));
        let once = match crate::org::listing::once(f) {
            crate::org::listing::Once::No => "off",
            crate::org::listing::Once::Before => "before",
            crate::org::listing::Once::Last => "last",
        };
        let (mut pause, mut back, mut resubmit, mut withdraw, mut retract) = ("none".to_string(), "none".to_string(), "none".to_string(), "none".to_string(), false);
        for l in f.ticket.iter().flat_map(|t| &t.lines) {
            match l {
                TicketLine::Pause { days, .. } => pause = words::grouped(i128::from(*days)),
                TicketLine::SendBack { to_requester, times, .. } => back = format!("machine.Back {{ to_requester: {to_requester}, times: {} }}", words::grouped(i128::from(*times))),
                TicketLine::Resubmit { days, .. } => resubmit = words::grouped(i128::from(*days)),
                TicketLine::Withdraw { until, .. } => {
                    let until = until.as_ref().map(|w| quoted(m.names.stage(&w.text))).unwrap_or_else(|| "none".to_string());
                    withdraw = format!("machine.Withdraw {{ until: {until} }}");
                }
                TicketLine::Retract { .. } => retract = true,
                _ => {}
            }
        }
        let order: Vec<String> = s.stages().unwrap_or_default().iter().map(|n| quoted(m.names.stage(&n.text))).collect();
        // the ways to choose: each once by its ASCII name, the request's own first
        let mut ways: Vec<String> = Vec::new();
        let mut seen: Vec<(String, String)> = Vec::new();
        for w in s.who_all() {
            let how = self.how(&w.who.how, &w.who.name.text)?;
            let ascii = w.who.name.ascii().to_string();
            if let Some((_, h)) = seen.iter().find(|(a, _)| *a == ascii) {
                if *h != how {
                    return Err(not_yet(format!("the way to choose `{}`, declared twice in two ways", w.who.name.text)));
                }
                continue;
            }
            seen.push((ascii.clone(), how.clone()));
            ways.push(format!("machine.Way {{ name: {}, written: {}, how: {how} }}", quoted(&ascii), quoted(&w.who.name.text)));
        }
        // each stage of the route once by its name and value, as the reference finds it (the first)
        let mut found: Vec<(String, String)> = Vec::new();
        let mut occs: Vec<String> = Vec::new();
        for o in m.paths.iter().flatten() {
            let stage = m.names.stage(&o.stage).to_string();
            let who = |w: &str| quoted(&words::ascii_of_who(s, w));
            let value = words::spec_value(s, o.spec).map(|x| x.0).unwrap_or_default();
            let (shape, names, keys): (String, Vec<String>, Vec<String>) = match o.spec {
                Spec::Who(ws) => ("machine.Shape.who".to_string(), ws.iter().map(|w| who(&w.text)).collect(), ws.iter().map(|w| w.text.clone()).collect()),
                Spec::Quorum(q, w, _) => {
                    let q = match q {
                        crate::ast::Quorum::All => "machine.Quorum.all".to_string(),
                        crate::ast::Quorum::AtLeast(n) => format!("machine.Quorum.at_least {{ n: {n} }}"),
                        crate::ast::Quorum::Percent(p) => format!("machine.Quorum.percent {{ p: {p} }}"),
                    };
                    (format!("machine.Shape.quorum {{ q: {q} }}"), vec![who(&w.text)], vec![w.text.clone()])
                }
                Spec::RuleOutput(p) => ("machine.Shape.rule_output".to_string(), Vec::new(), crate::org::resolve::rule_ways(s, p)),
                Spec::Circulate(w) => (format!("machine.Shape.circulate {{ way: {} }}", who(&w.text)), Vec::new(), Vec::new()),
                Spec::Note(w) => (format!("machine.Shape.note {{ flow: {} }}", quoted(m.names.used(&w.text))), Vec::new(), Vec::new()),
                Spec::Empty | Spec::Route(_) => continue,
            };
            if found.iter().any(|(st, v)| *st == stage && *v == value) {
                continue;
            }
            let (empty, top_way) = match &o.empty {
                WhenEmpty::Stay => ("machine.WhenEmpty.stay".to_string(), None),
                WhenEmpty::Skip => ("machine.WhenEmpty.skip".to_string(), None),
                WhenEmpty::Escalate(top) => {
                    let (t, way) = match top {
                        None => ("none".to_string(), None),
                        Some(AtTop::Stay) => ("machine.AtTop.stay".to_string(), None),
                        Some(AtTop::Skip) => ("machine.AtTop.skip".to_string(), None),
                        Some(AtTop::To(w)) => (format!("machine.AtTop.to {{ way: {} }}", who(&w.text)), Some(w.text.clone())),
                    };
                    (format!("machine.WhenEmpty.escalate {{ top: {t} }}"), way)
                }
            };
            let late = match o.late {
                None => "none".to_string(),
                Some(l) => {
                    let mut keys = keys.clone();
                    keys.extend(top_way);
                    let act = |a: &Late| -> String {
                        match a {
                            Late::Expire => "machine.LateAct.expire".to_string(),
                            Late::Stay => "machine.LateAct.stay".to_string(),
                            Late::Escalate(None) => "machine.LateAct.escalate { table: false, sends: [] }".to_string(),
                            Late::Escalate(Some(tw)) => {
                                let sent = l.file.tables.iter().find(|x| x.name.text == tw.text).map(|t| sends(s, l.file, t, &keys)).unwrap_or_default();
                                format!("machine.LateAct.escalate {{ table: true, sends: [{}] }}", sent.join(", "))
                            }
                        }
                    };
                    let after = l.then.map(act).unwrap_or_else(|| "none".to_string());
                    format!("machine.LateLine {{ first: {}, after: {after} }}", act(l.first))
                }
            };
            occs.push(format!("machine.Occ {{ stage: {}, value: {}, shape: {shape}, ways: [{}], empty: {empty}, late: {late} }}", quoted(&stage), quoted(&value), names.join(", ")));
            found.push((stage, value));
        }
        // the flows, by the names the JSON takes and as written, which the history names
        let flows: Vec<String> = s
            .uses_of(crate::ast::UseKind::Flow)
            .filter_map(|u| u.alias.as_ref())
            .map(|a| format!("machine.Flow {{ name: {}, shown: {} }}", quoted(a.ascii()), quoted(&a.text)))
            .collect();
        // the stages as the request writes them, where that is not the name the ticket gives them
        let named: Vec<String> = s
            .stages()
            .unwrap_or_default()
            .iter()
            .filter(|n| m.names.stage(&n.text) != n.text)
            .map(|n| format!("machine.StageName {{ name: {}, written: {} }}", quoted(m.names.stage(&n.text)), quoted(&n.text)))
            .collect();
        // the offsets of the clocks that are not the request's: every stage's, a stage's own where it
        // is not every stage's, a pause's and the requester's turn's
        let entry = |timer: &str, stage: Option<&str>, o: i32| format!("machine.ClockOffset {{ timer: {}, stage: {}, offset: {o} }}", quoted(timer), stage.map(quoted).unwrap_or_else(|| "none".to_string()));
        let mut offsets: Vec<String> = Vec::new();
        for timer in ["remind", "late"] {
            let every = m.clock_offset(timer, None);
            if every != m.offset {
                offsets.push(entry(timer, None, every));
            }
            for n in s.stages().unwrap_or_default() {
                let own = m.clock_offset(timer, Some(&n.text));
                if own != every {
                    offsets.push(entry(timer, Some(m.names.stage(&n.text)), own));
                }
            }
        }
        for timer in ["pause ends", "resubmit late"] {
            let o = m.clock_offset(timer, None);
            if o != m.offset {
                offsets.push(entry(timer, None, o));
            }
        }
        let request = quoted(f.name.ascii());
        // the events the request asks a reason with (`comment required for`), as the contract names them
        let reasoned: Vec<String> = f.ticket.iter().flat_map(|t| t.reasoned()).map(|o| quoted(o.event())).collect();
        let mut out = String::from("fn spec() -> machine.Spec total\n  description \"The request as the machine reads it\"\n");
        out.push_str(&format!(
            "  machine.Spec {{ request: {request}, version: {}, offset: {}, usable_from: {}, usable_to: {}, self_approval: {}, once: machine.Once.{once}, pause: {pause}, back: {back}, resubmit: {resubmit}, withdraw: {withdraw}, retract: {retract}, order: [{}], ways: [{}], occs: [{}], flows: [{}], named: [{}], offsets: [{}], comment_required: [{}] }}\n\n",
            f.version,
            self.offset,
            day(from),
            day(through),
            crate::org::listing::self_approval(f),
            order.join(", "),
            ways.join(", "),
            occs.join(", "),
            flows.join(", "),
            named.join(", "),
            offsets.join(", "),
            reasoned.join(", ")
        ));
        Ok(out)
    }

    /// The lowered deadlines: each file of them, with the dates it counts from the day a stage
    /// starts, by the request's names of them, and the days that day is read over. The alias of each
    /// after the first (`terms_1`) is taken from `names`, the names of the glue's `use` lines.
    fn terms(&self, names: &mut words::Names) -> Result<Vec<Terms>, Text> {
        let m = self.m;
        let mut out = Vec::new();
        let Some(dates) = &m.suite.dates else { return Ok(out) };
        for tf in m.lowered.files.iter() {
            let Part::Terms(calendar) = &tf.part else { continue };
            let facts = m.fs(|| dates.facts(&m.lowered.path(tf))).map_err(|_| not_yet(format!("the deadlines of `{}`, which do not read", tf.name)))?;
            let Some(started) = facts.inputs.first() else { continue };
            let alias = if out.is_empty() { "terms".to_string() } else { names.take(&format!("terms_{}", out.len())) };
            let dates = facts
                .functions
                .iter()
                .map(|f| {
                    let want = tf.renamed.iter().find(|(g, _)| *g == f.name).map(|(_, w)| w.clone()).unwrap_or_else(|| f.name.clone());
                    (want, f.name.clone(), f.at.unwrap_or(0))
                })
                .collect();
            out.push(Terms { alias, file: tf.name.clone(), calendar: calendar.clone(), input: started.name.clone(), lo: started.min, hi: started.max, dates });
        }
        Ok(out)
    }

    /// The calendars business days are counted in, each with the most it counts: a pause's and the
    /// requester's turn's (`pause`, `resubmit within`), and the calendar of a reminder counted apart
    /// from its deadline, after the stage goes up ([`crate::apart`]). The alias of each file after the
    /// first (`open_1`) and the function of each calendar (`open_office`) are taken from `names`, so
    /// that a calendar named `after` makes no second `open_after`.
    fn opens(&self, terms: &[Terms], names: &mut words::Names) -> Result<Vec<Open>, Text> {
        let (m, s) = (self.m, self.m.s);
        let first = m.calendars.first().map(|(a, _)| a.clone());
        let named = |c: Option<&crate::ast::Word>| c.map(|w| w.text.clone()).or_else(|| first.clone());
        let mut want: Vec<(String, u32)> = Vec::new();
        let mut add = |cal: Option<String>, n: u32| {
            let Some(cal) = cal else { return };
            match want.iter_mut().find(|(c, _)| *c == cal) {
                Some(w) => w.1 = w.1.max(n),
                None => want.push((cal, n)),
            }
        };
        for l in s.file.ticket.iter().flat_map(|t| &t.lines) {
            match l {
                TicketLine::Pause { days, calendar, .. } | TicketLine::Resubmit { days, calendar, .. } => add(named(calendar.as_ref()), *days),
                _ => {}
            }
        }
        for st in s.stages().unwrap_or_default() {
            if let Some(a) = crate::apart::of(s, first.as_deref(), &st.text)
                && a.ups > 0
            {
                add(named(a.remind.1), a.remind.0);
            }
        }
        if let Some(a) = crate::apart::stages(s, first.as_deref()).into_iter().find(|a| a.stage.is_none() && a.ups > 0) {
            add(named(a.remind.1), a.remind.0);
        }
        let cx = crate::lower::context(s, m.suite);
        let mut out = Vec::new();
        for (cal, most) in want {
            let Some(used) = cx.calendars.iter().find(|c| c.alias == cal) else { return Err(not_yet(format!("the business days of the calendar `{cal}`, which is not read"))) };
            let Some((_, facts)) = m.calendars.iter().find(|(a, _)| *a == cal) else { return Err(not_yet(format!("the business days of the calendar `{cal}`, which koyomi does not answer for"))) };
            let k = out.len();
            let (alias, file) = if k == 0 { ("open".to_string(), "open.cal".to_string()) } else { (names.take(&format!("open_{k}")), format!("open_{}.cal", used.file)) };
            let function = names.take(&format!("open_{}", used.file));
            // the days a stage starts on, as its deadlines are counted: the calendar's own, else the
            // first calendar of the deadlines' as this one reads them (DESIGN 7.5)
            let (lo, hi) = match (terms.iter().find(|t| t.calendar == cal), terms.first()) {
                (Some(t), _) => (t.lo, t.hi),
                (None, Some(t)) => {
                    let (before, after) = crate::usable::spread(m.offset_in(Some(&t.calendar)), m.offset_in(Some(&cal)));
                    (t.lo + before, t.hi + after)
                }
                (None, None) => (crate::usable::FIRST, crate::usable::FIRST + crate::usable::TURN - 1),
            };
            let lo = lo.max(facts.data.0);
            let mut hi = hi.min(facts.data.1);
            // no count runs past the calendar's data
            while hi >= lo && crate::apart::nth_open(facts, hi, most).is_none() {
                hi -= 1;
            }
            let room = OPEN_BUDGET / (i64::from(most) + 1);
            if hi - lo + 1 > room {
                hi = lo + room - 1;
            }
            let text = (most > 0 && hi >= lo).then(|| {
                let name = s.file.name.ascii();
                let suffix = if k == 0 { String::new() } else { format!("_{}", used.file) };
                let file = words::koyomi_file(&format!("{name}_open{suffix}"));
                format!(
                    "dates {file} v1\ndescription \"The business days of the calendar `{cal}` after a day, as the request {name} counts them: a pause, the requester's turn, a reminder counted apart from its deadline\"\nuse calendar \"{}\"\n\ninputs\n  begun : date  range >={} <={}\n  n     : int  range >=1 <={most}\n\ndate open_after = begun\n  + n business days\n",
                    used.path,
                    crate::lex::date_text(lo),
                    crate::lex::date_text(hi)
                )
            });
            out.push(Open { calendar: cal, function, alias, file, lo, hi, most, text });
        }
        Ok(out)
    }

    /// `open_after`: the `n`th business day after a day (the day itself for none), in the calendar a
    /// pause (`pause`) or the requester's turn (`resubmit`) counts in; and a function for each
    /// calendar, which a reminder counted apart reads too.
    fn open_after(&self, opens: &[Open]) -> String {
        let s = self.m.s;
        let mut out = String::new();
        for o in opens {
            let cal = &o.calendar;
            let refusal = format!(
                "err(machine.refusal(kind: machine.Kind.data, en: concat({}, text(day)), ja: concat({}, text(day), {})))",
                quoted(&format!("the business days of the calendar `{cal}` cannot be counted from ")),
                quoted(&format!("カレンダー `{cal}` の営業日を ")),
                quoted(" から数えられません")
            );
            out.push_str(&format!("fn {}(day: date, n: number) -> result[date, machine.Refusal] total\n  description \"The `n`th business day after the day in the calendar `{cal}`; the day itself for none\"\n", o.function));
            match &o.text {
                Some(_) => out.push_str(&format!(
                    "  if n = 0\n    ok(day)\n  else if day >= {} and day <= {} and n >= 1 and n <= {}\n    ok({}.open_after(begun: day, n: n))\n  else\n    {refusal}\n\n",
                    crate::lex::date_text(o.lo),
                    crate::lex::date_text(o.hi),
                    o.most,
                    o.alias
                )),
                None => out.push_str(&format!("  if n = 0\n    ok(day)\n  else\n    {refusal}\n\n")),
            }
        }
        let first = self.m.calendars.first().map(|(a, _)| a.clone());
        let mut arms = Vec::new();
        for l in s.file.ticket.iter().flat_map(|t| &t.lines) {
            let (key, cal) = match l {
                TicketLine::Pause { calendar, .. } => ("pause", calendar),
                TicketLine::Resubmit { calendar, .. } => ("resubmit", calendar),
                _ => continue,
            };
            let cal = cal.as_ref().map(|w| w.text.clone()).or_else(|| first.clone());
            let body = match cal.and_then(|c| opens.iter().find(|o| o.calendar == c)) {
                Some(o) => format!("{}(day: day, n: n)", o.function),
                None => "err(machine.refusal(kind: machine.Kind.data, en: \"the request has no calendar to count business days in\", ja: \"営業日を数えるカレンダーがありません\"))".to_string(),
            };
            arms.push(format!("    {} => {body}\n", quoted(key)));
        }
        out.push_str("fn open_after(key: string, day: date, n: number) -> result[date, machine.Refusal] total\n  description \"The `n`th business day after the day, in the calendar a pause (`pause`) or the requester's turn (`resubmit`) counts in; the day itself for none\"\n");
        let none = "err(machine.refusal(kind: machine.Kind.unread, en: concat(\"the request counts no business days for `\", key, \"`\"), ja: concat(\"この申請の種類は、`\", key, \"` の営業日を数えません\")))";
        if arms.is_empty() {
            out.push_str(&format!("  {none}\n\n"));
        } else {
            out.push_str("  match key\n");
            for a in arms {
                out.push_str(&a);
            }
            out.push_str(&format!("    else => {none}\n\n"));
        }
        out
    }

    /// The reminder and the deadline of a stage that starts on a day, after it has gone up for being
    /// late as many times as it has, as the reference counts them ([`Machine::clocks`]): the dates of
    /// the lowered deadlines, the stage's own (`<stage>_due`, `<stage>_remind`) or every stage's,
    /// then `_escalated` once and `_escalated_twice` after, each at its time of day in the UTC
    /// offset of the calendar it counts in; a reminder counted in another calendar than its deadline
    /// is counted after the stage goes up from the day the deadline before passed, in its own
    /// calendar. The reminder is none where the deadline is.
    fn clocks(&self, terms: &[Terms], opens: &[Open]) -> Result<String, Text> {
        let (m, s) = (self.m, self.m.s);
        let head = "fn clocks(stage: string, started: number, held: number, up: number) -> result[machine.Clocks, machine.Refusal] total\n  description \"The reminder and the deadline of a stage that started at the time, was paused for `held` business days and has gone up `up` times for being late: each calendar counts from its own day of the time, later by the days held\"\n";
        if terms.is_empty() {
            return Ok(format!("{head}  ok(machine.Clocks {{ remind: none, due: none }})\n\n"));
        }
        // the day each file of the deadlines counts from, by the file's place: `day`, `day_1`, …
        let day_of = |k: usize| if k == 0 { "day".to_string() } else { format!("day_{k}") };
        let all: Vec<(usize, &Terms, &String, &String, u32)> = terms.iter().enumerate().flat_map(|(k, t)| t.dates.iter().map(move |(w, f, at)| (k, t, w, f, *at))).collect();
        let has = |n: &str| all.iter().any(|(_, _, w, ..)| *w == n);
        let time = |n: &str| -> String {
            match all.iter().find(|(_, _, w, ..)| *w == n) {
                Some((k, t, _, f, at)) => format!(
                    "machine.time_in(x: machine.instant(day: {}.{f}({}: {}), minutes: {}, offset: {}))",
                    t.alias,
                    t.input,
                    day_of(*k),
                    words::grouped(i128::from(*at)),
                    m.offset_in(Some(&t.calendar))
                ),
                None => "none".to_string(),
            }
        };
        let first = m.calendars.first().map(|(a, _)| a.clone());
        // the plan of each stage (and of a stage of no name the request writes: every stage's)
        let mut plans: Vec<(Vec<String>, String)> = Vec::new();
        let stages: Vec<(Option<String>, String)> = s.stages().unwrap_or_default().iter().map(|n| (Some(n.text.clone()), m.names.stage(&n.text).to_string())).chain(std::iter::once((None, String::new()))).collect();
        for (written, ascii) in &stages {
            let (own_due, own_remind) = (format!("{ascii}_due"), format!("{ascii}_remind"));
            let own = written.is_some() && (has(&own_due) || has(&own_remind));
            let due_name = if written.is_some() && has(&own_due) { own_due.clone() } else { "due".to_string() };
            let due = [time(&due_name), time(&format!("{due_name}_escalated")), time(&format!("{due_name}_escalated_twice"))];
            let remind_first = time(if written.is_some() && has(&own_remind) { &own_remind } else { "remind" });
            let remind_base = if own { own_remind.clone() } else { "remind".to_string() };
            let apart = match written {
                Some(w) => crate::apart::of(s, first.as_deref(), w),
                None => crate::apart::stages(s, first.as_deref()).into_iter().find(|a| a.stage.is_none()),
            };
            // each level the stage has gone up to: its deadline, and its reminder (none where the
            // deadline is)
            let clocks = |remind: &str, due: &str| format!("ok(machine.Clocks {{ remind: {remind}, due: {due} }})");
            let mut level = Vec::new();
            for (k, (after, before)) in [("", ""), ("_escalated", ""), ("_escalated_twice", "_escalated")].into_iter().enumerate() {
                let due_k = &due[k];
                if due_k == "none" {
                    level.push(clocks("none", "none"));
                    continue;
                }
                if k == 0 {
                    level.push(clocks(&remind_first, due_k));
                    continue;
                }
                let lowered = time(&format!("{remind_base}{after}"));
                if lowered != "none" {
                    level.push(clocks(&lowered, due_k));
                    continue;
                }
                let counted = apart.as_ref().and_then(|a| {
                    let cal = a.remind.1.map(|w| w.text.clone()).or_else(|| first.clone())?;
                    let o = opens.iter().find(|o| o.calendar == cal)?;
                    Some((o.function.clone(), a.remind.0, a.remind.2, m.offset_in(Some(&cal))))
                });
                let went_up = time(&format!("{due_name}{before}"));
                match counted {
                    Some((f, n, at, own)) if went_up != "none" => level.push(format!(
                        "match {went_up}\n  none => {}\n  some w =>\n    match {f}(day: machine.day_at(at: w, offset: {own}), n: {n})\n      err e => err(e)\n      ok d => {}",
                        clocks("none", due_k),
                        clocks(&format!("machine.time_in(x: machine.instant(day: d, minutes: {}, offset: {own}))", words::grouped(i128::from(at))), due_k)
                    )),
                    _ => level.push(clocks("none", due_k)),
                }
            }
            let arm = |x: &str| if x.contains('\n') { format!("\n{}", indent(x, 6)) } else { format!(" {x}") };
            let body = format!("  match up\n    0 =>{}\n    1 =>{}\n    else =>{}\n", arm(&level[0]), arm(&level[1]), arm(&level[2]));
            let key = match written {
                Some(_) => ascii.clone(),
                None => String::new(),
            };
            match plans.iter_mut().find(|(_, b)| *b == body) {
                Some((names, _)) => names.push(key),
                None => plans.push((vec![key], body)),
            }
        }
        let mut out = String::new();
        let mut arms = Vec::new();
        let mut every = String::new();
        let days: Vec<String> = terms.iter().enumerate().map(|(k, t)| format!("{}: date range >={} <={}", day_of(k), crate::lex::date_text(t.lo), crate::lex::date_text(t.hi))).collect();
        for (i, (names, body)) in plans.iter().enumerate() {
            let fname = format!("clocks_{i}");
            out.push_str(&format!(
                "fn {fname}({}, up: number range >=0 <=255) -> result[machine.Clocks, machine.Refusal] total\n  description \"The reminder and the deadline of the stages {}\"\n{body}\n",
                days.join(", "),
                if names.iter().any(String::is_empty) { "the request writes no deadline of their own for".to_string() } else { names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ") }
            ));
            if names.iter().any(String::is_empty) {
                every = fname.clone();
            }
            for n in names.iter().filter(|n| !n.is_empty()) {
                arms.push((n.clone(), fname.clone()));
            }
        }
        let given: Vec<String> = (0..terms.len()).map(|k| format!("{0}: {0}", day_of(k))).collect();
        let call = |f: &str| format!("{f}({}, up: min(max(up, 0), 255))", given.join(", "));
        out.push_str(head);
        // the time the clocks count from: the time the stage started, later by the days the business
        // days it was paused for take in the pause's calendar; each file's day is its calendar's day
        // of that time (DESIGN 7.5)
        let at = "min(max(started, 0), 253_402_300_799)";
        let pause = s.file.ticket.iter().flat_map(|t| &t.lines).find_map(|l| if let TicketLine::Pause { calendar, .. } = l { Some(calendar.as_ref().map(|w| w.text.clone()).or_else(|| m.calendars.first().map(|(a, _)| a.clone()))) } else { None }).flatten();
        let moved = match pause.as_ref().and_then(|cal| opens.iter().find(|o| &o.calendar == cal)) {
            Some(o) => format!("machine.moved_time(spec: spec(), at: {at}, held: held, open: {})", o.function),
            None => format!("machine.unmoved_time(at: {at}, held: held)"),
        };
        out.push_str(&format!("  let moved = try {moved}\n"));
        for (k, t) in terms.iter().enumerate() {
            out.push_str(&format!("  let {} = machine.day_at(at: min(max(moved, 0), 253_402_300_799), offset: {})\n", day_of(k), m.offset_in(Some(&t.calendar))));
        }
        let within: Vec<String> = terms.iter().enumerate().map(|(k, t)| format!("{0} >= {1} and {0} <= {2}", day_of(k), crate::lex::date_text(t.lo), crate::lex::date_text(t.hi))).collect();
        out.push_str(&format!("  if {}\n", within.join(" and ")));
        let rest: Vec<&(String, String)> = arms.iter().filter(|(_, f)| *f != every).collect();
        if rest.is_empty() {
            out.push_str(&format!("    {}\n", call(&every)));
        } else {
            out.push_str("    match stage\n");
            for (n, f) in rest {
                out.push_str(&format!("      {} => {}\n", quoted(n), call(f)));
            }
            out.push_str(&format!("      else => {}\n", call(&every)));
        }
        // the refusal says the day of the first file that does not count from it
        let said = |d: &str| format!("err(machine.refusal(kind: machine.Kind.data, en: concat(\"the deadlines of the stage `\", stage, \"` cannot be counted from \", text({d})), ja: concat(\"ステップ `\", stage, \"` の期限を \", text({d}), \" から数えられません\")))");
        let mut refusal = said(&day_of(terms.len() - 1));
        for k in (0..terms.len() - 1).rev() {
            let (d, t) = (day_of(k), &terms[k]);
            refusal = format!("if {d} >= {} and {d} <= {}\n{}\nelse\n  {}", crate::lex::date_text(t.lo), crate::lex::date_text(t.hi), indent(&refusal, 2), said(&d));
        }
        out.push_str(&format!("  else\n{}\n\n", indent(&refusal, 4)));
        Ok(out)
    }

    /// `submitted_by`: whether the requester may submit the request (`submitted by`, DESIGN 5.13): one
    /// of the groups before `except` has them, and none after it does. A request with no such line
    /// takes every person of the organization.
    fn submitters(&self) -> String {
        let s = self.m.s;
        let head = "fn submitted_by(org: machine.Org, person: string) -> result[bool, machine.Refusal] total\n  description \"Whether the person may submit the request (`submitted by`)\"\n";
        let Some(l) = crate::org::submit::of(s.file) else { return format!("{head}  ok(true)\n\n") };
        let group = |g: &crate::ast::Group| -> String {
            use crate::ast::Group;
            let within = |t: &Option<crate::ast::Word>| t.as_ref().map(|t| format!("machine.under(org: org, person: person, team: {})", quoted(&t.text)));
            match g {
                Group::Everyone { within: w, .. } => within(w).unwrap_or_else(|| "true".to_string()),
                Group::Team(t) => format!("machine.member_of(org: org, person: person, team: {})", quoted(&t.text)),
                Group::Position { position, higher, within: w } => {
                    let at = format!("machine.at_position(org: org, person: person, position: {}, higher: {higher})", quoted(&position.text));
                    match within(w) {
                        Some(u) => format!("({at} and {u})"),
                        None => at,
                    }
                }
                Group::Person(id, _) => format!("person = {}", quoted(id)),
            }
        };
        let any = |gs: &[crate::ast::Group]| -> String {
            if gs.is_empty() {
                "false".to_string()
            } else {
                format!("({})", gs.iter().map(group).collect::<Vec<_>>().join(" or "))
            }
        };
        let name = std::path::Path::new(s.path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let said = format!("concat(machine.who_en(org: org, id: person), {}, text(org.version), {})", quoted(&format!(" may not submit {name} in the organization from ")), quoted(&format!(" (`submitted by {}`)", l.shown())));
        let said_ja = format!("concat(machine.who_ja(org: org, id: person), \"は、\", text(org.version), {})", quoted(&format!(" からの組織では {name} を出せません（`submitted by {}`）", l.shown())));
        format!(
            "{head}  match machine.in_org(org: org, id: person) and {} and not {}\n    true => ok(true)\n    false => err(machine.refusal(kind: machine.Kind.submit, en: {said}, ja: {said_ja}))\n\n",
            any(l.groups),
            any(l.except)
        )
    }

    /// What a host calls: `start`, for a request made, and `next`, for each event of a ticket.
    fn start_and_next(&self) -> String {
        let offset = self.offset;
        format!(
            "fn start(submit: machine.Submit[Given], organization: machine.Org, now: number range >=0 <=253_402_300_799) -> result[machine.Moved[Form], machine.Refusal] total
  description \"A request is made: the ticket at its first stage with what is to do, or why it is not made\"
  let day = try machine.on_day(org: organization, now: now, offset: {offset})
  let usable = try machine.usable_on(spec: spec(), day: day)
  let made_by = try machine.may_submit(org: organization, requester: submit.requester, by: submit.by)
  let may = try submitted_by(org: organization, person: submit.requester)
  let comment = try machine.said(c: submit.comment)
  let form = try given(g: submit.form, org: organization)
  let stages = try formed(form: form)
  machine.start(spec: spec(), id: submit.id, requester: submit.requester, made_by: made_by, form: form, vals: vals(form: form), stages: stages, org: organization, now: now, comment: comment, attempt: submit.attempt, clocks: clocks, open_after: open_after, on: on_lines)

fn next(ticket: machine.Ticket[Form], event: machine.Event[Given], organization: machine.Org, now: number range >=0 <=253_402_300_799) -> result[machine.Moved[Form], machine.Refusal] total
  description \"One event of a ticket: the ticket after it with what is to do, or why the event is not taken\"
  machine.next(spec: spec(), t: ticket, ev: event, org: organization, now: now, vals: vals(form: ticket.form), clocks: clocks, open_after: open_after, on: on_lines, reform: reform)
"
        )
    }
}

/// Text lines, each moved right by `pad` spaces.
fn indent(text: &str, pad: usize) -> String {
    text.lines().map(|l| if l.is_empty() { String::new() } else { format!("{}{l}", " ".repeat(pad)) }).collect::<Vec<_>>().join("\n")
}

/// The enum value of an ending, as the machine names it.
fn ending_word(e: crate::ast::Ending) -> &'static str {
    use crate::ast::Ending;
    match e {
        Ending::Submitted => "submitted",
        Ending::Approved => "approved",
        Ending::SentBack => "sent_back",
        Ending::Rejected => "rejected",
        Ending::Withdrawn => "withdrawn",
        Ending::Expired => "expired",
    }
}

/// A text as kaname writes it: between `"`, which it holds no escape for, so that a `"` in it is
/// written `'` and a line's end a space.
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "'").replace('\n', " "))
}
