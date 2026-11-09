//! The properties of a module (DESIGN 38.3): each run on the vectors of its parameters in the
//! reference interpreter, as a function is, and said as P5 says an answer.
//!
//! - E603 where a vector breaks one: on it the `when` lines are `true` and the last line is `false`.
//!   The first such vector in the vectors' order is said, with the values of the `let` lines.
//! - W605 where the `when` lines are `true` on none of its vectors: a property that holds of no input
//!   can be proved whatever it says.
//! - W604 where it uses what is not written for Lean (`r#gen::lean::obstacle`): what, and where.
//! - W603 where it is not shown: no proof is beside the module, or the proof file holds no theorem of
//!   it; one is, and `--lean` did not run; or it calls rules or dates, whose answers a proof knows
//!   only as far as the ports say.
//! - E604 where `--lean` ran and Lean does not take the proof (`crate::lean`): why. What is wrong
//!   with the file as a whole (it does not build, it holds what a proof file may not) is said once,
//!   naming the properties it was to prove.
//!
//! A property gets one of E603, E604, W604 and W603, and W605 beside it. Shown (no diagnostic) comes
//! only from `--lean`, when Lean takes the proof.

use crate::ast::ExprKind;
use crate::check::{Checked, CheckedProperty};
use crate::diag::Diag;
use crate::interp::{Bound, Interp, PropertyRun};
use crate::lean::{Rejection, Verdict};
use crate::r#gen::lean::{self, Obstacle};
use crate::suite::Suite;
use crate::types::Ref;
use crate::value::{Program, Value};
use ritsu_base::json::Json;
use ritsu_base::text::{Text, plural};
use std::path::{Path, PathBuf};

/// What the vectors and the files beside the module say of one property.
#[derive(Clone, Debug)]
pub struct PropertyReport {
    /// Its place among the module's properties.
    pub property: usize,
    /// How many vectors it was run on, and on how many of them its `when` lines are `true`.
    pub vectors: usize,
    pub held: usize,
    pub outcome: Outcome,
    /// What Lean said of it, when `--lean` sent it to Lean.
    pub lean: Option<Verdict>,
}

/// The answer of P5 for one property.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// Lean takes the proof beside the module (`--lean`).
    Shown,
    /// The first vector that breaks it: the input, and the values of the property's `let` lines.
    Example { input: Vec<Value>, lets: Vec<Bound> },
    Undecided(Undecided),
}

/// Why a property that no vector breaks is not shown.
#[derive(Clone, Debug)]
pub enum Undecided {
    /// No proof of the module is beside it.
    NoProof,
    /// The proof file beside the module writes a theorem of it, and `--lean` did not run.
    LeanNotRun,
    /// It has no proof and calls rules or dates (by the names the module reads them by), whose
    /// answers a proof knows only as far as the ports say.
    Inner { rules: Vec<String>, dates: Vec<String> },
    /// It uses what is not written for Lean.
    NotInLean(Obstacle),
    /// The proof file holds no theorem of it: as its text reads without `--lean`, and as Lean reads
    /// the file that builds with it.
    NoTheorem,
    /// `--lean` ran, and the writer wrote no statement of it (why, as the Lean's comment says it).
    NotWritten(String),
    /// `--lean` ran, and Lean does not take the proof (E604).
    Rejected(Rejection),
}

/// The file a module's proofs are in (DESIGN 38.5): `<the module's alias>.lean` beside the `.kn`.
pub fn proof_path(c: &Checked) -> PathBuf {
    let dir = Path::new(&c.path).parent().unwrap_or(Path::new(""));
    dir.join(format!("{}.lean", c.module.name.ascii()))
}

