//! An `enums` file is made into modules of types and nothing else (rulec's DESIGN 15.212), and
//! rulec's port answers only what is generated: no Connect service for it (rulec's DESIGN 15.258).
//! Every language that reads the port stops a file that uses an `enums` file as a rule's service or
//! reads an output of it, with its own diagnostic: dandori's `connect` (E005), a gate's and a
//! module's output of it (E101). sakai's published language is E302 (sakai's own mutants).
//!
//! The materials are `tests/projects/regions`, English first, the Japanese versions beside them.

use std::path::{Path, PathBuf};
use std::process::Command;

fn project() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/projects/regions")
}

/// `ritsu`, run in `dir`, with no language asked of the environment: its exit code, and what it
/// printed, stdout and stderr together.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu"))
        .current_dir(dir)
        .env_remove("RITSU_LANG")
        .env_remove("DANDORI_LANG")
        .env_remove("SEKISHO_LANG")
        .env_remove("KANAME_LANG")
        .args(args)
        .output()
        .expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

/// A workflow calls an `enums` file by `connect`: rulec says no service, and dandori cannot read
/// the rule (E005). It passed when the port answered the path of a service rulec never generates.
#[test]
fn a_workflow_cannot_call_an_enums_file_at_a_service() {
    for (flow, file) in [("connect.flow", "rules/regions.rule"), ("connect.ja.flow", "rules/配送の地域.rule")] {
        let (code, out) = ritsu_in(&project(), &["dandori", "check", flow]);
        assert_eq!(code, 1, "{flow}: {out}");
        assert!(out.starts_with(&format!("error[E005]: {flow}:4:10: could not read the rule `{file}`\n")), "{flow}: {out}");
        assert!(out.contains("  = rulec says nothing of the rule's Connect service, which `connect` calls\n"), "{flow}: {out}");
        let (code, ja) = ritsu_in(&project(), &["dandori", "check", flow, "--lang", "ja"]);
        assert_eq!(code, 1, "{flow}: {ja}");
        assert!(ja.contains(&format!("エラー[E005]: {flow}:4:10: 規則 `{file}` を読めませんでした\n")), "{flow}: {ja}");
        assert!(ja.contains("  = rulec から規則の Connect のサービスの情報を得られないので、`connect` で呼べません\n"), "{flow}: {ja}");
    }
}

/// A gate and a module read an output of an `enums` file as if it were a rule's: it has none, and
/// sekisho and kaname stop at E101.
#[test]
fn a_gate_and_a_module_find_no_output_in_an_enums_file() {
    for (gate, name, output) in [("region.gate", "regions", "region"), ("region.ja.gate", "地域", "地域")] {
        let (code, out) = ritsu_in(&project(), &["sekisho", "check", gate]);
        assert_eq!(code, 1, "{gate}: {out}");
        assert!(out.contains(&format!("error[E101]: {gate}:19:")) && out.contains(&format!("The rule `{name}` has no output `{output}`")), "{gate}: {out}");
        assert!(out.contains("  = No outputs are declared."), "{gate}: {out}");
        assert_eq!(out.matches("error[").count(), 1, "{gate}: {out}");
    }
    for (module, name, field) in [("region.kn", "regions", "region"), ("region.ja.kn", "地域", "地域")] {
        let (code, out) = ritsu_in(&project(), &["kaname", "check", module]);
        assert_eq!(code, 1, "{module}: {out}");
        assert!(out.contains(&format!("error[E101]: {module}:7:")) && out.contains(&format!("There is no field `{field}` in {name}.outputs")), "{module}: {out}");
        // and the enum's value the module compares with, which no rule it reads has
        let codes: Vec<&str> = out.lines().filter_map(|l| l.strip_prefix("error[")).filter_map(|l| l.split(']').next()).collect();
        assert!(!codes.is_empty() && codes.iter().all(|c| *c == "E101"), "{module}: {out}");
    }
}
