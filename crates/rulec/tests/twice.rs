//! A clause written twice is E155 (§15.348): a clause word of a declaration's tail
//! (`range`, `round`, `contract_only`, `random`, `unique`), or `policy`/`overrides` under a
//! table's heading. Each used to lose or win over its twin in silence — the first `round`
//! over the second, the second `range` over the first, `first` dropped after `policy unique`.

use rulec::i18n::{self, Lang};

fn codes(src: &str) -> Vec<String> {
    rulec::check_source(src, "t.rule")
        .iter()
        .map(|d| d.code.to_string())
        .collect()
}

fn e155s(src: &str) -> usize {
    rulec::check_source(src, "t.rule")
        .iter()
        .filter(|d| d.code == "E155")
        .count()
}

/// A second clause word of a declaration's tail is E155, at the second one, in both
/// languages.
#[test]
fn a_clause_word_written_twice_is_e155() {
    let table = "\ntable t\npolicy unique\n| qty | -> fee |\n| <=5 | 6USD   |\n| >5  | 12USD  |\n";
    // `round` written twice on an output, and `range` twice.
    for tail in [
        "round down(1USD)  round up(1USD)",
        "round down(1USD)  round down(1USD)",
        "round down(1USD)  range >=1USD <=100USD  range >=0USD <=99USD",
    ] {
        let src = format!("rule t v1\n\ninputs\n  qty : number  range >=1 <=10\n\noutputs\n  fee : money[USD]  {tail}\n{table}");
        for lang in [Lang::En, Lang::Ja] {
            let n = i18n::with(lang, || e155s(&src));
            assert_eq!(n, 1, "{tail} / {lang:?}");
        }
    }
    // A second `range`, `contract_only`, `random` on an input, and a second `unique` on a
    // field of `elements`.
    let src = "rule t v1\n\ninputs\n  qty : number  range >=1 <=10  range >=1 <=20\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable t\npolicy unique\n| qty | -> fee |\n| any | 6USD   |\n";
    assert_eq!(e155s(src), 1, "range twice: {:?}", codes(src));
    let src = "rule t v1\n\ninputs\n  qty : number  range >=1 <=10  contract_only  contract_only\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable t\npolicy unique\n| qty | -> fee |\n| any | 6USD   |\n";
    assert_eq!(e155s(src), 1, "contract_only twice: {:?}", codes(src));
    let src = "rule t v1\n\ninputs\n  qty : number  range >=1 <=10  random  random\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable t\npolicy unique\n| qty | -> fee |\n| any | 6USD   |\n";
    assert_eq!(e155s(src), 1, "random twice: {:?}", codes(src));
    let src = "rule t v1\n\nelements items\n  code : string  unique  unique\n  amount : money[USD]\n\ninputs\n  seq : items  range <=5\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable t\npolicy unique\n| count(seq) over items where amount >= 1USD | -> fee |\n| any                                        | 6USD   |\n";
    assert_eq!(e155s(src), 1, "unique twice: {:?}", codes(src));
}

/// A `policy` or `overrides` line under a table's heading, written a second time, is E155 —
/// and so is a second policy value on the `policy` line itself (`policy unique first`).
#[test]
fn a_tables_policy_or_overrides_written_twice_is_e155() {
    let head = "rule t v1\n\ninputs\n  qty : number  range >=1 <=10\n\noutputs\n  fee : money[USD]  round down(1USD)\n\n";
    let rows = "| qty | -> fee |\n| <=5 | 6USD   |\n| >5  | 12USD  |\n";
    assert_eq!(e155s(&format!("{head}table t\npolicy unique\npolicy first\n{rows}")), 1, "policy twice");
    assert_eq!(e155s(&format!("{head}table t\npolicy unique first\n{rows}")), 1, "policy with two values");
    let src = format!("{head}table base\npolicy unique\n| qty | -> fee |\n| any | 6USD   |\n\ntable ex\npolicy unique\noverrides base\noverrides base\n| qty | -> fee |\n| >8  | 20USD  |\n");
    assert_eq!(e155s(&src), 1, "overrides twice: {:?}", codes(&src));
    // Three `policy` lines: the second and the third are each E155, not some other error.
    let src = format!("{head}table t\npolicy unique\npolicy first\npolicy unique\n{rows}");
    assert_eq!(e155s(&src), 2, "policy three times: {:?}", codes(&src));
    // A clause is a one-row table: a second `overrides` line under it is E155 too.
    let src = format!("{head}table base\npolicy unique\n| qty | -> fee |\n| any | 6USD   |\n\nclause ex -> fee\nwhen qty >8\nthen 20USD\noverrides base\noverrides base\n");
    assert_eq!(e155s(&src), 1, "overrides under a clause: {:?}", codes(&src));
}

/// What is not a second clause keeps its own reading: a range bound that is an enum value, a
/// citation that ends the tail, and a word of a kind the line does not take (E047) or an
/// unknown policy (E007) are none of them a second clause.
#[test]
fn what_is_not_a_second_clause_stays_as_it_was() {
    // A range bound written as an enum value is not a `range` clause.
    let src = "rule t v1\n\nenum band = low < high\n\ninputs\n  b : band  range >=low <=high\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable t\npolicy unique\n| b   | -> fee |\n| any | 6USD   |\n";
    assert_eq!(e155s(src), 0, "{:?}", codes(src));
    // A citation ends the tail: `range` inside `@…` is not a clause.
    let src = "rule t v1\n\nsource s = file \"s.md\" sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n\ninputs\n  qty : number  range >=1 <=10  @s 第一条\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable t\npolicy unique\n| qty | -> fee |\n| any | 6USD   |\n";
    assert_eq!(e155s(src), 0, "{:?}", codes(src));
    // A word the line does not take is still E047, not E047 and E155.
    let src = "rule t v1\n\ninputs\n  qty : number  range >=1 <=10  round\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable t\npolicy unique\n| qty | -> fee |\n| any | 6USD   |\n";
    let ds = rulec::check_source(src, "t.rule");
    assert!(ds.iter().any(|d| d.code == "E047"), "{ds:?}");
    assert!(ds.iter().all(|d| d.code != "E155"), "{ds:?}");
    // An unknown policy keeps E007, not E155.
    let src = "rule t v1\n\ninputs\n  qty : number  range >=1 <=10\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable t\npolicy middle\n| qty | -> fee |\n| any | 6USD   |\n";
    let ds = rulec::check_source(src, "t.rule");
    assert!(ds.iter().any(|d| d.code == "E007"), "{ds:?}");
    assert!(ds.iter().all(|d| d.code != "E155"), "{ds:?}");
}
