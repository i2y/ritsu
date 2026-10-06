//! What `gen --target cedar` writes (DESIGN 5), with rulec, koyomi, chobo and dandori joined as
//! `ritsu sekisho` joins them: the policies and the schema of the example and of the gates of
//! `tests/gen/` — every form of condition (`conditions.gate` and its Japanese version), a forbid
//! read with `use gate` (`papers.gate`, which reads `people.gate`), an action that guards two
//! operations (`guards.gate`), one attribute written by its name and by its alias
//! (`two_spellings.gate` and its Japanese version) — as goldens, ritsu's version written
//! `<version>`; and the two versions of a gate, which come to the same Cedar and the same vectors
//! but for their names.
//!
//! Whether Cedar itself reads what is written, and decides as the vectors say, is
//! `tests/cedar.rs`. The goldens are `tests/golden/gen/`; `SEKISHO_BLESS=1` (or `RITSU_BLESS=1`)
//! writes them again.

mod common;

use ritsu_base::text::Lang;

const GATES: [&str; 9] = [
    "examples/refunds/refunds.gate",
    "examples/refunds/refunds.ja.gate",
    "tests/gen/conditions.gate",
    "tests/gen/conditions.ja.gate",
    "tests/gen/people.gate",
    "tests/gen/papers.gate",
    "tests/gen/guards.gate",
    "tests/gen/two_spellings.gate",
    "tests/gen/two_spellings.ja.gate",
];

/// The files `gen --target cedar` writes for the gate at `path`, in `lang`.
fn files(path: &str, lang: Lang) -> Vec<(String, String)> {
    let o = common::check(path);
    assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
    sekisho::r#gen::files(&o, "cedar", &ritsu_emit::header::file_name(path), lang).unwrap()
}

#[test]
fn the_cedar_of_each_gate() {
    for path in GATES {
        let made = files(path, Lang::En);
        let names: Vec<&str> = made.iter().map(|(rel, _)| rel.as_str()).collect();
        let alias = names[0].trim_start_matches("cedar/").trim_end_matches(".cedar");
        assert_eq!(names, vec![format!("cedar/{alias}.cedar"), format!("cedar/{alias}.cedarschema"), format!("cedar/{alias}.cedarschema.json"), format!("cedar/{alias}.policies.json")], "{path}");
        for (rel, body) in &made {
            let at = rel.trim_start_matches("cedar/");
            let shown = body.replace(env!("CARGO_PKG_VERSION"), "<version>");
            // the JSON forms are what Cedar's translate prints for the text: tests/cedar.rs
            if !at.ends_with(".json") {
                ritsu_testkit::golden(format!("tests/golden/gen/{at}"), &shown);
            }
        }
    }
    // what sekisho writes itself is in the language asked: the head, the doc of the roles, of the
    // workflows, of an input and of a computed value
    for (rel, body) in files("examples/refunds/refunds.ja.gate", Lang::Ja) {
        if !rel.ends_with(".json") {
            let at = rel.trim_start_matches("cedar/").replace("refunds_ja.", "refunds_ja.lang_ja.");
            ritsu_testkit::golden(format!("tests/golden/gen/{at}"), &body.replace(env!("CARGO_PKG_VERSION"), "<version>"));
        }
    }
}

