//! The Lean kaname writes for its modules (DESIGN 38.4), run by Lean: the definitions it writes
//! answer what the reference interpreter answers, the prelude's lemmas rest on Lean's three axioms
//! alone, and the package it writes builds the proofs beside the modules where they are. Then
//! `--lean` (DESIGN 38.5, 38.6): it takes a proof of the property, and turns away each way of
//! breaking one, a way at a time, by the stage the test names; and it proves the examples'
//! properties.
//!
//! They run `lake` of the toolchain `proofs/` uses, at the level that lets them; a SKIP line says
//! when they do not. What the scan stops before anything is built needs no Lean, and is held
//! everywhere.

mod common;

use common::targets;
use kaname::lean::{BreachKind, LeanOptions, Rejection, Verdict};
use ritsu_base::text::Lang;
use ritsu_testkit::{Need, TempDir, ready};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// `lake`: on the `PATH`, else elan's.
fn lake() -> Option<PathBuf> {
    let on_path = std::env::var_os("PATH").and_then(|ps| std::env::split_paths(&ps).map(|d| d.join("lake")).find(|p| p.is_file()));
    on_path.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".elan/bin/lake")).filter(|p| p.is_file()))
}

/// `lake`, when the level lets the tests run it and it is there.
fn lake_ready() -> Option<PathBuf> {
    let found = lake();
    let there = found.is_some();
    ready(Need::Lean, || there, "lake is not found (elan installs it, with the toolchain of proofs/)").then(|| found.unwrap())
}

fn write(dir: &Path, files: &[(PathBuf, String)]) {
    for (rel, text) in files {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
    }
}

