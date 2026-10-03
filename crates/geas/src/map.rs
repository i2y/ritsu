//! `geas map` (DESIGN §7): `check` with every runtime's coverage switched on, and
//! the record of the lines each claim ran, `.geas/<stem>.map.jsonl`. The record
//! never decides a verdict; `affected` reads it to say which claims a diff touches.

use crate::cover::{Reader, Report, Session};
use crate::diag::{self, Diag, DiagExt, Severity, count};
use ritsu_base::text::{Lang, Text};
use crate::hash;
use crate::json::{self, J};
use crate::lines::{self, Lines};
use crate::model::Spec;
use crate::run::{self, ClaimResult, ClaimStatus, Opts};
use crate::sched;
use crate::tree::{self, Runtime};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const FORMAT: u32 = 1;

/// A claim as the record has it: how it ended in the mapped run, and the targets
/// it started.
#[derive(Clone, Debug, PartialEq)]
pub struct RecClaim {
    pub name: String,
    pub status: String,
    pub targets: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecFile {
    pub path: String,
    pub blob: String,
    pub runtime: Runtime,
    /// The lines a runtime called code; None when no runtime reported the file.
    pub code: Option<Lines>,
}

/// The lines a claim ran in a file, through one target's processes.
#[derive(Clone, Debug, PartialEq)]
pub struct RecRan {
    /// Index into the record's claims.
    pub claim: usize,
    pub target: String,
    pub file: String,
    pub lines: Lines,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// The spec's path relative to the root.
    pub spec: String,
    /// The root relative to the spec's directory.
    pub root: String,
    pub claims: Vec<RecClaim>,
    pub files: Vec<RecFile>,
    pub ran: Vec<RecRan>,
    /// The line each file's entry is on, when the record was read from text.
    pub file_lines: BTreeMap<String, usize>,
}

impl Record {
    /// One JSON object a line: the header, the files by path, then the claims'
    /// lines in claim order, by target and path within a claim.
    pub fn render(&self) -> String {
        let claims: Vec<String> = self
            .claims
            .iter()
            .map(|c| {
                let ts: Vec<String> = c.targets.iter().map(|x| json::quote(x)).collect();
                format!(
                    "{{\"name\":{},\"status\":{},\"targets\":[{}]}}",
                    json::quote(&c.name),
                    json::quote(&c.status),
                    ts.join(",")
                )
            })
            .collect();
        let mut out = format!(
            "{{\"geas_map\":{FORMAT},\"spec\":{},\"root\":{},\"claims\":[{}]}}\n",
            json::quote(&self.spec),
            json::quote(&self.root),
            claims.join(",")
        );
        for f in &self.files {
            let code = match &f.code {
                Some(l) => json::quote(&lines::to_ranges(l)),
                None => "null".into(),
            };
            out.push_str(&format!(
                "{{\"file\":{},\"blob\":\"{}\",\"lang\":\"{}\",\"code\":{}}}\n",
                json::quote(&f.path),
                f.blob,
                f.runtime.word(),
                code
            ));
        }
        for r in &self.ran {
            out.push_str(&format!(
                "{{\"claim\":{},\"target\":{},\"file\":{},\"ran\":{}}}\n",
                json::quote(&self.claims[r.claim].name),
                json::quote(&r.target),
                json::quote(&r.file),
                json::quote(&lines::to_ranges(&r.lines))
            ));
        }
        out
    }

    /// Reads a record back. On failure, the line (from 1) and why.
    pub fn parse(text: &str) -> Result<Record, (usize, Text)> {
        let mut rec: Option<Record> = None;
        for (i, line) in text.lines().enumerate() {
            let n = i + 1;
            if line.trim().is_empty() {
                continue;
            }
            let v = json::parse(line)
                .map_err(|e| (n, tr!("この行は JSON ではありません", "the line is not JSON: {e}")))?;
            let Some(r) = rec.as_mut() else {
                rec = Some(header(&v).map_err(|why| (n, why))?);
                continue;
            };
            if get(&v, "blob").is_some() {
                let f = file_entry(&v).map_err(|why| (n, why))?;
                if r.file_lines.insert(f.path.clone(), n).is_some() {
                    return Err((n, tr!("{} が記録に二つあります", "{} is in the record twice", f.path)));
                }
                r.files.push(f);
            } else if get(&v, "claim").is_some() {
                let x = ran_entry(&v, r).map_err(|why| (n, why))?;
                r.ran.push(x);
            } else {
                return Err((n, tr!("この行はファイルの行でも主張の行でもありません", "the line is neither a file's line nor a claim's")));
            }
        }
        rec.ok_or((0, tr!("ファイルが空です", "the file is empty")))
    }

