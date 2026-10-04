//! The example (DESIGN 11; PLAN C.0): its map passes check with the numbers DESIGN 3.1 shows, with
//! every language joined as `ritsu sakai` joins them; the binary of sakai's own crate says what it
//! cannot read; what was copied from the suite still passes each language's own check (rulec,
//! koyomi, chobo, dandori, through ritsu's ports); the two protos written for it pass buf's lint;
//! and a change to it is caught.

mod common;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[test]
fn the_example_passes_check() {
    let (code, out, err) = common::joined(&["check", "examples/通販/通販.ctx"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, "examples/通販/通販.ctx: ok — 5 contexts, 7 relationships; 79 artifacts, each in one context; 9 crossings checked (proto 1, rulec 2, koyomi 1, dandori 5)\n");
    // What crosses: ordering's code takes inventory's types, as a conformist may; billing's rules
    // take ordering's order status in its layer and delivery's shipment as delivery's customer;
    // delivery's dates read billing's calendar from their shared kernel; ordering's workflow calls
    // delivery's rule and inventory's service, and runs delivery's workflow, as partners.
    let ex = std::fs::canonicalize(common::EXAMPLE).unwrap();
    let out = sakai::check::check_map_with(&ex, "通販.ctx", &common::suite()).unwrap();
    let c = out.checked.as_ref().unwrap();
    let crossings: Vec<(String, String, String, String, String)> = c
        .crossings
        .iter()
        .map(|x| (x.from.clone(), x.kind.via(), x.to_name().text(), c.model.contexts[x.from_ctx].name.clone(), c.model.contexts[x.to_ctx].name.clone()))
        .collect();
    let want: Vec<(String, String, String, String, String)> = [
        ("proto/shop/ordering/v1/fulfillment.proto", "proto import", "proto \"proto/warehouse/v1/stock.proto\"", "受注", "在庫"),
        ("billing/rules/出荷の送料.rule", "shape", "proto \"proto/shop/delivery/v1/shipment.proto\" message CreateShipmentRequest", "請求", "配送"),
        ("billing/rules/請求の要否.rule", "import proto", "proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus", "請求", "受注"),
        ("delivery/出荷日.cal", "use calendar", "koyomi \"calendars/東京の営業日.cal\"", "配送", "請求"),
        ("ordering/受注.flow", "use rule … connect", "rulec \"delivery/rules/出荷の急ぎ.rule\"", "受注", "配送"),
        ("ordering/受注.flow", "use proto", "proto \"proto/warehouse/v1/stock.proto\"", "受注", "在庫"),
        ("ordering/受注.flow", "connect", "proto \"proto/warehouse/v1/stock.proto\" service StockService method Reserve", "受注", "在庫"),
        ("ordering/受注.flow", "connect", "proto \"proto/warehouse/v1/stock.proto\" service StockService method Release", "受注", "在庫"),
        ("ordering/受注.flow", "flow", "dandori \"delivery/配送の手配.flow\"", "受注", "配送"),
    ]
    .iter()
    .map(|(a, b, c, d, e)| (a.to_string(), b.to_string(), c.to_string(), d.to_string(), e.to_string()))
    .collect();
    assert_eq!(crossings, want);
    assert_eq!(c.crossings[0].allowed, Some(sakai::refs::Allowed::Upstream(0)));
    assert_eq!(c.crossings[8].allowed, Some(sakai::refs::Allowed::Partnership));
}

/// The binary of sakai's own crate holds no other language (ritsu's DESIGN 2.3): on the example it
/// says, once a language, that it cannot read its artifacts (E104), and to run `ritsu sakai`, with
/// exit 2 (where sakai runs, not what the map says).
#[test]
fn the_binary_of_this_crate_says_what_it_cannot_read() {
    let o = common::sakai(&["check", "examples/通販/通販.ctx"]);
    assert_eq!(o.status.code(), Some(2));
    for cmd in [vec!["api", "examples/通販/通販.ctx"], vec!["export", "cml", "examples/通販/通販.ctx"], vec!["build", "examples/通販/通販.ctx", "--target", "import-linter", "--check"]] {
        assert_eq!(common::sakai(&cmd).status.code(), Some(2), "{cmd:?}");
    }
    let out = String::from_utf8_lossy(&o.stdout).to_string();
    assert_eq!(common::printed_codes(&out), ["E104", "E104", "E104"], "{out}");
    assert!(out.contains("This sakai cannot read rulec artifacts (4 of them, the first examples/通販/billing/rules/出荷の送料.rule)"), "{out}");
    assert!(out.contains("`ritsu sakai check examples/通販/通販.ctx`"), "{out}");
}

/// What was copied from the suite still passes each language's own check, read through ritsu's
/// ports in this process, as `ritsu sakai` reads it (ritsu's DESIGN 3.3): rulec answers for each
/// rule, koyomi for each dates file and calendar, chobo for the book, and dandori checks each
/// workflow with the rules it uses joined.
#[test]
fn what_was_copied_passes_the_suite() {
    use ritsu_ports::{Books, Dates, Rules};
    let ex = std::fs::canonicalize(common::EXAMPLE).unwrap();
    let rules = rulec::ports::Engine::new();
    let mut failures = Vec::new();
    for f in ["billing/rules/決済手数料.rule", "billing/rules/出荷の送料.rule", "billing/rules/請求の要否.rule", "delivery/rules/出荷の急ぎ.rule"] {
        if let Err(said) = rules.facts(&ex.join(f)) {
            failures.push(format!("rulec {f}: {said:?}"));
        }
    }
    for f in ["billing/支払条件.cal", "delivery/出荷日.cal"] {
        if let Err(said) = koyomi::ports::Engine.facts(&ex.join(f)) {
            failures.push(format!("koyomi {f}: {said:?}"));
        }
    }
    // a calendar file answers for its sources once it passes koyomi's check
    if let Err(said) = ritsu_ports::Sources::sources(&koyomi::ports::Engine, &ex.join("calendars/東京の営業日.cal")) {
        failures.push(format!("koyomi calendars/東京の営業日.cal: {said:?}"));
    }
    if let Err(said) = chobo::ports::Engine.facts(&ex.join("inventory/在庫の引当.book")) {
        failures.push(format!("chobo inventory/在庫の引当.book: {said:?}"));
    }
    for f in ["ordering/受注.flow", "delivery/配送の手配.flow"] {
        let args = vec!["check".to_string(), ex.join(f).to_string_lossy().to_string()];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = dandori::cli::run(&args, std::rc::Rc::new(rulec::ports::Engine::new()), &mut out, &mut err);
        if code != 0 {
            failures.push(format!("dandori check {f}:\n{}{}", String::from_utf8_lossy(&out), String::from_utf8_lossy(&err)));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // the book's accounts and transfers, as `doc` will list them (DESIGN 4.5)
    let (accounts, transfers) = sakai::suite::book_names(&common::suite(), &ex.join("inventory/在庫の引当.book")).unwrap().unwrap();
    assert_eq!(accounts, ["在庫", "仕入先", "客"]);
    assert_eq!(transfers, ["入荷", "引当", "返品"]);
}

#[test]
fn buf_lints_the_protos_written_for_the_example() {
    if !ritsu_testkit::need(ritsu_testkit::Need::Buf) {
        return;
    }
    let Some(buf) = common::program("BUF", "", "buf", &["--version"]) else {
        common::skip("buf is not there (SAKAI_BUF or the PATH)");
        return;
    };
    // The other two import files buf would fetch from the BSR (buf/validate) or read from
    // dandori (options.proto); these two stand alone.
    let dir = common::TempDir::new("buf-lint");
    for f in ["shop/ordering/v1/order.proto", "warehouse/v1/stock.proto"] {
        dir.write(f, &std::fs::read_to_string(Path::new(common::EXAMPLE).join("proto").join(f)).unwrap());
    }
    let r = common::run(Command::new(&buf).arg("lint").current_dir(dir.path()).env("BUF_CACHE_DIR", dir.path().join(".cache")), Duration::from_secs(120));
    assert!(r.ok, "{}{}", r.stdout, r.stderr);
}

fn changed(edits: &[(&str, &str, &str)]) -> Vec<sakai::check::Outcome> {
    let dir = common::TempDir::new("example");
    common::copy_dir(Path::new(common::EXAMPLE), dir.path());
    for (f, old, new) in edits {
        let p = dir.path().join(f);
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.contains(old), "{f} has no {old:?}");
        std::fs::write(&p, s.replacen(old, new, 1)).unwrap();
    }
    sakai::check::check_args_with(dir.path(), &["通販.ctx".to_string()], &common::suite()).unwrap()
}

/// PLAN C.15: inventory adds a value to the packing status (E401), ordering's glossary gains a
/// 引当 of another meaning (E406), billing's side of the shared kernel is gone (E307, and the
/// calendar delivery reads across the boundary has no relationship left to allow it, E201),
/// ordering adds a value to the order status that billing's rule takes in (rulec refuses the rule,
/// E105), billing's rule renames a value to billing's word of another meaning (E407), a value
/// line beside the rule's import disagrees with it (E405), a rule's published language names
/// another package (E302) or service (E301) than rulec's, and a term means an output its rule
/// does not have (E007).
#[test]
fn a_change_to_the_example_is_caught() {
    let os = changed(&[("proto/warehouse/v1/stock.proto", "  PACKING_STATUS_SHORT = 3;\n", "  PACKING_STATUS_SHORT = 3;\n  PACKING_STATUS_DAMAGED = 4;\n")]);
    assert_eq!(common::codes(&os), ["E401"]);
    assert!(os[0].diags[0].message.en.contains("PACKING_STATUS_DAMAGED"));
    let os = changed(&[("contexts/受注.ctx", "\nupstream 在庫 conformist", "  引当 \"客の注文の一行に、届ける日を割り当てること\"\n\nupstream 在庫 conformist")]);
    assert_eq!(common::codes(&os), ["E406"]);
    let os = changed(&[(
        "contexts/請求.ctx",
        "\nshared kernel with 配送\n  koyomi \"../calendars/東京の営業日.cal\"\n  dir \"../py/calendars\", \"../ts/calendars\", \"../java/src/main/java/calendars\", \"../go/calendars\"\n",
        "\n",
    )]);
    assert_eq!(common::codes(&os), ["E307", "E201"]);
    let os = changed(&[("proto/shop/ordering/v1/order.proto", "  ORDER_STATUS_CANCELLED = 4;\n", "  ORDER_STATUS_CANCELLED = 4;\n  ORDER_STATUS_RETURNED = 5;\n")]);
    assert_eq!(common::codes(&os), ["E105"]);
    assert!(os[0].diags[0].notes.iter().any(|n| n.en.contains("[E032]")), "{:?}", os[0].diags[0].notes);
    let os = changed(&[
        ("billing/rules/請求の要否.rule", "受注で取消(cancelled)", "キャンセル(cancelled)"),
        ("billing/rules/請求の要否.rule", "| 受注で取消 |", "| キャンセル |"),
    ]);
    assert_eq!(common::codes(&os), ["E407"]);
    let os = changed(&[("contexts/請求.ctx", "enum 注文の状態\n", "enum 注文の状態\n    ORDER_STATUS_RECEIVED  -> 受付\n    ORDER_STATUS_PAID      -> 支払済\n    ORDER_STATUS_SHIPPED   -> 出荷済\n    ORDER_STATUS_CANCELLED -> 支払済\n")]);
    assert_eq!(common::codes(&os), ["E405"]);
    // a published language of a rule is the rule's Connect service, as rulec names it: its
    // package (E302) and its service (E301)
    let os = changed(&[("contexts/請求.ctx", "published language rulec.payment_fee.v1", "published language rulec.fee.v1")]);
    assert_eq!(common::codes(&os), ["E302"]);
    assert!(os[0].diags[0].message.en.contains("rulec.payment_fee.v1"), "{}", os[0].diags[0].message.en);
    let os = changed(&[("contexts/請求.ctx", "open host service PaymentFeeService", "open host service FeeService")]);
    assert_eq!(common::codes(&os), ["E301"]);
    // a term means an output the rule does not have: rulec's names, as for a .proto's (E007)
    let os = changed(&[("contexts/請求.ctx", "output 手数料\n", "output 手数料の額\n")]);
    assert_eq!(common::codes(&os), ["E007"]);
    assert!(os[0].diags[0].message.en.contains("There is no output 手数料の額 in billing/rules/決済手数料.rule"), "{}", os[0].diags[0].message.en);
}
