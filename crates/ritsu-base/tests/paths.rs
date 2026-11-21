//! Paths (DESIGN 4.7, 4.15, 6.2): a written path made a path from the root, the root found by its
//! `.git`, and a path shown the shortest way from where the tool runs — not up to the root and
//! down again when the root is above (what yuen printed in the workspace: `../../crates/…`) — and
//! every path a run holds written that one way, however the run reached it.

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

/// A path a run holds is written one way however it was reached (DESIGN 4.15): `.` and `..` folded,
/// the shortest way from where the run is, a file beside it too, and absolute only while the run's
/// first path was absolute. Outside a run, a path held absolute stays absolute. The tests run in the
/// crate's directory.
#[test]
fn a_run_writes_every_path_it_holds_one_way() {
    let held = paths::absolute(Path::new("src/paths.rs"));
    let beside = paths::absolute(Path::new("../ritsu/Cargo.toml"));
    let abs = |p: &Path| p.to_string_lossy().to_string();
    assert_eq!(paths::show(Path::new("src/../src/./paths.rs")), "src/paths.rs");
    assert_eq!(paths::show(&held), abs(&held), "outside a run, absolute as it is held");
    {
        let _run = paths::run_from("tests/x.req");
        assert_eq!(paths::show(&held), "src/paths.rs", "held absolute, written from where the run is");
        assert_eq!(paths::show(Path::new("tests/../src/paths.rs")), "src/paths.rs");
        assert_eq!(paths::show_str("../ritsu-base/src/paths.rs"), "src/paths.rs", "up and down again, folded");
        assert_eq!(paths::show(&beside), "../ritsu/Cargo.toml", "beside the run, relative still");
        {
            let _inner = paths::run_from(&abs(&held));
            assert_eq!(paths::show(Path::new("src/paths.rs")), abs(&held), "a run given an absolute path writes absolute");
            assert_eq!(paths::show(Path::new("src/../../ritsu/Cargo.toml")), abs(&beside));
        }
        assert_eq!(paths::show(&held), "src/paths.rs", "the run before comes back");
    }
    assert_eq!(paths::show(&held), abs(&held), "and outside a run again");
}

