//! `ritsu mod site` (DESIGN 17.17; PLAN F.2), against a registry the test starts
//! (`ritsu_testkit::registry`: curl asks it on the loopback, and nothing else), with the modules of
//! the example (DESIGN 17.26) on it, two versions of each: tax 1.0.0 and 1.1.0 (the reduced rate
//! raised), pricing 1.0.0 and 1.1.0, and shop 0.1.0 and 0.2.0 (a line of a basket takes fewer).
//!
//! - The site of the three, in English and in Japanese: its pages held to golden files (what is
//!   between `<main>` and `</main>`, with the registry's address, the digests, the sizes of the
//!   files that name the registry, and the version of ritsu masked; `RITSU_BLESS=1` writes them),
//!   the files it writes, the pages each language's `doc` draws, and `module.json`, whose files are
//!   a project that `ritsu check` passes with no store, and whose copy of the store `ritsu mod
//!   vendor --check` holds to the lock.
//! - The list's `signature` lines check every version, with a cosign of the test's own that writes
//!   down how it is called; when it says no, each version is said unverified on its pages and on
//!   stderr, and the command exits 1.
//! - Working trees: one more version of a module of the list (after the registry's), one whose
//!   version the registry has (said on stderr, and the registry's shown), one that requires a
//!   module by a path (no button, no `module.json`), a file that is not UTF-8 (base64 in
//!   `module.json`), one too big for a link (its button says so when pressed, which the Chrome test
//!   of tests/playground.rs presses), and the playground the button opens: the command line's,
//!   else the list's, else the site's in the language of the run.
//! - What stops the command with nothing written (exit 2): a list that cannot be read, each of its
//!   mistakes said at its line and column; a list that is not there; no `--out`; and a registry
//!   that does not answer.

use ritsu_base::modules::store;
use ritsu_testkit::TempDir;
use ritsu_testkit::registry::{self as reg, Registry};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const LANG_VARS: [&str; 10] = ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG", "KANAME_LANG"];

/// A cosign of the test's own: it writes the arguments it is called with, a line each call, and
/// says no when `FAKE_COSIGN_EXIT` is not 0.
const FAKE_COSIGN: &str = "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$FAKE_COSIGN_LOG\"\nif [ -n \"$FAKE_COSIGN_EXIT\" ] && [ \"$FAKE_COSIGN_EXIT\" != 0 ]; then\n  echo 'Error: no matching signatures (the fake cosign of the test)' >&2\n  exit \"$FAKE_COSIGN_EXIT\"\nfi\n";

