//! The requests `ritsu mod` sends a registry, through curl (DESIGN 17.12, "HTTP は curl で"): one
//! function, [`send`], and the way it runs curl.
//!
//! - `-q` first, so that no `.curlrc` is read (a person's settings could follow redirects, say), and
//!   `--silent --show-error`.
//! - A secret (an `Authorization` header, the form of an identity token's `POST`) goes on curl's
//!   standard input, as its configuration (`--config -`): never on the command line, in the
//!   environment or in a file, where the list of processes or another user could read it. What
//!   holds one is a [`Secret`], which has neither `Debug` nor `Display`.
//! - The other headers are `--header`, the method `--request` (`--head` for a `HEAD`), the body a
//!   file in the store's `tmp/` (`--data-binary @…`), the answer's headers `--dump-header` and its
//!   body `--output`, both files there too, removed when the request is done; an answer that may
//!   hold a secret (a token's) is read from the standard output instead, and is in no file. The
//!   status comes from `--write-out`.
//! - No `--location`: ritsu follows a redirect itself, five at most, and sends `Authorization` only
//!   where the scheme, the host and the port are the request's own (a registry often sends a blob's
//!   `GET` to another host).
//! - `--proto =https`; `=http,https` for a host of the loopback alone (a registry a test starts). A
//!   registry over plain HTTP elsewhere is refused, and no flag lets it.
//! - `--connect-timeout 10 --max-time 300`, `--max-filesize` for what is fetched, `--user-agent
//!   ritsu/<version>`. An answer over its request's limit is told apart from the others
//!   ([`NoAnswer::Over`]): it is what the server has there that is wrong, not the way to it.
//! - A `GET` or a `HEAD` is tried three times at most, 2 s and then 4 s apart, when curl cannot
//!   reach the registry or the registry answers 429, 500, 502, 503 or 504; a 429 or a 503 waits as
//!   its `Retry-After` says (60 s at most). A request that sends something is not tried again (a
//!   push starts an upload over once itself).
//! - What goes wrong is said with the status, the URL without its query, and the `code` and the
//!   `message` of the registry's errors; never their `detail`, which may hold what was sent.
//!
//! Under WASI, curl cannot be started: [`send`] says so (`wasi::cannot_start("curl")`, DESIGN 17.22).

use ritsu_base::json::{self, Json};
use ritsu_base::text::Text;
use ritsu_base::tr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// A value no output may hold: a password, a token, an `Authorization` header. It has neither
/// `Debug` nor `Display`, so that nothing prints it by a slip; curl reads it on its standard input.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Secret {
        Secret(s.into())
    }

    /// The text, for what curl reads on its standard input and for a header made of it.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

/// What a request sends.
pub enum Body<'a> {
    None,
    /// Bytes, from a file in the store's `tmp/`.
    Bytes(&'a [u8]),
    /// A form that holds a secret (an identity token's `POST`), as the `data` of curl's
    /// configuration on its standard input.
    SecretForm(&'a Secret),
}

/// A request.
pub struct Request<'a> {
    pub method: &'a str,
    pub url: String,
    /// The headers that hold no secret.
    pub headers: Vec<(String, String)>,
    /// The value of `Authorization`, sent where the scheme, the host and the port are the URL's.
    pub auth: Option<Secret>,
    pub body: Body<'a>,
    /// The most bytes the answer's body may have.
    pub limit: u64,
    /// The answer may hold a secret (a token): its body is read from curl's standard output, and
    /// no file holds it.
    pub secret_answer: bool,
}

impl<'a> Request<'a> {
    pub fn new(method: &'a str, url: impl Into<String>, limit: u64) -> Request<'a> {
        Request { method, url: url.into(), headers: Vec::new(), auth: None, body: Body::None, limit, secret_answer: false }
    }

    pub fn header(mut self, k: &str, v: impl Into<String>) -> Request<'a> {
        self.headers.push((k.to_string(), v.into()));
        self
    }
}

