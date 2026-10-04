//! `yuen export` (PLAN C.10, C.11, DESIGN 12, 13). Every test project is written out as
//! ReqIF and as PROV, and held to what reads them: xmllint with the OMG's schema (through the
//! catalog, without the network), `reqif validate` of the StrictDoc project, and the Python
//! `prov` reading PROV-N and PROV-JSON as one document. What none of them checks is checked
//! here: that every `-REF` of the ReqIF lands on an element of its kind (xmllint lets an
//! `IDREF` point nowhere), and that the counts are `yuen api`'s.

mod common;

use common::TempDir;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;
use yuen::export::prov::{Kind, TERMS, YUEN_NS};

/// Every test project: the ones of yuen's own requirements alone (sources copied by yuen,
/// artifacts that are files), and the ones that name the things of another language — a rule, a
/// calendar, a book, a spec, a `.proto`, a workflow, a context — read through ritsu's ports.
const PROJECTS: [(&str, &str); 18] = [
    ("period", "tests/fixtures/period"),
    ("ecfr", "tests/fixtures/ecfr"),
    ("payment", "tests/fixtures/payment"),
    ("rulec", "tests/fixtures/rulec"),
    ("koyomi", "tests/fixtures/koyomi"),
    ("chobo", "tests/fixtures/chobo"),
    ("geas", "tests/fixtures/geas"),
    ("proto", "tests/fixtures/proto"),
    ("dandori", "tests/fixtures/dandori"),
    ("sakai", "tests/fixtures/sakai"),
    // the same, with the English fixtures (the eCFR for the law, English names in every language's file)
    ("period_of_months", "tests/fixtures/period_of_months"),
    ("payment_policy", "tests/fixtures/payment_policy"),
    ("fee_rules", "tests/fixtures/fee_rules"),
    ("calendar_sources", "tests/fixtures/calendar_sources"),
    ("refunds_book", "tests/fixtures/refunds_book"),
    ("warehouse_proto", "tests/fixtures/warehouse_proto"),
    ("delivery_flow", "tests/fixtures/delivery_flow"),
    ("ordering_terms", "tests/fixtures/ordering_terms"),
];

/// The command with every language joined, as `ritsu yuen` runs it.
fn run(args: &[&str]) -> common::Ran {
    common::run(args)
}

fn export(what: &str, dir: &str, more: &[&str]) -> String {
    let mut args = vec!["export", what, dir, "--root", dir];
    args.extend_from_slice(more);
    let r = run(&args);
    assert_eq!(r.code, 0, "yuen {}: {}", args.join(" "), r.stderr);
    r.stdout
}