/// `path` made executable.
fn executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn said(o: Output) -> (i32, String, String) {
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn golden(path: &str, got: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(&here().join(path), got) {
        failures.push(e);
    }
}

/// The file `rel` of `t`, with every `from` replaced by `to` (it must be there).
fn edit(t: &TempDir, rel: &str, from: &str, to: &str) {
    let text = t.read(rel);
    assert!(text.contains(from), "{rel} has no `{from}`");
    t.write(rel, text.replace(from, to));
}

/// The modules of the example on the registry `host`, as tests/modules_oci.rs names them: the
/// module named there, each `require` from it, and a `ritsu >=` line under the version.
fn on_registry(t: &TempDir, dir: &str, host: &str) {
    let rel = format!("{dir}/ritsu.mod");
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

/// The example on a registry the test starts, two versions of each module, pushed with the store
/// `pushed/`; the fake cosign first on the PATH, and a Docker config of the test's own, with
/// nothing in it.
struct Acme {
    t: TempDir,
    r: Registry,
}

impl Acme {
    fn new(what: &str) -> Acme {
        let r = Registry::start();
        let t = TempDir::new(what);
        let host = r.host.clone();
        let from = here().join("tests/projects/modules");
        for (src, dst) in [("tax", "tax"), ("tax", "tax-1.1.0"), ("versions/pricing-1.0.0", "pricing-1.0.0"), ("pricing", "pricing"), ("shop", "shop"), ("shop", "shop-0.2.0")] {
            ritsu_testkit::tmp::copy_dir(&from.join(src), &t.path().join(dst));
            on_registry(&t, dst, &host);
        }
        // tax 1.1.0 raises the reduced rate; shop 0.2.0 takes up to 5 of a line of a basket
        edit(&t, "tax-1.1.0/ritsu.mod", "version 1.0.0", "version 1.1.0");
        edit(&t, "tax-1.1.0/rules/vat_rate.rule", "5%", "6%");
        edit(&t, "shop-0.2.0/ritsu.mod", "version 0.1.0", "version 0.2.0");
        edit(&t, "shop-0.2.0/checkout/basket.kn", "range >=1 <=10", "range >=1 <=5");
        std::fs::create_dir_all(t.path().join("docker")).unwrap();
        executable(&t.write("bin/cosign", FAKE_COSIGN));
        let a = Acme { t, r };
        let pushed = a.path("pushed");
        for (dir, steps) in [
            ("tax", &["push"][..]),
            ("tax-1.1.0", &["push"][..]),
            ("pricing-1.0.0", &["lock", "fetch", "push"][..]),
            ("pricing", &["lock", "fetch", "push"][..]),
            ("shop", &["lock", "fetch", "push"][..]),
            ("shop-0.2.0", &["lock", "fetch", "push"][..]),
        ] {
            for step in steps {
                let (code, out, err) = a.run_in(dir, &pushed, &["mod", step], &[]);
                assert_eq!(code, 0, "ritsu mod {step} in {dir}:\n{out}{err}");
            }
        }
        a
    }

    fn host(&self) -> String {
        self.r.host.clone()
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.t.path().join(rel)
    }

    fn command(&self, dir: &str, store: &Path, args: &[&str], env: &[(&str, &str)]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(self.path(dir)).args(args);
        c.env(store::VARIABLE, store).env("DOCKER_CONFIG", self.path("docker")).env("FAKE_COSIGN_LOG", self.path("cosign.log"));
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

    /// ritsu, run in the directory `dir` of the test with the store at `store`, and `env` over the
    /// rest.
    fn run_in(&self, dir: &str, store: &Path, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
        said(self.command(dir, store, args, env).output().expect("could not run ritsu"))
    }

    /// `ritsu mod site <list> --out <out>` in the test's directory, with the store `store-<out>`
    /// (empty, so that the site fetches what it shows), and `more` after.
    fn site(&self, list: &str, out: &str, more: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
        let mut args = vec!["mod", "site", list, "--out", out];
        args.extend(more);
        self.run_in("", &self.path(&format!("store-{out}")), &args, env)
    }

    /// The calls of the fake cosign so far.
    fn cosign_calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.path("cosign.log")).unwrap_or_default().lines().map(str::to_string).collect()
    }

    /// The digest of the OCI manifest of the version `tag` of `repository` in the registry.
    fn digest(&self, repository: &str, tag: &str) -> String {
        self.r.tag(repository, tag).unwrap_or_else(|| panic!("the registry has no {repository}:{tag}"))
    }
}

/// The text with each digest (`sha256:` and 64 hexadecimal digits) masked, and each head of one
/// (`sha256:` and 12).
fn digests_masked(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("sha256:") {
        let after = &rest[at + 7..];
        let hex = after.chars().take_while(|c| c.is_ascii_hexdigit()).count();
        out.push_str(&rest[..at]);
        match hex {
            64 => out.push_str("sha256:<digest>"),
            12 => out.push_str("sha256:<head>"),
            _ => out.push_str("sha256:"),
        }
        rest = if hex == 64 || hex == 12 { &after[hex..] } else { after };
    }
    out.push_str(rest);
    out
}

/// A page as the golden files hold it: what is between `<main>` and `</main>`, with the registry's
/// address (as it is written, and as a link's path writes it), the digests, the version of ritsu,
/// and the sizes of `ritsu.mod` and `ritsu.lock` (which name the registry, whose port changes from
/// run to run) masked.
fn masked(page: &str, host: &str) -> String {
    let from = page.find("<main>").expect("the page has its <main>");
    let to = page.find("</main>").expect("the page has its </main>") + "</main>".len();
    let text = page[from..to].replace(host, "<registry>").replace(&host.replace(':', "%3A"), "<registry>").replace(env!("CARGO_PKG_VERSION"), "<version>");
    let text = digests_masked(&text);
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        if line.contains("<code>ritsu.mod</code></a></td>") || line.contains("<code>ritsu.lock</code></a></td>") {
            let cut = line.trim_end_matches('\n').trim_end_matches("</td></tr>");
            let cell = cut.rfind("<td>").unwrap();
            out.push_str(&format!("{}<td>&lt;bytes&gt;</td></tr>\n", &cut[..cell]));
        } else {
            out.push_str(line);
        }
    }
    out
}

