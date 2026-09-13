//! `rulec test <output directory>` — lets the generated code be run in the user's CI as well
//! (§9.3-3, §12).
//!
//! The generated files carry the vectors and expected values of each rule. All that happens
//! here is feeding them through the generated Python and the generated Go and checking that
//! the output is byte-identical to the expected values attached by the reference evaluator.
//! **This is the only stage that uses the python3 and go toolchains** (§12).
//!
//! The unit vectors of the rounding helpers (§8.5) run in the same place. Table agreement
//! alone would hide a helper bug in a table that never produces fractions.

use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Outcome {
    pub rule: String,
    pub lang: &'static str,
    pub vectors: usize,
    /// Which line disagreed. None when everything matched.
    pub diff: Option<String>,
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
fn first_diff(got: &str, want: &str) -> String {
    for (i, (g, w)) in got.lines().zip(want.lines()).enumerate() {
        if g != w {
            return tr!(
                "{} 行目\n      生成: {g}\n      期待: {w}",
                "line {}\n      generated: {g}\n      expected: {w}",
                i + 1
            );
        }
    }
    tr!(
        "行数が違います（生成 {} 行 / 期待 {} 行）",
        "line counts differ (generated {} lines / expected {} lines)",
        got.lines().count(),
        want.lines().count()
    )
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

    let (py, go) = (have("python3"), have("go"));
    // Python is run with `-B` and any bytecode cache in the output directory is removed
    // first. A `.pyc` is considered fresh when the source has the same size and the same
    // mtime in whole seconds, so a same-length edit made within a second of the previous
    // run would otherwise execute the *old* module and report a stale result as ok. The
    // cache would also litter a directory that is committed.
    let _ = std::fs::remove_dir_all(dir.join("python").join("__pycache__"));
    let mut out = Run { results: Vec::new(), skipped: Vec::new() };
    if !py {
        out.skipped.push(tr!("python3 が無いので Python 側を飛ばしました", "python3 not found; skipped the Python side"));
    }
    if !go {
        out.skipped.push(tr!("go が無いので Go 側を飛ばしました", "go not found; skipped the Go side"));
    }
    if !py && !go {
        return Err(tr!(
            "python3 も go も無いので、生成物を走らせられません",
            "neither python3 nor go is available, so the generated code cannot be run"
        ));
    }

    for alias in &aliases {
        let vec_path = vdir.join(format!("{alias}.jsonl"));
        let want = std::fs::read_to_string(vdir.join(format!("{alias}.expected.jsonl")))
            .map_err(|_| tr!("{alias}: 期待値がありません", "{alias}: no expected values"))?;
        let n = std::fs::read_to_string(&vec_path).map(|s| s.lines().count()).unwrap_or(0);
        let pkg = alias.replace('_', "");

        let mut one = |lang: &'static str, cmd: &str, cwd: PathBuf, args: &[&str]| {
            let Ok(stdin) = std::fs::File::open(&vec_path) else { return };
            let o = Command::new(cmd).current_dir(&cwd).args(args).envs(closed()).stdin(stdin).output();
            let diff = match o {
                Err(e) => Some(tr!("起動できません: {e}", "cannot start: {e}")),
                Ok(o) if !o.status.success() => {
                    Some(tr!("落ちました:\n{}", "failed:\n{}", String::from_utf8_lossy(&o.stderr).trim()))
                }
                Ok(o) => {
                    let got = String::from_utf8_lossy(&o.stdout).into_owned();
                    (got != want).then(|| first_diff(&got, &want))
                }
            };
            out.results.push(Outcome { rule: alias.clone(), lang, vectors: n, diff });
        };
        if py {
            one("Python", "python3", dir.join("python"), &["-B", &format!("{alias}_runner.py")]);
        }
        if go {
            one("Go", "go", dir.join("go").join(format!("{pkg}runner")), &["run", "."]);
        }
    }

    // Unit vectors of the rounding helpers. Every rule emits the same ones, so run just one.
    let pkg0 = aliases[0].replace('_', "");
    if py {
        let o = Command::new("python3")
            .current_dir(dir.join("python"))
            .args(["-B", "_round_test.py"])
            .output();
        let diff = match o {
            Ok(o) if o.status.success() => None,
            Ok(o) => Some(String::from_utf8_lossy(&o.stdout).trim().to_string()),
            Err(e) => Some(tr!("起動できません: {e}", "cannot start: {e}")),
        };
        out.results.push(Outcome { rule: tr!("丸めヘルパ", "rounding helper"), lang: "Python", vectors: 0, diff });
    }
    if go {
        let o = Command::new("go")
            .current_dir(dir.join("go").join(&pkg0))
            .args(["test", "./..."])
            .envs(closed())
            .output();
        let diff = match o {
            Ok(o) if o.status.success() => None,
            Ok(o) => Some(String::from_utf8_lossy(&o.stdout).trim().to_string()),
            Err(e) => Some(tr!("起動できません: {e}", "cannot start: {e}")),
        };
        out.results.push(Outcome { rule: tr!("丸めヘルパ", "rounding helper"), lang: "Go", vectors: 0, diff });
    }
    Ok(out)
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
                o.push_str(&format!("ok    {} ({}) {n}\n", x.rule, x.lang));
            }
            Some(d) => {
                bad += 1;
                o.push_str(&tr!(
                    "FAIL  {} ({}) 参照評価器と食い違います\n    {d}\n",
                    "FAIL  {} ({}) disagrees with the reference evaluator\n    {d}\n",
                    x.rule,
                    x.lang
                ));
            }
        }
    }
    if bad == 0 {
        o.push_str(&tr!("\n{} 件すべて一致しました。\n", "\nAll {} matched.\n", r.results.len()));
    } else {
        o.push_str(&tr!("\n{bad} 件が食い違いました。\n", "\n{bad} disagreed.\n"));
    }
    o
}
