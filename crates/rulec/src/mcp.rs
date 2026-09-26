//! `rulec mcp`: the command table as MCP tools, over stdio (§15.38).
//!
//! §12.1 put every command and flag in one table, with its purpose, its values and its exit
//! codes. An MCP server is that table in another syntax — one tool per command, one property
//! per flag — plus the documents an agent reads first, as resources. Nothing here knows how a
//! command works: a call runs this same binary with the arguments the table validates, and
//! hands back what it printed and its exit code. There is no second implementation of any
//! command to drift.
//!
//! The transport is the stdio one: one JSON-RPC 2.0 message per line, in and out.

use super::{commands, global_flags, Cmd};
use rulec::json::{self, Json, Obj};
use rulec::tr;
use std::io::{BufRead, Write};
use std::process::ExitCode;

/// The documents served as resources, embedded at build time so the server never serves a
/// version of them that differs from the binary. The links between them are rewritten to the
/// resource URIs.
const RESOURCES: &[(&str, &str, &str, &str)] = &[
    (
        "rulec://docs/agents.md",
        "agents",
        "The procedure for an agent: write, check, fix, generate, integrate, show the impact, ask a person. Read this first.",
        include_str!("../AGENTS.md"),
    ),
    ("rulec://docs/reference.md", "reference", "The complete grammar of a .rule file.", include_str!("../docs/reference.md")),
    ("rulec://docs/formats.md", "formats", "Every machine-readable format: --format json, vectors, fixtures, the manifest, the adapter protocol.", include_str!("../docs/formats.md")),
    ("rulec://docs/generated-code.md", "generated-code", "The shape and guarantees of the generated code in each language, and how to call it.", include_str!("../docs/generated-code.md")),
    ("rulec://docs/backends.md", "backends", "Targeting a language rulec does not generate, without losing the comparison.", include_str!("../docs/backends.md")),
];

pub fn serve(limit: std::time::Duration) -> ExitCode {
    let cmds = commands();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let req = match json::parse(line.trim()) {
            Ok(j) => j,
            Err(e) => {
                let _ = writeln!(out, "{}", error_msg("null", -32700, &format!("parse error: {e}")));
                let _ = out.flush();
                continue;
            }
        };
        // A notification carries no id and gets no answer.
        let Some(id) = req.get("id") else { continue };
        let id = json::unparse(id);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params");
        let answer = match method {
            "initialize" => Ok(initialize(params)),
            "ping" => Ok("{}".to_string()),
            "tools/list" => Ok(tools_list(&cmds)),
            "tools/call" => tools_call(&cmds, params, limit),
            "resources/list" => Ok(resources_list()),
            "resources/read" => resources_read(params),
            "prompts/list" => Ok("{\"prompts\":[]}".to_string()),
            _ => Err((-32601, format!("unknown method: {method}"))),
        };
        let msg = match answer {
            Ok(r) => format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{r}}}"),
            Err((code, m)) => error_msg(&id, code, &m),
        };
        let _ = writeln!(out, "{msg}");
        let _ = out.flush();
    }
    ExitCode::from(0)
}

