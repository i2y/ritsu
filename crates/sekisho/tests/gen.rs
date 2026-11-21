//! What `gen --target cedar` writes (DESIGN 5), with rulec, koyomi, chobo and dandori joined as
//! `ritsu sekisho` joins them: the policies and the schema of the example and of the gates of
//! `tests/gen/` — every form of condition (`conditions.gate` and its Japanese version), a forbid
//! read with `use gate` (`papers.gate`, which reads `people.gate`), an action that guards two
//! operations (`guards.gate`), one attribute written by its name and by its alias
//! (`two_spellings.gate` and its Japanese version), the units written with a sign, a temperature
//! in ℃ compared with constants in ℉ and given to a rule (`cold_chain.gate` and its Japanese
//! version) — as goldens, ritsu's version written
//! `<version>`; and the two versions of a gate, which come to the same Cedar and the same vectors
//! but for their names.
//!
//! Whether Cedar itself reads what is written, and decides as the vectors say, is
//! `tests/cedar.rs`. The goldens are `tests/golden/gen/`; `SEKISHO_BLESS=1` (or `RITSU_BLESS=1`)
//! writes them again.
//!
//! Of the code of the three languages, the entry that asks the gate by the name of an action
//! (DESIGN 30) is held to goldens too (`tests/golden/gen/by-name/`): the example's (an action that
//! takes inputs of its own among them), the same asking Amazon Verified Permissions, its Japanese
//! version with `--lang ja`, and `tests/gen/asked.gate`'s, whose action of two resource types
//! needs the helper that picks the one given. That each answers as the action's own function does
//! is the runners' (`tests/typescript.rs`, `tests/python.rs`, `tests/go.rs`). So are the lists of
//! the roles (DESIGN 33), held here to the gate's roles in each language and with each authorizer;
//! the runners call them, and narrow the roles a sign-in carries with the second.
//!
//! The comments beside the fields of the records the three languages write say a type by the name
//! the gate gives it, whatever the shape of the name (DESIGN 35): `tests/gen/approvals.gate`, whose
//! types end in s or begin with a vowel, and its Japanese version.

mod common;

use ritsu_base::text::Lang;

