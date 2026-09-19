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

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};

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
    /// How the generated code was reached: `runner` (the vectors piped through the
    /// generated runner) or `mcp` (the same vectors, one `tools/call` each, through the
    /// generated server; §15.44).
    pub via: &'static str,
    pub vectors: usize,
    /// How many inputs the reference evaluator refuses were put to it. The expected answer
    /// there is a refusal, so they are counted apart from the vectors (§15.56).
    pub refused: usize,
    /// Why it failed. None when everything matched.
    pub diff: Option<Failure>,
}

pub struct Run {
    pub results: Vec<Outcome>,
    /// Every "this was not run" note: a language whose toolchain is missing, or the Wasm pass.
    pub skipped: Vec<String>,
    /// How many of the languages were skipped, for the summary's scope.
    pub missing_langs: usize,
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

/// Whether the toolchain can build for `wasm32-wasip1`: the target's standard library sits
/// under the sysroot when `rustup target add wasm32-wasip1` has been run.
fn wasi_target() -> bool {
    let Ok(o) = Command::new("rustc").args(["--print", "sysroot"]).output() else { return false };
    if !o.status.success() {
        return false;
    }
    let root = String::from_utf8_lossy(&o.stdout).trim().to_string();
    Path::new(&root).join("lib").join("rustlib").join("wasm32-wasip1").join("lib").is_dir()
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
            // `<alias>.expected.jsonl` holds the answers and `<alias>.refused.jsonl` the
            // inputs with none; neither is a rule of its own.
            if !a.ends_with(".expected") && !a.ends_with(".refused") {
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
    let mut out = Run { results: Vec::new(), skipped: Vec::new(), missing_langs: 0 };
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
    out.missing_langs = out.skipped.len();
    if present.is_empty() {
        return Err(tr!(
            "どの toolchain も無いので、生成物を走らせられません（{}）",
            "none of the toolchains is available, so the generated code cannot be run ({})",
            crate::backend::ALL.iter().map(|b| b.tool).collect::<Vec<_>>().join(", ")
        ));
    }
    // The Rust runner once more as a WASI module under wasmtime (§15.63): the host a Shopify
    // Function or an Extism plugin gives a rule. It needs wasmtime on the PATH and the
    // wasm32-wasip1 standard library in the toolchain; without either the pass is skipped and
    // said so, like a missing language, but not counted as one.
    let wasm_host = if present.iter().any(|b| b.wasm.is_some()) {
        if !have("wasmtime") {
            out.skipped.push(tr!("wasmtime が無いので Wasm 側を飛ばしました", "wasmtime not found; skipped the Wasm side"));
            false
        } else if !wasi_target() {
            out.skipped.push(tr!(
                "wasm32-wasip1 の標準ライブラリが無いので Wasm 側を飛ばしました（rustup target add wasm32-wasip1）",
                "the wasm32-wasip1 standard library is not installed; skipped the Wasm side (rustup target add wasm32-wasip1)"
            ));
            false
        } else {
            true
        }
    } else {
        false
    };
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
        // Written only for a rule that has any; a rule without one has no file (§15.56).
        let refused: Vec<String> = std::fs::read_to_string(dir.join("vectors").join(format!("{alias}.refused.jsonl")))
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).map(|l| l.to_string()).collect())
            .unwrap_or_default();
        let pkg = alias.replace('_', "");
        for b in &present {
            // Not every rule is generated for every backend: a rule that walks a sequence is
            // written only where the walk can be (§15.56). What is not there is not run, and
            // saying "cannot start" about a file nobody wrote would read as a broken machine.
            if !wrote(dir, b.id, alias) {
                continue;
            }
            // One plan held to the rule: the vectors through it, then each refused input on its
            // own — the generated code raises on the first one it is given, so a file of them
            // would only ever prove the first (§15.56).
            let held = |plan: &crate::backend::Plan| -> Option<Failure> {
                let mut diff = match exec(plan, Some(&vec_path)) {
                    Err(f) => Some(f),
                    Ok(got) => (got != want).then(|| first_diff(&got, &want)),
                };
                if diff.is_none() {
                    for (k, line) in refused.iter().enumerate() {
                        let one = dir.join("vectors").join(format!(".{alias}.refused.{k}.jsonl"));
                        if std::fs::write(&one, format!("{line}\n")).is_err() {
                            diff = Some(broken(tr!("断る入力を書けません", "cannot write the refused input")));
                            break;
                        }
                        let got = exec(plan, Some(&one));
                        let _ = std::fs::remove_file(&one);
                        match got {
                            // Refusing is what was asked for: the run stopped without an answer.
                            Err(Failure::Broken(_)) => {}
                            Err(f) => {
                                diff = Some(f);
                                break;
                            }
                            Ok(said) => {
                                diff = Some(Failure::Lines(tr!(
                                    "参照評価器が断る入力に答えました（{}行目）: {}",
                                    "answered an input the reference evaluator refuses (line {}): {}",
                                    k + 1,
                                    said.trim()
                                )));
                                break;
                            }
                        }
                    }
                }
                diff
            };
            let diff = held(&(b.run)(alias, &pkg));
            out.results.push(Outcome { rule: alias.clone(), lang: b.name, via: "runner", vectors: n, refused: refused.len(), diff });
            // The same runner as a WASI module, held to the same records (§15.63).
            if let (Some(w), true) = (b.wasm, wasm_host) {
                let diff = held(&w(alias, &pkg));
                out.results.push(Outcome { rule: alias.clone(), lang: b.name, via: "wasm", vectors: n, refused: refused.len(), diff });
            }
            // The rule as an MCP tool answers the same vectors through `tools/call`, and its
            // answer is held to the same expected records (§15.44).
            if let Some(mcp) = b.mcp {
                for (via, http) in [("mcp", false), ("mcp-http", true)] {
                    let cwd = dir.join(&mcp(alias).cwd);
                    let diff = match via_mcp(&cwd, &mcp(alias), alias, &want, &refused, http) {
                        Err(f) => Some(f),
                        Ok(got) => (got != want).then(|| first_diff(&got, &want)),
                    };
                    out.results.push(Outcome {
                        rule: alias.clone(),
                        lang: b.name,
                        via,
                        vectors: n,
                        refused: refused.len(),
                        diff,
                    });
                }
            }
        }
    }

    // Unit vectors of the rounding helpers. Every rule emits the same ones, so run just one.
    let pkg0 = aliases[0].replace('_', "");
    for b in &present {
        // Same reason as above: a backend nothing was written for has no helpers either.
        if !dir.join(b.id).exists() {
            continue;
        }
        let diff = exec(&(b.round)(&pkg0), None).err();
        out.results.push(Outcome { rule: ROUND_HELPER.into(), lang: b.name, via: "runner", vectors: 0, refused: 0, diff });
    }

    Ok(out)
}

