//! The credentials of a registry (DESIGN 17.13, 17.25; PLAN D.3), against a registry the test starts
//! that asks for them: Basic; the token's steps with Basic credentials, with an identity token, and
//! anonymous (a token to pull, none to push); the Docker config's `auths` (`auth` and
//! `identitytoken`), `credHelpers` and `credsStore`, each helper a script of the test's own that
//! holds the registry's host to what it is given; the blobs sent to another port, where no
//! `Authorization` goes; credentials refused, and a helper that is not there (exit 2, where the
//! credentials came from named, never their value).
//!
//! Every secret the test plants is a fake one that says so (`ritsu-fake-…`): the password, the
//! identity token, the tokens the registry gives, and the base64 of the user and the password. Each
//! is held to be nowhere: not on the standard output or error, not in the JSON, not in the store or
//! its tmp/, not in the arguments curl was given (a wrapper of the test's own writes them down), and
//! not in the test's directory but for the config file and the helper that hold them.

use ritsu_base::modules::store;
use ritsu_testkit::TempDir;
use ritsu_testkit::registry::{self as reg, Auth, Registry, TokenAuth};
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const USER: &str = "ritsu-fake-user";
const PASSWORD: &str = "ritsu-fake-password-7f3a9c";
const IDENTITY: &str = "ritsu-fake-identity-token-91c2e4";

const LANG_VARS: [&str; 10] = ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG", "KANAME_LANG"];

/// A credential helper of the test's own: it writes down the key it is given on its standard input,
/// answers the planted credentials for the registry's host alone (`HELPER_GIVES` says which:
/// `basic`, or `token` for an identity token), and has none for any other key (exit 1).
fn helper() -> String {
    format!(
        "#!/bin/sh\n[ \"$1\" = get ] || exit 1\nread -r key\nprintf '%s\\n' \"$key\" >> \"$HELPER_LOG\"\nif [ \"$key\" != \"$WANT_HOST\" ]; then\n  echo 'credentials not found in native keychain'\n  exit 1\nfi\nif [ \"$HELPER_GIVES\" = token ]; then\n  printf '{{\"ServerURL\":\"%s\",\"Username\":\"<token>\",\"Secret\":\"%s\"}}\\n' \"$key\" '{IDENTITY}'\nelse\n  printf '{{\"ServerURL\":\"%s\",\"Username\":\"%s\",\"Secret\":\"%s\"}}\\n' \"$key\" '{USER}' '{PASSWORD}'\nfi\n"
    )
}

fn executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A test's directory and its registry: the module `tax` named on the registry, the module `user`
/// that requires it, an empty Docker config, the helper, and a curl that writes down its arguments
/// before it runs the real one.
struct World {
    t: TempDir,
    r: Registry,
}

