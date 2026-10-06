//! Cedar's policies and schemas (DESIGN 4.18). The readers and writers are held to what the
//! official CLI (`cedar-policy-cli` 4.13.0) printed for the files in `tests/fixtures/cedar/`:
//! `expected.sh` beside them writes those outputs, so the test needs neither Cedar nor the
//! network. `upstream/` holds files of the Cedar repository (its formatter's tests and the CLI's
//! samples), under their license.

use ritsu_base::cedar::{self, BinOp, Expr, ExprKind, Name, PatternElem};
use std::path::{Path, PathBuf};

fn dir(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cedar").join(rel)
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The files of a directory whose names end in `ext`, sorted (none when the directory is not
/// there: `upstream/` may be left out).
fn files(rel: &str, ext: &str) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir(rel)) else { return vec![] };
    let mut v: Vec<PathBuf> = rd.map(|e| e.unwrap().path()).filter(|p| p.to_string_lossy().ends_with(ext)).collect();
    v.sort();
    v
}

fn with_ext(p: &Path, ext: &str) -> PathBuf {
    let s = p.to_string_lossy();
    let base = s.rsplit_once('.').map(|(b, _)| b).unwrap_or(&s);
    PathBuf::from(format!("{base}{ext}"))
}

/// Ours, then the ones copied from the Cedar repository.
const ROOTS: [&str; 2] = ["", "upstream/"];

fn policy_files() -> Vec<(&'static str, PathBuf)> {
    ROOTS.iter().flat_map(|r| files(&format!("{r}policies"), ".cedar").into_iter().map(move |f| (*r, f))).collect()
}

#[test]
fn policies_translate_to_the_json_the_cli_prints() {
    let all = policy_files();
    assert!(files("policies", ".cedar").len() >= 14, "{}", all.len());
    for (_, f) in all {
        let set = cedar::parse_policies(&read(&f)).unwrap_or_else(|e| panic!("{}: {e:?}", f.display()));
        let got = cedar::policies_to_json(&set).compact() + "\n";
        assert_eq!(got, read(&with_ext(&f, ".json")), "{}", f.display());
    }
}

#[test]
fn policies_format_as_cedar_format_does() {
    for (_, f) in policy_files() {
        let src = read(&f);
        let got = cedar::format_policies(&src, 80, 2).unwrap_or_else(|e| panic!("{}: {e:?}", f.display()));
        assert_eq!(got, read(&with_ext(&f, ".fmt")), "{}", f.display());
        let got = cedar::format_policies(&src, 40, 4).unwrap();
        assert_eq!(got, read(&with_ext(&f, ".fmt40")), "{} (40 columns, indented by 4)", f.display());
        // what the formatter writes, it leaves as it is
        assert_eq!(cedar::format_policies(&got, 40, 4).unwrap(), got, "{}", f.display());
    }
}

/// The writer's output for each policy file: `<root>written/<name>.cedar`, which `expected.sh`
/// checks with the CLI (already formatted, and the same JSON as the policy it was written from).
/// `CEDAR_BLESS=1` writes them anew.
#[test]
fn written_policies_are_formatted_and_read_back_the_same() {
    let bless = std::env::var("CEDAR_BLESS").is_ok_and(|v| v == "1");
    for (root, f) in policy_files() {
        let set = cedar::parse_policies(&read(&f)).unwrap();
        let written = cedar::write_policies(&set).unwrap_or_else(|e| panic!("{}: {e:?}", f.display()));
        assert_eq!(cedar::format_policies(&written, 80, 2).unwrap(), written, "{}", f.display());
        let again = cedar::parse_policies(&written).unwrap_or_else(|e| panic!("{}: {e:?}\n{written}", f.display()));
        assert_eq!(cedar::policies_to_json(&again).compact() + "\n", read(&with_ext(&f, ".json")), "{}", f.display());
        let written_dir = dir(&format!("{root}written"));
        let path = written_dir.join(f.file_name().unwrap());
        if bless {
            std::fs::create_dir_all(&written_dir).unwrap();
            std::fs::write(&path, &written).unwrap();
        } else {
            assert_eq!(written, read(&path), "{}", path.display());
        }
    }
}

#[test]
fn what_the_cli_does_not_read_is_not_read() {
    let all = files("invalid", ".cedar");
    assert!(all.len() >= 80, "{}", all.len());
    for f in all {
        let src = read(&f);
        let e = match cedar::parse_policies(&src) {
            Ok(_) => panic!("{} reads", f.display()),
            Err(e) => e,
        };
        assert!(e.line >= 1 && e.col >= 1, "{}: {e:?}", f.display());
        assert!(!e.message.en.is_empty() && !e.message.ja.is_empty(), "{}", f.display());
        assert!(cedar::format_policies(&src, 80, 2).is_err(), "{} formats", f.display());
    }
}