    pub fn file(&self, path: &str) -> Option<&RecFile> {
        self.files.iter().find(|f| f.path == path)
    }
}

fn get<'a>(v: &'a J, key: &str) -> Option<&'a J> {
    match v {
        J::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

fn string(v: &J, key: &str) -> Result<String, Text> {
    match get(v, key) {
        Some(J::Str(s)) => Ok(s.clone()),
        _ => Err(tr!("この行に文字列の `{key}` がありません", "the line has no `{key}` string")),
    }
}

fn ranges(s: &str) -> Result<Lines, Text> {
    lines::from_ranges(s).map_err(|why| tr!("`{s}` は行番号の範囲として読めません", "`{s}` is not a list of line ranges: {why}"))
}

fn header(v: &J) -> Result<Record, Text> {
    match get(v, "geas_map") {
        Some(J::Num(n)) if *n == f64::from(FORMAT) => {}
        Some(J::Num(n)) => {
            return Err(tr!(
                "この記録の形式は {} で、この geas が読めるのは形式 {FORMAT} です",
                "the record is of format {}, and this geas reads format {FORMAT}",
                json::render_num(*n),
            ));
        }
        _ => {
            return Err(tr!(
                "最初の行が記録のヘッダー（`geas_map`）ではありません",
                "the first line is not a record's header (`geas_map`)",
            ));
        }
    }
    let mut claims = Vec::new();
    let Some(J::Arr(cs)) = get(v, "claims") else {
        return Err(tr!("ヘッダーに `claims` の配列がありません", "the header has no `claims` array"));
    };
    for c in cs {
        let mut targets = Vec::new();
        if let Some(J::Arr(ts)) = get(c, "targets") {
            for x in ts {
                if let J::Str(s) = x {
                    targets.push(s.clone());
                }
            }
        }
        claims.push(RecClaim { name: string(c, "name")?, status: string(c, "status")?, targets });
    }
    Ok(Record {
        spec: string(v, "spec")?,
        root: string(v, "root")?,
        claims,
        files: vec![],
        ran: vec![],
        file_lines: BTreeMap::new(),
    })
}

fn file_entry(v: &J) -> Result<RecFile, Text> {
    let path = string(v, "file")?;
    let blob = string(v, "blob")?;
    if blob.len() != 40 || !blob.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(tr!("{path} の blob が 16 進 40 桁ではありません", "the blob of {path} is not 40 hex digits"));
    }
    let lang = string(v, "lang")?;
    let Some(runtime) = Runtime::parse(&lang) else {
        return Err(tr!("`{lang}` は記録の知らない言語です", "`{lang}` is not a language the record knows"));
    };
    let code = match get(v, "code") {
        Some(J::Null) => None,
        Some(J::Str(s)) => Some(ranges(s)?),
        _ => return Err(tr!("{path} の code が行番号の範囲でも null でもありません", "the code of {path} is neither line ranges nor null")),
    };
    Ok(RecFile { path, blob, runtime, code })
}

fn ran_entry(v: &J, r: &Record) -> Result<RecRan, Text> {
    let name = string(v, "claim")?;
    let Some(claim) = r.claims.iter().position(|c| c.name == name) else {
        return Err(tr!("主張 \"{name}\" がヘッダーにありません", "the claim \"{name}\" is not in the header"));
    };
    let file = string(v, "file")?;
    if r.file(&file).is_none() {
        return Err(tr!("{file} の行が記録にありません", "{file} has no line of its own in the record"));
    }
    Ok(RecRan { claim, target: string(v, "target")?, file, lines: ranges(&string(v, "ran")?)? })
}

