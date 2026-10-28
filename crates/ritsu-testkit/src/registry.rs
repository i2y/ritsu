//! A small OCI registry inside a test (DESIGN 17.25): the part of the distribution-spec that
//! `ritsu mod` and cosign use, on `127.0.0.1` on a port no one listens on, stopped when its value
//! is dropped. Nothing but std, as the rest of the testkit.
//!
//! - `GET /v2/`; an OCI manifest's `HEAD`, `GET` and `PUT`, by a tag or a digest; a blob's `HEAD`
//!   and `GET`; an upload's `POST` (with a `digest`, in one request), `PATCH` (in parts) and `PUT`;
//!   `tags/list` with `n`, `last` and `Link`; the referrers of a digest, filtered by
//!   `artifactType`, and `OCI-Subject` when a manifest with a `subject` is put; the errors as the
//!   spec writes them (`{"errors": [{"code", "message", "detail"}]}`).
//! - Credentials ([`Auth`]): none, `Basic`, or the token's steps, whose realm is on the same server
//!   (`GET /token` with the user and the password or none, and `POST /token` with an identity
//!   token).
//! - A blob's `GET` sent on to a second port ([`Registry::redirect_blobs`]), whose log says whether
//!   an `Authorization` header reached it.
//! - Broken answers: other bytes for a digest ([`Registry::serve_instead`]), a status for the next
//!   requests that match ([`Registry::fail`]), with `Retry-After`; a tag moved and a tar that holds
//!   `..`, an absolute path or a link are put through [`Registry::put_blob`],
//!   [`Registry::put_manifest`] and [`tar`].
//!
//! Every request is logged ([`Registry::log`]): its method, its target, the port, and its
//! `Authorization`, which a test reads to see where credentials went.

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// The media types the registry knows by name.
pub const OCI_MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
pub const OCI_INDEX: &str = "application/vnd.oci.image.index.v1+json";

/// How the registry asks for credentials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Auth {
    /// Anyone pulls and pushes.
    None,
    /// `WWW-Authenticate: Basic`: this user and password on each request.
    Basic { user: String, password: String },
    /// `WWW-Authenticate: Bearer` with the realm on this server.
    Token(TokenAuth),
}

/// What the realm of the token's steps takes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TokenAuth {
    /// The user and the password a `GET` of the realm takes (`Basic`).
    pub credentials: Option<(String, String)>,
    /// The identity token a `POST` of the realm takes (`grant_type=refresh_token`).
    pub refresh_token: Option<String>,
    /// Whether a `GET` with no credentials gets a token: one to pull only when `credentials` or
    /// `refresh_token` is set, else one to pull and push.
    pub anonymous: bool,
}

/// A request the registry took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    pub method: String,
    /// The path and the query.
    pub target: String,
    /// The port it came to: the registry's, or the second one ([`Registry::redirect_blobs`]).
    pub port: u16,
    pub authorization: Option<String>,
    /// The status it was answered with.
    pub status: u16,
}

#[derive(Clone, Debug)]
struct Fault {
    method: Option<String>,
    contains: String,
    status: u16,
    retry_after: Option<u32>,
    times: usize,
}

#[derive(Default)]
struct State {
    auth: Option<Auth>,
    /// (repository, digest) → bytes.
    blobs: HashMap<(String, String), Vec<u8>>,
    /// (repository, digest) → (media type, bytes).
    manifests: HashMap<(String, String), (String, Vec<u8>)>,
    tags: HashMap<String, BTreeMap<String, String>>,
    /// upload id → (repository, the bytes so far).
    uploads: HashMap<String, (String, Vec<u8>)>,
    /// token → its scopes (`repository:<name>:<actions>`).
    tokens: HashMap<String, Vec<String>>,
    instead: HashMap<String, Vec<u8>>,
    faults: Vec<Fault>,
    redirect: bool,
    log: Vec<Seen>,
}

/// The registry, until it is dropped.
pub struct Registry {
    /// `127.0.0.1:<port>`: the host a module's name starts with (`127.0.0.1:<port>/acme/pricing`).
    pub host: String,
    /// `http://127.0.0.1:<port>`.
    pub url: String,
    /// The second port, where a blob's `GET` is sent on to.
    pub second: u16,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

static N: AtomicU64 = AtomicU64::new(0);

/// A word no other request takes at the same time: upload ids and tokens.
fn fresh(what: &str) -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{what}-{}-{nanos:x}", N.fetch_add(1, Ordering::SeqCst))
}

impl Registry {
    /// A registry anyone pulls from and pushes to.
    pub fn start() -> Registry {
        Registry::with_auth(Auth::None)
    }