/// Each property of `c` run on its vectors, in the order written.
pub fn run(c: &Checked, suite: &Suite) -> Vec<PropertyReport> {
    let Some(full) = &c.lowered else { return Vec::new() };
    let p = Program::of(full);
    // the proof file read as text: whether it writes a theorem of a property, before `--lean` asks Lean
    let proof = ritsu_base::fs::read_to_string(proof_path(c)).ok();
    let alias = c.module.name.ascii().to_string();
    c.properties
        .iter()
        .enumerate()
        .map(|(k, cp)| {
            let (vs, _) = crate::vectors::choose(&p, full, cp.lowered);
            let mut held = 0;
            let mut example = None;
            for v in &vs {
                let mut it = Interp::new(&p, suite);
                if it.check_entry(full, cp.lowered, &v.input).is_err() {
                    continue;
                }
                // a property that can fail is E401 already: a vector it fails on says nothing more
                match it.property(full, cp.lowered, v.input.clone()) {
                    Ok(PropertyRun::Holds) => held += 1,
                    Ok(PropertyRun::False { lets }) => {
                        held += 1;
                        if example.is_none() {
                            example = Some((v.input.clone(), lets));
                        }
                    }
                    Ok(PropertyRun::Vacuous) | Err(_) => {}
                }
            }
            let outcome = match example {
                Some((input, lets)) => Outcome::Example { input, lets },
                None => {
                    let written = proof.as_deref().is_some_and(|t| crate::lean::theorem_at(t, &alias, cp.decl.name.ascii()).is_some());
                    Outcome::Undecided(undecided(&p, full, cp, proof.is_some(), written))
                }
            };
            PropertyReport { property: k, vectors: vs.len(), held, outcome, lean: None }
        })
        .collect()
}

/// Why a property no vector breaks is not shown, without `--lean`: whether there is a proof file
/// (`proof`), and whether its text writes a theorem of the property (`written`).
fn undecided(p: &Program, full: &Checked, cp: &CheckedProperty, proof: bool, written: bool) -> Undecided {
    if let Some(o) = lean::obstacle(p, full, cp.lowered) {
        return Undecided::NotInLean(o);
    }
    if written {
        return Undecided::LeanNotRun;
    }
    let (rules, dates) = inner_calls(p, full, cp.lowered);
    if !rules.is_empty() || !dates.is_empty() {
        Undecided::Inner { rules, dates }
    } else if proof {
        Undecided::NoTheorem
    } else {
        Undecided::NoProof
    }
}

/// The reports of `c` with what Lean said of the properties `--lean` sent it (DESIGN 38.6), by their
/// places among the module's properties. A counterexample stands whatever Lean says; a property no
/// vector breaks is shown when Lean takes its proof, and has E604 when Lean does not. A proof file
/// that holds no theorem of a property leaves it as one with no proof is.
pub fn with_verdicts(c: &Checked, reports: &[PropertyReport], verdicts: &[(usize, Verdict)]) -> Vec<PropertyReport> {
    let Some(full) = &c.lowered else { return reports.to_vec() };
    let p = Program::of(full);
    reports
        .iter()
        .map(|r| {
            let mut r = r.clone();
            let Some((_, v)) = verdicts.iter().find(|(k, _)| *k == r.property) else { return r };
            r.lean = Some(v.clone());
            if matches!(r.outcome, Outcome::Example { .. } | Outcome::Undecided(Undecided::NotInLean(_))) {
                return r;
            }
            r.outcome = match v {
                Verdict::Shown(_) => Outcome::Shown,
                Verdict::Rejected(x) => Outcome::Undecided(Undecided::Rejected(x.clone())),
                Verdict::NotWritten(why) => Outcome::Undecided(Undecided::NotWritten(why.clone())),
                Verdict::NoTheorem => {
                    let (rules, dates) = inner_calls(&p, full, c.properties[r.property].lowered);
                    Outcome::Undecided(if rules.is_empty() && dates.is_empty() { Undecided::NoTheorem } else { Undecided::Inner { rules, dates } })
                }
            };
            r
        })
        .collect()
}

