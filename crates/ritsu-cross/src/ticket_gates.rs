//! A request's route held to the gate the request reads (ringi's DESIGN 9.3, X18). The route of a
//! request decides who answers each stage, and whether a requester who is among them answers their
//! own request (`self approval`); the gate is the company's rule, asked before every operation of a
//! ticket, with the stage's attributes as ringi decided them (`resource.requester`,
//! `resource.approvers`, `resource.self_approval`). The two are written apart, by different people,
//! and each passes its own check alone.
//!
//! ringi says what a ticket of the request does (`Requests::ticket`) and sekisho answers for the
//! gate (`Gates::facts`, and `Gates::allowed_with`, which holds some attributes to values and walks
//! every value of the rest). For each request and each gate it reads, three questions, each a
//! border:
//!
//! - **operations**: the gate declares an action for every operation the tickets of the request
//!   have and every one of the server's administration (a switch of a version of the request type,
//!   an import of the organization's data, and a read of the record of either), and none of them
//!   takes inputs of its own (`input`). One it does not declare is allowed to no one (E911); one
//!   that takes inputs is denied on every ticket, since the server asks the gate with the person,
//!   the resource and the time, and has nothing to give them (E911).
//! - **approvers**: the approvers the route decides are allowed what an approver does (`approve`,
//!   `reject`, and `send_back`, `pause` and `retract` where the request has them): asked with the
//!   principal in `resource.approvers` and not the requester. Allowed in no combination is E911;
//!   in some (a suspended member of the staff is denied) holds.
//! - **self approval**: where the route does not allow it, the gate allows a requester none of what
//!   decides a stage (`approve`, `reject`, `send_back`), asked with `resource.requester` the
//!   principal and `resource.self_approval` false: allowed in any combination is E911, with one.
//!   Where the route allows it, the gate allows it too, asked with the requester among the
//!   approvers and `resource.self_approval` true: allowed in none is E911.
//!
//! What sekisho cannot decide is W911, with why. The questions are the same whether the
//! organization has data or not: the attributes that decide them are what ringi hands the gate at
//! run time, held here to the values the route gives them, and everything else (the roles, the
//! other attributes) is walked in full, since the gate is the company's and outlives a version of
//! the organization. A gate that does not pass its own check is sekisho's to say, a request that
//! does not pass is ringi's, and a request that reads no gate has no border here.

use crate::{Border, Borders, Came, Check, Side};
use ritsu_base::diag::Diag;
use ritsu_base::json::Json;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{Allowance, Finding, Found, GateFacts, GateGiven, GateGivenValue, Gates, RequestTicket, Requests};
use ritsu_project::{Joined, Project};

/// The attributes of a stage ringi hands a gate (ringi's DESIGN 9.3): who submitted the request,
/// the approvers the route decided, and whether the route allows self approval.
pub const REQUESTER: &str = "requester";
pub const APPROVERS: &str = "approvers";
pub const SELF_APPROVAL: &str = "self_approval";

/// What the gate is asked of the approvers of a stage: the principal is one of them and is not the
/// requester, on a route whose `self approval` is `allowed`.
pub fn asked_of_approvers(allowed: bool) -> Vec<GateGiven> {
    vec![
        GateGiven::resource(APPROVERS, GateGivenValue::HoldsPrincipal(true)),
        GateGiven::resource(REQUESTER, GateGivenValue::IsPrincipal(false)),
        GateGiven::resource(SELF_APPROVAL, GateGivenValue::Bool(allowed)),
    ]
}

/// What the gate is asked of a requester at their own request. Where the route does not allow self
/// approval, whether they are among the approvers is left open: the gate allows them nothing either
/// way. Where it does, they are among the approvers, as ringi leaves them.
pub fn asked_of_requester(allowed: bool) -> Vec<GateGiven> {
    let mut given = vec![GateGiven::resource(REQUESTER, GateGivenValue::IsPrincipal(true))];
    if allowed {
        given.push(GateGiven::resource(APPROVERS, GateGivenValue::HoldsPrincipal(true)));
    }
    given.push(GateGiven::resource(SELF_APPROVAL, GateGivenValue::Bool(allowed)));
    given
}

