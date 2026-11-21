//! The decisions of X18, X19 and X20 (ringi's DESIGN 9.3), held to what each way a question can
//! come out comes to: what a gate answers for the approvers of a stage and for a requester at their
//! own request, a hold's expiry against the longest stretch it stays open, and what a request gives
//! a flow against what the flow takes. The checks over a project, with the languages answering
//! through their ports, are `crates/ritsu/tests/cross.rs` and the ledger's reproductions.

use ritsu_base::text::Text;
use ritsu_cross::ticket_flows::{Given, carried_text, fits, judge_answer, judge_given};
use ritsu_cross::ticket_gates::{APPROVERS, Approvers, Own, REQUESTER, SELF_APPROVAL, asked_of_approvers, asked_of_requester, judge_approvers, judge_own};
use ritsu_cross::ticket_holds::{Kept, Longest, judge, longest};
use ritsu_ports::{Allowance, Answer, Carried, FlowGiven, FlowPlace, Found, GateGiven, GateGivenValue, Lined, Spelled};
use ritsu_units::Unit;
use std::collections::BTreeSet;

// ── X18: the gate ──

#[test]
fn what_the_gate_is_asked() {
    let (holds, is, flag) = (GateGivenValue::HoldsPrincipal, GateGivenValue::IsPrincipal, GateGivenValue::Bool);
    // the approvers of a stage: one of them, who is not the requester, with what the route says
    for allowed in [false, true] {
        assert_eq!(asked_of_approvers(allowed), [GateGiven::resource(APPROVERS, holds(true)), GateGiven::resource(REQUESTER, is(false)), GateGiven::resource(SELF_APPROVAL, flag(allowed))]);
    }
    // a requester at their own request: where the route does not allow it, whether they are among
    // the approvers is left open, so the gate allows them nothing either way
    assert_eq!(asked_of_requester(false), [GateGiven::resource(REQUESTER, is(true)), GateGiven::resource(SELF_APPROVAL, flag(false))]);
    // where it does, they are among the approvers, as ringi leaves them
    assert_eq!(asked_of_requester(true), [GateGiven::resource(REQUESTER, is(true)), GateGiven::resource(APPROVERS, holds(true)), GateGiven::resource(SELF_APPROVAL, flag(true))]);
    // the names are the ones ringi hands the gate
    assert_eq!((REQUESTER, APPROVERS, SELF_APPROVAL), ("requester", "approvers", "self_approval"));
}

fn sometimes() -> Found<Allowance> {
    Found::Value(Allowance::Sometimes { allowed: Text::same("a"), denied: Text::same("d") })
}

#[test]
fn the_approvers_of_a_stage() {
    // allowed in every combination, or in some (a suspended member of the staff is denied): holds
    assert_eq!(judge_approvers(&Found::Value(Allowance::Always)), Approvers::Held);
    assert_eq!(judge_approvers(&sometimes()), Approvers::Held);
    // in none: every answer an approver gives is denied
    assert_eq!(judge_approvers(&Found::Value(Allowance::Never)), Approvers::Never);
    assert_eq!(judge_approvers(&Found::Undecided(Text::same("why"))), Approvers::Undecided(Text::same("why")));
}

#[test]
fn a_requester_at_their_own_request() {
    let (always, never, undecided) = (Found::Value(Allowance::Always), Found::Value(Allowance::Never), Found::Undecided(Text::same("why")));
    // a route that does not allow it: the gate allows it in no combination
    assert_eq!(judge_own(false, &never), Own::Held);
    // any combination it allows is one too many, and the example is the combination
    assert_eq!(judge_own(false, &sometimes()), Own::Allowed(Some(Text::same("a"))));
    assert_eq!(judge_own(false, &always), Own::Allowed(None));
    // a route that allows it: the gate allows it in some combination
    assert_eq!(judge_own(true, &always), Own::Held);
    assert_eq!(judge_own(true, &sometimes()), Own::Held);
    assert_eq!(judge_own(true, &never), Own::Never);
    // what sekisho does not decide is undecided either way
    assert_eq!(judge_own(false, &undecided), Own::Undecided(Text::same("why")));
    assert_eq!(judge_own(true, &undecided), Own::Undecided(Text::same("why")));
}

