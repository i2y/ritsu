//! What `ritsu gen --target requests` writes once for a project, into `<out>/requests/` (DESIGN
//! 12.2, 12.4): the kinds of request apart from any language, for a server, a screen and an agent
//! to be made from. Each kind is read once from its check ([`Kind::of`]), and [`files`] writes every
//! file from what was read:
//!
//! - `types/<name>.json` ([`types`]): what the kind is: its form, its stages and ways, how its
//!   tickets end, the events a person makes on them, who may submit it, the days it can be used on.
//! - `openapi.json` ([`openapi`]): the server's API, an operation for each event a person makes.
//! - `mcp/tools.json` ([`mcp`]): the tools an agent is given, each an operation of the API.
//! - `organization/schema.json` and `organization/columns.md` ([`orgschema`]): the data of the
//!   organization and of the masters the kinds read, as a JSON Schema and as the columns of their
//!   CSV files, and the part of the organization `start` and `next` are given.
//! - `vectors/<name>.jsonl`: the kind's vectors (DESIGN 10.4), as `ritsu ringi vectors` writes them;
//!   `vectors/worked/<name>.jsonl`, those of its entry `worked` ([`crate::engine::worked::vectors`]),
//!   a directory below, so that what reads `vectors/*.jsonl` as the kinds' vectors finds no other;
//!   and `organization/find.jsonl`, the vectors of finding a person, a team or a row of a master by
//!   words ([`crate::find`]), the same for every project.
//!
//! The screens (`a2ui/`, DESIGN 13) are [`crate::screens`]'s, made from the same kinds when `ritsu
//! gen` gives the catalog's ID. Every name is the ASCII one with the written one beside it
//! (`written`, DESIGN 16.2), and an object has the same keys whatever the kind: what a kind does not
//! have is null, or an empty list. The words are English, whatever `--lang` asks for, and every file
//! but the vectors begins with the head of what ritsu generates: the same input writes the same
//! bytes.

mod mcp;
mod openapi;
mod orgschema;
mod types;

use crate::ast::{Field, File, Group, LabelKey, Span, TicketLine, TypeExpr, UseKind};
use crate::check::Outcome;
use crate::engine::Machine;
use crate::names::Scope;
use crate::org::Org;
use ritsu_base::json::Json;
use ritsu_base::text::Lang;
use std::path::{Path, PathBuf};

/// A name in ASCII, the name as written, its text, and its texts in the other languages the file
/// writes (`labels ja`), each with its language.
#[derive(Clone, Debug)]
pub struct Named {
    pub name: String,
    pub written: String,
    pub label: String,
    pub labels: Vec<(String, String)>,
}

impl Named {
    fn json(&self) -> Json {
        Json::obj([("name", Json::str(&self.name)), ("written", Json::str(&self.written)), ("label", Json::str(&self.label)), ("labels", labels_json(&self.labels))])
    }
}

/// The condition a field is shown or asked for on: as written, the field of what the generated
/// code's `worked` answers that says whether it holds (`trip_to_shown`), and the fields of the form
/// it reads, through the computations it reads ([`crate::lower::form_reads`]): what it changes with.
#[derive(Clone, Debug)]
pub struct Cond {
    pub written: String,
    pub worked: String,
    pub reads: Vec<String>,
}

impl Cond {
    fn json(c: &Option<Cond>) -> Json {
        c.as_ref().map(|c| Json::obj([("condition", Json::str(&c.written)), ("worked", Json::str(&c.worked)), ("reads", Json::arr(c.reads.iter().map(Json::str)))])).unwrap_or(Json::Null)
    }
}