fn is_input_schema(p: &Path) -> bool {
    let s = p.to_string_lossy();
    !s.ends_with(".back.cedarschema") && !s.ends_with(".schema.cedarschema")
}

#[test]
fn schemas_translate_as_the_cli_translates_them() {
    let mut n = 0;
    for root in ROOTS {
        let rel = format!("{root}schemas");
        for f in files(&rel, ".cedarschema").into_iter().filter(|p| is_input_schema(p)) {
            n += 1;
            let s = cedar::parse_schema(&read(&f)).unwrap_or_else(|e| panic!("{}: {e:?}", f.display()));
            let json = read(&with_ext(&f, ".json"));
            assert_eq!(cedar::schema_to_json(&s).compact() + "\n", json, "{} to JSON", f.display());
            let back = read(&with_ext(&f, ".back.cedarschema"));
            assert_eq!(cedar::write_schema(&s).unwrap() + "\n", back, "{} written", f.display());
            // the JSON the CLI wrote reads as the same schema
            let from_json = cedar::parse_schema_json(&json).unwrap_or_else(|e| panic!("{}: {e:?}", f.display()));
            assert_eq!(cedar::schema_to_json(&from_json).compact() + "\n", json, "{} JSON again", f.display());
            assert_eq!(cedar::write_schema(&from_json).unwrap() + "\n", back, "{} from JSON, written", f.display());
        }
        for f in files(&rel, ".schema.json") {
            n += 1;
            let s = cedar::parse_schema_json(&read(&f)).unwrap_or_else(|e| panic!("{}: {e:?}", f.display()));
            let text = cedar::write_schema(&s).unwrap() + "\n";
            assert_eq!(text, read(&with_ext(&f, ".cedarschema")), "{} written", f.display());
            let again = cedar::parse_schema(&text).unwrap_or_else(|e| panic!("{}: {e:?}", f.display()));
            assert_eq!(cedar::schema_to_json(&again).compact() + "\n", read(&with_ext(&f, ".back.json")), "{} back to JSON", f.display());
        }
    }
    assert!(n >= 5, "{n}");
}

#[test]
fn what_the_cli_does_not_read_as_a_schema_is_not_read() {
    let cedar_files = files("invalid-schemas", ".cedarschema");
    let json_files = files("invalid-schemas", ".json");
    assert!(cedar_files.len() >= 40 && json_files.len() >= 30, "{} {}", cedar_files.len(), json_files.len());
    for f in cedar_files {
        match cedar::parse_schema(&read(&f)) {
            Ok(_) => panic!("{} reads", f.display()),
            Err(e) => assert!(e.line >= 1 && e.col >= 1 && !e.message.ja.is_empty() && !e.message.en.is_empty(), "{}: {e:?}", f.display()),
        }
    }
    // `json-to-cedar` stops either reading the JSON or writing what the Cedar format cannot say
    for f in json_files {
        let r = cedar::parse_schema_json(&read(&f)).and_then(|s| cedar::write_schema(&s));
        match r {
            Ok(_) => panic!("{} reads", f.display()),
            Err(e) => assert!(e.line >= 1 && e.col >= 1 && !e.message.ja.is_empty() && !e.message.en.is_empty(), "{}: {e:?}", f.display()),
        }
    }
}

#[test]
fn ids_are_the_ones_the_cli_gives() {
    let set = cedar::parse_policies("permit(principal, action, resource);\n@id(\"named\") forbid(principal, action, resource);\npermit(principal == ?principal, action, resource);\n").unwrap();
    let ids: Vec<&str> = set.policies.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec!["policy0", "named", "policy2"]);
    assert!(set.policies[2].is_template() && !set.policies[0].is_template());
    let json = cedar::policies_to_json(&set);
    let keys = |k: &str| json.get(k).unwrap().as_obj().unwrap().iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
    assert_eq!(keys("templates"), vec!["policy2"]);
    assert_eq!(keys("staticPolicies"), vec!["policy0", "named"]);
}

