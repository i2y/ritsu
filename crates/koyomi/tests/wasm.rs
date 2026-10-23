//! The Wasm target (DESIGN 13): what `gen --target wasm` writes, the module `koyomi api`'s build
//! line makes of it answering a line of `koyomi vectors` with that line, an input outside the
//! contract answered with a refusal rather than a trap, the component the `.wit` makes of the
//! module and wasmtime calls, and a world WIT reads for every alias. tests/targets.rs holds the
//! module to the reference interpreter on every line of every file, through the runner `gen`
//! writes. A tool that is missing (rustc's wasm32-unknown-unknown target, node, wasm-tools,
//! wasmtime) prints `SKIP:` and the test passes.

mod common;

use koyomi::check::{Checked, check};
use koyomi::codegen::{self, wasm};
use koyomi::naming::Target;
use koyomi::vectors::{self, Expect, Row};
use ritsu_base::text::Lang;
use ritsu_testkit::run::{Ran, pipe, run};
use ritsu_testkit::tools::{on_path, runs};
use ritsu_testkit::{Need, TempDir, ready};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

/// How long one program may take before it is killed.
const LIMIT: Duration = Duration::from_secs(600);

fn toolchain() -> bool {
    runs("rustc", &["--version"]) && common::rust_target("wasm32-unknown-unknown") && runs("node", &["--version"])
}

const NO_TOOLCHAIN: &str = "rustc, its wasm32-unknown-unknown target (rustup target add wasm32-unknown-unknown) or node is missing; the Wasm module is not built";

fn checked(p: &str) -> Checked {
    let mut o = check(p).unwrap();
    assert!(!o.has_errors(), "{p} passes check");
    o.checked.take().unwrap()
}

fn koyomi(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_koyomi")).args(args).env_remove("KOYOMI_LANG").env_remove("RITSU_LANG").output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// `koyomi api` of a checked file, from the library.
fn api(c: &Checked) -> Value {
    match c {
        Checked::Dates(m, _) => koyomi::api::dates_json(m),
        Checked::Calendar(cal) => koyomi::api::calendar_file_json(cal),
    }
}

/// The Wasm target of a checked file in `lang`, written under `dir`: the alias.
fn generate(dir: &Path, c: &Checked, lang: Lang) -> String {
    let u = codegen::unit_of(c, lang);
    for (rel, body) in codegen::files(&u, Target::Wasm) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }
    u.alias
}

fn ok_or_fail(what: &str, r: &Ran) {
    assert!(r.ok && !r.timed_out, "{what} failed:\n{}{}", r.stdout, r.stderr);
}

/// Build the module in `dir/wasm` with the line `koyomi api` gives under `wasm.build`: the module.
fn build(dir: &Path, c: &Checked) -> PathBuf {
    let a = api(c);
    let line = a["wasm"]["build"].as_str().unwrap().to_string();
    let words: Vec<&str> = line.split(' ').collect();
    let w = dir.join("wasm");
    ok_or_fail(&line, &run(Command::new(words[0]).current_dir(&w).args(&words[1..]), LIMIT));
    w.join(a["wasm"]["module"].as_str().unwrap())
}

/// A host of its own, as generated-code.md shows one: `call` through the canonical ABI, on each
/// line of standard input, the answer printed as it is. It refuses a module that imports
/// anything.
const HOST: &str = r#"import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";

const mod = new WebAssembly.Module(readFileSync(process.argv[2]));
if (WebAssembly.Module.imports(mod).length !== 0) throw new Error("the module imports " + JSON.stringify(WebAssembly.Module.imports(mod)));
const ex = new WebAssembly.Instance(mod, {}).exports;
const enc = new TextEncoder();
const dec = new TextDecoder();

