//! Where a project's files are read and written (`ritsu_base::fs`, DESIGN 4.15): the disk as
//! `std::fs` reads it, unless `with` hands over files held in memory, which answer as a directory
//! would, with the disk's own words for what is not there.

use ritsu_base::fs::{self, Kind, Memory};
use ritsu_base::paths;
use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn shop() -> Rc<Memory> {
    let m = Memory::new("/work");
    m.add("rules/fee.rule", "rule fee v1\n");
    m.add("flows/order.flow", "workflow order v1\n");
    m.add("/elsewhere/notes.txt", "notes\n");
    Rc::new(m)
}

#[test]
fn memory_reads_as_a_directory_does() {
    fs::with(shop(), || {
        assert_eq!(fs::read_to_string("rules/fee.rule").unwrap(), "rule fee v1\n");
        assert_eq!(fs::read("/work/flows/order.flow").unwrap(), b"workflow order v1\n");
        assert_eq!(fs::read_to_string("rules/../flows/./order.flow").unwrap(), "workflow order v1\n");
        assert_eq!(fs::current_dir().unwrap(), PathBuf::from("/work"));
        assert!(fs::is_dir("rules") && fs::is_dir(".") && fs::is_dir("/") && fs::is_file("rules/fee.rule"));
        assert!(!fs::exists("rules/missing.rule"));
        let m = fs::metadata("rules/fee.rule").unwrap();
        assert_eq!((m.kind, m.len()), (Kind::File, 12));
        let names: Vec<(String, bool)> = fs::read_dir(".").unwrap().map(|e| e.unwrap()).map(|e| (e.file_name().to_string_lossy().into_owned(), e.file_type().unwrap().is_dir())).collect();
        assert_eq!(names, [("flows".to_string(), true), ("rules".to_string(), true)]);
        let e = fs::read_dir("rules").unwrap().next().unwrap().unwrap();
        assert_eq!(e.path(), PathBuf::from("rules/fee.rule"));
        assert_eq!(fs::canonicalize("flows/../rules/fee.rule").unwrap(), PathBuf::from("/work/rules/fee.rule"));
    });
}

/// A message that quotes the error reads as it would from the disk.
#[test]
fn memory_says_what_the_disk_says() {
    fs::with(shop(), || {
        assert_eq!(fs::read("rules/missing.rule").unwrap_err().to_string(), "No such file or directory (os error 2)");
        assert_eq!(fs::read("rules").unwrap_err().to_string(), "Is a directory (os error 21)");
        assert_eq!(fs::read_dir("rules/fee.rule").unwrap_err().to_string(), "Not a directory (os error 20)");
        assert!(fs::canonicalize("nowhere").is_err());
        assert!(fs::write("nowhere/x.txt", "x").is_err(), "a file is written into a directory that is there");
        assert!(fs::create_dir_all("rules/fee.rule/x").is_err(), "no directory under a file");
    });
}

#[test]
fn what_is_written_is_held_and_listed() {
    let m = shop();
    fs::with(m.clone(), || {
        fs::create_dir_all("generated/python").unwrap();
        fs::write("generated/python/fee.py", "def fee(): ...\n").unwrap();
        fs::write("generated/fee.sql", "select 1;\n").unwrap();
        fs::write("generated/python/fee.py", "def fee(): pass\n").unwrap();
        assert_eq!(fs::read_to_string("generated/python/fee.py").unwrap(), "def fee(): pass\n");
    });
    let written: Vec<(String, String)> = m.written().into_iter().map(|(p, b)| (p, String::from_utf8(b).unwrap())).collect();
    assert_eq!(written, [("generated/python/fee.py".to_string(), "def fee(): pass\n".to_string()), ("generated/fee.sql".to_string(), "select 1;\n".to_string())]);
    let under: Vec<String> = m.files_under("generated").into_iter().map(|(p, _)| p).collect();
    assert_eq!(under, ["fee.sql", "python/fee.py"]);
}