    pub fn with_auth(auth: Auth) -> Registry {
        let main = TcpListener::bind("127.0.0.1:0").expect("a port on 127.0.0.1");
        let second = TcpListener::bind("127.0.0.1:0").expect("a port on 127.0.0.1");
        let port = main.local_addr().unwrap().port();
        let second_port = second.local_addr().unwrap().port();
        let state = Arc::new(Mutex::new(State { auth: Some(auth), ..State::default() }));
        let stop = Arc::new(AtomicBool::new(false));
        let mut threads = Vec::new();
        for (l, p, is_second) in [(main, port, false), (second, second_port, true)] {
            let (state, stop) = (Arc::clone(&state), Arc::clone(&stop));
            threads.push(std::thread::spawn(move || {
                for conn in l.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(conn) = conn else { continue };
                    let (state, stop) = (Arc::clone(&state), Arc::clone(&stop));
                    std::thread::spawn(move || serve_connection(conn, &state, &stop, p, port, second_port, is_second));
                }
            }));
        }
        Registry { host: format!("127.0.0.1:{port}"), url: format!("http://127.0.0.1:{port}"), second: second_port, state, stop, threads }
    }

    /// The registry's state, locked.
    fn st(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A blob put in `repository` (as an upload would): its digest.
    pub fn put_blob(&self, repository: &str, bytes: &[u8]) -> String {
        let d = digest(bytes);
        self.st().blobs.insert((repository.to_string(), d.clone()), bytes.to_vec());
        d
    }

    /// A manifest put in `repository` with the media type `media`, under `tag` when it is given
    /// (moving the tag when it is there): its digest. Nothing is checked, so a test puts what a
    /// registry should not hold too.
    pub fn put_manifest(&self, repository: &str, tag: Option<&str>, media: &str, bytes: &[u8]) -> String {
        let d = digest(bytes);
        let mut st = self.st();
        st.manifests.insert((repository.to_string(), d.clone()), (media.to_string(), bytes.to_vec()));
        if let Some(t) = tag {
            st.tags.entry(repository.to_string()).or_default().insert(t.to_string(), d.clone());
        }
        d
    }

    /// The bytes of a blob or a manifest of `repository`.
    pub fn get(&self, repository: &str, digest: &str) -> Option<Vec<u8>> {
        let st = self.st();
        let key = (repository.to_string(), digest.to_string());
        st.blobs.get(&key).cloned().or_else(|| st.manifests.get(&key).map(|(_, b)| b.clone()))
    }

    /// The digest a tag of `repository` points to.
    pub fn tag(&self, repository: &str, tag: &str) -> Option<String> {
        self.st().tags.get(repository).and_then(|t| t.get(tag).cloned())
    }

    /// The tags of `repository`, in their order.
    pub fn tags(&self, repository: &str) -> Vec<String> {
        self.st().tags.get(repository).map(|t| t.keys().cloned().collect()).unwrap_or_default()
    }

    /// The digests of the manifests in `repository`.
    pub fn manifests(&self, repository: &str) -> Vec<String> {
        let mut out: Vec<String> = self.st().manifests.keys().filter(|(r, _)| r == repository).map(|(_, d)| d.clone()).collect();
        out.sort();
        out
    }

    /// `bytes` given for `digest` (a blob's or a manifest's), whatever is held: a registry that
    /// answers with other bytes than those asked for.
    pub fn serve_instead(&self, digest: &str, bytes: &[u8]) {
        self.st().instead.insert(digest.to_string(), bytes.to_vec());
    }

    /// The next `times` requests whose method is `method` (any, when None) and whose target
    /// contains `contains` answered with `status` and an error, with `Retry-After` when it is
    /// given.
    pub fn fail(&self, method: Option<&str>, contains: &str, status: u16, times: usize, retry_after: Option<u32>) {
        self.st().faults.push(Fault { method: method.map(str::to_string), contains: contains.to_string(), status, retry_after, times });
    }

    /// A blob's `GET` answered with `307` to the second port, which serves it there.
    pub fn redirect_blobs(&self) {
        self.st().redirect = true;
    }

    /// Every request taken, in order.
    pub fn log(&self) -> Vec<Seen> {
        self.st().log.clone()
    }

    /// The log emptied.
    pub fn clear_log(&self) {
        self.st().log.clear();
    }

    /// Every token the realm gave.
    pub fn tokens(&self) -> Vec<String> {
        self.st().tokens.keys().cloned().collect()
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.host);
        let _ = TcpStream::connect(format!("127.0.0.1:{}", self.second));
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

/// A request read off a connection.
struct Request {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or("")
    }

    fn query(&self, key: &str) -> Option<String> {
        let q = self.target.split_once('?')?.1;
        q.split('&').find_map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            (percent_decode(k) == key).then(|| percent_decode(v))
        })
    }
}

