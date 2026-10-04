//! The playground on the site (website/docs/playground, DESIGN 8.7): the page answers what the
//! `ritsu` binary answers in a directory holding the same files, the module it runs answers what the
//! library answers, and the page starts in Chrome and shows what `ritsu check` prints.
//!
//! Two of its files are committed products: projects.json, the projects the page opens, written
//! from website/playground/ (`RITSU_BLESS=1` writes it anew), and ritsu.wasm, built by
//! website/tools/make_wasm.sh. Both can go stale, and these tests are what says so. Node drives the
//! module, and Chrome the page (ritsu-testkit's: `RITSU_CHROME`, else where macOS keeps it, else on
//! the PATH). A test that cannot find what it needs prints `SKIP: ritsu: …` and passes.

use ritsu_base::naming::Tool;
use ritsu_testkit::{Need, TempDir, need, ready, skip};
use ritsu_wasm::playground::{self, Request};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn site() -> PathBuf {
    repo().join("website/docs/playground")
}

/// The projects the page opens, in the order it lists them: English first, then the Japanese
/// version beside it (website/playground/<name>/), each with the file it opens on.
const PROJECTS: [(&str, &str); 2] = [("shop", "proto/shop/v1/order.proto"), ("shop.ja", "proto/shop/v1/order.proto")];

/// Every file under a directory, by its path from it, in the order the page shows them: the files
/// of the languages in the order `ritsu check` checks them (DESIGN 6.1), then by path.
fn files_of(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
                out.push((rel, std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))));
            }
        }
    }
    let rank = |p: &str| ritsu_project::project::kind_of(p).and_then(|t| ritsu_project::ORDER.iter().position(|o| *o == t)).unwrap_or(ritsu_project::ORDER.len());
    out.sort_by(|(a, _), (b, _)| (rank(a), a).cmp(&(rank(b), b)));
    out
}

/// projects.json: `{"projects": [{"name", "open", "files": [[path, text], ...]}]}`, a file to a line.
fn record() -> String {
    let mut s = String::from("{\n  \"projects\": [\n");
    for (i, (name, open)) in PROJECTS.iter().enumerate() {
        let files = files_of(&repo().join("website/playground").join(name));
        assert!(files.iter().any(|(p, _)| p == open), "{name} has no {open}");
        s.push_str(&format!("    {{\n      \"name\": {},\n      \"open\": {},\n      \"files\": [\n", json!(name), json!(open)));
        let lines: Vec<String> = files.iter().map(|(p, t)| format!("        {}", json!([p, t]))).collect();
        s.push_str(&lines.join(",\n"));
        s.push_str(&format!("\n      ]\n    }}{}\n", if i + 1 < PROJECTS.len() { "," } else { "" }));
    }
    s.push_str("  ]\n}\n");
    s
}

/// The projects as the page has them (the committed projects.json).
fn committed() -> Vec<(String, String, Vec<(String, String)>)> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(site().join("projects.json")).expect("website/docs/playground/projects.json")).unwrap();
    v["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let files = p["files"].as_array().unwrap().iter().map(|f| (f[0].as_str().unwrap().to_string(), f[1].as_str().unwrap().to_string())).collect();
            (p["name"].as_str().unwrap().to_string(), p["open"].as_str().unwrap().to_string(), files)
        })
        .collect()
}

#[test]
fn the_projects_are_what_the_page_opens() {
    let now = record();
    let file = site().join("projects.json");
    if ritsu_testkit::golden::bless() {
        std::fs::write(&file, &now).unwrap();
        return;
    }
    let was = std::fs::read_to_string(&file).unwrap_or_default();
    assert!(was == now, "website/docs/playground/projects.json is not what website/playground holds now; write it anew with RITSU_BLESS=1 cargo test -p ritsu --test playground");
}

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[String]) -> (u8, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1) as u8, String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// A directory holding the project's files.
fn written_out(files: &[(String, String)]) -> TempDir {
    let t = TempDir::new("playground");
    for (p, text) in files {
        t.write(p, text);
    }
    t
}