impl World {
    fn new(what: &str, auth: Auth) -> World {
        let t = TempDir::new(what);
        let r = Registry::with_auth(auth);
        let host = r.host.clone();
        t.write("tax/ritsu.mod", format!("module {host}/acme/tax\nversion 1.0.0\nritsu >=0.36.0\n"));
        ritsu_testkit::tmp::copy_dir(&here().join("tests/projects/modules/tax/rules"), &t.path().join("tax/rules"));
        t.write("user/ritsu.mod", format!("module example.com/acme/user\nversion 0.1.0\nrequire tax oci \"{host}/acme/tax\" 1.0.0\n"));
        std::fs::create_dir_all(t.path().join("docker")).unwrap();
        std::fs::create_dir_all(t.path().join("os-tmp")).unwrap();
        let curl =std::env::split_paths(&std::env::var_os("PATH").unwrap()).map(|d| d.join("curl")).find(|p| p.is_file()).expect("curl is on the PATH");
        executable(&t.write("bin/curl", format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CURL_ARGS_LOG\"\nexec '{}' \"$@\"\n", curl.display())));
        executable(&t.write("bin/docker-credential-ritsu-fake", helper()));
        World { t, r }
    }

    fn host(&self) -> String {
        self.r.host.clone()
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.t.path().join(rel)
    }

    /// The Docker config file.
    fn config(&self, json: &str) {
        self.t.write("docker/config.json", json);
    }

    fn run_env(&self, dir: &str, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(self.path(dir)).args(args);
        c.env(store::VARIABLE, self.path("store")).env("DOCKER_CONFIG", self.path("docker")).env("TMPDIR", self.path("os-tmp"));
        c.env("PATH",format!("{}:{}", self.path("bin").display(), std::env::var("PATH").unwrap_or_default()));
        c.env("CURL_ARGS_LOG", self.path("curl-args.log")).env("HELPER_LOG", self.path("helper.log")).env("WANT_HOST", self.host()).env("HELPER_GIVES", "basic");
        for v in LANG_VARS {
            c.env_remove(v);
        }
        for (k, v) in env {
            c.env(k, v);
        }
        let o = c.output().expect("could not run ritsu");
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
    }

    fn run(&self, dir: &str, args: &[&str]) -> (i32, String, String) {
        self.run_env(dir, args, &[])
    }

    /// The keys the helper was given, in order.
    fn helper_given(&self) -> Vec<String> {
        std::fs::read_to_string(self.path("helper.log")).unwrap_or_default().lines().map(str::to_string).collect()
    }

    /// The planted secrets, the tokens the registry gave and the base64 of the user and the password:
    /// nowhere in `said` (what ritsu printed), the store (its tmp/ too), the arguments curl was
    /// given, or any file of the test's directory but the config file and the helper; and the store's
    /// tmp/ and the temporary directory the runs were given (`TMPDIR`) left empty.
    fn nowhere(&self, said: &[String]) {
        let mut secrets: Vec<String> = vec![PASSWORD.to_string(), IDENTITY.to_string(), reg::base64_encode(format!("{USER}:{PASSWORD}").as_bytes())];
        secrets.extend(self.r.tokens());
        for s in &secrets {
            for t in said {
                assert!(!t.contains(s.as_str()), "the secret {s} is in what ritsu said:\n{t}");
            }
        }
        let args = std::fs::read_to_string(self.path("curl-args.log")).unwrap_or_default();
        assert!(args.lines().count() > 0 && args.lines().all(|l| l.starts_with("-q --silent --show-error --config - ")), "{args}");
        assert!(!args.contains("Authorization"), "{args}");
        let except = [self.path("docker/config.json"), self.path("bin/docker-credential-ritsu-fake")];
        let mut files = Vec::new();
        walk(self.t.path(), &mut files);
        for f in files.iter().filter(|f| !except.contains(f)) {
            let bytes = std::fs::read(f).unwrap_or_default();
            let text = String::from_utf8_lossy(&bytes);
            for s in &secrets {
                assert!(!text.contains(s.as_str()), "the secret {s} is in {}", f.display());
            }
        }
        for d in ["store/tmp", "os-tmp"] {
            let left: Vec<_> = std::fs::read_dir(self.path(d)).map(|r| r.map(|e| e.unwrap().path()).collect()).unwrap_or_default();
            assert_eq!(left, Vec::<PathBuf>::new(), "{d}");
        }
    }
}

/// Every file under `dir`.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.map(|e| e.unwrap()) {
        let t = e.file_type().unwrap();
        if t.is_dir() {
            walk(&e.path(), out);
        } else if t.is_file() {
            out.push(e.path());
        }
    }
}

fn auths_basic(host: &str, user: &str, password: &str) -> String {
    format!("{{\"auths\":{{\"{host}\":{{\"auth\":\"{}\"}}}}}}", reg::base64_encode(format!("{user}:{password}").as_bytes()))
}

/// ritsu's run, which has to pass: its standard output and error together.
fn ok(r: (i32, String, String)) -> String {
    assert_eq!(r.0, 0, "{}{}", r.1, r.2);
    format!("{}{}", r.1, r.2)
}