/// An answer.
#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// The URL that answered, after the redirects followed.
    pub url: String,
}

impl Response {
    /// A header's value, its name in any case.
    pub fn header(&self, k: &str) -> Option<&str> {
        self.headers.iter().find(|(x, _)| x.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str())
    }

    /// Whether the status is 2xx.
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// What the answer says went wrong: the method, the URL without its query, the status, and the
    /// `code` and `message` of each of the registry's errors (never their `detail`).
    pub fn error(&self, method: &str) -> Text {
        let url = without_query(&self.url);
        let status = self.status;
        let said = registry_errors(&self.body);
        if said.is_empty() {
            tr!("`{method} {url}` に、レジストリが {status} を返しました", "the registry answered `{method} {url}` with {status}")
        } else {
            tr!("`{method} {url}` に、レジストリが {status} を返しました（{said}）", "the registry answered `{method} {url}` with {status} ({said})")
        }
    }
}

/// The `code` and `message` of each error of a registry's error JSON (`{"errors": [{"code",
/// "message", "detail"}]}`), `; ` between them; empty when the body is not one.
pub fn registry_errors(body: &[u8]) -> String {
    let Some(j) = std::str::from_utf8(body).ok().and_then(|s| json::parse(s).ok()) else { return String::new() };
    let Some(errors) = j.get("errors").and_then(Json::as_arr) else { return String::new() };
    errors
        .iter()
        .map(|e| {
            let s = |k: &str| e.get(k).and_then(Json::as_str).unwrap_or("").to_string();
            let (code, message) = (s("code"), s("message"));
            if message.is_empty() { code } else { format!("{code}: {message}") }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// A URL without its query and its fragment.
pub fn without_query(url: &str) -> &str {
    &url[..url.find(['?', '#']).unwrap_or(url.len())]
}

/// The scheme, the host (lowercase) and the port of an absolute URL, the port the scheme's own when
/// none is written.
pub fn origin(url: &str) -> Option<(String, String, u16)> {
    let (scheme, host) = ritsu_base::urls::scheme_and_host(url)?;
    let rest = url.split_once("://")?.1;
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let hostport = authority.rsplit_once('@').map(|(_, h)| h).unwrap_or(authority);
    let after = &hostport[hostport.find(']').map(|e| e + 1).unwrap_or(0)..];
    let port = match after.rsplit_once(':') {
        Some((_, p)) if !p.is_empty() => p.parse().ok()?,
        _ => match scheme.as_str() {
            "https" => 443,
            "http" => 80,
            _ => return None,
        },
    };
    Some((scheme, host.to_ascii_lowercase(), port))
}

/// Whether a URL may be asked, and over plain HTTP: `https`, or `http` to a host of the loopback.
/// Else why not.
fn allowed(url: &str) -> Result<bool, Text> {
    let shown = without_query(url);
    match ritsu_base::urls::scheme_and_host(url) {
        Some((s, _)) if s == "https" => Ok(false),
        Some((s, h)) if s == "http" && ritsu_base::urls::is_loopback(&h) => Ok(true),
        Some((s, _)) if s == "http" => Err(tr!(
            "`{shown}` は暗号化しない HTTP です。ritsu はレジストリに https で問い合わせます（平文の HTTP は、このマシンの中のレジストリにだけ使えます）",
            "`{shown}` is plain HTTP; a registry is asked over https (plain HTTP is for a registry on this machine alone)"
        )),
        _ => Err(tr!("`{shown}` は、問い合わせられる URL ではありません", "`{shown}` is not a URL a registry is asked at")),
    }
}

/// `to` (a `Location`), made absolute against `from`.
pub fn resolve(from: &str, to: &str) -> String {
    if to.contains("://") {
        return to.to_string();
    }
    let (scheme, rest) = from.split_once("://").unwrap_or(("https", from));
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = &rest[..host_end];
    if let Some(r) = to.strip_prefix("//") {
        return format!("{scheme}://{r}");
    }
    if to.starts_with('/') {
        return format!("{scheme}://{host}{to}");
    }
    // relative to the path's directory
    let path = without_query(&rest[host_end..]);
    let dir = &path[..path.rfind('/').map(|i| i + 1).unwrap_or(0)];
    format!("{scheme}://{host}{}{to}", if dir.is_empty() { "/" } else { dir })
}

/// A file name in `dir` that no other request of this run or another takes.
fn unique(dir: &Path, what: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    dir.join(format!("curl-{}-{nanos}-{}.{what}", ritsu_base::wasi::run_id(), N.fetch_add(1, Ordering::Relaxed)))
}

/// Files removed when the request is done, whatever happens.
struct Cleanup(Vec<PathBuf>);

impl Drop for Cleanup {
    fn drop(&mut self) {
        for f in &self.0 {
            let _ = std::fs::remove_file(f);
        }
    }
}

/// A string as curl's configuration quotes it.
fn config_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// What one run of curl came to.
enum Ran {
    Answered(Response),
    /// curl's exit code and the first line of what it said.
    Failed(i32, String),
}

/// One run of curl for `req` at `url`, with `auth` or none.
fn run_once(req: &Request<'_>, method: &str, url: &str, auth: Option<&Secret>, http: bool, tmp: &Path) -> Result<Ran, Text> {
    std::fs::create_dir_all(tmp).map_err(|e| {
        let d = tmp.display();
        tr!("{d} を作れません（{e}）", "cannot make {d} ({e})")
    })?;
    let dump = unique(tmp, "headers");
    let out = unique(tmp, "body");
    let mut cleanup = Cleanup(vec![dump.clone(), out.clone()]);
    let mut c = Command::new("curl");
    c.arg("-q").args(["--silent", "--show-error", "--config", "-"]);
    c.args(["--proto", if http { "=http,https" } else { "=https" }]);
    // a proxy does not reach this machine's loopback
    if origin(url).is_some_and(|(_, host, _)| ritsu_base::urls::is_loopback(&host)) {
        c.args(["--noproxy", "*"]);
    }
    c.args(["--connect-timeout", "10", "--max-time", "300"]);
    if method != "HEAD" {
        c.args(["--max-filesize", &req.limit.to_string()]);
    }
    c.args(["--user-agent", &format!("ritsu/{}", env!("CARGO_PKG_VERSION"))]);
    if method == "HEAD" {
        c.arg("--head");
    } else {
        c.args(["--request", method]);
    }
    for (k, v) in &req.headers {
        c.arg("--header").arg(format!("{k}: {v}"));
    }
    let mut config = String::new();
    if let Some(a) = auth {
        config.push_str(&format!("header = {}\n", config_quote(&format!("Authorization: {}", a.expose()))));
    }
    match (&req.body, method) {
        (_, "GET" | "HEAD") | (Body::None, _) => {}
        (Body::Bytes(b), _) => {
            let sent = unique(tmp, "sent");
            std::fs::write(&sent, b).map_err(|e| {
                let d = sent.display();
                tr!("{d} に書けません（{e}）", "cannot write {d} ({e})")
            })?;
            c.arg("--data-binary").arg(format!("@{}", sent.display()));
            cleanup.0.push(sent);
        }
        (Body::SecretForm(f), _) => {
            config.push_str(&format!("data = {}\n", config_quote(f.expose())));
        }
    }
    c.arg("--dump-header").arg(&dump);
    if !req.secret_answer {
        c.arg("--output").arg(&out);
    }
    c.args(["--write-out", "\\n%{http_code}"]);
    c.arg(url);
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(tr!(
                "curl がありません。`ritsu mod` はレジストリに curl で問い合わせます。curl を入れて、PATH に置いてください",
                "curl is not there: `ritsu mod` asks a registry through curl; install curl and put it on the PATH"
            ));
        }
        Err(e) => return Err(ritsu_base::wasi::starting("curl", &e)),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(config.as_bytes());
    }
    let o = child.wait_with_output().map_err(|e| ritsu_base::wasi::starting("curl", &e))?;
    if !o.status.success() {
        let said = String::from_utf8_lossy(&o.stderr).lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
        return Ok(Ran::Failed(o.status.code().unwrap_or(-1), said));
    }
    // the status after the last newline: after the body when the body is on the standard output
    let stdout = o.stdout;
    let (body_end, code_start) = match stdout.iter().rposition(|b| *b == b'\n') {
        Some(at) => (at, at + 1),
        None => (0, 0),
    };
    let status: u16 = String::from_utf8_lossy(&stdout[code_start..]).trim().parse().unwrap_or(0);
    let body = if req.secret_answer { stdout[..body_end].to_vec() } else { std::fs::read(&out).unwrap_or_default() };
    let text = std::fs::read_to_string(&dump).unwrap_or_default();
    // the last block of headers: a `100 Continue` comes before it
    let block = text.split("\r\n\r\n").filter(|b| b.trim_start().starts_with("HTTP/")).last().unwrap_or("");
    let headers = block.lines().skip(1).filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))).collect();
    Ok(Ran::Answered(Response { status, headers, body, url: url.to_string() }))
}

