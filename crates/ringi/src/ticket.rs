//! What ringi knows of a request's ticket, for the checks across the borders and `ringi api`
//! (DESIGN 9.3, 9.4; the port `Requests::ticket`): the operations a gate is asked for and whether
//! the route allows self approval (X18), the holds it makes and how long a round takes (X19), and
//! the flows it starts and asks, each with what it is given (X20).
//!
//! Nothing here is counted again: how a ticket ends and how long it takes are what [`crate::ends`]
//! says, a field's range is the form's, and who may submit is [`crate::org::submit`]'s.
//!
//! What a flow is given is the form's, held to where the flow is given it. An agent's stage and a
//! summary are given the ticket's ID and every field of the form, on the rows of the route's table
//! whose cells name them: a field that is a column of the table is held to those rows' tests (the
//! rows of a route do not overlap, so a row's tests are the requests it takes), and a field shown
//! or asked for on a condition has a value where the rows make the condition hold. Where ringi does
//! not follow what narrows the form (a stage line's `when`, an `on` line's `when`, a way a rule's
//! output or another table names), it says so, and nothing that could be narrower is held to be
//! too wide.

use crate::ast::{ActionKind, CmpOp, Ending, Expr, ExprKind, Field, How, Route, RouteRow, RouteTable, Spec, Test, TicketLine, TypeExpr, UseKind, WhenEmpty};
use crate::lower::words;
use crate::names::{Got, Scope};
use crate::suite::Suite;
use ritsu_base::text::Text;
use ritsu_ports::{Carried, FlowGiven, Found, HoldArg, Lined, RequestTicket, Spelled, TicketCalendar, TicketFlow, TicketGate, TicketHold, TicketOperation, TicketTime, VersionWithout};
use std::path::PathBuf;

/// The ticket of the request in `s`, which passes its check.
pub fn facts(s: &Scope, suite: &Suite) -> RequestTicket {
    let f = s.file;
    let lines: &[TicketLine] = f.ticket.as_ref().map(|t| t.lines.as_slice()).unwrap_or(&[]);
    let line_of = |pick: &dyn Fn(&TicketLine) -> bool| lines.iter().find(|l| pick(l)).map(|l| l.line());
    let (self_approval, self_approval_line) = lines
        .iter()
        .find_map(|l| match l {
            TicketLine::SelfApproval { allowed, line } => Some((*allowed, *line)),
            _ => None,
        })
        .unwrap_or((false, 0));
    let gates = f
        .uses
        .iter()
        .zip(&s.got)
        .filter(|(u, _)| u.kind == UseKind::Gate)
        .filter_map(|(u, g)| match g {
            Got::Other(p) => Some(TicketGate { file: reached(p), line: u.line }),
            _ => None,
        })
        .collect();
    let (count, endings, no_end) = crate::ends::summary(s);
    let has = |e: Ending| endings.contains(&e);
    let mut operations = vec![op("submit", 0, false, false)];
    let send_back = line_of(&|l| matches!(l, TicketLine::SendBack { times, .. } if *times > 0));
    let withdraw = line_of(&|l| matches!(l, TicketLine::Withdraw { .. })).or(send_back);
    if has(Ending::Withdrawn) {
        operations.push(op("withdraw", withdraw.unwrap_or(0), false, false));
    }
    if has(Ending::Rejected) {
        operations.push(op("approve", 0, true, true));
        operations.push(op("reject", 0, true, true));
        if let Some(l) = send_back {
            operations.push(op("send_back", l, true, true));
        }
        if let Some(l) = line_of(&|l| matches!(l, TicketLine::Pause { .. })) {
            operations.push(op("pause", l, true, false));
        }
        if let Some(l) = line_of(&|l| matches!(l, TicketLine::Retract { .. })) {
            operations.push(op("retract", l, true, false));
        }
    }
    // a comment is no answer: what the approvers have to be allowed is the answers alone. `see` is
    // asked of everyone not on a ticket (DESIGN 12.4, who sees a ticket). The server's
    // administration asks the rest: before a version of the request type is switched or its record
    // read, and before the organization's data is imported or its record read (DESIGN 14)
    for n in ["add_approver", "hand_over", "change_request_type", "comment", "see", "read_audit", "import_organization", "read_organization_audit"] {
        operations.push(op(n, 0, false, false));
    }
    let cx = crate::lower::context(s, suite);
    let mut calendars: Vec<TicketCalendar> = Vec::new();
    let mut counted = |named: Option<&str>| {
        if let Some(c) = cx.calendar(named)
            && let Some(file) = &c.target
            && !calendars.iter().any(|x| x.alias == c.alias)
        {
            calendars.push(TicketCalendar { alias: c.alias.clone(), code_name: c.ascii.clone(), file: file.clone() });
        }
    };
    for o in count.paths.iter().flatten() {
        if let Some(d) = o.due
            && crate::ends::takes(o) != crate::ends::Takes::Nothing
        {
            counted(d.calendar.map(|w| w.text.as_str()));
        }
    }
    for l in lines {
        if let TicketLine::Pause { calendar, .. } = l {
            counted(calendar.as_ref().map(|w| w.text.as_str()));
        }
    }
    let time = TicketTime {
        round: count.round,
        backs: count.backs.map_or(0, |(n, _)| n),
        resubmit: count.resubmit.map(|(n, _)| n),
        pause: count.pause.map(|(n, _)| n),
        total: count.total,
        held: count.held,
        no_end,
        calendars,
    };
    RequestTicket {
        request: f.name.ascii().to_string(),
        named: f.name.text.clone(),
        self_approval,
        self_approval_line,
        gates,
        operations,
        endings: endings.iter().map(|e| e.word().to_string()).collect(),
        time,
        holds: holds(s, lines),
        flows: flows(s, lines),
    }
}