/// The English and the Japanese versions of a gate give their things the same aliases: their
/// Cedar is the same but for the namespace, the head of the ids, `@name` and `@doc`, and so are
/// their vectors but for the namespace and the head of the ids.
#[test]
fn the_two_versions_come_to_the_same_cedar() {
    use ritsu_base::cedar;
    let twins = [
        ("examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate", ("Shop", "ShopJa")),
        ("tests/gen/conditions.gate", "tests/gen/conditions.ja.gate", ("Lab", "LabJa")),
        ("tests/gen/two_spellings.gate", "tests/gen/two_spellings.ja.gate", ("Spell", "SpellJa")),
    ];
    for (en, ja, ns) in twins {
        let (e, j) = (files(en, Lang::En), files(ja, Lang::En));
        let (ea, ja_alias) = (e[0].0.trim_start_matches("cedar/").trim_end_matches(".cedar").to_string(), j[0].0.trim_start_matches("cedar/").trim_end_matches(".cedar").to_string());
        // the policies, read back, without what is said for people, and in the English names
        let strip = |set: &mut cedar::PolicySet, from: &str| {
            for p in &mut set.policies {
                p.annotations.retain(|a| a.key != "doc" && a.key != "name");
                p.id = p.id.replacen(&format!("{from}/"), "", 1);
                for a in &mut p.annotations {
                    a.value = a.value.as_ref().map(|v| v.replacen(&format!("{from}/"), "", 1));
                    (a.line, a.col) = (0, 0);
                }
            }
        };
        let mut pe = cedar::parse_policies(&e[0].1).unwrap();
        let mut pj = cedar::parse_policies(&j[0].1.replace(&format!("{}::", ns.1), &format!("{}::", ns.0))).unwrap();
        strip(&mut pe, &ea);
        strip(&mut pj, &ja_alias);
        let text = |s: &cedar::PolicySet| cedar::policies_to_json(s).compact();
        assert_eq!(text(&pe), text(&pj), "{en} and {ja}");
        // the schemas, without what is said for people
        let strip_schema = |src: &str| -> String {
            let mut s = cedar::parse_schema(src).unwrap();
            for n in &mut s.namespaces {
                n.annotations.clear();
                n.name = Some(cedar::Name::new(ns.0));
                for t in &mut n.entity_types {
                    t.annotations.clear();
                    if let cedar::EntityKind::Standard { shape: cedar::Type::Record(r), .. } = &mut t.kind {
                        r.attrs.iter_mut().for_each(|a| a.annotations.clear());
                    }
                }
                for a in &mut n.actions {
                    a.annotations.retain(|x| x.key == "guards");
                    if let Some(cedar::AppliesTo { context: cedar::Type::Record(r), .. }) = &mut a.applies_to {
                        r.attrs.iter_mut().for_each(|x| x.annotations.clear());
                    }
                }
            }
            cedar::schema_to_json(&s).compact()
        };
        assert_eq!(strip_schema(&e[1].1), strip_schema(&j[1].1), "{en} and {ja}");
        // the vectors
        let vectors = |path: &str| {
            let o = common::check(path);
            let (scope, checked) = (o.scope.as_ref().unwrap(), o.walked.as_ref().unwrap());
            let shape = sekisho::cedar::shape(&checked.gate, scope, checked);
            sekisho::vectors::text(&sekisho::vectors::tests(&checked.gate, &shape, &checked.report, None))
        };
        let ve = vectors(en);
        assert_eq!(vectors(ja).replace(&format!("{}::", ns.1), &format!("{}::", ns.0)).replace(&format!("\"{ja_alias}/"), &format!("\"{ea}/")), ve, "{en} and {ja}");
    }
}

/// An attribute is one term of a relation however a condition writes it, by its name or by its
/// alias, as it is one attribute in Cedar: the check counts the two ways the customer can be the
/// principal or not (not four), times whether the order is rushed.
#[test]
fn an_attribute_written_two_ways_is_one_term() {
    for path in ["tests/gen/two_spellings.gate", "tests/gen/two_spellings.ja.gate"] {
        let o = common::check(path);
        let checked = o.walked.as_ref().unwrap();
        assert_eq!(checked.report.actions[0].combinations, 2 * 2, "{path}");
        // the forbid turns what the permit allows into a deny when the order is rushed
        assert_eq!(checked.report.actions[0].allowed, 1, "{path}");
    }
}

/// A value of a rule's enum is named by the rule's name of it, by the name of the generated code,
/// or by the alias the `.rule` writes (DESIGN 3.2): written `WithinLimit`, the example is checked
/// with the same counts and compiles to the same policies (the string Cedar is given is the alias,
/// `within_limit`).
#[test]
fn a_value_of_a_rule_by_the_name_of_the_generated_code() {
    let t = ritsu_testkit::TempDir::new("sekisho-member");
    ritsu_testkit::tmp::copy_dir(std::path::Path::new("examples/refunds"), t.path());
    let gate = t.path().join("refunds.gate");
    let src = std::fs::read_to_string(&gate).unwrap();
    let changed = src.replace("  when refund_band is within_limit\n", "  when refund_band is WithinLimit\n");
    assert_ne!(changed, src);
    std::fs::write(&gate, changed).unwrap();
    let path = gate.to_string_lossy().to_string();
    let o = common::check(&path);
    assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
    let example = common::check("examples/refunds/refunds.gate");
    assert_eq!(o.walked.as_ref().unwrap().report.policies, example.walked.as_ref().unwrap().report.policies);
    let body = |o: &sekisho::check::Outcome| -> String {
        let files = sekisho::r#gen::files(o, "cedar", "refunds.gate", Lang::En).unwrap();
        files[0].1.lines().skip(2).collect::<Vec<_>>().join("\n")
    };
    assert_eq!(body(&o), body(&example));
}
