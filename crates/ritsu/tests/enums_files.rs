//! An `enums` file is made into modules of types and nothing else (rulec's DESIGN 15.212), and
//! rulec's port answers only what is generated: no Connect service for it (rulec's DESIGN 15.258).
//! Every language that reads the port stops a file that calls an `enums` file as a rule or reads an
//! output of it, with its own diagnostic: dandori's call of it, however `use rule` says to call it
//! (E005), a gate's and a module's output of it (E101). A workflow that uses its enums as types
//! alone passes. sakai's published language is E302 (sakai's own mutants).
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

/// A workflow calls an `enums` file as a rule, at its Connect service (`connect`) or with the rule's
/// code going with the workflow: an `enums` file has no outputs, and nothing is generated from it to
/// call, so dandori stops the call where it is made (E005), and not the `use rule` line. The call
/// at the service passed while the port answered the path of a service rulec never generates; the
/// call of the code passed as long as dandori wrote the call the port answers for a function rulec
/// never generates (the port still answers it).
#[test]
fn a_workflow_cannot_call_an_enums_file() {
    let flows = [("connect.flow", 14, "regions"), ("connect.ja.flow", 14, "地域"), ("bundled.flow", 13, "regions"), ("bundled.ja.flow", 13, "地域")];
    for (flow, line, name) in flows {
        let enum_type = if name == "regions" { "regions.region" } else { "地域.地域" };
        let (code, out) = ritsu_in(&project(), &["dandori", "check", flow]);
        assert_eq!(code, 1, "{flow}: {out}");
        assert!(out.starts_with(&format!("error[E005]: {flow}:{line}:11: the rule `{name}` has no outputs, so it cannot be called\n")), "{flow}: {out}");
        assert!(
            out.contains(&format!("  = a rule with no outputs is an `enums` file, which holds only enums for rules to read, and rulec generates no function from it to call; use its enums as types, as in `{enum_type}`\n")),
            "{flow}: {out}"
        );
        assert_eq!(out.matches("error[").count(), 1, "{flow}: {out}");
        let (code, ja) = ritsu_in(&project(), &["dandori", "check", flow, "--lang", "ja"]);
        assert_eq!(code, 1, "{flow}: {ja}");
        assert!(ja.starts_with(&format!("エラー[E005]: {flow}:{line}:11: 規則 `{name}` には出力が無いので、呼べません\n")), "{flow}: {ja}");
        assert!(
            ja.contains(&format!("  = 出力の無い規則は `enums` のファイルで、中にあるのは規則が読む列挙だけです。rulec は、このファイルから呼べる関数を生成しません。列挙は `{enum_type}` のように型として使ってください\n")),
            "{flow}: {ja}"
        );
    }
}

/// A workflow that uses the enums of an `enums` file as types, and calls nothing of it, passes, and
/// builds for every platform: the code generated for it holds the enums as types of its own, and
/// imports nothing of the code rulec generates.
#[test]
fn a_workflow_uses_the_enums_of_an_enums_file_as_types() {
    let tmp = ritsu_testkit::TempDir::new("enums-files");
    for flow in ["types.flow", "types.ja.flow"] {
        let (code, out) = ritsu_in(&project(), &["dandori", "check", flow]);
        assert_eq!((code, out.as_str()), (0, format!("{flow}: ok\n").as_str()));
        for target in ["asl", "temporal", "temporal-python", "temporal-go", "durable", "argo", "pydantic-graph"] {
            let dir = tmp.path().join(format!("{flow}-{target}"));
            let (code, out) = ritsu_in(&project(), &["dandori", "build", flow, "--target", target, "--out", dir.to_str().unwrap()]);
            assert_eq!(code, 0, "{flow} for {target}: {out}");
            let mut stack = vec![dir];
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d).unwrap().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else {
                        let text = std::fs::read_to_string(&p).unwrap_or_default();
                        assert!(!text.contains("rulec"), "{flow} for {target}: {} reads the code rulec generates", p.display());
                    }
                }
            }
        }
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