fn op(name: &str, line: usize, by_approver: bool, decides: bool) -> TicketOperation {
    TicketOperation { name: name.to_string(), line, by_approver, decides }
}

/// The path of a `use` of another language by its alias, as the request reaches it ([`reached`]).
fn used(s: &Scope, alias: &str, kind: UseKind) -> Option<PathBuf> {
    match s.alias_of(alias, kind) {
        Some((_, Got::Other(p) | Got::Rule(p, _))) => Some(reached(p)),
        _ => None,
    }
}

/// A file a `use` line reaches, as the run writes its paths (ritsu's DESIGN 4.15): the file the
/// system opens, from where the run is. The path joined to the request's directory (`in/../shared/
/// company.gate`) opens it too, but one who folds it by its letters, as the checks between the
/// languages do of a file outside the root, finds another file where `in` is a symbolic link.
fn reached(p: &std::path::Path) -> PathBuf {
    PathBuf::from(ritsu_base::paths::show(p))
}

/// The kind of team an expression reads above the requester (`requester.department`), when that
/// is all it is.
fn requester_team(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Path(p) if p.len() == 2 && p[0].text == "requester" && p[1].text != "id" => Some(p[1].text.as_str()),
        _ => None,
    }
}

/// Whether a requester has a team of the kind above them in a version of the organization: what
/// `requester.<kind>` has a value for (the machine reads it as [`crate::engine`]'s `on` does).
pub(crate) fn has_kind(v: &crate::org::Version, id: &str, kind: &str) -> bool {
    v.primary(id).is_some_and(|m| v.up(&m.team).iter().any(|t| t.kind == kind))
}

/// Who of the people who may submit the request has no team of the kind above them, by the version
/// of the organization: the requesters `requester.<kind>` has no value for. Undecided where the
/// organization has no data.
fn without(s: &Scope, kind: &str) -> Found<Vec<VersionWithout>> {
    let Some(org) = s.org_data() else {
        return Found::Undecided(tr!("組織にデータがありません", "the organization has no data"));
    };
    let by = crate::org::submit::of(s.file);
    Found::Value(
        org.versions
            .iter()
            .map(|v| {
                let who: Vec<&str> = v.people.iter().map(|p| p.id.as_str()).filter(|id| by.as_ref().is_none_or(|l| l.may(org, v, id))).collect();
                let without: Vec<String> = who.iter().filter(|id| !has_kind(v, id, kind)).map(|id| id.to_string()).collect();
                VersionWithout { from: v.from, requesters: who.len(), named: without.iter().map(|id| v.who(id)).collect(), without }
            })
            .collect(),
    )
}

/// The kinds of team a condition holds only for a requester who has one above them: `requester.<kind>
/// is not none`, alone or joined to others with `and`.
fn kinds_asked(e: &Expr) -> Vec<&str> {
    match &e.kind {
        ExprKind::And(a, b) => kinds_asked(a).into_iter().chain(kinds_asked(b)).collect(),
        ExprKind::Cmp(CmpOp::IsNot, a, b) if matches!(b.kind, ExprKind::None) => requester_team(a).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// An argument of a hold made at `submit` that reads where the requester is in the organization,
/// on a line that is not kept to the requesters who have a value for it.
pub(crate) struct HoldGap<'a> {
    /// The hold, as written: `budget.reserve`.
    pub alias: &'a str,
    pub transfer: &'a str,
    /// The line of the `on submitted hold`.
    pub line: usize,
    pub arg: &'a crate::ast::Arg,
    /// The kind of team the argument reads above the requester.
    pub kind: &'a str,
}

/// The arguments of the holds made at `submit` that not every requester has a value for: each reads
/// `requester.<kind>`, and the line's `when` does not ask for a team of the kind (`when
/// requester.<kind> is not none` makes the hold for those who have one alone).
pub(crate) fn hold_gaps(lines: &[TicketLine]) -> Vec<HoldGap<'_>> {
    let mut out = Vec::new();
    for l in lines {
        let TicketLine::On { endings, action: ActionKind::Hold, call, when, line } = l else { continue };
        if !endings.iter().any(|(e, _)| *e == Ending::Submitted) || call.target.len() != 2 {
            continue;
        }
        let asked = when.as_ref().map(kinds_asked).unwrap_or_default();
        for a in &call.args {
            if let Some(kind) = requester_team(&a.value).filter(|k| !asked.contains(k)) {
                out.push(HoldGap { alias: &call.target[0].text, transfer: &call.target[1].text, line: *line, arg: a, kind });
            }
        }
    }
    out
}

/// The holds the ticket makes when it is submitted.
fn holds(s: &Scope, lines: &[TicketLine]) -> Vec<TicketHold> {
    let gaps = hold_gaps(lines);
    let mut out = Vec::new();
    for l in lines {
        let TicketLine::On { endings, action: ActionKind::Hold, call, line, .. } = l else { continue };
        if !endings.iter().any(|(e, _)| *e == Ending::Submitted) || call.target.len() != 2 {
            continue;
        }
        let alias = &call.target[0].text;
        let Some(book) = used(s, alias, UseKind::Book) else { continue };
        let args = gaps
            .iter()
            .filter(|g| g.line == *line)
            .map(|g| HoldArg { name: g.arg.name.text.clone(), written: crate::types::written(s.src, g.arg.value.span), line: g.arg.value.span.line, col: g.arg.value.span.col, without: without(s, g.kind) })
            .collect();
        out.push(TicketHold { alias: alias.clone(), book, transfer: call.target[1].text.clone(), line: *line, col: call.target[0].span.col, args });
    }
    out
}