/// What a command printed, both streams, and whether it succeeded.
fn run(c: &mut Command) -> (bool, String) {
    let o = c.output().expect("cannot run lake");
    (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

/// The directory a test writes its package in: under `KANAME_KEEP` when it names one, else a
/// directory that goes with the test.
fn place(test: &str) -> (Option<TempDir>, PathBuf) {
    match targets::keep_in_for(test) {
        Some(d) => (None, d),
        None => {
            let t = TempDir::new(test);
            let p = t.path().to_path_buf();
            (Some(t), p)
        }
    }
}

/// The modules the comparison runs: every module the tests of the generated code run (the examples
/// and `tests/modules/`), and those of `tests/lean/`, which reach the ways numbers are held that the
/// others do not.
fn modules() -> Vec<Rc<kaname::check::Checked>> {
    let mut out: Vec<Rc<kaname::check::Checked>> = targets::modules().into_iter().map(|(c, _)| c).collect();
    for (name, src) in LEAN_MODULES {
        let o = common::check_text(&format!("{name}.kn"), src);
        let c = o.analysed.clone().unwrap_or_else(|| panic!("{name}: {}", common::shown(&o, ritsu_base::text::Lang::En)));
        out.push(c);
    }
    out
}

/// The modules only these tests run (not in `tests/modules/`, whose count the generated code's tests
/// and the models' comparisons say).
const LEAN_MODULES: [(&str, &str); 2] = [("numbers", include_str!("lean/numbers.kn")), ("numbers.ja", include_str!("lean/numbers.ja.kn"))];

/// The Lean definitions of every function of the modules that stops, evaluated on every vector of
/// its parameters that the reference interpreter answers, answer what it answers (PLAN 12.2.6): the
/// core of the feature, that what is proved is what runs. The rules and dates are given to `Inner`
/// as tables of what they answered the interpreter. Every function that stops and uses nothing not
/// written for Lean is written, and every vector is written as Lean. Lean builds what kaname wrote
/// with no warning, so that what `lake build` says of a proof is not lost among them (DESIGN 38.4).
#[test]
fn lean_definitions_agree_with_the_interpreter() {
    let Some(lake) = lake_ready() else { return };
    targets::deep(move || {
        let given = modules();
        let cmp = kaname::r#gen::lean::comparison(&given, &common::joined());
        // every module written: those given and those they read
        let shared = ["Kaname/Prelude.lean", "Kaname/Rules.lean", "Kaname/Books.lean"].map(Path::new);
        let written = cmp.package.iter().filter(|(p, _)| p.starts_with("Kaname") && !shared.contains(&p.as_path())).count();
        let (_keep, dir) = place("lean_definitions_agree_with_the_interpreter");
        write(&dir, &cmp.package);
        let files: Vec<(PathBuf, String)> = cmp.files.iter().map(|(_, p, t, _)| (p.clone(), t.clone())).collect();
        write(&dir, &files);
        assert!(cmp.unwritten.is_empty(), "total functions the writer did not write:\n{}", cmp.unwritten.join("\n"));
        assert!(cmp.left.is_empty(), "vectors the comparison could not write:\n{}", cmp.left.join("\n"));
        let (ok, said) = run(Command::new(&lake).current_dir(&dir).args(["build", "Kaname"]));
        assert!(ok, "lake build Kaname:\n{said}");
        let warned: Vec<&str> = said.lines().filter(|l| l.starts_with("warning:")).collect();
        assert!(warned.is_empty(), "Lean warns of the Lean kaname writes:\n{}", warned.join("\n"));
        // each module's file, a few at a time
        let width = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 6);
        let mut outputs: Vec<(String, bool, String)> = Vec::new();
        for group in cmp.files.chunks(width) {
            let children: Vec<(String, std::process::Child)> = group
                .iter()
                .map(|(alias, path, _, _)| {
                    let child = Command::new(&lake)
                        .current_dir(&dir)
                        .args(["env", "lean", "--run", &path.to_string_lossy()])
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .spawn()
                        .expect("cannot run lake");
                    (alias.clone(), child)
                })
                .collect();
            for (alias, child) in children {
                let o = child.wait_with_output().expect("cannot run lake");
                outputs.push((alias, o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), if o.status.success() { String::new() } else { String::from_utf8_lossy(&o.stderr).to_string() })));
            }
        }
        let mut wrong = Vec::new();
        let mut agree = 0;
        for ((alias, _, _, expected), (_, ok, printed)) in cmp.files.iter().zip(&outputs) {
            if !ok {
                wrong.push(format!("{alias}: Lean stopped:\n{printed}"));
                continue;
            }
            // each value on a line of its own after `= `; anything else is what Lean said of the file
            let lines: Vec<&str> = printed.lines().filter_map(|l| l.strip_prefix("= ")).collect();
            let said: Vec<&str> = printed.lines().filter(|l| !l.starts_with("= ")).collect();
            if !said.is_empty() {
                wrong.push(format!("{alias}: Lean said:\n{}", said.join("\n")));
            }
            if lines.len() != expected.len() {
                wrong.push(format!("{alias}: {} lines printed, {} expected", lines.len(), expected.len()));
            }
            for (k, ((f, want), got)) in expected.iter().zip(&lines).enumerate() {
                if want == got {
                    agree += 1;
                } else {
                    wrong.push(format!("{alias}.{f} (evaluation {k}): Lean {got}, the reference interpreter {want}"));
                }
            }
        }
        let evaluations: usize = cmp.compared.iter().map(|c| c.evaluations).sum();
        let mut modules_compared: Vec<&str> = cmp.compared.iter().map(|c| c.module.as_str()).collect();
        modules_compared.dedup();
        println!(
            "Lean: {agree} of {evaluations} evaluations agree with the reference interpreter, of {} functions in {} modules ({} modules written); {} expressions held as a count of a finer step, {} as a fraction; {} vectors of functions that can fail failed in the reference interpreter and are not compared",
            cmp.compared.len(),
            modules_compared.len(),
            written,
            cmp.stats.scaled,
            cmp.stats.fractions,
            cmp.failed
        );
        assert!(wrong.is_empty(), "{} disagree:\n{}", wrong.len(), wrong.iter().take(60).cloned().collect::<Vec<_>>().join("\n"));
        assert!(cmp.stats.scaled >= 1 && cmp.stats.fractions >= 1, "both ways of holding numbers are compared: {:?}", cmp.stats);
    });
}

/// The lemmas of the prelude a proof leans on (PLAN 12.2.6, DESIGN 38.4).
const LEMMAS: [&str; 12] = [
    "sum_nonneg",
    "sum_pos",
    "sum_mul_left",
    "allocate_self",
    "allocate_zero",
    "allocate_mono",
    "allocate_sub_le",
    "runningTotals_telescope",
    "runningTotals_length",
    "runningTotals_items",
    "runningTotals_total",
    "mem_runningTotals",
];

