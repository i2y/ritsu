//! The rule as one MCP tool (§15.44): the generated `_mcp` server, driven as a client would.
//!
//! `rulec test` already holds the server to the reference evaluator over every vector. What
//! is checked here is the rest of the contract a calling agent meets: what `tools/list`
//! says, what a refused call says, and that `--record` keeps what was asked.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const RULE: &str = "tests/corpus/厚生年金保険料.rule";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn generate(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rulec-tool-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["gen", RULE, "--out", dir.to_str().unwrap()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    dir
}

/// Send the requests one line at a time and collect one answer per request that has an id.
fn talk(cwd: &Path, cmd: &str, args: &[&str], reqs: &[&str]) -> Vec<rulec::json::Json> {
    let mut child = Command::new(cmd)
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("サーバを起動できない");
    let mut si = child.stdin.take().unwrap();
    let mut so = BufReader::new(child.stdout.take().unwrap());
    let mut out = Vec::new();
    for r in reqs {
        writeln!(si, "{r}").unwrap();
        si.flush().unwrap();
        if !r.contains("\"id\"") {
            continue; // a notification gets no answer
        }
        let mut line = String::new();
        assert!(so.read_line(&mut line).unwrap() > 0, "答えずに終わった: {r}");
        out.push(rulec::json::parse(line.trim()).unwrap_or_else(|e| panic!("JSON でない: {e}\n{line}")));
    }
    drop(si);
    let o = child.wait_with_output().unwrap();
    assert!(o.status.success(), "サーバが落ちた:\n{}", String::from_utf8_lossy(&o.stderr));
    out
}

fn text(j: &rulec::json::Json) -> String {
    j.get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| match c {
            rulec::json::Json::Arr(a) => a.first(),
            _ => None,
        })
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or_else(|| panic!("本文が無い: {}", rulec::json::unparse(j)))
        .to_string()
}

fn is_error(j: &rulec::json::Json) -> bool {
    j.get("result").and_then(|r| r.get("isError")) == Some(&rulec::json::Json::Bool(true))
}

const INIT: &str = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"t\",\"version\":\"0\"}}}";
const READY: &str = "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}";
const LIST: &str = "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}";
const OK_CALL: &str = "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"pension_premium\",\"arguments\":{\"報酬月額\":250000,\"料率\":183}}}";
const FLOAT_RATE: &str = "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"pension_premium\",\"arguments\":{\"報酬月額\":250000,\"料率\":18.3}}}";
const MISSING: &str = "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{\"name\":\"pension_premium\",\"arguments\":{\"報酬月額\":250000}}}";
const OUT_OF_RANGE: &str = "{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"tools/call\",\"params\":{\"name\":\"pension_premium\",\"arguments\":{\"報酬月額\":250000,\"料率\":400}}}";
const WRONG_TOOL: &str = "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"nope\",\"arguments\":{}}}";
const EXPECTED: &str = "{\"in\":{\"報酬月額\":250000,\"料率\":183},\"observed\":{\"標準報酬月額\":260000,\"給与控除額\":23790,\"現金納付額\":23790},\"trace\":[{\"table\":\"等級\",\"row\":17}]}";