// ── What a flow is given ──

/// Where a flow is given the form: what holds the form there.
enum Place<'a> {
    /// On every request.
    Everywhere,
    /// On these rows of the route's table.
    Rows(&'a RouteTable, Vec<&'a RouteRow>),
    /// Where one of these conditions holds (the `when` of an `on` line, of a stage): `word` names
    /// the condition, in words.
    When { conds: Vec<&'a Expr>, word: Text },
    /// On a condition ringi does not follow here, in words.
    Unfollowed(Text),
}

/// The parts of a condition joined with `and`.
fn parts<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
    match &e.kind {
        ExprKind::And(a, b) => {
            parts(a, out);
            parts(b, out);
        }
        _ => out.push(e),
    }
}

/// A part of a condition as a comparison of one field of the form: the field, the comparison and
/// what it is compared with.
fn compared<'a>(s: &Scope<'a>, e: &'a Expr) -> Option<(&'a Field, CmpOp, &'a Expr)> {
    let ExprKind::Cmp(op, a, b) = &e.kind else { return None };
    let ExprKind::Path(p) = &a.kind else { return None };
    let f = p.first().filter(|_| p.len() == 1).and_then(|w| s.field(&w.text))?;
    Some((f, *op, b))
}

/// Whether a part of a condition is one ringi follows: a number or a date compared with a constant
/// (by anything but `is not`, which leaves a hole no range says), a choice that is or is not one of
/// its values, a field that `is not none`.
fn followed(s: &Scope, e: &Expr) -> bool {
    let Some((f, op, b)) = compared(s, e) else { return false };
    match (&f.ty, &b.kind) {
        (_, ExprKind::None) => op == CmpOp::IsNot,
        (TypeExpr::Num(_) | TypeExpr::Date, _) => op != CmpOp::IsNot && wire(f, b).is_some(),
        (TypeExpr::Choice, ExprKind::Path(v)) => v.len() == 1 && matches!(op, CmpOp::Is | CmpOp::IsNot) && f.options.iter().any(|o| o.name.text == v[0].text),
        _ => false,
    }
}

/// What the field can hold where the condition holds: the form's range, held to less by each part
/// of the condition that compares the field with a constant. A part that reads anything else leaves
/// it as it is, so this is no less than the condition leaves.
fn where_holds(s: &Scope, f: &Field, e: &Expr) -> Dom {
    let all = whole(f);
    let mut d = whole(f);
    let mut ps = Vec::new();
    parts(e, &mut ps);
    for p in ps {
        let Some((g, op, b)) = compared(s, p) else { continue };
        if g.name.text != f.name.text || !followed(s, p) {
            continue;
        }
        match (&f.ty, &b.kind) {
            (TypeExpr::Num(_) | TypeExpr::Date, _) => {
                let Some(c) = wire(f, b) else { continue };
                let (lo, hi) = match op {
                    CmpOp::Lt => (None, Some(c - 1)),
                    CmpOp::Le => (None, Some(c)),
                    CmpOp::Gt => (Some(c + 1), None),
                    CmpOp::Ge => (Some(c), None),
                    CmpOp::Is => (Some(c), Some(c)),
                    CmpOp::IsNot => (None, None),
                };
                if let Some(l) = lo {
                    d.lo = Some(d.lo.map_or(l, |x| x.max(l)));
                }
                if let Some(h) = hi {
                    d.hi = Some(d.hi.map_or(h, |x| x.min(h)));
                }
            }
            (TypeExpr::Choice, ExprKind::Path(v)) => d.values.retain(|x| (*x == v[0].text) == (op == CmpOp::Is)),
            _ => {}
        }
    }
    d.narrowed = d.lo != all.lo || d.hi != all.hi || d.values.len() != all.values.len();
    d
}

/// Whether the condition can hold for some request, as far as [`where_holds`] reads it: it leaves
/// every field a value.
fn can_hold(s: &Scope, e: &Expr) -> bool {
    s.fields().iter().all(|f| {
        let d = where_holds(s, f, e);
        let some = match (d.lo, d.hi) {
            (Some(l), Some(h)) => l <= h,
            _ => true,
        };
        some && (f.ty != TypeExpr::Choice || !d.values.is_empty())
    })
}

/// The place where one of the conditions holds; a place ringi does not follow where none of them
/// can hold, which no request comes to.
fn place_where<'a>(s: &Scope, conds: Vec<&'a Expr>, word: Text) -> Place<'a> {
    let conds: Vec<&Expr> = conds.into_iter().filter(|c| can_hold(s, c)).collect();
    if conds.is_empty() { Place::Unfollowed(word) } else { Place::When { conds, word } }
}

