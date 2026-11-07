//! `import enum` (DESIGN 1.21): an enum read whole from rulec's `enums` file behaves as one the flow
//! writes with `enum`. What reads (a type of a record, an input, a `match`, a range), what the names
//! come to (a rename with `->`, the names of a Japanese file, the order rulec keeps), the mistakes
//! the line can make, the references and the page, and that the code every platform gets is the
//! code of the same enum written in the flow.

use dandori::check::{check_source, Checked};
use dandori::diag::Lang;
use ritsu_testkit::TempDir;
use std::path::Path;
use std::rc::Rc;

const STATES: &str = "enums states v1\n\nenum order_state = paid | shipped | returned | refunded\nenum tier = basic < silver < gold\n";

/// `files` and the flow `t.flow` (the text `src`) in a directory of their own, the flow checked
/// with rulec's port.
fn checked(files: &[(&str, &str)], src: &str) -> (TempDir, Checked) {
    let t = TempDir::new("dandori-import-enum");
    for (f, body) in files {
        t.write(f, body);
    }
    let path = t.path().join("t.flow");
    let c = checked_at(&path, src);
    (t, c)
}

fn checked_at(path: &Path, src: &str) -> Checked {
    dandori::sources::with_rules(Rc::new(rulec::ports::Engine::new()), || check_source(src, path))
}

fn codes(c: &Checked) -> Vec<(&'static str, usize)> {
    c.diags.iter().filter(|d| d.severity == dandori::diag::Severity::Error).map(|d| (d.code, d.line)).collect()
}

fn said(c: &Checked) -> String {
    c.diags.iter().map(|d| d.render("t.flow", "", Lang::En)).collect()
}

/// A flow that takes a state, ships a paid order and gives the state back.
fn flow(head: &str) -> String {
    format!("workflow t v1\n\n{head}\ntask ship(order_id: string)\n  lambda \"arn:aws:lambda:ap-northeast-1:123456789012:function:ship\"\n\ninputs\n  order_id : string\n  state    : order_state\n\noutputs\n  state : order_state\n\nflow\n  match state\n    paid =>\n      ship(order_id: order_id)\n      succeed state = shipped\n    shipped => succeed state = shipped\n    returned => succeed state = returned\n    refunded => succeed state = refunded\n")
}

const IMPORT: &str = "import enum order_state from \"states.rule\"\n";

#[test]
fn an_imported_enum_is_read_and_used_as_one_the_flow_writes() {
    let (_, c) = checked(&[("states.rule", STATES)], &flow(IMPORT));
    assert!(codes(&c).is_empty(), "{}", said(&c));
    let m = c.model.unwrap();
    let e = m.enums.iter().find(|e| e.name == "order_state").unwrap();
    assert_eq!(e.values, ["paid", "shipped", "returned", "refunded"]);
    assert!(!e.ordered);
    assert_eq!(e.from.as_ref().map(|(p, n)| (p.as_str(), n.as_str())), Some(("states.rule", "order_state")));
    // and as the type of a field of a record
    let src = "workflow t v1\n\nimport enum order_state from \"states.rule\"\n\nrecord Order\n  id    : string\n  state : order_state\n\ninputs\n  order : Order\n\noutputs\n  state : order_state\n\nflow\n  succeed state = order.state\n";
    let (_, c) = checked(&[("states.rule", STATES)], src);
    assert!(codes(&c).is_empty(), "{}", said(&c));
}

#[test]
fn a_rename_gives_the_enum_another_name_in_this_flow() {
    let src = flow("import enum order_state from \"states.rule\" -> status\n").replace("order_state", "status").replace("import enum status from", "import enum order_state from");
    let (_, c) = checked(&[("states.rule", STATES)], &src);
    assert!(codes(&c).is_empty(), "{}", said(&c));
    let m = c.model.unwrap();
    assert!(m.enums.iter().any(|e| e.name == "status" && e.from.is_some()));
    assert!(!m.enums.iter().any(|e| e.name == "order_state"));
}

