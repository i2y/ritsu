//! X14 (DESIGN 16.8): where a flow sends a secret value, against the map. The decision is
//! `egress::judge`, over the shape of the ports' answers only (`Flows::sends`, `Maps`); here it is
//! given a map of three contexts and the files they hold, and every way a secret can go.
//!
//! The map: Payments marks the card (`payments/v1/payment.proto`), Ordering is downstream of it,
//! Notifications is downstream of Ordering only, and Shipping goes separate ways from Payments.

use ritsu_cross::Borders;
use ritsu_cross::egress::{Call, Outcome, judge, related};
use ritsu_ports::{Destination, MapFacts, MapRelationship, Said, Secret, Send};
use ritsu_base::text::Text;
use std::path::{Path, PathBuf};

fn rel(from: &str, to: &str, words: &str) -> MapRelationship {
    MapRelationship { from: from.into(), to: to.into(), words: words.into(), separate: words == "separate ways from", file: format!("{}.ctx", from.to_lowercase()), line: 3 }
}

fn map() -> MapFacts {
    MapFacts {
        file: "webshop.ctx".into(),
        contexts: vec!["Payments".into(), "Ordering".into(), "Notifications".into(), "Shipping".into()],
        relationships: vec![
            rel("Ordering", "Payments", "downstream"),
            rel("Notifications", "Ordering", "downstream"),
            rel("Shipping", "Payments", "separate ways from"),
        ],
    }
}

/// The context sakai gives each file of the map; a file under `broken/` is one it cannot answer
/// for.
fn context_of(file: &str) -> Result<Option<String>, Vec<Said>> {
    if file.starts_with("broken/") {
        return Err(vec![Said { code: "E101".into(), file: "webshop.ctx".into(), line: Some(4), message: Text::new("読めません", "does not read") }]);
    }
    let c = match file.split('/').next().unwrap_or("") {
        "payments" => "Payments",
        "ordering" => "Ordering",
        "notifications" => "Notifications",
        "shipping" => "Shipping",
        _ => return Ok(None),
    };
    Ok(Some(c.to_string()))
}

/// dandori reaches files from the root here.
fn from_root(p: &Path) -> Option<String> {
    Some(p.to_string_lossy().to_string())
}

fn card() -> Secret {
    Secret { shown: "payment.card_last_digits".into(), marked_in: PathBuf::from("payments/v1/payment.proto"), line: 18, mark: "debug_redact = true".into() }
}

fn send(to: &str, secrets: Vec<(&str, Secret)>, disclosed: Vec<(&str, &str)>) -> Send {
    Send {
        line: 22,
        task: "send_notice".into(),
        to: Destination::File(PathBuf::from(to)),
        secrets: secrets.into_iter().map(|(p, s)| (p.to_string(), s)).collect(),
        disclosed: disclosed.into_iter().map(|(p, w)| (p.to_string(), w.to_string())).collect(),
    }
}

/// The calls of a flow at `flow`, judged.
fn judged(flow: &str, sends: &[Send]) -> Vec<Call> {
    judge(flow, sends, &map(), &context_of, &from_root).expect("the map answers").expect("the flow is in the map")
}

fn outcome(flow: &str, s: Send) -> Outcome {
    judged(flow, &[s]).remove(0).secrets.remove(0).outcome
}

#[test]
fn a_secret_sent_to_its_own_context_holds() {
    assert_eq!(outcome("ordering/checkout.flow", send("payments/api/payments.yaml", vec![("payment", card())], vec![])), Outcome::Same);
}

#[test]
fn a_secret_sent_to_a_related_context_holds_either_way() {
    // Ordering is downstream of Payments, which marked the card
    assert_eq!(outcome("payments/charge.flow", send("ordering/api/ordering.json", vec![("payment", card())], vec![])), Outcome::Related("downstream".into()));
    assert_eq!(related(&map(), "Payments", "Ordering"), Some("downstream".into()));
    assert_eq!(related(&map(), "Ordering", "Payments"), Some("downstream".into()));
}

#[test]
fn a_secret_sent_to_an_unrelated_context_fails() {
    // Notifications is related to Ordering only, not to Payments
    let c = judged("ordering/checkout.flow", &[send("notifications/notify.flow", vec![("payment", card())], vec![])]).remove(0);
    let j = &c.secrets[0];
    assert_eq!(j.outcome, Outcome::Unrelated);
    assert_eq!((j.marked_by.as_deref(), j.sent_to.as_deref()), (Some("Payments"), Some("Notifications")));
}