/// The seconds a 429 or a 503 asks to wait, at most 60.
fn retry_after(r: &Response) -> Option<u64> {
    r.header("Retry-After").and_then(|v| v.trim().parse::<u64>().ok()).map(|s| s.min(60))
}

/// curl's exit codes that say the registry was not reached or the answer broke off, which a `GET`
/// or a `HEAD` is tried again for.
fn transient(code: i32) -> bool {
    matches!(code, 6 | 7 | 18 | 28 | 52 | 55 | 56 | 92)
}

/// Why a request has no answer.
pub enum NoAnswer {
    /// The answer's body went over the request's limit, and curl stopped it (its exit code 63): the
    /// server has more there than what was asked for may have, which `fetch` and `push` refuse
    /// (exit 1).
    Over(Text),
    /// Anything else: curl not there, the server not reached, a redirect too many.
    Other(Text),
}

impl From<Text> for NoAnswer {
    fn from(t: Text) -> NoAnswer {
        NoAnswer::Other(t)
    }
}

impl From<NoAnswer> for Text {
    fn from(n: NoAnswer) -> Text {
        match n {
            NoAnswer::Over(t) | NoAnswer::Other(t) => t,
        }
    }
}

/// The request at one URL, tried again as a `GET` or a `HEAD` is.
fn with_retries(req: &Request<'_>, method: &str, url: &str, auth: Option<&Secret>, http: bool, tmp: &Path) -> Result<Response, NoAnswer> {
    let again = matches!(method, "GET" | "HEAD");
    let waits = [2u64, 4];
    let mut tries = 0;
    loop {
        let ran = run_once(req, method, url, auth, http, tmp)?;
        let wait = match &ran {
            Ran::Answered(r) if matches!(r.status, 429 | 500 | 502 | 503 | 504) => Some(if matches!(r.status, 429 | 503) { retry_after(r) } else { None }),
            Ran::Failed(code, _) if transient(*code) => Some(None),
            _ => None,
        };
        match (wait, again && tries < waits.len()) {
            (Some(after), true) => {
                std::thread::sleep(Duration::from_secs(after.unwrap_or(waits[tries])));
                tries += 1;
            }
            _ => {
                return match ran {
                    Ran::Answered(r) => Ok(r),
                    Ran::Failed(63, _) => {
                        let (u, n) = (without_query(url), req.limit);
                        Err(NoAnswer::Over(tr!("`{method} {u}` の答えが {n} バイトを超えました", "the answer to `{method} {u}` is over {n} bytes")))
                    }
                    Ran::Failed(code, said) => {
                        let u = without_query(url);
                        Err(NoAnswer::Other(tr!("`{method} {u}` に届きません（curl の終了コード {code}: {said}）", "cannot reach `{method} {u}` (curl exited with {code}: {said})")))
                    }
                };
            }
        }
    }
}