/// Each lemma of the prelude is proved, on `propext`, `Classical.choice` and `Quot.sound` alone.
#[test]
fn lean_the_prelude_rests_on_the_three_axioms() {
    let Some(lake) = lake_ready() else { return };
    let (_keep, dir) = place("lean_the_prelude_rests_on_the_three_axioms");
    let (files, _) = kaname::r#gen::lean::package(&[], &kaname::r#gen::lean::Layout::Temp);
    write(&dir, &files);
    let mut axioms = String::from("import Kaname.Prelude\n");
    for l in LEMMAS {
        axioms.push_str(&format!("#print axioms Kaname.{l}\n"));
    }
    std::fs::write(dir.join("Axioms.lean"), axioms).unwrap();
    let (ok, said) = run(Command::new(&lake).current_dir(&dir).args(["build", "Kaname"]));
    assert!(ok, "lake build Kaname:\n{said}");
    let (ok, said) = run(Command::new(&lake).current_dir(&dir).args(["env", "lean", "Axioms.lean"]));
    assert!(ok, "{said}");
    for l in LEMMAS {
        let line = said.lines().find(|x| x.starts_with(&format!("'Kaname.{l}'"))).unwrap_or_else(|| panic!("no line of {l}:\n{said}"));
        let rest = line.split_once("axioms:").map(|(_, r)| r).unwrap_or("");
        for a in rest.trim().trim_start_matches('[').trim_end_matches(']').split(',').map(str::trim).filter(|a| !a.is_empty()) {
            assert!(["propext", "Classical.choice", "Quot.sound"].contains(&a), "{l} rests on {a}:\n{said}");
        }
    }
}

/// The package written with the proofs where they are (`Layout::InPlace`) builds them there: the
/// library of each reads the directory of its module by a path from the package that goes up
/// (`srcDir = "../…"`), and Lean takes the proof beside the module.
#[test]
fn lean_the_generated_package_builds_the_proofs_in_place() {
    let Some(lake) = lake_ready() else { return };
    let (_keep, root) = place("lean_the_generated_package_builds_the_proofs_in_place");
    // the module and its proof, beside each other, away from the package
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/properties");
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    for f in ["in_place.kn", "in_place.lean"] {
        std::fs::copy(here.join(f), src.join(f)).unwrap();
    }
    let out = root.join("generated/lean");
    targets::deep(move || {
        let o = common::check(&src.join("in_place.kn").to_string_lossy());
        let c = o.analysed.clone().unwrap_or_else(|| panic!("{}", common::shown(&o, ritsu_base::text::Lang::En)));
        let proofs = kaname::r#gen::lean::proofs_beside(std::slice::from_ref(&c));
        assert_eq!(proofs.len(), 1, "the proof beside the module is found");
        let (files, _) = kaname::r#gen::lean::package(&[c], &kaname::r#gen::lean::Layout::InPlace { out: out.clone(), proofs });
        let lakefile = &files.iter().find(|(p, _)| p == Path::new("lakefile.toml")).unwrap().1;
        assert!(lakefile.contains("srcDir = \"../../src\""), "{lakefile}");
        write(&out, &files);
        let (ok, said) = run(Command::new(&lake).current_dir(&out).arg("build"));
        assert!(ok, "lake build:\n{said}");
        assert!(out.join(".lake/build/lib/lean/in_place.olean").is_file(), "the proof is built:\n{said}");
    });
}

// ── `--lean` (DESIGN 38.5, 38.6) ────────────────────────────────────────────

/// The module the proofs below are of (`tests/properties/in_place.kn`): `twice_is_even`, twice a
/// number from 0 to 1000 is even.
const MODULE: &str = include_str!("properties/in_place.kn");

/// Its proof, which Lean takes.
const PROOF: &str = include_str!("properties/in_place.lean");

/// The tests that run `--lean` run side by side; the one that stops a build past its time looks
/// for what is left of its own run among the processes, and runs alone.
static LEAN: std::sync::RwLock<()> = std::sync::RwLock::new(());

fn side_by_side() -> std::sync::RwLockReadGuard<'static, ()> {
    LEAN.read().unwrap_or_else(|e| e.into_inner())
}

/// `--lean` as the commands run it, the proof file scanned first.
fn scanned() -> LeanOptions {
    LeanOptions::default()
}

/// `--lean` without the scan, which only the tests run: what the stages after it stop.
fn unscanned() -> LeanOptions {
    LeanOptions { scan: false, ..LeanOptions::default() }
}

/// The module checked in `dir`, with `proof` beside it as its proof file.
fn module_with(dir: &Path, proof: &str) -> Rc<kaname::check::Outcome> {
    std::fs::write(dir.join("in_place.kn"), MODULE).unwrap();
    std::fs::write(dir.join("in_place.lean"), proof).unwrap();
    let o = common::check(&dir.join("in_place.kn").to_string_lossy());
    assert!(o.analysed.is_some(), "{}", common::shown(&o, Lang::En));
    o
}