function call(bytes) {
  const ptr = ex.cabi_realloc(0, 0, 1, bytes.length);
  new Uint8Array(ex.memory.buffer, ptr, bytes.length).set(bytes);
  const ret = ex.call(ptr, bytes.length);
  const [p, n] = new Uint32Array(ex.memory.buffer, ret, 2);
  const out = dec.decode(new Uint8Array(ex.memory.buffer, p, n));
  ex.cabi_post_call(ret);
  return out;
}

if (process.argv[3] === "--requests") {
  // A JSON array of requests, each a string or {"bytes": [...]}; then the first one called again
  // and again, with the memory's size before and after.
  const requests = JSON.parse(readFileSync(0, "utf8"));
  const bytes = (r) => (typeof r === "string" ? enc.encode(r) : new Uint8Array(r.bytes));
  for (const r of requests) console.log(call(bytes(r)));
  const first = bytes(requests[0]);
  for (let i = 0; i < 100; i++) call(first);
  const before = ex.memory.buffer.byteLength;
  for (let i = 0; i < 20000; i++) call(first);
  console.log("memory " + before + " " + ex.memory.buffer.byteLength);
} else {
  let buf = [];
  for await (const line of createInterface({ input: process.stdin, crlfDelay: Infinity })) {
    if (line === "") continue;
    buf.push(call(enc.encode(line)));
    if (buf.length >= 4096) {
      process.stdout.write(buf.join("\n") + "\n");
      buf = [];
    }
  }
  if (buf.length > 0) process.stdout.write(buf.join("\n") + "\n");
}
"#;

fn host(dir: &Path) -> PathBuf {
    let p = dir.join("host.mjs");
    std::fs::write(&p, HOST).unwrap();
    p
}

#[test]
fn gen_writes_the_crate_root_the_module_the_world_and_the_runner() {
    let t = TempDir::new("wasm-gen");
    let out = t.path().to_string_lossy().to_string();
    let (c, o, e) = koyomi(&["gen", "examples/payment_20th_close_next_10th.cal", "--out", &out]);
    assert_eq!(c, 0, "{o}{e}");
    let a = "payment_20th_close_next_10th";
    for f in [format!("{a}_wasm.rs"), format!("{a}.rs"), format!("{a}.wit"), format!("{a}_runner.mjs")] {
        assert!(t.path().join("wasm").join(&f).is_file(), "wasm/{f} is not written");
    }
    // The module in wasm/ is the one rust/ gets, byte for byte.
    assert_eq!(t.read(&format!("wasm/{a}.rs")), t.read(&format!("rust/{a}.rs")));
    let root = t.read(&format!("wasm/{a}_wasm.rs"));
    assert!(root.starts_with("// Code generated by koyomi "), "{root}");
    assert!(root.contains(&format!("#[path = \"{a}.rs\"]\nmod k;\n")), "{root}");
    for export in ["pub unsafe extern \"C\" fn cabi_realloc(", "pub unsafe extern \"C\" fn call(", "pub unsafe extern \"C\" fn cabi_post_call("] {
        assert!(root.contains(export), "the crate root has no `{export}`");
    }
    assert!(!root.contains("extern \"C\" {"), "the crate root imports a function:\n{root}");
    let wit = t.read(&format!("wasm/{a}.wit"));
    assert!(wit.contains("package koyomi:payment-20th-close-next-10th@1.0.0;\n"), "{wit}");
    assert!(wit.contains("world payment-20th-close-next-10th {\n"), "{wit}");
    assert!(wit.contains("    export call: func(input: string) -> string;\n}\n"), "{wit}");
    // What to send and what comes back are named for whoever reads the interface.
    assert!(wit.contains("    ///   received: a date (YYYY-MM-DD), 2026-01-01..2027-11-20\n"), "{wit}");
    assert!(wit.contains("    ///   payment.at: a time, RFC 3339 in UTC, ending in Z\n"), "{wit}");
    let runner = t.read(&format!("wasm/{a}_runner.mjs"));
    assert!(runner.contains(&format!("join(here, \"{a}.wasm\")")), "{runner}");
    // In Japanese, the comments and the sentences of the refusals.
    let ja = TempDir::new("wasm-gen-ja");
    let (c, o, e) = koyomi(&["gen", "examples/payment_20th_close_next_10th.ja.cal", "--target", "wasm", "--lang", "ja", "--out", &ja.path().to_string_lossy()]);
    assert_eq!(c, 0, "{o}{e}");
    let wit = ja.read("wasm/payment_terms.wit");
    assert!(wit.contains("    ///   受領日: 日付（YYYY-MM-DD）、2026-01-01〜2027-11-20\n"), "{wit}");
    let root = ja.read("wasm/payment_terms_wasm.rs");
    assert!(root.contains("_refusal(\"input\", \"入力が JSON のオブジェクトではありません\")"), "{root}");
    assert!(root.contains("format!(\"{name} がありません\")"), "{root}");
}

