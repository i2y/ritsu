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
    let items = Engine::default().items(&root(), file).unwrap();
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
    let items = Engine::default().items(&root(), ja).unwrap();
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
        let items = Engine::default().items(dir.path(), name).unwrap();
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
    let show = |file: &str| -> Vec<String> { Engine::default().references(&root(), file).unwrap().iter().map(|r| format!("{} {} [{}]", r.line, r.target.text(), r.how)).collect() };
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
    let ways = |file: &str| -> Vec<String> { Engine::default().references(&root(), file).unwrap().into_iter().filter(|r| r.target.tool == Tool::Rulec).map(|r| r.how).collect() };
    assert_eq!(ways("tests/flows/connect_rules.flow"), ["use rule … connect, local", "use rule … connect", "use rule … connect"]);
    assert_eq!(ways("tests/flows/local_rules.flow")[..2], ["use rule … lambda, local", "use rule … lambda"]);
    assert_eq!(ways("examples/inquiry/temporal/inquiry.flow"), ["use rule … local"]);
    // an OpenAPI document is openapi's (ritsu's DESIGN 6.2), a Smithy model a file
    assert!(show("examples/hotel/temporal/hotel.ja.flow").contains(&"6 openapi \"examples/hotel/specs/stripe.json\" [use openapi]".to_string()));
    // a dates file is koyomi's, and a book chobo's, with each transfer a task runs an operation of
    assert_eq!(
        show("examples/invoice/invoice.flow"),
        [
            "10 koyomi \"examples/invoice/dates/payment_terms.cal\" [use dates … lambda]",
            "12 chobo \"examples/invoice/books/stock.book\" [use book … lambda]",
            "35 chobo \"examples/invoice/books/stock.book\" transfer reserve [book]",
            "40 chobo \"examples/invoice/books/stock.book\" transfer reserve [book]",
            "45 chobo \"examples/invoice/books/stock.book\" transfer reserve [book]",
        ]
    );
    assert_eq!(ways_of("tests/flows/dates_and_books.flow", Tool::Koyomi), ["use dates … lambda, local"]);
}

/// The ways one tool's files are named by a flow.
fn ways_of(file: &str, tool: Tool) -> Vec<String> {
    Engine::default().references(&root(), file).unwrap().into_iter().filter(|r| r.target.tool == tool && r.target.items.is_empty()).map(|r| r.how).collect()
}

