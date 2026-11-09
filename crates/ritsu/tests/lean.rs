//! `ritsu check --lean` and `ritsu gen --target lean`, run with Lean (kaname's DESIGN 38): the example
//! `refund_split` checked with its proofs, a proof Lean does not take, and the package `gen` writes
//! building the proofs beside the modules where they are.
//!
//! They run `lake` of the toolchain `proofs/` uses, at the level that lets them; a SKIP line says
//! when they do not. proofs.yml runs them where Lean is installed.

use ritsu_testkit::{Need, TempDir, ready};
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `lake`, when the level lets the tests run it and it is there.
fn lake_ready() -> Option<PathBuf> {
    let found = kaname::lean::find_lake();
    let there = found.is_some();
    ready(Need::Lean, || there, "lake is not found (elan installs it, with the toolchain of proofs/)").then(|| found.unwrap())
}

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).current_dir(dir).args(args).env_remove("RITSU_LANG").env_remove("KANAME_LANG").output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The example `refund_split` (its two modules and their proofs), in a directory of its own.
fn example(what: &str) -> TempDir {
    let t = TempDir::new(what);
    ritsu_testkit::tmp::copy_dir(&here().join("../kaname/examples/refund_split"), t.path());
    t
}

/// With `--lean`, Lean takes the three proofs of each module, English and Japanese: the project
/// passes with none of the W603 it has without `--lean`, and the JSON says each property is shown.
#[test]
fn check_lean_shows_the_properties_of_the_example() {
    let Some(_) = lake_ready() else { return };
    let t = example("lean-example");
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(code, 0, "{out}");
    assert!(out.ends_with("ritsu check: 2 files (kaname 2): all pass (6 warnings); borders between the languages: 0 checked, 0 undecided\n"), "{out}");
    let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lean"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, "ok refund_split.ja.kn: 1 function, total\nok refund_split.kn: 1 function, total\nritsu check: 2 files (kaname 2): all pass; borders between the languages: 0 checked, 0 undecided\n");
    let (_, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lean", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
    for f in v["files"].as_array().unwrap() {
        let outcomes: Vec<&str> = f["check"]["properties"].as_array().unwrap().iter().map(|p| p["outcome"].as_str().unwrap()).collect();
        assert_eq!(outcomes, ["shown"; 3], "{f}");
    }
}

/// A proof Lean does not take fails the project with kaname's E604, at the theorem in the proof
/// file: here a proof by `sorry`.
#[test]
fn check_lean_turns_away_a_proof_by_sorry() {
    let Some(_) = lake_ready() else { return };
    let t = example("lean-sorry");
    let proof = t.path().join("refund_split.lean");
    let text = std::fs::read_to_string(&proof).unwrap();
    let (head, rest) = text.split_once("theorem no_share_is_negative : Kaname.refund_split.no_share_is_negative := by\n").unwrap();
    let (_, after) = rest.split_once("\n/--").unwrap();
    std::fs::write(&proof, format!("{head}theorem no_share_is_negative : Kaname.refund_split.no_share_is_negative := by\n  sorry\n\n/--{after}")).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lean"]);
    assert_eq!(code, 1, "{out}");
    // the Japanese module and its proof stand
    assert!(
        out.starts_with("ok refund_split.ja.kn: 1 function, total\nerror[kaname E604]: refund_split.lean:26:1: The proof of `no_share_is_negative` rests on `sorryAx`, besides `propext`, `Classical.choice` and `Quot.sound`\n"),
        "{out}"
    );
    assert!(out.ends_with("ritsu check: 2 files (kaname 2): 1 fail (1 error); borders between the languages: 0 checked, 0 undecided\n"), "{out}");
}

/// The package `ritsu gen --target lean` writes builds the proofs beside the modules, where they are
/// (kaname's DESIGN 38.5): the lakefile has a library for each proof file it found.
#[test]
fn gen_lean_builds_the_proofs_in_place() {
    let Some(lake) = lake_ready() else { return };
    let t = example("lean-gen");
    let (code, _, err) = ritsu_in(t.path(), &["gen", "--root", ".", "--target", "lean", "--out", "generated"]);
    assert_eq!(code, 0, "{err}");
    let lean = t.path().join("generated/lean");
    let o = Command::new(&lake).current_dir(&lean).arg("build").output().expect("cannot run lake");
    let said = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert!(o.status.success(), "lake build:\n{said}");
    for m in ["refund_split", "refund_split_ja"] {
        assert!(lean.join(format!(".lake/build/lib/lean/{m}.olean")).is_file(), "{m} is built:\n{said}");
    }
}