/// What `--lean` says of the property of the module in `dir` with `proof` beside it.
fn verdict_at(dir: &Path, proof: &str, opts: &LeanOptions) -> Verdict {
    let o = module_with(dir, proof);
    let got = kaname::lean::check(&[o.analysed.clone().unwrap()], opts).unwrap_or_else(|e| panic!("{}", e.text().get(Lang::En)));
    assert_eq!(got.len(), 1, "{got:?}");
    got.into_iter().next().unwrap().1
}

/// The same, in a directory of its own named `name` (`KANAME_KEEP` keeps it).
fn verdict(name: &str, proof: &str, opts: &LeanOptions) -> Verdict {
    let (_keep, dir) = place(name);
    verdict_at(&dir, proof, opts)
}

/// What the scan stopped a proof file at: the kind, and the words.
fn breach(v: &Verdict) -> (BreachKind, &str) {
    match v {
        Verdict::Rejected(Rejection::Breach(b)) => (b.kind, b.what.as_str()),
        other => panic!("not stopped by the scan: {other:?}"),
    }
}

/// The axioms beyond Lean's three that Lean says a proof rests on, as E604 names them.
fn rests_on(v: &Verdict) -> Vec<String> {
    match v {
        Verdict::Rejected(Rejection::RestsOn(axioms)) => kaname::lean::axiom_names(axioms),
        other => panic!("not turned away for its axioms: {other:?}"),
    }
}

/// The proof with `lines` before its theorem.
fn before_the_theorem(lines: &str) -> String {
    PROOF.replacen("theorem twice_is_even", &format!("{lines}\n\ntheorem twice_is_even"), 1)
}

/// The proof with its theorem's tactics replaced by `tactics`.
fn proved_by(tactics: &str) -> String {
    let (head, _) = PROOF.split_once(":= by\n").unwrap();
    format!("{head}:= by\n{tactics}\n\nend Proofs.in_place\n")
}

/// A proof that proves the property is taken: the theorem's type is the statement, and it rests on
/// no axiom but Lean's three.
#[test]
fn lean_takes_a_proof_that_proves_the_property() {
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    let v = verdict("lean_takes_a_proof_that_proves_the_property", PROOF, &scanned());
    let Verdict::Shown(axioms) = &v else { panic!("{v:?}") };
    assert!(axioms.iter().all(|a| ["propext", "Classical.choice", "Quot.sound"].contains(&a.as_str())), "{axioms:?}");
}

/// A theorem of another statement is turned away, with its type: what a theorem proves is read
/// from Lean's environment, not from the text (stage 6).
#[test]
fn lean_turns_away_a_changed_statement() {
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    let proof = PROOF.replacen("theorem twice_is_even : Kaname.in_place.twice_is_even := by\n  intro x _ _\n  unfold twice\n  omega", "theorem twice_is_even : ∀ (x : Int), 0 ≤ x → x ≤ 1000 → twice x % 2 = 0 ∨ True := by\n  intro x _ _\n  exact Or.inr trivial", 1);
    let v = verdict("lean_turns_away_a_changed_statement", &proof, &scanned());
    // its type, as Lean prints it
    assert!(matches!(&v, Verdict::Rejected(Rejection::StatesOtherwise(t)) if t.starts_with("∀ (x : Int), 0 ≤ x → x ≤ 1000 → ") && t.ends_with(" % 2 = 0 ∨ True")), "{v:?}");
}

/// The statement written open, as the `∀ …` it stands for, is turned away too (DESIGN 38.5): the
/// type of the theorem is the statement's constant itself (stage 6).
#[test]
fn lean_turns_away_the_opened_statement() {
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    let proof = PROOF.replacen("theorem twice_is_even : Kaname.in_place.twice_is_even := by", "theorem twice_is_even : ∀ (x : Int), 0 ≤ x → x ≤ 1000 → ((twice x) % 2) = 0 := by", 1);
    let v = verdict("lean_turns_away_the_opened_statement", &proof, &scanned());
    assert!(matches!(&v, Verdict::Rejected(Rejection::StatesOtherwise(t)) if t.starts_with("∀ (x : Int), 0 ≤ x → x ≤ 1000 → ") && t.ends_with(" % 2 = 0")), "{v:?}");
}

