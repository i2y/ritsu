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
//! - A module with requirements (`common/fines.rs`): the reason of its newest version on the list, and
//!   why it changed on its page, in the words of its `.req`, with links to the files.
//! - What stops the command with nothing written (exit 2): a list that cannot be read, each of its
//!   mistakes said at its line and column; a list that is not there; no `--out`; and a registry
//!   that does not answer.

#[path = "common/fines.rs"]
mod fines;

use ritsu_base::modules::store;
use ritsu_testkit::registry::{self as reg, Registry};
use ritsu_testkit::{Need, TempDir, need, skip};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn root() -> PathBuf {
    here().join("../..")
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
        if line.contains(">ritsu.mod</a></td>") || line.contains(">ritsu.lock</a></td>") {
            let cut = line.trim_end_matches('\n').trim_end_matches("</td></tr>");
            let cell = cut.rfind(">").unwrap();
            out.push_str(&format!("{}>&lt;bytes&gt;</td></tr>\n", &cut[..cell]));
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

/// The text of a file's code on its page: the element's, with its tags gone and its characters as
/// they are.
fn code_text(page: &str) -> String {
    let inner = page.split("<pre class=\"source lines").nth(1).expect("a page with code").split_once("<code>").unwrap().1.split("</code></pre>").next().unwrap();
    let bare: String = inner.split('<').enumerate().map(|(i, part)| if i == 0 { part } else { part.split_once('>').unwrap().1 }).collect();
    bare.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
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
    for (lang, said_out) in [("en", "ritsu mod site: wrote 44 pages of 3 modules in site-en\n"), ("ja", "ritsu mod site: モジュール 3 個のページを 44 枚、site-ja に書きました\n")] {
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
            assert!(p.contains(&format!("<a href=\"{up}index.html\">0.2.0</a></span>")), "{rel} ({lang}) has no way back to its version:\n{}", &p[..p.len().min(3000)]);
            assert!(p.contains(&format!("{up}files/{}.html\" selected>", rel)), "{rel} ({lang}) is not chosen in its menu of files");
            assert!(p.contains(&format!("{tool} ")) || p.contains(tool), "{rel} ({lang})");
            // the switch to its code, whose page says the file, and whose menu stays with code
            assert!(p.contains(&format!("{up}files/{rel}.html\" aria-current=\"page\">")) && p.contains(&format!("{up}code/{rel}.html\">")), "{rel} ({lang}) has no switch");
            let c = page(&format!("<registry>/acme/shop/0.2.0/code/{rel}.html"));
            assert!(c.contains(&format!("{up}code/{rel}.html\" aria-current=\"page\">")) && c.contains(&format!("{up}files/{rel}.html\">")), "{rel} ({lang}): the code has no switch");
            assert!(c.contains(&format!("{up}code/{rel}.html\" selected>")) && c.contains(&format!("{up}files/ritsu.mod.html\">ritsu.mod</option>")), "{rel} ({lang}): its menu of files");
            let module_json: serde_json::Value = serde_json::from_str(&page("<registry>/acme/shop/0.2.0/module.json")).unwrap();
            let file = module_json.as_array().unwrap().iter().find(|i| i[0] == rel).expect("the file is in module.json")[1].as_str().unwrap().to_string();
            assert_eq!(code_text(&c), file, "{rel} ({lang}): the text of its code is the file");
            assert!(page("<registry>/acme/shop/0.2.0/index.html").contains(&format!("href=\"./code/{rel}.html\"")) || page("<registry>/acme/shop/0.2.0/index.html").contains(&format!("href=\"code/{rel}.html\"")), "{rel} ({lang}): the tab of files leads to its code");
        }
        // a file with no page but its code has no switch, and nothing under code/
        let data = "orders/scenarios/charged.json";
        assert!(!page(&format!("<registry>/acme/shop/0.2.0/files/{data}.html")).contains("rm-switch"), "{lang}");
        assert!(page(&format!("<registry>/acme/shop/0.2.0/files/{data}.html")).contains("<pre class=\"source lines"), "{lang}");
        assert!(!site.join(host.replace(':', "%3A")).join(format!("acme/shop/0.2.0/code/{data}.html")).exists() && !site.join(&host).join(format!("acme/shop/0.2.0/code/{data}.html")).exists(), "{lang}");
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
        assert_eq!(files.len(), 8, "{files:#?}");
        let index = std::fs::read_to_string(site.join("index.html")).unwrap();
        let bad = if lang == "en" { "<span class=\"bad\">a version could not be verified</span>" } else { "<span class=\"bad\">確かめられない版があります</span>" };
        assert_eq!(index.matches(bad).count(), 3, "{index}");
        let tax = std::fs::read_to_string(site.join(format!("{host}/acme/tax/index.html"))).unwrap();
        let not = if lang == "en" { "<span class=\"bad\">not verified: " } else { "<span class=\"bad\">確かめられません。" };
        assert_eq!(tax.matches(not).count(), 2, "{tax}");
        assert!(!tax.contains("./1.0.0/index.html") && !tax.contains("./1.1.0/index.html"), "{tax}");
    }
}

/// A version is checked with its claims read and not run, wherever it comes from: running them would
/// run its author's programs on the machine that builds the site. A version with no `.geas` file says
/// what it always said, one that fails says what `ritsu check` printed, and its Changes tab says in
/// words what a rule now asks of its callers and where a value comes from.
#[test]
fn a_version_is_checked_with_its_claims_read_and_not_run() {
    let a = Acme::new("mod-site-claims");
    let host = a.host();
    let mark = a.path("ran");
    let claim = "target mark {\n  run \"sh leave_a_mark.sh\"\n}\n\nclaim \"it leaves a mark\" {\n  when mark.run()\n  then exit is 0\n}\n";
    ritsu_testkit::tmp::copy_dir(&a.path("tax-1.1.0"), &a.path("tax-1.2.0"));
    edit(&a.t, "tax-1.2.0/ritsu.mod", "version 1.1.0", "version 1.2.0");
    a.t.write("tax-1.2.0/leave_a_mark.sh", format!("touch '{}'\n", mark.display()));
    a.t.write("tax-1.2.0/mark.geas", claim);
    let origin = "rule origin v1\ndescription \"A category\"\n\nenum category = standard | reduced | zero\n\ninputs\n  asked : category\n\noutputs\n  category : category\n\ntable category\npolicy unique\n| asked    | -> category : category |\n| standard | standard               |\n| reduced  | reduced                |\n| zero     | zero                   |\n";
    a.t.write("tax-1.2.0/rules/origin.rule", origin);
    edit(&a.t, "tax-1.2.0/rules/vat_rate.rule", "  category : category\n", "  category : category  fed by rulec \"origin.rule\" output category\n");
    edit(&a.t, "tax-1.2.0/rules/vat_rate.rule", "\noutputs\n", "\nnever category = zero\n\noutputs\n");
    edit(&a.t, "tax-1.2.0/rules/vat_rate.rule", "| zero     | 0%      |\n", "");
    let (code, out, err) = a.run_in("tax-1.2.0", &a.path("pushed"), &["mod", "push"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    // one that does not pass, from a working tree
    ritsu_testkit::tmp::copy_dir(&a.path("tax-1.2.0"), &a.path("work/tax-next"));
    edit(&a.t, "work/tax-next/ritsu.mod", "version 1.2.0", "version 1.3.0");
    a.t.write("work/tax-next/mark.geas", "claim \"it never ends\" {\n");
    a.t.write(
        "work/tax.list",
        format!("module \"{host}/acme/tax\"\nlocal \"tax-next\"\n\nsignature \"{host}/acme/\" by matching \"^https://github[.]com/acme/\" issuer \"https://token.actions.githubusercontent.com\"\n"),
    );
    let (code, out, err) = a.site("work/tax.list", "claims", &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!mark.exists(), "writing the site ran a claim:\n{out}{err}");

    let page = |version: &str| {
        std::fs::read_to_string(a.path(&format!("claims/{host}/acme/tax/{version}/index.html"))).unwrap_or_else(|e| panic!("{version}: {e}"))
    };
    let (held, plain, failed) = (page("1.2.0"), page("1.1.0"), page("1.3.0"));
    assert!(held.contains("<code>ritsu check . --no-claims</code>: all 3 files pass."), "{held}");
    assert!(held.contains("were read and not run: running them would run its author"), "{held}");
    assert!(held.contains("Passes <code>ritsu check</code>, its claims not run"), "{held}");
    assert!(plain.contains("<code>ritsu check .</code>: all 1 file pass."), "{plain}");
    assert!(!plain.contains("--no-claims"), "{plain}");
    assert!(failed.contains("<code>ritsu check . --no-claims</code> found errors."), "{failed}");
    assert!(failed.contains("mark.geas:"), "what the check printed, its paths from the module:\n{failed}");
    assert!(!failed.contains("is not a value this flag takes"), "{failed}");
    assert!(!failed.contains(a.path("").to_string_lossy().as_ref()), "a path of this machine on the page:\n{failed}");

    // what 1.2.0 changes, in words
    assert!(held.contains("rules/vat_rate.rule</span> now asks its callers to keep to <code>never category = zero</code>"), "{held}");
    assert!(held.contains("may break a user"), "{held}");
    assert!(held.contains("<code>inputs/category</code> now says where its value comes from: <code>rulec &quot;origin.rule&quot; output category</code>"), "{held}");
    assert!(held.contains("The inputs both versions accept get the same answers, but 1 change to the interface may break a user."), "{held}");
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
            "ritsu mod site: wrote 43 pages of 3 modules in local\n",
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
    let rows: Vec<&str> = shop.lines().filter(|l| l.contains("<a href=\"./0.") && l.starts_with("<tr><td class=\"mono nowrap\">")).collect();
    assert_eq!(rows.len(), 3, "{shop}");
    assert!(rows[0].contains(">0.3.0</a>") && rows[0].contains("<span class=\"warn\">not published yet</span>"), "{shop}");
    assert!(rows[1].contains(">0.2.0</a>") && rows[2].contains(">0.1.0</a>"), "{shop}");
    let next = page(&format!("{host}/acme/shop/0.3.0/index.html"));
    assert!(next.contains("<div class=\"ctl mono fixed\">0.2.0</div>"), "the changes are from the version before:\n{next}");
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
    assert!(!paths.contains("id=\"open\"") && !site.join("example.com/acme/shop/0.1.0/module.json").exists());
    // the big module: the button, and what it says when the link would be too long
    let big = page("example.com/acme/big/0.1.0/index.html");
    assert!(big.contains("data-big=\"Too big to open in the playground\""), "{big}");
    assert!(big.contains("data-max=\"1000000\""), "{big}");
    assert!(std::fs::read_to_string(site.join("_site/site.js")).unwrap().contains("packed.length > Number(open.dataset.max)"));
    assert!(site.join("example.com/acme/big/0.1.0/module.json").metadata().unwrap().len() > 1_300_000);
    // the command line's playground over the list's
    let (code, _, err) = a.site("work/modules.list", "local-flag", &["--playground", "http://127.0.0.1:9/ritsu/play/", "--lang", "ja"], &[]);
    assert_eq!(code, 0, "{err}");
    let next = std::fs::read_to_string(a.path(&format!("local-flag/{host}/acme/shop/0.3.0/index.html"))).unwrap();
    assert!(next.contains("data-playground=\"http://127.0.0.1:9/ritsu/play/\""), "{next}");
    assert!(err.contains(&format!("備考: `{host}/acme/shop` 0.1.0 はレジストリにもうあるので、作業ツリーではなくレジストリの版を載せました\n")), "{err}");
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
    assert_eq!(run(&["mod", "site", "empty.list", "--out", "out"]), (2, String::new(), "error: empty.list has no `module` line, no `git` line and no `local` line\nerror: the list cannot be read; nothing was written\n".into()));
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

/// The text of the `<code>` of each step of a page's side, as a reader copies it.
fn steps_of(page: &str) -> Vec<String> {
    let unescape = |s: &str| s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&");
    page.split("<div class=\"step\">").skip(1).map(|s| s.split("<pre class=\"code\"><code>").nth(1).and_then(|c| c.split("</code></pre>").next()).map(unescape).expect("a step has its code")).collect()
}

/// The steps a page tells a reader to take, taken as the page says them, in a project of the test's
/// own: requiring the module under the name the list's `as` gives it, the `signature` line, and the
/// lines that read a calendar of the module or call what it holds. `ritsu check` passes what they
/// make. Without an `as`, the name is the last part of the module's.
#[test]
fn the_steps_on_a_page_can_be_taken() {
    let a = Acme::new("mod-site-steps");
    let host = a.host();
    ritsu_testkit::tmp::copy_dir(&root().join("modules/holidays"), &a.path("holidays"));
    edit(&a.t, "holidays/ritsu.mod", "module ghcr.io/i2y/ritsu-holidays", &format!("module {host}/acme/ritsu-holidays"));
    let (code, out, err) = a.run_in("holidays", &a.path("pushed"), &["mod", "push"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    // a module of more than one kind of thing to call
    a.t.write("mixed/ritsu.mod", format!("module {host}/acme/mixed\nversion 1.0.0\nritsu >=0.41.0\n"));
    a.t.write("mixed/fee.rule", "rule fee v1\ndescription \"A fee\"\n\ninputs\n  urgent : bool\n\noutputs\n  amount : money[GBP]  round down(1GBP)\n\ntable fees\npolicy unique\n| urgent | -> amount : money[GBP] |\n| true   | 5GBP                  |\n| false  | 1GBP                  |\n");
    a.t.write("mixed/due.cal", "dates due v1\ndescription \"A week later\"\n\ninputs\n  given : date  range >=2026-01-01 <=2026-12-31\n\ndate last = given\n  + 7 days\n");
    a.t.write("mixed/stock.book", "book stock v1\ndescription \"Stock\"\n\nunit each\n\naccount shelf(sku: string) : each\n  at least 0 refused as out_of_stock\naccount made : each outside\naccount sold : each outside\n\ntransfer stock_up(delivery: string, sku: string, count: each)\n  key delivery\n  move count from made to shelf(sku)\n\ntransfer sell(order: string, sku: string, count: each)\n  key order\n  move count from shelf(sku) to sold\n");
    let (code, out, err) = a.run_in("mixed", &a.path("pushed"), &["mod", "push"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    a.t.write(
        "modules.list",
        format!("module \"{host}/acme/ritsu-holidays\" as \"holidays\"\nmodule \"{host}/acme/pricing\"\nmodule \"{host}/acme/mixed\"\nsignature \"{host}/acme/\" by matching \"^https://github[.]com/acme/\" issuer \"https://token.actions.githubusercontent.com\"\n"),
    );
    let (code, out, err) = a.site("modules.list", "site", &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let store = a.path("store-steps");
    let run = |dir: &str, command: &str| {
        for line in command.lines() {
            let words: Vec<&str> = line.split(' ').skip(1).collect();
            let (code, out, err) = a.run_in(dir, &store, &words, &[]);
            assert_eq!(code, 0, "`{line}` in {dir}:\n{out}{err}");
        }
    };
    for (module, version, name, file) in [("ritsu-holidays", "1.1.0", "holidays", "office.cal"), ("pricing", "1.1.0", "pricing", "basket.kn"), ("mixed", "1.0.0", "mixed", "order.kn")] {
        let page = std::fs::read_to_string(a.path(&format!("site/{host}/acme/{module}/index.html"))).unwrap();
        let steps = steps_of(&page);
        assert_eq!(steps.len(), 4 + 3 * usize::from(module == "pricing"), "{steps:#?}");
        let project = format!("app-{module}");
        a.t.write(&format!("{project}/ritsu.mod"), format!("module example.com/me/{name}-user\nversion 0.1.0\n"));
        assert_eq!(steps[0], format!("ritsu mod add {name} {host}/acme/{module}@{version}"));
        run(&project, &steps[0]);
        assert!(steps[1].starts_with(&format!("signature \"{host}/acme/\" by matching ")), "{steps:#?}");
        let manifest = a.t.read(&format!("{project}/ritsu.mod"));
        a.t.write(&format!("{project}/ritsu.mod"), format!("{manifest}\n{}\n", steps[1]));
        assert_eq!(steps[2], "ritsu mod fetch");
        run(&project, &steps[2]);
        let text = match file {
            "office.cal" => steps[3].clone(),
            "order.kn" => {
                assert_eq!(steps[3], "use rule fee from \"@mixed/fee.rule\"\nuse dates due from \"@mixed/due.cal\"\nuse book stock from \"@mixed/stock.book\"\n", "{steps:#?}");
                format!("module order v1\ndescription \"An order\"\n\n{}\n\nfn charge(urgent: bool) -> money[GBP] total\n  description \"What an order pays\"\n  fee(urgent: urgent).amount\n", steps[3])
            }
            // the line that calls a rule, in a module that calls it
            _ => format!("module basket v1\ndescription \"The total of a basket\"\n\n{}\nfn total(subtotal: money[GBPc] range >=0GBPc <=1_000GBPc) -> money[GBPc]\n  order_total(subtotal: subtotal, category: order_total.category.zero, shipping: 0GBPc).total\n", steps[3]),
        };
        a.t.write(&format!("{project}/{file}"), text);
        run(&project, "ritsu check .");
    }
    // the steps before moving a project up to 1.1.0, from 1.0.0, in a repository whose main has it
    let page = std::fs::read_to_string(a.path(&format!("site/{host}/acme/pricing/index.html"))).unwrap();
    let steps = steps_of(&page);
    a.t.write("up/ritsu.mod", "module example.com/me/up\nversion 0.1.0\n");
    run("up", &format!("ritsu mod add pricing {host}/acme/pricing@1.0.0"));
    let manifest = a.t.read("up/ritsu.mod");
    a.t.write("up/ritsu.mod", format!("{manifest}\n{}\n", steps[1]));
    run("up", "ritsu mod fetch");
    a.t.write("up/basket.kn", a.t.read("app-pricing/basket.kn"));
    let git = |args: &[&str]| {
        let o = Command::new("git").current_dir(a.path("up")).args(["-c", "user.name=t", "-c", "user.email=t@example.com"]).args(args).env_remove("GIT_DIR").output().expect("git");
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    };
    git(&["init", "--quiet", "-b", "main"]);
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "on 1.0.0"]);
    assert_eq!(steps[4], "ritsu mod get pricing@1.1.0\nritsu mod fetch");
    run("up", &steps[4]);
    assert_eq!(steps[5], "ritsu check .");
    run("up", &steps[5]);
    assert_eq!(steps[6], "ritsu mod report --base main --format markdown");
    run("up", &steps[6]);
}

/// A module whose records of a signature are on its registry says the day it was published: the
/// earliest time of the records of the bundle, as `cosign` leaves them under the tag `sha256-<hex>`
/// (a registry with no referrers API, the older way) and only with a `signature` line that checks
/// it. A version with no such record says no date.
#[test]
fn the_records_of_a_signature_say_when_a_version_was_published() {
    let a = Acme::new("mod-site-dates");
    let host = a.host();
    let digest = a.digest("acme/tax", "1.1.0");
    let bundle = a.r.put_blob("acme/tax", br#"{"verificationMaterial":{"tlogEntries":[{"integratedTime":"1793312527"},{"integratedTime":1793399999}]}}"#);
    let kind = "application/vnd.dev.sigstore.bundle.v0.3+json";
    let manifest = format!(r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","artifactType":"{kind}","config":{{"mediaType":"application/vnd.oci.empty.v1+json","digest":"{bundle}","size":2}},"layers":[{{"mediaType":"{kind}","digest":"{bundle}","size":3}}]}}"#);
    let entry = a.r.put_manifest("acme/tax", None, "application/vnd.oci.image.manifest.v1+json", manifest.as_bytes());
    let index = format!(r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{{"mediaType":"application/vnd.oci.image.manifest.v1+json","artifactType":"{kind}","digest":"{entry}","size":{}}}]}}"#, manifest.len());
    a.r.put_manifest("acme/tax", Some(&format!("sha256-{}", &digest[7..])), "application/vnd.oci.image.index.v1+json", index.as_bytes());
    a.t.write("modules.list", list_of_three(&host));
    a.t.write("unsigned.list", format!("module \"{host}/acme/tax\"\n"));
    for (lang, said, how) in [("en", "Published 2026-10-29", "The day its signature went into the transparency log (UTC)"), ("ja", "公開 2026-10-29", "署名が透明性ログに入った日（UTC）")] {
        let out = format!("site-{lang}");
        let (code, stdout, err) = a.site("modules.list", &out, &["--lang", lang], &[]);
        assert_eq!(code, 0, "{stdout}{err}");
        let page = |rel: &str| std::fs::read_to_string(a.path(&format!("{out}/{rel}"))).unwrap();
        assert_eq!(page("index.html").matches(said).count(), 1, "{lang}: the list says it of tax, and of no other");
        let newest = page(&format!("{host}/acme/tax/1.1.0/index.html"));
        assert!(newest.contains(said) && newest.contains("<code>2026-10-29</code>") && newest.contains(how), "{lang}: {newest}");
        assert!(page(&format!("{host}/acme/tax/index.html")).contains(said), "{lang}");
        assert!(!page(&format!("{host}/acme/tax/1.0.0/index.html")).contains(said), "{lang}: a version with no record says no date");
    }
    // no line checks it, and the site does not read what no line has checked
    a.r.clear_log();
    let (code, stdout, err) = a.site("unsigned.list", "unsigned", &[], &[]);
    assert_eq!(code, 0, "{stdout}{err}");
    assert!(!std::fs::read_to_string(a.path("unsigned/index.html")).unwrap().contains("Published"));
    assert!(!a.r.log().iter().any(|s| s.target.contains("sha256-")), "the site asked for the records of a signature no line checks");
}

/// The lines and the flag that change what the pages say: `as` (the name a page tells a reader to
/// require a module by), `contribute` (how to add to the list, only when the list says where), the
/// wording the pages have with no `title` and no `description`, and `--alternate` (the same page in
/// the other language, from every kind of page). What is wrong with them stops the command.
#[test]
fn what_a_list_and_the_command_line_add_to_the_pages() {
    let a = Acme::new("mod-site-lines");
    let host = a.host();
    a.t.write("plain.list", format!("module \"{host}/acme/pricing\"\n"));
    a.t.write("full.list", format!("contribute \"https://example.com/acme/edit/main/modules.list\"\nmodule \"{host}/acme/pricing\" as \"prices\"\n"));
    let (code, out, err) = a.site("plain.list", "plain", &["--alternate", "ja=../plain-ja"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let page = |dir: &str, rel: &str| std::fs::read_to_string(a.path(&format!("{dir}/{rel}"))).unwrap();
    let index = page("plain", "index.html");
    assert!(index.contains("<title>Modules</title>") && index.contains("Rules, calendars and workflows you can read, try and check before you depend on them."), "{index}");
    assert!(!index.contains("List your module"), "{index}");
    assert!(index.contains("href=\"./../plain-ja/index.html\" lang=\"ja\" hreflang=\"ja\">日本語</a>"), "{index}");
    let module = page("plain", &format!("{host}/acme/pricing/index.html"));
    assert!(module.contains(&format!("ritsu mod add pricing {host}/acme/pricing@1.1.0")), "{module}");
    let back = "../".repeat(host.split('/').count() + 2);
    assert!(module.contains(&format!("href=\"{back}../plain-ja/{}/acme/pricing/index.html\"", host.replace(':', "%3A"))), "{module}");
    let file = page("plain", &format!("{host}/acme/pricing/1.1.0/files/ritsu.mod.html"));
    assert!(file.contains(&format!("{}/acme/pricing/1.1.0/files/ritsu.mod.html\" lang=\"ja\"", host.replace(':', "%3A"))), "{file}");
    let (code, out, err) = a.site("full.list", "full", &["--lang", "ja", "--alternate", "en=../full-en"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let index = page("full", "index.html");
    assert!(index.contains("<title>モジュール</title>") && index.contains("モジュールを載せる") && index.contains("href=\"https://example.com/acme/edit/main/modules.list\""), "{index}");
    assert!(index.contains("lang=\"en\" hreflang=\"en\">English</a>"), "{index}");
    let module = page("full", &format!("{host}/acme/pricing/index.html"));
    assert!(module.contains(&format!("ritsu mod add prices {host}/acme/pricing@1.1.0")) && module.contains("ritsu mod get prices@1.1.0\nritsu mod fetch"), "{module}");
    // the header: ritsu's word, or the name of a list that has one, on every page
    assert!(page("plain", "index.html").contains("<span class=\"rm-word\">ritsu</span><span class=\"rm-sub\">modules</span>"), "a list with no name");
    a.t.write("named.list", format!("title \"Acme & <sons>\"\nmodule \"{host}/acme/pricing\"\n"));
    let (code, out, err) = a.site("named.list", "named", &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
    for rel in ["index.html".to_string(), format!("{host}/acme/pricing/index.html"), format!("{host}/acme/pricing/1.1.0/files/ritsu.mod.html")] {
        let p = page("named", &rel);
        assert!(p.contains("<span class=\"rm-own\" title=\"Acme &amp; &lt;sons&gt;\">Acme &amp; &lt;sons&gt;</span>") && !p.contains("rm-word"), "{rel}: {p}");
    }
    // published from a branch, Jekyll would leave out `_site/`
    assert_eq!(page("named", ".nojekyll"), "");
    // both languages from one root, as the guide writes them: each page links to the other's
    let (code, out, err) = a.site("plain.list", "both", &["--alternate", "ja=ja"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, err) = a.site("plain.list", "both/ja", &["--lang", "ja", "--alternate", "en=.."], &[]);
    assert_eq!(code, 0, "{out}{err}");
    for (from, lang, to) in [("both", "ja", "both/ja"), ("both/ja", "en", "both")] {
        for rel in ["index.html".to_string(), format!("{host}/acme/pricing/index.html"), format!("{host}/acme/pricing/1.1.0/files/ritsu.mod.html")] {
            let p = page(from, &rel);
            let end = p.find(&format!("\" lang=\"{lang}\" hreflang=\"{lang}\"")).unwrap_or_else(|| panic!("{from}/{rel} has no link to {lang}"));
            let href = p[p[..end].rfind("href=\"").unwrap() + 6..end].replace("%3A", ":");
            let there = a.path(from).join(&rel).parent().unwrap().join(&href).canonicalize().unwrap_or_else(|e| panic!("{from}/{rel}: {href}: {e}"));
            assert_eq!(there, a.path(to).join(&rel).canonicalize().unwrap(), "{from}/{rel}: {href}");
        }
    }
    // what is wrong with them
    a.t.write("bad.list", format!("contribute \"http://example.com/\"\nmodule \"{host}/acme/pricing\" as \"Not A Name\"\nmodule \"{host}/acme/tax\" as prices\n"));
    let (code, out, err) = a.site("bad.list", "bad", &[], &[]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert!(err.contains("bad.list:1:12: `contribute` takes an https URL"), "{err}");
    assert!(err.contains("`as \"Not A Name\"` is not a dependency's name"), "{err}");
    assert!(err.contains("bad.list:3:") && err.contains("`as` takes a string"), "{err}");
    for wrong in ["frob", "fr=x", "ja=", "ja=/abs", "ja=https://example.com/"] {
        let (code, out, err) = a.site("plain.list", "bad", &["--alternate", wrong], &[]);
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", "error: `--alternate` takes `ja=<path>` or `en=<path>`\n"), "{wrong}");
    }
    assert!(!a.path("bad").exists());
}

/// The words of a module's `.req` on its pages: the reason in a line of the list, and why it changed
/// at the head of the changes, each thing with a link to its file; no interface of the `.req`, and the
/// copies its records keep apart from the data.
#[test]
fn the_reasons_of_a_version_on_its_pages() {
    let a = Acme::new("mod-site-why");
    let host = a.host();
    let name = format!("{host}/acme/fines");
    fines::write(&a.t, "fines-1.0.0", &name, &fines::OLD);
    fines::write(&a.t, "fines-1.1.0", &name, &fines::NEW);
    let pushed = a.path("pushed");
    for dir in ["fines-1.0.0", "fines-1.1.0"] {
        fines::review(a.t.path(), dir);
        let (code, out, err) = a.run_in(dir, &pushed, &["mod", "push"], &[]);
        assert_eq!(code, 0, "{dir}:\n{out}{err}");
    }
    a.t.write("fines.list", format!("module \"{name}\"\n"));
    let (code, out, err) = a.site("fines.list", "en", &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, err) = a.site("fines.list", "ja", &["--lang", "ja"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let page = |dir: &str, rel: &str| std::fs::read_to_string(a.path(&format!("{dir}/{rel}"))).unwrap();

    let reason = "Follows the amendment at 90 FR 1861 (January 10, 2025).";
    let index = page("en", "index.html");
    assert!(index.contains(&format!("<p class=\"latest reason\"><span class=\"dim\">Why:</span> {reason}</p>")), "{index}");

    let module = page("en", &format!("{name}/index.html"));
    let changes = &module[module.find("<section class=\"pane\" id=\"changes\">").unwrap()..];
    let changes = &changes[..changes.find("</section>").unwrap()];
    assert!(changes.contains("<dt>Requirements</dt><dd>1 moved, 1 new decision</dd>"), "{changes}");
    assert!(changes.contains("<dd>1 added, 1 removed, 3 changed, 0 the same, 8 moved in reviewed/. require: no change</dd>"), "{changes}");
    let why = changes.find("<h3>Why it changed</h3>").expect("why it changed");
    assert!(why < changes.find("<h3 class=\"mono\">fine.rule</h3>").expect("the rule's difference"), "{changes}");
    assert!(changes.contains("<a href=\"./1.1.0/code/fines.req.html#L10\">amounts</a></h4><span class=\"tag warnish\">Changed</span>"), "{changes}");
    assert!(changes.contains("<p class=\"whytext\">For penalties proposed after January 15, <del>2024</del><ins>2025</ins>, a penalty is at most $<del>100</del><ins>200</ins>.</p>"), "{changes}");
    assert!(changes.contains(&format!("<dt>Decided</dt><dd>{reason} <span class=\"small dim\">2026-01-01 · maintainer</span></dd>")), "{changes}");
    assert!(changes.contains("<p>(d) For penalties proposed after January 15, <del>2024</del><ins>2025</ins>, a penalty shall not exceed $<del>100</del><ins>200</ins>.</p>"), "{changes}");
    assert!(changes.contains("<dt>Met by</dt><dd><a href=\"./1.1.0/code/fine.rule.html\"><code>rulec &quot;fine.rule&quot; table amounts</code></a>"), "{changes}");
    assert!(changes.contains("<dt>Looked at</dt><dd><span class=\"mono\">2026-01-02</span> maintainer</dd>"), "{changes}");
    // the links reach the pages they name
    let dir = a.path(&format!("en/{name}"));
    for rel in ["1.1.0/code/fines.req.html", "1.1.0/code/fine.rule.html", "1.1.0/files/fines.req.html"] {
        assert!(dir.join(rel).is_file(), "{rel}");
    }
    // the `.req` has no interface, and the report under them says the same as `ritsu mod diff` does
    assert!(!changes.contains("<span class=\"mono\">fines.req</span>:"), "{changes}");
    assert!(changes[why..].contains(&format!("Decided 2026-01-01 by maintainer: {reason}")), "{changes}");
    // the copies the records keep are files, under a heading of their own
    let files = &module[module.find("<section class=\"pane\" id=\"files\">").unwrap()..];
    let (data, kept) = (files.find("Data and other files").unwrap(), files.find("What the records of who looked keep").unwrap());
    assert!(data < kept && files[data..kept].contains("sources/law/") && !files[data..kept].contains("reviewed/") && files[kept..].contains("reviewed/"), "{files}");

    let module = page("ja", &format!("{name}/index.html"));
    assert!(module.contains("<h3>変わった理由</h3>") && module.contains("<span class=\"tag warnish\">変わった</span>") && module.contains("<dt>決めたこと</dt>"), "{module}");
    assert!(module.contains("<dd>1 件変わり、新しく決めたこと 1 件</dd>") && module.contains("reviewed/ の入れ替わり 8"), "{module}");
    assert!(page("ja", "index.html").contains(&format!("<span class=\"dim\">理由：</span> {reason}")));
}

/// A module that keeps its requirements in English and in Japanese says each reason twice. A page
/// gives the reason in its own language, and does not count the other as one more; under the changes,
/// what is in its language comes first. A reason in one language alone is on both pages (the test
/// above).
#[test]
fn a_page_gives_the_reason_in_its_own_language() {
    let a = Acme::new("mod-site-why-two");
    let host = a.host();
    let name = format!("{host}/acme/fines");
    fines::write(&a.t, "fines-1.0.0", &name, &fines::OLD);
    fines::write(&a.t, "fines-1.1.0", &name, &fines::NEW);
    let pushed = a.path("pushed");
    for (dir, amount, from, because) in [("fines-1.0.0", "100", "2024-01-16", "89 FR 1817 の改正に合わせる。"), ("fines-1.1.0", "200", "2025-01-16", "90 FR 1861 の改正に合わせる。")] {
        a.t.write(
            &format!("{dir}/\u{7f70}\u{91d1}.req"),
            format!("requirements 罰金 v1\ndescription \"fine.rule の額がどこから来たか\"\n\nrole 保守 \"条と規則を読み合わせる\"\n\nrequirement 上限(amounts_ja)\n  text \"上限は {amount} ドルとする。\"\n  in force {from}..\n  owner 保守\n  decided 2026-01-01 by 保守 \"{because}\"\n  satisfied by rulec \"fine.rule\" table amounts\n  verified by rulec \"fine.rule\"\n"),
        );
        fines::review(a.t.path(), dir);
        let (code, out, err) = a.run_in(dir, &pushed, &["yuen", "review", "\u{7f70}\u{91d1}.req", "--root", ".", "--all", "--by", "保守", "--date", "2026-01-02"], &[]);
        assert_eq!(code, 0, "{dir}:\n{out}{err}");
        let (code, out, err) = a.run_in(dir, &pushed, &["mod", "push"], &[]);
        assert_eq!(code, 0, "{dir}:\n{out}{err}");
    }
    a.t.write("fines.list", format!("module \"{name}\"\n"));
    for (dir, flags, label, reason) in [("en", vec![], "Why:", "Follows the amendment at 90 FR 1861 (January 10, 2025)."), ("ja", vec!["--lang", "ja"], "理由：", "90 FR 1861 の改正に合わせる。")] {
        let (code, out, err) = a.site("fines.list", dir, &flags, &[]);
        assert_eq!(code, 0, "{out}{err}");
        let page = |rel: &str| std::fs::read_to_string(a.path(&format!("{dir}/{rel}"))).unwrap();
        let index = page("index.html");
        assert!(index.contains(&format!("<p class=\"latest reason\"><span class=\"dim\">{label}</span> {reason}</p>")), "{index}");
        let module = page(&format!("{name}/index.html"));
        let cards = &module[module.find("class=\"fileblock why\"").expect("why it changed")..];
        let (english, japanese) = (cards.find(">amounts</a></h4>").expect("the requirement in English"), cards.find(">上限</a></h4>").expect("the requirement in Japanese"));
        assert_eq!(japanese < english, dir == "ja", "{cards}");
    }
}

/// A version opens on a file in the language of its page: the tab to try lists those first, so it
/// opens on the first, the button of the playground names the same file, and the line that shows how
/// to use it reads that file. A version with nothing to try opens on what it holds in that language,
/// and one that holds nothing on its README.
#[test]
fn a_version_opens_on_a_file_in_the_language_of_its_page() {
    let t = TempDir::new("mod-site-first-file");
    let rule = |name: &str| format!("rule {name} v1\n\ninputs\n  n(n) : number  range >=0 <=9\n\noutputs\n  fee(fee) : money[USD]  round down(1USD)\n\ntable fees(fees)\npolicy unique\n| n | -> fee(fee) : money[USD] |\n| - | 5USD                    |\n");
    let core = |name: &str| format!("module {name} v1\n\nfn one(a: number range >=0 <=9) -> number\n  a\n");
    for (dir, files) in [
        ("rules", vec![("fee.rule", rule("fee(fee)")), ("\u{6599}\u{91d1}.rule", rule("\u{6599}\u{91d1}(fee_ja)")), ("README.md", "# Fees\n".to_string()), ("README.ja.md", "# \u{6599}\u{91d1}\n".to_string())]),
        ("cores", vec![("core.kn", core("core")), ("core.ja.kn", core("core_ja"))]),
        ("readme", vec![("README.md", "# Words\n".to_string()), ("README.ja.md", "# \u{8a00}\u{8449}\n".to_string())]),
    ] {
        t.write(&format!("{dir}/ritsu.mod"), format!("module example.com/acme/{dir}\nversion 0.1.0\n"));
        for (name, text) in files {
            t.write(&format!("{dir}/{name}"), text);
        }
    }
    t.write("three.list", "local \"rules\"\nlocal \"cores\"\nlocal \"readme\"\n");
    let (fee, ryokin) = ("fee.rule", "\u{6599}\u{91d1}.rule");
    for (lang, first, second, core_file, readme) in [("en", fee, ryokin, "core.kn", "README.md"), ("ja", ryokin, fee, "core.ja.kn", "README.ja.md")] {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(t.path()).args(["mod", "site", "three.list", "--out", lang, "--lang", lang]).env(store::VARIABLE, t.path().join("store"));
        for v in LANG_VARS {
            c.env_remove(v);
        }
        let (code, out, err) = said(c.output().unwrap());
        assert_eq!(code, 0, "{out}{err}");
        let page = |dir: &str| t.read(&format!("{lang}/example.com/acme/{dir}/0.1.0/index.html"));
        let rules = page("rules");
        let name_of = |file: &str| file.trim_end_matches(".rule").to_string();
        let at = |file: &str| rules.find(&format!(">{}</option>", name_of(file))).unwrap_or_else(|| panic!("{lang}: no option for {file}: {rules}"));
        assert!(at(first) < at(second), "{lang}: the tab to try opens on another file: {rules}");
        assert!(rules.contains(&format!(" data-file=\"{first}\" ")), "{lang}: the playground opens on another file: {rules}");
        assert!(rules.contains(&format!("use rule {} from", name_of(first))), "{lang}: the line of how to use it reads another file: {rules}");
        assert!(page("cores").contains(&format!(" data-file=\"{core_file}\" ")), "{lang}: {}", page("cores"));
        assert!(page("readme").contains(&format!(" data-file=\"{readme}\" ")), "{lang}: {}", page("readme"));
    }
}

/// What the pages do in a browser, in Chrome, from their files: a module's page picks a tab by the
/// address and keeps the head in sight, the tab to try opens a file in its frame, the list narrows
/// as it is asked, and a calendar's page answers about the day in its address (closed, open with the
/// line a calendar that reads it could write, and not known), also when a file's page is shown alone
/// inside the tab.
#[test]
fn the_pages_in_chrome() {
    if !need(Need::Chrome) {
        return;
    }
    let Some(chrome) = ritsu_testkit::chrome::find() else {
        skip("Chrome is not found; set RITSU_CHROME to run this test");
        return;
    };
    let a = Acme::new("mod-site-chrome");
    let host = a.host();
    ritsu_testkit::tmp::copy_dir(&root().join("modules/holidays"), &a.path("holidays"));
    edit(&a.t, "holidays/ritsu.mod", "module ghcr.io/i2y/ritsu-holidays", &format!("module {host}/acme/ritsu-holidays"));
    let (code, out, err) = a.run_in("holidays", &a.path("pushed"), &["mod", "push"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let list = list_of_three(&host);
    a.t.write("modules.list", format!("{list}module \"{host}/acme/ritsu-holidays\"\n"));
    let (code, out, err) = a.site("modules.list", "site", &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let dom = |rel: &str| ritsu_testkit::chrome::dump_dom(&chrome, &format!("file://{}/{rel}", a.path("site").display()), 2000, std::time::Duration::from_secs(60));
    let root_tag = |d: &str| d[d.find("<html").unwrap()..].split('>').next().unwrap().to_string();
    let pricing = format!("{host}/acme/pricing/index.html");
    // the tabs
    for (hash, tab) in [("", "overview"), ("#changes", "changes"), ("#nowhere", "overview"), ("#files", "files")] {
        let d = dom(&format!("{pricing}{hash}"));
        assert!(root_tag(&d).contains(&format!(" data-tab=\"{tab}\"")), "{hash}: {}", root_tag(&d));
        assert!(d.contains(&format!("data-tab=\"{tab}\" aria-current=\"page\"")), "{hash}: the tab is not marked");
    }
    // the tab to try, and the file the address names
    let d = dom(&format!("{pricing}#try:rules/shipping_fee.rule"));
    assert!(d.contains("files/rules/shipping_fee.rule.html?view=try\""), "{d}");
    // the list narrows: the rule's name finds its module, and marks it
    let d = dom("index.html?q=shipping_fee");
    let shown: Vec<&str> = d.split("<article ").skip(1).filter(|r| !r.split('>').next().unwrap().contains(" hidden")).collect();
    assert_eq!(shown.len(), 1, "{d}");
    assert!(shown[0].contains("class=\"item hit\" data-name=\"shipping_fee\""), "{}", shown[0]);
    let d = dom("index.html?q=nowhere-at-all");
    assert_eq!(d.split("<article ").skip(1).filter(|r| !r.split('>').next().unwrap().contains(" hidden")).count(), 0, "{d}");
    assert!(d.contains("Nothing matches “nowhere-at-all”."), "{d}");
    // a calendar's page, and the three answers
    let calendar = format!("{host}/acme/ritsu-holidays/1.1.0/files/uk/england_and_wales.cal.html");
    let d = dom(&format!("{calendar}?date=2026-12-25"));
    assert!(d.contains("class=\"ans closed\"") && d.contains("<p class=\"name\">Christmas Day</p>") && d.contains("It is a row of the table bank_holidays."), "{d}");
    assert!(d.contains("<td class=\"d\">2026-12-28</td><td class=\"w\">Mon</td><td>Boxing Day (Substitute day)</td>"), "{d}");
    let d = dom(&format!("{calendar}?date=2026-11-07"));
    assert!(d.contains("class=\"ans open\"") && d.contains("This is a Saturday, and this calendar leaves it open.") && d.contains("<code>closed weekly sat, sun</code>"), "{d}");
    let d = dom(&format!("{calendar}?date=2031-01-01"));
    assert!(d.contains("class=\"ans unknown\"") && d.contains("This calendar knows 2019-01-01 to 2028-12-31."), "{d}");
    // a file's page alone: only the section to try
    let d = dom(&format!("{calendar}?date=2026-12-25&view=try"));
    assert!(root_tag(&d).contains("rm-bare") && root_tag(&d).contains("rm-ready"), "{}", root_tag(&d));
    assert!(d.contains("<header hidden") , "the rest of the page is hidden:\n{d}");
}