/// Drive the generated MCP server as a client would: `initialize`, `tools/list`, then one
/// `tools/call` per expected record with its `in` as the arguments. The text of every result
/// is the record line the runner would have printed, so the caller compares the two the
/// same way.
///
/// `http` picks the transport (§15.51). The conversation below is the same either way, which
/// is the whole point of running both: what differs is the carrying, and the answers must
/// not differ with it.
fn via_mcp(
    cwd: &Path,
    plan: &crate::backend::Plan,
    alias: &str,
    want: &str,
    refused: &[String],
    http: bool,
) -> Result<String, Failure> {
    let mut cmd = Command::new(&plan.cmd);
    cmd.current_dir(cwd)
        .args(&plan.args)
        .envs(closed())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if http {
        // Port 0: the server takes a free one and says which.
        cmd.arg("--http").arg("127.0.0.1:0");
    }
    let mut child = cmd.spawn().map_err(|e| broken(tr!("起動できません: {e}", "cannot start: {e}")))?;
    let si = child.stdin.take().ok_or_else(|| broken("stdin".into()))?;
    let mut so = BufReader::new(child.stdout.take().ok_or_else(|| broken("stdout".into()))?);

    let mut wire: Box<dyn Wire> = if http {
        let mut line = String::new();
        if so.read_line(&mut line).map_err(|e| broken(e.to_string()))? == 0 {
            return Err(broken(tr!("HTTP の MCP サーバが待ち受けませんでした", "the HTTP MCP server never started listening")));
        }
        let at = line.trim().trim_start_matches("http://").trim_end_matches("/mcp").to_string();
        Box::new(HttpWire::connect(&at)?)
    } else {
        Box::new(StdioWire { si, so })
    };

    let init = wire.ask(
        "{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"rulec test\",\"version\":\"0\"}}}",
    )?;
    if init.get("serverInfo").is_none() {
        return Err(broken(tr!("initialize の答えに serverInfo がありません", "the initialize answer has no serverInfo")));
    }
    wire.notify("{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}")?;
    let tools = wire.ask("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{}}")?;
    let one = match tools.get("tools") {
        Some(crate::json::Json::Arr(ts)) if ts.len() == 1 => ts[0].get("name").and_then(|n| n.as_str()) == Some(alias),
        _ => false,
    };
    if !one {
        return Err(broken(tr!(
            "tools/list が `{alias}` 一つを出していません: {}",
            "tools/list does not list `{alias}` alone: {}",
            crate::json::unparse(&tools)
        )));
    }
    let mut got = String::new();
    for (k, line) in want.lines().enumerate() {
        let exp = crate::json::parse(line).map_err(broken)?;
        let Some(inp) = exp.get("in") else {
            return Err(broken(tr!("期待値に in がありません", "an expected record has no in")));
        };
        let r = wire.ask(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":\"tools/call\",\"params\":{{\"name\":{},\"arguments\":{}}}}}",
            k + 2,
            crate::json::quote(alias),
            crate::json::unparse(inp)
        ))?;
        let text = r
            .get("content")
            .and_then(|c| match c {
                crate::json::Json::Arr(a) => a.first(),
                _ => None,
            })
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| broken(tr!("tools/call の答えに本文がありません", "the tools/call answer has no text")))?;
        if r.get("isError") == Some(&crate::json::Json::Bool(true)) {
            return Err(broken(tr!("tools/call が {} 件目で断りました: {text}", "tools/call refused record {}: {text}", k + 1)));
        }
        got.push_str(text);
        got.push('\n');
    }
    // The inputs the reference evaluator refuses. A server stays up across an error, so these
    // go through the same connection: what is asked of it is `isError`, not a record (§15.56).
    for (k, line) in refused.iter().enumerate() {
        let one = crate::json::parse(line).map_err(broken)?;
        let Some(inp) = one.get("in") else {
            return Err(broken(tr!("断る入力に in がありません", "a refused input has no in")));
        };
        let r = wire.ask(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":\"tools/call\",\"params\":{{\"name\":{},\"arguments\":{}}}}}",
            want.lines().count() + k + 2,
            crate::json::quote(alias),
            crate::json::unparse(inp)
        ))?;
        if r.get("isError") != Some(&crate::json::Json::Bool(true)) {
            return Err(Failure::Lines(tr!(
                "参照評価器が断る入力に答えました（{}行目）",
                "answered an input the reference evaluator refuses (line {})",
                k + 1
            )));
        }
    }
    drop(wire);
    // Closing stdin is what ends the stdio server; the HTTP one waits for connections until
    // it is told to stop.
    let _ = child.kill();
    let _ = child.wait();
    Ok(got)
}

