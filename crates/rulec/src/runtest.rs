//! `rulec test <生成先>` — 生成物を、利用者の CI でも回せるようにする（§9.3-3、§12）。
//!
//! 生成物には規則ごとのベクタと期待値が埋めてある。ここでやるのは、それを
//! 生成 Python と生成 Go に流して、参照評価器が付けた期待値とバイト一致するか
//! を見ることだけ。**python3 と go の toolchain を使う唯一の段**である（§12）。
//!
//! 丸めヘルパの単体ベクタ（§8.5）も同じ場で回す。表の一致だけでは、端数の
//! 出ない表でヘルパの誤りが隠れる。

use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Outcome {
    pub rule: String,
    pub lang: &'static str,
    pub vectors: usize,
    /// 何行目で食い違ったか。合っていれば None。
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

/// 生成物は外部の依存を持たない（`go.mod` は自分の二つだけ）。それを**仮定ではなく
/// 検査された性質**にするため、Go の取得口を閉じて走らせる。取りに行こうとしたら
/// 落ちるので、依存がいつの間にか混ざったらここで分かる。
fn closed() -> [(&'static str, &'static str); 2] {
    [("GOPROXY", "off"), ("GOFLAGS", "-mod=mod")]
}

fn have(cmd: &str) -> bool {
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

/// 最初に食い違った行を、行番号つきで言う。何千行の diff を貼らない。
fn first_diff(got: &str, want: &str) -> String {
    for (i, (g, w)) in got.lines().zip(want.lines()).enumerate() {
        if g != w {
            return format!("{} 行目\n      生成: {g}\n      期待: {w}", i + 1);
        }
    }
    format!("行数が違います（生成 {} 行 / 期待 {} 行）", got.lines().count(), want.lines().count())
}

pub fn run(dir: &Path) -> Result<Run, String> {
    let vdir = dir.join("vectors");
    let rd = std::fs::read_dir(&vdir).map_err(|_| {
        format!("`{}` にベクタがありません。先に `rulec gen --out {}` を実行してください", vdir.display(), dir.display())
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
        return Err(format!("`{}` にベクタがありません", vdir.display()));
    }

    let (py, go) = (have("python3"), have("go"));
    let mut out = Run { results: Vec::new(), skipped: Vec::new() };
    if !py {
        out.skipped.push("python3 が無いので Python 側を飛ばしました".into());
    }
    if !go {
        out.skipped.push("go が無いので Go 側を飛ばしました".into());
    }
    if !py && !go {
        return Err("python3 も go も無いので、生成物を走らせられません".into());
    }

    for alias in &aliases {
        let vec_path = vdir.join(format!("{alias}.jsonl"));
        let want = std::fs::read_to_string(vdir.join(format!("{alias}.expected.jsonl")))
            .map_err(|_| format!("{alias}: 期待値がありません"))?;
        let n = std::fs::read_to_string(&vec_path).map(|s| s.lines().count()).unwrap_or(0);
        let pkg = alias.replace('_', "");

        let mut one = |lang: &'static str, cmd: &str, cwd: PathBuf, args: &[&str]| {
            let Ok(stdin) = std::fs::File::open(&vec_path) else { return };
            let o = Command::new(cmd).current_dir(&cwd).args(args).envs(closed()).stdin(stdin).output();
            let diff = match o {
                Err(e) => Some(format!("起動できません: {e}")),
                Ok(o) if !o.status.success() => {
                    Some(format!("落ちました:\n{}", String::from_utf8_lossy(&o.stderr).trim()))
                }
                Ok(o) => {
                    let got = String::from_utf8_lossy(&o.stdout).into_owned();
                    (got != want).then(|| first_diff(&got, &want))
                }
            };
            out.results.push(Outcome { rule: alias.clone(), lang, vectors: n, diff });
        };
        if py {
            one("Python", "python3", dir.join("python"), &[&format!("{alias}_runner.py")]);
        }
        if go {
            one("Go", "go", dir.join("go").join(format!("{pkg}runner")), &["run", "."]);
        }
    }

    // 丸めヘルパの単体ベクタ。規則ごとに同じものが出ているので、一本だけ回す。
    let pkg0 = aliases[0].replace('_', "");
    if py {
        let o = Command::new("python3")
            .current_dir(dir.join("python"))
            .arg("_round_test.py")
            .output();
        let diff = match o {
            Ok(o) if o.status.success() => None,
            Ok(o) => Some(String::from_utf8_lossy(&o.stdout).trim().to_string()),
            Err(e) => Some(format!("起動できません: {e}")),
        };
        out.results.push(Outcome { rule: "丸めヘルパ".into(), lang: "Python", vectors: 0, diff });
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
            Err(e) => Some(format!("起動できません: {e}")),
        };
        out.results.push(Outcome { rule: "丸めヘルパ".into(), lang: "Go", vectors: 0, diff });
    }
    Ok(out)
}

pub fn render(r: &Run) -> String {
    let mut o = String::new();
    for s in &r.skipped {
        o.push_str(&format!("注意: {s}\n"));
    }
    let mut bad = 0;
    for x in &r.results {
        match &x.diff {
            None => {
                let n = if x.vectors > 0 { format!("ベクタ {} 件", x.vectors) } else { "単体ベクタ".into() };
                o.push_str(&format!("ok    {} ({}) {n}\n", x.rule, x.lang));
            }
            Some(d) => {
                bad += 1;
                o.push_str(&format!("FAIL  {} ({}) 参照評価器と食い違います\n    {d}\n", x.rule, x.lang));
            }
        }
    }
    if bad == 0 {
        o.push_str(&format!("\n{} 件すべて一致しました。\n", r.results.len()));
    } else {
        o.push_str(&format!("\n{bad} 件が食い違いました。\n"));
    }
    o
}
