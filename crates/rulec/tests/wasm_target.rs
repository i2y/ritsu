//! The Wasm target (§15.64): the files `gen` writes, the module they build into, the
//! component the `.wit` makes of it, and the agreement of each with the reference evaluator.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root()).args(args).output().expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn have(cmd: &str) -> bool {
    rulec::backend::have(cmd)
}

fn toolchain() -> bool {
    have("node") && have("rustc") && rulec::backend::rust_target("wasm32-unknown-unknown")
}

/// Generate the shipping rule into a fresh directory; the `wasm/` directory inside it.
fn generate(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rulec-wasm-target-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (c, out, e) = run(&["gen", "tests/corpus/送料.rule", "--out", dir.to_str().unwrap()]);
    assert_eq!(c, 0, "{out}{e}");
    dir
}

/// Build the module the way `rulec api` says to, in the `wasm/` directory.
fn build(dir: &std::path::Path) {
    let (_, api, _) = run(&["api", "tests/corpus/送料.rule"]);
    let j = rulec::json::parse(api.trim()).unwrap();
    let line = j.get("wasm").unwrap().get("build").unwrap().as_str().unwrap().to_string();
    let words: Vec<&str> = line.split(' ').collect();
    let o = Command::new(words[0]).current_dir(dir.join("wasm")).args(&words[1..]).output().expect("rustc を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn genはwasmの一式を書く() {
    let dir = generate("files");
    for f in ["shipping_fee.rs", "shipping_fee_wasm.rs", "shipping_fee.wit", "shipping_fee_runner.mjs", "_round.rs", "_round_test.mjs"] {
        assert!(dir.join("wasm").join(f).exists(), "wasm/{f} が無い");
    }
    let wit = std::fs::read_to_string(dir.join("wasm/shipping_fee.wit")).unwrap();
    assert!(wit.contains("package rulec:shipping-fee@4.0.0;"), "{wit}");
    assert!(wit.contains("world shipping-fee {"), "{wit}");
    assert!(wit.contains("export call: func(input: string) -> string;"), "{wit}");
    // The inputs are named for whoever reads the interface.
    assert!(wit.contains("届け先: 都道府県"), "{wit}");
    // The module in wasm/ is the one rust/ gets: the same file, byte for byte.
    assert_eq!(
        std::fs::read_to_string(dir.join("wasm/shipping_fee.rs")).unwrap(),
        std::fs::read_to_string(dir.join("rust/shipping_fee.rs")).unwrap()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apiのwasmの項は生成物を指している() {
    let dir = generate("api");
    let (c, api, e) = run(&["api", "tests/corpus/送料.rule"]);
    assert_eq!(c, 0, "{e}");
    let j = rulec::json::parse(api.trim()).unwrap();
    let w = j.get("wasm").expect("wasm の項が無い");
    let s = |k: &str| w.get(k).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{k} が無い")).to_string();
    for k in ["source", "wit", "runner"] {
        assert!(dir.join("wasm").join(s(k)).exists(), "{k} = {} が無い", s(k));
    }
    assert_eq!(s("world"), "shipping-fee");
    assert_eq!(s("package"), "rulec:shipping-fee@4.0.0");
    assert_eq!(s("call"), "call");
    assert_eq!(s("post_return"), "cabi_post_call");
    assert_eq!(s("realloc"), "cabi_realloc");
    assert!(s("build").contains("--target wasm32-unknown-unknown") && s("build").ends_with(&format!("-o {}", s("module"))), "{}", s("build"));
    assert!(s("component").contains("wasm-tools component new"), "{}", s("component"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn モジュールは参照評価器と一致しtestがそう言う() {
    if !toolchain() {
        eprintln!("注意: node か rustc か wasm32-unknown-unknown が無いので飛ばした");
        return;
    }
    let dir = generate("test");
    let (c, out, e) = run(&["test", dir.to_str().unwrap(), "--lang", "en"]);
    assert_eq!(c, 0, "{out}{e}");
    assert!(out.contains("shipping_fee (Wasm) 70 vectors"), "{out}");
    assert!(out.contains("rounding helper (Wasm) unit vectors"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 契約の外の入力はerrorの行で返る() {
    if !toolchain() {
        eprintln!("注意: node か rustc か wasm32-unknown-unknown が無いので飛ばした");
        return;
    }
    let dir = generate("error");
    build(&dir);
    // A host of its own, twenty lines, the way generated-code.md shows it.
    let js = r#"
import { readFileSync } from "node:fs";
const { instance } = await WebAssembly.instantiate(readFileSync("shipping_fee.wasm"), {});
const ex = instance.exports;
function call(text) {
  const b = new TextEncoder().encode(text);
  const ptr = ex.cabi_realloc(0, 0, 1, b.length);
  new Uint8Array(ex.memory.buffer, ptr, b.length).set(b);
  const ret = ex.call(ptr, b.length);
  const [p, n] = new Uint32Array(ex.memory.buffer, ret, 2);
  const out = new TextDecoder().decode(new Uint8Array(ex.memory.buffer, p, n));
  ex.cabi_post_call(ret);
  return out;
}
console.log(call('{"届け先":"北海道","重量":2500,"注文金額":12000,"会員":"ゴールド"}'));
console.log(call('{"届け先":"火星","重量":2500,"注文金額":12000,"会員":"ゴールド"}'));
console.log(call('{"届け先":"北海道","重量":99999999,"注文金額":12000,"会員":"ゴールド"}'));
// The first call again, as Python's json.dumps writes it unless told otherwise (§15.151).
console.log(call('{"\\u5c4a\\u3051\\u5148": "\\u5317\\u6d77\\u9053", "\\u91cd\\u91cf": 2500, "\\u6ce8\\u6587\\u91d1\\u984d": 12000, "\\u4f1a\\u54e1": "\\u30b4\\u30fc\\u30eb\\u30c9"}'));
// An input that is not there, and one that is not a number: once answered as 0.
console.log(call('{"届け先":"北海道","注文金額":12000,"会員":"ゴールド"}'));
console.log(call('{"届け先":"北海道","重量":"2.5kg","注文金額":12000,"会員":"ゴールド"}'));
"#;
    std::fs::write(dir.join("wasm/host.mjs"), js).unwrap();
    let o = Command::new("node").current_dir(dir.join("wasm")).arg("host.mjs").output().expect("node を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let out = String::from_utf8_lossy(&o.stdout);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 6, "{out}");
    assert!(lines[0].starts_with("{\"in\":{\"届け先\":\"北海道\"") && lines[0].contains("\"observed\":{\"送料\":1800}"), "{out}");
    assert!(lines[1].starts_with("{\"error\":") && lines[1].contains("火星"), "{out}");
    assert!(lines[2].starts_with("{\"error\":") && lines[2].contains("重量"), "{out}");
    assert_eq!(lines[3], lines[0], "エスケープした入力で答えが変わった\n{out}");
    assert!(lines[4].starts_with("{\"error\":") && lines[4].contains("重量") && lines[4].contains("missing"), "{out}");
    assert!(lines[5].starts_with("{\"error\":") && lines[5].contains("重量") && lines[5].contains("whole number"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn witとモジュールはcomponentになりwasmtimeが呼べる() {
    if !toolchain() || !have("wasm-tools") {
        eprintln!("注意: toolchain か wasm-tools が無いので飛ばした");
        return;
    }
    let dir = generate("component");
    build(&dir);
    let w = dir.join("wasm");
    let sh = |args: &[&str]| {
        let o = Command::new(args[0]).current_dir(&w).args(&args[1..]).output().unwrap_or_else(|e| panic!("{}: {e}", args[0]));
        assert!(o.status.success(), "{:?}: {}", args, String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    sh(&["wasm-tools", "component", "embed", "shipping_fee.wit", "shipping_fee.wasm", "-o", "embedded.wasm"]);
    sh(&["wasm-tools", "component", "new", "embedded.wasm", "-o", "component.wasm"]);
    sh(&["wasm-tools", "validate", "component.wasm"]);
    let wit = sh(&["wasm-tools", "component", "wit", "component.wasm"]);
    assert!(wit.contains("export call: func(input: string) -> string;"), "{wit}");
    if !have("wasmtime") {
        return;
    }
    // One vector through the component, held to the expected record.
    let vec = std::fs::read_to_string(dir.join("vectors/shipping_fee.jsonl")).unwrap();
    let want = std::fs::read_to_string(dir.join("vectors/shipping_fee.expected.jsonl")).unwrap();
    let first = vec.lines().next().unwrap();
    let inner = &first[first.find("\"in\":").unwrap() + 5..];
    let (mut depth, mut end) = (0i32, 0usize);
    for (i, ch) in inner.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let input = &inner[..end];
    let arg = format!("call(\"{}\")", input.replace('\\', "\\\\").replace('"', "\\\""));
    let o = Command::new("wasmtime").current_dir(&w).args(["run", "--invoke", &arg, "component.wasm"]).output().expect("wasmtime を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    // wasmtime prints the returned string as a WAVE literal: quoted, with the quotes escaped.
    let got = String::from_utf8_lossy(&o.stdout).trim().to_string();
    let unquoted = got.trim_matches('"').replace("\\\"", "\"");
    assert_eq!(unquoted, want.lines().next().unwrap(), "{got}");
    let _ = std::fs::remove_dir_all(&dir);
}
