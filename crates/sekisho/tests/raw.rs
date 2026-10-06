//! The data of every combination (DESIGN 6.3, `src/raw.rs`), which the tests of the TypeScript,
//! the Python and the Go hand to the generated code: for every `.gate` of the examples and the
//! tests that passes its check, each case's answer by the reference evaluation of its data is the
//! answer the walk gave the combination it was made from, every combination the languages say can
//! happen is given data, and the faults the code is to refuse are refused with their kind.

mod common;

use sekisho::raw;
use std::path::Path;

/// Every `.gate` under the examples and the tests, in order.
fn gates() -> Vec<String> {
    fn walk(d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "gate") {
                out.push(p.to_string_lossy().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(Path::new("examples"), &mut out);
    walk(Path::new("tests"), &mut out);
    out.sort();
    out
}

#[test]
fn every_combination_is_given_data_that_comes_to_its_answer() {
    let suite = common::joined();
    let mut wrong = Vec::new();
    let mut made = 0;
    for gate in gates() {
        let o = common::check(&gate);
        let (Some(scope), Some(checked)) = (o.scope.as_ref(), o.walked.as_ref()) else { continue };
        if o.has_errors() {
            continue;
        }
        let r = raw::raw(scope, checked, &suite);
        made += 1;
        let (refused, allowed) = r.counts();
        println!("{gate}: {} combinations, {} cases ({allowed} allowed, {refused} refused), {} combinations no data gives", r.combinations, r.cases.len(), r.unrealized.len());
        for d in &r.disagree {
            wrong.push(format!("{gate}: {d}"));
        }
        // every case's answer is the reference evaluation of its data, again
        let langs = raw::langs(&suite);
        let m = raw::Model::new(scope, checked, &langs);
        for c in &r.cases {
            let again = m.evaluate(c);
            if again != c.expect {
                wrong.push(format!("{gate}: {}: evaluated again, {}", c.name, again.json().compact()));
            }
        }
    }
    assert!(made >= 20, "{made} gates");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The kind of refusal a case of a fault is to come to, by its name.
fn kind_of(name: &str) -> &'static str {
    if name.contains("today's range") {
        "today"
    } else if name.contains(": the input ") {
        "input"
    } else if name.contains("a resource ") || name.contains(": resource.") {
        "resource"
    } else {
        "principal"
    }
}

/// The example: 1,078 combinations, 2,134 of their cases at the two ends of the cells (as the
/// vectors), each fault refused with its kind, and a day outside today's range answered by the
/// actions that compute nothing from today. Its English and Japanese versions give the same data
/// and answers, but for the names of the gate and of the policies.
#[test]
fn the_example_is_given_both_ends_and_every_fault() {
    let suite = common::joined();
    let mut texts = Vec::new();
    for gate in ["examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate"] {
        let o = common::check(gate);
        let (scope, checked) = (o.scope.as_ref().unwrap(), o.walked.as_ref().unwrap());
        let r = raw::raw(scope, checked, &suite);
        assert_eq!(r.combinations, 1078, "{gate}");
        assert!(r.unrealized.is_empty() && r.disagree.is_empty(), "{gate}: {:?} {:?}", r.unrealized, r.disagree);
        // a day outside today's range, for an action that computes nothing from today: answered
        let answered: Vec<&raw::Case> = r.cases.iter().filter(|c| c.name.ends_with(", which it does not read")).collect();
        assert_eq!(answered.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["view_order: a day after today's range, which it does not read", "export_refunds: a day after today's range, which it does not read"], "{gate}");
        assert!(answered.iter().all(|c| c.expect.error.is_none() && c.expect.context.is_some()), "{gate}");
        let faults: Vec<&raw::Case> = r.cases.iter().filter(|c| c.name.contains(": ") && !answered.iter().any(|a| a.name == c.name)).collect();
        assert_eq!(r.cases.len() - faults.len() - answered.len(), 2134, "{gate}: as many as the vectors");
        for c in &faults {
            assert_eq!(c.expect.error.as_deref(), Some(kind_of(&c.name)), "{gate}: {}", c.name);
        }
        let kinds: std::collections::BTreeSet<&str> = faults.iter().filter_map(|c| c.expect.error.as_deref()).collect();
        assert_eq!(kinds.into_iter().collect::<Vec<_>>(), ["input", "principal", "resource", "today"], "{gate}");
        // the instants: both edges of a day, at the day's offset
        assert!(r.cases.iter().any(|c| c.now.ends_with("T00:00:00+00:00")) && r.cases.iter().any(|c| c.now.ends_with("T23:59:59+00:00")));
        let alias = &checked.gate.named.alias;
        texts.push(r.json(&checked.gate).replace(&format!("\"{alias}/"), "\"").replace(&format!("\"gate\": \"{alias}\""), "\"gate\""));
        println!("{gate}: {} cases, {} of faults", r.cases.len(), faults.len());
    }
    assert!(texts[0] == texts[1], "the English and the Japanese versions give other data");
}
