//! The modules of a registry (DESIGN 17.11–17.15, 17.25; PLAN D.4–D.6), against a registry the test
//! starts (`ritsu_testkit::registry`: curl asks it on the loopback, and nothing else): `push` (the
//! steps that refuse, the dry run's digest, a version that does not move), `lock`, `fetch`, `add`,
//! `get` and `versions`; each broken answer of a registry, which stops the fetch with nothing in the
//! store; two fetches at once; the files of an entry changed in the store (E305) and in the module's
//! copy; `fetch --rev` and `fetch <name>@<version>`; the signatures, with a cosign of the test's own
//! that writes down how it is called, and with the real one (the tools level, offline); a key
//! rotated between two revisions, which `fetch --rev`, `fetch` and `report` each check with the key
//! of their own revision; and a registry of the platforms level (`RITSU_TEST_REGISTRY`).
//!
//! The modules are the example's (DESIGN 17.26), copied, named on the test's registry, each
//! `require` from it.

use ritsu_base::modules::lock::{Lock, Locked, Pinned};
use ritsu_base::modules::names::ModuleName;
use ritsu_base::modules::semver::Version;
use ritsu_base::modules::store;
use ritsu_testkit::TempDir;
use ritsu_testkit::level::{Need, need, ready};
use ritsu_testkit::registry::{self as reg, Registry};
use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const LANG_VARS: [&str; 10] = ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG", "KANAME_LANG"];

/// A cosign of the test's own: it writes the arguments it is called with, a line each call, and
/// says no when `FAKE_COSIGN_EXIT` is not 0.
const FAKE_COSIGN: &str = "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$FAKE_COSIGN_LOG\"\nif [ -n \"$FAKE_COSIGN_EXIT\" ] && [ \"$FAKE_COSIGN_EXIT\" != 0 ]; then\n  echo 'Error: no matching signatures (the fake cosign of the test)' >&2\n  exit \"$FAKE_COSIGN_EXIT\"\nfi\n";

/// A cosign of the test's own that checks keys: it writes the arguments it is called with, a line
/// each call, and passes a reference only when the key file it is given has the bytes of the key the
/// reference was signed with: the file under `$FAKE_COSIGN_SIGNED` named by the reference, its `/`,
/// `:` and `@` made `_`.
const KEYED_COSIGN: &str = "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$FAKE_COSIGN_LOG\"\nkey=\"\"\nfor a in \"$@\"; do ref=\"$a\"; done\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--key\" ]; then key=\"$2\"; fi\n  shift\ndone\nsigned=\"$FAKE_COSIGN_SIGNED/$(printf '%s' \"$ref\" | tr '/:@' '___')\"\nif [ -n \"$key\" ] && [ -f \"$signed\" ] && cmp -s \"$key\" \"$signed\"; then exit 0; fi\necho 'Error: no matching signatures (the keyed cosign of the test)' >&2\nexit 1\n";

/// `path` made executable.
fn executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A file of an entry (which the store keeps read-only) made writable, to change it.
fn writable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).unwrap();
}

/// The modules of the example, copied into `t` (`tax`, `pricing`, `shop`, and `pricing-1.0.0`),
/// named on the registry `host`, each `require` from it, and a `ritsu >=` line under each version.
fn example_on(t: &TempDir, host: &str) {
    let from = here().join("tests/projects/modules");
    for (src, dst) in [("tax", "tax"), ("pricing", "pricing"), ("shop", "shop"), ("versions/pricing-1.0.0", "pricing-1.0.0")] {
        ritsu_testkit::tmp::copy_dir(&from.join(src), &t.path().join(dst));
        let rel = format!("{dst}/ritsu.mod");
        let text = t
            .read(&rel)
            .replace("example.com/acme/", &format!("{host}/acme/"))
            .replace("require tax path \"../tax\"", &format!("require tax oci \"{host}/acme/tax\" 1.0.0"))
            .replace("require tax path \"../../tax\"", &format!("require tax oci \"{host}/acme/tax\" 1.0.0"))
            .replace("require pricing path \"../pricing\"", &format!("require pricing oci \"{host}/acme/pricing\" 1.1.0"));
        let mut out = String::new();
        for line in text.split_inclusive('\n') {
            out.push_str(line);
            if line.starts_with("version ") {
                out.push_str("ritsu >=0.36.0\n");
            }
        }
        t.write(&rel, out);
    }
}

/// A test's directory (the modules, the store, a Docker config of its own with nothing in it, and
/// the fake cosign first on the PATH) and its registry.
struct World {
    t: TempDir,
    r: Registry,
}

impl World {
    fn new(what: &str) -> World {
        World::with(what, Registry::start())
    }

    fn with(what: &str, r: Registry) -> World {
        let t = TempDir::new(what);
        std::fs::create_dir_all(t.path().join("docker")).unwrap();
        executable(&t.write("bin/cosign", FAKE_COSIGN));
        example_on(&t, &r.host);
        World { t, r }
    }

    fn host(&self) -> String {
        self.r.host.clone()
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.t.path().join(rel)
    }

    fn store(&self) -> PathBuf {
        self.path("store")
    }

    /// ritsu, run in the directory `dir` of the test with the test's store, and `env` over the rest.
    fn run_env(&self, dir: &str, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
        let o = self.command(dir, args, env).output().expect("could not run ritsu");
        said(o)
    }

    fn command(&self, dir: &str, args: &[&str], env: &[(&str, &str)]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(self.path(dir)).args(args);
        c.env(store::VARIABLE, self.store()).env("DOCKER_CONFIG", self.path("docker")).env("FAKE_COSIGN_LOG", self.path("cosign.log"));
        c.env("PATH", format!("{}:{}", self.path("bin").display(), std::env::var("PATH").unwrap_or_default()));
        c.env_remove("FAKE_COSIGN_EXIT");
        for v in LANG_VARS {
            c.env_remove(v);
        }
        for (k, v) in env {
            c.env(k, v);
        }
        c
    }

    fn run(&self, dir: &str, args: &[&str]) -> (i32, String, String) {
        self.run_env(dir, args, &[])
    }

    /// ritsu, which has to pass: what it printed.
    fn ok(&self, dir: &str, args: &[&str]) -> String {
        let (code, out, err) = self.run(dir, args);
        assert_eq!(code, 0, "ritsu {args:?} in {dir}:\n{out}{err}");
        out
    }

    /// The calls of the fake cosign so far.
    fn cosign_calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.path("cosign.log")).unwrap_or_default().lines().map(str::to_string).collect()
    }

    /// The digest of the version `tag` of `repository` in the registry, and of its files.
    fn pushed(&self, repository: &str, tag: &str) -> (String, String) {
        let d = self.r.tag(repository, tag).unwrap_or_else(|| panic!("the registry has no {repository}:{tag}"));
        let m = json(&String::from_utf8(self.r.get(repository, &d).unwrap()).unwrap());
        (d, m["layers"][1]["digest"].as_str().unwrap().to_string())
    }

    /// The modules pushed that most tests start from: tax 1.0.0 and pricing 1.1.0 (pricing's lock and
    /// fetch first, as its check reads tax).
    fn push_tax_and_pricing(&self) {
        self.ok("tax", &["mod", "push"]);
        self.ok("pricing", &["mod", "lock"]);
        self.ok("pricing", &["mod", "fetch"]);
        self.ok("pricing", &["mod", "push"]);
    }

    /// What the requests the registry was sent come to: the method, the path without its query,
    /// and the status.
    fn asked(&self) -> Vec<String> {
        self.r.log().iter().map(|s| format!("{} {} {}", s.method, s.target.split('?').next().unwrap_or(""), s.status)).collect()
    }
}

