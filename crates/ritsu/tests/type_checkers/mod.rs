//! The type checkers the tests of `ritsu gen` hold a package's code to (tests/gen.rs and
//! tests/modules_gen.rs): `tsc --strict`, `mypy --strict`, `go vet` and gofmt, each with the packages
//! the generated code imports, from the tools the languages' tests install.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// The root of the workspace, where the languages keep the tools their tests use.
pub fn workspace() -> PathBuf {
    here().parent().and_then(Path::parent).expect("crates/ritsu is two below the root").to_path_buf()
}

/// tsc (TypeScript 7), as dandori's tests run it, and the packages it reads: Temporal's SDK from
/// dandori's runner, tigerbeetle-node from chobo's.
pub fn tsc() -> Option<(PathBuf, Vec<PathBuf>)> {
    let tsc = workspace().join("crates/dandori/tools/temporal/node_modules/.bin/tsc");
    let temporal = workspace().join("crates/dandori/tools/temporal/node_modules");
    let tigerbeetle = workspace().join("crates/chobo/tools/runner/node_modules/tigerbeetle-node");
    (tsc.exists() && tigerbeetle.exists()).then(|| {
        let mut modules: Vec<PathBuf> = std::fs::read_dir(&temporal).unwrap().flatten().map(|e| e.path()).filter(|p| !p.ends_with(".bin")).collect();
        modules.sort();
        modules.push(tigerbeetle);
        (tsc, modules)
    })
}

