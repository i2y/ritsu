//! DESIGN 2.2–2.7, one rule at a time: a hand-written scenario and what it must answer.

use chobo::model::Book;
use chobo::scenario;
use serde_json::{Value, json};

const BOOK: &str = r#"
book 試し v1
unit 個

account 在庫(sku: string) : 個
  at least 0 refused as 在庫切れ
account 棚(sku: string) : 個
  at least 2 refused as 安全在庫割れ
  at most 10 refused as 満杯
account 預り(注文: string) : 個
  at least 0 refused as 預り不足
account 仕入先 : 個 outside
account 客 : 個 outside

transfer 入荷(伝票: string, sku: string, 数: 個)
  key 伝票, sku
  move 数 from 仕入先 to 在庫(sku)
transfer 棚入れ(伝票: string, sku: string, 数: 個)
  key 伝票
  move 数 from 仕入先 to 棚(sku)
transfer 棚出し(伝票: string, sku: string, 数: 個)
  key 伝票
  move 数 from 棚(sku) to 客
transfer 引当(注文: string, sku: string, 数: 個)
  key 注文, sku
  pending expires after 30 minutes
  move 数 from 在庫(sku) to 客
transfer 取ってから入れる(注文: string, 数: 個)
  key 注文
  move 数 from 預り(注文) to 客
  move 数 from 仕入先 to 預り(注文)
transfer 入れてから取る(注文: string, 数: 個)
  key 注文
  move 数 from 仕入先 to 預り(注文)
  move 数 from 預り(注文) to 客
transfer 預りの仮押さえ(注文: string, 数: 個)
  key 注文
  pending never expires
  move 数 from 仕入先 to 預り(注文)
  move 数 from 預り(注文) to 客
transfer 両方から(注文: string, sku: string, 数: 個)
  key 注文
  move 数 from 在庫(sku) to 客
  move 数 from 棚(sku) to 客
transfer 移し替え(伝票: string, 元: string, 先: string, 数: 個)
  key 伝票
  move 数 from 在庫(元) to 在庫(先)
"#;

fn book() -> Book {
    let (b, d) = chobo::model::load(BOOK);
    assert!(d.is_empty(), "{:?}", d.iter().map(|x| x.message.en.clone()).collect::<Vec<_>>());
    b.unwrap()
}

fn run(steps: Value) -> Value {
    let b = book();
    let s = scenario::from_json(&b, &json!({"name": "", "steps": steps})).unwrap();
    scenario::run_json(&b, &s).unwrap()
}

/// What each step answered: `done`, `done_before`, `refused:<reason>`; `pass` for a pass.
fn answers(v: &Value) -> Vec<String> {
    v["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| match (s["result"].as_str(), s["reason"].as_str()) {
            (Some("refused"), Some(r)) => format!("refused:{r}"),
            (Some(r), _) => r.to_string(),
            _ => s["op"].as_str().unwrap().to_string(),
        })
        .collect()
}

/// posted, held_out, held_in of one account at the end.
fn bal(v: &Value, account: &str, args: &[&str]) -> (i64, i64, i64) {
    let a = v["accounts"].as_array().unwrap().iter().find(|a| a["account"] == json!(account) && a["args"] == json!(args)).unwrap_or_else(|| panic!("no {account}{args:?}"));
    (a["posted"].as_i64().unwrap(), a["held_out"].as_i64().unwrap(), a["held_in"].as_i64().unwrap())
}

fn hold_state(v: &Value, kind: &str, key: &[&str]) -> String {
    v["holds"].as_array().unwrap().iter().find(|h| h["kind"] == json!(kind) && h["key"] == json!(key)).unwrap()["state"].as_str().unwrap().to_string()
}

fn op(o: &str, kind: &str, args: Value) -> Value {
    json!({"op": o, "kind": kind, "args": args})
}