/// The files in `dir` that are not the project's, by their paths from it.
fn new_files(dir: &Path, files: &[(String, String)]) -> Vec<(String, String)> {
    let mut all = files_of(dir);
    all.retain(|(p, _)| !files.iter().any(|(q, _)| q == p));
    all.sort();
    all
}

/// The words of the command the page says it ran, with the language it ran in.
fn words_of(command: &str, lang: &str) -> Vec<String> {
    let mut w: Vec<String> = command.strip_prefix("ritsu ").expect("a command of ritsu").split_whitespace().map(str::to_string).collect();
    if !w.iter().any(|x| x == "--lang") {
        w.push("--lang".into());
        w.push(lang.into());
    }
    w
}

/// One of what the page answered, held to the binary run as the page says, in a directory holding
/// the same files: the exit code, what it prints, and (for a generator) what it writes.
fn holds(files: &[(String, String)], lang: &str, what: &str, answer: &Value, failures: &mut Vec<String>) {
    let dir = written_out(files);
    let (code, out, err) = ritsu_in(dir.path(), &words_of(answer["command"].as_str().unwrap(), lang));
    let got = (answer["code"].as_u64().unwrap() as u8, answer["out"].as_str().unwrap(), answer["err"].as_str().unwrap());
    if (code, out.as_str(), err.as_str()) != got {
        failures.push(format!("{what} ({lang}): `{}`\n--- the command (exit {code})\n{out}{err}\n--- the page (exit {})\n{}{}", answer["command"], got.0, got.1, got.2));
    }
    if let Some(written) = answer["files"].as_array() {
        let mine: Vec<(String, String)> = written.iter().map(|f| (f["path"].as_str().unwrap().to_string(), f["body"].as_str().unwrap().to_string())).collect();
        let mut sorted = mine.clone();
        sorted.sort();
        let theirs = new_files(dir.path(), files);
        if sorted != theirs {
            failures.push(format!("{what} ({lang}): the command wrote {} file(s), the page {}", theirs.len(), mine.len()));
        }
    }
}

/// The requests a project's page makes: `ritsu check`, and for every file its generator (with
/// every target the generator takes) and its page. The files go in with [`files_json`].
fn requests(files: &[(String, String)], lang: &str) -> Vec<(&'static str, Value)> {
    let base = |path: &str, target: Option<&str>| json!({ "files": {}, "lang": lang, "path": path, "target": target });
    let mut out = vec![("check", base("", None))];
    for (p, _) in files {
        let targets: Vec<&str> = match ritsu_project::project::kind_of(p) {
            Some(Tool::Dandori) => vec!["asl", "temporal", "temporal-python", "temporal-go", "durable", "argo", "pydantic-graph"],
            Some(Tool::Chobo) => vec!["postgres", "postgres-typescript", "postgres-python", "postgres-go", "tigerbeetle-typescript", "tigerbeetle-python", "tigerbeetle-go"],
            Some(Tool::Yuen) => vec!["reqif", "prov"],
            _ => vec![],
        };
        if targets.is_empty() {
            out.push(("gen", base(p, None)));
        }
        for t in targets {
            out.push(("gen", base(p, Some(t))));
        }
        out.push(("doc", base(p, None)));
    }
    out
}

fn files_json(files: &[(String, String)]) -> Value {
    Value::Object(files.iter().map(|(p, t)| (p.clone(), Value::String(t.clone()))).collect())
}