/// What one operation comes to for the approvers of a stage.
#[derive(Clone, Debug, PartialEq)]
pub enum Approvers {
    /// Allowed in some combination.
    Held,
    /// Allowed in none (E911).
    Never,
    /// sekisho does not decide it (W911): why.
    Undecided(Text),
}

pub fn judge_approvers(a: &Found<Allowance>) -> Approvers {
    match a {
        Found::Value(Allowance::Never) => Approvers::Never,
        Found::Value(Allowance::Always | Allowance::Sometimes { .. }) => Approvers::Held,
        Found::Undecided(why) => Approvers::Undecided(why.clone()),
    }
}

/// What one operation that decides a stage comes to for a requester at their own request.
#[derive(Clone, Debug, PartialEq)]
pub enum Own {
    /// The gate says what the route says.
    Held,
    /// The route does not allow it, and the gate does (E911): a combination it allows, or None
    /// where it allows every one.
    Allowed(Option<Text>),
    /// The route allows it, and the gate allows it in no combination (E911).
    Never,
    /// sekisho does not decide it (W911): why.
    Undecided(Text),
}

pub fn judge_own(route_allows: bool, a: &Found<Allowance>) -> Own {
    match (route_allows, a) {
        (_, Found::Undecided(why)) => Own::Undecided(why.clone()),
        (false, Found::Value(Allowance::Never)) => Own::Held,
        (false, Found::Value(Allowance::Always)) => Own::Allowed(None),
        (false, Found::Value(Allowance::Sometimes { allowed, .. })) => Own::Allowed(Some(allowed.clone())),
        (true, Found::Value(Allowance::Never)) => Own::Never,
        (true, Found::Value(Allowance::Always | Allowance::Sometimes { .. })) => Own::Held,
    }
}

/// X18 over the project, through the ports `joined` holds.
pub(crate) fn check(project: &Project, joined: &Joined, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    check_with(project, &*joined.requests(), &*joined.gates(), lang, borders)
}

/// [`check`], with each port given: what ringi says of the tickets, and what sekisho says of the
/// gates.
pub fn check_with(project: &Project, requests: &dyn Requests, gates: &dyn Gates, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in project.of(Tool::Ringi) {
        // by the path the file's own check was run with, which ringi keeps the check under
        let disk = std::path::Path::new(&f.shown);
        let Ok(Found::Value(t)) = requests.ticket(disk) else { continue };
        let src = ritsu_base::fs::read_to_string(disk).unwrap_or_default();
        for g in &t.gates {
            // sekisho is asked for a gate by its path from a root: the project's, or, for a gate the
            // request reaches outside it (a file given by its name in a directory that is no
            // project's), the nearest directory that holds both
            let Some((root, gate)) = asked_from(&project.root, &g.file) else { continue };
            let Ok(facts) = gates.facts(&root, &gate) else { continue };
            let one = One { gates, file: f, src: &src, t: &t, root: &root, gate: &gate, named: &partner(project, &g.file), to: ritsu_base::paths::from_root(&project.root, &g.file), facts: &facts, line: g.line };
            for (b, d) in [one.operations(), one.approvers(), one.own()] {
                borders.add(b);
                out.extend(d.map(|d| Finding::of(&d, Some(f.rel.clone()), lang)));
            }
        }
    }
    out
}

/// The root sekisho is asked from for the gate at `gate`, and the gate from it: the project's root
/// when the gate is under it, else the nearest directory above both.
fn asked_from(root: &std::path::Path, gate: &std::path::Path) -> Option<(std::path::PathBuf, String)> {
    if let Some(rel) = ritsu_base::paths::from_root(root, gate) {
        return Some((root.to_path_buf(), rel));
    }
    let (root, gate) = (ritsu_base::paths::absolute(root), ritsu_base::paths::absolute(gate));
    let above = root.ancestors().find(|d| gate.starts_with(d))?;
    Some((above.to_path_buf(), ritsu_base::paths::from_root(above, &gate)?))
}

