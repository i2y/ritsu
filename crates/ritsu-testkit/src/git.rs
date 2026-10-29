//! A git server on a port of `127.0.0.1`, for the dependencies from git repositories (DESIGN
//! 17.12.1), and the repositories it serves.
//!
//! The server reads HTTP itself and runs `git http-backend` as a CGI program for each request
//! (`GIT_PROJECT_ROOT`, `GIT_HTTP_EXPORT_ALL`, `PATH_INFO`, `QUERY_STRING`, `REQUEST_METHOD`,
//! `CONTENT_TYPE`, `CONTENT_LENGTH`; the `Git-Protocol` and `Content-Encoding` headers as
//! `HTTP_GIT_PROTOCOL` and `HTTP_CONTENT_ENCODING`; a body in chunks is read whole first). What a
//! test chooses ([`Options`]): whether the server takes a fetch without the files
//! (`uploadpack.allowFilter`), whether it speaks protocol version 0 only (the `Git-Protocol` header
//! is not passed on), and a user and a password for `Basic`. It keeps each request it answered
//! ([`Seen`]: the method, the target, the `Authorization` header, the status, and the sizes of the
//! body asked with and the body answered). It stops when its value is dropped.
//!
//! A [`Repo`] is a repository the server serves at `<root>/served/<path>.git`, with its working
//! tree at `<root>/work/<path>`: a test writes files, commits and tags there. Its git reads neither
//! the system's configuration nor the person's, and its commits and tags have a fixed name and a
//! date that counts up from one fixed time, so that a commit is the same commit on every machine
//! (golden files hold locks, which name commits). Several servers may serve one root, each with its
//! own options.

use crate::registry::{base64_encode, read_request, reason};
use std::cell::Cell;
use std::io::{BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// The variables that point git at a repository (`git rev-parse --local-env-vars`), taken away
/// from every git this module runs, and `GIT_PROTOCOL`, which the server sets from the request.
const TAKEN: [&str; 16] = [
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
    "GIT_PROTOCOL",
];

/// What a server is.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Whether it takes a fetch without the files (`--filter=blob:none`).
    pub filter: bool,
    /// Whether it speaks protocol version 0 only.
    pub v0: bool,
    /// The user and the password it asks for (`Basic`), if any.
    pub basic: Option<(String, String)>,
}

impl Options {
    /// What the large hosts are: protocol version 2, and a fetch without the files taken.
    pub fn hosted() -> Options {
        Options { filter: true, v0: false, basic: None }
    }
}

/// A request the server answered.
#[derive(Clone, Debug)]
pub struct Seen {
    pub method: String,
    /// The path and the query.
    pub target: String,
    pub authorization: Option<String>,
    /// The `Git-Protocol` header.
    pub protocol: Option<String>,
    pub status: u16,
    /// The size of the body asked with.
    pub asked: usize,
    /// The size of the body answered.
    pub answered: usize,
}