/// Edits a reader might make, each reaching a way the page answers that the projects as they open
/// do not: the contract put right, the rule given the value instead (one row short, then whole),
/// a rule's output renamed under the flow that reads it, a flow that does not parse, a file of no
/// language added, and a geas spec that does not parse (a spec that parses runs programs, which
/// the page cannot: it is not held to the binary).
fn edits() -> Vec<(&'static str, &'static str, Vec<(String, String)>)> {
    let mut out = Vec::new();
    for (name, open, files) in committed() {
        let _ = open;
        let swap = |files: &[(String, String)], path: &str, from: &str, to: &str| -> Vec<(String, String)> {
            let mut f = files.to_vec();
            let x = f.iter_mut().find(|(p, _)| p == path).unwrap_or_else(|| panic!("{name} has no {path}"));
            assert!(x.1.contains(from), "{path} no longer has `{from}`");
            x.1 = x.1.replacen(from, to, 1);
            f
        };
        let (billing, urgency, flow) = if name == "shop" {
            ("billing/rules/billing_need.rule", "ordering/rules/urgency.rule", "ordering/ship_order.flow")
        } else {
            ("billing/rules/請求の要否.rule", "ordering/rules/出荷の急ぎ.rule", "ordering/出荷.flow")
        };
        let lang = if name == "shop" { "en" } else { "ja" };
        let fixed = swap(&files, "proto/shop/v1/order.proto", "  ORDER_STATUS_RETURNED = 5;\n", "");
        out.push(("the contract put right", lang, fixed.clone()));
        let (enum_from, enum_to, row_from, row_to) = if name == "shop" {
            ("| shipped | cancelled", "| shipped | cancelled | returned", "| cancelled | skip        |\n", "| cancelled | skip        |\n| returned  | skip        |\n")
        } else {
            ("| 取消(cancelled)", "| 取消(cancelled) | 返品(returned)", "| 取消   | 請求しない  |\n", "| 取消   | 請求しない  |\n| 返品   | 請求しない  |\n")
        };
        let given = swap(&files, billing, enum_from, enum_to);
        out.push(("the rule given the value, a row short", lang, given.clone()));
        out.push(("the rule given the value and its row", lang, swap(&given, billing, row_from, row_to)));
        // the output renamed in the rule, its table and its examples: rulec passes, dandori does not
        let renames: [(&str, &str); 2] = if name == "shop" {
            [("  carrier : carrier\n", "  courier : carrier\n"), ("| -> urgent | carrier  |", "| -> urgent | courier  |")]
        } else {
            [("  便(carrier)  : 便\n", "  運び方(courier) : 便\n"), ("| -> 急ぎ | 便     |", "| -> 急ぎ | 運び方 |")]
        };
        let mut renamed = fixed.clone();
        for (from, to) in renames {
            let x = renamed.iter_mut().find(|(p, _)| p == urgency).unwrap();
            assert!(x.1.contains(from), "{urgency} no longer has `{from}`");
            x.1 = x.1.replace(from, to);
        }
        out.push(("a rule's output renamed under the flow", lang, renamed));
        out.push(("a flow that does not parse", lang, swap(&fixed, flow, "flow\n", "flow =\n")));
        let mut more = fixed.clone();
        more.push(("notes.txt".into(), "what the shop has yet to decide\n".into()));
        out.push(("a file of no language", lang, more));
        // geas reads a spec the page holds, and stops at its syntax before any program starts
        let mut spec = fixed.clone();
        spec.push(("checks/greeter.geas".into(), "target greeter {\n  run \"python3 greeter.py\"\n}\n\nclaim \"greets\" {\n".into()));
        out.push(("a spec that does not parse", lang, spec));
    }
    out
}

