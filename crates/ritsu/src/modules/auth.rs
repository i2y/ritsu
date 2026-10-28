//! The credentials of a registry and its tokens (DESIGN 17.13): the Docker config file, its helpers,
//! and the token a registry gives.
//!
//! A request goes first with no `Authorization`, or with what an earlier request of the run was
//! granted for the same repository and the same actions. A 401 says how to go on:
//!
//! - `WWW-Authenticate: Bearer realm="…",service="…",scope="…"`: the realm is asked for a token
//!   (`GET <realm>?service=…&scope=…`, with the credentials as `Basic` when there are some, with
//!   none for an anonymous token), or, for an identity token, `POST <realm>` with
//!   `grant_type=refresh_token` (a 404 there says the registry does not take one: the run stops).
//!   The token (`token`, else `access_token`) is kept for the run, by its realm, service and scope,
//!   for `expires_in` seconds (60 when it does not say).
//! - `WWW-Authenticate: Basic …`: the credentials, as `Basic`.
//!
//! A 401 after that names the registry and where the credentials came from (the helper of
//! `credHelpers`, `credsStore`, `auths`), never what they are. The credentials are looked up once,
//! when a registry first asks for them: `$DOCKER_CONFIG/config.json`, else `~/.docker/config.json`;
//! for the registry's host, `credHelpers` (the helper `docker-credential-<name> get`, the host on
//! its standard input), then `auths` (its keys compared without their scheme and path; `auth` is
//! `user:password` in base64, `identitytoken` an identity token), then `credsStore`. A helper that
//! answers that it has none (an exit code that is not 0) leaves the run anonymous. `docker.io` is
//! asked at `registry-1.docker.io` and its credentials are those of `https://index.docker.io/v1/`.
//!
//! Every value here is a [`Secret`]; what the helpers print is read and never shown.

use super::http::{self, Body, NoAnswer, Request, Response, Secret};
use ritsu_base::json::{self, Json};
use ritsu_base::text::Text;
use ritsu_base::tr;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Where credentials came from, as a sentence names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A helper of `credHelpers`, by its name.
    Helper(String),
    /// The helper of `credsStore`, by its name.
    Store(String),
    /// `auths` of the config file at this path.
    Auths(PathBuf),
}

impl Source {
    fn text(&self) -> Text {
        match self {
            Source::Helper(h) => tr!(
                "Docker の設定ファイルの `credHelpers` のヘルパー `docker-credential-{h}`",
                "the helper `docker-credential-{h}` of `credHelpers` in the Docker config file"
            ),
            Source::Store(h) => tr!(
                "Docker の設定ファイルの `credsStore` のヘルパー `docker-credential-{h}`",
                "the helper `docker-credential-{h}` of `credsStore` in the Docker config file"
            ),
            Source::Auths(p) => {
                let p = p.display();
                tr!("Docker の設定ファイル {p} の `auths`", "`auths` in the Docker config file {p}")
            }
        }
    }
}

/// Credentials: a user and a password, or an identity token.
#[derive(Clone)]
pub enum Kind {
    Basic { user: String, password: Secret },
    Identity(Secret),
}

#[derive(Clone)]
pub struct Credentials {
    pub kind: Kind,
    pub from: Source,
}

/// The key a registry's credentials are kept under: its host, and `https://index.docker.io/v1/`
/// for `docker.io`.
pub fn credential_key(registry: &str) -> String {
    if registry == "docker.io" { "https://index.docker.io/v1/".to_string() } else { registry.to_string() }
}

/// A key of `auths` or `credHelpers` without its scheme and its path, as it is compared.
fn bare(key: &str) -> String {
    let k = key.split_once("://").map(|(_, r)| r).unwrap_or(key);
    k.split('/').next().unwrap_or(k).to_ascii_lowercase()
}

/// The Docker config file: `$DOCKER_CONFIG/config.json`, else `~/.docker/config.json`.
pub fn config_file() -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    match var("DOCKER_CONFIG") {
        Some(d) => Some(d.join("config.json")),
        None => var("HOME").map(|h| h.join(".docker/config.json")),
    }
}

