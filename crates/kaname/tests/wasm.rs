//! The package's Wasm (DESIGN 36.10, 36.11): the crate of a package of Rust, with every module of the
//! examples and of `tests/modules/` in it, built for wasm32-unknown-unknown with the line
//! `ritsu_emit::wasm::CARGO_LINE`, is one module that imports nothing and answers `call` through the
//! package's door:
//!
//! - every vector of every function, the inputs outside what the parameters declare and the values
//!   nested 200 and 201 deep, through a host of its own, each with the reference interpreter's answer,
//!   and a vector's with the whole line `kaname vectors` writes, without its `why`; the crate built
//!   for wasm32 passes `-D warnings`, clippy's lints among them, on the 2021 and the 2024 editions
//!   (`the_wasm_answers_as_the_reference_does`);
//! - an input outside the contract (bytes that are not UTF-8, what is not one JSON object, what nests
//!   deeper than the door reads, a call of no function a host calls) is answered, never trapped, and
//!   the answers are freed (`an_input_outside_the_contract_is_answered`);
//! - the `.wit` makes the module a component, which wasmtime calls, for a package of one module in
//!   Japanese, one of two modules, and one of a module of a book, whose transfers come back as values
//!   and which alone has the world speak of them (`the_world_makes_the_package_a_component_wasmtime_calls`);
//! - every name of a package gives a world WIT reads (`every_package_name_gives_a_world_wit_reads`).
//!
//! A tool that is missing (rustc's wasm32-unknown-unknown target, node, wasm-tools, wasmtime) prints
//! `SKIP:` and the test passes.

mod common;

use common::targets::*;
use kaname::check::Checked;
use kaname::r#gen::Target;
use ritsu_base::json::Json;
use ritsu_base::text::Lang;
use ritsu_emit::package::Place;
use ritsu_testkit::run::Ran;
use ritsu_testkit::tools::{on_path, runs};
use ritsu_testkit::{Need, TempDir, ready};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(1800);

/// Whether rustc has the target of the Wasm.
fn wasm32() -> bool {
    let Ok(o) = Command::new("rustc").args(["--print", "sysroot"]).output() else { return false };
    o.status.success() && Path::new(String::from_utf8_lossy(&o.stdout).trim()).join("lib/rustlib/wasm32-unknown-unknown/lib").is_dir()
}

fn toolchain() -> bool {
    runs("rustc", &["--version"]) && wasm32() && runs("node", &["--version"])
}

const NO_TOOLCHAIN: &str = "rustc, its wasm32-unknown-unknown target (rustup target add wasm32-unknown-unknown) or node is missing; the package's Wasm is not built";

/// The target directory of one test's builds for wasm32: the crate reads no other crate there, so
/// each test builds in its own, and no test reads a module another wrote over.
fn wasm_target(test: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("kaname-wasm-{test}"))
}

fn ok_or_fail(what: &str, r: &Ran) {
    assert!(r.ok && !r.timed_out, "{what} failed:\n{}", r.both());
}

/// The crate in `dir` built with `ritsu_emit::wasm::CARGO_LINE`, as a user builds the package's Wasm:
/// the module.
fn build(dir: &Path, test: &str) -> PathBuf {
    let target = wasm_target(test);
    let line = ritsu_emit::wasm::CARGO_LINE.replacen("cargo rustc", "cargo rustc --locked --quiet", 1);
    let r = ritsu_testkit::run(Command::new("sh").current_dir(dir).env("CARGO_TARGET_DIR", &target).env_remove("RUSTFLAGS").env_remove("CARGO_ENCODED_RUSTFLAGS").arg("-c").arg(&line), LIMIT);
    ok_or_fail(&line, &r);
    target.join("wasm32-unknown-unknown/release/pkg.wasm")
}