fn api(dir: &str) -> Value {
    let r = run(&["api", dir, "--root", dir]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    serde_json::from_str(&r.stdout).unwrap()
}

fn arr(v: &Value) -> &Vec<Value> {
    v.as_array().unwrap()
}

/// The upper ends of a `from` line in `api`: its articles, or the one requirement.
fn uppers(f: &Value) -> usize {
    let n = arr(&f["sources"]).len();
    if n == 0 { 1 } else { n }
}

/// What every `-REF` of a ReqIF document must land on: for `<X-REF>id</X-REF>`, an element
/// `<X IDENTIFIER="id"`. Every identifier is `_` and 32 hex digits, and none is used twice.
fn references(xml: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let mut kinds: BTreeMap<String, String> = BTreeMap::new();
    let mut rest = xml;
    while let Some(i) = rest.find(" IDENTIFIER=\"") {
        let start = rest[..i].rfind('<').unwrap() + 1;
        let el = rest[start..i].to_string();
        let v = &rest[i + 13..];
        let id = v[..v.find('"').unwrap()].to_string();
        if !(id.len() == 33 && id.starts_with('_') && id[1..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())) {
            failures.push(format!("{el} has the identifier {id}, not _ and 32 hex digits"));
        }
        if let Some(other) = kinds.insert(id.clone(), el.clone()) {
            failures.push(format!("{id} identifies both {other} and {el}"));
        }
        rest = &v[id.len()..];
    }
    let mut n = 0;
    let mut rest = xml;
    while let Some(i) = rest.find("-REF>") {
        let open = rest[..i].rfind('<').unwrap();
        let tag = &rest[open + 1..i + 4];
        let after = &rest[i + 5..];
        rest = after;
        if tag.starts_with('/') {
            continue;
        }
        let id = &after[..after.find('<').unwrap()];
        let want = tag.trim_end_matches("-REF");
        n += 1;
        match kinds.get(id) {
            Some(el) if el == want => {}
            Some(el) => failures.push(format!("<{tag}>{id}</{tag}> lands on {el}, not {want}")),
            None => failures.push(format!("<{tag}>{id}</{tag}> lands on nothing")),
        }
    }
    if n == 0 {
        failures.push("no -REF at all".into());
    }
    failures
}

#[test]
fn every_reqif_is_well_formed_and_every_reference_lands() {
    let mut failures = Vec::new();
    for (name, dir) in PROJECTS {
        let xml = export("reqif", dir, &[]);
        assert_eq!(xml, export("reqif", dir, &[]), "{name}: two exports are the same bytes");
        common::golden(&format!("tests/golden/export/{name}.reqif"), &xml, &mut failures);
        for f in references(&xml) {
            failures.push(format!("{name}: {f}"));
        }
        // The counts are api's.
        let a = api(dir);
        let reqs = arr(&a["requirements"]);
        let req_type = {
            let i = xml.find("LONG-NAME=\"yuen requirement\"").unwrap();
            let v = &xml[xml[..i].rfind("IDENTIFIER=\"").unwrap() + 12..];
            v[..33].to_string()
        };
        let objects = xml.matches(&format!("<SPEC-OBJECT-TYPE-REF>{req_type}</SPEC-OBJECT-TYPE-REF>")).count();
        assert_eq!(objects, reqs.len(), "{name}: a SPEC-OBJECT for every requirement version");
        let rels: usize = reqs.iter().map(|r| arr(&r["from"]).iter().map(uppers).sum::<usize>() + arr(&r["links"]).len() + arr(&r["replaces"]).len()).sum();
        assert_eq!(xml.matches("<SPEC-RELATION ").count(), rels, "{name}: a SPEC-RELATION for every upper end of a from, every link, every replaces");
        assert_eq!(xml.matches("<SPEC-HIERARCHY ").count(), reqs.len() + count_sources(&a) + arr(&a["artifacts"]).len(), "{name}: every object is in a specification");
    }
    // The check catches what xmllint does not: a reference to nothing.
    let xml = export("reqif", "tests/fixtures/period", &[]);
    let i = xml.find("<SPEC-OBJECT-REF>").unwrap() + "<SPEC-OBJECT-REF>".len();
    let broken = format!("{}_00000000000000000000000000000000{}", &xml[..i], &xml[i + 33..]);
    assert!(references(&broken).iter().any(|f| f.contains("lands on nothing")), "the reference check finds a broken reference");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The distinct sources of a project in `api`: an article of a law as of a date, a file.
fn count_sources(a: &Value) -> usize {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for s in arr(&a["sources"]) {
        match s["kind"].as_str() {
            Some("law") => {
                for p in arr(&s["pins"]) {
                    seen.insert(format!("{} {} {} {}", s["db"], s["id"], s["asof"], p["fragment"]));
                }
            }
            Some("file") => {
                seen.insert(format!("file {}", s["path"]));
            }
            _ => {}
        }
    }
    seen.len()
}

#[test]
fn every_reqif_validates_against_the_schema_and_in_reqif() {
    let lint = common::xmllint_and_schema("the ReqIF schema check");
    let py = common::python("reqif validate");
    if lint.is_none() && py.is_none() {
        return;
    }
    let t = TempDir::new("reqif");
    for (name, dir) in PROJECTS {
        let file = t.write(&format!("{name}.reqif"), export("reqif", dir, &[]).as_bytes());
        if let Some((lint, xsd)) = &lint {
            let o = Command::new(lint)
                .args(["--nonet", "--noout", "--schema"])
                .arg(xsd.join("www.omg.org/spec/ReqIF/20110401/reqif.xsd"))
                .arg(&file)
                .env("XML_CATALOG_FILES", xsd.join("catalog.xml"))
                .output()
                .unwrap();
            let said = String::from_utf8_lossy(&o.stderr).to_string();
            assert!(o.status.success() && said.contains(&format!("{} validates", file.display())), "{name}: xmllint says\n{}", said.lines().filter(|l| !l.contains("Skipping import")).collect::<Vec<_>>().join("\n"));
            println!("compared: {name}.reqif validates against the OMG schema (xmllint, through the catalog, --nonet)");
        }
        if let Some(py) = &py {
            for strict in [false, true] {
                // What the `reqif` command runs, through the Python that has it.
                let mut c = Command::new(py);
                c.args(["-c", "import sys; from reqif.cli.main import main; sys.argv[0] = 'reqif'; sys.exit(main())", "validate"]);
                if strict {
                    c.arg("--use-reqif-schema");
                }
                let o = c.arg(&file).output().unwrap();
                let said = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
                assert!(o.status.success() && said.contains("0 errors, 0 schema issues found, 0 semantic issues found"), "{name}: reqif validate{} says\n{said}", if strict { " --use-reqif-schema" } else { "" });
            }
            println!("compared: {name}.reqif passes reqif validate, and with --use-reqif-schema");
        }
    }
}

/// The records PROV holds, by type, as `yuen api` counts them (DESIGN 13).
fn prov_counts(a: &Value) -> BTreeMap<String, usize> {
    let reqs = arr(&a["requirements"]);
    let mut c: BTreeMap<String, usize> = BTreeMap::new();
    let mut add = |k: &str, n: usize| *c.entry(k.to_string()).or_default() += n;
    add("Entity", reqs.len() + count_sources(a) + arr(&a["artifacts"]).len());
    add("Agent", arr(&a["roles"]).len());
    let mut versions: BTreeMap<String, usize> = BTreeMap::new();
    for r in reqs {
        *versions.entry(r["name"].as_str().unwrap().to_string()).or_default() += 1;
        let decided = arr(&r["decided"]).len();
        let froms = arr(&r["from"]);
        let links = arr(&r["links"]);
        let waivers = arr(&r["waivers"]);
        let looked_from: Vec<&Value> = froms.iter().filter(|f| !f["reviewed"].is_null()).collect();
        let looked_links = links.iter().filter(|l| !l["reviewed"].is_null()).count();
        let cited: BTreeSet<String> = froms.iter().flat_map(|f| arr(&f["sources"]).iter().map(|s| format!("{} {}", s["source"], s["fragment"]))).collect();
        add("Activity", decided + looked_from.len() + looked_links + waivers.len());
        add("Derivation", froms.iter().map(uppers).sum::<usize>() + arr(&r["replaces"]).len());
        add("Attribution", usize::from(!r["owner"].is_null()));
        add("Influence", decided + links.len());
        add("Association", decided + looked_from.len() + looked_links + waivers.iter().filter(|w| !w["approved"].is_null()).count());
        add("Usage", decided * cited.len() + looked_from.iter().map(|f| uppers(f) + 1).sum::<usize>() + looked_links * 2 + waivers.len());
    }
    add("Derivation", versions.values().map(|n| n - 1).sum());
    // What the files of the rules and calendars pin (DESIGN 3.3): each file once (an entity of
    // its own when no link names it whole), each article it pins, and the articles no `.req`
    // declares.
    let mut sources: BTreeSet<String> = BTreeSet::new();
    for s in arr(&a["sources"]) {
        for p in s["pins"].as_array().into_iter().flatten() {
            sources.insert(format!("{} {} {} {}", s["db"], s["id"], s["asof"], p["fragment"]));
        }
    }
    let mut files: BTreeMap<String, &Value> = BTreeMap::new();
    for art in arr(&a["artifacts"]) {
        if !arr(&art["pins"]).is_empty() {
            files.entry(art["path"].as_str().unwrap().to_string()).or_insert(art);
        }
    }
    let mut more: BTreeSet<String> = BTreeSet::new();
    for (path, art) in &files {
        let whole = arr(&a["artifacts"]).iter().any(|x| x["path"] == path.as_str() && arr(&x["items"]).is_empty());
        add("Entity", usize::from(!whole));
        add("Influence", arr(&art["pins"]).len());
        for p in arr(&art["pins"]) {
            let key = format!("{} {} {} {}", p["db"], p["id"], p["asof"], p["fragment"]);
            if !sources.contains(&key) {
                more.insert(key);
            }
        }
    }
    add("Entity", more.len());
    c.retain(|_, n| *n > 0);
    c
}

const READ_BOTH: &str = r#"
import collections, json, sys
from prov.model import ProvDocument
n = ProvDocument.deserialize(sys.argv[1], format="provn", profile="strict")
j = ProvDocument.deserialize(sys.argv[2], format="json")
counts = collections.Counter(r.get_type().localpart for r in n.get_records())
print(json.dumps({"equal": n == j, "counts": counts}))
"#;

#[test]
fn every_prov_reads_as_one_document_in_both_forms() {
    let mut failures = Vec::new();
    let mut written = Vec::new();
    for (name, dir) in PROJECTS {
        let n = export("prov", dir, &[]);
        let j = export("prov", dir, &["--format", "json"]);
        assert_eq!(n, export("prov", dir, &["--format", "provn"]), "{name}: PROV-N is the default");
        assert_eq!(j, export("prov", dir, &["--format", "json"]), "{name}: two exports are the same bytes");
        assert!(!n.contains("hadPrimarySource") && !j.contains("hadPrimarySource"), "{name}: hadPrimarySource is not written (DESIGN 13)");
        assert!(n.contains("prov:type='prov:PrimarySource'"), "{name}");
        serde_json::from_str::<Value>(&j).unwrap();
        common::golden(&format!("tests/golden/export/{name}.provn"), &n, &mut failures);
        common::golden(&format!("tests/golden/export/{name}.prov.json"), &j, &mut failures);
        written.push((name, dir, n, j));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let Some(py) = common::python("the PROV reading") else { return };
    let t = TempDir::new("prov");
    let script = t.write("read_both.py", READ_BOTH.as_bytes());
    for (name, dir, n, j) in written {
        let pn = t.write(&format!("{name}.provn"), n.as_bytes());
        let pj = t.write(&format!("{name}.json"), j.as_bytes());
        let o = Command::new(&py).arg(&script).arg(&pn).arg(&pj).output().unwrap();
        assert!(o.status.success(), "{name}: prov cannot read it:\n{}", String::from_utf8_lossy(&o.stderr));
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(v["equal"], true, "{name}: PROV-N and PROV-JSON read as the same document");
        let got: BTreeMap<String, usize> = v["counts"].as_object().unwrap().iter().map(|(k, n)| (k.clone(), n.as_u64().unwrap() as usize)).collect();
        assert_eq!(got, prov_counts(&api(dir)), "{name}: the records of each type are the ones api counts");
        println!("compared: {name} PROV-N and PROV-JSON read by prov as one document of {} records", got.values().sum::<usize>());
    }
}

/// The words of yuen's namespace, as every project writes them in PROV-JSON: for each word, what
/// it is and where it is written (`Term::on`).
fn words_written() -> BTreeMap<String, (Kind, BTreeSet<String>)> {
    let mut words: BTreeMap<String, (Kind, BTreeSet<String>)> = BTreeMap::new();
    let mut note = |name: &str, kind: Kind, on: &str, failures: &mut Vec<String>| {
        let entry = words.entry(name.to_string()).or_insert((kind, BTreeSet::new()));
        if entry.0 != kind {
            failures.push(format!("yuen:{name} is written as a {:?} and as a {kind:?}", entry.0));
        }
        entry.1.insert(on.to_string());
    };
    let mut failures = Vec::new();
    for (name, dir) in PROJECTS {
        let doc: Value = serde_json::from_str(&export("prov", dir, &["--format", "json"])).unwrap();
        for (record, group) in doc.as_object().unwrap() {
            if record == "prefix" {
                continue;
            }
            for body in group.as_object().unwrap().values() {
                // `prov:type` is a qualified name: `{"$": "yuen:Role", "type": "prov:QUALIFIED_NAME"}`.
                let ty = body.get("prov:type").and_then(|t| t["$"].as_str()).and_then(|t| t.strip_prefix("yuen:"));
                if ["entity", "agent", "activity"].contains(&record.as_str()) {
                    let Some(ty) = ty else {
                        failures.push(format!("{name}: a {record} without a type of yuen's"));
                        continue;
                    };
                    note(ty, Kind::Type, record, &mut failures);
                    for key in body.as_object().unwrap().keys().filter_map(|k| k.strip_prefix("yuen:")) {
                        note(key, Kind::Attribute, ty, &mut failures);
                    }
                } else if let Some(ty) = ty {
                    note(ty, Kind::Relation, record, &mut failures);
                    // A relation carries no attribute of yuen's.
                    if body.as_object().unwrap().keys().any(|k| k.starts_with("yuen:")) {
                        failures.push(format!("{name}: a {record} carries an attribute of yuen's"));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    words
}

/// The words of yuen's namespace (`yuen::export::prov::TERMS`, which the pages of the namespace on
/// ritsu's site are held to) are the ones the export writes: every word that a project writes is
/// on the list, as the kind of word it is and on the records the list says, and every word on the
/// list is written by some project. A word the code spells that no project reaches would pass
/// that, so the spelling in the source is held to the list as well.
#[test]
fn the_words_of_the_namespace_are_the_ones_the_export_writes() {
    let written = words_written();
    let mut listed: BTreeMap<String, (Kind, BTreeSet<String>)> = BTreeMap::new();
    for t in TERMS {
        let was = listed.insert(t.name.to_string(), (t.kind, t.on.iter().map(|s| s.to_string()).collect()));
        assert!(was.is_none(), "TERMS lists {} twice", t.name);
    }
    let mut wrong = Vec::new();
    for (name, (kind, on)) in &written {
        match listed.get(name) {
            None => wrong.push(format!("the export writes yuen:{name} ({kind:?}, on {on:?}), which TERMS does not list")),
            Some(l) if l != &(*kind, on.clone()) => wrong.push(format!("yuen:{name}: the export writes it as {kind:?} on {on:?}, TERMS lists {:?} on {:?}", l.0, l.1)),
            _ => {}
        }
    }
    for name in listed.keys().filter(|n| !written.contains_key(*n)) {
        wrong.push(format!("TERMS lists yuen:{name}, which no project of the tests writes"));
    }
    // A word the code spells and the projects do not reach would pass the comparison above.
    let code = include_str!("../src/export/prov.rs");
    let mut spelled: BTreeSet<String> = BTreeSet::new();
    let mut rest = code;
    while let Some(i) = rest.find("yuen:") {
        let after = &rest[i + 5..];
        let word: String = after.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        if !word.is_empty() && !rest[..i].ends_with("urn:") {
            spelled.insert(word);
        }
        rest = after;
    }
    for name in spelled.iter().filter(|n| !listed.contains_key(*n)) {
        wrong.push(format!("src/export/prov.rs spells yuen:{name}, which TERMS does not list"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // The namespace is one IRI, and a word is it and the word.
    assert!(YUEN_NS.ends_with("/ns/yuen#"), "{YUEN_NS}");
    for (name, dir) in PROJECTS {
        let n = export("prov", dir, &[]);
        assert!(n.contains(&format!("  prefix yuen <{YUEN_NS}>\n")), "{name}: PROV-N declares the prefix yuen");
        let j: Value = serde_json::from_str(&export("prov", dir, &["--format", "json"])).unwrap();
        assert_eq!(j["prefix"]["yuen"], YUEN_NS, "{name}: PROV-JSON declares the prefix yuen");
    }
}

#[test]
fn what_stops_an_export_and_what_does_not() {
    // An error of the first four stages: nothing is written.
    let m = "tests/mutants/E101_写しが無い";
    for what in ["reqif", "prov"] {
        let r = run(&["export", what, m, "--root", m]);
        assert_eq!(r.code, 1, "{what}: {}", r.stderr);
        assert!(r.stdout.is_empty(), "{what}");
        assert!(r.stderr.contains("error[E101]"), "{what}: {}", r.stderr);
    }
    // A mark is written as the state of the relation.
    let m = "tests/mutants/E302_条が変わった";
    let xml = export("reqif", m, &[]);
    assert!(xml.contains("THE-VALUE=\"up_changed\""), "the mark is in the ReqIF");
    assert!(references(&xml).is_empty());
    let n = export("prov", m, &[]);
    assert!(n.contains("yuen:status=\"up_changed\""), "the mark is in PROV");
    // A project that writes down no day: ReqIF needs --time; PROV does not.
    let t = TempDir::new("notime");
    t.write("t.req", "requirements t v1\nrole 経理\n\nrequirement r1\n  text \"x\"\n  owner 経理\n  from r0\n  not satisfied \"y\"\n  not verified \"z\"\n\nrequirement r0\n  text \"w\"\n  owner 経理\n  from r1\n  not satisfied \"y\"\n  not verified \"z\"\n".as_bytes());
    let r = common::yuen(t.path(), &["export", "reqif", ".", "--root", "."]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("--time 2026-10-03T00:00:00Z"), "{}", r.stderr);
    let r = common::yuen(t.path(), &["export", "reqif", ".", "--root", ".", "--time", "2026-10-03T09:00:00+09:00"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.contains("<CREATION-TIME>2026-10-03T09:00:00+09:00</CREATION-TIME>"));
    assert!(r.stdout.contains("THE-VALUE=\"unreadable\""), "the links of a cycle (E405), whose ends cannot be made, are written as states");
    assert_eq!(common::yuen(t.path(), &["export", "prov", ".", "--root", "."]).code, 0);
    // The flags of one form are refused with the other.
    let p = "tests/fixtures/period";
    assert_eq!(run(&["export", "reqif", p, "--format", "json"]).code, 2);
    assert_eq!(run(&["export", "prov", p, "--time", "2026-10-03T00:00:00Z"]).code, 2);
    assert_eq!(run(&["export", "reqif", p, "--time", "2026-10-03"]).code, 2);
    assert_eq!(run(&["export", "xml", p]).code, 2);
    assert_eq!(run(&["export", "reqif"]).code, 2);
    // A value the string type cannot hold, and a character XML cannot hold.
    let t = TempDir::new("long");
    let long = "あ".repeat(100_001);
    t.write("t.req", format!("requirements t v1\ndescription \"{long}\"\nrole 経理\n\nrequirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"y\"\n  not satisfied \"y\"\n  not verified \"z\"\n").as_bytes());
    let r = common::yuen(t.path(), &["export", "reqif", ".", "--root", "."]);
    assert_eq!(r.code, 2, "{}", r.stderr);
    assert!(r.stderr.contains("100,001 characters, more than the 100,000"), "{}", r.stderr);
    let t = TempDir::new("ctrl");
    t.write("t.req", "requirements t v1\nrole 経理\n\nrequirement r1\n  text \"x\u{1}y\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"y\"\n  not satisfied \"y\"\n  not verified \"z\"\n".as_bytes());
    let r = common::yuen(t.path(), &["export", "reqif", ".", "--root", "."]);
    assert_eq!(r.code, 2, "{}", r.stderr);
    assert!(r.stderr.contains("U+0001"), "{}", r.stderr);
    // --out writes the file and says so.
    let t = TempDir::new("out");
    let out = t.path().join("period.reqif");
    let r = run(&["export", "reqif", p, "--root", p, "--out", out.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(r.stdout, format!("wrote: {}\n", out.display()));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), export("reqif", p, &[]));
}