/// The same contract in every language that gets a server.
fn contract(dir: &Path, cwd: &str, cmd: &str, args: &[&str]) {
    let record = dir.join("calls.jsonl");
    let mut args: Vec<&str> = args.to_vec();
    let rec = record.to_str().unwrap().to_string();
    args.extend(["--record", rec.as_str()]);
    let a = talk(&dir.join(cwd), cmd, &args, &[INIT, READY, LIST, OK_CALL, FLOAT_RATE, MISSING, OUT_OF_RANGE, WRONG_TOOL]);
    assert_eq!(a.len(), 7, "{}", a.len());

    // initialize: the server is the rule.
    let info = a[0].get("result").and_then(|r| r.get("serverInfo")).expect("serverInfo");
    assert_eq!(info.get("name").and_then(|n| n.as_str()), Some("pension_premium"));
    assert_eq!(info.get("version").and_then(|n| n.as_str()), Some("1"));

    // tools/list: one tool, whose inputSchema is the wire `in` of `rulec schema`, and whose
    // description says what a rate is.
    let tools = match a[1].get("result").and_then(|r| r.get("tools")) {
        Some(rulec::json::Json::Arr(t)) => t,
        other => panic!("tools が無い: {other:?}"),
    };
    assert_eq!(tools.len(), 1);
    let tool = &tools[0];
    assert_eq!(tool.get("name").and_then(|n| n.as_str()), Some("pension_premium"));
    let desc = tool.get("description").and_then(|d| d.as_str()).unwrap();
    assert!(desc.contains("厚生年金保険料") && desc.contains("報酬月額"), "{desc}");
    let schema = tool.get("inputSchema").expect("inputSchema");
    let want: rulec::json::Json = {
        let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root()).args(["schema", RULE]).output().unwrap();
        let j = rulec::json::parse(String::from_utf8_lossy(&o.stdout).trim()).unwrap();
        j.get("properties").unwrap().get("in").unwrap().clone()
    };
    assert_eq!(rulec::json::unparse(schema), rulec::json::unparse(&want), "inputSchema がワイヤの in と違う");
    let rate = schema.get("properties").unwrap().get("料率").unwrap();
    assert!(rate.get("description").and_then(|d| d.as_str()).unwrap().contains("0.1%"), "{}", rulec::json::unparse(rate));
    assert!(tool.get("outputSchema").is_some(), "outputSchema が無い");

    // A call answers with the record line, as text and as structured content.
    assert!(!is_error(&a[2]), "{}", rulec::json::unparse(&a[2]));
    assert_eq!(text(&a[2]), EXPECTED);
    let sc = a[2].get("result").and_then(|r| r.get("structuredContent")).expect("structuredContent");
    assert_eq!(sc, &rulec::json::parse(EXPECTED).unwrap());

    // 18.3 for a rate in steps of 0.1% is refused, not taken as 1.83%.
    assert!(is_error(&a[3]), "{}", rulec::json::unparse(&a[3]));
    assert!(text(&a[3]).contains("料率") && text(&a[3]).contains("整数"), "{}", text(&a[3]));
    // A missing argument is named.
    assert!(is_error(&a[4]) && text(&a[4]).contains("料率"), "{}", rulec::json::unparse(&a[4]));
    // The entry guard of the module reaches the caller as a refusal, not a crash.
    assert!(is_error(&a[5]) && text(&a[5]).contains("範囲"), "{}", rulec::json::unparse(&a[5]));
    // A tool the server does not have is a protocol error.
    assert_eq!(a[6].get("error").and_then(|e| e.get("code")).and_then(|c| c.as_int()), Some(-32602));

    // `--record` kept the one call that was answered, as one fixtures record.
    let kept = std::fs::read_to_string(&record).unwrap();
    assert_eq!(kept, format!("{EXPECTED}\n"));
    let _ = std::fs::remove_file(&record);
}