/// `req` sent, its redirects followed (five at most), with the files of curl in `tmp`: the answer,
/// whatever its status, or why there is none.
pub fn send(req: &Request<'_>, tmp: &Path) -> Result<Response, NoAnswer> {
    if cfg!(target_os = "wasi") {
        return Err(ritsu_base::wasi::cannot_start("curl").into());
    }
    let first = origin(&req.url);
    let mut url = req.url.clone();
    let mut method = req.method.to_string();
    let mut auth = req.auth.as_ref();
    for hop in 0..=5 {
        let http = allowed(&url)?;
        let r = with_retries(req, &method, &url, auth, http, tmp)?;
        let to = match r.status {
            301 | 302 | 303 | 307 | 308 => r.header("Location").map(|l| resolve(&url, l)),
            _ => None,
        };
        let Some(to) = to else { return Ok(r) };
        if hop == 5 {
            let u = without_query(&req.url);
            return Err(tr!("`{method} {u}` のリダイレクトが 5 回を超えました", "`{method} {u}` was redirected more than five times").into());
        }
        // the credentials go to the request's own origin alone
        if origin(&to) != first {
            auth = None;
        }
        if r.status == 303 || (matches!(r.status, 301 | 302) && method == "POST") {
            method = "GET".to_string();
        }
        url = to;
    }
    unreachable!("the loop returns by its sixth hop")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_and_locations() {
        assert_eq!(origin("http://127.0.0.1:5000/v2/"), Some(("http".into(), "127.0.0.1".into(), 5000)));
        assert_eq!(origin("https://Ghcr.io/v2/x?y=1"), Some(("https".into(), "ghcr.io".into(), 443)));
        assert_eq!(origin("http://[::1]:8080/a"), Some(("http".into(), "[::1]".into(), 8080)));
        assert_ne!(origin("http://127.0.0.1:5000/a"), origin("http://127.0.0.1:5001/a"));
        assert_eq!(resolve("http://h:1/v2/a/blobs/uploads/", "/v2/a/blobs/uploads/x?y"), "http://h:1/v2/a/blobs/uploads/x?y");
        assert_eq!(resolve("http://h:1/v2/a/b", "c"), "http://h:1/v2/a/c");
        assert_eq!(resolve("http://h:1/v2/a", "https://other/x"), "https://other/x");
        assert_eq!(without_query("https://h/a?b=c"), "https://h/a");
        assert_eq!(config_quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }

    #[test]
    fn plain_http_is_for_the_loopback_alone() {
        assert_eq!(allowed("http://127.0.0.1:5000/v2/"), Ok(true));
        assert_eq!(allowed("http://localhost/v2/"), Ok(true));
        assert_eq!(allowed("https://ghcr.io/v2/"), Ok(false));
        assert!(allowed("http://ghcr.io/v2/").is_err());
        assert!(allowed("ftp://ghcr.io/").is_err());
    }

    #[test]
    fn the_errors_of_a_registry_leave_out_their_detail() {
        let body = br#"{"errors":[{"code":"UNAUTHORIZED","message":"authentication required","detail":"ritsu-fake-secret"}]}"#;
        assert_eq!(registry_errors(body), "UNAUTHORIZED: authentication required");
        let r = Response { status: 401, headers: vec![], body: body.to_vec(), url: "http://127.0.0.1:1/v2/a/tags/list?n=1000".into() };
        let e = r.error("GET").en;
        assert!(!e.contains("ritsu-fake-secret") && !e.contains("n=1000"), "{e}");
    }
}
