//! A `.proto` as the contract of a flow: the types a flow makes of it (DESIGN 1.12) are what the
//! description says, and a task is held to the same description by the same table.
//!
//! The first tests read the `.proto`s of tests/fixtures/protos and need no rulec and no other tool.
//! The last two are about the `.proto` rulec writes for a rule (DESIGN 1.13): that the names dandori
//! builds from `rulec api` are the ones in it, and that the service rulec writes answers what
//! dandori sends it. They need rulec; the second also needs buf and tools/connect/.venv, and says
//! SKIP when one is not there (read the output with `-- --nocapture`).

use dandori::apis::{self, Api, ApiDoc, ApiKind};
use dandori::model::{Model, Range, RecordOrigin, RuleConnect, Ty};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The model of a flow that names every message and every enum of the `.proto` `file` (from
/// tests/fixtures), as the type of a field of a record of its own, under the name `alias`.
fn made_from(alias: &str, file: &str) -> (Model, Api, Vec<(String, String, bool)>) {
    let path = root().join("tests/fixtures").join(file);
    let doc = apis::load(ApiKind::Proto, &path).unwrap_or_else(|e| panic!("{e}"));
    let ApiDoc::Proto(pf) = &doc else { unreachable!("a .proto") };
    let own = format!("{}.", pf.package);
    // (the name in the flow, the full name in the .proto, whether it is an enum)
    let mut names: Vec<(String, String, bool)> = Vec::new();
    for (full, is_enum) in pf.messages.keys().map(|k| (k, false)).chain(pf.enums.keys().map(|k| (k, true))) {
        let flow = match full.strip_prefix(&own) {
            Some(rest) => format!("{alias}.{rest}"),
            None => format!("{alias}.{full}"),
        };
        names.push((flow, full.clone(), is_enum));
    }
    let mut text = format!("workflow probe v1\n\nuse proto {alias} from \"{file}\"\n  url \"https://example.com\"\n\n");
    for (i, (flow, _, _)) in names.iter().enumerate() {
        text.push_str(&format!("record P{i}\n  f : {flow}\n\n"));
    }
    text.push_str("flow\n  pass\n");
    let checked = dandori::check::check_source(&text, &root().join("tests/fixtures/probe.flow"));
    let said: String = checked.diags.iter().map(|d| d.render("probe.flow", &text, dandori::diag::Lang::En)).collect();
    let m = checked.model.unwrap_or_else(|| panic!("the flow does not pass check:\n{said}"));
    let api = Api { name: alias.to_string(), doc: doc.clone(), url: None };
    (m, api, names)
}

#[test]
fn made_types_fit_their_descriptions() {
    for (alias, file) in [("types", "protos/types.proto"), ("shop", "protos/shop/v1/order.proto")] {
        let (m, api, names) = made_from(alias, file);
        assert!(names.len() >= 4, "{file} has messages and enums");
        let mut wrong = Vec::new();
        for (flow, full, is_enum) in &names {
            let t = if *is_enum {
                m.enums.iter().position(|e| e.name == *flow).map(Ty::Enum)
            } else {
                m.records.iter().position(|r| r.name == *flow).map(Ty::Record)
            };
            let Some(t) = t else {
                wrong.push(format!("{flow} was not made"));
                continue;
            };
            for (en, _) in apis::made_fits(&api, full, &m, &t) {
                wrong.push(format!("{flow}: {en}"));
            }
        }
        assert!(wrong.is_empty(), "what a flow makes of {file} is not what a task is held to:\n{}", wrong.join("\n"));
    }
}

#[test]
fn the_types_made_are_the_designs() {
    let (m, _, _) = made_from("types", "protos/types.proto");
    let enum_values = |name: &str| m.enums.iter().find(|e| e.name == name).unwrap_or_else(|| panic!("no enum {name}")).values.clone();
    // the zero value that says nothing was set is left out, and a zero value that is a value stays
    assert_eq!(enum_values("types.Status"), ["STATUS_OPEN", "STATUS_CLOSED"]);
    assert_eq!(enum_values("types.Mode"), ["ACTIVE", "PAUSED"]);

    let rid = |name: &str| m.records.iter().position(|r| r.name == name).unwrap_or_else(|| panic!("no record {name}"));
    let line = &m.records[rid("types.Line")];
    assert_eq!(line.origin, RecordOrigin::Proto { api: "types".into() });
    let range = |lo: Option<i64>, hi: Option<i64>| Range { lo, hi };
    assert_eq!(line.ranges.get("quantity"), Some(&range(Some(1), Some(99))));
    assert_eq!(line.ranges.get("discount"), Some(&range(Some(0), Some(50))));
    assert_eq!(line.ranges.get("offset"), Some(&range(Some(0), None)));
    assert_eq!(line.ranges.get("marks"), Some(&range(Some(1), Some(5))));
    assert_eq!(line.ranges.get("fixed"), Some(&range(Some(7), Some(7))));
    // no range: nothing says one, or what is said is not one range, or the integer is a string
    for f in ["outside", "free", "total", "sku"] {
        assert_eq!(line.ranges.get(f), None, "{f}");
    }
    let f = |name: &str| line.fields.iter().find(|(n, _)| n == name).unwrap().1.clone();
    assert_eq!(f("total"), Ty::Str);
    assert_eq!(f("marks"), Ty::List(Box::new(Ty::Int)));

    let all = &m.records[rid("types.Everything")];
    let e = |name: &str| all.fields.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("no field {name}")).1.clone();
    let line_ty = Ty::Record(rid("types.Line"));
    let opt = |t: Ty| Ty::Opt(Box::new(t));
    assert_eq!(e("line"), opt(line_ty.clone()));
    assert_eq!(e("requiredLine"), line_ty);
    assert_eq!(e("at"), opt(Ty::Timestamp));
    assert_eq!(e("nothing"), opt(Ty::Record(rid("types.google.protobuf.Empty"))));
    assert!(m.records[rid("types.google.protobuf.Empty")].fields.is_empty());
    assert_eq!(e("big"), Ty::Str);
    assert_eq!(e("weight"), Ty::Json);
    assert_eq!(e("tags"), Ty::Json);
    assert_eq!(all.ranges.get("amount"), Some(&range(Some(1), None)));

    let nested = &m.records[rid("types.Everything.Nested")];
    assert_eq!(nested.fields, [("tag".to_string(), Ty::Str)]);
}