/// The package's crate in `dir` checked for wasm32 on `edition` with no warning: clippy's lints, or
/// rustc's alone without clippy.
fn lints(dir: &Path, test: &str, edition: &str, clippy: bool) {
    crate_files(dir, edition, "harness");
    let mut c = cargo_in(dir);
    c.env("CARGO_TARGET_DIR", wasm_target(test));
    if clippy {
        c.args(["clippy", "--locked", "--quiet", "--lib", "--target", "wasm32-unknown-unknown", "--", "-D", "warnings"]);
    } else {
        c.args(["rustc", "--locked", "--quiet", "--lib", "--target", "wasm32-unknown-unknown", "--", "-D", "warnings"]);
    }
    let r = ritsu_testkit::run(&mut c, LIMIT);
    assert!(r.ok, "the package built for wasm32 does not pass -D warnings on the {edition} edition:\n{}", r.both());
}

/// A host of its own: `call` through the canonical ABI of a module that imports nothing. With a file,
/// each of its lines is called and the answer printed, a line each; a call that traps, which none
/// should, is printed as an error, and the next is called in a new instance. With `--requests`, a JSON
/// array of requests on standard input, each a string or `{"bytes": [...]}`, then the first called
/// again and again, and the memory's size before and after.
const HOST: &str = r#"import { readFileSync } from "node:fs";

const mod = new WebAssembly.Module(readFileSync(process.argv[2]));
if (WebAssembly.Module.imports(mod).length !== 0) throw new Error("the module imports " + JSON.stringify(WebAssembly.Module.imports(mod)));
let ex = new WebAssembly.Instance(mod, {}).exports;
const enc = new TextEncoder();
const dec = new TextDecoder("utf-8", { fatal: true });

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
  const requests = JSON.parse(readFileSync(0, "utf8"));
  const bytes = (r) => (typeof r === "string" ? enc.encode(r) : new Uint8Array(r.bytes));
  for (const r of requests) console.log(call(bytes(r)));
  const first = bytes(requests[0]);
  for (let i = 0; i < 100; i++) call(first);
  const before = ex.memory.buffer.byteLength;
  for (let i = 0; i < 20000; i++) call(first);
  console.log("memory " + before + " " + ex.memory.buffer.byteLength);
} else {
  const out = [];
  for (const line of readFileSync(process.argv[3], "utf8").split("\n")) {
    if (line === "") continue;
    try {
      out.push(call(enc.encode(line)));
    } catch (e) {
      out.push(JSON.stringify({ error: "the call trapped: " + e.message }));
      ex = new WebAssembly.Instance(mod, {}).exports;
    }
  }
  process.stdout.write(out.join("\n") + "\n");
}
"#;

fn host(dir: &Path) -> PathBuf {
    let p = dir.join("host.mjs");
    std::fs::write(&p, HOST).unwrap();
    p
}

