//! `rulec test <output directory>` — lets the generated code be run in the user's CI as well
//! (§9.3-3, §12).
//!
//! The generated files carry the vectors and expected values of each rule. All that happens
//! here is feeding them through the generated Python and the generated Go and checking that
//! the output is byte-identical to the expected values attached by the reference evaluator.
//! **This is the only stage that reaches a toolchain** — python3, node, rustc, ruby, go (§12).
//!
//! The unit vectors of the rounding helpers (§8.5) run in the same place. Table agreement
//! alone would hide a helper bug in a table that never produces fractions.

use std::path::Path;
use std::process::Command;

/// Why one run failed, in the two kinds a reader has to tell apart (docs/formats.md).
///
/// A disagreement is about the rule and its generated code. **Not being able to run at all** is
/// about the machine — a toolchain that is missing or would not fetch, code that did not
/// compile, a process that died. Both used to be printed under "disagrees with the reference
/// evaluator", which told a reader the comparison had happened and come out badly when it had
/// never happened at all.
pub enum Failure {
    /// The first line of the canonical JSON on which the two sides disagreed.
    Diff { line: usize, generated: String, expected: String },
    /// Both sides ran and answered a different number of times. A disagreement with no single
    /// line to point at. **Prose.**
    Lines(String),
    /// The comparison never happened. **Prose.**
    Broken(String),
}

impl Failure {
    /// Whether the generated code ran far enough to be compared at all.
    pub fn ran(&self) -> bool {
        !matches!(self, Failure::Broken(_))
    }
}

impl Failure {
    /// The form the text rendering puts under `FAIL`.
    pub fn text(&self) -> String {
        match self {
            Failure::Diff { line, generated, expected } => tr!(
                "{line} 行目\n      生成: {generated}\n      期待: {expected}",
                "line {line}\n      generated: {generated}\n      expected: {expected}"
            ),
            Failure::Lines(s) | Failure::Broken(s) => s.clone(),
        }
    }
}

/// The id of the pseudo-rule that stands for the rounding helpers. A stable token rather
/// than prose, because `--format json` reports `rule` as an identifier.
pub const ROUND_HELPER: &str = "_round";

pub struct Outcome {
    /// The rule's ASCII alias, or `_round` for the rounding helpers.
    pub rule: String,
    pub lang: &'static str,
    pub vectors: usize,
    /// Why it failed. None when everything matched.
    pub diff: Option<Failure>,
}

pub struct Run {
    pub results: Vec<Outcome>,
    pub skipped: Vec<String>,
}

impl Run {
    pub fn ok(&self) -> bool {
        self.results.iter().all(|r| r.diff.is_none())
    }
}

/// The generated code has no external dependencies (`go.mod` lists only our own two modules).
/// To make that a **checked property rather than an assumption**, Go runs with its download
/// path shut. Any attempt to fetch fails, so a dependency that sneaks in shows up here.
fn closed() -> [(&'static str, &'static str); 2] {
    [("GOPROXY", "off"), ("GOFLAGS", "-mod=mod")]
}

