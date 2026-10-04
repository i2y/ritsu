//! Nothing in the Lean models is left open (DESIGN 11.4).
//!
//! A `sorry` would make every theorem above it worth nothing, and it is the one thing a reader
//! cannot see from the outside. Two tests hold `proofs/` to that. One reads every `.lean` of it for
//! `sorry`, `axiom`, `native_decide` and `implemented_by`. The other asks Lean: every declaration of
//! `ChoboModel`, `KoyomiModel` and `DandoriCore` — every theorem, and every definition the theorems
//! and `ritsu-model` stand on — is put to `collectAxioms`, what `#print axioms` prints, and may stand
//! only on the three axioms Lean itself stands on: `propext`, `Classical.choice`, `Quot.sound`. A
//! `sorry` shows there as `sorryAx`, a `native_decide` as `Lean.ofReduceBool`, an `axiom` by its
//! name, wherever a tactic hid it.

use ritsu_testkit::{Need, TempDir, ready, skip};
use std::path::{Path, PathBuf};
use std::process::Command;

fn proofs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proofs")
}

/// The libraries of the models, and the theorems each one exists for.
const LIBRARIES: &[&str] = &["ChoboModel", "KoyomiModel", "DandoriCore"];
const THEOREMS: &[&str] = &[
    "ChoboModel.apply_kept",
    "ChoboModel.pass_kept",
    "ChoboModel.within_stays",
    "ChoboModel.refused_keeps_balances",
    "ChoboModel.doneBefore_keeps_state",
    "ChoboModel.again_doneBefore",
    "KoyomiModel.monotone_of_adjacentOk",
    "KoyomiModel.monotone_on_range",
    "KoyomiModel.seek_following",
    "KoyomiModel.seek_preceding",
    "KoyomiModel.roll_open",
    "KoyomiModel.addBusiness_open",
    "KoyomiModel.ifClosed_open",
    "DandoriCore.exec_sound",
    "DandoriCore.chkFlow_sound",
];

fn lean_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != ".lake") {
                lean_files(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "lean") {
            out.push(p);
        }
    }
}

#[test]
fn no_proof_is_left_open() {
    let mut files = Vec::new();
    lean_files(&proofs(), &mut files);
    assert!(files.len() >= 15, "proofs/ has fewer .lean files than it did: {}", files.len());
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        for bad in ["sorry", "axiom ", "@[implemented_by", "native_decide"] {
            assert!(!text.contains(bad), "{}: `{bad}` is there, and leaves a proof open", f.display());
        }
    }
}

/// The program that asks Lean for every declaration of the libraries and what it stands on.
fn script() -> String {
    let mut s = String::from("import Lean\n");
    for l in LIBRARIES {
        s.push_str(&format!("import {l}\n"));
    }
    s.push_str(
        r#"open Lean Elab Command

#eval show CommandElabM Unit from do
  let env ← getEnv
  let libs : List Name := [LIBS]
  let mut n : Nat := 0
  for (name, info) in env.constants.toList do
    let some idx := env.getModuleIdxFor? name | continue
    let some m := env.header.moduleNames[idx.toNat]? | continue
    unless libs.any (fun l => l.isPrefixOf m) do continue
    let kind := match info with
      | .thmInfo _ => "theorem"
      | .defnInfo _ => "def"
      | .opaqueInfo _ => "opaque"
      | .axiomInfo _ => "axiom"
      | _ => ""
    if kind == "" then continue
    let axs ← collectAxioms name
    n := n + 1
    logInfo m!"standing {kind} {name}: {axs.toList}"
  logInfo m!"standing declarations: {n}"
"#,
    );
    s.replace("LIBS", &LIBRARIES.iter().map(|l| format!("`{l}")).collect::<Vec<_>>().join(", "))
}

#[test]
fn every_declaration_stands_on_the_three_axioms_lean_stands_on() {
    let built = proofs().join(".lake/build/lib/lean/DandoriCore.olean");
    if !ready(Need::Lean, || built.exists(), "proofs/ is not built (lake build in proofs/)") {
        return;
    }
    let lake = std::env::var("HOME").map(|h| PathBuf::from(h).join(".elan/bin/lake")).ok().filter(|p| p.exists());
    let Some(lake) = lake.or_else(|| {
        Command::new("sh")
            .args(["-c", "command -v lake"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().to_string()))
    }) else {
        skip("lake is not found");
        return;
    };
    let tmp = TempDir::new("model-axioms");
    let file = tmp.write("Standing.lean", script());
    let o = Command::new(&lake).current_dir(proofs()).args(["env", "lean", &file.to_string_lossy()]).output().expect("cannot run lake");
    let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{said}");
    let (mut theorems, mut count) = (Vec::new(), None);
    for line in said.lines() {
        let Some(rest) = line.split("standing ").nth(1) else { continue };
        if let Some(n) = rest.strip_prefix("declarations: ") {
            count = n.trim().parse::<usize>().ok();
            continue;
        }
        let (head, axioms) = rest.split_once(": [").unwrap_or_else(|| panic!("a line not of the form: {line}"));
        let (kind, name) = head.split_once(' ').unwrap();
        for ax in axioms.trim_end_matches(']').split(',').map(str::trim).filter(|a| !a.is_empty()) {
            assert!(["propext", "Classical.choice", "Quot.sound"].contains(&ax), "{kind} {name} stands on `{ax}`, not only on the three Lean stands on");
        }
        assert_ne!(kind, "axiom", "{name} is an axiom");
        if kind == "theorem" {
            theorems.push(name.to_string());
        }
    }
    let count = count.unwrap_or_else(|| panic!("Lean did not say how many declarations it read:\n{said}"));
    assert!(count >= 1500, "the libraries have fewer declarations than they did: {count}");
    for t in THEOREMS {
        assert!(theorems.iter().any(|x| x == t), "{t} is not among the theorems Lean read");
    }
    println!("{count} declarations, {} of them theorems, stand on propext, Classical.choice and Quot.sound at most", theorems.len());
}