/// Every file under `dir`, by its path from it, in order.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p.strip_prefix(dir).unwrap().to_string_lossy().into_owned());
            }
        }
    }
    out.sort();
    out
}

/// The files `module.json` holds, each with its bytes.
fn opened(json: &str) -> Vec<(String, Vec<u8>)> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|e| panic!("{e}: {json}"));
    v.as_array()
        .expect("module.json is a list")
        .iter()
        .map(|item| {
            let path = item[0].as_str().expect("a path").to_string();
            let bytes = match &item[1] {
                serde_json::Value::String(s) => s.as_bytes().to_vec(),
                other => reg::base64_decode(other["base64"].as_str().expect("text, or base64")).expect("base64"),
            };
            (path, bytes)
        })
        .collect()
}

/// The list of the three modules on the registry `host`, each version checked by the line for the
/// registry's acme/.
fn list_of_three(host: &str) -> String {
    format!(
        "# The modules of the Acme shop.\ntitle \"Acme modules\"\ndescription \"The modules the Acme shop publishes\"\n\nmodule \"{host}/acme/tax\"\nmodule \"{host}/acme/pricing\"\nmodule \"{host}/acme/shop\"\n\nsignature \"{host}/acme/\" by matching \"^https://github[.]com/acme/\" issuer \"https://token.actions.githubusercontent.com\"\n"
    )
}