#[test]
fn a_lower_bound_lets_a_call_reach_it_and_refuses_one_past_it() {
    let v = run(json!([
        op("do", "棚入れ", json!({"伝票": "a", "sku": "x", "数": 5})),
        op("do", "棚出し", json!({"伝票": "b", "sku": "x", "数": 3})),
        op("do", "棚出し", json!({"伝票": "c", "sku": "x", "数": 1})),
    ]));
    assert_eq!(answers(&v), ["done", "done", "refused:安全在庫割れ"]);
    assert_eq!(bal(&v, "棚", &["x"]), (2, 0, 0));
}

#[test]
fn an_upper_bound_lets_a_call_reach_it_and_refuses_one_past_it() {
    let v = run(json!([
        op("do", "棚入れ", json!({"伝票": "a", "sku": "x", "数": 10})),
        op("do", "棚入れ", json!({"伝票": "b", "sku": "x", "数": 1})),
    ]));
    assert_eq!(answers(&v), ["done", "refused:満杯"]);
    assert_eq!(bal(&v, "棚", &["x"]), (10, 0, 0));
}

#[test]
fn what_is_held_going_out_counts_against_the_lower_bound() {
    let v = run(json!([
        op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 3})),
        op("hold", "引当", json!({"注文": "o1", "sku": "x", "数": 2})),
        op("hold", "引当", json!({"注文": "o2", "sku": "x", "数": 2})),
        op("hold", "引当", json!({"注文": "o3", "sku": "x", "数": 1})),
    ]));
    assert_eq!(answers(&v), ["done", "done", "refused:在庫切れ", "done"]);
    assert_eq!(bal(&v, "在庫", &["x"]), (3, 3, 0));
}

#[test]
fn what_is_held_coming_in_cannot_be_taken() {
    let v = run(json!([op("hold", "預りの仮押さえ", json!({"注文": "o1", "数": 1}))]));
    assert_eq!(answers(&v), ["refused:預り不足"]);
    assert_eq!(bal(&v, "預り", &["o1"]), (0, 0, 0));
}

#[test]
fn moves_are_checked_in_the_order_they_are_written() {
    let v = run(json!([
        op("do", "取ってから入れる", json!({"注文": "o1", "数": 1})),
        op("do", "入れてから取る", json!({"注文": "o2", "数": 1})),
    ]));
    assert_eq!(answers(&v), ["refused:預り不足", "done"]);
    assert_eq!(bal(&v, "預り", &["o2"]), (0, 0, 0));
    assert_eq!(bal(&v, "客", &[]), (1, 0, 0));
}

#[test]
fn a_refusal_at_one_move_leaves_every_move_undone() {
    let v = run(json!([
        op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 5})),
        op("do", "両方から", json!({"注文": "o1", "sku": "x", "数": 1})),
    ]));
    assert_eq!(answers(&v), ["done", "refused:安全在庫割れ"]);
    assert_eq!(bal(&v, "在庫", &["x"]), (5, 0, 0));
    assert_eq!(bal(&v, "棚", &["x"]), (0, 0, 0));
    assert_eq!(bal(&v, "客", &[]), (0, 0, 0));
}

#[test]
fn the_same_key_with_the_same_content_does_nothing_and_with_other_content_is_refused() {
    let v = run(json!([
        op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 3})),
        op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 3})),
        op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 4})),
        op("do", "入荷", json!({"伝票": "a", "sku": "y", "数": 4})),
    ]));
    assert_eq!(answers(&v), ["done", "done_before", "refused:key_conflict", "done"]);
    assert_eq!(bal(&v, "在庫", &["x"]), (3, 0, 0));
}

#[test]
fn a_key_a_bound_refused_stays_spent() {
    let v = run(json!([
        op("hold", "引当", json!({"注文": "o1", "sku": "x", "数": 1})),
        op("hold", "引当", json!({"注文": "o1", "sku": "x", "数": 1})),
        op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 5})),
        op("hold", "引当", json!({"注文": "o1", "sku": "x", "数": 1})),
        op("hold", "引当", json!({"注文": "o1", "sku": "x", "数": 2})),
        op("hold", "引当", json!({"注文": "o2", "sku": "x", "数": 1})),
    ]));
    assert_eq!(answers(&v), ["refused:在庫切れ", "refused:already_refused", "done", "refused:already_refused", "refused:already_refused", "done"]);
}