#[test]
fn every_flow_of_the_examples_names_what_is_there() {
    let all = flows();
    assert!(all.len() >= 40, "{all:?}");
    let (mut items, mut refs) = (0, 0);
    for f in &all {
        let n = std::fs::read_to_string(root().join(f)).unwrap().lines().count();
        for it in Engine::default().items(&root(), f).unwrap() {
            assert!(it.lines.0 >= 1 && it.lines.0 <= it.lines.1 && it.lines.1 <= n, "{f}: {it:?}");
            assert!(!it.text.is_empty(), "{f}: {it:?}");
            assert_eq!(naming::parse_one(&it.naming.text()).ok().as_ref(), Some(&it.naming), "{f}");
            items += 1;
        }
        for r in Engine::default().references(&root(), f).unwrap() {
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
    for said in [Engine::default().items(dir.path(), "broken.flow").unwrap_err(), Engine::default().references(dir.path(), "broken.flow").unwrap_err()] {
        assert_eq!(said.len(), 1);
        assert_eq!((said[0].code.as_str(), said[0].line), ("E001", Some(3)), "{said:?}");
    }
    let missing = Engine::default().items(dir.path(), "missing.flow").unwrap_err();
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
        let c = dandori::ports::Engine::default().crossings(&t.path().join("spans.flow"), &ports).unwrap_or_else(|e| panic!("{e:?}\n{}", flow(body)));
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
    // fewest days koyomi counts (18), less the day `now` can fall late in, to the 09:00; at the most,
    // the most days (51) from the latest `now` (10 seconds into the hold, as the hold's call ends), to
    // the 09:00, and the post's own 10 seconds (the date's call, 33 seconds at the most, ends sooner)
    let body = "  let due = terms.payment(received: now)\n  wait until due.at\n  goods <- ship(order: order, sku: sku)\n    on expired => fail Expired \"too late\"\n";
    assert_eq!(spans(body), vec![(43, "post".to_string(), 17 * day + 9 * hour, Some(10 + 51 * day + 9 * hour + 10))]);
    // `timeout until` the same time: a callback's answer, or its timeout, can come at once (the id is
    // handed on first), and at the most as the time comes, a second more for the rounding up (the id
    // is handed on within a minute, long before); the handler of `timeout` alone under an event starts
    // no sooner than the time
    let waits = |body: &str| {
        let tasks = "task ask(order: string, by: timestamp) -> string\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask\"\n  callback\n  timeout until by\n\ntask approval(by: timestamp) -> string\n  event\n  timeout until by\n\ncase goods";
        write("spans.flow", &flow(body).replacen("case goods", tasks, 1));
        let c = dandori::ports::Engine::default().crossings(&t.path().join("spans.flow"), &ports).unwrap();
        c.holds.iter().map(|h| (h.line, h.op.clone(), h.least, h.most.clone().ok(), h.most_why.iter().map(|w| w.en.clone()).collect::<Vec<_>>())).collect::<Vec<_>>()
    };
    let longest = "The longest is when the `now` given to `payment` (line 50) falls on 2026-07-21: `ask` (line 51) waits until `due.at`, 2026-09-10 at 09:00, 51 days after it".to_string();
    let body = "  let due = terms.payment(received: now)\n  let a = ask(order: order, by: due.at)\n    on timeout => pass\n  goods <- ship(order: order, sku: sku)\n    on expired => fail Expired \"too late\"\n";
    assert_eq!(waits(body), vec![(53, "post".to_string(), 0, Some(10 + 51 * day + 9 * hour + 1 + 10), vec![longest])]);
    let body = "  let due = terms.payment(received: now)\n  let a = approval(by: due.at)\n    on timeout =>\n      goods <- put_back(order: order, sku: sku)\n        on expired => pass\n      fail Late \"no approval\"\n  goods <- ship(order: order, sku: sku)\n    on expired => fail Expired \"too late\"\n";
    let held = waits(body);
    assert_eq!(held.iter().map(|h| (h.0, h.1.clone(), h.2, h.3)).collect::<Vec<_>>(), vec![(53, "void".to_string(), 17 * day + 9 * hour, None), (56, "post".to_string(), 0, Some(10 + 51 * day + 9 * hour + 1 + 10))]);
    // a callback hands its id on before it waits, a minute at the most on every target (DESIGN 17):
    // each try of one with `timeout 30 seconds` counts 90 seconds, and its retry the wait between
    let with_tasks = |tasks: &str, body: &str| {
        write("spans.flow", &flow(body).replacen("case goods", &format!("{tasks}\n\ncase goods"), 1));
        let c = dandori::ports::Engine::default().crossings(&t.path().join("spans.flow"), &ports).unwrap();
        c.holds.iter().map(|h| (h.line, h.op.clone(), h.least, h.most.clone().ok())).collect::<Vec<_>>()
    };
    let confirm = "task confirm(order: string) -> string\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:confirm\"\n  callback\n  timeout 30 seconds\n  retry 1 times every 2 seconds\n  idempotent";
    let body = "  let a = confirm(order: order)\n    on timeout => pass\n  goods <- ship(order: order, sku: sku)\n    on expired => fail Expired \"too late\"\n";
    assert_eq!(with_tasks(confirm, body), vec![(50, "post".to_string(), 0, Some(10 + 2 * (60 + 30) + 2 + 10))]);
    // with `timeout until`, a retry after a failure waits between, hands the id on again (a minute)
    // and waits until the time again, a second more for the rounding up
    let ask = "task ask(order: string, by: timestamp) -> string\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask\"\n  callback\n  timeout until by\n  retry 1 times every 5 seconds on failure\n  idempotent";
    let body = "  let due = terms.payment(received: now)\n  let a = ask(order: order, by: due.at)\n    on timeout => pass\n  goods <- ship(order: order, sku: sku)\n    on expired => fail Expired \"too late\"\n";
    assert_eq!(with_tasks(ask, body), vec![(51, "post".to_string(), 0, Some(10 + 51 * day + 9 * hour + 1 + (5 + 60 + 1) + 10))]);
    // `now` read before the hold says nothing of how long after the hold the time comes
    let body = "  wait until due.at\n  goods <- ship(order: order, sku: sku)\n    on expired => fail Expired \"too late\"\n";
    let early = flow(body).replace("flow\n  goods <- reserve", "flow\n  let due = terms.payment(received: now)\n  goods <- reserve");
    write("spans.flow", &early);
    let c = dandori::ports::Engine::default().crossings(&t.path().join("spans.flow"), &ports).unwrap();
    assert_eq!((c.holds[0].least, c.holds[0].most.is_err()), (0, true));
    // where values come from: the day of a koyomi date, `now`
    assert_eq!(c.dates.len(), 1);
    assert_eq!((c.dates[0].date.as_str(), c.dates[0].input.as_str(), c.dates[0].from.clone()), ("payment", "received", vec![Origin::Now]));
    // the hold's amount comes from the workflow's input, whose range dandori knows; no amount can
    // be a rule's output, and the flow is not followed for the calls of rules (X4)
    assert_eq!(c.transfers[0].amounts[0].from, vec![Origin::Range(Some(1), Some(100))]);
    assert_eq!(c.transfers[0].amounts[0].calls, None);
}

/// A date held to a range, and a time, as it goes to a koyomi date or a rule (ritsu's X6, X3 (a)
/// and X2; dandori's DESIGN 20): a date's range is its days (`Origin::Range`, the day numbers
/// rulec and kaname count), a time's its moments (`Origin::Moment`, seconds since 1970 in UTC,
/// with the range as the `.flow` writes it), and so is `now` in a workflow that says the period it
/// runs in, but in `on failure`, which the period does not hold (`Origin::Now`). A date given to a
/// rule's date input from places that each have a range has their days as its range
/// (`CallArg::range`), and none from a place without one.
#[test]
fn a_date_or_a_time_says_its_range_where_it_goes() {
    use ritsu_ports::{day_of, Flows, Origin, Ports};
    use std::rc::Rc;
    let t = ritsu_testkit::TempDir::new("date-origins");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("weekdays.cal", "calendar weekdays v1\ndescription \"Open Monday to Friday, in Tokyo\"\noffset +09:00\n\nclosed weekly sat, sun\n");
    write("terms.cal", "dates terms v1\ndescription \"Pays on the 10th of the month after a closing on the 20th\"\nuse calendar \"weekdays.cal\"\n\ninputs\n  received : date  range >=2025-12-01 <=2027-12-20\n\ndate closing = received\n  close day 20\n\ndate payment = closing\n  day 10 of month +1\n  roll preceding\n");
    write("batch.rule", "rule batch v1\ndescription \"The billing batch a payment day falls in\"\n\nenum run = spring | autumn\n\ninputs\n  pay_day : date  range >=2026-03-01 <=2027-01-31\n\noutputs\n  batch : run\n\ntable pick\npolicy unique\n| pay_day      | -> batch : run |\n| <=2026-08-31 | spring         |\n| >=2026-09-01 | autumn         |\n");
    write(
        "given.flow",
        "workflow given v1\n  now range >=2026-04-01 <=2026-06-30 offset +09:00\ndescription \"Gives dates and times held to ranges where they go\"\n\nuse dates terms from \"terms.cal\"\n\nrecord Visit\n  day : date  range >=2026-03-01 <=2026-08-31\n\ninputs\n  received : date       range >=2026-03-01 <=2026-08-31\n  at       : timestamp  range >=2026-03-01 <=2026-08-31 offset +09:00\n  visit    : Visit\n  open     : date\n\ntask pick_day() -> date  range >=2026-04-01 <=2026-05-31\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:pick-day\"\n  idempotent\n\ntask settle(day: date)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:settle\"\n  idempotent\n\nflow\n  let a = terms.payment(received: received)\n  let b = terms.payment(received: at)\n  let c = terms.payment(received: now)\n  let d = terms.payment(received: visit.day)\n  let picked = pick_day()\n  let e = terms.payment(received: picked)\n  let f = terms.payment(received: open)\n\non failure\n  let g = terms.payment(received: now)\n  settle(day: g.day)\n",
    );
    let ports = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    let c = dandori::ports::Engine::default().crossings(&t.path().join("given.flow"), &ports).unwrap_or_else(|e| panic!("{e:?}"));
    let day = |d: &str| day_of(d).unwrap();
    let days = |a: &str, b: &str| Origin::Range(Some(i128::from(day(a))), Some(i128::from(day(b))));
    // a time's range in an offset: from the first second of its first day there to the last of its last
    let moments = |a: &str, b: &str, text: &str, now: bool| Origin::Moment { lo: Some(day(a) * 86_400 - 9 * 3_600), hi: Some((day(b) + 1) * 86_400 - 9 * 3_600 - 1), text: text.into(), now };
    let from: Vec<Vec<Origin>> = c.dates.iter().map(|d| d.from.clone()).collect();
    assert_eq!(
        from,
        [
            vec![days("2026-03-01", "2026-08-31")],
            vec![moments("2026-03-01", "2026-08-31", ">=2026-03-01 <=2026-08-31 offset +09:00", false)],
            vec![moments("2026-04-01", "2026-06-30", ">=2026-04-01 <=2026-06-30 offset +09:00", true)],
            vec![days("2026-03-01", "2026-08-31")],
            vec![days("2026-04-01", "2026-05-31")],
            vec![Origin::Unknown(ritsu_base::text::Text { ja: "入力 `open`".into(), en: "the input `open`".into() })],
            vec![Origin::Now],
        ]
    );
    // the calls of rules alone, which the port of rules reads
    write("billing.flow", "workflow billing v1\ndescription \"Picks the batch of a day held to a range, and of one that is not\"\n\nuse rule batch from \"batch.rule\"\n\ninputs\n  received : date  range >=2026-03-01 <=2026-08-31\n  open     : date\n\nflow\n  let p = batch(pay_day: received)\n  let q = batch(pay_day: open)\n");
    let calls = dandori::ports::Engine::default().rule_calls(&t.path().join("billing.flow"), Rc::new(rulec::ports::Engine::new())).unwrap_or_else(|e| panic!("{e:?}"));
    let ranges: Vec<_> = calls.iter().map(|c| c.args[0].range).collect();
    assert_eq!(ranges, [Some((Some(i128::from(day("2026-03-01"))), Some(i128::from(day("2026-08-31"))))), None]);
}

/// A number written out in a call of an operation is a number of the parameter's type, in its unit
/// (as dandori's check takes it, and the code it writes passes it): where an integer on the wire
/// counts steps of the unit (`rate[step 0.1%]`), the number written is that many steps, as the code
/// passes it; written with its unit (`fee: 0.5%`), it is the same number, said as written. The
/// same with the names in Japanese.
#[test]
fn a_number_written_out_is_of_the_parameters_unit() {
    use ritsu_ports::{Carried, Flows, Ports};
    use std::rc::Rc;
    let ports = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    let t = ritsu_testkit::TempDir::new("operation-call-written");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("desk.json", "{\"openapi\": \"3.1.0\", \"info\": {\"title\": \"Claims desk\", \"version\": \"1\"}, \"servers\": [{\"url\": \"https://desk.example\"}], \"paths\": {\"/claims/{claimId}/payments\": {\"post\": {\"operationId\": \"payClaim\", \"parameters\": [{\"name\": \"claimId\", \"in\": \"path\", \"required\": true, \"schema\": {\"type\": \"string\"}}], \"requestBody\": {\"required\": true, \"content\": {\"application/json\": {\"schema\": {\"type\": \"object\", \"required\": [\"amount\", \"fee\"], \"properties\": {\"amount\": {\"type\": \"integer\", \"minimum\": 1, \"maximum\": 10000}, \"fee\": {\"type\": \"integer\", \"minimum\": 0, \"maximum\": 100}}}}}}, \"responses\": {\"200\": {\"description\": \"Paid\"}}}}}}\n");
    let pay = "workflow pay v1
description \"Pays a claim an amount and a fee written out\"

use openapi desk from \"desk.json\"

inputs
  claim_id : string

outputs
  done : bool

task pay_claim(claimId: string, amount: money[JPY] range >=1JPY <=10_000JPY, fee: rate[step 0.1%] range >=0 <=100)
  http POST desk \"/claims/{claimId}/payments\"
  idempotent

flow
  pay_claim(claimId: claim_id, amount: 300, fee: 5)
  succeed done = true
";
    write("pay.flow", pay);
    // the same, with the names in Japanese (the operation's parameters keep theirs)
    let ja = pay
        .replace("workflow pay v1", "workflow 支払い v1")
        .replace("claim_id", "請求番号")
        .replace("done", "済み")
        .replace("pay_claim", "支払う");
    write("支払い.flow", &ja);
    // the same numbers written with their units (DESIGN 40): the same values, said as written
    let units = |s: &str| s.replace("amount: 300, fee: 5", "amount: 300JPY, fee: 0.5%");
    write("pay_units.flow", &units(pay));
    write("支払い_単位.flow", &units(&ja));
    let e = Engine::default();
    for (file, shown) in [("pay.flow", ["300", "5"]), ("支払い.flow", ["300", "5"]), ("pay_units.flow", ["300JPY", "0.5%"]), ("支払い_単位.flow", ["300JPY", "0.5%"])] {
        let (_, calls) = e.operation_calls(t.path(), file, &ports).unwrap_or_else(|s| panic!("{file}: {s:?}"));
        let [call] = &calls[..] else { panic!("{file}: {calls:?}") };
        let arg = |name: &str| call.args.iter().find(|(p, _)| p == name).map(|(_, v)| v.holds.clone()).unwrap();
        // 300 yen
        let Carried::Num { unit: Some(unit), min, max, .. } = arg("amount") else { panic!("{file}: {:?}", arg("amount")) };
        assert_eq!((unit.unit.as_str(), min, max), ("JPY", Some(300), Some(300)), "{file}");
        // 5 steps of 0.1%, 0.5%: what the code dandori writes passes
        let Carried::Num { written, unit: Some(unit), min, max } = arg("fee") else { panic!("{file}: {:?}", arg("fee")) };
        assert_eq!((written.as_str(), unit.to_string().as_str(), min, max), ("rate[step 0.1%]", "rate[step 0.1%]", Some(5), Some(5)), "{file}");
        let said = |name: &str| call.args.iter().find(|(p, _)| p == name).map(|(_, v)| v.shown.clone()).unwrap();
        assert_eq!([said("amount"), said("fee")], shown, "{file}");
    }
}

/// A number written with its unit where a rule or a transfer takes it (DESIGN 40) crosses as the
/// whole number of the place's unit, and is said as the flow writes it: the input of a rule
/// (`CallArg`) and the amount of a transfer (`Amount`), `3万円` and `300JPY` where the whole numbers
/// are 30000 and 300, which are said as written too. The same with the names in Japanese.
#[test]
fn a_number_written_with_its_unit_crosses_as_written() {
    use ritsu_ports::{Flows, Origin, Ports};
    use std::rc::Rc;
    let ports = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    let t = ritsu_testkit::TempDir::new("crossings-written");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("urgency.rule", "rule urgency v1\ndescription \"Whether an order goes out in a hurry: a member's always does, and anyone else's from 30,000 yen\"\n\ninputs\n  member : bool\n  amount : money[JPY, incl_tax]  range >=0JPY <=1000000JPY\n\noutputs\n  urgent : bool\n\ntable decide\npolicy unique\n| member | amount     | -> urgent |\n| true   | -          | true      |\n| false  | >=30000JPY | true      |\n| false  | <30000JPY  | false     |\n");
    write("wallet.book", "book wallet v1\ndescription \"Credit a member spends, in yen\"\n\nunit JPY\n\naccount credit(member: string) : JPY\n  at least 0 refused as short\naccount shop : JPY outside\n\ntransfer spend(order: string, member: string, amount: JPY)\n  key order\n  move amount from credit(member) to shop\n");
    let spend = "workflow spend v1
description \"Spends a member's credit on an order, asking first whether it goes out in a hurry\"

use rule urgency from \"urgency.rule\"
use book wallet from \"wallet.book\"

inputs
  order  : string
  member : string
  first  : bool

task spend(order: string, member: string, amount: money[JPY])
  book wallet.spend.do
  errors short

flow
  let u = urgency(member: first, amount: 30000)
  spend(order: order, member: member, amount: 300)
    on short => fail Short \"no credit left\"
";
    // the same, with the names of the flow in Japanese (the task keeps the transfer's)
    let ja = spend
        .replace("workflow spend v1", "workflow 使う v1")
        .replace("  order  : string\n  member : string\n  first  : bool", "  注文番号 : string\n  会員     : string\n  初回     : bool")
        .replace("  let u = urgency(member: first,", "  let 判定 = urgency(member: 初回,")
        .replace("spend(order: order, member: member,", "spend(order: 注文番号, member: 会員,");
    assert_eq!(ja.matches("注文番号").count() + ja.matches("初回").count(), 4, "{ja}");
    let units = |s: &str| s.replace("amount: 30000)", "amount: 3万円)").replace("amount: 300)", "amount: 300JPY)");
    for (file, src) in [("spend.flow", spend.to_string()), ("使う.flow", ja.clone()), ("spend_units.flow", units(spend)), ("使う_単位.flow", units(&ja))] {
        write(file, &src);
        let c = Engine::default().crossings(&t.path().join(file), &ports).unwrap_or_else(|e| panic!("{file}: {e:?}\n{src}"));
        let written = file.contains('_');
        let [rule] = &c.rules[..] else { panic!("{file}: {:?}", c.rules) };
        let arg = rule.args.iter().find(|a| a.input == "amount").unwrap_or_else(|| panic!("{file}: {:?}", rule.args));
        assert_eq!((arg.shown.as_str(), arg.range), (if written { "3万円" } else { "30000" }, Some((Some(30000), Some(30000)))), "{file}");
        let [transfer] = &c.transfers[..] else { panic!("{file}: {:?}", c.transfers) };
        let [amount] = &transfer.amounts[..] else { panic!("{file}: {:?}", transfer.amounts) };
        assert_eq!((amount.shown.as_str(), amount.from.clone()), (if written { "300JPY" } else { "300" }, vec![Origin::Range(Some(300), Some(300))]), "{file}");
    }
}

/// A call of a rule says, for each truth value and each value of an enum it gives, every value it
/// can be on the runs that reach the call (ritsu's X2 reads them for the rule's `never` lines): a
/// value written out is itself, an input any value its range lets in, and in the arm of a `match`
/// on it one of the arm's values, until another value is put in it.
#[test]
fn a_call_of_a_rule_says_the_values_its_truth_values_and_enums_can_be() {
    use ritsu_ports::{Flows, Ports, Value};
    use std::rc::Rc;
    let t = ritsu_testkit::TempDir::new("call-values");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("route.rule", "rule route v1\ndescription \"Where an order of a tier goes\"\n\nenum tier = basic < silver < gold\nenum path = at_once | review\n\ninputs\n  rush  : bool\n  tier  : tier\n  tools : number  range >=0 <=5\n\nnever tier = gold, tools = 0\n\noutputs\n  way : path\n\ntable pick\npolicy unique\n| rush  | -> way : path |\n| true  | at_once       |\n| false | review        |\n");
    write(
        "send.flow",
        "workflow send v1\ndescription \"Asks where an order goes, on several ways through the flow\"\n\nuse rule route from \"route.rule\"\n\ninputs\n  rush  : bool\n  tier  : route.tier\n  held  : route.tier  range >=silver <=gold\n  tools : int  range >=0 <=5\n\ntask pick() -> route.tier\n  http GET \"https://example.com/pick\"\n  idempotent\n\nflow\n  let a = route(rush: rush, tier: tier, tools: tools)\n  let b = route(rush: true, tier: basic, tools: tools)\n  let c = route(rush: rush, tier: held, tools: tools)\n  let chosen = tier\n  match chosen\n    gold => pass\n    basic, silver =>\n      let d = route(rush: rush, tier: chosen, tools: tools)\n      match rush\n        true => pass\n        false =>\n          let e = route(rush: rush, tier: chosen, tools: tools)\n      let chosen = pick()\n      let f = route(rush: rush, tier: chosen, tools: tools)\n",
    );
    let ports = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    let engine = dandori::ports::Engine::default();
    let c = engine.crossings(&t.path().join("send.flow"), &ports).unwrap_or_else(|e| panic!("{e:?}"));
    let shown = |v: &Option<Vec<Value>>| -> String {
        match v {
            None => "-".into(),
            Some(vs) => vs.iter().map(|v| match v { Value::Bool(b) => b.to_string(), Value::Enum(e) => e.clone(), other => format!("{other:?}") }).collect::<Vec<_>>().join("|"),
        }
    };
    let said = |calls: &[ritsu_ports::RuleCall]| -> Vec<String> { calls.iter().map(|c| format!("{}: {}", c.line, c.args.iter().map(|a| format!("{}={}", a.input, shown(&a.values))).collect::<Vec<_>>().join(" "))).collect() };
    let want = [
        "17: rush=false|true tier=basic|silver|gold tools=-",
        "18: rush=true tier=basic tools=-",
        "19: rush=false|true tier=silver|gold tools=-",
        "24: rush=false|true tier=basic|silver tools=-",
        "28: rush=false tier=basic|silver tools=-",
        "30: rush=false|true tier=basic|silver|gold tools=-",
    ];
    assert_eq!(said(&c.rules), want);
    // the calls of rules alone say the same
    let calls = engine.rule_calls(&t.path().join("send.flow"), Rc::new(rulec::ports::Engine::new())).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(said(&calls), want);
}

/// A variable given a value in more than one place is, where a call gives it on, what the runs that
/// reach the call last put in it (DESIGN 1.3): the range of what a rule is given (`CallArg::range`
/// and `unknown`), the places a day given to a koyomi date comes from (`DateCall::from`) and what a
/// call of an operation gives (`OperationCall::args`) say that value, not every value put in the
/// variable anywhere in the flow. After the arms it is either, and a variable the rounds of a loop
/// put a value in is not followed in the loop.
#[test]
fn a_variable_given_on_is_what_the_runs_that_reach_the_call_last_put_in_it() {
    use ritsu_ports::{day_of, Carried, Flows, Origin, Ports};
    use std::rc::Rc;
    let t = ritsu_testkit::TempDir::new("rebound");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("weekdays.cal", "calendar weekdays v1\ndescription \"Open Monday to Friday, in Tokyo\"\noffset +09:00\n\nclosed weekly sat, sun\n");
    write("terms.cal", "dates terms v1\ndescription \"Pays on the 10th of the month after a closing on the 20th\"\nuse calendar \"weekdays.cal\"\n\ninputs\n  received : date  range >=2025-12-01 <=2027-12-20\n\ndate closing = received\n  close day 20\n\ndate payment = closing\n  day 10 of month +1\n  roll preceding\n");
    write("fee.rule", "rule fee v1\ndescription \"Whether a parcel is light or heavy, by its weight\"\n\nenum band = light | heavy\n\ninputs\n  weight : number  range >=0 <=1000\n\noutputs\n  size : band\n\ntable pick\npolicy unique\n| weight | -> size : band |\n| <=100  | light          |\n| >100   | heavy          |\n");
    write("desk.json", "{\"openapi\": \"3.1.0\", \"info\": {\"title\": \"Claims desk\", \"version\": \"1\"}, \"servers\": [{\"url\": \"https://desk.example\"}], \"paths\": {\"/claims/{claimId}/payments\": {\"post\": {\"operationId\": \"payClaim\", \"parameters\": [{\"name\": \"claimId\", \"in\": \"path\", \"required\": true, \"schema\": {\"type\": \"string\"}}], \"requestBody\": {\"required\": true, \"content\": {\"application/json\": {\"schema\": {\"type\": \"object\", \"required\": [\"amount\"], \"properties\": {\"amount\": {\"type\": \"integer\", \"minimum\": 1, \"maximum\": 10000}}}}}}, \"responses\": {\"200\": {\"description\": \"Paid\"}}}}}}\n");
    let flow = "workflow parcel v1
description \"Gives a variable on after another value is put in it on one way\"

use rule fee from \"fee.rule\"
use dates terms from \"terms.cal\"
use openapi desk from \"desk.json\"

inputs
  claim   : string
  express : bool
  few     : int   range >=1 <=10
  many    : int   range >=1 <=100
  open    : int
  early   : date  range >=2026-03-01 <=2026-08-31
  late    : date
  others  : list[int]

task pay_claim(claimId: string, amount: int range >=1 <=10000)
  http POST desk \"/claims/{claimId}/payments\"
  idempotent

flow
  let n = few
  match express
    true =>
      let a = fee(weight: n)
      pay_claim(claimId: claim, amount: n)
    false =>
      let n = many
      let b = fee(weight: n)
      pay_claim(claimId: claim, amount: n)
  let c = fee(weight: n)
  let m = few
  match express
    true =>
      let m = open
      let d = fee(weight: m)
    false =>
      let e = fee(weight: m)
  let day = early
  match express
    true =>
      let f = terms.payment(received: day)
    false =>
      let day = late
      let g = terms.payment(received: day)
  let t = few
  for other in others at most 5
    let h = fee(weight: t)
    let t = other
";
    write("parcel.flow", flow);
    let ports = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    let engine = Engine::default();
    let c = engine.crossings(&t.path().join("parcel.flow"), &ports).unwrap_or_else(|e| panic!("{e:?}"));
    // the calls of the rule, by the variable each answer goes to
    let said = |flow: &str, calls: &[ritsu_ports::RuleCall]| -> Vec<String> {
        calls
            .iter()
            .map(|c| {
                let a = &c.args[0];
                let to = flow.lines().nth(c.line - 1).unwrap().trim().split(' ').nth(1).unwrap().to_string();
                let range = match a.range {
                    Some((lo, hi)) => format!("{lo:?}..{hi:?}"),
                    None => "none".to_string(),
                };
                format!("{to} = fee({}): {range}, {}", a.shown, a.unknown.as_ref().map_or("-", |u| u.en.as_str()))
            })
            .collect()
    };
    let want = [
        // the arm that puts nothing in `n`: the first value; the arm that does: the wider one
        "a = fee(n): Some(1)..Some(10), -",
        "b = fee(n): Some(1)..Some(100), -",
        // after the arms: either
        "c = fee(n): Some(1)..Some(100), -",
        // a value with no range on one way only
        "d = fee(m): none, the input `open`",
        "e = fee(m): Some(1)..Some(10), -",
        // a variable the rounds of a loop put a value in: every value put in it anywhere
        "h = fee(t): none, the input `others`",
    ];
    assert_eq!(said(flow, &c.rules), want);
    // the calls of rules alone, which the port of rules reads, say the same of the flow without its
    // dates
    let days = "  let day = early\n  match express\n    true =>\n      let f = terms.payment(received: day)\n    false =>\n      let day = late\n      let g = terms.payment(received: day)\n";
    assert!(flow.contains(days));
    let plain = flow.replace("use dates terms from \"terms.cal\"\n", "").replace(days, "");
    write("plain.flow", &plain);
    let calls = engine.rule_calls(&t.path().join("plain.flow"), Rc::new(rulec::ports::Engine::new())).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(said(&plain, &calls), want);
    // the places a day comes from: the range of `early` where nothing else is put in `day`, the
    // input with no range where it is
    let day = |d: &str| i128::from(day_of(d).unwrap());
    let from: Vec<Vec<Origin>> = c.dates.iter().map(|d| d.from.clone()).collect();
    assert_eq!(from, [vec![Origin::Range(Some(day("2026-03-01")), Some(day("2026-08-31")))], vec![Origin::Unknown(ritsu_base::text::Text { ja: "入力 `late`".into(), en: "the input `late`".into() })]]);
    // what the calls of the operation give
    let (_, calls) = engine.operation_calls(t.path(), "parcel.flow", &ports).unwrap_or_else(|s| panic!("{s:?}"));
    let gives: Vec<String> = calls
        .iter()
        .map(|call| {
            let (to, v) = &call.args[1];
            match &v.holds {
                Carried::Num { min, max, .. } => format!("{to} = {}: {min:?}..{max:?}", v.shown),
                other => format!("{to} = {}: {other:?}", v.shown),
            }
        })
        .collect();
    assert_eq!(gives, ["amount = n: Some(1)..Some(10)", "amount = n: Some(1)..Some(100)"]);
}

/// The calls of rules whose outputs an amount given to a transfer can be, for ritsu's X4 (ritsu's
/// DESIGN 7.6; dandori's DESIGN 18), along the runs through the flow: what the runs that reach the
/// transfer can give each call, less what the arms of a `match` before the transfer leave out.
#[test]
fn an_amount_says_the_calls_of_rules_it_can_come_from() {
    use ritsu_ports::{Flows, Held, OutputCall, Ports};
    use std::rc::Rc;
    let t = ritsu_testkit::TempDir::new("output-calls");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("seats.rule", "rule seats v1\ndescription \"How many seats an event of a kind needs; a handback gives seats back\"\n\nenum kind = handback | workshop | talk\n\ninputs\n  event_kind : kind\n\noutputs\n  needed : number  round down(1)\n\ntable pick\npolicy unique\n| event_kind | -> needed : number |\n| handback   | -20                |\n| workshop   | 30                 |\n| talk       | 80                 |\n");
    write("extra.rule", "rule extra v1\ndescription \"The seats a late event asks for on top\"\n\ninputs\n  late  : bool\n  asked : number  range >=0 <=100\n\noutputs\n  more : number  round down(1)\n\ntable pick\npolicy unique\n| late  | asked | -> more : number |\n| true  | <=50  | 10               |\n| true  | >50   | 20               |\n| false | -     | 0                |\n");
    write("hall.book", "book hall v1\ndescription \"The seats of the hall\"\n\nunit seat\n\naccount given(event: string) : seat\n  at least 0 refused as not_given\n  at most 300 refused as over_capacity\naccount venue : seat outside\n\ntransfer assign(event: string, count: seat)\n  key event\n  move count from venue to given(event)\n\ntransfer release(event: string, count: seat)\n  key event\n  move count from given(event) to venue\n");
    let flow = |body: &str| {
        format!(
            "workflow booking v1\ndescription \"Gives an event the seats its kind needs\"\n\nuse rule seats from \"seats.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:seats\"\nuse rule extra from \"extra.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:extra\"\nuse book hall from \"hall.book\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:hall\"\n\ninputs\n  event : string\n  kind  : seats.kind\n  other : seats.kind\n  late  : bool\n  asked : int  range >=0 <=40\n  days  : list[string]\n\ntask give_seats(event: string, count: int)\n  book hall.assign.do\n\ntask take_back(event: string, count: int)\n  book hall.release.do\n\ntask ask_organiser(event: string) -> int\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask-organiser\"\n  idempotent\n\nflow\n{body}"
        )
    };
    let ports = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    // a call as `<line> <rule>.<output>: <input>=<value as written> <what it is held to>`, `m`
    // when a `match` on the value before the transfer narrows it; lines from the first of the body
    let show = |c: &OutputCall, base: usize| -> String {
        let inputs: Vec<String> = c
            .inputs
            .iter()
            .map(|i| {
                let held = match &i.held {
                    None => "all".to_string(),
                    Some(Held::Values(v)) => v.join("|"),
                    Some(Held::Range(lo, hi)) => format!("{}..{}", lo.map(|n| n.to_string()).unwrap_or_default(), hi.map(|n| n.to_string()).unwrap_or_default()),
                };
                format!("{}={} {held}{}", i.input, i.shown, if i.matchable { " m" } else { "" })
            })
            .collect();
        let rule = c.rule.file_stem().unwrap().to_string_lossy().to_string();
        format!("{} {rule}.{}: {}", c.line - base, c.output, inputs.join(", "))
    };
    let calls = |body: &str| -> Vec<(usize, Option<Vec<String>>)> {
        let text = flow(body);
        write("booking.flow", &text);
        let base = text.lines().position(|l| l == "flow").unwrap() + 1;
        let c = dandori::ports::Engine::default().crossings(&t.path().join("booking.flow"), &ports).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
        c.transfers.iter().map(|tr| (tr.line - base, tr.amounts[0].calls.as_ref().map(|cs| cs.iter().map(|c| show(c, base)).collect()))).collect()
    };
    let some = |line: usize, calls: &[&str]| (line, Some(calls.iter().map(|c| c.to_string()).collect::<Vec<_>>()));
    // no `match`: the call is given every kind
    let body = "  let need = seats(event_kind: kind)\n  give_seats(event: event, count: need.needed)\n";
    assert_eq!(calls(body), [some(2, &["1 seats.needed: event_kind=kind all m"])]);
    // the transfer in an arm of a `match` on the value after the call, or the call and the
    // transfer in it: the call is given the arm's values
    let body = "  let need = seats(event_kind: kind)\n  match kind\n    handback => pass\n    workshop, talk => give_seats(event: event, count: need.needed)\n";
    assert_eq!(calls(body), [some(4, &["1 seats.needed: event_kind=kind workshop|talk m"])]);
    let body = "  match kind\n    handback => pass\n    workshop, talk =>\n      let need = seats(event_kind: kind)\n      give_seats(event: event, count: need.needed)\n";
    assert_eq!(calls(body), [some(5, &["4 seats.needed: event_kind=kind workshop|talk m"])]);
    // an arm that ends in `fail` is not joined: after the `match`, the value is one of the others
    let body = "  match kind\n    handback => fail Handback \"a handback gives no seats\"\n    workshop, talk => pass\n  let need = seats(event_kind: kind)\n  give_seats(event: event, count: need.needed)\n";
    assert_eq!(calls(body), [some(5, &["4 seats.needed: event_kind=kind workshop|talk m"])]);
    // arms joined again give the call every value they took
    let body = "  let need = seats(event_kind: kind)\n  match kind\n    handback => pass\n    workshop, talk => pass\n  give_seats(event: event, count: need.needed)\n";
    assert_eq!(calls(body), [some(5, &["1 seats.needed: event_kind=kind all m"])]);
    // another value put in the variable after the call: a `match` on it says nothing of the call
    let body = "  let k = kind\n  let need = seats(event_kind: k)\n  let k = other\n  match k\n    handback => pass\n    workshop, talk => give_seats(event: event, count: need.needed)\n";
    assert_eq!(calls(body), [some(6, &["2 seats.needed: event_kind=k all"])]);
    // the output put in a variable, and the variable given another value before the transfer: no
    // run gives the transfer the rule's output
    let body = "  let need = seats(event_kind: kind)\n  let count = need.needed\n  let count = ask_organiser(event: event)\n  give_seats(event: event, count: count)\n";
    assert_eq!(calls(body), [some(4, &[])]);
    // a value written out, and the output put in a variable the transfer is given
    let body = "  let need = seats(event_kind: workshop)\n  give_seats(event: event, count: need.needed)\n";
    assert_eq!(calls(body), [some(2, &["1 seats.needed: event_kind=workshop workshop"])]);
    let body = "  let need = seats(event_kind: kind)\n  let n = need.needed\n  match kind\n    handback => pass\n    workshop, talk => give_seats(event: event, count: n)\n";
    assert_eq!(calls(body), [some(5, &["1 seats.needed: event_kind=kind workshop|talk m"])]);
    // a call in each arm: the transfer after them is given either, each with what its arm gave it
    let body = "  match kind\n    handback => let need = seats(event_kind: talk)\n    workshop, talk => let need = seats(event_kind: kind)\n  give_seats(event: event, count: need.needed)\n";
    assert_eq!(calls(body), [some(4, &["2 seats.needed: event_kind=talk talk", "3 seats.needed: event_kind=kind workshop|talk m"])]);
    // a handler starts from before the call, which gives its variable nothing when it fails
    let body = "  let need = seats(event_kind: talk)\n  let need = seats(event_kind: kind)\n    on timeout => pass\n  give_seats(event: event, count: need.needed)\n";
    assert_eq!(calls(body), [some(4, &["1 seats.needed: event_kind=talk talk", "2 seats.needed: event_kind=kind all m"])]);
    // a loop is walked round after round until what it holds stops changing: inside it, what a
    // round gives a variable; after it, what the rounds leave in it (a `repeat` goes round at
    // least once, so the value from before the loop does not come out of it)
    let body = "  let need = seats(event_kind: workshop)\n  repeat at most 2 times\n    let need = seats(event_kind: kind)\n    give_seats(event: event, count: need.needed)\n  take_back(event: event, count: need.needed)\n";
    assert_eq!(calls(body), [some(4, &["3 seats.needed: event_kind=kind all m"]), some(5, &["3 seats.needed: event_kind=kind all m"])]);
    // a `for` may go round no time: after it, the value from before it, or what a round left
    let body = "  let need = seats(event_kind: workshop)\n  for day in days at most 7\n    let need = seats(event_kind: kind)\n  take_back(event: event, count: need.needed)\n";
    assert_eq!(calls(body), [some(4, &["1 seats.needed: event_kind=workshop workshop", "3 seats.needed: event_kind=kind all m"])]);
    // `on failure` starts where a task's failure stops the flow: here, only at the transfer, after
    // the `match` whose other arm fails (a `fail` does not run `on failure`)
    let body = "  let need = seats(event_kind: kind)\n    on timeout => fail Slow \"the rule did not answer\"\n    on failure => fail Broken \"the rule failed\"\n  match kind\n    handback => fail Handback \"a handback gives no seats\"\n    workshop, talk => pass\n  give_seats(event: event, count: need.needed)\n\non failure\n  take_back(event: event, count: need.needed)\n";
    assert_eq!(calls(body), [some(7, &["1 seats.needed: event_kind=kind workshop|talk m"]), some(10, &["1 seats.needed: event_kind=kind workshop|talk m"])]);
    // a bool is held as an enum is; a number to the range X2 gives the call, when it is less than
    // the rule's input takes; a value written out, to itself
    let body = "  let x = extra(late: late, asked: asked)\n  match late\n    true => pass\n    false => give_seats(event: event, count: x.more)\n";
    assert_eq!(calls(body), [some(4, &["1 extra.more: late=late false m, asked=asked 0..40"])]);
    let body = "  let x = extra(late: true, asked: 30)\n  give_seats(event: event, count: x.more)\n";
    assert_eq!(calls(body), [some(2, &["1 extra.more: late=true true, asked=30 30..30"])]);
}

/// The calls of operations of contracts a flow makes, for sekisho's check of what a workflow is
/// allowed (`Flows::operation_calls`, sekisho's X16): each task call bound to an OpenAPI operation
/// (`http` on a `use openapi`) or a Connect method (`connect` on a `use proto`), named by the
/// reference a gate's `guards` names it by, from the root, with the error the task declares for a
/// denial.
#[test]
fn a_flow_says_which_operations_it_calls() {
    use ritsu_ports::{Flows, Ports};
    use std::rc::Rc;
    let ports = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    let show = |root: &Path, file: &str| -> (String, Vec<String>) {
        let (name, calls) = Engine::default().operation_calls(root, file, &ports).unwrap_or_else(|e| panic!("{file}: {e:?}"));
        (name, calls.iter().map(|c| format!("{} {} {} [{}]", c.line, c.task, c.operation.text(), c.denied.as_deref().unwrap_or("-"))).collect())
    };
    // Stripe's operations by their operationId; a call in a handler is a call too
    let (name, calls) = show(&root(), "examples/hotel/temporal/hotel.flow");
    assert_eq!(name, "hotel_stay");
    assert!(calls.iter().all(|c| c.contains(" openapi \"examples/hotel/specs/stripe.json\" operation ")), "{calls:?}");
    assert!(calls.iter().any(|c| c.ends_with("create_intent openapi \"examples/hotel/specs/stripe.json\" operation PostPaymentIntents [-]")), "{calls:?}");
    assert!(calls.iter().any(|c| c.ends_with("get_intent openapi \"examples/hotel/specs/stripe.json\" operation GetPaymentIntentsIntent [-]")), "{calls:?}");
    // a Connect method by its service and method; a child flow and a callback are no operation
    let (_, calls) = show(&root(), "examples/fulfillment/temporal/fulfillment.flow");
    assert!(!calls.is_empty() && calls.iter().all(|c| c.contains(" proto \"examples/fulfillment/specs/warehouse.proto\" service StockService method ")), "{calls:?}");
    // the error of a denial: 403 for an HTTP operation, `permission_denied` for a Connect method;
    // an operation with no operationId by its method and path
    let t = ritsu_testkit::TempDir::new("operation-calls");
    let write = |name: &str, body: &str| {
        let p = t.path().join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    };
    write("api/orders.json", "{\"openapi\": \"3.1.0\", \"info\": {\"title\": \"Orders\", \"version\": \"1\"}, \"servers\": [{\"url\": \"https://orders.example.com\"}], \"paths\": {\"/orders/{orderId}/refunds\": {\"post\": {\"operationId\": \"refundOrder\", \"parameters\": [{\"name\": \"orderId\", \"in\": \"path\", \"required\": true, \"schema\": {\"type\": \"string\"}}], \"responses\": {\"201\": {\"description\": \"refunded\"}, \"403\": {\"description\": \"denied\"}}}}, \"/orders/{orderId}\": {\"get\": {\"parameters\": [{\"name\": \"orderId\", \"in\": \"path\", \"required\": true, \"schema\": {\"type\": \"string\"}}], \"responses\": {\"200\": {\"description\": \"the order\"}}}}}}\n");
    write("proto/stock.proto", "syntax = \"proto3\";\n\npackage shop.v1;\n\nservice Stock {\n  rpc Reserve(ReserveRequest) returns (ReserveResponse);\n}\n\nmessage ReserveRequest {\n  string sku = 1;\n}\n\nmessage ReserveResponse {\n  string id = 1;\n}\n");
    let flow = "workflow refunds v1\ndescription \"Looks at an order, refunds it and holds its stock\"\n\nuse openapi orders from \"api/orders.json\"\nuse proto stock from \"proto/stock.proto\"\n  url \"https://stock.example.com\"\n\ninputs\n  order : string\n\ntask look(orderId: string)\n  http GET orders \"/orders/{orderId}\"\n  idempotent\n\ntask refund(orderId: string)\n  http POST orders \"/orders/{orderId}/refunds\"\n  errors denied = 403\n  key\n\ntask reserve(sku: string) -> stock.ReserveResponse\n  connect stock \"shop.v1.Stock/Reserve\"\n  errors denied = permission_denied\n  key\n\nflow\n  look(orderId: order)\n  refund(orderId: order)\n    on denied => fail Denied \"the refund is not allowed\"\n  let held = reserve(sku: order)\n    on denied => fail Denied \"the stock is not ours to hold\"\n";
    write("refunds.flow", flow);
    let (name, calls) = show(t.path(), "refunds.flow");
    assert_eq!(name, "refunds");
    assert_eq!(
        calls,
        [
            "26 look openapi \"api/orders.json\" operation \"GET /orders/{orderId}\" [-]",
            "27 refund openapi \"api/orders.json\" operation refundOrder [denied]",
            "29 reserve proto \"proto/stock.proto\" service Stock method Reserve [denied]",
        ]
    );
    // a flow that does not pass dandori's check says why
    write("broken.flow", &flow.replace("look(orderId: order)\n", "look(orderId: nothing)\n"));
    assert!(Engine::default().operation_calls(t.path(), "broken.flow", &ports).unwrap_err().iter().any(|s| s.code.starts_with('E')));
}

/// What a call of an operation gives, and what the runs that reach it hold there
/// (`OperationCall::args` and `reached`, sekisho's X16): each value by the task's parameter, a
/// number in the range the flow knows it in and an enum with the values its places let in; each
/// value a `match` on the way has read with what its arms leave it at the call; the calls of rules
/// whose answers the variables hold. `Flows::value` says what a value holds wherever it is read,
/// and `Flows::value_names` the names a message lists where there is no such value.
#[test]
fn a_call_of_an_operation_says_what_it_gives_and_what_the_runs_hold() {
    use ritsu_ports::{Carried, Flows, Ports};
    use std::rc::Rc;
    let ports = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    let t = ritsu_testkit::TempDir::new("operation-call-values");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("desk.json", "{\"openapi\": \"3.1.0\", \"info\": {\"title\": \"Claims desk\", \"version\": \"1\"}, \"servers\": [{\"url\": \"https://desk.example\"}], \"paths\": {\"/claims/{claimId}/payments\": {\"post\": {\"operationId\": \"payClaim\", \"parameters\": [{\"name\": \"claimId\", \"in\": \"path\", \"required\": true, \"schema\": {\"type\": \"string\"}}], \"requestBody\": {\"required\": true, \"content\": {\"application/json\": {\"schema\": {\"type\": \"object\", \"required\": [\"amount\", \"channel\"], \"properties\": {\"amount\": {\"type\": \"integer\", \"minimum\": 1, \"maximum\": 10000}, \"channel\": {\"type\": \"string\", \"enum\": [\"bank\", \"cash\"]}}}}}}, \"responses\": {\"200\": {\"description\": \"Paid\"}}}}}}\n");
    write("who.rule", "rule who_pays(k) v1\ndescription \"Who pays a claim: the desk up to 500 yen, a person above it\"\n\nenum payer = desk \"Desk\" | person \"Person\"\n\ninputs\n  amount : money[JPY]  range >=1JPY <=10_000JPY\n\noutputs\n  who : payer\n\ntable t\n| amount   | -> who |\n| <=500JPY | desk   |\n| >500JPY  | person |\n\nexamples\n| amount | -> who |\n| 500JPY | desk   |\n| 501JPY | person |\n");
    let flow = "workflow pay v1
description \"Pays the claims the desk may pay\"

use rule k from \"who.rule\"
use openapi desk from \"desk.json\"

enum channel = bank | cash

record Claim
  id    : string
  total : money[JPY]  range >=1JPY <=10_000JPY

inputs
  claim   : Claim
  amount  : money[JPY]  range >=1JPY <=600JPY
  channel : channel

outputs
  done : bool

task pay_claim(claimId: string, amount: money[JPY] range >=1JPY <=10_000JPY, channel: channel)
  http POST desk \"/claims/{claimId}/payments\"
  idempotent

flow
  let r = k(amount: amount)
  match r.who
    desk =>
      match channel
        bank =>
          pay_claim(claimId: claim.id, amount: amount, channel: channel)
          succeed done = true
        cash => succeed done = false
    person => succeed done = false
";
    write("pay.flow", flow);
    let e = Engine::default();
    let (_, calls) = e.operation_calls(t.path(), "pay.flow", &ports).unwrap_or_else(|s| panic!("{s:?}"));
    let [call] = &calls[..] else { panic!("{calls:?}") };
    // what the call gives: the field it reads, the amount in the input's range, the channel by
    // its type (what the arm leaves of it is with what the runs hold, below)
    let gives: Vec<String> = call
        .args
        .iter()
        .map(|(to, v)| {
            let holds = match &v.holds {
                Carried::Num { min, max, .. } => format!("{min:?}..{max:?}"),
                Carried::Enum { values, .. } => values.iter().map(|x| x.written.as_str()).collect::<Vec<_>>().join("|"),
                Carried::Str => "string".to_string(),
                other => format!("{other:?}"),
            };
            format!("{to} = {} ({:?}): {holds}", v.shown, v.path.as_deref())
        })
        .collect();
    assert_eq!(gives, ["claimId = claim.id (Some(\"claim.id\")): string", "amount = amount (Some(\"amount\")): Some(1)..Some(600)", "channel = channel (Some(\"channel\")): bank|cash"]);
    // what the runs that reach it hold
    let reached = call.reached.as_ref().expect("the runs are followed to the call");
    assert_eq!(reached.held, [("channel".to_string(), vec!["bank".to_string()]), ("r.who".to_string(), vec!["desk".to_string()])]);
    let [(var, answers)] = &reached.answers[..] else { panic!("{:?}", reached.answers) };
    let asked_on = flow.lines().position(|l| l.contains("let r = k(")).unwrap() + 1;
    assert_eq!((var.as_str(), answers.len(), answers[0].line), ("r", 1, asked_on));
    assert!(answers[0].rule.ends_with("who.rule"), "{:?}", answers[0].rule);
    assert_eq!(answers[0].inputs.iter().map(|(n, v)| (n.as_str(), v.shown.as_str())).collect::<Vec<_>>(), [("amount", "amount")]);
    for name in ["claim", "amount", "channel", "r"] {
        assert!(reached.set.iter().any(|s| s == name), "{name}: {:?}", reached.set);
    }
    // a value wherever it is read: every value of the enum, the field's range; none for a name the
    // flow does not have, and the names it has for the message that says so
    let value = |path: &str| e.value(t.path(), "pay.flow", &ports, path).unwrap();
    assert!(matches!(value("channel").map(|v| v.holds), Some(Carried::Enum { values, .. }) if values.len() == 2));
    assert!(matches!(value("claim.total").map(|v| v.holds), Some(Carried::Num { min: Some(1), max: Some(10_000), .. })));
    assert_eq!(value("claim.sum"), None);
    assert_eq!(value("order"), None);
    let mut names = e.value_names(t.path(), "pay.flow", &ports, "").unwrap();
    names.sort();
    assert_eq!(names, ["amount", "channel", "claim", "r"]);
    assert_eq!(e.value_names(t.path(), "pay.flow", &ports, "claim").unwrap(), ["id", "total"]);
    assert!(e.value_names(t.path(), "pay.flow", &ports, "amount").unwrap().is_empty());
    assert!(e.value_names(t.path(), "pay.flow", &ports, "order").unwrap().is_empty());
}

/// One engine keeps the check it makes of a flow (DESIGN 31): the questions the checks across the
/// languages ask of one flow in a run are answered by one check, while the flow, every file the
/// check read and the ports are the same. A change to any of them checks the flow again, and the
/// answers follow it.
#[test]
fn a_flow_asked_many_things_is_checked_once() {
    use ritsu_ports::{Carried, Feeds, Flows, Ports};
    use std::rc::Rc;
    let t = ritsu_testkit::TempDir::new("kept-checks");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    let rule = "rule extra v1\ndescription \"The seats a late event asks for on top\"\n\ninputs\n  late  : bool\n  asked : number  range >=0 <=100\n\noutputs\n  more : number  round down(1)\n\ntable pick\npolicy unique\n| late  | asked | -> more : number |\n| true  | <=50  | 10               |\n| true  | >50   | 20               |\n| false | -     | 0                |\n";
    let flow = "workflow seats v1\ndescription \"Asks how many seats more a late event needs\"\n\nuse rule extra from \"extra.rule\"\n\ninputs\n  late  : bool\n  asked : int  range >=0 <=100\n\noutputs\n  more : int\n\nflow\n  let e = extra(late: late, asked: asked)\n  succeed more = e.more\n";
    write("extra.rule", rule);
    write("seats.flow", flow);
    let ports = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    let e = Engine::default();
    let file = t.path().join("seats.flow");
    let asked = naming::Name::file(Tool::Dandori, "seats.flow").with("input", "asked");
    let range = |e: &Engine, ports: &Ports| e.holds(t.path(), "seats.flow", &asked, ports).unwrap_or_else(|s| panic!("{s:?}"));
    // `ritsu check`'s report, and what the checks across the borders ask of the flow
    let units = e.checked_with(t.path(), &[file.to_string_lossy().to_string()], &ports, ritsu_base::text::Lang::En);
    assert_eq!(units[0].verdict, ritsu_ports::Verdict::Passes);
    assert_eq!(e.crossings(&file, &ports).unwrap().rules.len(), 1);
    assert!(e.sends(&file, &ports).unwrap().is_empty());
    assert!(e.operation_calls(t.path(), "seats.flow", &ports).unwrap().1.is_empty());
    assert_eq!(range(&e, &ports), Some(Carried::whole("int", Some(0), Some(100))));
    assert_eq!(e.checks(), 1);
    // the flow written otherwise is checked again, and answers as it is written now
    write("seats.flow", &flow.replace("range >=0 <=100", "range >=0 <=50"));
    assert_eq!(range(&e, &ports), Some(Carried::whole("int", Some(0), Some(50))));
    assert_eq!(e.checks(), 2);
    // a file the check read written otherwise, the rule here: the flow no longer passes
    write("extra.rule", &rule.replace("more", "seats"));
    let said = e.crossings(&file, &ports).unwrap_err();
    assert!(said.iter().any(|s| s.code == "E002" && s.message.en.contains("more")), "{said:?}");
    assert_eq!(e.checks(), 3);
    write("extra.rule", rule);
    assert_eq!(range(&e, &ports), Some(Carried::whole("int", Some(0), Some(50))));
    // the check of the flow as it was before the rule changed is kept, and taken again
    assert_eq!(e.checks(), 3);
    // other ports, even with the same files: a check of their own
    let other = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    assert_eq!(range(&e, &other), Some(Carried::whole("int", Some(0), Some(50))));
    assert_eq!(e.checks(), 4);
    assert_eq!(range(&e, &ports), Some(Carried::whole("int", Some(0), Some(50))));
    assert_eq!(e.checks(), 4);
}

/// A flow is one file however its path is written (ritsu's DESIGN 4.15): asked under four spellings
/// of its path in a run, it is checked once, and what the port says of one that does not pass names
/// it one way. Written otherwise, a flow is checked again, whichever way it is asked.
#[test]
fn a_flow_is_one_file_however_its_path_is_written() {
    use ritsu_ports::{Flows, Ports};
    use std::rc::Rc;
    let t = ritsu_testkit::TempDir::new("kept-spelled");
    let rule = "rule extra v1\ndescription \"The seats a late event asks for on top\"\n\ninputs\n  late  : bool\n  asked : number  range >=0 <=100\n\noutputs\n  more : number  round down(1)\n\ntable pick\npolicy unique\n| late  | asked | -> more : number |\n| true  | <=50  | 10               |\n| true  | >50   | 20               |\n| false | -     | 0                |\n";
    let flow = "workflow seats v1\ndescription \"Asks how many seats more a late event needs\"\n\nuse rule extra from \"extra.rule\"\n\ninputs\n  late  : bool\n  asked : int  range >=0 <=100\n\noutputs\n  more : int\n\nflow\n  let e = extra(late: late, asked: asked)\n  succeed more = e.more\n";
    t.write("extra.rule", rule);
    t.write("seats.flow", flow);
    t.write("wrong.flow", flow.replace("succeed more = e.more", "succeed more = e.fewer"));
    let _run = ritsu_base::paths::run_from(".");
    let ports = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    let e = Engine::default();
    for p in t.spellings("seats.flow") {
        assert_eq!(e.crossings(&p, &ports).unwrap_or_else(|s| panic!("{}: {s:?}", p.display())).rules.len(), 1);
    }
    assert_eq!(e.checks(), 1, "one check for every spelling");
    let said: Vec<_> = t.spellings("wrong.flow").iter().map(|p| e.crossings(p, &ports).unwrap_err()).collect();
    assert!(!said[0].is_empty() && said[0].iter().all(|s| !s.code.is_empty()), "{:?}", said[0]);
    assert!(said.windows(2).all(|w| w[0] == w[1]), "{said:?}");
    assert_eq!(e.checks(), 2);
    t.write("seats.flow", format!("{flow}\n# the flow written otherwise\n"));
    for p in t.spellings("seats.flow") {
        assert_eq!(e.crossings(&p, &ports).unwrap_or_else(|s| panic!("{}: {s:?}", p.display())).rules.len(), 1);
    }
    assert_eq!(e.checks(), 3, "the flow written otherwise, checked again once");
}

/// A child flow is asked of the engine that is checking its parent (DESIGN 35): run by nine tasks of
/// one flow, it is checked once. What the check says of the children, the one that is not there and
/// the one that does not pass among them, is what the flow's check says on its own, without an
/// engine.
#[test]
fn a_child_flow_is_checked_once_and_said_as_on_its_own() {
    let file = root().join("tests/fixtures/children.flow");
    let (_, alone) = dandori::check::check_file(&file).unwrap();
    ritsu_base::tally::take();
    let e = Engine::default();
    let (_, kept) = e.lending(|| dandori::check::check_kept(&file)).unwrap();
    assert_eq!(format!("{:?}", kept.diags), format!("{:?}", alone.diags));
    // the flow, `children/査定.flow` (run by nine tasks) and `children/自分.flow`; the flow that is
    // not there is read, not checked
    assert_eq!(e.checks(), 3);
    let more: Vec<_> = ritsu_base::tally::take().into_iter().filter(|(_, _, n)| *n > 1).collect();
    assert!(more.is_empty(), "checked more than once: {more:?}");
}

/// What `ritsu run` runs (`Engine::built`, DESIGN 32) is the check the engine keeps: the
/// preconditions ritsu could not decide are asked of the same flow by the same path, and the one
/// check answers both. The model has a check of each of them, and the answers of the port after it
/// take the same check.
#[test]
fn a_flow_built_for_a_run_is_checked_once_with_what_it_asks() {
    use dandori::model::TK;
    use ritsu_ports::{Flows, Ports, Undecided, UndecidedPrecondition};
    use std::rc::Rc;
    // every precondition of every rule the flow calls, undecided, asked of the flow as ritsu asks it
    struct Asks {
        engine: Rc<Engine>,
        ports: Ports,
    }
    impl Undecided for Asks {
        fn preconditions(&self, file: &Path) -> Vec<UndecidedPrecondition> {
            let calls = self.engine.crossings(file, &self.ports).map(|c| c.rules).unwrap_or_default();
            let mut out = Vec::new();
            for call in calls {
                for p in self.ports.rules.facts(&call.rule).map(|f| f.preconditions).unwrap_or_default() {
                    out.push(UndecidedPrecondition { line: call.line, rule: call.rule.clone(), precondition: p });
                }
            }
            out
        }
    }
    let t = ritsu_testkit::TempDir::new("built-checks");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("refund.rule", "rule refund v1\ndescription \"Pays a refund back at once or looks at it first\"\n\nenum path = at_once | review\n\ninputs\n  paid  : number  range >=0 <=10000\n  asked : number  range >=0 <=10000\n\nconstraint asked <= paid\n\noutputs\n  route : path\n\ntable pick\npolicy unique\n| asked | -> route : path |\n| <=100 | at_once         |\n| >100  | review          |\n");
    write("refunds.flow", "workflow refunds v1\ndescription \"Pays a refund back or looks at it\"\n\nuse rule refund from \"refund.rule\"\n\ninputs\n  order : string\n  paid  : int  range >=0 <=10000\n\ntask ask_amount(order: string) -> int\n  lambda \"arn:aws:lambda:eu-west-2:123456789012:function:ask-amount\"\n  idempotent\n\nflow\n  let asked = ask_amount(order: order)\n  let decision = refund(paid: paid, asked: asked)\n  succeed\n");
    let ports = Ports { rules: Rc::new(rulec::ports::Engine::new()), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) };
    let e = Rc::new(Engine::default());
    let file = t.path().join("refunds.flow");
    let asks = Rc::new(Asks { engine: e.clone(), ports: ports.clone() });
    let (_, checked) = e.built(&file, &ports, asks).unwrap();
    let model = checked.model.expect("the flow passes");
    assert_eq!(model.all_stmts().iter().filter(|s| matches!(s.kind, TK::Check(_))).count(), 1);
    assert_eq!(e.checks(), 1);
    // what the checks across the borders ask after it
    assert_eq!(e.crossings(&file, &ports).unwrap().rules.len(), 1);
    assert_eq!(e.checks(), 1);
}