/// A proof by `sorry` is turned away for the axiom it rests on (stage 6), and `kaname check
/// --lean` says so at the theorem, with exit 1.
#[test]
fn lean_turns_away_sorry() {
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    let (_keep, dir) = place("lean_turns_away_sorry");
    let v = verdict_at(&dir, &proved_by("  sorry"), &scanned());
    assert_eq!(rests_on(&v), ["sorryAx"]);
    let o = Command::new(env!("CARGO_BIN_EXE_kaname")).current_dir(&dir).args(["check", "--lean", "in_place.kn"]).env_remove("KANAME_LANG").output().unwrap();
    let said = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert_eq!(o.status.code(), Some(1), "{said}");
    assert!(said.starts_with("error[E604]: in_place.lean:7:1: The proof of `twice_is_even` rests on `sorryAx`, besides `propext`, `Classical.choice` and `Quot.sound`\n"), "{said}");
    assert!(said.contains("= Write the proof in place of `sorry`."), "{said}");
}

/// A theorem that is an axiom of the proof file's own is turned away, the axiom named (stage 6).
#[test]
fn lean_turns_away_an_axiom_of_its_own() {
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    let proof = PROOF.replacen("theorem twice_is_even : Kaname.in_place.twice_is_even := by\n  intro x _ _\n  unfold twice\n  omega", "axiom every_number_is_even : Kaname.in_place.twice_is_even\n\ntheorem twice_is_even : Kaname.in_place.twice_is_even := every_number_is_even", 1);
    let v = verdict("lean_turns_away_an_axiom_of_its_own", &proof, &scanned());
    assert_eq!(rests_on(&v), ["Proofs.in_place.every_number_is_even"]);
}

/// `native_decide` is stopped by the scan, before the build runs compiled code; without the scan,
/// the axiom Lean makes of it turns the proof away, said as `native_decide`.
#[test]
fn lean_turns_away_native_decide() {
    let proof = proved_by("  have _h : (1 : Nat) + 1 = 2 := by native_decide\n  intro x _ _\n  unfold twice\n  omega");
    assert_eq!(breach(&verdict("lean_turns_away_native_decide", &proof, &scanned())), (BreachKind::Native, "native_decide"));
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    assert_eq!(rests_on(&verdict("lean_turns_away_native_decide-unscanned", &proof, &unscanned())), ["native_decide"]);
}

/// A theorem put into the environment with the kernel's check turned off, by a command that needs
/// `import Lean`: the scan stops the import; without the scan, the kernel turns the declaration away
/// when `leanchecker` replays it (stage 5), though its type is the statement.
#[test]
fn lean_turns_away_a_theorem_added_past_the_kernel() {
    let proof = "import Lean\nimport Kaname.in_place\nopen Lean Elab Command\n\n-- the theorem, its proof `True.intro`, put in with the kernel's check turned off\nset_option debug.skipKernelTC true in\nrun_cmd liftCoreM do\n  let decl := Declaration.thmDecl\n    { name := `Proofs.in_place.twice_is_even\n      levelParams := []\n      type := mkConst `Kaname.in_place.twice_is_even\n      value := mkConst ``True.intro }\n  addDecl decl\n";
    assert_eq!(breach(&verdict("lean_turns_away_a_theorem_added_past_the_kernel", proof, &scanned())), (BreachKind::Import, "import Lean"));
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    let v = verdict("lean_turns_away_a_theorem_added_past_the_kernel-unscanned", proof, &unscanned());
    assert!(matches!(&v, Verdict::Rejected(Rejection::Kernel(said)) if said.contains("twice_is_even")), "{v:?}");
}

/// A proof file that does not read the module written from the `.kn`, and declares its own
/// statement under the statement's name, is turned away: the two modules cannot be read together
/// (stage 6).
#[test]
fn lean_turns_away_a_statement_defined_again() {
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    let proof = "-- Does not read the module written from the `.kn`: declares its own statement under its name.\nnamespace Kaname.in_place\ndef twice_is_even : Prop := True\nend Kaname.in_place\n\ntheorem Proofs.in_place.twice_is_even : Kaname.in_place.twice_is_even := trivial\n";
    let v = verdict("lean_turns_away_a_statement_defined_again", proof, &scanned());
    assert!(matches!(&v, Verdict::Rejected(Rejection::Load(said)) if said.contains("Kaname.in_place.twice_is_even")), "{v:?}");
}

