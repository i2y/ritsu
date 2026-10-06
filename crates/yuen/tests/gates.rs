//! The things of a gate as the ends of links (sekisho's DESIGN 8.4): a policy that meets a
//! requirement and an expectation that checks it, each named by itself (`sekisho "refunds.gate"
//! policy clerks_refund`), read through the index from sekisho's `Items`. The material is in English
//! (`refund_gates`) and in Japanese (`返金のゲート`), and the two read the same.
//!
//! An end is the thing's block, each line without its comment and alignment, under the one above by
//! its depth: a change to another policy of the same gate changes neither end, and a change to the
//! policy changes its own.

mod common;

use std::collections::BTreeMap;

/// The end of every artifact the links name, by its reference, and the codes the check says.
fn ends(dir: &str) -> (BTreeMap<String, String>, Vec<&'static str>) {
    let c = common::check(dir);
    let m = c.model.as_ref().unwrap_or_else(|| panic!("{dir} does not read: {:?}", c.diags.iter().map(|d| (d.code, d.message.en.clone())).collect::<Vec<_>>()));
    let ends = m.artifacts.iter().map(|(n, e)| (n.text(), e.as_ref().map(|e| String::from_utf8_lossy(&e.bytes).to_string()).unwrap_or_else(|u| format!("{u:?}")))).collect();
    (ends, c.diags.iter().map(|d| d.code).collect())
}

/// The two versions of the material: its directory, the gate, and the policy's and the
/// expectation's names and ends.
const MATERIALS: [(&str, &str, &str, &str, &str, &str); 2] = [
    (
        "refund_gates",
        "refunds.gate",
        "clerks_refund",
        "permit clerks_refund\n  description \"A clerk refunds an order\"\n  principal in clerk\n  action refund_order",
        "clerks_who_are_not_suspended_refund",
        "expect allow clerks_who_are_not_suspended_refund\n  description \"A clerk who is not suspended refunds\"\n  principal in clerk\n  action refund_order\n  unless principal.suspended",
    ),
    (
        "返金のゲート",
        "返金.gate",
        "係は返金できる",
        "permit 係は返金できる(clerks_refund)\n  description \"係は注文を返金できる\"\n  principal in 係\n  action 返金する",
        "停止中でない係は返金できる",
        "expect allow 停止中でない係は返金できる\n  description \"停止中でない係は返金できる\"\n  principal in 係\n  action 返金する\n  unless principal.停止中",
    ),
];

#[test]
fn a_policy_and_an_expectation_of_a_gate_are_the_ends_of_links() {
    for (dir, gate, policy, policy_end, expect, expect_end) in MATERIALS {
        let (e, codes) = ends(&format!("tests/fixtures/{dir}"));
        // the two links have no record yet, and nothing else is said
        assert_eq!(codes, ["E301", "E301"], "{dir}");
        assert_eq!(e[&format!("sekisho \"{gate}\" policy {policy}")], policy_end, "{dir}");
        assert_eq!(e[&format!("sekisho \"{gate}\" expect {expect}")], expect_end, "{dir}");
    }
}

#[test]
fn a_change_to_another_policy_leaves_the_ends_as_they_were() {
    for (dir, gate, policy, policy_end, expect, expect_end) in MATERIALS {
        let t = common::fixture(dir);
        let d = t.path().join(dir);
        // the forbid's description, the spacing of the permit, and a comment on it
        let (forbid, changed) = if dir == "refund_gates" {
            ("description \"A suspended member of the staff does nothing\"", "description \"A suspended member of the staff does nothing at all\"")
        } else {
            ("description \"停止中の職員は何もできない\"", "description \"停止中の職員は、何もできない\"")
        };
        common::edit(&d, gate, forbid, changed);
        common::edit(&d, gate, "  principal in ", "  principal   in ");
        common::edit(&d, gate, &format!("{}\n", policy_end.lines().last().unwrap()), &format!("{}   # the action it allows\n", policy_end.lines().last().unwrap()));
        let (e, _) = ends(&d.to_string_lossy());
        assert_eq!(e[&format!("sekisho \"{gate}\" policy {policy}")], policy_end, "{dir}");
        assert_eq!(e[&format!("sekisho \"{gate}\" expect {expect}")], expect_end, "{dir}");
        // the permit's own description is its end's
        let said = if dir == "refund_gates" { "A clerk refunds an order" } else { "係は注文を返金できる" };
        common::edit(&d, gate, said, &format!("{said}, in whole or in part"));
        let (e, _) = ends(&d.to_string_lossy());
        assert_ne!(e[&format!("sekisho \"{gate}\" policy {policy}")], policy_end, "{dir}");
    }
}