/// The rules and the dates the function `f` of `m` calls, and the functions it calls do, by the names
/// the modules read them by (`charges`, `terms.due`), in the order met.
pub fn inner_calls(p: &Program, m: &Checked, f: usize) -> (Vec<String>, Vec<String>) {
    let (mut rules, mut dates) = (Vec::new(), Vec::new());
    for (c, g) in lean::reached(p, m, f) {
        let mut es = Vec::new();
        c.module.fns[g].body.walk(&mut es);
        for e in es {
            let ExprKind::Call(callee, _) = &e.kind else { continue };
            let shown = callee.path().map(|ws| ws.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>().join(".")).unwrap_or_default();
            match c.typed.refs.get(callee.id) {
                Some(Ref::Rule(_)) if !rules.contains(&shown) => rules.push(shown),
                Some(Ref::Date(..)) if !dates.contains(&shown) => dates.push(shown),
                _ => {}
            }
        }
    }
    (rules, dates)
}

/// The diagnostics of the properties of `c`, which passes everything else (DESIGN 38.3).
pub fn check(c: &Checked, suite: &Suite) -> Vec<Diag> {
    let reports = run(c, suite);
    diags(c, &reports)
}

/// `` `a` ``, as a list of names in a message: `` `a`, `b` and `c` ``.
fn names(ns: &[String]) -> Text {
    let parts: Vec<Text> = ns.iter().map(|n| tr!("`{n}`", "`{n}`")).collect();
    Text::list(&parts)
}

/// What a proof file that is not taken as a whole does not prove: "does not prove `a`", "proves none
/// of `a` and `b`".
fn none_of(ns: &[String]) -> Text {
    let all = names(ns);
    if ns.len() == 1 {
        tr!("{} を示せません", "does not prove {}", all.ja; all.en)
    } else {
        tr!("{} のどれも示せません", "proves none of {}", all.ja; all.en)
    }
}

/// Why a proof file may not hold what the scan found (DESIGN 38.5).
fn breach_note(kind: crate::lean::BreachKind) -> Text {
    use crate::lean::BreachKind;
    match kind {
        BreachKind::Import => tr!(
            "証明のファイルが import できるのは、.kn から書いたモジュール（`Kaname.…`）だけです。",
            "A proof file imports only the modules written from the .kn files (`Kaname.…`)."
        ),
        BreachKind::Command => tr!(
            "ビルドの中でコードを走らせるか、ファイルの残りの読み方を変える構文を足すものです。証明にはどちらも要りません。",
            "It runs code while the file is built, or adds syntax that changes how the rest of the file is read; a proof needs neither."
        ),
        BreachKind::Native => tr!(
            "ビルドの中でコンパイルしたコードを走らせるもので、その答えはカーネルが確かめません。カーネルが確かめる `decide` を使ってください。",
            "It has the build run compiled code, whose answer the kernel does not check: use `decide`, which the kernel checks."
        ),
        BreachKind::Attribute => tr!(
            "属性は、ビルドの中で走るコードを足せます。証明のファイルに書ける属性は `simp`・`grind`・`reducible`・`irreducible`・`instance` だけです。",
            "An attribute can install code that runs in the build: a proof file gives only `simp`, `grind`, `reducible`, `irreducible` and `instance`."
        ),
        BreachKind::Option => tr!(
            "証明のファイルで設定できるのは、`maxHeartbeats`・`maxRecDepth` と、`synthInstance.`・`linter.`・`pp.`・`trace.`（`trace.profiler` を除く）で始まるオプションだけです。",
            "A proof file sets only `maxHeartbeats`, `maxRecDepth` and the options under `synthInstance.`, `linter.`, `pp.` and `trace.` (but `trace.profiler`)."
        ),
        BreachKind::Interpolation => tr!(
            "コマンドによっては、文字列の `{{` と `}}` のあいだを、走らせるコードとして読みます。証明のファイルには `{{` を含む文字列を書けません。",
            "Some commands read what is between `{{` and `}}` in a string as code to run: a proof file holds no string with `{{` in it."
        ),
    }
}

/// What `--lean` takes, said under each E604.
fn takes_note(alias: &str, kn: &str) -> Text {
    tr!(
        "`--lean` は、{kn} から書き直した Lean に対して証明をビルドし、性質ごとに、型が `Kaname.{alias}.<性質>` で、`propext`・`Classical.choice`・`Quot.sound` のほかの公理に頼らない定理 `Proofs.{alias}.<性質>` を受け付けます。",
        "`--lean` builds the proofs against the Lean written again from {kn}, and takes for each property a theorem `Proofs.{alias}.<property>` whose type is `Kaname.{alias}.<property>` and which rests on no axiom but `propext`, `Classical.choice` and `Quot.sound`."
    )
}