#[test]
fn every_value_knows_where_it_was_written() {
    let src = read(&dir("policies/photos.cedar"));
    let set = cedar::parse_policies(&src).unwrap();
    let p = &set.policies[0];
    assert_eq!((p.line, p.col, p.id.as_str()), (2, 1, "owners-edit"));
    assert_eq!(p.annotations.iter().map(|a| (a.key.as_str(), a.line, a.col)).collect::<Vec<_>>(), vec![("id", 2, 1), ("description", 3, 1)]);
    let c = &p.conditions[0];
    assert_eq!((c.line, c.col), (5, 1));
    // `resource.owner == principal`: the comparison, then its two sides
    let ExprKind::Binary { op: BinOp::Eq, left, right } = &c.body.kind else { panic!("{:?}", c.body) };
    assert_eq!((c.body.line, c.body.col), (5, 8));
    assert_eq!((left.line, left.col, right.line, right.col), (5, 8, 5, 26));
    let cedar::ActionScope::InList(actions) = &p.action else { panic!("{:?}", p.action) };
    assert_eq!(actions.iter().map(|a| (a.id.as_str(), a.line, a.col)).collect::<Vec<_>>(), vec![("edit", 4, 31), ("delete", 4, 57)]);
    // columns count characters, not bytes
    let ja = cedar::parse_policies(&read(&dir("policies/ja.cedar"))).unwrap();
    let ExprKind::Binary { right, .. } = &ja.policies[0].conditions[0].body.kind else { panic!() };
    let ExprKind::Binary { left: place, .. } = &right.kind else { panic!("{right:?}") };
    assert_eq!((place.line, place.col), (5, 40));
    // schemas too, in both formats
    let s = cedar::parse_schema(&read(&dir("schemas/photos.cedarschema"))).unwrap();
    let ns = &s.namespaces[0];
    assert_eq!((ns.name.as_ref().unwrap().to_string(), ns.line, ns.col), ("PhotoApp".to_string(), 3, 11));
    let user = ns.entity_types.iter().find(|e| e.name == "User").unwrap();
    assert_eq!((user.line, user.col), (5, 10));
    let cedar::EntityKind::Standard { shape: cedar::Type::Record(r), .. } = &user.kind else { panic!() };
    assert_eq!(r.attrs.iter().map(|a| (a.name.as_str(), a.line)).collect::<Vec<_>>(), vec![("name", 6), ("age", 7), ("address", 9), ("tags", 10), ("profile", 11)]);
    let j = cedar::parse_schema_json(&read(&dir("schemas/json-types.schema.json"))).unwrap();
    let color = j.namespaces[0].entity_types.iter().find(|e| e.name == "Color").unwrap();
    assert_eq!((color.line, color.col), (24, 7));
}

#[test]
fn errors_say_where_and_what_in_both_languages() {
    let e = cedar::parse_policies("permit(principal, action, resource)\nwhen { 4 / 2 == 2 };").unwrap_err();
    assert_eq!((e.line, e.col), (2, 8));
    assert_eq!(e.message.en, "division (`/`) is not supported");
    assert_eq!(e.message.ja, "Cedar に割り算（`/`）はありません");
    let e = cedar::parse_policies("permit(principal, action, resource) when { context.名前 };").unwrap_err();
    assert_eq!((e.line, e.col), (1, 52));
    assert_eq!(e.message.en, "`名` is not a token of Cedar");
    let e = cedar::parse_schema("entity E;\nentity E;").unwrap_err();
    assert_eq!((e.line, e.col, e.message.en.as_str(), e.message.ja.as_str()), (2, 8, "`E` is declared twice", "`E` が二度宣言されています"));
    let e = cedar::parse_schema_json("{\"\": {\"entityTypes\": {}}}").unwrap_err();
    assert_eq!((e.line, e.col), (1, 6));
    assert_eq!(e.message.en, "a namespace has no `actions`");
}

fn e(kind: ExprKind) -> Box<Expr> {
    Box::new(Expr::new(kind))
}

fn long(n: i64) -> Box<Expr> {
    e(ExprKind::Long(n))
}

fn ctx(attr: &str) -> Box<Expr> {
    e(ExprKind::GetAttr { expr: e(ExprKind::Var(cedar::Var::Context)), attr: attr.into() })
}