/// The server, until it is dropped.
pub struct GitServer {
    /// `http://127.0.0.1:<port>`.
    pub url: String,
    pub port: u16,
    root: PathBuf,
    log: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl GitServer {
    /// A server of the repositories under `root` (`<root>/served`), made if it is not there.
    pub fn start(root: &Path, options: Options) -> GitServer {
        let served = root.join("served");
        std::fs::create_dir_all(&served).unwrap_or_else(|e| panic!("cannot make {}: {e}", served.display()));
        let l = TcpListener::bind("127.0.0.1:0").expect("a port on 127.0.0.1");
        let port = l.local_addr().unwrap().port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (log2, stop2) = (Arc::clone(&log), Arc::clone(&stop));
        let thread = std::thread::spawn(move || {
            for conn in l.incoming() {
                if stop2.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(conn) = conn else { continue };
                let (log, stop, served, options) = (Arc::clone(&log2), Arc::clone(&stop2), served.clone(), options.clone());
                std::thread::spawn(move || serve_connection(conn, &served, &options, &log, &stop));
            }
        });
        GitServer { url: format!("http://127.0.0.1:{port}"), port, root: root.to_path_buf(), log, stop, thread: Some(thread) }
    }

    /// A new repository the server serves at `<url>/<path>.git` (and `<url>/<path>`), its first
    /// branch `main`; SHA-1.
    pub fn repo(&self, path: &str) -> Repo {
        Repo::init(&self.root, path, false)
    }

    /// A new repository whose objects are named by SHA-256.
    pub fn repo_sha256(&self, path: &str) -> Repo {
        Repo::init(&self.root, path, true)
    }

    /// The URL of the repository at `path`.
    pub fn url_of(&self, path: &str) -> String {
        format!("{}/{path}", self.url)
    }

    /// The requests answered, in order.
    pub fn log(&self) -> Vec<Seen> {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn clear_log(&self) {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// The bytes of the bodies answered, all told.
    pub fn answered(&self) -> usize {
        self.log().iter().map(|s| s.answered).sum()
    }
}

impl Drop for GitServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The requests of one connection, until it closes or asks to.
fn serve_connection(conn: TcpStream, served: &Path, options: &Options, log: &Arc<Mutex<Vec<Seen>>>, stop: &AtomicBool) {
    let Ok(mut writer) = conn.try_clone() else { return };
    let mut reader = BufReader::new(conn);
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let Some(req) = read_request(&mut reader, &mut writer) else { return };
        let close = req.header("Connection").is_some_and(|v| v.eq_ignore_ascii_case("close"));
        let authorization = req.header("Authorization").map(str::to_string);
        let (status, headers, body) = match &options.basic {
            Some((user, password)) if authorization.as_deref() != Some(format!("Basic {}", base64_encode(format!("{user}:{password}").as_bytes())).as_str()) => {
                (401, vec![("WWW-Authenticate".to_string(), "Basic realm=\"git\"".to_string())], b"authentication required\n".to_vec())
            }
            _ => cgi(&req, served, options),
        };
        log.lock().unwrap_or_else(|e| e.into_inner()).push(Seen {
            method: req.method.clone(),
            target: req.target.clone(),
            authorization,
            protocol: req.header("Git-Protocol").map(str::to_string),
            status,
            asked: req.body.len(),
            answered: body.len(),
        });
        let mut out = format!("HTTP/1.1 {status} {}\r\n", reason(status));
        for (k, v) in &headers {
            out.push_str(&format!("{k}: {v}\r\n"));
        }
        out.push_str(&format!("Content-Length: {}\r\n", body.len()));
        if close {
            out.push_str("Connection: close\r\n");
        }
        out.push_str("\r\n");
        if writer.write_all(out.as_bytes()).is_err() || (req.method != "HEAD" && writer.write_all(&body).is_err()) {
            return;
        }
        let _ = writer.flush();
        if close {
            return;
        }
    }
}

/// `git http-backend`, run for one request: the status, the headers and the body it gave.
fn cgi(req: &crate::registry::Request, served: &Path, options: &Options) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let (path, query) = req.target.split_once('?').unwrap_or((req.target.as_str(), ""));
    let mut c = Command::new("git");
    c.arg("http-backend");
    for v in TAKEN {
        c.env_remove(v);
    }
    c.env("GIT_CONFIG_NOSYSTEM", "1").env("GIT_CONFIG_GLOBAL", "/dev/null");
    c.env("GIT_PROJECT_ROOT", served).env("GIT_HTTP_EXPORT_ALL", "1");
    c.env("REQUEST_METHOD", &req.method).env("PATH_INFO", path).env("QUERY_STRING", query).env("REMOTE_ADDR", "127.0.0.1");
    c.env("CONTENT_LENGTH", req.body.len().to_string());
    match req.header("Content-Type") {
        Some(t) => c.env("CONTENT_TYPE", t),
        None => c.env_remove("CONTENT_TYPE"),
    };
    c.env_remove("HTTP_GIT_PROTOCOL").env_remove("HTTP_CONTENT_ENCODING");
    if !options.v0
        && let Some(p) = req.header("Git-Protocol")
    {
        c.env("HTTP_GIT_PROTOCOL", p);
    }
    if let Some(e) = req.header("Content-Encoding") {
        c.env("HTTP_CONTENT_ENCODING", e);
    }
    if let Some((user, _)) = &options.basic {
        c.env("REMOTE_USER", user);
    }
    c.env("GIT_CONFIG_COUNT", "1").env("GIT_CONFIG_KEY_0", "uploadpack.allowFilter").env("GIT_CONFIG_VALUE_0", if options.filter { "true" } else { "false" });
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = c.spawn() else { return (500, Vec::new(), b"cannot start git http-backend\n".to_vec()) };
    let mut stdin = child.stdin.take();
    let body = req.body.clone();
    let writer = std::thread::spawn(move || {
        if let Some(s) = stdin.as_mut() {
            let _ = s.write_all(&body);
        }
    });
    let mut out = Vec::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_end(&mut out);
    }
    let _ = writer.join();
    let _ = child.wait();
    // the CGI headers, to the blank line
    let end = out.windows(4).position(|w| w == b"\r\n\r\n").map(|i| (i, 4)).or_else(|| out.windows(2).position(|w| w == b"\n\n").map(|i| (i, 2)));
    let Some((at, len)) = end else { return (500, Vec::new(), out) };
    let head = String::from_utf8_lossy(&out[..at]).to_string();
    let mut status = 200;
    let mut headers = Vec::new();
    for line in head.lines() {
        let Some((k, v)) = line.split_once(':') else { continue };
        let (k, v) = (k.trim(), v.trim());
        if k.eq_ignore_ascii_case("Status") {
            status = v.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(500);
        } else if !k.eq_ignore_ascii_case("Content-Length") {
            headers.push((k.to_string(), v.to_string()));
        }
    }
    (status, headers, out[at + len..].to_vec())
}