#[test]
fn a_message_that_holds_itself_is_refused() {
    let root = root();
    let check = |text: &str| {
        let checked = dandori::check::check_source(text, &root.join("tests/fixtures/probe.flow"));
        checked.diags.iter().map(|d| (d.code, d.line, d.en.clone(), d.ja.clone())).collect::<Vec<_>>()
    };
    // directly, through a list; and two that hold each other, which are said once
    let said = check("workflow probe v1\n\nuse proto tree from \"protos/tree.proto\"\n\nrecord A\n  t : tree.Tree\n  l : tree.Left\n\nflow\n  pass\n");
    let codes: Vec<&str> = said.iter().map(|d| d.0).collect();
    assert_eq!(codes, ["E003", "E003"], "{said:?}");
    assert!(said[0].2.contains("`tree.Tree` contains itself, through `children`"), "{said:?}");
    assert!(said[0].3.contains("`tree.Tree` は、フィールド `children` を通して自分自身を含んでいます"), "{said:?}");
    assert!(said[1].2.contains("`tree.Left` contains itself, through `right`"), "{said:?}");
    // taking the recursive field as json, in a record of one's own, is the way out
    let ok = check("workflow probe v1\n\nrecord Node\n  name : string\n  next : json\n\nflow\n  pass\n");
    assert!(ok.is_empty(), "{ok:?}");
}

/// A record a flow declares is held to what a message made from a `.proto` is: it does not hold
/// itself, directly, through a list or a value that may be absent, or through other records.
#[test]
fn a_record_written_by_hand_that_holds_itself_is_refused() {
    let root = root();
    let check = |text: &str| {
        let checked = dandori::check::check_source(text, &root.join("tests/fixtures/probe.flow"));
        checked.diags.iter().map(|d| (d.code, d.line, d.en.clone(), d.ja.clone())).collect::<Vec<_>>()
    };
    for (decl, through, line) in [
        ("record Node\n  name : string\n  next : Node?\n", "next", 5),
        ("record Tree\n  children : list[Tree]\n", "children", 4),
        ("record Ring\n  me : Ring\n", "me", 4),
    ] {
        let said = check(&format!("workflow probe v1\n\n{decl}\nflow\n  pass\n"));
        assert_eq!(said.len(), 1, "{said:?}");
        assert_eq!((said[0].0, said[0].1), ("E003", line), "the field that holds the record: {said:?}");
        assert!(said[0].2.contains(&format!("contains itself, through `{through}`")), "{said:?}");
        assert!(said[0].3.contains(&format!("フィールド `{through}` を通して自分自身を含んでいます")), "{said:?}");
    }
    // two that hold each other are said once, for the first; a record that reaches them is not said
    let said = check("workflow probe v1\n\nrecord A\n  b : B?\n\nrecord B\n  a : A?\n\nrecord C\n  a : A\n\nflow\n  pass\n");
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].2.contains("`A` contains itself, through `b`"), "{said:?}");
    // with the field that holds itself taken as `json`, the record is fine
    let ok = check("workflow probe v1\n\nrecord Node\n  name : string\n  next : json\n\nflow\n  pass\n");
    assert!(ok.is_empty(), "{ok:?}");
}

#[test]
fn a_type_of_another_package_is_named_from_the_root() {
    let (m, _, _) = made_from("shop", "protos/shop/v1/order.proto");
    // the file's own package is left off; the other one, which the file imports, is not
    let order = m.records.iter().find(|r| r.name == "shop.Order").expect("shop.Order");
    let money = m.records.iter().position(|r| r.name == "shop.common.v1.Money").expect("shop.common.v1.Money");
    assert_eq!(order.fields.iter().find(|(n, _)| n == "total").unwrap().1, Ty::Opt(Box::new(Ty::Record(money))));
    let currency = m.enums.iter().find(|e| e.name == "shop.common.v1.Currency").expect("shop.common.v1.Currency");
    assert_eq!(currency.values, ["CURRENCY_JPY", "CURRENCY_USD"]);
    assert_eq!(m.records[money].ranges.get("nanos"), Some(&Range { lo: Some(0), hi: Some(999_999_999) }));
}