/// The page answers as the binary does: for every project and edit, in both languages, `ritsu
/// check` (its text and its JSON), and every file's generator and page.
#[test]
fn the_page_answers_as_the_command_does() {
    let mut jobs: Vec<(String, &'static str, Vec<(String, String)>)> = Vec::new();
    for (name, _, files) in committed() {
        for lang in ["en", "ja"] {
            jobs.push((format!("{name} as it opens"), lang, files.clone()));
        }
    }
    for (what, lang, files) in edits() {
        jobs.push((what.to_string(), lang, files));
    }
    let failures: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|(what, lang, files)| {
                s.spawn(move || {
                    let mut failures = Vec::new();
                    let mut n = 0;
                    for (kind, req) in requests(files, lang) {
                        let mut req = req;
                        req["files"] = files_json(files);
                        let r = Request::from_json(&req).unwrap();
                        let path = r.path.clone();
                        let label = format!("{what}: {kind} {path}");
                        match kind {
                            "check" => {
                                let a = playground::check(&r);
                                holds(files, lang, &label, &a, &mut failures);
                                let dir = written_out(files);
                                let (_, out, _) = ritsu_in(dir.path(), &words_of("ritsu check . --format json", lang));
                                let want: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
                                if want != a["json"] {
                                    failures.push(format!("{label} ({lang}): the JSON differs"));
                                }
                                n += 2;
                            }
                            "gen" => {
                                let a = playground::generate(&r);
                                if a["none"] != true {
                                    holds(files, lang, &label, &a, &mut failures);
                                    n += 1;
                                }
                            }
                            _ => {
                                let a = playground::doc(&r);
                                if a["none"] != true {
                                    holds(files, lang, &format!("{label} (html)"), &a["html"], &mut failures);
                                    holds(files, lang, &format!("{label} (markdown)"), &a["markdown"], &mut failures);
                                    n += 2;
                                }
                            }
                        }
                    }
                    (failures, n)
                })
            })
            .collect();
        let mut all = Vec::new();
        let mut n = 0;
        for h in handles {
            let (f, k) = h.join().unwrap();
            all.extend(f);
            n += k;
        }
        eprintln!("compared: {n} answers of the page with what the binary prints and writes, on {} projects and edits", jobs.len());
        all
    });
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The page's side of the boundary: allocate, write, call, read the length out of the header,
/// free. It is here rather than in a committed file because it is a test's: playground.js is the
/// copy that ships, and if the two ever disagree about the convention, this one fails.
const DRIVER: &str = r#"
import { readFileSync, writeFileSync } from "node:fs";
const [wasmPath, requestsPath, outPath] = process.argv.slice(2);
const inst = await WebAssembly.instantiate(await WebAssembly.compile(readFileSync(wasmPath)), {});
const e = inst.exports;
function put(text) {
  const bytes = Buffer.from(text, "utf8");
  const ptr = e.ritsu_alloc(bytes.length);
  new Uint8Array(e.memory.buffer, ptr + 4, bytes.length).set(bytes);
  return ptr;
}
function take(ptr) {
  const len = new DataView(e.memory.buffer).getUint32(ptr, true);
  const text = Buffer.from(new Uint8Array(e.memory.buffer, ptr + 4, len)).toString("utf8");
  e.ritsu_free(ptr);
  return text;
}
function call(fn, text) {
  const ptr = put(text);
  try {
    return take(e[fn](ptr));
  } finally {
    e.ritsu_free(ptr);
  }
}
const version = take(e.ritsu_version());
const imports = WebAssembly.Module.imports(await WebAssembly.compile(readFileSync(wasmPath))).length;
const answers = JSON.parse(readFileSync(requestsPath, "utf8")).map(([what, req]) => call("ritsu_" + what, req));
writeFileSync(outPath, JSON.stringify({ version, imports, answers }));
"#;