/// Whether this backend has anything for this rule under the output directory.
fn wrote(dir: &Path, id: &str, alias: &str) -> bool {
    let d = dir.join(id);
    let named = |p: &Path| -> bool {
        p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(alias))
    };
    let Ok(rd) = std::fs::read_dir(&d) else { return false };
    rd.flatten().any(|e| {
        let p = e.path();
        // Go puts the module in a directory of its own, named after the package.
        named(&p) || (p.is_dir() && std::fs::read_dir(&p).map(|r| r.flatten().any(|x| named(&x.path()))).unwrap_or(false))
    })
}

fn broken(m: String) -> Failure {
    Failure::Broken(m)
}

/// The `result` of one answer, or the refusal as a failure.
fn result_of(line: &str) -> Result<crate::json::Json, Failure> {
    let j = crate::json::parse(line.trim()).map_err(|e| {
        broken(tr!(
            "MCP サーバの答えが JSON ではありません: {e}\n{line}",
            "the MCP server's answer is not JSON: {e}\n{line}"
        ))
    })?;
    if let Some(e) = j.get("error") {
        return Err(broken(tr!("MCP サーバが断りました: {}", "the MCP server refused: {}", crate::json::unparse(e))));
    }
    match j.get("result") {
        Some(r) => Ok(r.clone()),
        None => Err(broken(tr!("MCP サーバの答えに result がありません", "the MCP server's answer has no result"))),
    }
}