/// What the registry answers.
struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    /// For a `HEAD`: the length the `GET` would have, and no body.
    head: bool,
}

impl Response {
    fn new(status: u16) -> Response {
        Response { status, headers: Vec::new(), body: Vec::new(), head: false }
    }

    fn header(mut self, k: &str, v: impl Into<String>) -> Response {
        self.headers.push((k.to_string(), v.into()));
        self
    }

    fn body(mut self, b: impl Into<Vec<u8>>) -> Response {
        self.body = b.into();
        self
    }

    fn error(status: u16, code: &str, message: &str) -> Response {
        let body = format!("{{\"errors\":[{{\"code\":{},\"message\":{},\"detail\":null}}]}}", quote(code), quote(message));
        Response::new(status).header("Content-Type", "application/json").body(body)
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        307 => "Temporary Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        416 => "Range Not Satisfiable",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// The requests of one connection, until it closes or asks to.
fn serve_connection(conn: TcpStream, state: &Arc<Mutex<State>>, stop: &AtomicBool, port: u16, main: u16, second: u16, is_second: bool) {
    let Ok(write_half) = conn.try_clone() else { return };
    let mut writer = write_half;
    let mut reader = BufReader::new(conn);
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let Some(req) = read_request(&mut reader, &mut writer) else { return };
        let close = req.header("Connection").is_some_and(|v| v.eq_ignore_ascii_case("close"));
        let resp = if is_second { serve_second(&req, state) } else { serve(&req, state, main, second) };
        {
            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
            st.log.push(Seen { method: req.method.clone(), target: req.target.clone(), port, authorization: req.header("Authorization").map(str::to_string), status: resp.status });
        }
        let mut out = format!("HTTP/1.1 {} {}\r\n", resp.status, reason(resp.status));
        for (k, v) in &resp.headers {
            out.push_str(&format!("{k}: {v}\r\n"));
        }
        out.push_str(&format!("Content-Length: {}\r\n", resp.body.len()));
        if close {
            out.push_str("Connection: close\r\n");
        }
        out.push_str("\r\n");
        if writer.write_all(out.as_bytes()).is_err() {
            return;
        }
        if !resp.head && req.method != "HEAD" && writer.write_all(&resp.body).is_err() {
            return;
        }
        let _ = writer.flush();
        if close {
            return;
        }
    }
}

/// One request: its line, its headers and its body (by `Content-Length` or in chunks). A request
/// that says `Expect: 100-continue` is told to go on first.
fn read_request(r: &mut BufReader<TcpStream>, w: &mut TcpStream) -> Option<Request> {
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        if !line.trim().is_empty() {
            break;
        }
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).ok()? == 0 {
            return None;
        }
        let h = h.trim_end_matches(['\r', '\n']);
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let get = |name: &str| headers.iter().find(|(k, _): &&(String, String)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone());
    if get("Expect").is_some_and(|v| v.eq_ignore_ascii_case("100-continue")) {
        let _ = w.write_all(b"HTTP/1.1 100 Continue\r\n\r\n");
        let _ = w.flush();
    }
    let mut body = Vec::new();
    if get("Transfer-Encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked")) {
        loop {
            let mut size = String::new();
            r.read_line(&mut size).ok()?;
            let n = usize::from_str_radix(size.trim().split(';').next().unwrap_or("0").trim(), 16).ok()?;
            if n == 0 {
                // the trailers, to the blank line
                loop {
                    let mut t = String::new();
                    if r.read_line(&mut t).ok()? == 0 || t.trim().is_empty() {
                        break;
                    }
                }
                break;
            }
            let mut chunk = vec![0u8; n];
            r.read_exact(&mut chunk).ok()?;
            body.extend_from_slice(&chunk);
            let mut crlf = String::new();
            r.read_line(&mut crlf).ok()?;
        }
    } else if let Some(n) = get("Content-Length").and_then(|v| v.parse::<usize>().ok()) {
        body = vec![0u8; n];
        r.read_exact(&mut body).ok()?;
    }
    Some(Request { method, target, headers, body })
}

/// The second port: the blobs a `GET` was sent on to.
fn serve_second(req: &Request, state: &Arc<Mutex<State>>) -> Response {
    let st = state.lock().unwrap_or_else(|e| e.into_inner());
    let Some(rest) = req.path().strip_prefix("/blobs/") else { return Response::error(404, "NOT_FOUND", "not found") };
    let (repo, d) = rest.rsplit_once('/').unwrap_or(("", rest));
    let repo = percent_decode(repo);
    let held = st.instead.get(d).cloned().or_else(|| st.blobs.get(&(repo, d.to_string())).cloned());
    match held {
        Some(b) => Response::new(200).header("Content-Type", "application/octet-stream").body(b),
        None => Response::error(404, "BLOB_UNKNOWN", "blob unknown to registry"),
    }
}