/// A registry that asks for Basic, and the credentials in `auths` (the key with a scheme and a path,
/// compared bare): the first request of a run goes with none, and after the 401 each goes with
/// Basic; a push, a lock and a fetch pass, and no secret is anywhere.
#[test]
fn basic_credentials_from_auths() {
    let w = World::new("auth-basic", Auth::Basic { user: USER.into(), password: PASSWORD.into() });
    let host = w.host();
    w.config(&auths_basic(&format!("http://{host}/v2/"), USER, PASSWORD));
    let mut said = vec![ok(w.run("tax", &["mod", "push"]))];
    said.push(ok(w.run("user", &["mod", "lock"])));
    said.push(ok(w.run("user", &["mod", "fetch", "--format", "json"])));
    let log = w.r.log();
    assert_eq!((log[0].authorization.as_deref(), log[0].status), (None, 401));
    assert!(log.iter().filter(|s| s.status != 401).all(|s| s.authorization.as_deref().is_some_and(|a| a.starts_with("Basic "))), "{log:?}");
    assert_eq!(w.r.tags("acme/tax"), ["1.0.0"]);
    w.nowhere(&said);
}

/// The token's steps with Basic credentials: the realm is asked with Basic, the token goes with
/// each request after, and the blobs a fetch is sent to on another port get no `Authorization`.
#[test]
fn a_token_for_basic_credentials() {
    let w = World::new("auth-token", Auth::Token(TokenAuth { credentials: Some((USER.into(), PASSWORD.into())), refresh_token: None, anonymous: false }));
    let host = w.host();
    w.config(&auths_basic(&host, USER, PASSWORD));
    w.r.redirect_blobs();
    let mut said = vec![ok(w.run("tax", &["mod", "push", "--format", "json"]))];
    said.push(ok(w.run("user", &["mod", "lock"])));
    said.push(ok(w.run("user", &["mod", "fetch"])));
    assert!(!w.r.tokens().is_empty());
    let log = w.r.log();
    let realm: Vec<_> = log.iter().filter(|s| s.target.starts_with("/token")).collect();
    assert!(!realm.is_empty() && realm.iter().all(|s| s.method == "GET" && s.authorization.as_deref().is_some_and(|a| a.starts_with("Basic "))), "{realm:?}");
    let elsewhere: Vec<_> = log.iter().filter(|s| s.port == w.r.second).collect();
    assert!(!elsewhere.is_empty() && elsewhere.iter().all(|s| s.authorization.is_none() && s.status == 200), "{elsewhere:?}");
    assert!(log.iter().filter(|s| s.port != w.r.second && s.target.starts_with("/v2/acme/") && s.status != 401).all(|s| s.authorization.as_deref().is_some_and(|a| a.starts_with("Bearer "))), "{log:?}");
    w.nowhere(&said);
}

/// An identity token (`auths`' `identitytoken`, and a helper's `<token>`): the realm is sent a
/// `POST` with the token as a refresh token, in a form curl reads from its standard input.
#[test]
fn an_identity_token_is_a_refresh_token() {
    let w = World::new("auth-identity", Auth::Token(TokenAuth { credentials: None, refresh_token: Some(IDENTITY.into()), anonymous: false }));
    let host = w.host();
    w.config(&format!("{{\"auths\":{{\"{host}\":{{\"auth\":\"\",\"identitytoken\":\"{IDENTITY}\"}}}}}}"));
    let mut said = vec![ok(w.run("tax", &["mod", "push"]))];
    said.push(ok(w.run("user", &["mod", "lock"])));
    let realm: Vec<String> = w.r.log().iter().filter(|s| s.target.starts_with("/token")).map(|s| format!("{} {}", s.method, s.status)).collect();
    assert!(!realm.is_empty() && realm.iter().all(|s| s == "POST 200"), "{realm:?}");
    // the same token from a helper
    w.config(&format!("{{\"credHelpers\":{{\"{host}\":\"ritsu-fake\"}}}}"));
    said.push(ok(w.run_env("user", &["mod", "fetch"], &[("HELPER_GIVES", "token")])));
    assert_eq!(w.helper_given(), [host]);
    w.nowhere(&said);
}