/// The root a spec's record is relative to: `--root`, or the nearest directory
/// above the spec holding `.git`, else the spec's own directory; and the spec's path
/// relative to it.
pub fn root_and_spec(file: &str, root_flag: Option<&str>) -> Result<(PathBuf, String), Diag> {
    let spec_abs = std::fs::canonicalize(file).map_err(|e| {
        diag::error("E081", 0, 0, tr!("このファイルを読めません: {e}", "cannot read this file: {e}"))
    })?;
    let spec_dir = spec_abs.parent().expect("a file is in a directory");
    let root = match root_flag {
        Some(r) => {
            let p = std::fs::canonicalize(r).map_err(|e| {
                diag::error(
                    "E081",
                    0,
                    0,
                    tr!("ルート `{r}` を読めません: {e}", "cannot read the root `{r}`: {e}"),
                )
            })?;
            if !p.is_dir() {
                return Err(diag::error(
                    "E080",
                    0,
                    0,
                    tr!(
                        "`--root` の `{r}` はディレクトリではありません",
                        "`--root` names `{r}`, which is not a directory",
                    ),
                ));
            }
            p
        }
        None => tree::root_of(spec_dir),
    };
    match tree::relative(&root, &spec_abs) {
        Some(rel) => Ok((root, rel)),
        None => Err(diag::error(
            "E080",
            0,
            0,
            tr!(
                "主張のファイル `{file}` がルート `{}` の下にありません",
                "the spec `{file}` is not under the root `{}`",
                root_flag.unwrap_or("."),
            ),
        )
        .note(tr!(
            "記録のパスはルートからの相対パスなので、主張のファイルはルートの下に置きます",
            "the record's paths are relative to the root, so the spec has to be under it",
        ))),
    }
}

/// What a `map` run gives: the claims' results, the record when nothing stopped it,
/// and its diagnostics (warnings, and the errors that stop the record).
pub struct MapRun {
    pub results: Vec<ClaimResult>,
    pub record: Option<Record>,
    pub diags: Vec<Diag>,
}

impl MapRun {
    pub fn stopped(&self) -> bool {
        self.diags.iter().any(|d| d.severity == Severity::Error)
    }
}

/// Where the record's paths start: the root, and the spec's path relative to it.
pub struct Place<'a> {
    /// The directory the targets' commands run in, as given.
    pub cwd: &'a Path,
    /// `.geas` beside the spec, absolute.
    pub geas_dir: &'a Path,
    /// Absolute and canonical.
    pub root: &'a Path,
    /// The spec relative to the root.
    pub spec_rel: &'a str,
}

impl Place<'_> {
    /// The spec's file name without `.geas`, which names its files under `.geas/`.
    pub fn stem(&self) -> String {
        Path::new(self.spec_rel).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "spec".into())
    }
}