/// What an API request of `repository` needs: `pull`, or `push` too.
fn actions(method: &str) -> &'static str {
    match method {
        "GET" | "HEAD" => "pull",
        _ => "pull,push",
    }
}

/// Whether a token's scopes let `actions` on `repository`.
fn scopes_let(scopes: &[String], repository: &str, actions: &str) -> bool {
    actions.split(',').all(|a| {
        scopes.iter().any(|s| {
            let mut it = s.splitn(3, ':');
            let (kind, name, acts) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""));
            kind == "repository" && name == repository && acts.split(',').any(|x| x == a || x == "*")
        })
    })
}

/// Why a request is not let in, when it is not: the 401 to answer with.
fn unauthorized(req: &Request, st: &State, main: u16, repository: Option<&str>) -> Option<Response> {
    let auth = st.auth.clone().unwrap_or(Auth::None);
    let given = req.header("Authorization").unwrap_or("");
    match auth {
        Auth::None => None,
        Auth::Basic { user, password } => {
            let ok = given.strip_prefix("Basic ").and_then(|b| base64_decode(b.trim())).is_some_and(|b| b == format!("{user}:{password}").into_bytes());
            (!ok).then(|| Response::error(401, "UNAUTHORIZED", "authentication required").header("WWW-Authenticate", "Basic realm=\"ritsu-test\""))
        }
        Auth::Token(_) => {
            let acts = actions(&req.method);
            let ok = match given.strip_prefix("Bearer ") {
                Some(t) => match (st.tokens.get(t.trim()), repository) {
                    (Some(_), None) => true,
                    (Some(scopes), Some(r)) => scopes_let(scopes, r, acts),
                    (None, _) => false,
                },
                None => false,
            };
            if ok {
                return None;
            }
            let mut challenge = format!("Bearer realm=\"http://127.0.0.1:{main}/token\",service=\"ritsu-test\"");
            if let Some(r) = repository {
                challenge.push_str(&format!(",scope=\"repository:{r}:{acts}\""));
            }
            Some(Response::error(401, "UNAUTHORIZED", "authentication required").header("WWW-Authenticate", challenge))
        }
    }
}

/// The realm of the token's steps: `GET /token` and `POST /token`.
fn token(req: &Request, st: &mut State) -> Response {
    let Some(Auth::Token(t)) = st.auth.clone() else { return Response::error(404, "NOT_FOUND", "no realm") };
    let (form, scopes, who): (HashMap<String, String>, Vec<String>, Option<bool>) = if req.method == "POST" {
        let form: HashMap<String, String> = String::from_utf8_lossy(&req.body)
            .split('&')
            .filter_map(|kv| kv.split_once('=').map(|(k, v)| (percent_decode(k), percent_decode(v))))
            .collect();
        let scopes = form.get("scope").map(|s| s.split(' ').map(str::to_string).collect()).unwrap_or_default();
        let ok = form.get("grant_type").map(String::as_str) == Some("refresh_token") && t.refresh_token.is_some() && form.get("refresh_token") == t.refresh_token.as_ref();
        (form, scopes, Some(ok))
    } else {
        let scopes: Vec<String> = req.query("scope").map(|s| s.split(' ').map(str::to_string).collect()).unwrap_or_default();
        let who = match req.header("Authorization").and_then(|v| v.strip_prefix("Basic ")) {
            Some(b) => Some(t.credentials.as_ref().is_some_and(|(u, p)| base64_decode(b.trim()) == Some(format!("{u}:{p}").into_bytes()))),
            None => None,
        };
        (HashMap::new(), scopes, who)
    };
    let _ = form;
    let scopes = match who {
        Some(true) => scopes,
        Some(false) => return Response::error(401, "UNAUTHORIZED", "credentials refused"),
        None if t.anonymous => {
            if t.credentials.is_some() || t.refresh_token.is_some() {
                scopes.iter().map(|s| match s.rsplit_once(':') {
                    Some((head, _)) => format!("{head}:pull"),
                    None => s.clone(),
                }).collect()
            } else {
                scopes
            }
        }
        None => return Response::error(401, "UNAUTHORIZED", "credentials required"),
    };
    let tok = fresh("ritsu-fake-token");
    st.tokens.insert(tok.clone(), scopes);
    let body = if req.method == "POST" {
        format!("{{\"access_token\":{},\"expires_in\":300,\"scope\":\"\"}}", quote(&tok))
    } else {
        format!("{{\"token\":{},\"access_token\":{},\"expires_in\":300}}", quote(&tok), quote(&tok))
    };
    Response::new(200).header("Content-Type", "application/json").body(body)
}