/// `credHelpers`, then `credsStore`: the helper is given the registry's host and nothing else, and
/// its credentials let the push and the fetch in. A helper that has none for the host leaves the run
/// anonymous, which this registry refuses (exit 2); a helper that is not on the PATH stops the run
/// (exit 2). Neither says a secret.
#[test]
fn credential_helpers_are_given_the_host() {
    let w = World::new("auth-helpers", Auth::Token(TokenAuth { credentials: Some((USER.into(), PASSWORD.into())), refresh_token: None, anonymous: false }));
    let host = w.host();
    w.config(&format!("{{\"credHelpers\":{{\"{host}\":\"ritsu-fake\",\"ghcr.io\":\"elsewhere\"}}}}"));
    let mut said = vec![ok(w.run("tax", &["mod", "push"]))];
    assert_eq!(w.helper_given(), std::slice::from_ref(&host));
    w.config("{\"credsStore\":\"ritsu-fake\"}");
    said.push(ok(w.run("user", &["mod", "lock"])));
    said.push(ok(w.run("user", &["mod", "fetch"])));
    assert_eq!(w.helper_given(), [host.clone(), host.clone(), host.clone()]);
    // the helper has nothing for this host
    let (code, out, err) = w.run_env("tax", &["mod", "push"], &[("WANT_HOST", "ghcr.io")]);
    assert_eq!(code, 2, "{out}{err}");
    let (code, out, err) = w.run_env("user", &["mod", "versions", "tax"], &[("WANT_HOST", "ghcr.io")]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(
        err,
        format!(
            "error: the registry {host} does not take a request with no credentials (the registry answered `GET http://{host}/token` with 401 (UNAUTHORIZED: credentials required)); put credentials in the Docker config file with `docker login` or a credential helper\n"
        )
    );
    said.push(err);
    // a helper that is not there
    w.config(&format!("{{\"credHelpers\":{{\"{host}\":\"ritsu-missing\"}}}}"));
    let (code, out, err) = w.run("user", &["mod", "versions", "tax"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(err, "error: `docker-credential-ritsu-missing`, the helper `docker-credential-ritsu-missing` of `credHelpers` in the Docker config file, is not on the PATH\n");
    w.nowhere(&said);
}

/// A registry that lets anyone pull and only its user push: with no credentials, a fetch passes on an
/// anonymous token and a push stops (exit 2, `docker login` named); with a wrong password, the push
/// stops, naming where the credentials came from (`auths` of the config file) and not the password.
#[test]
fn anonymous_pulls_and_credentials_refused() {
    let w = World::new("auth-anonymous", Auth::Token(TokenAuth { credentials: Some((USER.into(), PASSWORD.into())), refresh_token: None, anonymous: true }));
    let host = w.host();
    let (code, out, err) = w.run("tax", &["mod", "push"]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert!(err.contains(&format!("the registry {host} does not take a request with no credentials")) && err.contains("`docker login`"), "{err}");
    let mut said = vec![err];
    w.config(&auths_basic(&host, USER, PASSWORD));
    said.push(ok(w.run("tax", &["mod", "push"])));
    std::fs::remove_file(w.path("docker/config.json")).unwrap();
    said.push(ok(w.run("user", &["mod", "lock"])));
    said.push(ok(w.run("user", &["mod", "fetch"])));
    // a wrong password
    w.config(&auths_basic(&host, USER, "ritsu-fake-wrong-password"));
    let m = w.t.read("tax/ritsu.mod").replace("version 1.0.0", "version 1.0.1");
    w.t.write("tax/ritsu.mod", m);
    let (code, out, err) = w.run("tax", &["mod", "push", "--format", "json"]);
    assert_eq!(code, 2, "{err}");
    let file = w.path("docker/config.json");
    let e = serde_json::from_str::<serde_json::Value>(&out).unwrap()["error"].as_str().unwrap().to_string();
    assert_eq!(
        e,
        format!(
            "the registry {host} does not take the credentials from `auths` in the Docker config file {} (the registry answered `GET http://{host}/token` with 401 (UNAUTHORIZED: credentials refused))",
            file.display()
        )
    );
    let wrong = reg::base64_encode(format!("{USER}:ritsu-fake-wrong-password").as_bytes());
    for s in ["ritsu-fake-wrong-password", wrong.as_str()] {
        assert!(!out.contains(s) && !err.contains(s), "{out}{err}");
    }
    assert_eq!(w.r.tags("acme/tax"), ["1.0.0"]);
    said.push(out);
    said.push(err);
    // a config file that is no JSON stops a run that needs credentials, and says nothing of it
    w.config("{\"auths\": ritsu-fake-not-json");
    let (code, _, err) = w.run("tax", &["mod", "push"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("does not read as JSON") && !err.contains("ritsu-fake-not-json"), "{err}");
    w.config("{}");
    w.nowhere(&said);
}