/// What a run of code read (`watched`, DESIGN 4.15) holds while every file reads the same, and not
/// once one reads otherwise, with as many bytes or with a file where there was none; what it read
/// is what each file held then.
#[test]
fn what_was_read_holds_while_every_file_reads_the_same() {
    let m = shop();
    let ((), seen) = fs::with(m.clone(), || {
        fs::watched(|| {
            assert!(fs::read("rules/fee.rule").is_ok());
            assert!(fs::read("rules/missing.rule").is_err());
        })
    });
    let then = vec![(PathBuf::from("rules/fee.rule"), Some(b"rule fee v1\n".to_vec())), (PathBuf::from("rules/missing.rule"), None)];
    assert!(fs::with(m.clone(), || seen.still()));
    assert_eq!(reads(&seen), then);
    m.add("rules/fee.rule", "rule fii v1\n");
    assert!(!fs::with(m.clone(), || seen.still()));
    assert_eq!(reads(&seen), then);
    m.add("rules/fee.rule", "rule fee v1\n");
    assert!(fs::with(m.clone(), || seen.still()));
    m.add("rules/missing.rule", "rule missing v1\n");
    assert!(!fs::with(m.clone(), || seen.still()));
}

/// What `seen` read, path and bytes.
fn reads(seen: &fs::Seen) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    seen.reads().map(|(p, b)| (p.to_path_buf(), b.map(<[u8]>::to_vec))).collect()
}

/// The checks of a thread that read the same bytes at a path hold one copy of them (DESIGN 4.15).
/// Once the file is written anew, the checks that read the old bytes no longer hold, one that reads
/// it then holds the new bytes in a copy of its own, and the old copy goes with the last check that
/// held it.
#[test]
fn the_checks_that_read_a_file_share_one_copy_until_it_is_written_anew() {
    let m = shop();
    let read = || fs::with(m.clone(), || fs::watched(|| fs::read("rules/fee.rule").unwrap()).1);
    let (first, second) = (read(), read());
    assert_eq!(fs::copies_held(), (1, 12));
    m.add("rules/fee.rule", "rule fee v2 with more\n");
    assert!(!fs::with(m.clone(), || first.still() || second.still()));
    let third = read();
    assert!(fs::with(m.clone(), || third.still()));
    assert_eq!(fs::copies_held(), (2, 12 + 22));
    drop(first);
    assert_eq!(fs::copies_held(), (2, 12 + 22));
    drop(second);
    assert_eq!(fs::copies_held(), (1, 22));
    assert_eq!(reads(&third), [(PathBuf::from("rules/fee.rule"), Some(b"rule fee v2 with more\n".to_vec()))]);
    drop(third);
    assert_eq!(fs::copies_held(), (0, 0));
}

/// Two files of the same bytes are two files: each is held at its own path, and a check that read
/// one does not rest on the other.
#[test]
fn two_files_of_the_same_bytes_are_held_apart() {
    let m = shop();
    m.add("rules/fee_again.rule", "rule fee v1\n");
    let read = |paths: &[&str]| {
        fs::with(m.clone(), || {
            fs::watched(|| {
                for p in paths {
                    fs::read(p).unwrap();
                }
            })
            .1
        })
    };
    let both = read(&["rules/fee.rule", "rules/fee_again.rule"]);
    let one = read(&["rules/fee.rule"]);
    assert_eq!(fs::copies_held(), (2, 24));
    m.add("rules/fee_again.rule", "rule fee v2\n");
    assert!(fs::with(m.clone(), || one.still() && !both.still()));
    assert_eq!(reads(&both)[1], (PathBuf::from("rules/fee_again.rule"), Some(b"rule fee v1\n".to_vec())));
}