/// What the reports say: E603, E604, W604 or W603 for each property, and W605 beside it. What is
/// wrong with the proof file as a whole is said once, after the properties.
pub fn diags(c: &Checked, reports: &[PropertyReport]) -> Vec<Diag> {
    let Some(full) = &c.lowered else { return Vec::new() };
    let p = Program::of(full);
    let proof = proof_path(c);
    let proof_name = proof.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let proof_shown = proof.to_string_lossy().to_string();
    let alias = c.module.name.ascii().to_string();
    let kn = Path::new(&c.path).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let proof_text = std::cell::OnceCell::new();
    let text = || proof_text.get_or_init(|| ritsu_base::fs::read_to_string(&proof).unwrap_or_default()).as_str();
    let mut of_file: Vec<(Rejection, Vec<String>)> = Vec::new();
    let mut out = Vec::new();
    for r in reports {
        let cp = &c.properties[r.property];
        let decl = &cp.decl;
        let name = &decl.name.text;
        let ascii = decl.name.ascii().to_string();
        let at = |code: &'static str, msg: Text| Diag::at(code, &c.path, decl.line, 1, msg).source(&c.src);
        let (n, held) = (r.vectors, r.held);
        let ran = if held == n {
            tr!("{n} 個のベクタのすべてで成り立ちました", "held on all {}", ; plural(n, "vector", "vectors"))
        } else {
            tr!("`when` の行が成り立つ {held} 個のベクタ（{n} 個のうち）のすべてで成り立ちました", "held on all {held} of its {n} vectors on which its `when` lines are true")
        };
        match &r.outcome {
            Outcome::Shown => {}
            Outcome::Example { input, lets } => {
                let info = &full.decls.fns[cp.lowered];
                let shown = info.params.iter().zip(input).map(|(prm, v)| format!("{}: {}", prm.name.text, p.show(full, v, &prm.slot.ty))).collect::<Vec<_>>().join(", ");
                let mut d = at("E603", tr!("性質 `{name}` は、{shown} で成り立ちません", "The property `{name}` is false for {shown}"));
                for b in lets {
                    let (line, ln, text) = (b.line, &b.name, &b.text);
                    d = d.note(tr!("{line} 行目: `{ln}` は {text}", "Line {line}: `{ln}` is {text}"));
                }
                d = d
                    .note(tr!(
                        "`check` は、性質を引数のベクタで参照インタプリタに流します。この入力では、`when` の行がどれも `true` で、最後の行が `false` になりました。",
                        "`check` runs a property in the reference interpreter on the vectors of its parameters: on this one the `when` lines hold and the last line is false."
                    ))
                    .note(tr!(
                        "関数を直すか、`when` の行か引数の範囲で性質を狭めるか、性質を直してください。",
                        "Correct the function, narrow the property with `when` or the ranges of its parameters, or correct the property."
                    ));
                if matches!(r.lean, Some(Verdict::Shown(_))) {
                    d = d.note(tr!(
                        "Lean は {proof_name} の証明を受け付けました。モジュールから書いた Lean と参照インタプリタの答えが食い違っていて、kaname の不具合です。報告してください。",
                        "Lean takes the proof of it in {proof_name}: the Lean written from the module and the reference interpreter disagree on it, which is a fault of kaname; please report it."
                    ));
                }
                out.push(d);
            }
            Outcome::Undecided(Undecided::NotInLean(o)) => {
                // a name in backquotes keeps a space from the word after it
                let what = Text { ja: if o.what.ja.ends_with('`') { format!("{} ", o.what.ja) } else { o.what.ja.clone() }, en: o.what.en.clone() };
                let place = o.place(&c.path);
                out.push(at(
                    "W604",
                    tr!(
                        "性質 `{name}` は、{}が、Lean に書けません。{}の{}は、まだ Lean に書き出しません",
                        "The property `{name}` {}, but it cannot be written for Lean: it uses {} ({}), which is not written for Lean yet",
                        ran.ja, place.ja, what.ja; ran.en, what.en, place.en
                    ),
                ));
            }
            Outcome::Undecided(Undecided::NotWritten(why)) => {
                out.push(at("W604", tr!("性質 `{name}` は、{}が、Lean に文を書けません（{why}）", "The property `{name}` {}, but its statement is not written for Lean: {why}", ran.ja; ran.en)));
            }
            Outcome::Undecided(Undecided::Rejected(x)) if x.of_the_file() => match of_file.iter_mut().find(|(y, _)| y == x) {
                Some((_, ns)) => ns.push(name.to_string()),
                None => of_file.push((x.clone(), vec![name.to_string()])),
            },
            Outcome::Undecided(Undecided::Rejected(x)) => {
                let (line, col) = crate::lean::theorem_at(text(), &alias, &ascii).unwrap_or((1, 1));
                let thm = format!("Proofs.{alias}.{ascii}");
                let stmt = format!("Kaname.{alias}.{ascii}");
                let d = match x {
                    Rejection::StatesOtherwise(ty) => {
                        let ty: String = if ty.chars().count() > 300 { format!("{}…", ty.chars().take(300).collect::<String>()) } else { ty.clone() };
                        Diag::at("E604", &proof_shown, line, col, tr!("定理 `{thm}` は性質 `{name}` を述べていません。型が `{stmt}` ではありません", "The theorem `{thm}` does not state the property `{name}`: its type is not `{stmt}`"))
                            .note(tr!("型: {ty}", "Its type: {ty}"))
                            .note(tr!(
                                "型には文の名前を `theorem {ascii} : {stmt}` と書いてください。文を書き下すと、文より弱いことを述べられてしまいます。",
                                "Write the statement's name as its type, `theorem {ascii} : {stmt}`, and not the statement written out, which can say less than it."
                            ))
                    }
                    Rejection::RestsOn(axs) => {
                        let shown = crate::lean::axiom_names(axs);
                        let all = names(&shown);
                        let fix = if shown.iter().any(|a| a == "sorryAx") {
                            tr!("`sorry` を証明に置き換えてください。", "Write the proof in place of `sorry`.")
                        } else if shown.iter().any(|a| a == "native_decide") {
                            tr!(
                                "`native_decide` はコンパイルしたコードで決めるので、その答えはカーネルが確かめません。カーネルが確かめる `decide` か、ほかのタクティクを使ってください。",
                                "`native_decide` decides with compiled code, whose answer the kernel does not check: use `decide`, which the kernel checks, or another tactic."
                            )
                        } else {
                            tr!("公理は仮定で、証明ではありません。その中身を定理として証明してください。", "An axiom is assumed, not proved: prove what it says as a theorem.")
                        };
                        Diag::at(
                            "E604",
                            &proof_shown,
                            line,
                            col,
                            tr!(
                                "`{name}` の証明は、`propext`・`Classical.choice`・`Quot.sound` のほかに {} に頼っています",
                                "The proof of `{name}` rests on {}, besides `propext`, `Classical.choice` and `Quot.sound`",
                                all.ja; all.en
                            ),
                        )
                        .note(fix)
                    }
                    Rejection::NotATheorem(kind) => Diag::at("E604", &proof_shown, line, col, tr!("`{thm}` は、定理ではなく `{kind}` として宣言されています", "`{thm}` is declared as `{kind}`, not as a theorem"))
                        .note(tr!("`theorem` で宣言してください。", "Declare it with `theorem`.")),
                    Rejection::Elsewhere(module) => Diag::at(
                        "E604",
                        &proof_shown,
                        line,
                        col,
                        tr!("定理 `{thm}` は、{proof_name} ではなく `{module}` で宣言されています", "The theorem `{thm}` is declared in `{module}`, not in {proof_name}"),
                    ),
                    _ => unreachable!("said of the file"),
                };
                out.push(d.source(text()).note(takes_note(&alias, &kn)));
            }
            Outcome::Undecided(u) => {
                let why = match u {
                    Undecided::NoProof => tr!(
                        "証明がありません（{proof_name} に、この性質の定理がありません）",
                        "there is no proof of it ({proof_name} holds no theorem of it)"
                    ),
                    Undecided::NoTheorem => tr!("{proof_name} に定理 `Proofs.{alias}.{ascii}` がありません", "{proof_name} holds no theorem `Proofs.{alias}.{ascii}`"),
                    Undecided::LeanNotRun => tr!("{proof_name} の証明は `--lean` で確かめます", "the proof in {proof_name} is checked with `--lean`"),
                    Undecided::Inner { rules, dates } => {
                        let mut named: Vec<Text> = rules.iter().map(|r| tr!("規則 `{r}`", "the rule `{r}`")).collect();
                        named.extend(dates.iter().map(|d| tr!("日付 `{d}`", "the date `{d}`")));
                        let all = Text::list(&named);
                        tr!(
                            "{} の答えに頼っていて、証明が使えるのは、口がその答えについて教えることだけです",
                            "it rests on the answers of {}, and a proof may use only what the port says of them",
                            all.ja; all.en
                        )
                    }
                    Undecided::NotInLean(_) | Undecided::NotWritten(_) | Undecided::Rejected(_) => unreachable!("said above"),
                };
                let msg = if held == 0 {
                    tr!("性質 `{name}` は示されていません。{}", "The property `{name}` is not shown: {}", why.ja; why.en)
                } else {
                    tr!("性質 `{name}` は、{}が、示されていません。{}", "The property `{name}` {}, but it is not shown: {}", ran.ja, why.ja; ran.en, why.en)
                };
                let mut d = at("W603", msg);
                if matches!(u, Undecided::NoProof) {
                    d = d.note(tr!(
                        "`ritsu gen --target lean` が、定理の名前と `sorry` だけの下書き `samples/{alias}.lean` を書きます。それを .kn の隣に {proof_name} として置き、`sorry` を証明に置き換えてください。",
                        "`ritsu gen --target lean` writes `samples/{alias}.lean`, the theorems' names with `sorry` for their proofs: put it beside the .kn as {proof_name}, and write the proofs in place of `sorry`."
                    ));
                }
                if matches!(u, Undecided::NoTheorem) {
                    d = d.note(tr!(
                        "{proof_name} の名前空間 `Proofs.{alias}` に `theorem {ascii} : Kaname.{alias}.{ascii}` を書いてください。`ritsu gen --target lean` が、その下書きを `samples/{alias}.lean` に書きます。",
                        "Write `theorem {ascii} : Kaname.{alias}.{ascii}` in the namespace `Proofs.{alias}` of {proof_name}; `ritsu gen --target lean` writes a draft of it in `samples/{alias}.lean`."
                    ));
                }
                out.push(d);
            }
        }
        if !decl.when.is_empty() && held == 0 && n > 0 {
            out.push(at(
                "W605",
                tr!(
                    "性質 `{name}` の `when` の行は、{n} 個のベクタのどれでも成り立ちません",
                    "The `when` lines of the property `{name}` hold on none of its {}",
                    ; plural(n, "vector", "vectors")
                ),
            )
            .note(tr!(
                "どの入力でも `when` の行が成り立たない性質は、何を言っていても証明できてしまいます。`when` の行と引数の範囲を見直してください。",
                "A property whose `when` lines hold for no input can be proved whatever it says: look again at its `when` lines and the ranges of its parameters."
            )));
        }
    }
    // what is wrong with the proof file as a whole, once
    for (x, ns) in of_file {
        let none = none_of(&ns);
        let d = match &x {
            Rejection::Build { line, col, message } => {
                let message = message.replace(&format!("Proofs/{alias}.lean"), &proof_name);
                Diag::at("E604", &proof_shown, *line, *col, tr!("証明のファイル {proof_name} はビルドが通らないので、{}", "The proof file {proof_name} does not build, so it {}", none.ja; none.en))
                    .note(tr!("Lean のエラー: {message}", "Lean says: {message}"))
            }
            Rejection::Breach(b) => {
                let what = &b.what;
                Diag::at("E604", &proof_shown, b.line, b.col, tr!("証明のファイルには `{what}` を書けないので、{proof_name} はビルドしません。{}", "A proof file may not hold `{what}`: {proof_name} is not built, so it {}", none.ja; none.en))
                    .note(breach_note(b.kind))
            }
            Rejection::Timeout(t) => {
                let secs = t.as_secs();
                Diag::at("E604", &proof_shown, 1, 1, tr!("{proof_name} の確かめが {secs} 秒で終わらなかったので、{}", "Lean did not finish {proof_name} in {secs} seconds, so it {}", none.ja; none.en))
            }
            Rejection::Kernel(said) => {
                let (line, col) = said
                    .split('\'')
                    .find_map(|w| w.strip_prefix(&format!("Proofs.{alias}.")))
                    .and_then(|n| crate::lean::theorem_at(text(), &alias, n))
                    .unwrap_or((1, 1));
                Diag::at("E604", &proof_shown, line, col, tr!("{proof_name} の宣言をカーネルが受け付けないので、{}", "The kernel does not take what {proof_name} declares, so it {}", none.ja; none.en))
                    .note(tr!("leanchecker: {said}", "leanchecker says: {said}"))
            }
            Rejection::Load(said) => Diag::at("E604", &proof_shown, 1, 1, tr!("証明のファイル {proof_name} は、{kn} から書いた Lean と一緒に読めないので、{}", "The proof file {proof_name} cannot be read beside the Lean written from {kn}, so it {}", none.ja; none.en))
                .note(tr!("Lean のエラー: {said}", "Lean says: {said}"))
                .note(tr!(
                    "証明のファイルは `Kaname.{alias}` を import し、その中には何も宣言しません。文は .kn から書いたものです。",
                    "A proof file imports `Kaname.{alias}` and declares nothing in it: its statements are the ones written from the .kn."
                )),
            _ => unreachable!("said of each property"),
        };
        out.push(d.source(text()).note(takes_note(&alias, &kn)));
    }
    out
}

