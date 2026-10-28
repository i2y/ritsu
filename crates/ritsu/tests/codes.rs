//! ritsu's own codes (DESIGN 4.3, 7.1; PLAN E.3): `ritsu explain` looks them up, and every
//! code's reproduction — the files of a small project — printed by `ritsu check .`, with the
//! code's headline carrying `ritsu`.

use ritsu_testkit::TempDir;
use std::path::Path;
use std::process::Command;

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).current_dir(dir).env_remove("RITSU_LANG").args(args).output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Every reproduction prints its code, in English and in Japanese.
#[test]
fn every_reproduction_prints_its_code() {
    let l = ritsu_cross::codes::ledger();
    let scratch = TempDir::new("codes");
    let failures = ritsu_base::ledger::check_every(&l, scratch.path(), |e, dir| {
        let ritsu_base::ledger::Repro::Dir { command, .. } = &e.repro else { return vec![] };
        let mut codes = Vec::new();
        for lang in ["en", "ja"] {
            let mut args: Vec<&str> = command[1..].to_vec();
            args.extend(["--lang", lang]);
            let (_, out, err) = ritsu_in(dir, &args);
            for l in out.lines().chain(err.lines()) {
                if let Some(rest) = l.split_once("[ritsu ").map(|(_, r)| r)
                    && let Some((code, _)) = rest.split_once(']')
                {
                    codes.push(code.to_string());
                }
            }
        }
        codes.dedup();
        codes
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The reproductions of the borders call the rule, the date or the transfer from a module of kaname
/// too, and the module's call prints the code beside the workflow's, in English and in Japanese.
/// E204 has none: a transfer a module builds leaves its refusals to the host, and they go on the
/// type kaname generates for it; W204 comes to a module only when chobo cannot answer (kaname's
/// DESIGN 14.4). E206 and W206 are of a workflow's waits, which a module does not have.
#[test]
fn the_reproductions_of_the_borders_call_from_a_module_too() {
    const WITH_A_MODULE: [&str; 8] = ["E201", "W201", "E202", "W202", "E203", "W203", "E205", "W205"];
    let l = ritsu_cross::codes::ledger();
    let scratch = TempDir::new("codes-kaname");
    let mut wrong = Vec::new();
    for e in &l.entries {
        let ritsu_base::ledger::Repro::Dir { files, .. } = &e.repro else { continue };
        let module = files.iter().find(|(n, _)| n.ends_with(".kn")).map(|(n, _)| *n);
        let Some(kn) = module else {
            if WITH_A_MODULE.contains(&e.code) {
                wrong.push(format!("{}: the reproduction has no module", e.code));
            }
            continue;
        };
        if !WITH_A_MODULE.contains(&e.code) {
            wrong.push(format!("{}: a module in a reproduction this test does not expect one in", e.code));
            continue;
        }
        let dir = scratch.path().join(e.code);
        l.lay_out(e, &dir).unwrap();
        for lang in ["en", "ja"] {
            let (_, out, _) = ritsu_in(&dir, &["check", ".", "--lang", lang]);
            if !out.lines().any(|line| line.contains(&format!("[ritsu {}]: {kn}:", e.code))) {
                wrong.push(format!("{} ({lang}): {kn} does not print the code:\n{out}", e.code));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// `ritsu explain` looks up ritsu's codes, and says where a language's code is looked up.
#[test]
fn ritsu_explain() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (code, out, err) = ritsu_in(here, &["explain", "E101"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with("E101 (error) — A file that does not read as a .proto\n"), "{out}");
    let (code, out, _) = ritsu_in(here, &["explain", "e101", "--lang", "ja"]);
    assert!(code == 0 && out.starts_with("E101 (エラー) — .proto として読めないファイル\n"), "{out}");
    let (code, out, _) = ritsu_in(here, &["explain", "--all", "--format", "markdown"]);
    assert_eq!(code, 0);
    assert_eq!(out, std::fs::read_to_string(here.join("../ritsu-cross/docs/codes.md")).unwrap());
    let (code, out, _) = ritsu_in(here, &["explain", "--all", "--format", "json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v[0]["code"], "E101");
    assert_eq!(v[0]["repro"]["command"], serde_json::json!(["ritsu", "check", "."]));
    let (code, _, err) = ritsu_in(here, &["explain", "E032"]);
    assert_eq!(code, 2);
    assert_eq!(err, "error: ritsu has no code `E032`; a language's code is looked up with `ritsu <language> explain E032` (`ritsu explain --all` lists ritsu's)\n");
    let (code, _, err) = ritsu_in(here, &["explain"]);
    assert!(code == 2 && err.contains("`ritsu explain` needs a code such as `E101`, or `--all`"), "{err}");
}

/// `ritsu explain` looks up the codes of the modules (DESIGN 17.20) as it does the others: in
/// English and in Japanese, as text, as the section of the pages of codes, and as JSON with the
/// files of its reproduction, which hold a dependency from a registry and its lock.
#[test]
fn ritsu_explain_a_code_of_the_modules() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (code, out, err) = ritsu_in(here, &["explain", "E304"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with("E304 (error) — `ritsu.lock` does not match `ritsu.mod`\n"), "{out}");
    assert!(out.contains("\nFix: Run `ritsu mod lock` to write the lock again.\n"), "{out}");
    assert!(out.contains("    require tax oci \"example.com/acme/tax\" 1.2.0\n"), "{out}");
    let (code, out, _) = ritsu_in(here, &["explain", "E304", "--lang", "ja"]);
    assert!(code == 0 && out.starts_with("E304 (エラー) — `ritsu.lock` が `ritsu.mod` に合いません\n"), "{out}");
    assert!(out.contains("\n直し方: `ritsu mod lock` を走らせて、lock を書き直してください。\n"), "{out}");
    for (lang, page) in [("en", "codes.md"), ("ja", "codes.ja.md")] {
        let (code, out, _) = ritsu_in(here, &["explain", "E304", "--format", "markdown", "--lang", lang]);
        assert_eq!(code, 0);
        assert!(out.starts_with("<a id=\"e304\"></a>\n"), "{out}");
        let page = std::fs::read_to_string(here.join("../ritsu-cross/docs").join(page)).unwrap();
        assert!(page.contains(&out), "the section of E304 ({lang}) is the one of the page of codes:\n{out}");
    }
    let (code, out, _) = ritsu_in(here, &["explain", "E304", "--format", "json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!((v["code"].as_str(), v["severity"].as_str()), (Some("E304"), Some("error")));
    assert_eq!(v["repro"]["command"], serde_json::json!(["ritsu", "check", "."]));
    let paths: Vec<&str> = v["repro"]["files"].as_array().unwrap().iter().filter_map(|f| f["path"].as_str()).collect();
    assert_eq!(paths, ["ritsu.mod", "ritsu.lock", "fee.rule"]);
    assert_eq!(v["related"], serde_json::json!(["E303", "W302"]));
}