/// The credentials of `registry` (a host as a module's name writes it), from the Docker config
/// file: none for a run that goes anonymous.
pub fn credentials(registry: &str) -> Result<Option<Credentials>, Text> {
    let Some(file) = config_file() else { return Ok(None) };
    let Ok(bytes) = std::fs::read(&file) else { return Ok(None) };
    let shown = file.display().to_string();
    let j = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|s| json::parse(s).ok())
        .ok_or_else(|| tr!("Docker の設定ファイル {shown} を JSON として読めません", "the Docker config file {shown} does not read as JSON"))?;
    let key = credential_key(registry);
    let want = bare(&key);
    let entry = |k: &str| -> Option<&Json> { j.get(k)?.as_obj()?.iter().find(|(x, _)| bare(x) == want).map(|(_, v)| v) };
    if let Some(h) = entry("credHelpers").and_then(Json::as_str) {
        return helper(h, &key, Source::Helper(h.to_string()));
    }
    if let Some(a) = entry("auths") {
        if let Some(t) = a.get("identitytoken").and_then(Json::as_str).filter(|t| !t.is_empty()) {
            return Ok(Some(Credentials { kind: Kind::Identity(Secret::new(t)), from: Source::Auths(file.clone()) }));
        }
        if let Some(auth) = a.get("auth").and_then(Json::as_str).filter(|t| !t.is_empty()) {
            let pair = ritsu_base::sources::base64_decode(auth).and_then(|b| String::from_utf8(b).ok());
            let Some((user, password)) = pair.as_deref().and_then(|p| p.split_once(':')) else {
                return Err(tr!(
                    "Docker の設定ファイル {shown} の `auths` の `auth` が、`<ユーザー>:<パスワード>` の base64 ではありません",
                    "the `auth` of `auths` in the Docker config file {shown} is not `<user>:<password>` in base64"
                ));
            };
            return Ok(Some(Credentials { kind: Kind::Basic { user: user.to_string(), password: Secret::new(password) }, from: Source::Auths(file.clone()) }));
        }
    }
    if let Some(h) = j.get("credsStore").and_then(Json::as_str).filter(|h| !h.is_empty()) {
        return helper(h, &key, Source::Store(h.to_string()));
    }
    Ok(None)
}

/// `docker-credential-<name> get`, the key on its standard input: what it answers, or none when it
/// has none (an exit code that is not 0). Neither what it prints nor what it says on its standard
/// error is shown.
fn helper(name: &str, key: &str, from: Source) -> Result<Option<Credentials>, Text> {
    let program = format!("docker-credential-{name}");
    let child = Command::new(&program).arg("get").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let said = from.text();
            return Err(tr!("{} の `{program}` が PATH にありません", "`{program}`, {}, is not on the PATH", said.ja; said.en));
        }
        Err(e) => return Err(ritsu_base::wasi::starting(&program, &e)),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(key.as_bytes());
    }
    let o = child.wait_with_output().map_err(|e| ritsu_base::wasi::starting(&program, &e))?;
    if !o.status.success() {
        return Ok(None);
    }
    let said = from.text();
    let j = std::str::from_utf8(&o.stdout).ok().and_then(|s| json::parse(s).ok());
    let field = |k: &str| j.as_ref().and_then(|j| j.get(k)).and_then(Json::as_str).map(str::to_string);
    let (Some(user), Some(secret)) = (field("Username"), field("Secret")) else {
        return Err(tr!("{} の答えが、`Username` と `Secret` の JSON ではありません", "what {} answered is no JSON with `Username` and `Secret`", said.ja; said.en));
    };
    let kind = if user == "<token>" { Kind::Identity(Secret::new(secret)) } else { Kind::Basic { user, password: Secret::new(secret) } };
    Ok(Some(Credentials { kind, from }))
}

/// A challenge of `WWW-Authenticate`: its scheme and its parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Challenge {
    pub scheme: String,
    pub params: Vec<(String, String)>,
}

impl Challenge {
    pub fn get(&self, k: &str) -> Option<&str> {
        self.params.iter().find(|(x, _)| x.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str())
    }
}

/// `Bearer realm="…",service="…"` read: the scheme, and each `key=value` (a value quoted or not; a
/// quoted one may hold commas).
pub fn challenge(header: &str) -> Option<Challenge> {
    let h = header.trim();
    let (scheme, rest) = h.split_once(' ').unwrap_or((h, ""));
    let mut params = Vec::new();
    let b = rest.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i] == b' ' || b[i] == b',') {
            i += 1;
        }
        let k0 = i;
        while i < b.len() && b[i] != b'=' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let key = rest[k0..i].trim().to_string();
        i += 1;
        let mut value = String::new();
        if i < b.len() && b[i] == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' && i + 1 < b.len() {
                    i += 1;
                }
                let ch = rest[i..].chars().next().unwrap_or(' ');
                value.push(ch);
                i += ch.len_utf8();
            }
            i += 1;
        } else {
            let v0 = i;
            while i < b.len() && b[i] != b',' {
                i += 1;
            }
            value = rest[v0..i].trim().to_string();
        }
        params.push((key, value));
    }
    Some(Challenge { scheme: scheme.to_string(), params })
}

/// A token kept for the run, and when it stops being used.
struct Kept {
    value: Secret,
    until: Instant,
}

/// What a token is kept by: (realm, service, scope) for the tokens, (registry, repository, actions)
/// for what a repository was granted.
type Key = (String, String, String);

/// The tokens kept, each by its key.
type Keeping = Mutex<Vec<(Key, Kept)>>;