/// `macro_rules` is stopped by the scan: a macro can make commands the text does not show, here a
/// `#print axioms` that says what it is told. Without the scan, the axioms are read from the
/// environment, which the macro does not reach, and the proof by `sorry` is turned away (stage 6).
#[test]
fn lean_turns_away_macro_rules() {
    let proof = "import Kaname.in_place\n\n-- `#print axioms` says what it is told, in every file that reads this one\nmacro_rules\n  | `(#print axioms $_) => `(#eval IO.println \"'Proofs.in_place.twice_is_even' does not depend on any axioms\")\n\nnamespace Proofs.in_place\n\ntheorem twice_is_even : Kaname.in_place.twice_is_even := by\n  sorry\n\nend Proofs.in_place\n";
    assert_eq!(breach(&verdict("lean_turns_away_macro_rules", proof, &scanned())), (BreachKind::Command, "macro_rules"));
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    assert_eq!(rests_on(&verdict("lean_turns_away_macro_rules-unscanned", proof, &unscanned())), ["sorryAx"]);
}

/// A command that runs code in the build, given code that writes a file: the scan stops it, and the
/// file is not there. It is never built without the scan.
fn runs_nothing(test: &str, command: &str, what: &str) {
    let (_keep, dir) = place(test);
    let ran = dir.join("ran.txt");
    let proof = before_the_theorem(&command.replace("RAN", &ran.to_string_lossy()));
    assert_eq!(breach(&verdict_at(&dir, &proof, &scanned())), (BreachKind::Command, what));
    assert!(!ran.exists(), "{} was written", ran.display());
}

#[test]
fn lean_turns_away_eval() {
    runs_nothing("lean_turns_away_eval", "#eval IO.FS.writeFile \"RAN\" \"ran\"", "#eval");
}

#[test]
fn lean_turns_away_run_cmd() {
    runs_nothing("lean_turns_away_run_cmd", "run_cmd (IO.FS.writeFile \"RAN\" \"ran\" : IO Unit)", "run_cmd");
}

#[test]
fn lean_turns_away_initialize() {
    runs_nothing("lean_turns_away_initialize", "initialize IO.FS.writeFile \"RAN\" \"ran\"", "initialize");
}

/// An import of a module that is not written from a `.kn` is stopped by the scan: `Lean` gives a
/// proof file the elaborator's own commands.
#[test]
fn lean_turns_away_an_import_outside_kaname() {
    for (k, import) in ["import Lean", "import Std.Data.HashMap"].into_iter().enumerate() {
        let proof = PROOF.replacen("import Kaname.in_place", &format!("import Kaname.in_place\n{import}"), 1);
        assert_eq!(breach(&verdict(&format!("lean_turns_away_an_import_outside_kaname-{k}"), &proof, &scanned())), (BreachKind::Import, import));
    }
}

/// An attribute that installs code is stopped by the scan: `implemented_by`, which has compiled code
/// answer other than the definition, and `macro`, which rewrites a tactic. Without the scan, what
/// they lead to is turned away for its axioms (stage 6): the compiled answer reaches a proof only
/// through `native_decide`, and the rewritten `decide` is `sorry`.
#[test]
fn lean_turns_away_an_attribute() {
    let implemented_by = proved_by("  have _h : Proofs.in_place.answer = true := by native_decide\n  intro x _ _\n  unfold twice\n  omega").replacen(
        "theorem twice_is_even",
        "def yes : Bool := true\n\n@[implemented_by yes] def answer : Bool := false\n\ntheorem twice_is_even",
        1,
    );
    let macro_ = proved_by("  decide").replacen("theorem twice_is_even", "@[macro Lean.Parser.Tactic.decide] def anything : Lean.Macro := fun _ => `(tactic| sorry)\n\ntheorem twice_is_even", 1);
    assert_eq!(breach(&verdict("lean_turns_away_an_attribute-0", &implemented_by, &scanned())), (BreachKind::Attribute, "@[implemented_by]"));
    assert_eq!(breach(&verdict("lean_turns_away_an_attribute-1", &macro_, &scanned())), (BreachKind::Attribute, "@[macro]"));
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    assert_eq!(rests_on(&verdict("lean_turns_away_an_attribute-2", &implemented_by, &unscanned())), ["native_decide"]);
    assert_eq!(rests_on(&verdict("lean_turns_away_an_attribute-3", &macro_, &unscanned())), ["sorryAx"]);
}

/// `set_option debug.skipKernelTC` is stopped by the scan. What it lets into the environment, the
/// kernel's replay turns away (stage 5): `lean_turns_away_a_theorem_added_past_the_kernel` adds its
/// theorem with this option.
#[test]
fn lean_turns_away_an_option() {
    let proof = before_the_theorem("set_option debug.skipKernelTC true");
    assert_eq!(breach(&verdict("lean_turns_away_an_option", &proof, &scanned())), (BreachKind::Option, "set_option debug.skipKernelTC"));
}