/// Runs every claim with the switches on, up to `jobs` at once, reads what each
/// ran, and builds the record. Err is the diagnostic of a session that could not
/// start (E081). Whatever finishes first, the warnings are put together in claim
/// order, so that one run says what another says.
pub fn run(spec: &Spec, place: &Place, jobs: usize, journal: &mut Vec<String>) -> Result<MapRun, Diag> {
    let session = Session::start(place.geas_dir, place.root).map_err(|e| {
        diag::error(
            "E081",
            0,
            0,
            tr!(
                ".geas/ の下にカバレッジのフックを書けません: {e}",
                "cannot write geas's coverage hooks under .geas/: {e}",
            ),
        )
    })?;
    let reader = Reader::new(place.root);
    let gui = crate::gui::Shared::new(place.geas_dir, &place.stem(), jobs);
    let runs = sched::each(spec, jobs, |worker, i| {
        let (r, lines) = run::run_claim(spec, &spec.claims[i], i + 1, place.cwd, Opts { cover: Some(&session), gui: &gui, worker });
        let slice = reader.read_claim(&r.started);
        session.done_with(i + 1);
        (r, slice, lines)
    });
    let mut results = Vec::new();
    let mut diags = Vec::new();
    // per claim, per target: what it ran
    let mut slices: Vec<BTreeMap<String, Report>> = Vec::new();
    let mut w061: BTreeSet<String> = BTreeSet::new();
    let mut e065: BTreeSet<String> = BTreeSet::new();
    // targets whose missing lines another diagnostic already explains
    let mut explained: BTreeSet<String> = BTreeSet::new();
    for (r, slice, lines) in runs {
        journal.extend(lines);
        for p in &r.started {
            if p.killed {
                diags.push(killed(&r, p));
                explained.insert(p.target.clone());
            }
        }
        for prob in slice.problems {
            explained.insert(prob.target.clone());
            // once per target, and a missing tool once per run
            if prob.code == "W061" && !w061.insert(prob.target.clone()) {
                continue;
            }
            if prob.code == "E065" && !e065.insert(prob.msg.en.clone()) {
                continue;
            }
            let col = r.started.iter().find(|p| p.line == prob.line).map_or(0, |p| p.col);
            let mut d = diag::error(prob.code, prob.line, col, prob.msg).with_path(run::run_up_to(&r, prob.line));
            if prob.code.starts_with('W') {
                d.severity = Severity::Warning;
            }
            d.notes = prob.notes;
            diags.push(d);
        }
        slices.push(slice.by_target);
        results.push(r);
    }
    diags.extend(no_record(spec, &results, &slices, &explained));
    drop(session);
    crate::diag::sort(&mut diags);
    let stopped = diags.iter().any(|d| d.severity == Severity::Error);
    let record = if stopped {
        None
    } else {
        match build(spec, place, &results, &slices) {
            Ok(r) => Some(r),
            Err(d) => {
                diags.push(d);
                None
            }
        }
    };
    Ok(MapRun { results, record, diags })
}

/// E066: a service that outlived SIGTERM by 5 s and was killed.
fn killed(r: &ClaimResult, p: &run::Started) -> Diag {
    let w = crate::proc::shell_words(&p.words);
    let mut d = diag::error(
        "E066",
        p.line,
        p.col,
        tr!(
            "サービス `{}` が SIGTERM から 5 秒のうちに終了しなかったので、geas が強制終了しました。実行した行が記録から抜けているかもしれません",
            "the service `{}` did not exit within 5 s of SIGTERM, so geas killed it, and what it ran may be missing from the record",
            p.target,
        ),
    )
    .with_path(run::run_up_to(r, usize::MAX));
    d.notes = vec![
        tr!("コマンド: {w}", "command: {w}"),
        tr!(
            "サービスが記録されるのは、SIGTERM を受けて自分で終了したときです。Python と Node は、プログラムがシグナルを無視していなければ geas のフックで終了します。Go のサービスは main から戻る必要があり（signal.NotifyContext と Server.Shutdown）、Rust のサービスには SIGTERM を受けて終了するコードが要ります",
            "a service is recorded when it exits by itself on SIGTERM: Python and Node do, through geas's hooks, unless the program ignores the signal; a Go service has to return from main (signal.NotifyContext and Server.Shutdown); a Rust service needs code of its own",
        ),
    ];
    d
}