const GATES: [&str; 11] = [
    "examples/refunds/refunds.gate",
    "examples/refunds/refunds.ja.gate",
    "tests/gen/conditions.gate",
    "tests/gen/conditions.ja.gate",
    "tests/gen/people.gate",
    "tests/gen/papers.gate",
    "tests/gen/guards.gate",
    "tests/gen/two_spellings.gate",
    "tests/gen/two_spellings.ja.gate",
    "tests/gen/cold_chain.gate",
    "tests/gen/cold_chain.ja.gate",
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
        ("tests/gen/cold_chain.gate", "tests/gen/cold_chain.ja.gate", ("Depot", "DepotJa")),
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

/// What a gate reads is named from the root (DESIGN 2.6), in the `@doc` of the generated Cedar as
/// in `@guards`: the example with its gates moved down into `gates/`, which then read `../rules/…`,
/// names `rules/…` from the root above, as the example does. With `gates/` as the root, every
/// file the gate reads is outside it: E201 at each `use` line, E208 at the workflow.
#[test]
fn what_a_gate_reads_is_named_from_the_root() {
    let t = ritsu_testkit::TempDir::new("sekisho-root");
    ritsu_testkit::tmp::copy_dir(std::path::Path::new("examples/refunds"), t.path());
    for gate in ["refunds.gate", "refunds.ja.gate"] {
        let src = std::fs::read_to_string(t.path().join(gate)).unwrap();
        let moved = t.write(&format!("gates/{gate}"), src.replace(" from \"", " from \"../"));
        let path = moved.to_string_lossy().to_string();
        let check = |root: &std::path::Path| sekisho::check::check_file(&path, &common::joined(), &sekisho::check::Options { root: Some(root.to_path_buf()), ..Default::default() }).unwrap();
        // the directory above as the root: the same references as the example's
        let o = check(t.path());
        assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
        let schema = |o: &sekisho::check::Outcome, lang: Lang| -> String {
            let files = sekisho::r#gen::files(o, "cedar", gate, lang).unwrap();
            files.into_iter().find(|(rel, _)| rel.ends_with(".cedarschema")).unwrap().1.lines().skip(2).collect::<Vec<_>>().join("\n")
        };
        let example = common::check(&format!("examples/refunds/{gate}"));
        for lang in [Lang::En, Lang::Ja] {
            assert_eq!(schema(&o, lang), schema(&example, lang), "{gate}");
        }
        let doc = schema(&o, Lang::En);
        for named in ["dandori \\\"flows/returns.flow\\\"", "koyomi \\\"calendars/england_and_wales.cal\\\"", "rulec \\\"rules/", "koyomi \\\"dates/", "@guards(\"openapi \\\"api/orders.json\\\" operation refundOrder\")"] {
            assert!(doc.contains(named), "{gate}: {named}\n{doc}");
        }
        assert!(!doc.contains("../"), "{doc}");
        // `gates/` as the root: what the gate reads is outside it
        let o = check(&t.path().join("gates"));
        let said: Vec<(&str, Option<usize>)> = o.diags.iter().map(|d| (d.code, d.line)).collect();
        assert_eq!(said, [("E201", Some(5)), ("E201", Some(6)), ("E201", Some(7)), ("E201", Some(8)), ("E208", Some(39))], "{gate}: {}", common::shown(&o, Lang::En));
        let shown = [Lang::En, Lang::Ja].map(|l| common::shown(&o, l).replace(&format!("{}/", t.path().display()), "")).concat();
        assert!(shown.contains(": `../flows/returns.flow` is outside the root\n"), "{shown}");
        assert!(shown.contains(": `../flows/returns.flow` はルートの外にあります\n"), "{shown}");
        assert!(shown.contains("in the `@doc` of the generated Cedar and by the other languages. The root is the nearest directory above"), "{shown}");
        assert!(shown.contains("生成する Cedar の `@doc` でも、ほかの言語からも、ルートからのパスで参照します。ルートは、"), "{shown}");
    }
}

/// The `resource` lines under a workflow (DESIGN 2.12) are for the checks, and nothing `gen`
/// writes holds them: Cedar has no word for what a flow holds, and the code that asks Cedar reads
/// the resource from the entities it is given. The example with the lines taken out comes to the
/// same files for every target, but for the digest of the source in their heads.
#[test]
fn the_lines_under_a_workflow_change_nothing_gen_writes() {
    let src = std::fs::read_to_string("examples/claims/claims.gate").unwrap();
    let lines = "  resource Claim from claim_id\n    amount = amount\n";
    assert!(src.contains(lines), "{src}");
    let t = ritsu_testkit::TempDir::new("gen-holds");
    let copy = |from: &std::path::Path, to: &std::path::Path| {
        for e in walk(from) {
            let into = to.join(e.strip_prefix(from).unwrap());
            std::fs::create_dir_all(into.parent().unwrap()).unwrap();
            std::fs::copy(&e, &into).unwrap();
        }
    };
    let made = |dir: &str, body: &str| -> Vec<(String, String, String)> {
        let root = t.path().join(dir);
        copy(std::path::Path::new("examples/claims"), &root);
        let path = root.join("claims.gate");
        std::fs::write(&path, body).unwrap();
        let o = sekisho::check::check_file(path.to_str().unwrap(), &common::joined(), &sekisho::check::Options { root: Some(root.clone()), ..Default::default() }).unwrap();
        assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
        let mut out = Vec::new();
        for target in ["cedar", "typescript", "python", "go"] {
            let to = sekisho::r#gen::Target { name: target, authorizer: sekisho::r#gen::Authorizer::Cedar, go_module: "example.com/claims".to_string(), place: ritsu_emit::package::Place::here() };
            for (rel, body) in sekisho::r#gen::generate(&o, &to, "claims.gate", Lang::En, &common::joined()).unwrap() {
                // the digest of the source, in the head of each file and in the names the code
                // gives the policies it was made from
                let digest = body.split("sha256:").nth(1).map(|x| x.chars().take_while(|c| c.is_ascii_hexdigit()).collect::<String>()).unwrap_or_default();
                let shown = if digest.is_empty() { body.clone() } else { body.replace(&digest, "<digest>") };
                out.push((target.to_string(), rel, shown));
            }
        }
        out
    };
    let (with, without) = (made("with", &src), made("without", &src.replace(lines, "")));
    assert!(with.len() >= 4 + 3, "{}", with.len());
    assert_eq!(with.len(), without.len());
    for (a, b) in with.iter().zip(&without) {
        assert_eq!((&a.0, &a.1), (&b.0, &b.1));
        assert!(a.2 == b.2, "{} {} differs with the lines:\n{}", a.0, a.1, ritsu_testkit::golden::line_diff(&b.2, &a.2));
        assert!(!a.2.contains("claim_id"), "{} {} names the flow's value", a.0, a.1);
    }
}

/// Every file under a directory.
fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