/// `Lean.ofReduceBool`, which has the kernel take what compiled code answers, is stopped by the
/// scan. Without the scan, the kernel's replay turns the proof away (stage 5): `leanchecker` has no
/// compiled code to run, and stops at the declaration that asks it to.
#[test]
fn lean_turns_away_reduce_bool() {
    let proof = proved_by("  have _h : Proofs.in_place.yes = true := Lean.ofReduceBool Proofs.in_place.yes true rfl\n  intro x _ _\n  unfold twice\n  omega").replacen("theorem twice_is_even", "def yes : Bool := true\n\ntheorem twice_is_even", 1);
    assert_eq!(breach(&verdict("lean_turns_away_reduce_bool", &proof, &scanned())), (BreachKind::Native, "Lean.ofReduceBool"));
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    let v = verdict("lean_turns_away_reduce_bool-unscanned", &proof, &unscanned());
    assert!(matches!(&v, Verdict::Rejected(Rejection::Kernel(said)) if said.contains("Proofs.in_place.yes")), "{v:?}");
}

/// A theorem of the property's name in another namespace is no proof of it: Lean finds no
/// `Proofs.in_place.twice_is_even`, and the property is not shown (W603, which names the theorem
/// it looks for).
#[test]
fn lean_turns_away_a_theorem_in_another_namespace() {
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    let (_keep, dir) = place("lean_turns_away_a_theorem_in_another_namespace");
    let proof = PROOF.replace("namespace Proofs.in_place", "namespace Proofs.elsewhere").replace("end Proofs.in_place", "end Proofs.elsewhere");
    assert_eq!(verdict_at(&dir, &proof, &scanned()), Verdict::NoTheorem);
    let o = module_with(&dir, &proof);
    let answered = kaname::check::with_lean(&[o], &scanned()).unwrap();
    assert_eq!(common::codes(&answered[0]), ["W603"]);
    let w = answered[0].diags[0].render(Lang::En);
    assert!(w.contains("The property `twice_is_even` held on all 2 vectors, but it is not shown: in_place.lean holds no theorem `Proofs.in_place.twice_is_even`"), "{w}");
}

/// A `def` of the theorem's name, though of the statement's type, is turned away: a proof is a
/// theorem (stage 6).
#[test]
fn lean_turns_away_a_def() {
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    let proof = PROOF.replacen("theorem twice_is_even", "def twice_is_even", 1);
    let v = verdict("lean_turns_away_a_def", &proof, &scanned());
    assert_eq!(v, Verdict::Rejected(Rejection::NotATheorem("def".into())));
}

/// A build longer than the time given is stopped (stage 4), with what it started: no process is
/// left in a directory of `--lean`'s, and the directory is gone.
#[test]
fn lean_stops_a_build_past_the_time_limit() {
    let Some(_) = lake_ready() else { return };
    let _alone = LEAN.write().unwrap_or_else(|e| e.into_inner());
    let slow = before_the_theorem("set_option maxHeartbeats 0 in\nset_option maxRecDepth 100000 in\ntheorem slow : ∀ a b c : Fin 64, a + b + c = a + (b + c) := by decide");
    let limit = Duration::from_secs(1);
    let start = Instant::now();
    let v = verdict("lean_stops_a_build_past_the_time_limit", &slow, &LeanOptions { timeout: limit, scan: true });
    assert_eq!(v, Verdict::Rejected(Rejection::Timeout(limit)));
    assert!(start.elapsed() < Duration::from_secs(30), "{:?}", start.elapsed());
    let mark = format!("kaname-lean-{}-", std::process::id());
    let left = || -> Vec<String> {
        let o = Command::new("ps").args(["axww", "-o", "pid=,command="]).output().unwrap();
        String::from_utf8_lossy(&o.stdout).lines().filter(|l| l.contains(&mark)).map(str::to_string).collect()
    };
    let mut still = left();
    for _ in 0..50 {
        if still.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
        still = left();
    }
    assert!(still.is_empty(), "left running: {still:?}");
    let dirs: Vec<String> = std::fs::read_dir(std::env::temp_dir()).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with(&mark)).collect();
    assert!(dirs.is_empty(), "{dirs:?}");
}

