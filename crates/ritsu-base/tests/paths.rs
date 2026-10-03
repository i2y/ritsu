//! Paths (DESIGN 4.7, 6.2): a written path made a path from the root, the root found by its
//! `.git`, and a path shown the shortest way from where the tool runs — not up to the root and
//! down again when the root is above (what yuen printed in the workspace: `../../crates/…`).

use ritsu_base::paths::{self, PathError, Shown};
use ritsu_testkit::TempDir;
use std::path::Path;

#[test]
fn a_written_path_becomes_a_path_from_the_root() {
    assert_eq!(paths::join("contexts", "../inventory"), Ok("inventory".into()));
    assert_eq!(paths::join(".", "./calendars/../支払条件.cal"), Ok("支払条件.cal".into()));
    assert_eq!(paths::join("", "src/"), Ok("src".into()), "a trailing slash is dropped");
    assert_eq!(paths::join("a", "b//c/"), Ok("a/b/c".into()));
    assert_eq!(paths::join(".", "."), Ok(".".into()));
    assert_eq!(paths::join("a/b", "../../x"), Ok("x".into()));
    assert_eq!(paths::join("a/b", "../../.."), Err(PathError::Outside));
    assert_eq!(paths::join(".", "/etc/hosts"), Err(PathError::Absolute));
    assert_eq!(paths::join(".", "C:/x"), Err(PathError::Absolute));
    assert_eq!(paths::join(".", "\\x"), Err(PathError::Absolute));
    assert_eq!(paths::join(".", ""), Err(PathError::Empty));
    assert_eq!(PathError::Outside.text("../x").en, "`../x` goes outside the root");
}

#[test]
fn containment_depth_and_paths_between_paths_from_the_root() {
    assert!(paths::contains(".", "a/b") && paths::contains("a", "a/b") && paths::contains("a", "a"));
    assert!(!paths::contains("a", "ab/c"));
    assert_eq!((paths::depth("."), paths::depth("a"), paths::depth("a/b")), (0, 1, 2));
    assert_eq!(paths::parent("a/b.ctx"), "a");
    assert_eq!(paths::parent("b.ctx"), ".");
    assert_eq!(paths::relative("ctx", "proto/x.proto"), "../proto/x.proto");
    assert_eq!(paths::relative(".", "proto"), "proto");
    assert_eq!(paths::relative("a/b", "a/b"), ".");
}

#[test]
fn the_root_is_the_nearest_directory_with_a_git() {
    let t = TempDir::new("root");
    std::fs::create_dir_all(t.path().join("repo/.git")).unwrap();
    std::fs::create_dir_all(t.path().join("repo/crates/a/sub")).unwrap();
    t.write("repo/crates/a/sub/x.req", "x");
    let repo = t.path().join("repo");
    assert_eq!(paths::find_root(&t.path().join("repo/crates/a/sub/x.req")), repo, "from a file");
    assert_eq!(paths::find_root(&t.path().join("repo/crates/a")), repo, "from a directory");
    // A worktree's .git is a file.
    std::fs::create_dir_all(t.path().join("wt/d")).unwrap();
    t.write("wt/.git", "gitdir: elsewhere\n");
    assert_eq!(paths::find_root(&t.path().join("wt/d")), t.path().join("wt"));
    // No .git above: the directory given, or the file's.
    std::fs::create_dir_all(t.path().join("plain/d")).unwrap();
    t.write("plain/d/f.ctx", "");
    assert_eq!(paths::find_root(&t.path().join("plain/d/f.ctx")), t.path().join("plain/d"));
    assert_eq!(paths::from_root(&repo, &t.path().join("repo/crates/a/sub/x.req")).as_deref(), Some("crates/a/sub/x.req"));
    assert_eq!(paths::from_root(&repo, &repo).as_deref(), Some("."));
    assert_eq!(paths::from_root(&repo, &t.path().join("plain")), None);
}

#[test]
fn a_path_is_shown_the_shortest_way_from_where_the_tool_runs() {
    let root = Path::new("/w/repo");
    // Run in a crate two below the root: a path under it is written from there.
    let s = Shown::run_in(root, Path::new("/w/repo/crates/yuen"), "tests/x");
    assert_eq!(s.path("crates/yuen/tests/x/民法の期間.req"), "tests/x/民法の期間.req");
    assert_eq!(s.path("crates/sakai/a.ctx"), "../sakai/a.ctx");
    assert_eq!(s.root(), "../..");
    assert_eq!(s.path("src/"), "../../src/", "a directory keeps its slash");
    // Run at the root, and above it.
    assert_eq!(Shown::run_in(root, root, "x").path("a/b.req"), "a/b.req");
    assert_eq!(Shown::run_in(root, root, "x").root(), ".");
    assert_eq!(Shown::run_in(root, Path::new("/w"), "repo/a").path("a/b.req"), "repo/a/b.req");
    // A path given absolute is written absolute.
    assert_eq!(Shown::run_in(root, Path::new("/elsewhere"), "/w/repo/a").path("a/b.req"), "/w/repo/a/b.req");
    assert_eq!(paths::between(Path::new("/a/b"), Path::new("/a/c/d")), "../c/d");
    assert_eq!(paths::between(Path::new("/a"), Path::new("/a")), ".");
}

#[test]
fn a_walk_passes_over_the_state_of_tools_and_build_outputs() {
    let t = TempDir::new("walk");
    for f in ["a/x.py", "a/b/y.py", ".git/config", "node_modules/m/i.js", "a/__pycache__/x.pyc", "target/debug/z", ".venv/bin/p", "a/site-packages/s.py", "keep/k.txt"] {
        t.write(f, "");
    }
    let mut out = Vec::new();
    paths::walk(t.path(), ".", &["keep".to_string()], &mut out);
    assert_eq!(out, ["a/b/y.py", "a/x.py"]);
    assert!(paths::skipped_name(".geas") && paths::skipped_name("target") && !paths::skipped_name("src"));
    assert!(paths::has_skipped_part("a/node_modules/x") && !paths::has_skipped_part("."));
}