#[test]
fn a_proto_read_for_its_types_needs_no_url() {
    let check = |text: &str| {
        let checked = dandori::check::check_source(text, &root().join("tests/fixtures/probe.flow"));
        checked.diags.iter().map(|d| (d.code, d.line, d.en.clone())).collect::<Vec<_>>()
    };
    let types_only = "workflow probe v1\n\nuse proto shop from \"protos/shop/v1/order.proto\"\n\nrecord R\n  order : shop.Order\n\nflow\n  pass\n";
    assert_eq!(check(types_only), []);
    // where a task calls the service, once however many tasks do, at the `use`
    let called = format!("{types_only}\ntask place(customer: string) -> shop.Order\n  connect shop \"OrderService/Place\"\n\ntask place_again(customer: string) -> shop.Order\n  connect shop \"OrderService/Place\"\n");
    let said = check(&called);
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!((said[0].0, said[0].1), ("E016", 3));
    assert!(said[0].2.contains("a `.proto` does not say where the service is"), "{said:?}");
}

/// A `.proto` that imports files which are not on the disk (`google/api/annotations.proto` for its
/// options, `google/type/money.proto` for a type) is read all the same (DESIGN 1.10). What the
/// missing files hold is not known, and a flow that comes to a type of them is told so, with the
/// files named.
#[test]
fn a_proto_with_an_import_that_is_not_there_is_read_all_the_same() {
    let root = root();
    let head = "workflow probe v1\n\nuse proto catalog from \"protos/unread.proto\"\n  url \"https://catalog.example.com\"\n\n";
    let check = |body: &str| {
        let checked = dandori::check::check_source(&format!("{head}{body}"), &root.join("tests/fixtures/probe.flow"));
        checked.diags.iter().map(|d| (d.code, d.line, d.en.clone(), d.ja.clone(), d.notes.clone())).collect::<Vec<_>>()
    };
    let note_en = "`google/api/annotations.proto`, `google/type/money.proto` and `shop/v2/cancel.proto` could not be read, so the types in them cannot be used";
    let note_ja = "`google/api/annotations.proto`・`google/type/money.proto`・`shop/v2/cancel.proto` を読めなかったので、そこにある型は使えません";
    let told = |d: &(&str, usize, String, String, Vec<(String, String)>)| d.4.iter().any(|(en, ja)| en == note_en && ja == note_ja);

    // what does not come to them is as it is: the options' file is not needed for a type
    let fine = "record R\n  item : catalog.Item\n\ntask get(sku: string) -> catalog.Item\n  connect catalog \"CatalogService/GetItem\"\n\nflow\n  pass\n";
    assert_eq!(check(fine), []);

    // a type made of a message with a field of the type of a file that was not read
    let said = check("record R\n  priced : catalog.Priced\n\nflow\n  pass\n");
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!((said[0].0, said[0].1), ("E002", 7));
    assert!(said[0].2.contains("`catalog.Priced` cannot be made: its field `price` is of the type `google.type.Money`"), "{said:?}");
    assert!(said[0].3.contains("`catalog.Priced` を作れません。フィールド `price` の型 `google.type.Money` が分かりません"), "{said:?}");
    assert!(told(&said[0]), "{said:?}");
    // the record written by hand, with `json` for the field, is the way out, also as what a task answers
    let way_out = "record Priced\n  sku   : string\n  price : json\n\ntask get(sku: string) -> Priced\n  connect catalog \"CatalogService/GetPriced\"\n\nflow\n  pass\n";
    assert_eq!(check(way_out), []);

    // a name the `.proto` does not have, with the names it has, and the files not read
    let said = check("record R\n  x : catalog.Nope\n\nflow\n  pass\n");
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(said[0].0, "E002");
    assert_eq!(said[0].4.len(), 2, "the names, and the files not read: {said:?}");
    assert!(told(&said[0]), "{said:?}");

    // a task held to a message with a field of a type not known, and to a request that is not known
    let said = check("task quote(budget: string) -> catalog.QuoteResponse\n  connect catalog \"CatalogService/Quote\"\n\nflow\n  pass\n");
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(said[0].0, "E016");
    assert!(said[0].2.contains("its type `google.type.Money` is not known"), "{said:?}");
    assert!(told(&said[0]), "{said:?}");
    let said = check("task cancel(order: string)\n  connect catalog \"CatalogService/Cancel\"\n\nflow\n  pass\n");
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].2.contains("takes `shop.v2.CancelRequest`, a type that is not known"), "{said:?}");
    assert!(told(&said[0]), "{said:?}");

    // a service that is not there
    let said = check("task gone(sku: string)\n  connect catalog \"CatalogService/Discontinue\"\n\nflow\n  pass\n");
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].2.contains("has no method `CatalogService/Discontinue`") && told(&said[0]), "{said:?}");

    // a `.proto` that read every import tells nothing of the kind
    let all = dandori::check::check_source("workflow probe v1\n\nuse proto shop from \"protos/shop/v1/order.proto\"\n\nrecord R\n  x : shop.Nope\n\nflow\n  pass\n", &root.join("tests/fixtures/probe.flow"));
    assert!(all.diags.iter().all(|d| d.notes.len() == 1), "only the names it has: {:?}", all.diags);
}

// ---------------------------------------------------------------------------
// The `.proto` and the service rulec writes for a rule

fn rulec_bin() -> String {
    std::env::var("DANDORI_RULEC").unwrap_or_else(|_| "rulec".into())
}

fn rulec_available() -> bool {
    Command::new(rulec_bin()).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Whether the rulec at hand says, in `rulec api`, what a rule's service calls the values of its
/// enums (`connect.enums`): rulec 0.22.0 does. 0.21.2 did not, and its services took a field they did
/// not know and an input left out (DESIGN 1.13).
fn rulec_names_enums() -> bool {
    let out = Command::new(rulec_bin()).arg("api").arg(root().join("examples/order/rules/urgency.rule")).output();
    out.ok().and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok()).is_some_and(|v| v["connect"]["enums"].is_array())
}