/// What a value given at the place may be held to less by, where ringi does not follow it: None
/// where what [`dom`] says of the field is all the place leaves it. A condition is followed in full
/// where each of its parts is ([`followed`]) and none reads another field the value is counted
/// with (a computed field, or a field read while the value itself is computed).
fn less<'a>(s: &Scope<'a>, at: &Place<'a>, f: Option<&Field>) -> Option<Text> {
    match at {
        Place::Everywhere | Place::Rows(..) => None,
        Place::Unfollowed(why) => Some(why.clone()),
        Place::When { conds, word } => {
            let Some(f) = f else { return Some(word.clone()) };
            let apart = |g: &Field| g.name.text == f.name.text || (g.computed.is_none() && f.computed.is_none());
            let all = conds.iter().all(|c| {
                let mut ps = Vec::new();
                parts(c, &mut ps);
                ps.iter().all(|p| followed(s, p) && compared(s, p).is_some_and(|(g, _, _)| apart(g)))
            });
            if all { None } else { Some(word.clone()) }
        }
    }
}

/// What a field can hold at a place: a number or a date between two ends (as the integers on the
/// wire, None at an open end), a choice among its values.
struct Dom {
    lo: Option<i128>,
    hi: Option<i128>,
    values: Vec<String>,
    /// Whether the place holds it to less than the form does.
    narrowed: bool,
}

/// A constant as the integer on the wire of the field's type.
fn wire(f: &Field, e: &Expr) -> Option<i128> {
    let r = words::constant_in(e, &f.ty)?.rat();
    let q = match &f.ty {
        TypeExpr::Num(u) => r.div(words::step(u)),
        _ => r,
    };
    q.is_int().then_some(q.num)
}

/// The ends a test of a column holds a number or a date to; (None, None) for one that holds it to
/// nothing this reads.
fn interval(f: &Field, t: &Test) -> (Option<i128>, Option<i128>) {
    let (mut lo, mut hi): (Option<i128>, Option<i128>) = (None, None);
    let mut up = |v: i128| lo = Some(lo.map_or(v, |x: i128| x.max(v)));
    let mut down = |v: i128| hi = Some(hi.map_or(v, |x: i128| x.min(v)));
    match t {
        Test::Any => {}
        Test::Compare(cs) => {
            for (op, e) in cs {
                let Some(c) = wire(f, e) else { continue };
                match op {
                    CmpOp::Lt => down(c - 1),
                    CmpOp::Le => down(c),
                    CmpOp::Gt => up(c + 1),
                    CmpOp::Ge => up(c),
                    CmpOp::Is => {
                        up(c);
                        down(c);
                    }
                    CmpOp::IsNot => {}
                }
            }
        }
        Test::Values { values, not: false } => {
            let cs: Vec<i128> = values.iter().filter_map(|e| wire(f, e)).collect();
            if cs.len() == values.len()
                && let (Some(a), Some(b)) = (cs.iter().min(), cs.iter().max())
            {
                up(*a);
                down(*b);
            }
        }
        Test::Values { .. } => {}
    }
    (lo, hi)
}

/// The values a test of a column lets a choice take, of `all`.
fn chosen(all: &[String], t: &Test) -> Vec<String> {
    match t {
        Test::Values { values, not } => {
            let named: Vec<&str> = values
                .iter()
                .filter_map(|e| match &e.kind {
                    ExprKind::Path(p) if p.len() == 1 => Some(p[0].text.as_str()),
                    _ => None,
                })
                .collect();
            if named.len() != values.len() {
                return all.to_vec();
            }
            all.iter().filter(|v| named.contains(&v.as_str()) != *not).cloned().collect()
        }
        _ => all.to_vec(),
    }
}

/// What the field can hold on every request: the form's range, a choice's values.
fn whole(f: &Field) -> Dom {
    let r = crate::ports::field(f);
    Dom { lo: r.lo, hi: r.hi, values: f.options.iter().map(|o| o.name.text.clone()).collect(), narrowed: false }
}

/// What the field can hold on the rows `rows` of the table `t`.
fn on_rows(f: &Field, t: &RouteTable, rows: &[&RouteRow]) -> Dom {
    let all = whole(f);
    let Some(col) = t.inputs.iter().position(|p| p.len() == 1 && p[0].text == f.name.text) else { return all };
    let tests: Vec<&Test> = rows.iter().filter_map(|r| r.tests.get(col).map(|(t, _)| t)).collect();
    match &f.ty {
        TypeExpr::Num(_) | TypeExpr::Date => {
            let each: Vec<(Option<i128>, Option<i128>)> = tests.iter().map(|t| interval(f, t)).collect();
            // the hull of the rows: open where a row is
            let lo = each.iter().map(|(l, _)| *l).collect::<Option<Vec<i128>>>().and_then(|v| v.into_iter().min());
            let hi = each.iter().map(|(_, h)| *h).collect::<Option<Vec<i128>>>().and_then(|v| v.into_iter().max());
            let lo = match (all.lo, lo) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            };
            let hi = match (all.hi, hi) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            let narrowed = lo != all.lo || hi != all.hi;
            Dom { lo, hi, values: all.values, narrowed }
        }
        TypeExpr::Choice => {
            let values: Vec<String> = all.values.iter().filter(|v| tests.iter().any(|t| chosen(&all.values, t).contains(v))).cloned().collect();
            let narrowed = values.len() != all.values.len();
            Dom { values, narrowed, ..all }
        }
        _ => all,
    }
}