/// The lines of a file's vectors the module is held to as JSON: all of them for a file of fewer
/// than 100,000, else the first 2,000 and the inputs just outside the range; and for a dates file
/// with a calendar, the calendar's first 400 days and the days just outside what it knows.
fn lines_of(c: &Checked) -> Vec<(String, Row)> {
    let mut out = Vec::new();
    match c {
        Checked::Dates(m, _) => {
            let w = vectors::DatesLines::new(m);
            let outside = vectors::outside(m);
            if m.combinations() + (outside.len() as u128) < 100_000 {
                out.extend(vectors::DatesRows::new(m).map(|r| (w.line(&r), r)));
            } else {
                out.extend(vectors::DatesRows::new(m).take(2_000).map(|r| (w.line(&r), r)));
                out.extend(outside.into_iter().map(|vals| Row { vals, expect: Expect::Error("range") }).map(|r| (w.line(&r), r)));
            }
            if let Some(cal) = &m.cal {
                out.extend(vectors::calendar_rows(cal).enumerate().filter(|(i, r)| *i < 400 || matches!(r.expect, Expect::Error(_))).map(|(_, r)| (vectors::calendar_line(&r), r)));
            }
        }
        Checked::Calendar(cal) => {
            let rows: Vec<Row> = vectors::calendar_rows(cal).collect();
            let all = rows.len() < 100_000;
            out.extend(rows.into_iter().enumerate().filter(|(i, r)| all || *i < 2_000 || matches!(r.expect, Expect::Error(_))).map(|(_, r)| (vectors::calendar_line(&r), r)));
        }
    }
    out
}

/// What the module answers a line of the vectors with: the line itself, with `in` holding only
/// the inputs some date takes (the module reads no other), or for a line that expects an error,
/// a refusal of that kind.
fn agrees(c: &Checked, line: &str, r: &Row, got: &str) -> Result<(), String> {
    let want: Value = serde_json::from_str(line).unwrap();
    if let Expect::Error(kind) = r.expect {
        let a: Value = serde_json::from_str(got).map_err(|e| format!("{e}: {got}"))?;
        let keys: Vec<&String> = a.as_object().map(|o| o.keys().collect()).unwrap_or_default();
        if keys != ["refused", "error"] || a["refused"] != kind || a["error"].as_str().is_none_or(str::is_empty) {
            return Err(format!("{line}\n  wants a refusal of kind {kind}, and the module answers\n  {got}"));
        }
        return Ok(());
    }
    let mut want = want;
    if let (Checked::Dates(m, _), false) = (c, line.starts_with("{\"in\":{\"date\":")) {
        let taken: Vec<&str> = m.inputs.iter().enumerate().filter(|(k, _)| m.dates.iter().any(|d| d.params.contains(k))).map(|(_, i)| i.name.as_str()).collect();
        want["in"].as_object_mut().unwrap().retain(|k, _| taken.contains(&k.as_str()));
    }
    let want = serde_json::to_string(&want).unwrap();
    if got != want {
        return Err(format!("{line}\n  wants {want}\n  and the module answers\n  {got}"));
    }
    Ok(())
}