#[test]
fn a_post_takes_all_or_part_and_never_more_than_was_held() {
    let v = run(json!([
        op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 5})),
        op("hold", "引当", json!({"注文": "o1", "sku": "x", "数": 3})),
        json!({"op": "post", "kind": "引当", "args": {"注文": "o1", "sku": "x"}, "amounts": {"数": 4}}),
        json!({"op": "post", "kind": "引当", "args": {"注文": "o1", "sku": "x"}, "amounts": {"数": 2}}),
        op("hold", "引当", json!({"注文": "o2", "sku": "x", "数": 2})),
        op("post", "引当", json!({"注文": "o2", "sku": "x"})),
    ]));
    assert_eq!(answers(&v), ["done", "done", "refused:over_hold", "done", "done", "done"]);
    // 5 in, 2 posted out of the hold of 3 (1 back), then 2 out in full
    assert_eq!(bal(&v, "在庫", &["x"]), (1, 0, 0));
    assert_eq!(bal(&v, "客", &[]), (4, 0, 0));
}

#[test]
fn posting_and_voiding_after_one_another() {
    let k = |o: &str| json!({"注文": o, "sku": "x"});
    let v = run(json!([
        op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 10})),
        op("hold", "引当", json!({"注文": "p", "sku": "x", "数": 2})),
        op("post", "引当", k("p")),
        op("post", "引当", k("p")),
        json!({"op": "post", "kind": "引当", "args": k("p"), "amounts": {"数": 2}}),
        json!({"op": "post", "kind": "引当", "args": k("p"), "amounts": {"数": 1}}),
        op("void", "引当", k("p")),
        op("hold", "引当", json!({"注文": "v", "sku": "x", "数": 2})),
        op("void", "引当", k("v")),
        op("void", "引当", k("v")),
        op("post", "引当", k("v")),
    ]));
    assert_eq!(
        answers(&v),
        ["done", "done", "done", "done_before", "done_before", "refused:key_conflict", "refused:already_posted", "done", "done", "done_before", "refused:already_voided"]
    );
    assert_eq!(hold_state(&v, "引当", &["p", "x"]), "posted");
    assert_eq!(hold_state(&v, "引当", &["v", "x"]), "voided");
    assert_eq!(bal(&v, "在庫", &["x"]), (8, 0, 0));
}

#[test]
fn a_post_before_its_hold_spends_no_key() {
    let k = json!({"注文": "o1", "sku": "x"});
    let v = run(json!([
        op("post", "引当", k.clone()),
        op("void", "引当", k.clone()),
        op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 1})),
        op("hold", "引当", json!({"注文": "o1", "sku": "x", "数": 1})),
        op("post", "引当", k),
    ]));
    assert_eq!(answers(&v), ["refused:no_such_hold", "refused:no_such_hold", "done", "done", "done"]);
}

#[test]
fn a_hold_expires_when_the_clock_reaches_its_expiry() {
    let k = json!({"注文": "o1", "sku": "x"});
    let start = || {
        vec![op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 1})), op("hold", "引当", json!({"注文": "o1", "sku": "x", "数": 1}))]
    };
    let mut before = start();
    before.push(json!({"op": "pass", "duration": "1799 seconds"}));
    before.push(op("post", "引当", k.clone()));
    let v = run(Value::Array(before));
    assert_eq!(answers(&v), ["done", "done", "pass", "done"]);

    let mut at = start();
    at.push(json!({"op": "pass", "duration": "30 minutes"}));
    at.push(op("post", "引当", k.clone()));
    at.push(op("void", "引当", k.clone()));
    let v = run(Value::Array(at));
    assert_eq!(answers(&v), ["done", "done", "pass", "refused:expired", "refused:expired"]);
    assert_eq!(hold_state(&v, "引当", &["o1", "x"]), "expired");
    assert_eq!(bal(&v, "在庫", &["x"]), (1, 0, 0));
}

