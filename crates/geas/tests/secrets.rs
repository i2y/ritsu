//! W901 (ritsu's DESIGN 16.3): `geas check` says a key written in a spec, in a string or in a
//! comment, with its kind, its prefix and its length, and never with the key nor its line; a key
//! whose line says `ritsu: test secret` is not said. The claims run as they always do. What it
//! prints for each spec of `tests/secrets`, in English and in Japanese, is its golden file in
//! `tests/golden/<lang>/secrets`. Each spec holds one key that is said.

mod common;
use common::*;

fn specs() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(root().join("tests/secrets")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn a_key_is_said_without_the_key_and_a_test_secret_is_not_said() {
    // the materials' one fake key, put together here so that this file does not hold it whole
    let key = ["AIzaSyD-ritsu-fake-", "key-for-tests-000000"].concat();
    let names = specs();
    assert_eq!(names.len(), 6, "{names:?}");
    for name in &names {
        let s = Scratch::new("secrets");
        s.write(name, std::fs::read_to_string(root().join("tests/secrets").join(name)).unwrap());
        let log = pid_log(&s);
        let log_s = log.to_str().expect("a UTF-8 path");
        let stem = name.trim_end_matches(".geas");
        for lang in ["en", "ja"] {
            let (out, err, code) = run(s.path(), &["check", name, "--lang", lang], &[("GEAS_PID_LOG", log_s)]);
            assert_eq!((err.as_str(), code), ("", 0), "{name} ({lang})");
            assert!(!out.contains(&key), "{name} ({lang}) prints the key:\n{out}");
            assert_eq!(out.matches("[W901]").count(), 1, "{name} ({lang}):\n{out}");
            golden(&format!("{lang}/secrets/{stem}.txt"), &out);
        }
        no_process_left(&log);
    }
}

/// `--json` puts the keys in `diagnostics`, beside the claims; a spec without one has
/// `"diagnostics":[]`, so that the shape a reader reads does not change with the spec.
#[test]
fn the_json_always_has_the_diagnostics() {
    let s = Scratch::new("secrets-json");
    let name = "W901_key_in_a_comment.geas";
    s.write(name, std::fs::read_to_string(root().join("tests/secrets").join(name)).unwrap());
    let (out, err, code) = run(s.path(), &["check", name, "--json"], &[]);
    assert_eq!((err.as_str(), code), ("", 0));
    let v = json(out.trim_end());
    let d = v.get("diagnostics").arr();
    assert_eq!(d.len(), 1, "{out}");
    assert_eq!(d[0].get("code").str(), "W901");
    assert_eq!(d[0].get("line").num(), 2.0);
    s.write("plain.geas", "target lookup {\n  run \"echo\"\n}\n\nclaim \"passes the address on\" {\n  when lookup.run(\"x\")\n  then stdout is \"x\"\n}\n");
    let (out, _, code) = run(s.path(), &["check", "plain.geas", "--json"], &[]);
    assert_eq!(code, 0);
    assert!(json(out.trim_end()).get("diagnostics").arr().is_empty(), "{out}");
    assert!(out.trim_end().ends_with("],\"diagnostics\":[]}"), "{out}");
}

/// A key in a check that fails is masked in the report and in the JSON: the check, the line quoted,
/// and what the run got (ritsu-base's `secrets::mask`).
#[test]
fn a_key_in_a_failing_check_is_masked() {
    let key = ["AIzaSyD-ritsu-fake-", "key-for-tests-000000"].concat();
    let s = Scratch::new("secrets-mask");
    // echo prints the key and `x`, which is not what the check says
    s.write("w.geas", format!("target lookup {{\n  run \"echo\"\n}}\n\nclaim \"answers with the key\" {{\n  when lookup.run(\"{key}\", \"x\")\n  then stdout is \"{key}\"\n}}\n"));
    for lang in ["en", "ja"] {
        let (out, err, code) = run(s.path(), &["check", "w.geas", "--lang", lang], &[]);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        assert!(out.contains("not ok 1"), "{out}");
        assert!(!out.contains(&key), "{lang}:\n{out}");
        assert!(out.contains("AIza…"), "{out}");
    }
    let (out, _, code) = run(s.path(), &["check", "w.geas", "--json"], &[]);
    assert_eq!(code, 1);
    assert!(!out.contains(&key), "{out}");
    // a claim that cannot run: its command, which holds the key, is in the notes of its error
    s.write("x.geas", format!("target lookup {{\n  run \"geas-test-no-such-program --key {key}\"\n}}\n\nclaim \"answers\" {{\n  when lookup.run(\"x\")\n  then exit is 0\n}}\n"));
    for args in [&["check", "x.geas"][..], &["check", "x.geas", "--json"][..], &["check", "x.geas", "--lang", "ja"][..]] {
        let (out, err, code) = run(s.path(), args, &[]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("E030"), "{out}");
        assert!(!out.contains(&key) && !err.contains(&key), "{args:?}:\n{out}{err}");
    }
}
