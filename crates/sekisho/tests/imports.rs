//! `import enum` (DESIGN 25): an enum read whole from rulec's `enums` file behaves as one the gate
//! writes with `enum`. What reads, what the names come to (a rename with `->`, an alias, the order
//! rulec keeps and the gate does not), the five mistakes the line can make, and that the code
//! generated for the two ways of having the enum is one code.

mod common;

use ritsu_base::text::Lang;
use ritsu_testkit::TempDir;
use sekisho::check::{Options, Outcome, check_file};

const STATES: &str = "enums states v1\n\nenum order_status = paid | shipped | returned | refunded\nenum tier = basic < silver < gold\n";

/// A gate that takes `status` as an attribute of an order and allows a clerk the orders that are paid.
fn gate(head: &str) -> String {
    format!("gate t v1\n\n{head}\nrole clerk\n\nprincipal User\n  roles clerk\n\nresource Order\n  attributes\n    status : order_status\n\naction view\n  principal User\n  resource Order\n\npermit clerks_view_paid_orders\n  principal in clerk\n  action view\n  when resource.status is paid\n")
}

/// `files` and the gate `t.gate` in a directory of their own, the gate checked with every language joined.
fn checked(files: &[(&str, &str)], src: &str) -> (TempDir, Outcome) {
    let t = TempDir::new("sekisho-import-enum");
    for (f, body) in files {
        t.write(f, body);
    }
    t.write("t.gate", src);
    let path = t.path().join("t.gate").to_string_lossy().to_string();
    let o = check_file(&path, &common::joined(), &Options::default()).unwrap();
    (t, o)
}

/// The codes and the lines of what is wrong, in the order the check says them.
fn wrong(o: &Outcome) -> Vec<(&'static str, usize)> {
    o.diags.iter().filter(|d| d.is_error()).map(|d| (d.code, d.line.unwrap_or(0))).collect()
}

fn said(o: &Outcome) -> String {
    o.diags.iter().map(|d| d.render(Lang::En)).collect()
}

const IMPORT: &str = "import enum order_status from \"states.rule\"\n";

#[test]
fn an_imported_enum_is_read_and_used_as_one_the_gate_writes() {
    let (_, o) = checked(&[("states.rule", STATES)], &gate(IMPORT));
    assert!(wrong(&o).is_empty(), "{}", said(&o));
    let scope = o.scope.as_ref().unwrap();
    let e = scope.enum_decl("order_status").unwrap();
    assert_eq!(e.values.iter().map(|v| v.text.as_str()).collect::<Vec<_>>(), ["paid", "shipped", "returned", "refunded"]);
    assert!(e.from.is_some() && e.span.line == 3);
    // the gate's other enums go beside it, in the line they are written on, and it stays among them
    let two = gate("import enum order_status from \"states.rule\"\nenum department = sales | support\n");
    let (_, o) = checked(&[("states.rule", STATES)], &two);
    assert!(wrong(&o).is_empty(), "{}", said(&o));
    let names: Vec<&str> = o.scope.as_ref().unwrap().file().enums.iter().map(|e| e.name.text.as_str()).collect();
    assert_eq!(names, ["order_status", "department"]);
}

#[test]
fn it_may_stand_among_the_enums_in_any_order_but_not_before_a_use() {
    // `import enum` has the place of `enum` (E003 otherwise)
    let (_, o) = checked(&[("states.rule", STATES)], &gate("enum department = sales | support\nimport enum order_status from \"states.rule\"\n"));
    assert!(wrong(&o).is_empty(), "{}", said(&o));
    let (_, o) = checked(&[("states.rule", STATES)], &gate("import enum order_status from \"states.rule\"\nuse rule r from \"states.rule\"\n"));
    assert!(wrong(&o).contains(&("E003", 4)), "{}", said(&o));
}

#[test]
fn a_rename_gives_the_enum_another_name_in_this_file() {
    let src = gate("import enum order_status from \"states.rule\" -> status\n").replace("status : order_status", "status : status");
    let (_, o) = checked(&[("states.rule", STATES)], &src);
    assert!(wrong(&o).is_empty(), "{}", said(&o));
    assert!(o.scope.as_ref().unwrap().enum_decl("status").is_some());
    assert!(o.scope.as_ref().unwrap().enum_decl("order_status").is_none());
}