/// W060: every target that was started and gave no line under the root, in any
/// claim, unless another diagnostic (W061, E065, E066) already says why.
fn no_record(
    spec: &Spec,
    results: &[ClaimResult],
    slices: &[BTreeMap<String, Report>],
    explained: &BTreeSet<String>,
) -> Vec<Diag> {
    let mut out = Vec::new();
    for tg in spec.targets.iter().filter(|t| !explained.contains(&t.name)) {
        let mut first: Option<(&ClaimResult, &run::Started)> = None;
        let mut gave = false;
        for (r, s) in results.iter().zip(slices) {
            if let Some(p) = r.started.iter().find(|p| p.target == tg.name) {
                first = first.or(Some((r, p)));
                gave |= s.get(&tg.name).is_some_and(|rep| !rep.is_empty());
            }
        }
        let Some((r, p)) = first else {
            continue;
        };
        if gave {
            continue;
        }
        let w = crate::proc::shell_words(&p.words);
        let mut d = diag::error(
            "W060",
            tg.pos.line,
            tg.pos.col,
            tr!(
                "ターゲット `{}` からは記録が取れませんでした。起動したプロセスのどれも、ルートの下のファイルの行を報告していません",
                "the target `{}` gave no record: none of the processes it started reported a line of a file under the root",
                tg.name,
            ),
        )
        .with_path(run::run_up_to(r, p.line));
        d.severity = Severity::Warning;
        d.notes = vec![
            tr!("コマンド: {w}", "command: {w}"),
            tr!(
                "geas が記録できるのは、ルートの下のファイルを実行する Python 3.12 以降、Node、-cover を付けてビルドした Go のプログラム、-C instrument-coverage を付けてビルドした Rust のプログラムです。シェルスクリプト、古い Python、カバレッジなしでビルドしたプログラムからは何も取れません",
                "geas records Python 3.12 and later, Node, a Go program built with -cover and a Rust program built with -C instrument-coverage, in the files under the root; a shell script, an older Python or a program built without coverage gives nothing",
            ),
        ];
        out.push(d);
    }
    out
}

/// The record: every source file under the root with its blob hash and its code
/// lines, and every claim's lines per target and file.
fn build(spec: &Spec, place: &Place, results: &[ClaimResult], slices: &[BTreeMap<String, Report>]) -> Result<Record, Diag> {
    let e081 = |what: String, e: std::io::Error| {
        diag::error("E081", 0, 0, tr!("{what} を読めません: {e}", "cannot read {what}: {e}"))
    };
    let sources = tree::sources(place.root).map_err(|e| e081("the tree under the root".into(), e))?;
    let mut code: BTreeMap<&str, Lines> = BTreeMap::new();
    for s in slices {
        for rep in s.values() {
            for (path, f) in rep {
                code.entry(path.as_str()).or_default().extend(f.code.iter().copied());
            }
        }
    }
    let mut files = Vec::new();
    for path in &sources {
        let bytes = std::fs::read(place.root.join(path)).map_err(|e| e081(path.clone(), e))?;
        files.push(RecFile {
            path: path.clone(),
            blob: hash::blob(&bytes),
            runtime: Runtime::of(path).expect("a source file has a runtime"),
            code: code.get(path.as_str()).cloned(),
        });
    }
    let known: BTreeSet<&str> = sources.iter().map(String::as_str).collect();
    let order: Vec<&str> = spec.targets.iter().map(|t| t.name.as_str()).collect();
    let mut claims = Vec::new();
    let mut ran = Vec::new();
    for (i, (r, s)) in results.iter().zip(slices).enumerate() {
        let mut targets: Vec<String> = Vec::new();
        for p in &r.started {
            if !targets.contains(&p.target) {
                targets.push(p.target.clone());
            }
        }
        claims.push(RecClaim {
            name: r.name.clone(),
            status: match r.status {
                ClaimStatus::Ok => "ok",
                ClaimStatus::Fail => "fail",
                ClaimStatus::Error(_) => "error",
            }
            .into(),
            targets,
        });
        for target in &order {
            let Some(rep) = s.get(*target) else {
                continue;
            };
            for (path, f) in rep {
                if known.contains(path.as_str()) && !f.ran.is_empty() {
                    ran.push(RecRan { claim: i, target: target.to_string(), file: path.clone(), lines: f.ran.clone() });
                }
            }
        }
    }
    let spec_dir = place.spec_rel.rsplit_once('/').map_or("", |(d, _)| d);
    Ok(Record {
        spec: place.spec_rel.to_string(),
        root: tree::up_to(spec_dir),
        claims,
        files,
        ran,
        file_lines: BTreeMap::new(),
    })
}