#[test]
fn pythonのサーバは規則を一つのツールとして出す() {
    if !have("python3") {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    let dir = generate("py");
    contract(&dir, "python", "python3", &["-B", "pension_premium_mcp.py"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nodeのサーバは規則を一つのツールとして出す() {
    if !have("node") {
        eprintln!("注意: node が無いので飛ばした");
        return;
    }
    let dir = generate("js");
    contract(&dir, "javascript", "node", &["pension_premium_mcp.mjs"]);
    contract(&dir, "typescript", "node", &["--no-warnings", "pension_premium_mcp.ts"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The server is generated code like the module: `gen --check` sees it go stale, and
/// `rulec api` names it.
#[test]
fn サーバは生成物の一つとして数えられる() {
    let dir = generate("api");
    for f in ["python/pension_premium_mcp.py", "typescript/pension_premium_mcp.ts", "javascript/pension_premium_mcp.mjs"] {
        assert!(dir.join(f).exists(), "{f} が無い");
    }
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root()).args(["api", RULE]).output().unwrap();
    let j = rulec::json::parse(String::from_utf8_lossy(&o.stdout).trim()).unwrap();
    for (lang, want) in [("python", "pension_premium_mcp.py"), ("typescript", "pension_premium_mcp.ts"), ("javascript", "pension_premium_mcp.mjs")] {
        assert_eq!(j.get(lang).and_then(|l| l.get("mcp")).and_then(|m| m.as_str()), Some(want), "{lang}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The HTTP side of the same server (§15.51). `rulec test` already holds its answers to the
/// reference evaluator over every vector, on both transports; what is checked here is what
/// the transport itself has to do — the session, the codes, and the origin check that keeps
/// a page open in a browser from reaching a server on the reader's own machine.
fn http_contract(dir: &Path, cwd: &str, cmd: &str, args: &[&str]) {
    let mut child = Command::new(cmd)
        .current_dir(dir.join(cwd))
        .args(args)
        .args(["--http", "127.0.0.1:0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("サーバを起動できない");
    let mut so = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    assert!(so.read_line(&mut line).unwrap() > 0, "待ち受けを言わずに終わった");
    let at = line.trim().trim_start_matches("http://").trim_end_matches("/mcp").to_string();

    // (status, headers as one lowercase blob, body)
    let call = |method: &str, headers: &str, body: &str| -> (u16, String, String) {
        let mut s = std::net::TcpStream::connect(&at).expect("繋げない");
        let req = format!(
            "{method} /mcp HTTP/1.1\r\nHost: {at}\r\nContent-Type: application/json\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).unwrap();
        s.flush().unwrap();
        let mut all = String::new();
        std::io::Read::read_to_string(&mut s, &mut all).unwrap();
        let (head, body) = all.split_once("\r\n\r\n").unwrap_or((all.as_str(), ""));
        let code: u16 = head.lines().next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
        (code, head.to_lowercase(), body.to_string())
    };

    let init = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{}}}"#;
    let (code, head, body) = call("POST", "", init);
    assert_eq!(code, 200, "{cwd}: initialize が 200 でない");
    assert!(head.contains("mcp-session-id:"), "{cwd}: セッションを渡していない: {head}");
    assert!(body.contains("\"serverInfo\""), "{cwd}: {body}");
    let session = head
        .lines()
        .find_map(|l| l.strip_prefix("mcp-session-id:"))
        .map(|v| v.trim().to_string())
        .expect("セッション");

    // A message with no id is answered with 202 and nothing else.
    let (code, _, body) = call("POST", "", r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    assert_eq!((code, body.as_str()), (202, ""), "{cwd}: 通知の扱い");

    // The session that was handed out is accepted; one that was not is 404, which is how a
    // client is told to start again.
    let ping = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
    let (code, _, _) = call("POST", &format!("Mcp-Session-Id: {session}\r\n"), ping);
    assert_eq!(code, 200, "{cwd}: 渡したセッションを断った");
    let (code, _, _) = call("POST", "Mcp-Session-Id: 0123456789abcdef\r\n", ping);
    assert_eq!(code, 404, "{cwd}: 知らないセッションを通した");

    // Nothing is ever sent unasked, so there is no stream to open.
    let (code, _, _) = call("GET", "", "");
    assert_eq!(code, 405, "{cwd}: GET の扱い");
    let (code, _, _) = call("DELETE", &format!("Mcp-Session-Id: {session}\r\n"), "");
    assert_eq!(code, 204, "{cwd}: DELETE の扱い");

    // The origin check: a page in a browser must not be able to reach this.
    let (code, _, _) = call("POST", "Origin: https://evil.example\r\n", ping);
    assert_eq!(code, 403, "{cwd}: 外の Origin を通した");
    let (code, _, _) = call("POST", "Origin: http://localhost:5173\r\n", ping);
    assert_eq!(code, 200, "{cwd}: 手元の Origin を断った");

    // A batch is not part of this protocol version, and is refused rather than half-read.
    let (code, _, _) = call("POST", "", "[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}]");
    assert_eq!(code, 400, "{cwd}: まとめ送りの扱い");
    let (code, _, _) = call("POST", "", "{not json");
    assert_eq!(code, 400, "{cwd}: JSON でない本文の扱い");

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn pythonのサーバはhttpでも同じ約束を守る() {
    if !have("python3") {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    let dir = generate("pyhttp");
    http_contract(&dir, "python", "python3", &["-B", "pension_premium_mcp.py"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nodeのサーバはhttpでも同じ約束を守る() {
    if !have("node") {
        eprintln!("注意: node が無いので飛ばした");
        return;
    }
    let dir = generate("jshttp");
    http_contract(&dir, "javascript", "node", &["pension_premium_mcp.mjs"]);
    http_contract(&dir, "typescript", "node", &["--no-warnings", "pension_premium_mcp.ts"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The tool's view (SEP-1865, §15.52): the page an approver reads, served as a `ui://`
/// resource, and offered only to a host that said it can render one.
fn ui_contract(dir: &Path, cwd: &str, cmd: &str, args: &[&str]) {
    let with_ui = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2026-01-26","capabilities":{"extensions":{"io.modelcontextprotocol/ui":{"mimeTypes":["text/html;profile=mcp-app"]}}}}}"#;
    let plain = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{}}}"#;
    let list = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
    let resources = r#"{"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}"#;
    let read = r#"{"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"ui://pension_premium/table"}}"#;
    let missing = r#"{"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"ui://nope"}}"#;

    let a = talk(&dir.join(cwd), cmd, args, &[with_ui, list, resources, read, missing]);
    // The server says it serves resources now.
    let caps = a[0].get("result").and_then(|r| r.get("capabilities")).expect("capabilities");
    assert!(caps.get("resources").is_some(), "{cwd}: resources を出していない");
    // The tool carries its view.
    let rulec::json::Json::Arr(tools) = a[1].get("result").unwrap().get("tools").unwrap() else {
        panic!("tools")
    };
    let uri = tools[0]
        .get("_meta")
        .and_then(|m| m.get("ui"))
        .and_then(|u| u.get("resourceUri"))
        .and_then(|u| u.as_str());
    assert_eq!(uri, Some("ui://pension_premium/table"), "{cwd}: ツールに view が付いていない");
    // The resource is listed, and reading it gives the page with the profile the spec fixes.
    let rulec::json::Json::Arr(rs) = a[2].get("result").unwrap().get("resources").unwrap() else {
        panic!("resources")
    };
    assert_eq!(rs.len(), 1, "{cwd}: 資源が一つでない");
    assert_eq!(rs[0].get("mimeType").and_then(|m| m.as_str()), Some("text/html;profile=mcp-app"));
    let rulec::json::Json::Arr(cs) = a[3].get("result").unwrap().get("contents").unwrap() else {
        panic!("contents")
    };
    let html = cs[0].get("text").and_then(|t| t.as_str()).expect("text");
    // It is the page `rulec doc --format html` renders — the same renderer, not a second one.
    let want = std::fs::read_to_string(dir.join(cwd).join("pension_premium_page.html")).unwrap();
    assert_eq!(html, want, "{cwd}: 出している資源がページと違う");
    assert!(html.contains("ui/notifications/tool-result"), "{cwd}: ページに橋が無い");
    // A resource that is not there is refused by name.
    assert!(a[4].get("error").is_some(), "{cwd}: 知らない資源を答えた");

    // A host that did not say it can render one is not told about a view (the UI is an
    // enhancement, and a tool that promises one where none can be shown is a lie).
    let b = talk(&dir.join(cwd), cmd, args, &[plain, list]);
    let rulec::json::Json::Arr(tools) = b[1].get("result").unwrap().get("tools").unwrap() else {
        panic!("tools")
    };
    assert!(tools[0].get("_meta").is_none(), "{cwd}: 描けない相手に view を出した");
}

#[test]
fn pythonのサーバはページをviewとして出す() {
    if !have("python3") {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    let dir = generate("pyui");
    ui_contract(&dir, "python", "python3", &["-B", "pension_premium_mcp.py"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nodeのサーバはページをviewとして出す() {
    if !have("node") {
        eprintln!("注意: node が無いので飛ばした");
        return;
    }
    let dir = generate("jsui");
    ui_contract(&dir, "javascript", "node", &["pension_premium_mcp.mjs"]);
    ui_contract(&dir, "typescript", "node", &["--no-warnings", "pension_premium_mcp.ts"]);
    let _ = std::fs::remove_dir_all(&dir);
}
