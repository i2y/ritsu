//! A project and its names (DESIGN 1.1–1.3, 2.2, PLAN B.3): what is declared twice or not at
//! all, versions, aliases, the namings of links and scopes (E007–E013, E403), the root, and
//! paths written from a `.req` in a directory.

mod common;

use common::{TempDir, yurai};

const HEAD: &str = "requirements 試し v1\nrole 経理\n\n";
const OK_REQ: &str = "requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  not satisfied \"例\"\n  not verified \"例\"\n";

fn codes_of(files: &[(&str, &str)]) -> Vec<String> {
    let t = TempDir::new("project");
    for (n, s) in files {
        t.write(n, s.as_bytes());
    }
    let r = yurai(t.path(), &["check", ".", "--root", "."]);
    common::codes(&r.stdout)
}

fn first_code(body: &str) -> String {
    let src = format!("{HEAD}{body}");
    let got = codes_of(&[("t.req", &src), ("a.txt", "a\n")]);
    got.first().cloned().unwrap_or_else(|| "none".into())
}

#[test]
fn names_declared_twice_or_not_at_all() {
    let cases: &[(&str, &str)] = &[
        ("role 経理\n\n", "E007"),
        (&format!("{OK_REQ}\n{OK_REQ}"), "E007"),
        ("requirement 支払日(pay)\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n\nrequirement 締め日(pay)\n  text \"y\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n", "E007"),
        ("requirement 支払日(pay) v1\n  text \"x\"\n  in force ..2026-12-31\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n\nrequirement 支払日(pay_day) v2\n  text \"y\"\n  in force 2027-01-01..\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n", "E007"),
        ("requirement r1\n  text \"x\"\n  owner 経理部\n  decided 2026-10-03 by 経理 \"例\"\n", "E008"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 法務 \"例\"\n", "E008"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  from 無い要件\n", "E008"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  satisfied by file \"a.txt\"\n    reviewed 2026-10-03 by 法務 sha256:0000000000000000 -> sha256:0000000000000000\n", "E008"),
        ("requirement r1 v0\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n", "E009"),
        ("requirement r1 v1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n\nrequirement r1\n  text \"y\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n", "E009"),
        ("requirement r1 v1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n\nrequirement r1 v1\n  text \"y\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n", "E009"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n\nrequirement r2\n  text \"y\"\n  owner 経理\n  from r1 v2\n", "E009"),
        ("requirement r1\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n", "E010"),
        ("requirement r1\n  text \"x\"\n  decided 2026-10-03 by 経理 \"例\"\n", "E010"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n", "E010"),
        ("requirement 支払日\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n", "E010"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  satisfied by excel \"a.xlsx\"\n", "E011"),
        ("scope dir \"src\"\n\n", "E011"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  satisfied by proto \"o.proto\" method Create\n", "E012"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  satisfied by koyomi \"a.cal\" source 民法\n", "E012"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  satisfied by yurai \"b.req\" requirement r2\n", "E012"),
        ("scope koyomi \"a.cal\" source\n\n", "E012"),
        ("scope proto \"o.proto\" method\n\n", "E012"),
        ("source 法 = yurai \"b.req\" source 法\n\n", "E012"),
        ("source 法 = rulec \"x.rule\" output y\n\n", "E012"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  satisfied by file \"/etc/hosts\"\n", "E013"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  satisfied by file \"../x\"\n", "E013"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  satisfied by file \"\"\n", "E013"),
        ("source 約款 = file \"/etc/terms.md\" sha256:0000000000000000\n\n", "E013"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  verified by rulec \"x.rule\" output y\n", "E403"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  verified by chobo \"x.book\" transfer t\n", "E403"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  verified by proto \"x.proto\"\n", "E403"),
        ("requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  verified by koyomi \"x.cal\" date d\n", "E403"),
    ];
    for (body, code) in cases {
        assert_eq!(first_code(body), *code, "{body}");
    }
}

#[test]
fn what_the_side_that_verifies_takes() {
    // A claim, the whole file of a tool that checks it, or a file: no E403 (the tools' JSON is
    // the next stage's, so this stage refuses them with exit 2 after the names pass).
    for v in ["geas \"g.geas\" claim \"rejects an empty name\"", "koyomi \"a.cal\" claim c", "rulec \"x.rule\"", "chobo \"x.book\"", "dandori \"o.flow\"", "geas \"g.geas\"", "sakai \"c.ctx\"", "file \"a.txt\""] {
        let src = format!("{HEAD}requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  not satisfied \"例\"\n  verified by {v}\n");
        let got = codes_of(&[("t.req", &src), ("a.txt", "a\n")]);
        assert!(!got.iter().any(|c| c == "E403" || c.starts_with("E0")), "{v}: {got:?}");
    }
}

#[test]
fn the_names_of_a_project_span_its_files() {
    let a = format!("{HEAD}{OK_REQ}");
    let b = "requirements 別 v1\n\nrequirement r2\n  text \"y\"\n  owner 経理\n  from r1\n  not satisfied \"例\"\n  not verified \"例\"\n";
    // r2 is read from r1, in another file, and owned by a role declared there: no errors of
    // names (nothing is looked at yet: E301 and E304).
    let got = codes_of(&[("a.req", &a), ("sub/b.req", b)]);
    assert!(got.iter().all(|c| c == "E301" || c == "E304"), "{got:?}");
    // The same heading twice is E007.
    let got = codes_of(&[("a.req", &a), ("b.req", &format!("{HEAD}requirement r2\n  text \"y\"\n  owner 経理\n  from r1\n"))]);
    assert!(got.contains(&"E007".to_string()), "{got:?}");
}

#[test]
fn the_root_is_the_nearest_directory_with_a_git() {
    let t = TempDir::new("root");
    std::fs::create_dir_all(t.path().join(".git")).unwrap();
    let src = format!("{HEAD}requirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n  satisfied by file \"../code/./a.txt\"\n  not verified \"例\"\n");
    t.write("reqs/deep/t.req", src.as_bytes());
    t.write("reqs/code/a.txt", b"a\n");
    // From below the root, the paths in the JSON are from the root.
    let r = yurai(&t.path().join("reqs"), &["api", "deep"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["root"], "..");
    assert_eq!(v["files"][0]["path"], "reqs/deep/t.req");
    assert_eq!(v["requirements"][0]["links"][0]["artifact"]["path"], "reqs/code/a.txt");
    assert_eq!(v["requirements"][0]["links"][0]["artifact"]["text"], "file \"reqs/code/a.txt\"");
    // `--root` changes it.
    let r = yurai(&t.path().join("reqs"), &["api", "deep", "--root", "."]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["root"], ".");
    assert_eq!(v["requirements"][0]["links"][0]["artifact"]["path"], "code/a.txt");
    // A path that climbs above the root is E013.
    let r = yurai(&t.path().join("reqs"), &["check", "deep", "--root", "deep"]);
    assert_eq!(common::codes(&r.stdout), ["E013"]);
    // Without a `.git`, the root is the directory given.
    std::fs::remove_dir_all(t.path().join(".git")).unwrap();
    let r = yurai(&t.path().join("reqs"), &["api", "deep/t.req"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    assert_eq!(common::codes(&r.stderr), ["E013"], "a path above deep/ is outside the root");
}

#[test]
fn the_files_a_directory_stands_for() {
    let t = TempDir::new("expand");
    t.write("b.req", format!("{HEAD}{OK_REQ}").as_bytes());
    t.write("a/c.req", b"requirements c v1\n");
    t.write(".hidden/d.req", b"not even a file of the language\n");
    t.write("target/e.req", b"not even a file of the language\n");
    let found = yurai::project::expand(t.path().to_str().unwrap());
    let names: Vec<String> = found.iter().map(|f| f.rsplit('/').next().unwrap().to_string()).collect();
    assert_eq!(names, ["c.req", "b.req"]);
}