#[test]
fn the_names_are_the_names_of_the_file_and_the_order_is_kept() {
    let ja = "enums 状態(states) v1\n\nenum 注文の状態(order_state) = 支払済(paid) | 発送済(shipped)\nenum 段階(tier) = 基本(basic) < 銀(silver) < 金(gold)\n";
    let src = "workflow t v1\n\nimport enum 注文の状態 from \"states.rule\"\nimport enum 段階 from \"states.rule\"\n\ninputs\n  state : 注文の状態\n  tier  : 段階  range >=基本 <=銀\n\noutputs\n  state : 注文の状態\n\nflow\n  match state\n    支払済 => succeed state = 発送済\n    発送済 => succeed state = 発送済\n";
    let (_, c) = checked(&[("states.rule", ja)], src);
    assert!(codes(&c).is_empty(), "{}", said(&c));
    let m = c.model.unwrap();
    let tier = m.enums.iter().find(|e| e.name == "段階").unwrap();
    assert_eq!(tier.values, ["基本", "銀", "金"]);
    assert!(tier.ordered);
}

#[test]
fn the_mistakes_a_line_can_make() {
    // the file is not there
    let (_, c) = checked(&[], &flow(IMPORT));
    assert!(codes(&c).contains(&("E005", 3)), "{}", said(&c));
    // the file is a rule
    let rule = "rule r v1\n\ninputs\n  x : bool\n\noutputs\n  y : bool\n\ntable t\npolicy unique\n| x     | -> y : bool |\n| true  | true        |\n| false | false       |\n";
    let (_, c) = checked(&[("states.rule", rule)], &flow(IMPORT));
    assert_eq!(codes(&c)[0], ("E005", 3), "{}", said(&c));
    assert!(said(&c).contains("is a rule, not an `enums` file"), "{}", said(&c));
    // the `enums` file does not pass rulec's check
    let (_, c) = checked(&[("states.rule", "enums states v1\n\nenum order_state = paid | paid\n")], &flow(IMPORT));
    assert_eq!(codes(&c)[0], ("E005", 3), "{}", said(&c));
    // the enum is not in the file: E002, with the enums there are
    let (_, c) = checked(&[("states.rule", STATES)], &flow("import enum status from \"states.rule\"\n"));
    assert!(codes(&c).contains(&("E002", 3)), "{}", said(&c));
    assert!(said(&c).contains("`order_state`, `tier`"), "{}", said(&c));
    // a name that is already an enum's, a record's or another import's
    let (_, c) = checked(&[("states.rule", STATES)], &flow("import enum order_state from \"states.rule\"\nenum order_state = a | b\n"));
    assert!(codes(&c).contains(&("E006", 4)), "{}", said(&c));
    let (_, c) = checked(&[("states.rule", STATES)], &flow("import enum order_state from \"states.rule\"\nimport enum tier from \"states.rule\" -> order_state\n"));
    assert!(codes(&c).contains(&("E006", 4)), "{}", said(&c));
    let (_, c) = checked(&[("states.rule", STATES)], &flow("import enum order_state from \"states.rule\"\nrecord order_state\n  id : string\n"));
    assert!(codes(&c).iter().any(|(code, _)| *code == "E006"), "{}", said(&c));
    // a value the flow writes and the file does not have
    let (_, c) = checked(&[("states.rule", "enums states v1\n\nenum order_state = paid | shipped\n")], &flow(IMPORT));
    assert!(codes(&c).iter().any(|(code, _)| *code == "E002" || *code == "E003"), "{}", said(&c));
}

#[test]
fn the_flow_stops_where_it_writes_a_value_the_file_has_renamed() {
    let renamed = STATES.replace("paid |", "settled |");
    let (_, c) = checked(&[("states.rule", &renamed)], &flow(IMPORT));
    assert!(!codes(&c).is_empty());
    assert!(said(&c).contains("`paid` is not a value of `order_state`"), "{}", said(&c));
}