/// The site of the three modules, two versions each, in English and in Japanese: what it says, the
/// files it writes, its pages held to golden files, the pages each language's `doc` draws (with the
/// way back to the version), cosign asked for every version, and `module.json`: the version's files
/// and the modules its lock pins as a copy of the store, which `ritsu check` passes with an empty
/// store and `ritsu mod vendor --check` holds to the lock.
#[test]
fn the_site_of_the_example_held_to_golden_files() {
    let a = Acme::new("mod-site");
    let host = a.host();
    a.t.write("modules.list", list_of_three(&host));
    let mut failures = Vec::new();
    let mut written: Option<Vec<String>> = None;
    for (lang, said_out) in [("en", "ritsu mod site: wrote 34 pages of 3 modules in site-en\n"), ("ja", "ritsu mod site: モジュール 3 個のページを 34 枚、site-ja に書きました\n")] {
        let out = format!("site-{lang}");
        let (code, stdout, err) = a.site("modules.list", &out, &["--lang", lang], &[]);
        assert_eq!((code, stdout.as_str(), err.as_str()), (0, said_out, ""), "{lang}");
        let site = a.path(&out);
        // the same files in each language
        let files: Vec<String> = files_under(&site).iter().map(|f| f.replace(&host.replace(':', "%3A"), "<registry>").replace(&host, "<registry>")).collect();
        match &written {
            None => {
                golden("tests/golden/modules/site-files.txt", &(files.join("\n") + "\n"), &mut failures);
                written = Some(files);
            }
            Some(w) => assert_eq!(&files, w, "{lang}"),
        }
        let page = |rel: &str| std::fs::read_to_string(site.join(rel.replace("<registry>", &host))).unwrap_or_else(|e| panic!("{rel}: {e}"));
        for (rel, name) in [
            ("index.html", "index"),
            ("<registry>/acme/pricing/index.html", "pricing"),
            ("<registry>/acme/pricing/1.1.0/index.html", "pricing-1.1.0"),
            ("<registry>/acme/tax/1.1.0/index.html", "tax-1.1.0"),
            ("<registry>/acme/shop/0.2.0/index.html", "shop-0.2.0"),
            ("<registry>/acme/shop/0.2.0/files/orders/scenarios/charged.json.html", "shop-0.2.0-charged"),
        ] {
            golden(&format!("tests/golden/modules/site-{name}.{lang}.html"), &masked(&page(rel), &host), &mut failures);
        }
        // the pages the languages draw, each with the way back to its version
        for (rel, tool) in [("checkout/basket.kn", "kaname"), ("orders/place_order.flow", "dandori")] {
            let p = page(&format!("<registry>/acme/shop/0.2.0/files/{rel}.html"));
            assert!(p.starts_with("<!doctype html>"), "{rel} ({lang})");
            let up = "../".repeat(rel.matches('/').count() + 1);
            assert!(p.contains(&format!("<a href=\"{up}index.html\">0.2.0</a></nav>\n")), "{rel} ({lang}) has no way back to its version:\n{}", &p[..p.len().min(3000)]);
            assert!(p.contains(&format!("{tool} ")) || p.contains(tool), "{rel} ({lang})");
        }
    }
    // every version checked against the list's line, once in each run
    let refs: Vec<String> = [("tax", "1.0.0"), ("tax", "1.1.0"), ("pricing", "1.0.0"), ("pricing", "1.1.0"), ("shop", "0.1.0"), ("shop", "0.2.0")]
        .iter()
        .map(|(m, v)| format!("{host}/acme/{m}@{}", a.digest(&format!("acme/{m}"), v)))
        .collect();
    let calls = a.cosign_calls();
    for r in &refs {
        let n = calls.iter().filter(|c| c.ends_with(&format!(" {r}"))).count();
        assert_eq!(n, 2, "{r} in {calls:#?}");
        assert!(calls.contains(&format!("verify --certificate-identity-regexp ^https://github[.]com/acme/ --certificate-oidc-issuer https://token.actions.githubusercontent.com --allow-http-registry {r}")), "{calls:#?}");
    }
    // module.json of shop 0.2.0: its files, and pricing 1.1.0 and tax 1.0.0 as a copy of the store
    let json = std::fs::read_to_string(a.path(&format!("site-en/{host}/acme/shop/0.2.0/module.json"))).unwrap();
    assert_eq!(json, std::fs::read_to_string(a.path(&format!("site-ja/{host}/acme/shop/0.2.0/module.json"))).unwrap());
    let files = opened(&json);
    let (pricing, tax) = (a.digest("acme/pricing", "1.1.0"), a.digest("acme/tax", "1.0.0"));
    let paths: Vec<String> = files.iter().map(|(p, _)| p.replace(&pricing[7..], "<pricing>").replace(&tax[7..], "<tax>")).collect();
    golden("tests/golden/modules/site-module-json.txt", &(paths.join("\n") + "\n"), &mut failures);
    for (p, bytes) in &files {
        if !p.starts_with(".ritsu/") {
            assert_eq!(bytes, &std::fs::read(a.path(&format!("shop-0.2.0/{p}"))).unwrap(), "{p}");
        }
    }
    let project = TempDir::new("mod-site-opened");
    for (p, bytes) in &files {
        project.write(p, bytes);
    }
    let empty = project.path().join("no-store");
    for args in [&["check", "."][..], &["mod", "vendor", "--check"][..]] {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(project.path()).args(args).env(store::VARIABLE, &empty);
        for v in LANG_VARS {
            c.env_remove(v);
        }
        let (code, out, err) = said(c.output().unwrap());
        assert_eq!(code, 0, "ritsu {args:?} over module.json's files:\n{out}{err}");
    }
    assert!(!empty.exists(), "a check of module.json's files made the store");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A cosign that says no: every version is said unverified, on stderr (a line each, with the first
/// module of its tree that does not pass) and on the pages of the list and of its module, which link
/// to no page of it; nothing of it is written but those, and the command exits 1. In each language.
#[test]
fn a_version_that_cannot_be_verified_exits_1() {
    let a = Acme::new("mod-site-unverified");
    let host = a.host();
    a.t.write("modules.list", list_of_three(&host));
    let line = format!("signature \"{host}/acme/\" by matching \"^https://github[.]com/acme/\" issuer \"https://token.actions.githubusercontent.com\"");
    for (lang, head, why) in [
        ("en", "error: `{m}` {v} could not be verified: ", format!("the signature of `{{m}}` {{v}} does not pass `{line}`; cosign said:\nError: no matching signatures (the fake cosign of the test)")),
        ("ja", "エラー: `{m}` {v} を確かめられません。", format!("`{{m}}` {{v}} の署名が `{line}` で確かめられません。cosign の出力：\nError: no matching signatures (the fake cosign of the test)")),
    ] {
        let out = format!("site-{lang}");
        let (code, stdout, err) = a.site("modules.list", &out, &["--lang", lang], &[("FAKE_COSIGN_EXIT", "1")]);
        assert_eq!(code, 1, "{stdout}{err}");
        // a version of shop stops at the first module of its tree that does not pass: pricing
        for (m, v, first) in [("tax", "1.0.0", None), ("tax", "1.1.0", None), ("pricing", "1.0.0", None), ("pricing", "1.1.0", None), ("shop", "0.1.0", Some(("pricing", "1.1.0"))), ("shop", "0.2.0", Some(("pricing", "1.1.0")))] {
            let (fm, fv) = first.unwrap_or((m, v));
            let (m, fm) = (format!("{host}/acme/{m}"), format!("{host}/acme/{fm}"));
            let said = format!("{}{}", head.replace("{m}", &m).replace("{v}", v), why.replace("{m}", &fm).replace("{v}", fv));
            assert!(err.contains(&said), "{lang}: no\n{said}\nin\n{err}");
        }
        assert_eq!(err.lines().filter(|l| l.starts_with(head.split('`').next().unwrap())).count(), 6, "{err}");
        let site = a.path(&out);
        let files = files_under(&site);
        assert_eq!(files.len(), 4, "{files:#?}");
        let index = std::fs::read_to_string(site.join("index.html")).unwrap();
        let bad = if lang == "en" { "<span class=\"bad\">a version could not be verified</span>" } else { "<span class=\"bad\">確かめられない版があります</span>" };
        assert_eq!(index.matches(bad).count(), 3, "{index}");
        let tax = std::fs::read_to_string(site.join(format!("{host}/acme/tax/index.html"))).unwrap();
        let not = if lang == "en" { "<span class=\"bad\">not verified: " } else { "<span class=\"bad\">確かめられません。" };
        assert_eq!(tax.matches(not).count(), 2, "{tax}");
        assert!(!tax.contains("./1.0.0/index.html") && !tax.contains("./1.1.0/index.html"), "{tax}");
    }
}

/// Working trees beside the registry's modules: shop 0.3.0 (one more version of shop, after the
/// registry's two, compared with 0.2.0; its lock's modules fetched and said; a file in Shift_JIS,
/// in `module.json` as base64), shop 0.1.0 (the registry has it: said, and the registry's shown),
/// the example as it is committed (it requires its modules by paths: no button, no `module.json`),
/// and a module too big for a link (the button and what it says when it is pressed). The
/// playground the buttons open: the command line's, else the list's.
#[test]
fn working_trees_and_the_playground_they_open_in() {
    let a = Acme::new("mod-site-local");
    let host = a.host();
    ritsu_testkit::tmp::copy_dir(&a.path("shop-0.2.0"), &a.path("work/shop-next"));
    edit(&a.t, "work/shop-next/ritsu.mod", "version 0.2.0", "version 0.3.0");
    let sjis: &[u8] = b"\x93\xfa\x95\x74,\x8b\x78\x93\xfa\n2026-01-01,\x8c\xb3\x93\xfa\n";
    a.t.write("work/shop-next/notes/holidays.csv", sjis);
    let (code, out, err) = a.run_in("work/shop-next", &a.path("pushed"), &["mod", "lock"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let from = here().join("tests/projects/modules");
    for m in ["tax", "pricing", "shop"] {
        ritsu_testkit::tmp::copy_dir(&from.join(m), &a.path(&format!("work/paths/{m}")));
    }
    // a megabyte that does not compress
    let mut noise = Vec::with_capacity(1_000_000);
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    while noise.len() < 1_000_000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        noise.extend_from_slice(&x.to_le_bytes());
    }
    a.t.write("work/big/ritsu.mod", "module example.com/acme/big\nversion 0.1.0\ndescription \"A module too big to open in the playground\"\n");
    a.t.write("work/big/data/noise.bin", &noise);
    a.t.write(
        "work/modules.list",
        format!("title \"Acme modules\"\nplayground \"https://example.com/play/\"\nlocal \"shop-next\"\nmodule \"{host}/acme/shop\"\nlocal \"../shop\"\nlocal \"paths/shop\"\nlocal \"big\"\n"),
    );
    let (pricing, tax) = (a.digest("acme/pricing", "1.1.0"), a.digest("acme/tax", "1.0.0"));
    let (code, stdout, err) = a.site("work/modules.list", "local", &[], &[]);
    assert_eq!(
        (code, stdout.as_str(), err.as_str()),
        (
            0,
            "ritsu mod site: wrote 35 pages of 3 modules in local\n",
            format!(
                "fetched {host}/acme/pricing 1.1.0 ({pricing})\nfetched {host}/acme/tax 1.0.0 ({tax})\nnote: `{host}/acme/shop` 0.1.0 is on its registry already, so the site shows the registry's and not the working tree's\n"
            )
            .as_str()
        )
    );
    let site = a.path("local");
    let page = |rel: &str| std::fs::read_to_string(site.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    // shop: the registry's two versions and the working tree's, newest first
    let shop = page(&format!("{host}/acme/shop/index.html"));
    let rows: Vec<&str> = shop.lines().filter(|l| l.starts_with("<tr><td><a href=\"./")).collect();
    assert_eq!(rows.len(), 3, "{shop}");
    assert!(rows[0].contains(">0.3.0</a>") && rows[0].contains("<span class=\"warn\">not published yet</span>"), "{shop}");
    assert!(rows[1].contains(">0.2.0</a>") && rows[2].contains(">0.1.0</a>"), "{shop}");
    let next = page(&format!("{host}/acme/shop/0.3.0/index.html"));
    assert!(next.contains("<h2>Changes since 0.2.0</h2>"), "{next}");
    assert!(next.contains("data-playground=\"https://example.com/play/\""), "{next}");
    // module.json: the file in Shift_JIS as its bytes, and the copy of the store
    let files = opened(&page(&format!("{host}/acme/shop/0.3.0/module.json")));
    assert!(files.contains(&("notes/holidays.csv".to_string(), sjis.to_vec())));
    let raw = page(&format!("{host}/acme/shop/0.3.0/module.json"));
    assert!(raw.contains(&format!("[\"notes/holidays.csv\",{{\"base64\":\"{}\"}}]", reg::base64_encode(sjis))), "{raw}");
    for d in [&pricing, &tax] {
        let at = format!(".ritsu/modules/sha256/{}/", &d[7..]);
        for f in ["manifest.json", "ritsu.mod", "record.json"] {
            assert!(files.iter().any(|(p, _)| *p == format!("{at}{f}")), "module.json has no {at}{f}");
        }
    }
    let csv = page(&format!("{host}/acme/shop/0.3.0/files/notes/holidays.csv.html"));
    assert!(csv.contains(&format!("This file is not UTF-8 text ({} bytes, <code>sha256:{}</code>)", sjis.len(), reg::sha256_hex(sjis))), "{csv}");
    // the tree of paths: no button, no module.json
    let paths = page("example.com/acme/shop/0.1.0/index.html");
    assert!(paths.contains("This working tree requires a module by a path, so the playground cannot open it"), "{paths}");
    assert!(!paths.contains("<button") && !site.join("example.com/acme/shop/0.1.0/module.json").exists());
    // the big module: the button, and what it says when the link would be too long
    let big = page("example.com/acme/big/0.1.0/index.html");
    assert!(big.contains("data-big=\"Too big to open in the playground\""), "{big}");
    assert!(big.contains("if (packed.length > 1000000) {"), "{big}");
    assert!(site.join("example.com/acme/big/0.1.0/module.json").metadata().unwrap().len() > 1_300_000);
    // the command line's playground over the list's
    let (code, _, err) = a.site("work/modules.list", "local-flag", &["--playground", "http://127.0.0.1:9/ritsu/play/", "--lang", "ja"], &[]);
    assert_eq!(code, 0, "{err}");
    let next = std::fs::read_to_string(a.path(&format!("local-flag/{host}/acme/shop/0.3.0/index.html"))).unwrap();
    assert!(next.contains("data-playground=\"http://127.0.0.1:9/ritsu/play/\""), "{next}");
    assert!(err.contains(&format!("注: `{host}/acme/shop` 0.1.0 はレジストリにもうあるので、作業ツリーではなくレジストリの版を載せました\n")), "{err}");
}

/// What stops the command, with nothing written (exit 2): a list with mistakes (each said at its
/// line and its column, held to golden files), a list that is not there, a list with no module, a
/// playground that is not https on the command line, no `--out`, and a registry that does not
/// answer.
#[test]
fn what_stops_the_site_writes_nothing() {
    let t = TempDir::new("mod-site-stops");
    t.write("shop/ritsu.mod", "module example.com/acme/shop\nversion 0.1.0\n");
    t.write(
        "bad.list",
        "title \"One\"\ntitle \"Two\"\nfrob \"x\"\nmodule \"tax\"\nmodule \"example.com/acme/shop\"\nmodule \"example.com/acme/shop\"\nlocal \"nowhere\"\nlocal \"/abs\"\nlocal \"shop\"\nlocal \"./shop\"\nplayground \"ftp://example.com/play/\"\nplayground \"https://example.com/play/?a=1\"\nsignature \"example.com/acme/\" frob\ndescription\nmodule \"example.com/acme/\"\n",
    );
    t.write("empty.list", "# nothing yet\ntitle \"Nothing\"\n");
    t.write("ok.list", "local \"shop\"\n");
    let run = |args: &[&str]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(t.path()).args(args).env(store::VARIABLE, t.path().join("store"));
        for v in LANG_VARS {
            c.env_remove(v);
        }
        said(c.output().unwrap())
    };
    let mut failures = Vec::new();
    for lang in ["en", "ja"] {
        let (code, out, err) = run(&["mod", "site", "bad.list", "--out", "out", "--lang", lang]);
        assert_eq!((code, out.as_str()), (2, ""), "{err}");
        golden(&format!("tests/golden/modules/site-bad-list.{lang}.txt"), &err, &mut failures);
    }
    assert_eq!(run(&["mod", "site", "nope.list", "--out", "out"]), (2, String::new(), "error: cannot read nope.list (No such file or directory (os error 2)); nothing was written\n".into()));
    assert_eq!(run(&["mod", "site", "empty.list", "--out", "out"]), (2, String::new(), "error: empty.list has no `module` line and no `local` line\nerror: the list cannot be read; nothing was written\n".into()));
    assert_eq!(
        run(&["mod", "site", "ok.list", "--out", "out", "--playground", "http://example.com/play/"]),
        (2, String::new(), "error: the playground's URL `http://example.com/play/` is an https URL (or an http one on this machine) with no user, no query and no `#`\n".into())
    );
    assert_eq!(run(&["mod", "site", "ok.list"]), (2, String::new(), "error: say where the site goes with `--out <dir>`\n".into()));
    assert_eq!(run(&["mod", "site", "ok.list", "--lang", "ja"]), (2, String::new(), "エラー: 書く先のディレクトリを `--out <ディレクトリ>` で渡してください\n".into()));
    assert!(!t.path().join("out").exists());
    // a registry that does not answer: what curl said, after its tries
    let r = Registry::start();
    let host = r.host.clone();
    drop(r);
    t.write("down.list", format!("module \"{host}/acme/tax\"\n"));
    let (code, out, err) = run(&["mod", "site", "down.list", "--out", "out"]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    let head = format!("error: cannot list the versions of `{host}/acme/tax`: cannot reach `GET http://{host}/v2/acme/tax/tags/list");
    assert!(err.starts_with(&head) && err.ends_with("; nothing was written\n"), "{err}");
    assert!(!t.path().join("out").exists());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