/// How a request reaches the server and how the answer comes back.
trait Wire {
    fn ask(&mut self, req: &str) -> Result<crate::json::Json, Failure>;
    /// A message with no id: the server must not answer it.
    fn notify(&mut self, req: &str) -> Result<(), Failure>;
}

struct StdioWire {
    si: std::process::ChildStdin,
    so: BufReader<std::process::ChildStdout>,
}

impl Wire for StdioWire {
    fn ask(&mut self, req: &str) -> Result<crate::json::Json, Failure> {
        self.notify(req)?;
        let mut line = String::new();
        loop {
            line.clear();
            if self.so.read_line(&mut line).map_err(|e| broken(e.to_string()))? == 0 {
                return Err(broken(tr!("MCP サーバが答えずに終わりました", "the MCP server ended without answering")));
            }
            if !line.trim().is_empty() {
                return result_of(&line);
            }
        }
    }

    fn notify(&mut self, req: &str) -> Result<(), Failure> {
        writeln!(self.si, "{req}").map_err(|e| broken(e.to_string()))?;
        self.si.flush().ok();
        Ok(())
    }
}

/// MCP's Streamable HTTP, written out: one POST per message, the answer's length stated, so
/// the same connection carries the whole conversation.
struct HttpWire {
    out: TcpStream,
    inn: BufReader<TcpStream>,
    at: String,
    session: Option<String>,
}

impl HttpWire {
    fn connect(at: &str) -> Result<HttpWire, Failure> {
        let out = TcpStream::connect(at).map_err(|e| broken(tr!("{at} に繋げません: {e}", "cannot connect to {at}: {e}")))?;
        let inn = BufReader::new(out.try_clone().map_err(|e| broken(e.to_string()))?);
        Ok(HttpWire { out, inn, at: at.to_string(), session: None })
    }