/// `tsc --strict` on the TypeScript package at `dir`. The packages the default transport loads only
/// when a task needs them (the AWS SDK's clients, the agents' SDKs) are declared as modules of any
/// type, as dandori's own test of its TypeScript does.
pub fn type_check_typescript(dir: &Path, tsc: &Path, modules: &[PathBuf]) {
    let out = tsc_in(dir, tsc, modules);
    assert!(out.status.success(), "{}: tsc --strict finds fault with the package:\n{}{}", dir.display(), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// `tsc --strict` on the TypeScript under `dir`, as [`type_check_typescript`] runs it, in `dir`: what
/// it answers, the files it names by their paths from `dir`.
pub fn tsc_in(dir: &Path, tsc: &Path, modules: &[PathBuf]) -> std::process::Output {
    let nm = dir.join("node_modules");
    std::fs::create_dir_all(&nm).unwrap();
    for m in modules {
        std::os::unix::fs::symlink(m, nm.join(m.file_name().unwrap())).unwrap();
    }
    std::fs::write(dir.join("shims.d.ts"), "declare module \"@anthropic-ai/sdk\";\ndeclare module \"openai\";\ndeclare module \"@openai/agents\";\ndeclare module \"@aws-sdk/*\";\n").unwrap();
    let types = workspace().join("crates/dandori/tools/temporal/node_modules/@types");
    let tsconfig = serde_json::json!({
        "compilerOptions": {
            "strict": true, "noEmit": true, "module": "nodenext", "moduleResolution": "nodenext", "target": "es2022",
            "skipLibCheck": true, "types": ["node"], "typeRoots": [types]
        },
        "include": ["**/*.ts"],
        "exclude": ["node_modules"]
    });
    std::fs::write(dir.join("tsconfig.json"), serde_json::to_string_pretty(&tsconfig).unwrap()).unwrap();
    Command::new(tsc).arg("-p").arg("tsconfig.json").current_dir(dir).output().unwrap()
}

/// mypy (koyomi's, or one on the PATH), the Python whose packages it reads (dandori's runner's, with
/// Temporal's SDK), and chobo's runner's tigerbeetle package.
pub fn mypy() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let mypy = Some(workspace().join("crates/koyomi/tools/.venv/bin/mypy")).filter(|p| p.exists()).or_else(|| {
        Command::new("mypy").arg("--version").output().ok().filter(|o| o.status.success()).map(|_| PathBuf::from("mypy"))
    })?;
    let python = workspace().join("crates/dandori/tools/temporal-python/.venv/bin/python");
    let lib = workspace().join("crates/chobo/tools/runner/.venv/lib");
    let tigerbeetle = std::fs::read_dir(&lib).ok()?.flatten().map(|e| e.path().join("site-packages/tigerbeetle")).find(|p| p.exists())?;
    python.exists().then_some((mypy, python, tigerbeetle))
}

/// `mypy --strict` on the Python package at `dir` (the directory of pyproject.toml). The installed
/// packages `extra` names (tigerbeetle from chobo's runner, cedarpy from sekisho's) are read beside
/// it, and followed without their own errors said, as an installed package is.
pub fn type_check_python(dir: &Path, mypy: &Path, python: &Path, extra_packages: &[PathBuf]) {
    let out = mypy_in(dir, mypy, python, extra_packages, &["-p", "generated"], &[]);
    assert!(out.status.success(), "{}: mypy --strict finds fault with the package:\n{}{}", dir.display(), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// `mypy --strict` in `dir` on `what`, as [`type_check_python`] runs it, with `path` on its path
/// beside the installed packages: what it answers.
pub fn mypy_in(dir: &Path, mypy: &Path, python: &Path, extra_packages: &[PathBuf], what: &[&str], path: &[PathBuf]) -> std::process::Output {
    let extra = dir.join("extra");
    std::fs::create_dir_all(&extra).unwrap();
    let mut ini = String::from("[mypy]\n");
    for p in extra_packages {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        std::os::unix::fs::symlink(p, extra.join(&name)).unwrap();
        ini.push_str(&format!("\n[mypy-{name}.*]\nfollow_imports = silent\n"));
    }
    std::fs::write(dir.join("mypy.ini"), ini).unwrap();
    let mypypath = std::env::join_paths(path.iter().chain([&extra])).unwrap();
    Command::new(mypy)
        .args(["--strict", "--config-file", "mypy.ini", "--no-incremental", "--cache-dir"])
        .arg(dir.join(".mypy_cache"))
        .arg("--python-executable")
        .arg(python)
        .args(what)
        .env("MYPYPATH", mypypath)
        .current_dir(dir)
        .output()
        .unwrap()
}

/// cedarpy, from sekisho's runner of Python (tools/runner-py), which the Python of a gate imports.
pub fn cedarpy() -> Option<PathBuf> {
    let lib = workspace().join("crates/sekisho/tools/runner-py/.venv/lib");
    std::fs::read_dir(&lib).ok()?.flatten().map(|e| e.path().join("site-packages/cedarpy")).find(|p| p.join("py.typed").exists())
}

/// The go.mod and go.sum of a module that requires what the Go of a package imports: those of
/// dandori's runner (Temporal's SDK) and chobo's (pgx, tigerbeetle-go), put together.
pub fn go_module() -> Option<(String, String)> {
    let dandori = workspace().join("crates/dandori/tools/temporal-go");
    let chobo = workspace().join("crates/chobo/tools/runner/go");
    if !dandori.join("go.mod").exists() || !chobo.join("go.mod").exists() {
        return None;
    }
    let mut go = String::new();
    let mut requires: Vec<(String, String)> = Vec::new();
    for dir in [&dandori, &chobo] {
        let text = read(&dir.join("go.mod"));
        let mut inside = false;
        for line in text.lines() {
            let l = line.trim();
            if let Some(v) = l.strip_prefix("go ").filter(|_| go.is_empty()) {
                go = v.to_string();
            } else if l == "require (" {
                inside = true;
            } else if l == ")" {
                inside = false;
            } else if inside {
                let mut parts = l.split_whitespace();
                if let (Some(p), Some(v)) = (parts.next(), parts.next())
                    && !requires.iter().any(|(q, _)| q == p)
                {
                    requires.push((p.to_string(), v.to_string()));
                }
            }
        }
    }
    requires.sort();
    let lines: String = requires.iter().map(|(p, v)| format!("\t{p} {v}\n")).collect();
    let mut sums: Vec<String> = [&dandori, &chobo].iter().flat_map(|d| read(&d.join("go.sum")).lines().map(str::to_string).collect::<Vec<_>>()).collect();
    sums.sort();
    sums.dedup();
    Some((format!("module gocheck\n\ngo {go}\n\nrequire (\n{lines})\n"), sums.join("\n") + "\n"))
}

/// The module `gocheck` in `dir`, with the Go package at `package` as gocheck/generated.
pub fn go_module_in(dir: &Path, package: &Path, gomod: &(String, String)) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("go.mod"), &gomod.0).unwrap();
    std::fs::write(dir.join("go.sum"), &gomod.1).unwrap();
    ritsu_testkit::tmp::copy_dir(package, &dir.join("generated"));
}

pub fn go_in(dir: &Path) -> Command {
    let mut go = Command::new("go");
    // the modules come from the module cache, as the tests of dandori and chobo leave it
    go.current_dir(dir).env("GOWORK", "off").env("GOFLAGS", "-mod=mod -trimpath").env("GOPROXY", "off");
    go
}

/// `go vet` and gofmt on the Go package at `package`, in a module made in `dir`.
pub fn type_check_go(dir: &Path, package: &Path, gomod: &(String, String)) {
    go_module_in(dir, package, gomod);
    let out = go_in(dir).args(["vet", "./..."]).output().unwrap();
    assert!(out.status.success(), "{}: go vet finds fault with the package:\n{}{}", package.display(), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let fmt = Command::new("gofmt").arg("-l").arg("generated").current_dir(dir).output().unwrap();
    assert!(fmt.status.success() && fmt.stdout.is_empty(), "{}: gofmt would write these otherwise:\n{}{}", package.display(), String::from_utf8_lossy(&fmt.stdout), String::from_utf8_lossy(&fmt.stderr));
}

pub fn have_go() -> bool {
    Command::new("go").arg("version").output().map(|o| o.status.success()).unwrap_or(false)
}

// ── Rust ─────────────────────────────────────────────────────────────────────────────────────────

/// The cargo that runs the tests, run in `dir` with `target` a target directory of its own in the
/// tests' temporary directory, so that the crates the books' clients need are built once.
pub fn cargo_in(dir: &Path, target: &str) -> Command {
    let mut c = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    c.current_dir(dir).env("CARGO_TARGET_DIR", rust_target(target)).env_remove("RUSTFLAGS").env_remove("CARGO_ENCODED_RUSTFLAGS");
    c
}

/// The target directory of [`cargo_in`].
pub fn rust_target(target: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(target)
}

/// Whether cargo's clippy is here.
pub fn have_clippy() -> bool {
    ritsu_testkit::tools::runs(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()), &["clippy", "--version"])
}

/// Whether rustc has the target of the Wasm.
pub fn have_wasm32() -> bool {
    let Ok(o) = Command::new("rustc").args(["--print", "sysroot"]).output() else { return false };
    o.status.success() && Path::new(String::from_utf8_lossy(&o.stdout).trim()).join("lib/rustlib/wasm32-unknown-unknown/lib").is_dir()
}

/// The package of Rust at `dir` made a crate a test builds with `--offline`: a workspace of its own
/// (the temporary directory may be under this one's), and the lock of the crate of kaname's tests, whose
/// crates are the ones the package's manifest may name, at the versions tested. Its version is made
/// from `tag`: to cargo, crates of one name and version are one library in a target directory, whatever
/// their directories. Up to here, the manifest is as `ritsu gen` writes it.
pub fn rust_crate(dir: &Path, tag: &str) {
    static FETCHED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let tools = workspace().join("crates/kaname/tools/rust");
    FETCHED.get_or_init(|| {
        let fetch = |more: &[&str]| Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())).args(["fetch", "--locked"]).args(more).current_dir(&tools).output().is_ok_and(|o| o.status.success());
        assert!(fetch(&["--offline"]) || fetch(&[]), "cargo fetch of the crates of {} failed", tools.display());
    });
    let toml = read(&dir.join("Cargo.toml"));
    let name = toml.lines().find_map(|l| l.strip_prefix("name = \"")).and_then(|l| l.strip_suffix('"')).expect("the manifest names the crate").to_string();
    let version = format!("0.0.{}", tag.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b))));
    let toml = toml.replacen("version = \"0.0.0\"", &format!("version = \"{version}\""), 1);
    std::fs::write(dir.join("Cargo.toml"), format!("{toml}\n[workspace]\n")).unwrap();
    let lock = read(&tools.join("Cargo.lock"));
    let tested = "name = \"pkg\"\nversion = \"0.0.0\"\n";
    assert!(lock.contains(tested), "the crate of kaname's tests is pkg 0.0.0");
    std::fs::write(dir.join("Cargo.lock"), lock.replacen(tested, &format!("name = \"{name}\"\nversion = \"{version}\"\n"), 1)).unwrap();
}