fn error_msg(id: &str, code: i64, m: &str) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{{\"code\":{code},\"message\":{}}}}}", json_str(m))
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 32 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn initialize(params: Option<&Json>) -> String {
    let version = params
        .and_then(|p| p.get("protocolVersion"))
        .and_then(|v| v.as_str())
        .unwrap_or("2025-06-18");
    Obj::new()
        .str("protocolVersion", version)
        .raw("capabilities", "{\"tools\":{},\"resources\":{}}")
        .raw(
            "serverInfo",
            Obj::new().str("name", "rulec").str("version", env!("CARGO_PKG_VERSION")).finish(),
        )
        .str(
            "instructions",
            "rulec turns a table-shaped business rule (a .rule file) into proved, dependency-free code. \
             Read the resource rulec://docs/agents.md first: it is the procedure, and it says what an \
             agent must not decide on its own. Every tool is one rulec command; pass format \"json\" \
             to get its findings as data. The result's last text says the exit code: 0 no errors, \
             1 findings, 2 bad arguments.",
        )
        .finish()
}

/// The positional arguments a command takes, as (property, description, required, is a
/// list, the values it takes when they are a closed set). This is the one place that reads
/// `Cmd::args`, which is written for a person. A usage line that opens with `a|b|c` names a
/// subcommand, and that becomes `subcommand`, one of those words, ahead of the file.
fn positionals(c: &Cmd) -> Vec<(&'static str, String, bool, bool, Vec<&'static str>)> {
    let help = |i: usize| c.params.get(i).map(|(_, h)| h.clone()).unwrap_or_default();
    let first = c.args.split_whitespace().next().unwrap_or("");
    match c.name {
        "explain" => vec![("code", help(0), false, false, vec![])],
        "test" => vec![("dir", help(0), true, false, vec![])],
        "fixtures" => vec![("fixtures", help(1), true, false, vec![]), ("rule", help(2), true, false, vec![])],
        "diff" => vec![("old", help(0), true, false, vec![]), ("new", help(1), true, false, vec![])],
        "mcp" => vec![],
        _ if first.contains('|') && !first.starts_with('<') => vec![
            ("subcommand", help(0), true, false, first.split('|').collect()),
            ("file", help(1), true, false, vec![]),
        ],
        _ if c.args.ends_with("...") => vec![("files", help(0), true, true, vec![])],
        _ if c.args.is_empty() => vec![],
        _ => vec![("file", help(0), true, false, vec![])],
    }
}

fn prop_name(flag: &str) -> String {
    flag.trim_start_matches("--").replace('-', "_")
}

fn tools_list(cmds: &[Cmd]) -> String {
    let globals = global_flags();
    let tools: Vec<String> = cmds
        .iter()
        .filter(|c| c.name != "mcp")
        .map(|c| {
            let mut props: Vec<String> = Vec::new();
            let mut required: Vec<String> = Vec::new();
            for (name, desc, req, list, choices) in positionals(c) {
                let ty = if list {
                    "\"type\":\"array\",\"items\":{\"type\":\"string\"}".to_string()
                } else if !choices.is_empty() {
                    format!("\"type\":\"string\",\"enum\":{}", json::strs(&choices))
                } else {
                    "\"type\":\"string\"".to_string()
                };
                props.push(format!("{}:{{{ty},\"description\":{}}}", json_str(name), json_str(&desc)));
                if req {
                    required.push(name.to_string());
                }
            }
            for f in c.flags.iter().chain(globals.iter().filter(|g| g.name == "--lang")) {
                let mut o = String::new();
                if f.value.is_none() {
                    o.push_str("\"type\":\"boolean\"");
                } else if f.repeat {
                    o.push_str("\"type\":\"array\",\"items\":{\"type\":\"string\"}");
                } else {
                    o.push_str("\"type\":\"string\"");
                    if !f.choices.is_empty() {
                        o.push_str(&format!(",\"enum\":{}", json::strs(f.choices)));
                    }
                }
                let mut help = f.help.clone();
                if f.rest {
                    help.push_str(" (one string; it is split on spaces into the command and its arguments)");
                }
                o.push_str(&format!(",\"description\":{}", json_str(&help)));
                props.push(format!("{}:{{{o}}}", json_str(&prop_name(f.name))));
            }
            let mut desc = c.purpose.clone();
            desc.push_str(". Exit codes: ");
            desc.push_str(&c.exits.iter().map(|(n, h)| format!("{n} = {h}")).collect::<Vec<_>>().join("; "));
            if !c.examples.is_empty() {
                desc.push_str(". For example: ");
                desc.push_str(&c.examples.join(" / "));
            }
            let schema = format!(
                "{{\"type\":\"object\",\"properties\":{{{}}},\"required\":{}}}",
                props.join(","),
                json::strs(&required)
            );
            Obj::new()
                .str("name", &format!("rulec_{}", c.name))
                .str("description", &desc)
                .raw("inputSchema", schema)
                .finish()
        })
        .collect();
    format!("{{\"tools\":{}}}", json::arr(&tools))
}

/// Run the command this binary would run for the same arguments, and hand back what it
/// printed. The exit code travels as the last piece of text, since an agent is told to read
/// it rather than the emptiness of the output.
fn tools_call(cmds: &[Cmd], params: Option<&Json>, limit: std::time::Duration) -> Result<String, (i64, String)> {
    let name = params.and_then(|p| p.get("name")).and_then(|n| n.as_str()).unwrap_or("");
    let Some(c) = name.strip_prefix("rulec_").and_then(|n| cmds.iter().find(|c| c.name == n)) else {
        return Err((-32602, format!("unknown tool: {name}")));
    };
    let empty = std::collections::BTreeMap::new();
    let args = params.and_then(|p| p.get("arguments")).and_then(|a| a.as_obj()).unwrap_or(&empty);
    let mut argv: Vec<String> = vec![c.name.to_string()];
    if c.name == "fixtures" {
        argv.push("lint".into());
    }
    let text = |v: &Json| -> Result<String, (i64, String)> {
        match v {
            Json::Str(s) => Ok(s.clone()),
            Json::Int(n) => Ok(n.to_string()),
            Json::Frac(s) => Ok(s.clone()),
            Json::Bool(b) => Ok(b.to_string()),
            other => Err((-32602, format!("a {} is not a value an argument takes", other.kind()))),
        }
    };
    let pos = positionals(c);
    for (pname, _, req, list, choices) in &pos {
        match args.get(*pname) {
            Some(Json::Arr(items)) if *list => {
                for it in items {
                    argv.push(text(it)?);
                }
            }
            Some(v) if !*list => {
                let v = text(v)?;
                if !choices.is_empty() && !choices.contains(&v.as_str()) {
                    return Err((-32602, format!("`{pname}` is one of {}", choices.join(", "))));
                }
                argv.push(v);
            }
            Some(_) => return Err((-32602, format!("`{pname}` has the wrong shape"))),
            None if *req => return Err((-32602, format!("`{pname}` is required"))),
            None => {}
        }
    }
    let globals = global_flags();
    for (k, v) in args {
        if pos.iter().any(|(p, ..)| p == k) {
            continue;
        }
        let flag_name = format!("--{}", k.replace('_', "-"));
        let Some(f) = c.flags.iter().chain(globals.iter()).find(|f| f.name == flag_name) else {
            return Err((-32602, format!("`{k}` is not an argument of rulec {}", c.name)));
        };
        if f.rest {
            argv.push(f.name.to_string());
            argv.extend(text(v)?.split_whitespace().map(|s| s.to_string()));
        } else if f.value.is_none() {
            match v {
                Json::Bool(true) => argv.push(f.name.to_string()),
                Json::Bool(false) | Json::Null => {}
                _ => return Err((-32602, format!("`{k}` is a boolean"))),
            }
        } else if let Json::Arr(items) = v {
            for it in items {
                argv.push(f.name.to_string());
                argv.push(text(it)?);
            }
        } else {
            argv.push(f.name.to_string());
            argv.push(text(v)?);
        }
    }
    let exe = std::env::current_exe().map_err(|e| (-32603, format!("cannot find rulec itself: {e}")))?;
    let mut cmd = std::process::Command::new(exe);
    cmd.args(&argv);
    let ran = run_limited(cmd, limit).map_err(|e| (-32603, format!("cannot run rulec: {e}")))?;
    let mut body = String::from_utf8_lossy(&ran.stdout).into_owned();
    let err = String::from_utf8_lossy(&ran.stderr);
    if !err.trim().is_empty() {
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(&err);
    }
    // 0 and 1 are answers (§11 principle 6); anything else is the call failing, and a call
    // that did not finish, or was stopped by a signal, is not reported as a bad argument.
    let (status, failed) = match ran.code {
        _ if ran.timed_out => (
            tr!(
                "止めました: rulec {} が上限の {} 秒を過ぎても終わりませんでした（上限は `rulec mcp --timeout <秒>` で変えられます）",
                "stopped: rulec {} ran past the limit of {} s (`rulec mcp --timeout <seconds>` sets it)",
                c.name,
                limit.as_secs()
            ),
            true,
        ),
        Some(code) => (format!("exit code {code}"), !matches!(code, 0 | 1)),
        None => (tr!("シグナルで止まりました", "stopped by a signal"), true),
    };
    let content = vec![
        Obj::new().str("type", "text").str("text", &body).finish(),
        Obj::new().str("type", "text").str("text", &status).finish(),
    ];
    Ok(Obj::new().raw("content", json::arr(&content)).bool("isError", failed).finish())
}

/// What one run of rulec left behind.
struct Ran {
    code: Option<i32>,
    timed_out: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Run one command and wait for it at most `limit`. A command that does not finish is
/// killed and its call answered as failed: before this, one call that never finished held the
/// server for good, and every call after it waited behind it (§15.156).
///
/// The output goes to two files rather than pipes. `rulec test` starts compilers of its own,
/// and one still running after its parent was killed would hold a pipe open, and a read of it
/// would wait for that compiler instead.
fn run_limited(mut cmd: std::process::Command, limit: std::time::Duration) -> std::io::Result<Ran> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!("rulec-mcp-{}-{n}", std::process::id()));
    let (out_path, err_path) = (base.with_extension("out"), base.with_extension("err"));
    let out = std::fs::File::create(&out_path)?;
    let err = std::fs::File::create(&err_path)?;
    // A group of its own, so that what it started — an adapter `verify` stood up, a compiler
    // `test` ran — is stopped with it, not left running on its own.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let mut child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(out))
        .stderr(std::process::Stdio::from(err))
        .spawn()?;
    let start = std::time::Instant::now();
    let (code, timed_out) = loop {
        if let Some(s) = child.try_wait()? {
            break (s.code(), false);
        }
        if start.elapsed() >= limit {
            #[cfg(unix)]
            let _ = std::process::Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            let _ = child.kill();
            let _ = child.wait();
            break (None, true);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let stdout = std::fs::read(&out_path).unwrap_or_default();
    let stderr = std::fs::read(&err_path).unwrap_or_default();
    let _ = std::fs::remove_file(&out_path);
    let _ = std::fs::remove_file(&err_path);
    Ok(Ran { code, timed_out, stdout, stderr })
}

fn resources_list() -> String {
    let rs: Vec<String> = RESOURCES
        .iter()
        .map(|(uri, name, desc, _)| {
            Obj::new().str("uri", uri).str("name", name).str("description", desc).str("mimeType", "text/markdown").finish()
        })
        .collect();
    format!("{{\"resources\":{}}}", json::arr(&rs))
}

fn resources_read(params: Option<&Json>) -> Result<String, (i64, String)> {
    let uri = params.and_then(|p| p.get("uri")).and_then(|u| u.as_str()).unwrap_or("");
    let Some((_, _, _, text)) = RESOURCES.iter().find(|(u, ..)| *u == uri) else {
        return Err((-32002, format!("no such resource: {uri}")));
    };
    // AGENTS.md links its companions as `docs/…`; here they are resources.
    let text = text.replace("](docs/", "](rulec://docs/");
    let one = Obj::new().str("uri", uri).str("mimeType", "text/markdown").str("text", &text).finish();
    Ok(format!("{{\"contents\":[{one}]}}"))
}
