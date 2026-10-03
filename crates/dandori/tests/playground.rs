//! The playground on the site (website/docs/playground): what it reads the examples from is what
//! the command reads, the module it runs answers what the binary answers, and the page starts in
//! Chrome and shows what `check` prints.
//!
//! Two of its files are committed products: presets.json, recorded from the examples here (with
//! rulec; `DANDORI_BLESS=1` records it anew), and dandori.wasm, built by
//! website/tools/make_wasm.sh. Both can go stale, and these tests are what says so. Node drives
//! the module, and Chrome the page (`DANDORI_CHROME`, else where macOS keeps it, else on the PATH).
//! A test that cannot find what it needs prints SKIP and passes.

use dandori::commands::TARGETS;
use dandori::diag::Lang;
use dandori::playground::{self, Request};
use dandori::sources::{self, Bundle, Recorder};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rel(p: &Path) -> String {
    sources::key(p.strip_prefix(root()).unwrap_or(p))
}

fn site() -> PathBuf {
    root().join("website/docs/playground")
}

fn rulec_available() -> bool {
    let bin = std::env::var("DANDORI_RULEC").unwrap_or_else(|_| "rulec".into());
    Command::new(&bin).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn node_available() -> bool {
    Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn chrome() -> Option<String> {
    if let Ok(c) = std::env::var("DANDORI_CHROME") {
        return Some(c);
    }
    let mac = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
    if Path::new(mac).exists() {
        return Some(mac.into());
    }
    ["google-chrome", "chromium", "chromium-browser"].iter().find(|c| Command::new(c).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)).map(|c| c.to_string())
}

macro_rules! need_rulec {
    () => {
        if !rulec_available() {
            eprintln!("SKIP: rulec is not on the PATH; set DANDORI_RULEC to run this test");
            return;
        }
    };
}

fn lang(tag: &str) -> Lang {
    if tag == "ja" {
        Lang::Ja
    } else {
        Lang::En
    }
}

/// The examples in the order the site lists them, and the versions of each in the order it
/// lists them.
const EXAMPLES: [&str; 5] = ["hotel", "order", "fulfillment", "inquiry", "review"];
const VERSIONS: [&str; 4] = ["temporal", "aws", "pydantic-graph", "argo"];

/// The flows the playground opens, for each language: a first draft whose check finds errors,
/// then every version of every example, and the flows beside the versions. The Japanese page
/// opens the same draft, as the Japanese pages show it, and the Japanese versions.
fn presets() -> Vec<(&'static str, Vec<PathBuf>)> {
    let mut out = Vec::new();
    for tag in ["en", "ja"] {
        let mine = |p: &Path| {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            name.ends_with(".flow") && (name.ends_with(".ja.flow") == (tag == "ja"))
        };
        let mut list = vec![root().join("tests/fixtures/hotel_naive.flow")];
        for ex in EXAMPLES {
            for v in VERSIONS {
                let dir = root().join(format!("examples/{ex}/{v}"));
                let mut found: Vec<PathBuf> = std::fs::read_dir(&dir).map(|rd| rd.map(|e| e.unwrap().path()).filter(|p| mine(p)).collect()).unwrap_or_default();
                found.sort();
                list.extend(found);
            }
            let mut beside: Vec<PathBuf> = std::fs::read_dir(root().join(format!("examples/{ex}"))).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_file() && mine(p)).collect();
            beside.sort();
            list.extend(beside);
        }
        out.push((tag, list));
    }
    out
}

/// The platform a version is written for, which the page builds for when it opens the version:
/// the one its directory names. A version for AWS is built for Step Functions, unless it asks for
/// what Step Functions cannot run; then for Lambda durable functions.
fn target_for(path: &str, m: Option<&dandori::model::Model>) -> &'static str {
    if path.contains("/aws/") {
        match m {
            Some(m) if dandori::commands::build(m, "asl").unwrap().is_err() => "durable",
            _ => "asl",
        }
    } else if path.contains("/pydantic-graph/") {
        "pydantic-graph"
    } else if path.contains("/argo/") {
        "argo"
    } else {
        "temporal"
    }
}