/// The part of the path after `/v2/`, split into the repository and what is asked of it.
enum Route {
    Base,
    Manifest(String, String),
    Blob(String, String),
    UploadStart(String),
    Upload(String, String),
    Tags(String),
    Referrers(String, String),
    Unknown,
}

fn route(path: &str) -> Route {
    let Some(rest) = path.strip_prefix("/v2/") else { return Route::Unknown };
    if rest.is_empty() {
        return Route::Base;
    }
    if let Some(r) = rest.strip_suffix("/tags/list") {
        return Route::Tags(r.to_string());
    }
    if let Some(r) = rest.strip_suffix("/blobs/uploads/").or_else(|| rest.strip_suffix("/blobs/uploads")) {
        return Route::UploadStart(r.to_string());
    }
    for (mark, make) in [
        ("/blobs/uploads/", Route::Upload as fn(String, String) -> Route),
        ("/manifests/", Route::Manifest),
        ("/referrers/", Route::Referrers),
        ("/blobs/", Route::Blob),
    ] {
        if let Some(at) = rest.rfind(mark) {
            return make(rest[..at].to_string(), rest[at + mark.len()..].to_string());
        }
    }
    Route::Unknown
}

fn serve(req: &Request, state: &Arc<Mutex<State>>, main: u16, second: u16) -> Response {
    let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
    // a fault the test asked for
    if let Some(i) = st.faults.iter().position(|f| f.times > 0 && f.method.as_deref().is_none_or(|m| m == req.method) && req.target.contains(&f.contains)) {
        st.faults[i].times -= 1;
        let f = st.faults[i].clone();
        let mut r = Response::error(f.status, "TOOMANYREQUESTS", "a fault the test asked for");
        if let Some(s) = f.retry_after {
            r = r.header("Retry-After", s.to_string());
        }
        return r;
    }
    if req.path() == "/token" {
        return token(req, &mut st);
    }
    let r = route(req.path());
    let repository = match &r {
        Route::Manifest(n, _) | Route::Blob(n, _) | Route::UploadStart(n) | Route::Upload(n, _) | Route::Tags(n) | Route::Referrers(n, _) => Some(n.clone()),
        Route::Base | Route::Unknown => None,
    };
    if !matches!(r, Route::Unknown)
        && let Some(no) = unauthorized(req, &st, main, repository.as_deref())
    {
        return no;
    }
    let head = req.method == "HEAD";
    let mut resp = match (req.method.as_str(), r) {
        ("GET" | "HEAD", Route::Base) => Response::new(200).header("Content-Type", "application/json").header("Docker-Distribution-API-Version", "registry/2.0").body("{}"),
        ("GET" | "HEAD", Route::Manifest(repo, reference)) => {
            let d = if is_digest(&reference) { Some(reference.clone()) } else { st.tags.get(&repo).and_then(|t| t.get(&reference).cloned()) };
            match d.and_then(|d| st.manifests.get(&(repo.clone(), d.clone())).map(|m| (d, m.clone()))) {
                Some((d, (media, bytes))) => {
                    let bytes = st.instead.get(&d).cloned().unwrap_or(bytes);
                    Response::new(200).header("Content-Type", media).header("Docker-Content-Digest", d).body(bytes)
                }
                None => Response::error(404, "MANIFEST_UNKNOWN", "manifest unknown to registry"),
            }
        }
        ("PUT", Route::Manifest(repo, reference)) => put_manifest(&mut st, req, &repo, &reference),
        ("GET" | "HEAD", Route::Blob(repo, d)) => match st.blobs.get(&(repo.clone(), d.clone())).cloned() {
            Some(bytes) if req.method == "GET" && st.redirect => {
                let _ = bytes;
                Response::new(307).header("Location", format!("http://127.0.0.1:{second}/blobs/{}/{d}?expires=ritsu-test", percent_encode(&repo)))
            }
            Some(bytes) => {
                let bytes = st.instead.get(&d).cloned().unwrap_or(bytes);
                Response::new(200).header("Content-Type", "application/octet-stream").header("Docker-Content-Digest", d).body(bytes)
            }
            None => Response::error(404, "BLOB_UNKNOWN", "blob unknown to registry"),
        },
        ("POST", Route::UploadStart(repo)) => match req.query("digest") {
            // in one request
            Some(d) => {
                if digest(&req.body) != d {
                    Response::error(400, "DIGEST_INVALID", "provided digest did not match uploaded content")
                } else {
                    st.blobs.insert((repo.clone(), d.clone()), req.body.clone());
                    Response::new(201).header("Location", format!("/v2/{repo}/blobs/{d}")).header("Docker-Content-Digest", d)
                }
            }
            None => {
                let id = fresh("upload");
                st.uploads.insert(id.clone(), (repo.clone(), Vec::new()));
                Response::new(202).header("Location", format!("/v2/{repo}/blobs/uploads/{id}")).header("Docker-Upload-UUID", id).header("Range", "0-0")
            }
        },
        ("PATCH", Route::Upload(repo, id)) => match st.uploads.get_mut(&id) {
            Some((r, bytes)) if *r == repo => {
                bytes.extend_from_slice(&req.body);
                let end = bytes.len().saturating_sub(1);
                Response::new(202).header("Location", format!("/v2/{repo}/blobs/uploads/{id}")).header("Docker-Upload-UUID", id).header("Range", format!("0-{end}"))
            }
            _ => Response::error(404, "BLOB_UPLOAD_UNKNOWN", "blob upload unknown to registry"),
        },
        ("PUT", Route::Upload(repo, id)) => match st.uploads.remove(&id) {
            Some((r, mut bytes)) if r == repo => {
                bytes.extend_from_slice(&req.body);
                match req.query("digest") {
                    Some(d) if digest(&bytes) == d => {
                        st.blobs.insert((repo.clone(), d.clone()), bytes);
                        Response::new(201).header("Location", format!("/v2/{repo}/blobs/{d}")).header("Docker-Content-Digest", d)
                    }
                    _ => Response::error(400, "DIGEST_INVALID", "provided digest did not match uploaded content"),
                }
            }
            _ => Response::error(404, "BLOB_UPLOAD_UNKNOWN", "blob upload unknown to registry"),
        },
        ("GET", Route::Tags(repo)) => tags_list(&st, req, &repo),
        ("GET", Route::Referrers(repo, d)) => referrers(&st, req, &repo, &d),
        (_, Route::Unknown) => Response::error(404, "NOT_FOUND", "not found"),
        _ => Response::error(405, "UNSUPPORTED", "the operation is unsupported"),
    };
    resp.head = head;
    resp
}

