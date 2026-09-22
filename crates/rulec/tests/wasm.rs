//! The module the site loads answers what the binary answers (§15.48).
//!
//! `website/docs/playground/rulec.wasm` is committed, like the diagrams, so that building
//! the site needs no Rust toolchain — which means it can go stale. So it is held the way
//! every other target is held in this repository: driven over the same inputs and compared
//! **byte for byte** against the tool itself. `check` in both languages, everything `gen`
//! writes, and the version string, which is what makes a stale file a failing test rather
//! than a page quietly answering an old way.
//!
//! Needs `node` (for `WebAssembly`); skipped where there is none. Re-build the file with
//! `website/tools/make_wasm.sh`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn wasm() -> PathBuf {
    root().join("website/docs/playground/rulec.wasm")
}

/// The whole of the page's side of the boundary: allocate, write, call, read the length
/// out of the header, print the bytes. It lives here rather than in a committed file
/// because it is a test fixture — `playground.js` is the copy that ships, and if the two
/// ever disagree about the convention, this one fails.
const DRIVER: &str = r#"
import { readFileSync } from "node:fs";
const [wasmPath, rulePath, lang, what] = process.argv.slice(2);
const inst = await WebAssembly.instantiate(await WebAssembly.compile(readFileSync(wasmPath)), {});
const e = inst.exports;
let ptr = 0;
if (what !== "version") {
  const src = readFileSync(rulePath);
  ptr = e.rulec_alloc(src.length);
  new Uint8Array(e.memory.buffer, ptr + 4, src.length).set(src);
}
const ja = lang === "ja" ? 1 : 0;
const rp =
  what === "version" ? e.rulec_version()
  : what === "check" ? e.rulec_check(ptr, ja)
  : what === "gen"   ? e.rulec_gen(ptr, ja)
  :                    e.rulec_doc(ptr, ja);
const len = new DataView(e.memory.buffer).getUint32(rp, true);
process.stdout.write(Buffer.from(new Uint8Array(e.memory.buffer, rp + 4, len)));
"#;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-wasm-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A directory holding the driver and one `playground.rule` — the name the module gives
/// what the reader typed, so that the paths in the findings line up with the binary's.
fn bench(name: &str, src: &str) -> PathBuf {
    let d = tmp(name);
    std::fs::write(d.join("driver.mjs"), DRIVER).unwrap();
    std::fs::write(d.join("playground.rule"), src).unwrap();
    d
}

fn drive(d: &Path, lang: &str, what: &str) -> String {
    let o = Command::new("node")
        .current_dir(d)
        .args(["driver.mjs", wasm().to_str().unwrap(), "playground.rule", lang, what])
        .output()
        .expect("node を起動できない");
    assert!(
        o.status.success(),
        "wasm の driver が失敗した: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8(o.stdout).expect("UTF-8 で返っていない")
}

fn rulec(d: &Path, lang: &str, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(d)
        .env("RULEC_LANG", lang)
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

fn json(s: &str) -> rulec::json::Json {
    rulec::json::parse(s).unwrap_or_else(|e| panic!("JSON として読めない: {e}\n{s}"))
}

fn text(j: &rulec::json::Json, k: &str) -> String {
    j.get(k).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{k} が無い")).to_string()
}

fn arr<'a>(j: &'a rulec::json::Json, k: &str) -> &'a Vec<rulec::json::Json> {
    match j.get(k) {
        Some(rulec::json::Json::Arr(a)) => a,
        other => panic!("{k} が配列でない: {other:?}"),
    }
}

fn yes(j: &rulec::json::Json, k: &str) -> bool {
    matches!(j.get(k), Some(rulec::json::Json::Bool(true)))
}

