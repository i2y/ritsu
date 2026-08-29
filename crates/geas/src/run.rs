use crate::json::{self, J};
use crate::model::*;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write as _;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const STEP_TIMEOUT: Duration = Duration::from_secs(5);

pub enum Obs {
    Proc { stdout: String, stderr: String, exit: i32 },
    Http { status: u16, headers: Vec<(String, String)>, body: String },
}

pub struct ObsRec {
    pub idx: usize,
    pub target: String,
    pub call: String,
    pub obs: Obs,
}

pub struct CheckResult {
    pub line: usize,
    pub label: String,
    pub expected: String,
    pub actual: String,
    pub ok: bool,
}

pub enum ClaimStatus {
    Ok,
    Fail,
    Error { message: String, line: usize },
}

pub struct ClaimResult {
    pub name: String,
    pub line: usize,
    pub status: ClaimStatus,
    pub checks: Vec<CheckResult>,
    pub observations: Vec<ObsRec>,
}

struct Server {
    child: Child,
    stderr: thread::JoinHandle<Vec<u8>>,
}

pub fn run_spec(spec: &Spec, dir: &Path, journal: &mut Vec<String>) -> Vec<ClaimResult> {
    spec.claims
        .iter()
        .map(|c| run_claim(spec, c, dir, journal))
        .collect()
}

fn run_claim(spec: &Spec, claim: &Claim, dir: &Path, journal: &mut Vec<String>) -> ClaimResult {
    let mut servers: HashMap<String, Server> = HashMap::new();
    let mut checks: Vec<CheckResult> = Vec::new();
    let mut error: Option<(String, usize)> = None;
    let mut obs: Option<Obs> = None;
    let mut observations: Vec<ObsRec> = Vec::new();
    let mut when_idx = 0usize;

    for step in &claim.steps {
        match step {
            Step::When { target, call, line } => {
                let t = spec.target(target).expect("validated");
                let result = match (&t.kind, call) {
                    (TargetKind::Run(cmd), Call::Run(args)) => run_process(cmd, args, dir),
                    (TargetKind::Serve { cmd, port }, _) => {
                        match ensure_server(target, cmd, *port, dir, &mut servers) {
                            Err(e) => Err(e),
                            Ok(()) => match call {
                                Call::Get(path) => http(*port, "GET", path, None),
                                Call::Post { path, body } => {
                                    http(*port, "POST", path, body.as_deref())
                                }
                                Call::Run(_) => unreachable!("validated"),
                            },
                        }
                    }
                    _ => unreachable!("validated"),
                };
                match result {
                    Ok(o) => {
                        journal.push(journal_when(&claim.name, target, call, &o));
                        observations.push(ObsRec {
                            idx: when_idx,
                            target: target.clone(),
                            call: call_display(call),
                            obs: clone_obs(&o),
                        });
                        when_idx += 1;
                        obs = Some(o);
                    }
                    Err(e) => {
                        journal.push(format!(
                            "{{\"claim\":\"{}\",\"event\":\"error\",\"line\":{},\"message\":\"{}\"}}",
                            json::esc(&claim.name),
                            line,
                            json::esc(&e)
                        ));
                        error = Some((e, *line));
                        break;
                    }
                }
            }
            Step::Then(check) => {
                let o = obs.as_ref().expect("parser guarantees a `when` first");
                let cr = eval_check(check, o);
                journal.push(format!(
                    "{{\"claim\":\"{}\",\"event\":\"check\",\"line\":{},\"check\":\"{}\",\"expected\":{},\"actual\":{},\"ok\":{}}}",
                    json::esc(&claim.name),
                    cr.line,
                    json::esc(&cr.label),
                    quote(&cr.expected),
                    quote(&cr.actual),
                    cr.ok
                ));
                checks.push(cr);
            }
        }
    }

    for (_, mut s) in servers.drain() {
        let _ = s.child.kill();
        let _ = s.child.wait();
        let _ = s.stderr.join();
    }

    let status = if let Some((message, line)) = error {
        ClaimStatus::Error { message, line }
    } else if checks.iter().any(|c| !c.ok) {
        ClaimStatus::Fail
    } else {
        ClaimStatus::Ok
    };
    ClaimResult { name: claim.name.clone(), line: claim.line, status, checks, observations }
}