// ── X19: the hold ──

/// The open days of `weeks` weeks from a Monday (day 0), Saturdays and Sundays closed, without the
/// days of `closed`.
fn weekdays(weeks: i64, closed: &[i64]) -> Vec<i64> {
    (0..weeks * 7).filter(|d| d % 7 < 5 && !closed.contains(d)).collect()
}

#[test]
fn the_longest_a_hold_stays_open() {
    let open = weekdays(8, &[]);
    // ten business days from a Monday end on the Monday two weeks later: 15 days, both days in
    assert_eq!(longest(&open, 0, 0, 10), Some(Longest { submitted: 0, last: 14, days: 15 }));
    // from a Saturday, the same ten end a day sooner, counted from the Saturday: 14 days
    assert_eq!(longest(&open, 5, 5, 10), Some(Longest { submitted: 5, last: 18, days: 14 }));
    // over a range of days, the first day that makes the longest
    assert_eq!(longest(&open, 5, 20, 10), Some(Longest { submitted: 7, last: 21, days: 15 }));
    // one business day from a Friday is the Monday: four days, the longest of a week
    assert_eq!(longest(&open, 0, 6, 1), Some(Longest { submitted: 4, last: 7, days: 4 }));
    // a break makes it longer: the week from day 7 closed, ten business days from the Monday end
    // three weeks later
    let with_a_break = weekdays(8, &[7, 8, 9, 10, 11]);
    assert_eq!(longest(&with_a_break, 0, 13, 10), Some(Longest { submitted: 0, last: 21, days: 22 }));
    // a day in the break is nearer the same last day than the open day before it
    assert_eq!(longest(&with_a_break, 7, 11, 10), Some(Longest { submitted: 7, last: 25, days: 19 }));
    // the open days end before the round does: nothing to say from the days that are there
    assert_eq!(longest(&weekdays(1, &[]), 0, 6, 10), None);
    // from the last days of the data, the days before still count
    assert_eq!(longest(&weekdays(3, &[]), 0, 20, 10), Some(Longest { submitted: 0, last: 14, days: 15 }));
    // no day to submit on, and a round that takes no day
    assert_eq!(longest(&open, 9, 8, 10), None);
    assert_eq!(longest(&open, 0, 6, 0), None);
}

#[test]
fn a_hold_against_its_expiry() {
    const DAY: u64 = 86_400;
    // shorter than the expiry: held, in one calendar or in several
    assert_eq!(judge(15, 0, 16 * DAY, 1), Kept::Held);
    assert_eq!(judge(15, 0, 15 * DAY + 1, 1), Kept::Held);
    assert_eq!(judge(15, 0, 16 * DAY, 3), Kept::Held);
    // a hold whose expiry the clock reaches has expired: the very second does not hold
    assert_eq!(judge(15, 0, 15 * DAY, 1), Kept::Expires);
    assert_eq!(judge(15, 0, 7 * DAY, 1), Kept::Expires);
    // over several calendars the stretch is more than a ticket may take: undecided
    assert_eq!(judge(15, 0, 15 * DAY, 2), Kept::MayExpire);
    // calendars whose offsets are apart make the stretch longer by that much
    assert_eq!(judge(15, 3_600, 15 * DAY + 3_600, 2), Kept::MayExpire);
    assert_eq!(judge(15, 3_600, 15 * DAY + 3_601, 2), Kept::Held);
    // the largest numbers do not wrap
    assert_eq!(judge(i64::MAX, u64::MAX, u64::MAX, 1), Kept::Expires);
    assert_eq!(judge(-1, 0, 1, 1), Kept::Held);
}

// ── X20: the flow ──

fn yen(min: Option<i128>, max: Option<i128>) -> Carried {
    Carried::Num { written: "money[JPY]".into(), unit: Unit::money("JPY", None), min, max }
}