#[test]
fn the_module_answers_a_line_of_its_vectors_with_that_line() {
    if !ready(Need::Rustc, toolchain, NO_TOOLCHAIN) || !ready(Need::Node, toolchain, NO_TOOLCHAIN) {
        return;
    }
    let t = TempDir::new("wasm-lines");
    let host = host(t.path());
    let files: Vec<(&str, Checked)> = common::TARGET_FILES.iter().map(|p| (*p, checked(p))).collect();
    let queue: Mutex<Vec<&(&str, Checked)>> = Mutex::new(files.iter().rev().collect());
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let total = Mutex::new(0usize);
    std::thread::scope(|sc| {
        for _ in 0..4 {
            sc.spawn(|| {
                loop {
                    let Some((p, c)) = queue.lock().unwrap().pop() else { break };
                    let dir = t.path().join(p.replace('/', "_"));
                    let lang = if p.ends_with(".ja.cal") || !p.is_ascii() { Lang::Ja } else { Lang::En };
                    generate(&dir, c, lang);
                    let module = build(&dir, c);
                    let lines = lines_of(c);
                    let mut it = lines.iter();
                    let mut wrong = Vec::new();
                    let mut n = 0usize;
                    let ran = pipe(
                        Command::new("node").arg(&host).arg(&module),
                        LIMIT,
                        |w| {
                            for (l, _) in &lines {
                                if writeln!(w, "{l}").is_err() {
                                    return;
                                }
                            }
                        },
                        |got| match it.next() {
                            Some((l, r)) => {
                                n += 1;
                                if let Err(e) = agrees(c, l, r, got)
                                    && wrong.len() < 3
                                {
                                    wrong.push(e);
                                }
                            }
                            None => wrong.push(format!("an answer too many: {got}")),
                        },
                    );
                    if !ran.ok || n != lines.len() || !wrong.is_empty() {
                        failures.lock().unwrap().push(format!("{p}: {n} answers to {} lines (exit ok: {})\n{}\n{}", lines.len(), ran.ok, wrong.join("\n"), ran.stderr));
                    } else {
                        println!("answered {p}: {n} lines, each with the line of its vectors");
                        *total.lock().unwrap() += n;
                    }
                }
            });
        }
    });
    let f = failures.into_inner().unwrap();
    assert!(f.is_empty(), "the Wasm module does not answer as the vectors say:\n{}", f.join("\n"));
    println!("wasm: {} lines answered as JSON, in {} files", total.into_inner().unwrap(), files.len());
}

/// Call a module with each of `requests` (a string, or the bytes of one), then the first again and
/// again: the answers, and whether the memory stayed the size it was.
fn ask(host: &Path, module: &Path, requests: &[Value]) -> (Vec<String>, bool) {
    let input = serde_json::to_string(requests).unwrap();
    let r = ritsu_testkit::run::run_with_input(Command::new("node").arg(host).arg(module).arg("--requests"), Some(input.as_bytes()), LIMIT);
    ok_or_fail("the host", &r);
    let mut lines: Vec<String> = r.stdout.lines().map(str::to_string).collect();
    let memory = lines.pop().unwrap();
    let sizes: Vec<&str> = memory.split(' ').collect();
    assert_eq!(sizes[0], "memory", "{}", r.stdout);
    (lines, sizes[1] == sizes[2])
}

/// `\u` escapes for every character of `s`, as an encoder that escapes everything writes it.
fn escaped(s: &str) -> String {
    s.encode_utf16().map(|u| format!("\\u{u:04x}")).collect()
}

