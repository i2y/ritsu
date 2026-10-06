//! sekisho's gates and the Cedar written by hand on the map (sekisho's DESIGN 8.5, DESIGN 17): a
//! `.gate` is an artifact as a `.rule` is, and belongs to one context; a file of Cedar is one where
//! an entry names it (`cedar "…"`). What each names outside itself is held to the map as any
//! artifact's is, and an operation is guarded only by its own context (E211). On the example
//! `webshop`, with a gate and a pair of Cedar laid over it.

mod common;

use ritsu_base::text::Lang;
use std::path::Path;

/// The codes the check of the map says, in order, and what it prints.
fn said(dir: &Path) -> (Vec<String>, String) {
    let os = common::check_dir(dir);
    let codes = os.iter().flat_map(|o| o.diags.iter().map(|d| d.code.to_string())).collect();
    (codes, os.iter().flat_map(|o| o.diags.iter().map(|d| d.render(Lang::En))).collect())
}

fn webshop() -> common::TempDir {
    let t = common::TempDir::new("gates");
    common::copy_dir(Path::new("examples/webshop"), t.path());
    t
}

fn write(t: &common::TempDir, name: &str, body: &str) {
    let p = t.path().join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

/// A gate of Payments that guards Payments' own operation.
const CHARGES: &str = "gate charges v1\n\nuse openapi payments from \"../api/payments.yaml\"\n\nrole cashier\n\nprincipal Staff\n  roles cashier\n\nresource Charge\n\naction create_charge\n  guards payments createCharge\n  principal Staff\n  resource Charge\n\npermit cashiers_charge\n  principal in cashier\n  action create_charge\n";

/// A pair of Cedar whose schema guards Payments' `getCharge`.
const SCHEMA: &str = "namespace Payments {\n  entity Staff;\n  entity Charge;\n\n  @guards(\"openapi \\\"payments/api/payments.yaml\\\" operation getCharge\")\n  action \"get_charge\" appliesTo {\n    principal: [Staff],\n    resource: [Charge],\n    context: {}\n  };\n}\n";
const POLICIES: &str = "@id(\"staff_look\")\npermit (principal, action == Payments::Action::\"get_charge\", resource);\n";

#[test]
fn a_gate_and_a_pair_of_cedar_of_the_context_that_holds_the_operations() {
    let t = webshop();
    write(&t, "payments/gates/charges.gate", CHARGES);
    write(&t, "payments/policies/charges.cedarschema", SCHEMA);
    write(&t, "payments/policies/charges.cedar", POLICIES);
    // the gate is an artifact Payments owns by its directory; the pair, by the entries that name it
    let ctx = t.path().join("contexts/payments.ctx");
    let src = std::fs::read_to_string(&ctx).unwrap();
    std::fs::write(&ctx, src.replace("owns\n  dir \"../payments\"\n", "owns\n  dir \"../payments\"\n  cedar \"../payments/policies/charges.cedar\", \"../payments/policies/charges.cedarschema\"\n")).unwrap();
    let (codes, text) = said(t.path());
    assert!(codes.is_empty(), "{text}");
}

#[test]
fn an_operation_guarded_from_another_context_is_e211() {
    let t = webshop();
    // a pair of Cedar in Ordering whose schema guards Payments' operation
    write(&t, "ordering/policies/charges.cedarschema", SCHEMA);
    write(&t, "ordering/policies/charges.cedar", POLICIES);
    let ctx = t.path().join("contexts/ordering.ctx");
    let src = std::fs::read_to_string(&ctx).unwrap();
    std::fs::write(&ctx, src.replace("owns\n  dir \"../ordering\", \"../common\"\n", "owns\n  dir \"../ordering\", \"../common\"\n  cedar \"../ordering/policies/charges.cedarschema\"\n")).unwrap();
    let (codes, text) = said(t.path());
    assert_eq!(codes, ["E211"], "{text}");
    assert!(text.contains("error[E211]: ordering/policies/charges.cedarschema:5:1: The file ordering/policies/charges.cedarschema of Ordering guards openapi \"payments/api/payments.yaml\" operation getCharge, an operation of Payments"), "{text}");
}

#[test]
fn a_file_of_cedar_no_entry_names_is_no_artifact() {
    let t = webshop();
    // in Payments' directory, and no entry names it: it belongs to no context, and nothing is said
    write(&t, "payments/policies/charges.cedarschema", SCHEMA);
    write(&t, "payments/policies/charges.cedar", POLICIES);
    let (codes, text) = said(t.path());
    assert!(codes.is_empty(), "{text}");
    // a `cedar` entry names a Cedar file, and a gate no context owns is E101
    let ctx = t.path().join("contexts/payments.ctx");
    let src = std::fs::read_to_string(&ctx).unwrap();
    std::fs::write(&ctx, src.replace("owns\n  dir \"../payments\"\n", "owns\n  dir \"../payments\"\n  cedar \"../payments/api/payments.yaml\"\n")).unwrap();
    write(&t, "elsewhere/charges.gate", CHARGES);
    let (codes, text) = said(t.path());
    assert!(codes.contains(&"E011".to_string()), "{text}");
    assert!(text.contains("A cedar line names a .cedar, .cedarschema or .cedarschema.json file (\"../payments/api/payments.yaml\")"), "{text}");
}

/// A rule, in English and in Japanese: whether a payment is small or large.
const RULES: [(&str, &str); 2] = [
    (
        "limits.rule",
        "rule limits v1\ndescription \"Whether a payment is small or large\"\n\nenum amount_band = small | large\n\ninputs\n  amount : number  range >=0 <=1000\n\noutputs\n  band : amount_band\n\ntable decide\npolicy unique\n| amount | -> band : amount_band |\n| <=100  | small                 |\n| >100   | large                 |\n",
    ),
    (
        "上限.rule",
        "rule 上限(limits_ja) v1\ndescription \"支払いの額が少額か多額か\"\n\nenum 額の区分(amount_band) = 少額(small) | 多額(large)\n\ninputs\n  金額(amount) : number  range >=0 <=1000\n\noutputs\n  区分(band) : 額の区分\n\ntable 決める\npolicy unique\n| 金額  | -> 区分 : 額の区分 |\n| <=100 | 少額               |\n| >100  | 多額               |\n",
    ),
];

/// A gate that reads the rule at `rule` (from the gate's directory), in the language of the map.
fn gate_reading(ja: bool, rule: &str) -> String {
    if ja {
        format!("gate 上限(limits_ja) v1\n\nuse rule 上限 from \"{rule}\"\n\nrole 係(cashier)\n\nprincipal 職員(Staff)\n  roles 係\n\nresource 代金(Charge)\n\naction 見る(look)\n  principal 職員\n  resource 代金\n\npermit 係は見られる(cashiers_look)\n  principal in 係\n  action 見る\n")
    } else {
        format!("gate limits v1\n\nuse rule limits from \"{rule}\"\n\nrole cashier\n\nprincipal Staff\n  roles cashier\n\nresource Charge\n\naction look\n  principal Staff\n  resource Charge\n\npermit cashiers_look\n  principal in cashier\n  action look\n")
    }
}

/// A gate's `use rule` across a boundary reads the rule itself (DESIGN 17.2): from the two's shared
/// kernel it passes, and counts in the summary; a rule inside the other context is E202 with a
/// relationship and E201 without one. On both examples, `webshop` and `webshop.ja`, where Payments
/// shares `common/` with Ordering, and Shipping has no relationship with Ordering.
#[test]
fn a_gate_reads_a_rule_across_a_boundary_from_the_shared_kernel_only() {
    for (example, ja) in [("examples/webshop", false), ("examples/webshop.ja", true)] {
        let (file, body) = RULES[usize::from(ja)];
        let ordering = if ja { "受注" } else { "Ordering" };
        let gate = if ja { "上限.gate" } else { "limits.gate" };
        let fresh = || {
            let t = common::TempDir::new("gates");
            common::copy_dir(Path::new(example), t.path());
            t
        };
        // in the shared kernel
        let t = fresh();
        write(&t, &format!("common/{file}"), body);
        write(&t, &format!("payments/gates/{gate}"), &gate_reading(ja, &format!("../../common/{file}")));
        let (codes, text) = said(t.path());
        assert!(codes.is_empty(), "{example}: {text}");
        let os = common::check_dir(t.path());
        let summary = os.iter().find_map(|o| o.summary.as_ref()).expect("a summary");
        assert!(summary.en.contains("sekisho 1"), "{example}: {}", summary.en);
        // Ordering's own: E202, and why a gate's rule is the rule itself
        let t = fresh();
        write(&t, &format!("ordering/{file}"), body);
        write(&t, &format!("payments/gates/{gate}"), &gate_reading(ja, &format!("../../ordering/{file}")));
        let (codes, text) = said(t.path());
        assert_eq!(codes, ["E202"], "{example}: {text}");
        assert!(text.contains(&format!("The code a gate is made into calls the code made of the rule to compute its answer: the rule itself, inside {ordering}.")), "{text}");
        // from a context with no relationship with Ordering: E201
        let t = fresh();
        write(&t, &format!("ordering/{file}"), body);
        write(&t, &format!("shipping/gates/{gate}"), &gate_reading(ja, &format!("../../ordering/{file}")));
        let (codes, text) = said(t.path());
        assert_eq!(codes, ["E201"], "{example}: {text}");
        assert!(text.contains(&format!("rulec \"ordering/{file}\"")) && text.contains(ordering), "{text}");
    }
}
