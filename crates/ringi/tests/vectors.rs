//! The vectors (DESIGN 10.4; PLAN C3.4): `ringi vectors` writes transitions of the ticket machine,
//! one JSON object a line, for the code a request is generated to, and a server, to answer the
//! same.
//!
//! What is held here: the list of the vectors of each example and of each request written for the
//! machine (`tests/golden/vectors/`, one line a vector: where it comes from, the event, the time,
//! the answer), and the JSON Lines of the leave request whole. `RINGI_BLESS=1 cargo test -p ringi
//! --test vectors` writes them again. The rest is said of every line: it is a transition the
//! reference interpreter makes again from what the line holds, the rows of the scenarios are the
//! rows `ringi run` shows, and the names are the ASCII ones.

mod common;

use ringi::check::{Options, Run};
use ringi::engine::{Event, Machine, What};
use ringi::org::Org;
use ringi::scenario::{self, Input, Stepped};
use ringi::vectors::{self, Vector};
use ritsu_base::json::{self, Json};
use ritsu_base::text::Lang;

/// The requests written for the machine that have a scenario.
const MACHINE: [&str; 17] = ["absent", "ahead", "behind", "deep", "desk", "flows", "last", "late", "once", "ranges", "reminds", "seat", "steps", "supplies", "top", "waits", "who"];
/// The examples of `examples/`.
const EXAMPLES: [&str; 5] = ["account", "contract", "expense", "leave", "purchase"];

/// The keys of a line, in the order they are written.
const KEYS: [&str; 7] = ["request", "from", "now", "ticket", "event", "organization", "answer"];