/// The rules of the examples, which the flows of the examples and of tests/flows call, and the two of
/// tests/fixtures/rules whose enum is a contract's (`import proto`): one whose values carry the
/// contract's prefix and whose value 0 says it is not set (`delivery_fee.rule`, `MEMBER_TIER_GOLD`),
/// and one whose values have no prefix and whose value 0 is a state of its own (`account_fee.rule`,
/// `ACTIVE = 0`). The rule there that walks a list is not among them: dandori does not call one (E005).
fn rule_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for ex in std::fs::read_dir(root().join("examples")).unwrap().flatten() {
        if let Ok(rd) = std::fs::read_dir(ex.path().join("rules")) {
            out.extend(rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "rule")));
        }
    }
    out.extend(std::fs::read_dir(root().join("tests/fixtures/rules")).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "rule")));
    out.retain(|p| dandori::rulec::load(p).is_ok_and(|info| info.walks.is_none()));
    out.sort();
    out
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("dandori-protos-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The model of a rule called at the service `base`, as a flow that does says it, read through the
/// checker: what dandori sends the service and how it reads the answer. Err, the diagnostics, when
/// the flow does not pass check.
fn connect_of(rule: &Path, base: &str) -> Result<(RuleConnect, dandori::rulec::RuleInfo), Vec<dandori::diag::Diag>> {
    let text = format!("workflow probe v1\n\nuse rule r from \"{}\"\n  connect \"{base}\"\n\nflow\n  pass\n", rule.display());
    let checked = dandori::check::check_source(&text, &root().join("tests/fixtures/probe.flow"));
    let Some(m) = checked.model else { return Err(checked.diags) };
    let r = m.rules.into_iter().next().expect("a rule");
    Ok((r.connect.expect("a rule at its service"), r.info))
}

/// `connect_of`, or None after a SKIP line when the rulec at hand does not say what the service calls
/// the values of a contract's enum, which a rule that has one needs to be called at its service (E005).
fn connect_or_skip(rule: &Path, base: &str, names: bool) -> Option<(RuleConnect, dandori::rulec::RuleInfo)> {
    match connect_of(rule, base) {
        Ok(c) => Some(c),
        Err(diags) => {
            let unnamed = diags.iter().any(|d| d.code == "E005" && d.notes.iter().any(|(en, _)| en.contains("`connect.enums`")));
            if unnamed && !names {
                eprintln!("SKIP: {}: its enum is a contract's, and this rulec's `rulec api` does not say what the rule's service calls the values (`connect.enums`)", rule.strip_prefix(root()).unwrap_or(rule).display());
                return None;
            }
            panic!("{}: the flow that calls it at its service does not pass check: {:?}", rule.display(), diags.iter().map(|d| (&d.code, &d.en, &d.notes)).collect::<Vec<_>>());
        }
    }
}

/// What dandori reads from `rulec api` for a rule called at its service — the path, the fields of the
/// request and the response by their JSON names and kinds, the names of the enums' values and of
/// value 0, the zero values — is what the `.proto` rulec writes for the same rule says. The `.proto`
/// of a contract the rule imports is where `rulec gen` copies it (`proto/shop/v1/order.proto`, by its
/// package), and is read from there as an import. A change of how rulec names them shows here.
#[test]
fn rulec_gen_writes_the_names_dandori_builds() {
    if !rulec_available() {
        eprintln!("SKIP: rulec is not on the PATH; set DANDORI_RULEC to run this test");
        return;
    }
    use dandori::proto::PType;
    use dandori::rulec::WireKind;
    let names = rulec_names_enums();
    let mut checked = 0;
    for rule in rule_files() {
        let Some((c, info)) = connect_or_skip(&rule, "https://rules.example.com", names) else { continue };
        let out = scratch("gen");
        let gen = Command::new(rulec_bin()).arg("gen").arg(&rule).arg("--out").arg(&out).output().unwrap();
        assert!(gen.status.success(), "rulec gen failed: {}", String::from_utf8_lossy(&gen.stderr));
        let written = info.api["connect"]["proto"].as_str().expect("rulec api names the .proto");
        let pf = dandori::proto::load(&out.join(written)).unwrap_or_else(|e| panic!("{}: {e}", rule.display()));
        let name = rule.file_name().unwrap().to_string_lossy().to_string();
        assert!(pf.unread.is_empty(), "{name}: the .proto rulec writes imports what it does not write: {:?}", pf.unread);

        // the path: the service, and the method, with the package
        let (svc, method) = (&pf.services[0], &pf.services[0].methods[0]);
        assert_eq!((pf.services.len(), svc.methods.len()), (1, 1), "{name}: one service with one method");
        let path = format!("/{}/{}", svc.name, method.name);
        assert_eq!(c.url, format!("https://rules.example.com{path}"), "{name}: the path");

        // each field by its JSON name: the kind, and for an enum its values and its value 0, which is the
        // first of the `.proto`'s and one of the rule's values when it does not say it is not set
        let check = |side: &str, wire: &[dandori::rulec::WireField], message: &str, extra: &[&str]| {
            let fields = &pf.messages[message];
            let json_names: Vec<&str> = fields.iter().map(|f| f.json.as_str()).filter(|j| !extra.contains(j)).collect();
            assert_eq!(wire.iter().map(|w| w.json.as_str()).collect::<Vec<_>>(), json_names, "{name}: the {side}'s fields");
            for w in wire {
                let f = fields.iter().find(|f| f.json == w.json).unwrap();
                // a response's field is filled in when it is left out, which is right only when it has no presence;
                // a request's may have it (every one is `optional` once rulec tells one left out from zero)
                assert!(!f.repeated && (side == "request" || !f.presence), "{name}: {side} `{}` is a plain field", w.json);
                match (&w.kind, &f.ty) {
                    (WireKind::Bool, PType::Scalar(s)) => assert_eq!(s, "bool", "{name}: {}", w.json),
                    (WireKind::Int, PType::Scalar(s)) => assert_eq!(s, "int64", "{name}: {}", w.json),
                    (WireKind::Str, PType::Scalar(s)) => assert_eq!(s, "string", "{name}: {}", w.json),
                    (WireKind::Enum { zero, values }, PType::Named(e)) => {
                        let have = &pf.enums[e];
                        assert_eq!(&have[0], zero, "{name}: value 0 of {e}");
                        let mut mine: Vec<&String> = values.iter().map(|(_, proto)| proto).collect();
                        let mut theirs: Vec<&String> = if w.kind.zero_value().is_some() { have.iter().collect() } else { have[1..].iter().collect() };
                        mine.sort();
                        theirs.sort();
                        assert_eq!(mine, theirs, "{name}: the values of {e}");
                    }
                    (kind, ty) => panic!("{name}: `{}` is {kind:?} here and {ty:?} in the .proto", w.json),
                }
            }
        };
        check("request", &c.request, &method.input, &[]);
        check("response", &c.response, &method.output, &["trace"]);
        assert_eq!(c.request.iter().map(|w| &w.name).collect::<Vec<_>>(), info.inputs.iter().map(|i| &i.name).collect::<Vec<_>>(), "{name}: the rule's inputs, in order");
        assert_eq!(c.response.iter().map(|w| &w.name).collect::<Vec<_>>(), info.outputs.iter().map(|i| &i.name).collect::<Vec<_>>(), "{name}: the rule's outputs, in order");

        // the zero values the answer's JSON leaves out are the `.proto`'s, but for the rows that matched
        let mut zeros = apis::zeros_of(&pf, &method.output);
        for key in ["f", "l"] {
            if let Some(o) = zeros.get_mut(key).and_then(|v| v.as_object_mut()) {
                o.remove("trace");
            }
        }
        let zeros: Value = Value::Object(zeros.as_object().unwrap().iter().filter(|(_, v)| v.as_object().is_none_or(|o| !o.is_empty())).map(|(k, v)| (k.clone(), v.clone())).collect());
        assert_eq!(c.zeros, zeros, "{name}: the zero values");
        checked += 1;
    }
    assert!(checked >= 11, "the examples' rules: {checked}");
    eprintln!("the names dandori reads from rulec api are in the .proto rulec writes, for {checked} rule(s)");
}

/// A running service of a rule, killed when it goes out of scope.
struct Service(std::process::Child);

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Whether a value sent for a field is the field's zero value: what protobuf's JSON would leave out,
/// and dandori sends all the same.
fn at_zero(f: &dandori::rulec::WireField, v: &Value) -> bool {
    use dandori::rulec::WireKind;
    match (&f.kind, v) {
        (WireKind::Bool, Value::Bool(b)) => !b,
        (WireKind::Int, Value::String(s)) => s == "0",
        (WireKind::Str, Value::String(s)) => s.is_empty(),
        (WireKind::Enum { zero, .. }, Value::String(s)) => s == zero,
        _ => false,
    }
}

/// The service rulec writes for each rule run as it is (`--http`, the standard library's server, with
/// the stubs buf writes, those of a contract the rule imports among them), answers what dandori sends
/// it, and dandori reads the answer as the rule's own: every vector `rulec vectors` writes is sent in
/// the body `render::rule_request` makes, every input written out, at its zero value too, and what
/// `render::rule_read` reads of the answer is the vector's output, an enum the answer leaves out at a
/// value 0 that is the rule's among it. An answer carries the version of the table that decided it in
/// `rulec-source-sha256`. The service refuses with `invalid_argument` (400) an input outside the
/// rule's range and a name its enum does not have, and, written by a rulec that names the enums of its
/// service, a field it does not know and an input left out. Only the code and the status are looked
/// at: the messages are connectrpc's and protobuf's, and may change with their versions.
#[test]
fn rule_services_answer_as_dandori_reads_them() {
    if !rulec_available() {
        eprintln!("SKIP: rulec is not on the PATH; set DANDORI_RULEC to run this test");
        return;
    }
    let venv = root().join("tools/connect/.venv/bin");
    if !venv.join("python").exists() || !venv.join("protoc-gen-py").exists() {
        eprintln!("SKIP: tools/connect/.venv is missing; make it as tools/connect/requirements.txt says");
        return;
    }
    let buf = ["/opt/homebrew/bin/buf", "/usr/local/bin/buf", "buf"].into_iter().find(|b| Command::new(b).arg("--version").output().map(|o| o.status.success()).unwrap_or(false));
    let Some(buf) = buf else {
        eprintln!("SKIP: buf is not installed; it writes the stubs the service imports");
        return;
    };
    let names = rulec_names_enums();
    if !names {
        eprintln!("SKIP: this rulec's `rulec api` does not name the enums of a rule's service, and the services it writes take a field they do not know and an input left out; those two refusals are not asked for");
    }
    let (mut rules, mut sent, mut zeros) = (0, 0, 0);
    // the requests refused, by what is wrong with them: above a range, a name, a field, an input left out
    let mut refused = [0; 4];
    for rule in rule_files() {
        let name = rule.file_name().unwrap().to_string_lossy().to_string();
        // a rule whose enum is a contract's, with a rulec that does not say its names, is not called
        if connect_or_skip(&rule, "http://127.0.0.1", names).is_none() {
            continue;
        }
        let out = scratch("service");
        let gen = Command::new(rulec_bin()).arg("gen").arg(&rule).arg("--out").arg(&out).output().unwrap();
        assert!(gen.status.success(), "rulec gen failed: {}", String::from_utf8_lossy(&gen.stderr));
        // the stubs, by buf, with the plugins of the venv in place of the ones buf fetches
        let template = format!(
            "version: v2\nplugins:\n  - local: {0}/protoc-gen-py\n    out: ../python/stubs\n    strategy: all\n  - local: {0}/protoc-gen-connectrpc\n    out: ../python/stubs\n    strategy: all\n",
            venv.display()
        );
        std::fs::write(out.join("proto/buf.gen.local.yaml"), template).unwrap();
        let stubs = Command::new(buf).args(["generate", "--template", "buf.gen.local.yaml"]).current_dir(out.join("proto")).output().unwrap();
        assert!(stubs.status.success(), "{name}: buf generate failed: {}", String::from_utf8_lossy(&stubs.stderr));
        // the service of the rule, which says on its first line where it is
        let module = std::fs::read_dir(out.join("python")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).find(|f| f.ends_with("_service.py")).expect("rulec gen writes the service");
        let mut child = Command::new(venv.join("python"))
            .arg(&module)
            .args(["--http", "127.0.0.1:0"])
            .current_dir(out.join("python"))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let service = Service(child);
        let mut first = String::new();
        std::io::BufRead::read_line(&mut std::io::BufReader::new(stdout), &mut first).unwrap();
        let base = first.trim().to_string();
        assert!(base.starts_with("http://127.0.0.1:"), "{name}: the service says where it is on its first line, not {first:?}");

        let (c, info) = connect_of(&rule, &base).unwrap();
        let vectors = Command::new(rulec_bin()).arg("vectors").arg(&rule).output().unwrap();
        let lines: Vec<Value> = String::from_utf8_lossy(&vectors.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        // what is sent: each vector's input as dandori writes it
        let mut bodies: Vec<Value> = lines.iter().map(|l| dandori::render::rule_request(&c, l["in"].as_object().unwrap())).collect();
        // and what the service refuses, each made from the first of them: (what it is, the body)
        let mut wrong: Vec<(usize, String, Value)> = Vec::new();
        let one = bodies[0].as_object().unwrap().clone();
        if let Some((input, hi)) = info.inputs.iter().find_map(|i| i.range().and_then(|r| r.hi).map(|hi| (i.name.clone(), hi))) {
            let mut args = lines[0]["in"].as_object().unwrap().clone();
            args.insert(input.clone(), json!(hi + 1));
            wrong.push((0, format!("{input} {} is above the range", hi + 1), dandori::render::rule_request(&c, &args)));
        }
        if let Some(f) = c.request.iter().find(|f| matches!(f.kind, dandori::rulec::WireKind::Enum { .. })) {
            let mut body = one.clone();
            body.insert(f.json.clone(), json!("DANDORI_NOT_A_VALUE"));
            wrong.push((1, format!("`{}` has a name its enum does not have", f.json), Value::Object(body)));
        }
        if names {
            let mut body = one.clone();
            body.insert("dandoriNotAField".into(), json!(true));
            wrong.push((2, "`dandoriNotAField` is no field of the request".into(), Value::Object(body)));
            let mut body = one.clone();
            body.remove(&c.request[0].json);
            wrong.push((3, format!("the input `{}` is left out", c.request[0].name), Value::Object(body)));
        }
        bodies.extend(wrong.iter().map(|(_, _, b)| b.clone()));
        let dir = scratch("post");
        std::fs::write(dir.join("requests.json"), serde_json::to_string(&json!({ "url": c.url, "bodies": bodies })).unwrap()).unwrap();
        let post = Command::new(venv.join("python")).arg(root().join("tools/connect/post.py")).arg(dir.join("requests.json")).arg(dir.join("answers.json")).output().unwrap();
        assert!(post.status.success(), "{name}: post.py failed: {}", String::from_utf8_lossy(&post.stderr));
        let answers: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(dir.join("answers.json")).unwrap()).unwrap();
        assert_eq!(answers.len(), bodies.len());
        for (i, line) in lines.iter().enumerate() {
            let a = &answers[i];
            assert_eq!(a["status"], json!(200), "{name} vector {}: the service says {a}; it was sent {}", i + 1, bodies[i]);
            assert_eq!(a["headers"]["rulec-source-sha256"], json!(info.sha256), "{name}: the version of the table that decided");
            assert_eq!(dandori::render::rule_read(&c, &a["body"]), line["out"], "{name} vector {}: sent {}, got {}", i + 1, bodies[i], a["body"]);
            sent += 1;
            // every input is in the body, the ones at their zero values too
            assert_eq!(bodies[i].as_object().unwrap().len(), c.request.len(), "{name} vector {}: every input is sent: {}", i + 1, bodies[i]);
            if c.request.iter().any(|f| at_zero(f, &bodies[i][&f.json])) {
                zeros += 1;
            }
        }
        for (k, (kind, what, body)) in wrong.iter().enumerate() {
            let a = &answers[lines.len() + k];
            assert_eq!((&a["status"], &a["body"]["code"]), (&json!(400), &json!("invalid_argument")), "{name}: {what}, and the service says {a}; it was sent {body}");
            refused[*kind] += 1;
        }
        rules += 1;
        drop(service);
    }
    assert!(rules >= 11, "the examples' rules: {rules}");
    eprintln!(
        "sent {sent} vector(s) of {rules} rule(s) to the services rulec writes, {zeros} of them with an input at its zero value, and read the answers as the rules' own; refused with invalid_argument: {} input(s) above a range, {} name(s) no enum has, {} field(s) the request does not have, {} request(s) with an input left out",
        refused[0], refused[1], refused[2], refused[3]
    );
}

// ---------------------------------------------------------------------------
// The `.proto` of a service a workflow implements (DESIGN 1.14)

/// A program of the machine, where Homebrew and others put it, else on the PATH.
fn tool(name: &str) -> Option<String> {
    [format!("/opt/homebrew/bin/{name}"), format!("/usr/local/bin/{name}"), name.to_string()]
        .into_iter()
        .find(|b| Command::new(b).arg("--version").output().map(|o| o.status.success()).unwrap_or(false))
}

/// The `.proto` files under `dir`.
fn proto_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else if p.extension().is_some_and(|x| x == "proto") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// What a `.proto` imports.
fn imports_of(text: &str) -> Vec<String> {
    text.lines().filter_map(|l| l.trim().strip_prefix("import ")).filter_map(|l| l.split('"').nth(1)).map(String::from).collect()
}

/// dandori's options, which a user copies into the root of their protos, pass buf's lint as they
/// are (STANDARD, as `proto/buf.yaml` says), and build.
#[test]
fn dandori_options_pass_buf_lint() {
    let Some(buf) = tool("buf") else {
        eprintln!("SKIP: buf is not installed; it lints proto/dandori/v1/options.proto");
        return;
    };
    for what in ["lint", "build"] {
        let out = Command::new(&buf).arg(what).current_dir(root().join("proto")).output().unwrap();
        assert!(out.status.success(), "buf {what} refuses proto/:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    }
    eprintln!("buf lint and buf build pass proto/dandori/v1/options.proto");
}

/// The `.proto` of every service of the examples and the tests is one protoc builds, with this
/// repository's `proto/` as a root of the imports, as a user's is with the file copied into theirs:
/// every file that imports dandori's options, but the ones that import a file the tests do not keep
/// (Protovalidate's rules, and a file left out on purpose).
#[test]
fn service_protos_build_with_protoc() {
    let Some(protoc) = tool("protoc") else {
        eprintln!("SKIP: protoc is not installed; it builds the .proto of the services");
        return;
    };
    let mut built = Vec::new();
    for f in proto_files(&root().join("examples")).into_iter().chain(proto_files(&root().join("tests"))) {
        let imports = imports_of(&std::fs::read_to_string(&f).unwrap());
        let dir = f.parent().unwrap();
        let kept = |i: &String| i.starts_with("google/protobuf/") || i == "dandori/v1/options.proto" || dir.join(i).exists();
        if !imports.iter().any(|i| i == "dandori/v1/options.proto") || !imports.iter().all(kept) {
            continue;
        }
        let out = Command::new(&protoc)
            .arg("-I")
            .arg(dir)
            .arg("-I")
            .arg(root().join("proto"))
            .arg("--include_imports")
            .arg(format!("--descriptor_set_out={}", scratch("protoc").join("set.pb").display()))
            .arg(&f)
            .output()
            .unwrap();
        assert!(out.status.success(), "protoc refuses {}:\n{}", f.display(), String::from_utf8_lossy(&out.stderr));
        built.push(f.strip_prefix(root()).unwrap().display().to_string());
    }
    assert!(built.len() >= 5, "the services' .proto: {built:?}");
    eprintln!("protoc built {} .proto of services: {}", built.len(), built.join(", "));
}

/// The flows that implement a service, and run.
fn service_flows() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = ["temporal", "aws", "pydantic-graph"].iter().flat_map(|v| ["fulfillment.flow", "fulfillment.ja.flow"].map(|f| root().join(format!("examples/fulfillment/{v}/{f}")))).collect();
    out.push(root().join("tests/flows/events.flow"));
    out.push(root().join("tests/flows/service.flow"));
    out
}

/// A value with its nulls left out, and its numbers as numbers: protobuf reads a null as a field
/// not set, and writes an integer as one.
fn plain(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(_, x)| !x.is_null()).map(|(k, x)| (k.clone(), plain(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(plain).collect()),
        Value::Number(n) => json!(n.as_f64().unwrap()),
        other => other.clone(),
    }
}

/// What a workflow that implements a service takes and answers is what protobuf itself reads as the
/// messages of the service (`json_format` of protobuf's Python, which refuses a field it does not
/// know): every scenario's input as the request of the method that starts a run, every output the
/// reference interpreter ends a run with as its response, every value of an event and every answer
/// of a callback a method of the service sends as its request, and what the query `dandori.status`
/// answers as the response of the method that asks where a run is. And the input, as protobuf
/// writes it back, with the zero values it leaves out, is the scenario's input once dandori fills
/// them in.
#[test]
fn protobuf_reads_what_the_services_carry() {
    if !rulec_available() {
        eprintln!("SKIP: rulec is not on the PATH; set DANDORI_RULEC to run this test");
        return;
    }
    let Some(protoc) = tool("protoc") else {
        eprintln!("SKIP: protoc is not installed; it writes the descriptors protobuf reads the messages with");
        return;
    };
    let python = root().join("tools/temporal-python/.venv/bin/python");
    if !python.exists() {
        eprintln!("SKIP: tools/temporal-python/.venv is missing (its protobuf reads the messages); make it as tools/temporal-python/requirements.txt says");
        return;
    }
    use dandori::model::{Mark, TK};
    let (mut read, mut inputs_back) = (0, 0);
    for f in service_flows() {
        let name = f.strip_prefix(root()).unwrap().display().to_string();
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.unwrap_or_else(|| panic!("{name} does not pass check"));
        let s = m.service.clone().expect("a flow that implements a service");
        let dir = scratch("carry");
        let proto = f.parent().unwrap().join(&s.file);
        let set = dir.join("set.pb");
        let out = Command::new(&protoc)
            .arg("-I")
            .arg(proto.parent().unwrap())
            .arg("-I")
            .arg(root().join("proto"))
            .arg("--include_imports")
            .arg(format!("--descriptor_set_out={}", set.display()))
            .arg(&proto)
            .output()
            .unwrap();
        assert!(out.status.success(), "{name}: protoc refuses {}:\n{}", s.file, String::from_utf8_lossy(&out.stderr));
        let start = m.service_start().expect("a method that starts a run").clone();
        // the request of the method that sends each task its value
        let sent: Vec<(usize, String)> = s
            .methods
            .iter()
            .filter_map(|mt| match mt.marks.first() {
                Some(Mark::Event { task } | Mark::Answer { task }) => m.tasks.iter().position(|t| t.name == *task).map(|t| (t, mt.request.clone())),
                _ => None,
            })
            .collect();
        // (what it is, the message, the value)
        let mut values: Vec<(String, String, Value)> = Vec::new();
        for (i, sc) in dandori::scenarios::generate(&m).iter().enumerate() {
            let run = i + 1;
            values.push((format!("run {run}: the input"), start.request.clone(), sc["input"].clone()));
            let (trace, calls) = dandori::interp::run_traced(&m, sc, dandori::render::View::Temporal).unwrap();
            if let Some(out) = trace["end"].get("succeed") {
                values.push((format!("run {run}: the output"), start.response.clone(), out.clone()));
            }
            // the answers that come as the value a method of the service sends, as the run took them
            for (k, c) in calls.iter().enumerate() {
                let (Some(t), None) = (c.task, &c.kind) else { continue };
                let Some((_, request)) = sent.iter().find(|(x, _)| *x == t) else { continue };
                let v = &sc["answers"][k]["ok"];
                let task = &m.tasks[t];
                let filled = apis::fill(v, task.answer_zeros.as_ref().unwrap());
                if task.result.as_ref().is_some_and(|rt| dandori::render::value_fits(&m, &filled, rt, task.result_range)) {
                    values.push((format!("run {run}: the value `{}` takes", task.name), request.clone(), v.clone()));
                }
            }
        }
        // what the query `dandori.status` answers: before any case starts, and with every case in a state and an event waited for
        if let Some(st) = m.service_status() {
            let none: serde_json::Map<String, Value> = m.cases.iter().map(|c| (c.name.clone(), Value::Null)).collect();
            let some: serde_json::Map<String, Value> = m.cases.iter().map(|c| (c.name.clone(), json!(m.machine(m.cases.iter().position(|x| x.name == c.name).unwrap()).states[0]))).collect();
            let waits: Vec<&String> = m.tasks.iter().filter(|t| t.event).map(|t| &t.name).take(1).collect();
            values.push(("the status at the start".into(), st.response.clone(), json!({ "at": null, "cases": none, "events": [] })));
            values.push(("the status at a wait".into(), st.response.clone(), json!({ "at": m.all_stmts().iter().find(|x| matches!(x.kind, TK::Call { .. })).map(|x| x.line), "cases": some, "events": waits })));
        }
        let asked: Vec<Value> = values.iter().map(|(_, msg, v)| json!({ "message": msg, "json": v })).collect();
        std::fs::write(dir.join("values.json"), serde_json::to_string(&asked).unwrap()).unwrap();
        let out = Command::new(&python).arg(root().join("tools/protojson/check.py")).arg(&set).arg(dir.join("values.json")).arg(dir.join("results.json")).output().unwrap();
        assert!(out.status.success(), "{name}: check.py failed:\n{}", String::from_utf8_lossy(&out.stderr));
        let results: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(dir.join("results.json")).unwrap()).unwrap();
        assert_eq!(results.len(), values.len());
        for ((what, msg, v), r) in values.iter().zip(&results) {
            assert!(r.get("ok").is_some(), "{name}: {what} is not what protobuf reads as `{msg}`: {}\n{v}", r["error"]);
            read += 1;
            // the input, as protobuf writes it again, and filled in, is the input
            if what.ends_with("the input") {
                assert_eq!(plain(&apis::fill(&r["ok"], &s.input_zeros)), plain(&apis::fill(v, &s.input_zeros)), "{name}: {what}, as protobuf writes it, does not fill in to what it was: {}", r["ok"]);
                inputs_back += 1;
            }
        }
    }
    eprintln!("protobuf read {read} message(s) that the services carry, and {inputs_back} input(s) filled in from what it writes are the inputs");
}
