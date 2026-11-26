//! The ticket machine (DESIGN 7, 10.1, 10.2; PLAN C3.5): the reference interpreter's `start` and
//! `next`, and the scenarios that run on them.
//!
//! The requests of `tests/machine/` are written for the machine alone, over the organization and
//! the calendar of `examples/`: `desk` has one scenario for each row of the table of transitions
//! (DESIGN 7.3), `top` the stages with nobody at the top of the organization, `late` the late
//! stages with nowhere to go up to, `once` the stages done with an earlier approval, `who` the
//! ones who may submit, `flows` a stage an agent and a person answer, `ranges` the numbers a form
//! takes and works out (`bare` has no scenario), `held` a hold that cannot be made for the ones
//! above every department, `apart` a stage due by one calendar and reminded by another, `last` and
//! `waits` the stages that wait for a later one (`same approver approves once, at the last stage`),
//! and `desk.ja`, `held.ja` and `last.ja` are `desk`, `held` and `last` in Japanese names. Every
//! scenario of them and of the five examples of `examples/` goes as written, and what `ringi run`
//! prints for each is held in `tests/golden/run/` (`RINGI_BLESS=1 cargo test -p ringi --test
//! machine` writes them again).
//!
//! A row of the table is tested by the transition a scenario's row makes: the state before, the
//! state after, and what the transition gives to do, read from the machine's own answer.

mod common;

use ringi::check::{Options, Outcome, Run};
use ringi::engine::{Effect, Event, Machine, Revision, Submit, Ticket, What};
use ringi::org::Org;
use ringi::scenario::{self, Input, Ran, Stepped};
use ritsu_base::json::{self, Json};
use ritsu_base::text::Lang;
use ritsu_ports::{Value, Values};

/// The business days the slowest walk of the desk request takes, of the 74 the check counts.
const LONGEST_DESK_WALK: u32 = 73;

/// The business days the slowest walk of `waits` takes, of the 95 the check counts.
const LONGEST_WAITS_WALK: u32 = 94;

/// The requests written for the machine.
const MACHINE: [&str; 19] = ["absent", "ahead", "apart", "behind", "deep", "desk", "flows", "held", "last", "late", "once", "ranges", "reminds", "seat", "steps", "supplies", "top", "waits", "who"];
/// The examples of `examples/`.
const EXAMPLES: [&str; 5] = ["account", "contract", "expense", "leave", "purchase"];