fn node_available() -> bool {
    Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// The module answers as the library does, every request of the comparison above and a few the
/// page can be made to send by hand (a path outside the project, a request that is not JSON).
#[test]
fn the_module_answers_as_the_library_does() {
    let wasm = site().join("ritsu.wasm");
    if !ready(Need::Node, || node_available() && wasm.exists(), "node or website/docs/playground/ritsu.wasm is missing") {
        return;
    }
    let mut requests: Vec<(String, String)> = Vec::new();
    for (_, _, files) in committed() {
        for lang in ["en", "ja"] {
            for (kind, mut req) in requests_of(&files, lang) {
                req["files"] = files_json(&files);
                requests.push((kind.to_string(), req.to_string()));
            }
        }
    }
    for (_, lang, files) in edits() {
        let mut req = json!({ "lang": lang, "path": "" });
        req["files"] = files_json(&files);
        requests.push(("check".into(), req.to_string()));
    }
    requests.push(("check".into(), json!({ "files": { "../outside.rule": "" }, "lang": "en" }).to_string()));
    requests.push(("gen".into(), "not a request".into()));

    let tmp = TempDir::new("wasm");
    let dir = tmp.path();
    std::fs::write(dir.join("driver.mjs"), DRIVER).unwrap();
    std::fs::write(dir.join("requests.json"), serde_json::to_string(&requests).unwrap()).unwrap();
    let o = Command::new("node")
        .current_dir(dir)
        .args(["driver.mjs", wasm.to_str().unwrap(), "requests.json", "answers.json"])
        .output()
        .expect("could not run node");
    assert!(o.status.success(), "the driver failed: {}", String::from_utf8_lossy(&o.stderr));
    let got: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("answers.json")).unwrap()).unwrap();

    assert_eq!(got["version"], env!("CARGO_PKG_VERSION"), "website/docs/playground/ritsu.wasm is of another version; run website/tools/make_wasm.sh");
    assert_eq!(got["imports"], 0, "the module asks the page for imports; the page gives it none");
    let answers = got["answers"].as_array().unwrap();
    assert_eq!(answers.len(), requests.len());
    let mut stale = Vec::new();
    for ((what, req), a) in requests.iter().zip(answers) {
        let want = playground::answer(what, req);
        if a.as_str() != Some(want.as_str()) {
            let r: Value = serde_json::from_str(req).unwrap_or(Value::Null);
            stale.push(format!("{what} {} ({}) {}", r["path"], r["lang"], r["target"]));
            if stale.len() == 1 {
                let pretty = |s: &str| serde_json::from_str::<Value>(s).map(|v| serde_json::to_string_pretty(&v).unwrap()).unwrap_or_else(|_| s.to_string());
                stale.push(ritsu_testkit::golden::line_diff(&pretty(&want), &pretty(a.as_str().unwrap_or_default())));
            }
        }
    }
    assert!(stale.is_empty(), "website/docs/playground/ritsu.wasm answers otherwise than the library now; run website/tools/make_wasm.sh:\n{}", stale.join("\n"));
    eprintln!("compared: {} requests answered by the module as by the library", requests.len());
}

fn requests_of(files: &[(String, String)], lang: &str) -> Vec<(&'static str, Value)> {
    requests(files, lang)
}

/// Chrome, when the level lets the test run it and the machine has it; a SKIP line says why not.
fn chrome() -> Option<PathBuf> {
    if !need(Need::Chrome) {
        return None;
    }
    let found = ritsu_testkit::chrome::find();
    if found.is_none() {
        skip("Chrome is not found; set RITSU_CHROME to run this test");
    }
    found
}

/// A server for the page: `page` at `/`, and the playground's files beside it.
fn serve(page: String) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let page = page.clone();
            std::thread::spawn(move || respond(stream, &page));
        }
    });
    port
}

fn respond(mut s: std::net::TcpStream, page: &str) {
    let mut buf = [0u8; 4096];
    let n = s.read(&mut buf).unwrap_or(0);
    let head = String::from_utf8_lossy(&buf[..n]);
    let asked = head.split_whitespace().nth(1).unwrap_or("/").split(['?', '#']).next().unwrap_or("/").to_string();
    let found = if asked == "/" {
        Some((page.as_bytes().to_vec(), "text/html; charset=utf-8"))
    } else {
        let kind = match asked.rsplit('.').next() {
            Some("js") => "text/javascript",
            Some("css") => "text/css",
            Some("wasm") => "application/wasm",
            Some("json") => "application/json",
            _ => "application/octet-stream",
        };
        std::fs::read(site().join(asked.trim_start_matches('/'))).ok().map(|b| (b, kind))
    };
    let (status, body, kind) = match found {
        Some((b, k)) => ("200 OK", b, k),
        None => ("404 Not Found", Vec::new(), "text/plain"),
    };
    let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    let _ = s.write_all(&body);
}

/// The DOM of a page after its scripts ran (up to 20 s of virtual time), as headless Chrome dumps it.
fn dump_dom(chrome: &Path, url: &str) -> String {
    ritsu_testkit::chrome::dump_dom(chrome, url, 20_000, Duration::from_secs(120))
}