fn cli(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = ringi::run::run(&args, common::joined(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// Every request the vectors are made for here: the examples in English names and in Japanese
/// names, and the requests written for the machine.
fn files() -> Vec<String> {
    let mut out: Vec<String> = EXAMPLES.iter().flat_map(|n| [format!("examples/{n}.ringi"), format!("examples/{n}.ja.ringi")]).collect();
    out.extend(MACHINE.iter().map(|n| format!("tests/machine/{n}.ringi")));
    out
}

/// The vectors of the request at `path`, as `ringi vectors` makes them: with the machine and the
/// organization they are made on.
fn with<R>(path: &str, f: impl FnOnce(&Machine, &Org, &[Vector]) -> R) -> R {
    let run = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
    let o = run.file(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert!(!o.has_errors(), "{path}:\n{}", common::shown(&o, Lang::En));
    ringi::engine::with(&o, &run, |m, org| {
        let rans: Vec<_> = m.s.file.scenarios.iter().map(|sc| scenario::run(m, org, sc)).collect();
        assert!(rans.iter().all(|r| r.stop.is_none()), "{path}");
        f(m, org, &vectors::make(m, org, &rans))
    })
    .unwrap_or_else(|t| panic!("{path}: {}", t.en))
}

/// The lines of `ringi vectors <path>`, each parsed.
fn lines(path: &str, lang: Lang) -> Vec<Json> {
    let (code, out, err) = cli(&["vectors", path, "--lang", lang.code()]);
    assert_eq!((code, err.as_str()), (0, ""), "{path}");
    out.lines().map(|l| json::parse(l).unwrap_or_else(|e| panic!("{path}: {e:?}: {l}"))).collect()
}

fn text<'j>(j: &'j Json, key: &str) -> &'j str {
    j.get(key).and_then(Json::as_str).unwrap_or_else(|| panic!("no `{key}` in {}", j.compact()))
}

/// The event of a line: a variant named by it (`{"approve": …}`), or the name alone (`"late"`).
fn event_of(j: &Json) -> String {
    match j.get("event").unwrap_or_else(|| panic!("no event in {}", j.compact())) {
        Json::Str(s) => s.clone(),
        Json::Obj(pairs) if pairs.len() == 1 => pairs[0].0.clone(),
        other => panic!("an event is a variant: {}", other.compact()),
    }
}

// ── What is written ──

/// The list of the vectors of each request (`--list`): where each comes from, the event, the time
/// and the answer. A vector added, lost or answered otherwise shows here as a line.
#[test]
fn the_vectors_of_each_request_listed() {
    for path in files() {
        let (code, out, err) = cli(&["vectors", &path, "--list"]);
        assert_eq!((code, err.as_str()), (0, ""), "{path}");
        let name = path.trim_start_matches("examples/").trim_start_matches("tests/").trim_end_matches(".ringi");
        ritsu_testkit::golden(format!("tests/golden/vectors/{name}.txt"), &out);
        // the list is in words of its own, the same in either language
        assert_eq!(cli(&["vectors", &path, "--list", "--lang", "ja"]).1, out, "{path}");
    }
}

/// The JSON Lines of the leave request, whole: what a line holds, key by key, is the form the
/// generated code and a server read (DESIGN 10.4).
#[test]
fn the_vectors_of_the_leave_request() {
    let (code, out, err) = cli(&["vectors", "examples/leave.ringi"]);
    assert_eq!((code, err.as_str()), (0, ""));
    ritsu_testkit::golden("tests/golden/vectors/leave.jsonl", &out);
}

/// Every line is one JSON object with the same seven keys, and holds one transition: nothing
/// before a `submit`, a ticket before anything else; the answer is `ok` with a ticket and the things
/// to do, or `err` with a refusal's kind and its sentence (DESIGN 10.4).
#[test]
fn a_line_is_one_transition() {
    for path in files() {
        let all = lines(&path, Lang::En);
        assert!(all.len() > 50, "{path}: {}", all.len());
        for j in &all {
            let keys: Vec<&str> = j.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
            assert_eq!(keys, KEYS, "{path}");
            let event = event_of(j);
            assert_eq!(j.get("ticket").unwrap().is_null(), event == "submit", "{path}: {}", text(j, "from"));
            let answer = j.get("answer").unwrap();
            let keys: Vec<&str> = answer.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
            match keys.as_slice() {
                ["ok"] => {
                    let ok = answer.get("ok").unwrap();
                    let keys: Vec<&str> = ok.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
                    assert_eq!(keys, ["ticket", "effects"], "{path}");
                    assert!(ok.get("effects").and_then(Json::as_arr).is_some());
                }
                // a refusal: its kind, and its sentence
                ["err"] => {
                    let err = answer.get("err").unwrap();
                    assert!(!text(err, "refused").is_empty() && !text(err, "text").is_empty(), "{path}");
                }
                other => panic!("{path}: {other:?}"),
            }
            // the part of the organization is of the day of the event
            assert!(j.get("organization").and_then(|o| o.get("day")).and_then(Json::as_str).is_some(), "{path}");
        }
    }
}

/// Made twice, the vectors are the same bytes: nothing of them comes from the time they are made
/// at or from an order no file gives.
#[test]
fn the_vectors_are_the_same_each_time() {
    for path in ["examples/purchase.ringi", "examples/leave.ja.ringi", "tests/machine/once.ringi"] {
        let (a, b) = (cli(&["vectors", path]).1, cli(&["vectors", path]).1);
        assert!(!a.is_empty() && a == b, "{path}");
    }
}

/// A refusal names the place of the form it is of (`at`, DESIGN 12.4), in every request's
/// vectors: a field of the form by its ASCII name, or a field of a row the event gives, after its
/// place from 0 within the rows given, in brackets (`items[1].price`); only a refusal of the form
/// names one, and every refusal of the form names one but those of no field (a name the form does
/// not have, the comment).
#[test]
fn a_refusal_names_the_place_it_is_of() {
    use ritsu_ports::Value;
    let (mut placed, mut in_rows) = (0, 0);
    for path in files() {
        with(&path, |m, _, vs| {
            for v in vs {
                let Err(r) = &v.stepped.answer else { continue };
                let given = match &v.stepped.input {
                    Input::Submit(s) => s.form.clone(),
                    Input::Event(Event::Resubmit { form, .. }) => form.clone(),
                    Input::Event(Event::Edit { fields, .. }) => fields.clone(),
                    _ => Vec::new(),
                };
                let Some(at) = &r.at else {
                    let t = &r.text.en;
                    let unnamed = t.starts_with("the form has no field") || t.starts_with("the comment is") || t.starts_with("the text of `comment`");
                    assert!(r.kind != "form" || unnamed, "{path}: {}: a refusal of the form that names no place: {t}", v.from);
                    continue;
                };
                assert_eq!(r.kind, "form", "{path}: {}: {at}", v.from);
                // a field (`purpose`), or a field of a row with the row's place in brackets
                // (`items[1].price`), as the errors of the entries write a place (DESIGN 12.4)
                let first = at.split(['[', '.']).next().unwrap_or_default();
                let f = m.s.fields().iter().find(|f| f.name.ascii() == first).unwrap_or_else(|| panic!("{path}: {}: `{at}` names no field", v.from));
                if let Some(rest) = at.strip_prefix(&format!("{first}[")) {
                    let (row, cell) = rest.split_once("].").unwrap_or_else(|| panic!("{path}: {}: `{at}`", v.from));
                    let row: usize = row.parse().unwrap_or_else(|_| panic!("{path}: {}: `{at}`", v.from));
                    let rows = given.iter().find(|(n, _)| n == first).and_then(|(_, x)| if let Value::List(rs) = x { Some(rs.len()) } else { None });
                    assert!(rows.is_some_and(|n| row < n), "{path}: {}: `{at}` is past the rows given ({rows:?})", v.from);
                    let c = f.children.iter().find(|c| c.name.ascii() == cell).unwrap_or_else(|| panic!("{path}: {}: `{at}` names no field of a row", v.from));
                    // the words name the field of the row at the same place, as the request writes the
                    // names (`items[1].price`, `明細[1].金額`)
                    let said = format!("{}[{row}].{}", f.name.text, c.name.text);
                    assert!(r.text.en.contains(&said) && r.text.ja.contains(&said), "{path}: {}: `{at}`: {} / {}", v.from, r.text.en, r.text.ja);
                    in_rows += 1;
                } else {
                    assert_eq!(at, first, "{path}: {}: `{at}`", v.from);
                }
                placed += 1;
            }
        });
    }
    eprintln!("{placed} refusals of the vectors name a place of the form, {in_rows} of them a field of a row");
    assert!(placed > 50 && in_rows > 10, "{placed}, {in_rows}");
}

/// A ticket is one of the person's own requests (`list_mine`, DESIGN 12.4) for its requester and for
/// the one who made it in their name: the ticket says both from its submit on (`requester`,
/// `made_by`, null where the requester made it), and no later event changes them, so a server lists
/// it from the ticket it keeps.
#[test]
fn a_ticket_says_whose_own_request_it_is() {
    let (mut made_for, mut kept) = (0, 0);
    for path in files() {
        with(&path, |_, _, vs| {
            for v in vs {
                let Ok((after, _)) = &v.stepped.answer else { continue };
                match (&v.stepped.input, &v.stepped.before) {
                    (Input::Submit(s), None) => {
                        assert_eq!(after.requester, s.requester, "{path}: {}", v.from);
                        assert_eq!(after.made_by, s.by.clone().filter(|b| *b != s.requester), "{path}: {}", v.from);
                        made_for += usize::from(after.made_by.is_some());
                    }
                    (_, Some(before)) => {
                        assert_eq!((&after.requester, &after.made_by), (&before.requester, &before.made_by), "{path}: {}", v.from);
                        kept += 1;
                    }
                    _ => {}
                }
            }
        });
    }
    eprintln!("{made_for} tickets made by a proxy; {kept} events after a submit, none changing whose request it is");
    assert!(made_for > 0 && kept > 0, "{made_for} {kept}");
}

// ── Every vector is a transition of the reference interpreter ──

/// Given what a vector holds (the ticket before, the event, the part of the organization, the
/// time), the machine answers what the vector says: `start` and `next` read nothing else.
#[test]
fn the_machine_answers_each_vector_again() {
    for path in files() {
        with(&path, |m, _, made| {
            for v in made {
                let s = &v.stepped;
                let answer = match (&s.input, &s.before) {
                    (Input::Submit(sub), None) => m.start(sub, s.comment.as_deref(), &s.part, s.now),
                    (Input::Event(ev), Some(t)) => m.next(t, ev, s.comment.as_deref(), s.seen.as_ref(), &s.part, s.now),
                    _ => panic!("{path}: {}", v.from),
                };
                let again = Stepped { answer, ..s.clone() };
                assert_eq!(again.to_json(&m.names).compact(), s.to_json(&m.names).compact(), "{path}: {}", v.from);
            }
        });
    }
}

/// The rows of the scenarios are among the vectors as `ringi run` shows them: the same ticket,
/// event, part and answer, row by row.
#[test]
fn the_rows_of_the_scenarios_are_vectors() {
    for path in files() {
        let (code, out, _) = cli(&["run", &path, "--format", "json"]);
        assert_eq!(code, 0, "{path}");
        let ran = json::parse(&out).unwrap();
        let all = lines(&path, Lang::En);
        // a line without where it comes from: the transition alone
        let alone = |j: &Json| Json::Obj(j.as_obj().unwrap().iter().filter(|(k, _)| !["request", "from"].contains(&k.as_str())).cloned().collect()).compact();
        let transitions: std::collections::HashSet<String> = all.iter().map(alone).collect();
        let mut rows = 0;
        for sc in ran.get("scenarios").and_then(Json::as_arr).unwrap() {
            let name = text(sc, "scenario");
            for row in sc.get("rows").and_then(Json::as_arr).unwrap() {
                rows += 1;
                let step = row.get("step").unwrap().compact();
                // a row another scenario made the same is one vector
                assert!(transitions.contains(&step), "{path}: scenario {name}, line {}", row.get("line").unwrap().compact());
                let from = format!("scenario {name}, line {}", row.get("line").unwrap().compact());
                if let Some(v) = all.iter().find(|j| text(j, "from") == from) {
                    assert_eq!(alone(v), step, "{path}: {from}");
                }
            }
        }
        assert!(rows > 0, "{path}");
        // and no two vectors are the same transition
        assert_eq!(transitions.len(), all.len(), "{path}");
    }
}

// ── What the vectors are made from (PLAN C3.4) ──

/// Where the vectors of a request come from, without the scenario's name and the numbers.
fn sources(path: &str) -> Vec<String> {
    with(path, |_, _, made| {
        let mut out: Vec<String> = Vec::new();
        for v in made {
            let from = match v.from.strip_prefix("scenario ") {
                Some(rest) => match rest.split_once(": ") {
                    Some((_, what)) => what.to_string(),
                    None if rest.contains(", made on ") => "made before closed days".to_string(),
                    None => "a row".to_string(),
                },
                None => v.from.split([' ', ':']).next().unwrap().to_string(),
            };
            if !out.contains(&from) {
                out.push(from);
            }
        }
        out
    })
}

/// The answers of the vectors of `path` that come from `from` (a prefix), each `event -> answer`.
fn answers(path: &str, from: &str) -> Vec<String> {
    with(path, |m, _, made| made.iter().filter(|v| v.from.starts_with(from)).map(|v| v.listed(m).rsplit_once(": ").unwrap().1.to_string()).collect())
}

/// The vectors are made from the scenarios' rows, the edges of the quorums, the limit of the
/// send-backs, requests made before closed days, every person as the requester, the proxies, a
/// ticket across the versions of the organization, and the edges of the form's numbers and rows.
#[test]
fn the_vectors_come_from_each_source() {
    let desk = sources("tests/machine/desk.ringi");
    for from in ["a row", "approvals", "rejections", "the requester", "sent back", "sent back to the limit", "paused", "retracted", "an administrator", "clocks", "made before closed days", "requester", "across", "edge"] {
        assert!(desk.iter().any(|s| s == from), "{from}: {desk:?}");
    }
    // the proxies of the organization's data are of the requests their rows name
    assert!(sources("examples/leave.ringi").iter().any(|s| s == "proxy"), "{:?}", sources("examples/leave.ringi"));
    assert!(sources("examples/expense.ringi").iter().any(|s| s == "proxy"));
    // an agent's and a flow's answers, where a stage asks one
    let flows = sources("tests/machine/flows.ringi");
    assert!(flows.iter().any(|s| s == "flows"), "{flows:?}");
    // the rows of the examples laid over a scenario's form
    let expense = sources("examples/expense.ringi");
    assert!(expense.iter().any(|s| s.starts_with("examples")), "{expense:?}");
}

/// What a resubmit and an edit make empty (DESIGN 7.3), each once for the request. On the ticket
/// the supplies request's first scenario sends back: each field the requester gives made empty (the
/// kind, the price and the quantity, which the form asks for on every request, and the contact,
/// which it asks for while a vendor is named, are refused), a name the form does not have, a
/// computed field, one of the approvers', and the vendor given a value too (each refused: the
/// vendor, as it could be made empty alone, by `clear` naming it only), the kind changed to a
/// service, which drops the vendor and the contact, and the same with the vendor given its value,
/// which is refused (a field the form does not show takes no value, and what the event gives is not
/// dropped). At its first stage, the edits of the approver
/// it waits for: each field they give or edit made empty (the kind is refused), one of the
/// requester's they do not edit, a name the form does not have, and the vendor given a value too.
#[test]
fn the_vectors_of_fields_made_empty() {
    let path = "tests/machine/supplies.ringi";
    let sc = "scenario a_kind_changed_on_resubmit, after line 46";
    let again = |to: &str| format!("resubmit by e1201 at 2026-11-17 10:00 -> {to}");
    let edit = |to: &str| format!("edit by e1101 at 2026-11-16 10:00 -> {to}");
    let (no, on) = ("refused (form)", "first, in progress");
    assert_eq!(answers(path, &format!("{sc}: made again, with fields made empty")), [no, no, no, on, no, on, no, no, no, no, on, no].map(again));
    assert_eq!(answers(path, &format!("{sc}: edited, with fields made empty")), [no, on, on, on, on, no, no, no].map(edit));
    // what each names: the fields made empty, and the values given with them
    with(path, |_, _, made| {
        let named: Vec<String> = made
            .iter()
            .filter(|v| v.from.ends_with("with fields made empty"))
            .map(|v| match &v.stepped.input {
                Input::Event(Event::Resubmit { form, clear, .. } | Event::Edit { fields: form, clear, .. }) => {
                    format!("{}{}", clear.join(" "), form.iter().map(|(n, _)| format!(" ({n} given)")).collect::<String>())
                }
                other => panic!("{}", other.word()),
            })
            .collect();
        let want = [
            "kind", "price", "quantity", "vendor", "contact", "note", "no_such_field", "total", "label", "vendor (vendor given)", " (kind given)", " (kind given) (vendor given)",
            "kind", "vendor", "note", "label", "checked", "price", "no_such_field", "vendor (vendor given)",
        ];
        assert_eq!(named, want);
        // the kind changed, and the vendor made empty, drop the vendor and the contact, and the
        // stage accounting answers for a vendor
        for v in made.iter().filter(|v| v.from.ends_with("with fields made empty")) {
            let Ok((t, fx)) = &v.stepped.answer else { continue };
            let cleared: Vec<String> = fx.iter().find_map(|e| if let What::Record(r) = &e.what { Some(r.cleared.clone()) } else { None }).unwrap();
            if cleared.contains(&"vendor".to_string()) {
                assert_eq!(cleared, ["vendor", "contact"], "{}", v.from);
                assert!(t.stages.iter().all(|s| s.name != "check"), "{}", v.from);
            }
        }
    });
}

/// The edges of a quorum: of the board's five with two needed, the second approval moves the
/// ticket on, and the fourth rejection ends it (the quorum is then out of reach).
#[test]
fn the_edges_of_a_quorum() {
    let approvals = answers("tests/machine/desk.ringi", "scenario approved_by_a_quorum, after line 43: approvals");
    assert_eq!(approvals, ["approve by e0001 at 2026-11-16 11:00 -> board, in progress", "approve by e0201 at 2026-11-16 11:00 -> decide, in progress"]);
    let rejections = answers("tests/machine/desk.ringi", "scenario approved_by_a_quorum, after line 43: rejections");
    assert_eq!(rejections.len(), 4, "{rejections:?}");
    assert!(rejections[..3].iter().all(|r| r.ends_with("-> board, in progress")) && rejections[3].ends_with("-> rejected"), "{rejections:?}");
}

/// The limit of the send-backs: the request allows two, the second is taken and the third is
/// refused (the first is the vector of the send-back before them).
#[test]
fn the_limit_of_the_send_backs() {
    let from = "scenario approved_by_a_quorum, after line 42: sent back to the limit";
    assert_eq!(answers("tests/machine/desk.ringi", from), ["send back by e1101 at 2026-11-17 10:00 -> with requester", "resubmit by e1201 at 2026-11-18 10:00 -> first, in progress", "send back by e1101 at 2026-11-18 10:00 -> refused (limit)"]);
    with("tests/machine/desk.ringi", |_, _, made| {
        // the tickets the two were given: sent back once, and twice
        let backs: Vec<u32> = made.iter().filter(|v| v.from == from && v.stepped.input.word() == "send back").map(|v| v.stepped.before.as_ref().unwrap().backs).collect();
        assert_eq!(backs, [1, 2]);
    });
}

/// A request made before closed days: its clocks are set past them, and ring there, the reminder
/// of the one the stage goes up to among them.
#[test]
fn a_request_made_before_closed_days() {
    // 2026-12-28 is the last day before the office closes for the new year
    let made = answers("examples/leave.ringi", "scenario a_long_paid_leave, made on 2026-12-28");
    assert_eq!(made, ["submit by e1203 at 2026-12-28 09:00 -> manager, in progress", "remind at 2027-01-04 09:00 -> manager, in progress", "late at 2027-01-06 00:00 -> manager, in progress", "remind at 2027-01-07 09:00 -> manager, in progress", "late at 2027-01-09 00:00 -> expired"]);
}

/// A stage that waits for a later one (`same approver approves once, at the last stage`; DESIGN
/// 5.12): at the later stage, the vectors hold what comes of its being answered by another, an
/// approver an administrator added or handed it to, and the one it went up to when it was late,
/// each of whom the stage that waited starts after; a send-back to the stage that waited, which is
/// answered there; and the retraction of the approval that did it.
#[test]
fn the_vectors_of_a_stage_that_waits() {
    let last = "tests/machine/last.ringi";
    // the line of the scenario's row `n` (from 1), as the vectors name where they branch from
    let src = std::fs::read_to_string(format!("{}/{last}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let row = |n: usize| src.lines().position(|l| l == "scenario one_answer_after_the_stage_between").unwrap() + 4 + n;
    // `first` waits for `second`, and the ticket is at `second` after the second row
    let at_second = format!("scenario one_answer_after_the_stage_between, after line {}", row(2));
    assert_eq!(
        answers(last, &format!("{at_second}: late, and approved where it went up to")),
        ["approve by e0101 at 2026-11-20 01:00 -> first, in progress", "approve by e3101 at 2026-11-20 01:00 -> decide, in progress", "approve by e0101 at 2026-11-20 01:00 -> approved"]
    );
    let admin = answers(last, &format!("{at_second}: an administrator"));
    assert_eq!(admin.iter().filter(|a| *a == "approve by e0001 at 2026-11-16 11:00 -> first, in progress").count(), 2, "{admin:?}");
    let back = answers(last, &format!("{at_second}: sent back"));
    assert!(back.contains(&"send back by e3101 at 2026-11-16 11:00 -> first, in progress".to_string()) && back.contains(&"approve by e3101 at 2026-11-16 11:00 -> middle, in progress".to_string()), "{back:?}");
    assert_eq!(answers(last, &format!("scenario one_answer_after_the_stage_between, after line {}: retracted", row(3))), ["retract by e3101 at 2026-11-16 12:00 -> second, in progress"]);
    with(last, |_, _, made| {
        use ringi::engine::What;
        let did = |d: &str| made.iter().filter(|v| v.stepped.answer.as_ref().is_ok_and(|(_, fx)| fx.iter().any(|e| matches!(&e.what, What::Record(r) if r.did == d)))).count();
        // each of the three lines of the history is among the transitions, and so is a ticket a
        // stage of which came again
        assert!(did("deferred") > 50 && did("deferral ended") > 10, "{} {}", did("deferred"), did("deferral ended"));
        let again = made.iter().filter(|v| v.stepped.before.as_ref().is_some_and(|t| (0..t.stages.len()).any(|i| !t.live(i)))).count();
        assert!(again > 10, "{again}");
        // every note of them is one of the four the machine writes
        for v in made {
            let Ok((_, fx)) = &v.stepped.answer else { continue };
            for e in fx {
                if let What::Record(r) = &e.what
                    && matches!(r.did, "deferred" | "deferral ended")
                    && let Some(n) = &r.note
                {
                    assert!(n.starts_with("the stage it waited for ") && (n.ends_with(" alone") || n.ends_with("is not on the route") || n.ends_with("waits too") || n.contains("was answered without ")), "{}: {n}", v.from);
                }
            }
        }
    });
}

/// The edges of the numbers a requester writes, of what the form works out of them, and of the
/// number of rows: the last value taken and the first refused, each way.
#[test]
fn the_edges_of_the_form() {
    let edges = answers("tests/machine/ranges.ringi", "edge of ");
    let (taken, refused): (Vec<&String>, Vec<&String>) = edges.iter().partition(|e| e.ends_with("in progress"));
    assert!(refused.iter().all(|e| e.ends_with("-> refused (form)")), "{edges:?}");
    assert_eq!((taken.len(), refused.len()), (7, 7), "{edges:?}");
    with("tests/machine/ranges.ringi", |_, _, made| {
        // a number written (`>0 <10`), a number of a row with what the row works out of it (`fee
        // = price * 2`, under 1,500 JPY), the rows with what is worked out of their sum, and the
        // number of rows (`count >=2 <=3`)
        let edge = |from: &str| {
            let v = made.iter().find(|v| v.from == format!("edge of {from}")).unwrap_or_else(|| panic!("{from}: {:?}", made.iter().map(|v| &v.from).collect::<Vec<_>>()));
            v.stepped.answer.as_ref().err().map(|r| r.text.en.clone())
        };
        assert_eq!(edge("rate, going up: 9"), None);
        assert_eq!(edge("rate, going up: 10").as_deref(), Some("`10` is outside the range of `rate` (`>0 <10`)"));
        assert_eq!(edge("rate, going down: 1"), None);
        assert_eq!(edge("rate, going down: 0").as_deref(), Some("`0` is outside the range of `rate` (`>0 <10`)"));
        assert_eq!(edge("lines.price, going up: 749JPY"), None);
        assert_eq!(edge("lines.price, going up: 750JPY").as_deref(), Some("the `lines[0].fee` the form works out is outside its range (`<1500JPY`); change `price` (`lines`)"));
        assert_eq!(edge("the rows of lines: 3 rows, price 251JPY in the last"), None);
        assert_eq!(edge("lines.price, going down: 1JPY"), None);
        assert_eq!(edge("lines.price, going down: 0JPY").as_deref(), Some("`0JPY` is outside the range of `lines[0].price` (`>=1JPY <=1000JPY`)"));
        assert_eq!(edge("the rows of lines: 3 rows, price 252JPY in the last").as_deref(), Some("the `double` the form works out is outside its range (`>20JPY <=3000JPY`); change `price` (`lines`)"));
        assert_eq!(edge("the number of rows of lines: 3 rows"), None);
        assert_eq!(edge("the number of rows of lines: 4 rows").as_deref(), Some("`lines` has 4 rows, past the limit of its rows (`count >=2 <=3`)"));
        assert_eq!(edge("the number of rows of lines: 2 rows"), None);
        assert_eq!(edge("the number of rows of lines: 1 row").as_deref(), Some("`lines` has 1 row, short of the least of its rows (`count >=2 <=3`)"));
    });
    // the leave request: 20 days are taken and 21 are not, 1 is and 0 is not
    assert_eq!(answers("examples/leave.ringi", "edge of days, going up").iter().map(|a| a.rsplit_once("-> ").unwrap().1).collect::<Vec<_>>(), ["manager, in progress", "refused (form)"]);
    assert_eq!(answers("examples/leave.ringi", "edge of days, going down").iter().map(|a| a.rsplit_once("-> ").unwrap().1).collect::<Vec<_>>(), ["manager, in progress", "refused (form)"]);
}

/// Who may submit: every person of the organization as the requester, the proxies on the first
/// and the last day of their row and the days around them, and a request made again after the
/// organization changed.
#[test]
fn the_ones_who_submit() {
    // 24 people, less the scenario's own requester, whose request is the scenario's first row
    let people = answers("examples/leave.ringi", "requester ");
    assert_eq!(people.len(), 23);
    let refused: Vec<&str> = people.iter().filter(|p| p.ends_with("refused (submit)")).map(|p| p.split(' ').nth(2).unwrap()).collect();
    assert_eq!(refused, ["e0001", "e0201", "e0202", "e0203", "e0204"]);
    let proxy = answers("examples/leave.ringi", "proxy e1201 for e1203");
    let (made, said): (Vec<&String>, Vec<&String>) = proxy.iter().partition(|p| p.starts_with("submit "));
    assert_eq!(
        made.iter().map(|p| p.split(" at ").nth(1).unwrap()).collect::<Vec<_>>(),
        ["2026-11-30 09:00 -> refused (person)", "2026-12-01 09:00 -> manager, in progress", "2026-12-31 09:00 -> manager, in progress", "2027-01-01 09:00 -> refused (person)"]
    );
    // a comment on the ticket made on the first day of the proxy, and the ticket taken back by the
    // one who made it; the ticket sent back, the requester's comment on it, and the one who made it
    // discarding it
    assert_eq!(
        said,
        [
            "comment by e1101 at 2026-12-01 09:00 -> manager, in progress",
            "withdraw by e1201 at 2026-12-01 09:00 -> with requester",
            "send back by e1101 at 2026-12-01 09:00 -> with requester",
            "comment by e1203 at 2026-12-01 09:00 -> with requester",
            "discard by e1201 at 2026-12-01 09:00 -> withdrawn"
        ]
    );
    // one who is no proxy of theirs
    assert_eq!(answers("examples/leave.ringi", "proxy e0001 for e1203"), ["submit by e0001 at 2026-11-16 09:00 -> refused (person)"]);
    // across the versions: the ticket goes on with the organization of the day, and one who left
    // does not make theirs again
    let across = answers("examples/leave.ringi", "across the versions of the organization");
    assert!(across.contains(&"resubmit by e1203 at 2027-04-01 09:00 -> manager, in progress".to_string()), "{across:?}");
    assert!(across.contains(&"resubmit by e9001 at 2027-04-01 09:00 -> refused (person)".to_string()), "{across:?}");
}

/// What people write on a ticket (DESIGN 7.2), each once for the request: the request made with a
/// comment and with one too long; at the first stage a scenario comes to, a comment by the approver
/// and by the requester, an empty one, the longest one and one a character past it, an approval and
/// a send-back with a comment and an approval with one too long; a comment on a ticket that has
/// ended, and one on a ticket that has moved on from the revision it was written on.
#[test]
fn what_people_write_is_in_the_vectors() {
    let path = "examples/leave.ringi";
    assert_eq!(answers(path, "scenario a_long_paid_leave, made with a comment"), ["submit by e1203 at 2026-11-16 09:00 -> manager, in progress", "submit by e1203 at 2026-11-16 09:00 -> refused (comment)"]);
    let first = answers(path, "scenario a_long_paid_leave, after line 47: comments");
    assert_eq!(
        first,
        [
            "comment by e1101 at 2026-11-16 09:00 -> manager, in progress",
            "comment by e1203 at 2026-11-16 09:00 -> manager, in progress",
            "comment by e1101 at 2026-11-16 09:00 -> refused (comment)",
            "comment by e1101 at 2026-11-16 09:00 -> manager, in progress",
            "comment by e1101 at 2026-11-16 09:00 -> refused (comment)",
            "approve by e1101 at 2026-11-16 09:00 -> head, in progress",
            "send back by e1101 at 2026-11-16 09:00 -> with requester",
            "approve by e1101 at 2026-11-16 09:00 -> refused (comment)"
        ]
    );
    assert_eq!(answers(path, "scenario a_long_paid_leave, after it ended"), ["comment by e1203 at 2026-11-19 10:00 -> refused (state)"]);
    assert_eq!(answers(path, "scenario a_long_paid_leave, after line 48: a comment, which says no revision"), ["comment by e1101 at 2026-11-16 17:00 -> head, in progress"]);
}

/// A request that asks a reason with a rejection and a send-back (`comment required for`, DESIGN
/// 7.2): each given with none, with an empty comment and with white space alone (U+0085 alone among
/// it) is refused (`comment`), once for the request, before who gives it is asked, and each given
/// with a character alone that is no white space (U+FEFF, U+001C) is taken; wherever else the
/// vectors give one, it is given with a reason, which the line of the history keeps, and taken. A
/// request that asks none takes them with none.
#[test]
fn a_reason_asked_is_in_the_vectors() {
    let leave = "examples/leave.ringi";
    let refused = |e: &str| format!("{e} by e1101 at 2026-11-16 09:00 -> refused (comment)");
    let none = answers(leave, "scenario a_long_paid_leave, after line 47: a reason asked, and none given");
    assert_eq!(none, [vec![refused("reject"); 4], vec![refused("send back"); 4]].concat());
    // the comments given: none, an empty one, white space alone
    let given = |from: &str| -> Vec<Option<String>> { with(leave, |_, _, made| made.iter().filter(|v| v.from.ends_with(from)).map(|v| v.stepped.comment.clone()).collect()) };
    let four = [None, Some(String::new()), Some(" \u{3000}\n".to_string()), Some("\u{85}".to_string())];
    assert_eq!(given(": a reason asked, and none given"), [four.clone(), four].concat());
    // a character that is no white space is a reason: the rejection ends the ticket, the send-back
    // gives it back to the requester
    let some = answers(leave, "scenario a_long_paid_leave, after line 47: a reason asked, given in a character that is no white space");
    let taken = ["reject by e1101 at 2026-11-16 09:00 -> rejected", "send back by e1101 at 2026-11-16 09:00 -> with requester"];
    assert_eq!(some, [taken[0], taken[0], taken[1], taken[1]]);
    let two = [Some("\u{feff}".to_string()), Some("\u{1c}".to_string())];
    assert_eq!(given(": a reason asked, given in a character that is no white space"), [two.clone(), two].concat());
    // a rejection and a send-back given elsewhere, with a reason the history keeps
    let kept = |from: &str, did: &str| -> Vec<Option<String>> {
        with(leave, |_, _, made| {
            made.iter()
                .filter(|v| v.from == from)
                .filter_map(|v| v.stepped.answer.as_ref().ok())
                .flat_map(|(_, fx)| fx.iter().filter_map(|e| if let What::Record(r) = &e.what { (r.did == did).then(|| r.comment.clone()) } else { None }))
                .collect()
        })
    };
    let reason = Some("The reason for it".to_string());
    assert_eq!(kept("scenario a_long_paid_leave, after line 47: rejections", "rejected"), [reason.clone()]);
    assert_eq!(kept("scenario a_long_paid_leave, after line 47: sent back", "sent back"), [reason]);
    // a request that asks none: its rejections, with no comment, are taken, and nothing asks a reason
    with("tests/machine/desk.ringi", |_, _, made| {
        let rejections: Vec<_> = made.iter().filter(|v| v.from.ends_with(": rejections")).collect();
        assert!(!rejections.is_empty() && rejections.iter().all(|v| v.stepped.comment.is_none() && v.stepped.answer.is_ok()));
        assert!(!made.iter().any(|v| v.from.contains("a reason asked")));
    });
}

/// The comments of the vectors of `path` that come from `from`: each by its writer, with the ones
/// it is told to (DESIGN 7.4).
fn told(path: &str, from: &str) -> Vec<(String, Vec<String>)> {
    with(path, |_, _, made| {
        made.iter()
            .filter(|v| v.from == from && v.stepped.input.word() == "comment")
            .map(|v| {
                let to = v.stepped.answer.as_ref().ok().and_then(|(_, fx)| fx.iter().find_map(|e| if let What::Notify { to, .. } = &e.what { Some(to.clone()) } else { None }));
                (v.stepped.input.by().unwrap_or_default().to_string(), to.unwrap_or_default())
            })
            .collect()
    })
}

/// The ones each vector of `path` from `from` with the event `word` leaves the ticket sent back by
/// (DESIGN 7.1), where it is taken.
fn sent_back_by(path: &str, from: &str, word: &str) -> Vec<Option<String>> {
    with(path, |_, _, made| made.iter().filter(|v| v.from == from && v.stepped.input.word() == word).filter_map(|v| v.stepped.answer.as_ref().ok().map(|(t, _)| t.sent_back_by.clone())).collect())
}

/// The one who sent the ticket back (DESIGN 7.1, 7.3, 7.4), each once for the request: the ticket
/// holds them while it is with the requester, and nobody once it is made again; the requester's
/// comment goes to them, and theirs to the requester. On a ticket made in another's name, the
/// requester's goes to the one who made it and to the one who sent it back. A ticket the requester
/// took back themselves holds nobody, and the requester's comment then goes to nobody. A proxy who
/// sends it back is held as themselves, and the line of the history says whose place they answered
/// in.
#[test]
fn the_one_who_sent_it_back_is_in_the_vectors() {
    let path = "examples/leave.ringi";
    let from = "scenario a_long_paid_leave, after line 47: sent back";
    let one = |by: &str, to: &[&str]| (by.to_string(), to.iter().map(|x| x.to_string()).collect::<Vec<_>>());
    assert_eq!(told(path, from), [one("e1101", &["e1203"]), one("e1203", &["e1101"])]);
    assert_eq!(sent_back_by(path, from, "send back"), [Some("e1101".to_string())]);
    assert_eq!(sent_back_by(path, from, "resubmit"), [None]);
    assert_eq!(sent_back_by(path, from, "discard"), [None]);
    assert_eq!(told(path, "proxy e1201 for e1203: sent back"), [one("e1203", &["e1201", "e1101"])]);
    let from = "scenario a_long_paid_leave, after line 47: the requester";
    assert_eq!(sent_back_by(path, from, "withdraw"), [None]);
    assert_eq!(told(path, from), [one("e1203", &[])]);
    // the proxy of an approver sends the ticket back in the approver's place
    with("examples/expense.ringi", |_, _, made| {
        let v = made.iter().find(|v| v.from.ends_with(": sent back by the proxy")).expect("a send-back by a proxy");
        assert_eq!(v.from, "proxy e1101 for e3101: sent back by the proxy");
        let (t, fx) = v.stepped.answer.as_ref().unwrap();
        assert_eq!((t.status.word(), t.sent_back_by.as_deref()), ("with requester", Some("e1101")));
        let line = fx.iter().find_map(|e| if let What::Record(r) = &e.what { Some(r) } else { None }).unwrap();
        assert_eq!((line.did, line.by.as_deref(), line.proxy_of.as_deref()), ("sent back", Some("e1101"), Some("e3101")));
    });
}

/// The lines of the history the vectors of `path` from `from` write: each `(did, by, proxy of,
/// once with)`, `by` empty where nobody did it.
fn history(path: &str, from: &str) -> Vec<(String, String, Option<String>, Option<String>)> {
    with(path, |_, _, made| {
        made.iter()
            .filter(|v| v.from == from)
            .filter_map(|v| v.stepped.answer.as_ref().ok())
            .flat_map(|(_, fx)| fx.iter().filter_map(|e| if let What::Record(r) = &e.what { Some((r.did.to_string(), r.by.clone().unwrap_or_default(), r.proxy_of.clone(), r.once_with.clone())) } else { None }))
            .collect()
    })
}

/// Whom a proxy acts for is on every line of the history their events write (DESIGN 5.8, 7.4). On
/// the requester's side, the one who made a request in another's name takes it back and discards
/// it for them; on the approvers', a proxy rejects (and ends the ticket), pauses, resumes, edits
/// and takes back an approval in the seat they answer for; and a stage done with an approval a
/// proxy gave (`same approver approves once`) is done by the proxy, for the approver whose seat
/// it was. What one does for themselves says nobody.
#[test]
fn a_proxy_says_whom_they_act_for() {
    let line = |did: &str, by: &str, of: Option<&str>| (did.to_string(), by.to_string(), of.map(str::to_string), None);
    let leave = "examples/leave.ringi";
    assert_eq!(history(leave, "proxy e1201 for e1203: taken back by the one who made it"), [line("withdrawn", "e1201", Some("e1203"))]);
    assert_eq!(history(leave, "proxy e1201 for e1203: sent back, and discarded by the one who made it"), [line("ticket withdrawn", "e1201", Some("e1203"))]);
    let own = history(leave, "scenario a_long_paid_leave, after line 47: the requester");
    assert!(own.contains(&line("withdrawn", "e1203", None)), "{own:?}");
    let once = "tests/machine/once.ringi";
    assert_eq!(history(once, "proxy e1101 for e3101: rejected by the proxy"), [line("rejected", "e1101", Some("e3101")), line("ticket rejected", "e1101", Some("e3101"))]);
    assert_eq!(history(once, "proxy e1101 for e3101: approved and taken back by the proxy"), [line("retracted", "e1101", Some("e3101"))]);
    let last = "tests/machine/last.ringi";
    assert_eq!(history(last, "proxy e1101 for e3101: paused by the proxy"), [line("paused", "e1101", Some("e3101")), line("resumed", "e1101", Some("e3101"))]);
    assert_eq!(history(last, "proxy e1101 for e3101: edited by the proxy"), [line("edited", "e1101", Some("e3101"))]);
    // the head of the department alone answers the second stage, right after the first, which was
    // handed to them and approved by their proxy: the second is done with the proxy's approval
    for path in [once, "examples/expense.ringi"] {
        let done = history(path, "proxy e1101 for e3101: a stage handed to e3101");
        let approved: Vec<_> = done.iter().filter(|l| l.0 == "approved").collect();
        assert_eq!(approved, [&line("approved", "e1101", Some("e3101")), &("approved".to_string(), "e1101".to_string(), Some("e3101".to_string()), Some("first".to_string()))], "{path}");
    }
}

// ── The names and the language ──

/// `json` without the sentences for people and the values people write: the people of the
/// organization's part by their IDs, without the names their data writes (DESIGN 7.5).
fn names(json: &Json) -> Json {
    const LEFT_OUT: [&str; 16] = ["text", "text_ja", "note", "form", "fields", "args", "outputs", "request", "value", "way", "key", "part", "proxies", "reason", "from", "comment"];
    match json {
        Json::Obj(pairs) => Json::Obj(
            pairs
                .iter()
                .filter(|(k, _)| !LEFT_OUT.contains(&k.as_str()))
                .map(|(k, v)| match (k.as_str(), v) {
                    ("people", Json::Arr(people)) => (k.clone(), Json::Arr(people.iter().map(|p| p.get("id").cloned().unwrap_or(Json::Null)).collect())),
                    _ => (k.clone(), names(v)),
                })
                .collect(),
        ),
        Json::Arr(items) => Json::Arr(items.iter().map(names).collect()),
        other => other.clone(),
    }
}

/// The names of a line are the ASCII ones (DESIGN 3.4): the vectors of a request in Japanese
/// names hold no stage, book, flow or master by its Japanese name.
#[test]
fn the_names_of_a_line_are_ascii() {
    for name in EXAMPLES {
        let path = format!("examples/{name}.ja.ringi");
        for j in lines(&path, Lang::En) {
            let n = names(&j).compact();
            assert!(n.is_ascii(), "{path}: {}: {n}", text(&j, "from"));
            // the ways and their keys are names too
            let stages = j.get("answer").and_then(|a| a.get("ok")).and_then(|a| a.get("ticket")).and_then(|t| t.get("stages")).and_then(Json::as_arr);
            for part in stages.into_iter().flatten().flat_map(|s| s.get("parts").and_then(Json::as_arr).unwrap()) {
                assert!(text(part, "way").is_ascii() && text(part, "key").is_ascii(), "{path}: {}", part.compact());
            }
        }
    }
}

/// The desk request in Japanese names has the vectors of the one in English names, line by line:
/// the same events at the same times, the same tickets, the same things to do, by the same names.
#[test]
fn a_request_in_japanese_names_has_the_same_vectors() {
    let (en, ja) = (lines("tests/machine/desk.ringi", Lang::En), lines("tests/machine/desk.ja.ringi", Lang::En));
    assert_eq!(en.len(), ja.len());
    for (e, j) in en.iter().zip(&ja) {
        let n = names(j).compact();
        assert_eq!(names(e).compact(), n, "{}", text(e, "from"));
        assert!(n.is_ascii(), "{}: {n}", text(j, "from"));
    }
}

/// Each example in Japanese names has as many vectors as the one in English names, the proxies'
/// too: the two read the same organization, whose `proxies.csv` names both by their ASCII names
/// (DESIGN 5.8). The lists are the goldens `the_vectors_of_each_request_listed` holds to.
#[test]
fn an_example_in_japanese_names_has_as_many_vectors() {
    let count = |file: &str| {
        let list = std::fs::read_to_string(format!("tests/golden/vectors/{file}.txt")).unwrap();
        (list.lines().count(), list.lines().filter(|l| l.starts_with("proxy ")).count())
    };
    for name in EXAMPLES {
        assert_eq!(count(name), count(&format!("{name}.ja")), "{name}");
    }
}

/// `--lang` changes the sentences of the refusals and nothing else.
/// A refusal says why in both languages, whatever language the vectors are asked in (DESIGN
/// 12.4): `text` in English, `text_ja` in Japanese, so the vectors are the same bytes in either.
#[test]
fn a_refusal_is_said_in_both_languages() {
    let japanese = |t: &str| t.chars().any(|c| ('\u{3040}'..='\u{30ff}').contains(&c));
    for path in ["examples/leave.ringi", "examples/expense.ja.ringi", "tests/machine/desk.ringi"] {
        let (en, ja) = (lines(path, Lang::En), lines(path, Lang::Ja));
        assert_eq!(en, ja, "{path}");
        let mut refusals = 0;
        for j in &en {
            let Some(err) = j.get("answer").and_then(|a| a.get("err")) else { continue };
            refusals += 1;
            let (t, t_ja) = (text(err, "text"), text(err, "text_ja"));
            assert!(!t.is_empty() && t != t_ja && japanese(t_ja), "{path}: {}: {t} / {t_ja}", text(j, "from"));
        }
        assert!(refusals > 0, "{path}");
    }
}

// ── The command ──

/// `ringi vectors` on what it does not make vectors of: a request with no scenario and a file
/// that is no request (exit 2, with why), a request whose scenario does not go as written (exit
/// 1, the scenario's finding and no vector), a request that does not pass its check (the check's
/// findings).
#[test]
fn vectors_of_what_has_none() {
    let (code, out, err) = cli(&["vectors", "tests/machine/bare.ringi"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(err, "error: `tests/machine/bare.ringi` has no scenario: the vectors are made from the requester and the form of a scenario\n");
    let (code, _, err) = cli(&["vectors", "tests/machine/bare.ringi", "--lang", "ja"]);
    assert_eq!((code, err.as_str()), (2, "エラー: `tests/machine/bare.ringi` にシナリオがありません。ベクタは、シナリオの申請者とフォームから作ります\n"));
    let (code, out, err) = cli(&["vectors", "examples/shared/company.ringi"]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert!(err.starts_with("error: "), "{err}");
    let (code, out, err) = cli(&["vectors"]);
    assert_eq!((code, out.as_str(), err.as_str()), (2, "", "error: `ringi vectors` takes <file.ringi>\nusage: ringi vectors <file.ringi> [--list] [--org <dir>]\n"));
    // a scenario that does not go as written: its finding, and no vector
    let (code, out, _) = cli(&["vectors", "tests/mutants/E602_another_stage_than_the_row_writes.ringi"]);
    assert_eq!(code, 1);
    assert!(out.starts_with("error[E602]: tests/mutants/E602_another_stage_than_the_row_writes.ringi:") && !out.contains("{\""), "{out}");
    // a file that does not pass its check
    let (code, out, _) = cli(&["vectors", "tests/mutants/E004_a_cell_not_in_who.ringi"]);
    assert_eq!(code, 1);
    assert!(out.starts_with("error[E004]: "), "{out}");
}

/// `--org` reads the organization's data from another directory, as `check` and `run` do.
#[test]
fn vectors_with_another_organization() {
    let (code, out, err) = cli(&["vectors", "examples/leave.ringi", "--org", "examples/org", "--list"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(out, cli(&["vectors", "examples/leave.ringi", "--list"]).1);
    let (code, _, err) = cli(&["vectors", "examples/leave.ringi", "--org", "tests/machine"]);
    assert_eq!(code, 2);
    assert!(err.starts_with("error: the organization has no data"), "{err}");
}