#[test]
fn an_input_outside_the_contract_is_answered_with_a_refusal() {
    if !ready(Need::Rustc, toolchain, NO_TOOLCHAIN) || !ready(Need::Node, toolchain, NO_TOOLCHAIN) {
        return;
    }
    let t = TempDir::new("wasm-contract");
    let host = host(t.path());
    let s = |x: &str| Value::String(x.to_string());
    let ok = r#"{"in":{"received":"2026-04-01"},"out":{"closing":"2026-04-20","payment":"2026-05-08","payment.at":"2026-05-08T00:00:00Z"}}"#;
    let deep = format!("{}{}", "[".repeat(100), "]".repeat(100));
    let cases: Vec<(&str, Lang, Vec<(Value, String)>)> = vec![
        (
            "examples/payment_20th_close_next_10th.cal",
            Lang::En,
            vec![
                (s(r#"{"received":"2026-04-01"}"#), ok.into()),
                // A line of the vectors, with the inputs under "in", and the keys spaced and escaped
                // as Python's json.dumps writes them unless told otherwise.
                (s(r#"{"in":{"received":"2026-04-01"},"out":{"closing":"2026-04-20","payment":"2026-05-08","payment.at":"2026-05-08T00:00:00Z"}}"#), ok.into()),
                (s(&format!("{{\"{}\": \"2026-04-01\"}}", escaped("received"))), ok.into()),
                // A key written twice counts as its last value; "in" holding no object is not the inputs.
                (s(r#"{"received":"2025-01-01","received":"2026-04-01"}"#), ok.into()),
                (s(r#"{"in":"2026-04-01","received":"2026-04-01"}"#), ok.into()),
                (s(r#"{"received":"2025-12-31"}"#), r#"{"refused":"range","error":"received 2025-12-31 is outside its range 2026-01-01..2027-11-20"}"#.into()),
                (s(r#"{"received":"2026-02-30"}"#), r#"{"refused":"date","error":"2026-02-30 is not a date of 0001-01-01..9999-12-31 written YYYY-MM-DD"}"#.into()),
                (s(r#"{"received":"2026-4-1"}"#), r#"{"refused":"date","error":"2026-4-1 is not a date of 0001-01-01..9999-12-31 written YYYY-MM-DD"}"#.into()),
                (s(r#"{"received":20260401}"#), r#"{"refused":"input","error":"received is not a string"}"#.into()),
                (s(r#"{"received":null}"#), r#"{"refused":"input","error":"received is missing"}"#.into()),
                (s("{}"), r#"{"refused":"input","error":"received is missing"}"#.into()),
                // The calendar the file reads, asked the way its own vectors ask it.
                (s(r#"{"date":"2026-05-04"}"#), r#"{"in":{"date":"2026-05-04"},"out":{"open":false}}"#.into()),
                (s(r#"{"in":{"date":"2026-05-07"}}"#), r#"{"in":{"date":"2026-05-07"},"out":{"open":true}}"#.into()),
                (s(r#"{"date":"2028-01-01"}"#), r#"{"refused":"data","error":"whether 2028-01-01 is a business day is outside what the calendar knows, 1955-01-01..2027-12-31"}"#.into()),
                (s(r#"{"date":5}"#), r#"{"refused":"input","error":"date is not a string"}"#.into()),
                // Not one JSON object: answered, never trapped.
                (s(""), r#"{"refused":"input","error":"the input is not a JSON object"}"#.into()),
                (s("[]"), r#"{"refused":"input","error":"the input is not a JSON object"}"#.into()),
                (s(r#""received""#), r#"{"refused":"input","error":"the input is not a JSON object"}"#.into()),
                (s(r#"{"received":"2026-04-01",}"#), r#"{"refused":"input","error":"the input is not a JSON object"}"#.into()),
                (s(r#"{"received":"2026-04-01"} {}"#), r#"{"refused":"input","error":"the input is not a JSON object"}"#.into()),
                // A control character inside a string, which JSON writes only escaped.
                (s(&format!("{{\"received\":\"2026-04-01{}\"}}", '\u{1}')), r#"{"refused":"input","error":"the input is not a JSON object"}"#.into()),
                (s(&deep), r#"{"refused":"input","error":"the input is not a JSON object"}"#.into()),
                (serde_json::json!({"bytes": [123, 34, 255, 34, 125]}), r#"{"refused":"input","error":"the input is not UTF-8"}"#.into()),
            ],
        ),
        (
            "examples/payment_20th_close_next_10th.ja.cal",
            Lang::Ja,
            vec![
                (s(r#"{"受領日":"2026-04-01"}"#), r#"{"in":{"受領日":"2026-04-01"},"out":{"締め日":"2026-04-20","支払日":"2026-05-08","支払日.at":"2026-05-08T00:00:00Z"}}"#.into()),
                (s(&format!("{{\"{}\": \"2026-04-01\"}}", escaped("受領日"))), r#"{"in":{"受領日":"2026-04-01"},"out":{"締め日":"2026-04-20","支払日":"2026-05-08","支払日.at":"2026-05-08T00:00:00Z"}}"#.into()),
                (s(r#"{"受領日":"2025-12-31"}"#), r#"{"refused":"range","error":"受領日 2025-12-31 は範囲 2026-01-01〜2027-11-20 の外です"}"#.into()),
                (s(r#"{"受領日":"2026-02-30"}"#), r#"{"refused":"date","error":"2026-02-30 は 0001-01-01〜9999-12-31 の日付（YYYY-MM-DD の形）ではありません"}"#.into()),
                (s(r#"{"受領日":1}"#), r#"{"refused":"input","error":"受領日 が文字列ではありません"}"#.into()),
                (s(r#"{"received":"2026-04-01"}"#), r#"{"refused":"input","error":"受領日 がありません"}"#.into()),
                (s("[]"), r#"{"refused":"input","error":"入力が JSON のオブジェクトではありません"}"#.into()),
                (s(r#"{"date":"2028-01-01"}"#), r#"{"refused":"data","error":"2028-01-01 が営業日かは、カレンダーが知っている範囲 1955-01-01〜2027-12-31 の外なので分かりません"}"#.into()),
                (serde_json::json!({"bytes": [255]}), r#"{"refused":"input","error":"入力が UTF-8 ではありません"}"#.into()),
            ],
        ),
        (
            "examples/close_and_pay_on_given_days.cal",
            Lang::En,
            vec![
                (s(r#"{"received":"2027-04-01","closing_day":20,"payment_month":1,"payment_day":10}"#), String::new()),
                // A whole number in any spelling JSON has is that integer.
                (s(r#"{"received":"2027-04-01","closing_day":20.0,"payment_month":1e0,"payment_day":1.0E1}"#), String::new()),
                (s(r#"{"received":"2027-04-01","closing_day":"20","payment_month":1,"payment_day":10}"#), r#"{"refused":"input","error":"closing_day is not an integer"}"#.into()),
                (s(r#"{"received":"2027-04-01","closing_day":20.5,"payment_month":1,"payment_day":10}"#), r#"{"refused":"input","error":"closing_day is not an integer"}"#.into()),
                (s(r#"{"received":"2027-04-01","closing_day":true,"payment_month":1,"payment_day":10}"#), r#"{"refused":"input","error":"closing_day is not an integer"}"#.into()),
                (s(r#"{"received":"2027-04-01","closing_day":99999999999999999999,"payment_month":1,"payment_day":10}"#), r#"{"refused":"input","error":"closing_day is not an integer"}"#.into()),
                (s(r#"{"received":"2027-04-01","closing_day":0,"payment_month":1,"payment_day":10}"#), r#"{"refused":"range","error":"closing_day 0 is outside its range 1..31"}"#.into()),
                (s(r#"{"received":"2027-04-01","closing_day":20,"payment_day":10}"#), r#"{"refused":"input","error":"payment_month is missing"}"#.into()),
            ],
        ),
        (
            "examples/calendars/england_and_wales.cal",
            Lang::En,
            vec![
                (s(r#"{"date":"2026-04-03"}"#), r#"{"in":{"date":"2026-04-03"},"out":{"open":false}}"#.into()),
                (s(r#"{"in":{"date":"2026-04-07"},"out":{"open":true}}"#), r#"{"in":{"date":"2026-04-07"},"out":{"open":true}}"#.into()),
                (s(r#"{"date":"2029-01-01"}"#), r#"{"refused":"data","error":"whether 2029-01-01 is a business day is outside what the calendar knows, 2019-01-01..2028-12-31"}"#.into()),
                (s(r#"{"day":"2026-04-03"}"#), r#"{"refused":"input","error":"date is missing"}"#.into()),
            ],
        ),
    ];
    for (p, lang, asked) in &cases {
        let c = checked(p);
        let dir = t.path().join(p.replace('/', "_"));
        generate(&dir, &c, *lang);
        let module = build(&dir, &c);
        let requests: Vec<Value> = asked.iter().map(|(r, _)| r.clone()).collect();
        let (got, steady) = ask(&host, &module, &requests);
        assert_eq!(got.len(), asked.len(), "{p}: {got:?}");
        // The two whole-number spellings answer alike.
        let first = got[0].clone();
        for ((r, want), g) in asked.iter().zip(&got) {
            let want = if want.is_empty() { &first } else { want };
            assert_eq!(g, want, "{p}: asked {r}");
        }
        assert!(first.starts_with("{\"in\":{"), "{p}: {first}");
        assert!(steady, "{p}: the memory grows with every call: the answers are not freed");
    }
}

/// The component of a module, made with the line `koyomi api` gives under `wasm.component`, and
/// what wasmtime prints for one call of it, unquoted.
fn invoke(w: &Path, component: &str, input: &str) -> String {
    let arg = format!("call(\"{}\")", input.replace('\\', "\\\\").replace('"', "\\\""));
    let r = run(Command::new("wasmtime").current_dir(w).args(["run", "--invoke", &arg, component]), LIMIT);
    ok_or_fail(&format!("wasmtime run --invoke {arg}"), &r);
    // wasmtime prints the returned string as a WAVE literal: quoted, with the quotes escaped.
    let got = r.stdout.trim();
    got.strip_prefix('"').and_then(|g| g.strip_suffix('"')).unwrap_or(got).replace("\\\"", "\"").replace("\\\\", "\\")
}

#[test]
fn the_world_makes_the_module_a_component_wasmtime_calls() {
    if !ready(Need::Rustc, || toolchain() && on_path("wasm-tools").is_some(), "rustc's wasm32-unknown-unknown target, node or wasm-tools is missing; the component of the Wasm target is not made") {
        return;
    }
    let t = TempDir::new("wasm-component");
    for (p, lang, ask, want) in [
        (
            "examples/payment_20th_close_next_10th.cal",
            Lang::En,
            r#"{"received":"2026-04-01"}"#,
            r#"{"in":{"received":"2026-04-01"},"out":{"closing":"2026-04-20","payment":"2026-05-08","payment.at":"2026-05-08T00:00:00Z"}}"#,
        ),
        (
            "examples/payment_20th_close_next_10th.ja.cal",
            Lang::Ja,
            r#"{"受領日":"2025-12-31"}"#,
            r#"{"refused":"range","error":"受領日 2025-12-31 は範囲 2026-01-01〜2027-11-20 の外です"}"#,
        ),
        ("examples/calendars/england_and_wales.cal", Lang::En, r#"{"date":"2026-12-25"}"#, r#"{"in":{"date":"2026-12-25"},"out":{"open":false}}"#),
    ] {
        let c = checked(p);
        let dir = t.path().join(p.replace('/', "_"));
        let alias = generate(&dir, &c, lang);
        build(&dir, &c);
        let w = dir.join("wasm");
        let a = api(&c);
        let line = a["wasm"]["component"].as_str().unwrap();
        ok_or_fail(line, &run(Command::new("sh").current_dir(&w).arg("-c").arg(line), LIMIT));
        let component = format!("{alias}.component.wasm");
        ok_or_fail("wasm-tools validate", &run(Command::new("wasm-tools").current_dir(&w).args(["validate", &component]), LIMIT));
        let r = run(Command::new("wasm-tools").current_dir(&w).args(["component", "wit", &component]), LIMIT);
        ok_or_fail("wasm-tools component wit", &r);
        assert!(r.stdout.contains("export call: func(input: string) -> string;"), "{p}: the component's world:\n{}", r.stdout);
        if !ready(Need::Rustc, || on_path("wasmtime").is_some(), "wasmtime is missing; the component of the Wasm target is made but not called") {
            return;
        }
        assert_eq!(invoke(&w, &component, ask), want, "{p}");
        println!("wasmtime called the component of {p}");
    }
}

/// The aliases that are words of WIT, and the shapes of an alias WIT spells otherwise: a word of
/// digits after the first, a run of `_`, a `_` at the end.
fn aliases() -> Vec<String> {
    let mut v: Vec<String> = wasm::WIT_KEYWORDS.iter().map(|k| k.replace('-', "_")).collect();
    v.extend(["close_20th", "net_30", "a__b", "trailing_", "x_1_2", "payment_terms"].map(String::from));
    v
}

#[test]
fn every_alias_gives_a_world_wit_reads() {
    if !ready(Need::Rustc, || on_path("wasm-tools").is_some(), "wasm-tools is missing; the worlds of the Wasm target are not read") {
        return;
    }
    let t = TempDir::new("wasm-worlds");
    let mut read = Vec::new();
    let mut refused = Vec::new();
    let cases: Vec<(String, String, String)> = aliases()
        .into_iter()
        .flat_map(|a| {
            [
                (a.clone(), format!("dates {a} v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate later = d\n  + 1 day\n"), "1.0.0".to_string()),
                (a.clone(), format!("calendar {a} v1\n\nclosed weekly sat, sun\n"), "1.0.0".to_string()),
            ]
        })
        .chain([
            ("leading_zeros".to_string(), "dates leading_zeros v007\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate later = d\n  + 1 day\n".to_string(), "7.0.0".to_string()),
            ("huge".to_string(), "dates huge v123456789012345678901\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate later = d\n  + 1 day\n".to_string(), String::new()),
        ])
        .collect();
    for (i, (alias, text, version)) in cases.iter().enumerate() {
        let p = t.path().join(format!("f{i}.cal"));
        std::fs::write(&p, text).unwrap();
        let mut o = check(&p.to_string_lossy()).unwrap();
        if o.has_errors() {
            // Another target will not take the alias (E009), so no world is ever made of it.
            assert!(o.diags.iter().any(|d| d.code == "E009"), "{alias}: {:?}", o.diags.iter().map(|d| d.code).collect::<Vec<_>>());
            refused.push(alias.clone());
            continue;
        }
        let u = codegen::unit_of(&o.checked.take().unwrap(), Lang::En);
        let wit = t.path().join(format!("f{i}.wit"));
        std::fs::write(&wit, wasm::wit(&u)).unwrap();
        let r = run(Command::new("wasm-tools").args(["component", "wit"]).arg(&wit), LIMIT);
        assert!(r.ok, "{alias}: wasm-tools does not read the world:\n{}{}", r.stderr, std::fs::read_to_string(&wit).unwrap());
        let world = wasm::world(alias);
        assert!(r.stdout.contains(&format!("world {world} {{")), "{alias}: {}", r.stdout);
        let package = if version.is_empty() { format!("package koyomi:{world};") } else { format!("package koyomi:{world}@{version};") };
        assert!(r.stdout.contains(&package), "{alias}: {}", r.stdout);
        read.push(world);
    }
    println!("worlds wasm-tools reads: {}", read.join(" "));
    println!("aliases another target will not take (E009): {}", refused.join(" "));
    assert!(read.iter().any(|w| w.starts_with('%')), "no alias that is a word of WIT passed check: {refused:?}");
    assert!(read.len() >= 20, "{} worlds", read.len());
}