/// A manifest's `PUT`: its blobs have to be in the repository, a digest given has to be the
/// bytes'.
fn put_manifest(st: &mut State, req: &Request, repo: &str, reference: &str) -> Response {
    let d = digest(&req.body);
    if is_digest(reference) && reference != d {
        return Response::error(400, "DIGEST_INVALID", "provided digest did not match uploaded content");
    }
    let Some(j) = std::str::from_utf8(&req.body).ok().and_then(|s| Json::parse(s).ok()) else {
        return Response::error(400, "MANIFEST_INVALID", "manifest invalid");
    };
    let media = req.header("Content-Type").map(str::to_string).or_else(|| j.get("mediaType").and_then(Json::as_str).map(str::to_string)).unwrap_or_else(|| OCI_MANIFEST.to_string());
    if media != OCI_INDEX {
        let mut needed: Vec<String> = Vec::new();
        if let Some(c) = j.get("config").and_then(|c| c.get("digest")).and_then(Json::as_str) {
            needed.push(c.to_string());
        }
        for l in j.get("layers").and_then(Json::as_arr).unwrap_or(&[]) {
            if let Some(x) = l.get("digest").and_then(Json::as_str) {
                needed.push(x.to_string());
            }
        }
        if needed.iter().any(|x| !st.blobs.contains_key(&(repo.to_string(), x.clone()))) {
            return Response::error(400, "MANIFEST_BLOB_UNKNOWN", "blob unknown to registry");
        }
    }
    st.manifests.insert((repo.to_string(), d.clone()), (media, req.body.clone()));
    if !is_digest(reference) {
        st.tags.entry(repo.to_string()).or_default().insert(reference.to_string(), d.clone());
    }
    let mut r = Response::new(201).header("Location", format!("/v2/{repo}/manifests/{d}")).header("Docker-Content-Digest", d);
    if let Some(s) = j.get("subject").and_then(|s| s.get("digest")).and_then(Json::as_str) {
        r = r.header("OCI-Subject", s.to_string());
    }
    r
}