fn clone_obs(o: &Obs) -> Obs {
    match o {
        Obs::Proc { stdout, stderr, exit } => Obs::Proc {
            stdout: stdout.clone(),
            stderr: stderr.clone(),
            exit: *exit,
        },
        Obs::Http { status, headers, body } => Obs::Http {
            status: *status,
            headers: headers.clone(),
            body: body.clone(),
        },
    }
}

pub fn call_display(call: &Call) -> String {
    match call {
        Call::Run(args) => {
            let a: Vec<String> = args.iter().map(|s| format!("\"{}\"", s)).collect();
            format!("run({})", a.join(", "))
        }
        Call::Get(p) => format!("get(\"{}\")", p),
        Call::Post { path, body } => match body {
            Some(b) => format!("post(\"{}\", body: \"{}\")", path, b),
            None => format!("post(\"{}\")", path),
        },
    }
}

fn journal_when(claim: &str, target: &str, call: &Call, obs: &Obs) -> String {
    let call_s = call_display(call);
    let obs_s = match obs {
        Obs::Proc { stdout, stderr, exit } => format!(
            "{{\"stdout\":\"{}\",\"stderr\":\"{}\",\"exit\":{}}}",
            json::esc(stdout),
            json::esc(stderr),
            exit
        ),
        Obs::Http { status, body, .. } => format!(
            "{{\"status\":{},\"body\":\"{}\"}}",
            status,
            json::esc(body)
        ),
    };
    format!(
        "{{\"claim\":\"{}\",\"event\":\"when\",\"target\":\"{}\",\"call\":\"{}\",\"obs\":{}}}",
        json::esc(claim),
        json::esc(target),
        json::esc(&call_s),
        obs_s
    )
}

fn quote(s: &str) -> String {
    format!("\"{}\"", json::esc(s))
}

// ---------- process adapter ----------

fn slurp(mut r: impl Read + Send + 'static) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut v = Vec::new();
        let _ = r.read_to_end(&mut v);
        v
    })
}

fn run_process(cmd_base: &str, args: &[String], dir: &Path) -> Result<Obs, String> {
    let mut parts = cmd_base.split_whitespace();
    let Some(prog) = parts.next() else {
        return Err("empty command".into());
    };
    let mut cmd = Command::new(prog);
    cmd.args(parts)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to start `{}`: {}", prog, e))?;
    let so = slurp(child.stdout.take().expect("piped"));
    let se = slurp(child.stderr.take().expect("piped"));
    let deadline = Instant::now() + STEP_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "`{}` did not finish within {}s",
                        prog,
                        STEP_TIMEOUT.as_secs()
                    ));
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(format!("wait failed: {}", e)),
        }
    };
    let stdout = String::from_utf8_lossy(&so.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&se.join().unwrap_or_default()).into_owned();
    Ok(Obs::Proc { stdout, stderr, exit: status.code().unwrap_or(-1) })
}

// ---------- service adapter ----------

fn ensure_server(
    name: &str,
    cmd: &str,
    port: u16,
    dir: &Path,
    servers: &mut HashMap<String, Server>,
) -> Result<(), String> {
    if servers.contains_key(name) {
        return Ok(());
    }
    let mut parts = cmd.split_whitespace();
    let Some(prog) = parts.next() else {
        return Err("empty serve command".into());
    };
    let mut c = Command::new(prog);
    c.args(parts)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = c
        .spawn()
        .map_err(|e| format!("failed to start `{}`: {}", prog, e))?;
    let se = slurp(child.stderr.take().expect("piped"));
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().expect("addr");
    let deadline = Instant::now() + STEP_TIMEOUT;
    loop {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok() {
            break;
        }
        if let Ok(Some(st)) = child.try_wait() {
            let err = String::from_utf8_lossy(&se.join().unwrap_or_default()).into_owned();
            return Err(format!(
                "server exited before opening port {} (exit {}): {}",
                port,
                st.code().unwrap_or(-1),
                err.trim()
            ));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = se.join();
            return Err(format!(
                "server did not open port {} within {}s",
                port,
                STEP_TIMEOUT.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(25));
    }
    servers.insert(name.to_string(), Server { child, stderr: se });
    Ok(())
}

fn http(port: u16, method: &str, path: &str, body: Option<&str>) -> Result<Obs, String> {
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().expect("addr");
    let mut s = TcpStream::connect_timeout(&addr, STEP_TIMEOUT)
        .map_err(|e| format!("connect to port {} failed: {}", port, e))?;
    let _ = s.set_read_timeout(Some(STEP_TIMEOUT));
    let _ = s.set_write_timeout(Some(STEP_TIMEOUT));
    let b = body.unwrap_or("");
    let req = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        method,
        path,
        b.len(),
        b
    );
    s.write_all(req.as_bytes())
        .map_err(|e| format!("write failed: {}", e))?;
    let mut buf = Vec::new();
    s.read_to_end(&mut buf)
        .map_err(|e| format!("read failed: {}", e))?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    let Some(hdr_end) = text.find("\r\n\r\n") else {
        return Err("malformed HTTP response".into());
    };
    let head = &text[..hdr_end];
    let body = text[hdr_end + 4..].to_string();
    let status_line = head.lines().next().unwrap_or("");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("bad status line `{}`", status_line))?;
    let mut headers: Vec<(String, String)> = Vec::new();
    for line in head.lines().skip(1) {
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_lowercase(), value.trim().to_string()));
        }
    }
    headers.sort();
    Ok(Obs::Http { status, headers, body })
}