fn have(cmd: &str) -> bool {
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

/// Report the first line that disagrees, with its line number. No thousand-line diffs.
fn first_diff(got: &str, want: &str) -> Failure {
    for (i, (g, w)) in got.lines().zip(want.lines()).enumerate() {
        if g != w {
            return Failure::Diff {
                line: i + 1,
                generated: g.to_string(),
                expected: w.to_string(),
            };
        }
    }
    Failure::Lines(tr!(
        "行数が違います（生成 {} 行 / 期待 {} 行）",
        "line counts differ (generated {} lines / expected {} lines)",
        got.lines().count(),
        want.lines().count()
    ))
}

pub fn run(dir: &Path) -> Result<Run, String> {
    let vdir = dir.join("vectors");
    let rd = std::fs::read_dir(&vdir).map_err(|_| {
        tr!(
            "`{}` にベクタがありません。先に `rulec gen --out {}` を実行してください",
            "no vectors in `{}`; run `rulec gen --out {}` first",
            vdir.display(),
            dir.display()
        )
    })?;
    let mut aliases: Vec<String> = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(a) = name.strip_suffix(".jsonl") {
            if !a.ends_with(".expected") {
                aliases.push(a.to_string());
            }
        }
    }
    aliases.sort();
    if aliases.is_empty() {
        return Err(tr!("`{}` にベクタがありません", "no vectors in `{}`", vdir.display()));
    }

    // Which toolchains are here. The set of backends lives in src/backend.rs, so a new
    // language is a row there rather than five more blocks in this file.
    let mut out = Run { results: Vec::new(), skipped: Vec::new() };
    let present: Vec<&crate::backend::Backend> = crate::backend::ALL
        .iter()
        .filter(|b| {
            let ok = have(b.tool);
            if !ok {
                out.skipped.push(tr!(
                    "{} が無いので {} 側を飛ばしました",
                    "{} not found; skipped the {} side",
                    b.tool,
                    b.name
                ));
            }
            ok
        })
        .collect();
    if present.is_empty() {
        return Err(tr!(
            "どの toolchain も無いので、生成物を走らせられません（{}）",
            "none of the toolchains is available, so the generated code cannot be run ({})",
            crate::backend::ALL.iter().map(|b| b.tool).collect::<Vec<_>>().join(", ")
        ));
    }
    // A `.pyc` counts as fresh when the source has the same length and the same
    // whole-second mtime, so a same-length edit within a second of the last run would
    // otherwise execute the old module and report a stale result as ok.
    let _ = std::fs::remove_dir_all(dir.join("python").join("__pycache__"));

    // One plan is one command, with an optional build that has to succeed first.
    // Ok is the command's stdout; Err is anything that stopped it from producing one.
    let exec = |plan: &crate::backend::Plan, stdin: Option<&Path>| -> Result<String, Failure> {
        let cwd = dir.join(&plan.cwd);
        if let Some((cmd, args)) = &plan.build {
            match Command::new(cmd).current_dir(&cwd).args(args).envs(closed()).output() {
                Ok(o) if o.status.success() => {}
                Ok(o) => {
                    return Err(Failure::Broken(tr!(
                        "コンパイルできません:\n{}",
                        "does not compile:\n{}",
                        String::from_utf8_lossy(&o.stderr).trim()
                    )))
                }
                Err(e) => return Err(Failure::Broken(tr!("起動できません: {e}", "cannot start: {e}"))),
            }
        }
        let mut c = Command::new(&plan.cmd);
        c.current_dir(&cwd).args(&plan.args).envs(closed());
        if let Some(p) = stdin {
            let Ok(f) = std::fs::File::open(p) else {
                return Err(Failure::Broken(tr!("ベクタが読めません", "cannot read the vectors")));
            };
            c.stdin(f);
        }
        match c.output() {
            Err(e) => Err(Failure::Broken(tr!("起動できません: {e}", "cannot start: {e}"))),
            Ok(o) if !o.status.success() => {
                // A round-test script reports on stdout and exits non-zero; a runner that
                // crashes says so on stderr. Show whichever is not empty.
                let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                let msg = if err.is_empty() { String::from_utf8_lossy(&o.stdout).trim().to_string() } else { err };
                Err(Failure::Broken(tr!("落ちました:\n{}", "failed:\n{}", msg)))
            }
            Ok(o) => Ok(String::from_utf8_lossy(&o.stdout).into_owned()),
        }
    };

    for alias in &aliases {
        let vec_path = dir.join("vectors").join(format!("{alias}.jsonl"));
        let want = std::fs::read_to_string(dir.join("vectors").join(format!("{alias}.expected.jsonl")))
            .map_err(|_| tr!("{alias}: 期待値がありません", "{alias}: no expected values"))?;
        let n = std::fs::read_to_string(&vec_path).map(|s| s.lines().count()).unwrap_or(0);
        let pkg = alias.replace('_', "");
        for b in &present {
            let diff = match exec(&(b.run)(alias, &pkg), Some(&vec_path)) {
                Err(f) => Some(f),
                Ok(got) => (got != want).then(|| first_diff(&got, &want)),
            };
            out.results.push(Outcome { rule: alias.clone(), lang: b.name, vectors: n, diff });
        }
    }

    // Unit vectors of the rounding helpers. Every rule emits the same ones, so run just one.
    let pkg0 = aliases[0].replace('_', "");
    for b in &present {
        let diff = exec(&(b.round)(&pkg0), None).err();
        out.results.push(Outcome { rule: ROUND_HELPER.into(), lang: b.name, vectors: 0, diff });
    }

    Ok(out)
}