fn dom(s: &Scope, f: &Field, at: &Place) -> Dom {
    match at {
        Place::Rows(t, rows) => on_rows(f, t, rows),
        Place::When { conds, .. } => {
            // the hull of the conditions: open where one is
            let each: Vec<Dom> = conds.iter().map(|c| where_holds(s, f, c)).collect();
            let all = whole(f);
            let lo = each.iter().map(|d| d.lo).collect::<Option<Vec<i128>>>().and_then(|v| v.into_iter().min());
            let hi = each.iter().map(|d| d.hi).collect::<Option<Vec<i128>>>().and_then(|v| v.into_iter().max());
            let values: Vec<String> = all.values.iter().filter(|v| each.iter().any(|d| d.values.contains(v))).cloned().collect();
            let narrowed = lo != all.lo || hi != all.hi || values.len() != all.values.len();
            Dom { lo, hi, values, narrowed }
        }
        _ => whole(f),
    }
}

/// Whether a condition holds on every request of a place.
enum Holds {
    Always,
    /// It can fail: an example, in words, with the row of the route it is on.
    CanFail(Lined),
    Unfollowed,
}

/// Whether the condition holds for whatever the field it reads can hold (`d`).
fn atom(s: &Scope, e: &Expr, d: &dyn Fn(&Field) -> Dom) -> Holds {
    let ExprKind::Cmp(op, a, b) = &e.kind else { return Holds::Unfollowed };
    let ExprKind::Path(p) = &a.kind else { return Holds::Unfollowed };
    let Some(f) = p.first().filter(|_| p.len() == 1).and_then(|w| s.field(&w.text)) else { return Holds::Unfollowed };
    let n = &f.name.text;
    match (&f.ty, &b.kind) {
        (TypeExpr::Choice, ExprKind::Path(v)) if v.len() == 1 && matches!(op, CmpOp::Is | CmpOp::IsNot) => {
            let (v, vals) = (&v[0].text, d(f).values);
            let fails = vals.iter().find(|x| (*x == v) != (*op == CmpOp::Is));
            match fails {
                Some(x) => Holds::CanFail(Lined::plain(tr!("`{n}` が `{x}` のとき", "when `{n}` is `{x}`"))),
                None => Holds::Always,
            }
        }
        (TypeExpr::Num(_) | TypeExpr::Date, _) => {
            let Some(c) = wire(f, b) else { return Holds::Unfollowed };
            let Dom { lo, hi, .. } = d(f);
            // a value of the field the comparison fails for, when there is one
            let fails: Option<Option<i128>> = match op {
                CmpOp::Gt => (lo.is_none_or(|l| l <= c)).then_some(lo),
                CmpOp::Ge => (lo.is_none_or(|l| l < c)).then_some(lo),
                CmpOp::Lt => (hi.is_none_or(|h| h >= c)).then_some(hi),
                CmpOp::Le => (hi.is_none_or(|h| h > c)).then_some(hi),
                CmpOp::Is => (lo != Some(c) || hi != Some(c)).then_some(if lo == Some(c) { hi } else { lo }),
                CmpOp::IsNot => (lo.is_none_or(|l| l <= c) && hi.is_none_or(|h| h >= c)).then_some(Some(c)),
            };
            let w = crate::types::written(s.src, e.span);
            match fails {
                None => Holds::Always,
                Some(Some(v)) => {
                    let shown = shown(f, v);
                    Holds::CanFail(Lined::plain(tr!("`{n}` が {shown} のとき（`{w}` が成り立ちません）", "when `{n}` is {shown} (`{w}` does not hold)")))
                }
                Some(None) => Holds::CanFail(Lined::plain(tr!("`{w}` が成り立たないとき", "when `{w}` does not hold"))),
            }
        }
        _ => Holds::Unfollowed,
    }
}

/// A value on the wire as the form writes it: `30,000JPY`, `2026-04-01`.
fn shown(f: &Field, v: i128) -> String {
    match &f.ty {
        TypeExpr::Num(u) => words::number(ritsu_units::Rat::int(v).mul(words::step(u)), u),
        TypeExpr::Date => crate::lex::date_text(v as i64),
        _ => v.to_string(),
    }
}

/// Whether the condition holds on every request of the place: an atom over one field, or atoms
/// joined with `and`.
fn condition(s: &Scope, e: &Expr, at: &Place) -> Holds {
    if let ExprKind::And(a, b) = &e.kind {
        return match (condition(s, a, at), condition(s, b, at)) {
            (Holds::CanFail(x), _) | (_, Holds::CanFail(x)) => Holds::CanFail(x),
            (Holds::Always, Holds::Always) => Holds::Always,
            _ => Holds::Unfollowed,
        };
    }
    match at {
        Place::Unfollowed(_) => Holds::Unfollowed,
        Place::Everywhere => atom(s, e, &whole),
        Place::When { conds, .. } => {
            // a failure counts where the place leaves the field no less than this reads
            let firm = compared(s, e).is_some_and(|(f, _, _)| less(s, at, Some(f)).is_none());
            let mut unfollowed = false;
            for c in conds {
                match atom(s, e, &|f| where_holds(s, f, c)) {
                    Holds::Always => {}
                    Holds::CanFail(x) if firm => return Holds::CanFail(x),
                    Holds::CanFail(_) | Holds::Unfollowed => unfollowed = true,
                }
            }
            if unfollowed { Holds::Unfollowed } else { Holds::Always }
        }
        Place::Rows(t, rows) => {
            let mut unfollowed = false;
            for r in rows {
                match atom(s, e, &|f| on_rows(f, t, &[r])) {
                    Holds::Always => {}
                    Holds::CanFail(x) => {
                        let l = r.line;
                        // the row in the words a message says, and apart from those `ringi api` writes
                        return Holds::CanFail(Lined {
                            text: tr!("経路の行で、{}", "{}, on the row of the route", x.text.ja; x.text.en),
                            lines: x.lines.iter().copied().chain([l]).collect(),
                            said: tr!("経路の {l} 行目で、{}", "{}, on the row of the route on line {l}", x.said.ja; x.said.en),
                        });
                    }
                    Holds::Unfollowed => unfollowed = true,
                }
            }
            if unfollowed { Holds::Unfollowed } else { Holds::Always }
        }
    }
}