/// What sekisho does: build the tree in code and write it. The text reads back as the same
/// policy (the same JSON), and it is what `cedar format` makes of it.
#[test]
fn policies_made_in_code_are_written_as_cedar_reads_them() {
    let not = |x: Box<Expr>| e(ExprKind::Not(x));
    let neg = |x: Box<Expr>| e(ExprKind::Neg(x));
    let bin = |op, l, r| e(ExprKind::Binary { op, left: l, right: r });
    let conditions = vec![
        // a minus before a literal, four minus signs before a negative one, five `!`
        bin(BinOp::Eq, neg(long(5)), neg(neg(neg(neg(long(-5)))))),
        not(not(not(not(not(e(ExprKind::Bool(true))))))),
        // the grammar's levels: `(a || b) && c`, `a - (b - c)`, `(a + b) * c`, `-(x.y)`
        bin(BinOp::And, bin(BinOp::Or, ctx("a"), ctx("b")), ctx("c")),
        bin(BinOp::Eq, bin(BinOp::Sub, long(1), bin(BinOp::Sub, long(2), long(-3))), bin(BinOp::Mul, bin(BinOp::Add, long(1), long(2)), neg(ctx("y")))),
        // names that cannot stand bare
        bin(BinOp::Eq, e(ExprKind::GetAttr { expr: ctx("if"), attr: "two words".into() }), e(ExprKind::Str("tab\t\"quote\" \\ 改行\n".into()))),
        e(ExprKind::Has { expr: e(ExprKind::Var(cedar::Var::Principal)), attrs: vec!["has".into()] }),
        e(ExprKind::Has { expr: e(ExprKind::Var(cedar::Var::Principal)), attrs: vec!["a".into(), "b".into()] }),
        bin(BinOp::Eq, e(ExprKind::Record(vec![("then".into(), *long(1)), ("ok".into(), *e(ExprKind::Set(vec![])))])), e(ExprKind::Record(vec![]))),
        // a pattern with a star that is a star
        e(ExprKind::Like { expr: ctx("path"), pattern: vec![PatternElem::Char('*'), PatternElem::Wildcard, PatternElem::Char('x')] }),
        // `if` inside an operator, and a negative number as a receiver
        bin(BinOp::Less, e(ExprKind::If { cond: e(ExprKind::Bool(true)), then: long(1), els: long(2) }), long(3)),
        e(ExprKind::Method { expr: e(ExprKind::Set(vec![*long(-1)])), name: "contains".into(), args: vec![*long(-1)] }),
        e(ExprKind::Method { expr: e(ExprKind::Call { func: Name::new("decimal"), args: vec![*e(ExprKind::Str("1.5".into()))] }), name: "lessThan".into(), args: vec![*e(ExprKind::Call { func: Name::new("decimal"), args: vec![*e(ExprKind::Str("2".into()))] })] }),
        e(ExprKind::Is { expr: e(ExprKind::Var(cedar::Var::Resource)), ty: Name::parse("App::Doc"), in_expr: Some(e(ExprKind::Entity(cedar::EntityUid::new(Name::parse("App::Folder"), "f")))) }),
    ];
    let policy = cedar::Policy {
        id: "made".into(),
        annotations: vec![cedar::Annotation { key: "id".into(), value: Some("made".into()), line: 0, col: 0 }, cedar::Annotation { key: "note".into(), value: Some("built in code: \"quoted\"".into()), line: 0, col: 0 }],
        effect: cedar::Effect::Permit,
        principal: cedar::Scope::IsIn(Name::parse("App::User"), cedar::EntityOrSlot::Entity(cedar::EntityUid::new(Name::parse("App::Group"), "staff"))),
        action: cedar::ActionScope::InList(vec![cedar::EntityUid::new(Name::parse("App::Action"), "read"), cedar::EntityUid::new(Name::parse("App::Action"), "list")]),
        resource: cedar::Scope::Any,
        conditions: conditions.into_iter().map(|b| cedar::Condition { kind: cedar::CondKind::When, body: *b, line: 0, col: 0 }).collect(),
        line: 0,
        col: 0,
    };
    let written = cedar::write_policy(&policy).unwrap();
    assert_eq!(cedar::format_policies(&written, 80, 2).unwrap(), written);
    let back = cedar::parse_policies(&written).unwrap_or_else(|err| panic!("{err:?}\n{written}"));
    assert_eq!(cedar::policy_to_json(&back.policies[0]).compact(), cedar::policy_to_json(&policy).compact(), "\n{written}");
    assert!(written.contains("-(5) == ----(-5)"), "{written}");
    assert!(written.contains("!!!!(!true)"), "{written}");
    assert!(written.contains("context[\"if\"][\"two words\"]"), "{written}");
    assert!(written.contains("principal has \"has\""), "{written}");
    assert!(written.contains("like \"\\**x\""), "{written}");
    // one expression on one line
    assert_eq!(cedar::write_expr(&Expr::new(ExprKind::Binary { op: BinOp::Mul, left: e(ExprKind::Binary { op: BinOp::Add, left: long(1), right: long(2) }), right: long(3) })), "(1 + 2) * 3");
    // a tree Cedar would not read is an error, not a text Cedar stops at
    let mut bad = policy.clone();
    bad.action = cedar::ActionScope::Eq(cedar::EntityUid::new(Name::new("User"), "read"));
    assert!(cedar::write_policy(&bad).is_err());
}