/// The rules the page opens on, and the one the front page's picture is about: the same
/// table with its last row taken off.
fn cases() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (name, rel) in
        [("en", "website/tools/overview.rule"), ("ja", "website/tools/overview-ja.rule")]
    {
        let full = std::fs::read_to_string(root().join(rel)).unwrap();
        let gap = full.trim_end().lines().collect::<Vec<_>>();
        let gap = gap[..gap.len() - 1].join("\n") + "\n";
        out.push((format!("{name}-full"), full));
        out.push((format!("{name}-gap"), gap));
    }
    // A rule with rows that shadow each other and examples that run: the note lines and the
    // `ok` line after the findings are part of what `check` prints.
    out.push((
        "corpus".into(),
        std::fs::read_to_string(root().join("tests/corpus/送料.rule")).unwrap(),
    ));
    // Two rules whose answer the elimination decides (§15.126, §15.127), because the three
    // above do not reach it — and a sample that does not reach a check cannot tell a stale
    // module from a fresh one. The committed file went on printing W114 for the first of
    // these for a whole session after the binary had stopped (§15.131). Neither needs a
    // file beside it, which is what makes them runnable where there is no filesystem.
    out.push((
        "elimination".into(),
        std::fs::read_to_string(root().join("tests/corpus/クーポン併用.rule")).unwrap(),
    ));
    out.push((
        "undecided".into(),
        std::fs::read_to_string(root().join("tests/mutants/m_w114.rule")).unwrap(),
    ));
    out
}

#[test]
fn 版がバイナリと同じ() {
    if !have("node") || !wasm().exists() {
        eprintln!("skip: node が無いか rulec.wasm が無い");
        return;
    }
    let d = bench("version", "");
    let got = drive(&d, "en", "version");
    assert_eq!(
        got,
        env!("CARGO_PKG_VERSION"),
        "website/docs/playground/rulec.wasm が古い。website/tools/make_wasm.sh を実行する"
    );
}

#[test]
fn checkの文面がコマンドと一字一句同じ() {
    if !have("node") || !wasm().exists() {
        eprintln!("skip: node が無いか rulec.wasm が無い");
        return;
    }
    for (name, src) in cases() {
        let d = bench(&format!("check-{name}"), &src);
        for lang in ["en", "ja"] {
            let (_, want) = rulec(&d, lang, &["check", "playground.rule"]);
            let got = json(&drive(&d, lang, "check"));
            assert_eq!(text(&got, "text"), want, "{name} ({lang}) で文面が違う");
        }
    }
}

#[test]
fn genが書くファイルがコマンドと一字一句同じ() {
    if !have("node") || !wasm().exists() {
        eprintln!("skip: node が無いか rulec.wasm が無い");
        return;
    }
    for (name, src) in cases() {
        let d = bench(&format!("gen-{name}"), &src);
        let got = json(&drive(&d, "en", "gen"));
        let (code, _) = rulec(&d, "en", &["gen", "playground.rule", "--out", "out"]);
        if !yes(&got, "ok") {
            // The page refuses exactly where the command refuses (§1.6).
            assert_ne!(code, 0, "{name}: wasm は断ったのにコマンドは生成した");
            continue;
        }
        assert_eq!(code, 0, "{name}: コマンドは断ったのに wasm は生成した");
        let files = arr(&got, "files");
        let mut seen = 0usize;
        for f in files {
            let rel = text(f, "path");
            let want = std::fs::read_to_string(d.join("out").join(&rel))
                .unwrap_or_else(|_| panic!("{name}: コマンドは {rel} を書いていない"));
            assert_eq!(text(f, "body"), want, "{name}: {rel} の中身が違う");
            seen += 1;
        }
        // Every file the command wrote is also in the answer; a backend the module quietly
        // left out would otherwise pass.
        assert_eq!(seen, walkdir(&d.join("out")).len(), "{name}: ファイルの数が違う");
    }
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walkdir(&p));
        } else {
            out.push(p);
        }
    }
    out
}

#[test]
fn 承認者向けのページがコマンドと一字一句同じ() {
    if !have("node") || !wasm().exists() {
        eprintln!("skip: node が無いか rulec.wasm が無い");
        return;
    }
    let (name, src) = cases().into_iter().next().unwrap();
    let d = bench(&format!("doc-{name}"), &src);
    for lang in ["en", "ja"] {
        let got = json(&drive(&d, lang, "doc"));
        rulec(&d, lang, &["doc", "playground.rule", "--format", "html", "--out", "out"]);
        let files = walkdir(&d.join("out"));
        let want = std::fs::read_to_string(files.first().expect("doc が書かれていない")).unwrap();
        assert_eq!(text(&got, "html"), want, "{lang} で承認者向けのページが違う");
        let _ = std::fs::remove_dir_all(d.join("out"));
    }
}
