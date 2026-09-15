//! `rulec mcp` (§15.38): the command table as MCP tools over stdio. The server runs this same
//! binary for a call, so what is tested here is the seam — the tool list is the table, a call
//! reaches the command with the arguments the table validates, the exit code comes back, and
//! the documents are served as resources with their links pointing at each other.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Send the requests, one per line, and read one answer per request that carries an id.
fn talk(requests: &[&str]) -> Vec<rulec::json::Json> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rulec mcp を起動できない");
    {
        let mut stdin = child.stdin.take().unwrap();
        for r in requests {
            writeln!(stdin, "{r}").unwrap();
        }
    }
    let out = BufReader::new(child.stdout.take().unwrap());
    let answers: Vec<rulec::json::Json> = out
        .lines()
        .map(|l| l.unwrap())
        .filter(|l| !l.trim().is_empty())
        .map(|l| rulec::json::parse(&l).unwrap_or_else(|e| panic!("JSON として読めない: {e}\n{l}")))
        .collect();
    assert!(child.wait().unwrap().success());
    answers
}

fn s<'a>(j: &'a rulec::json::Json, k: &str) -> &'a str {
    j.get(k).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{k} が無い: {j:?}"))
}

fn arr<'a>(j: &'a rulec::json::Json, k: &str) -> &'a Vec<rulec::json::Json> {
    match j.get(k) {
        Some(rulec::json::Json::Arr(a)) => a,
        other => panic!("{k} が配列でない: {other:?}"),
    }
}

#[test]
fn 初期化に答え_通知には答えない() {
    let a = talk(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":"p","method":"ping"}"#,
    ]);
    assert_eq!(a.len(), 2, "通知に答えている: {a:?}");
    let r = a[0].get("result").expect("result が無い");
    assert_eq!(s(r, "protocolVersion"), "2025-06-18");
    assert_eq!(s(r.get("serverInfo").unwrap(), "name"), "rulec");
    assert!(r.get("capabilities").and_then(|c| c.get("tools")).is_some());
    assert!(s(r, "instructions").contains("rulec://docs/agents.md"), "手順の資料を最初に読めと言う");
    // A string id comes back as it was.
    assert_eq!(a[1].get("id").and_then(|i| i.as_str()), Some("p"));
}

#[test]
fn ツールの一覧はコマンドの表そのもの() {
    let a = talk(&[r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#]);
    let tools = arr(a[0].get("result").unwrap(), "tools");
    let names: Vec<&str> = tools.iter().map(|t| s(t, "name")).collect();
    for want in ["rulec_check", "rulec_gen", "rulec_test", "rulec_doc", "rulec_verify", "rulec_replay", "rulec_diff", "rulec_explain", "rulec_fixtures", "rulec_api"] {
        assert!(names.contains(&want), "{want} が無い: {names:?}");
    }
    assert!(!names.contains(&"rulec_mcp"), "サーバ自身はツールにしない");
    let check = tools.iter().find(|t| s(t, "name") == "rulec_check").unwrap();
    let schema = check.get("inputSchema").unwrap();
    let props = schema.get("properties").and_then(|p| p.as_obj()).unwrap();
    for want in ["files", "format", "diff_base", "terse", "lang"] {
        assert!(props.contains_key(want), "check の引数 {want} が無い: {:?}", props.keys().collect::<Vec<_>>());
    }
    assert_eq!(s(props.get("terse").unwrap(), "type"), "boolean");
    assert_eq!(s(props.get("files").unwrap(), "type"), "array");
    assert!(s(check, "description").contains("Exit codes"), "exit code の意味を説明に入れる");
    let verify = tools.iter().find(|t| s(t, "name") == "rulec_verify").unwrap();
    let props = verify.get("inputSchema").unwrap().get("properties").and_then(|p| p.as_obj()).unwrap();
    assert!(props.contains_key("adapter") && props.contains_key("file"));
    let fx = tools.iter().find(|t| s(t, "name") == "rulec_fixtures").unwrap();
    let props = fx.get("inputSchema").unwrap().get("properties").and_then(|p| p.as_obj()).unwrap();
    assert!(props.contains_key("fixtures") && props.contains_key("rule") && props.contains_key("fill"));
    assert_eq!(s(props.get("fill").unwrap(), "type"), "array", "繰り返せるフラグは配列");
}

#[test]
fn 呼び出しはコマンドに届き_exit_codeが戻る() {
    let a = talk(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"rulec_check","arguments":{"files":["tests/corpus/送料.rule"],"format":"json"}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"rulec_check","arguments":{"files":["tests/mutants/m_e101.rule"],"format":"json","lang":"en"}}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"rulec_explain","arguments":{"code":"E101","lang":"ja"}}}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"rulec_check","arguments":{"files":["tests/corpus/送料.rule"],"bogus":true}}}"#,
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"rulec_nothing","arguments":{}}}"#,
    ]);
    let text = |r: &rulec::json::Json, i: usize| s(&arr(r, "content")[i], "text").to_string();
    let r = a[0].get("result").unwrap();
    assert!(text(r, 0).contains("\"code\":\"W105\"") || text(r, 0).contains("\"v\":2"), "{}", text(r, 0));
    assert_eq!(text(r, 1), "exit code 0");
    assert_eq!(r.get("isError"), Some(&rulec::json::Json::Bool(false)));
    let r = a[1].get("result").unwrap();
    assert!(text(r, 0).contains("\"code\":\"E101\""), "{}", text(r, 0));
    assert!(text(r, 0).contains("Completeness gap"), "--lang en が届く: {}", text(r, 0));
    assert_eq!(text(r, 1), "exit code 1");
    assert_eq!(r.get("isError"), Some(&rulec::json::Json::Bool(false)), "発見は誤りではない");
    let r = a[2].get("result").unwrap();
    assert!(text(r, 0).contains("完全性"), "{}", text(r, 0));
    // An argument the table does not know is refused before anything runs.
    let e = a[3].get("error").expect("知らない引数を通している");
    assert!(s(e, "message").contains("bogus"), "{e:?}");
    let e = a[4].get("error").expect("知らないツールを通している");
    assert!(s(e, "message").contains("rulec_nothing"), "{e:?}");
}

#[test]
fn 文書はリソースで_互いへのリンクはリソースを指す() {
    let a = talk(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"resources/list"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"rulec://docs/agents.md"}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"rulec://docs/nowhere.md"}}"#,
    ]);
    let rs = arr(a[0].get("result").unwrap(), "resources");
    let uris: Vec<&str> = rs.iter().map(|r| s(r, "uri")).collect();
    for want in ["rulec://docs/agents.md", "rulec://docs/reference.md", "rulec://docs/formats.md", "rulec://docs/generated-code.md", "rulec://docs/backends.md"] {
        assert!(uris.contains(&want), "{want} が無い: {uris:?}");
    }
    let c = &arr(a[1].get("result").unwrap(), "contents")[0];
    let text = s(c, "text");
    let agents = std::fs::read_to_string(root().join("AGENTS.md")).unwrap();
    assert!(text.starts_with(agents.lines().next().unwrap()), "AGENTS.md がそのまま出る");
    assert!(text.contains("](rulec://docs/reference.md)"), "リンクがリソースを指す");
    assert!(!text.contains("](docs/"), "リポジトリの相対パスが残っている");
    assert!(a[2].get("error").is_some(), "無いリソースは error");
}