fn choice(name: &str, values: &[&str]) -> Carried {
    Carried::Enum { name: name.into(), values: values.iter().map(|v| Spelled { written: v.to_string(), public: v.to_string() }).collect() }
}

/// The value and the words of what does not fit; None where it fits, Err where it is undecided.
fn against(given: &Carried, takes: &Carried) -> Result<Option<(Option<String>, String, bool)>, String> {
    match fits(given, takes) {
        Answer::Holds => Ok(None),
        Answer::Fails(a) => Ok(Some((a.value, a.why.en, a.whatever))),
        Answer::Undecided(why) => Err(why.en),
    }
}

#[test]
fn what_an_input_takes() {
    let some = |v: &str, why: &str| Ok(Some((Some(v.to_string()), why.to_string(), false)));
    let types = |why: &str| Ok(Some((None, why.to_string(), true)));
    // a number: every value given is one taken
    assert_eq!(against(&yen(Some(1), Some(500_000)), &yen(Some(1), Some(500_000))), Ok(None));
    assert_eq!(against(&yen(Some(1), Some(500_000)), &yen(None, None)), Ok(None));
    // an end outside: the end is the example
    assert_eq!(against(&yen(Some(1), Some(1_000_000)), &yen(Some(1), Some(500_000))), some("1_000_000JPY", "the flow takes 500_000JPY or less"));
    assert_eq!(against(&yen(Some(0), Some(100)), &yen(Some(1), Some(500_000))), some("0JPY", "the flow takes 1JPY or more"));
    // an end the request does not write: the nearest value past what the flow takes
    assert_eq!(against(&yen(Some(1), None), &yen(Some(1), Some(500_000))), some("500_001JPY", "nothing holds the value from above, and the flow takes 500_000JPY or less"));
    assert_eq!(against(&yen(None, Some(9)), &yen(Some(1), Some(500_000))), some("0JPY", "nothing holds the value from below, and the flow takes 1JPY or more"));
    // each number as its side writes it: what the request gives in its unit, what the flow takes in
    // the flow's (`円` is `JPY`)
    let sign = Carried::Num { written: "money[円]".into(), unit: Unit::money("円", None), min: Some(1), max: Some(1_000_000) };
    assert_eq!(against(&sign, &yen(Some(1), Some(500_000))), some("1_000_000円", "the flow takes 500_000JPY or less"));
    // another unit is another type, whatever the ranges
    let usd = Carried::Num { written: "money[USD]".into(), unit: Unit::money("USD", None), min: Some(1), max: Some(2) };
    assert_eq!(against(&yen(Some(1), Some(2)), &usd), types("the request counts in money[JPY], and the flow in money[USD]"));
    // a count with a name is a number
    let items = Carried::Num { written: "count[items]".into(), unit: Some(Unit::count("items")), min: Some(1), max: Some(20) };
    assert_eq!(against(&items, &Carried::whole("number", Some(0), Some(100))), Ok(None));
    assert_eq!(against(&Carried::whole("number", Some(1), Some(4)), &items), Ok(None));
    // a choice: by the names of its values
    let kinds = choice("kind", &["travel", "expense", "purchase"]);
    assert_eq!(against(&choice("category", &["travel", "expense"]), &kinds), Ok(None));
    assert_eq!(against(&choice("category", &["travel", "gift"]), &kinds), some("gift", "the flow's enum has no such value (enum kind (travel, expense, purchase))"));
    // a choice is given by its name, which a string takes; a string is no choice
    assert_eq!(against(&kinds, &Carried::Str), Ok(None));
    assert_eq!(against(&Carried::Str, &kinds), types("the request gives any string, and the flow takes only the values of the enum (enum kind (travel, expense, purchase))"));
    // a date: by its days
    let days = |a: i64, b: i64| Carried::Date { min: Some(a), max: Some(b) };
    assert_eq!(against(&days(20_454, 20_818), &days(20_454, 21_183)), Ok(None));
    assert_eq!(against(&days(20_454, 21_184), &days(20_454, 21_183)), some("2028-01-01", "the flow takes 2027-12-31 or less"));
    let set: BTreeSet<i64> = [20_454, 20_455, 20_457].into();
    assert_eq!(against(&days(20_454, 20_455), &Carried::Days(set.clone())), Ok(None));
    assert_eq!(against(&days(20_454, 20_457), &Carried::Days(set.clone())), some("2026-01-03", "it is no day the flow takes"));
    assert_eq!(against(&Carried::Days(set.clone()), &days(20_454, 20_456)), some("2026-01-04", "it is no day the flow takes"));
    assert_eq!(
        against(&Carried::Date { min: Some(20_454), max: None }, &Carried::Days(set.clone())),
        Ok(Some((None, "nothing holds the day at an end, and the flow takes certain days only (date (3 days from 2026-01-01 to 2026-01-04))".to_string(), false)))
    );
    // `T?` takes what `T` does, and what is given as `T?` is compared as `T`
    assert_eq!(against(&yen(Some(1), Some(9)), &Carried::Opt(Box::new(yen(Some(1), Some(9))))), Ok(None));
    assert_eq!(against(&Carried::Opt(Box::new(Carried::Bool)), &Carried::Bool), Ok(None));
    // two types
    assert_eq!(against(&Carried::Bool, &Carried::Str), types("the request gives bool, and the flow takes string: the types differ"));
    assert_eq!(against(&Carried::Str, &yen(Some(1), Some(9))), types("the request gives string, and the flow takes money[JPY] >=1JPY <=9JPY: the types differ"));
    // what is not compared value by value
    assert_eq!(against(&Carried::Other("rows".into()), &Carried::Other("list[Item]".into())), Err("rows is not compared value by value".to_string()));
    assert_eq!(against(&Carried::Str, &Carried::Other("list[Item]".into())), Err("list[Item] is not compared value by value".to_string()));
    // as a message shows them
    assert_eq!(carried_text(&yen(Some(1), None)), "money[JPY] >=1JPY");
    assert_eq!(carried_text(&yen(Some(1), Some(10_000))), "money[JPY] >=1JPY <=10_000JPY");
    assert_eq!(carried_text(&Carried::Opt(Box::new(days(20_454, 20_454)))), "date >=2026-01-01 <=2026-01-01?");
}