/// The entry by name of the code `text` of `language`, with the comment above it, and the helper
/// for an action of two resource types where the code has one.
fn entry_of(text: &str, language: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let (heads, comment): (&[&str], fn(&str) -> bool) = match language {
        "go" => (&["func Authorize(", "func given("], |l| l.starts_with("//")),
        "typescript" => (&["export async function authorize(", "function _given("], |l| l.starts_with("/**") || l.starts_with(" *")),
        _ => (&["def authorize(", "def _given("], |_| false),
    };
    let mut out = String::new();
    for head in heads {
        let Some(at) = lines.iter().position(|l| l.starts_with(head)) else { continue };
        let mut from = at;
        while from > 0 && comment(lines[from - 1]) {
            from -= 1;
        }
        let mut to = at + 1;
        if language == "python" {
            // to the next line of the module's own level
            while to < lines.len() && (lines[to].is_empty() || lines[to].starts_with(' ')) {
                to += 1;
            }
            while lines[to - 1].is_empty() {
                to -= 1;
            }
        } else {
            while to < lines.len() && lines[to] != "}" {
                to += 1;
            }
            to += 1;
        }
        out.push_str(&lines[from..to].join("\n"));
        out.push_str("\n\n");
    }
    out
}

/// The lists of the roles (DESIGN 33), as each language writes them: the roles the gate declares,
/// in its order, and the roles of each type of principal, by the type's name in Cedar, those of its
/// `roles` line in the order the gate declares them; empty lists for a gate with no role. Each with
/// both authorizers, the Japanese version of the example with `--lang ja`, and a gate whose two types
/// of principal hold roles of their own (`tests/gen/roles_of.gate`).
#[test]
fn the_lists_of_the_roles_of_each_language() {
    use sekisho::r#gen::{Authorizer, Target};
    let suite = common::joined();
    let example: (&[&str], &[(&str, &[&str])]) = (&["clerk", "manager", "auditor"], &[("User", &["clerk", "manager", "auditor"])]);
    let staff: (&[&str], &[(&str, &[&str])]) = (&["clerk", "manager", "auditor"], &[("Employee", &["clerk", "manager"]), ("Inspector", &["auditor"])]);
    let none: (&[&str], &[(&str, &[&str])]) = (&[], &[]);
    for (path, lang, (roles, of)) in [
        ("examples/refunds/refunds.gate", Lang::En, example),
        ("examples/refunds/refunds.ja.gate", Lang::Ja, example),
        ("tests/gen/roles_of.gate", Lang::En, staff),
        ("tests/gen/roles_of.ja.gate", Lang::Ja, staff),
        ("tests/gen/guards.gate", Lang::En, none),
    ] {
        let o = common::check(path);
        assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
        let quoted = |rs: &[&str], q: char| rs.iter().map(|r| format!("{q}{r}{q}")).collect::<Vec<_>>().join(", ");
        // each language's two functions, as its lines of code (Python's docstrings left out)
        let ts = {
            let mut l = vec!["export function roles(): string[] {".to_string(), format!("  return [{}];", quoted(roles, '"')), "}".to_string(), "export function rolesOf(t: string): string[] {".to_string()];
            if !of.is_empty() {
                l.push("  switch (t) {".to_string());
                for (ty, rs) in of {
                    l.extend([format!("    case \"{ty}\":"), format!("      return [{}];", quoted(rs, '"'))]);
                }
                l.push("  }".to_string());
            }
            l.extend(["  return [];".to_string(), "}".to_string()]);
            l
        };
        let py = {
            let mut l = vec!["def roles() -> list[str]:".to_string(), format!("    return [{}]", quoted(roles, '\'')), "def roles_of(t: str) -> list[str]:".to_string()];
            for (ty, rs) in of {
                l.extend([format!("    if t == '{ty}':"), format!("        return [{}]", quoted(rs, '\''))]);
            }
            l.push("    return []".to_string());
            l
        };
        let go = {
            let mut l = vec!["func Roles() []string {".to_string(), format!("\treturn []string{{{}}}", quoted(roles, '"')), "}".to_string(), "func RolesOf(t string) []string {".to_string()];
            if !of.is_empty() {
                l.push("\tswitch t {".to_string());
                for (ty, rs) in of {
                    l.extend([format!("\tcase \"{ty}\":"), format!("\t\treturn []string{{{}}}", quoted(rs, '"'))]);
                }
                l.push("\t}".to_string());
            }
            l.extend(["\treturn []string{}".to_string(), "}".to_string()]);
            l
        };
        for (language, want) in [("typescript", ts), ("python", py), ("go", go)] {
            for authorizer in [Authorizer::Cedar, Authorizer::Avp] {
                let target = Target { name: language, authorizer, go_module: "generated".to_string(), place: ritsu_emit::package::Place::here() };
                let made = sekisho::r#gen::generate(&o, &target, &ritsu_emit::header::file_name(path), lang, &suite).unwrap();
                let (_, text) = made.iter().find(|(rel, _)| rel.contains("authz/") && !rel.ends_with("__init__.py") && !rel.ends_with("go.mod")).unwrap();
                let lines: Vec<&str> = text.lines().collect();
                let at = lines.iter().position(|l| *l == want[0]).unwrap_or_else(|| panic!("{path} {language}: no {}", want[0]));
                assert_eq!(lines.iter().filter(|l| **l == want[0]).count(), 1, "{path} {language}");
                // from the first function to the end of the second, leaving out the docs and the blank lines
                let got: Vec<&str> = lines[at..]
                    .iter()
                    .copied()
                    .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with("/**") && !l.starts_with(" *") && !l.trim_start().starts_with("\"\"\""))
                    .take(want.len())
                    .collect();
                assert_eq!(got, want, "{path} {language} {authorizer:?}");
            }
        }
    }
}