/// Whether a field can have no value where a flow is given it: None for one that always has one.
fn lacks(s: &Scope, f: &Field, at: &Place) -> Found<Option<Lined>> {
    if f.computed.is_some() || matches!(f.ty, TypeExpr::Rows) {
        return Found::Value(None);
    }
    let n = &f.name.text;
    let conds: Vec<(&Expr, &str)> = [(f.shown_when.as_ref(), "shown when"), (f.required_when.as_ref(), "required when")].into_iter().filter_map(|(c, w)| Some((c?, w))).collect();
    let plain = f.optional.is_none() && f.approvers_only.is_none() && conds.is_empty();
    if plain {
        return Found::Value(None);
    }
    // a place that asks for the value has it
    let asks = |c: &&Expr| {
        let mut ps = Vec::new();
        parts(c, &mut ps);
        ps.iter().any(|p| matches!(compared(s, p), Some((g, CmpOp::IsNot, b)) if g.name.text == f.name.text && matches!(b.kind, ExprKind::None)))
    };
    if let Place::When { conds, .. } = at
        && conds.iter().all(asks)
    {
        return Found::Value(None);
    }
    if let Place::Unfollowed(why) = at {
        return Found::Undecided(tr!("`{n}` に値があるかは、{}によります", "whether `{n}` has a value there rests on {}", why.ja; why.en));
    }
    if f.approvers_only.is_some() {
        return Found::Value(Some(Lined::plain(tr!("`{n}` は承認者だけが入れるフィールドで、承認者が入れるまで値がありません", "`{n}` is filled in by approvers only, and has no value until one does"))));
    }
    if f.optional.is_some() {
        return Found::Value(Some(Lined::plain(tr!("`{n}` は `optional` で、空のまま出せます", "`{n}` is `optional`, and a request may leave it empty"))));
    }
    let mut unfollowed = None;
    for (c, word) in conds {
        let w = crate::types::written(s.src, c.span);
        match condition(s, c, at) {
            Holds::Always => {}
            Holds::CanFail(x) => return Found::Value(Some(x.within(|x| tr!("{}、`{n}` に値がありません（`{word} {w}`）", "{}, `{n}` has no value (`{word} {w}`)", x.ja; x.en)))),
            Holds::Unfollowed => unfollowed = Some(tr!("`{n}` に値があるかは、`{word} {w}` によります", "whether `{n}` has a value there rests on `{word} {w}`")),
        }
    }
    match unfollowed {
        Some(why) => Found::Undecided(why),
        None => Found::Value(None),
    }
}

/// What a field carries where a flow is given it: its type with what the place holds it to.
fn carried(s: &Scope, f: &Field, at: &Place) -> (Found<Carried>, bool) {
    let d = dom(s, f, at);
    let c = match &f.ty {
        TypeExpr::Text | TypeExpr::LongText | TypeExpr::Attachment | TypeExpr::Person | TypeExpr::Team => Carried::Str,
        TypeExpr::Bool => Carried::Bool,
        TypeExpr::Num(u) => Carried::Num { written: crate::ast::unit_type(u), unit: Some(u.clone()), min: d.lo, max: d.hi },
        TypeExpr::Date => Carried::Date { min: d.lo.map(|x| x as i64), max: d.hi.map(|x| x as i64) },
        TypeExpr::Choice => {
            let values = f.options.iter().filter(|o| d.values.contains(&o.name.text)).map(|o| Spelled { written: o.name.text.clone(), public: o.name.ascii().to_string() }).collect();
            Carried::Enum { name: f.name.ascii().to_string(), values }
        }
        TypeExpr::ChoiceFrom(p) => match p.first().and_then(|a| s.rule_facts(&a.text)).zip(p.get(1)) {
            // a rule's enum: its values, by the names the rule gives them
            Some((facts, name)) => match facts.enums.iter().find(|e| e.name == name.text || e.alias == name.text) {
                Some(e) => Carried::Enum { name: e.name.clone(), values: e.values.iter().map(|v| Spelled { written: v.name.clone(), public: v.public.clone() }).collect() },
                None => Carried::Str,
            },
            // a row of a master, by its key
            None => Carried::Str,
        },
        TypeExpr::Rows => Carried::Other("rows".into()),
    };
    (Found::Value(c), d.narrowed)
}