/// The crate at `dir` ([`rust_crate`]) passes clippy's lints (rustc's alone where clippy is not here)
/// with `-D warnings`, as it is built, and for wasm32 where rustc has that target.
pub fn rust_lints(dir: &Path, target: &str) {
    let clippy = have_clippy();
    let mut more: Vec<&[&str]> = vec![&[]];
    if have_wasm32() {
        more.push(&["--target", "wasm32-unknown-unknown"]);
    }
    for more in more {
        let mut c = cargo_in(dir, target);
        c.args(if clippy { ["clippy", "--offline", "--quiet", "--lib"] } else { ["rustc", "--offline", "--quiet", "--lib"] }).args(more).args(["--", "-D", "warnings"]);
        let r = ritsu_testkit::run(&mut c, std::time::Duration::from_secs(1800));
        assert!(r.ok, "{}: the crate does not build with no warning ({more:?}):\n{}", dir.display(), r.both());
    }
}

/// The crate at `dir` ([`rust_crate`]) built for Wasm with the line its manifest says: the module. The
/// line is `ritsu_emit::wasm::CARGO_LINE`, which kaname's tests build with.
pub fn rust_wasm(dir: &Path, target: &str) -> PathBuf {
    let toml = read(&dir.join("Cargo.toml"));
    let line = toml.lines().find_map(|l| l.strip_prefix("#   cargo rustc ")).expect("the manifest says the line of its Wasm");
    let line = format!("cargo rustc --offline --quiet {line}");
    let r = ritsu_testkit::run(Command::new("sh").current_dir(dir).env("CARGO_TARGET_DIR", rust_target(target)).env_remove("RUSTFLAGS").env_remove("CARGO_ENCODED_RUSTFLAGS").arg("-c").arg(&line), std::time::Duration::from_secs(1800));
    assert!(r.ok, "{line}:\n{}", r.both());
    let name = toml.lines().find_map(|l| l.strip_prefix("name = \"")).and_then(|l| l.strip_suffix('"')).expect("the manifest names the crate");
    rust_target(target).join("wasm32-unknown-unknown/release").join(format!("{name}.wasm"))
}

