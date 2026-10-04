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