#[test]
fn the_wasm_answers_as_the_reference_does() {
    if !ready(Need::Rustc, toolchain, NO_TOOLCHAIN) || !ready(Need::Node, toolchain, NO_TOOLCHAIN) {
        return;
    }
    let clippy = clippy();
    if !clippy {
        ritsu_testkit::skip("cargo clippy is not here; the package built for wasm32 is held to rustc's lints alone");
    }
    let t = TempDir::new("kaname-wasm");
    let root = keep_in_for("answers").unwrap_or_else(|| t.path().to_path_buf());
    let report = deep(move || {
        let modules = modules();
        let checked: Vec<_> = modules.iter().map(|(c, _)| c.clone()).collect();
        let host = host(&root);
        let mut out = Vec::new();
        for lang in [Lang::En, Lang::Ja] {
            let base = root.join(format!("wasm-{}", if lang == Lang::Ja { "ja" } else { "en" }));
            write_package(&base, Target::Rust, lang, &modules);
            let dir = package_dir(&base, Target::Rust);
            // the crate's Cargo.toml names the native harness, which the Wasm leaves out
            std::fs::create_dir_all(dir.join("harness")).unwrap();
            std::fs::write(dir.join("harness/main.rs"), kaname::r#gen::harness::program(&checked, Target::Rust, PY_NAME, GO_PACKAGE)).unwrap();
            // the cases the native harness runs, the door being the same
            let (cases, counts) = cases(&modules, lang, Target::Rust);
            std::fs::write(dir.join("calls.jsonl"), kaname::r#gen::harness::call_lines(&checked, &cases)).unwrap();
            for edition in ["2021", "2024"] {
                lints(&dir, "answers", edition, clippy);
            }
            let module = build(&dir, "answers");
            let size = std::fs::metadata(&module).unwrap().len();
            let r = ritsu_testkit::run(Command::new("node").arg(&host).arg(&module).arg(dir.join("calls.jsonl")), LIMIT);
            ok_or_fail("the host", &r);
            let (agree, wrong) = kaname::r#gen::harness::compare_lines(&cases, &r.stdout);
            assert!(wrong.is_empty(), "the package's Wasm ({lang:?}) answers otherwise than the reference:\n{}", wrong.join("\n"));
            out.push(format!("{} (the module, {size} bytes, imports nothing)", counts.line(lang, agree, modules.len())));
            if lang == Lang::En {
                let v = ritsu_testkit::run(Command::new("node").arg("--version"), LIMIT);
                out.insert(0, format!("{} passes the crate built for wasm32 on the 2021 and the 2024 editions in English and in Japanese, and Node {} calls the module", if clippy { "cargo clippy -- -D warnings" } else { "rustc -D warnings" }, v.stdout.trim()));
            }
        }
        out
    });
    for line in report {
        println!("{line}");
    }
}

/// The modules of `paths` (as [`modules`] names them), and the modules each reads, which go into the
/// package with it.
fn picked(all: &[(Rc<Checked>, String)], paths: &[&str]) -> Vec<(Rc<Checked>, String)> {
    let mut out: Vec<(Rc<Checked>, String)> = Vec::new();
    for p in paths {
        let (c, _) = all.iter().find(|(_, s)| s == p).unwrap_or_else(|| panic!("no module {p}"));
        for canon in c.world.modules.keys().chain(std::iter::once(&c.canon)) {
            if !out.iter().any(|(x, _)| x.canon == *canon)
                && let Some(m) = all.iter().find(|(x, _)| x.canon == *canon)
            {
                out.push(m.clone());
            }
        }
    }
    out
}

/// The package of `modules` in `lang` under `root`, built for wasm32 in the target directory of
/// `test`: its directory and the module.
fn built(root: &Path, test: &str, lang: Lang, modules: &[(Rc<Checked>, String)]) -> (PathBuf, PathBuf) {
    let checked: Vec<_> = modules.iter().map(|(c, _)| c.clone()).collect();
    write_package(root, Target::Rust, lang, modules);
    let dir = package_dir(root, Target::Rust);
    std::fs::create_dir_all(dir.join("harness")).unwrap();
    std::fs::write(dir.join("harness/main.rs"), kaname::r#gen::harness::program(&checked, Target::Rust, PY_NAME, GO_PACKAGE)).unwrap();
    crate_files(&dir, "2024", "harness");
    let module = build(&dir, test);
    (dir, module)
}

/// A request of the host: a string, or bytes that may be no UTF-8.
fn text(s: &str) -> Json {
    Json::str(s)
}

fn bytes(b: &[u8]) -> Json {
    Json::obj([("bytes", Json::arr(b.iter().map(|x| Json::int(*x as i128))))])
}

/// Each of `requests` ([`text`] or [`bytes`]) called, then the first again and again: the answers,
/// and whether the memory stayed the size it was.
fn ask(host: &Path, module: &Path, requests: &[Json]) -> (Vec<String>, bool) {
    let input = Json::arr(requests.iter().cloned()).compact();
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
fn an_input_outside_the_contract_is_answered() {
    if !ready(Need::Rustc, toolchain, NO_TOOLCHAIN) || !ready(Need::Node, toolchain, NO_TOOLCHAIN) {
        return;
    }
    let t = TempDir::new("kaname-wasm-contract");
    let root = keep_in_for("contract").unwrap_or_else(|| t.path().to_path_buf());
    let report = deep(move || {
        let all = modules();
        let host = host(&root);
        let s = text;
        let fee = r#"{"function":"fee","in":{"a":1000},"out":84}"#;
        const NOT_OBJECT: &str = r#"{"refused":"input","error":"The input is not a JSON object"}"#;
        let deep_arr = format!("{}{}", "[".repeat(2000), "]".repeat(2000));
        // two modules, so that a call names its module; the messages in English
        let en: Vec<(Json, String)> = vec![
            (s(r#"{"module":"calc","function":"fee","in":{"a":1000}}"#), fee.into()),
            // a line of kaname vectors as it is, with its out and its why, which are not read
            (s(r#"{"module":"calc","function":"fee","in":{"a":1000},"out":84,"why":"the base values"}"#), fee.into()),
            // the keys escaped and spaced, as Python's json.dumps writes them unless told otherwise
            (s(&format!("{{\"{}\": \"calc\", \"{}\": \"fee\", \"in\": {{\"{}\": 1000}}}}", escaped("module"), escaped("function"), escaped("a"))), fee.into()),
            // a key written twice is its last value; a whole number in any spelling JSON has
            (s(r#"{"module":"calc","function":"fee","in":{"a":5,"a":1000}}"#), fee.into()),
            (s(r#"{"module":"calc","function":"fee","in":{"a":1000.0}}"#), fee.into()),
            (s(r#"{"module":"calc","function":"fee","in":{"a":1e3}}"#), fee.into()),
            (s(r#"{"module":"calc","function":"fee","in":{"a":10.00E2}}"#), fee.into()),
            (s(r#"{"module":"calc","function":"fee","in":{"a":1000.5}}"#), r#"{"refused":"input","error":"The argument `a` of `fee` is not an integer"}"#.into()),
            (s(r#"{"module":"calc","function":"fee","in":{"a":99999999999999999999}}"#), r#"{"refused":"input","error":"The argument `a` of `fee` is outside int64"}"#.into()),
            // outside the range the parameter declares: the entry's failure, as the vectors write it
            (
                s(r#"{"module":"calc","function":"fee","in":{"a":-1}}"#),
                r#"{"function":"fee","in":{"a":-1},"fails":{"kind":"input","module":"calc","line":20,"message":"The argument `a` of `fee` is -1USDc, outside its range >=0USDc <=1000000USDc"}}"#.into(),
            ),
            (s(r#"{"module":"calc","function":"fee","in":{"a":"1000"}}"#), r#"{"refused":"input","error":"The argument `a` of `fee` is not an integer"}"#.into()),
            (s(r#"{"module":"calc","function":"fee","in":{}}"#), r#"{"refused":"input","error":"The argument `a` of `fee` is missing"}"#.into()),
            // a day of no calendar is read, and the entry finds it outside the range, between whose
            // ends its day number falls (that of 2026-03-02); a date not written YYYY-MM-DD is none
            (
                s(r#"{"module":"calc","function":"days","in":{"a":"2026-02-30","b":"2026-03-01"}}"#),
                r#"{"function":"days","in":{"a":"2026-02-30","b":"2026-03-01"},"fails":{"kind":"input","module":"calc","line":29,"message":"The argument `a` of `days` is 2026-02-30, outside its range >=2026-01-01 <=2026-12-31"}}"#.into(),
            ),
            (s(r#"{"module":"calc","function":"days","in":{"a":"2026-2-3","b":"2026-03-01"}}"#), r#"{"refused":"input","error":"The argument `a` of `days` is not a date written YYYY-MM-DD"}"#.into()),
            (
                s(r#"{"module":"calc","function":"total_weight","in":{"lines":[{"qty":2,"weight":3},{"qty":1,"weight":160}]}}"#),
                r#"{"function":"total_weight","in":{"lines":[{"qty":2,"weight":3},{"qty":1,"weight":160}]},"out":166}"#.into(),
            ),
            (s(r#"{"module":"calc","function":"total_weight","in":{"lines":[{"qty":2}]}}"#), r#"{"refused":"input","error":"The argument `lines[0].weight` of `total_weight` is missing"}"#.into()),
            // a string's escapes read, and written as the vectors write them
            (s(r#"{"module":"texts","function":"handle","in":{"name":"  Ada \u00e9\tLovelace "}}"#), r#"{"function":"handle","in":{"name":"  Ada é\tLovelace "},"out":"ada é lovelace"}"#.into()),
            // a lone surrogate, which no string of Rust holds: the entry's failure of a string that is not
            // well formed, with the character in its place in `in`
            (
                s(r#"{"module":"texts","function":"handle","in":{"name":"a\ud800b"}}"#),
                "{\"function\":\"handle\",\"in\":{\"name\":\"a\u{fffd}b\"},\"fails\":{\"kind\":\"input\",\"module\":\"texts\",\"line\":14,\"message\":\"The argument `name` of `handle` is no well-formed string of Unicode\"}}".into(),
            ),
            (s(r#"{"module":"texts","function":"handle","in":{"name":"a\ud83d\ude00b"}}"#), "{\"function\":\"handle\",\"in\":{\"name\":\"a\u{1f600}b\"},\"out\":\"a\u{1f600}b\"}".into()),
            // an inner function: no host calls it
            (s(r#"{"module":"generic","function":"first_or","in":{"xs":[],"fallback":1}}"#), r#"{"refused":"input","error":"The module `generic` has no function `first_or` a host calls"}"#.into()),
            // what is no call of a function a host calls
            (s(r#"{"function":"fee","in":{"a":1000}}"#), r#"{"refused":"input","error":"The input has no `module`"}"#.into()),
            (s(r#"{"module":"nope","function":"fee","in":{"a":1000}}"#), r#"{"refused":"input","error":"There is no module `nope`"}"#.into()),
            (s(r#"{"module":"calc","function":"nope","in":{}}"#), r#"{"refused":"input","error":"The module `calc` has no function `nope` a host calls"}"#.into()),
            (s(r#"{"module":"calc","function":1,"in":{}}"#), r#"{"refused":"input","error":"The `function` of the input is not a string"}"#.into()),
            (s(r#"{"module":"calc","function":"fee"}"#), r#"{"refused":"input","error":"The input has no `in`"}"#.into()),
            (s(r#"{"module":"calc","function":"fee","in":[1000]}"#), r#"{"refused":"input","error":"The `in` of the input is not a JSON object"}"#.into()),
            // not one JSON object: answered, never trapped
            (s(""), NOT_OBJECT.into()),
            (s("[]"), NOT_OBJECT.into()),
            (s(r#""calc""#), NOT_OBJECT.into()),
            (s(r#"{"module":"calc","function":"fee","in":{"a":1000},}"#), NOT_OBJECT.into()),
            (s(r#"{"module":"calc","function":"fee","in":{"a":1000}} {}"#), NOT_OBJECT.into()),
            // a control character inside a string, which JSON writes only escaped
            (s(&format!("{{\"module\":\"calc{}\",\"function\":\"fee\",\"in\":{{}}}}", '\u{1}')), NOT_OBJECT.into()),
            // deeper than the door reads: refused before it is read to its end, where it nests no deeper,
            // read
            (s(&deep_arr), r#"{"refused":"input","error":"The input nests deeper than 1024"}"#.into()),
            (s(&format!("{{\"module\":\"calc\",\"function\":\"fee\",\"in\":{{\"a\":{}1{}}}}}", "[".repeat(600), "]".repeat(600))), r#"{"refused":"input","error":"The argument `a` of `fee` is not an integer"}"#.into()),
            (bytes(&[123, 34, 255, 34, 125]), r#"{"refused":"input","error":"The input is not UTF-8"}"#.into()),
        ];
        // one module, which a call need not name; the messages in Japanese
        let claim = r#"{"function":"遅れの請求","in":{"請求書":{"番号":"inv_1","請求日":"2026-03-02","金額":100000},"支払日":"2026-05-20","基準金利":50},"out":{"期日":"2026-03-31","遅れた日数":50,"定額の補償":70,"利息":1150}}"#;
        let ja: Vec<(Json, String)> = vec![
            (s(r#"{"function":"遅れの請求","in":{"請求書":{"番号":"inv_1","請求日":"2026-03-02","金額":100000},"支払日":"2026-05-20","基準金利":50}}"#), claim.into()),
            (s(r#"{"module":"支払いの遅れ","function":"遅れの請求","in":{"請求書":{"番号":"inv_1","請求日":"2026-03-02","金額":100000},"支払日":"2026-05-20","基準金利":50}}"#), claim.into()),
            (
                s(&format!(
                    "{{\"{}\": \"{}\", \"in\": {{\"{}\": {{\"{}\": \"inv_1\", \"{}\": \"2026-03-02\", \"{}\": 100000}}, \"{}\": \"2026-05-20\", \"{}\": 50}}}}",
                    escaped("function"),
                    escaped("遅れの請求"),
                    escaped("請求書"),
                    escaped("番号"),
                    escaped("請求日"),
                    escaped("金額"),
                    escaped("支払日"),
                    escaped("基準金利")
                )),
                claim.into(),
            ),
            (s(r#"{"function":"遅れの請求","in":{"請求書":{"番号":"inv_1","請求日":"2026-03-02","金額":100000},"基準金利":50}}"#), r#"{"refused":"input","error":"関数 `遅れの請求` の引数 `支払日` がありません"}"#.into()),
            (
                s(r#"{"function":"遅れの請求","in":{"請求書":{"番号":"inv_1","請求日":"2026-03-02","金額":100000},"支払日":"2026-02-30","基準金利":50}}"#),
                r#"{"function":"遅れの請求","in":{"請求書":{"番号":"inv_1","請求日":"2026-03-02","金額":100000},"支払日":"2026-02-30","基準金利":50},"fails":{"kind":"input","module":"支払いの遅れ","line":18,"message":"関数 `遅れの請求` の引数 `支払日` の 2026-02-30 は、範囲 >=2026-01-01 <=2029-12-31 の外です"}}"#.into(),
            ),
            (
                s(r#"{"function":"遅れの請求","in":{"請求書":{"番号":"inv_1","請求日":"2026-03-02","金額":100000},"支払日":"2026-5-20","基準金利":50}}"#),
                r#"{"refused":"input","error":"関数 `遅れの請求` の引数 `支払日` が YYYY-MM-DD と書いた日付ではありません"}"#.into(),
            ),
            (
                s(r#"{"function":"遅れの請求","in":{"請求書":{"番号":"inv_1","請求日":"2026-03-02","金額":100000},"支払日":"2030-01-01","基準金利":50}}"#),
                r#"{"function":"遅れの請求","in":{"請求書":{"番号":"inv_1","請求日":"2026-03-02","金額":100000},"支払日":"2030-01-01","基準金利":50},"fails":{"kind":"input","module":"支払いの遅れ","line":18,"message":"関数 `遅れの請求` の引数 `支払日` の 2030-01-01 は、範囲 >=2026-01-01 <=2029-12-31 の外です"}}"#.into(),
            ),
            // a module is named as it writes its name, not by its alias
            (s(r#"{"module":"late_payment_ja","function":"遅れの請求","in":{}}"#), r#"{"refused":"input","error":"モジュール `late_payment_ja` はありません"}"#.into()),
            (s(r#"{"function":"claim","in":{}}"#), r#"{"refused":"input","error":"モジュール `支払いの遅れ` に、ホストが呼ぶ関数 `claim` はありません"}"#.into()),
            (s("[]"), r#"{"refused":"input","error":"入力が JSON のオブジェクトではありません"}"#.into()),
            (bytes(&[0xff]), r#"{"refused":"input","error":"入力が UTF-8 ではありません"}"#.into()),
        ];
        let mut out = Vec::new();
        for (name, lang, paths, asked) in [
            ("en", Lang::En, vec!["tests/modules/calc.kn", "tests/modules/texts.kn", "tests/modules/generic.kn"], &en),
            ("ja", Lang::Ja, vec!["examples/late_payment/late_payment.ja.kn"], &ja),
        ] {
            let (_, module) = built(&root.join(name), "contract", lang, &picked(&all, &paths));
            let requests: Vec<Json> = asked.iter().map(|(r, _)| r.clone()).collect();
            let (got, steady) = ask(&host, &module, &requests);
            assert_eq!(got.len(), asked.len(), "{name}: {got:?}");
            for ((r, want), g) in asked.iter().zip(&got) {
                assert_eq!(g, want, "{name}: asked {}", r.compact());
            }
            assert!(steady, "{name}: the memory grows with every call: the answers are not freed");
            out.push(format!("{name}: {} inputs answered as the table says, and the memory the same after 20,000 calls", asked.len()));
        }
        out
    });
    for line in report {
        println!("{line}");
    }
}

/// What wasmtime prints for one call of `component` in `w`, unquoted: wasmtime prints the string
/// `call` returns as a WAVE literal, quoted, with its quotes and backslashes escaped.
fn invoke(w: &Path, component: &str, input: &str) -> String {
    let arg = format!("call(\"{}\")", input.replace('\\', "\\\\").replace('"', "\\\""));
    let r = ritsu_testkit::run(Command::new("wasmtime").current_dir(w).args(["run", "--invoke", &arg, component]), LIMIT);
    ok_or_fail(&format!("wasmtime run --invoke {arg}"), &r);
    let got = r.stdout.trim();
    got.strip_prefix('"').and_then(|g| g.strip_suffix('"')).unwrap_or(got).replace("\\\"", "\"").replace("\\\\", "\\")
}

/// `modules`, each in the place the package of the tests puts it.
fn placed(modules: &[(Rc<Checked>, String)]) -> Vec<(Rc<Checked>, String, Place)> {
    modules.iter().map(|(c, s)| (c.clone(), s.clone(), Place::here())).collect()
}

#[test]
fn the_world_makes_the_package_a_component_wasmtime_calls() {
    if !ready(Need::Rustc, || toolchain() && on_path("wasm-tools").is_some(), "rustc's wasm32-unknown-unknown target, node or wasm-tools is missing; the component of the package's Wasm is not made") {
        return;
    }
    let called = on_path("wasmtime").is_some();
    if !called {
        ritsu_testkit::skip("wasmtime is missing; the component of the package's Wasm is made but not called");
    }
    let t = TempDir::new("kaname-wasm-component");
    let root = keep_in_for("component").unwrap_or_else(|| t.path().to_path_buf());
    let report = deep(move || {
        let all = modules();
        let mut out = Vec::new();
        for (name, lang, paths, ask, want) in [
            (
                "late_payment",
                Lang::Ja,
                vec!["examples/late_payment/late_payment.ja.kn"],
                r#"{"function":"遅れの請求","in":{"請求書":{"番号":"inv_1","請求日":"2026-03-02","金額":100000},"支払日":"2026-05-20","基準金利":50}}"#,
                r#"{"function":"遅れの請求","in":{"請求書":{"番号":"inv_1","請求日":"2026-03-02","金額":100000},"支払日":"2026-05-20","基準金利":50},"out":{"期日":"2026-03-31","遅れた日数":50,"定額の補償":70,"利息":1150}}"#,
            ),
            (
                "fees",
                Lang::En,
                vec!["tests/modules/calc.kn", "tests/modules/texts.kn"],
                r#"{"module":"texts","function":"handle","in":{"name":"  Ada Lovelace "}}"#,
                r#"{"function":"handle","in":{"name":"  Ada Lovelace "},"out":"ada lovelace"}"#,
            ),
            // a module of a book: the transfers come back as values
            (
                "payout",
                Lang::En,
                vec!["examples/payout/payout.kn"],
                r#"{"function":"settle","in":{"payment":{"id":"a","seller":"a","amount":50000000,"international":true}}}"#,
                r#"{"function":"settle","in":{"payment":{"id":"a","seller":"a","amount":50000000,"international":true}},"out":{"capture":{"book":"ledger","transfer":"capture","op":"do","args":{"payment":"a","seller":"a","amount":50000000}},"fee":{"book":"ledger","transfer":"processing_fee","op":"do","args":{"payment":"a","seller":"a","fee":2495049}}}}"#,
            ),
        ] {
            let modules = picked(&all, &paths);
            let (dir, module) = built(&root.join(name), "component", lang, &modules);
            let w = dir.join("component");
            std::fs::create_dir_all(&w).unwrap();
            let wit = kaname::r#gen::wasm::wit(&placed(&modules), name, Some("1.0.0"), lang);
            // the doc of the world speaks of the transfers where a module of the package builds them
            assert_eq!(wit.contains(r#"{"book","transfer","op","args"}"#), name == "payout", "{name}:\n{wit}");
            std::fs::write(w.join(kaname::r#gen::wasm::wit_file(name)), wit).unwrap();
            std::fs::copy(&module, w.join(format!("{name}.wasm"))).unwrap();
            let line = ritsu_emit::wasm::component_line(name);
            ok_or_fail(&line, &ritsu_testkit::run(Command::new("sh").current_dir(&w).arg("-c").arg(&line), LIMIT));
            let component = format!("{name}.component.wasm");
            ok_or_fail("wasm-tools validate", &ritsu_testkit::run(Command::new("wasm-tools").current_dir(&w).args(["validate", &component]), LIMIT));
            let r = ritsu_testkit::run(Command::new("wasm-tools").current_dir(&w).args(["component", "wit", &component]), LIMIT);
            ok_or_fail("wasm-tools component wit", &r);
            assert!(r.stdout.contains(&format!("export {};", ritsu_emit::wasm::CALL_SIGNATURE)), "{name}: the component's world:\n{}", r.stdout);
            if called {
                assert_eq!(invoke(&w, &component, ask), want, "{name}");
                out.push(format!("wasmtime called the component of the package {name}"));
            } else {
                out.push(format!("the component of the package {name} is made"));
            }
        }
        out
    });
    for line in report {
        println!("{line}");
    }
}

/// The names of a package that are words of WIT, and the shapes of a name WIT spells otherwise: a
/// word of digits after the first, a run of `_`, a `_` at the end.
fn names() -> Vec<String> {
    let mut v: Vec<String> = ritsu_emit::wasm::WIT_KEYWORDS.iter().map(|k| k.replace('-', "_")).collect();
    v.extend(["fee_2nd", "net_30", "a__b", "trailing_", "x_1_2", "late_payment", "generated"].map(String::from));
    v
}

/// Whether `ritsu gen` takes `n` as the name of a package (`--name`): a lowercase letter first, then
/// lowercase letters, digits and `_`, and no word Python keeps.
fn taken(n: &str) -> bool {
    n.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') && !ritsu_emit::words::python::KEYWORDS.contains(&n)
}

#[test]
fn every_package_name_gives_a_world_wit_reads() {
    if !ready(Need::Rustc, || on_path("wasm-tools").is_some(), "wasm-tools is missing; the worlds of the package's Wasm are not read") {
        return;
    }
    let t = TempDir::new("kaname-wasm-worlds");
    let root = t.path().to_path_buf();
    let report = deep(move || {
        let all = modules();
        let modules = placed(&picked(&all, &["tests/modules/calc.kn", "tests/modules/texts.kn"]));
        let mut read = Vec::new();
        let mut refused = Vec::new();
        for (i, name) in names().iter().enumerate() {
            if !taken(name) {
                refused.push(name.clone());
                continue;
            }
            let world = kaname::r#gen::wasm::world(name);
            for (j, version) in [None, Some("1.0.0")].into_iter().enumerate() {
                let wit = root.join(format!("w{i}-{j}.wit"));
                std::fs::write(&wit, kaname::r#gen::wasm::wit(&modules, name, version, if j == 0 { Lang::En } else { Lang::Ja })).unwrap();
                let r = ritsu_testkit::run(Command::new("wasm-tools").args(["component", "wit"]).arg(&wit), LIMIT);
                assert!(r.ok, "{name}: wasm-tools does not read the world:\n{}{}", r.stderr, std::fs::read_to_string(&wit).unwrap());
                assert!(r.stdout.contains(&format!("world {world} {{")), "{name}: {}", r.stdout);
                let package = format!("package {};", kaname::r#gen::wasm::package(name, version));
                assert!(r.stdout.contains(&package), "{name}: no `{package}` in\n{}", r.stdout);
                assert!(r.stdout.contains(&format!("export {};", ritsu_emit::wasm::CALL_SIGNATURE)), "{name}: {}", r.stdout);
            }
            read.push(world);
        }
        assert!(read.iter().any(|w| w.starts_with('%')), "no name that is a word of WIT is taken: {refused:?}");
        assert!(read.len() >= 40, "{} worlds", read.len());
        vec![
            format!("worlds wasm-tools reads, with and without a version, the doc in English and in Japanese: {}", read.join(" ")),
            format!("names ritsu gen does not take (words Python keeps): {}", refused.join(" ")),
        ]
    });
    for line in report {
        println!("{line}");
    }
}