#[test]
fn the_alias_of_a_value_is_how_the_gate_writes_it() {
    let ja = "enums 注文の状態(states) v1\n\nenum 状態(order_status) = 支払済(paid) | 発送済(shipped)\n";
    // by the alias, as an `enum` of the gate would be written
    let (_, o) = checked(&[("states.rule", ja)], &gate("import enum 状態 from \"states.rule\"\n").replace("status : order_status", "status : 状態"));
    assert!(wrong(&o).is_empty(), "{}", said(&o));
    let e = o.scope.as_ref().unwrap().enum_decl("order_status").unwrap();
    assert_eq!(e.name.text, "状態");
    assert_eq!(e.name.ascii(), "order_status");
    assert_eq!(e.values.iter().map(|v| (v.text.as_str(), v.ascii())).collect::<Vec<_>>(), [("支払済", "paid"), ("発送済", "shipped")]);
    // and by the name
    let (_, o) = checked(&[("states.rule", ja)], &gate("import enum 状態 from \"states.rule\"\n").replace("status : order_status", "status : 状態").replace("is paid", "is 支払済"));
    assert!(wrong(&o).is_empty(), "{}", said(&o));
}

#[test]
fn the_order_of_an_ordered_enum_is_said_and_not_used() {
    let src = gate("import enum tier from \"states.rule\"\n").replace("status : order_status", "status : tier").replace("is paid", "is gold");
    let (_, o) = checked(&[("states.rule", STATES)], &src);
    assert!(wrong(&o).is_empty(), "{}", said(&o));
    let e = o.scope.as_ref().unwrap().enum_decl("tier").unwrap();
    assert_eq!(e.values.iter().map(|v| v.text.as_str()).collect::<Vec<_>>(), ["basic", "silver", "gold"]);
    assert!(e.from.as_ref().unwrap().ordered);
}

#[test]
fn the_five_mistakes_a_line_can_make() {
    // the file is not there: E201, at the string
    let (_, o) = checked(&[], &gate(IMPORT));
    assert!(wrong(&o).contains(&("E201", 3)), "{}", said(&o));
    // the file is a rule, not an `enums` file
    let rule = "rule r v1\n\ninputs\n  x : bool\n\noutputs\n  y : bool\n\ntable t\npolicy unique\n| x     | -> y : bool |\n| true  | true        |\n| false | false       |\n";
    let (_, o) = checked(&[("states.rule", rule)], &gate(IMPORT));
    assert_eq!(wrong(&o)[0], ("E201", 3), "{}", said(&o));
    assert!(said(&o).contains("is a rule, not an `enums` file"), "{}", said(&o));
    // the `enums` file does not pass rulec's check, which says why in the notes
    let (_, o) = checked(&[("states.rule", "enums states v1\n\nenum order_status = paid | paid\n")], &gate(IMPORT));
    assert_eq!(wrong(&o)[0], ("E201", 3), "{}", said(&o));
    assert!(said(&o).contains("does not pass rulec's check"), "{}", said(&o));
    // the enum is not in the file
    let (_, o) = checked(&[("states.rule", STATES)], &gate("import enum status from \"states.rule\"\n"));
    assert!(wrong(&o).contains(&("E101", 3)), "{}", said(&o));
    assert!(said(&o).contains("`order_status`, `tier`"), "{}", said(&o));
    // the name is one the file has already: an `enum` of the same name, another import's, and the rename
    let (_, o) = checked(&[("states.rule", STATES)], &gate("import enum order_status from \"states.rule\"\nenum order_status = a | b\n"));
    assert!(wrong(&o).contains(&("E006", 4)), "{}", said(&o));
    let (_, o) = checked(&[("states.rule", STATES)], &gate("import enum order_status from \"states.rule\"\nimport enum tier from \"states.rule\" -> order_status\n"));
    assert!(wrong(&o).contains(&("E006", 4)), "{}", said(&o));
    // a name rulec takes and Cedar cannot: the note says where to correct it
    let (_, o) = checked(&[("states.rule", "enums states v1\n\nenum OrderStatus = Paid | Shipped\n")], &gate("import enum OrderStatus from \"states.rule\"\n").replace("status : order_status", "status : OrderStatus"));
    assert!(wrong(&o).contains(&("E007", 3)), "{}", said(&o));
    assert!(said(&o).contains("its names are that file's"), "{}", said(&o));
}

#[test]
fn a_gate_that_reads_a_rule_cannot_be_read_where_rulec_is_not_joined() {
    let t = TempDir::new("sekisho-import-enum-unjoined");
    t.write("states.rule", STATES);
    t.write("t.gate", &gate(IMPORT));
    let path = t.path().join("t.gate").to_string_lossy().to_string();
    let o = check_file(&path, &sekisho::suite::Suite::default(), &Options::default()).unwrap();
    assert_eq!(wrong(&o), [("E209", 3)], "{}", said(&o));
    assert!(said(&o).contains("cannot read rulec's `enums` files"), "{}", said(&o));
}