/// What presets.json holds: the flows the page opens, and every file the page reads of them with
/// what rulec printed for their rules, by their paths from the repository's root. What the page
/// reads is read here by the same functions: checking a flow reads the rules and the APIs it names
/// and its child flows; drawing it, what `rulec doc` renders for its rules in the page's language;
/// the rules tab, the rules' files.
fn record() -> String {
    let rec = Rc::new(Recorder::new(&root()));
    let mut flows = serde_json::Map::new();
    sources::with(rec.clone(), || {
        for (tag, list) in presets() {
            let mut entries = Vec::new();
            for f in list {
                let (_, checked) = dandori::check::check_file(&f).unwrap();
                let path = rel(&f);
                entries.push(json!({ "path": path, "target": target_for(&path, checked.model.as_ref()) }));
                let r = Request { path: f.to_string_lossy().into_owned(), source: String::new(), lang: lang(tag) };
                playground::doc_here(&r);
                playground::rules_here(&r);
            }
            flows.insert(tag.to_string(), Value::Array(entries));
        }
    });
    let got = rec.got.borrow().clone();
    let v = json!({ "flows": flows, "files": got.files, "rulec": got.rulec });
    laid_out(&v, 3, 0) + "\n"
}

/// JSON laid out to a depth, with each value below it on one line: a flow the page opens, a file,
/// and what one command of rulec printed for a rule. A change to one of them is a change of a line.
fn laid_out(v: &Value, depth: usize, indent: usize) -> String {
    let (pad, end) = ("  ".repeat(indent + 1), "  ".repeat(indent));
    match v {
        Value::Object(m) if depth > 0 && !m.is_empty() => {
            let items: Vec<String> = m.iter().map(|(k, x)| format!("{pad}{}: {}", Value::String(k.clone()), laid_out(x, depth - 1, indent + 1))).collect();
            format!("{{\n{}\n{end}}}", items.join(",\n"))
        }
        Value::Array(a) if depth > 0 && !a.is_empty() => {
            let items: Vec<String> = a.iter().map(|x| format!("{pad}{}", laid_out(x, depth - 1, indent + 1))).collect();
            format!("[\n{}\n{end}]", items.join(",\n"))
        }
        _ => serde_json::to_string(v).unwrap(),
    }
}

fn committed() -> (Value, Rc<Bundle>) {
    let text = std::fs::read_to_string(site().join("presets.json")).expect("website/docs/playground/presets.json");
    let v: Value = serde_json::from_str(&text).unwrap();
    let b = Bundle::from_json(&v).unwrap();
    (v, Rc::new(b))
}

#[test]
fn every_example_is_a_preset() {
    let listed: Vec<PathBuf> = presets().into_iter().flat_map(|(_, l)| l).collect();
    let mut stack = vec![root().join("examples")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "flow") {
                assert!(listed.contains(&p), "{} is not among the flows the playground opens", rel(&p));
            }
        }
    }
}

/// Whether what rulec printed in a presets.json says what a rule's service calls the values of its
/// enums (`connect.enums` in `rulec api`), which rulec 0.22.0 does and 0.21.2 and before do not.
fn names_enums(presets: &str) -> bool {
    serde_json::from_str::<Value>(presets).ok().is_some_and(|v| v["rulec"].as_object().is_some_and(|r| r.values().any(|o| o["api"]["connect"]["enums"].is_array())))
}

