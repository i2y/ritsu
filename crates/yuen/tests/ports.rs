//! ritsu's ports, as yuen answers them (ritsu's DESIGN 3.2, PLAN D.2): a `.req`'s requirements and
//! sources as items, each requirement's definition its end (the hash the records under its links
//! were reviewed against), and the namings it writes as references.

use ritsu_ports::{Items, References};
use std::path::PathBuf;
use yuen::ports::Engine;

fn root(at: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(at)
}

#[test]
fn a_requirements_definition_is_its_end() {
    let r = root("tests/fixtures/period");
    let items = Engine.items(&r, "民法の期間.req").unwrap();
    let hash = |name: &str| ritsu_base::sha256::short(items.iter().find(|i| i.kind() == "requirement" && i.name() == name).unwrap().text.as_bytes());
    // the hashes the file's own records were reviewed against
    assert_eq!(hash("起算日"), "a9ebc73907faddc8");
    assert_eq!(hash("満了日"), "465b83ed8c251406");
    let first = items.iter().find(|i| i.name() == "起算日").unwrap();
    assert!(first.text.starts_with("text 日、週、月又は年によって"), "{}", first.text);
    assert_eq!(first.naming.text(), "yuen \"民法の期間.req\" requirement 起算日");
    assert!(first.lines.1 > first.lines.0);
    // a source's definition is a line for each article it pins
    let law = items.iter().find(|i| i.kind() == "source" && i.name() == "民法").unwrap();
    assert_eq!(law.text.lines().count(), 4, "{}", law.text);
    assert!(law.text.starts_with("from law egov 129AC0000000089 第140条 sha256:e880059021fbb67d\n"), "{}", law.text);
    // every requirement's end is the one `yuen check` computes
    let checked = yuen::check::check(&[r.join("民法の期間.req").to_string_lossy().to_string()], Some(&r.to_string_lossy())).unwrap();
    let p = checked.project.as_ref().unwrap();
    let ends = &checked.model.as_ref().unwrap().req_ends;
    for it in items.iter().filter(|i| i.kind() == "requirement") {
        let k = p.reqs.iter().position(|v| v.name == it.name()).unwrap();
        assert_eq!(it.text.as_bytes(), ends[k].as_ref().unwrap().bytes.as_slice(), "{}", it.name());
    }
}

#[test]
fn a_req_names_what_meets_its_requirements() {
    for (dir, file) in [("tests/fixtures/period", "民法の期間.req"), ("tests/fixtures/payment", "支払.req"), ("tests/fixtures/ecfr", "osha.req")] {
        let r = root(dir);
        let refs = Engine.references(&r, file).unwrap();
        // osha.req names no artifact: its requirement is not satisfied, with a reason
        assert_eq!(refs.iter().any(|x| x.how == "satisfied by"), file != "osha.req", "{file}: {refs:?}");
        for x in &refs {
            assert_eq!(ritsu_base::naming::parse_one(&x.target.text()).map(|n| n.text()).ok(), Some(x.target.text()), "{file}");
        }
        let n = std::fs::read_to_string(r.join(file)).unwrap().lines().count();
        for it in Engine.items(&r, file).unwrap() {
            assert!(it.lines.0 >= 1 && it.lines.0 <= it.lines.1 && it.lines.1 <= n, "{file}: {it:?}");
        }
    }
    let osha = Engine.items(&root("tests/fixtures/ecfr"), "osha.req").unwrap();
    let req = osha.iter().find(|i| i.kind() == "requirement").unwrap();
    assert_eq!(ritsu_base::sha256::short(req.text.as_bytes()), "08a4819829372b9b");
    let refs = Engine.references(&root("tests/fixtures/period"), "民法の期間.req").unwrap();
    assert!(refs.iter().any(|x| x.how == "scope" && x.target.text() == "file \"民法の期間.cal\""), "{refs:?}");
}