/// A file a request reaches, as the notes name it: from the root; one outside the root as ringi
/// reached it, its `..` folded, so that a request given by its name and one found in a directory
/// name the same file the same way (`shared/company.gate`, not `requests/../shared/company.gate`).
pub(crate) fn partner(project: &Project, file: &std::path::Path) -> String {
    use std::path::Component;
    if let Some(rel) = ritsu_base::paths::from_root(&project.root, file) {
        return rel;
    }
    let mut out = std::path::PathBuf::new();
    for c in file.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir if matches!(out.components().next_back(), Some(Component::Normal(_))) => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out.to_string_lossy().to_string()
}

/// One request and one gate it reads.
struct One<'a> {
    gates: &'a dyn Gates,
    file: &'a ritsu_project::File,
    src: &'a str,
    t: &'a RequestTicket,
    /// The root sekisho is asked from, and the gate from it.
    root: &'a std::path::Path,
    gate: &'a str,
    /// The gate as the notes name it, and from the project's root (None outside it).
    named: &'a str,
    to: Option<String>,
    /// What sekisho knows of the gate.
    facts: &'a GateFacts,
    /// The line of the `use gate`.
    line: usize,
}

/// The operations of the server's administration (ringi's DESIGN 14), which it asks the gate about
/// with no ticket: of a version of a request type, a switch and a read of its record; of the
/// organization, an import of its data and a read of its record. The other operations are a
/// ticket's.
const ADMINISTRATION: [&str; 4] = ["change_request_type", "read_audit", "import_organization", "read_organization_audit"];

/// Names in code, as a message lists them: `` `approve`, `reject` and `send_back` ``.
fn listed(names: &[&str]) -> Text {
    Text::list(&names.iter().map(|n| Text::same(format!("`{n}`"))).collect::<Vec<_>>())
}

/// A piece of Japanese text a word follows: a space after it when it ends in a word of ASCII or in
/// code, as the suite's Japanese writes it.
fn spaced(piece: &str) -> String {
    match piece.chars().last() {
        Some(c) if c.is_ascii_alphanumeric() || c == '`' || c == '"' || c == '_' => format!("{piece} "),
        _ => piece.to_string(),
    }
}

/// What ringi hands a gate of a stage, for a gate that declares it another way.
fn handed() -> Text {
    tr!(
        "ringi がステップについてゲートに渡すのは、`resource.requester`（申請者。principal と同じ型のエンティティ）、`resource.approvers`（承認者がメンバーであるエンティティ）、`resource.self_approval`（bool）です。ゲートの resource に、その型で宣言してください。",
        "Of a stage, ringi hands the gate `resource.requester` (the requester: an entity of the principal's type), `resource.approvers` (an entity the approvers are members of) and `resource.self_approval` (a bool). Declare them so on the gate's resource."
    )
}

/// What the gate was asked, in words: each attribute held, joined.
fn asked(given: &[GateGiven]) -> Text {
    Text::list(&given.iter().map(GateGiven::text).collect::<Vec<_>>())
}