/// A field of the form (or of a master's row), with what the files say of it.
#[derive(Clone, Debug)]
pub struct Fld {
    pub named: Named,
    /// The line its name is written on.
    pub line: usize,
    pub ty: TypeExpr,
    /// Its type with the names in it in ASCII (`choice from m.accounts`), and as written.
    pub type_ascii: String,
    pub type_written: String,
    /// The range of a number as the integers on the wire, of a date as its day; None at an open end.
    pub min: Option<i128>,
    pub max: Option<i128>,
    /// The limits on a text's length, or on the count of a `rows` field's rows.
    pub count_min: Option<i128>,
    pub count_max: Option<i128>,
    /// The values of a `choice`, or of the rule's enum a `choice from` takes; none for a master's
    /// rows, which the data decide.
    pub choices: Vec<Named>,
    pub optional: bool,
    pub computed: bool,
    /// For a computed field, the fields of the form its computation reads, through the computations
    /// it reads ([`crate::lower::form_reads`]): what its value changes with. Empty for any other.
    pub reads: Vec<String>,
    /// Whether the screens work it out as the form is filled in (`functions.js`, DESIGN 13.7): a
    /// computed number of the form. A computed date, and a computed field of a row, the machine
    /// alone works out.
    pub worked_on_screen: bool,
    pub approvers_only: bool,
    /// The calendar whose open days the date is (`open in`), by its alias as written.
    pub open_in: Option<String>,
    pub shown_when: Option<Cond>,
    pub required_when: Option<Cond>,
    /// The fields of a row.
    pub fields: Vec<Fld>,
}

impl Fld {
    /// Whether the requester has to fill it in whatever the other values are.
    pub fn required(&self) -> bool {
        !self.optional && self.given() && self.shown_when.is_none() && self.required_when.is_none()
    }

    /// Whether the requester gives it: it is no computation, nor an approver's.
    pub fn given(&self) -> bool {
        !self.computed && !self.approvers_only
    }

    /// Whether the requester fills it in wherever the form shows it, as the machine asks for it
    /// (DESIGN 4.2): a field they give with no `optional` and no `required when` (one shown on a
    /// condition is asked for while it is shown); of a row's fields (`in_rows`), each they give but
    /// an `optional` one and an attachment, which a row may leave empty (DESIGN 4.3).
    pub fn asked(&self, in_rows: bool) -> bool {
        match in_rows {
            true => self.given() && !self.optional && self.ty != TypeExpr::Attachment,
            false => self.given() && !self.optional && self.required_when.is_none(),
        }
    }

    /// A date's end as its text, a number's as itself.
    fn end(&self, n: Option<i128>) -> Json {
        match (&self.ty, n) {
            (_, None) => Json::Null,
            (TypeExpr::Date, Some(d)) => i64::try_from(d).map(|d| Json::str(crate::lex::date_text(d))).unwrap_or(Json::Null),
            (_, Some(n)) => Json::int(n),
        }
    }
}

/// A master the kinds read: by the masters file's name and the master's (`company_masters`,
/// `vendors`), with the name a request reads it by (`m.vendors`), its key and its fields.
#[derive(Clone, Debug)]
pub struct MasterOf {
    pub named: Named,
    pub masters: String,
    pub master: String,
    /// The masters' file, from the root.
    pub file: String,
    pub key: Option<String>,
    /// The field that names a row, as a screen shows the row (DESIGN 12.2): the first text field
    /// that is not the key; None where there is none.
    pub name_field: Option<String>,
    pub fields: Vec<Fld>,
    /// The CSV of its rows, from the root.
    pub data: Option<String>,
}

/// A flow a kind of request uses (DESIGN 12.2): its role, the alias the request reads it by, its
/// file, where it is asked, and what dandori declares of it.
#[derive(Clone, Debug)]
pub struct FlowOf {
    /// `launch` (started once a ticket ends), `agent` (a way to choose an approver) or `summary` (a
    /// stage `note from`).
    pub role: String,
    /// The alias, in ASCII and as written.
    pub alias: String,
    pub written: String,
    /// Its file, from the root, and as the run reaches it.
    pub file: String,
    pub path: PathBuf,
    /// Where it is asked: the endings a launch starts on, the way to choose an agent is, the stage
    /// of a summary, in ASCII.
    pub at: Vec<String>,
    /// What dandori declares of it; None when the run does not read flows.
    pub declared: Option<FlowFacts>,
}

