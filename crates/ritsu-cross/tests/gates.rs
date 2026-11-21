//! The decisions of X15 and X16 (sekisho's DESIGN 4.6), held to what each way an operation and a
//! call can come out comes to. The checks over a project, with the languages answering through
//! their ports, are `crates/ritsu/tests/cross.rs` and the ledger's reproductions.

use ritsu_base::naming::{Name, Tool};
use ritsu_base::text::Text;
use ritsu_cross::gates::{Called, Opened, judge_call, judge_opened};
use ritsu_ports::{AllowedAt, Allowance, Found, OperationCall, PublishedOperation};
use std::collections::{BTreeMap, BTreeSet};

fn op(context: &str, id: &str, open: bool) -> PublishedOperation {
    PublishedOperation { context: context.into(), operation: Name::file(Tool::Openapi, "api.json").with("operation", id), open_to_anyone: open, file: format!("contexts/{context}.ctx"), line: 9 }
}

#[test]
fn an_operation_a_context_opens() {
    let ops = [op("Orders", "getOrder", false), op("Orders", "refundOrder", false), op("Orders", "listOrders", true), op("Payments", "createCharge", false), op("Notices", "tell", false)];
    let mut guarded = BTreeMap::new();
    guarded.insert(ops[1].operation.clone(), vec![("orders/refunds.gate".to_string(), "refund_order".to_string())]);
    let skipped: BTreeSet<String> = ["Notices".to_string()].into();
    let judged: Vec<(String, Opened)> = judge_opened(&ops, &guarded, &skipped).into_iter().map(|(o, c)| (o.operation.items[0].1.clone(), c)).collect();
    assert_eq!(
        judged,
        [
            // Orders guards one: the one it does not is E907, the one open to anyone holds
            ("getOrder".to_string(), Opened::Unguarded),
            ("refundOrder".to_string(), Opened::Guarded(vec![("orders/refunds.gate".to_string(), "refund_order".to_string())])),
            ("listOrders".to_string(), Opened::Open),
            // Payments guards none of its own: W907
            ("createCharge".to_string(), Opened::NotYet),
            // a gate of Notices does not answer, and Notices is not looked at
        ]
    );
}

#[test]
fn a_call_a_workflow_makes() {
    let call = |denied: Option<&str>| OperationCall::new(14, "refund_order", Name::file(Tool::Openapi, "orders.json").with("operation", "refundOrder"), denied);
    let at = |allowance: Found<Allowance>| AllowedAt::of(allowance);
    let sometimes = || Found::Value(Allowance::Sometimes { allowed: Text::same("a"), denied: Text::same("d") });
    let undecided = || Found::Undecided(Text::same("why"));
    // allowed in no combination: every run is denied there, handled or not
    assert_eq!(judge_call(&call(None), &at(Found::Value(Allowance::Never))), Called::Never);
    assert_eq!(judge_call(&call(Some("denied")), &at(Found::Value(Allowance::Never))), Called::Never);
    // allowed in every combination, or handled when it is not
    assert_eq!(judge_call(&call(None), &at(Found::Value(Allowance::Always))), Called::Held);
    assert_eq!(judge_call(&call(Some("denied")), &at(sometimes())), Called::Held);
    assert_eq!(judge_call(&call(Some("denied")), &at(undecided())), Called::Held);
    // denied in some, and the task says nothing of it
    assert_eq!(judge_call(&call(None), &at(sometimes())), Called::Unhandled { allowed: Text::same("a"), denied: Text::same("d") });
    assert_eq!(judge_call(&call(None), &at(undecided())), Called::Undecided(Text::same("why")));
}

#[test]
fn a_call_held_to_what_the_flow_holds() {
    let call = |denied: Option<&str>| OperationCall::new(14, "refund_order", Name::file(Tool::Openapi, "orders.json").with("operation", "refundOrder"), denied);
    let line = || vec![(Text::same("`amount = order.total` under `resource Order from order.id`"), 9)];
    let sometimes = || Found::Value(Allowance::Sometimes { allowed: Text::same("a"), denied: Text::same("d") });
    // allowed in every combination only as the gate's `resource` lines say: a denial is still
    // the flow's to handle, since nothing checks the lines of the resource the service reads
    let rests = AllowedAt { rests_on: line(), ..AllowedAt::of(Found::Value(Allowance::Always)) };
    assert_eq!(judge_call(&call(None), &rests), Called::Rests);
    assert_eq!(judge_call(&call(Some("denied")), &rests), Called::Held);
    // in none, whatever it rests on: every run that comes to the call is denied
    let never = AllowedAt { rests_on: line(), ..AllowedAt::of(Found::Value(Allowance::Never)) };
    assert_eq!(judge_call(&call(Some("denied")), &never), Called::Never);
    // in some of the combinations the lines leave: it can be denied
    let some = AllowedAt { rests_on: line(), ..AllowedAt::of(sometimes()) };
    assert_eq!(judge_call(&call(None), &some), Called::Unhandled { allowed: Text::same("a"), denied: Text::same("d") });
    // in some of the gate's, with nothing written of what the flow holds of what the answer
    // reads, or with something the flow holds that the gate could not be asked with: which of
    // them the call comes to is not known
    let unrelated = AllowedAt { unrelated: vec!["resource.status".into()], ..AllowedAt::of(sometimes()) };
    assert_eq!(judge_call(&call(None), &unrelated), Called::Unsaid { allowed: Text::same("a"), denied: Text::same("d") });
    assert_eq!(judge_call(&call(Some("denied")), &unrelated), Called::Held);
    let unheld = AllowedAt { unheld: vec![Text::same("why")], ..AllowedAt::of(sometimes()) };
    assert_eq!(judge_call(&call(None), &unheld), Called::Unsaid { allowed: Text::same("a"), denied: Text::same("d") });
    // what is not held changes nothing of an answer that is one for every combination
    let always = AllowedAt { unrelated: vec!["resource.status".into()], unheld: vec![Text::same("why")], ..AllowedAt::of(Found::Value(Allowance::Always)) };
    assert_eq!(judge_call(&call(None), &always), Called::Held);
}
