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
    /// How many calls the machine's traces made, each passed the state its own language
    /// answered to the call before (§15.148).
    pub calls: usize,
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

use crate::backend::{have, rust_target};

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
    run_with(dir, false)
}

/// The same, with the proof harnesses (§15.95). They are behind a flag because they are the
/// one pass whose cost a person would notice: two rules take 7 seconds over the vectors and
/// 33 with the proofs, and a rule that walks fifty elements is 27 of those seconds on its
/// own. The vectors are what `test` is for; the proofs are what `--proofs` asks for.
pub fn run_with(dir: &Path, proofs: bool) -> Result<Run, String> {
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
            // `<alias>.traces.jsonl` holds a machine's sequences of calls (§15.148).
            if !a.ends_with(".expected") && !a.ends_with(".refused") && !a.ends_with(".traces") {
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
            if !have(b.tool) {
                out.skipped.push(tr!(
                    "{} が無いので {} 側を飛ばしました",
                    "{} not found; skipped the {} side",
                    b.tool,
                    b.name
                ));
                return false;
            }
            if let Some(Err(why)) = b.ready.map(|r| r()) {
                out.skipped.push(why);
                return false;
            }
            true
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
    // The Rust runner once more as a WASI module under wasmtime (§15.63): the shape a host
    // that speaks through stdin and stdout gives a rule — Fastly Compute, Spin, a batch step.
    // Not Shopify Functions: those export a named function and read the input through the
    // platform's own host calls (§15.73). It needs wasmtime on the PATH and the
    // wasm32-wasip1 standard library in the toolchain; without either the pass is skipped and
    // said so, like a missing language, but not counted as one.
    let wasi_host = if present.iter().any(|b| b.wasi.is_some()) {
        if !have("wasmtime") {
            out.skipped.push(tr!("wasmtime が無いので WASI 側を飛ばしました", "wasmtime not found; skipped the WASI side"));
            false
        } else if !rust_target("wasm32-wasip1") {
            out.skipped.push(tr!(
                "wasm32-wasip1 の標準ライブラリが無いので WASI 側を飛ばしました（rustup target add wasm32-wasip1）",
                "the wasm32-wasip1 standard library is not installed; skipped the WASI side (rustup target add wasm32-wasip1)"
            ));
            false
        } else {
            true
        }
    } else {
        false
    };
    // The proof harnesses (§15.95), when `--proofs` asks for them. Not vectors: the model
    // checker reads the generated harnesses and either verifies them over the whole declared
    // domain or brings back the input that breaks one. Seconds where the vectors are
    // milliseconds — 7s to 33s on two rules, +155s on the corpus — which is why it is asked
    // for rather than assumed. Without kani on the PATH the pass is skipped
    // and said so, like a missing toolchain, but not counted as a missing language.
    let proof_host = if proofs && present.iter().any(|b| b.proof.is_some()) {
        if !have("kani") {
            out.skipped.push(tr!("kani が無いので証明を飛ばしました", "kani not found; skipped the proofs"));
            false
        } else {
            true
        }
    } else {
        false
    };
    // The rule as a function inside a database (§15.80): the door PostgREST and Supabase
    // turn into an endpoint. SQLite has no `CREATE FUNCTION`, so this is the one pass the
    // query's own runner cannot stand in for, and it needs a PostgreSQL to run on — `psql`
    // on the PATH and a server it can reach. Without one the pass is skipped and said so,
    // like a missing toolchain, but not counted as a missing language: SQL itself ran.
    let pg_host = if present.iter().any(|b| b.pg.is_some()) {
        if !have("psql") {
            out.skipped.push(tr!(
                "psql が無いので PostgreSQL 側を飛ばしました",
                "psql not found; skipped the PostgreSQL function side"
            ));
            false
        } else if !crate::backend::psql_ready() {
            out.skipped.push(tr!(
                "psql がサーバに繋がらないので PostgreSQL 側を飛ばしました（PGHOST・PGDATABASE）",
                "psql cannot reach a server; skipped the PostgreSQL function side (PGHOST, PGDATABASE)"
            ));
            false
        } else {
            true
        }
    } else {
        false
    };
    // The rule as a Connect service (§15.112): the shape a service another team calls gives a
    // rule. It needs three things this repository does not carry — protoc, the plugin that
    // writes the stubs, and the runtime the generated service imports — so without them the
    // pass is skipped and said so, like a missing toolchain, but not counted as a missing
    // language: Python itself ran.
    let connect_host = if present.iter().any(|b| b.connect.is_some()) {
        match crate::backend::connect_ready() {
            Ok(()) => true,
            Err(why) => {
                out.skipped.push(why);
                false
            }
        }
    } else {
        false
    };
    // The ASGI half of it needs one thing more, and it is the same kind of "not here" as the
    // rest: without uvicorn the two WSGI passes still run and the note says which half was
    // skipped.
    let asgi_host = connect_host
        && match crate::backend::asgi_ready() {
            Ok(()) => true,
            Err(why) => {
                out.skipped.push(why);
                false
            }
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
        // A machine's traces, and what each call should answer (§15.148).
        let traces_path = dir.join("vectors").join(format!("{alias}.traces.jsonl"));
        let traces_want = std::fs::read_to_string(dir.join("vectors").join(format!("{alias}.traces.expected.jsonl"))).ok();
        let calls = std::fs::read_to_string(&traces_path)
            .map(|s| s.lines().filter(|l| l.contains("\"step\":")).count())
            .unwrap_or(0);
        let pkg = alias.replace('_', "");
        for b in &present {
            // Not every rule is generated for every backend: a rule that walks a sequence is
            // written only where the walk can be (§15.56). What is not there is not run, and
            // saying "cannot start" about a file nobody wrote would read as a broken machine.
            if !wrote(dir, b.id, &crate::backend::stem(b, alias)) {
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
                // The traces: one call after another, each passed the state the runner's own
                // language answered to the one before, and the constants printed first.
                if diff.is_none() {
                    if let Some(tw) = &traces_want {
                        diff = match exec(plan, Some(&traces_path)) {
                            Err(f) => Some(f),
                            Ok(got) => (got != *tw).then(|| first_diff(&got, tw)),
                        };
                    }
                }
                diff
            };
            let diff = held(&(b.run)(alias, &pkg));
            out.results.push(Outcome { rule: alias.clone(), lang: b.name, via: "runner", vectors: n, refused: refused.len(), calls, diff });
            // The same runner as a WASI module, held to the same records (§15.63).
            if let (Some(w), true) = (b.wasi, wasi_host) {
                let diff = held(&w(alias, &pkg));
                out.results.push(Outcome { rule: alias.clone(), lang: b.name, via: "wasi", vectors: n, refused: refused.len(), calls, diff });
            }
            // The same rule as a function on a real PostgreSQL, held to the same records
            // (§15.80). What it proves that the query's runner cannot: the signature, the
            // declared return types, and that an input outside the declaration raises
            // instead of coming back with a number beside a column nobody read.
            if let (Some(f), true) = (b.pg, pg_host) {
                let diff = held(&f(alias));
                out.results.push(Outcome { rule: alias.clone(), lang: b.name, via: "function", vectors: n, refused: refused.len(), calls, diff });
            }
            // The proofs of the same rule (§15.95): what every input in the declared domain
            // does, rather than what the vectors do.
            if let (Some(f), true) = (b.proof, proof_host) {
                let plan = f(alias);
                let (h, diff) = via_proof(&dir.join(&plan.cwd), &plan);
                out.results.push(Outcome { rule: alias.clone(), lang: b.name, via: "proof", vectors: h, refused: 0, calls: 0, diff });
            }
            // The rule as an MCP tool answers the same vectors through `tools/call`, and its
            // answer is held to the same expected records (§15.44).
            if let Some(mcp) = b.mcp {
                for (via, http) in [("mcp", false), ("mcp-http", true)] {
                    let cwd = dir.join(&mcp(alias).cwd);
                    let traces = traces_want.as_ref().and_then(|_| std::fs::read_to_string(&traces_path).ok());
                    let diff = match via_mcp(&cwd, &mcp(alias), alias, &want, &refused, http, traces.as_deref()) {
                        Err(f) => Some(f),
                        Ok((got, got_traces)) => {
                            if got != want {
                                Some(first_diff(&got, &want))
                            } else {
                                // The records of the calls, without the constants' line.
                                let tw: String = traces_want
                                    .as_deref()
                                    .map(|t| t.lines().skip(1).map(|l| format!("{l}\n")).collect())
                                    .unwrap_or_default();
                                (got_traces != tw).then(|| first_diff(&got_traces, &tw))
                            }
                        }
                    };
                    out.results.push(Outcome {
                        rule: alias.clone(),
                        lang: b.name,
                        via,
                        vectors: n,
                        refused: refused.len(),
                        calls,
                        diff,
                    });
                }
            }
            // The same vectors once more, through the generated Connect service: the stubs
            // are built from the `.proto`, the runner stands the service up on a free port
            // and calls it, and what is compared is the record that came back over the
            // socket. Four times — the ASGI application and the WSGI one, each by POST and
            // by GET — because both are generated and the method declares itself free of
            // side effects (§15.112).
            if let (Some(f), true) = (b.connect, connect_host) {
                if let Some(proto) = proto_of(&dir, alias) {
                    for (via, flags) in [
                        ("connect-asgi", &[][..]),
                        ("connect-asgi-get", &["--get"][..]),
                        ("connect-wsgi", &["--wsgi"][..]),
                        ("connect-wsgi-get", &["--wsgi", "--get"][..]),
                    ] {
                        // The ASGI half is skipped on its own when uvicorn is not here.
                        if !flags.contains(&"--wsgi") && !asgi_host {
                            continue;
                        }
                        let mut plan = f(alias, &proto);
                        plan.args.extend(flags.iter().map(|s| (*s).to_string()));
                        let diff = held(&plan);
                        out.results.push(Outcome {
                            rule: alias.clone(),
                            lang: b.name,
                            via,
                            vectors: n,
                            refused: refused.len(),
                            calls,
                            diff,
                        });
                    }
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
        out.results.push(Outcome { rule: ROUND_HELPER.into(), lang: b.name, via: "runner", vectors: 0, refused: 0, calls: 0, diff });
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
    traces: Option<&str>,
) -> Result<(String, String), Failure> {
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
    // A machine's traces (§15.148). The tool carries no constants, so the line that asks for
    // them is passed over; each call is passed the state the tool answered to the one before.
    let mut got_traces = String::new();
    if let Some(tr) = traces {
        let mut carry: Option<(String, String)> = None;
        let mut state: Option<crate::json::Json> = None;
        let base = want.lines().count() + refused.len() + 2;
        for (k, line) in tr.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let v = crate::json::parse(line).map_err(broken)?;
            if v.get("machine").is_some() {
                let c = v.get("carry");
                let name = |w: &str| c.and_then(|c| c.get(w)).and_then(|x| x.as_str()).map(|x| x.to_string());
                carry = name("in").zip(name("out"));
                continue;
            }
            let (Some((cin, cout)), Some(inp)) = (carry.as_ref(), v.get("in")) else {
                return Err(broken(tr!("手順の行の形が違います", "a line of the traces is not shaped right")));
            };
            if v.get("step").and_then(|x| x.as_str()) == Some("start") {
                state = v.get("state").cloned();
            }
            let Some(st) = state.clone() else {
                return Err(broken(tr!("手順が状態なしで始まりました", "a trace began with no state")));
            };
            let mut args = match inp {
                crate::json::Json::Obj(o) => o.clone(),
                _ => return Err(broken(tr!("手順の in がオブジェクトではありません", "a trace's in is not an object"))),
            };
            args.insert(cin.clone(), st);
            let r = wire.ask(&format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":\"tools/call\",\"params\":{{\"name\":{},\"arguments\":{}}}}}",
                base + k,
                crate::json::quote(alias),
                crate::json::unparse(&crate::json::Json::Obj(args))
            ))?;
            let text = r
                .get("content")
                .and_then(|c| match c {
                    crate::json::Json::Arr(a) => a.first(),
                    _ => None,
                })
                .and_then(|c| c.get("text"))
                .and_then(|t| t.as_str())
                .ok_or_else(|| broken(tr!("tools/call の答えに本文がありません", "the tools/call answer has no text")))?
                .to_string();
            let rec = crate::json::parse(&text).map_err(broken)?;
            state = rec.get("observed").and_then(|o| o.get(cout)).cloned();
            got_traces.push_str(&text);
            got_traces.push('\n');
        }
    }
    drop(wire);
    // Closing stdin is what ends the stdio server; the HTTP one waits for connections until
    // it is told to stop.
    let _ = child.kill();
    let _ = child.wait();
    Ok((got, got_traces))
}

/// The `.proto` of one rule, as a path under `proto/`.
///
/// The directory carries the rule's version (§15.112), which the alias does not, so it is
/// read off the tree rather than spelled.
fn proto_of(dir: &Path, alias: &str) -> Option<String> {
    let base = dir.join("proto").join("rulec").join(alias);
    let ver = std::fs::read_dir(&base).ok()?.flatten().find(|e| e.path().is_dir())?;
    let v = ver.file_name().to_string_lossy().into_owned();
    base.join(&v).join(format!("{alias}.proto")).is_file().then(|| format!("rulec/{alias}/{v}/{alias}.proto"))
}

/// Whether this backend has anything for this rule under the output directory.
fn wrote(dir: &Path, id: &str, stem: &str) -> bool {
    let d = dir.join(id);
    let named = |p: &Path| -> bool {
        p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(stem))
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
                let mut n = if x.via == "proof" {
                    tr!("ハーネス {} 本", "{} harnesses", x.vectors)
                } else if x.vectors > 0 {
                    tr!("ベクタ {} 件", "{} vectors", x.vectors)
                } else {
                    tr!("単体ベクタ", "unit vectors")
                };
                if x.refused > 0 {
                    n.push_str(&tr!("、断る入力 {} 件", ", {} refused", x.refused));
                }
                if x.calls > 0 {
                    n.push_str(&tr!("、手順の呼び出し {} 回", ", {} calls in traces", x.calls));
                }
                o.push_str(&format!("ok    {} ({}{}) {n}\n", shown(&x.rule), x.lang, via(x)));
            }
            Some(d) => {
                bad += 1;
                let text = d.text();
                o.push_str(&if x.via == "proof" {
                    tr!(
                        "FAIL  {} ({}{}) 証明が通りませんでした\n    {text}\n",
                        "FAIL  {} ({}{}) a harness did not verify\n    {text}\n",
                        shown(&x.rule),
                        x.lang,
                        via(x)
                    )
                } else if d.ran() {
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

/// Run one proof plan. The checker says how it went on stdout and exits non-zero when a
/// harness fails, so the text is read either way: how many harnesses ran, for the report,
/// and which ones did not verify, for the failure.
fn via_proof(cwd: &Path, plan: &crate::backend::Plan) -> (usize, Option<Failure>) {
    let out = match Command::new(&plan.cmd).current_dir(cwd).args(&plan.args).envs(closed()).output() {
        Err(e) => return (0, Some(Failure::Broken(tr!("起動できません: {e}", "cannot start: {e}")))),
        Ok(o) => o,
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let n = text.matches("VERIFICATION:- ").count();
    if out.status.success() && !text.contains("VERIFICATION:- FAILED") {
        return (n, None);
    }
    let failed: Vec<String> =
        text.lines().filter(|l| l.contains("Verification failed for")).map(|l| l.trim().to_string()).collect();
    let why = if failed.is_empty() {
        let e = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if e.is_empty() { text.lines().rev().take(3).collect::<Vec<_>>().join(" ") } else { e }
    } else {
        failed.join("\n    ")
    };
    (n, Some(Failure::Lines(why)))
}

/// The suffix that says a result came through the generated MCP server.
fn via(x: &Outcome) -> &'static str {
    match x.via {
        "mcp" => ", MCP",
        "connect-asgi" => ", Connect/ASGI",
        "connect-asgi-get" => ", Connect/ASGI+GET",
        "connect-wsgi" => ", Connect/WSGI",
        "connect-wsgi-get" => ", Connect/WSGI+GET",
        "mcp-http" => ", MCP/HTTP",
        "wasi" => ", WASI",
        "function" => ", PostgreSQL",
        "proof" => ", proof",
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
                .int("calls", x.calls as i128)
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