impl One<'_> {
    fn border(&self, about: &str, line: usize, to: Option<usize>, came: Came, code: Option<&str>, asked: &[&str]) -> Border {
        let mut b = Border::new(Check::TicketGate, about, Side::at(Tool::Ringi, &self.file.rel, line), Some(Side::new(Tool::Sekisho, self.to.clone(), self.to.as_ref().and(to))), came, code);
        b.detail = vec![("operations".into(), Json::arr(asked.iter().map(|n| Json::str(*n))))];
        b
    }

    fn at(&self, code: &'static str, line: usize, message: Text) -> Diag {
        Diag::at(code, &self.file.shown, line, 0, message).source(self.src).rel(&self.file.rel)
    }

    /// The gate's action for an operation of the ticket: its name as the gate asks for it, and its
    /// line.
    fn action(&self, operation: &str) -> Option<(&str, usize)> {
        self.facts.actions.iter().find(|a| a.name == operation || a.alias == operation).map(|a| (a.alias.as_str(), a.line))
    }

    /// The inputs of its own the gate's action for an operation takes, and its line.
    fn inputs(&self, operation: &str) -> Option<(&[ritsu_ports::GateInput], usize)> {
        self.facts.actions.iter().find(|a| a.name == operation || a.alias == operation).map(|a| (a.inputs.as_slice(), a.line))
    }

    /// The gate declares an action for every operation.
    fn operations(&self) -> (Border, Option<Diag>) {
        let (request, gate) = (&self.t.named, self.named);
        let all: Vec<&str> = self.t.operations.iter().map(|o| o.name.as_str()).collect();
        let missing: Vec<&ritsu_ports::TicketOperation> = self.t.operations.iter().filter(|o| self.action(&o.name).is_none()).collect();
        if missing.is_empty() {
            return self.taken(&all);
        }
        let names: Vec<&str> = missing.iter().map(|o| o.name.as_str()).collect();
        let (l, n, first) = (listed(&names), names.len(), names[0]);
        // how many of them the server's administration asks, and not a ticket
        let admin = names.iter().filter(|n| ADMINISTRATION.contains(n)).count();
        let message = match (n, admin) {
            (1, 0) => tr!("ゲート {gate} に、`{request}` のチケットの操作 `{first}` の action がありません", "The gate {gate} declares no action `{first}`, an operation the tickets of `{request}` have"),
            (_, 0) => tr!("ゲート {gate} に、`{request}` のチケットの操作 {n} 個（{}）の action がありません", "The gate {gate} declares no action for {n} operations the tickets of `{request}` have: {}", l.ja; l.en),
            (1, _) => tr!("ゲート {gate} に、サーバーの管理の操作 `{first}` の action がありません", "The gate {gate} declares no action `{first}`, an operation of the server's administration"),
            _ if admin == n => tr!("ゲート {gate} に、サーバーの管理の操作 {n} 個（{}）の action がありません", "The gate {gate} declares no action for {n} operations of the server's administration: {}", l.ja; l.en),
            _ => tr!(
                "ゲート {gate} に、`{request}` のチケットの操作とサーバーの管理の操作の {n} 個（{}）の action がありません",
                "The gate {gate} declares no action for {n} operations of the tickets of `{request}` and of the server's administration: {}",
                l.ja; l.en
            ),
        };
        let mut d = self.at("E911", self.line, message);
        // the operations a line of the request gives its tickets, said once for the line
        let mut by_line: std::collections::BTreeMap<usize, Vec<&str>> = std::collections::BTreeMap::new();
        for o in missing.iter().filter(|o| o.line > 0) {
            by_line.entry(o.line).or_default().push(&o.name);
        }
        for (line, given) in &by_line {
            let g = listed(given);
            d = d.note(tr!("{} は、申請の {line} 行目が書いている操作です。", "Line {line} of the request gives its tickets {}.", g.ja; g.en));
        }
        // `see` is no operation on a ticket: what is asked of it is who else sees one
        let d = if names.contains(&"see") {
            d.note(tr!(
                "`see` は、チケットに載っていない人にチケットを見せるかを、サーバーがゲートに尋ねる action です。無ければ、申請者の側と承認者の側の人のほかはだれも見られず、チケットに載っていない管理する人は、承認者を足すことも引き継がせることもできません（見られない人の操作は 404 になります）。",
                "`see` is what the server asks the gate before it shows a ticket to someone not on it. Without it, no one but the requester's side and the approvers' side sees a ticket, and an administrator not on the ticket can neither add an approver nor hand a place over (every operation of one who does not see a ticket is answered 404)."
            ))
        } else {
            d
        };
        // what the administration asks, and with which resource
        let d = if admin > 0 {
            d.note(tr!(
                "サーバーの管理の操作は四つです。申請の種類の版を切り替える `change_request_type` と、その記録を読む `read_audit` は、申請の種類を resource にして、組織のデータを取り込む `import_organization` と、その記録を読む `read_organization_audit` は、組織を resource にして、サーバーがゲートに尋ねます。",
                "The server's administration asks the gate about four operations: `change_request_type`, which switches a version of a request type, and `read_audit`, which reads its record, with the request type as the resource; `import_organization`, which imports the organization's data, and `read_organization_audit`, which reads its record, with the organization as the resource."
            ))
        } else {
            d
        };
        let d = d.note(match (n, admin) {
            (1, 0) => tr!(
                "サーバーは、操作を受け付ける前にゲートに尋ねます。ゲートに無い action はだれにも許されないので、その操作はどのチケットでも拒まれます。",
                "The server asks the gate before it takes an operation, and an action the gate does not declare is allowed to no one: the operation would be denied on every ticket."
            ),
            (_, 0) => tr!(
                "サーバーは、操作を受け付ける前にゲートに尋ねます。ゲートに無い action はだれにも許されないので、これらの操作はどのチケットでも拒まれます。",
                "The server asks the gate before it takes an operation, and an action the gate does not declare is allowed to no one: these operations would be denied on every ticket."
            ),
            (1, _) => tr!(
                "サーバーは、操作を受け付ける前にゲートに尋ねます。ゲートに無い action はだれにも許されないので、その操作はだれもできません。",
                "The server asks the gate before it takes an operation, and an action the gate does not declare is allowed to no one: no one could do the operation."
            ),
            _ => tr!(
                "サーバーは、操作を受け付ける前にゲートに尋ねます。ゲートに無い action はだれにも許されないので、これらの操作はだれもできません。",
                "The server asks the gate before it takes an operation, and an action the gate does not declare is allowed to no one: no one could do these operations."
            ),
        });
        let d = d.note(if n == 1 {
            tr!(
                "{gate} に action を宣言し（`action {first}` と、受け取る principal と resource）、だれに許すかを permit で書いてください。",
                "Declare the action in {gate} (`action {first}`, with the principal and the resource it takes), and write a permit for who may do it."
            )
        } else {
            tr!(
                "{gate} に、それぞれの action を宣言し（`action {first}` のように、受け取る principal と resource を書きます）、だれに許すかを permit で書いてください。",
                "Declare an action for each in {gate} (as `action {first}`, with the principal and the resource it takes), and write a permit for who may do each."
            )
        });
        (self.border("operations", self.line, None, Came::Failed, Some("E911"), &all), Some(d))
    }

    /// Every operation has an action: none of them takes inputs of its own, which the server, asking
    /// the gate with the person, the resource and the time, has nothing to give.
    fn taken(&self, all: &[&str]) -> (Border, Option<Diag>) {
        let (request, gate) = (&self.t.named, self.named);
        let with: Vec<(&str, &[ritsu_ports::GateInput], usize)> = self.t.operations.iter().filter_map(|o| self.inputs(&o.name).filter(|(i, _)| !i.is_empty()).map(|(i, line)| (o.name.as_str(), i, line))).collect();
        let Some(&(first, own, to)) = with.first() else {
            return (self.border("operations", self.line, None, Came::Held, None, all), None);
        };
        let names: Vec<&str> = with.iter().map(|(n, _, _)| *n).collect();
        let (l, n) = (listed(&names), names.len());
        // how many of them the server's administration asks, and not a ticket
        let admin = names.iter().filter(|n| ADMINISTRATION.contains(n)).count();
        // each input by its name, as the gate writes it
        let inputs = |i: &[ritsu_ports::GateInput]| listed(&i.iter().map(|x| x.name.as_str()).collect::<Vec<_>>());
        let message = match (n, admin) {
            (1, 0) => {
                let i = inputs(own);
                tr!(
                    "ゲート {gate} の action `{first}` は自分の入力（{}）を受け取りますが、`{request}` のチケットの操作でゲートに尋ねるサーバーには、渡す値がありません",
                    "The action `{first}` of the gate {gate} takes inputs of its own ({}), which the server has nothing to give when it asks the gate for an operation the tickets of `{request}` have",
                    i.ja; i.en
                )
            }
            (1, _) => {
                let i = inputs(own);
                tr!(
                    "ゲート {gate} の action `{first}` は自分の入力（{}）を受け取りますが、管理の操作でゲートに尋ねるサーバーには、渡す値がありません",
                    "The action `{first}` of the gate {gate} takes inputs of its own ({}), which the server has nothing to give when it asks the gate for an operation of its administration",
                    i.ja; i.en
                )
            }
            (_, 0) => tr!(
                "ゲート {gate} の action {n} 個（{}）が自分の入力を受け取りますが、`{request}` のチケットの操作でゲートに尋ねるサーバーには、渡す値がありません",
                "{n} actions of the gate {gate} ({}) take inputs of their own, which the server has nothing to give when it asks the gate for the operations the tickets of `{request}` have",
                l.ja; l.en
            ),
            _ if admin == n => tr!(
                "ゲート {gate} の action {n} 個（{}）が自分の入力を受け取りますが、管理の操作でゲートに尋ねるサーバーには、渡す値がありません",
                "{n} actions of the gate {gate} ({}) take inputs of their own, which the server has nothing to give when it asks the gate for the operations of its administration",
                l.ja; l.en
            ),
            _ => tr!(
                "ゲート {gate} の action {n} 個（{}）が自分の入力を受け取りますが、`{request}` のチケットの操作と管理の操作でゲートに尋ねるサーバーには、渡す値がありません",
                "{n} actions of the gate {gate} ({}) take inputs of their own, which the server has nothing to give when it asks the gate for the operations the tickets of `{request}` have and those of its administration",
                l.ja; l.en
            ),
        };
        let mut d = self.at("E911", self.line, message);
        if n > 1 {
            for (name, i, _) in &with {
                let i = inputs(i);
                d = d.note(tr!("`{name}` の入力は {} です。", "`{name}` takes {}.", i.ja; i.en));
            }
        }
        let d = d
            .note(if admin == 0 {
                tr!(
                    "サーバーがゲートに渡すのは、人と、resource（チケット、ステップか申請の種類と、ringi が決めた属性）と、時刻だけです。自分の入力を受け取る action は、どのチケットでも拒まれます（種類 `input` の断り）。",
                    "The server hands the gate the person, the resource (the ticket, the stage or the kind of request, with the attributes ringi decides) and the time, and nothing else: an action that takes inputs of its own is denied on every ticket (a refusal of the kind `input`)."
                )
            } else {
                tr!(
                    "サーバーがゲートに渡すのは、人と、resource（チケット、ステップ、申請の種類か組織と、ringi が決めた属性）と、時刻だけです。自分の入力を受け取る action は、いつも拒まれます（種類 `input` の断り）。",
                    "The server hands the gate the person, the resource (the ticket, the stage, the kind of request or the organization, with the attributes ringi decides) and the time, and nothing else: an action that takes inputs of its own is always denied (a refusal of the kind `input`)."
                )
            })
            .note(tr!(
                "{gate} の {} から `input` の行を外してください。自分の入力が要る決まりは別の action に書き、プログラムからその action の関数で尋ねます。",
                "Take the `input` lines off {} in {gate}. A rule that needs inputs of its own is another action, which a program asks through that action's own function.",
                l.ja; l.en
            ));
        (self.border("operations", self.line, Some(to), Came::Failed, Some("E911"), all), Some(d))
    }

    /// The approvers the route decides are allowed what an approver does.
    fn approvers(&self) -> (Border, Option<Diag>) {
        let (request, gate) = (&self.t.named, self.named);
        let given = asked_of_approvers(self.t.self_approval);
        let (mut all, mut never, mut undecided): (Vec<&str>, Vec<&str>, Vec<(&str, Text)>) = (Vec::new(), Vec::new(), Vec::new());
        let mut to = None;
        for o in self.t.operations.iter().filter(|o| o.by_approver) {
            // an action the gate does not declare is the first question's
            let Some((action, line)) = self.action(&o.name) else { continue };
            let Ok(a) = self.gates.allowed_with(self.root, self.gate, action, None, &given) else { continue };
            all.push(&o.name);
            to = to.or(Some(line));
            match judge_approvers(&a) {
                Approvers::Held => {}
                Approvers::Never => never.push(&o.name),
                Approvers::Undecided(why) => undecided.push((&o.name, why)),
            }
        }
        let asked = asked(&given);
        let unknown = |d: Diag| match undecided.first() {
            Some((_, why)) => {
                let l = listed(&undecided.iter().map(|(n, _)| *n).collect::<Vec<_>>());
                d.note(tr!("{} を許すかは決められません: {}。", "Whether it allows {} cannot be decided: {}.", l.ja, why.ja; l.en, why.en))
            }
            None => d,
        };
        if !never.is_empty() {
            let (l, first) = (listed(&never), never[0]);
            let d = self
                .at("E911", self.line, tr!(
                    "ゲート {gate} は、`{request}` の経路が決めた承認者に、{} をどの組み合わせでも許しません",
                    "The gate {gate} allows the approvers the route of `{request}` decides {} in no combination",
                    l.ja; l.en
                ))
                .note(tr!(
                    "ゲートに尋ねた形: {}。action が受け取るどの型の principal でも、どの役割でも、ほかの属性がどの値でも、許されません。",
                    "Asked of the gate: {}. Whatever type of principal the action takes, whatever its roles and the other attributes, it is not allowed.",
                    asked.ja; asked.en
                ))
                .note(tr!("承認者がステップに答えても、いつも拒まれます。", "Every answer an approver gives a stage would be denied."));
            let d = unknown(d).note(tr!(
                "{gate} に、ステップの承認者を許す permit を書いてください（`action {first}` と `when principal in resource.approvers`）。forbid がそれを打ち消さないようにします。",
                "Write a permit in {gate} for the approvers of a stage (`action {first}`, `when principal in resource.approvers`), which no forbid takes back."
            ));
            return (self.border("approvers", self.line, to, Came::Failed, Some("E911"), &all), Some(d));
        }
        if let Some((_, why)) = undecided.first() {
            let l = listed(&undecided.iter().map(|(n, _)| *n).collect::<Vec<_>>());
            let d = self
                .at("W911", self.line, tr!(
                    "ゲート {gate} が、`{request}` の経路が決めた承認者に {} を許すかを決められません",
                    "Whether the gate {gate} allows the approvers the route of `{request}` decides {} cannot be decided",
                    l.ja; l.en
                ))
                .note(tr!("ゲートに尋ねた形: {}。", "Asked of the gate: {}.", asked.ja; asked.en))
                .note(tr!("決められない理由: {}。", "Why it cannot be decided: {}.", why.ja; why.en))
                .note(handed());
            return (self.border("approvers", self.line, to, Came::Undecided, Some("W911"), &all), Some(d));
        }
        (self.border("approvers", self.line, to, Came::Held, None, &all), None)
    }

    /// A requester decides their own request where the route allows it, and nowhere else.
    fn own(&self) -> (Border, Option<Diag>) {
        let (request, gate, allows) = (&self.t.named, self.named, self.t.self_approval);
        let given = asked_of_requester(allows);
        let at = if self.t.self_approval_line > 0 { self.t.self_approval_line } else { self.line };
        let (mut all, mut allowed, mut never, mut undecided): (Vec<&str>, Vec<(&str, Option<Text>)>, Vec<&str>, Vec<(&str, Text)>) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut to = None;
        for o in self.t.operations.iter().filter(|o| o.decides) {
            let Some((action, line)) = self.action(&o.name) else { continue };
            let Ok(a) = self.gates.allowed_with(self.root, self.gate, action, None, &given) else { continue };
            all.push(&o.name);
            to = to.or(Some(line));
            match judge_own(allows, &a) {
                Own::Held => {}
                Own::Allowed(example) => allowed.push((&o.name, example)),
                Own::Never => never.push(&o.name),
                Own::Undecided(why) => undecided.push((&o.name, why)),
            }
        }
        let asked = asked(&given);
        let says = if self.t.self_approval_line > 0 {
            let (l, word) = (self.t.self_approval_line, if allows { "self approval allowed" } else { "self approval not allowed" });
            tr!("申請の {l} 行目が `{word}` と書いています。", "Line {l} of the request says `{word}`.")
        } else {
            tr!("`self approval` の行を書かない申請の種類は、自己承認を許しません。", "A request that writes no `self approval` line does not allow it.")
        };
        let unknown = |d: Diag| match undecided.first() {
            Some((_, why)) => {
                let l = listed(&undecided.iter().map(|(n, _)| *n).collect::<Vec<_>>());
                d.note(tr!("{} については決められません: {}。", "It cannot be decided for {}: {}.", l.ja, why.ja; l.en, why.en))
            }
            None => d,
        };
        if let Some((first, example)) = allowed.first() {
            let l = listed(&allowed.iter().map(|(n, _)| *n).collect::<Vec<_>>());
            let d = self
                .at("E911", at, tr!(
                    "`{request}` は申請者に自分の申請を決めさせませんが、ゲート {gate} は申請者に {} を許します",
                    "`{request}` does not let a requester decide their own request, and the gate {gate} allows a requester {} on it",
                    l.ja; l.en
                ))
                .note(says)
                .note(tr!("ゲートに尋ねた形: {}。", "Asked of the gate: {}.", asked.ja; asked.en))
                .note(match example {
                    // sekisho writes the example as it writes its own messages, with no space
                    // around an ASCII word, and spaces them when it prints
                    Some(x) => tr!("たとえば、{}のときに `{first}` が許されます。", "`{first}` is allowed, for example: {}.", spaced(&ritsu_base::text::ja_spacing(&x.ja)); x.en),
                    None => tr!("その形のどの組み合わせでも、`{first}` が許されます。", "`{first}` is allowed in every such combination."),
                });
            let d = unknown(d).note(tr!(
                "{gate} に forbid を書くか（`action {first}`、`when resource.requester is principal`、`unless resource.self_approval`）、申請の種類で自己承認を許してください（`self approval allowed`）。",
                "Write a forbid in {gate} (`action {first}`, `when resource.requester is principal`, `unless resource.self_approval`), or allow it in the request (`self approval allowed`)."
            ));
            return (self.border("self approval", at, to, Came::Failed, Some("E911"), &all), Some(d));
        }
        if !never.is_empty() {
            let (l, first) = (listed(&never), never[0]);
            let d = self
                .at("E911", at, tr!(
                    "`{request}` は申請者に自分の申請を決めさせますが、ゲート {gate} は申請者に {} をどの組み合わせでも許しません",
                    "`{request}` lets a requester decide their own request, and the gate {gate} allows a requester {} on it in no combination",
                    l.ja; l.en
                ))
                .note(says)
                .note(tr!("ゲートに尋ねた形: {}。", "Asked of the gate: {}.", asked.ja; asked.en))
                .note(tr!("ステップの承認者に入っている申請者が答えても、いつも拒まれます。", "Every answer a requester who is among a stage's approvers gives would be denied."));
            let d = unknown(d).note(tr!(
                "申請者が自分の申請を決めるのを止めている {gate} の forbid に `unless resource.self_approval` を書くか（`action {first}`）、申請の種類から `self approval allowed` を外してください。",
                "Write `unless resource.self_approval` on the forbid of {gate} that keeps a requester from deciding their own request (`action {first}`), or take `self approval allowed` out of the request."
            ));
            return (self.border("self approval", at, to, Came::Failed, Some("E911"), &all), Some(d));
        }
        if let Some((_, why)) = undecided.first() {
            let l = listed(&undecided.iter().map(|(n, _)| *n).collect::<Vec<_>>());
            let message = tr!(
                "ゲート {gate} が、`{request}` の申請者に、自分の申請への {} を許すかを決められません",
                "Whether the gate {gate} allows a requester of `{request}` {} on their own request cannot be decided",
                l.ja; l.en
            );
            let d = self
                .at("W911", at, message)
                .note(says)
                .note(tr!("ゲートに尋ねた形: {}。", "Asked of the gate: {}.", asked.ja; asked.en))
                .note(tr!("決められない理由: {}。", "Why it cannot be decided: {}.", why.ja; why.en))
                .note(handed());
            return (self.border("self approval", at, to, Came::Undecided, Some("W911"), &all), Some(d));
        }
        (self.border("self approval", at, to, Came::Held, None, &all), None)
    }
}
