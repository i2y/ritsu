//! ritsu's ports, as yuen answers them (ritsu's DESIGN 3.2), in English: the twin of ports.rs, on
//! the fixtures `period_of_months`, `payment_policy`, `fee_rules` and `calendar_sources`.

mod common;

use ritsu_ports::{Items, References};
use std::path::PathBuf;
use yuen::ports::Engine;

fn root(at: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(at)
}

#[test]
fn a_requirements_definition_is_its_end_in_english() {
    let r = root("tests/fixtures/period_of_months");
    let items = Engine::default().items(&r, "period_of_months.req").unwrap();
    let hash = |name: &str| ritsu_base::sha256::short(items.iter().find(|i| i.kind() == "requirement" && i.name() == name).unwrap().text.as_bytes());
    // the hashes the file's own records were reviewed against
    assert_eq!(hash("date_of_receipt"), "c6a57f069e4638f2");
    assert_eq!(hash("timely_filing"), "8d02c6a1f0ad3364");
    let first = items.iter().find(|i| i.name() == "date_of_receipt").unwrap();
    assert!(first.text.starts_with("text Correspondence received in the Patent and Trademark Office"), "{}", first.text);
    assert_eq!(first.naming.text(), "yuen \"period_of_months.req\" requirement date_of_receipt");
    assert!(first.lines.1 > first.lines.0);
    // a source's definition is a line for each section it pins
    let law = items.iter().find(|i| i.kind() == "source" && i.name() == "cfr").unwrap();
    assert_eq!(law.text.lines().count(), 4, "{}", law.text);
    assert!(law.text.starts_with("from law ecfr 37 CFR 1 §1.6 sha256:6bcdc27c3428886c\n"), "{}", law.text);
    // every requirement's end is the one `yuen check` computes
    let checked = yuen::check::check(&[r.join("period_of_months.req").to_string_lossy().to_string()], Some(&r.to_string_lossy())).unwrap();
    let p = checked.project.as_ref().unwrap();
    let ends = &checked.model.as_ref().unwrap().req_ends;
    for it in items.iter().filter(|i| i.kind() == "requirement") {
        let k = p.reqs.iter().position(|v| v.name == it.name()).unwrap();
        assert_eq!(it.text.as_bytes(), ends[k].as_ref().unwrap().bytes.as_slice(), "{}", it.name());
    }
}

#[test]
fn a_req_names_what_meets_its_requirements_in_english() {
    for (dir, file) in [("tests/fixtures/period_of_months", "period_of_months.req"), ("tests/fixtures/payment_policy", "payment.req"), ("tests/fixtures/fee_rules", "extension_fees.req"), ("tests/fixtures/calendar_sources", "payment_terms.req")] {
        let r = root(dir);
        let refs = Engine::default().references(&r, file).unwrap();
        assert!(refs.iter().any(|x| x.how == "satisfied by"), "{file}: {refs:?}");
        for x in &refs {
            assert_eq!(ritsu_base::naming::parse_one(&x.target.text()).map(|n| n.text()).ok(), Some(x.target.text()), "{file}");
        }
        let n = std::fs::read_to_string(r.join(file)).unwrap().lines().count();
        for it in Engine::default().items(&r, file).unwrap() {
            assert!(it.lines.0 >= 1 && it.lines.0 <= it.lines.1 && it.lines.1 <= n, "{file}: {it:?}");
        }
    }
    let refs = Engine::default().references(&root("tests/fixtures/period_of_months"), "period_of_months.req").unwrap();
    assert!(refs.iter().any(|x| x.how == "scope" && x.target.text() == "file \"period_of_months.cal\""), "{refs:?}");
}

/// A requirement read from a borrowed source has its definition when the language it borrows
/// from is joined (`Engine::with`, as `ritsu yuen` makes it): the same end as the requirement read
/// from its own copies. yuen alone gives it none, and a link to it is not reviewed against an
/// empty text.
#[test]
fn a_borrowed_source_is_read_through_its_language_in_english() {
    let r = root("tests/fixtures/calendar_sources");
    let alone = Engine::default().items(&r, "period_of_months.req").unwrap();
    let joined = Engine::with(common::suite()).items(&r, "period_of_months.req").unwrap();
    let text = |items: &[ritsu_ports::Item], name: &str| items.iter().find(|i| i.kind() == "requirement" && i.name() == name).unwrap().text.clone();
    assert_eq!(text(&alone, "last_day_moved"), "");
    assert_eq!(ritsu_base::sha256::short(text(&joined, "last_day_moved").as_bytes()), "edc4cc0bb4910f57");
    let src = joined.iter().find(|i| i.kind() == "source").unwrap();
    assert_eq!(src.text, "koyomi \"calendars/england_and_wales.cal\" source bank_holidays\n");
}