#[test]
fn separate_ways_is_no_relationship() {
    assert_eq!(related(&map(), "Shipping", "Payments"), None);
    assert_eq!(outcome("ordering/checkout.flow", send("shipping/api/shipping.yaml", vec![("payment", card())], vec![])), Outcome::Unrelated);
}

#[test]
fn a_secret_sent_outside_the_map_fails() {
    assert_eq!(outcome("ordering/checkout.flow", send("tools/report.flow", vec![("payment", card())], vec![])), Outcome::Outside);
    // a file outside the root is outside the map too
    let outside_root = |p: &Path| (!p.starts_with("..")).then(|| p.to_string_lossy().to_string());
    let calls = judge("ordering/checkout.flow", &[send("../elsewhere/x.yaml", vec![("payment", card())], vec![])], &map(), &context_of, &outside_root).unwrap().unwrap();
    assert_eq!((calls[0].to.as_deref(), &calls[0].secrets[0].outcome), (None, &Outcome::Outside));
}

#[test]
fn discloses_lets_a_secret_go_where_it_would_fail() {
    let o = outcome("ordering/checkout.flow", send("notifications/notify.flow", vec![("payment", card())], vec![("payment", "the notice shows the last digits")]));
    assert_eq!(o, Outcome::Disclosed("the notice shows the last digits".into()));
    // only the parameter it names
    let c = judged("ordering/checkout.flow", &[send("notifications/notify.flow", vec![("payment", card()), ("refund", card())], vec![("payment", "why")])]).remove(0);
    let os: Vec<&Outcome> = c.secrets.iter().map(|j| &j.outcome).collect();
    assert_eq!(os, [&Outcome::Disclosed("why".into()), &Outcome::Unrelated]);
}

#[test]
fn a_mark_the_flow_writes_is_the_flows_context() {
    let token = Secret { shown: "api_token".into(), marked_in: PathBuf::from("notifications/notify.flow"), line: 7, mark: "secret".into() };
    // the flow is in Notifications, which is related to Ordering
    assert_eq!(outcome("notifications/notify.flow", send("ordering/api/ordering.json", vec![("token", token.clone())], vec![])), Outcome::Related("downstream".into()));
    // and not to Payments
    assert_eq!(outcome("notifications/notify.flow", send("payments/api/payments.yaml", vec![("token", token)], vec![])), Outcome::Unrelated);
}

#[test]
fn a_mark_in_a_file_no_context_holds_is_the_flows_context() {
    let email = Secret { shown: "customer.email".into(), marked_in: PathBuf::from("specs/crm.json"), line: 0, mark: "x-data-classification".into() };
    let c = judged("ordering/checkout.flow", &[send("ordering/api/ordering.json", vec![("customer", email)], vec![])]).remove(0);
    assert_eq!((c.secrets[0].marked_by.as_deref(), &c.secrets[0].outcome), (Some("Ordering"), &Outcome::Same));
}

#[test]
fn a_flow_no_context_holds_is_not_looked_at() {
    let r = judge("tools/report.flow", &[send("notifications/notify.flow", vec![("payment", card())], vec![])], &map(), &context_of, &from_root).unwrap();
    assert!(r.is_none());
}

#[test]
fn what_sakai_cannot_answer_is_undecided() {
    // the flow itself: whether the map holds it cannot be said
    let r = judge("broken/checkout.flow", &[send("ordering/api/ordering.json", vec![("payment", card())], vec![])], &map(), &context_of, &from_root);
    assert_eq!(r.unwrap_err()[0].code, "E101");
    // where it sends
    let o = outcome("ordering/checkout.flow", send("broken/api.yaml", vec![("payment", card())], vec![]));
    assert!(matches!(o, Outcome::Undecided(ref s) if s[0].code == "E101"), "{o:?}");
    // where the mark is
    let odd = Secret { marked_in: PathBuf::from("broken/payment.proto"), ..card() };
    assert!(matches!(outcome("ordering/checkout.flow", send("ordering/api/ordering.json", vec![("payment", odd)], vec![])), Outcome::Undecided(_)));
}

#[test]
fn each_call_is_one_border() {
    let calls = judged(
        "ordering/checkout.flow",
        &[
            send("payments/api/payments.yaml", vec![("payment", card())], vec![]),
            send("notifications/notify.flow", vec![("payment", card()), ("refund", card())], vec![]),
            send("broken/api.yaml", vec![("payment", card())], vec![]),
            send("notifications/notify.flow", vec![("payment", card())], vec![("payment", "why")]),
        ],
    );
    let mut b = Borders::default();
    for c in &calls {
        c.count(&mut b);
    }
    // held: the first and the disclosed; failed: the second (two secrets, one border); undecided: the third
    assert_eq!(b, Borders { held: 2, failed: 1, undecided: 1 });
}