#[test]
fn holding_again_with_the_key_of_an_ended_hold_holds_nothing() {
    let v = run(json!([
        op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 1})),
        op("hold", "引当", json!({"注文": "o1", "sku": "x", "数": 1})),
        {"op": "pass", "duration": "31 minutes"},
        op("hold", "引当", json!({"注文": "o1", "sku": "x", "数": 1})),
    ]));
    assert_eq!(answers(&v), ["done", "done", "pass", "done_before"]);
    assert_eq!(bal(&v, "在庫", &["x"]), (1, 0, 0));
    assert_eq!(hold_state(&v, "引当", &["o1", "x"]), "expired");
}

#[test]
fn a_move_from_an_account_to_itself_is_refused_and_spends_no_key() {
    let v = run(json!([
        op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 2})),
        op("do", "移し替え", json!({"伝票": "t", "元": "x", "先": "x", "数": 1})),
        op("do", "移し替え", json!({"伝票": "t", "元": "x", "先": "y", "数": 1})),
    ]));
    assert_eq!(answers(&v), ["done", "refused:same_account", "done"]);
    assert_eq!(bal(&v, "在庫", &["y"]), (1, 0, 0));
}

#[test]
fn together_is_every_order_the_callers_can_interleave_in() {
    let b = book();
    let h = |o: &str| op("hold", "引当", json!({"注文": o, "sku": "x", "数": 1}));
    let s = json!({"name": "", "steps": [
        op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": 1})),
        {"op": "together", "callers": [[h("a"), h("b")], [h("c")]]},
    ]});
    let s = scenario::from_json(&b, &s).unwrap();
    // three orders, a b c, a c b and c a b; the first two come out the same
    assert_eq!(scenario::interleavings(&[2, 1]).len(), 3);
    assert_eq!(scenario::interleavings(&[2, 2]).len(), 6);
    assert_eq!(scenario::interleavings(&[1, 1, 1]).len(), 6);
    assert_eq!(scenario::interleavings(&[4, 4]).len(), 70);
    assert_eq!(scenario::run(&b, &s).unwrap().len(), 2);
    let v = scenario::run_json(&b, &s).unwrap();
    let outs = v["outcomes"].as_array().unwrap();
    // a wins (twice over) or c wins: two distinct outcomes, and b never wins
    assert_eq!(outs.len(), 2);
    for o in outs {
        let callers = o["steps"][1]["callers"].as_array().unwrap();
        assert_eq!(callers[0][1]["result"], json!("refused"));
    }
}

#[test]
fn a_scenario_that_does_not_fit_the_book_is_a_mistake_not_a_refusal() {
    let b = book();
    for steps in [
        json!([op("do", "無い", json!({}))]),
        json!([op("hold", "入荷", json!({"伝票": "a", "sku": "x", "数": 1}))]),
        json!([op("do", "入荷", json!({"伝票": "a", "sku": "x"}))]),
        json!([op("do", "入荷", json!({"伝票": "a", "sku": "x", "数": -1}))]),
        json!([op("do", "入荷", json!({"伝票": "a", "sku": 3, "数": 1}))]),
        json!([{"op": "post", "kind": "引当", "args": {"注文": "o", "sku": "x", "数": 1}}]),
        json!([{"op": "pass", "duration": "a while"}]),
        json!([{"op": "together", "callers": [[{"op": "pass", "duration": "1 second"}]]}]),
    ] {
        assert!(scenario::from_json(&b, &json!({"name": "", "steps": steps})).is_err(), "{steps}");
    }
    let nine: Vec<Value> = (0..9).map(|i| op("hold", "引当", json!({"注文": format!("o{i}"), "sku": "x", "数": 1}))).collect();
    let too_many = json!({"name": "", "steps": [{"op": "together", "callers": [nine]}]});
    assert!(scenario::from_json(&b, &too_many).is_err());
}