fn cli(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = ringi::run::run(&args, common::joined(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// The machine of the request at `path` and the organization it reads, as `ringi run` makes them:
/// the file checked once, its approvers unlisted and its scenarios unrun.
fn with<R>(path: &str, f: impl FnOnce(&Machine, &Org) -> R) -> R {
    let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
    let o = run.file(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert!(!o.has_errors(), "{path}:\n{}", common::shown(&o, Lang::En));
    ringi::engine::with(&o, &run, f).unwrap_or_else(|t| panic!("{path}: {}", t.en))
}

fn machine_file(name: &str) -> String {
    format!("tests/machine/{name}.ringi")
}

/// The scenario `name` of the machine `m`, run.
fn ran(m: &Machine, org: &Org, name: &str) -> Ran {
    let sc = m.s.file.scenarios.iter().find(|sc| sc.name.text == name).unwrap_or_else(|| panic!("no scenario `{name}`"));
    let r = scenario::run(m, org, sc);
    assert!(r.stop.is_none(), "`{name}` stops: {}", r.stop.as_ref().unwrap().text.en);
    r
}

/// An instant: a day and a time in the offset of the request's calendars.
fn at(m: &Machine, day: &str, time: &str) -> i64 {
    let (h, min) = time.split_once(':').unwrap();
    m.instant(ritsu_ports::day_of(day).unwrap(), h.parse::<u32>().unwrap() * 60 + min.parse::<u32>().unwrap())
}

/// Where a ticket is: its state, and the stage it is at.
fn place(t: Option<&Ticket>) -> String {
    match t {
        None => "(none)".to_string(),
        Some(t) => match t.stage_name() {
            Some(s) => format!("{} at {s}", t.status.word()),
            None => t.status.word().to_string(),
        },
    }
}

/// One thing to do, in short: `record(approved)`, `notify(waiting: e1101)`, `set(late)`.
fn short(fx: &Effect) -> String {
    match &fx.what {
        What::Record(r) => format!("record({})", r.did),
        What::CancelTimer { timer, .. } => format!("cancel({})", timer.word()),
        What::Book { op, book, transfer, .. } => format!("{op}({book}.{transfer})"),
        What::Launch { flow, .. } => format!("launch({flow})"),
        What::AskAgent { flow, .. } => format!("ask agent({flow})"),
        What::AskNote { flow, .. } => format!("ask note({flow})"),
        What::Notify { to, notice, .. } => format!("notify({}: {})", notice.word(), to.join(", ")),
        What::SetTimer { timer, .. } => format!("set({})", timer.word()),
    }
}

/// A transition as the machine made it, on one line: the event and who it is by, the state before
/// and after, and what it gives to do; or the kind it was refused with.
fn went(s: &Stepped) -> String {
    let by = s.input.by().map(|b| format!(" by {b}")).unwrap_or_default();
    match &s.answer {
        Ok((t, fx)) => format!("{}{by}: {} -> {} | {}", s.input.word(), place(s.before.as_ref()), place(Some(t)), fx.iter().map(short).collect::<Vec<_>>().join(" ")),
        Err(r) => format!("{}{by}: {} refused ({})", s.input.word(), place(s.before.as_ref()), r.kind),
    }
}

/// The rows of the scenario `name` of `tests/machine/<file>.ringi`, each as [`went`] writes it.
fn rows(file: &str, name: &str) -> Vec<String> {
    with(&machine_file(file), |m, org| ran(m, org, name).rows.iter().map(|r| went(&r.stepped)).collect())
}

/// The lines of the history a transition gives, each with its note.
fn recorded(s: &Stepped) -> Vec<(String, Option<String>)> {
    let Ok((_, fx)) = &s.answer else { return Vec::new() };
    fx.iter()
        .filter_map(|e| match &e.what {
            What::Record(r) => Some((r.did.to_string(), r.note.clone())),
            _ => None,
        })
        .collect()
}

/// The ticket of the scenario `name` after its row `n` (from 0).
fn ticket_after(m: &Machine, org: &Org, name: &str, n: usize) -> Ticket {
    ran(m, org, name).rows[n].ticket().cloned().unwrap_or_else(|| panic!("`{name}` row {n} has no ticket"))
}

/// One event given to the ticket `t` at `now`: the machine's answer on one line, and the ticket
/// it comes to.
fn give(m: &Machine, org: &Org, t: &Ticket, ev: Event, now: i64) -> (String, Option<Ticket>) {
    let s = scenario::step(m, org, Some(t), Input::Event(ev), now).unwrap_or_else(|e| panic!("{}", e.en));
    (went(&s), s.answer.ok().map(|(t, _)| t))
}

fn approve(by: &str) -> Event {
    Event::Approve { by: by.to_string() }
}

// ── Every scenario goes as written ──

/// What `ringi run` prints for the requests written for the machine, in both languages: every
/// scenario as written (exit 0).
#[test]
fn the_requests_written_for_the_machine_run_as_written() {
    for name in MACHINE {
        for lang in [Lang::En, Lang::Ja] {
            let (code, out, err) = cli(&["run", &machine_file(name), "--lang", lang.code()]);
            assert_eq!((code, err.as_str()), (0, ""), "{name}:\n{out}");
            ritsu_testkit::golden(format!("tests/golden/run/machine/{name}.{}.txt", lang.code()), &out);
        }
    }
    // the requests over two offsets, the desk request, the one that holds a budget, the one
    // answered at the last stage and the one whose fields are made empty, in Japanese names
    for name in ["ahead.ja", "behind.ja", "desk.ja", "held.ja", "last.ja", "supplies.ja"] {
        let (code, out, err) = cli(&["run", &machine_file(name), "--lang", "ja"]);
        assert_eq!((code, err.as_str()), (0, ""), "{name}:\n{out}");
        ritsu_testkit::golden(format!("tests/golden/run/machine/{name}.ja.txt"), &out);
    }
}

/// What the check says of them: `top` is written with stages that stop for the president, which
/// the check says (E502) and `run` shows as they come; the others pass, and the check runs their
/// scenarios with nothing to say (no E602, E603 or E604).
#[test]
fn what_the_check_says_of_the_requests_written_for_the_machine() {
    let run = Run::new(common::joined());
    let errors = |o: &Outcome| common::codes(o).into_iter().filter(|c| c.starts_with('E')).collect::<Vec<_>>();
    for name in MACHINE {
        let o = run.file(&machine_file(name)).unwrap();
        let want: &[&str] = if name == "top" { &["E502", "E502", "E502"] } else { &[] };
        assert_eq!(errors(&o), want, "{name}:\n{}", common::shown(&o, Lang::En));
    }
    // the stages of `top` that stop, each for the president alone
    let top = run.file(&machine_file("top")).unwrap();
    let said: Vec<String> = top.diags.iter().filter(|d| d.code == "E502").map(|d| d.message.en.clone()).collect();
    for stage in ["stopped", "nobody", "stays"] {
        assert!(said.iter().any(|s| s.contains(&format!("`{stage}`"))), "{stage}: {said:?}");
    }
}

/// The scenarios of the five examples of `examples/` go as written (PLAN C3.5), in English names and
/// in Japanese names; what `ringi run` prints is held for each, and its JSON for the leave request.
#[test]
fn the_examples_run_as_written() {
    for name in EXAMPLES {
        for (file, lang) in [(format!("examples/{name}.ringi"), Lang::En), (format!("examples/{name}.ja.ringi"), Lang::Ja)] {
            let (code, out, err) = cli(&["run", &file, "--lang", lang.code()]);
            assert_eq!((code, err.as_str()), (0, ""), "{file}:\n{out}");
            assert!(out.lines().last().is_some_and(|l| l.contains(if lang == Lang::Ja { "書いたとおり" } else { "as written" })), "{file}:\n{out}");
            ritsu_testkit::golden(format!("tests/golden/run/{name}.{}.txt", lang.code()), &out);
        }
    }
    let (code, out, _) = cli(&["run", "examples/leave.ringi", "--format", "json"]);
    assert_eq!(code, 0, "{out}");
    json::parse(&out).unwrap_or_else(|e| panic!("{e:?}:\n{out}"));
    ritsu_testkit::golden("tests/golden/run/leave.json", &out);
}

/// `json` without what an example in Japanese names writes otherwise than its English version: the
/// sentences for people and the ones people write (`comment`), the values of the form and of the
/// calls, the request's own name, the ASCII names the two versions choose for their ways, and the
/// proxies, which the organization's data gives by the request's name; and the people's names in
/// Japanese (`name_ja`), which the data gives whatever names the request is written in.
fn bare(json: &Json) -> Json {
    const LEFT_OUT: [&str; 15] = ["text", "note", "form", "fields", "args", "outputs", "request", "value", "way", "key", "part", "proxies", "reason", "comment", "name_ja"];
    match json {
        Json::Obj(pairs) => Json::Obj(pairs.iter().filter(|(k, _)| !LEFT_OUT.contains(&k.as_str())).map(|(k, v)| (k.clone(), bare(v))).collect()),
        Json::Arr(items) => Json::Arr(items.iter().map(bare).collect()),
        other => other.clone(),
    }
}

/// The JSON of a transition takes the ASCII names (DESIGN 3.4): a request in Japanese names writes
/// the stages, the books and the flows of its tickets, events and things to do as its English
/// version writes them, and no name in Japanese is left in them.
#[test]
fn a_request_in_japanese_names_writes_the_same_json() {
    let pairs = EXAMPLES.iter().map(|name| (name.to_string(), format!("examples/{name}.ringi"), format!("examples/{name}.ja.ringi"))).chain(["desk", "held", "last", "supplies"].map(|name| (name.to_string(), machine_file(name), machine_file(&format!("{name}.ja")))));
    for (name, en, ja) in pairs {
        let steps = |file: String| with(&file, |m, org| m.s.file.scenarios.iter().flat_map(|sc| scenario::run(m, org, sc).rows).map(|r| r.stepped.to_json(&m.names)).collect::<Vec<_>>());
        let (en, ja) = (steps(en), steps(ja));
        assert!(!en.is_empty(), "{name}");
        assert_eq!(en.len(), ja.len(), "{name}");
        for (n, (e, j)) in en.iter().zip(&ja).enumerate() {
            assert_eq!(bare(e).pretty(), bare(j).pretty(), "{name}, row {n}");
            // what is left out above is values and sentences: every name that stays is ASCII
            let names = bare(j).compact();
            assert!(names.is_ascii(), "{name}, row {n}: {names}");
        }
    }
    // the names an example in Japanese names gives its ways are the ASCII ones too
    with("examples/account.ja.ringi", |m, org| {
        let made = ran(m, org, "管理者の権限の申請").rows[0].stepped.to_json(&m.names);
        let ticket = made.get("answer").and_then(|a| a.get("ok")).and_then(|a| a.get("ticket")).unwrap();
        assert_eq!(ticket.get("stage").and_then(Json::as_str), Some("manager"));
        let part = &ticket.get("stages").and_then(Json::as_arr).unwrap()[0].get("parts").and_then(Json::as_arr).unwrap()[0];
        assert_eq!((part.get("way").and_then(Json::as_str), part.get("key").and_then(Json::as_str)), (Some("manager"), Some("manager")));
        // and the machine holds them as the request writes them, which is what `run` shows
        assert_eq!(ran(m, org, "管理者の権限の申請").rows[0].ticket().unwrap().stage_name(), Some("上長"));
    });
}

// ── The table of transitions (DESIGN 7.3), a test for each row ──

/// (none) -- `submit` --> `in progress` at the first stage: the line of the history, the notice
/// to the stage's approver, the reminder and the deadline.
#[test]
fn row_submit() {
    assert_eq!(rows("desk", "approved_by_a_quorum")[0], "submit by e1201: (none) -> in progress at first | record(submitted) notify(waiting: e1101) set(remind) set(late)");
    // the things `on submitted` says to do come with it: the hold of the leave request
    let leave = with("examples/leave.ringi", |m, org| went(&ran(m, org, "a_long_paid_leave").rows[0].stepped));
    assert!(leave.starts_with("submit by e1203: (none) -> in progress at manager | record(submitted) hold(leave.take) "), "{leave}");
}

/// `approve` that reaches the quorum: the next stage, with its notice and its clocks; with no
/// stage left, `approved`.
#[test]
fn row_approve_reaching_the_quorum() {
    let r = rows("desk", "approved_by_a_quorum");
    assert_eq!(r[1], "approve by e1101: in progress at first -> in progress at board | record(approved) cancel(remind) cancel(late) notify(waiting: e0001, e0201, e0202, e0203, e0204) set(remind) set(late)");
    assert_eq!(r[3], "approve by e0202: in progress at board -> in progress at decide | record(approved) cancel(remind) cancel(late) notify(waiting: e3101) set(remind) set(late)");
    assert_eq!(r[4], "approve by e3101: in progress at decide -> approved | record(approved) record(ticket approved) cancel(remind) cancel(late) notify(decided: e1201)");
    // `on approved` posts what the ticket holds
    let leave = with("examples/leave.ringi", |m, org| went(&ran(m, org, "a_long_paid_leave").rows.last().unwrap().stepped));
    assert!(leave.contains("-> approved | ") && leave.contains(" post(leave.take) "), "{leave}");
}

/// `approve` that does not reach the quorum: the same stage, the answer added, nothing to do but
/// the line of the history; the clocks stay as they were.
#[test]
fn row_approve_short_of_the_quorum() {
    assert_eq!(rows("desk", "approved_by_a_quorum")[2], "approve by e0201: in progress at board -> in progress at board | record(approved)");
    with(&machine_file("desk"), |m, org| {
        let (before, after) = (ticket_after(m, org, "approved_by_a_quorum", 1), ticket_after(m, org, "approved_by_a_quorum", 2));
        let (b, a) = (before.stage().unwrap(), after.stage().unwrap());
        assert_eq!((b.remind, b.due, b.started), (a.remind, a.due, a.started));
        assert_eq!((b.parts[0].yes(), a.parts[0].yes(), a.parts[0].needed), (0, 1, 2));
    });
}

/// `reject`: the ticket is `rejected` when the stage can no longer get the approvals it needs,
/// and stays at the stage while it still can (two of five: three rejections leave two to approve,
/// the fourth leaves one).
#[test]
fn row_reject() {
    let r = rows("desk", "rejected_when_the_quorum_is_out_of_reach");
    for n in 2..5 {
        assert!(r[n].ends_with(": in progress at board -> in progress at board | record(rejected)"), "{}", r[n]);
    }
    assert_eq!(r[5], "reject by e0204: in progress at board -> rejected | record(rejected) record(ticket rejected) cancel(remind) cancel(late) notify(decided: e1201)");
    // a stage one person answers ends with the one rejection
    with(&machine_file("desk"), |m, org| {
        let t = ticket_after(m, org, "approved_by_a_quorum", 0);
        let (line, after) = give(m, org, &t, Event::Reject { by: "e1101".into() }, at(m, "2026-11-16", "11:00"));
        assert_eq!(line, "reject by e1101: in progress at first -> rejected | record(rejected) record(ticket rejected) cancel(remind) cancel(late) notify(decided: e1201)");
        let ended = after.unwrap().ended.unwrap();
        assert_eq!((ended.stage.as_deref(), ended.by.as_deref()), (Some("first"), Some("e1101")));
    });
}

/// `send back` to the requester: `with requester`, the stage's clocks cancelled and the clock of
/// the resubmission set.
#[test]
fn row_send_back_to_the_requester() {
    assert_eq!(rows("desk", "sent_back_and_made_again")[1], "send back by e1101: in progress at first -> with requester | record(sent back) cancel(remind) cancel(late) notify(returned: e1201) set(resubmit late)");
    with(&machine_file("desk"), |m, org| {
        let t = ticket_after(m, org, "sent_back_and_made_again", 1);
        // three business days from Monday the 16th: the end of Thursday the 19th
        assert_eq!((t.backs, t.round, t.resubmit_due), (1, 1, Some(at(m, "2026-11-20", "00:00"))));
    });
}

/// `send back` to an earlier stage: `in progress` at that stage, its answers empty again, its
/// notice and its clocks counted from the time it came back.
#[test]
fn row_send_back_to_a_stage() {
    assert_eq!(rows("desk", "sent_back_to_a_stage")[2], "send back by e0201: in progress at board -> in progress at first | record(sent back) cancel(remind) cancel(late) notify(waiting: e1101) set(remind) set(late)");
    with(&machine_file("desk"), |m, org| {
        let t = ticket_after(m, org, "sent_back_to_a_stage", 2);
        let first = t.stage().unwrap();
        assert!(first.parts.iter().all(|p| p.answers.is_empty()), "{first:?}");
        assert_eq!((t.round, t.backs, first.started), (1, 1, Some(at(m, "2026-11-16", "13:00"))));
        // the stage it left is as it was before the ticket came to it
        let board = t.stages.iter().find(|s| s.name == "board").unwrap();
        assert_eq!((board.phase.word(), board.parts.len(), board.due), ("ahead", 0, None));
    });
}

/// `withdraw`: back with the requester, who makes it again or discards it.
#[test]
fn row_withdraw() {
    assert_eq!(rows("desk", "withdrawn_and_discarded")[1], "withdraw by e1201: in progress at first -> with requester | record(withdrawn) cancel(remind) cancel(late) notify(returned: e1101) set(resubmit late)");
}

/// `retract`: back at the stage before, whose deadline is as it was (the approval was given a day
/// after the stage started, and the deadline is still counted from the start).
#[test]
fn row_retract() {
    assert_eq!(rows("desk", "an_approval_taken_back")[2], "retract by e1101: in progress at board -> in progress at first | record(retracted) cancel(remind) cancel(late) notify(waiting: e1101) set(remind) set(late)");
    with(&machine_file("desk"), |m, org| {
        let (made, back) = (ticket_after(m, org, "an_approval_taken_back", 0), ticket_after(m, org, "an_approval_taken_back", 2));
        let (a, b) = (made.stage().unwrap(), back.stage().unwrap());
        assert_eq!((a.started, a.due), (b.started, b.due));
        assert_eq!(b.due, Some(at(m, "2026-11-20", "00:00")));
        assert!(b.parts[0].answers.is_empty());
    });
}

/// `remind`: the same stage, a reminder to its approvers; the deadline is as it was, and no
/// reminder is set again.
#[test]
fn row_remind() {
    assert_eq!(rows("desk", "late_and_up")[1], "remind: in progress at first -> in progress at first | record(reminded) notify(reminder: e1101)");
    with(&machine_file("desk"), |m, org| {
        let (before, after) = (ticket_after(m, org, "late_and_up", 0), ticket_after(m, org, "late_and_up", 1));
        let (b, a) = (before.stage().unwrap(), after.stage().unwrap());
        assert_eq!((b.remind, a.remind, a.due), (Some(at(m, "2026-11-18", "09:00")), None, b.due));
        assert_eq!(after.timers().len(), 1);
    });
}

/// `late` with `escalate`: the same stage, the head above the one who was late, a new deadline
/// and a new reminder counted from the time it went up (DESIGN 21, item 2), the reminder set
/// after the one it went up to is told.
#[test]
fn row_late_goes_up() {
    assert_eq!(rows("desk", "late_and_up")[2], "late: in progress at first -> in progress at first | record(went up) notify(went up: e3101) set(remind) set(late)");
    with(&machine_file("desk"), |m, org| {
        let (before, after) = (ticket_after(m, org, "late_and_up", 1), ticket_after(m, org, "late_and_up", 2));
        let (b, a) = (before.stage().unwrap(), after.stage().unwrap());
        assert_eq!((b.parts[0].approvers.clone(), b.up, b.due), (vec!["e1101".to_string()], 0, Some(at(m, "2026-11-20", "00:00"))));
        // it went up on Friday the 20th at 00:00: three business days from that day, the end of
        // Wednesday the 25th, and two business days from that day, Tuesday the 24th at 09:00
        assert_eq!((a.parts[0].approvers.clone(), a.up, a.due, a.remind), (vec!["e3101".to_string()], 1, Some(at(m, "2026-11-26", "00:00")), Some(at(m, "2026-11-24", "09:00"))));
        assert_eq!(after.timers().len(), 2);
    });
}

/// `remind` after the stage went up: a reminder to the one it went up to, once; the deadline is
/// as it was. Taken back after that reminder, the stage has its deadline set again and no
/// reminder.
#[test]
fn row_remind_where_it_went_up_to() {
    assert_eq!(rows("desk", "late_and_up")[3], "remind: in progress at first -> in progress at first | record(reminded) notify(reminder: e3101)");
    with(&machine_file("desk"), |m, org| {
        let (before, after) = (ticket_after(m, org, "late_and_up", 2), ticket_after(m, org, "late_and_up", 3));
        let (b, a) = (before.stage().unwrap(), after.stage().unwrap());
        assert_eq!((a.remind, a.due, a.up), (None, b.due, 1));
        assert_eq!(after.timers().len(), 1);
        assert_eq!(give(m, org, &after, Event::Remind, at(m, "2026-11-25", "09:00")).0, "remind: in progress at first refused (clock)");
        let (line, board) = give(m, org, &after, approve("e3101"), at(m, "2026-11-24", "10:00"));
        assert!(line.contains("-> in progress at board | "), "{line}");
        let (line, back) = give(m, org, &board.unwrap(), Event::Retract { by: "e3101".into() }, at(m, "2026-11-24", "11:00"));
        assert_eq!(line, "retract by e3101: in progress at board -> in progress at first | record(retracted) cancel(remind) cancel(late) notify(waiting: e3101) set(late)");
        let back = back.unwrap();
        assert_eq!((back.stage().unwrap().remind, back.stage().unwrap().due), (None, Some(at(m, "2026-11-26", "00:00"))));
    });
}

/// Each time a stage goes up it has a reminder, counted from the time it went up as the stage's
/// reminder is from its start (`reminds`): the reminder of every stage, a reminder of the stage's
/// own, a stage that goes up its own number of times, and a deadline of its own at a time of day.
#[test]
fn a_stage_is_reminded_each_time_it_goes_up() {
    with(&machine_file("reminds"), |m, org| {
        // the clocks of the stage after each row of a scenario: the reminder and the deadline
        let clocks = |name: &str, row: usize| {
            let t = ticket_after(m, org, name, row);
            let st = t.stage().unwrap();
            (st.up, st.remind.map(|r| scenario::local(m, r)), st.due.map(|d| scenario::local(m, d)))
        };
        let both = |up: u8, remind: &str, due: &str| (up, Some(remind.to_string()), Some(due.to_string()));
        let every = "reminded_each_time_it_goes_up";
        assert_eq!(clocks(every, 0), both(0, "2026-11-17 10:00", "2026-11-20 00:00"));
        assert_eq!(clocks(every, 2), both(1, "2026-11-23 10:00", "2026-11-26 00:00"));
        assert_eq!(clocks(every, 4), both(2, "2026-11-27 10:00", "2026-12-02 00:00"));
        let own = "a_reminder_of_its_own";
        assert_eq!(clocks(own, 1), both(0, "2026-11-18 15:00", "2026-11-20 00:00"));
        assert_eq!(clocks(own, 3), both(1, "2026-11-24 15:00", "2026-11-26 00:00"));
        assert_eq!(clocks(own, 5), both(2, "2026-11-30 15:00", "2026-12-02 00:00"));
        let once = "a_way_up_of_its_own";
        assert_eq!(clocks(once, 2), both(0, "2026-11-17 10:00", "2026-11-20 00:00"));
        assert_eq!(clocks(once, 4), both(1, "2026-11-23 10:00", "2026-11-26 00:00"));
        // a deadline at 17:00: it goes up at that time, and the day counted from is that day
        let timed = "a_deadline_of_its_own";
        assert_eq!(clocks(timed, 3), both(0, "2026-11-17 10:00", "2026-11-18 17:00"));
        assert_eq!(clocks(timed, 5), both(1, "2026-11-19 10:00", "2026-11-20 17:00"));
        assert_eq!(clocks(timed, 7), both(2, "2026-11-23 10:00", "2026-11-24 17:00"));
    });
    let r = rows("reminds", "reminded_each_time_it_goes_up");
    assert_eq!(r[2], "late: in progress at plain -> in progress at plain | record(went up) notify(went up: e3101) set(remind) set(late)");
    assert_eq!(r[3], "remind: in progress at plain -> in progress at plain | record(reminded) notify(reminder: e3101)");
    assert_eq!(r[4], "late: in progress at plain -> in progress at plain | record(went up) notify(went up: e0101) set(remind) set(late)");
    assert_eq!(r[5], "remind: in progress at plain -> in progress at plain | record(reminded) notify(reminder: e0101)");
    assert_eq!(r[6], "late: in progress at plain -> expired | record(ticket expired) notify(decided: e1201)");
}

/// A reminder is set where it comes before its deadline (DESIGN 5.10). The stage `tight` is due at
/// 09:00 of the day the reminder of every stage would go at 10:00: no reminder is set where it
/// starts or where it goes up. And a `late` given after its time sets the reminder no earlier than
/// that, and none once the deadline it comes before has passed too.
#[test]
fn no_reminder_is_set_at_or_after_its_deadline() {
    let r = rows("reminds", "no_reminder_at_or_after_the_deadline");
    assert_eq!(r[4], "approve by e1101: in progress at timed -> in progress at tight | record(approved) cancel(remind) cancel(late) notify(waiting: e1101) set(late)");
    assert_eq!(r[5], "late: in progress at tight -> in progress at tight | record(went up) notify(went up: e3101) set(late)");
    assert_eq!(r[6], "late: in progress at tight -> in progress at tight | record(went up) notify(went up: e0101) set(late)");
    with(&machine_file("reminds"), |m, org| {
        for row in [4, 5, 6] {
            let t = ticket_after(m, org, "no_reminder_at_or_after_the_deadline", row);
            assert_eq!((t.stage().unwrap().remind, t.timers().len()), (None, 1), "row {row}");
        }
    });
    with(&machine_file("desk"), |m, org| {
        let reminded = ticket_after(m, org, "late_and_up", 1);
        let up = |day: &str, time: &str| {
            let (line, t) = give(m, org, &reminded, Event::Late, at(m, day, time));
            let t = t.unwrap_or_else(|| panic!("{line}"));
            let st = t.stage().unwrap();
            (st.remind.map(|r| scenario::local(m, r)), st.due.map(|d| scenario::local(m, d)), line.ends_with(" set(remind) set(late)"))
        };
        let some = |x: &str| Some(x.to_string());
        // given two days late: both as they are counted, from the time the deadline passed
        assert_eq!(up("2026-11-22", "00:00"), (some("2026-11-24 09:00"), some("2026-11-26 00:00"), true));
        // given after the reminder's time: the reminder at once, the deadline as it is counted
        assert_eq!(up("2026-11-25", "12:00"), (some("2026-11-25 12:00"), some("2026-11-26 00:00"), true));
        // given after the deadline it gets: that deadline at once, and no reminder before it
        assert_eq!(up("2026-11-27", "10:00"), (None, some("2026-11-27 10:00"), false));
    });
}

/// `simulate` shows the reminder of the first stage where the machine sets one, at the day and the
/// time the machine sets it for: a reminder of the stage's own with the deadline of every stage,
/// the reminder of every stage with a deadline of the stage's own, and none where the reminder
/// does not come before the deadline.
#[test]
fn simulate_shows_the_reminder_the_machine_sets() {
    use ringi::simulate::{Ask, On, simulate};
    let file = machine_file("reminds");
    let src = std::fs::read_to_string(&file).unwrap();
    // the request without its scenarios, which name the stages of its own route
    let src = &src[..src.find("\n# ").unwrap()];
    let (from, to) = (src.find("route\n").unwrap(), src.find("who\n").unwrap());
    let day = ritsu_ports::day_of("2026-11-16").unwrap();
    for (first, said, clocks) in [
        ("  own     manager\n    remind after 2 business days at 15:00\n", "due 2026-11-19 at the end of the day, reminder on 2026-11-18 15:00", (Some("2026-11-18 15:00"), "2026-11-20 00:00")),
        ("  timed   manager\n    due in 2 business days at 17:00\n", "due 2026-11-18 at 17:00, reminder on 2026-11-17 10:00", (Some("2026-11-17 10:00"), "2026-11-18 17:00")),
        ("  tight   manager\n    due in 1 business day at 09:00\n", "due 2026-11-17 at 09:00", (None, "2026-11-17 09:00")),
    ] {
        // the request with that stage first, and a plain stage after it
        let text = format!("{}route\n{first}  plain   manager\n\n{}", &src[..from], &src[to..]);
        let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
        let o = run.text(&file, &text);
        assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
        let form: Values = vec![("what".to_string(), Value::Str("A new chair".into()))];
        let sim = simulate(&o, &run, &Ask { requester: Some("e1201".into()), every: false, form: Some("what = \"A new chair\"".to_string()), on: On::Day(day) }).unwrap_or_else(|t| panic!("{}", t.en));
        assert_eq!(sim.sims[0].stages[0].due.as_ref().map(|d| d.en.as_str()), Some(said), "{first}");
        ringi::engine::with_text(&o, &run, &text, |m, org| {
            let sub = Submit { id: "T-1".into(), requester: "e1201".into(), by: None, form: form.clone(), attempt: "attempt-1".into() };
            let s = scenario::step(m, org, None, Input::Submit(sub), at(m, "2026-11-16", "10:00")).unwrap();
            let (t, _) = s.answer.unwrap_or_else(|r| panic!("{}", r.text.en));
            let st = t.stage().unwrap();
            assert_eq!((st.remind.map(|r| scenario::local(m, r)), st.due.map(|d| scenario::local(m, d))), (clocks.0.map(str::to_string), Some(clocks.1.to_string())), "{first}");
        })
        .unwrap_or_else(|t| panic!("{}", t.en));
    }
}

/// Paused by the one a late stage went up to, the stage has its reminder and its deadline moved
/// later by the business days of the pause, as the first ones are; taken back by the one it went
/// up to, the stage has both set again at the times they had.
#[test]
fn the_clocks_where_a_stage_went_up_to_move_as_the_first_ones_do() {
    let r = rows("desk", "paused_where_it_went_up_to");
    assert_eq!(r[3], "pause by e3101: in progress at first -> paused at first | record(paused) cancel(remind) cancel(late) notify(paused: e1201) set(pause ends)");
    assert_eq!(r[4], "resume by e3101: paused at first -> in progress at first | record(resumed) cancel(pause ends) notify(resumed: e1201, e3101) set(remind) set(late)");
    assert_eq!(r[5], "remind: in progress at first -> in progress at first | record(reminded) notify(reminder: e3101)");
    let back = rows("desk", "taken_back_where_it_went_up_to");
    assert_eq!(back[4], "retract by e3101: in progress at board -> in progress at first | record(retracted) cancel(remind) cancel(late) notify(waiting: e3101) set(remind) set(late)");
    with(&machine_file("desk"), |m, org| {
        // paused on Friday the 20th and resumed on Tuesday the 24th: two business days, and the
        // reminder of the 24th and the deadline at the end of the 25th are two business days later
        let resumed = ticket_after(m, org, "paused_where_it_went_up_to", 4);
        let st = resumed.stage().unwrap();
        assert_eq!((st.up, st.held, st.remind, st.due), (1, 2, Some(at(m, "2026-11-26", "09:00")), Some(at(m, "2026-11-28", "00:00"))));
        // taken back on the 23rd: the reminder of the 24th and the deadline at the end of the 25th
        let taken_back = ticket_after(m, org, "taken_back_where_it_went_up_to", 4);
        let st = taken_back.stage().unwrap();
        assert_eq!((st.parts[0].approvers.clone(), st.up, st.remind, st.due), (vec!["e3101".to_string()], 1, Some(at(m, "2026-11-24", "09:00")), Some(at(m, "2026-11-26", "00:00"))));
    });
}

/// `late` with `escalate` and nowhere to go up to: what is written after `then` is done at that
/// deadline, and the history says there was nowhere to go.
#[test]
fn row_late_with_nowhere_to_go_up() {
    // `then expire`: the president's own stage
    with(&machine_file("desk"), |m, org| {
        let r = ran(m, org, "late_at_the_top");
        assert_eq!(went(&r.rows[2].stepped), "late: in progress at first -> expired | record(ticket expired) notify(decided: e0101)");
        let said = recorded(&r.rows[2].stepped);
        assert!(said[0].1.as_deref().is_some_and(|n| n.contains("nowhere to go up")), "{said:?}");
    });
    // `then stay`: the same stage, waiting, with no clock left
    with(&machine_file("late"), |m, org| {
        let r = ran(m, org, "nowhere_to_go_up");
        let late = r.rows.iter().find(|row| row.stepped.input.word() == "late").unwrap();
        assert_eq!(went(&late.stepped), "late: in progress at waits -> in progress at waits | record(late)");
        assert_eq!(recorded(&late.stepped), [("late".to_string(), Some("nowhere to go up; it waits on".to_string()))]);
        let t = late.ticket().unwrap();
        assert_eq!((t.stage().unwrap().due, t.timers().len()), (None, 0));
    });
}

/// `late` with `stay`: the same stage and the same approver, waiting on with no clock left, and
/// `late` in the history; the approver answers however late it is.
#[test]
fn row_late_stays() {
    let file = machine_file("late");
    let src = std::fs::read_to_string(&file).unwrap();
    let text = src.replace("    when late escalate, then stay\n", "    when late stay\n");
    assert!(text != src);
    let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
    let o = run.text(&file, &text);
    assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
    ringi::engine::with_text(&o, &run, &text, |m, org| {
        let made = ticket_after(m, org, "up_twice", 0);
        let due = made.stage().unwrap().due.unwrap();
        let s = scenario::step(m, org, Some(&made), Input::Event(Event::Late), due).unwrap();
        assert_eq!(went(&s), "late: in progress at waits -> in progress at waits | record(late)");
        assert_eq!(recorded(&s), [("late".to_string(), Some("it waits on".to_string()))]);
        let (after, _) = s.answer.unwrap();
        let waits = after.stage().unwrap();
        assert_eq!((waits.parts[0].approvers.clone(), waits.up, waits.due, after.timers().len()), (vec!["e1101".to_string()], 0, None, 0));
        assert!(give(m, org, &after, approve("e1101"), due + 30 * 86_400).0.starts_with("approve by e1101: in progress at waits -> in progress at twice |"));
    })
    .unwrap();
}

/// `late` with `expire`, and `late` after the stage went up: `expired`.
#[test]
fn row_late_expires() {
    assert_eq!(rows("desk", "late_and_up")[4], "late: in progress at first -> expired | record(ticket expired) notify(decided: e1201)");
    with(&machine_file("late"), |m, org| {
        let r = ran(m, org, "a_deadline_at_a_time");
        let late = r.rows.last().unwrap();
        assert!(went(&late.stepped).starts_with("late: in progress at timed -> expired | record(ticket expired)"), "{}", went(&late.stepped));
    });
    // `on expired` voids what the ticket holds
    let leave = with("examples/leave.ringi", |m, org| {
        let t = ticket_after(m, org, "a_long_paid_leave", 0);
        let due = t.stage().unwrap().due.unwrap();
        let (_, up) = give(m, org, &t, Event::Late, due);
        let up = up.unwrap();
        give(m, org, &up, Event::Late, up.stage().unwrap().due.unwrap()).0
    });
    assert!(leave.contains("-> expired | ") && leave.contains(" void(leave.take) "), "{leave}");
}

/// `late` at a summary's stage: the ticket goes on to the next stage without the summary, and the
/// history says the summary is missing.
#[test]
fn row_late_at_a_summary() {
    with("examples/contract.ringi", |m, org| {
        let name = m.s.file.scenarios[0].name.text.clone();
        let r = ran(m, org, &name);
        let asked = r.rows.iter().find(|row| row.effects().iter().any(|e| matches!(e.what, What::AskNote { .. }))).unwrap();
        let mut t = asked.ticket().unwrap().clone();
        // the clocks of the stage one after another, to its deadline
        let line = loop {
            let (ev, rings) = first_clock(&t).unwrap();
            let late = ev.word() == "late";
            let (line, after) = give(m, org, &t, ev, rings);
            t = after.unwrap_or_else(|| panic!("{line}"));
            if late {
                break line;
            }
        };
        assert!(line.starts_with("late: in progress at summary -> in progress at intake | record(note missing) "), "{line}");
        assert!(t.notes.is_empty());
    });
}

/// `pause`: `paused`, the stage's clocks cancelled and the clock of the pause set.
#[test]
fn row_pause() {
    assert_eq!(rows("desk", "paused_and_resumed")[1], "pause by e1101: in progress at first -> paused at first | record(paused) cancel(remind) cancel(late) notify(paused: e1201) set(pause ends)");
    with(&machine_file("desk"), |m, org| {
        let t = ticket_after(m, org, "paused_and_resumed", 1);
        // five business days from Tuesday the 17th: the end of Tuesday the 24th
        assert_eq!((t.paused_at, t.pause_ends, t.pause_left), (Some(at(m, "2026-11-17", "10:00")), Some(at(m, "2026-11-25", "00:00")), 5));
    });
}

/// `resume` and `pause ends`: `in progress`, the clocks set again with the business days of the
/// pause added.
#[test]
fn row_resume_and_pause_ends() {
    assert_eq!(rows("desk", "paused_and_resumed")[2], "resume by e1101: paused at first -> in progress at first | record(resumed) cancel(pause ends) notify(resumed: e1201, e1101) set(remind) set(late)");
    assert_eq!(rows("desk", "a_pause_used_up")[2], "pause ends: paused at first -> in progress at first | record(resumed) notify(resumed: e1201, e1101) set(remind) set(late)");
    with(&machine_file("desk"), |m, org| {
        let (made, back) = (ticket_after(m, org, "paused_and_resumed", 0), ticket_after(m, org, "paused_and_resumed", 2));
        // two business days paused (the 17th to the 19th): the deadline moves from the end of
        // Thursday the 19th to the end of Monday the 23rd, and two of the five days of pause are
        // used
        assert_eq!(made.stage().unwrap().due, Some(at(m, "2026-11-20", "00:00")));
        assert_eq!((back.stage().unwrap().due, back.stage().unwrap().held, back.pause_left), (Some(at(m, "2026-11-24", "00:00")), 2, 3));
        let used = ticket_after(m, org, "a_pause_used_up", 2);
        assert_eq!((used.pause_left, used.pause_ends, used.paused_at), (0, None, None));
    });
}

/// `resubmit`: `in progress` at the first stage of the next round, with the values it changes.
#[test]
fn row_resubmit() {
    assert_eq!(rows("desk", "sent_back_and_made_again")[2], "resubmit by e1201: with requester -> in progress at first | record(resubmitted) cancel(resubmit late) notify(waiting: e1101) set(remind) set(late)");
    with(&machine_file("desk"), |m, org| {
        let r = ran(m, org, "sent_back_and_made_again");
        let t = r.rows[2].ticket().unwrap();
        assert_eq!((t.round, t.backs, t.resubmit_due), (2, 1, None));
        assert_eq!(t.value("price"), Some(&Value::Int(90_000)));
        // the route is read again: at 90,000 JPY the board does not answer
        assert_eq!(went(&r.rows[3].stepped).split(" | ").next().unwrap(), "approve by e1101: in progress at first -> in progress at decide");
        // the things to do of the new round carry it in their keys
        assert!(r.rows[2].effects().iter().all(|e| e.round == 2 || matches!(e.what, What::CancelTimer { .. })), "{:?}", r.rows[2].effects());
    });
}

/// `discard`: `withdrawn`.
#[test]
fn row_discard() {
    assert_eq!(rows("desk", "withdrawn_and_discarded")[2], "discard by e1201: with requester -> withdrawn | record(ticket withdrawn) cancel(resubmit late) notify(decided: e1201)");
}

/// `resubmit late`: `expired`.
#[test]
fn row_resubmit_late() {
    assert_eq!(rows("desk", "not_made_again_in_time")[2], "resubmit late: with requester -> expired | record(ticket expired) notify(decided: e1201)");
}

/// `edit`: the same stage; when the route reads the value, the stages after this one are read
/// again (at 90,000 JPY the board no longer answers).
#[test]
fn row_edit() {
    let r = rows("desk", "edited_by_an_approver");
    assert_eq!(r[1], "edit by e1101: in progress at first -> in progress at first | record(edited)");
    assert_eq!(r[2].split(" | ").next().unwrap(), "approve by e1101: in progress at first -> in progress at decide");
    with(&machine_file("desk"), |m, org| {
        let (before, after) = (ticket_after(m, org, "edited_by_an_approver", 0), ticket_after(m, org, "edited_by_an_approver", 1));
        let names = |t: &Ticket| t.stages.iter().map(|s| s.name.clone()).collect::<Vec<_>>();
        assert_eq!((names(&before), names(&after)), (vec!["first".to_string(), "board".to_string(), "decide".to_string()], vec!["first".to_string(), "decide".to_string()]));
        // the stage it is at keeps its start, its clocks and its approver
        assert_eq!(before.stage(), after.stage());
    });
}

/// A flow's answer: the agent's `ok` is the answer of its part of the stage, which still waits for
/// the person of its other part; the summary that is ready is kept with the ticket, which goes on
/// to the next stage.
#[test]
fn row_a_flow_answers() {
    assert_eq!(rows("flows", "the_agent_and_a_person")[1], "agent answered: in progress at first -> in progress at first | record(agent ok)");
    with("examples/contract.ringi", |m, org| {
        let name = m.s.file.scenarios[0].name.text.clone();
        let r = ran(m, org, &name);
        let ready = r.rows.iter().find(|row| row.stepped.input.word() == "note ready").unwrap();
        let line = went(&ready.stepped);
        assert_eq!(line, "note ready: in progress at summary -> in progress at intake | record(note attached) cancel(remind) cancel(late) notify(waiting: e9001, e9002, e9003) set(remind) set(late)");
        assert_eq!(ready.ticket().unwrap().notes.len(), 1);
    });
}

// ── What the table says under it ──

/// An answer is of a round and a stage: one for another stage, or by someone the stage does not
/// wait for, is refused, and so is a second answer by the same person.
#[test]
fn an_answer_for_another_stage_is_refused() {
    with(&machine_file("desk"), |m, org| {
        let now = at(m, "2026-11-16", "12:00");
        let first = ticket_after(m, org, "approved_by_a_quorum", 0);
        // e3101 answers `decide`, which the ticket has not come to
        assert_eq!(give(m, org, &first, approve("e3101"), now).0, "approve by e3101: in progress at first refused (person)");
        let board = ticket_after(m, org, "approved_by_a_quorum", 2);
        // e1101 answered `first`, which the ticket has left; e0201 has answered `board`
        assert_eq!(give(m, org, &board, approve("e1101"), now).0, "approve by e1101: in progress at board refused (person)");
        assert_eq!(give(m, org, &board, approve("e0201"), now).0, "approve by e0201: in progress at board refused (person)");
        // the requester does not answer their own request
        assert_eq!(give(m, org, &first, approve("e1201"), now).0, "approve by e1201: in progress at first refused (person)");
    });
}

/// The states an event is taken in (DESIGN 7.2): an event the state does not take is refused, and
/// the ticket is as it was.
#[test]
fn an_event_the_state_does_not_take_is_refused() {
    with(&machine_file("desk"), |m, org| {
        let now = at(m, "2026-11-16", "12:00");
        let kind = |t: &Ticket, ev: Event| {
            let (line, after) = give(m, org, t, ev, now);
            assert!(after.is_none(), "{line}");
            line.rsplit_once(" refused ").unwrap_or_else(|| panic!("{line}")).1.to_string()
        };
        let made = ticket_after(m, org, "sent_back_and_made_again", 0);
        let back = ticket_after(m, org, "sent_back_and_made_again", 1);
        let paused = ticket_after(m, org, "paused_and_resumed", 1);
        let ended = ticket_after(m, org, "approved_by_a_quorum", 4);
        // in progress
        assert_eq!(kind(&made, Event::Resubmit { by: "e1201".into(), form: Values::new(), clear: Vec::new(), attempt: "attempt-2".into() }), "(state)");
        assert_eq!(kind(&made, Event::Discard { by: "e1201".into() }), "(state)");
        assert_eq!(kind(&made, Event::Resume { by: "e1101".into() }), "(state)");
        assert_eq!(kind(&made, Event::ResubmitLate), "(state)");
        assert_eq!(kind(&made, Event::PauseEnds), "(state)");
        // with requester
        for ev in [approve("e1101"), Event::Reject { by: "e1101".into() }, Event::SendBack { by: "e1101".into(), to: None }, Event::Pause { by: "e1101".into() }, Event::Withdraw { by: "e1201".into() }, Event::Retract { by: "e1101".into() }] {
            assert_eq!(kind(&back, ev.clone()), "(state)", "{ev:?}");
        }
        assert_eq!(kind(&back, Event::Late), "(state)");
        // paused: nobody answers until it is resumed
        for ev in [approve("e1101"), Event::Reject { by: "e1101".into() }, Event::SendBack { by: "e1101".into(), to: None }, Event::Pause { by: "e1101".into() }] {
            assert_eq!(kind(&paused, ev.clone()), "(state)", "{ev:?}");
        }
        assert_eq!(kind(&paused, Event::Late), "(state)");
        assert_eq!(kind(&paused, Event::Remind), "(state)");
        // ended: nothing
        for ev in [approve("e3101"), Event::Withdraw { by: "e1201".into() }, Event::Resubmit { by: "e1201".into(), form: Values::new(), clear: Vec::new(), attempt: "attempt-2".into() }, Event::Discard { by: "e1201".into() }, Event::Late, Event::Remind, Event::ResubmitLate, Event::PauseEnds] {
            assert_eq!(kind(&ended, ev.clone()), "(state)", "{ev:?}");
        }
    });
}

/// A paused ticket can be withdrawn while `withdraw until` allows it (DESIGN 7.2), and an
/// administrator's hand-over reaches it.
#[test]
fn a_paused_ticket_takes_withdraw_and_the_administrator() {
    with(&machine_file("desk"), |m, org| {
        let now = at(m, "2026-11-18", "10:00");
        let paused = ticket_after(m, org, "paused_and_resumed", 1);
        let (line, _) = give(m, org, &paused, Event::Withdraw { by: "e1201".into() }, now);
        // the pause cancelled the stage's clocks: only the clock of the pause is left to cancel
        assert_eq!(line, "withdraw by e1201: paused at first -> with requester | record(withdrawn) cancel(pause ends) notify(returned: e1101) set(resubmit late)");
        let (line, after) = give(m, org, &paused, Event::HandOver { by: None, from: "e1101".into(), to: "e3101".into() }, now);
        assert!(line.starts_with("hand over: paused at first -> paused at first | record(handed over)"), "{line}");
        assert_eq!(after.unwrap().stage().unwrap().parts[0].approvers, ["e3101"]);
    });
}

/// `withdraw until board`: the requester withdraws up to the stage, and not past it.
#[test]
fn withdraw_until_a_stage() {
    with(&machine_file("desk"), |m, org| {
        let now = at(m, "2026-11-16", "15:00");
        let withdraw = || Event::Withdraw { by: "e1201".into() };
        let board = ticket_after(m, org, "approved_by_a_quorum", 1);
        assert!(give(m, org, &board, withdraw(), now).0.starts_with("withdraw by e1201: in progress at board -> with requester | "));
        let decide = ticket_after(m, org, "approved_by_a_quorum", 3);
        assert_eq!(give(m, org, &decide, withdraw(), now).0, "withdraw by e1201: in progress at decide refused (state)");
        // and only the requester
        assert_eq!(give(m, org, &board, Event::Withdraw { by: "e1101".into() }, now).0, "withdraw by e1101: in progress at board refused (person)");
    });
}

/// `send back at most 2 times` counts the send-backs and the withdrawals of a ticket: a third
/// send-back is refused, to the requester and to a stage alike, and a ticket withdrawn then cannot
/// be made again, so it ends.
#[test]
fn the_send_backs_have_a_limit() {
    with(&machine_file("desk"), |m, org| {
        let mut t = ticket_after(m, org, "approved_by_a_quorum", 0);
        let mut now = at(m, "2026-11-16", "11:00");
        for n in 1..=2 {
            let (line, back) = give(m, org, &t, Event::SendBack { by: "e1101".into(), to: None }, now);
            assert!(line.contains("-> with requester | "), "{n}: {line}");
            now += 3600;
            let (line, again) = give(m, org, &back.unwrap(), Event::Resubmit { by: "e1201".into(), form: Values::new(), clear: Vec::new(), attempt: "attempt-2".into() }, now);
            assert!(line.contains("-> in progress at first | "), "{n}: {line}");
            t = again.unwrap();
            assert_eq!((t.backs, t.round), (n, n + 1));
            now += 3600;
        }
        assert_eq!(give(m, org, &t, Event::SendBack { by: "e1101".into(), to: None }, now).0, "send back by e1101: in progress at first refused (limit)");
        assert_eq!(give(m, org, &t, Event::Withdraw { by: "e1201".into() }, now).0, "withdraw by e1201: in progress at first -> withdrawn | record(ticket withdrawn) cancel(remind) cancel(late) notify(decided: e1201)");
        let (_, board) = give(m, org, &t, approve("e1101"), now);
        assert_eq!(give(m, org, &board.unwrap(), Event::SendBack { by: "e0201".into(), to: Some("first".into()) }, now).0, "send back by e0201: in progress at board refused (limit)");
    });
}

/// A stage is sent back to one the ticket passed, not to itself or one ahead.
#[test]
fn send_back_names_a_stage_the_ticket_passed() {
    with(&machine_file("desk"), |m, org| {
        let now = at(m, "2026-11-16", "15:00");
        let board = ticket_after(m, org, "approved_by_a_quorum", 1);
        for to in ["board", "decide", "nowhere"] {
            let (line, _) = give(m, org, &board, Event::SendBack { by: "e0201".into(), to: Some(to.into()) }, now);
            assert!(line.ends_with(" refused (event)") || line.ends_with(" refused (state)"), "{to}: {line}");
        }
    });
}

/// `retract approval until the next stage answers`: once someone of the next stage has answered,
/// the approval stays; and only the one who gave it takes it back.
#[test]
fn retract_until_the_next_stage_answers() {
    with(&machine_file("desk"), |m, org| {
        let now = at(m, "2026-11-16", "15:00");
        let retract = |by: &str| Event::Retract { by: by.to_string() };
        let board = ticket_after(m, org, "approved_by_a_quorum", 1);
        assert_eq!(give(m, org, &board, retract("e3101"), now).0, "retract by e3101: in progress at board refused (person)");
        assert!(give(m, org, &board, retract("e1101"), now).0.contains("-> in progress at first | "));
        let answered = ticket_after(m, org, "approved_by_a_quorum", 2);
        assert_eq!(give(m, org, &answered, retract("e1101"), now).0, "retract by e1101: in progress at board refused (state)");
        // at the first stage there is no approval to take back
        let first = ticket_after(m, org, "approved_by_a_quorum", 0);
        let (line, _) = give(m, org, &first, retract("e1101"), now);
        assert!(line.contains(" refused ("), "{line}");
    });
}

/// Taking an approval back does not move a deadline, so approving and retracting over and over
/// does not keep a ticket: the stage's deadline rings where it was set, and the stage after it is
/// counted from the approval that brought the ticket to it each time.
#[test]
fn retracting_does_not_move_a_deadline() {
    with(&machine_file("desk"), |m, org| {
        let made = ticket_after(m, org, "approved_by_a_quorum", 0);
        let due = made.stage().unwrap().due.unwrap();
        let mut t = made.clone();
        let mut now = at(m, "2026-11-16", "11:00");
        for _ in 0..20 {
            let (_, board) = give(m, org, &t, approve("e1101"), now);
            now += 600;
            let (line, back) = give(m, org, &board.unwrap(), Event::Retract { by: "e1101".into() }, now);
            t = back.unwrap_or_else(|| panic!("{line}"));
            now += 600;
            assert_eq!(t.stage().unwrap().due, Some(due));
        }
        assert_eq!(t.stage().unwrap().started, made.stage().unwrap().started);
        // the deadline rings as it was set
        let (line, _) = give(m, org, &t, Event::Late, due);
        assert!(line.starts_with("late: in progress at first -> in progress at first | record(went up)"), "{line}");
    });
}

/// `pause at most 5 business days` is the total of a ticket's pauses: a second pause gets what the
/// first left, and none is left after the clock of a pause rings.
#[test]
fn the_pauses_have_a_total() {
    with(&machine_file("desk"), |m, org| {
        let resumed = ticket_after(m, org, "paused_and_resumed", 2);
        assert_eq!(resumed.pause_left, 3);
        let now = at(m, "2026-11-19", "12:00");
        let (line, again) = give(m, org, &resumed, Event::Pause { by: "e1101".into() }, now);
        assert!(line.contains("-> paused at first | "), "{line}");
        // the three business days left, from Thursday the 19th: the end of Tuesday the 24th
        assert_eq!(again.unwrap().pause_ends, Some(at(m, "2026-11-25", "00:00")));
        // a minute after the clock of the pause rang, with none of the five days left
        let r = ran(m, org, "a_pause_used_up");
        let used = r.rows[2].ticket().unwrap();
        assert_eq!(give(m, org, used, Event::Pause { by: "e1101".into() }, r.rows[2].stepped.now + 60).0, "pause by e1101: in progress at first refused (limit)");
    });
}

/// An approver edits the fields `approvers edit` names and no other, with values the form takes.
#[test]
fn an_approver_edits_what_the_ticket_lets_them() {
    with(&machine_file("desk"), |m, org| {
        let now = at(m, "2026-11-16", "12:00");
        let t = ticket_after(m, org, "edited_by_an_approver", 0);
        let edit = |by: &str, name: &str, v: Value| Event::Edit { by: by.to_string(), fields: [(name.to_string(), v)].into_iter().collect(), clear: Vec::new() };
        assert_eq!(give(m, org, &t, edit("e1101", "what", Value::Str("A chair".into())), now).0, "edit by e1101: in progress at first refused (form)");
        assert_eq!(give(m, org, &t, edit("e1101", "price", Value::Int(1_000_001)), now).0, "edit by e1101: in progress at first refused (form)");
        assert_eq!(give(m, org, &t, edit("e1201", "price", Value::Int(90_000)), now).0, "edit by e1201: in progress at first refused (person)");
        let (line, after) = give(m, org, &t, edit("e1101", "note", Value::Str("Seen".into())), now);
        assert_eq!(line, "edit by e1101: in progress at first -> in progress at first | record(edited)");
        assert_eq!(after.unwrap().value("note"), Some(&Value::Str("Seen".into())));
    });
}

/// The values of a ticket are in the order the form declares them, whatever order they come in
/// (DESIGN 10.4), as the generated code has them: the values given, then the numbers and the dates
/// the form works out, and a row's cells in the order of its fields. What a flow is given is in
/// that order, and an approver's edit names the fields it changed (`to`) in that order too.
#[test]
fn the_values_of_a_ticket_are_in_the_forms_order() {
    let names = |vals: &Values| vals.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
    with("examples/expense.ringi", |m, org| {
        let r = ran(m, org, "a_day_trip_the_agent_decides");
        let Input::Submit(sub) = &r.rows[0].stepped.input else { panic!("the scenario does not begin with `submit`") };
        // the values backwards, and the cells of each row too
        let mut form = sub.form.clone();
        form.reverse();
        for (_, v) in form.iter_mut() {
            if let Value::List(rows) = v {
                rows.iter_mut().for_each(|row| row.reverse());
            }
        }
        assert_ne!(names(&form), names(&sub.form));
        let s = scenario::step(m, org, None, Input::Submit(Submit { form, ..sub.clone() }), r.rows[0].stepped.now).unwrap();
        // the same answer as the values in the form's order give
        assert_eq!(s.answer, r.rows[0].stepped.answer);
        let (t, fx) = s.answer.unwrap();
        assert_eq!(names(&t.form), ["category", "items", "purpose", "used_on", "trip_to", "receipt", "amount"]);
        let Some(Value::List(rows)) = t.value("items") else { panic!("no rows of `items`") };
        assert_eq!(names(&rows[0]), ["what", "price"]);
        let asked = fx.iter().find_map(|e| if let What::AskAgent { args, .. } = &e.what { Some(names(args)) } else { None }).expect("the agent is asked");
        assert_eq!(asked, ["request_id", "category", "items", "purpose", "used_on", "trip_to", "receipt", "amount"]);
    });
    with("examples/contract.ringi", |m, org| {
        let r = ran(m, org, "a_large_license");
        let row = r.rows.iter().find(|row| matches!(row.stepped.input, Input::Event(Event::Edit { .. }))).expect("an edit");
        let Input::Event(Event::Edit { by, fields, .. }) = &row.stepped.input else { unreachable!() };
        // `risks` comes after `reviewer` in the form, and is given before it here
        let fields: Values = [("risks".to_string(), Value::Str("None found".into()))].into_iter().chain(fields.iter().cloned()).collect();
        assert_eq!(names(&fields), ["risks", "reviewer"]);
        let s = scenario::step(m, org, row.stepped.before.as_ref(), Input::Event(Event::Edit { by: by.clone(), fields, clear: Vec::new() }), row.stepped.now).unwrap();
        let (t, fx) = s.answer.unwrap_or_else(|r| panic!("{}", r.text.en));
        let to = fx.iter().find_map(|e| match &e.what {
            What::Record(rec) if rec.did == "edited" => Some(rec.to.clone()),
            _ => None,
        });
        assert_eq!(to.as_deref(), Some(&["reviewer".to_string(), "risks".to_string()][..]));
        assert_eq!(names(&t.form), ["counterparty", "kind", "value", "months", "document", "text", "reviewer", "risks"]);
    });
}

/// A value of none is not given (DESIGN 10.4), as the generated code reads `null` in the JSON: in
/// a request made, one of a field the form works out, of the approvers' or that the form does not
/// have is not refused; given again, or edited by an approver, the ticket keeps the value it holds,
/// and the edit does not name it. What makes a field empty is the list `clear` (DESIGN 7.3), which
/// the tests below give.
#[test]
fn a_value_of_none_is_not_given() {
    // each event of a scenario's row given again with `more` among its values, and the answer the
    // row has
    fn again(m: &Machine, org: &Org, s: &Stepped, more: &[&str]) -> Stepped {
        let nones = more.iter().map(|n| (n.to_string(), Value::None));
        let input = match &s.input {
            Input::Submit(sub) => Input::Submit(Submit { form: sub.form.iter().cloned().chain(nones).collect(), ..sub.clone() }),
            Input::Event(Event::Resubmit { by, form, attempt, .. }) => Input::Event(Event::Resubmit { by: by.clone(), form: nones.chain(form.iter().cloned()).collect(), clear: Vec::new(), attempt: attempt.clone() }),
            Input::Event(Event::Edit { by, fields, .. }) => Input::Event(Event::Edit { by: by.clone(), fields: nones.chain(fields.iter().cloned()).collect(), clear: Vec::new() }),
            other => panic!("{}", other.word()),
        };
        scenario::step_with(m, org, s.before.as_ref(), input, s.comment.clone(), s.now).unwrap_or_else(|e| panic!("{}", e.en))
    }
    let row = |r: &Ran, word: &str| r.rows.iter().find(|row| row.stepped.input.word() == word).unwrap_or_else(|| panic!("no `{word}`")).stepped.clone();
    with("examples/expense.ringi", |m, org| {
        let submit = row(&ran(m, org, "a_day_trip_the_agent_decides"), "submit");
        assert_eq!(again(m, org, &submit, &["amount", "account", "colour"]).answer, submit.answer);
    });
    with("examples/leave.ringi", |m, org| {
        let resubmit = row(&ran(m, org, "a_leave_sent_back_with_a_comment"), "resubmit");
        let s = again(m, org, &resubmit, &["reason"]);
        assert_eq!(s.answer, resubmit.answer);
        assert_eq!(s.answer.unwrap().0.value("reason"), Some(&Value::Str("A family visit".into())));
    });
    with("examples/contract.ringi", |m, org| {
        let edit = row(&ran(m, org, "a_large_license"), "edit");
        let s = again(m, org, &edit, &["risks"]);
        assert_eq!(s.answer, edit.answer);
        let to = s.answer.unwrap().1.into_iter().find_map(|e| match e.what {
            What::Record(rec) if rec.did == "edited" => Some(rec.to),
            _ => None,
        });
        assert_eq!(to, Some(vec!["reviewer".to_string()]));
    });
}

// ── Fields made empty, and dropped as the form no longer shows them (DESIGN 7.3) ──

/// The kind and the English sentence of the refusal of the event `ev` given to the ticket `t`.
fn refused_with(m: &Machine, org: &Org, t: &Ticket, ev: Event, now: i64) -> (&'static str, String) {
    let s = scenario::step(m, org, Some(t), Input::Event(ev), now).unwrap_or_else(|e| panic!("{}", e.en));
    match s.answer {
        Err(r) => (r.kind, r.text.en),
        Ok(_) => panic!("not refused: {}", went(&s)),
    }
}

/// The fields the line of the history of a resubmit or an edit says lost their value (`cleared`,
/// DESIGN 7.4).
fn emptied(s: &Stepped) -> Vec<String> {
    let Ok((_, fx)) = &s.answer else { panic!("refused: {}", went(s)) };
    fx.iter()
        .find_map(|e| match &e.what {
            What::Record(r) if r.did == "resubmitted" || r.did == "edited" => Some(r.cleared.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no line of a resubmit or an edit: {}", went(s)))
}

/// The names of the stages of a ticket.
fn stage_names(t: &Ticket) -> Vec<String> {
    t.stages.iter().map(|s| s.name.clone()).collect()
}

fn resubmit(by: &str, form: Values, clear: &[&str]) -> Event {
    Event::Resubmit { by: by.to_string(), form, clear: clear.iter().map(|n| n.to_string()).collect(), attempt: "attempt-2".into() }
}

fn edit(by: &str, fields: Values, clear: &[&str]) -> Event {
    Event::Edit { by: by.to_string(), fields, clear: clear.iter().map(|n| n.to_string()).collect() }
}

/// A resubmit that changes what the form shows drops the value of each field it no longer shows,
/// then of each field shown by one dropped, until none is left to drop: goods made a service drops
/// the vendor, and the contact shown where a vendor is named. The line of the history names both
/// (`cleared`), and the route, read again, no longer has accounting check the vendor. A resubmit
/// that changes nothing the form shows drops nothing, and its line names none.
#[test]
fn a_resubmit_drops_the_fields_the_form_no_longer_shows() {
    with(&machine_file("supplies"), |m, org| {
        let r = ran(m, org, "a_kind_changed_on_resubmit");
        assert_eq!(stage_names(r.rows[0].ticket().unwrap()), ["first", "check"]);
        let again = r.rows[2].ticket().unwrap();
        assert_eq!((again.value("kind"), again.value("vendor"), again.value("contact")), (Some(&Value::Enum("service".into())), None, None));
        assert_eq!(stage_names(again), ["first"]);
        assert_eq!(emptied(&r.rows[2].stepped), ["vendor", "contact"]);
        // the line of the history says it in the JSON, and every other line says none
        let json = |n: usize| r.rows[n].stepped.to_json(&m.names).compact();
        assert!(json(2).contains(r#""cleared":["vendor","contact"]"#), "{}", json(2));
        assert!(json(0).contains(r#""cleared":[]"#), "{}", json(0));
        let back = r.rows[1].ticket().unwrap();
        let now = at(m, "2026-11-16", "13:00");
        let s = scenario::step(m, org, Some(back), Input::Event(resubmit("e1201", vec![("quantity".into(), Value::Int(3))], &[])), now).unwrap();
        assert_eq!(emptied(&s), Vec::<String>::new());
        let t = s.answer.unwrap().0;
        assert_eq!((t.value("vendor"), stage_names(&t)), (Some(&Value::Str("Kato Trading".into())), vec!["first".to_string(), "check".to_string()]));
        // a value given for a field the resubmit hides is refused, as before
        let hidden = resubmit("e1201", vec![("kind".into(), Value::Enum("service".into())), ("vendor".into(), Value::Str("Sato Supplies".into()))], &[]);
        assert_eq!(refused_with(m, org, back, hidden, now), ("form", "`vendor` is not shown for this request (its `shown when` does not hold), and takes no value".to_string()));
    });
    // a trip claimed again as an expense: the destination, asked only of a trip, is dropped, and
    // the route of an expense is read; naming it in `clear` too comes to the same
    with("examples/expense.ringi", |m, org| {
        let made = ticket_after(m, org, "a_trip_called_off", 0);
        let now = at(m, "2026-11-16", "11:00");
        let (line, back) = give(m, org, &made, Event::SendBack { by: "e1101".into(), to: None }, now);
        assert!(line.starts_with("send back by e1101: in progress at first -> with requester"), "{line}");
        let back = back.unwrap();
        let as_expense = vec![("category".to_string(), Value::Enum("expense".into()))];
        let later = at(m, "2026-11-16", "12:00");
        let dropped = scenario::step(m, org, Some(&back), Input::Event(resubmit("e1201", as_expense.clone(), &[])), later).unwrap();
        let named = scenario::step(m, org, Some(&back), Input::Event(resubmit("e1201", as_expense, &["trip_to"])), later).unwrap();
        assert_eq!(dropped.answer, named.answer);
        assert_eq!(emptied(&dropped), ["trip_to"]);
        let t = dropped.answer.unwrap().0;
        assert_eq!((t.value("trip_to"), t.stage_name()), (None, Some("precheck")));
    });
}

/// `none` in a resubmit's cell, and a name in its `clear`, make a field the form shows empty;
/// one that holds no value stays empty, and the line names none. An approver makes empty a field
/// `approvers edit` names, and one of the approvers': the vendor made empty drops the contact,
/// and the stages after this one are read again, as for a value an approver changes.
#[test]
fn a_field_is_made_empty_by_its_name() {
    with(&machine_file("supplies"), |m, org| {
        let r = ran(m, org, "a_note_made_empty");
        assert_eq!((r.rows[1].ticket().unwrap().value("note").is_some(), r.rows[2].ticket().unwrap().value("note")), (true, None));
        assert_eq!(emptied(&r.rows[2].stepped), ["note"]);
        let r = ran(m, org, "made_empty_where_it_is_hidden_or_empty");
        assert_eq!(emptied(&r.rows[2].stepped), ["vendor", "contact"]);
        // the approver's edits: the kind and the note; then one of the approvers' fields
        let r = ran(m, org, "edited_by_an_approver");
        assert_eq!(emptied(&r.rows[1].stepped), Vec::<String>::new());
        assert_eq!(emptied(&r.rows[2].stepped), ["vendor", "contact", "note", "label"]);
        assert_eq!(emptied(&r.rows[3].stepped), ["checked"]);
        let t = r.rows[3].ticket().unwrap();
        assert_eq!(t.form.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["kind", "price", "quantity", "total"]);
        // the vendor made empty by an approver: the route after this stage is read again
        let r = ran(m, org, "the_vendor_made_empty_by_an_approver");
        assert_eq!((stage_names(r.rows[0].ticket().unwrap()), stage_names(r.rows[1].ticket().unwrap())), (vec!["first".to_string(), "check".to_string()], vec!["first".to_string()]));
        assert_eq!(emptied(&r.rows[1].stepped), ["vendor", "contact"]);
        assert_eq!(r.rows[1].ticket().unwrap().stage(), r.rows[0].ticket().unwrap().stage());
        // a field with no value made empty: nothing changes, and the line names nothing
        let made = r.rows[0].ticket().unwrap();
        let now = at(m, "2026-11-16", "11:00");
        for name in ["checked", "label"] {
            let s = scenario::step(m, org, Some(made), Input::Event(edit("e1101", Values::new(), &[name])), now).unwrap();
            assert_eq!(emptied(&s), Vec::<String>::new(), "{name}");
            let t = s.answer.unwrap().0;
            assert_eq!((&t.form, &t.stages), (&made.form, &made.stages), "{name}");
        }
    });
}

/// What a resubmit may not make empty, each refused as the form refuses it (no new kind of refusal):
/// a field the form does not have, one it computes, one of the approvers', one it gives a value
/// too, one the form asks for on every request, and one the form shows and asks for where it is
/// shown, which the form's check refuses as one not filled in.
#[test]
fn what_a_resubmit_may_not_make_empty_is_refused() {
    with(&machine_file("supplies"), |m, org| {
        let back = ticket_after(m, org, "a_kind_changed_on_resubmit", 1);
        let now = at(m, "2026-11-16", "13:00");
        let no = |form: Values, clear: &[&str]| refused_with(m, org, &back, resubmit("e1201", form, clear), now);
        let form = |text: &str| ("form", text.to_string());
        assert_eq!(no(Values::new(), &["colour"]), form("the form has no field `colour`"));
        assert_eq!(no(Values::new(), &["total"]), form("`total` is computed, and is not given"));
        assert_eq!(no(Values::new(), &["checked"]), form("`checked` is a field of the approvers only, which the requester does not fill in"));
        assert_eq!(no(vec![("note".into(), Value::Str("Soon".into()))], &["note"]), form("`note` is given a value, and is in `clear` too"));
        assert_eq!(no(Values::new(), &["price"]), form("`price` is not filled in, and the form asks for it"));
        assert_eq!(no(Values::new(), &["kind"]), form("`kind` is not filled in, and the form asks for it"));
        assert_eq!(no(Values::new(), &["contact"]), form("`contact` is not filled in, and the form asks for it"));
        // the first name the list gives that is refused is the one said
        assert_eq!(no(Values::new(), &["note", "total", "colour"]), form("`total` is computed, and is not given"));
        // and in Japanese
        let ja = scenario::step(m, org, Some(&back), Input::Event(resubmit("e1201", vec![("note".into(), Value::Str("Soon".into()))], &["note"])), now).unwrap();
        assert_eq!(ja.answer.unwrap_err().text.ja, "`note` に値を渡していて、`clear` にも挙げています");
    });
}

/// What an approver may not make empty, each refused as the form refuses a value they give for it:
/// a field `approvers edit` does not name, a field the form does not have, one it computes, one
/// they give a value too, and one the form asks for on every request; and the requester, who does
/// not edit, is refused as one who does not answer the stage.
#[test]
fn what_an_approver_may_not_make_empty_is_refused() {
    with(&machine_file("supplies"), |m, org| {
        let made = ticket_after(m, org, "edited_by_an_approver", 0);
        let now = at(m, "2026-11-16", "11:00");
        let no = |by: &str, fields: Values, clear: &[&str]| refused_with(m, org, &made, edit(by, fields, clear), now);
        let form = |text: &str| ("form", text.to_string());
        assert_eq!(no("e1101", Values::new(), &["price"]), form("`price` is not a field approvers may edit (it is not in `approvers edit`)"));
        assert_eq!(no("e1101", Values::new(), &["contact"]), form("`contact` is not a field approvers may edit (it is not in `approvers edit`)"));
        assert_eq!(no("e1101", Values::new(), &["colour"]), form("the form has no field `colour`"));
        assert_eq!(no("e1101", Values::new(), &["total"]), form("`total` is computed, and is not given"));
        assert_eq!(no("e1101", vec![("checked".into(), Value::Str("e1101".into()))], &["checked"]), form("`checked` is given a value, and is in `clear` too"));
        assert_eq!(no("e1101", Values::new(), &["kind"]), form("`kind` is not filled in, and the form asks for it"));
        assert_eq!(no("e1201", Values::new(), &["note"]).0, "person");
    });
}

// ── Flows ──

/// A flow answers when it is done, a paused ticket too (DESIGN 7.2). An `ok` that is the last
/// answer the stage waits for ends the pause with the stage: the business days it took are
/// counted, its clock is cancelled, and the next stage starts.
#[test]
fn a_flow_answers_a_paused_ticket() {
    let r = rows("flows", "paused_while_the_agent_checks");
    assert_eq!(r[2], "pause by e3101: in progress at first -> paused at first | record(paused) cancel(late) notify(paused: e1201) set(pause ends)");
    assert_eq!(r[3], "agent answered: paused at first -> in progress at decide | record(agent ok) record(resumed) cancel(pause ends) notify(resumed: e1201) notify(waiting: e0101) set(late)");
    with(&machine_file("flows"), |m, org| {
        let t = ticket_after(m, org, "paused_while_the_agent_checks", 3);
        // paused on Monday the 16th, done on Tuesday the 17th: one of the five business days
        assert_eq!((t.pause_left, t.paused_at, t.pause_ends), (4, None, None));
        assert_eq!(t.stage().unwrap().due, Some(at(m, "2026-11-21", "00:00")));
    });
    // a doubt, and a flow that fails, leave the stage paused, with the way after `else` to answer
    let r = rows("flows", "a_doubt_while_paused");
    assert_eq!(r[2], "agent answered: paused at first -> paused at first | record(agent doubt) notify(waiting: e1101)");
    assert_eq!(r[3], "resume by e3101: paused at first -> in progress at first | record(resumed) cancel(pause ends) notify(resumed: e1201, e1101, e3101) set(late)");
    let r = rows("flows", "the_flow_fails_while_paused");
    assert_eq!(r[2], "flow failed: paused at first -> paused at first | record(agent failed) notify(waiting: e1101)");
    assert_eq!(r[3], "pause ends: paused at first -> in progress at first | record(resumed) notify(resumed: e1201, e1101, e3101) set(late)");
}

/// A flow's answer goes to the stage that asked for it: a stage that waits for no agent, a ticket
/// with the requester and one that has ended take none.
#[test]
fn a_flow_answers_the_stage_that_asked() {
    let ok = || Event::AgentAnswered { ok: true, reason: String::new() };
    with(&machine_file("flows"), |m, org| {
        let now = at(m, "2026-11-16", "12:00");
        // answered once: the stage waits for the agent no more
        let checked = ticket_after(m, org, "the_agent_and_a_person", 1);
        assert_eq!(give(m, org, &checked, ok(), now).0, "agent answered: in progress at first refused (state)");
        assert_eq!(give(m, org, &checked, Event::FlowFailed, now).0, "flow failed: in progress at first refused (state)");
        // no summary is asked for at the stage
        let (line, _) = give(m, org, &checked, Event::NoteReady { outputs: Values::new() }, now);
        assert_eq!(line, "note ready: in progress at first refused (state)");
        let decide = ticket_after(m, org, "the_agent_and_a_person", 2);
        assert_eq!(give(m, org, &decide, ok(), now).0, "agent answered: in progress at decide refused (state)");
    });
    with("examples/contract.ringi", |m, org| {
        let name = m.s.file.scenarios[0].name.text.clone();
        let r = ran(m, org, &name);
        let asked = r.rows.iter().find(|row| row.effects().iter().any(|e| matches!(e.what, What::AskNote { .. }))).unwrap();
        let t = asked.ticket().unwrap();
        // a summary's stage takes `note ready`, not an agent's verdict; its flow failing lets the
        // ticket go on without the summary
        assert!(give(m, org, t, ok(), asked.stepped.now + 60).0.ends_with(" refused (event)"));
        let (line, after) = give(m, org, t, Event::FlowFailed, asked.stepped.now + 60);
        assert!(line.contains(" | record(note missing) "), "{line}");
        assert!(after.unwrap().notes.is_empty());
    });
}

// ── The machine and `simulate` (PLAN C3.5) ──

/// The machine of the request at `path`, with the check it was made of: what `simulate` reads.
fn with_checked<R>(path: &str, f: impl FnOnce(&Outcome, &Run, &Machine, &Org) -> R) -> R {
    let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
    let o = run.file(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert!(!o.has_errors(), "{path}:\n{}", common::shown(&o, Lang::En));
    ringi::engine::with(&o, &run, |m, org| f(&o, &run, m, org)).unwrap_or_else(|t| panic!("{path}: {}", t.en))
}

/// The form of each scenario of a request, as its `form` line writes it: what `--form` takes.
fn forms_written(path: &str) -> Vec<String> {
    let src = std::fs::read_to_string(path).unwrap();
    src.lines().filter_map(|l| l.strip_prefix("  form ")).map(str::to_string).collect()
}

/// What a way of a stage comes to for a requester, in the words the machine and `simulate` are
/// compared by.
#[derive(Debug, PartialEq, Clone)]
enum Comes {
    /// These people answer it, and this many approvals pass it.
    People(Vec<String>, usize),
    Skipped,
    Stopped,
    /// For lack of an approver it went up to this person.
    Up(String),
    /// At the top it went to this way.
    To(String),
    /// An agent's flow answers first.
    Agent,
}

/// What `simulate` says a way comes to; None for a person an approver chooses as the ticket runs,
/// which the organization does not decide.
fn simulated(w: &ringi::simulate::WaySim) -> Option<Comes> {
    let Some(n) = &w.nobody else { return Some(Comes::People(w.people.clone(), w.needed.unwrap_or(1))) };
    let n = &n.en;
    if n == "a person chooses when the ticket runs" {
        return None;
    }
    if let Some(rest) = n.split("it goes up to ").nth(1) {
        return Some(Comes::Up(rest.split(' ').next().unwrap().to_string()));
    }
    if n.contains("the stage is skipped") {
        return Some(Comes::Skipped);
    }
    if n.contains("waits until an administrator adds an approver") || n.ends_with("it cannot go up") {
        return Some(Comes::Stopped);
    }
    match n.split("it goes to `").nth(1) {
        Some(rest) => Some(Comes::To(rest.split('`').next().unwrap().to_string())),
        None => panic!("what `simulate` says of a way with nobody is not read: {n}"),
    }
}

/// What a part of a ticket's stage came to as the stage started.
fn came(p: &ringi::engine::Part) -> Comes {
    use ringi::engine::PartState;
    match &p.state {
        PartState::Skipped => Comes::Skipped,
        PartState::Stopped if p.approvers.is_empty() => Comes::Stopped,
        PartState::Agent(_) | PartState::Checked(_) => Comes::Agent,
        _ if p.key != p.way => Comes::To(p.key.clone()),
        _ => Comes::People(p.approvers.clone(), p.needed),
    }
}

/// How a stage is late for a requester: it goes up each time (`None`), or has nowhere to go up to
/// the first time (and expires, or waits on), or the second.
#[derive(Debug, PartialEq, Clone, Copy)]
enum Nowhere {
    FirstExpires,
    FirstWaits,
    Second,
}

fn simulated_late(ways: &[&ringi::simulate::WaySim]) -> Option<Nowhere> {
    let said: Vec<&str> = ways.iter().filter_map(|w| w.late.as_ref()).map(|t| t.en.as_str()).collect();
    if said.iter().any(|t| t.contains("waits on for the one who is late")) {
        Some(Nowhere::FirstWaits)
    } else if said.iter().any(|t| t.starts_with("it cannot go up")) {
        Some(Nowhere::FirstExpires)
    } else if said.iter().any(|t| t.starts_with("the second time, it cannot go up")) {
        Some(Nowhere::Second)
    } else {
        assert!(said.is_empty(), "what `simulate` says of a late stage is not read: {said:?}");
        None
    }
}

/// The ticket's stage rung late at its deadline, and late again where it went up: where it had
/// nowhere to go up to, by the history's note.
fn rung_late(m: &Machine, org: &Org, t: &Ticket) -> Option<Nowhere> {
    let mut t = t.clone();
    for ring in 0..2 {
        let due = t.stage()?.due?;
        let s = scenario::step(m, org, Some(&t), Input::Event(Event::Late), due).ok()?;
        let Ok((after, _)) = &s.answer else { panic!("{}", went(&s)) };
        let nowhere = recorded(&s).iter().any(|(_, note)| note.as_deref().is_some_and(|n| n.contains("nowhere to go up")));
        if nowhere {
            return Some(match (ring, after.status.ended()) {
                (0, true) => Nowhere::FirstExpires,
                (0, false) => Nowhere::FirstWaits,
                _ => Nowhere::Second,
            });
        }
        if !recorded(&s).iter().any(|(did, _)| did == "went up") {
            return None;
        }
        t = after.clone();
    }
    None
}

/// One requester's ticket walked through its route: each stage as it started (its ways, and how it
/// is late), the stages done with an earlier approval, and how the walk ended.
struct Walk {
    stages: Vec<(String, Vec<(String, Comes)>, Option<Nowhere>)>,
    once: Vec<(String, String)>,
    /// The stages that went up for lack of an approver, by the history: the stage, and to whom.
    ups: Vec<(String, String)>,
    ended: String,
}

/// The lines of the history that say a stage went up for lack of an approver.
fn went_up(answer: &Result<(Ticket, Vec<Effect>), ringi::engine::Refusal>) -> Vec<(String, String)> {
    let Ok((_, fx)) = answer else { return Vec::new() };
    fx.iter()
        .filter_map(|e| match &e.what {
            What::Record(r) if r.did == "went up" && r.note.as_deref() != Some("late") => Some((r.stage.clone()?, r.to.first()?.clone())),
            _ => None,
        })
        .collect()
}

/// The request of `sub` made by `requester` at `now` and approved stage by stage: every approver a
/// stage needs approves, a minute apart; an agent answers `ok`, a summary's flow fails (the ticket
/// goes on without it), an administrator adds an approver where a stage has stopped, and an
/// approver fills in what the scenario's approvers fill in (`edits`) where the next stage asks for
/// it. Err with the kind when the request is not taken.
fn walk(m: &Machine, org: &Org, sub: &Submit, edits: &[Values], requester: &str, now: i64) -> Result<Walk, &'static str> {
    use ringi::engine::{PartState, Phase, Status};
    let sub = Submit { requester: requester.to_string(), by: None, ..sub.clone() };
    let s = scenario::step(m, org, None, Input::Submit(sub), now).unwrap_or_else(|e| panic!("{}", e.en));
    let mut w = Walk { stages: Vec::new(), once: Vec::new(), ups: went_up(&s.answer), ended: String::new() };
    let mut t = match s.answer {
        Ok((t, _)) => t,
        Err(r) => return Err(r.kind),
    };
    let mut now = now;
    let mut seen: Vec<usize> = Vec::new();
    for _ in 0..200 {
        // the stages the last transition passed or skipped, and the one the ticket is at
        for (i, st) in t.stages.iter().enumerate() {
            if seen.contains(&i) || st.phase == Phase::Ahead || st.parts.is_empty() {
                continue;
            }
            seen.push(i);
            let late = if t.at == Some(i) && t.status == Status::InProgress { rung_late(m, org, &t) } else { None };
            w.stages.push((st.name.clone(), st.parts.iter().map(|p| (p.way.clone(), came(p))).collect(), late));
            // done with the approval of an earlier stage, or waiting for that of a later one
            for p in &st.parts {
                if let PartState::Once(with) | PartState::Deferred(with) = &p.state {
                    w.once.push((st.name.clone(), with.clone()));
                }
            }
        }
        if t.status != Status::InProgress {
            w.ended = t.status.word().to_string();
            // nothing changes on the way of a walk: no stage that waited was left waiting, and
            // none was started after all
            for (i, st) in t.stages.iter().enumerate() {
                assert!(st.parts.iter().all(|p| p.deferred().is_none()), "{requester}: `{}` still waits as the ticket ends", st.name);
                assert!(t.live(i), "{requester}: `{}` came twice", st.name);
            }
            return Ok(w);
        }
        let st = t.stage().unwrap();
        let open = st.parts.iter().find(|p| !p.done()).unwrap_or_else(|| panic!("{requester}: the stage `{}` has no part left and stays", st.name));
        let ev = match &open.state {
            PartState::Agent(_) if open.way.is_empty() => Event::FlowFailed,
            PartState::Agent(_) => Event::AgentAnswered { ok: true, reason: String::new() },
            PartState::Stopped => {
                // anyone of the version who did not make the request, and is not at the stage yet
                let v = org.in_effect(m.day(now)).unwrap();
                let add = v.people.iter().map(|p| p.id.as_str()).find(|id| *id != requester && !open.approvers.iter().any(|a| a == id)).unwrap();
                Event::AddApprover { by: None, approver: add.to_string(), part: Some(open.way.clone()) }
            }
            _ => {
                let next = open.approvers.iter().find(|a| !open.answers.iter().any(|x| x.seat == **a)).unwrap_or_else(|| panic!("{requester}: nobody is left to answer `{}`: {open:?}", st.name));
                approve(next)
            }
        };
        now += 60;
        let s = scenario::step(m, org, Some(&t), Input::Event(ev.clone()), now).unwrap_or_else(|e| panic!("{}", e.en));
        w.ups.extend(went_up(&s.answer));
        match s.answer {
            Ok((after, _)) => t = after,
            // the next stage is answered by a person an approver chooses: chosen as the scenario
            // chooses, and approved again
            Err(ref r) if r.kind == "form" && !edits.is_empty() && ev.word() == "approve" => {
                for fields in edits {
                    let s = scenario::step(m, org, Some(&t), Input::Event(Event::Edit { by: ev.by().unwrap().to_string(), fields: fields.clone(), clear: Vec::new() }), now).unwrap_or_else(|e| panic!("{}", e.en));
                    match s.answer {
                        Ok((after, _)) => t = after,
                        Err(ref r) => panic!("{requester}: {} ({})", went(&s), r.text.en),
                    }
                }
            }
            Err(ref r) => panic!("{requester}: {} ({})", went(&s), r.text.en),
        }
    }
    panic!("{requester}: the walk does not end");
}

/// How many of each thing a comparison of the machine with `simulate` came across.
#[derive(Debug, Default)]
struct Compared {
    walked: usize,
    may_not_submit: usize,
    found_nobody: usize,
    nowhere: usize,
    done_once: usize,
    /// The requesters whose hold the book cannot be asked for, each once a form and a day.
    not_held: Vec<String>,
}

/// The machine and `simulate` say the same of every requester (PLAN C3.5), for the form of each
/// scenario of the request at `file`, in each version of the organization: who may not submit;
/// who answers each way of each stage and how many approvals pass it; the ways that find nobody,
/// and whether the stage is skipped, goes up, goes to the way after `then`, or waits for an
/// administrator; the stages that are late with nowhere to go up to, the first time and the
/// second; the stages `same approver approves once` does with an earlier approval; and the ones
/// whose `submit` is refused for a hold that cannot be made for them. `simulate` solves the ways
/// for a requester without running a ticket; the machine comes to each stage by the events before
/// it.
fn compared(file: &str) -> Compared {
    use ringi::simulate::{Ask, On, simulate};
    let forms = forms_written(file);
    let mut n = Compared::default();
    with_checked(file, |o, run, m, org| {
        let scenarios = &m.s.file.scenarios;
        assert_eq!(forms.len(), scenarios.len(), "{file}");
        let edits_in = |r: &scenario::Ran| -> Vec<Values> { r.rows.iter().filter_map(|row| if let Input::Event(Event::Edit { fields, .. }) = &row.stepped.input { Some(fields.clone()) } else { None }).collect() };
        let every_edit: Vec<Values> = scenarios.iter().flat_map(|sc| edits_in(&scenario::run(m, org, sc))).collect();
        // each form once
        let mut tried: Vec<&String> = Vec::new();
        for (sc, form) in scenarios.iter().zip(&forms) {
            if tried.contains(&form) {
                continue;
            }
            tried.push(form);
            let r = scenario::run(m, org, sc);
            let Input::Submit(sub) = &r.rows[0].stepped.input else { panic!("{file}: `{}` does not begin with `submit`", sc.name.text) };
            // what the approvers of the scenario fill in on the way; a scenario that ends before
            // the approvers fill anything in walks with what those of the other scenarios do
            let edits = match edits_in(&r) {
                own if own.is_empty() => every_edit.clone(),
                own => own,
            };
            for day in ["2026-11-16", "2027-04-05"] {
                let now = at(m, day, "10:00");
                let on = ritsu_ports::day_of(day).unwrap();
                let v = org.in_effect(on).unwrap();
                let every = simulate(o, run, &Ask { requester: None, every: true, form: Some(form.clone()), on: On::Day(on) }).unwrap_or_else(|t| panic!("{file}, `{}` on {day}: {}", sc.name.text, t.en));
                let may_not: Vec<String> = every.submitted.as_ref().map(|(_, ids)| ids.clone()).unwrap_or_default();
                for p in &v.people {
                    let ctx = format!("{file}, `{}`, {} on {day}", sc.name.text, p.id);
                    let sim = every.sims.iter().find(|x| x.requester.id == p.id && x.requester.by.is_none());
                    let w = match walk(m, org, sub, &edits, &p.id, now) {
                        Ok(w) => w,
                        // the requester may not submit: `simulate` names the same people
                        Err("submit") => {
                            assert!(may_not.contains(&p.id) && sim.is_none(), "{ctx}: the machine refuses the requester, and `simulate` does not");
                            n.may_not_submit += 1;
                            continue;
                        }
                        // a hold the book cannot be asked for (the requester has no department to
                        // name its budget by): `simulate` says the hold, and of nobody else
                        Err("book") => {
                            assert!(sim.is_some_and(|x| !x.no_hold.is_empty()), "{ctx}: the machine refuses the hold, and `simulate` says no hold that cannot be made");
                            n.not_held.push(p.id.clone());
                            continue;
                        }
                        Err(kind) => panic!("{ctx}: the request is refused ({kind}), which no requester of the organization changes"),
                    };
                    assert!(!may_not.contains(&p.id), "{ctx}: `simulate` says the requester may not submit, and the machine takes the request");
                    let sim = sim.unwrap_or_else(|| panic!("{ctx}: `simulate` shows no such requester"));
                    assert!(sim.no_hold.is_empty(), "{ctx}: `simulate` says a hold cannot be made ({:?}), and the machine takes the request", sim.no_hold);
                    n.walked += 1;
                    for (name, ways, late) in &w.stages {
                        // a summary has no way to choose, and `simulate` shows none for it
                        if ways.iter().all(|(way, _)| way.is_empty()) {
                            continue;
                        }
                        let st = sim.stages.iter().find(|s| &s.stage == name).unwrap_or_else(|| panic!("{ctx}: `simulate` has no stage `{name}`"));
                        let mut waits: Vec<&ringi::simulate::WaySim> = Vec::new();
                        let mut unsolved = false;
                        for (way, came) in ways {
                            // an agent answers first: `simulate` shows the way after `else`, which
                            // answers when the agent doubts
                            if *came == Comes::Agent {
                                unsolved = true;
                                continue;
                            }
                            let sw = st.ways.iter().find(|x| &x.way == way).unwrap_or_else(|| panic!("{ctx}: `simulate` has no way `{way}` at `{name}`: {:?}", st.ways));
                            let want = match simulated(sw) {
                                // a person an approver chooses as the ticket runs
                                None => {
                                    unsolved = true;
                                    continue;
                                }
                                // the one it went up to answers it alone, and the history says that
                                // it went up
                                Some(Comes::Up(to)) => {
                                    assert!(w.ups.contains(&(name.clone(), to.clone())), "{ctx}: the history does not say that `{name}` went up to {to}: {:?}", w.ups);
                                    n.found_nobody += 1;
                                    Comes::People(vec![to], 1)
                                }
                                Some(other) => other,
                            };
                            assert_eq!(*came, want, "{ctx}: the way `{way}` of the stage `{name}`");
                            match came {
                                Comes::People(..) => waits.push(sw),
                                Comes::To(_) => {
                                    n.found_nobody += 1;
                                    waits.push(sw);
                                }
                                _ => n.found_nobody += 1,
                            }
                        }
                        // late: asked of the stage as it starts, when people wait at it
                        if !unsolved && !waits.is_empty() && w.once.iter().all(|(s, _)| s != name) && ways.iter().all(|(_, c)| *c != Comes::Stopped) {
                            assert_eq!(*late, simulated_late(&waits), "{ctx}: the stage `{name}`, late");
                            if late.is_some() {
                                n.nowhere += 1;
                            }
                        }
                    }
                    // done with an earlier approval: the same stages, with the same one (the walk's
                    // agents agree, and what `simulate` says of a doubt is not walked)
                    let mut want: Vec<(String, String)> = sim.stages.iter().flat_map(|s| s.ways.iter().filter(|x| !x.once_if_doubted).filter_map(|x| Some((s.stage.clone(), x.once_with.clone()?)))).collect();
                    let mut got = w.once.clone();
                    (want.sort(), want.dedup(), got.sort(), got.dedup());
                    assert_eq!(got, want, "{ctx}: the stages done with an earlier approval");
                    n.done_once += got.len();
                    assert_eq!(w.ended, "approved", "{ctx}");
                }
            }
        }
    });
    eprintln!("{file}: {n:?}");
    n
}

#[test]
fn the_machine_and_simulate_on_desk() {
    let n = compared(&machine_file("desk"));
    assert!(n.walked >= 47 && n.found_nobody > 0 && n.nowhere > 0, "{n:?}");
}

#[test]
fn the_machine_and_simulate_on_flows() {
    let n = compared(&machine_file("flows"));
    assert!(n.walked >= 47, "{n:?}");
}

/// The late stages with nowhere to go up to: `then stay`, `then expire`, the second time.
#[test]
fn the_machine_and_simulate_on_late() {
    let n = compared(&machine_file("late"));
    assert!(n.walked >= 47 && n.nowhere > 20, "{n:?}");
}

/// The stages that remind each time they go up: where they go up to, and where they have nowhere.
#[test]
fn the_machine_and_simulate_on_reminds() {
    let n = compared(&machine_file("reminds"));
    assert!(n.walked >= 47 && n.nowhere > 20, "{n:?}");
}

/// The stages done with an earlier approval, in each shape of the route.
#[test]
fn the_machine_and_simulate_on_once() {
    let n = compared(&machine_file("once"));
    assert!(n.walked >= 47 * 6 && n.done_once > 50, "{n:?}");
}

/// The stages that wait for a later one, in each shape of the route: the stage `simulate` says a way
/// answers once at is the one the machine makes it wait for, and the approval given there does it.
#[test]
fn the_machine_and_simulate_on_last() {
    let n = compared(&machine_file("last"));
    assert!(n.walked >= 47 * 9 && n.done_once > 500, "{n:?}");
    let n = compared(&machine_file("waits"));
    assert!(n.walked >= 47 * 2 && n.done_once > 10, "{n:?}");
}

/// The stages with nobody at the top of the organization, by each `when empty`.
#[test]
fn the_machine_and_simulate_on_top() {
    let n = compared(&machine_file("top"));
    assert!(n.walked >= 47 && n.found_nobody > 10, "{n:?}");
}

/// The ones who may not submit: 6 of 24 in the first version and 9 of 23 in the second.
#[test]
fn the_machine_and_simulate_on_who() {
    let n = compared(&machine_file("who"));
    assert!(n.may_not_submit >= 15 && n.walked >= 32, "{n:?}");
}

/// The 24 people of the first version and the 23 of the second, for each form the scenarios of a
/// request write; the leave request is not the board's to submit (5 people in each version, for
/// each of its two forms). The expense and the purchase requests hold a budget by the requester's
/// department where the requester has one (`when requester.department is not none`), so nobody's
/// request is refused for the hold.
#[test]
fn the_machine_and_simulate_on_the_examples() {
    for (name, walked, may_not) in [("account", 94, 0), ("contract", 188, 0), ("expense", 188, 0), ("leave", 74, 20), ("purchase", 235, 0)] {
        let n = compared(&format!("examples/{name}.ringi"));
        assert_eq!((n.walked, n.may_not_submit, n.not_held.len()), (walked, may_not, 0), "{name}: {n:?}");
    }
}

/// A hold made by the requester's department whoever asks: the seven people of each version who are
/// in no department (the president, the heads of the divisions, the directors) are refused the hold
/// by the machine, `simulate` says the hold of each of them and of nobody else, and every other
/// requester walks as `simulate` says. Its JSON names the hold, what the argument reads and the kind
/// of team in ASCII, with the names as written beside them.
#[test]
fn the_machine_and_simulate_on_held() {
    for name in ["held", "held.ja"] {
        let n = compared(&machine_file(name));
        let mut none = n.not_held.clone();
        (none.sort(), none.dedup());
        assert_eq!(none, ["e0001", "e0101", "e0102", "e0201", "e0202", "e0203", "e0204"], "{name}: {n:?}");
        assert_eq!((n.walked, n.not_held.len()), (17 + 16, 14), "{name}: {n:?}");
    }
    let refused = |file: &str, form: &str| {
        let (code, out, err) = cli(&["simulate", &machine_file(file), "--requester", "e0101", "--form", form, "--on", "2026-11-16", "--format", "json"]);
        assert_eq!((code, err.as_str()), (0, ""), "{file}:\n{out}");
        let v = json::parse(&out).unwrap_or_else(|e| panic!("{file}: {e:?}\n{out}"));
        v.get("requesters").and_then(Json::as_arr).unwrap()[0].get("holds_refused").cloned().unwrap()
    };
    let en = refused("held", "what = \"A monitor\", amount = 60_000JPY");
    assert_eq!(
        en.compact(),
        r#"[{"hold":"budget.reserve","hold_written":"budget.reserve","argument":"dept","reads":"requester.department","reads_written":"requester.department","team_kind":"department","team_kind_written":"department"}]"#
    );
    let ja = refused("held.ja", "品目 = \"モニター\", 金額 = 60_000円");
    assert_eq!(
        ja.compact(),
        r#"[{"hold":"budget.reserve","hold_written":"予算.reserve","argument":"dept","reads":"requester.department","reads_written":"requester.department","team_kind":"department","team_kind_written":"department"}]"#
    );
    // and one in a department has none
    let (_, out, _) = cli(&["simulate", &machine_file("held"), "--requester", "e1201", "--on", "2026-11-16", "--format", "json"]);
    let v = json::parse(&out).unwrap();
    assert!(v.get("requesters").and_then(Json::as_arr).unwrap()[0].get("holds_refused").and_then(Json::as_arr).unwrap().is_empty(), "{out}");
}

// ── The machine and the check's findings (PLAN C3.5) ──

/// What `ringi check --format json` writes for a file.
fn checked_json(file: &str) -> Json {
    let (_, out, err) = cli(&["check", file, "--format", "json"]);
    json::parse(&out).unwrap_or_else(|e| panic!("{file}: {e:?}\n{out}\n{err}"))
}

fn strs(j: Option<&Json>) -> Vec<String> {
    j.and_then(Json::as_arr).unwrap_or_default().iter().filter_map(Json::as_str).map(str::to_string).collect()
}

/// The first scenario's request walked for `requester` on `day`.
fn walked(m: &Machine, org: &Org, requester: &str, day: &str) -> Walk {
    let r = scenario::run(m, org, &m.s.file.scenarios[0]);
    let Input::Submit(sub) = &r.rows[0].stepped.input else { panic!("the first scenario does not begin with `submit`") };
    walk(m, org, sub, &[], requester, at(m, day, "10:00")).unwrap_or_else(|k| panic!("{requester} on {day}: refused ({k})"))
}

/// The requesters the check counts under `ok` (the JSON's `cases`: the stages that go elsewhere or
/// are skipped by the shape of the organization, and the late stages with nowhere to go up to) are
/// the ones the machine finds so: a ticket of each, in each version the case names, comes to the
/// stage skipped, gone up to the one the case names, gone to the way after `then`, or late with
/// nowhere to go up to.
#[test]
fn the_cases_the_check_counts_are_what_the_machine_does() {
    let mut compared = 0;
    for name in ["desk", "late", "once", "who", "flows"] {
        let file = machine_file(name);
        let checked = checked_json(&file);
        let facts = checked.get("facts").and_then(Json::as_arr).unwrap_or_else(|| panic!("{file}: no facts"));
        let cases: Vec<&Json> = facts.iter().flat_map(|f| f.get("cases").and_then(Json::as_arr).unwrap_or_default()).collect();
        assert!(!cases.is_empty(), "{file}");
        with(&file, |m, org| {
            let mut walks: Vec<((String, String), Walk)> = Vec::new();
            for c in cases {
                let (stage, way, what) = (c.get("stage").and_then(Json::as_str).unwrap(), c.get("way").and_then(Json::as_str).unwrap(), c.get("what").and_then(Json::as_str).unwrap());
                for version in strs(c.get("versions")) {
                    // a Monday of the version
                    let day = if version == "2026-04-01" { "2026-11-16" } else { "2027-04-05" };
                    for r in c.get("requesters").and_then(Json::as_arr).unwrap() {
                        if r.get("made_by").is_some_and(|x| *x != Json::Null) {
                            continue;
                        }
                        let id = r.get("id").and_then(Json::as_str).unwrap();
                        let to = r.get("to").and_then(Json::as_str);
                        let key = (id.to_string(), day.to_string());
                        if !walks.iter().any(|(k, _)| *k == key) {
                            walks.push((key.clone(), walked(m, org, id, day)));
                        }
                        let w = &walks.iter().find(|(k, _)| *k == key).unwrap().1;
                        let ctx = format!("{file}: {what} at `{stage}` (`{way}`) for {id} on {day}");
                        // the first scenario's form does not bring every ticket to every stage, nor
                        // a stage to every way a table gives it
                        let Some((_, ways, late)) = w.stages.iter().find(|(s, ..)| s == stage) else { continue };
                        let Some(came) = ways.iter().find(|(x, _)| x == way).map(|(_, c)| c.clone()) else { continue };
                        match what {
                            // a stage done with an earlier approval does not wait, and is not late
                            "late" if w.once.iter().any(|(s, _)| s == stage) => continue,
                            "late" => assert!(late.is_some() || came == Comes::Agent, "{ctx}: the machine goes up"),
                            // an agent answers first: the way after `else` is skipped when it doubts
                            _ if came == Comes::Agent => continue,
                            "skipped" => assert_eq!(came, Comes::Skipped, "{ctx}"),
                            "up" => {
                                let to = to.unwrap_or_else(|| panic!("{ctx}: the case names nobody it goes up to")).to_string();
                                assert_eq!(came, Comes::People(vec![to.clone()], 1), "{ctx}");
                                assert!(w.ups.contains(&(stage.to_string(), to)), "{ctx}: {:?}", w.ups);
                            }
                            "then" => assert!(matches!(came, Comes::To(_)), "{ctx}: {came:?}"),
                            other => panic!("{ctx}: a case of a kind the test does not read: {other}"),
                        }
                        compared += 1;
                    }
                }
            }
        });
    }
    assert!(compared > 150, "{compared}");
}

/// W510 names the requesters whose ticket nobody answers, and the machine approves a ticket of
/// theirs as it is made, skipping each stage in the history; anyone else's waits for someone. The
/// desk request says it of the tickets its stage with a condition does not come to (DESIGN 5.4).
#[test]
fn w510_names_the_requesters_whose_ticket_nobody_answers() {
    for (name, scenario_name, skipped) in [("late", "nowhere_to_go_up", 4), ("desk", "nobody_answers_the_president", 2)] {
        let file = machine_file(name);
        let o = common::check(&file);
        let w510: Vec<&ringi::diag::Diag> = o.diags.iter().filter(|d| d.code == "W510").collect();
        assert_eq!(w510.len(), 1, "{file}:\n{}", common::shown(&o, Lang::En));
        let notes: Vec<&str> = w510[0].notes.iter().map(|n| n.en.as_str()).collect();
        assert!(notes.contains(&"For 1 requester: e0001 (President (fictional))."), "{notes:?}");
        with(&file, |m, org| {
            let sc = m.s.file.scenarios.iter().find(|sc| sc.name.text == scenario_name).or(m.s.file.scenarios.first()).unwrap();
            let r = scenario::run(m, org, sc);
            let Input::Submit(sub) = &r.rows[0].stepped.input else { panic!() };
            // the form of the desk's scenario keeps the board away (90,000 JPY)
            let sub = if name == "desk" { ran(m, org, "nobody_answers_the_president").rows[0].stepped.input.clone() } else { Input::Submit(sub.clone()) };
            let Input::Submit(sub) = sub else { panic!() };
            let now = at(m, "2026-11-16", "10:00");
            for p in &org.in_effect(m.day(now)).unwrap().people {
                let s = scenario::step(m, org, None, Input::Submit(Submit { requester: p.id.clone(), by: None, ..sub.clone() }), now).unwrap();
                let (t, _) = s.answer.as_ref().unwrap_or_else(|r| panic!("{}: {}", p.id, r.text.en));
                let at_once = t.status.word() == "approved";
                assert_eq!(at_once, p.id == "e0001", "{file}: {}: {}", p.id, went(&s));
                if at_once {
                    let said = recorded(&s);
                    assert_eq!(said.iter().filter(|(did, _)| did == "skipped").count(), skipped, "{said:?}");
                    assert_eq!(said.last().map(|(did, _)| did.as_str()), Some("ticket approved"));
                }
            }
        });
    }
    // what the desk's finding says of the stage a condition decides
    let o = common::check(&machine_file("desk"));
    let d = o.diags.iter().find(|d| d.code == "W510").unwrap();
    assert_eq!(d.notes[0].en, "`first` and `decide` are skipped for lack of an approver, and a ticket that `board` does not come to (a condition decides it) has no stage with anyone to answer it.");
    assert_eq!(d.notes[1].en, "Such a ticket of theirs is approved as soon as it is submitted, with nobody answering it.");
    assert_eq!(d.notes[0].ja, "`first`、`decide` はどれも承認者がいないので飛ばされ、条件で来る `board` が来ないチケットには、答える人のいるステップがありません。");
}

/// E502 names the stages that stop for the president (`top`: a stage that goes up with nothing
/// after `then`, one whose way after `then` has nobody either, and `when empty stay`), and the
/// machine stops a ticket of the president's at each of them, and nobody else's anywhere; the
/// stage with `then skip` is skipped and the one with `then board` goes to the board.
#[test]
fn e502_names_the_stages_the_machine_stops_at() {
    let file = machine_file("top");
    let o = common::check(&file);
    let stopped: Vec<String> = o.diags.iter().filter(|d| d.code == "E502").map(|d| d.message.en.split('`').nth(1).unwrap().to_string()).collect();
    assert_eq!(stopped, ["stopped", "nobody", "stays"]);
    assert!(o.diags.iter().filter(|d| d.code == "E502").all(|d| d.notes.iter().any(|n| n.en == "For 1 requester: e0001 (President (fictional)).")));
    with(&file, |m, org| {
        for day in ["2026-11-16", "2027-04-05"] {
            for p in &org.in_effect(ritsu_ports::day_of(day).unwrap()).unwrap().people {
                let w = walked(m, org, &p.id, day);
                let stops: Vec<&str> = w.stages.iter().filter(|(_, ways, _)| ways.iter().any(|(_, c)| *c == Comes::Stopped)).map(|(s, ..)| s.as_str()).collect();
                assert_eq!(stops, if p.id == "e0001" { vec!["stopped", "nobody", "stays"] } else { Vec::new() }, "{} on {day}", p.id);
                if p.id == "e0001" {
                    let of = |stage: &str| w.stages.iter().find(|(s, ..)| s == stage).map(|(_, ways, _)| ways[0].1.clone());
                    assert_eq!(of("skipped"), Some(Comes::Skipped));
                    assert_eq!(of("turned"), Some(Comes::To("board".into())));
                    assert_eq!(w.ended, "approved");
                }
            }
        }
    });
}

/// W502 names a stage that goes elsewhere for a seat that is empty (the head of legal, in the
/// version of 2027-04-01), and the machine sends the stage of those requesters' tickets up to the
/// one the finding names, in that version alone.
#[test]
fn w502_names_the_stage_that_goes_up_from_an_empty_seat() {
    let file = machine_file("desk");
    let o = common::check(&file);
    let d = o.diags.iter().find(|d| d.code == "W502").unwrap_or_else(|| panic!("{}", common::shown(&o, Lang::En)));
    assert_eq!(d.message.en, "In the organization from 2027-04-01, the stage `decide` goes elsewhere or is skipped for lack of an approver");
    let notes: Vec<&str> = d.notes.iter().map(|n| n.en.as_str()).collect();
    assert!(notes.contains(&"It goes up to e0102 (Head of Corporate Division) for 2 requesters: e9002 (Lawyer 2) and e9003 (Lawyer 3)."), "{notes:?}");
    with(&file, |m, org| {
        for id in ["e9002", "e9003"] {
            let decide = |day: &str| walked(m, org, id, day).stages.iter().find(|(s, ..)| s == "decide").map(|(_, ways, _)| ways[0].1.clone());
            assert_eq!(decide("2027-04-05"), Some(Comes::People(vec!["e0102".into()], 1)), "{id}");
            assert!(walked(m, org, id, "2027-04-05").ups.contains(&("decide".to_string(), "e0102".to_string())), "{id}");
            // with the general counsel in the seat, the head of legal answers
            assert_eq!(decide("2026-11-16"), Some(Comes::People(vec!["e9001".into()], 1)), "{id}");
        }
    });
}

// ── What a scenario does not hold (DESIGN 21, items 1 and 2) ──

/// A scenario holds no date the calendar gives: with a calendar that closes two more days, the
/// same rows go as written and `run` shows the later deadlines, and a row for a clock, written at
/// the time the first calendar gives, is said where it is with the time the clock is set for.
#[test]
fn the_deadlines_come_from_the_calendar_not_from_the_scenario() {
    let file = machine_file("desk");
    let src = std::fs::read_to_string(&file).unwrap();
    let other = src.replace("use calendar office from \"../../examples/shared/office.cal\"", "use calendar office from \"closed.cal\"");
    assert_ne!(src, other);
    let shown = |text: &str, name: &str| {
        let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
        let o = run.text(&file, text);
        assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
        ringi::engine::with_text(&o, &run, text, |m, org| {
            let sc = m.s.file.scenarios.iter().find(|sc| sc.name.text == name).unwrap();
            let r = scenario::run(m, org, sc);
            (scenario::render(m, &r, Lang::En), r.stop.map(|s| (s.code, s.text.en.clone(), s.notes.iter().map(|n| n.en.clone()).collect::<Vec<_>>())))
        })
        .unwrap()
    };
    // no row of a clock: as written under either calendar, with the deadline each gives
    let (office, stop) = shown(&src, "approved_by_a_quorum");
    assert!(stop.is_none());
    assert!(office.starts_with("2026-11-16 10:00  submit   by e1201  -> first (e1101), due 2026-11-19 end of day\n"), "{office}");
    let (closed, stop) = shown(&other, "approved_by_a_quorum");
    assert!(stop.is_none(), "{stop:?}");
    assert!(closed.starts_with("2026-11-16 10:00  submit   by e1201  -> first (e1101), due 2026-11-23 end of day\n"), "{closed}");
    // a row of a clock at the time the office's calendar gives: not when the clock rings
    let (_, stop) = shown(&other, "late_and_up");
    let (code, text, notes) = stop.unwrap();
    assert_eq!((code, text.as_str()), ("E604", "The clock of `remind` (`first`) is set for 2026-11-20 09:00, and the row is at 2026-11-18 09:00"));
    assert_eq!(notes[0], "A clock's row is written at the time the clock rings: the lowered dates decide the time, and a scenario holds none.");
}

/// A scenario holds no answer of a book: the balance `run` shows at the end is the book's, counted
/// from what the scenario grants, and a grant too small for the request is the book's to refuse.
#[test]
fn the_balances_come_from_the_book_not_from_the_scenario() {
    let file = "examples/leave.ringi";
    let src = std::fs::read_to_string(file).unwrap();
    let ran_with = |days: u32| {
        let text = src.replace("days: 20)", &format!("days: {days})"));
        assert!(days == 20 || text != src);
        let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
        let o = run.text(file, &text);
        assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
        ringi::engine::with_text(&o, &run, &text, |m, org| {
            let r = scenario::run(m, org, &m.s.file.scenarios[0]);
            (scenario::render(m, &r, Lang::En), r.stop.map(|s| (s.code, s.text.en.clone())))
        })
        .unwrap()
    };
    let (shown, stop) = ran_with(20);
    assert!(stop.is_none());
    assert!(shown.contains("the book `leave` at the end: leave_left(e1203) 12 day, granted -20 day, taken 8 day\n"), "{shown}");
    let (shown, stop) = ran_with(9);
    assert!(stop.is_none());
    assert!(shown.contains("the book `leave` at the end: leave_left(e1203) 1 day, granted -9 day, taken 8 day\n"), "{shown}");
    // eight days of leave from a grant of five
    let (shown, stop) = ran_with(5);
    let (code, text) = stop.unwrap();
    assert_eq!((code, text.as_str()), ("E603", "The book `leave` refuses to hold `take` (`over_balance`): a server does not take the event of this row"), "{shown}");
}

/// After a stage goes up, its deadline is counted from the time it went up, not from the time the
/// stage or the ticket started (DESIGN 7.5; 21, item 2), and a clock given late does not make the
/// deadline longer than that.
#[test]
fn after_going_up_the_deadline_is_counted_from_the_time_it_went_up() {
    with(&machine_file("desk"), |m, org| {
        // the board's stage starts a day after the ticket: Tuesday the 17th
        let r = ran(m, org, "clocks_from_each_start");
        let board = r.rows[1].ticket().unwrap();
        assert_eq!((board.stage().unwrap().started, board.stage().unwrap().due), (Some(at(m, "2026-11-17", "11:00")), Some(at(m, "2026-11-21", "00:00"))));
        // it goes up as Saturday the 21st begins: three business days from that day, the end of
        // Wednesday the 25th. From the start of the stage it would be the end of the 20th, and from
        // the start of the ticket the end of the 19th
        let up = r.rows[3].ticket().unwrap();
        assert_eq!((up.stage().unwrap().up, up.stage().unwrap().due), (1, Some(at(m, "2026-11-26", "00:00"))));
        // the clock given two days late, on Monday the 23rd: the same deadline, not three business
        // days from the 23rd
        let (line, late) = give(m, org, board, Event::Late, at(m, "2026-11-23", "09:00"));
        assert!(line.contains("record(went up)"), "{line}");
        assert_eq!(late.unwrap().stage().unwrap().due, Some(at(m, "2026-11-26", "00:00")));
        // given after that deadline too, the stage is late again at once: its deadline is not
        // before the time it went up
        let (_, later) = give(m, org, board, Event::Late, at(m, "2026-11-27", "09:00"));
        assert_eq!(later.unwrap().stage().unwrap().due, Some(at(m, "2026-11-27", "09:00")));
    });
}

/// A clock's event comes before whatever else happens at its time or after it (DESIGN 7.3): an
/// approval a minute before the deadline is taken, one at the deadline or after it is refused
/// until the clock's `late` is given, and a clock given before its time is refused.
#[test]
fn a_clock_that_has_rung_comes_first() {
    with(&machine_file("desk"), |m, org| {
        let t = ticket_after(m, org, "approved_by_a_quorum", 0);
        let (remind, due) = (t.stage().unwrap().remind.unwrap(), t.stage().unwrap().due.unwrap());
        assert_eq!((remind, due), (at(m, "2026-11-18", "09:00"), at(m, "2026-11-20", "00:00")));
        assert!(give(m, org, &t, approve("e1101"), remind - 60).0.contains("-> in progress at board | "));
        assert_eq!(give(m, org, &t, approve("e1101"), remind).0, "approve by e1101: in progress at first refused (clock)");
        assert_eq!(give(m, org, &t, Event::Late, remind).0, "late: in progress at first refused (clock)");
        assert_eq!(give(m, org, &t, Event::Remind, remind - 60).0, "remind: in progress at first refused (clock)");
        let (_, reminded) = give(m, org, &t, Event::Remind, remind);
        let reminded = reminded.unwrap();
        // reminded once: no reminder is set again
        assert_eq!(give(m, org, &reminded, Event::Remind, remind + 60).0, "remind: in progress at first refused (clock)");
        assert!(give(m, org, &reminded, approve("e1101"), due - 60).0.contains("-> in progress at board | "));
        for (who, ev) in [("an approver", approve("e1101")), ("the requester", Event::Withdraw { by: "e1201".into() }), ("an administrator", Event::HandOver { by: None, from: "e1101".into(), to: "e3101".into() })] {
            let (line, _) = give(m, org, &reminded, ev, due);
            assert!(line.ends_with(" refused (clock)"), "{who}: {line}");
        }
        // the refusal says which clock, and when it rang
        let s = scenario::step(m, org, Some(&reminded), Input::Event(approve("e1101")), due + 3600).unwrap();
        let said = s.answer.unwrap_err().text.en;
        assert!(said.contains("`late`") && said.contains("2026-11-20 00:00"), "{said}");
        // with the clock given, the one it went up to answers
        let (_, up) = give(m, org, &reminded, Event::Late, due);
        assert!(give(m, org, &up.unwrap(), approve("e3101"), due + 3600).0.contains("-> in progress at board | "));
    });
}

// ── The longest a ticket takes (DESIGN 7.6) ──

/// The business days of the office's calendar from the day of `a` to the day of `b`: Saturdays,
/// Sundays and 12-29..01-03 are closed.
fn business_days(m: &Machine, a: i64, b: i64) -> u32 {
    let closed = |day: i64| {
        let weekday = (day + 3).rem_euclid(7); // 1970-01-01 is a Thursday: 0 is a Monday
        let date = ringi::lex::date_text(day);
        weekday >= 5 || &date[5..] >= "12-29" || &date[5..] <= "01-03"
    };
    ((m.day(a) + 1)..=m.day(b)).filter(|d| !closed(*d)).count() as u32
}

/// The slowest a ticket of the desk request can be walked: paused for all the days it may be,
/// every stage late and gone up and answered in the last minute of its second deadline, sent back
/// from the last stage as often as it may be and made again in the last minute each time. The
/// check says the request `ends within 74 business days at the most`: the walk takes no longer,
/// and comes within the days the calendar's weekends take from it (a deadline that ends before a
/// closed day goes up on the closed day, and the three days after it are counted from there).
#[test]
fn no_walk_takes_longer_than_the_check_counts() {
    let o = common::check(&machine_file("desk"));
    let fact = o.facts.iter().map(|f| f.text.en.clone()).find(|t| t.starts_with("ends within")).unwrap();
    assert_eq!(fact, "ends within 74 business days at the most (3 × 21 at its stages, 2 × 3 with the requester, 5 paused)");
    with(&machine_file("desk"), |m, org| {
        use ringi::engine::Status;
        let mut longest = 0;
        // made on each day of two weeks: how the weekends fall changes what a walk takes
        for offset in 0..14 {
            let made = at(m, "2026-11-16", "10:00") + offset * 86_400;
            if business_days(m, made - 86_400, made) == 0 {
                continue;
            }
            let sub = ran(m, org, "approved_by_a_quorum").rows[0].stepped.input.clone();
            let s = scenario::step(m, org, None, sub, made).unwrap();
            let mut t = s.answer.unwrap().0;
            let mut steps = 0;
            let mut now = made;
            while !t.status.ended() {
                steps += 1;
                assert!(steps < 200, "the walk does not end");
                let (ev, when) = match t.status {
                    Status::WithRequester => (Event::Resubmit { by: "e1201".into(), form: Values::new(), clear: Vec::new(), attempt: "attempt-2".into() }, t.resubmit_due.unwrap() - 60),
                    Status::Paused => (Event::PauseEnds, t.pause_ends.unwrap()),
                    _ => {
                        let st = t.stage().unwrap();
                        let due = st.due.unwrap();
                        let by = st.parts[0].approvers.iter().find(|a| !st.parts[0].answers.iter().any(|x| x.seat == **a)).unwrap().clone();
                        let last = t.at == Some(t.stages.len() - 1);
                        if t.pause_left > 0 {
                            // paused as late as the reminder lets it be, if one is set
                            (Event::Pause { by }, st.remind.unwrap_or(due) - 60)
                        } else if let Some(r) = st.remind {
                            (Event::Remind, r)
                        } else if st.up == 0 {
                            (Event::Late, due)
                        } else if last && t.backs < 2 && st.parts[0].answers.is_empty() {
                            (Event::SendBack { by, to: None }, due - 60)
                        } else {
                            (Event::Approve { by }, due - 60)
                        }
                    }
                };
                now = now.max(when);
                let s = scenario::step(m, org, Some(&t), Input::Event(ev), now).unwrap();
                t = s.answer.as_ref().map(|(t, _)| t.clone()).unwrap_or_else(|r| panic!("{}: {}", went(&s), r.text.en));
            }
            assert_eq!(t.status, Status::Approved);
            let took = business_days(m, made, now);
            assert!(took <= 74, "made on {}: {took} business days", scenario::local(m, made));
            longest = longest.max(took);
        }
        // the longest of the ten: a day short of the 74, which a request made on no day of these
        // two weeks comes to
        assert_eq!(longest, LONGEST_DESK_WALK, "the longest walk");
    });
}

/// The slowest a ticket of `waits` can be walked, where a way of `first` waits for `second`
/// (`same approver approves once, at the last stage`): accounting answers `first` in the last
/// minute after it went up, `second` goes up and is approved by the one it went up to, so that the
/// way that waited starts then and `first` comes again, late too; then the last stage, sent back as
/// often as it may be. The check counts `first` once for each of its two ways (`2 × 7`), and says
/// `ends within 95 business days at the most`: the walk takes no longer, and takes longer than the
/// 74 that `first` counted once would give.
#[test]
fn no_walk_takes_longer_than_the_check_counts_where_a_stage_comes_again() {
    let o = common::check(&machine_file("waits"));
    let fact = o.facts.iter().map(|f| f.text.en.clone()).find(|t| t.starts_with("ends within")).unwrap();
    assert_eq!(fact, "ends within 95 business days at the most (3 × 28 at its stages, 2 × 3 with the requester, 5 paused)");
    let said = common::shown(&o, Lang::En);
    assert!(!said.contains("W402") && !said.contains("E401"), "{said}");
    with(&machine_file("waits"), |m, org| {
        use ringi::engine::Status;
        let mut longest = 0;
        let mut came_again = 0;
        for offset in 0..14 {
            let made = at(m, "2026-11-16", "10:00") + offset * 86_400;
            if business_days(m, made - 86_400, made) == 0 {
                continue;
            }
            let sub = ran(m, org, "one_way_waits_for_the_later_stage").rows[0].stepped.input.clone();
            let s = scenario::step(m, org, None, sub, made).unwrap();
            let mut t = s.answer.unwrap().0;
            let mut steps = 0;
            let mut now = made;
            while !t.status.ended() {
                steps += 1;
                assert!(steps < 300, "the walk does not end");
                let (ev, when) = match t.status {
                    Status::WithRequester => (Event::Resubmit { by: "e1101".into(), form: Values::new(), clear: Vec::new(), attempt: "attempt-2".into() }, t.resubmit_due.unwrap() - 60),
                    Status::Paused => (Event::PauseEnds, t.pause_ends.unwrap()),
                    _ => {
                        let st = t.stage().unwrap();
                        let due = st.due.unwrap();
                        let open = st.parts.iter().find(|p| !p.done()).unwrap();
                        let by = open.approvers.iter().find(|a| !open.answers.iter().any(|x| x.seat == **a)).unwrap().clone();
                        if t.pause_left > 0 {
                            (Event::Pause { by }, st.remind.unwrap_or(due) - 60)
                        } else if let Some(r) = st.remind {
                            (Event::Remind, r)
                        } else if st.up == 0 {
                            (Event::Late, due)
                        } else if st.name == "last" && t.backs < 2 {
                            (Event::SendBack { by, to: None }, due - 60)
                        } else {
                            (Event::Approve { by }, due - 60)
                        }
                    }
                };
                now = now.max(when);
                let s = scenario::step(m, org, Some(&t), Input::Event(ev), now).unwrap();
                came_again += recorded(&s).iter().filter(|(did, _)| did == "deferral ended").count();
                t = s.answer.as_ref().map(|(t, _)| t.clone()).unwrap_or_else(|r| panic!("{}: {}", went(&s), r.text.en));
            }
            assert_eq!(t.status, Status::Approved);
            let took = business_days(m, made, now);
            assert!(took <= 95, "made on {}: {took} business days", scenario::local(m, made));
            longest = longest.max(took);
        }
        // `first` came again in each of the three rounds of each of the ten walks
        assert_eq!(came_again, 30);
        assert_eq!(longest, LONGEST_WAITS_WALK, "the longest walk");
        assert!(longest > 74);
    });
}

// ── The edges of the form's ranges ──

/// The check (a scenario's form), `simulate --form` and the machine's `start` read the ends of a
/// range alike, a value at each end and the one next to it: an end that is in (`>=`, `<=`) and one
/// that is out (`>`, `<`), of a number a person enters, of the count of rows, of a computation in
/// a row, of a sum, and of a computation of a sum. Where the value is not taken, the three say the
/// same sentence.
#[test]
fn the_ends_of_a_range_are_read_alike() {
    use ringi::simulate::{Ask, On, simulate};
    let file = machine_file("ranges");
    let src = std::fs::read_to_string(&file).unwrap();
    let written = "rate = 5, lines = [{price: 500JPY}, {price: 500JPY}]";
    assert!(src.contains(written));
    // rate, the prices of the rows, and what is outside its range when the form is not taken
    let edges: [(i128, &[i128], Option<&str>); 20] = [
        (0, &[500, 500], Some("`0` is outside the range of `rate` (`>0 <10`)")),
        (1, &[500, 500], None),
        (9, &[500, 500], None),
        (10, &[500, 500], Some("`10` is outside the range of `rate` (`>0 <10`)")),
        (5, &[500], Some("`lines` has 1 row, short of the least of its rows (`count >=2 <=3`)")),
        (5, &[500, 500], None),
        (5, &[500, 500, 500], None),
        (5, &[400, 400, 400, 400], Some("`lines` has 4 rows, past the limit of its rows (`count >=2 <=3`)")),
        (5, &[0, 500], Some("`0JPY` is outside the range of `lines[0].price` (`>=1JPY <=1000JPY`)")),
        (5, &[1, 500], None),
        (5, &[749, 500], None),
        (5, &[750, 500], Some("the `lines[0].fee` the form works out is outside its range (`<1500JPY`); change `price` (`lines`)")),
        (5, &[500, 750], Some("the `lines[1].fee` the form works out is outside its range (`<1500JPY`); change `price` (`lines`)")),
        (5, &[500, 1001], Some("`1001JPY` is outside the range of `lines[1].price` (`>=1JPY <=1000JPY`)")),
        (5, &[5, 4], Some("the `total` the form works out is outside its range (`>=10JPY <2000JPY`); change `price` (`lines`)")),
        (5, &[5, 5], Some("the `double` the form works out is outside its range (`>20JPY <=3000JPY`); change `price` (`lines`)")),
        (5, &[6, 5], None),
        (5, &[749, 749, 2], None),
        (5, &[749, 749, 3], Some("the `double` the form works out is outside its range (`>20JPY <=3000JPY`); change `price` (`lines`)")),
        (5, &[749, 749, 501], Some("the `double` the form works out is outside its range (`>20JPY <=3000JPY`); change `price` (`lines`)")),
    ];
    let unrun = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
    for (rate, prices, outside) in edges {
        let form = format!("rate = {rate}, lines = [{}]", prices.iter().map(|p| format!("{{price: {p}JPY}}")).collect::<Vec<_>>().join(", "));
        let text = src.replace(written, &form);
        // the check, running the scenario of the form
        let checked = common::check_text(&file, &text);
        let said: Vec<&ringi::diag::Diag> = checked.diags.iter().filter(|d| d.code.starts_with('E')).collect();
        // `simulate --form`, and the machine given the values
        let o = unrun.file(&file).unwrap();
        let sim = simulate(&o, &unrun, &Ask { requester: Some("e1201".into()), every: false, form: Some(form.clone()), on: On::Day(ritsu_ports::day_of("2026-11-16").unwrap()) });
        let values: Values = vec![("rate".to_string(), Value::Int(rate)), ("lines".to_string(), Value::List(prices.iter().map(|p| vec![("price".to_string(), Value::Int(*p))]).collect()))];
        let started = ringi::engine::with(&o, &unrun, |m, org| {
            let sub = Submit { id: "T-1".into(), requester: "e1201".into(), by: None, form: values, attempt: "attempt-1".into() };
            scenario::step(m, org, None, Input::Submit(sub), at(m, "2026-11-16", "10:00")).unwrap().answer.map(|(t, _)| t.form).map_err(|r| (r.kind, r.text.en))
        })
        .unwrap();
        match outside {
            None => {
                assert!(said.is_empty(), "{form}:\n{}", common::shown(&checked, Lang::En));
                assert!(sim.is_ok(), "{form}: {}", sim.err().unwrap().en);
                let taken = started.unwrap_or_else(|(k, t)| panic!("{form}: refused ({k}): {t}"));
                // what the form works out
                let total: i128 = prices.iter().sum();
                assert_eq!(taken.iter().find(|(n, _)| n == "total").map(|(_, v)| v), Some(&Value::Int(total)), "{form}");
                assert_eq!(taken.iter().find(|(n, _)| n == "double").map(|(_, v)| v), Some(&Value::Int(total * 2)), "{form}");
            }
            Some(why) => {
                assert_eq!(started.as_ref().err(), Some(&("form", why.to_string())), "{form}");
                assert_eq!(sim.err().map(|t| t.en), Some(why.to_string()), "{form}");
                assert_eq!(said.iter().map(|d| d.code).collect::<Vec<_>>(), ["E603"], "{form}:\n{}", common::shown(&checked, Lang::En));
                let message = said[0].message.en.to_lowercase();
                assert!(message.ends_with(&why.to_lowercase()), "{form}: {message}");
            }
        }
    }
}

/// The people a request is not theirs to make (DESIGN 5.13, 7.3): `submit` is refused for one
/// `submitted by` leaves out and for a proxy who asks for such a person, with who, in which
/// version, and by which line; one whom a later version leaves out makes a ticket of theirs again.
#[test]
fn submitted_by_refuses_the_ones_it_leaves_out() {
    with(&machine_file("who"), |m, org| {
        let r = ran(m, org, "a_proxy_asks_for_someone");
        let Input::Submit(sub) = &r.rows[0].stepped.input else { panic!() };
        // e1201 asks for e1203, as the proxies' data lets them in December
        assert_eq!((sub.requester.as_str(), sub.by.as_deref()), ("e1203", Some("e1201")));
        let made = r.rows[0].ticket().unwrap();
        assert_eq!((made.requester.as_str(), made.made_by.as_deref()), ("e1203", Some("e1201")));
        let now = at(m, "2026-12-01", "10:00");
        let ask = |requester: &str, by: Option<&str>, now: i64| {
            let s = scenario::step(m, org, None, Input::Submit(Submit { requester: requester.into(), by: by.map(str::to_string), ..sub.clone() }), now).unwrap();
            s.answer.map(|(t, _)| t.status.word()).map_err(|r| (r.kind, r.text.en))
        };
        assert_eq!(ask("e1203", None, now), Ok("in progress"));
        // the head of the first sales department is in `sales1`, the president on the board
        assert_eq!(
            ask("e3101", None, now),
            Err(("submit", "e3101 (Head of Sales Department 1) may not submit who.ringi in the organization from 2026-04-01 (`submitted by everyone except team sales1, team board`)".to_string()))
        );
        assert_eq!(ask("e0001", None, now).map_err(|(k, _)| k), Err("submit"));
        // a proxy asks in the name of the one they stand for: the one stood for is who is read
        assert_eq!(ask("e1203", Some("e1201"), now), Ok("in progress"));
        assert_eq!(
            ask("e1203", Some("e0001"), now),
            Err(("person", "e0001 (President (fictional)) is no proxy of e1203 (Sales staff 3) to submit the request on 2026-12-01 (no `submit` row of `proxies.csv`)".to_string()))
        );
        // outside the days of the proxy's row
        assert_eq!(ask("e1203", Some("e1201"), at(m, "2026-11-30", "10:00")).map_err(|(k, _)| k), Err("person"));
        assert_eq!(ask("e1203", Some("e1201"), at(m, "2027-01-04", "10:00")).map_err(|(k, _)| k), Err("person"));
        // in the version of 2027-04-01 the three of the first section are in `sales1`: e1201 does
        // not submit there, and a ticket e1201 made before is made again
        assert_eq!(ask("e1201", None, at(m, "2027-03-31", "10:00")), Ok("in progress"));
        assert_eq!(ask("e1201", None, at(m, "2027-04-01", "10:00")).map_err(|(k, _)| k), Err("submit"));
        let again = ran(m, org, "made_again_after_the_organization_changed");
        let resubmit = again.rows.iter().find(|row| row.stepped.input.word() == "resubmit").unwrap();
        assert_eq!(m.day(resubmit.stepped.now), ritsu_ports::day_of("2027-04-01").unwrap());
        let t = resubmit.ticket().unwrap();
        assert_eq!((t.round, t.status.word(), t.stage().unwrap().parts[0].approvers.clone()), (2, "in progress", vec!["e3101".to_string()]));
    });
    // `simulate --requester` refuses the same people, with the same sentence
    let (code, _, err) = cli(&["simulate", &machine_file("who"), "--requester", "e3101", "--on", "2026-12-01"]);
    assert_eq!((code, err.as_str()), (2, "error: e3101 (Head of Sales Department 1) may not submit who.ringi in the organization from 2026-04-01 (`submitted by everyone except team sales1, team board`)\n"));
    let (code, _, err) = cli(&["simulate", &machine_file("who"), "--requester", "e1201", "--on", "2027-04-01"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("e1201 (Sales staff 1) may not submit"), "{err}");
}

/// The stages `same approver approves once` does (DESIGN 5.12, 7.3), by the history of each
/// scenario of `once`: each stage done, the stage whose approval did it, and who gave it.
#[test]
fn same_approver_approves_once() {
    let done = |name: &str| -> Vec<String> {
        with(&machine_file("once"), |m, org| {
            let mut out = Vec::new();
            for row in &ran(m, org, name).rows {
                for e in row.effects() {
                    if let What::Record(r) = &e.what
                        && let Some(with) = &r.once_with
                    {
                        out.push(format!("{} with {with} ({})", r.stage.as_deref().unwrap_or_default(), r.by.as_deref().unwrap_or_default()));
                    }
                }
            }
            out
        })
    };
    // the head of the department answers `first` and `second`
    assert_eq!(done("done_with_the_approval_before"), ["second with first (e3101)"]);
    // another person's stage comes between them
    assert_eq!(done("not_across_a_stage_between"), [] as [&str; 0]);
    // one part of a stage two ways answer: the other part still waits
    assert_eq!(done("one_part_of_a_joint_stage"), ["middle with first (e3101)"]);
    // an agent answers between them
    assert_eq!(done("not_across_an_agent"), [] as [&str; 0]);
    // the agent doubts: the one after `else` answers its stage, not done with their approval
    // before it, and the stage after it is done with that approval
    assert_eq!(done("done_after_an_agent_doubts"), ["second with middle (e3101)"]);
    // gone up for lack of an approver: the one it went up to answers the stages after it once
    assert_eq!(done("done_where_it_went_up_to_for_lack_of_an_approver"), ["second with first (e0101)", "decide with first (e0101)"]);
    // gone up for being late: the stage waits for the one it went up to
    assert_eq!(done("not_done_where_it_went_up_to_late"), [] as [&str; 0]);
    assert_eq!(done("done_after_the_one_it_went_up_to_approves"), ["second with first (e3101)"]);
    // a proxy's approval is the approval of the one they answer for, and not their own
    assert_eq!(done("a_proxy_answers_for_one_and_then_for_themselves"), [] as [&str; 0]);
    assert_eq!(done("a_proxy_with_a_part_of_their_own"), [] as [&str; 0]);
    // sent back to the stage that was done: it waits there, and is not done again
    assert_eq!(done("sent_back_to_a_stage_that_was_done"), ["second with first (e3101)"]);
    // made again in the next round: the approval of the round before does nothing, and the one of
    // this round does the stage again
    assert_eq!(done("made_again_after_it_was_done"), ["second with first (e3101)", "second with first (e3101)"]);
    // the approval taken back takes the stage it did with it; given again, it does the stage again
    assert_eq!(done("taken_back_with_the_stage_it_did"), ["second with first (e3101)", "second with first (e3101)"]);
    with(&machine_file("once"), |m, org| {
        use ringi::engine::{PartState, Phase};
        // a stage that is done has no clock and no notice, and the stage after it is counted from
        // the approval that did it
        let r = ran(m, org, "done_with_the_approval_before");
        let row = r.rows.iter().find(|row| row.effects().iter().any(|e| matches!(&e.what, What::Record(r) if r.once_with.is_some()))).unwrap();
        let t = row.ticket().unwrap();
        let second = t.stages.iter().find(|s| s.name == "second").unwrap();
        assert_eq!((second.phase, second.parts[0].state.clone(), second.remind, second.due), (Phase::Once, PartState::Once("first".into()), None, None));
        assert!(!row.effects().iter().any(|e| matches!(&e.what, What::Notify { stage: Some(s), .. } | What::SetTimer { stage: Some(s), .. } if s == "second")), "{:?}", row.effects());
        assert_eq!(t.stage().unwrap().started, Some(row.stepped.now));
        // taken back: the stage it did is as it was before the ticket came to it
        let r = ran(m, org, "taken_back_with_the_stage_it_did");
        let back = r.rows.iter().find(|row| row.stepped.input.word() == "retract").unwrap().ticket().unwrap();
        let second = back.stages.iter().find(|s| s.name == "second").unwrap();
        assert_eq!((back.stage_name(), second.phase, second.parts.len()), (Some("first"), Phase::Ahead, 0));
    });
}

// ── Answering once, at the last stage (DESIGN 5.12, 7.3) ──

/// The lines of the history of a scenario of `tests/machine/<file>.ringi` that say a stage waits
/// for a later one (`deferred`), is done with the approval given there (`approved`), or is started
/// after all (`deferral ended`): the row, what was done, the stage, the later stage, who (the one
/// waited for; the one who approved, and for whom; the ones the stage is started with), the note.
fn put_off(file: &str, name: &str) -> Vec<String> {
    with(&machine_file(file), |m, org| {
        let mut out = Vec::new();
        for (n, row) in ran(m, org, name).rows.iter().enumerate() {
            for e in row.effects() {
                if let What::Record(r) = &e.what
                    && let Some(with) = &r.once_with
                {
                    let who = match (&r.by, &r.proxy_of) {
                        (Some(by), Some(of)) => format!("{by} for {of}"),
                        (Some(by), None) => by.clone(),
                        _ => r.to.join(", "),
                    };
                    let note = r.note.as_ref().map(|n| format!("; {n}")).unwrap_or_default();
                    out.push(format!("{n}: {} {} with {with} ({who}){note}", r.did, r.stage.as_deref().unwrap_or_default()));
                }
            }
        }
        out
    })
}

/// The stages of a ticket in the order it holds them, each with its phase; a stage that came again
/// has `*`.
fn phases(t: &Ticket) -> String {
    t.stages.iter().map(|s| format!("{}{} {}", s.name, if s.again { "*" } else { "" }, s.phase.word())).collect::<Vec<_>>().join(", ")
}

/// in progress -- a stage starts whose one approver alone answers a later stage too --> the stage
/// waits (`deferred`), with no clock and no notice, and the ticket goes on; the approval given at
/// the last such stage does it (`once`), with a line of the history for each.
#[test]
fn a_stage_waits_for_the_last_stage_its_one_approver_answers() {
    // the head of the department is the manager's manager: `first` waits for `second`
    assert_eq!(put_off("last", "one_answer_at_the_later_of_two_stages"), ["0: deferred first with second (e3101)", "1: approved first with second (e3101)"]);
    let two = rows("last", "one_answer_at_the_later_of_two_stages");
    assert_eq!(two[0], "submit by e1101: (none) -> in progress at second | record(submitted) record(deferred) notify(waiting: e3101) set(remind) set(late)");
    assert_eq!(two[1], "approve by e3101: in progress at second -> in progress at decide | record(approved) record(approved) cancel(remind) cancel(late) notify(waiting: e0101) set(remind) set(late)");
    // accounting answers between them: the approval comes after it, once
    assert_eq!(put_off("last", "one_answer_after_the_stage_between"), ["0: deferred first with second (e3101)", "2: approved first with second (e3101)"]);
    // three stages of one person: the first two wait for the last
    assert_eq!(put_off("last", "three_stages_one_answer_at_the_last"), ["0: deferred first with decide (e3101)", "0: deferred middle with decide (e3101)", "2: approved first with decide (e3101)", "2: approved middle with decide (e3101)"]);
    // a stage between that every director answers, the president among them: it is answered there,
    // and the stage the president alone answers waits for the last
    assert_eq!(put_off("last", "a_stage_between_that_takes_their_answer_too"), ["0: deferred first with decide (e0001)", "6: approved first with decide (e0001)"]);
    with(&machine_file("last"), |m, org| {
        use ringi::engine::{PartState, Phase};
        let r = ran(m, org, "one_answer_after_the_stage_between");
        // the stage that waits has not started: no clock, and nobody is told of it
        let t = r.rows[0].ticket().unwrap();
        let first = &t.stages[0];
        assert_eq!((first.phase, first.parts[0].state.clone(), first.started, first.remind, first.due), (Phase::Deferred, PartState::Deferred("second".into()), None, None, None));
        assert_eq!(phases(t), "first deferred, middle open, second ahead, decide ahead");
        for row in &r.rows {
            assert!(!row.effects().iter().any(|e| matches!(&e.what, What::Notify { stage: Some(s), .. } | What::SetTimer { stage: Some(s), .. } if s == "first")), "{:?}", row.effects());
        }
        // the one it waits for is not asked until the later stage: the ticket is not theirs to
        // answer, to pause or to send back at the stage between
        let now = at(m, "2026-11-16", "10:30");
        assert_eq!(give(m, org, t, approve("e3101"), now).0, "approve by e3101: in progress at middle refused (person)");
        assert_eq!(give(m, org, t, Event::Pause { by: "e3101".into() }, now).0, "pause by e3101: in progress at middle refused (person)");
        assert_eq!(give(m, org, t, Event::SendBack { by: "e3101".into(), to: None }, now).0, "send back by e3101: in progress at middle refused (person)");
        // done: the stage is `once`, as one done with an earlier approval is, and never started
        let t = r.rows[2].ticket().unwrap();
        let first = &t.stages[0];
        assert_eq!((first.phase, first.parts[0].state.clone(), first.started), (Phase::Once, PartState::Once("second".into()), None));
        assert_eq!(phases(t), "first once, middle answered, second answered, decide open");
        // the clocks of the stage after it are counted from the approval
        assert_eq!(t.stage().unwrap().started, Some(r.rows[2].stepped.now));
    });
    // the JSON of the ticket and of the lines (DESIGN 16.2): no key is added for it
    let (code, out, _) = cli(&["run", &machine_file("last"), "--format", "json"]);
    assert_eq!(code, 0);
    let v = json::parse(&out).unwrap();
    let step = v.get("scenarios").and_then(Json::as_arr).unwrap()[0].get("rows").and_then(Json::as_arr).unwrap()[0].get("step").and_then(|s| s.get("answer")).and_then(|a| a.get("ok")).cloned().unwrap();
    let first = &step.get("ticket").and_then(|t| t.get("stages")).and_then(Json::as_arr).unwrap()[0];
    assert_eq!(first.get("phase").and_then(Json::as_str), Some("deferred"));
    assert_eq!(first.get("parts").and_then(Json::as_arr).unwrap()[0].compact(), r#"{"way":"manager","key":"manager","approvers":["e3101"],"team":null,"needed":1,"answers":[],"state":"deferred","of":"second","sole":{"by":{"person":"e3101"}}}"#);
    assert_eq!(
        step.get("effects").and_then(Json::as_arr).unwrap()[1].compact(),
        r#"{"key":{"ticket":"T-1","round":1,"serial":2},"what":{"history":{"did":"deferred","stage":"first","by":null,"proxy_of":null,"organization":"2026-04-01","once_with":"second","to":["e3101"],"note":null,"comment":null,"cleared":[]}}}"#
    );
}

/// A stage of several ways (`a and b`): each way waits or not by itself. The stage starts with the
/// ways that are answered there, waits (`deferred`) when they have answered, and is `answered` when
/// the approval given at the later stage does the way that waited. A way that waits for a stage of
/// several ways is done when that stage has every approval it needs.
#[test]
fn one_way_of_a_stage_waits_and_the_others_are_answered_there() {
    assert_eq!(put_off("last", "one_part_of_a_joint_stage_waits"), ["0: deferred first with second (e3101)", "2: approved first with second (e3101)"]);
    let joint = rows("last", "one_part_of_a_joint_stage_waits");
    assert_eq!(joint[0], "submit by e1101: (none) -> in progress at first | record(submitted) record(deferred) notify(waiting: e1001, e1002) set(remind) set(late)");
    with(&machine_file("last"), |m, org| {
        use ringi::engine::{PartState, Phase};
        let r = ran(m, org, "one_part_of_a_joint_stage_waits");
        let states = |n: usize| {
            let st = &r.rows[n].ticket().unwrap().stages[0];
            (st.phase, st.parts.iter().map(|p| p.state.clone()).collect::<Vec<_>>())
        };
        assert_eq!(states(0), (Phase::Open, vec![PartState::Deferred("second".into()), PartState::Open]));
        assert_eq!(states(1), (Phase::Deferred, vec![PartState::Deferred("second".into()), PartState::Passed]));
        assert_eq!(states(2), (Phase::Answered, vec![PartState::Once("second".into()), PartState::Passed]));
        // the one whose way waits does not answer the stage where the others do
        let now = at(m, "2026-11-16", "10:30");
        assert_eq!(give(m, org, r.rows[0].ticket().unwrap(), approve("e3101"), now).0, "approve by e3101: in progress at first refused (person)");
        // nor do they pause it: the stage is the others' to answer there
        assert_eq!(give(m, org, r.rows[0].ticket().unwrap(), Event::Pause { by: "e3101".into() }, now).0, "pause by e3101: in progress at first refused (person)");
        assert!(give(m, org, r.rows[0].ticket().unwrap(), Event::Pause { by: "e1002".into() }, now).0.starts_with("pause by e1002: in progress at first -> paused at first"));
        // the approval given there is retracted while the stage waits: the ticket is back at it,
        // and the way that waits waits as before
        let (said, back) = give(m, org, r.rows[1].ticket().unwrap(), Event::Retract { by: "e1002".into() }, at(m, "2026-11-16", "11:30"));
        assert_eq!(said, "retract by e1002: in progress at second -> in progress at first | record(retracted) cancel(remind) cancel(late) notify(waiting: e1001, e1002) set(remind) set(late)");
        let back = back.unwrap();
        assert_eq!(phases(&back), "first open, second ahead, decide ahead");
        assert_eq!(back.stages[0].parts.iter().map(|p| p.state.clone()).collect::<Vec<_>>(), vec![PartState::Deferred("second".into()), PartState::Open]);
    });
    // the later stage is one of two ways: the approval of the one waited for is given in row 1,
    // and the stage that waited is done in row 2, when accounting has answered too
    assert_eq!(put_off("last", "waits_for_a_part_of_a_joint_stage"), ["0: deferred first with second (e3101)", "2: approved first with second (e3101)"]);
    // stage lines, the later one on a condition: where it comes, the way waits for it
    assert_eq!(put_off("waits", "one_way_waits_for_the_later_stage"), ["0: deferred first with second (e3101)", "2: approved first with second (e3101)"]);
    // where it does not come for the form, the stage is answered where it is
    assert_eq!(put_off("waits", "the_later_stage_does_not_come"), [] as [&str; 0]);
    assert_eq!(rows("waits", "the_later_stage_does_not_come")[1], "approve by e3101: in progress at first -> in progress at first | record(approved)");
}

/// Two ways of one stage, each of which waits for a later stage of its own: each is done where its
/// one approver answers. Where the stage one of them waits for is answered by another, the stage
/// comes again with both: the way that starts, and the way that waits on, which is the later
/// coming's from then, and is done there.
#[test]
fn two_ways_of_a_stage_wait_for_two_stages() {
    assert_eq!(put_off("last", "two_ways_wait_for_two_stages"), ["0: deferred first with second (e1101)", "0: deferred first with decide (e3101)", "2: approved first with second (e1101)", "3: approved first with decide (e3101)"]);
    assert_eq!(
        put_off("last", "one_of_two_ways_starts_and_the_other_waits_on"),
        [
            "0: deferred first with second (e1101)",
            "0: deferred first with decide (e3101)",
            "3: deferral ended first with second (e1101); the stage it waited for was answered without e1101",
            "5: approved first with decide (e3101)",
        ]
    );
    with(&machine_file("last"), |m, org| {
        use ringi::engine::PartState;
        let r = ran(m, org, "two_ways_wait_for_two_stages");
        assert_eq!(phases(r.rows[2].ticket().unwrap()), "first deferred, middle answered, second answered, decide open");
        assert_eq!(phases(r.rows[3].ticket().unwrap()), "first once, middle answered, second answered, decide answered");
        let r = ran(m, org, "one_of_two_ways_starts_and_the_other_waits_on");
        let states = |n: usize| {
            let t = r.rows[n].ticket().unwrap();
            (phases(t), t.stages[3].parts.iter().map(|p| p.state.clone()).collect::<Vec<_>>(), t.deferred(t.stages.len()))
        };
        let decide = || PartState::Deferred("decide".into());
        assert_eq!(states(3), ("first deferred, middle answered, second answered, first* open, decide ahead".to_string(), vec![PartState::Open, decide()], vec![(3, 1)]));
        // answered by the one it started with: it waits, having started
        assert_eq!(states(4), ("first deferred, middle answered, second answered, first* deferred, decide open".to_string(), vec![PartState::Passed, decide()], vec![(3, 1)]));
        assert_eq!(states(5), ("first deferred, middle answered, second answered, first* answered, decide answered".to_string(), vec![PartState::Passed, PartState::Once("decide".into())], Vec::new()));
    });
}

/// An agent's stage is answered by the agent first, so no stage waits for it, and it waits for
/// none as it starts. When the agent doubts, the one after `else` answers it, and their part waits
/// for the last stage ahead they alone answer, as a part does when its stage starts: they answer
/// once, seeing the doubt there.
#[test]
fn an_agent_is_waited_for_by_no_stage_and_waits_once_it_doubts() {
    // `first` waits for `second`, past the agent's stage
    assert_eq!(put_off("last", "past_an_agent"), ["0: deferred first with second (e3101)", "2: approved first with second (e3101)"]);
    // the agent doubts: its stage comes to the manager, who answers `second` too, so it waits for
    // `second` with `first`, and the manager's one approval there does both
    assert_eq!(put_off("last", "the_agent_doubts"), ["0: deferred first with second (e3101)", "1: deferred middle with second (e3101)", "2: approved first with second (e3101)", "2: approved middle with second (e3101)"]);
    let doubt = rows("last", "the_agent_doubts");
    assert_eq!(doubt[1], "agent answered: in progress at middle -> in progress at second | record(agent doubt) record(deferred) cancel(remind) cancel(late) notify(waiting: e3101) set(remind) set(late)");
    assert_eq!(doubt[2], "approve by e3101: in progress at second -> in progress at decide | record(approved) record(approved) record(approved) cancel(remind) cancel(late) notify(waiting: e0101) set(remind) set(late)");
    with(&machine_file("last"), |m, org| {
        use ringi::engine::PartState;
        let r = ran(m, org, "the_agent_doubts");
        // the agent's stage waited, having started; done with the approval of `second`
        assert_eq!(phases(r.rows[1].ticket().unwrap()), "first deferred, middle deferred, second open, decide ahead");
        let t = r.rows[2].ticket().unwrap();
        assert_eq!(phases(t), "first once, middle answered, second answered, decide open");
        assert_eq!(t.stages[1].parts[0].state, PartState::Once("second".into()));
    });
}

/// in progress -- the later stage has every approval it needs, and none in the seat of the one a
/// stage waited for --> the stage that waited starts then (`deferral ended`), before the ticket
/// goes on: late and approved where it went up to, handed over to another, answered by an approver
/// an administrator added. It comes again in the ticket's stages, after the later stage, with a
/// deadline counted from then.
#[test]
fn a_stage_that_waited_starts_when_the_later_stage_is_answered_without_its_approver() {
    let without = "the stage it waited for was answered without e3101";
    assert_eq!(put_off("last", "late_and_approved_where_it_went_up_to"), ["0: deferred first with second (e3101)".to_string(), format!("4: deferral ended first with second (e3101); {without}")]);
    assert_eq!(put_off("last", "handed_over_at_the_later_stage"), ["0: deferred first with second (e3101)".to_string(), format!("3: deferral ended first with second (e3101); {without}")]);
    assert_eq!(put_off("last", "an_added_approver_answers_the_later_stage"), ["0: deferred first with second (e3101)".to_string(), format!("3: deferral ended first with second (e3101); {without}")]);
    // an approver is added, and the one waited for answers all the same: their approval does it
    assert_eq!(put_off("last", "an_approver_is_added_and_the_one_who_waited_answers"), ["0: deferred first with second (e3101)", "3: approved first with second (e3101)"]);
    let late = rows("last", "late_and_approved_where_it_went_up_to");
    assert_eq!(late[4], "approve by e0101: in progress at second -> in progress at first | record(approved) record(deferral ended) cancel(remind) cancel(late) notify(waiting: e3101) set(remind) set(late)");
    assert_eq!(late[5], "approve by e3101: in progress at first -> in progress at decide | record(approved) cancel(remind) cancel(late) notify(waiting: e0101) set(remind) set(late)");
    with(&machine_file("last"), |m, org| {
        use ringi::engine::{PartState, Phase};
        let r = ran(m, org, "late_and_approved_where_it_went_up_to");
        let t = r.rows[4].ticket().unwrap();
        // the stage came again, after the later stage: the stages are in the order of time
        assert_eq!(phases(t), "first deferred, middle answered, second answered, first* open, decide ahead");
        assert_eq!((t.at, t.live(0), t.live(3)), (Some(3), false, true));
        // its approver decided then, its clocks counted from then: three business days from
        // Friday the 20th, at the end of Wednesday the 25th
        let again = &t.stages[3];
        assert_eq!((again.parts[0].approvers.clone(), again.parts[0].state.clone(), again.started), (vec!["e3101".to_string()], PartState::Open, Some(at(m, "2026-11-20", "10:00"))));
        assert_eq!((again.remind, again.due), (Some(at(m, "2026-11-24", "09:00")), Some(at(m, "2026-11-26", "00:00"))));
        // answered: the first coming stays as it was, and no part of the ticket waits
        let t = r.rows[5].ticket().unwrap();
        assert_eq!(phases(t), "first deferred, middle answered, second answered, first* answered, decide open");
        assert_eq!((t.stages[0].phase, t.deferred(t.stages.len())), (Phase::Deferred, Vec::new()));
        assert_eq!(r.ticket().unwrap().status.word(), "approved");
    });
}

/// in progress -- the ticket comes to the later stage, and it no longer comes to the one a stage
/// waited for alone (an approver's edit took it off the route, or gave it to another; the version
/// of the organization changed, and it has nobody) --> the stage that waited starts first, before
/// the later stage, with the one who answers it now.
#[test]
fn a_stage_that_waited_starts_before_a_later_stage_that_no_longer_comes_to_its_approver() {
    assert_eq!(put_off("last", "the_later_stage_leaves_the_route"), ["0: deferred first with second (e3101)", "2: deferral ended first with second (e3101); the stage it waited for is not on the route"]);
    assert_eq!(put_off("last", "the_later_stage_comes_to_another"), ["0: deferred first with second (e3101)", "2: deferral ended first with second (e3101); the stage it waited for does not come to e3101 alone"]);
    with(&machine_file("last"), |m, org| {
        // off the route: the stage that waited comes again where the ticket is, before the stage
        // that decides
        assert_eq!(phases(ran(m, org, "the_later_stage_leaves_the_route").rows[2].ticket().unwrap()), "first deferred, middle answered, first* open, decide ahead");
        // to another: before the later stage, which its new approver answers after it
        let r = ran(m, org, "the_later_stage_comes_to_another");
        assert_eq!(phases(r.rows[2].ticket().unwrap()), "first deferred, middle answered, first* open, second ahead, decide ahead");
        assert_eq!(went(&r.rows[4].stepped), "approve by e7001: in progress at second -> in progress at decide | record(approved) cancel(remind) cancel(late) notify(waiting: e0101) set(remind) set(late)");
    });
    // the version of the organization changes while a way of the stage waits, and the later stage
    // is skipped for lack of an approver: the way starts with the one who answers it in the new
    // version, and the later stage is skipped after it
    assert_eq!(put_off("waits", "the_later_stage_is_skipped_for_lack_of_an_approver"), ["0: deferred first with second (e9001)", "2: deferral ended first with second (e0102); the stage it waited for does not come to e9001 alone"]);
    let skipped = rows("waits", "the_later_stage_is_skipped_for_lack_of_an_approver");
    assert_eq!(skipped[2], "approve by e1002: in progress at first -> in progress at first | record(approved) record(deferral ended) cancel(late) notify(waiting: e0102) set(remind) set(late)");
    assert_eq!(skipped[3], "approve by e0102: in progress at first -> in progress at last | record(approved) record(skipped) cancel(remind) cancel(late) notify(waiting: e4001) set(remind) set(late)");
    with(&machine_file("waits"), |m, org| {
        use ringi::engine::PartState;
        let r = ran(m, org, "the_later_stage_is_skipped_for_lack_of_an_approver");
        let t = r.rows[2].ticket().unwrap();
        // the first coming keeps accounting's answer; the second has the way that waited alone
        assert_eq!(phases(t), "first deferred, first* open, second ahead, last ahead");
        assert_eq!(t.stages[0].parts.iter().map(|p| p.state.clone()).collect::<Vec<_>>(), [PartState::Deferred("second".into()), PartState::Passed]);
        assert_eq!(t.stages[1].parts.iter().map(|p| (p.way.clone(), p.approvers.clone())).collect::<Vec<_>>(), [("manager".to_string(), vec!["e0102".to_string()])]);
    });
}

/// The one who answers the stage that waited now alone answers a stage ahead too: it waits on, for
/// the last of them, and no stage is started. The version of the organization changes while
/// `first` waits for `second`; the head of the department is gone, both go up to the head of the
/// division, who decides too, and one approval does the three.
#[test]
fn a_stage_that_waited_waits_on_where_the_one_who_answers_it_now_answers_a_stage_ahead() {
    assert_eq!(
        put_off("last", "the_organization_changes_while_a_stage_waits"),
        [
            "0: deferred first with second (e9001)",
            "2: deferred first with decide (e0102); the stage it waited for does not come to e9001 alone",
            "2: deferred second with decide (e0102)",
            "3: approved first with decide (e0102)",
            "3: approved second with decide (e0102)",
        ]
    );
    // an approver's edit gives the stage that decides to the head of the department too: `second`
    // waits for it as it starts, and `first`, which waited for `second`, waits on with it
    assert_eq!(
        put_off("last", "the_later_stage_waits_too"),
        [
            "0: deferred first with second (e3101)",
            "2: deferred second with decide (e3101)",
            "2: deferred first with decide (e3101); the stage it waited for waits too",
            "3: approved first with decide (e3101)",
            "3: approved second with decide (e3101)",
        ]
    );
    with(&machine_file("last"), |m, org| {
        let r = ran(m, org, "the_organization_changes_while_a_stage_waits");
        let t = r.rows[2].ticket().unwrap();
        assert_eq!(phases(t), "first deferred, middle answered, second deferred, decide open");
        // the stage is decided in the version it was decided again in
        let v2027 = org.in_effect(ritsu_ports::day_of("2027-04-01").unwrap()).unwrap().from;
        assert_eq!((t.stages[0].org, t.stages[0].parts[0].approvers.clone()), (Some(v2027), vec!["e0102".to_string()]));
        assert_eq!(phases(r.rows[3].ticket().unwrap()), "first once, middle answered, second once, decide answered");
    });
}

/// Sent back and made again, a stage waits again. Sent back to the stage that waited, it is
/// answered there (the one who sent it back asks for its answer), and the later stage is answered
/// again. Sent back to a stage between, or the approval at the later stage retracted, the approval
/// that did the stage that waited is gone, and it waits again.
#[test]
fn sent_back_and_taken_back_with_a_stage_that_waited() {
    assert_eq!(put_off("last", "made_again_after_the_later_stage_sent_it_back"), ["0: deferred first with second (e3101)", "3: deferred first with second (e3101)", "5: approved first with second (e3101)"]);
    // to the stage that waited: answered there in row 4, and nothing waits or is done after it
    assert_eq!(put_off("last", "sent_back_to_the_stage_that_waited"), ["0: deferred first with second (e3101)", "2: approved first with second (e3101)"]);
    let to_it = rows("last", "sent_back_to_the_stage_that_waited");
    assert_eq!(to_it[3], "send back by e0101: in progress at decide -> in progress at first | record(sent back) cancel(remind) cancel(late) notify(waiting: e3101) set(remind) set(late)");
    assert_eq!(to_it[4], "approve by e3101: in progress at first -> in progress at middle | record(approved) cancel(remind) cancel(late) notify(waiting: e1001, e1002) set(remind) set(late)");
    assert_eq!(to_it[6], "approve by e3101: in progress at second -> in progress at decide | record(approved) cancel(remind) cancel(late) notify(waiting: e0101) set(remind) set(late)");
    // to the stage between
    assert_eq!(put_off("last", "sent_back_to_the_stage_between"), ["0: deferred first with second (e3101)", "2: approved first with second (e3101)", "5: approved first with second (e3101)"]);
    // retracted
    assert_eq!(put_off("last", "taken_back_with_the_stage_it_did"), ["0: deferred first with second (e3101)", "2: approved first with second (e3101)", "4: approved first with second (e3101)"]);
    with(&machine_file("last"), |m, org| {
        use ringi::engine::{PartState, Phase};
        let waits = |t: &Ticket| (t.stages[0].phase, t.stages[0].parts[0].state.clone());
        let again = (Phase::Deferred, PartState::Deferred("second".into()));
        assert_eq!(waits(ran(m, org, "sent_back_to_the_stage_between").rows[3].ticket().unwrap()), again);
        let r = ran(m, org, "taken_back_with_the_stage_it_did");
        assert_eq!(waits(r.rows[3].ticket().unwrap()), again);
        assert_eq!(went(&r.rows[3].stepped), "retract by e3101: in progress at decide -> in progress at second | record(retracted) cancel(remind) cancel(late) notify(waiting: e3101) set(remind) set(late)");
        // the later stage is one of two ways, and the approval of the other way is retracted: the
        // stage is open again, and the stage it did waits for it to have every approval again
        let r = ran(m, org, "waits_for_a_part_of_a_joint_stage");
        let (said, back) = give(m, org, r.rows[2].ticket().unwrap(), Event::Retract { by: "e1001".into() }, at(m, "2026-11-16", "12:30"));
        assert_eq!(said, "retract by e1001: in progress at decide -> in progress at second | record(retracted) cancel(remind) cancel(late) notify(waiting: e1001, e1002) set(remind) set(late)");
        let back = back.unwrap();
        assert_eq!(waits(&back), again);
        let (said, on) = give(m, org, &back, approve("e1002"), at(m, "2026-11-16", "12:40"));
        assert_eq!(said, "approve by e1002: in progress at second -> in progress at decide | record(approved) record(approved) cancel(remind) cancel(late) notify(waiting: e0101) set(remind) set(late)");
        assert_eq!(phases(&on.unwrap()), "first once, second answered, decide open");
        // a stage that came again is taken off when the ticket goes back before it. The later
        // stage went up, the one it went up to approved it, and the stage that waited started:
        // that approval retracted, the stage that waited waits as before
        let r = ran(m, org, "late_and_approved_where_it_went_up_to");
        let now = at(m, "2026-11-20", "10:30");
        let (said, back) = give(m, org, r.rows[4].ticket().unwrap(), Event::Retract { by: "e0101".into() }, now);
        assert_eq!(said, "retract by e0101: in progress at first -> in progress at second | record(retracted) cancel(remind) cancel(late) notify(waiting: e0101) set(remind) set(late)");
        let back = back.unwrap();
        assert_eq!(phases(&back), "first deferred, middle answered, second open, decide ahead");
        assert_eq!(waits(&back), again);
        // the approval of the stage that came again retracted: the ticket is back at it
        let (said, back) = give(m, org, r.rows[5].ticket().unwrap(), Event::Retract { by: "e3101".into() }, at(m, "2026-11-20", "11:30"));
        assert_eq!(said, "retract by e3101: in progress at decide -> in progress at first | record(retracted) cancel(remind) cancel(late) notify(waiting: e3101) set(remind) set(late)");
        assert_eq!(phases(&back.unwrap()), "first deferred, middle answered, second answered, first* open, decide ahead");
        // sent back from the stage that decides to the stage that came again: one `first` is left,
        // answered there
        let (said, back) = give(m, org, r.rows[5].ticket().unwrap(), Event::SendBack { by: "e0101".into(), to: Some("first".into()) }, at(m, "2026-11-20", "11:30"));
        assert_eq!(said, "send back by e0101: in progress at decide -> in progress at first | record(sent back) cancel(remind) cancel(late) notify(waiting: e3101) set(remind) set(late)");
        assert_eq!(phases(&back.unwrap()), "first* open, middle ahead, second ahead, decide ahead");
    });
}

/// `withdraw until <stage>` where the stage waits: it has not answered, and the request is
/// withdrawn until the approval it waits for is given. A stage that came again is the stage the
/// ticket is at. `pause` is for the approver of the stage the ticket is at.
#[test]
fn withdrawing_and_pausing_with_a_stage_that_waits() {
    let withdrawn = rows("last", "withdrawn_while_the_stage_waits");
    assert_eq!(withdrawn[2], "withdraw by e1101: in progress at second -> with requester | record(withdrawn) cancel(remind) cancel(late) notify(returned: e3101) set(resubmit late)");
    with(&machine_file("last"), |m, org| {
        let withdraw = || Event::Withdraw { by: "e1101".into() };
        // the approval it waited for is given: `first` has answered, and the ticket is past it
        let r = ran(m, org, "one_answer_after_the_stage_between");
        let (said, _) = give(m, org, r.rows[2].ticket().unwrap(), withdraw(), at(m, "2026-11-16", "12:30"));
        assert_eq!(said, "withdraw by e1101: in progress at decide refused (state)");
        // the later stage answered without the one waited for: `first` came again, and is the
        // stage the ticket is at
        let r = ran(m, org, "late_and_approved_where_it_went_up_to");
        let (said, _) = give(m, org, r.rows[4].ticket().unwrap(), withdraw(), at(m, "2026-11-20", "10:30"));
        assert!(said.starts_with("withdraw by e1101: in progress at first -> with requester | record(withdrawn)"), "{said}");
        let (said, _) = give(m, org, r.rows[5].ticket().unwrap(), withdraw(), at(m, "2026-11-20", "11:30"));
        assert_eq!(said, "withdraw by e1101: in progress at decide refused (state)");
    });
    // `withdraw until second` (`waits`): `second` is late, goes up and is approved by the one it
    // went up to, and the way of `first` that waited starts after it. The ticket is at `first`,
    // and has come past `second`
    with(&machine_file("waits"), |m, org| {
        let withdraw = || Event::Withdraw { by: "e1101".into() };
        let r = ran(m, org, "one_way_waits_for_the_later_stage");
        let t = r.rows[1].ticket().unwrap();
        let due = t.stage().unwrap().due.unwrap();
        let (said, up) = give(m, org, t, Event::Late, due);
        assert!(said.starts_with("late: in progress at second -> in progress at second | record(went up)"), "{said}");
        let up = up.unwrap();
        assert!(give(m, org, &up, withdraw(), due + 60).0.starts_with("withdraw by e1101: in progress at second -> with requester"));
        let (said, again) = give(m, org, &up, approve("e0101"), due + 60);
        assert!(said.starts_with("approve by e0101: in progress at second -> in progress at first | record(approved) record(deferral ended)"), "{said}");
        assert_eq!(give(m, org, &again.unwrap(), withdraw(), due + 120).0, "withdraw by e1101: in progress at first refused (state)");
    });
    let paused = rows("last", "paused_at_the_later_stage");
    assert_eq!(paused[2], "pause by e3101: in progress at second -> paused at second | record(paused) cancel(remind) cancel(late) notify(paused: e1101) set(pause ends)");
}

/// A proxy's approval at the later stage is the approval of the one they answer for (DESIGN 5.8):
/// it does the stage of theirs that waited, and no stage of the proxy's own. A proxy who has a way
/// of their own at the later stage answers twice, once in each seat, and each approval does the
/// stage that waited for that seat: one operation uses one person's authority.
#[test]
fn a_proxy_answers_the_later_stage_for_the_one_who_waits() {
    assert_eq!(put_off("last", "a_proxy_answers_the_later_stage"), ["1: deferred middle with decide (e3101)", "3: approved middle with decide (e1101 for e3101)"]);
    assert_eq!(
        put_off("last", "a_proxy_with_a_stage_of_their_own"),
        ["0: deferred first with second (e1101)", "0: deferred middle with second (e3101)", "2: approved first with second (e1101)", "2: approved middle with second (e1101 for e3101)"]
    );
    with(&machine_file("last"), |m, org| {
        // the first approval is the proxy's own, in their own seat; the stage is not done by it
        let r = ran(m, org, "a_proxy_with_a_stage_of_their_own");
        let t = r.rows[1].ticket().unwrap();
        let seats: Vec<(String, String)> = t.stages[2].parts.iter().flat_map(|p| p.answers.iter().map(|a| (a.by.clone(), a.seat.clone()))).collect();
        assert_eq!(seats, [("e1101".to_string(), "e1101".to_string())]);
        assert_eq!(phases(t), "first deferred, middle deferred, second open, decide ahead");
        let t = r.rows[2].ticket().unwrap();
        let seats: Vec<(String, String)> = t.stages[2].parts.iter().flat_map(|p| p.answers.iter().map(|a| (a.by.clone(), a.seat.clone()))).collect();
        assert_eq!(seats, [("e1101".to_string(), "e1101".to_string()), ("e1101".to_string(), "e3101".to_string())]);
        assert_eq!(phases(t), "first once, middle once, second answered, decide open");
    });
}

// ── Late with nowhere to go up, the top of the organization, a requester who left (PLAN C3.5) ──

/// The notes of the history a row of a scenario gives, each with what was done.
fn noted(file: &str, name: &str, row: usize) -> Vec<(String, String)> {
    with(&machine_file(file), |m, org| recorded(&ran(m, org, name).rows[row].stepped).into_iter().filter_map(|(did, note)| Some((did, note?))).collect())
}

fn pair(did: &str, note: &str) -> (String, String) {
    (did.to_string(), note.to_string())
}

/// A late stage goes up from the one it waits for, as many times as the request writes, and then
/// does what is written after `then` (`late`: `twice` goes up twice and expires, `waits` goes up
/// once and waits on with no clock).
#[test]
fn a_late_stage_goes_up_and_then_does_what_is_written_after() {
    let up = rows("late", "up_twice");
    // the manager's stage goes up to the head of the department, and from them to the head of the division
    assert_eq!(up[2], "late: in progress at twice -> in progress at twice | record(went up) notify(went up: e3101) set(late)");
    assert_eq!(up[3], "late: in progress at twice -> in progress at twice | record(went up) notify(went up: e0101) set(late)");
    assert_eq!(up[4], "late: in progress at twice -> expired | record(ticket expired) notify(decided: e1201)");
    assert_eq!(noted("late", "up_twice", 4), [pair("ticket expired", "late")]);
    // `then stay`: the stage is late in the history, waits on, and no clock is set again
    let waits = rows("late", "up_once_and_waits");
    assert_eq!(waits[1], "late: in progress at waits -> in progress at waits | record(went up) notify(went up: e3101) set(late)");
    assert_eq!(waits[2], "late: in progress at waits -> in progress at waits | record(late)");
    assert_eq!(noted("late", "up_once_and_waits", 2), [pair("late", "it waits on")]);
    with(&machine_file("late"), |m, org| {
        let t = ticket_after(m, org, "up_once_and_waits", 2);
        assert!(t.timers().is_empty(), "{:?}", t.timers());
        // no clock is left to ring, and the one it went up to answers whenever they do
        let later = at(m, "2026-12-10", "10:00");
        assert_eq!(give(m, org, &t, Event::Late, later).0, "late: in progress at waits refused (clock)");
        assert_eq!(give(m, org, &t, approve("e1101"), later).0, "approve by e1101: in progress at waits refused (person)");
        assert_eq!(give(m, org, &t, approve("e3101"), later).0, "approve by e3101: in progress at waits -> in progress at twice | record(approved) notify(waiting: e1101) set(late)");
    });
}

/// A late stage with nobody above the one it waits for (the president; PLAN C3.5): it does what is
/// written after `then` at that deadline, and the history says there was nowhere to go up. The
/// second time it is late it goes up from the one it went up to the first time, and so does a
/// stage that went up for lack of an approver.
#[test]
fn a_late_stage_with_nowhere_to_go_up() {
    // `then stay`, at the president
    assert_eq!(rows("late", "nowhere_to_go_up")[1], "late: in progress at waits -> in progress at waits | record(late)");
    assert_eq!(noted("late", "nowhere_to_go_up", 1), [pair("late", "nowhere to go up; it waits on")]);
    // `then expire`, at the president
    assert_eq!(rows("late", "nowhere_to_go_up")[3], "late: in progress at twice -> expired | record(ticket expired) notify(decided: e0101)");
    assert_eq!(noted("late", "nowhere_to_go_up", 3), [pair("ticket expired", "late; nowhere to go up")]);
    // the second time: up to the president the first time, and nowhere from there
    let second = rows("late", "nowhere_the_second_time");
    assert_eq!(second[2], "late: in progress at twice -> in progress at twice | record(went up) notify(went up: e0001) set(late)");
    assert_eq!(second[3], "late: in progress at twice -> expired | record(ticket expired) notify(decided: e3101)");
    assert_eq!(noted("late", "nowhere_the_second_time", 3), [pair("ticket expired", "late; nowhere to go up")]);
    // the one a stage went up to for lack of an approver is late: it goes up from them
    let went_up = rows("late", "late_where_it_went_up_to");
    assert_eq!(went_up[3], "approve by e0101: in progress at timed -> in progress at alone | record(approved) record(went up) cancel(late) notify(waiting: e0101) set(late)");
    assert_eq!(noted("late", "late_where_it_went_up_to", 3), [pair("went up", "it comes to the requester alone")]);
    assert_eq!(went_up[4], "late: in progress at alone -> in progress at alone | record(went up) notify(went up: e0001) set(late)");
    // the desk request, whose stages remind: the president's stage expires at its deadline
    assert_eq!(rows("desk", "late_at_the_top")[2], "late: in progress at first -> expired | record(ticket expired) notify(decided: e0101)");
    assert_eq!(noted("desk", "late_at_the_top", 2), [pair("ticket expired", "late; nowhere to go up")]);
}

/// The stages of the president's ticket, with nobody above (`top`; DESIGN 5.4): `then skip` skips
/// the stage, `then board` turns it to the board, and a stage with nothing after `then`, one
/// whose way after `then` has nobody either, and `when empty stay` stop until an administrator
/// adds an approver. Each is a line of the history with why. A member of the staff's ticket
/// passes the same stages with their manager answering.
#[test]
fn the_stages_at_the_top_of_the_organization() {
    let top = rows("top", "the_president_asks");
    assert_eq!(top[0], "submit by e0001: (none) -> in progress at turned | record(submitted) record(skipped) record(went to) notify(waiting: e0201, e0202, e0203, e0204) set(late)");
    assert_eq!(noted("top", "the_president_asks", 0), [pair("skipped", "no head is above the requester; nobody is above to go up to"), pair("went to", "no head is above the requester; nobody is above to go up to: board")]);
    // a stage that stops: its clock runs, the administrators are told, and nobody is asked
    assert_eq!(top[1], "approve by e0201: in progress at turned -> in progress at stopped | record(approved) record(stopped) cancel(late) notify(stopped: ) set(late)");
    assert_eq!(noted("top", "the_president_asks", 1), [pair("stopped", "no head is above the requester; nobody is above to go up to")]);
    assert_eq!(noted("top", "the_president_asks", 3), [pair("stopped", "no head is above the requester; nobody is above to go up to, and dept_head has nobody: no team of the kind department is above the requester")]);
    assert_eq!(noted("top", "the_president_asks", 5), [pair("stopped", "no head is above the requester")]);
    assert_eq!(top[2], "add approver: in progress at stopped -> in progress at stopped | record(approver added) notify(waiting: e0101)");
    assert_eq!(top[8], "approve by e0202: in progress at last -> approved | record(approved) record(ticket approved) cancel(late) notify(decided: e0001)");
    with(&machine_file("top"), |m, org| {
        // while it is stopped nobody answers it, the requester least of all
        let stopped = ticket_after(m, org, "the_president_asks", 1);
        let now = at(m, "2026-11-16", "11:30");
        for by in ["e0001", "e0101", "e0201"] {
            assert_eq!(give(m, org, &stopped, approve(by), now).0, format!("approve by {by}: in progress at stopped refused (person)"));
        }
        assert_eq!(give(m, org, &stopped, Event::AddApprover { by: None, approver: "e0001".into(), part: None }, now).0, "add approver: in progress at stopped refused (person)");
        // the one added answers it
        let added = ticket_after(m, org, "the_president_asks", 2);
        assert_eq!(added.stage().unwrap().parts[0].approvers, ["e0101"]);
    });
    // nobody is added: the stage's deadline comes all the same
    assert_eq!(rows("top", "nobody_is_added")[2], "late: in progress at stopped -> expired | record(ticket expired) notify(decided: e0001)");
    assert_eq!(noted("top", "nobody_is_added", 2), [pair("ticket expired", "late; nowhere to go up")]);
    // the board the stage turned to is late: nobody is above the requester to go up to
    assert_eq!(rows("top", "the_board_is_late")[1], "late: in progress at turned -> expired | record(ticket expired) notify(decided: e0001)");
    assert_eq!(noted("top", "the_board_is_late", 1), [pair("ticket expired", "late; nowhere to go up")]);
    // a member of the staff: no stage is skipped, turned or stopped
    with(&machine_file("top"), |m, org| {
        let r = ran(m, org, "a_member_of_the_staff_asks");
        let did: Vec<String> = r.rows.iter().flat_map(|row| recorded(&row.stepped)).map(|(did, _)| did).collect();
        assert!(did.iter().all(|d| ["submitted", "approved", "ticket approved"].contains(&d.as_str())), "{did:?}");
        assert_eq!(r.ticket().unwrap().status.word(), "approved");
    });
}

/// A requester who is no longer in the organization's version in effect: a stage decided from
/// them has nobody, and stops for an administrator instead of being skipped (nobody would have
/// answered the ticket); and what is the requester's to do is refused to one who is not there.
#[test]
fn a_requester_who_has_left_the_organization() {
    let left = rows("desk", "the_requester_has_left");
    assert_eq!(left[1], "approve by e0102: in progress at first -> in progress at decide | record(approved) record(stopped) cancel(remind) cancel(late) notify(stopped: ) set(remind) set(late)");
    assert_eq!(noted("desk", "the_requester_has_left", 1), [pair("stopped", "no head is above the requester; the requester e9001 is not in the organization from 2027-04-01")]);
    assert_eq!(left[2], "add approver: in progress at decide -> in progress at decide | record(approver added) notify(waiting: e0102)");
    assert_eq!(left[3], "approve by e0102: in progress at decide -> approved | record(approved) record(ticket approved) cancel(remind) cancel(late) notify(decided: e9001)");
    with(&machine_file("desk"), |m, org| {
        // made on the last day of the version they are in
        let made = ticket_after(m, org, "the_requester_has_left", 0);
        let now = at(m, "2027-04-01", "09:00");
        let s = scenario::step(m, org, Some(&made), Input::Event(Event::Withdraw { by: "e9001".into() }), now).unwrap();
        assert_eq!(s.answer.as_ref().err().map(|r| (r.kind, r.text.en.as_str())), Some(("person", "e9001 is not in the organization from 2027-04-01")));
        // sent back to them: they neither make it again nor discard it, and the clock closes it
        let (line, back) = give(m, org, &made, Event::SendBack { by: "e0102".into(), to: None }, now);
        assert_eq!(line, "send back by e0102: in progress at first -> with requester | record(sent back) cancel(remind) cancel(late) notify(returned: e9001) set(resubmit late)");
        let back = back.unwrap();
        let Input::Submit(sub) = ran(m, org, "the_requester_has_left").rows[0].stepped.input.clone() else { panic!() };
        for ev in [Event::Resubmit { by: "e9001".into(), form: sub.form.clone(), clear: Vec::new(), attempt: "attempt-2".into() }, Event::Discard { by: "e9001".into() }] {
            let s = scenario::step(m, org, Some(&back), Input::Event(ev), now + 3600).unwrap();
            assert_eq!(s.answer.as_ref().err().map(|r| (r.kind, r.text.en.as_str())), Some(("person", "e9001 is not in the organization from 2027-04-01")), "{}", went(&s));
        }
        let (_, _, rings) = back.timers().into_iter().find(|(w, ..)| *w == "resubmit late").unwrap();
        assert_eq!(give(m, org, &back, Event::ResubmitLate, rings).0, "resubmit late: with requester -> expired | record(ticket expired) notify(decided: e9001)");
    });
}

// ── Proxies and the administrator ──

/// Who answered and for whom, from the lines of the history a scenario gives: `(by, proxy of)`.
fn answered(m: &Machine, org: &Org, name: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for row in &ran(m, org, name).rows {
        let Ok((_, fx)) = &row.stepped.answer else { continue };
        for e in fx {
            if let What::Record(r) = &e.what
                && r.did == "approved"
            {
                out.push((r.by.clone().unwrap(), r.proxy_of.clone()));
            }
        }
    }
    out
}

/// A proxy answers for the one they stand in for, in the days of their row of `proxies.csv` and
/// for the requests it names: the line of the history holds both. One answer uses one person's
/// authority: a proxy who is an approver of the stage too answers once as themselves and once for
/// the other, and the one stood for answers as well while the proxy has not.
#[test]
fn a_proxy_answers_in_the_place_of_one_approver() {
    with(&machine_file("once"), |m, org| {
        let some = |s: &str| Some(s.to_string());
        // e1101 stands in for e3101 from 2026-12-28 to 2027-01-08
        assert_eq!(answered(m, org, "a_proxy_answers_for_one_and_then_for_themselves"), [("e1101".to_string(), some("e3101")), ("e1101".to_string(), None)]);
        assert_eq!(answered(m, org, "a_proxy_with_a_part_of_their_own"), [("e1101".to_string(), None), ("e1101".to_string(), some("e3101"))]);
        let first = ticket_after(m, org, "a_proxy_answers_for_one_and_then_for_themselves", 0);
        // the one stood for answers too
        assert!(give(m, org, &first, approve("e3101"), at(m, "2026-12-28", "11:00")).0.starts_with("approve by e3101: in progress at first -> in progress at second |"));
        // the day before the row begins, the proxy is nobody to the stage
        let s = scenario::step(m, org, Some(&first), Input::Event(approve("e1101")), at(m, "2026-12-25", "11:00")).unwrap();
        assert_eq!(s.answer.as_ref().err().map(|r| (r.kind, r.text.en.as_str())), Some(("person", "e1101 (Manager of Section 1A) is not an approver of the stage `first`, nor a proxy of one")));
        // with their own answer given, the next answer of the proxy is the other's: and the other's
        // own answer closes the stage the same way
        let own = ticket_after(m, org, "a_proxy_with_a_part_of_their_own", 1);
        let now = at(m, "2026-12-28", "11:30");
        assert!(give(m, org, &own, approve("e3101"), now).0.starts_with("approve by e3101: in progress at first -> in progress at decide |"));
        assert!(give(m, org, &own, approve("e1101"), now).0.starts_with("approve by e1101: in progress at first -> in progress at decide |"));
    });
    // the desk request is not one the proxy's row names (`expense purchase`): no proxy there
    with(&machine_file("desk"), |m, org| {
        let Input::Submit(sub) = ran(m, org, "approved_by_a_quorum").rows[0].stepped.input.clone() else { panic!() };
        let now = at(m, "2026-12-28", "10:00");
        let s = scenario::step(m, org, None, Input::Submit(Submit { requester: "e1101".into(), by: None, ..sub }), now).unwrap();
        let (t, _) = s.answer.unwrap();
        assert_eq!(t.stage().unwrap().parts[0].approvers, ["e3101"]);
        assert_eq!(give(m, org, &t, approve("e1101"), now + 3600).0, "approve by e1101: in progress at first refused (person)");
    });
}

/// What an administrator does to a stage (DESIGN 7.2): `add approver` adds one more who may
/// answer, `hand over` puts one in the place of an approver who has yet to answer. Neither takes
/// the requester, a person outside the organization, or one who is an approver already.
#[test]
fn the_administrator_adds_an_approver_and_hands_a_stage_over() {
    with(&machine_file("desk"), |m, org| {
        let t = ticket_after(m, org, "approved_by_a_quorum", 0);
        let now = at(m, "2026-11-16", "10:30");
        let refused = |ev: Event| {
            let s = scenario::step(m, org, Some(&t), Input::Event(ev), now).unwrap();
            s.answer.err().map(|r| (r.kind, r.text.en)).unwrap()
        };
        let add = |who: &str| Event::AddApprover { by: None, approver: who.to_string(), part: None };
        let hand = |from: &str, to: &str| Event::HandOver { by: None, from: from.to_string(), to: to.to_string() };
        assert_eq!(refused(add("e1101")), ("event", "e1101 is an approver of the stage `first` already".to_string()));
        assert_eq!(refused(add("e1201")), ("person", "e1201 (Sales staff 1) made the request, and does not answer it (`self approval not allowed`)".to_string()));
        assert_eq!(refused(add("e1202")), ("person", "e1202 is not in the organization from 2026-04-01".to_string()));
        assert_eq!(refused(Event::AddApprover { by: None, approver: "e1203".into(), part: Some("board".into()) }), ("event", "the stage `first` has no part that waits for an approver".to_string()));
        assert_eq!(refused(hand("e1203", "e1002")), ("event", "e1203 is no approver of the stage `first` who has yet to answer (or e1002 is one already)".to_string()));
        assert_eq!(refused(hand("e1101", "e1101")), ("event", "e1101 is no approver of the stage `first` who has yet to answer (or e1101 is one already)".to_string()));
        assert_eq!(refused(hand("e1101", "e1201")), ("person", "e1201 (Sales staff 1) made the request, and does not answer it (`self approval not allowed`)".to_string()));
        assert_eq!(refused(hand("e1101", "e1202")), ("person", "e1202 is not in the organization from 2026-04-01".to_string()));
        // one more approver: either of the two answers the stage
        let (line, added) = give(m, org, &t, add("e1203"), now);
        assert_eq!(line, "add approver: in progress at first -> in progress at first | record(approver added) notify(waiting: e1203)");
        let added = added.unwrap();
        assert_eq!(added.stage().unwrap().parts[0].approvers, ["e1101", "e1203"]);
        for by in ["e1101", "e1203"] {
            assert!(give(m, org, &added, approve(by), now + 60).0.starts_with(&format!("approve by {by}: in progress at first -> in progress at board |")), "{by}");
        }
        // handed over: the one it is handed to answers, and the one it is taken from no longer does
        let (line, handed) = give(m, org, &t, hand("e1101", "e1203"), now);
        assert_eq!(line, "hand over: in progress at first -> in progress at first | record(handed over) notify(waiting: e1203)");
        let handed = handed.unwrap();
        assert_eq!(give(m, org, &handed, approve("e1101"), now + 60).0, "approve by e1101: in progress at first refused (person)");
        assert!(give(m, org, &handed, approve("e1203"), now + 60).0.starts_with("approve by e1203: in progress at first -> in progress at board |"));
        // the clocks of the stage are as they were: neither starts it again
        assert_eq!((added.timers(), handed.timers()), (t.timers(), t.timers()));
    });
}

// ── The revision an event is made on (DESIGN 7.1, 7.2) ──

/// The event `ev` given to `t` by one who saw the ticket at the revision `seen` (`None`: an event
/// that says no revision): the machine's answer on one line, and the ticket it comes to.
fn give_seen(m: &Machine, org: &Org, t: &Ticket, ev: Event, seen: Option<Revision>, now: i64) -> (String, Option<Ticket>) {
    let s = scenario::step_seen(m, org, Some(t), Input::Event(ev), None, seen, now).unwrap_or_else(|e| panic!("{}", e.en));
    (went(&s), s.answer.ok().map(|(t, _)| t))
}

/// The refusal of the event `ev` given to `t` by one who saw the ticket at `seen`: its kind and
/// its text.
fn refused_seen(m: &Machine, org: &Org, t: &Ticket, ev: Event, seen: Option<Revision>, now: i64) -> (&'static str, String) {
    let s = scenario::step_seen(m, org, Some(t), Input::Event(ev), None, seen, now).unwrap_or_else(|e| panic!("{}", e.en));
    let line = went(&s);
    s.answer.err().map(|r| (r.kind, r.text.en)).unwrap_or_else(|| panic!("taken: {line}"))
}

/// A desk of `price` yen asked for by `requester` on 2026-11-16 at 10:00.
fn a_desk(m: &Machine, org: &Org, requester: &str, price: i128) -> Ticket {
    let form: Values = vec![("what".to_string(), Value::Str("A standing desk".into())), ("price".to_string(), Value::Int(price))];
    let sub = Submit { id: "T-1".into(), requester: requester.into(), by: None, form, attempt: "attempt-1".into() };
    let s = scenario::step(m, org, None, Input::Submit(sub), at(m, "2026-11-16", "10:00")).unwrap();
    s.answer.unwrap_or_else(|r| panic!("{}", r.text.en)).0
}

/// What is asked of an event, in order (DESIGN 7.3): a clock that has rung, then the revision of
/// the ticket it was made on, then who gives it and the state of the ticket. One who saw the ticket
/// as it was is told to see it again, whoever they are and whatever the event; an event that says
/// no revision is refused alike, and a clock's event carries none. Another's answer at the stage,
/// going up, an approver added and a place handed over leave the revision as it is.
#[test]
fn the_revision_is_asked_after_the_clocks_and_before_who_and_the_state() {
    with(&machine_file("desk"), |m, org| {
        let t = a_desk(m, org, "e1201", 180_000);
        let shown = Some(t.revision());
        assert_eq!(t.revision(), Revision { round: 1, stage: Some(0), edits: 0 });
        let (_, board) = give(m, org, &t, approve("e1101"), at(m, "2026-11-16", "11:00"));
        let board = board.unwrap();
        assert_eq!(board.revision(), Revision { round: 1, stage: Some(1), edits: 0 });
        let now = at(m, "2026-11-16", "12:00");
        // e1101 is no approver of the board, and a ticket in progress takes no resubmit: the
        // revision is asked first
        assert_eq!(
            refused_seen(m, org, &board, approve("e1101"), shown, now),
            ("stale", "`approve` was made on the ticket at round 1, stage 1 of the ticket, 0 edits, and it is at round 1, stage 2 of the ticket, 0 edits now: the ticket is to be shown again".to_string())
        );
        assert_eq!(give_seen(m, org, &board, Event::Resubmit { by: "e1201".into(), form: Values::new(), clear: Vec::new(), attempt: "attempt-2".into() }, shown, now).0, "resubmit by e1201: in progress at board refused (stale)");
        // on the revision as it is, who gives it is asked, and the state
        assert_eq!(give(m, org, &board, approve("e1101"), now).0, "approve by e1101: in progress at board refused (person)");
        assert_eq!(give(m, org, &board, Event::Resubmit { by: "e1201".into(), form: Values::new(), clear: Vec::new(), attempt: "attempt-2".into() }, now).0, "resubmit by e1201: in progress at board refused (state)");
        // an event that says no revision
        assert_eq!(refused_seen(m, org, &board, approve("e0201"), None, now), ("stale", "`approve` does not say which revision of the ticket it was made on".to_string()));
        // a clock that has rung comes before it; a clock's event carries none
        let due = board.stage().unwrap().due.unwrap();
        assert_eq!(give_seen(m, org, &board, approve("e0201"), shown, due).0, "approve by e0201: in progress at board refused (clock)");
        let (line, up) = give(m, org, &board, Event::Late, due);
        assert!(line.starts_with("late: in progress at board -> in progress at board |"), "{line}");
        // another's answer, going up, an approver added and a place handed over
        let (_, one) = give(m, org, &board, approve("e0203"), now);
        let add = Event::AddApprover { by: None, approver: "e1203".into(), part: None };
        let hand = Event::HandOver { by: None, from: "e1101".into(), to: "e1203".into() };
        let (_, added) = give(m, org, &t, add, now);
        let (_, handed) = give(m, org, &t, hand, now);
        assert_eq!([one, up, added, handed].map(|x| x.unwrap().revision()), [board.revision(), board.revision(), t.revision(), t.revision()]);
        // an ended ticket: the revision first, then the state
        let (_, ended) = give(m, org, &t, Event::Reject { by: "e1101".into() }, now);
        let ended = ended.unwrap();
        assert_eq!(ended.revision(), Revision { round: 1, stage: None, edits: 0 });
        assert_eq!(give_seen(m, org, &ended, approve("e1101"), shown, now + 60).0, "approve by e1101: rejected refused (stale)");
        assert_eq!(give(m, org, &ended, approve("e1101"), now + 60).0, "approve by e1101: rejected refused (state)");
    });
}

/// An approver's edit makes another revision (DESIGN 7.1): an approval made on the ticket as it
/// was before the edit is refused (`stale`), not counted for a price its approver has not seen, and
/// the approval made on the ticket as it is now is taken. What was answered before the edit stays:
/// the stage counts the approval given before it with the one given after.
#[test]
fn an_approval_made_before_an_approvers_edit_is_refused() {
    with(&machine_file("desk"), |m, org| {
        let t = a_desk(m, org, "e1201", 180_000);
        let now = at(m, "2026-11-16", "11:00");
        let (_, board) = give(m, org, &t, approve("e1101"), now);
        let (line, one) = give(m, org, &board.unwrap(), approve("e0203"), now + 600);
        assert!(line.starts_with("approve by e0203: in progress at board -> in progress at board |"), "{line}");
        let one = one.unwrap();
        let shown = one.revision();
        let (line, edited) = give(m, org, &one, Event::Edit { by: "e0201".into(), fields: vec![("price".to_string(), Value::Int(150_000))], clear: Vec::new() }, now + 1200);
        assert_eq!(line, "edit by e0201: in progress at board -> in progress at board | record(edited)");
        let edited = edited.unwrap();
        assert_eq!(edited.revision(), Revision { edits: 1, ..shown });
        assert_eq!(edited.stage().unwrap().parts[0].answers.iter().map(|a| a.by.as_str()).collect::<Vec<_>>(), ["e0203"]);
        // e0202 approves from a screen shown before the edit, and from the ticket as it is now
        assert_eq!(
            refused_seen(m, org, &edited, approve("e0202"), Some(shown), now + 1800),
            ("stale", "`approve` was made on the ticket at round 1, stage 2 of the ticket, 0 edits, and it is at round 1, stage 2 of the ticket, 1 edit now: the ticket is to be shown again".to_string())
        );
        let (line, decide) = give(m, org, &edited, approve("e0202"), now + 1800);
        assert!(line.starts_with("approve by e0202: in progress at board -> in progress at decide |"), "{line}");
        assert_eq!(decide.unwrap().revision(), Revision { round: 1, stage: Some(2), edits: 1 });
    });
}

/// A ticket made again is of another round (DESIGN 7.1): an approval made on the ticket of the
/// round before is refused (`stale`), not counted for what the requester made again.
#[test]
fn an_approval_made_before_the_ticket_is_made_again_is_refused() {
    with(&machine_file("desk"), |m, org| {
        let t = a_desk(m, org, "e1201", 180_000);
        let (_, back) = give(m, org, &t, Event::SendBack { by: "e1101".into(), to: None }, at(m, "2026-11-16", "11:00"));
        let back = back.unwrap();
        assert_eq!(back.revision(), Revision { round: 1, stage: None, edits: 0 });
        let (line, again) = give(m, org, &back, Event::Resubmit { by: "e1201".into(), form: vec![("price".to_string(), Value::Int(90_000))], clear: Vec::new(), attempt: "attempt-2".into() }, at(m, "2026-11-17", "10:00"));
        assert!(line.starts_with("resubmit by e1201: with requester -> in progress at first |"), "{line}");
        let again = again.unwrap();
        assert_eq!(again.revision(), Revision { round: 2, stage: Some(0), edits: 0 });
        let now = at(m, "2026-11-17", "11:00");
        assert_eq!(give_seen(m, org, &again, approve("e1101"), Some(t.revision()), now).0, "approve by e1101: in progress at first refused (stale)");
        assert!(give(m, org, &again, approve("e1101"), now).0.starts_with("approve by e1101: in progress at first -> in progress at decide |"));
    });
}

/// One person approves two stages one after the other (DESIGN 7.1): the manager of e1002 is the
/// head of the department, e1001, who answers the first stage and the one that decides. The
/// approval made twice on one revision (from two tabs) approves the first stage, and the second is
/// refused (`stale`), not taken as the approval of the stage after it.
#[test]
fn an_approval_made_twice_on_one_revision_is_refused_the_second_time() {
    with(&machine_file("desk"), |m, org| {
        let t = a_desk(m, org, "e1002", 90_000);
        assert_eq!(t.stage().unwrap().parts[0].approvers, ["e1001"]);
        let shown = Some(t.revision());
        let now = at(m, "2026-11-16", "11:00");
        let (line, decide) = give_seen(m, org, &t, approve("e1001"), shown, now);
        assert!(line.starts_with("approve by e1001: in progress at first -> in progress at decide |"), "{line}");
        let decide = decide.unwrap();
        assert_eq!(decide.stage().unwrap().parts[0].approvers, ["e1001"]);
        assert_eq!(give_seen(m, org, &decide, approve("e1001"), shown, now + 60).0, "approve by e1001: in progress at decide refused (stale)");
        assert!(give(m, org, &decide, approve("e1001"), now + 60).0.starts_with("approve by e1001: in progress at decide -> approved |"));
    });
}

/// A flow's answer is given with the revision of the ticket the transition that asked the flow
/// answered with (DESIGN 7.1, 7.2). `flows`, which here may be sent back, made again and edited:
/// made again, it asks its agent again, and the agent's answer to the round before is refused
/// (`stale`) while its answer to this round is taken. An approver's edit asks again an agent that
/// has yet to answer, and the agent's answer to the form as it was is refused; an answer the agent
/// gave before the edit stays, and the agent is not asked again.
#[test]
fn an_agents_answer_to_the_ticket_as_it_was_is_refused() {
    let file = machine_file("flows");
    let src = std::fs::read_to_string(&file).unwrap();
    let text = src.replace("  pause          at most 5 business days\n", "  pause          at most 5 business days\n  send back      to the requester, at most 2 times\n  resubmit       within 3 business days\n  approvers edit what\n");
    assert!(text.contains("  approvers edit what\n"));
    let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
    let o = run.text(&file, &text);
    assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
    ringi::engine::with_text(&o, &run, &text, |m, org| {
        let sub = Submit { id: "T-1".into(), requester: "e1201".into(), by: None, form: vec![("what".to_string(), Value::Str("A projector".into()))], attempt: "attempt-1".into() };
        let (t, fx) = scenario::step(m, org, None, Input::Submit(sub), at(m, "2026-11-16", "10:00")).unwrap().answer.unwrap();
        assert!(fx.iter().any(|e| short(e) == "ask agent(precheck_agent)"));
        let asked = Some(t.revision());
        let ok = || Event::AgentAnswered { ok: true, reason: String::new() };
        // sent back and made again: the agent is asked again
        let (_, back) = give(m, org, &t, Event::SendBack { by: "e3101".into(), to: None }, at(m, "2026-11-16", "11:00"));
        let s = scenario::step(m, org, back.as_ref(), Input::Event(Event::Resubmit { by: "e1201".into(), form: Values::new(), clear: Vec::new(), attempt: "attempt-2".into() }), at(m, "2026-11-17", "10:00")).unwrap();
        let (again, fx) = s.answer.unwrap();
        assert!(fx.iter().any(|e| short(e) == "ask agent(precheck_agent)"));
        let now = at(m, "2026-11-17", "10:05");
        assert_eq!(give_seen(m, org, &again, ok(), asked, now).0, "agent answered: in progress at first refused (stale)");
        assert_eq!(give_seen(m, org, &again, Event::FlowFailed, asked, now).0, "flow failed: in progress at first refused (stale)");
        assert!(give_seen(m, org, &again, ok(), Some(again.revision()), now).0.starts_with("agent answered: in progress at first -> in progress at first |"));
        // edited while the agent checks: it is asked again with the form as it is now
        let edit = || Event::Edit { by: "e3101".into(), fields: vec![("what".to_string(), Value::Str("A projector and a screen".into()))], clear: Vec::new() };
        let s = scenario::step(m, org, Some(&t), Input::Event(edit()), at(m, "2026-11-16", "10:30")).unwrap();
        assert_eq!(went(&s), "edit by e3101: in progress at first -> in progress at first | record(edited) record(agent asked) ask agent(precheck_agent)");
        let (edited, fx) = s.answer.unwrap();
        let args = fx.iter().find_map(|e| if let What::AskAgent { args, .. } = &e.what { Some(args.clone()) } else { None }).unwrap();
        assert!(args.contains(&("what".to_string(), Value::Str("A projector and a screen".into()))), "{args:?}");
        let now = at(m, "2026-11-16", "10:40");
        assert_eq!(give_seen(m, org, &edited, ok(), asked, now).0, "agent answered: in progress at first refused (stale)");
        assert!(give_seen(m, org, &edited, ok(), Some(edited.revision()), now).0.starts_with("agent answered: in progress at first -> in progress at first |"));
        // the agent answered before the edit: its answer stays, and it is not asked again
        let (_, checked) = give(m, org, &t, ok(), at(m, "2026-11-16", "10:10"));
        assert_eq!(give(m, org, &checked.unwrap(), edit(), at(m, "2026-11-16", "10:30")).0, "edit by e3101: in progress at first -> in progress at first | record(edited)");
    })
    .unwrap_or_else(|t| panic!("{}", t.en));
}

// ── Comments (DESIGN 7.2–7.4) ──

/// The lines of the history a transition gives, each with its comment.
fn commented(s: &Stepped) -> Vec<(String, Option<String>)> {
    let Ok((_, fx)) = &s.answer else { return Vec::new() };
    fx.iter()
        .filter_map(|e| match &e.what {
            What::Record(r) => Some((r.did.to_string(), r.comment.clone())),
            _ => None,
        })
        .collect()
}

/// The event `ev` given to `t` at `now` with what its person wrote, on the ticket as it is.
fn said(m: &Machine, org: &Org, t: &Ticket, ev: Event, comment: &str, now: i64) -> Stepped {
    scenario::step_with(m, org, Some(t), Input::Event(ev), Some(comment.to_string()), now).unwrap_or_else(|e| panic!("{}", e.en))
}

/// The refusal of a transition: its kind and its text.
fn refusal(s: &Stepped) -> (&'static str, String) {
    let line = went(s);
    s.answer.as_ref().err().map(|r| (r.kind, r.text.en.clone())).unwrap_or_else(|| panic!("taken: {line}"))
}

fn one(did: &str, comment: Option<&str>) -> (String, Option<String>) {
    (did.to_string(), comment.map(str::to_string))
}

/// What a person writes with what they do stays on the line of the history of what they did
/// (DESIGN 7.2, 7.4): the request made, an approval, a send-back and the request made again. The
/// lines the transition gives besides are as they are with no comment, and so is the ticket; an
/// empty comment is none.
#[test]
fn a_comment_stays_on_the_line_of_what_its_writer_did() {
    with(&machine_file("desk"), |m, org| {
        let form: Values = vec![("what".to_string(), Value::Str("A standing desk".into())), ("price".to_string(), Value::Int(180_000))];
        let sub = Submit { id: "T-1".into(), requester: "e1201".into(), by: None, form, attempt: "attempt-1".into() };
        let s = scenario::step_with(m, org, None, Input::Submit(sub), Some("For the new office".into()), at(m, "2026-11-16", "10:00")).unwrap();
        assert_eq!(commented(&s), [one("submitted", Some("For the new office"))]);
        let t = s.answer.unwrap().0;
        assert_eq!(t, a_desk(m, org, "e1201", 180_000));
        let now = at(m, "2026-11-16", "11:00");
        let s = said(m, org, &t, approve("e1101"), "Fine by me", now);
        let (plain, lines) = (give(m, org, &t, approve("e1101"), now), commented(&s));
        assert_eq!(lines[0], one("approved", Some("Fine by me")));
        assert!(lines[1..].iter().all(|(_, c)| c.is_none()), "{lines:?}");
        assert_eq!((went(&s), s.answer.unwrap().0), (plain.0, plain.1.unwrap()));
        let s = said(m, org, &t, Event::SendBack { by: "e1101".into(), to: None }, "Please add the quote", now);
        assert_eq!(commented(&s), [one("sent back", Some("Please add the quote"))]);
        let back = s.answer.unwrap().0;
        let s = said(m, org, &back, Event::Resubmit { by: "e1201".into(), form: Values::new(), clear: Vec::new(), attempt: "attempt-2".into() }, "The quote is added", at(m, "2026-11-17", "10:00"));
        assert_eq!(commented(&s)[0], one("resubmitted", Some("The quote is added")));
        assert_eq!(commented(&said(m, org, &t, approve("e1101"), "", now))[0], one("approved", None));
    });
}

/// `comment` moves nothing (DESIGN 7.2, 7.3): it adds its line to the history and tells the
/// requester and the approvers the stage waits for, past the one who wrote it; the state, the
/// clocks and the revision are as they were. It says no revision, so one written on the ticket as
/// it was before an approval is taken. A paused ticket takes one, and the approvers it waits for
/// are told; a ticket sent back to its requester tells the requester's side and the one who sent it
/// back; a ticket made in another's name tells the one who made it too.
#[test]
fn a_comment_moves_nothing() {
    with(&machine_file("desk"), |m, org| {
        let t = a_desk(m, org, "e1201", 180_000);
        let (_, board) = give(m, org, &t, approve("e1101"), at(m, "2026-11-16", "11:00"));
        let board = board.unwrap();
        let now = at(m, "2026-11-16", "12:00");
        let s = said(m, org, &board, Event::Comment { by: "e1201".into() }, "When is it decided?", now);
        assert_eq!(went(&s), "comment by e1201: in progress at board -> in progress at board | record(commented) notify(comment: e0001, e0201, e0202, e0203, e0204)");
        assert_eq!(commented(&s), [one("commented", Some("When is it decided?"))]);
        let (after, fx) = s.answer.unwrap();
        assert_eq!(after.revision(), board.revision());
        assert_eq!(after.serial, board.serial + fx.len() as u32);
        assert_eq!(Ticket { serial: board.serial, ..after.clone() }, board);
        // the machine reads no revision of it: one the ticket had before, or none, is all one
        let ev = Event::Comment { by: "e1201".into() };
        let part = m.part(org, &ringi::engine::Need::event(&board, &ev), now).unwrap();
        for seen in [Some(t.revision()), None] {
            let (again, more) = m.next(&board, &ev, Some("When is it decided?"), seen.as_ref(), &part, now).unwrap();
            assert_eq!((again, more.len()), (after.clone(), fx.len()));
        }
        // the line is of the stage, by its writer
        let s = said(m, org, &board, Event::Comment { by: "e0202".into() }, "Is the price with tax?", now);
        assert_eq!(went(&s), "comment by e0202: in progress at board -> in progress at board | record(commented) notify(comment: e1201, e0001, e0201, e0203, e0204)");
        let Ok((_, fx)) = &s.answer else { panic!() };
        let What::Record(r) = &fx[0].what else { panic!() };
        assert_eq!((r.stage.as_deref(), r.by.as_deref()), (Some("board"), Some("e0202")));
        // paused, and with the requester
        let paused = ticket_after(m, org, "paused_and_resumed", 1);
        let waits = paused.stage().unwrap().parts[0].approvers.join(", ");
        let s = said(m, org, &paused, Event::Comment { by: "e1201".into() }, "Any news?", at(m, "2026-11-18", "10:00"));
        assert_eq!(went(&s), format!("comment by e1201: paused at first -> paused at first | record(commented) notify(comment: {waits})"));
        let back = ticket_after(m, org, "sent_back_and_made_again", 1);
        let now = at(m, "2026-11-17", "09:00");
        assert_eq!(went(&said(m, org, &back, Event::Comment { by: "e1101".into() }, "The quote, please", now)), "comment by e1101: with requester -> with requester | record(commented) notify(comment: e1201)");
        assert_eq!(went(&said(m, org, &back, Event::Comment { by: "e1201".into() }, "Which quote?", now)), "comment by e1201: with requester -> with requester | record(commented) notify(comment: e1101)");
    });
    with(&machine_file("who"), |m, org| {
        let made = ran(m, org, "a_proxy_asks_for_someone").rows[0].ticket().cloned().unwrap();
        let waits = made.stage().unwrap().parts[0].approvers.clone();
        let s = said(m, org, &made, Event::Comment { by: waits[0].clone() }, "Who is it for?", at(m, "2026-12-01", "11:00"));
        assert!(went(&s).ends_with("| record(commented) notify(comment: e1203, e1201)"), "{}", went(&s));
    });
}

/// The one who sent the ticket back to the requester (DESIGN 7.1, 7.3): the ticket holds them while
/// it is with the requester, and a comment written then goes to them too, past its writer. A
/// send-back to a stage and a withdrawal by the requester hold nobody, and the ticket made again,
/// discarded or late to be made again holds nobody any more.
#[test]
fn the_one_who_sent_it_back() {
    with(&machine_file("desk"), |m, org| {
        let t = a_desk(m, org, "e1201", 180_000);
        assert_eq!(t.sent_back_by, None);
        let back = give(m, org, &t, Event::SendBack { by: "e1101".into(), to: None }, at(m, "2026-11-16", "11:00")).1.unwrap();
        assert_eq!((back.status.word(), back.sent_back_by.as_deref()), ("with requester", Some("e1101")));
        // the requester asks the one who sent it back, who answers the requester; a third person's
        // goes to both
        let now = at(m, "2026-11-16", "12:00");
        let note = |by: &str, text: &str| went(&said(m, org, &back, Event::Comment { by: by.into() }, text, now));
        assert_eq!(note("e1201", "Which quote?"), "comment by e1201: with requester -> with requester | record(commented) notify(comment: e1101)");
        assert_eq!(note("e1101", "The one from the shop"), "comment by e1101: with requester -> with requester | record(commented) notify(comment: e1201)");
        assert_eq!(note("e9001", "From legal"), "comment by e9001: with requester -> with requester | record(commented) notify(comment: e1201, e1101)");
        // made again, discarded, late to be made again: nobody is held
        let after = |ev: Event, when: i64| give(m, org, &back, ev, when).1.unwrap();
        let again = after(Event::Resubmit { by: "e1201".into(), form: Values::new(), clear: Vec::new(), attempt: "attempt-2".into() }, at(m, "2026-11-17", "10:00"));
        assert_eq!((again.status.word(), again.sent_back_by), ("in progress", None));
        let discarded = after(Event::Discard { by: "e1201".into() }, now);
        assert_eq!((discarded.status.word(), discarded.sent_back_by), ("withdrawn", None));
        let late = after(Event::ResubmitLate, back.resubmit_due.unwrap());
        assert_eq!((late.status.word(), late.sent_back_by), ("expired", None));
        // sent back to a stage: in progress, with nobody held
        let to_stage = ticket_after(m, org, "sent_back_to_a_stage", 2);
        assert_eq!((to_stage.status.word(), to_stage.sent_back_by), ("in progress", None));
        // taken back by the requester: with them, sent back by nobody; their comment goes to nobody
        let withdrawn = ticket_after(m, org, "withdrawn_and_discarded", 1);
        assert_eq!((withdrawn.status.word(), withdrawn.sent_back_by.as_deref()), ("with requester", None));
        let s = said(m, org, &withdrawn, Event::Comment { by: "e1201".into() }, "I will correct it", now);
        assert_eq!(went(&s), "comment by e1201: with requester -> with requester | record(commented)");
    });
    with(&machine_file("who"), |m, org| {
        // made in another's name and sent back: the requester's comment goes to the one who made it
        // and to the one who sent it back
        let back = ticket_after(m, org, "a_proxy_asks_for_someone", 1);
        assert_eq!((back.made_by.as_deref(), back.sent_back_by.as_deref()), (Some("e1201"), Some("e1101")));
        let s = said(m, org, &back, Event::Comment { by: "e1203".into() }, "Which part, please?", at(m, "2026-12-01", "12:00"));
        assert!(went(&s).ends_with("| record(commented) notify(comment: e1201, e1101)"), "{}", went(&s));
    });
}

/// What a comment is refused for, in order (DESIGN 7.3): the day the part of the organization is
/// of, a clock that has rung, a ticket that has ended, then an empty one and one past 2,000
/// characters (`comment`, which names no place of the form). A person's event with a comment past
/// it is refused alike, with the ticket as it was; 2,000 characters are taken, counted as
/// characters and not as bytes.
#[test]
fn what_a_comment_is_refused_for() {
    with(&machine_file("desk"), |m, org| {
        let t = a_desk(m, org, "e1201", 180_000);
        let now = at(m, "2026-11-16", "12:00");
        let note = |by: &str| Event::Comment { by: by.into() };
        let most = "𠮷".repeat(ringi::engine::COMMENT_MOST);
        let past = "𠮷".repeat(ringi::engine::COMMENT_MOST + 1);
        assert_eq!(commented(&said(m, org, &t, note("e1101"), &most, now)), [one("commented", Some(&most))]);
        let long = ("comment", "the comment is 2001 characters long, past the limit (2000)".to_string());
        assert_eq!(refusal(&said(m, org, &t, note("e1101"), &past, now)), long);
        assert_eq!(refusal(&said(m, org, &t, approve("e1101"), &past, now)), long);
        assert_eq!(said(m, org, &t, note("e1101"), &past, now).answer.unwrap_err().text.ja, "コメントは 2,001 文字で、上限の 2,000 を超えています");
        assert_eq!(refusal(&said(m, org, &t, note("e1101"), "", now)), ("comment", "the text of `comment` is empty".to_string()));
        assert_eq!(said(m, org, &t, approve("e1101"), &past, now).answer.unwrap_err().at, None);
        // before them: the part of the organization of another day than the comment's, a clock that
        // has rung, a ticket that has ended
        let part = m.part(org, &ringi::engine::Need::event(&t, &note("e1101")), now).unwrap();
        assert_eq!(m.next(&t, &note("e1101"), Some(past.as_str()), None, &part, at(m, "2026-11-17", "12:00")).unwrap_err().kind, "data");
        let due = t.stage().unwrap().due.unwrap();
        assert_eq!(refusal(&said(m, org, &t, note("e1101"), &past, due)).0, "clock");
        let ended = ticket_after(m, org, "approved_by_a_quorum", 4);
        assert_eq!(refusal(&said(m, org, &ended, note("e1201"), "", now)), ("state", "the ticket has ended (`approved`), and takes no `comment`".to_string()));
        // who writes it is the gate's to say: the machine takes one from anyone in the organization
        assert!(said(m, org, &t, note("e9001"), "From legal", now).answer.is_ok());
    });
}

/// An agent's reason is the comment of its line (DESIGN 7.4): the note names the flow alone.
#[test]
fn an_agents_reason_is_the_comment_of_its_line() {
    with(&machine_file("flows"), |m, org| {
        let t = ticket_after(m, org, "the_agent_and_a_person", 0);
        let now = at(m, "2026-11-16", "10:10");
        let s = scenario::step(m, org, Some(&t), Input::Event(Event::AgentAnswered { ok: true, reason: "Within the limit".into() }), now).unwrap();
        assert_eq!(recorded(&s), [("agent ok".to_string(), Some("precheck_agent".to_string()))]);
        assert_eq!(commented(&s), [one("agent ok", Some("Within the limit"))]);
        let s = scenario::step(m, org, Some(&t), Input::Event(Event::AgentAnswered { ok: true, reason: String::new() }), now).unwrap();
        assert_eq!(commented(&s), [one("agent ok", None)]);
    });
}

/// A scenario's table writes what a person wrote in a `comment` column after `by` (DESIGN 3.2,
/// 10.2): the row's event is given with it, and `ringi run` shows it on the row. A clock's row
/// leaves the cell empty, and a comment the machine refuses stops the scenario at its row (E603).
#[test]
fn a_scenario_writes_what_a_person_wrote_in_its_own_column() {
    let file = machine_file("desk");
    let src = std::fs::read_to_string(&file).unwrap();
    let ran_with = |rows: &str| {
        let text = format!("{src}\nscenario with_comments\n  requester e1201\n  form what = \"A standing desk\", price = 180_000JPY\n| at | event | by | comment | -> stage | status |\n| 2026-11-16 10:00 | submit | e1201 | \"For the new office\" | first | in progress |\n{rows}");
        let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
        let o = run.text(&file, &text);
        assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
        ringi::engine::with_text(&o, &run, &text, |m, org| {
            let sc = m.s.file.scenarios.iter().find(|s| s.name.text == "with_comments").unwrap();
            let r = scenario::run(m, org, sc);
            let given: Vec<Option<String>> = r.rows.iter().map(|row| row.stepped.comment.clone()).collect();
            (scenario::render(m, &r, Lang::En), given, r.stop.map(|s| (s.code, s.text.en.clone())))
        })
        .unwrap()
    };
    let (shown, given, stop) = ran_with("| 2026-11-16 11:00 | comment | e1101 | \"Is the price with tax?\" | first | in progress |\n| 2026-11-16 11:30 | send back | e1101 | \"Please add the quote\" | | with requester |\n| 2026-11-17 10:00 | resubmit | e1201 | | first | in progress |\n");
    assert!(stop.is_none(), "{shown}");
    assert_eq!(given, [Some("For the new office".to_string()), Some("Is the price with tax?".to_string()), Some("Please add the quote".to_string()), None]);
    assert!(shown.contains("2026-11-16 11:00  comment    by e1101  -> first (e1101), due 2026-11-19 end of day; comment \"Is the price with tax?\"\n"), "{shown}");
    // a clock's row
    let (_, _, stop) = ran_with("| 2026-11-18 09:00 | remind | | \"Late\" | first | in progress |\n");
    assert_eq!(stop, Some(("E603", "`remind` is a clock's event: its `comment` cell is empty".to_string())));
    // one past what a comment holds
    let (_, _, stop) = ran_with(&format!("| 2026-11-16 11:00 | comment | e1101 | \"{}\" | first | in progress |\n", "𠮷".repeat(2_001)));
    let (code, text) = stop.unwrap();
    assert_eq!(code, "E603");
    assert!(text.contains("the comment is 2001 characters long, past the limit (2000)"), "{text}");
}

/// Each calendar counts its business days from its own day of an event, read in its UTC offset,
/// and rings its clocks at their times in that offset (DESIGN 7.5). Behind the request's offset
/// (`behind`, read in Tokyo's time): a stage submitted at 10:00 on Monday in Tokyo, still Sunday at
/// a plant five hours behind UTC, is reminded from the plant's Sunday, at 09:00 on its Monday
/// (23:00 in Tokyo); the end of a pause and of the requester's turn are counted from the plant's
/// day of the pause and of the sending back, and a stage resumed after its three days counts its
/// clocks from three days after it started. Ahead of it (`ahead`, read in the plant's time): a
/// stage submitted at 20:00 on Monday at the plant, Tuesday in Tokyo, is reminded from the head
/// office's Tuesday, at 09:00 on its Wednesday (19:00 on Tuesday at the plant); counted from the
/// plant's Monday, it would be 09:00 on Tuesday in Tokyo, an hour before the submit. After the
/// stage goes up, the reminder is counted from the head office's day the deadline passed on. A
/// pause counted in the plant's days over a day only the plant closes (its inventory day; the head
/// office is open) takes four days of the plant's calendar for its three business days, and every
/// clock, the head office's deadline too, counts from four days after the stage started: the clocks
/// of a stage submitted four days later with no pause. A refusal says a clock's times in its offset:
/// the time it is set for and the time it is, and the time it rang. So it is in Japanese names, the
/// stage said as written.
#[test]
fn each_clock_rings_in_the_offset_of_its_calendar() {
    for (name, stage) in [("behind", "decide"), ("behind.ja", "決裁")] {
        with(&machine_file(name), |m, org| {
            let scenarios: Vec<String> = m.s.file.scenarios.iter().map(|s| s.name.text.clone()).collect();
            let t = ticket_after(m, org, &scenarios[0], 0);
            let st = t.stage().unwrap();
            assert_eq!((st.remind, st.due), (Some(at(m, "2026-11-16", "23:00")), Some(at(m, "2026-11-19", "00:00"))), "{name}");
            // before the reminder rings: its time and the time it is, both in the plant's offset
            let early = scenario::step(m, org, Some(&t), Input::Event(Event::Remind), at(m, "2026-11-16", "12:00")).unwrap();
            let r = early.answer.unwrap_err();
            assert_eq!(
                (r.kind, r.text.en.as_str(), r.text.ja.as_str()),
                ("clock", "the clock of `remind` is set for 2026-11-16 09:00, and it is 2026-11-15 22:00", "`remind` の時計は 2026-11-16 09:00 に鳴ります。今は 2026-11-15 22:00 です"),
                "{name}"
            );
            // after it rings, an approval waits for its event: the time it rang in its offset
            let late = scenario::step(m, org, Some(&t), Input::Event(approve("e1101")), at(m, "2026-11-16", "23:30")).unwrap();
            let r = late.answer.unwrap_err();
            assert_eq!(r.text.en, format!("the clock of `remind` (`{stage}`) rang at 2026-11-16 09:00: its event comes before `approve`"), "{name}");
            assert_eq!(r.text.ja, format!("`remind` の時計（`{stage}`）が 2026-11-16 09:00 に鳴っています。`approve` より先に、その時計のイベントを渡してください"), "{name}");
            // sent back on the plant's Tuesday: the requester's turn ends with the plant's Thursday,
            // 14:00 on Friday in Tokyo
            let back = ticket_after(m, org, &scenarios[0], 3);
            assert_eq!(back.resubmit_due, Some(at(m, "2026-11-20", "14:00")), "{name}");
            let r = scenario::step(m, org, Some(&back), Input::Event(Event::ResubmitLate), at(m, "2026-11-20", "13:00")).unwrap().answer.unwrap_err();
            assert_eq!(r.text.en, "the clock of `resubmit late` is set for 2026-11-20 00:00, and it is 2026-11-19 23:00", "{name}");
            // paused on the plant's Sunday: to the end of its Wednesday, 14:00 on Thursday in Tokyo
            let paused = ticket_after(m, org, &scenarios[1], 1);
            assert_eq!(paused.pause_ends, Some(at(m, "2026-11-19", "14:00")), "{name}");
            let r = scenario::step(m, org, Some(&paused), Input::Event(Event::PauseEnds), at(m, "2026-11-19", "13:00")).unwrap().answer.unwrap_err();
            assert_eq!(r.text.en, "the clock of `pause ends` is set for 2026-11-19 00:00, and it is 2026-11-18 23:00", "{name}");
            // resumed with the three days used: the head office counts from its Thursday, the plant
            // from its Wednesday
            let resumed = ticket_after(m, org, &scenarios[1], 2);
            let st = resumed.stage().unwrap();
            assert_eq!((resumed.pause_left, st.remind, st.due), (0, Some(at(m, "2026-11-19", "23:00")), Some(at(m, "2026-11-24", "00:00"))), "{name}");
            // paused on the plant's Sunday before its inventory day (Wednesday 2 December), which the
            // head office does not close: the three business days are the plant's Monday, Tuesday and
            // Thursday, to the end of its Thursday, 14:00 on Friday in Tokyo
            let r = ran(m, org, &scenarios[2]);
            assert_eq!(r.rows[1].ticket().unwrap().pause_ends, Some(at(m, "2026-12-04", "14:00")), "{name}");
            // resumed with the three days used, which take four days of the plant's calendar: every
            // clock, the head office's deadline too, counts from four days after the stage started,
            // the clocks of a stage submitted four days later with no pause; the deadline is a day
            // later than three days would make it, though the head office is open on the 2nd
            let resumed = r.rows[2].ticket().unwrap();
            let st = resumed.stage().unwrap();
            assert_eq!((resumed.pause_left, st.remind, st.due), (0, Some(at(m, "2026-12-04", "23:00")), Some(at(m, "2026-12-09", "00:00"))), "{name}");
            let Input::Submit(sub) = r.rows[0].stepped.input.clone() else { panic!("{name}") };
            let later = at(m, "2026-11-30", "10:00") + 4 * 86_400;
            let (t, _) = scenario::step(m, org, None, Input::Submit(sub), later).unwrap().answer.unwrap_or_else(|e| panic!("{name}: {}", e.text.en));
            let unpaused = t.stage().unwrap();
            assert_eq!((st.remind, st.due), (unpaused.remind, unpaused.due), "{name}");
        });
    }
    for name in ["ahead", "ahead.ja"] {
        with(&machine_file(name), |m, org| {
            let scenarios: Vec<String> = m.s.file.scenarios.iter().map(|s| s.name.text.clone()).collect();
            let submit = at(m, "2026-11-16", "20:00");
            let t = ticket_after(m, org, &scenarios[0], 0);
            let st = t.stage().unwrap();
            assert_eq!((st.remind, st.due), (Some(at(m, "2026-11-17", "19:00")), Some(at(m, "2026-11-19", "12:00"))), "{name}");
            assert!(st.remind.unwrap() > submit, "{name}");
            let r = scenario::step(m, org, Some(&t), Input::Event(Event::Remind), at(m, "2026-11-17", "08:00")).unwrap().answer.unwrap_err();
            assert_eq!(r.text.en, "the clock of `remind` is set for 2026-11-18 09:00, and it is 2026-11-17 22:00", "{name}");
            // sent back on the head office's Wednesday: the requester's turn ends with its Friday,
            // 10:00 on Friday at the plant
            let back = ticket_after(m, org, &scenarios[0], 2);
            assert_eq!(back.resubmit_due, Some(at(m, "2026-11-20", "10:00")), "{name}");
            // gone up at noon on the plant's Thursday, Friday in Tokyo: the reminder of the one it goes
            // up to is counted from the head office's Friday
            let up = ticket_after(m, org, &scenarios[1], 2);
            let st = up.stage().unwrap();
            assert_eq!((st.remind, st.due), (Some(at(m, "2026-11-22", "19:00")), Some(at(m, "2026-11-24", "12:00"))), "{name}");
            // paused on the head office's Tuesday: to the end of its Thursday, 10:00 on Thursday at
            // the plant; resumed with the two days used, the clocks count from two days after the start
            let paused = ticket_after(m, org, &scenarios[2], 1);
            assert_eq!(paused.pause_ends, Some(at(m, "2026-11-19", "10:00")), "{name}");
            let resumed = ticket_after(m, org, &scenarios[2], 2);
            let st = resumed.stage().unwrap();
            assert_eq!((resumed.pause_left, st.remind, st.due), (0, Some(at(m, "2026-11-19", "19:00")), Some(at(m, "2026-11-23", "12:00"))), "{name}");
        });
    }
}

/// Whether a stage's reminder has rung is the stage's to say (`reminded`, DESIGN 7.5), and a stage
/// resumed sets again the reminder that has not rung, set before or not, where it comes before the
/// deadline now. In `ahead`, a reminder counted past the head office's New Year break comes after
/// the deadline, and is not set; resumed with its clocks moved past the break, the stage sets it,
/// at the first resume, or at the second after a first pause that used no business day. A reminder
/// that rang between two pauses is not set at the second resume, though it would come before the
/// deadline. A stage resumed after its deadline passed sets the deadline at the time of the resume,
/// to ring at once, and no reminder, which would come after it; the one it goes up to has a reminder
/// of its own. A stage that goes up after its reminder rang has the reminder of the one it goes up
/// to set, not yet rung.
#[test]
fn a_stage_says_whether_its_reminder_has_rung() {
    for name in ["ahead", "ahead.ja"] {
        with(&machine_file(name), |m, org| {
            let scenarios: Vec<String> = m.s.file.scenarios.iter().map(|s| s.name.text.clone()).collect();
            let clocks = |r: &Ran, n: usize| {
                let st = r.rows[n].ticket().unwrap_or_else(|| panic!("{name}: row {n}")).stage().unwrap().clone();
                (st.remind, st.reminded, st.due, st.held)
            };
            // gone up after its reminder rang
            let r = ran(m, org, &scenarios[1]);
            assert!(clocks(&r, 1).1, "{name}");
            assert_eq!(clocks(&r, 2), (Some(at(m, "2026-11-22", "19:00")), false, Some(at(m, "2026-11-24", "12:00")), 0), "{name}");
            // past the New Year break: not set at the submit, set at the resume
            let r = ran(m, org, &scenarios[3]);
            assert_eq!(clocks(&r, 0), (None, false, Some(at(m, "2026-12-31", "12:00")), 0), "{name}");
            assert_eq!(clocks(&r, 2), (Some(at(m, "2027-01-05", "19:00")), false, Some(at(m, "2027-01-08", "12:00")), 2), "{name}");
            assert!(clocks(&r, 3).1, "{name}");
            // a first pause that uses no business day: not set at the first resume, set at the second
            let r = ran(m, org, &scenarios[4]);
            assert_eq!(clocks(&r, 2), (None, false, Some(at(m, "2026-12-31", "12:00")), 0), "{name}");
            assert_eq!(clocks(&r, 4), (Some(at(m, "2027-01-05", "19:00")), false, Some(at(m, "2027-01-08", "12:00")), 2), "{name}");
            // rung between two pauses: not set at the second resume, though a stage started two days
            // later with no pause has it before the deadline
            let r = ran(m, org, &scenarios[5]);
            assert_eq!(clocks(&r, 2), (Some(at(m, "2026-11-18", "19:00")), false, Some(at(m, "2026-11-20", "12:00")), 1), "{name}");
            assert!(clocks(&r, 3).1, "{name}");
            assert_eq!(clocks(&r, 5), (None, true, Some(at(m, "2026-11-23", "12:00")), 2), "{name}");
            let Input::Submit(sub) = r.rows[0].stepped.input.clone() else { panic!("{name}") };
            let (t, _) = scenario::step(m, org, None, Input::Submit(sub), at(m, "2026-11-18", "20:00")).unwrap().answer.unwrap_or_else(|e| panic!("{name}: {}", e.text.en));
            let st = t.stage().unwrap();
            assert_eq!((st.remind, st.due), (Some(at(m, "2026-11-19", "19:00")), Some(at(m, "2026-11-23", "12:00"))), "{name}");
            // resumed after the deadline passed: it rings at once, and no reminder is set; the one
            // the stage goes up to has one
            let r = ran(m, org, &scenarios[6]);
            assert_eq!(clocks(&r, 2), (None, false, Some(at(m, "2027-01-01", "10:00")), 0), "{name}");
            assert_eq!(clocks(&r, 3), (Some(at(m, "2027-01-03", "19:00")), false, Some(at(m, "2027-01-05", "12:00")), 0), "{name}");
        });
    }
}

/// The calendars of a request written again under a ticket in progress, the request and its
/// version as they were (`tests/rewritten/`: the same request in `before/` and `after/`,
/// whose calendars close days the ones before keep open: the head office's move from 18 to 20
/// November, and the plant's shutdown from 29 to 31 December). Closed days put in between the time
/// a stage's clocks were set and the time it is resumed do not change whether its reminder has rung,
/// which the stage says. The reminder that rang does not ring again: counted in the new calendars, it
/// would have come after the deadline, and it comes before it after the pause. The one not set, as it
/// came after the deadline, is set where it comes before it now: counted in the new calendars, it
/// would have come before it from the start. The deadline is counted in the new calendars.
#[test]
fn a_reminder_rings_once_when_the_calendars_are_written_again() {
    let dir = "tests/rewritten";
    for file in ["rewritten", "rewritten.ja"] {
        let (before, after) = (format!("{dir}/before/{file}.ringi"), format!("{dir}/after/{file}.ringi"));
        assert_eq!(std::fs::read_to_string(&before).unwrap(), std::fs::read_to_string(&after).unwrap(), "{file}");
        with(&before, |mb, ob| {
            with(&after, |ma, oa| {
                let go = |m: &Machine, org: &Org, t: Option<&Ticket>, input: Input, now: i64| -> Ticket {
                    let s = scenario::step(m, org, t, input, now).unwrap_or_else(|e| panic!("{file}: {}", e.en));
                    s.answer.unwrap_or_else(|r| panic!("{file}: {}", r.text.en)).0
                };
                let sub = |id: &str| Input::Submit(Submit { id: id.into(), requester: "e1201".into(), by: None, form: vec![("what".to_string(), Value::Str("A part".into()))], attempt: "attempt-1".into() });
                let by = || "e1101".to_string();
                let clocks = |t: &Ticket| {
                    let st = t.stage().unwrap();
                    (st.held, st.remind, st.reminded, st.due)
                };
                // set at the submit and rung; paused, and resumed by the clock of the pause after the
                // head office closed for its move
                let t = go(mb, ob, None, sub("T-1"), at(mb, "2026-11-16", "20:00"));
                assert_eq!(clocks(&t), (0, Some(at(mb, "2026-11-17", "19:00")), false, Some(at(mb, "2026-11-19", "12:00"))), "{file}");
                let t = go(mb, ob, Some(&t), Input::Event(Event::Remind), at(mb, "2026-11-17", "19:00"));
                let t = go(mb, ob, Some(&t), Input::Event(Event::Pause { by: by() }), at(mb, "2026-11-17", "20:00"));
                assert_eq!(t.pause_ends, Some(at(mb, "2026-11-20", "10:00")), "{file}");
                let fresh = go(ma, oa, None, sub("T-2"), at(ma, "2026-11-16", "20:00"));
                assert_eq!(clocks(&fresh), (0, None, false, Some(at(ma, "2026-11-19", "12:00"))), "{file}");
                let t = go(ma, oa, Some(&t), Input::Event(Event::PauseEnds), at(ma, "2026-11-20", "10:00"));
                assert_eq!(clocks(&t), (2, None, true, Some(at(ma, "2026-11-26", "12:00"))), "{file}");
                let later = go(ma, oa, None, sub("T-3"), at(ma, "2026-11-23", "20:00"));
                assert_eq!(clocks(&later), (0, Some(at(ma, "2026-11-24", "19:00")), false, Some(at(ma, "2026-11-26", "12:00"))), "{file}");
                let r = scenario::step(ma, oa, Some(&t), Input::Event(Event::Remind), at(ma, "2026-11-24", "19:00")).unwrap().answer.unwrap_err();
                assert_eq!(r.kind, "clock", "{file}: {}", r.text.en);
                // not set at the submit, as it came after the deadline; paused over the head office's
                // New Year break, and resumed in it, after the plant closed for its shutdown
                let t = go(mb, ob, None, sub("T-4"), at(mb, "2026-12-28", "08:00"));
                assert_eq!(clocks(&t), (0, None, false, Some(at(mb, "2026-12-31", "12:00"))), "{file}");
                let t = go(mb, ob, Some(&t), Input::Event(Event::Pause { by: by() }), at(mb, "2026-12-28", "09:00"));
                let fresh = go(ma, oa, None, sub("T-5"), at(ma, "2026-12-28", "08:00"));
                assert_eq!(clocks(&fresh), (0, Some(at(ma, "2027-01-03", "19:00")), false, Some(at(ma, "2027-01-05", "12:00"))), "{file}");
                let t = go(ma, oa, Some(&t), Input::Event(Event::Resume { by: by() }), at(ma, "2027-01-01", "10:00"));
                assert_eq!(clocks(&t), (0, Some(at(ma, "2027-01-03", "19:00")), false, Some(at(ma, "2027-01-05", "12:00"))), "{file}");
                let t = go(ma, oa, Some(&t), Input::Event(Event::Remind), at(ma, "2027-01-03", "19:00"));
                assert!(clocks(&t).2, "{file}");
            })
        });
    }
}

/// A clock counted `after N business days` (N ≥ 1) rings after the event it is counted from, at
/// whatever time of day the event comes (DESIGN 7.5): each calendar counts from its own day of the
/// event, so the first business day it counts is a later day there, and any time of that day is
/// after the event. Over every hour of two weeks, in both requests over two offsets: the reminder
/// and the deadline after the submit, the end of a pause after the pause, and the end of the
/// requester's turn after the sending back.
#[test]
fn a_clock_counted_after_business_days_rings_after_its_event() {
    for name in ["behind", "ahead"] {
        with(&machine_file(name), |m, org| {
            let first = &m.s.file.scenarios[0].name.text;
            let Input::Submit(sub) = ran(m, org, first).rows[0].stepped.input.clone() else { panic!("{name}") };
            let from = at(m, "2026-11-13", "00:00");
            for hour in 0..14 * 24 {
                let now = from + hour * 3_600;
                let when = scenario::local(m, now);
                let (t, _) = scenario::step(m, org, None, Input::Submit(sub.clone()), now).unwrap().answer.unwrap_or_else(|r| panic!("{name}, {when}: {}", r.text.en));
                let st = t.stage().unwrap();
                assert!(st.remind.is_some_and(|r| r > now) && st.due.is_some_and(|d| d > now), "{name}, submitted {when}: {:?}", (st.remind, st.due));
                let later = now + 60;
                let paused = scenario::step(m, org, Some(&t), Input::Event(Event::Pause { by: "e1101".into() }), later).unwrap().answer.unwrap_or_else(|r| panic!("{name}, {when}: {}", r.text.en)).0;
                assert!(paused.pause_ends.is_some_and(|e| e > later), "{name}, paused {when}: {:?}", paused.pause_ends);
                let back = scenario::step(m, org, Some(&t), Input::Event(Event::SendBack { by: "e1101".into(), to: None }), later).unwrap().answer.unwrap_or_else(|r| panic!("{name}, {when}: {}", r.text.en)).0;
                assert!(back.resubmit_due.is_some_and(|d| d > later), "{name}, sent back {when}: {:?}", back.resubmit_due);
            }
        });
    }
}

// ── The days a request can be used on (DESIGN 9.6) ──

/// The clock of `t` that rings first, as its event and its time.
fn first_clock(t: &Ticket) -> Option<(Event, i64)> {
    let (word, _, rings) = t.timers().into_iter().min_by_key(|(.., at)| *at)?;
    let ev = match word {
        "remind" => Event::Remind,
        "late" => Event::Late,
        "pause ends" => Event::PauseEnds,
        "resubmit late" => Event::ResubmitLate,
        other => panic!("{other}"),
    };
    Some((ev, rings))
}

/// A stage due by one calendar and reminded by another (DESIGN 5.10): after it goes up, the reminder
/// of the ones it goes up to is counted in the reminder's calendar from the day the deadline passed
/// (the scenario of `apart`, held in its golden file), and the last day such a reminder can be
/// counted from is one of the last days the request is used on. With the plant's data ending on
/// 2027-12-31, a stage started on 2027-12-23 is due on the 27th, goes up on the 28th, is due again
/// on 2028-01-05 (the office closes over the New Year), and would go up a second time on the 6th,
/// past the plant's data: the request is used through 2027-12-22, and a ticket made that day has
/// every clock counted, the reminders in the plant's days. What decides the day is said to be the
/// reminders (the kind `reminder`), not the deadlines, which the office's calendar counts.
#[test]
fn a_reminder_counted_in_another_calendar_than_its_deadline() {
    let file = machine_file("apart");
    let src = std::fs::read_to_string(&file).unwrap();
    let text = src.replace("use calendar plant from \"plant.cal\"", "use calendar plant from \"plant_finite.cal\"");
    assert!(text != src);
    let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
    let o = run.text(&file, &text);
    assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
    let ends = "the data of the calendar `plant` (`tests/machine/plant_finite.cal`) ends on 2027-12-31, and the reminders of a stage started later cannot be counted in it after the stage goes up";
    assert!(common::shown(&o, Lang::En).contains(&format!("  used through 2027-12-22: {ends}\n")), "{}", common::shown(&o, Lang::En));
    let by = |o: &ringi::check::Outcome| o.usable.as_ref().and_then(|u| u.through.as_ref()).map(|e| e.by.iter().map(|b| b.kind.word()).collect::<Vec<_>>());
    assert_eq!(by(&o), Some(vec!["reminder"]));
    // a stage that does not go up has the one reminder, counted from the day it starts: the last
    // day is the day before the plant's last, and what cannot be counted is said to be a reminder
    let plain = text.replace("when late escalate, then escalate", "when late expire");
    let plain = &plain[..plain.find("\nscenario ").unwrap()];
    assert!(plain != text);
    let p = run.text(&file, plain);
    assert!(!p.has_errors(), "{}", common::shown(&p, Lang::En));
    let said = common::shown(&p, Lang::En);
    assert!(said.contains("  used through 2027-12-30: the data of the calendar `plant` (`tests/machine/plant_finite.cal`) ends on 2027-12-31, and the reminders of a stage started later cannot be counted in it\n"), "{said}");
    assert_eq!(by(&p), Some(vec!["reminder"]));
    ringi::engine::with_text(&o, &run, &text, |m, org| {
        let Input::Submit(sub) = ran(m, org, "late_and_up_twice").rows[0].stepped.input.clone() else { panic!() };
        let submit = |day: &str| scenario::step(m, org, None, Input::Submit(sub.clone()), at(m, day, "10:00")).unwrap().answer;
        let no = submit("2027-12-23").unwrap_err();
        assert_eq!((no.kind, no.text.en), ("data", format!("the request can be used through 2027-12-22, not on 2027-12-23 ({ends})")));
        let (mut t, _) = submit("2027-12-22").unwrap();
        let mut went = Vec::new();
        while let Some((ev, rings)) = first_clock(&t) {
            let s = scenario::step(m, org, Some(&t), Input::Event(ev), rings).unwrap();
            let (after, _) = s.answer.as_ref().unwrap_or_else(|r| panic!("{} at {}: {}", s.input.word(), scenario::local(m, rings), r.text.en));
            went.push(format!("{} {}", s.input.word(), scenario::local(m, rings)));
            t = after.clone();
        }
        assert_eq!(t.status.word(), "expired");
        // Saturday the 25th to Tuesday the 28th over the plant's Sunday and Monday; the second
        // deadline over the office's New Year break
        assert_eq!(went, ["remind 2027-12-23 09:00", "late 2027-12-25 00:00", "remind 2027-12-28 09:00", "late 2027-12-29 00:00", "remind 2027-12-30 09:00", "late 2028-01-06 00:00"]);
    })
    .unwrap();
}

/// A request is not made, and no stage starts, on a day the data does not cover. With a calendar
/// whose data ends on 2027-12-31 the desk request is used through 2027-12-15 (what the check
/// says): a stage started that day and paused for the five business days the request allows has
/// its clocks counted from 2027-12-22, the last day its deadline, and the deadline after it goes
/// up, are inside the data. The machine refuses a request made the day after, with the data that
/// ends; a ticket made on the last day goes all its way with nobody answering, paused or not,
/// every clock inside the data. The part of the organization given with an event is the part of
/// the event's day.
#[test]
fn a_day_the_data_does_not_cover_is_refused() {
    let file = machine_file("desk");
    let src = std::fs::read_to_string(&file).unwrap();
    let text = src.replace("use calendar office from \"../../examples/shared/office.cal\"", "use calendar office from \"../mutants/shared/finite.cal\"");
    assert!(text != src);
    let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
    let o = run.text(&file, &text);
    assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
    let ends = "the data of the calendar `office` (`tests/mutants/shared/finite.cal`) ends on 2027-12-31, and the deadlines of a stage started later and paused for 5 business days (`pause at most 5 business days`) cannot be counted in it";
    assert!(common::shown(&o, Lang::En).contains(&format!("  used through 2027-12-15: {ends}\n")), "{}", common::shown(&o, Lang::En));
    ringi::engine::with_text(&o, &run, &text, |m, org| {
        let Input::Submit(sub) = ran(m, org, "approved_by_a_quorum").rows[0].stepped.input.clone() else { panic!() };
        let submit = |day: &str| scenario::step(m, org, None, Input::Submit(sub.clone()), at(m, day, "10:00")).unwrap().answer;
        let no = submit("2027-12-16").unwrap_err();
        assert_eq!((no.kind, no.text.en), ("data", format!("the request can be used through 2027-12-15, not on 2027-12-16 ({ends})")));
        let (made, _) = submit("2027-12-15").unwrap();
        // the clocks one after another, with nobody answering: each event and the time it is given at
        let clocks = |t: &Ticket| {
            let (mut t, mut went) = (t.clone(), Vec::new());
            while let Some((ev, rings)) = first_clock(&t) {
                let s = scenario::step(m, org, Some(&t), Input::Event(ev), rings).unwrap();
                let (after, _) = s.answer.as_ref().unwrap_or_else(|r| panic!("{} at {}: {}", s.input.word(), scenario::local(m, rings), r.text.en));
                went.push(format!("{} {}", s.input.word(), scenario::local(m, rings)));
                t = after.clone();
            }
            (t.status.word(), went)
        };
        // it goes up after the last day the request is used on, with a deadline the data covers
        assert_eq!(
            clocks(&made),
            ("expired", vec!["remind 2027-12-17 09:00".to_string(), "late 2027-12-21 00:00".to_string(), "remind 2027-12-23 09:00".to_string(), "late 2027-12-25 00:00".to_string()])
        );
        // paused at once for all the days it may be: the clocks are five business days later, and
        // the deadline after it goes up is the last day of the data
        let (line, paused) = give(m, org, &made, Event::Pause { by: "e3101".into() }, at(m, "2027-12-15", "11:00"));
        assert!(line.starts_with("pause by e3101: in progress at first -> paused at first |"), "{line}");
        assert_eq!(
            clocks(&paused.unwrap()),
            (
                "expired",
                vec![
                    "pause ends 2027-12-23 00:00".to_string(),
                    "remind 2027-12-24 09:00".to_string(),
                    "late 2027-12-28 00:00".to_string(),
                    "remind 2027-12-30 09:00".to_string(),
                    "late 2028-01-01 00:00".to_string(),
                ],
            )
        );
        // a stage that would start past the last day: the approval that moves the ticket on is refused
        let s = scenario::step(m, org, Some(&made), Input::Event(approve("e3101")), at(m, "2027-12-16", "10:00")).unwrap();
        let no = s.answer.unwrap_err();
        assert_eq!((no.kind, no.text.en), ("data", format!("the request can be used through 2027-12-15, not on 2027-12-16 ({ends})")));
        assert!(give(m, org, &made, approve("e3101"), at(m, "2027-12-15", "12:00")).0.starts_with("approve by e3101: in progress at first -> in progress at board |"));
        // the part of another day than the event's
        let part = m.part(org, &ringi::engine::Need::event(&made, &approve("e3101")), at(m, "2027-12-15", "11:00")).unwrap();
        let no = m.next(&made, &approve("e3101"), None, Some(&made.revision()), &part, at(m, "2027-12-16", "11:00")).unwrap_err();
        assert_eq!((no.kind, no.text.en.as_str()), ("data", "the part of the organization is of 2027-12-15, and the event is on 2027-12-16"));
        assert!(m.next(&made, &approve("e3101"), None, Some(&made.revision()), &part, at(m, "2027-12-15", "11:00")).is_ok());
    })
    .unwrap();
}

/// The days to resubmit are counted from the day a ticket goes back to its requester: the last day
/// the request is used on is one they can be counted from, to the day (DESIGN 9.6). With thirteen
/// business days to resubmit and the calendar's data through 2027-12-31, a ticket goes back on
/// 2027-12-14 and has its days to the last day of the data; a day later it is not sent back.
#[test]
fn the_days_to_resubmit_are_in_the_days_the_request_is_used_on() {
    let file = machine_file("desk");
    let src = std::fs::read_to_string(&file).unwrap();
    let text = src.replace("use calendar office from \"../../examples/shared/office.cal\"", "use calendar office from \"../mutants/shared/finite.cal\"").replace("resubmit          within 3 business days", "resubmit          within 13 business days");
    assert!(text.contains("../mutants/shared/finite.cal") && text.contains("within 13 business days"));
    let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
    let o = run.text(&file, &text);
    assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
    let ends = "the data of the calendar `office` (`tests/mutants/shared/finite.cal`) ends on 2027-12-31, and the days to resubmit a ticket that goes back to its requester later cannot be counted in it (`resubmit within 13 business days`)";
    assert!(common::shown(&o, Lang::En).contains(&format!("  used through 2027-12-14: {ends}\n")), "{}", common::shown(&o, Lang::En));
    let ja = "  2027-12-14 まで使えます: カレンダー `office`（`tests/mutants/shared/finite.cal`）のデータが 2027-12-31 までなので、その後に申請者へ戻ったチケットの再申請の期限を数えられません（`resubmit within 13 business days`）\n";
    assert!(common::shown(&o, Lang::Ja).contains(ja), "{}", common::shown(&o, Lang::Ja));
    ringi::engine::with_text(&o, &run, &text, |m, org| {
        let Input::Submit(sub) = ran(m, org, "approved_by_a_quorum").rows[0].stepped.input.clone() else { panic!() };
        let submit = |day: &str| scenario::step(m, org, None, Input::Submit(sub.clone()), at(m, day, "10:00")).unwrap().answer;
        assert_eq!(submit("2027-12-15").unwrap_err().kind, "data");
        let (made, _) = submit("2027-12-14").unwrap();
        // sent back on the last day: the thirteenth business day after it is the last of the data
        let back = |day: &str| give(m, org, &made, Event::SendBack { by: "e3101".into(), to: None }, at(m, day, "11:00"));
        let (line, t) = back("2027-12-14");
        assert!(line.starts_with("send back by e3101: in progress at first -> with requester |"), "{line}");
        let (ev, rings) = first_clock(&t.unwrap()).unwrap();
        assert_eq!((ev.word(), scenario::local(m, rings)), ("resubmit late", "2028-01-01 00:00".to_string()));
        // a day later the days cannot be counted, and the ticket does not go back
        let s = scenario::step(m, org, Some(&made), Input::Event(Event::SendBack { by: "e3101".into(), to: None }), at(m, "2027-12-15", "11:00")).unwrap();
        let no = s.answer.unwrap_err();
        assert_eq!((no.kind, no.text.en.as_str()), ("data", "the data of the calendar `office` covers 2026-01-01 to 2027-12-31: business days cannot be counted from 2027-12-15"));
    })
    .unwrap();
    // `--through` asks past it: the line of `resubmit within`, and what to extend
    let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, through: Some(ritsu_ports::day_of("2028-03-31").unwrap()), ..Options::default() });
    let shown = common::shown(&run.text(&file, &text), Lang::En);
    assert!(shown.contains("The request cannot be used through 2028-03-31 (`--through`): it can be used through 2027-12-14\n"), "{shown}");
    assert!(shown.contains("  = Extend the calendar's table of closed days, so that the days to resubmit a ticket that goes back on 2028-03-31 can be counted in it.\n"), "{shown}");
}

// ── The command (DESIGN 16.1) ──

/// `ringi run`: every scenario as written is exit 0; one that is not is exit 1, with the rows that
/// ran, where it stops and the finding the check gives for it (E602, E603, E604); what it cannot
/// run is exit 2, with why.
#[test]
fn run_exits_by_how_the_scenarios_go() {
    let desk = machine_file("desk");
    // one scenario by its name
    let (code, out, err) = cli(&["run", &desk, "--scenario", "withdrawn_and_discarded"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        out,
        "2026-11-16 10:00  submit    by e1201  -> first (e1101), due 2026-11-19 end of day\n\
         2026-11-16 11:00  withdraw  by e1201  -> with requester, resubmit by 2026-11-19 end of day\n\
         2026-11-16 12:00  discard   by e1201  -> withdrawn\n\
         scenario `withdrawn_and_discarded`: as written (3 rows)\n"
    );
    let (code, out, err) = cli(&["run", &desk, "--scenario", "nope"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.starts_with("error: `tests/machine/desk.ringi` has no scenario `nope`: it has `approved_by_a_quorum`, `rejected_when_the_quorum_is_out_of_reach`, "), "{err}");
    // a request with no scenario has nothing to run: exit 2, as `vectors` says it, whatever is asked for
    let bare = machine_file("bare");
    let none = (2, String::new(), "error: `tests/machine/bare.ringi` has no scenario\n".to_string());
    assert_eq!(cli(&["run", &bare]), none);
    assert_eq!(cli(&["run", &bare, "--scenario", "x"]), none);
    // asked for JSON, the same sentence is the JSON's `error` (ritsu's DESIGN 6.2, item 10)
    let error = "{\"error\":{\"code\":2,\"message\":\"`tests/machine/bare.ringi` has no scenario\"}}\n".to_string();
    assert_eq!(cli(&["run", &bare, "--format", "json"]), (2, error, none.2.clone()));
    assert_eq!(cli(&["run", &bare, "--lang", "ja"]), (2, String::new(), "エラー: `tests/machine/bare.ringi` にシナリオがありません\n".to_string()));
    assert_eq!(cli(&["vectors", &bare]).0, 2);
    // a scenario that does not go as written: the rows that ran, and the finding
    let e602 = "tests/mutants/E602_another_stage_than_the_row_writes.ringi";
    let (code, out, err) = cli(&["run", e602]);
    assert_eq!((code, err.as_str()), (1, ""));
    assert_eq!(
        out,
        "2026-06-01 09:00  submit   by e03  -> check (e04)\n\
         2026-06-01 10:00  approve  by e04  -> decide (one of e02, e03)\n\
         scenario `a_small_expense`: not as written\n\
         error[E602]: tests/mutants/E602_another_stage_than_the_row_writes.ringi:24:49: After `approve`, the ticket is `decide`, `in progress`, and the scenario writes `approved`\n\
        \x20   24 | | 2026-06-01 10:00 | approve | e04 |          | approved    |\n\
        \x20 = The ticket is at `decide`, and is `in progress`. No clock is set.\n"
    );
    let (code, out, _) = cli(&["run", "tests/mutants/E603_an_approval_by_someone_else.ringi", "--lang", "ja"]);
    assert_eq!(code, 1);
    assert!(out.contains("2026-06-01 10:00  approve  e02  -> 受け付けません: e02（営業部長）は、ステップ `check` の承認者でも、その代理でもありません\nシナリオ `a_small_expense`: 書いたとおりに進みません\nエラー[E603]: "), "{out}");
    let (code, out, _) = cli(&["run", "tests/mutants/E604_a_deadline_a_minute_early.ringi"]);
    assert_eq!(code, 1);
    assert!(out.contains("scenario `nobody_answers`: not as written\nerror[E604]: tests/mutants/E604_a_deadline_a_minute_early.ringi:27:22: The clock of `late` (`check`) is set for 2026-06-05 00:00, and the row is at 2026-06-04 23:59\n"), "{out}");
    // the same in JSON: where it stops, with the finding's code
    let (code, out, _) = cli(&["run", e602, "--format", "json"]);
    assert_eq!(code, 1);
    let v = json::parse(&out).unwrap();
    let sc = &v.get("scenarios").and_then(Json::as_arr).unwrap()[0];
    assert_eq!((sc.get("as_written").and_then(Json::as_bool), sc.get("stage").and_then(Json::as_str), sc.get("status").and_then(Json::as_str)), (Some(false), Some("decide"), Some("in_progress")));
    let stopped = sc.get("stopped").unwrap();
    assert_eq!((stopped.get("code").and_then(Json::as_str), stopped.get("line").map(Json::compact)), (Some("E602"), Some("24".to_string())));
    assert_eq!(sc.get("rows").and_then(Json::as_arr).map(|r| r.len()), Some(2));
    // and a scenario as written says so
    let (_, out, _) = cli(&["run", "examples/leave.ringi", "--format", "json"]);
    let v = json::parse(&out).unwrap();
    let keys: Vec<&str> = v.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["ringi", "file", "scenarios"]);
    let sc = &v.get("scenarios").and_then(Json::as_arr).unwrap()[0];
    let keys: Vec<&str> = sc.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["scenario", "as_written", "status", "stage", "timers", "rows", "books", "stopped"]);
    assert!(sc.get("as_written").and_then(Json::as_bool) == Some(true) && sc.get("stopped").is_some_and(Json::is_null));
    let keys: Vec<&str> = sc.get("rows").and_then(Json::as_arr).unwrap()[0].as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["line", "at", "step", "books"]);
    // a file that does not pass its check: what the check prints
    let (code, out, _) = cli(&["run", "tests/mutants/E004_a_cell_not_in_who.ringi"]);
    assert_eq!((code, out), (1, std::fs::read_to_string("tests/golden/E004_a_cell_not_in_who.en.txt").unwrap()));
    // what it does not run
    let stops = |args: &[&str]| {
        let (code, out, err) = cli(args);
        assert_eq!(out, "", "{args:?}");
        (code, err)
    };
    let takes = "error: `ringi run` takes <file.ringi>\nusage: ringi run <file.ringi> [--scenario <name>] [--org <dir>] [--format json]\n".to_string();
    assert_eq!(stops(&["run"]), (2, takes.clone()));
    assert_eq!(stops(&["run", "examples/leave.ringi", "examples/expense.ringi"]), (2, takes));
    assert_eq!(stops(&["run", "examples/shared/company.ringi"]), (2, "error: a ticket is of a request's file\n".to_string()));
    assert_eq!(stops(&["run", "examples/leave.ringi", "--fast"]), (2, "error: unknown flag `--fast`; run `ringi run --help`\n".to_string()));
    assert_eq!(stops(&["run", "examples/leave.ringi", "--format", "yaml"]), (2, "error: `--format yaml` is not a value this flag takes; it takes only json\n".to_string()));
    assert_eq!(stops(&["run", "examples/leave.ringi", "--org", "nowhere"]), (2, "error: `--org nowhere` is not a directory\n".to_string()));
    assert_eq!(stops(&["run", "nowhere.ringi"]), (2, "error: `nowhere.ringi` is not there\n".to_string()));
    assert!(stops(&["run", "examples/leave.ringi", "--org", "tests/machine"]).1.starts_with("error: the organization has no data"));
}

/// `--help` of the two commands, and the line of each in the command's own help.
#[test]
fn help_of_run_and_vectors() {
    let (code, out, _) = cli(&["run", "--help"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("ringi run — run the scenarios: each row of a table given to the ticket machine at the time it is written at"), "{out}");
    assert!(out.contains("\nUsage:\n  ringi run <file.ringi> [--scenario <name>] [--org <dir>] [--format json]\n") && out.contains("  --scenario <name>  run that scenario alone; else every scenario of the file\n"), "{out}");
    let (code, out, _) = cli(&["help", "run", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("ringi run — シナリオを流す。表の行を一つずつ、書いた時刻にチケットの機械へ渡し"), "{out}");
    let (code, out, _) = cli(&["vectors", "--help"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("ringi vectors — write the vectors: transitions of the ticket machine, one JSON object a line (JSON Lines)"), "{out}");
    assert!(out.contains("\nUsage:\n  ringi vectors <file.ringi> [--list] [--org <dir>]\n") && out.contains("  --list        instead of the JSON, each vector in a line of words"), "{out}");
    let (code, out, _) = cli(&["help", "vectors", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("ringi vectors — ベクタを書く。チケットの機械の遷移を、一行に一つの JSON で（JSON Lines）"), "{out}");
    let (_, out, _) = cli(&["--help"]);
    assert!(out.contains("  ringi run <file.ringi>  ") && out.contains("  ringi vectors <file.ringi>  "), "{out}");
}

/// A `reject` and a `send back` the request asks a reason with (`comment required for`, DESIGN
/// 7.2): refused (`comment`) with no comment, or with one of white space alone (the ideographic
/// space and U+0085 among it), before who gives them is asked, in English and in Japanese; taken
/// with a reason, one character that is no white space (U+FEFF, U+001C) among them. An approval,
/// which the line does not name, needs none.
#[test]
fn a_reject_and_a_send_back_need_a_reason() {
    with(&machine_file("reasons"), |m, org| {
        let t = ticket_after(m, org, "sent_back_approved_and_rejected_with_reasons", 0);
        let now = at(m, "2026-11-16", "11:00");
        let reject = |by: &str| Event::Reject { by: by.into() };
        let back = |by: &str| Event::SendBack { by: by.into(), to: None };
        for (ev, word) in [(reject("e1101"), "reject"), (back("e1101"), "send back")] {
            for c in ["", " \u{3000}\n\t", "\u{85}"] {
                let s = said(m, org, &t, ev.clone(), c, now);
                assert_eq!(refusal(&s), ("comment", format!("a `{word}` of this request needs a reason: write it in the comment")));
                assert_eq!(s.answer.as_ref().unwrap_err().text.ja, format!("この申請の `{word}` には理由が要ります。コメントに書いてください"));
                assert_eq!(s.answer.as_ref().unwrap_err().at, None);
            }
        }
        // before who gives it: one who answers no stage of the ticket is told of the reason first
        assert_eq!(refusal(&said(m, org, &t, reject("e1203"), "", now)).0, "comment");
        assert_eq!(refusal(&said(m, org, &t, reject("e1203"), "Not mine to say", now)).0, "person");
        // with a reason, taken; an approval needs none
        assert!(said(m, org, &t, reject("e1101"), "Not this year", now).answer.is_ok());
        assert!(said(m, org, &t, back("e1101"), "Which desk?", now).answer.is_ok());
        for c in ["\u{feff}", "\u{1c}"] {
            assert!(said(m, org, &t, reject("e1101"), c, now).answer.is_ok(), "{c:?}");
            assert!(said(m, org, &t, back("e1101"), c, now).answer.is_ok(), "{c:?}");
        }
        assert!(said(m, org, &t, approve("e1101"), "", now).answer.is_ok());
    });
}