/// What `check --format json` says of each property (DESIGN 36, 38.3): its name and line, the answer
/// (`shown`, `example`, `undecided`), why it is undecided, the code said of it, how many vectors it
/// ran on and on how many its `when` lines hold, and the input of a counterexample.
pub fn to_json(c: &Checked, reports: &[PropertyReport]) -> Json {
    let full = c.lowered.as_ref();
    Json::arr(reports.iter().map(|r| {
        let decl = &c.properties[r.property].decl;
        let (outcome, reason, code) = match &r.outcome {
            Outcome::Shown => ("shown", None, None),
            Outcome::Example { .. } => ("example", None, Some("E603")),
            Outcome::Undecided(u) => {
                let (reason, code) = match u {
                    Undecided::NoProof | Undecided::NoTheorem => ("no-proof", "W603"),
                    Undecided::LeanNotRun => ("lean-not-run", "W603"),
                    Undecided::Inner { .. } => ("inner", "W603"),
                    Undecided::NotInLean(_) | Undecided::NotWritten(_) => ("not-in-lean", "W604"),
                    Undecided::Rejected(_) => ("proof-rejected", "E604"),
                };
                ("undecided", Some(reason), Some(code))
            }
        };
        let input = match (&r.outcome, full) {
            (Outcome::Example { input, .. }, Some(full)) => {
                let p = Program::of(full);
                let info = &full.decls.fns[c.properties[r.property].lowered];
                Json::obj(info.params.iter().zip(input).map(|(prm, v)| (prm.name.text.clone(), p.to_json(v, &prm.slot.ty))))
            }
            _ => Json::Null,
        };
        Json::obj([
            ("name", Json::str(decl.name.text.clone())),
            ("line", Json::int(decl.line as i128)),
            ("outcome", Json::str(outcome)),
            ("reason", Json::opt_str(reason)),
            ("code", Json::opt_str(code)),
            ("vectors", Json::int(r.vectors as i128)),
            ("held", Json::int(r.held as i128)),
            ("input", input),
        ])
    }))
}