/// The tokens of the run, by (realm, service, scope); and what each repository was granted, by
/// (registry, repository, actions).
static TOKENS: Keeping = Mutex::new(Vec::new());
static GRANTED: Keeping = Mutex::new(Vec::new());

fn kept(store: &Keeping, key: &Key) -> Option<Secret> {
    let now = Instant::now();
    let guard = store.lock().unwrap_or_else(|e| e.into_inner());
    guard.iter().find(|(k, v)| k == key && v.until > now).map(|(_, v)| v.value.clone())
}

fn keep(store: &Keeping, key: Key, value: Secret, until: Instant) {
    let mut guard = store.lock().unwrap_or_else(|e| e.into_inner());
    guard.retain(|(k, _)| *k != key);
    guard.push((key, Kept { value, until }));
}

/// The credentials of the run, by registry: looked up once.
static FOUND: Mutex<Option<HashMap<String, Option<Credentials>>>> = Mutex::new(None);

fn found(registry: &str) -> Result<Option<Credentials>, Text> {
    {
        let guard = FOUND.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = guard.as_ref().and_then(|m| m.get(registry)) {
            return Ok(c.clone());
        }
    }
    let c = credentials(registry)?;
    let mut guard = FOUND.lock().unwrap_or_else(|e| e.into_inner());
    guard.get_or_insert_with(HashMap::new).insert(registry.to_string(), c.clone());
    Ok(c)
}

/// The value of `Authorization` for a user and a password.
fn basic(user: &str, password: &Secret) -> Secret {
    Secret::new(format!("Basic {}", ritsu_base::sources::base64_encode(format!("{user}:{}", password.expose()).as_bytes())))
}