#[test]
fn the_entry_by_name_of_each_language() {
    use sekisho::r#gen::{Authorizer, Target};
    let suite = common::joined();
    for (path, authorizer, lang, name) in [
        ("examples/refunds/refunds.gate", Authorizer::Cedar, Lang::En, "refunds"),
        ("examples/refunds/refunds.gate", Authorizer::Avp, Lang::En, "refunds.avp"),
        ("examples/refunds/refunds.ja.gate", Authorizer::Cedar, Lang::Ja, "refunds_ja.lang_ja"),
        ("tests/gen/asked.gate", Authorizer::Cedar, Lang::En, "asked"),
    ] {
        let o = common::check(path);
        assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
        for (language, ext) in [("typescript", "ts"), ("python", "py"), ("go", "go")] {
            let target = Target { name: language, authorizer, go_module: "generated".to_string(), place: ritsu_emit::package::Place::here() };
            let made = sekisho::r#gen::generate(&o, &target, &ritsu_emit::header::file_name(path), lang, &suite).unwrap();
            let authz: Vec<&(String, String)> = made.iter().filter(|(rel, _)| rel.contains("authz/") && !rel.ends_with("__init__.py") && !rel.ends_with("go.mod")).collect();
            assert_eq!(authz.len(), 1, "{path} {language}: {:?}", made.iter().map(|(r, _)| r).collect::<Vec<_>>());
            let entry = entry_of(&authz[0].1, language);
            assert!(entry.contains("uthorize("), "{path} {language}: no entry");
            ritsu_testkit::golden(format!("tests/golden/gen/by-name/{name}.{ext}"), &entry);
        }
    }
}

#[test]
fn the_comments_of_the_records_say_a_type_by_its_name_as_it_is() {
    use sekisho::r#gen::{Authorizer, Target};
    let suite = common::joined();
    // a name ending in s is not made plural again, and none is given an article that may not fit it
    let en: [(&str, &[&str]); 3] = [
        ("go", &["// the ids of the groups of type Approvers it is a member of", "// the ids of the groups of type Owners it is a member of", "the id of an entity of type Approvers", "the id of an entity of type Editor"]),
        ("python", &["# the ids of the groups of type Approvers it is a member of", "# the ids of the groups of type Owners it is a member of", "the id of an entity of type Approvers", "the id of an entity of type Editor"]),
        ("typescript", &["/** The groups it is a member of */", "The id of an entity of type Approvers", "The id of an entity of type Editor"]),
    ];
    let ja: [(&str, &[&str]); 3] = [
        ("go", &["// メンバーである Approvers の ID", "// メンバーである Owners の ID", "Approvers の ID", "Editor の ID"]),
        ("python", &["# メンバーである Approvers の ID", "# メンバーである Owners の ID", "Approvers の ID", "Editor の ID"]),
        ("typescript", &["/** メンバーであるグループ */", "Approvers の ID", "Editor の ID"]),
    ];
    for (path, lang, wanted) in [
        ("tests/gen/approvals.gate", Lang::En, &en),
        ("tests/gen/approvals.ja.gate", Lang::En, &en),
        ("tests/gen/approvals.gate", Lang::Ja, &ja),
        ("tests/gen/approvals.ja.gate", Lang::Ja, &ja),
    ] {
        let o = common::check(path);
        assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
        for (language, lines) in wanted.iter() {
            let target = Target { name: language, authorizer: Authorizer::Cedar, go_module: "generated".to_string(), place: ritsu_emit::package::Place::here() };
            let made = sekisho::r#gen::generate(&o, &target, &ritsu_emit::header::file_name(path), lang, &suite).unwrap();
            let text: Vec<&str> = made.iter().filter(|(rel, _)| rel.contains("authz/")).map(|(_, t)| t.as_str()).collect();
            let text = text.join("\n");
            for l in lines.iter() {
                assert!(text.contains(l), "{path}, {language}, {}: no `{l}` in\n{text}", lang.code());
            }
            for wrong in ["Approverss", "Ownerss", "a Approvers", "a Owners", "a Editor"] {
                assert!(!text.contains(wrong), "{path}, {language}, {}: `{wrong}`", lang.code());
            }
        }
    }
}