/// `tags/list`, in the order of the tags, from after `last`, `n` at a time with a `Link` to the
/// next ones.
fn tags_list(st: &State, req: &Request, repo: &str) -> Response {
    let Some(all) = st.tags.get(repo) else { return Response::error(404, "NAME_UNKNOWN", "repository name not known to registry") };
    let last = req.query("last");
    let mut tags: Vec<&String> = all.keys().filter(|t| last.as_ref().is_none_or(|l| t.as_str() > l.as_str())).collect();
    let mut link = None;
    if let Some(n) = req.query("n").and_then(|n| n.parse::<usize>().ok())
        && tags.len() > n
    {
        tags.truncate(n);
        if let Some(l) = tags.last() {
            link = Some(format!("</v2/{repo}/tags/list?n={n}&last={}>; rel=\"next\"", percent_encode(l)));
        }
    }
    let list: Vec<String> = tags.iter().map(|t| quote(t)).collect();
    let mut r = Response::new(200).header("Content-Type", "application/json").body(format!("{{\"name\":{},\"tags\":[{}]}}", quote(repo), list.join(",")));
    if let Some(l) = link {
        r = r.header("Link", l);
    }
    r
}

/// The referrers of a digest: an image index of the manifests whose `subject` is it, filtered by
/// `artifactType`.
fn referrers(st: &State, req: &Request, repo: &str, d: &str) -> Response {
    let filter = req.query("artifactType");
    let mut found: Vec<String> = Vec::new();
    let mut keys: Vec<&(String, String)> = st.manifests.keys().filter(|(r, _)| r == repo).collect();
    keys.sort();
    for k in keys {
        let (media, bytes) = &st.manifests[k];
        let Some(j) = std::str::from_utf8(bytes).ok().and_then(|s| Json::parse(s).ok()) else { continue };
        if j.get("subject").and_then(|s| s.get("digest")).and_then(Json::as_str) != Some(d) {
            continue;
        }
        let at = j.get("artifactType").and_then(Json::as_str).map(str::to_string).or_else(|| j.get("config").and_then(|c| c.get("mediaType")).and_then(Json::as_str).map(str::to_string));
        if filter.as_ref().is_some_and(|f| at.as_deref() != Some(f.as_str())) {
            continue;
        }
        let mut entry = format!("{{\"mediaType\":{},\"digest\":{},\"size\":{}", quote(media), quote(&k.1), bytes.len());
        if let Some(a) = &at {
            entry.push_str(&format!(",\"artifactType\":{}", quote(a)));
        }
        if let Some(Json::Obj(notes)) = j.get("annotations") {
            let pairs: Vec<String> = notes.iter().filter_map(|(k, v)| v.as_str().map(|s| format!("{}:{}", quote(k), quote(s)))).collect();
            entry.push_str(&format!(",\"annotations\":{{{}}}", pairs.join(",")));
        }
        entry.push('}');
        found.push(entry);
    }
    let mut r = Response::new(200).header("Content-Type", OCI_INDEX).body(format!("{{\"schemaVersion\":2,\"mediaType\":{},\"manifests\":[{}]}}", quote(OCI_INDEX), found.join(",")));
    if filter.is_some() {
        r = r.header("OCI-Filters-Applied", "artifactType");
    }
    r
}

/// Whether `s` is a sha256 digest.
pub fn is_digest(s: &str) -> bool {
    s.strip_prefix("sha256:").is_some_and(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn percent_encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// A JSON string.
pub fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---- SHA-256 (FIPS 180-4), for the digests ----

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7,
    0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
    0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
    0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The SHA-256 of `bytes`, in lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = bytes.to_vec();
    let bits = (bytes.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks(4).enumerate().take(16) {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

/// `sha256:` and the hex of the bytes' SHA-256.
pub fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(bytes))
}

// ---- base64, for `Basic` ----

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim_end_matches('=');
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = B64.iter().position(|b| *b == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ---- JSON, enough to read a manifest ----

/// A JSON value, its objects in the order written.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(s: &str) -> Result<Json, String> {
        let b = s.as_bytes();
        let mut at = 0;
        let v = value(b, &mut at)?;
        skip_ws(b, &mut at);
        if at != b.len() {
            return Err(format!("text after the value at byte {at}"));
        }
        Ok(v)
    }

    pub fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(ps) => ps.iter().find(|(x, _)| x == k).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::Num(n) => n.parse().ok(),
            _ => None,
        }
    }
}

fn skip_ws(b: &[u8], at: &mut usize) {
    while *at < b.len() && matches!(b[*at], b' ' | b'\n' | b'\r' | b'\t') {
        *at += 1;
    }
}