/// The numbers of the line after the report: files, lines of code, lines some
/// claim ran.
pub fn counts(r: &Record) -> (usize, usize, usize) {
    let code: usize = r.files.iter().filter_map(|f| f.code.as_ref()).map(|c| c.len()).sum();
    let mut ran: BTreeMap<&str, Lines> = BTreeMap::new();
    for x in &r.ran {
        ran.entry(&x.file).or_default().extend(x.lines.iter().copied());
    }
    (r.files.len(), code, ran.values().map(|l| l.len()).sum())
}

/// The line after the report.
pub fn summary(r: &Record, path: &str, lang: Lang) -> String {
    let (files, code, ran) = counts(r);
    match lang {
        Lang::En => format!(
            "map: {} · {}, {ran} run by some claim → {path}\n",
            count(files, "file", "files"),
            count(code, "line of code", "lines of code")
        ),
        Lang::Ja => format!("記録: ファイル {files} 件 · コード {code} 行 · どれかの主張が通った行 {ran} 行 → {path}\n"),
    }
}

/// `"map":{…}` of the JSON answer.
pub fn json_part(r: &Option<Record>, path: &str) -> String {
    match r {
        Some(r) => {
            let (files, code, ran) = counts(r);
            format!(
                "{{\"record\":{},\"files\":{files},\"code\":{code},\"ran\":{ran}}}",
                json::quote(path)
            )
        }
        None => "null".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_reads_back_as_written() {
        let r = Record {
            spec: "examples/calc/calc.geas".into(),
            root: "../..".into(),
            claims: vec![
                RecClaim { name: "adds".into(), status: "ok".into(), targets: vec!["calc".into()] },
                RecClaim { name: "fails \"quoted\"".into(), status: "fail".into(), targets: vec![] },
            ],
            files: vec![
                RecFile {
                    path: "examples/calc/calc.py".into(),
                    blob: "ce013625030ba8dba906f756967f9e9ca394464a".into(),
                    runtime: Runtime::Python,
                    code: Some(lines::from_ranges("1-4,6").unwrap()),
                },
                RecFile {
                    path: "examples/calc/old.py".into(),
                    blob: "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391".into(),
                    runtime: Runtime::Python,
                    code: None,
                },
            ],
            ran: vec![RecRan {
                claim: 0,
                target: "calc".into(),
                file: "examples/calc/calc.py".into(),
                lines: lines::from_ranges("1-3").unwrap(),
            }],
            file_lines: BTreeMap::new(),
        };
        let text = r.render();
        assert_eq!(
            text.lines().nth(2).unwrap(),
            "{\"file\":\"examples/calc/old.py\",\"blob\":\"e69de29bb2d1d6434b8b29ae775ad8c2e48c5391\",\"lang\":\"python\",\"code\":null}"
        );
        let back = Record::parse(&text).unwrap();
        assert_eq!(back.render(), text);
        assert_eq!(back.file_lines["examples/calc/calc.py"], 2);
    }

    #[test]
    fn records_that_do_not_read() {
        let head = "{\"geas_map\":1,\"spec\":\"a.geas\",\"root\":\".\",\"claims\":[{\"name\":\"c\",\"status\":\"ok\",\"targets\":[]}]}\n";
        assert_eq!(Record::parse("").unwrap_err().0, 0);
        assert_eq!(Record::parse("{\"geas_map\":2}\n").unwrap_err().0, 1);
        assert!(Record::parse("{\"geas_map\":2}\n").unwrap_err().1.en.contains("format 2"));
        assert_eq!(Record::parse(&format!("{head}not json\n")).unwrap_err().0, 2);
        let bad_blob = format!("{head}{{\"file\":\"a.py\",\"blob\":\"x\",\"lang\":\"python\",\"code\":null}}\n");
        assert_eq!(Record::parse(&bad_blob).unwrap_err().0, 2);
        let unknown_claim = format!(
            "{head}{{\"file\":\"a.py\",\"blob\":\"e69de29bb2d1d6434b8b29ae775ad8c2e48c5391\",\"lang\":\"python\",\"code\":\"1\"}}\n{{\"claim\":\"z\",\"target\":\"t\",\"file\":\"a.py\",\"ran\":\"1\"}}\n"
        );
        assert_eq!(Record::parse(&unknown_claim).unwrap_err().0, 3);
    }
}