/// The fixed time the commits of every repository count up from (2026-09-15, 00:00 UTC).
const EPOCH: u64 = 1_789_430_400;

/// A repository a [`GitServer`] serves, and its working tree.
pub struct Repo {
    /// Its working tree.
    pub work: PathBuf,
    /// Its git directory, which the server serves.
    pub served: PathBuf,
    /// Where it is under the server's root: `acme/rates`.
    pub path: String,
    ticks: Cell<u64>,
}

impl Repo {
    fn init(root: &Path, path: &str, sha256: bool) -> Repo {
        let work = root.join("work").join(path);
        let served = root.join("served").join(format!("{path}.git"));
        std::fs::create_dir_all(&work).unwrap_or_else(|e| panic!("cannot make {}: {e}", work.display()));
        if let Some(p) = served.parent() {
            std::fs::create_dir_all(p).unwrap_or_else(|e| panic!("cannot make {}: {e}", p.display()));
        }
        let r = Repo { work: work.clone(), served: served.clone(), path: path.to_string(), ticks: Cell::new(0) };
        let mut args = vec!["init".to_string(), "--quiet".to_string(), "--initial-branch=main".to_string(), format!("--separate-git-dir={}", served.display())];
        if sha256 {
            args.push("--object-format=sha256".to_string());
        }
        args.push(work.display().to_string());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        r.git(&args);
        r
    }

    /// git in the working tree, which has to pass: what it printed. Neither the system's
    /// configuration nor the person's is read; the name and the date of a commit or a tag are fixed,
    /// the date one minute after the last one's.
    pub fn git(&self, args: &[&str]) -> String {
        let t = self.ticks.get() + 1;
        self.ticks.set(t);
        let date = format!("{} +0000", EPOCH + 60 * t);
        let mut c = Command::new("git");
        for v in TAKEN {
            c.env_remove(v);
        }
        c.current_dir(&self.work)
            .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false", "-c", "core.autocrlf=false", "-c", "core.hooksPath=/dev/null", "-c", "advice.detachedHead=false"])
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "ritsu-test")
            .env("GIT_AUTHOR_EMAIL", "ritsu-test@example.com")
            .env("GIT_COMMITTER_NAME", "ritsu-test")
            .env("GIT_COMMITTER_EMAIL", "ritsu-test@example.com")
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date)
            .stdin(Stdio::null());
        let o = c.output().expect("could not run git");
        assert!(o.status.success(), "git {args:?} in {}:\n{}{}", self.work.display(), String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).into_owned()
    }

    /// A file of the working tree written (its directories made), and added.
    pub fn write(&self, rel: &str, bytes: impl AsRef<[u8]>) -> &Repo {
        let p = self.work.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, bytes).unwrap_or_else(|e| panic!("cannot write {}: {e}", p.display()));
        self.git(&["add", "--", rel]);
        self
    }

    /// A symbolic link in the working tree, to `target`, added.
    pub fn symlink(&self, rel: &str, target: &str) -> &Repo {
        let p = self.work.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::os::unix::fs::symlink(target, &p).unwrap_or_else(|e| panic!("cannot make {}: {e}", p.display()));
        self.git(&["add", "--", rel]);
        self
    }

    /// A submodule at `rel` that names the commit `commit` (the index's entry alone).
    pub fn submodule(&self, rel: &str, commit: &str) -> &Repo {
        self.git(&["update-index", "--add", "--cacheinfo", &format!("160000,{commit},{rel}")]);
        self
    }

    /// A file of the working tree removed.
    pub fn remove(&self, rel: &str) -> &Repo {
        self.git(&["rm", "--quiet", "--", rel]);
        self
    }

    /// What was added, committed: the commit.
    pub fn commit(&self, message: &str) -> String {
        self.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
        self.head()
    }

    /// The commit `main` is at.
    pub fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_string()
    }

    /// A tag at the last commit, without a message (a lightweight tag).
    pub fn tag(&self, name: &str) -> &Repo {
        self.git(&["tag", name]);
        self
    }

    /// A tag at the last commit, with a message (an annotated tag).
    pub fn annotated(&self, name: &str) -> &Repo {
        self.git(&["tag", "-a", "-m", &format!("release {name}"), name]);
        self
    }

    /// A tag moved to the last commit.
    pub fn retag(&self, name: &str) -> &Repo {
        self.git(&["tag", "-f", name]);
        self
    }

    /// A tag removed.
    pub fn untag(&self, name: &str) -> &Repo {
        self.git(&["tag", "-d", name]);
        self
    }
}