/// A part of a URL's query, escaped.
fn escape(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// A token from the realm of `c`, for `scope`, with the credentials of `registry` or none.
fn token(registry: &str, c: &Challenge, scope: &str, tmp: &Path) -> Result<Secret, Text> {
    let realm = c.get("realm").unwrap_or("").to_string();
    let service = c.get("service").unwrap_or("").to_string();
    let key = (realm.clone(), service.clone(), scope.to_string());
    if let Some(t) = kept(&TOKENS, &key) {
        return Ok(t);
    }
    let creds = found(registry)?;
    let sep = if realm.contains('?') { '&' } else { '?' };
    let mut query = Vec::new();
    if !service.is_empty() {
        query.push(format!("service={}", escape(&service)));
    }
    query.push(format!("scope={}", escape(scope)));
    let r: Response = match creds.as_ref().map(|c| &c.kind) {
        Some(Kind::Identity(id)) => {
            let form = Secret::new(format!(
                "grant_type=refresh_token&service={}&scope={}&client_id=ritsu&refresh_token={}",
                escape(&service),
                escape(scope),
                escape(id.expose())
            ));
            let mut req = Request::new("POST", realm.clone(), 1024 * 1024).header("Content-Type", "application/x-www-form-urlencoded");
            req.body = Body::SecretForm(&form);
            req.secret_answer = true;
            let r = http::send(&req, tmp)?;
            if r.status == 404 {
                let from = creds.as_ref().map(|c| c.from.text()).unwrap_or_default();
                return Err(tr!(
                    "レジストリ {registry} は、ID トークン（{}）を受け付けません（`POST {realm}` が 404 でした）",
                    "the registry {registry} does not take an identity token ({}): `POST {realm}` answered 404",
                    from.ja; from.en
                ));
            }
            r
        }
        Some(Kind::Basic { user, password }) => {
            let auth = basic(user, password);
            let mut req = Request::new("GET", format!("{realm}{sep}{}", query.join("&")), 1024 * 1024);
            req.auth = Some(auth);
            req.secret_answer = true;
            http::send(&req, tmp)?
        }
        None => {
            let mut req = Request::new("GET", format!("{realm}{sep}{}", query.join("&")), 1024 * 1024);
            req.secret_answer = true;
            http::send(&req, tmp)?
        }
    };
    if !r.ok() {
        return Err(refused(registry, creds.as_ref(), &r.error(if matches!(creds.as_ref().map(|c| &c.kind), Some(Kind::Identity(_))) { "POST" } else { "GET" })));
    }
    let j = std::str::from_utf8(&r.body).ok().and_then(|s| json::parse(s).ok());
    let field = |k: &str| j.as_ref().and_then(|j| j.get(k)).and_then(Json::as_str).filter(|s| !s.is_empty()).map(str::to_string);
    let Some(t) = field("token").or_else(|| field("access_token")) else {
        return Err(tr!("レジストリ {registry} のトークンの答え（{realm}）に、`token` も `access_token` もありません", "the token the registry {registry} answered ({realm}) has neither `token` nor `access_token`"));
    };
    let secs = j.as_ref().and_then(|j| j.get("expires_in")).and_then(Json::as_int).filter(|n| *n > 0).unwrap_or(60);
    let value = Secret::new(format!("Bearer {t}"));
    keep(&TOKENS, key, value.clone(), Instant::now() + Duration::from_secs(secs as u64));
    Ok(value)
}

/// What a 401 after the credentials says: the registry, and where they came from.
fn refused(registry: &str, creds: Option<&Credentials>, what: &Text) -> Text {
    match creds {
        Some(c) => {
            let from = c.from.text();
            tr!(
                "レジストリ {registry} が、{} から取った資格情報を受け付けません（{}）",
                "the registry {registry} does not take the credentials from {} ({})",
                from.ja, what.ja; from.en, what.en
            )
        }
        None => tr!(
            "レジストリ {registry} は、資格情報の無い問い合わせを受け付けません（{}）。`docker login` か、Docker の設定ファイルの資格情報のヘルパーで、資格情報を置いてください",
            "the registry {registry} does not take a request with no credentials ({}); put credentials in the Docker config file with `docker login` or a credential helper",
            what.ja; what.en
        ),
    }
}

/// A request to `registry`'s repository `repository`, which asks for `actions` (`pull`, or
/// `pull,push`), sent with the credentials the registry asks for: the answer, whatever its status
/// but 401, or why there is none (an answer over the request's limit told apart, as
/// [`http::send`] tells it).
pub fn send(registry: &str, repository: &str, actions: &str, mut req: Request<'_>, tmp: &Path) -> Result<Response, NoAnswer> {
    let granted_key = (registry.to_string(), repository.to_string(), actions.to_string());
    let earlier = kept(&GRANTED, &granted_key);
    req.auth = earlier;
    let r = http::send(&req, tmp)?;
    if r.status != 401 {
        return Ok(r);
    }
    let Some(c) = r.header("WWW-Authenticate").and_then(challenge) else {
        return Err(refused(registry, None, &r.error(req.method)).into());
    };
    let (value, until) = if c.scheme.eq_ignore_ascii_case("bearer") {
        let scope = c.get("scope").map(str::to_string).unwrap_or_else(|| format!("repository:{repository}:{actions}"));
        let t = token(registry, &c, &scope, tmp)?;
        (t, Instant::now() + Duration::from_secs(50))
    } else if c.scheme.eq_ignore_ascii_case("basic") {
        match found(registry)? {
            Some(Credentials { kind: Kind::Basic { user, password }, .. }) => (basic(&user, &password), Instant::now() + Duration::from_secs(3600)),
            Some(Credentials { kind: Kind::Identity(_), from }) => {
                let from = from.text();
                return Err(tr!(
                    "レジストリ {registry} は Basic の認証を求めていますが、{} の資格情報は ID トークンです",
                    "the registry {registry} asks for Basic credentials, and those of {} are an identity token",
                    from.ja; from.en
                )
                .into());
            }
            None => return Err(refused(registry, None, &r.error(req.method)).into()),
        }
    } else {
        let s = c.scheme.clone();
        return Err(tr!("レジストリ {registry} が求める認証 `{s}` は、ritsu が知らない形です", "the registry {registry} asks for `{s}`, which ritsu does not know").into());
    };
    req.auth = Some(value.clone());
    let again = http::send(&req, tmp)?;
    if again.status == 401 {
        let creds = found(registry)?;
        return Err(refused(registry, creds.as_ref(), &again.error(req.method)).into());
    }
    keep(&GRANTED, granted_key, value.clone(), until);
    Ok(again)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_challenge_is_read() {
        let c = challenge("Bearer realm=\"https://ghcr.io/token\",service=\"ghcr.io\",scope=\"repository:acme/pricing:pull,push\"").unwrap();
        assert_eq!(c.scheme, "Bearer");
        assert_eq!(c.get("realm"), Some("https://ghcr.io/token"));
        assert_eq!(c.get("scope"), Some("repository:acme/pricing:pull,push"));
        let c = challenge("Basic realm=\"registry\"").unwrap();
        assert_eq!((c.scheme.as_str(), c.get("realm")), ("Basic", Some("registry")));
        let c = challenge("Bearer realm=x, service=y").unwrap();
        assert_eq!((c.get("realm"), c.get("service")), (Some("x"), Some("y")));
    }

    #[test]
    fn the_keys_of_the_config_file_are_compared_bare() {
        assert_eq!(bare("https://index.docker.io/v1/"), "index.docker.io");
        assert_eq!(bare("ghcr.io"), "ghcr.io");
        assert_eq!(bare("http://127.0.0.1:5000"), "127.0.0.1:5000");
        assert_eq!(credential_key("docker.io"), "https://index.docker.io/v1/");
        assert_eq!(escape("repository:acme/pricing:pull"), "repository%3Aacme%2Fpricing%3Apull");
    }
}