/// The text inside the first element of a class, up to the tag that closes it, as a person reads it.
fn text_in(dom: &str, class: &str, close: &str) -> String {
    let from = dom.find(&format!("class=\"{class}\"")).map(|i| i + dom[i..].find('>').unwrap() + 1).unwrap_or(dom.len());
    let inner = &dom[from..from + dom[from..].find(close).unwrap_or(0)];
    let mut text = String::new();
    let mut in_tag = false;
    for c in inner.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", "\u{a0}").replace("&amp;", "&")
}

/// The page starts in Chrome, as the site's pages have it: it loads the module and the projects,
/// shows what `ritsu check` prints for the project it opens with a mark on the files it names, and
/// follows a link to a file, a view and a target.
#[test]
fn the_page_starts_in_chrome() {
    let Some(chrome) = chrome() else {
        return;
    };
    if !site().join("ritsu.wasm").exists() {
        skip("website/docs/playground/ritsu.wasm is missing; run website/tools/make_wasm.sh");
        return;
    }
    let (_, _, files) = committed().into_iter().next().unwrap();
    let mut looked = 0;
    for (tag, page) in [("en", "website/docs/playground.md"), ("ja", "website/docs-ja/playground.md")] {
        // the widget as the page has it, with the script served from here
        let md = std::fs::read_to_string(repo().join(page)).unwrap();
        let from = md.find("<div class=\"pg\"").expect("the page has no playground");
        let tail = "<script src=\"playground/playground.js\" defer></script>";
        let to = md[from..].find(tail).expect("the page does not load playground.js") + from + tail.len();
        let widget = md[from..to].replace("playground/playground.js", "/playground.js");
        let port = serve(format!("<!doctype html><html><head><meta charset=\"utf-8\"></head><body>{widget}</body></html>"));
        let ask = |path: &str, target: Option<&str>| {
            let mut req = json!({ "lang": tag, "path": path, "target": target });
            req["files"] = files_json(&files);
            Request::from_json(&req).unwrap()
        };

        let dom = dump_dom(&chrome, &format!("http://127.0.0.1:{port}/"));
        let status = text_in(&dom, "pg-status", "</span>");
        assert!(status.starts_with(&format!("ritsu {}", env!("CARGO_PKG_VERSION"))), "{page}: the status says {status:?}");
        let want = playground::check(&ask("", None));
        let counts = if tag == "ja" { "エラー 3 件、警告 0 件" } else { "3 errors, 0 warnings" };
        assert!(status.ends_with(counts), "{page}: the status says {status:?}");
        let shown = text_in(&dom, "pg-out", "</div>");
        assert_eq!(shown, format!("$ {}\n{}{}", want["command"].as_str().unwrap(), want["out"].as_str().unwrap(), want["err"].as_str().unwrap()), "{page}: the page shows another check");
        for marked in ["billing/rules/billing_need.rule", "requirements/billing.req"] {
            assert!(dom.contains(&format!("class=\"pg-file err\" role=\"tab\" aria-selected=\"false\">{marked}")), "{page}: the tab of {marked} is not marked");
        }
        assert!(dom.contains("aria-selected=\"true\">proto/shop/v1/order.proto"), "{page}: the page does not open on the contract");

        let flow = "ordering/ship_order.flow";
        let dom = dump_dom(&chrome, &format!("http://127.0.0.1:{port}/#project=shop&file={flow}&view=gen&target=temporal"));
        let want = playground::generate(&ask(flow, Some("temporal")));
        let first = &want["files"][0];
        assert_eq!(text_in(&dom, "pg-out", "</div>"), first["body"].as_str().unwrap(), "{page}: the page shows another first file of {flow} for Temporal");

        let rule = "ordering/rules/urgency.rule";
        let dom = dump_dom(&chrome, &format!("http://127.0.0.1:{port}/#project=shop&file={rule}&view=doc"));
        assert!(dom.contains("class=\"pg-open\" href=\"blob:"), "{page}: the page has no link to the page rulec doc draws of {rule}");
        looked += 3;
    }
    eprintln!("compared: {looked} views of the page in Chrome with what the library answers");
}