/// What dandori declares of a flow: the workflow's name and version, its type on Temporal (which is
/// also its task queue), and its outputs in the order written, each with its type as dandori
/// writes it and whether it may be absent.
#[derive(Clone, Debug)]
pub struct FlowFacts {
    pub workflow: String,
    pub version: u32,
    pub workflow_type: String,
    pub outputs: Vec<(String, String, bool)>,
}

/// The organization a kind reads: its file, its name and title, its positions from the lowest and
/// its kinds of team from the widest.
#[derive(Clone, Debug)]
pub struct OrgOf {
    pub file: String,
    pub name: String,
    pub title: String,
    pub positions: Vec<Named>,
    pub team_kinds: Vec<Named>,
    /// Where the versions' directories are, from the root.
    pub data: Option<String>,
}

/// A kind of request, as the files describe it.
#[derive(Clone, Debug)]
pub struct Kind {
    /// Its file, from the project's root.
    pub file: String,
    pub named: Named,
    pub version: u32,
    /// The language it is written in, `en` or `ja` ([`written_in`]): the one its titles and the names
    /// it shows are in (`title`, each `label`), beside which `labels` adds the others.
    pub language: &'static str,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub effective_from: Option<String>,
    /// The days it can be used on, and the data that decide them (DESIGN 9.6), as `ringi api`
    /// writes them; null when nothing ends them.
    pub usable: Json,
    /// The UTC offset its days are read in, in minutes east of UTC (DESIGN 7.5): the machine's, by
    /// which a server finds the day of an event.
    pub utc_offset: i32,
    /// Who may submit it (`submitted by`, DESIGN 5.13): the groups, and the ones left out of them;
    /// None for everyone in the organization.
    pub submitted_by: Option<(Vec<Json>, Vec<Json>)>,
    /// Whether its requester may approve their own request (`self approval allowed`, DESIGN 5.5):
    /// what a server gives the gate as a stage's `self_approval` (DESIGN 12.4).
    pub self_approval: bool,
    pub form: Vec<Fld>,
    pub stages: Vec<Named>,
    pub ways: Vec<Named>,
    /// How its tickets end: of `approved`, `rejected`, `withdrawn` and `expired`.
    pub endings: Vec<String>,
    /// The events a person makes on its tickets, by their names in the contract (`send_back`).
    pub events: Vec<&'static str>,
    /// The events it asks a reason with (`comment required for`, DESIGN 7.2), by their names in the
    /// contract (`reject`, `send_back`), in the order written: at every stage, which
    /// `types/<name>.json` writes as each stage's `comment_required`.
    pub reasoned: Vec<&'static str>,
    /// The fields an approver may edit besides the approvers' own (`approvers edit`), in ASCII.
    pub approvers_edit: Vec<String>,
    /// Its vectors, a line each.
    pub vectors: String,
    /// The vectors of its entry `worked`, a line each ([`crate::engine::worked::vectors`]).
    pub worked: String,
    pub organization: Option<OrgOf>,
    pub masters: Vec<MasterOf>,
    /// The flows it uses, in the order the ticket asks them (launches last).
    pub flows: Vec<FlowOf>,
    /// What its screens need besides (DESIGN 13).
    pub screen: crate::screens::Screen,
}

/// The events of a person, in the order the files list them, by their names in the contract.
pub const EVENTS: [&str; 13] = ["approve", "reject", "send_back", "retract", "pause", "resume", "withdraw", "resubmit", "discard", "edit", "add_approver", "hand_over", "comment"];

/// The four ways a ticket ends.
const ENDINGS: [&str; 4] = ["approved", "rejected", "withdrawn", "expired"];

impl Kind {
    /// The kind of the checked request `o`, on the machine `m` with the organization `org`, the
    /// organization's file `org_file` beside it; its paths written from `root`.
    pub fn of(m: &Machine, org: &Org, o: &Outcome, root: &Path, org_file: Option<(&File, PathBuf)>) -> Kind {
        Kind::made(m, org, o, root, org_file, true)
    }

