//! A project read as one (ritsu's DESIGN 6, PLAN E.1): the files of the test project (a copy of
//! sakai's example, `crates/ritsu/tests/projects/通販`) found and sorted by language, the
//! languages joined once, and every reference between its files resolved through the index.

use ritsu_base::naming::Tool;
use ritsu_ports::Lookup;
use ritsu_project::{Joined, Landing, Project};
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn shop() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ritsu/tests/projects/通販")
}

fn load(root: &Path) -> Project {
    let r = root.to_string_lossy().to_string();
    Project::load(std::slice::from_ref(&r), Some(&r)).unwrap()
}

/// Every file under the paths, each with its language, in the order the languages are checked:
/// the ones that give facts first, then dandori, then yuen and sakai.
#[test]
fn the_files_of_a_project_and_their_languages() {
    let p = load(&shop());
    let counts: Vec<(&str, usize)> = p.counts().into_iter().map(|(t, n)| (t.word(), n)).collect();
    assert_eq!(counts, [("rulec", 4), ("koyomi", 3), ("chobo", 1), ("proto", 5), ("dandori", 2), ("sakai", 6)]);
    let rels: Vec<&str> = p.files.iter().map(|f| f.rel.as_str()).collect();
    assert_eq!(rels[..4], ["billing/rules/出荷の送料.rule", "billing/rules/決済手数料.rule", "billing/rules/請求の要否.rule", "delivery/rules/出荷の急ぎ.rule"]);
    assert_eq!(rels.last(), Some(&"通販.ctx"));
    // the code under `py/`, `ts/`, `java/` and `go/`, the holidays' table and the tools' settings
    // are no file of a language: sakai reads them through its map
    assert!(p.files.iter().all(|f| !f.rel.ends_with(".py") && !f.rel.ends_with(".csv")));
    assert_eq!(p.root_shown(), p.shown.path("."));
    assert_eq!(p.given_for(Tool::Sakai), [shop().to_string_lossy().to_string()]);
    assert!(p.given_for(Tool::Yuen).is_empty());
}

/// What the person has to correct before anything is read: a path that is not there, one
/// outside the root, a file of no language named by itself, a project with no file of any.
#[test]
fn what_stops_a_project_before_it_is_read() {
    let root = shop().to_string_lossy().to_string();
    let missing = format!("{root}/無い.rule");
    let e = Project::load(std::slice::from_ref(&missing), Some(&root)).unwrap_err();
    assert!(e.en.ends_with("無い.rule` is not there"), "{}", e.en);
    let outside = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml").to_string_lossy().to_string();
    let e = Project::load(std::slice::from_ref(&outside), Some(&root)).unwrap_err();
    assert!(e.en.contains("is outside the root"), "{}", e.en);
    let py = format!("{root}/py/ordering/fulfill.py");
    let e = Project::load(std::slice::from_ref(&py), Some(&root)).unwrap_err();
    assert!(e.en.contains("is a file of none of ritsu's languages"), "{}", e.en);
    assert!(e.ja.contains("は ritsu のどの言語のファイルでもありません"), "{}", e.ja);
    let code = format!("{root}/py");
    let e = Project::load(std::slice::from_ref(&code), Some(&root)).unwrap_err();
    assert!(e.en.starts_with("there is no file of ritsu's languages"), "{}", e.en);
}

/// The languages are made once: the rules dandori reads, and the index yuen and sakai look their
/// namings up in, are the same ones.
#[test]
fn the_languages_are_joined_once() {
    let j = Joined::new();
    let (y, s) = (j.yuen(), j.sakai());
    assert!(Rc::ptr_eq(&y.index, &j.index) && Rc::ptr_eq(&s.index, &j.index));
    for t in [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Geas, Tool::Dandori, Tool::Sakai] {
        assert!(j.index.reads_items(t), "{t:?}");
        assert!(y.reads(t), "{t:?}");
    }
    for t in [Tool::Rulec, Tool::Koyomi, Tool::Dandori] {
        assert!(s.reads(t), "{t:?}");
    }
}

fn landing(l: &Landing) -> String {
    match l {
        Landing::Missing => "no file".into(),
        Landing::File => "a file".into(),
        Landing::Proto(Ok(true)) => "the .proto holds it".into(),
        Landing::Proto(Ok(false)) => "the .proto does not hold it".into(),
        Landing::Proto(Err(t)) => format!("the .proto does not read: {}", t.en),
        Landing::Thing(Lookup::NotJoined) => "its language is not joined".into(),
        Landing::Thing(Lookup::Refused(said)) => format!("its language does not answer: {}", said.first().map(|s| s.message.en.as_str()).unwrap_or("")),
        Landing::Thing(Lookup::Found(None)) => "the file, read".into(),
        Landing::Thing(Lookup::Found(Some(i))) => format!("found at line {}", i.lines.0),
        Landing::Thing(Lookup::Missing(same)) => format!("no such thing ({} of the same kind)", same.len()),
    }
}

/// Every reference between the files of the project, resolved through the index: the file it
/// lands on and what that file's language says of the thing named.
#[test]
fn every_reference_of_the_project_resolved() {
    let p = load(&shop());
    let j = Joined::new();
    let (refs, refused) = p.references(&j);
    assert!(refused.is_empty(), "{refused:?}");
    let mut text = String::new();
    for r in &refs {
        let where_ = if r.in_project { "in the project" } else { "outside it" };
        text.push_str(&format!("{}:{} {} -> {} ({where_}): {}\n", r.from.rel, r.line, r.how, r.target.text(), landing(&r.landing)));
    }
    let mut failures = Vec::new();
    if let Err(e) = ritsu_testkit::golden::check(Path::new("tests/golden/shop.references.txt"), &text) {
        failures.push(e);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // every reference of the example lands on what it names
    assert!(refs.iter().all(|r| !matches!(r.landing, Landing::Missing | Landing::Proto(Ok(false)) | Landing::Thing(Lookup::Missing(_) | Lookup::Refused(_)))), "{text}");
}

/// A naming that names nothing in a file the index reads comes back with the things of the same
/// kind beside it, for whoever says what is wrong.
#[test]
fn a_thing_that_is_not_there_is_missing() {
    let j = Joined::new();
    let root = shop();
    let rule = ritsu_base::naming::Name::file(Tool::Rulec, "delivery/rules/出荷の急ぎ.rule");
    assert!(matches!(j.index.find(&root, &rule.clone().with("output", "急ぎ")), Lookup::Found(Some(_))));
    match j.index.find(&root, &rule.with("output", "急がない")) {
        Lookup::Missing(same) => assert!(same.iter().any(|i| i.name() == "急ぎ"), "{same:?}"),
        other => panic!("{other:?}"),
    }
}