/// A path given to a command is held as the run writes it (DESIGN 4.15, 6.2 item 8): `./fee.rule`
/// and `x/../fee.rule` are `fee.rule`, and what is no path of the files (`-`, a dependency's `@…`, an
/// address) is kept as written. In a module, `@notes.rule` names a dependency from the module's
/// root, so a file below the working directory whose name starts with `@` is held and written
/// `./@notes.rule`; outside every module, `@notes.rule` is the file beside, and is written so. A
/// path a flag takes (`--out ./out`) is folded the same, and is absolute only when it was given so.
/// A path the system does not open as it is written (`no/../fee.rule` with no `no`, `fee.rule/`) is
/// kept as written.
#[test]
fn a_path_given_is_held_as_the_run_writes_it() {
    let t = TempDir::new("given");
    let top = std::fs::canonicalize(t.path()).unwrap();
    for f in ["shop/rules/fee.rule", "shop/rules/@notes.rule", "loose/@notes.rule"] {
        t.write(f, f);
    }
    t.write("shop/ritsu.mod", "module example.com/acme/shop\n");
    t.write("loose/.git", "gitdir: elsewhere\n");
    std::fs::create_dir_all(top.join("shop/rules/x")).unwrap();
    let given = |dir: &str, p: &str| {
        let here = std::rc::Rc::new(Here { dir: top.join(dir), asked: std::cell::Cell::new(0) });
        ritsu_base::fs::with(here, || {
            let _run = paths::run_from(p);
            paths::show_given(p)
        })
    };
    assert_eq!(given("shop/rules", "./fee.rule"), "fee.rule");
    assert_eq!(given("shop/rules", "x/../fee.rule"), "fee.rule");
    assert_eq!(given("shop", "rules/./fee.rule"), "rules/fee.rule");
    assert_eq!(given("shop/rules", "./@notes.rule"), "./@notes.rule", "in a module, `@notes.rule` is a dependency's");
    assert_eq!(given("shop/rules", "../rules/@notes.rule"), "./@notes.rule");
    assert_eq!(given("shop", "rules/../rules/@notes.rule"), "rules/@notes.rule");
    assert_eq!(given("loose", "./@notes.rule"), "@notes.rule", "outside every module, the file beside");
    for kept in ["-", "@pricing/rules/fee.rule", "@pricing@1.0.0/rules/fee.rule", "https://example.com/fee.rule", ""] {
        assert_eq!(given("shop/rules", kept), kept);
    }
    let shown = Shown::run_in(&top.join("shop"), &top.join("shop/rules"), "fee.rule");
    assert_eq!(shown.path("rules/@notes.rule"), "./@notes.rule");
    assert_eq!(shown.path("rules/fee.rule"), "fee.rule");
    // a path a flag takes is folded the same, and stays absolute or relative as it was given,
    // whichever way the run writes the files it reads
    let flag = |first: &str, p: &str| {
        let here = std::rc::Rc::new(Here { dir: top.join("shop/rules"), asked: std::cell::Cell::new(0) });
        ritsu_base::fs::with(here, || {
            let _run = paths::run_from(first);
            paths::fold_given(p)
        })
    };
    let abs_rule = top.join("shop/rules/fee.rule").to_string_lossy().to_string();
    let abs_out = top.join("shop/rules/out").to_string_lossy().to_string();
    for p in ["out", "./out", "out/", "x/../out", "../rules/out"] {
        assert_eq!(flag("fee.rule", p), "out", "{p}");
        assert_eq!(flag(&abs_rule, p), "out", "{p}, in a run given an absolute path");
    }
    assert_eq!(flag("fee.rule", &format!("{}/x/../out", top.join("shop/rules").display())), abs_out);
    assert_eq!(flag("fee.rule", "./@notes"), "./@notes");
    for kept in ["-", "@pricing/out", ""] {
        assert_eq!(flag("fee.rule", kept), kept);
    }
    // a path the system does not open as it is written is kept so, to be refused as it was: a `..`
    // out of what is not there, and a `/` at the end of a file. A `/` at the end of a directory,
    // and of a path that is not there at all, goes with the folding.
    assert_eq!(given("shop", "rules/"), "rules");
    assert_eq!(given("shop", "./rules//"), "rules");
    assert_eq!(given("shop/rules", "no/../fee.rule"), "no/../fee.rule");
    assert_eq!(given("shop/rules", "fee.rule/"), "fee.rule/");
    assert_eq!(given("shop/rules", "./x/../fee.rule/"), "./x/../fee.rule/");
    assert_eq!(flag("fee.rule", "fee.rule/"), "fee.rule/");
    assert_eq!(flag("fee.rule", "no/../out"), "no/../out");
    assert_eq!(flag("fee.rule", "./x/../out/"), "out");
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

#[cfg(unix)]
fn link(to: impl AsRef<Path>, at: &Path) {
    std::os::unix::fs::symlink(to, at).unwrap_or_else(|e| panic!("cannot link {}: {e}", at.display()));
}

#[cfg(not(unix))]
fn link(_: impl AsRef<Path>, _: &Path) {
    unreachable!("links are made on Unix only")
}

/// The file a path names (DESIGN 4.15): `x/..` is folded by its letters where `x` is no symbolic
/// link; where `x` is one, the `..` is taken from where the link leads, as the system takes it, for
/// a link written relative or absolute and for a link to a link. A link the path goes through
/// without `..` is not followed; a part that is not there is folded by its letters, and so is a loop
/// of links, and every path of a page's files, which hold no link.
#[test]
fn a_path_that_climbs_out_of_a_link_is_the_file_the_system_opens() {
    if !cfg!(unix) {
        ritsu_testkit::skip("the test makes symbolic links, which it makes on Unix only");
        return;
    }
    let t = TempDir::new("opened");
    let top = std::fs::canonicalize(t.path()).unwrap();
    for f in ["common/ex/rules/fee.rule", "common/ex/in/x.flow", "common/rules/fee.rule", "project/rules/fee.rule"] {
        t.write(f, f);
    }
    link("../common/ex/in", &top.join("project/in"));
    link(top.join("common/ex"), &top.join("project/abs"));
    link("in", &top.join("project/again"));
    link("round", &top.join("project/round"));
    let p = |s: &str| top.join(s);
    let cases = [
        ("project/in/../rules/fee.rule", "common/ex/rules/fee.rule"),
        ("project/abs/../rules/fee.rule", "common/rules/fee.rule"),
        ("project/again/../rules/fee.rule", "common/ex/rules/fee.rule"),
        ("project/in/./../in/../rules/fee.rule", "common/ex/rules/fee.rule"),
        // `abs/in` is a directory, not a link: the path keeps its letters, `abs` not followed
        ("project/abs/in/../rules/fee.rule", "project/abs/rules/fee.rule"),
        ("project/rules/../rules/fee.rule", "project/rules/fee.rule"),
        ("project/in/x.flow", "project/in/x.flow"),
        ("project/nothere/../rules/fee.rule", "project/rules/fee.rule"),
        ("project/round/../rules/fee.rule", "project/rules/fee.rule"),
    ];
    for (given, folded) in cases {
        assert_eq!(paths::opened(&p(given)), p(folded), "{given}");
    }
    for (given, _) in &cases[..7] {
        assert_eq!(std::fs::read(paths::opened(&p(given))).unwrap(), std::fs::read(p(given)).unwrap(), "{given} opens another file");
    }
    assert_eq!(paths::absolute(&p("project/in/../rules/fee.rule")), p("project/rules/fee.rule"), "absolute folds by the letters the person wrote");
    let page = std::rc::Rc::new(ritsu_base::fs::Memory::new(top.clone()));
    page.add("project/rules/fee.rule", "rule fee v1\n");
    assert_eq!(ritsu_base::fs::with(page, || paths::opened(&p("project/in/../rules/fee.rule"))), p("project/rules/fee.rule"), "a page's files hold no link");
}

/// The root above a path and the path's name from a root are those of the file the system opens
/// (DESIGN 4.15): a path whose `..` crosses a symbolic link finds the root above where the link
/// leads, is named from a root by the file it opens, and is outside a root that file is outside, as
/// the path written to that file is. A path a file writes is joined to the file the system opens by
/// `join_opened` and by its letters by `join`, which a `.req`, a `.ctx` and a `.geas` keep for the
/// references they write (DESIGN 6.2, item 3); a path that crosses no link is the same either way.
#[test]
fn the_root_above_a_path_and_its_name_from_there_are_the_file_the_system_opens() {
    if !cfg!(unix) {
        ritsu_testkit::skip("the test makes symbolic links, which it makes on Unix only");
        return;
    }
    use ritsu_base::modules::{self, Refused};
    let t = TempDir::new("rooted");
    let top = std::fs::canonicalize(t.path()).unwrap();
    for f in ["common/ex/rules/fee.rule", "common/ex/in/x.rule", "project/rules/fee.rule", "project/rules/base.rule"] {
        t.write(f, f);
    }
    t.write("common/ex/.git", "gitdir: elsewhere\n");
    t.write("project/.git", "gitdir: elsewhere\n");
    link("../common/ex/in", &top.join("project/in"));
    let p = |s: &str| top.join(s);
    let through = p("project/in/../rules/fee.rule");
    assert_eq!(paths::find_root(&through), p("common/ex"), "the root above where the link leads");
    assert_eq!(paths::find_root(&p("project/in/x.rule")), p("project"), "a link gone through without `..` is not followed");
    assert_eq!(paths::from_root(&p("common/ex"), &through).as_deref(), Some("rules/fee.rule"));
    assert_eq!(paths::from_root(&p("project"), &through), None, "outside the root, as `../common/ex/rules/fee.rule` is");
    assert_eq!(paths::from_root(&top, &through).as_deref(), Some("common/ex/rules/fee.rule"));
    assert_eq!(paths::from_root(&p("project/in/.."), &p("common/ex/rules/fee.rule")).as_deref(), Some("rules/fee.rule"), "a root given through the link");
    assert_eq!(paths::from_root(&p("project"), &p("project/in/x.rule")).as_deref(), Some("in/x.rule"));
    assert_eq!(modules::join_opened(&top, "project/in/x.rule", "../rules/fee.rule").ok().as_deref(), Some("common/ex/rules/fee.rule"));
    assert_eq!(modules::join(&top, "project/in/x.rule", "../rules/fee.rule").ok().as_deref(), Some("project/rules/fee.rule"), "a reference keeps its letters");
    assert!(
        matches!(modules::join_opened(&p("project"), "in/x.rule", "../rules/fee.rule"), Err(Refused::Path(PathError::Outside, ref w)) if w == "../rules/fee.rule"),
        "the file opened is outside the root"
    );
    for (from, written) in [("project/rules/base.rule", "fee.rule"), ("project/rules/base.rule", "../rules/./fee.rule"), ("project/in/x.rule", "y.rule")] {
        assert_eq!(modules::join_opened(&top, from, written).ok(), modules::join(&top, from, written).ok(), "{from} writes {written}");
    }
}

/// The files on the disk, read from `dir` as the working directory, counting how often the working
/// directory is asked for.
struct Here {
    dir: std::path::PathBuf,
    asked: std::cell::Cell<usize>,
}

impl Here {
    fn abs(&self, p: &Path) -> std::path::PathBuf {
        if p.is_absolute() { p.to_path_buf() } else { self.dir.join(p) }
    }
}

impl ritsu_base::fs::Files for Here {
    fn read(&self, p: &Path) -> std::io::Result<Vec<u8>> {
        ritsu_base::fs::Disk.read(&self.abs(p))
    }
    fn metadata(&self, p: &Path) -> std::io::Result<ritsu_base::fs::Meta> {
        ritsu_base::fs::Disk.metadata(&self.abs(p))
    }
    fn read_dir(&self, p: &Path) -> std::io::Result<Vec<(std::ffi::OsString, ritsu_base::fs::Kind)>> {
        ritsu_base::fs::Disk.read_dir(&self.abs(p))
    }
    fn write(&self, p: &Path, bytes: &[u8]) -> std::io::Result<()> {
        ritsu_base::fs::Disk.write(&self.abs(p), bytes)
    }
    fn create_dir_all(&self, p: &Path) -> std::io::Result<()> {
        ritsu_base::fs::Disk.create_dir_all(&self.abs(p))
    }
    fn current_dir(&self) -> std::io::Result<std::path::PathBuf> {
        self.asked.set(self.asked.get() + 1);
        Ok(self.dir.clone())
    }
    fn canonicalize(&self, p: &Path) -> std::io::Result<std::path::PathBuf> {
        ritsu_base::fs::Disk.canonicalize(&self.abs(p))
    }
    fn link(&self, p: &Path) -> Option<std::path::PathBuf> {
        ritsu_base::fs::Disk.link(&self.abs(p))
    }
}

/// Whether the file the system opens is the one the letters of a path name (DESIGN 4.15) is what
/// `opened` and `absolute` say, for a path written relative and made absolute: no `..`, a link gone
/// through without `..`, a `..` that crosses no link, one that crosses a link, one that climbs above
/// the working directory (named with no link in it, as the system names it), with and without a
/// `..` below it that crosses a link, a path through a module's `@`, and a page's files, which hold
/// no link. The working directory is asked for only where a `..` crosses a link or the path goes
/// through an `@`: the question is asked of every path a check joins, as often as it joins one.
#[test]
fn a_path_folds_by_its_letters_unless_a_dotdot_of_it_crosses_a_link() {
    if !cfg!(unix) {
        ritsu_testkit::skip("the test makes symbolic links, which it makes on Unix only");
        return;
    }
    let t = TempDir::new("letters");
    let top = std::fs::canonicalize(t.path()).unwrap();
    for f in ["common/ex/rules/fee.rule", "common/ex/in/x.rule", "project/rules/fee.rule", "project/plain/x.rule"] {
        t.write(f, f);
    }
    link("../common/ex/in", &top.join("project/in"));
    let here = std::rc::Rc::new(Here { dir: top.join("project"), asked: std::cell::Cell::new(0) });
    // (the path from `project`, whether it folds by its letters, whether the working directory is asked)
    let cases = [
        ("rules/fee.rule", true, false),
        ("in/x.rule", true, false),
        ("plain/../rules/fee.rule", true, false),
        ("nothere/../rules/fee.rule", true, false),
        ("in/../rules/fee.rule", false, true),
        ("in/./../rules/fee.rule", false, true),
        ("plain/../in/../rules/fee.rule", false, true),
        ("../project/rules/fee.rule", true, false),
        ("plain/../../project/rules/fee.rule", true, false),
        ("../project/plain/../rules/fee.rule", true, false),
        ("../project/in/../rules/fee.rule", false, true),
        ("@dep/../rules/fee.rule", true, true),
    ];
    for (given, letters, asked) in cases {
        let p = Path::new(given);
        here.asked.set(0);
        let got = ritsu_base::fs::with(here.clone(), || paths::folds_by_letters(p));
        assert_eq!((got, here.asked.get() > 0), (letters, asked), "{given}: folds by its letters, the working directory asked");
        assert_eq!(got, ritsu_base::fs::with(here.clone(), || paths::opened(p) == paths::absolute(p)), "{given}: as opened and absolute say");
        let whole = top.join("project").join(given);
        assert_eq!(ritsu_base::fs::with(here.clone(), || paths::folds_by_letters(&whole)), letters, "{given} made absolute");
    }
    let page = std::rc::Rc::new(ritsu_base::fs::Memory::new(top.clone()));
    page.add("project/rules/fee.rule", "rule fee v1\n");
    assert!(ritsu_base::fs::with(page, || paths::folds_by_letters(Path::new("project/in/../rules/fee.rule"))), "a page's files hold no link");
}
