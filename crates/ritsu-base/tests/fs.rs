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