/// `--lean` writes the module's Lean again from the `.kn` (stage 2): the statement of the package
/// `ritsu gen --target lean` wrote, made `True` by hand, lets a proof of `True` build there, and
/// `--lean` does not take it.
#[test]
fn lean_ignores_edits_to_the_generated_package() {
    let Some(lake) = lake_ready() else { return };
    let _g = side_by_side();
    let (_keep, dir) = place("lean_ignores_edits_to_the_generated_package");
    let proof = proved_by("  trivial");
    let o = module_with(&dir, &proof);
    let c = o.analysed.clone().unwrap();
    let out = dir.join("generated/lean");
    let proofs = kaname::r#gen::lean::proofs_beside(std::slice::from_ref(&c));
    let (files, _) = kaname::r#gen::lean::package(std::slice::from_ref(&c), &kaname::r#gen::lean::Layout::InPlace { out: out.clone(), proofs });
    write(&out, &files);
    let written = out.join("Kaname/in_place.lean");
    let text = std::fs::read_to_string(&written).unwrap();
    let (head, rest) = text.split_once("abbrev twice_is_even : Prop :=\n").unwrap();
    let tail: String = rest.lines().skip_while(|l| l.starts_with("  ")).map(|l| format!("{l}\n")).collect();
    std::fs::write(&written, format!("{head}abbrev twice_is_even : Prop := True\n{tail}")).unwrap();
    let (ok, said) = run(Command::new(&lake).current_dir(&out).arg("build"));
    assert!(ok, "the edited package builds the proof of `True`:\n{said}");
    let got = kaname::lean::check(&[c], &scanned()).unwrap();
    assert!(matches!(&got[0].1, Verdict::Rejected(Rejection::Build { line: 8, .. })), "{got:?}");
}

/// What Lean says of a proof file that does not build is E604 at the place in the proof file, its
/// path that of the proof file beside the module, not the one `--lean` built (stage 4).
#[test]
fn lean_maps_build_errors_to_the_proof_file() {
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    let (_keep, dir) = place("lean_maps_build_errors_to_the_proof_file");
    let o = module_with(&dir, &proved_by("  intro x _ _\n  exact no_such_lemma x"));
    let answered = kaname::check::with_lean(&[o], &scanned()).unwrap();
    assert_eq!(common::codes(&answered[0]), ["E604"], "{}", common::shown(&answered[0], Lang::En));
    let d = &answered[0].diags[0];
    assert_eq!((d.file.as_str(), d.line, d.col), (dir.join("in_place.lean").to_string_lossy().as_ref(), Some(9), Some(9)));
    let e = d.render(Lang::En);
    assert!(e.contains("The proof file in_place.lean does not build, so it does not prove `twice_is_even`") && e.contains("no_such_lemma") && !e.contains("Proofs/in_place.lean"), "{e}");
    let j = d.render(Lang::Ja);
    assert!(j.contains("証明のファイル in_place.lean はビルドが通らないので、`twice_is_even` を示せません"), "{j}");
}

/// Every example checked with `--lean`, the twenty modules at once as `ritsu check --lean` sends a
/// project (PLAN 12.2.8): each property's answer, and why one is undecided, as
/// `tests/golden/lean/examples.txt` holds them (`KANAME_BLESS=1` writes it again). Lean takes every
/// proof beside them; what it says nothing of is the one property that rests on a rule's answers,
/// which is W603 still.
#[test]
fn lean_proves_the_properties_of_the_examples() {
    let Some(_) = lake_ready() else { return };
    let _g = side_by_side();
    let outcomes: Vec<Rc<kaname::check::Outcome>> = common::EXAMPLES.iter().map(|f| common::check(f)).collect();
    let start = Instant::now();
    let answered = kaname::check::with_lean(&outcomes, &scanned()).unwrap_or_else(|e| panic!("{}", e.text().get(Lang::En)));
    eprintln!("--lean on the {} modules of the examples: {:?}", answered.len(), start.elapsed());
    let mut text = String::new();
    for (f, o) in common::EXAMPLES.iter().zip(&answered) {
        let c = o.analysed.as_ref().unwrap_or_else(|| panic!("{f}"));
        for p in kaname::properties::to_json(c, &o.properties).as_arr().unwrap() {
            let s = |k: &str| p.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let why = p.get("reason").and_then(|x| x.as_str()).map(|r| format!(" ({r}, {})", s("code"))).unwrap_or_default();
            text.push_str(&format!("{f}: {}: {}{why}\n", s("name"), s("outcome")));
        }
        let undecided = o.properties.iter().filter(|r| !matches!(r.outcome, kaname::properties::Outcome::Shown)).count();
        assert_eq!(common::codes(o), vec!["W603"; undecided], "{f}:\n{}", common::shown(o, Lang::En));
    }
    ritsu_testkit::golden("tests/golden/lean/examples.txt", &text);
}