fn shown(rule: &str) -> String {
    if rule == ROUND_HELPER {
        tr!("丸めヘルパ", "rounding helper")
    } else {
        rule.to_string()
    }
}

pub fn render(r: &Run) -> String {
    let mut o = String::new();
    for s in &r.skipped {
        o.push_str(&tr!("注意: {s}\n", "warning: {s}\n"));
    }
    let mut bad = 0;
    for x in &r.results {
        match &x.diff {
            None => {
                let n = if x.vectors > 0 {
                    tr!("ベクタ {} 件", "{} vectors", x.vectors)
                } else {
                    tr!("単体ベクタ", "unit vectors")
                };
                o.push_str(&format!("ok    {} ({}) {n}\n", shown(&x.rule), x.lang));
            }
            Some(d) => {
                bad += 1;
                let text = d.text();
                o.push_str(&if d.ran() {
                    tr!(
                        "FAIL  {} ({}) 参照評価器と食い違います\n    {text}\n",
                        "FAIL  {} ({}) disagrees with the reference evaluator\n    {text}\n",
                        shown(&x.rule),
                        x.lang
                    )
                } else {
                    tr!(
                        "FAIL  {} ({}) 走らせられませんでした\n    {text}\n",
                        "FAIL  {} ({}) could not be run\n    {text}\n",
                        shown(&x.rule),
                        x.lang
                    )
                });
            }
        }
    }
    // What the run covered belongs in the summary. "All 2 matched." after four languages were
    // skipped reads as the whole claim holding, when the claim is that the reference evaluator
    // and **every** generated language agree.
    let scope = if r.skipped.is_empty() {
        String::new()
    } else {
        tr!(
            "（{} 言語中 {} 言語を飛ばしました）",
            " ({} of {} languages skipped)",
            r.skipped.len(),
            crate::backend::ALL.len()
        )
    };
    if bad == 0 {
        o.push_str(&tr!("\n{} 件すべて一致しました{scope}。\n", "\nAll {} matched{scope}.\n", r.results.len()));
    } else {
        o.push_str(&tr!("\n{bad} 件が食い違いました{scope}。\n", "\n{bad} disagreed{scope}.\n"));
    }
    o
}

/// `--format json` (docs/formats.md). One object for the run.
pub fn render_json(r: &Run) -> String {
    let results: Vec<String> = r
        .results
        .iter()
        .map(|x| {
            let first = match &x.diff {
                Some(Failure::Diff { line, generated, expected }) => crate::json::Obj::new()
                    .int("line", *line as i128)
                    .str("generated", generated)
                    .str("expected", expected)
                    .finish(),
                // A failure with no single line to point at is prose, and it goes in `error`
                // rather than pretending to be a diff.
                Some(_) | None => "null".into(),
            };
            let err = match &x.diff {
                Some(Failure::Lines(e)) | Some(Failure::Broken(e)) => crate::json::quote(e),
                _ => "null".into(),
            };
            crate::json::Obj::new()
                .str("rule", &x.rule)
                .str("lang", x.lang.to_lowercase())
                .int("vectors", x.vectors as i128)
                .bool("ok", x.diff.is_none())
                // Whether the generated code ran at all. `ok:false` with `ran:false` is a
                // machine that could not build or start it, not a rule that answered wrongly.
                .bool("ran", x.diff.as_ref().is_none_or(|d| d.ran()))
                .raw("first_diff", first)
                .raw("error", err)
                .finish()
        })
        .collect();
    crate::json::Obj::new()
        .raw("results", crate::json::arr(&results))
        .raw("skipped", crate::json::strs(&r.skipped))
        .finish()
}
