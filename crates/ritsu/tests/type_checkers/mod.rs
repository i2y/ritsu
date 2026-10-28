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