#[test]
fn presets_are_what_the_examples_read() {
    need_rulec!();
    let now = record();
    let file = site().join("presets.json");
    let was = std::fs::read_to_string(&file).unwrap_or_default();
    // the presets are recorded with the rulec the site is built with; one that says less is not held to them, nor writes them
    if was != now && names_enums(&was) && !names_enums(&now) {
        eprintln!("SKIP: website/docs/playground/presets.json was recorded with a rulec whose `rulec api` says what a rule's service calls the values of its enums (`connect.enums`), and this one does not; it is held to the presets, and records them anew, only with such a rulec");
        return;
    }
    if std::env::var("DANDORI_BLESS").is_ok() {
        std::fs::write(&file, &now).unwrap();
        return;
    }
    assert!(was == now, "website/docs/playground/presets.json is not what the examples read now; record it anew with DANDORI_BLESS=1 cargo test --test playground");
    let v: Value = serde_json::from_str(&now).unwrap();
    for k in v["files"].as_object().unwrap().keys().chain(v["rulec"].as_object().unwrap().keys()) {
        assert!(!k.starts_with('/') && !k.starts_with(".."), "presets.json names a file outside the repository: {k}");
    }
}

/// The binary, run from the repository's root on the file as it is committed.
fn dandori(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_dandori")).current_dir(root()).args(args).output().expect("could not run dandori");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The files under a directory, by their paths from it.
fn written(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((sources::key(p.strip_prefix(dir).unwrap()), std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// The page reads the examples from the bundle as the binary reads them from the disk: for every
/// flow it opens, what `check` prints, what `build` writes or refuses for every platform, and the
/// page and the Markdown `doc` writes.
#[test]
fn the_bundle_answers_as_the_command_does() {
    need_rulec!();
    // presets.json is being recorded anew by the test beside this one, and this one may read it before
    if std::env::var("DANDORI_BLESS").is_ok() {
        eprintln!("SKIP: presets.json is being recorded anew; run this test again without DANDORI_BLESS");
        return;
    }
    let scratch = std::env::temp_dir().join(format!("dandori-playground-{}", std::process::id()));
    let jobs: Vec<(&str, PathBuf)> = presets().into_iter().flat_map(|(tag, list)| list.into_iter().map(move |f| (tag, f))).collect();
    let failures: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = jobs
            .chunks(jobs.len().div_ceil(8))
            .map(|chunk| {
                let scratch = &scratch;
                s.spawn(move || {
                    let (_, bundle) = committed();
                    let mut failures = Vec::new();
                    for (tag, f) in chunk {
                        let path = rel(f);
                        let r = Request { path: path.clone(), source: std::fs::read_to_string(f).unwrap(), lang: lang(tag) };
                        let differs = |failures: &mut Vec<String>, what: &str, want: &str, got: &Value| {
                            let got = got.as_str().unwrap();
                            if want != got {
                                failures.push(format!("{path} ({tag}), {what}:\n--- the command\n{want}\n--- the page\n{got}"));
                            }
                        };
                        let (_, _, err) = dandori(&["check", &path, "--lang", tag]);
                        let got = playground::check(&bundle, &r);
                        differs(&mut failures, "check", &err, &got["text"]);
                        for t in TARGETS {
                            let out = scratch.join(format!("{}-{tag}-{t}", path.replace('/', "-")));
                            let (code, _, err) = dandori(&["build", &path, "--target", t, "--out", out.to_str().unwrap(), "--lang", tag]);
                            let got = playground::build(&bundle, &r, t);
                            differs(&mut failures, &format!("build --target {t}"), &err, &got["text"]);
                            let mut files: Vec<(String, String)> = got["files"].as_array().unwrap().iter().map(|f| (f["path"].as_str().unwrap().to_string(), f["body"].as_str().unwrap().to_string())).collect();
                            files.sort();
                            let want = written(&out);
                            if (code == 0) != got["ok"].as_bool().unwrap() || files != want {
                                failures.push(format!("{path} ({tag}), build --target {t}: the command wrote {} file(s) (exit {code}), the page {}", want.len(), files.len()));
                            }
                        }
                        let (_, page, err) = dandori(&["doc", &path, "--format", "html", "--lang", tag]);
                        let (_, md, _) = dandori(&["doc", &path, "--lang", tag]);
                        let got = playground::doc(&bundle, &r);
                        differs(&mut failures, "doc (the diagnostics)", &err, &got["text"]);
                        differs(&mut failures, "doc --format html", &page, &got["html"]);
                        differs(&mut failures, "doc", &md, &got["markdown"]);
                        // the rules tab has no command: it reads the disk and rulec as the page reads the bundle
                        let disk = sources::with(Rc::new(sources::Disk), || playground::rules_here(&r));
                        let got = playground::rules(&bundle, &r);
                        if disk != got {
                            failures.push(format!("{path} ({tag}), the rules:\n--- the disk\n{disk:#}\n--- the page\n{got:#}"));
                        }
                        if got["rules"].as_array().unwrap().iter().any(|x| x["page"].is_null()) {
                            failures.push(format!("{path} ({tag}): a rule has no page: {got:#}"));
                        }
                    }
                    failures
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    eprintln!("compared: {} flows, each checked, built for {} platforms, drawn and its rules shown, from the bundle and by the command", jobs.len(), TARGETS.len());
}

/// The page's side of the boundary: allocate, write, call, read the length out of the header,
/// free. It is here rather than in a committed file because it is a test's: playground.js is the
/// copy that ships, and if the two ever disagree about the convention, this one fails.
const DRIVER: &str = r#"
import { readFileSync, writeFileSync } from "node:fs";
const [wasmPath, bundlePath, requestsPath, outPath] = process.argv.slice(2);
const inst = await WebAssembly.instantiate(await WebAssembly.compile(readFileSync(wasmPath)), {});
const e = inst.exports;
function put(text) {
  const bytes = Buffer.from(text, "utf8");
  const ptr = e.dandori_alloc(bytes.length);
  new Uint8Array(e.memory.buffer, ptr + 4, bytes.length).set(bytes);
  return ptr;
}
function take(ptr) {
  const len = new DataView(e.memory.buffer).getUint32(ptr, true);
  const text = Buffer.from(new Uint8Array(e.memory.buffer, ptr + 4, len)).toString("utf8");
  e.dandori_free(ptr);
  return text;
}
function call(fn, text) {
  const ptr = put(text);
  try {
    return take(e[fn](ptr));
  } finally {
    e.dandori_free(ptr);
  }
}
const version = take(e.dandori_version());
const before = call("dandori_check", "{}");
const kept = call("dandori_bundle", readFileSync(bundlePath, "utf8"));
const answers = JSON.parse(readFileSync(requestsPath, "utf8")).map(([what, req]) => call("dandori_" + what, req));
writeFileSync(outPath, JSON.stringify({ version, before, kept, answers }));
"#;

/// Edits a reader might make, each reaching a way the page answers that the flows as they are do
/// not: a flow that does not parse, a rule and a child flow the page does not have, a flow that
/// runs itself, a file that is not in the bundle at all, and a draft put right.
fn edits() -> Vec<(String, String, &'static str)> {
    let read = |p: &str| std::fs::read_to_string(root().join(p)).unwrap();
    let swap = |p: &str, from: &str, to: &str| {
        let src = read(p);
        assert!(src.contains(from), "{p} no longer has `{from}`");
        src.replacen(from, to, 1)
    };
    let hotel = "examples/hotel/temporal/hotel.flow";
    let fulfillment = "examples/fulfillment/temporal/fulfillment.flow";
    let draft = "tests/fixtures/hotel_naive.flow";
    vec![
        (hotel.into(), swap(hotel, "record Booking", "record Booking ="), "en"),
        (hotel.into(), swap(hotel, "\"../rules/hold_amount.rule\"", "\"../rules/deposit.rule\""), "en"),
        (hotel.into(), swap(hotel, "\"../rules/hold_amount.rule\"", "\"../rules/deposit.rule\""), "ja"),
        (fulfillment.into(), swap(fulfillment, "flow \"../arrange_delivery.flow\"", "flow \"fulfillment.flow\""), "en"),
        (fulfillment.into(), swap(fulfillment, "flow \"../arrange_delivery.flow\"", "flow \"../arrange.flow\""), "ja"),
        ("scratch/new.flow".into(), "workflow new v1\n\ninputs\n  n : int\n\nflow\n  pass\n".into(), "en"),
        (draft.into(), swap(draft, "    requires_payment_method => fail CardDeclined \"The card was declined\"", "    requires_payment_method => fail CardDeclined \"The card was declined\" leaving pi"), "en"),
    ]
}

#[test]
fn the_module_answers_as_the_library_does() {
    let wasm = site().join("dandori.wasm");
    if !node_available() || !wasm.exists() {
        eprintln!("SKIP: node or website/docs/playground/dandori.wasm is missing");
        return;
    }
    let (v, bundle) = committed();
    let mut requests: Vec<(String, String)> = Vec::new();
    let ask = |requests: &mut Vec<(String, String)>, path: &str, source: &str, tag: &str| {
        requests.push(("check".into(), json!({ "path": path, "source": source, "lang": tag }).to_string()));
        for t in TARGETS {
            requests.push(("build".into(), json!({ "path": path, "source": source, "lang": tag, "target": t }).to_string()));
        }
        requests.push(("doc".into(), json!({ "path": path, "source": source, "lang": tag }).to_string()));
        requests.push(("rules".into(), json!({ "path": path, "source": source, "lang": tag }).to_string()));
    };
    for (tag, list) in v["flows"].as_object().unwrap() {
        for f in list.as_array().unwrap() {
            let path = f["path"].as_str().unwrap();
            ask(&mut requests, path, &bundle.files[path], tag);
        }
    }
    for (path, source, tag) in edits() {
        ask(&mut requests, &path, &source, tag);
    }
    requests.push(("build".into(), json!({ "path": "scratch/new.flow", "source": "", "lang": "en", "target": "cobol" }).to_string()));
    requests.push(("check".into(), "not a request".into()));

    let dir = std::env::temp_dir().join(format!("dandori-wasm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("driver.mjs"), DRIVER).unwrap();
    std::fs::write(dir.join("requests.json"), serde_json::to_string(&requests).unwrap()).unwrap();
    let o = Command::new("node")
        .current_dir(&dir)
        .args(["driver.mjs", wasm.to_str().unwrap(), site().join("presets.json").to_str().unwrap(), "requests.json", "answers.json"])
        .output()
        .expect("could not run node");
    assert!(o.status.success(), "the driver failed: {}", String::from_utf8_lossy(&o.stderr));
    let got: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("answers.json")).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(got["version"], env!("CARGO_PKG_VERSION"), "website/docs/playground/dandori.wasm is of another version; run website/tools/make_wasm.sh");
    assert!(got["before"].as_str().unwrap().contains("the bundle has not been handed over"), "before the bundle: {}", got["before"]);
    assert_eq!(got["kept"], "", "the module did not keep the bundle");
    let answers = got["answers"].as_array().unwrap();
    assert_eq!(answers.len(), requests.len());
    let mut stale = Vec::new();
    for ((what, req), a) in requests.iter().zip(answers) {
        let want = playground::answer(&bundle, what, req);
        if a.as_str() != Some(want.as_str()) {
            let r: Value = serde_json::from_str(req).unwrap_or(Value::Null);
            stale.push(format!("{what} {} ({}) {}", r["path"], r["lang"], r["target"]));
        }
    }
    assert!(
        stale.is_empty(),
        "website/docs/playground/dandori.wasm answers otherwise than the library now; run website/tools/make_wasm.sh:\n{}",
        stale.join("\n")
    );
    eprintln!("compared: {} requests answered by the module as by the library", requests.len());
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

/// The DOM of a page after its scripts ran, as headless Chrome dumps it.
fn dump_dom(chrome: &str, url: &str) -> String {
    let out = Command::new(chrome)
        .args(["--headless=new", "--disable-gpu", "--no-first-run", "--no-default-browser-check", "--virtual-time-budget=20000", "--dump-dom", url])
        .output()
        .expect("could not run chrome");
    String::from_utf8_lossy(&out.stdout).into_owned()
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

/// The page starts in Chrome, as the site's pages have it: it loads the module and the bundle,
/// shows what `check` prints for the flow it opens, and follows a link to a flow, a tab and a
/// platform.
#[test]
fn the_page_starts_in_chrome() {
    let Some(chrome) = chrome() else {
        eprintln!("SKIP: Chrome is not found; set DANDORI_CHROME to run this test");
        return;
    };
    if !site().join("dandori.wasm").exists() {
        eprintln!("SKIP: website/docs/playground/dandori.wasm is missing; run website/tools/make_wasm.sh");
        return;
    }
    let (_, bundle) = committed();
    let draft = "tests/fixtures/hotel_naive.flow";
    let mut looked = 0;
    for (tag, page, hotel, order) in [
        ("en", "website/docs/playground.md", "examples/hotel/temporal/hotel.flow", "examples/order/temporal/order.flow"),
        ("ja", "website/docs-ja/playground.md", "examples/hotel/temporal/hotel.ja.flow", "examples/order/temporal/order.ja.flow"),
    ] {
        // the widget as the page has it, with the script served from here
        let md = std::fs::read_to_string(root().join(page)).unwrap();
        let from = md.find("<div class=\"pg\"").expect("the page has no playground");
        let tail = "<script src=\"playground/playground.js\" defer></script>";
        let to = md[from..].find(tail).expect("the page does not load playground.js") + from + tail.len();
        let widget = md[from..to].replace("playground/playground.js", "/playground.js");
        let port = serve(format!("<!doctype html><html><head><meta charset=\"utf-8\"></head><body>{widget}</body></html>"));
        let ask = |path: &str, target: Option<&str>| {
            let r = Request { path: path.into(), source: bundle.files[path].clone(), lang: lang(tag) };
            match target {
                Some(t) => playground::build(&bundle, &r, t),
                None => playground::check(&bundle, &r),
            }
        };

        let dom = dump_dom(&chrome, &format!("http://127.0.0.1:{port}/"));
        let status = text_in(&dom, "pg-status", "</span>");
        assert!(status.starts_with(&format!("dandori {}", env!("CARGO_PKG_VERSION"))), "{page}: the status says {status:?}");
        let counts = if tag == "ja" { "エラー 4 件、警告 2 件" } else { "4 errors, 2 warnings" };
        assert!(status.ends_with(counts), "{page}: the status says {status:?}");
        assert_eq!(text_in(&dom, "pg-out", "</div>"), ask(draft, None)["text"].as_str().unwrap(), "{page}: the page shows another check of {draft}");

        let dom = dump_dom(&chrome, &format!("http://127.0.0.1:{port}/#flow={hotel}&view=build&target=asl"));
        let want = ask(hotel, Some("asl"));
        assert!(!want["ok"].as_bool().unwrap());
        let shown = text_in(&dom, "pg-out", "</div>");
        assert!(shown.ends_with(want["text"].as_str().unwrap()), "{page}: the page shows another refusal of {hotel} for Step Functions:\n{shown}");

        let dom = dump_dom(&chrome, &format!("http://127.0.0.1:{port}/#flow={order}&view=doc"));
        assert!(dom.contains("class=\"pg-open\" href=\"blob:"), "{page}: the page has no link to the page doc draws of {order}");

        let dom = dump_dom(&chrome, &format!("http://127.0.0.1:{port}/#flow={hotel}&view=rules"));
        let want = playground::rules(&bundle, &Request { path: hotel.into(), source: bundle.files[hotel].clone(), lang: lang(tag) });
        let rules = want["rules"].as_array().unwrap();
        assert!(!rules.is_empty(), "{hotel} calls no rule");
        assert_eq!(dom.matches("class=\"pg-rule\"").count(), rules.len(), "{page}: the page shows another number of rules of {hotel}");
        assert_eq!(dom.matches("class=\"pg-open\" href=\"blob:").count(), rules.len(), "{page}: a rule of {hotel} has no link to its page");
        let shown = text_in(&dom, "pg-out", "</div>\n</div>");
        for r in rules {
            assert!(shown.contains(r["source"].as_str().unwrap()), "{page}: the page does not show the text of {}", r["file"]);
        }
        looked += 4;
    }
    eprintln!("compared: {looked} views of the page in Chrome with what the library answers");
}