fn said(o: Output) -> (i32, String, String) {
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn json(out: &str) -> serde_json::Value {
    serde_json::from_str(out).unwrap_or_else(|e| panic!("{e}: {out}"))
}

/// The entries of the store at `store`: each digest, and whether it has its files.
fn entries(store: &Path) -> Vec<(String, bool)> {
    let Ok(rd) = std::fs::read_dir(store.join("sha256")) else { return Vec::new() };
    let mut out: Vec<(String, bool)> = rd.map(|e| e.unwrap()).map(|e| (format!("sha256:{}", e.file_name().to_string_lossy()), e.path().join("files").is_dir())).collect();
    out.sort();
    out
}

/// What is left under the `tmp/` of the store at `store`.
fn left(store: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(store.join("tmp")) else { return Vec::new() };
    rd.map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect()
}

/// The `record.json` of the entry `digest` of the store at `store`.
fn record(store: &Path, digest: &str) -> serde_json::Value {
    let hex = digest.strip_prefix("sha256:").unwrap();
    json(&std::fs::read_to_string(store.join("sha256").join(hex).join("record.json")).unwrap())
}

/// The lock of one module from a registry: `name` at `version`, with its two digests.
fn lock_of(name: &str, version: &str, digest: &str, files: &str) -> String {
    let pinned = Pinned::Oci { version: Version::parse(version).unwrap(), digest: digest.to_string(), files: files.to_string() };
    Lock::new(vec![Locked { name: ModuleName::parse(name).unwrap(), pinned, line: 0 }]).text()
}

/// A module pushed, required, locked, fetched and checked: tax, then pricing (whose lock reads
/// tax's `ritsu.mod` from the registry, whose fetch takes tax's files, whose push checks pricing
/// over them), then shop, the module at hand, which fetches pricing and finds tax here. What each
/// prints, and that the registry is asked nothing more once every module is here.
#[test]
fn a_module_pushed_is_locked_fetched_and_checked() {
    let w = World::new("oci-flow");
    let host = w.host();
    let out = w.ok("tax", &["mod", "push"]);
    let (tax, tax_files) = w.pushed("acme/tax", "1.0.0");
    let r = format!("{host}/acme/tax@{tax}");
    assert_eq!(
        out,
        format!(
            "pushed {host}/acme/tax 1.0.0\n  digest: {tax}\n  files: {tax_files}\n  reference: {r}\nsign it with cosign:\n  in CI, with no key: cosign sign --yes --allow-http-registry {r}\n  with a key: cosign sign --key cosign.key --allow-http-registry {r}\n"
        )
    );
    // the artifact of DESIGN 17.11: the empty config and two layers, each blob in the registry
    let m = json(&String::from_utf8(w.r.get("acme/tax", &tax).unwrap()).unwrap());
    assert_eq!((&m["artifactType"], &m["config"]["size"], &m["layers"][0]["mediaType"]), (&serde_json::json!("application/vnd.ritsu.module.v1"), &serde_json::json!(2), &serde_json::json!("application/vnd.ritsu.mod.v1")));
    assert_eq!(w.r.get("acme/tax", m["layers"][0]["digest"].as_str().unwrap()).unwrap(), std::fs::read(w.path("tax/ritsu.mod")).unwrap());
    assert!(w.r.log().iter().all(|s| s.authorization.is_none()));
    // pricing requires tax 1.0.0 from the registry
    assert_eq!(w.ok("pricing", &["mod", "lock"]), "wrote ritsu.lock: 1 module\n");
    assert_eq!(w.ok("pricing", &["mod", "fetch"]), format!("fetched {host}/acme/tax 1.0.0 ({tax})\nfetched 1 module\n"));
    assert_eq!(w.ok("pricing", &["mod", "fetch"]), "every module is here already (1 module)\n");
    w.ok("pricing", &["mod", "push"]);
    let (pricing, pricing_files) = w.pushed("acme/pricing", "1.1.0");
    // shop: its lock pins both, its fetch takes pricing and finds tax, and its check passes
    assert_eq!(w.ok("shop", &["mod", "lock"]), "wrote ritsu.lock: 2 modules\n");
    let v = json(&w.ok("shop", &["mod", "fetch", "--format", "json"]));
    assert_eq!(
        v,
        serde_json::json!({
            "ritsu": env!("CARGO_PKG_VERSION"),
            "fetched": [{"module": format!("{host}/acme/pricing"), "version": "1.1.0", "digest": pricing, "commit": null, "verified": []}],
            "present": [{"module": format!("{host}/acme/tax"), "version": "1.0.0", "digest": tax, "commit": null, "verified": []}],
            "error": null
        })
    );
    let (code, out, err) = w.run("shop", &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    // every module here: the registry is asked nothing
    w.r.clear_log();
    assert_eq!(w.ok("shop", &["mod", "fetch", "--lang", "ja"]), "モジュールはどれもそろっています（2 個）\n");
    assert_eq!(w.asked(), Vec::<String>::new());
    // the store: the two entries with their files, their records, and nothing left in tmp/
    assert_eq!(entries(&w.store()), {
        let mut e = vec![(tax.clone(), true), (pricing.clone(), true)];
        e.sort();
        e
    });
    let rec = record(&w.store(), &pricing);
    assert_eq!((&rec["module"], &rec["version"], &rec["files"]), (&serde_json::json!(format!("{host}/acme/pricing")), &serde_json::json!("1.1.0"), &serde_json::json!(pricing_files)));
    assert_eq!(left(&w.store()), Vec::<String>::new());
}

/// `versions` lists the tags that are versions (a signature's old tag is none); `get` moves a
/// dependency to a version, or to the latest, and writes the lock with it; `add` takes `@latest`
/// when no version is written; a version the registry does not have changes nothing (exit 2).
#[test]
fn versions_get_and_add_ask_the_registry() {
    let w = World::new("oci-versions");
    let host = w.host();
    w.push_tax_and_pricing();
    w.ok("pricing-1.0.0", &["mod", "lock"]);
    w.ok("pricing-1.0.0", &["mod", "fetch"]);
    w.ok("pricing-1.0.0", &["mod", "push"]);
    let (old, _) = w.pushed("acme/pricing", "1.0.0");
    // a tag of cosign's old form, beside the versions
    let bytes = w.r.get("acme/pricing", &old).unwrap();
    w.r.put_manifest("acme/pricing", Some(&format!("sha256-{}.sig", &old[7..])), reg::OCI_MANIFEST, &bytes);
    assert_eq!(w.ok("shop", &["mod", "versions", "pricing"]), "1.0.0\n1.1.0\n");
    let v = json(&w.ok("shop", &["mod", "versions", &format!("{host}/acme/pricing"), "--format", "json"]));
    assert_eq!(v, serde_json::json!({"ritsu": env!("CARGO_PKG_VERSION"), "module": format!("{host}/acme/pricing"), "versions": ["1.0.0", "1.1.0"], "latest": "1.1.0"}));
    // get: down to 1.0.0, then the latest
    w.ok("shop", &["mod", "lock"]);
    assert_eq!(
        w.ok("shop", &["mod", "get", "pricing@1.0.0"]),
        format!("changed the version of the dependency `pricing` from 1.1.0 to 1.0.0 ({host}/acme/pricing)\nwrote ritsu.lock: 2 modules\n")
    );
    assert!(w.t.read("shop/ritsu.mod").contains(&format!("require pricing oci \"{host}/acme/pricing\" 1.0.0\n")));
    assert!(w.t.read("shop/ritsu.lock").contains(&format!("oci {host}/acme/pricing 1.0.0 {old} ")), "{}", w.t.read("shop/ritsu.lock"));
    let v = json(&w.ok("shop", &["mod", "get", "pricing@latest", "--format", "json"]));
    assert_eq!((&v["require"]["version"], &v["lock"]), (&serde_json::json!("1.1.0"), &serde_json::json!("written")));
    // add, in a module of its own: the latest, and a version written
    std::fs::create_dir_all(w.path("shop2")).unwrap();
    w.ok("shop2", &["mod", "init", "example.com/acme/shop2"]);
    assert_eq!(
        w.ok("shop2", &["mod", "add", "pricing", &format!("{host}/acme/pricing")]),
        format!("added `require pricing oci \"{host}/acme/pricing\" 1.1.0` to ritsu.mod ({host}/acme/pricing 1.1.0)\nwrote ritsu.lock: 2 modules\n")
    );
    let v = json(&w.ok("shop2", &["mod", "add", "tax", &format!("{host}/acme/tax@1.0.0"), "--format", "json"]));
    assert_eq!(v["require"], serde_json::json!({"name": "tax", "module": format!("{host}/acme/tax"), "version": "1.0.0", "source": "oci", "path": null, "url": null, "dir": null}));
    // a version the registry does not have: nothing changes
    let before = (w.t.read("shop2/ritsu.mod"), w.t.read("shop2/ritsu.lock"));
    let (code, out, err) = w.run("shop2", &["mod", "get", "pricing@9.9.9"]);
    assert_eq!((code, out.as_str(), err), (2, "", format!("error: the registry has no version 9.9.9 of `{host}/acme/pricing`\n")));
    assert_eq!((w.t.read("shop2/ritsu.mod"), w.t.read("shop2/ritsu.lock")), before);
    let (code, _, err) = w.run("shop2", &["mod", "versions", "nothing"]);
    assert_eq!(code, 2, "{err}");
}

/// What `push` refuses, each before it sends anything: a name that is not published, no version, no
/// `ritsu >=`, a path dependency, a key in a file (W901, the key never shown), a check that fails;
/// and, at the registry, other files at the version's tag (a version does not move), or something
/// other than a module there (an OCI manifest over its limit too). The same files again are pushed
/// already (exit 0).
#[test]
fn push_refuses_what_it_cannot_send() {
    let w = World::new("oci-push-refused");
    let host = w.host();
    let original = w.t.read("tax/ritsu.mod");
    let refused = |rel_from: &str, rel_to: &str, want: &str| {
        let text = original.replacen(rel_from, rel_to, 1);
        assert_ne!(text, original, "{rel_from}");
        w.t.write("tax/ritsu.mod", &text);
        let (code, out, err) = w.run("tax", &["mod", "push"]);
        assert_eq!((code, out.as_str()), (1, ""), "{err}");
        assert!(err.contains(want), "{err}");
        w.t.write("tax/ritsu.mod", &original);
    };
    refused(&format!("module {host}/acme/tax"), "module tax", "has a name that is not published, so it cannot be pushed");
    refused("version 1.0.0\n", "", "`ritsu.mod` has no `version`; write the version to push");
    refused("ritsu >=0.36.0\n", "", "`ritsu.mod` has no `ritsu` line");
    std::fs::create_dir_all(w.path("lib")).unwrap();
    w.t.write("lib/ritsu.mod", "module example.com/acme/lib\nversion 0.1.0\n");
    refused("license", "require lib path \"../lib\"\nlicense", "a module with a path dependency cannot be pushed");
    // a key in a file
    w.t.write("tax/notes/keys.txt", "the deploy key: ghp_RitsuFakeTokenForPushTests0123456789\n");
    let (code, out, err) = w.run("tax", &["mod", "push"]);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(err.contains("warning[ritsu W901]: notes/keys.txt:1:17: A GitHub token is written here (ghp_…, 40 characters)"), "{err}");
    assert!(err.contains("the files to push have 1 key written in them, and nothing was pushed"), "{err}");
    assert!(!err.contains("RitsuFakeToken"), "{err}");
    let (code, out, _) = w.run("tax", &["mod", "push", "--format", "json"]);
    assert_eq!(code, 1);
    assert!(!out.contains("RitsuFakeToken"), "{out}");
    assert_eq!(json(&out)["pushed"], serde_json::Value::Null);
    // a value for tests says so on its line
    w.t.write("tax/notes/keys.txt", "the deploy key: ghp_RitsuFakeTokenForPushTests0123456789  # ritsu: test secret\n");
    // the check fails
    let rule = w.t.read("tax/rules/vat_rate.rule");
    w.t.write("tax/rules/vat_rate.rule", format!("{rule}\nthis is no rule\n"));
    let (code, _, err) = w.run("tax", &["mod", "push"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("26 | this is no rule") && err.contains("the check of the files to push found errors (above), and nothing was pushed"), "{err}");
    w.t.write("tax/rules/vat_rate.rule", &rule);
    // nothing was sent
    assert_eq!(w.asked(), Vec::<String>::new());
    // pushed; the same files again; other files at the same version
    w.ok("tax", &["mod", "push"]);
    let (tax, _) = w.pushed("acme/tax", "1.0.0");
    assert_eq!(w.ok("tax", &["mod", "push"]).lines().next(), Some(format!("{host}/acme/tax 1.0.0 is pushed already: the registry has the same files").as_str()));
    let v = json(&w.ok("tax", &["mod", "push", "--format", "json", "--lang", "ja"]));
    assert_eq!((&v["pushed"], &v["digest"]), (&serde_json::json!("already"), &serde_json::json!(tax)));
    w.t.write("tax/rules/notes.md", "a note\n");
    let (code, _, err) = w.run("tax", &["mod", "push"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("a version pushed does not move: raise the `version` of `ritsu.mod` and push again"), "{err}");
    // something other than a module at the tag, and an OCI manifest over its 64 KiB there
    let other = format!("the registry has something other than a module of ritsu at the tag 1.0.0 of {host}/acme/tax; raise the version and push again: ");
    w.r.serve_instead(&tax, b"{}");
    let (code, _, err) = w.run("tax", &["mod", "push"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains(&other), "{err}");
    w.r.serve_instead(&tax, &vec![b' '; 64 * 1024 + 1]);
    let (code, _, err) = w.run("tax", &["mod", "push"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains(&format!("{other}the answer to `GET http://{host}/v2/acme/tax/manifests/1.0.0` is over 65536 bytes")), "{err}");
    assert_eq!(w.r.tag("acme/tax", "1.0.0"), Some(tax));
}

/// `push --dry-run` builds the artifact and says its digest, asking no registry: the same module
/// gives the same bytes every time, and on every system (the golden files hold the digest).
#[test]
fn a_dry_run_gives_the_same_digest_every_time() {
    let t = TempDir::new("oci-dry-run");
    t.write("tax/ritsu.mod", "module example.com/acme/tax\nversion 1.0.0\nritsu >=0.36.0\ndescription \"The VAT rates of the Acme shop\"\nlicense \"MIT OR Apache-2.0\"\n");
    std::fs::create_dir_all(t.path().join("tax/rules")).unwrap();
    std::fs::copy(here().join("tests/projects/modules/tax/rules/vat_rate.rule"), t.path().join("tax/rules/vat_rate.rule")).unwrap();
    // a curl that writes down that it was called and does nothing: the dry run must not call it
    executable(&t.write("bin/curl", format!("#!/bin/sh\necho called >> '{}'\nexit 7\n", t.path().join("curl.log").display())));
    let path = format!("{}:{}", t.path().join("bin").display(), std::env::var("PATH").unwrap_or_default());
    let run = |args: &[&str]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(t.path().join("tax")).args(args).env(store::VARIABLE, t.path().join("store")).env("DOCKER_CONFIG", t.path().join("docker")).env("PATH", &path);
        for v in LANG_VARS {
            c.env_remove(v);
        }
        said(c.output().unwrap())
    };
    let mut failures = Vec::new();
    for (lang, ext) in [("en", "en.txt"), ("ja", "ja.txt")] {
        let (code, out, err) = run(&["mod", "push", "--dry-run", "--lang", lang]);
        assert_eq!((code, err.as_str()), (0, ""), "{out}");
        assert_eq!(run(&["mod", "push", "--dry-run", "--lang", lang]).1, out);
        if let Err(e) = ritsu_testkit::golden::check(&here().join(format!("tests/golden/modules/push-dry-run.{ext}")), &out) {
            failures.push(e);
        }
    }
    let (code, out, _) = run(&["mod", "push", "--dry-run", "--format", "json"]);
    assert_eq!(code, 0);
    if let Err(e) = ritsu_testkit::golden::check(&here().join("tests/golden/modules/push-dry-run.json"), &out) {
        failures.push(e);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(!t.exists("curl.log"), "the dry run called curl");
}

/// `push --dry-run` of the seed module of the repository (`modules/holidays`, DESIGN 17.18), run
/// where it is: the digest a publication would give, held to golden files written on macOS, which the
/// platforms level holds Linux to. A change to a file of the module, or to its `ritsu >=` line, gives
/// another digest, and the golden files are written again (`RITSU_BLESS=1`).
#[test]
fn the_seed_module_s_dry_run_gives_the_same_digest_every_time() {
    let t = TempDir::new("oci-seed-dry-run");
    let seed = here().join("../../modules/holidays");
    executable(&t.write("bin/curl", format!("#!/bin/sh\necho called >> '{}'\nexit 7\n", t.path().join("curl.log").display())));
    let path = format!("{}:{}", t.path().join("bin").display(), std::env::var("PATH").unwrap_or_default());
    let run = |args: &[&str]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(&seed).args(args).env(store::VARIABLE, t.path().join("store")).env("DOCKER_CONFIG", t.path().join("docker")).env("PATH", &path);
        for v in LANG_VARS {
            c.env_remove(v);
        }
        said(c.output().unwrap())
    };
    let mut failures = Vec::new();
    for (lang, ext) in [("en", "en.txt"), ("ja", "ja.txt")] {
        let (code, out, err) = run(&["mod", "push", "--dry-run", "--lang", lang]);
        assert_eq!((code, err.as_str()), (0, ""), "{out}");
        if let Err(e) = ritsu_testkit::golden::check(&here().join(format!("tests/golden/modules/holidays-push-dry-run.{ext}")), &out) {
            failures.push(e);
        }
    }
    let (code, out, _) = run(&["mod", "push", "--dry-run", "--format", "json"]);
    assert_eq!(code, 0);
    if let Err(e) = ritsu_testkit::golden::check(&here().join("tests/golden/modules/holidays-push-dry-run.json"), &out) {
        failures.push(e);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(!t.exists("curl.log"), "the dry run called curl");
}

/// Each broken answer of a registry stops `fetch` (exit 1 for what the answer holds, exit 2 for an
/// answer that does not come), and nothing is put in the store: an OCI manifest that is not its
/// digest, a layer that is not its digest, an OCI manifest and a layer with more bytes than they may
/// have (curl stops them), a lock whose files the OCI manifest does not have (E304), sizes over the
/// limits, a tar with `..`, an absolute path or a link, a 500, a 429 and a 503 that last. A tag
/// that moved after the lock was written is not followed: the lock's digest is fetched.
#[test]
fn every_broken_answer_stops_with_nothing_in_the_store() {
    let w = World::new("oci-broken");
    let host = w.host();
    w.ok("tax", &["mod", "push"]);
    let (tax, files) = w.pushed("acme/tax", "1.0.0");
    let manifest = String::from_utf8(w.r.get("acme/tax", &tax).unwrap()).unwrap();
    let m = json(&manifest);
    let mod_digest = m["layers"][0]["digest"].as_str().unwrap().to_string();
    let tar_size = m["layers"][1]["size"].as_u64().unwrap();
    let tar = w.r.get("acme/tax", &files).unwrap();
    let mod_bytes = w.r.get("acme/tax", &mod_digest).unwrap();
    w.t.write("app/ritsu.mod", format!("module example.com/acme/app\nversion 0.1.0\nrequire tax oci \"{host}/acme/tax\" 1.0.0\n"));
    let name = format!("{host}/acme/tax");
    // a manifest of the test's own, with the files' layer put to `layer` of `size` bytes
    let with_files = |layer: &str, size: u64| -> String {
        let m = manifest.replacen(&format!("\"digest\":\"{files}\",\"size\":{tar_size}"), &format!("\"digest\":\"{layer}\",\"size\":{size}"), 1);
        assert_ne!(m, manifest);
        w.r.put_manifest("acme/tax", None, reg::OCI_MANIFEST, m.as_bytes())
    };
    let mut n = 0;
    let mut stops = |lock: String, code: i32, want: &str| {
        n += 1;
        let store = w.path(&format!("store-{n}"));
        w.t.write("app/ritsu.lock", &lock);
        let (got, out, err) = w.run_env("app", &["mod", "fetch"], &[(store::VARIABLE, store.to_str().unwrap())]);
        assert_eq!((got, out.as_str()), (code, ""), "case {n}: {err}");
        assert!(err.contains(want), "case {n}: {err}");
        assert_eq!(entries(&store), Vec::<(String, bool)>::new(), "case {n}");
        assert_eq!(left(&store), Vec::<String>::new(), "case {n}");
        assert!(!store.join("refs").exists(), "case {n}");
    };
    let pinned = lock_of(&name, "1.0.0", &tax, &files);
    // 1. the OCI manifest is not its digest
    w.r.serve_instead(&tax, manifest.replace("Acme", "Acne").as_bytes());
    stops(pinned.clone(), 1, "have the digest sha256:");
    // an OCI manifest over its 64 KiB
    w.r.serve_instead(&tax, &vec![b' '; 64 * 1024 + 1]);
    stops(pinned.clone(), 1, &format!("`{name}` 1.0.0: the answer to `GET http://{host}/v2/acme/tax/manifests/{tax}` is over 65536 bytes"));
    w.r.serve_instead(&tax, manifest.as_bytes());
    // 2. the tar is not its digest (the same size)
    let mut other = tar.clone();
    other[600] ^= 1;
    w.r.serve_instead(&files, &other);
    stops(pinned.clone(), 1, &format!("fetched as the layer `application/vnd.ritsu.files.v1.tar` of `{name}` 1.0.0 have the digest"));
    // a byte more than the OCI manifest names
    let mut longer = tar.clone();
    longer.push(0);
    w.r.serve_instead(&files, &longer);
    stops(pinned.clone(), 1, &format!("the bytes fetched as the layer `application/vnd.ritsu.files.v1.tar` of `{name}` 1.0.0 are over the {tar_size} bytes its OCI manifest names ({files})"));
    w.r.serve_instead(&files, &tar);
    // 3. the `ritsu.mod` layer is not its digest
    let mut other = mod_bytes.clone();
    other[0] ^= 1;
    w.r.serve_instead(&mod_digest, &other);
    stops(pinned.clone(), 1, "fetched as the layer `application/vnd.ritsu.mod.v1`");
    w.r.serve_instead(&mod_digest, &mod_bytes);
    // 4. the lock pins files the OCI manifest does not have
    let wrong = format!("sha256:{}", "0".repeat(64));
    stops(lock_of(&name, "1.0.0", &tax, &wrong), 1, "error[ritsu E304]: ritsu.lock");
    // 5. an OCI manifest that says its files are over the limit
    let big = with_files(&files, 64 * 1024 * 1024 + 1);
    stops(lock_of(&name, "1.0.0", &big, &files), 1, "is 67108865 bytes, and it is 67108864 bytes at most");
    // 6–8. a tar with `..`, an absolute path, a link
    for (entry, typeflag, link, want) in [
        ("../evil.txt", b'0', "", "the path `../evil.txt` of a module's file is refused: it has a part `..`"),
        ("/tmp/ritsu-test-evil.txt", b'0', "", "the path `/tmp/ritsu-test-evil.txt` of a module's file is refused: it is absolute"),
        ("rules/link.rule", b'2', "/etc/hosts", "the type `2` of `rules/link.rule` is refused: a module's tar holds files only"),
    ] {
        let bad = reg::tar(&[("ritsu.mod", b'0', &mod_bytes, ""), (entry, typeflag, b"evil\n", link)]);
        let d = w.r.put_blob("acme/tax", &bad);
        let m = with_files(&d, bad.len() as u64);
        stops(lock_of(&name, "1.0.0", &m, &d), 1, want);
    }
    assert!(!w.t.path().join("evil.txt").exists() && !Path::new("/tmp/ritsu-test-evil.txt").exists());
    // 9. a 500 that lasts (three tries, two and four seconds apart)
    w.r.fail(Some("GET"), &files[7..], 500, 3, None);
    stops(pinned.clone(), 2, "500");
    // 10. a 429 that lasts, its Retry-After 0
    w.r.fail(Some("GET"), "/manifests/", 429, 3, Some(0));
    stops(pinned.clone(), 2, "429");
    // a 503 once, its Retry-After 1: the second try takes it
    w.r.fail(Some("GET"), "/manifests/", 503, 1, Some(1));
    let store = w.path("store-retried");
    w.t.write("app/ritsu.lock", &pinned);
    let (code, out, err) = w.run_env("app", &["mod", "fetch"], &[(store::VARIABLE, store.to_str().unwrap())]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(entries(&store), [(tax.clone(), true)]);
    // the tag moved after the lock was written: the lock's digest is what is fetched
    let moved = w.r.put_manifest("acme/tax", Some("1.0.0"), reg::OCI_MANIFEST, manifest.replace("The VAT rates", "The VAT rates, moved").as_bytes());
    assert_eq!(w.r.tag("acme/tax", "1.0.0"), Some(moved));
    let store = w.path("store-moved");
    let (code, out, err) = w.run_env("app", &["mod", "fetch"], &[(store::VARIABLE, store.to_str().unwrap())]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(entries(&store), [(tax, true)]);
}

/// Two fetches of the same lock at once, with one store: both pass, and the store has each entry
/// once, and nothing left in tmp/.
#[test]
fn two_fetches_at_once_leave_one_store() {
    let w = World::new("oci-two-fetches");
    w.push_tax_and_pricing();
    w.ok("shop", &["mod", "lock"]);
    let store = w.path("store-shared");
    let env = [(store::VARIABLE, store.to_str().unwrap())];
    let a = w.command("shop", &["mod", "fetch"], &env).spawn().unwrap();
    let b = w.command("shop", &["mod", "fetch"], &env).spawn().unwrap();
    for c in [a, b] {
        let (code, out, err) = said(c.wait_with_output().unwrap());
        assert_eq!(code, 0, "{out}{err}");
    }
    assert_eq!(entries(&store).len(), 2);
    assert!(entries(&store).iter().all(|(_, files)| *files));
    assert_eq!(left(&store), Vec::<String>::new());
    let (code, out, err) = w.run_env("shop", &["check"], &env);
    assert_eq!(code, 0, "{out}{err}");
}

/// Files of an entry changed in the store: `check` says E305, and `fetch` takes the entry again. In
/// the module's copy of the store, the same change stops `fetch` (exit 1), which asks for `ritsu mod
/// vendor`.
#[test]
fn files_changed_in_the_store_are_fetched_again() {
    let w = World::new("oci-e305");
    w.push_tax_and_pricing();
    let (pricing, _) = w.pushed("acme/pricing", "1.1.0");
    w.ok("shop", &["mod", "lock"]);
    w.ok("shop", &["mod", "fetch"]);
    let file = w.store().join("sha256").join(&pricing[7..]).join("files/rules/shipping_fee.rule");
    writable(&file);
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("{text}\n# changed\n")).unwrap();
    let (code, out, err) = w.run("shop", &["check"]);
    assert_eq!(code, 1, "{err}");
    assert!(out.contains("E305"), "{out}");
    let host = w.host();
    assert_eq!(w.ok("shop", &["mod", "fetch"]), format!("fetched {host}/acme/pricing 1.1.0 ({pricing})\nfetched 1 module (1 here already)\n"));
    let (code, out, err) = w.run("shop", &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    // the module's copy
    w.ok("shop", &["mod", "vendor"]);
    let copy = w.path("shop/.ritsu/modules/sha256").join(&pricing[7..]).join("files/rules/shipping_fee.rule");
    writable(&copy);
    std::fs::write(&copy, "# changed\n").unwrap();
    let (code, out, err) = w.run("shop", &["mod", "fetch"]);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(err.contains("in this module's copy of the store differ from the digest of `ritsu.lock`; run `ritsu mod vendor`"), "{err}");
    w.ok("shop", &["mod", "vendor"]);
    assert_eq!(w.ok("shop", &["mod", "fetch"]), "every module is here already (2 modules)\n");
}

/// `fetch --rev` fetches the modules of the lock of a git revision (the working tree's lock may be
/// another, or none); `fetch <name>@<version>` fetches a version and what the choice of the versions
/// from it reaches, and writes no lock. Without a lock, a tree with a module from a registry stops
/// (exit 2).
#[test]
fn fetch_takes_a_revision_or_versions_named() {
    let w = World::new("oci-rev");
    let host = w.host();
    w.push_tax_and_pricing();
    let (pricing, _) = w.pushed("acme/pricing", "1.1.0");
    let (tax, _) = w.pushed("acme/tax", "1.0.0");
    w.ok("shop", &["mod", "lock"]);
    let git = |args: &[&str]| {
        let o = Command::new("git")
            .current_dir(w.path("shop"))
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "the lock"]);
    std::fs::remove_file(w.path("shop/ritsu.lock")).unwrap();
    let (code, _, err) = w.run("shop", &["mod", "fetch"]);
    assert_eq!(code, 2);
    assert!(err.contains("there is no `ritsu.lock`"), "{err}");
    let store = w.path("store-rev");
    let env = [(store::VARIABLE, store.to_str().unwrap())];
    let (code, out, err) = w.run_env("shop", &["mod", "fetch", "--rev", "HEAD"], &env);
    assert_eq!(code, 0, "{err}");
    assert!(out.ends_with("fetched 2 modules\n"), "{out}");
    let mut want = vec![(tax.clone(), true), (pricing.clone(), true)];
    want.sort();
    assert_eq!(entries(&store), want);
    let (code, _, err) = w.run_env("shop", &["mod", "fetch", "--rev", "nothing-of-that-name"], &env);
    assert_eq!(code, 2, "{err}");
    // versions named, with no module at hand: pricing 1.1.0 and the tax it requires
    let store = w.path("store-named");
    std::fs::create_dir_all(w.path("elsewhere")).unwrap();
    let (code, out, err) = w.run_env("elsewhere", &["mod", "fetch", &format!("{host}/acme/pricing@1.1.0")], &[(store::VARIABLE, store.to_str().unwrap())]);
    assert_eq!(code, 0, "{err}");
    assert!(out.ends_with("fetched 2 modules\n"), "{out}");
    assert_eq!(entries(&store), want);
    assert!(!w.path("elsewhere/ritsu.lock").exists());
    let (code, _, err) = w.run_env("elsewhere", &["mod", "fetch", &format!("{host}/acme/pricing@7.0.0")], &[(store::VARIABLE, store.to_str().unwrap())]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("the registry has no version 7.0.0"), "{err}");
}

/// The signatures, with the fake cosign (DESIGN 17.14): each line that applies is checked with the
/// arguments of its kind; the record names the line and the digest of its key file, so `check`
/// passes, a fetch again asks cosign nothing, and a key file changed or a line added asks for the
/// check again (of that line alone). A cosign that says no stops the fetch (exit 1, its words shown,
/// nothing in the store); no cosign stops it (exit 2).
#[test]
fn signatures_are_checked_with_cosign_and_recorded() {
    let w = World::new("oci-cosign-fake");
    let host = w.host();
    w.ok("tax", &["mod", "push"]);
    let (tax, _) = w.pushed("acme/tax", "1.0.0");
    let line = format!("signature \"{host}/acme/\" key \"keys/acme.pub\" tlog off");
    let by = format!("signature \"{host}/acme/tax\" by \"release@example.com\" issuer \"https://issuer.example.com\"");
    let m = w.t.read("pricing/ritsu.mod");
    w.t.write("pricing/ritsu.mod", format!("{m}\n{line}\n{by}\n"));
    w.t.write("pricing/keys/acme.pub", "-----BEGIN PUBLIC KEY-----\nthe key of the test\n-----END PUBLIC KEY-----\n");
    w.ok("pricing", &["mod", "lock"]);
    // cosign says no: nothing in the store
    let (code, out, err) = w.run_env("pricing", &["mod", "fetch"], &[("FAKE_COSIGN_EXIT", "1")]);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(err.contains(&format!("the signature of `{host}/acme/tax` 1.0.0 does not pass `{line}`; cosign said:\nError: no matching signatures (the fake cosign of the test)")), "{err}");
    assert_eq!(entries(&w.store()).iter().filter(|(_, files)| *files).count(), 0);
    // no cosign
    let (code, _, err) = w.run_env("pricing", &["mod", "fetch"], &[("PATH", "/usr/bin:/bin")]);
    if Path::new("/usr/bin/cosign").exists() || Path::new("/bin/cosign").exists() {
        ritsu_testkit::skip::skip("cosign is in /usr/bin or /bin, so a run without cosign cannot be made");
    } else {
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("cosign is not there, and a module has a signature to check"), "{err}");
    }
    std::fs::remove_file(w.path("cosign.log")).ok();
    // cosign says yes: both lines, the arguments of each
    let out = w.ok("pricing", &["mod", "fetch"]);
    assert_eq!(out, format!("fetched {host}/acme/tax 1.0.0 ({tax})\n  checked its signature against `{line}`\n  checked its signature against `{by}`\nfetched 1 module\n"));
    let key = std::fs::canonicalize(w.path("pricing/keys/acme.pub")).unwrap();
    let r = format!("{host}/acme/tax@{tax}");
    assert_eq!(
        w.cosign_calls(),
        [
            format!("verify --key {} --insecure-ignore-tlog=true --allow-http-registry {r}", key.display()),
            format!("verify --certificate-identity release@example.com --certificate-oidc-issuer https://issuer.example.com --allow-http-registry {r}"),
        ]
    );
    let key_digest = format!("sha256:{}", reg::sha256_hex(&std::fs::read(&key).unwrap()));
    let rec = record(&w.store(), &tax);
    let verified: Vec<&str> = rec["verified"].as_array().unwrap().iter().map(|v| v["policy"].as_str().unwrap()).collect();
    assert_eq!(verified, [format!("{line} {key_digest}"), by.clone()]);
    let (code, out, err) = w.run("pricing", &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    // here, and recorded: cosign is not asked
    assert_eq!(w.ok("pricing", &["mod", "fetch"]), "every module is here already (1 module)\n");
    assert_eq!(w.cosign_calls().len(), 2);
    // the key file changed: check stops, and fetch checks again with the new key
    w.t.write("pricing/keys/acme.pub", "-----BEGIN PUBLIC KEY-----\nanother key of the test\n-----END PUBLIC KEY-----\n");
    let (code, _, err) = w.run("pricing", &["check"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("ritsu mod fetch"), "{err}");
    assert_eq!(w.ok("pricing", &["mod", "fetch"]), format!("checked the signature of {host}/acme/tax 1.0.0 against `{line}`\nevery module is here already (1 module)\n"));
    assert_eq!(w.cosign_calls().len(), 3);
    let (code, out, err) = w.run("pricing", &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    // a line added: check stops, and fetch checks that line alone
    let matching = format!("signature \"{host}/acme/\" by matching \"^release@example[.]com$\" issuer \"https://issuer.example.com\"");
    let m = w.t.read("pricing/ritsu.mod");
    w.t.write("pricing/ritsu.mod", format!("{m}{matching}\n"));
    let (code, _, err) = w.run("pricing", &["check"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("ritsu mod fetch"), "{err}");
    assert_eq!(w.ok("pricing", &["mod", "fetch"]), format!("checked the signature of {host}/acme/tax 1.0.0 against `{matching}`\nevery module is here already (1 module)\n"));
    let calls = w.cosign_calls();
    assert_eq!(calls[3..], [format!("verify --certificate-identity-regexp ^release@example[.]com$ --certificate-oidc-issuer https://issuer.example.com --allow-http-registry {r}")]);
    // the record is only added to: the check with the first key stays, for another module that
    // shares the store and has that key under the same path
    let new_digest = format!("sha256:{}", reg::sha256_hex(&std::fs::read(&key).unwrap()));
    let verified: Vec<String> = record(&w.store(), &tax)["verified"].as_array().unwrap().iter().map(|v| v["policy"].as_str().unwrap().to_string()).collect();
    assert_eq!(verified, [format!("{line} {key_digest}"), by.clone(), format!("{line} {new_digest}"), matching.clone()]);
    let (code, out, err) = w.run("pricing", &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    // a missing key file is said at its line
    std::fs::remove_file(w.path("pricing/keys/acme.pub")).unwrap();
    let (code, _, err) = w.run("pricing", &["mod", "fetch"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("the key file `keys/acme.pub`"), "{err}");
}

/// A line of a module from a registry applies to the modules it requires directly: pricing, pushed
/// with a line for tax and the key file among its files, has tax checked when shop fetches the two;
/// the key file is written for cosign under the store's tmp/ and removed after.
#[test]
fn the_lines_of_a_module_from_a_registry_apply_to_what_it_requires() {
    let w = World::new("oci-cosign-theirs");
    let host = w.host();
    w.ok("tax", &["mod", "push"]);
    let (tax, _) = w.pushed("acme/tax", "1.0.0");
    let line = format!("signature \"{host}/acme/tax\" key \"keys/tax.pub\"");
    let m = w.t.read("pricing/ritsu.mod");
    w.t.write("pricing/ritsu.mod", format!("{m}\n{line}\n"));
    w.t.write("pricing/keys/tax.pub", "-----BEGIN PUBLIC KEY-----\nthe key of tax\n-----END PUBLIC KEY-----\n");
    w.ok("pricing", &["mod", "lock"]);
    w.ok("pricing", &["mod", "fetch"]);
    w.ok("pricing", &["mod", "push"]);
    std::fs::remove_file(w.path("cosign.log")).unwrap();
    w.ok("shop", &["mod", "lock"]);
    let store = w.path("store-shop");
    let env = [(store::VARIABLE, store.to_str().unwrap())];
    let (code, out, err) = w.run_env("shop", &["mod", "fetch"], &env);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains(&format!("fetched {host}/acme/tax 1.0.0 ({tax})\n  checked its signature against `{line}`\n")), "{out}");
    let calls = w.cosign_calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    let key = calls[0].strip_prefix("verify --key ").and_then(|c| c.strip_suffix(&format!(" --allow-http-registry {host}/acme/tax@{tax}"))).unwrap_or_else(|| panic!("{calls:?}"));
    assert!(key.contains("/store-shop/tmp/key-") && !Path::new(key).exists(), "{key}");
    assert_eq!(left(&store), Vec::<String>::new());
    let (code, out, err) = w.run_env("shop", &["check"], &env);
    assert_eq!(code, 0, "{out}{err}");
}

/// A key rotated between two revisions (DESIGN 17.12, 17.14). shop's revision `one` requires pricing
/// 1.0.0, signed with the old key, and has the old key; `two` (the working tree) requires 1.1.0,
/// signed with the new key, which took the old one's place. The cosign of this test passes a
/// reference only with the key it was signed with. From an empty store, `fetch --rev one` checks
/// 1.0.0 with the revision's key (written for cosign under the store's tmp/, and removed after), and
/// `fetch` checks 1.1.0 with the working tree's (at its own path); each record names its key, and
/// `check` passes. `report --base one`, from an empty store too, fetches both and checks the base's
/// lock with the base's key. The working tree's key does not pass 1.0.0.
#[test]
fn a_key_rotated_between_two_revisions() {
    let w = World::new("oci-rotated");
    let host = w.host();
    executable(&w.t.write("bin/cosign", KEYED_COSIGN));
    w.ok("tax", &["mod", "push"]);
    for step in ["lock", "fetch", "push"] {
        w.ok("pricing-1.0.0", &["mod", step]);
    }
    w.ok("pricing", &["mod", "lock"]);
    w.ok("pricing", &["mod", "push"]);
    let (old, _) = w.pushed("acme/pricing", "1.0.0");
    let (new, _) = w.pushed("acme/pricing", "1.1.0");
    let (old_key, new_key) = ("-----BEGIN PUBLIC KEY-----\nthe old key of the test\n-----END PUBLIC KEY-----\n", "-----BEGIN PUBLIC KEY-----\nthe new key of the test\n-----END PUBLIC KEY-----\n");
    let (r10, r11) = (format!("{host}/acme/pricing@{old}"), format!("{host}/acme/pricing@{new}"));
    w.t.write(&format!("signed/{}", r10.replace(['/', ':', '@'], "_")), old_key);
    w.t.write(&format!("signed/{}", r11.replace(['/', ':', '@'], "_")), new_key);
    let signed = w.path("signed");
    let env = |store: &Path| -> Vec<(String, String)> {
        vec![(store::VARIABLE.to_string(), store.display().to_string()), ("FAKE_COSIGN_SIGNED".to_string(), signed.display().to_string())]
    };
    let git = |args: &[&str]| {
        let o = Command::new("git")
            .current_dir(w.path("shop"))
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    };
    // `one`: pricing 1.0.0, the line for pricing, the old key
    let line = format!("signature \"{host}/acme/pricing\" key \"keys/acme.pub\"");
    let at = |v: &str| format!("require pricing oci \"{host}/acme/pricing\" {v}\n");
    let m = w.t.read("shop/ritsu.mod").replace(&at("1.1.0"), &at("1.0.0"));
    w.t.write("shop/ritsu.mod", format!("{m}\n{line}\n"));
    w.t.write("shop/keys/acme.pub", old_key);
    w.ok("shop", &["mod", "lock"]);
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "pricing 1.0.0, the old key"]);
    git(&["tag", "one"]);
    // `two`: pricing 1.1.0, the new key in the old one's place
    w.t.write("shop/ritsu.mod", w.t.read("shop/ritsu.mod").replace(&at("1.0.0"), &at("1.1.0")));
    w.t.write("shop/keys/acme.pub", new_key);
    w.ok("shop", &["mod", "lock"]);
    git(&["commit", "-q", "-a", "-m", "pricing 1.1.0, the new key"]);
    std::fs::remove_file(w.path("cosign.log")).ok();

    let store = w.path("store-rotated");
    let e = env(&store);
    let (code, out, err) = w.run_env("shop", &["mod", "fetch", "--rev", "one"], &pairs(&e));
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains(&format!("fetched {host}/acme/pricing 1.0.0 ({old})\n  checked its signature against `{line}`\n")) && out.ends_with("fetched 2 modules\n"), "{out}");
    let (code, out, err) = w.run_env("shop", &["mod", "fetch"], &pairs(&e));
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, format!("fetched {host}/acme/pricing 1.1.0 ({new})\n  checked its signature against `{line}`\nfetched 1 module (1 here already)\n"));
    let calls = w.cosign_calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    let key = calls[0].strip_prefix("verify --key ").and_then(|c| c.strip_suffix(&format!(" --allow-http-registry {r10}"))).unwrap_or_else(|| panic!("{calls:?}"));
    assert!(key.contains("/store-rotated/tmp/key-") && !Path::new(key).exists(), "{key}");
    let disk = std::fs::canonicalize(w.path("shop/keys/acme.pub")).unwrap();
    assert_eq!(calls[1], format!("verify --key {} --allow-http-registry {r11}", disk.display()));
    assert_eq!(left(&store), Vec::<String>::new());
    let digest_of = |k: &str| format!("sha256:{}", reg::sha256_hex(k.as_bytes()));
    let policies = |d: &str| -> Vec<String> { record(&store, d)["verified"].as_array().unwrap().iter().map(|v| v["policy"].as_str().unwrap().to_string()).collect() };
    assert_eq!((policies(&old), policies(&new)), (vec![format!("{line} {}", digest_of(old_key))], vec![format!("{line} {}", digest_of(new_key))]));
    let (code, out, err) = w.run_env("shop", &["check"], &pairs(&e));
    assert_eq!(code, 0, "{out}{err}");

    // the report fetches both into an empty store, and checks the base's lock with the base's key
    let report = w.path("store-report");
    let (code, out, err) = w.run_env("shop", &["mod", "report", "--base", "one", "--format", "json"], &pairs(&env(&report)));
    assert_eq!(code, 0, "{out}{err}");
    let v = json(&out);
    assert_eq!((&v["check"]["base"]["errors"], &v["check"]["head"]["errors"], &v["modules"][0]["change"]), (&serde_json::json!(0), &serde_json::json!(0), &serde_json::json!("upgraded")), "{out}");
    assert!(err.contains(&format!("fetched {host}/acme/pricing 1.0.0 ({old})\n  checked its signature against `{line}`\n")), "{err}");
    assert_eq!(w.cosign_calls().len(), 4);
    assert_eq!(left(&report), Vec::<String>::new());

    // the working tree's key does not pass 1.0.0: what the base's lock is checked with is the base's
    let (code, out, err) = w.run_env("shop", &["mod", "fetch", &format!("{host}/acme/pricing@1.0.0")], &pairs(&env(&w.path("store-wrong-key"))));
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(err.contains(&format!("the signature of `{host}/acme/pricing` 1.0.0 does not pass `{line}`; cosign said:\nError: no matching signatures (the keyed cosign of the test)")), "{err}");
}

/// Whether `program` is on the PATH.
fn on_path(program: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(program)).find(|p| p.is_file())
}

/// A proxy that answers nothing and writes down the first line of each request made to it: the
/// network beyond the loopback, for the tests of the real cosign. Its thread lives as long as the
/// test's process.
struct Sink {
    url: String,
    seen: Arc<Mutex<Vec<String>>>,
}

fn sink() -> Sink {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s = seen.clone();
    std::thread::spawn(move || {
        for c in l.incoming() {
            let Ok(mut c) = c else { continue };
            let _ = c.set_read_timeout(Some(std::time::Duration::from_secs(2)));
            let mut buf = [0u8; 1024];
            let n = c.read(&mut buf).unwrap_or(0);
            s.lock().unwrap().push(String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or("").to_string());
        }
    });
    Sink { url, seen }
}

/// cosign with no way out of the loopback: no password on its keys, every proxy the sink, its
/// TUF root and its home in the test's directory.
struct Cosign {
    env: Vec<(String, String)>,
}

impl Cosign {
    fn new(w: &World, sink: &Sink) -> Cosign {
        std::fs::create_dir_all(w.path("home")).unwrap();
        let env = vec![
            ("COSIGN_PASSWORD".to_string(), String::new()),
            ("HTTPS_PROXY".to_string(), sink.url.clone()),
            ("HTTP_PROXY".to_string(), sink.url.clone()),
            ("https_proxy".to_string(), sink.url.clone()),
            ("http_proxy".to_string(), sink.url.clone()),
            ("NO_PROXY".to_string(), "127.0.0.1,localhost".to_string()),
            ("no_proxy".to_string(), "127.0.0.1,localhost".to_string()),
            ("TUF_ROOT".to_string(), w.path("tuf").display().to_string()),
            ("HOME".to_string(), w.path("home").display().to_string()),
        ];
        Cosign { env }
    }

    fn run(&self, w: &World, dir: &Path, args: &[&str]) -> (i32, String, String) {
        let mut c = Command::new("cosign");
        c.current_dir(dir).args(args).env("DOCKER_CONFIG", w.path("docker"));
        for (k, v) in &self.env {
            c.env(k, v);
        }
        said(c.output().unwrap())
    }

    /// A key pair in `dir`, `<prefix>.key` and `<prefix>.pub`.
    fn keys(&self, w: &World, dir: &Path, prefix: &str) {
        std::fs::create_dir_all(dir).unwrap();
        let (code, out, err) = self.run(w, dir, &["generate-key-pair", "--output-key-prefix", prefix]);
        assert_eq!(code, 0, "{out}{err}");
    }

    /// The module at `reference` signed with the key `key`, and no transparency log: the command
    /// DESIGN 17.14 prints, with the two flags that keep cosign 3 off the network.
    fn sign(&self, w: &World, key: &Path, reference: &str) {
        let k = key.display().to_string();
        let (code, out, err) = self.run(w, &w.path("."), &["sign", "--key", &k, "--tlog-upload=false", "--use-signing-config=false", "--allow-http-registry", "--yes", reference]);
        assert_eq!(code, 0, "{out}{err}");
    }

    /// The environment of a run of ritsu that runs the real cosign: the same, and the PATH of the
    /// test's process (where cosign is), not the fake one's.
    fn ritsu_env(&self) -> Vec<(String, String)> {
        let mut e = self.env.clone();
        e.push(("PATH".to_string(), std::env::var("PATH").unwrap_or_default()));
        e
    }
}

fn pairs(e: &[(String, String)]) -> Vec<(&str, &str)> {
    e.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
}

/// The real cosign (the tools level), with no network: a key pair of the test's own, made with an
/// empty password; tax signed with it, with no transparency log, at the test's registry; a fetch
/// whose line names the public key (`tlog off`) passes, and one whose line names another key stops
/// (exit 1) with cosign's words. A proxy that answers nothing stands for the network beyond the
/// loopback, and nothing reaches it.
#[test]
fn the_real_cosign_signs_and_checks_a_module_offline() {
    if !ready(Need::Cosign, || on_path("cosign").is_some(), "cosign is not on the PATH") {
        return;
    }
    let w = World::new("oci-cosign-real");
    let host = w.host();
    let net = sink();
    let cosign = Cosign::new(&w, &net);
    cosign.keys(&w, &w.path("keys"), "cosign");
    cosign.keys(&w, &w.path("keys"), "other");
    w.ok("tax", &["mod", "push"]);
    let (tax, _) = w.pushed("acme/tax", "1.0.0");
    cosign.sign(&w, &w.path("keys/cosign.key"), &format!("{host}/acme/tax@{tax}"));
    // the consumer names the public key
    let line = format!("signature \"{host}/acme/\" key \"keys/acme.pub\" tlog off");
    let m = w.t.read("pricing/ritsu.mod");
    w.t.write("pricing/ritsu.mod", format!("{m}\n{line}\n"));
    std::fs::create_dir_all(w.path("pricing/keys")).unwrap();
    std::fs::copy(w.path("keys/cosign.pub"), w.path("pricing/keys/acme.pub")).unwrap();
    w.ok("pricing", &["mod", "lock"]);
    let env = cosign.ritsu_env();
    let (code, out, err) = w.run_env("pricing", &["mod", "fetch"], &pairs(&env));
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, format!("fetched {host}/acme/tax 1.0.0 ({tax})\n  checked its signature against `{line}`\nfetched 1 module\n"));
    let (code, out, err) = w.run_env("pricing", &["check"], &pairs(&env));
    assert_eq!(code, 0, "{out}{err}");
    // another key: cosign says no, and nothing is put in the store
    std::fs::copy(w.path("keys/other.pub"), w.path("pricing/keys/acme.pub")).unwrap();
    let store = w.path("store-other");
    let mut other = env.clone();
    other.push((store::VARIABLE.to_string(), store.display().to_string()));
    let (code, out, err) = w.run_env("pricing", &["mod", "fetch"], &pairs(&other));
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(err.contains(&format!("does not pass `{line}`; cosign said:")), "{err}");
    assert_eq!(entries(&store).iter().filter(|(_, files)| *files).count(), 0);
    assert_eq!(*net.seen.lock().unwrap(), Vec::<String>::new());
}

/// A registry of the platforms level (`RITSU_TEST_REGISTRY`, the distribution's reference
/// implementation in CI): a module pushed and fetched back, its digests the same, pushed again as
/// already there; and, with cosign on the PATH, signed there with a key of the test's own and
/// checked by the fetch.
#[test]
fn a_registry_of_the_platforms_level_takes_a_module_and_gives_it_back() {
    if !need(Need::Registry) {
        return;
    }
    let Some(host) = std::env::var("RITSU_TEST_REGISTRY").ok().filter(|h| !h.is_empty()) else {
        ritsu_testkit::skip::skip("RITSU_TEST_REGISTRY is not set");
        return;
    };
    let w = World::new("oci-platforms");
    let unique = format!("tax-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    let name = format!("{host}/acme/{unique}");
    w.t.write("real/ritsu.mod", format!("module {name}\nversion 1.0.0\nritsu >=0.36.0\n"));
    ritsu_testkit::tmp::copy_dir(&here().join("tests/projects/modules/tax/rules"), &w.path("real/rules"));
    let out = w.ok("real", &["mod", "push"]);
    assert!(out.starts_with(&format!("pushed {name} 1.0.0\n")), "{out}");
    let digest = out.lines().find_map(|l| l.trim().strip_prefix("digest: ")).unwrap().to_string();
    assert!(w.ok("real", &["mod", "push"]).starts_with(&format!("{name} 1.0.0 is pushed already")));
    w.t.write("user/ritsu.mod", format!("module example.com/acme/user\nversion 0.1.0\nrequire tax oci \"{name}\" 1.0.0\n"));
    w.ok("user", &["mod", "lock"]);
    assert_eq!(w.ok("user", &["mod", "fetch"]), format!("fetched {name} 1.0.0 ({digest})\nfetched 1 module\n"));
    assert_eq!(w.ok("user", &["mod", "versions", "tax"]), "1.0.0\n");
    if on_path("cosign").is_none() {
        ritsu_testkit::skip::skip("cosign is not on the PATH: the signature at the registry of the platforms level is not checked");
        return;
    }
    let net = sink();
    let cosign = Cosign::new(&w, &net);
    cosign.keys(&w, &w.path("user/keys"), "acme");
    cosign.sign(&w, &w.path("user/keys/acme.key"), &format!("{name}@{digest}"));
    let line = format!("signature \"{name}\" key \"keys/acme.pub\" tlog off");
    let m = w.t.read("user/ritsu.mod");
    w.t.write("user/ritsu.mod", format!("{m}{line}\n"));
    let env = cosign.ritsu_env();
    let (code, out, err) = w.run_env("user", &["mod", "fetch"], &pairs(&env));
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, format!("checked the signature of {name} 1.0.0 against `{line}`\nevery module is here already (1 module)\n"));
    assert_eq!(*net.seen.lock().unwrap(), Vec::<String>::new());
}