#[test]
fn what_the_file_brings_into_view_is_read_by_a_gate_that_reads_it_with_use_gate() {
    let t = TempDir::new("sekisho-use-gate-import-enum");
    t.write("states.rule", STATES);
    t.write("base.gate", "gate base v1\nnamespace Shop\n\nimport enum order_status from \"states.rule\"\n");
    let user = gate("use gate \"base.gate\"\n").replace("gate t v1\n", "gate t v1\nnamespace Shop\n");
    t.write("t.gate", &user);
    let path = t.path().join("t.gate").to_string_lossy().to_string();
    let o = check_file(&path, &common::joined(), &Options::default()).unwrap();
    assert!(wrong(&o).is_empty(), "{}", said(&o));
    assert!(o.scope.as_ref().unwrap().enum_decl("order_status").is_some());
}

#[test]
fn the_items_leave_the_enum_to_the_file_that_declares_it() {
    use ritsu_ports::{Items, References};
    let (t, o) = checked(&[("states.rule", STATES)], &gate(&format!("{IMPORT}enum department = sales | support\n")));
    assert!(wrong(&o).is_empty());
    let engine = sekisho::ports::Engine::new(common::joined());
    let items = engine.items(t.path(), "t.gate").unwrap();
    let enums: Vec<String> = items.iter().map(|i| i.naming.text()).filter(|n| n.contains("enum")).collect();
    assert!(enums.iter().all(|n| n.contains("department")), "{enums:?}");
    // and the reference says which enum of which file
    let refs = engine.references(t.path(), "t.gate").unwrap();
    let r = refs.iter().find(|r| r.how == "import enum").unwrap();
    assert_eq!((r.line, r.target.text()), (3, "rulec \"states.rule\" enum order_status".to_string()));
}

#[test]
fn the_page_and_the_api_say_where_the_enum_was_read_from() {
    let (_, o) = checked(&[("states.rule", STATES)], &gate(IMPORT));
    let page = sekisho::doc::page(&o, &common::joined(), Lang::En, sekisho::doc::Format::Markdown, None).unwrap();
    let text = sekisho::doc::markdown::write(&page);
    assert!(text.contains("## Enums read from other files"), "{text}");
    assert!(text.contains("| `order_status` | `paid`, `shipped`, `returned`, `refunded` | `states.rule` | `order_status` | no |"), "{text}");
    let api = sekisho::api::json(o.scope.as_ref().unwrap(), o.walked.as_ref().unwrap());
    let enums = api.get("enums").unwrap().as_arr().unwrap();
    assert_eq!(enums.len(), 1);
    let e = &enums[0];
    assert_eq!(e.get("from").unwrap().get("path").unwrap().as_str(), Some("states.rule"));
    assert_eq!(e.get("ordered").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(e.get("values").unwrap().as_arr().unwrap().len(), 4);
}

/// The Cedar, the code of the requests and the vectors generated for the gate that imports the enum
/// are those of the gate that writes it: the two files differ in the line that has the enum, and in
/// nothing a target writes.
#[test]
fn the_code_generated_is_the_same_as_for_an_enum_written_in_the_gate() {
    let (a, o) = checked(&[("states.rule", STATES)], &gate(IMPORT));
    assert!(wrong(&o).is_empty(), "{}", said(&o));
    let written = gate("enum order_status = paid | shipped | returned | refunded\n");
    let (b, o) = checked(&[], &written);
    assert!(wrong(&o).is_empty(), "{}", said(&o));
    let generated = |t: &TempDir, target: &str| -> Vec<(String, String)> {
        let out = t.path().join(format!("out-{target}"));
        let args: Vec<String> = ["gen", t.path().join("t.gate").to_str().unwrap(), "--target", target, "--out", out.to_str().unwrap()].iter().map(|s| s.to_string()).collect();
        let (mut so, mut se) = (Vec::new(), Vec::new());
        let code = sekisho::run::run(&args, common::joined(), &mut so, &mut se);
        assert_eq!(code, 0, "{target}: {}{}", String::from_utf8_lossy(&so), String::from_utf8_lossy(&se));
        let mut files = Vec::new();
        let mut todo = vec![out.clone()];
        while let Some(d) = todo.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    todo.push(p);
                } else {
                    // the head names the digest of the source, which differs by the line
                    let text: String = std::fs::read_to_string(&p).unwrap().lines().filter(|l| !l.contains("sha256:")).map(|l| format!("{l}\n")).collect();
                    files.push((p.strip_prefix(&out).unwrap().to_string_lossy().to_string(), text));
                }
            }
        }
        files.sort();
        files
    };
    for target in ["cedar", "typescript", "python", "go"] {
        let (x, y) = (generated(&a, target), generated(&b, target));
        assert!(!x.is_empty(), "{target}");
        assert_eq!(x, y, "{target}");
    }
}
