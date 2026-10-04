//! What a `.flow` holds and what it names outside itself, as ritsu's ports ask for them
//! (ritsu's DESIGN 6.3 and 6.4; dandori's DESIGN 7): the kinds, the lines, the definitions, and
//! the references with the ways they are written.

use dandori::ports::Engine;
use ritsu_base::naming::{self, Tool};
use ritsu_ports::{Items, References};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.flow` of the examples and of the flows the tests build, from the crate's directory.
fn flows() -> Vec<String> {
    let mut all = Vec::new();
    for dir in ["examples", "tests/flows"] {
        ritsu_base::paths::walk(&root(), dir, &[], &mut all);
    }
    all.retain(|f| f.ends_with(".flow"));
    all.sort();
    all
}

#[test]
fn a_flow_names_its_tasks_cases_records_enums_inputs_and_outputs() {
    let file = "examples/fulfillment/temporal/fulfillment.flow";
    let items = Engine.items(&root(), file).unwrap();
    let find = |kind: &str, name: &str| items.iter().find(|i| i.kind() == kind && i.name() == name).unwrap_or_else(|| panic!("no {kind} {name}: {items:?}"));
    // a record is its block, with the colons no longer lined up; a field is its line
    let line = find("record", "Line");
    assert_eq!((line.lines, line.text.as_str()), ((9, 11), "record Line\n  sku : string\n  quantity : int"));
    let sku = items.iter().find(|i| i.naming.items == [("record".to_string(), "Line".to_string()), ("field".to_string(), "sku".to_string())]).unwrap();
    assert_eq!((sku.lines, sku.text.as_str()), ((10, 10), "sku : string"));
    assert_eq!(find("input", "order").text, "order : Order");
    assert_eq!(find("output", "tracking_number").lines, (40, 40));
    // a task is its block, without the comment above it
    let reserve = find("task", "reserve_stock");
    assert_eq!(reserve.lines, (43, 47));
    assert_eq!(
        reserve.text,
        "task reserve_stock(sku: string, quantity: int) -> warehouse.ReserveResponse\n  connect warehouse \"StockService/Reserve\"\n  errors busy = resource_exhausted\n  key\n  retry 2 times every 1 second on busy"
    );
    assert_eq!(reserve.naming.text(), format!("dandori \"{file}\" task reserve_stock"));
    let kinds: Vec<&str> = items.iter().map(|i| i.kind()).collect();
    assert_eq!(kinds.iter().filter(|k| **k == "task").count(), 6, "{kinds:?}");
    // the items are in the order they are written
    assert!(items.windows(2).all(|w| w[0].lines.0 <= w[1].lines.0));

    // the names are the flow's own, in Japanese too: a case, an enum and its values
    let ja = "examples/hotel/temporal/hotel.ja.flow";
    let items = Engine.items(&root(), ja).unwrap();
    let case = items.iter().find(|i| i.kind() == "case").unwrap();
    assert_eq!((case.name(), case.lines), ("決済", (68, 72)));
    assert!(case.text.starts_with("case 決済 : PaymentIntent follows payment_intent.payment\n  held capture_method = manual\n"), "{}", case.text);
    let value = items.iter().find(|i| i.kind() == "value" && i.name() == "確認待ち").unwrap();
    assert_eq!((value.naming.text(), value.text.as_str()), (format!("dandori \"{ja}\" enum 結果 value 確認待ち"), "確認待ち"));
    assert_eq!(items.iter().find(|i| i.kind() == "enum").unwrap().text, "enum 結果 = 宿泊済 | 確認待ち");
}

#[test]
fn a_definition_is_the_same_however_it_is_spaced_and_commented() {
    let dir = ritsu_testkit::TempDir::new("ports");
    let tight = "workflow w v1\n\nrecord Box\n  id : string\n  weight : mass[kg]\n\ntask weigh(id: string) -> Box\n  jev \"How heavy is it?\"\n    light \"under a kilo\"\n  timeout 10 seconds\n";
    let loose = "workflow w v1\n# a comment\nrecord Box   # the box\n    id     : string\n    weight : mass[kg]\n\n\ntask weigh(id: string) -> Box\n    jev \"How heavy is it?\"\n\n        light \"under a kilo\"   # one level\n    # a comment in the block\n    timeout 10 seconds\n# the end\n";
    let changed = tight.replace("mass[kg]", "mass[g]");
    let mut texts = Vec::new();
    for (name, src) in [("tight.flow", tight), ("loose.flow", loose), ("changed.flow", changed.as_str())] {
        std::fs::write(dir.path().join(name), src).unwrap();
        let items = Engine.items(dir.path(), name).unwrap();
        texts.push(items.into_iter().map(|i| (i.naming.items, i.text)).collect::<Vec<_>>());
    }
    assert_eq!(texts[0], texts[1], "spacing and comments change no definition");
    let differ: Vec<&Vec<(String, String)>> = texts[0].iter().zip(&texts[2]).filter(|(a, b)| a != b).map(|(a, _)| &a.0).collect();
    assert_eq!(differ, [&vec![("record".to_string(), "Box".to_string())], &vec![("record".to_string(), "Box".to_string()), ("field".to_string(), "weight".to_string())]]);
    // the block of a task keeps what is under what
    let weigh = &texts[1].iter().find(|(n, _)| n[0].0 == "task").unwrap().1;
    assert_eq!(weigh, "task weigh(id: string) -> Box\n  jev \"How heavy is it?\"\n    light \"under a kilo\"\n  timeout 10 seconds");
}

#[test]
fn a_flow_names_its_rules_apis_services_methods_and_children() {
    let show = |file: &str| -> Vec<String> { Engine.references(&root(), file).unwrap().iter().map(|r| format!("{} {} [{}]", r.line, r.target.text(), r.how)).collect() };
    assert_eq!(
        show("examples/fulfillment/temporal/fulfillment.flow"),
        [
            "1 proto \"examples/fulfillment/specs/fulfillment.proto\" service FulfillmentService [implements]",
            "4 rulec \"examples/order/rules/urgency.rule\" [use rule]",
            "5 proto \"examples/fulfillment/specs/fulfillment.proto\" [use proto]",
            "6 proto \"examples/fulfillment/specs/warehouse.proto\" [use proto]",
            "44 proto \"examples/fulfillment/specs/warehouse.proto\" service StockService method Reserve [connect]",
            "50 proto \"examples/fulfillment/specs/warehouse.proto\" service StockService method Release [connect]",
            "56 dandori \"examples/fulfillment/arrange_delivery.flow\" [flow]",
        ]
    );
    // the ways a rule is called, as written under its `use rule`
    let ways = |file: &str| -> Vec<String> { Engine.references(&root(), file).unwrap().into_iter().filter(|r| r.target.tool == Tool::Rulec).map(|r| r.how).collect() };
    assert_eq!(ways("tests/flows/connect_rules.flow"), ["use rule … connect, local", "use rule … connect", "use rule … connect"]);
    assert_eq!(ways("tests/flows/local_rules.flow")[..2], ["use rule … lambda, local", "use rule … lambda"]);
    assert_eq!(ways("examples/inquiry/temporal/inquiry.flow"), ["use rule … local"]);
    // an OpenAPI document is a file
    assert!(show("examples/hotel/temporal/hotel.ja.flow").contains(&"6 file \"examples/hotel/specs/stripe.json\" [use openapi]".to_string()));
    // a dates file is koyomi's, and a book chobo's, with each transfer a task runs an operation of
    assert_eq!(
        show("examples/invoice/invoice.flow"),
        [
            "7 koyomi \"examples/invoice/dates/payment_terms.cal\" [use dates … lambda]",
            "9 chobo \"examples/invoice/books/stock.book\" [use book … lambda]",
            "32 chobo \"examples/invoice/books/stock.book\" transfer reserve [book]",
            "37 chobo \"examples/invoice/books/stock.book\" transfer reserve [book]",
            "42 chobo \"examples/invoice/books/stock.book\" transfer reserve [book]",
        ]
    );
    assert_eq!(ways_of("tests/flows/dates_and_books.flow", Tool::Koyomi), ["use dates … lambda, local"]);
}

/// The ways one tool's files are named by a flow.
fn ways_of(file: &str, tool: Tool) -> Vec<String> {
    Engine.references(&root(), file).unwrap().into_iter().filter(|r| r.target.tool == tool && r.target.items.is_empty()).map(|r| r.how).collect()
}

#[test]
fn every_flow_of_the_examples_names_what_is_there() {
    let all = flows();
    assert!(all.len() >= 40, "{all:?}");
    let (mut items, mut refs) = (0, 0);
    for f in &all {
        let n = std::fs::read_to_string(root().join(f)).unwrap().lines().count();
        for it in Engine.items(&root(), f).unwrap() {
            assert!(it.lines.0 >= 1 && it.lines.0 <= it.lines.1 && it.lines.1 <= n, "{f}: {it:?}");
            assert!(!it.text.is_empty(), "{f}: {it:?}");
            assert_eq!(naming::parse_one(&it.naming.text()).ok().as_ref(), Some(&it.naming), "{f}");
            items += 1;
        }
        for r in Engine.references(&root(), f).unwrap() {
            assert!(r.line >= 1 && r.line <= n, "{f}: {r:?}");
            assert!(Path::new(&root()).join(&r.target.path).is_file(), "{f}: {} is not there", r.target.text());
            assert_eq!(naming::parse_one(&r.target.text()).ok().as_ref(), Some(&r.target), "{f}");
            refs += 1;
        }
    }
    assert!(items > 500 && refs > 80, "{items} items and {refs} references");
}

#[test]
fn a_flow_that_does_not_parse_says_why() {
    let dir = ritsu_testkit::TempDir::new("ports");
    std::fs::write(dir.path().join("broken.flow"), "workflow w v1\n\ntask t(\n").unwrap();
    for said in [Engine.items(dir.path(), "broken.flow").unwrap_err(), Engine.references(dir.path(), "broken.flow").unwrap_err()] {
        assert_eq!(said.len(), 1);
        assert_eq!((said[0].code.as_str(), said[0].line), ("E001", Some(3)), "{said:?}");
    }
    let missing = Engine.items(dir.path(), "missing.flow").unwrap_err();
    assert_eq!(missing[0].code, "", "a file that cannot be read has no code");
}

/// What a flow crosses into, for ritsu's checks across the borders (`Flows::crossings`, ritsu's
/// DESIGN 7.4–7.8): where each value given to a rule, a koyomi date or a chobo transfer can come
/// from, and how long each hold can be held before a call its expiry can refuse, the fewest and the
/// most seconds along every way through the flow.
#[test]
fn a_flow_says_what_it_crosses_into() {
    use ritsu_ports::{Flows, Origin, Ports};
    use std::rc::Rc;
    let t = ritsu_testkit::TempDir::new("crossings");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("weekdays.cal", "calendar weekdays v1\ndescription \"Open Monday to Friday, in UTC\"\noffset +00:00\n\nclosed weekly sat, sun\n");
    write("terms.cal", "dates terms v1\ndescription \"Pays on the 10th of the month after a closing on the 20th, at 09:00\"\nuse calendar \"weekdays.cal\"\n\ninputs\n  received : date  range >=2026-01-01 <=2026-12-20\n\ndate closing = received\n  close day 20\n\ndate payment = closing\n  day 10 of month +1\n  roll preceding\n  at 09:00\n");
    write("stock.book", "book stock v1\ndescription \"Stock, held for 14 days at most\"\n\nunit pcs\n\naccount shelf(sku: string) : pcs\n  at least 0 refused as out_of_stock\naccount suppliers : pcs outside\naccount customers : pcs outside\n\ntransfer receive(delivery: string, sku: string, qty: pcs)\n  key delivery, sku\n  move qty from suppliers to shelf(sku)\n\ntransfer reserve(order: string, sku: string, qty: pcs)\n  key order, sku\n  pending expires after 14 days\n  move qty from shelf(sku) to customers\n");
    let flow = |body: &str| {
        format!(
            "workflow spans v1\ndescription \"Holds goods, then ships or puts them back\"\n\nuse dates terms from \"terms.cal\"\nuse book stock from \"stock.book\"\n\ninputs\n  order : string\n  sku   : string\n  qty   : int  range >=1 <=100\n  fast  : bool\n\ntask reserve(order: string, sku: string, qty: int) -> stock.reserve\n  book stock.reserve.hold\n  starts stock.reserve\n  errors out_of_stock\n  timeout 10 seconds\n\ntask ship(order: string, sku: string) -> stock.reserve\n  book stock.reserve.post\n  sends post\n  errors expired, already_voided\n  timeout 10 seconds\n\ntask put_back(order: string, sku: string) -> stock.reserve\n  book stock.reserve.void\n  sends void\n  errors expired\n\ntask pack(order: string)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:pack\"\n  timeout 5 seconds\n  retry 1 times every 2 seconds\n  idempotent\n\ncase goods : stock.reserve follows stock.reserve\n\nflow\n  goods <- reserve(order: order, sku: sku, qty: qty)\n    on out_of_stock => fail OutOfStock \"nothing left\"\n{body}"
        )
    };
    let ports = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    let spans = |body: &str| {
        write("spans.flow", &flow(body));
        let c = dandori::ports::Engine.crossings(&t.path().join("spans.flow"), &ports).unwrap_or_else(|e| panic!("{e:?}\n{}", flow(body)));
        c.holds.iter().map(|h| (h.line, h.op.clone(), h.least, h.most.clone().ok())).collect::<Vec<_>>()
    };
    let (day, hour) = (86_400, 3_600);
    // a wait, a match whose arms wait differently, a loop of a task with a retry: the fewest takes
    // the shorter arm and no round, the most the longer arm and every round of every attempt
    let body = "  wait 1 day\n  match fast\n    true => pass\n    false => wait 2 days\n  repeat at most 3 times\n    pack(order: order)\n  goods <- ship(order: order, sku: sku)\n    on expired => fail Expired \"too late\"\n";
    assert_eq!(spans(body), vec![(47, "post".to_string(), day, Some(10 + 3 * day + 3 * (2 * 5 + 2) + 10))]);
    // a void with no timeout leaves nothing bounding the most; `on failure` starts at 0 seconds, and
    // a failure can come as late as the void with no timeout runs on
    let body = "  wait 1 day\n  goods <- put_back(order: order, sku: sku)\n    on expired => pass\n\non failure\n  match goods.state\n    none => pass\n    held =>\n      goods <- ship(order: order, sku: sku)\n        on expired => pass\n        on already_voided => pass\n";
    assert_eq!(spans(body), vec![(42, "void".to_string(), day, None), (49, "post".to_string(), 0, None)]);
    // a wait until a koyomi date's time, its date input given `now` after the hold: from the
    // fewest days koyomi counts (18), less the day `now` can fall late in, to the 09:00
    let body = "  let due = terms.payment(received: now)\n  wait until due.at\n  goods <- ship(order: order, sku: sku)\n    on expired => fail Expired \"too late\"\n";
    assert_eq!(spans(body), vec![(43, "post".to_string(), 17 * day + 9 * hour, None)]);
    // `now` read before the hold says nothing of how long after the hold the time comes
    let body = "  wait until due.at\n  goods <- ship(order: order, sku: sku)\n    on expired => fail Expired \"too late\"\n";
    let early = flow(body).replace("flow\n  goods <- reserve", "flow\n  let due = terms.payment(received: now)\n  goods <- reserve");
    write("spans.flow", &early);
    let c = dandori::ports::Engine.crossings(&t.path().join("spans.flow"), &ports).unwrap();
    assert_eq!((c.holds[0].least, c.holds[0].most.is_err()), (0, true));
    // where values come from: the day of a koyomi date, `now`
    assert_eq!(c.dates.len(), 1);
    assert_eq!((c.dates[0].date.as_str(), c.dates[0].input.as_str(), c.dates[0].from.clone()), ("payment", "received", vec![Origin::Now]));
    // the hold's amount comes from the workflow's input, whose range dandori knows
    assert_eq!(c.transfers[0].amounts[0].from, vec![Origin::Range(Some(1), Some(100))]);
}