fn given(carried: Found<Carried>, none: Found<Option<Text>>, may_be_less: Option<&str>) -> FlowGiven {
    let none = match none {
        Found::Value(n) => Found::Value(n.map(Lined::plain)),
        Found::Undecided(why) => Found::Undecided(why),
    };
    FlowGiven { name: "amount".into(), written: "amount".into(), carried, none, narrowed: None, may_be_less: may_be_less.map(Text::same), line: 7 }
}

#[test]
fn one_value_a_flow_is_given() {
    let (always, takes) = (Found::Value(None), yen(Some(1), Some(500_000)));
    let wide = || Found::Value(yen(Some(1), Some(1_000_000)));
    assert_eq!(judge_given(&given(Found::Value(yen(Some(1), Some(500_000))), always.clone(), None), &takes), Given::Held);
    // a value the input does not take
    let Given::Outside(a) = judge_given(&given(wide(), always.clone(), None), &takes) else { panic!("outside") };
    assert_eq!((a.value.as_deref(), a.why.en.as_str()), (Some("1_000_000JPY"), "the flow takes 500_000JPY or less"));
    // at a place whose condition ringi does not follow, the condition may hold it to less
    let Given::Undecided(why) = judge_given(&given(wide(), always.clone(), Some("the `when` of the `on` line")), &takes) else { panic!("undecided") };
    assert_eq!(why.en, "it looks as if it can give 1_000_000JPY (the flow takes 500_000JPY or less); but the `when` of the `on` line may hold the value to less, which ringi does not follow");
    // which changes nothing where the value fits, or where the types differ
    assert_eq!(judge_given(&given(Found::Value(yen(Some(1), Some(9))), always.clone(), Some("the `when` of the stage")), &takes), Given::Held);
    let Given::Outside(a) = judge_given(&given(Found::Value(Carried::Str), always.clone(), Some("the `when` of the stage")), &takes) else { panic!("types") };
    assert!(a.whatever && a.value.is_none(), "{a:?}");
    // a value that can be absent, where the input is not `T?`
    let empty = Found::Value(Some(Text::same("`amount` is `optional`, and a request may leave it empty")));
    assert_eq!(judge_given(&given(wide(), empty.clone(), None), &takes), Given::Lacking(Text::same("`amount` is `optional`, and a request may leave it empty")));
    // `T?` takes it, and the value is then compared
    let optional = Carried::Opt(Box::new(yen(Some(1), Some(1_000_000))));
    assert_eq!(judge_given(&given(wide(), empty.clone(), None), &optional), Given::Held);
    assert!(matches!(judge_given(&given(wide(), empty.clone(), None), &Carried::Opt(Box::new(takes.clone()))), Given::Outside(_)));
    // where the condition is not followed, it may be what gives the value
    let Given::Undecided(why) = judge_given(&given(wide(), empty, Some("the shared route")), &takes) else { panic!("undecided") };
    assert_eq!(why.en, "`amount` is `optional`, and a request may leave it empty; but the shared route may hold the value to less, which ringi does not follow");
    // what ringi does not say is undecided, with its reason
    assert_eq!(judge_given(&given(Found::Undecided(Text::same("why")), always, None), &takes), Given::Undecided(Text::same("why")));
    assert_eq!(judge_given(&given(wide(), Found::Undecided(Text::same("why")), None), &takes), Given::Undecided(Text::same("why")));
    // whether it has a value is not asked of an input that is `T?`
    assert!(matches!(judge_given(&given(wide(), Found::Undecided(Text::same("why")), None), &optional), Given::Held));
}