/// A value a `launch` writes: what it carries, whether it can be none, whether the place holds it
/// to less than the form does, and what may hold it to less still, which ringi does not follow.
fn written_value<'a>(s: &Scope<'a>, e: &Expr, at: &Place<'a>) -> (Found<Carried>, Found<Option<Lined>>, bool, Option<Text>) {
    // what the form does not decide is the same wherever it is given
    let apart = |(c, none): (Found<Carried>, Found<Option<Lined>>)| (c, none, false, None);
    let w = crate::types::written(s.src, e.span);
    let always = Found::Value(None);
    apart(match &e.kind {
        ExprKind::Str(_) | ExprKind::Person(_) | ExprKind::Team(_) => (Found::Value(Carried::Str), always),
        ExprKind::Bool(_) => (Found::Value(Carried::Bool), always),
        ExprKind::Date(d) => (Found::Value(Carried::Date { min: Some(*d), max: Some(*d) }), always),
        ExprKind::Path(p) => match (p[0].text.as_str(), p.get(1).map(|x| x.text.as_str())) {
            (name, None) if s.field(name).is_some() => {
                let f = s.field(name).expect("a field");
                let (c, narrowed) = carried(s, f, at);
                return (c, lacks(s, f, at), narrowed, less(s, at, Some(f)));
            }
            ("requester", None) | ("requester", Some("id")) | ("ticket", Some("id")) | ("ticket", Some("attempt")) => (Found::Value(Carried::Str), always),
            // a ticket is in its first round, and one more each time it comes back from its requester
            ("ticket", Some("round")) => {
                let backs = s.file.ticket.iter().flat_map(|t| &t.lines).find_map(|l| if let TicketLine::SendBack { times, .. } = l { Some(*times) } else { None }).unwrap_or(0);
                (Found::Value(Carried::whole("number", Some(1), Some(i128::from(backs) + 1))), always)
            }
            // a line kept to the requesters who have the team gives it a value every time
            ("requester", Some(kind)) if matches!(at, Place::When { conds, .. } if conds.iter().all(|c| kinds_asked(c).contains(&kind))) => (Found::Value(Carried::Str), always),
            ("requester", Some(kind)) => {
                let none = match without(s, kind) {
                    Found::Undecided(why) => Found::Undecided(tr!("`{w}` がだれにもあるかを決められません（{}）", "whether every requester has `{w}` is not decided ({})", why.ja; why.en)),
                    Found::Value(vs) => match vs.iter().find_map(|v| v.without.first().map(|id| (id.clone(), v.from))) {
                        Some((id, from)) => {
                            let day = crate::lex::date_text(from);
                            Found::Value(Some(Lined::plain(tr!("{day} からの組織の {id} には、上に `{kind}` のチームが無く、`{w}` に値がありません", "{id}, in the organization from {day}, has no team of the kind `{kind}` above, and `{w}` has no value"))))
                        }
                        None => Found::Value(None),
                    },
                };
                (Found::Value(Carried::Str), none)
            }
            (alias, Some(out)) if s.rule_facts(alias).is_some() => {
                let facts = s.rule_facts(alias).expect("a rule");
                // a rule decides its output from the form, which the place may hold to less
                return match facts.outputs.iter().find(|c| c.name == out || c.alias == out) {
                    Some(c) => (Found::Value(Carried::of_column(facts, &c.ty)), always, false, less(s, at, None)),
                    None => (Found::Undecided(tr!("`{w}` が何になるかを、ringi はここでは追いません", "ringi does not follow what `{w}` comes to here")), always, false, None),
                };
            }
            _ => (Found::Undecided(tr!("`{w}` が何になるかを、ringi はここでは追いません", "ringi does not follow what `{w}` comes to here")), always),
        },
        _ => (Found::Undecided(tr!("`{w}` が何になるかを、ringi はここでは追いません", "ringi does not follow what `{w}` comes to here")), always),
    })
}

/// The ticket's ID and every field of the form, as an agent's stage and a summary are given them
/// ([`crate::engine`]'s `flow_args`).
fn form_given<'a>(s: &Scope<'a>, at: &Place<'a>, line: usize) -> Vec<FlowGiven> {
    let how = held_where(at);
    let mut out = vec![FlowGiven { name: "request_id".into(), written: "ticket.id".into(), carried: Found::Value(Carried::Str), none: Found::Value(None), narrowed: None, may_be_less: None, line }];
    for f in s.fields() {
        let (c, narrowed) = carried(s, f, at);
        out.push(FlowGiven {
            name: f.name.ascii().to_string(),
            written: f.name.text.clone(),
            carried: c,
            none: lacks(s, f, at),
            narrowed: how.clone().filter(|_| narrowed),
            may_be_less: less(s, at, Some(f)),
            line: f.line,
        });
    }
    out
}

/// Where a place holds a value to less than the form does, in words, with the rows of the route
/// it is: None for one that does not.
fn held_where(at: &Place) -> Option<Lined> {
    match at {
        Place::Rows(_, rows) => {
            let ls = rows.iter().map(|r| r.line.to_string()).collect::<Vec<_>>().join(", ");
            let one = rows.len() == 1;
            Some(Lined {
                text: if one { tr!("経路の行", "on the row of the route") } else { tr!("経路の行", "on the rows of the route") },
                lines: rows.iter().map(|r| r.line).collect(),
                said: if one { tr!("経路の {ls} 行目", "on the row of the route on line {ls}") } else { tr!("経路の {ls} 行目", "on the rows of the route on lines {ls}") },
            })
        }
        Place::When { word, .. } => Some(Lined::plain(tr!("{}が成り立つ所", "where {} holds", word.ja; word.en))),
        _ => None,
    }
}

/// Whether a cell names the way to choose.
fn names(spec: &Spec, way: &str) -> bool {
    spec.who().iter().any(|w| w.text == way)
}