/// A path read through other files (an `Overlay` laid over the disk) is held apart from the same
/// path on the disk: each check holds what it read, and holds while the files it is asked through
/// read the same.
#[test]
fn a_path_read_through_other_files_is_held_apart() {
    let t = TempDir::new("fs");
    let file = t.write("fee.rule", "rule fee v1\n");
    let on_disk = fs::watched(|| fs::read(&file).unwrap()).1;
    let mut laid = fs::Overlay::new(None);
    laid.put(&file, "rule fee v2\n");
    let laid: Rc<dyn fs::Files> = Rc::new(laid);
    let through = fs::with(laid.clone(), || fs::watched(|| fs::read(&file).unwrap()).1);
    assert_eq!(reads(&on_disk), [(file.clone(), Some(b"rule fee v1\n".to_vec()))]);
    assert_eq!(reads(&through), [(file.clone(), Some(b"rule fee v2\n".to_vec()))]);
    assert!(on_disk.still() && !through.still());
    assert!(fs::with(laid.clone(), || through.still() && !on_disk.still()));
    assert_eq!(fs::copies_held(), (2, 24));
}

/// The paths of ritsu-base read through it: the root of a project, a path from it, a walk.
#[test]
fn the_paths_of_a_project_read_memory_too() {
    let m = Memory::new("/work/shop/billing");
    m.add("/work/shop/.git/HEAD", "ref: refs/heads/main\n");
    m.add("/work/shop/billing/net30.cal", "dates net30 v1\n");
    m.add("/work/shop/billing/rules/fee.rule", "rule fee v1\n");
    m.add("/work/shop/node_modules/x.js", "");
    fs::with(Rc::new(m), || {
        assert_eq!(paths::absolute(Path::new("rules/../net30.cal")), PathBuf::from("/work/shop/billing/net30.cal"));
        let root = paths::find_root(Path::new("."));
        assert_eq!(root, PathBuf::from("/work/shop"));
        assert_eq!(paths::from_root(&root, Path::new("rules/fee.rule")).as_deref(), Some("billing/rules/fee.rule"));
        let mut files = Vec::new();
        paths::walk(&root, ".", &[], &mut files);
        assert_eq!(files, ["billing/net30.cal", "billing/rules/fee.rule"]);
        assert_eq!(paths::Shown::new(&root, ".").path("billing/rules/fee.rule"), "rules/fee.rule");
    });
}