fn output(name: &str, carried: Carried) -> FlowPlace {
    FlowPlace { name: name.into(), ty: carried_text(&carried), optional: false, range: None, carried, line: 12 }
}

#[test]
fn what_an_agents_flow_answers() {
    let wrong = |outputs: &[FlowPlace]| judge_answer(outputs).map(|t| t.en);
    let verdict = || output("verdict", choice("Check", &["ok", "doubt"]));
    let reason = || output("reason", Carried::Str);
    // `verdict`, an enum of `ok` and `doubt`, and `reason`, a string; more outputs are the flow's own
    assert_eq!(wrong(&[verdict(), reason()]), None);
    assert_eq!(wrong(&[reason(), output("score", Carried::whole("number", None, None)), output("verdict", choice("Verdict", &["doubt", "ok"]))]), None);
    // one of the two is not there
    assert_eq!(wrong(&[reason()]).as_deref(), Some("it has no output `verdict`"));
    assert_eq!(wrong(&[verdict()]).as_deref(), Some("it has no output `reason`"));
    // a verdict with another value, or one fewer, is not what ringi reads
    assert_eq!(wrong(&[output("verdict", choice("Check", &["ok", "doubt", "unsure"])), reason()]).as_deref(), Some("its output `verdict` is enum Check (ok, doubt, unsure)"));
    assert_eq!(wrong(&[output("verdict", choice("Check", &["ok"])), reason()]).as_deref(), Some("its output `verdict` is enum Check (ok)"));
    assert_eq!(wrong(&[output("verdict", Carried::Bool), reason()]).as_deref(), Some("its output `verdict` is bool"));
    assert_eq!(wrong(&[verdict(), output("reason", choice("Why", &["late"]))]).as_deref(), Some("its output `reason` is enum Why (late)"));
    // both
    assert_eq!(wrong(&[]).as_deref(), Some("it has no output `verdict`, and it has no output `reason`"));
    assert_eq!(judge_answer(&[]).map(|t| t.ja).as_deref(), Some("出力 `verdict` がありません。出力 `reason` がありません"));
}