    /// [`Kind::of`], with no vectors when `vectors` is false (none of its scenarios run): what the
    /// server's API is written from, which reads none, as ritsu's check across the borders asks
    /// for it (X20, DESIGN 9.3).
    pub fn made(m: &Machine, org: &Org, o: &Outcome, root: &Path, org_file: Option<(&File, PathBuf)>, vectors: bool) -> Kind {
        let s = m.s;
        let f = s.file;
        let rel = |p: &Path| ritsu_base::paths::from_root(root, p).unwrap_or_else(|| p.display().to_string());
        let screen = crate::screens::Screen::of(m);
        let mut form: Vec<Fld> = f.form.iter().map(|x| field(m, f, s.src, x, &[], true)).collect();
        // what the screens work out: a computed number of the form that `values.rule` gives
        for x in form.iter_mut() {
            x.worked_on_screen = x.computed && matches!(x.ty, TypeExpr::Num(_)) && screen.worked.as_ref().is_some_and(|w| w.outputs.iter().any(|(o, _)| *o == x.named.name));
        }
        // a stage and a way by its title in the file's language, and its labels: of a shared route,
        // what the route's `labels` give where the file gives none (DESIGN 4.8, 5.11)
        let word = |n: &crate::ast::Name| crate::ast::Word { text: n.text.clone(), span: n.span };
        let stages = s
            .stages()
            .unwrap_or_default()
            .into_iter()
            .map(|n| {
                let (title, labels) = crate::screens::titled(s, &LabelKey::Stage(word(n)), &n.text);
                named(&n.text, n.ascii(), &title, labels)
            })
            .collect();
        let ways = s
            .who_all()
            .iter()
            .map(|w| &w.who.name)
            .map(|n| {
                let (title, labels) = crate::screens::titled(s, &LabelKey::Who(word(n)), &n.text);
                named(&n.text, n.ascii(), &title, labels)
            })
            .collect();
        let lines: &[TicketLine] = f.ticket.as_ref().map(|t| t.lines.as_slice()).unwrap_or_default();
        let approvers_edit: Vec<String> = lines
            .iter()
            .filter_map(|l| if let TicketLine::ApproversEdit { fields, .. } = l { Some(fields) } else { None })
            .flatten()
            .map(|w| s.field(&w.text).map(|x| x.name.ascii().to_string()).unwrap_or_else(|| w.text.clone()))
            .collect();
        let ops: Vec<&str> = o.ticket.as_ref().map(|t| t.operations.iter().map(|op| op.name.as_str()).collect()).unwrap_or_default();
        let edits = !approvers_edit.is_empty() || form.iter().any(|x| x.approvers_only);
        let events = EVENTS
            .into_iter()
            .filter(|e| match *e {
                "resubmit" | "discard" => ops.contains(&"send_back"),
                "resume" => ops.contains(&"pause"),
                "edit" => edits && ops.contains(&"approve"),
                // every kind's tickets take a comment (DESIGN 7.2)
                "comment" => true,
                other => ops.contains(&other),
            })
            .collect();
        let endings = o.ticket.as_ref().map(|t| ENDINGS.iter().filter(|e| t.endings.iter().any(|x| x == *e)).map(|e| e.to_string()).collect()).unwrap_or_default();
        // the vectors, as `ritsu ringi vectors` makes them: the scenarios first, each as written
        let rans: Vec<_> = if vectors { f.scenarios.iter().map(|sc| crate::scenario::run(m, org, sc)).collect() } else { Vec::new() };
        let vectors = if !vectors || rans.iter().any(|r| r.stop.is_some()) {
            String::new()
        } else {
            crate::vectors::make(m, org, &rans).iter().map(|v| format!("{}\n", v.to_json(m).compact())).collect()
        };
        // the vectors of the entry `worked`, from the forms the vectors submit and edit
        let worked = crate::engine::worked::vectors(m, &vectors);
        let organization = org_file.map(|(of, p)| OrgOf {
            file: rel(&p),
            name: of.name.ascii().to_string(),
            title: of.title.clone().unwrap_or_else(|| of.name.text.clone()),
            positions: of.positions.iter().map(|x| named(&x.name.text, x.name.ascii(), x.title.as_deref().unwrap_or(&x.name.text), Vec::new())).collect(),
            team_kinds: of.team_kinds.iter().map(|x| named(&x.name.text, x.name.ascii(), x.title.as_deref().unwrap_or(&x.name.text), Vec::new())).collect(),
            data: of.data.as_ref().map(|(d, _)| rel(&crate::org::data::beside(&p.display().to_string(), d))),
        });
        let positions = organization.as_ref().map(|x| x.positions.as_slice()).unwrap_or_default();
        let submitted_by = crate::org::submit::of(f).map(|l| (l.groups.iter().map(|g| group_json(g, positions)).collect(), l.except.iter().map(|g| group_json(g, positions)).collect()));
        let mut masters = Vec::new();
        for u in s.uses_of(UseKind::Masters) {
            let Some(alias) = &u.alias else { continue };
            let Some(mf) = s.ringi(&alias.text, UseKind::Masters) else { continue };
            let Ok(path) = crate::names::beside(s.path, &u.path) else { continue };
            for ms in &mf.masters {
                let key = ms.key.as_ref().map(|k| ms.fields.iter().find(|x| x.name.text == k.text).map(|x| x.name.ascii().to_string()).unwrap_or_else(|| k.text.clone()));
                let name_field = ms
                    .fields
                    .iter()
                    .find(|x| matches!(x.ty, TypeExpr::Text | TypeExpr::LongText) && ms.key.as_ref().is_none_or(|k| k.text != x.name.text))
                    .map(|x| x.name.ascii().to_string());
                masters.push(MasterOf {
                    named: named(&format!("{}.{}", alias.text, ms.name.text), &format!("{}.{}", alias.ascii(), ms.name.ascii()), ms.title.as_deref().unwrap_or(&ms.name.text), Vec::new()),
                    masters: mf.name.ascii().to_string(),
                    master: ms.name.ascii().to_string(),
                    file: rel(&path),
                    key,
                    name_field,
                    fields: ms.fields.iter().map(|x| field(m, mf, "", x, &[], false)).collect(),
                    data: ms.data.as_ref().map(|(d, _)| rel(&crate::org::data::beside(&path.display().to_string(), d))),
                });
            }
        }
        let flows = o
            .ticket
            .as_ref()
            .map(|t| {
                t.flows
                    .iter()
                    .map(|fl| FlowOf {
                        role: fl.kind.clone(),
                        alias: o.names.used(&fl.alias).to_string(),
                        written: fl.alias.clone(),
                        file: rel(&fl.flow),
                        path: fl.flow.clone(),
                        at: fl
                            .at
                            .iter()
                            .map(|x| match fl.kind.as_str() {
                                "agent" => o.names.way(x).to_string(),
                                "summary" => o.names.stage(x).to_string(),
                                _ => x.clone(),
                            })
                            .collect(),
                        declared: None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Kind {
            file: rel(Path::new(s.path)),
            named: named(&f.name.text, f.name.ascii(), f.title.as_deref().unwrap_or(&f.name.text), labels(f, |k| matches!(k, LabelKey::Request))),
            version: f.version,
            language: written_in(f),
            description: f.description.as_ref().map(|d| d.0.clone()),
            owner: f.owner.as_ref().map(|w| w.text.clone()),
            effective_from: f.effective_from.map(|d| crate::lex::date_text(d.0)),
            usable: o.usable.as_ref().map(|u| u.to_json(&o.names, root, Lang::En)).unwrap_or(Json::Null),
            utc_offset: m.offset,
            submitted_by,
            self_approval: crate::org::listing::self_approval(f),
            form,
            stages,
            ways,
            endings,
            events,
            reasoned: f.ticket.as_ref().map(|t| t.reasoned().into_iter().map(crate::ast::Reasoned::event).collect()).unwrap_or_default(),
            approvers_edit,
            vectors,
            worked,
            organization,
            masters,
            flows,
            screen,
        }
    }

    /// The fields the requester gives, in the order written.
    pub fn given(&self) -> impl Iterator<Item = &Fld> {
        self.form.iter().filter(|f| f.given())
    }

    /// The fields an approver gives: the approvers' own, and those `approvers edit` names.
    pub fn edited(&self) -> impl Iterator<Item = &Fld> {
        self.form.iter().filter(|f| f.approvers_only || self.approvers_edit.contains(&f.named.name))
    }

    pub fn has(&self, event: &str) -> bool {
        self.events.contains(&event)
    }
}

/// The language of the screens (`en` or `ja`) the file `f` is written in: the one of the two it
/// writes no `labels` for where it writes them for the other alone (`labels ja` in a file written in
/// English, DESIGN 4.8); else Japanese where the request's name is written apart from its ASCII name
/// (`request 経費(expense)`), English where it is written in ASCII (`request expense`).
pub fn written_in(f: &File) -> &'static str {
    let declared: Vec<&str> = f.labels.iter().map(|b| b.lang.text.as_str()).collect();
    match crate::screens::LANGS.iter().filter(|l| !declared.contains(l)).collect::<Vec<_>>().as_slice() {
        [one] => one,
        _ if f.name.text != f.name.ascii() => "ja",
        _ => "en",
    }
}

/// The language a rule writes the display names of its enums' values in (rulec's §15.211: one, in
/// the words of the rule): `ja` for a rule named in other than ASCII (`rule 権限表 v1`), else `en`,
/// as a request that declares no language is read (DESIGN 4.8). A request takes a value's display
/// name as its title only in that language.
pub fn rule_written_in(r: &ritsu_ports::RuleFacts) -> &'static str {
    if r.rule.is_ascii() { "en" } else { "ja" }
}

/// The organization file the request reads, as the run reaches it: through its own `use
/// organization`, else through the shared route it uses.
pub fn org_path(s: &Scope) -> Option<PathBuf> {
    if let Some(u) = s.uses_of(UseKind::Organization).next() {
        return crate::names::beside(s.path, &u.path).ok();
    }
    for u in s.uses_of(UseKind::Route) {
        let Ok(route) = crate::names::beside(s.path, &u.path) else { continue };
        let Some(alias) = &u.alias else { continue };
        let Some(rf) = s.ringi(&alias.text, UseKind::Route) else { continue };
        if let Some(o) = rf.uses.iter().find(|x| x.kind == UseKind::Organization) {
            return crate::names::beside(&route.display().to_string(), &o.path).ok();
        }
    }
    None
}

/// The `openapi.json` of `<out>/requests/` for the kinds, as [`files`] writes it.
pub fn openapi_json(kinds: &[Kind]) -> String {
    format!("{}\n", openapi::json(kinds).pretty())
}

/// Every file of `<out>/requests/` for the kinds, in the order written: each with its path under
/// that directory, its text, and the request it is written from (none for one of them all).
pub fn files(kinds: &[Kind]) -> Vec<(String, String, Option<String>)> {
    let mut out = Vec::new();
    let pretty = |j: Json| format!("{}\n", j.pretty());
    for k in kinds {
        out.push((format!("types/{}.json", k.named.name), pretty(types::json(k)), Some(k.file.clone())));
    }
    out.push(("openapi.json".to_string(), pretty(openapi::json(kinds)), None));
    out.push(("mcp/tools.json".to_string(), pretty(mcp::json(kinds)), None));
    out.push(("organization/schema.json".to_string(), pretty(orgschema::schema(kinds)), None));
    out.push(("organization/columns.md".to_string(), orgschema::columns(kinds), None));
    out.push(("organization/find.jsonl".to_string(), crate::find::vectors(), None));
    for k in kinds {
        out.push((format!("vectors/{}.jsonl", k.named.name), k.vectors.clone(), Some(k.file.clone())));
        out.push((format!("vectors/worked/{}.jsonl", k.named.name), k.worked.clone(), Some(k.file.clone())));
    }
    out
}

/// The head of a file ritsu generates.
pub(crate) fn head() -> String {
    ritsu_emit::header::generated("ritsu")
}

fn named(written: &str, ascii: &str, label: &str, labels: Vec<(String, String)>) -> Named {
    Named { name: ascii.to_string(), written: written.to_string(), label: label.to_string(), labels }
}

/// The texts of `labels <language>` whose key `is` takes, each with its language.
fn labels(f: &File, is: impl Fn(&LabelKey) -> bool) -> Vec<(String, String)> {
    f.labels.iter().filter_map(|b| b.entries.iter().find(|e| is(&e.key)).map(|e| (b.lang.text.clone(), e.text.clone()))).collect()
}

fn labels_json(ls: &[(String, String)]) -> Json {
    Json::arr(ls.iter().map(|(lang, text)| Json::obj([("lang", Json::str(lang)), ("text", Json::str(text))])))
}

/// What is written at `at` in `src`, on its line.
fn written_at(src: &str, at: Span) -> String {
    let line = src.lines().nth(at.line.saturating_sub(1)).unwrap_or_default();
    line.chars().skip(at.col.saturating_sub(1)).take(at.end.saturating_sub(at.col)).collect::<String>().trim().to_string()
}

/// A field of the form (or of a master's row), `path` the written names of the rows it is in, `src`
/// the text of its file for the conditions it is shown and asked for on; `form` for a request's
/// form, whose computations the generated code works out.
fn field(m: &Machine, file: &File, src: &str, f: &Field, path: &[&str], form: bool) -> Fld {
    let rf = crate::ports::field(f);
    let mut at: Vec<&str> = path.to_vec();
    at.push(&f.name.text);
    let key_is = |words: &[crate::ast::Word], want: &[&str]| words.len() == want.len() && words.iter().zip(want).all(|(w, x)| w.text == *x);
    let label = labels(file, |k| matches!(k, LabelKey::Field(ws) if key_is(ws, &at)));
    let (min, max, count_min, count_max) = match &f.ty {
        TypeExpr::Num(_) | TypeExpr::Date => (rf.lo, rf.hi, None, None),
        TypeExpr::Text | TypeExpr::LongText | TypeExpr::Rows => (None, None, rf.lo, rf.hi),
        _ => (None, None, None, None),
    };
    let choices = match &f.ty {
        TypeExpr::Choice => f
            .options
            .iter()
            .map(|o| {
                let mut v = at.clone();
                v.push(&o.name.text);
                named(&o.name.text, o.name.ascii(), o.title.as_deref().unwrap_or(&o.name.text), labels(file, |k| matches!(k, LabelKey::Field(ws) if key_is(ws, &v))))
            })
            .collect(),
        // a value of a rule's enum, titled by the line under the field that names it (by the rule's
        // name or the one in brackets; a line with no title shows the name), else by the display
        // name the rule writes for it in the file's language, else by its name; and labelled at
        // `<field>.<value>` by either name (DESIGN 4.8)
        TypeExpr::ChoiceFrom(p) if form && p.len() == 2 => match m.s.rule_facts(&p[0].text).and_then(|r| r.enums.iter().find(|e| e.name == p[1].text || e.public == p[1].text).map(|e| (r, e))) {
            Some((r, e)) => e
                .values
                .iter()
                .map(|v| {
                    let line = f.value_titles.iter().find(|t| t.name.text == v.name || t.name.text == v.public || t.name.ascii() == v.public);
                    let ruled = v.title.as_deref().filter(|_| rule_written_in(r) == written_in(file));
                    let title = match line {
                        Some(t) => t.title.as_deref().unwrap_or(&v.name),
                        None => ruled.unwrap_or(&v.name),
                    };
                    let (mut by_name, mut by_ascii) = (at.clone(), at.clone());
                    by_name.push(&v.name);
                    by_ascii.push(&v.public);
                    named(&v.name, &v.public, title, labels(file, |k| matches!(k, LabelKey::Field(ws) if key_is(ws, &by_name) || key_is(ws, &by_ascii))))
                })
                .collect(),
            None => Vec::new(),
        },
        _ => Vec::new(),
    };
    let ascii = f.name.ascii().to_string();
    // a condition of the form, worked out by the copy of `form.rule` the generated code reads
    // what a condition or a computation reads: as the lowered rule the screens work it out with
    // reads it, else as it is written, through the computations it reads
    let values = m.lowered.files.iter().find(|f| f.part == crate::lower::Part::Values);
    let lowered = |out: &str| values.map(|lf| crate::lower::reads_of(m.s, lf, out)).filter(|r| !r.is_empty());
    let cond = |e: &Option<crate::ast::Expr>, word: &str| {
        e.as_ref().filter(|_| form && path.is_empty()).map(|e| {
            let worked = format!("{ascii}_{word}");
            let reads = lowered(&worked).unwrap_or_else(|| crate::lower::form_reads(m.s, e, None));
            Cond { written: written_at(src, e.span), worked, reads }
        })
    };
    let rows = path.first().and_then(|r| m.s.field(r));
    // what a computation reads, as the words of its refusal name the fields to change (DESIGN 4.4)
    let reads = if form { crate::lower::computed_reads(m.s, values, f, rows) } else { Vec::new() };
    let type_written = f.ty.word();
    let type_ascii = if form { crate::api::type_in_ascii(&type_written, &m.names) } else { crate::api::stepped(&type_written) };
    Fld {
        named: named(&f.name.text, &ascii, f.title.as_deref().unwrap_or(&f.name.text), label),
        line: f.name.span.line,
        ty: f.ty.clone(),
        type_ascii,
        type_written,
        min,
        max,
        count_min,
        count_max,
        choices,
        optional: f.optional.is_some(),
        computed: f.computed.is_some(),
        reads,
        worked_on_screen: false,
        approvers_only: f.approvers_only.is_some(),
        open_in: f.open_in.as_ref().map(|w| w.text.clone()),
        shown_when: cond(&f.shown_when, "shown"),
        required_when: cond(&f.required_when, "required"),
        fields: f.children.iter().map(|c| field(m, file, src, c, &at, form)).collect(),
    }
}

/// A group of `submitted by`, each key there whatever its kind: `kind` (`everyone`, `team`,
/// `position`, `person`), `team` (a team's ID), `position` (in ASCII, `positions` the organization's)
/// and `position_written`, `higher`, `within` (a team's ID), `person` (a person's ID).
fn group_json(g: &Group, positions: &[Named]) -> Json {
    let w = |x: &Option<crate::ast::Word>| x.as_ref().map(|t| Json::str(&t.text)).unwrap_or(Json::Null);
    let n = Json::Null;
    let (kind, team, position, written, higher, within, person) = match g {
        Group::Everyone { within, .. } => ("everyone", n.clone(), n.clone(), n.clone(), n.clone(), w(within), n),
        Group::Team(t) => ("team", Json::str(&t.text), n.clone(), n.clone(), n.clone(), n.clone(), n),
        Group::Position { position, higher, within } => {
            let ascii = positions.iter().find(|p| p.written == position.text || p.name == position.text).map(|p| p.name.as_str()).unwrap_or(&position.text);
            ("position", n.clone(), Json::str(ascii), Json::str(&position.text), Json::from(*higher), w(within), n)
        }
        Group::Person(id, _) => ("person", n.clone(), n.clone(), n.clone(), n.clone(), n, Json::str(id)),
    };
    Json::obj([("kind", Json::str(kind)), ("team", team), ("position", position), ("position_written", written), ("higher", higher), ("within", within), ("person", person)])
}