/// Where the stages the way `way` answers come: the rows of the route's table whose cells name it;
/// None for a way no stage names.
fn place_of_way<'a>(s: &'a Scope, way: &str) -> Option<Place<'a>> {
    let f = s.file;
    // a way another line can send a stage to is asked where that line says, which is not a row
    let by_table = f.tables.iter().any(|t| t.rows.iter().any(|r| r.stages.iter().any(|(sp, _)| names(sp, way))));
    let every = f.every_stage.iter().chain(match &f.route {
        Some(Route::Stages(ls)) => ls.iter().filter_map(|l| l.deadlines.as_ref()).collect::<Vec<_>>(),
        _ => Vec::new(),
    });
    let at_top = every.flat_map(|d| d.lines.iter()).any(|l| matches!(l, crate::ast::Deadline::Empty { what: w @ WhenEmpty::Escalate(_), .. } if w.at_top_way().is_some_and(|x| x.text == way)));
    let by_else = f.who.iter().any(|w| matches!(&w.how, How::Agent { otherwise, .. } if otherwise.text == way));
    let elsewhere = || tr!("ほかの行がこの決め方へ回す所", "where another line sends a stage to the way");
    match &f.route {
        Some(Route::Table(t)) if !t.chooses_route => {
            let rows: Vec<&RouteRow> = t.rows.iter().filter(|r| r.stages.iter().any(|(sp, _)| names(sp, way))).collect();
            if t.rows.iter().any(|r| r.stages.iter().any(|(sp, _)| matches!(sp, Spec::RuleOutput(_)))) {
                return Some(Place::Unfollowed(tr!("規則の出力が決め方を決める行", "the rows where a rule's output names the way")));
            }
            if by_table || at_top || by_else {
                return Some(Place::Unfollowed(elsewhere()));
            }
            (!rows.is_empty()).then_some(Place::Rows(t, rows))
        }
        Some(Route::Stages(ls)) => {
            let mine: Vec<_> = ls.iter().filter(|l| names(&l.spec, way)).collect();
            if by_table || at_top || by_else {
                return Some(Place::Unfollowed(elsewhere()));
            }
            if mine.is_empty() {
                return None;
            }
            // every stage that names the way has a condition: the way is asked where one of them holds
            Some(match mine.iter().map(|l| l.when.as_ref()).collect::<Option<Vec<&Expr>>>() {
                Some(conds) => place_where(s, conds, tr!("ステップの `when` の条件", "the `when` of the stage")),
                None => Place::Everywhere,
            })
        }
        Some(_) => Some(Place::Unfollowed(tr!("共有の経路", "the shared route"))),
        None => None,
    }
}

/// The flows the ticket starts and asks.
fn flows(s: &Scope, lines: &[TicketLine]) -> Vec<TicketFlow> {
    let f = s.file;
    let mut out = Vec::new();
    // the agents, in the order of `who`
    for w in &f.who {
        let How::Agent { flow, .. } = &w.how else { continue };
        let (Some(path), Some(at)) = (used(s, &flow.text, UseKind::Flow), place_of_way(s, &w.name.text)) else { continue };
        out.push(TicketFlow { kind: "agent".into(), alias: flow.text.clone(), flow: path, line: w.line, at: vec![w.name.text.clone()], given: form_given(s, &at, w.line) });
    }
    // the summaries, each stage once
    let mut noted: Vec<(String, String, usize, Place)> = Vec::new();
    match &f.route {
        Some(Route::Table(t)) if !t.chooses_route => {
            for (i, stage) in t.stages.iter().enumerate() {
                let rows: Vec<&RouteRow> = t.rows.iter().filter(|r| matches!(r.stages.get(i), Some((Spec::Note(_), _)))).collect();
                let Some(Some((Spec::Note(fl), _))) = rows.first().map(|r| r.stages.get(i)) else { continue };
                noted.push((stage.text.clone(), fl.text.clone(), rows[0].line, Place::Rows(t, rows)));
            }
        }
        Some(Route::Stages(ls)) => {
            for l in ls {
                let Spec::Note(fl) = &l.spec else { continue };
                let at = match &l.when {
                    Some(c) => place_where(s, vec![c], tr!("ステップの `when` の条件", "the `when` of the stage")),
                    None => Place::Everywhere,
                };
                noted.push((l.name.text.clone(), fl.text.clone(), l.line, at));
            }
        }
        _ => {}
    }
    for (stage, alias, line, at) in noted {
        let Some(path) = used(s, &alias, UseKind::Flow) else { continue };
        out.push(TicketFlow { kind: "summary".into(), alias, flow: path, line, at: vec![stage], given: form_given(s, &at, line) });
    }
    // the flows an ending starts
    for l in lines {
        let TicketLine::On { endings, action: ActionKind::Launch, call, when, line } = l else { continue };
        let alias = &call.target[0].text;
        let Some(path) = used(s, alias, UseKind::Flow) else { continue };
        let at = match when {
            Some(c) => place_where(s, vec![c], tr!("`on` の行の `when` の条件", "the `when` of the `on` line")),
            None => Place::Everywhere,
        };
        let how = held_where(&at);
        let given = call
            .args
            .iter()
            .map(|a| {
                let (carried, none, narrowed, may_be_less) = written_value(s, &a.value, &at);
                FlowGiven { name: a.name.text.clone(), written: crate::types::written(s.src, a.value.span), carried, none, narrowed: how.clone().filter(|_| narrowed), may_be_less, line: a.value.span.line }
            })
            .collect();
        out.push(TicketFlow { kind: "launch".into(), alias: alias.clone(), flow: path, line: *line, at: endings.iter().map(|(e, _)| e.word().to_string()).collect(), given });
    }
    out
}