/// Without `with`, and after it, even when what ran inside it panicked, the disk is read.
#[test]
fn the_disk_is_read_outside_with() {
    let t = TempDir::new("fs");
    let file = t.write("a.txt", "on the disk\n");
    assert_eq!(fs::read_to_string(&file).unwrap(), "on the disk\n");
    let caught = std::panic::catch_unwind(|| fs::with(shop(), || panic!("inside")));
    assert!(caught.is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "on the disk\n");
    assert!(fs::is_dir(t.path()) && !fs::exists(t.path().join("missing")));
    let listed: Vec<String> = fs::read_dir(t.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(listed, ["a.txt"]);
    assert_eq!(fs::current_dir().unwrap(), std::env::current_dir().unwrap());
}

/// One `with` inside another reads the inner files, then the outer ones again.
#[test]
fn with_nests() {
    let inner = Memory::new("/inner");
    inner.add("x.txt", "inner\n");
    fs::with(shop(), || {
        fs::with(Rc::new(inner), || assert_eq!(fs::read_to_string("x.txt").unwrap(), "inner\n"));
        assert!(fs::read_to_string("x.txt").is_err());
        assert!(fs::is_file("rules/fee.rule"));
    });
}

/// `realpath`, which `canonicalize` is built for WASI (whose std has none), gives the answer of
/// `std::fs::canonicalize`: through a link of the system (macOS's `/tmp` is `/private/tmp`), a
/// relative link, an absolute one, a link to a link, `..` after a link (which leaves where the
/// link led), `.`; and an error where `canonicalize` gives one (nothing there, a file with more
/// after it, a loop of links).
#[test]
fn realpath_is_what_canonicalize_answers() {
    use std::os::unix::fs::symlink;
    let t = TempDir::new("realpath");
    let d = t.path();
    t.write("real/sub/file.txt", "x");
    t.write("other/inner/b.txt", "y");
    symlink("real", d.join("rel")).unwrap();
    symlink(d.join("real/sub"), d.join("abs")).unwrap();
    symlink("rel", d.join("chain")).unwrap();
    symlink("../other/inner", d.join("real/up")).unwrap();
    symlink("loop-b", d.join("loop-a")).unwrap();
    symlink("loop-a", d.join("loop-b")).unwrap();
    let mut ok = vec![
        PathBuf::from("/tmp"),
        PathBuf::from("/"),
        d.to_path_buf(),
        d.join("rel/sub/file.txt"),
        d.join("abs/file.txt"),
        d.join("chain/sub/./file.txt"),
        d.join("rel/sub/../sub/file.txt"),
        d.join("real/up/b.txt"),
        d.join("real/up/../../real/sub"),
        d.join("abs/.."),
        PathBuf::from("tests/fs.rs"),
        PathBuf::from("./src/../tests"),
    ];
    // the temporary directory as the system names it: through /var on macOS, as it is elsewhere
    ok.push(std::env::temp_dir());
    for p in &ok {
        assert_eq!(fs::realpath(p).unwrap(), std::fs::canonicalize(p).unwrap(), "{}", p.display());
    }
    for p in [d.join("missing"), d.join("rel/missing/x")] {
        assert_eq!(std::fs::canonicalize(&p).unwrap_err().kind(), std::io::ErrorKind::NotFound, "{}", p.display());
        assert_eq!(fs::realpath(&p).unwrap_err().kind(), std::io::ErrorKind::NotFound, "{}", p.display());
    }
    // (`file.txt/..` is not here: macOS's realpath(3) takes it to the directory, Linux's refuses it)
    for p in [d.join("real/sub/file.txt/x"), d.join("loop-a")] {
        assert!(std::fs::canonicalize(&p).is_err() && fs::realpath(&p).is_err(), "{} leads nowhere", p.display());
    }
}

/// An error of WASI says the system's number where Linux and macOS give the same words and number
/// (the ten of `renumbered`); any other is left as it is.
#[test]
fn an_error_of_wasi_takes_the_systems_number() {
    assert_eq!(fs::renumbered("No such file or directory (os error 44)", 44).as_deref(), Some("No such file or directory (os error 2)"));
    assert_eq!(fs::renumbered("Permission denied (os error 2)", 2).as_deref(), Some("Permission denied (os error 13)"));
    assert_eq!(fs::renumbered("Is a directory (os error 31)", 31).as_deref(), Some("Is a directory (os error 21)"));
    assert_eq!(fs::renumbered("Not a directory (os error 54)", 54).as_deref(), Some("Not a directory (os error 20)"));
    assert_eq!(fs::renumbered("Symbolic link loop (os error 32)", 32), None, "ELOOP is 40 on Linux and 62 on macOS, and worded otherwise");
    assert_eq!(fs::renumbered("No such file or directory", 44), None, "only the words std writes are renumbered");
    // natively nothing changes
    let e = std::fs::read("/no/such/file").unwrap_err();
    let words = e.to_string();
    assert_eq!(fs::as_native(e).to_string(), words);
}

/// git's version as `git version` prints it, held to 2.30, the oldest ritsu runs (DESIGN 17.12.1).
#[test]
fn the_version_of_git() {
    assert_eq!(fs::git_version("git version 2.50.1 (Apple Git-155)"), Some((2, 50)));
    assert_eq!(fs::git_version("git version 2.30.0.windows.1"), Some((2, 30)));
    assert_eq!(fs::git_version("git version 2.29.2"), Some((2, 29)));
    assert_eq!(fs::git_version("hub version 1"), None);
    assert_eq!(fs::GIT_OLDEST, (2, 30));
    for new in ["git version 2.30.0", "git version 2.56.0", "git version 3.0.0"] {
        assert!(fs::git_new_enough(new), "{new}");
    }
    for old in ["git version 2.29.2", "git version 1.99.0", "hub version 1", ""] {
        assert!(!fs::git_new_enough(old), "{old}");
    }
}

/// A file laid in a directory the disk does not have (what ringi lowers a request to): the directory
/// is there while the overlay is, it lists what is laid in it, a path that goes up out of it reads
/// the disk's file, and a path the disk has is read as it was. Laid under a symbolic link, a path
/// that goes up out of the directory and then out of the link reads the file the system would open
/// were the directory there, from where the link leads, not the one its letters name.
#[test]
fn an_overlay_makes_the_directory_of_what_it_lays() {
    let t = TempDir::new("overlay");
    t.write("shared/office.cal", "calendar office v1\n");
    let mut o = fs::Overlay::new(None);
    o.put(&t.path().join("expense.lowered/terms.cal"), "dates expense_terms v1\n");
    let o: Rc<dyn fs::Files> = Rc::new(o);
    fs::with(o, || {
        let lowered = t.path().join("expense.lowered");
        assert_eq!(fs::read_to_string(lowered.join("terms.cal")).unwrap(), "dates expense_terms v1\n");
        assert_eq!(fs::read_to_string(lowered.join("../shared/office.cal")).unwrap(), "calendar office v1\n");
        assert!(fs::is_dir(&lowered) && fs::is_file(lowered.join("terms.cal")) && !fs::exists(lowered.join("route.rule")));
        let mut top: Vec<String> = fs::read_dir(t.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        top.sort();
        assert_eq!(top, ["expense.lowered", "shared"]);
        let inside: Vec<String> = fs::read_dir(&lowered).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(inside, ["terms.cal"]);
        let real = std::fs::canonicalize(t.path()).unwrap();
        assert_eq!(fs::canonicalize(lowered.join("terms.cal")).unwrap(), real.join("expense.lowered/terms.cal"));
        assert_eq!(fs::canonicalize(lowered.join("../shared/office.cal")).unwrap(), real.join("shared/office.cal"));
        assert_eq!(fs::read(lowered.join("route.rule")).unwrap_err().kind(), std::io::ErrorKind::NotFound);
        assert_eq!(fs::read(t.path().join("missing/../shared/office.cal")).map(|_| ()).unwrap_err().kind(), std::io::ErrorKind::NotFound, "a directory the overlay does not make is not folded");
    });
    #[cfg(unix)]
    {
        // `in` leads to `common/in`: up out of it is `common`, where the letters say the top
        t.write("common/in/expense.ringi", "request expense v1\n");
        t.write("common/shared/office.cal", "calendar office v2\n");
        std::os::unix::fs::symlink("common/in", t.path().join("in")).unwrap();
        let mut o = fs::Overlay::new(None);
        o.put(&t.path().join("in/expense.lowered/terms.cal"), "dates expense_terms v1\n");
        let o: Rc<dyn fs::Files> = Rc::new(o);
        let real = std::fs::canonicalize(t.path()).unwrap();
        fs::with(o, || {
            let lowered = t.path().join("in/expense.lowered");
            assert_eq!(fs::read_to_string(lowered.join("terms.cal")).unwrap(), "dates expense_terms v1\n");
            let climbed = lowered.join("../../shared/office.cal");
            assert_eq!(fs::read_to_string(&climbed).unwrap(), "calendar office v2\n", "up out of the link is where it leads");
            assert!(fs::is_file(&climbed));
            assert_eq!(fs::canonicalize(&climbed).unwrap(), real.join("common/shared/office.cal"));
            assert_eq!(fs::read_to_string(lowered.join("../expense.ringi")).unwrap(), "request expense v1\n");
        });
    }
}

/// What a run asked of a symbolic link, where it leads (`fs::link`, which `paths::opened` asks), holds
/// while the link leads there, and not once it leads elsewhere or is a directory.
#[test]
fn what_was_asked_of_a_link_holds_while_it_leads_there() {
    #[cfg(unix)]
    fn relink(to: &str, at: &Path) {
        let _ = std::fs::remove_file(at);
        std::os::unix::fs::symlink(to, at).unwrap();
    }
    #[cfg(not(unix))]
    fn relink(_: &str, _: &Path) {}
    if !cfg!(unix) {
        ritsu_testkit::skip("the test makes symbolic links, which it makes on Unix only");
        return;
    }
    let t = TempDir::new("fs-link");
    t.write("a/x", "a");
    t.write("b/x", "b");
    let at = t.path().join("to");
    relink("a", &at);
    let (to, seen) = fs::watched(|| fs::link(&at));
    assert_eq!(to, Some(PathBuf::from("a")));
    assert!(seen.still());
    relink("b", &at);
    assert!(!seen.still());
    relink("a", &at);
    assert!(seen.still());
    std::fs::remove_file(&at).unwrap();
    std::fs::create_dir(&at).unwrap();
    assert!(!seen.still());
}

/// Within `steady` (DESIGN 4.15), the disk is asked each question once: a file, a directory and a
/// link changed on the disk behind this module's back read as they did, and so does a file that was
/// not there, in the same words; a check kept within holds there, and one that asks it notes what
/// it rests on. Once this module writes a file, every question is asked anew; once the steady run
/// ends, so is every question, and a check kept within no longer holds when its file has changed.
#[test]
fn within_steady_the_disk_is_asked_each_question_once() {
    if !cfg!(unix) {
        ritsu_testkit::skip("the test makes symbolic links, which it makes on Unix only");
        return;
    }
    let t = TempDir::new("fs-steady");
    let fee = t.write("rules/fee.rule", "rule fee v1\n");
    let late = t.path().join("rules/late.rule");
    let to = t.path().join("to");
    #[cfg(unix)]
    std::os::unix::fs::symlink("rules", &to).unwrap();
    let names = |d: &Path| -> Vec<String> { fs::read_dir(d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect() };
    let kept = fs::steady(|| {
        let (text, kept) = fs::watched(|| fs::read_to_string(&fee).unwrap());
        assert_eq!(text, "rule fee v1\n");
        let missing = fs::read(&late).unwrap_err().to_string();
        assert!(!fs::exists(&late));
        assert_eq!(names(&t.path().join("rules")), ["fee.rule"]);
        assert_eq!(fs::link(&to), Some(PathBuf::from("rules")));
        assert_eq!(fs::metadata(&fee).unwrap().len(), 12);
        let cwd = fs::current_dir().unwrap();
        // behind this module's back
        std::fs::write(&fee, "rule fee v2, longer\n").unwrap();
        std::fs::write(&late, "rule late v1\n").unwrap();
        std::fs::remove_file(&to).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("elsewhere", &to).unwrap();
        assert_eq!(fs::read_to_string(&fee).unwrap(), "rule fee v1\n");
        assert_eq!(fs::read(&fee).unwrap(), b"rule fee v1\n");
        assert_eq!(fs::metadata(&fee).unwrap().len(), 12);
        assert_eq!(fs::read(&late).unwrap_err().to_string(), missing);
        assert!(!fs::exists(&late));
        assert_eq!(names(&t.path().join("rules")), ["fee.rule"]);
        assert_eq!(fs::link(&to), Some(PathBuf::from("rules")));
        assert_eq!(fs::current_dir().unwrap(), cwd);
        assert!(kept.still(), "what the check read reads the same within");
        assert_eq!(fs::steady(|| fs::read_to_string(&fee).unwrap()), "rule fee v1\n", "a steady inside another takes its answers");
        // a check that asks whether another holds rests on what that one read, within too
        let ((), asking) = fs::watched(|| assert!(kept.still()));
        assert_eq!(reads(&asking), [(fee.clone(), Some(b"rule fee v1\n".to_vec()))]);
        // written through this module: every question asked anew
        fs::write(t.path().join("out.txt"), "x").unwrap();
        assert_eq!(fs::read_to_string(&fee).unwrap(), "rule fee v2, longer\n");
        assert!(fs::exists(&late));
        assert_eq!(fs::link(&to), Some(PathBuf::from("elsewhere")));
        assert!(!kept.still());
        kept
    });
    assert!(!kept.still());
    let (_, kept) = fs::steady(|| fs::watched(|| fs::read_to_string(&fee).unwrap()));
    assert!(kept.still());
    std::fs::write(&fee, "rule fee v3\n").unwrap();
    assert!(!kept.still(), "after the steady run, the disk is asked again");
    assert_eq!(fs::read_to_string(&fee).unwrap(), "rule fee v3\n");
    let mut listed = names(&t.path().join("rules"));
    listed.sort();
    assert_eq!(listed, ["fee.rule", "late.rule"]);
}