    fn post(&mut self, body: &str) -> Result<(u16, String), Failure> {
        let mut head = format!(
            "POST /mcp HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nMCP-Protocol-Version: 2025-06-18\r\nContent-Length: {}\r\n",
            self.at,
            body.len()
        );
        // The session the server handed out at initialize goes back on every later message.
        if let Some(s) = &self.session {
            head.push_str(&format!("Mcp-Session-Id: {s}\r\n"));
        }
        head.push_str("\r\n");
        self.out
            .write_all(head.as_bytes())
            .and_then(|()| self.out.write_all(body.as_bytes()))
            .and_then(|()| self.out.flush())
            .map_err(|e| broken(tr!("送れません: {e}", "cannot send: {e}")))?;

        let mut line = String::new();
        if self.inn.read_line(&mut line).map_err(|e| broken(e.to_string()))? == 0 {
            return Err(broken(tr!("HTTP の答えがありません", "there was no HTTP answer")));
        }
        let status: u16 = line
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .ok_or_else(|| broken(tr!("HTTP の一行目が読めません: {line}", "the first line of the answer is not HTTP: {line}")))?;
        let mut len = 0usize;
        loop {
            let mut h = String::new();
            if self.inn.read_line(&mut h).map_err(|e| broken(e.to_string()))? == 0 {
                return Err(broken(tr!("HTTP の見出しが途中で切れました", "the HTTP headers ended in the middle")));
            }
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            let Some((k, v)) = h.split_once(':') else { continue };
            match k.to_ascii_lowercase().as_str() {
                "content-length" => len = v.trim().parse().unwrap_or(0),
                "mcp-session-id" => self.session = Some(v.trim().to_string()),
                _ => {}
            }
        }
        let mut buf = vec![0u8; len];
        self.inn.read_exact(&mut buf).map_err(|e| broken(e.to_string()))?;
        Ok((status, String::from_utf8_lossy(&buf).into_owned()))
    }
}

impl Wire for HttpWire {
    fn ask(&mut self, req: &str) -> Result<crate::json::Json, Failure> {
        let (status, body) = self.post(req)?;
        if status != 200 {
            return Err(broken(tr!("HTTP {status} が返りました: {body}", "the server answered HTTP {status}: {body}")));
        }
        result_of(&body)
    }

    fn notify(&mut self, req: &str) -> Result<(), Failure> {
        let (status, body) = self.post(req)?;
        // A message with no id has no answer, and the protocol says so with 202.
        if status != 202 {
            return Err(broken(tr!("通知に HTTP {status} が返りました: {body}", "a notification was answered with HTTP {status}: {body}")));
        }
        Ok(())
    }
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
                let mut n = if x.vectors > 0 {
                    tr!("ベクタ {} 件", "{} vectors", x.vectors)
                } else {
                    tr!("単体ベクタ", "unit vectors")
                };
                if x.refused > 0 {
                    n.push_str(&tr!("、断る入力 {} 件", ", {} refused", x.refused));
                }
                o.push_str(&format!("ok    {} ({}{}) {n}\n", shown(&x.rule), x.lang, via(x)));
            }
            Some(d) => {
                bad += 1;
                let text = d.text();
                o.push_str(&if d.ran() {
                    tr!(
                        "FAIL  {} ({}{}) 参照評価器と食い違います\n    {text}\n",
                        "FAIL  {} ({}{}) disagrees with the reference evaluator\n    {text}\n",
                        shown(&x.rule),
                        x.lang,
                        via(x)
                    )
                } else {
                    tr!(
                        "FAIL  {} ({}{}) 走らせられませんでした\n    {text}\n",
                        "FAIL  {} ({}{}) could not be run\n    {text}\n",
                        shown(&x.rule),
                        x.lang,
                        via(x)
                    )
                });
            }
        }
    }
    // What the run covered belongs in the summary. "All 2 matched." after four languages were
    // skipped reads as the whole claim holding, when the claim is that the reference evaluator
    // and **every** generated language agree.
    let scope = if r.missing_langs == 0 {
        String::new()
    } else {
        tr!(
            "（{} 言語中 {} 言語を飛ばしました）",
            " ({} of {} languages skipped)",
            r.missing_langs,
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

/// The suffix that says a result came through the generated MCP server.
fn via(x: &Outcome) -> &'static str {
    match x.via {
        "mcp" => ", MCP",
        "mcp-http" => ", MCP/HTTP",
        "wasm" => ", Wasm",
        _ => "",
    }
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
                .str("via", x.via)
                .int("vectors", x.vectors as i128)
                .int("refused", x.refused as i128)
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