#[test]
fn a_reference_names_the_enum_of_the_file() {
    use ritsu_ports::References;
    let t = TempDir::new("dandori-import-enum-refs");
    t.write("states.rule", STATES);
    t.write("t.flow", &flow(&format!("{IMPORT}import enum tier from \"states.rule\" -> grade\n")));
    let refs = dandori::ports::Engine.references(t.path(), "t.flow").unwrap();
    let got: Vec<(usize, String, String)> = refs.iter().filter(|r| r.how == "import enum").map(|r| (r.line, r.target.text(), r.how.clone())).collect();
    assert_eq!(got, [(3, "rulec \"states.rule\" enum order_state".to_string(), "import enum".to_string()), (4, "rulec \"states.rule\" enum tier".to_string(), "import enum".to_string())]);
    // and the items leave the enum to the file that declares it
    use ritsu_ports::Items;
    let items = dandori::ports::Engine.items(t.path(), "t.flow").unwrap();
    assert!(!items.iter().any(|i| i.kind() == "enum"), "{items:?}");
}

#[test]
fn the_page_says_where_the_enum_was_read_from() {
    let t = TempDir::new("dandori-import-enum-doc");
    t.write("states.rule", STATES);
    t.write("t.flow", &flow(IMPORT));
    let path = t.path().join("t.flow");
    let (src, d) = dandori::sources::with_rules(Rc::new(rulec::ports::Engine::new()), || dandori::check::drawable(&path).unwrap());
    let m = d.model.as_ref().unwrap();
    for (lang, head, row) in [
        (Lang::En, "## Enums read from other files", "| `order_state` | `paid`, `shipped`, `returned`, `refunded` | `states.rule` | `order_state` | no |"),
        (Lang::Ja, "## ほかのファイルから読んだ列挙", "| `order_state` | `paid`・`shipped`・`returned`・`refunded` | `states.rule` | `order_state` | なし |"),
    ] {
        let i = dandori::doc::Input { m, src: &src, file: "t.flow", facts: &d.facts, diags: &d.diags, lang };
        let page = dandori::doc::markdown(&i);
        assert!(page.contains(head) && page.contains(row), "{page}");
        let html = dandori::doc::html(&i);
        assert!(html.contains("<code>states.rule</code>"), "the page of {lang:?} does not say the file");
    }
}

/// Every platform gets the code of the same enum written in the flow: the files differ in the line
/// that has the enum, and in nothing a target writes but the digest of the source, which the head
/// of a file and the build id of a worker name.
#[test]
fn the_code_of_every_platform_is_that_of_an_enum_written_in_the_flow() {
    let (a, ca) = checked(&[("states.rule", STATES)], &flow(IMPORT));
    assert!(codes(&ca).is_empty(), "{}", said(&ca));
    let (_b, cb) = checked(&[], &flow("enum order_state = paid | shipped | returned | refunded\n"));
    assert!(codes(&cb).is_empty(), "{}", said(&cb));
    let _ = a;
    let (ma, mb) = (ca.model.unwrap(), cb.model.unwrap());
    let masked = |files: Vec<(String, String)>| -> Vec<(String, String)> { files.into_iter().map(|(p, t)| (p, t.lines().filter(|l| !l.contains("sha256") && !l.to_lowercase().contains("buildid") && !l.to_lowercase().contains("build_id")).map(|l| format!("{l}\n")).collect())).collect() };
    for target in dandori::commands::TARGETS {
        let x = dandori::commands::build(&ma, target).unwrap().unwrap_or_else(|_| panic!("{target}"));
        let y = dandori::commands::build(&mb, target).unwrap().unwrap_or_else(|_| panic!("{target}"));
        assert!(!x.is_empty(), "{target}");
        let (x, y) = (masked(x), masked(y));
        for ((px, tx), (py, ty)) in x.iter().zip(&y) {
            assert_eq!(px, py, "{target}");
            let differing: Vec<(&str, &str)> = tx.lines().zip(ty.lines()).filter(|(l, r)| l != r).collect();
            assert!(differing.is_empty() && tx.lines().count() == ty.lines().count(), "{target} {px}: {differing:?}");
        }
        assert_eq!(x.len(), y.len(), "{target}");
    }
}