fn value(b: &[u8], at: &mut usize) -> Result<Json, String> {
    skip_ws(b, at);
    match b.get(*at) {
        Some(b'{') => {
            *at += 1;
            let mut ps = Vec::new();
            skip_ws(b, at);
            if b.get(*at) == Some(&b'}') {
                *at += 1;
                return Ok(Json::Obj(ps));
            }
            loop {
                skip_ws(b, at);
                let Json::Str(k) = string(b, at)? else { unreachable!() };
                skip_ws(b, at);
                if b.get(*at) != Some(&b':') {
                    return Err(format!("no `:` at byte {at}"));
                }
                *at += 1;
                ps.push((k, value(b, at)?));
                skip_ws(b, at);
                match b.get(*at) {
                    Some(b',') => *at += 1,
                    Some(b'}') => {
                        *at += 1;
                        return Ok(Json::Obj(ps));
                    }
                    _ => return Err(format!("no `,` or `}}` at byte {at}")),
                }
            }
        }
        Some(b'[') => {
            *at += 1;
            let mut xs = Vec::new();
            skip_ws(b, at);
            if b.get(*at) == Some(&b']') {
                *at += 1;
                return Ok(Json::Arr(xs));
            }
            loop {
                xs.push(value(b, at)?);
                skip_ws(b, at);
                match b.get(*at) {
                    Some(b',') => *at += 1,
                    Some(b']') => {
                        *at += 1;
                        return Ok(Json::Arr(xs));
                    }
                    _ => return Err(format!("no `,` or `]` at byte {at}")),
                }
            }
        }
        Some(b'"') => string(b, at),
        Some(b't') if b[*at..].starts_with(b"true") => {
            *at += 4;
            Ok(Json::Bool(true))
        }
        Some(b'f') if b[*at..].starts_with(b"false") => {
            *at += 5;
            Ok(Json::Bool(false))
        }
        Some(b'n') if b[*at..].starts_with(b"null") => {
            *at += 4;
            Ok(Json::Null)
        }
        Some(c) if c.is_ascii_digit() || *c == b'-' => {
            let start = *at;
            while *at < b.len() && (b[*at].is_ascii_digit() || matches!(b[*at], b'-' | b'+' | b'.' | b'e' | b'E')) {
                *at += 1;
            }
            Ok(Json::Num(String::from_utf8_lossy(&b[start..*at]).into_owned()))
        }
        _ => Err(format!("no value at byte {at}")),
    }
}

fn string(b: &[u8], at: &mut usize) -> Result<Json, String> {
    if b.get(*at) != Some(&b'"') {
        return Err(format!("no string at byte {at}"));
    }
    *at += 1;
    let mut out: Vec<u8> = Vec::new();
    while let Some(&c) = b.get(*at) {
        *at += 1;
        match c {
            b'"' => return Ok(Json::Str(String::from_utf8_lossy(&out).into_owned())),
            b'\\' => {
                let e = *b.get(*at).ok_or("an escape at the end")?;
                *at += 1;
                match e {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'r' => out.push(b'\r'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'u' => {
                        let hex = std::str::from_utf8(b.get(*at..*at + 4).ok_or("a short escape")?).map_err(|e| e.to_string())?;
                        let n = u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
                        *at += 4;
                        let ch = char::from_u32(n).unwrap_or('\u{fffd}');
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    other => out.push(other),
                }
            }
            c => out.push(c),
        }
    }
    Err("a string with no end".to_string())
}

// ---- tar, for the broken answers ----

/// A tar of entries as a test writes it, any typeflag and any name: the files of a module that a
/// fetch has to refuse (a `..`, an absolute path, a link). `(name, typeflag, bytes, link)`.
pub fn tar(entries: &[(&str, u8, &[u8], &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, flag, bytes, link) in entries {
        let mut h = [0u8; 512];
        h[..name.len().min(100)].copy_from_slice(&name.as_bytes()[..name.len().min(100)]);
        h[100..107].copy_from_slice(b"0000644");
        h[108..115].copy_from_slice(b"0000000");
        h[116..123].copy_from_slice(b"0000000");
        let size = format!("{:011o}", bytes.len());
        h[124..135].copy_from_slice(size.as_bytes());
        h[136..147].copy_from_slice(b"00000000000");
        h[156] = *flag;
        h[157..157 + link.len().min(100)].copy_from_slice(&link.as_bytes()[..link.len().min(100)]);
        h[257..263].copy_from_slice(b"ustar\0");
        h[263..265].copy_from_slice(b"00");
        h[148..156].copy_from_slice(b"        ");
        let sum: u32 = h.iter().map(|x| u32::from(*x)).sum();
        h[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
        h[155] = b' ';
        out.extend_from_slice(&h);
        out.extend_from_slice(bytes);
        let pad = (512 - bytes.len() % 512) % 512;
        out.extend(std::iter::repeat_n(0u8, pad));
    }
    out.extend(std::iter::repeat_n(0u8, 1024));
    out
}
