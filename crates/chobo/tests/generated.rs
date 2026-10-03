//! The code `chobo build` writes, as each language's own tools read it (PLAN C8): the TypeScript
//! clients pass `tsc --strict` with only erasable syntax (Node runs them by stripping the
//! types), the Python ones compile, and the Go ones are as gofmt writes them and pass go vet.

mod common;
use chobo::target::Target;
use common::runners::*;
use common::*;
use std::process::Command;

#[test]
fn typescript_type_checks() {
    if let Err(why) = node() {
        eprintln!("SKIP: {why}; the TypeScript clients are not type-checked");
        return;
    }
    let cases = cases();
    let work = TempDir::new("generated-ts");
    let dir = ts_dir(work.path());
    build_all(&cases, Target::PostgresTypeScript, &dir.join("postgres"));
    build_all(&cases, Target::TigerBeetleTypeScript, &dir.join("tigerbeetle"));
    std::fs::write(
        dir.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "strict": true,
    "noEmit": true,
    "target": "esnext",
    "module": "nodenext",
    "moduleResolution": "nodenext",
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "erasableSyntaxOnly": true,
    "noUnusedLocals": true,
    "types": ["node"]
  },
  "include": ["postgres/*.ts", "tigerbeetle/*.ts"]
}
"#,
    )
    .unwrap();
    let out = Command::new(runner_dir().join("node_modules/.bin/tsc")).arg("-p").arg(&dir).output().unwrap();
    assert!(out.status.success(), "tsc:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    eprintln!("type-checked: {} books, both databases", cases.len());
}

#[test]
fn python_compiles() {
    let py = match python() {
        Ok(p) => p,
        Err(why) => {
            eprintln!("SKIP: {why}; the Python clients are not compiled");
            return;
        }
    };
    let cases = cases();
    let work = TempDir::new("generated-py");
    let mut files = build_all(&cases, Target::PostgresPython, &work.path().join("postgres"));
    files.extend(build_all(&cases, Target::TigerBeetlePython, &work.path().join("tigerbeetle")));
    let out = Command::new(py).args(["-m", "py_compile"]).args(&files).output().unwrap();
    assert!(out.status.success(), "py_compile:\n{}", String::from_utf8_lossy(&out.stderr));
    eprintln!("compiled: {} Python files", files.len());
}

#[test]
fn go_is_formatted_and_vets() {
    if let Err(why) = go() {
        eprintln!("SKIP: {why}; the Go clients are not vetted");
        return;
    }
    let cases = cases();
    let work = TempDir::new("generated-go");
    let module = work.path();
    for f in ["go.mod", "go.sum"] {
        std::fs::copy(runner_dir().join("go").join(f), module.join(f)).unwrap();
    }
    for (i, c) in cases.iter().enumerate() {
        for (key, target) in [(format!("pg{i}"), Target::PostgresGo), (format!("tb{i}"), Target::TigerBeetleGo)] {
            for (rel, text) in chobo::target::build(&c.copy, &c.stem, target).unwrap() {
                let p = module.join("books").join(&key).join(std::path::Path::new(&rel).file_name().unwrap());
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, text).unwrap();
            }
        }
    }
    let out = Command::new("gofmt").arg("-l").arg(".").current_dir(module).output().unwrap();
    assert!(out.status.success() && out.stdout.is_empty(), "gofmt -l lists files it would change:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let out = go_in(module).args(["vet", "./..."]).output().unwrap();
    assert!(out.status.success(), "go vet:\n{}", String::from_utf8_lossy(&out.stderr));
    eprintln!("vetted: {} Go packages", cases.len() * 2);
}