// ---------- checks ----------

fn eval_check(check: &Check, obs: &Obs) -> CheckResult {
    let matcher_word = match check.matcher {
        Matcher::Is => "is",
        Matcher::Contains => "contains",
    };
    let label = format!("{} {}", check.subject.label(), matcher_word);
    let expected = match &check.expected {
        Expected::S(s) => format!("\"{}\"", json::esc(s)),
        Expected::N(n) => json::render_num(*n),
    };
    let (actual, ok) = match (&check.subject, obs) {
        (Subject::Stdout, Obs::Proc { stdout, .. }) => cmp_text(stdout, check),
        (Subject::Stderr, Obs::Proc { stderr, .. }) => cmp_text(stderr, check),
        (Subject::Exit, Obs::Proc { exit, .. }) => cmp_num(f64::from(*exit), check),
        (Subject::Status, Obs::Http { status, .. }) => cmp_num(f64::from(*status), check),
        (Subject::Body, Obs::Http { body, .. }) => cmp_text(body, check),
        (Subject::BodyJson(path), Obs::Http { body, .. }) => cmp_json(body, path, check),
        (s, _) => (
            format!("<{} is not observable after this `when`>", s.label()),
            false,
        ),
    };
    CheckResult { line: check.line, label, expected, actual, ok }
}

fn trim_one_newline(s: &str) -> &str {
    s.strip_suffix('\n').unwrap_or(s)
}

fn short(s: &str) -> String {
    let t: String = s.chars().take(160).collect();
    if t.len() < s.len() {
        format!("{}…", t)
    } else {
        t
    }
}

fn cmp_text(actual_raw: &str, check: &Check) -> (String, bool) {
    let trimmed = trim_one_newline(actual_raw);
    let disp = format!("\"{}\"", json::esc(&short(trimmed)));
    match (&check.matcher, &check.expected) {
        (Matcher::Is, Expected::S(want)) => (disp, trimmed == want),
        (Matcher::Is, Expected::N(n)) => (disp, trimmed == json::render_num(*n)),
        (Matcher::Contains, Expected::S(want)) => (disp, actual_raw.contains(want)),
        (Matcher::Contains, Expected::N(_)) => (disp, false), // rejected at parse
    }
}

fn cmp_num(actual: f64, check: &Check) -> (String, bool) {
    let disp = json::render_num(actual);
    match &check.expected {
        Expected::N(n) => ((disp), (actual - n).abs() < 1e-9),
        Expected::S(_) => (disp, false), // rejected at parse
    }
}

fn cmp_json(body: &str, path: &str, check: &Check) -> (String, bool) {
    let parsed = match json::parse(body) {
        Ok(v) => v,
        Err(e) => return (format!("<body is not JSON: {}>", e), false),
    };
    let leaf = match json::path_get(&parsed, path) {
        Ok(v) => v,
        Err(e) => return (format!("<{}>", e), false),
    };
    let disp = short(&json::render(leaf));
    let ok = match (&check.matcher, &check.expected, leaf) {
        (Matcher::Is, Expected::S(want), J::Str(s)) => s == want,
        (Matcher::Is, Expected::N(n), J::Num(m)) => (m - n).abs() < 1e-9,
        (Matcher::Is, Expected::N(n), J::Str(s)) => s == &json::render_num(*n),
        (Matcher::Contains, Expected::S(want), J::Str(s)) => s.contains(want),
        _ => false,
    };
    (disp, ok)
}