/// A host of a package's Wasm in Node: the module imports nothing, and each line of the file is called,
/// each answer a line on the standard output.
const WASM_HOST: &str = r#"import { readFileSync } from "node:fs";

const mod = new WebAssembly.Module(readFileSync(process.argv[2]));
if (WebAssembly.Module.imports(mod).length !== 0) throw new Error("the module imports " + JSON.stringify(WebAssembly.Module.imports(mod)));
const ex = new WebAssembly.Instance(mod, {}).exports;
const enc = new TextEncoder();
const dec = new TextDecoder("utf-8", { fatal: true });
for (const line of readFileSync(process.argv[3], "utf8").split("\n")) {
  if (line === "") continue;
  const bytes = enc.encode(line);
  const ptr = ex.cabi_realloc(0, 0, 1, bytes.length);
  new Uint8Array(ex.memory.buffer, ptr, bytes.length).set(bytes);
  const ret = ex.call(ptr, bytes.length);
  const [p, n] = new Uint32Array(ex.memory.buffer, ret, 2);
  console.log(dec.decode(new Uint8Array(ex.memory.buffer, p, n)));
  ex.cabi_post_call(ret);
}
"#;

/// What the module answers to each line of the file `calls`, through a host of Node.
pub fn wasm_answers(dir: &Path, module: &Path, calls: &Path) -> String {
    let host = dir.join("wasm_host.mjs");
    std::fs::write(&host, WASM_HOST).unwrap();
    let r = ritsu_testkit::run(Command::new("node").arg(&host).arg(module).arg(calls), std::time::Duration::from_secs(600));
    assert!(r.ok, "the host of the Wasm:\n{}", r.both());
    r.stdout
}
