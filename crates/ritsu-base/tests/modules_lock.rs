//! `ritsu.lock` (DESIGN 17.4): written one way (its head, its format, the ritsu that wrote it, a line
//! a module in the order of the bytes of their names), read back to the same, E303 for what cannot
//! be read, and a stop for a format newer than this ritsu reads.

use ritsu_base::modules::lock::{self, Lock, Locked, Pinned};
use ritsu_base::modules::names::ModuleName;
use ritsu_base::modules::semver::Version;

const A: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const B: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";

fn name(s: &str) -> ModuleName {
    ModuleName::parse(s).unwrap()
}

fn oci(n: &str, v: &str) -> Locked {
    Locked { name: name(n), pinned: Pinned::Oci { version: Version::parse(v).unwrap(), digest: A.into(), files: B.into() }, line: 0 }
}

/// The code, the line and the English message of each diagnostic of a lock that does not read.
fn unreadable(text: &str) -> Vec<(&'static str, Option<usize>, String)> {
    match lock::parse(text.as_bytes(), "ritsu.lock") {
        Err(lock::Error::Unreadable(ds)) => ds.iter().map(|d| (d.code, d.line, d.message.en.clone())).collect(),
        other => panic!("{text}\n{other:?}"),
    }
}

#[test]
fn a_lock_is_written_in_one_way_and_read_back() {
    let mut l = Lock::new(vec![oci("example.com/acme/tax", "1.0.0"), Locked { name: name("pricing"), pinned: Pinned::Path { path: "../pricing".into() }, line: 0 }, oci("example.com/acme/pricing", "1.1.0")]);
    l.ritsu = Version::parse("0.37.0").unwrap();
    let text = l.text();
    assert_eq!(
        text,
        format!(
            "# Written by ritsu mod. Run `ritsu mod lock` to write it again; do not edit it by hand.\nlock 1\nritsu 0.37.0\noci example.com/acme/pricing 1.1.0 {A} files {B}\noci example.com/acme/tax 1.0.0 {A} files {B}\npath pricing \"../pricing\"\n"
        )
    );
    let back = lock::parse(text.as_bytes(), "ritsu.lock").unwrap();
    assert_eq!(back.text(), text);
    assert_eq!(back.get(&name("example.com/acme/tax")).map(|m| m.line), Some(5));
    assert_eq!(back.get(&name("example.com/acme/tax")).and_then(|m| m.version()).map(|v| v.to_string()), Some("1.0.0".into()));
    assert_eq!(back.get(&name("pricing")).map(|m| m.pinned.clone()), Some(Pinned::Path { path: "../pricing".into() }));
    // what this ritsu writes says this ritsu
    assert_eq!(Lock::new(vec![]).ritsu.to_string(), env!("CARGO_PKG_VERSION"));
    // comments and blank lines are passed over, CRLF read
    let crlf = "# hand\r\n\r\nlock 1\r\nritsu 0.36.0\r\n  # more\r\n".to_string();
    assert_eq!(lock::parse(crlf.as_bytes(), "ritsu.lock").unwrap().modules.len(), 0);
}

#[test]
fn a_newer_format_stops() {
    match lock::parse(b"lock 2\nritsu 9.0.0\nnew things\n", "ritsu.lock") {
        Err(e @ lock::Error::Newer(2)) => {
            assert_eq!(e.text().en, "`ritsu.lock` is in the format `lock 2`, and this ritsu reads `lock 1` at most: a newer ritsu wrote it, so run a newer ritsu");
            assert_eq!(e.text().ja, "`ritsu.lock` の書式は `lock 2` で、この ritsu が読めるのは `lock 1` までです。新しい ritsu が書いた lock なので、ritsu を新しくしてください");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn what_cannot_be_read_is_e303() {
    let digest_text = "`sha256:abc` is not a digest (`sha256:` and 64 lowercase hex digits)";
    let cases: Vec<(String, Vec<(&str, Option<usize>, String)>)> = vec![
        ("ritsu 0.37.0\n".into(), vec![("E303", Some(1), "`ritsu.lock` starts with the line `lock 1`".into())]),
        ("lock 0\n".into(), vec![("E303", Some(1), "there is no format `lock 0`; it is `lock 1`".into())]),
        ("lock one\n".into(), vec![("E303", Some(1), "the format `lock one` is not a number".into())]),
        ("lock 1\nlock 1\n".into(), vec![("E303", Some(2), "the line after `lock 1` is `ritsu <version>`".into())]),
        ("lock 1\n".into(), vec![("E303", None, "`ritsu.lock` has no line `ritsu <version>`".into())]),
        ("".into(), vec![("E303", None, "`ritsu.lock` has no line `lock 1`".into())]),
        (format!("lock 1\nritsu 0.37.0\noci example.com/a 1.0.0 sha256:abc files {B}\n"), vec![("E303", Some(3), digest_text.into())]),
        (format!("lock 1\nritsu 0.37.0\noci example.com/a v1.0.0 {A} files {B}\n"), vec![("E303", Some(3), "`v1.0.0` is not a version: write it with no `v` before it".into())]),
        (format!("lock 1\nritsu 0.37.0\noci pricing 1.0.0 {A} files {B}\n"), vec![("E303", Some(3), "`pricing` is not the name of a module in a registry".into())]),
        ("lock 1\nritsu 0.37.0\npath pricing \"/srv/pricing\"\n".into(), vec![("E303", Some(3), "`/srv/pricing` is not a path from the root of the module".into())]),
        ("lock 1\nritsu 0.37.0\npath pricing ../pricing\n".into(), vec![("E303", Some(3), "this line cannot be read: a line of `ritsu.lock` is `oci <name> <version> <digest> files <digest>` or `path <name> \"<path>\"`".into())]),
        (
            "lock 1\nritsu 0.37.0\npath pricing \"../a\"\npath pricing \"../b\"\n".into(),
            vec![("E303", Some(4), "`pricing` already has a line, line 3".into())],
        ),
    ];
    for (text, want) in cases {
        let got = unreadable(&text);
        let got: Vec<(&str, Option<usize>, String)> = got.into_iter().collect();
        assert_eq!(got, want, "{text}");
    }
    match lock::parse(b"lock 1\nritsu \xff\n", "ritsu.lock") {
        Err(lock::Error::Unreadable(ds)) => assert_eq!((ds[0].line, ds[0].message.en.as_str()), (Some(2), "there are bytes that are not UTF-8")),
        other => panic!("{other:?}"),
    }
}
