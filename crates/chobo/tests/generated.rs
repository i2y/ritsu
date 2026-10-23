//! The code `chobo build` writes, as each language's own tools read it (PLAN C8): the TypeScript
//! clients pass `tsc --strict` with only erasable syntax (Node runs them by stripping the
//! types), the Python ones compile, the Go ones are as gofmt writes them and pass go vet, and the
//! Rust ones build on the 2024 and the 2021 editions with no warning, clippy's among them.

mod common;
use chobo::target::Target;
use common::runners::*;
use common::*;
use std::process::Command;

#[test]
fn typescript_type_checks() {
    if !need(Need::Node) {
        return;
    }
    if let Err(why) = node() {
        skip(&format!("{why}; the TypeScript clients are not type-checked"));
        return;
    }
    let cases = cases();
    let work = TempDir::new("generated-ts");
    let dir = ts_dir(work.path());
    build_all(&cases, Target::PostgresTypeScript, &dir.join("postgres"));
    build_all(&cases, Target::TigerBeetleTypeScript, &dir.join("tigerbeetle"));
    build_all(&cases, Target::MySqlTypeScript, &dir.join("mysql"));
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
  "include": ["postgres/*.ts", "tigerbeetle/*.ts", "mysql/*.ts"]
}
"#,
    )
    .unwrap();
    let out = Command::new(runner_dir().join("node_modules/.bin/tsc")).arg("-p").arg(&dir).output().unwrap();
    assert!(out.status.success(), "tsc:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    eprintln!("type-checked: {} books, the three databases", cases.len());
}

#[test]
fn python_compiles() {
    if !need(Need::Python) {
        return;
    }
    let py = match python() {
        Ok(p) => p,
        Err(why) => {
            skip(&format!("{why}; the Python clients are not compiled"));
            return;
        }
    };
    let cases = cases();
    let work = TempDir::new("generated-py");
    let mut files = build_all(&cases, Target::PostgresPython, &work.path().join("postgres"));
    files.extend(build_all(&cases, Target::TigerBeetlePython, &work.path().join("tigerbeetle")));
    files.extend(build_all(&cases, Target::MySqlPython, &work.path().join("mysql")));
    let out = Command::new(py).args(["-m", "py_compile"]).args(&files).output().unwrap();
    assert!(out.status.success(), "py_compile:\n{}", String::from_utf8_lossy(&out.stderr));
    eprintln!("compiled: {} Python files", files.len());
}

#[test]
fn go_is_formatted_and_vets() {
    if !need(Need::Go) {
        return;
    }
    if let Err(why) = go() {
        skip(&format!("{why}; the Go clients are not vetted"));
        return;
    }
    let cases = cases();
    let work = TempDir::new("generated-go");
    let module = work.path();
    for f in ["go.mod", "go.sum"] {
        std::fs::copy(runner_dir().join("go").join(f), module.join(f)).unwrap();
    }
    for (i, c) in cases.iter().enumerate() {
        for (key, target) in [(format!("pg{i}"), Target::PostgresGo), (format!("tb{i}"), Target::TigerBeetleGo), (format!("my{i}"), Target::MySqlGo)] {
            for (rel, text) in chobo::target::build(&c.copy, &c.stem, target, &c.origin).unwrap() {
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
    eprintln!("vetted: {} Go packages", cases.len() * 3);
}

/// A crate of the runner's Cargo.toml and Cargo.lock in `dir`, its binary `name`, at `edition`,
/// with `modules` under src/books, `units` as src/units.rs when given, and a main.rs that holds
/// them to no warning.
fn rust_crate(dir: &Path, name: &str, edition: &str, modules: &[(String, String)], units: Option<&str>) {
    let toml = std::fs::read_to_string(runner_dir().join("rust/Cargo.toml")).unwrap();
    let toml = toml.replace("edition = \"2024\"", &format!("edition = \"{edition}\"")).replace("name = \"chobo-runner\"\npath", &format!("name = \"{name}\"\npath"));
    std::fs::create_dir_all(dir.join("src/books")).unwrap();
    std::fs::write(dir.join("Cargo.toml"), toml).unwrap();
    std::fs::copy(runner_dir().join("rust/Cargo.lock"), dir.join("Cargo.lock")).unwrap();
    let mut main = String::from("//! The Rust clients of the books, each a module.\n#![deny(warnings)]\n\n");
    if let Some(u) = units {
        std::fs::write(dir.join("src/units.rs"), u).unwrap();
        main.push_str("pub mod units;\n");
    }
    for (key, text) in modules {
        std::fs::write(dir.join("src/books").join(format!("{key}.rs")), text).unwrap();
        main.push_str(&format!("#[path = \"books/{key}.rs\"]\npub mod {key};\n"));
    }
    main.push_str("\nfn main() {}\n");
    std::fs::write(dir.join("src/main.rs"), main).unwrap();
}

/// cargo clippy (or cargo check where there is no clippy) on the crate in `dir`, held to no
/// warning: what it said when it did not pass.
fn rust_lints(dir: &Path, clippy: bool) -> Result<(), String> {
    let mut c = cargo_in(dir);
    if clippy {
        c.args(["clippy", "--locked", "--quiet", "--", "-D", "warnings"]);
    } else {
        c.args(["check", "--locked", "--quiet"]);
    }
    let out = c.output().unwrap();
    if out.status.success() { Ok(()) } else { Err(format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))) }
}

/// The Rust clients of every book, for the three databases, build on the 2024 and the 2021
/// editions with no warning, clippy's lints among them (the tests that run them build them on the
/// 2024 edition too, held to rustc's).
#[test]
fn rust_builds_on_both_editions_and_passes_clippy() {
    if !need(Need::Rustc) {
        return;
    }
    if let Err(why) = rust() {
        skip(&format!("{why}; the Rust clients are not built"));
        return;
    }
    let clippy = ritsu_testkit::tools::runs(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()), &["clippy", "--version"]);
    if !clippy {
        skip("cargo clippy is not here; the Rust clients are held to rustc's lints only");
    }
    let cases = cases();
    let mut modules = Vec::new();
    for (i, c) in cases.iter().enumerate() {
        for (prefix, target) in [("pg", Target::PostgresRust), ("tb", Target::TigerBeetleRust), ("my", Target::MySqlRust)] {
            let files = chobo::target::build(&c.copy, &c.stem, target, &c.origin).unwrap_or_else(|d| panic!("{}: {}", c.stem, d[0].message.en));
            assert_eq!(files.len(), 1, "{}: {target:?}", c.stem);
            modules.push((format!("{prefix}{i}"), files[0].1.clone()));
        }
    }
    let work = TempDir::new("generated-rust");
    for edition in ["2024", "2021"] {
        let dir = work.path().join(edition);
        rust_crate(&dir, &format!("generated-{edition}"), edition, &modules, None);
        if let Err(e) = rust_lints(&dir, clippy) {
            panic!("the Rust clients on the {edition} edition:\n{e}");
        }
    }
    eprintln!("built: {} Rust modules, on the 2024 and the 2021 editions{}", modules.len(), if clippy { ", clippy's lints among the warnings" } else { "" });
}

/// A client in a package of ritsu's takes its amounts of money and of quantities as the package's
/// types of their units (`crate::units::USDc`), read with `i64::from`; it builds with a `units`
/// module of newtypes of an i64, which is what the package's types are to be.
#[test]
fn a_rust_client_in_a_package_takes_the_types_of_its_units() {
    if !need(Need::Rustc) {
        return;
    }
    if let Err(why) = rust() {
        skip(&format!("{why}; the Rust clients in a package are not built"));
        return;
    }
    let pkg = chobo::client::InPackage { module: "example.com/shop".into() };
    let mut modules = Vec::new();
    let mut types: Vec<String> = Vec::new();
    for (i, path) in ["examples/marketplace/marketplace.book", "tests/books/allocation.book"].iter().enumerate() {
        let c = case(&root().join(path));
        for u in chobo::client::package_units(&c.copy) {
            let n = u.type_name().unwrap();
            if !types.contains(&n) {
                types.push(n);
            }
        }
        for (prefix, target) in [("pg", Target::PostgresRust), ("tb", Target::TigerBeetleRust), ("my", Target::MySqlRust)] {
            let files = chobo::target::build_in(&c.copy, &c.stem, target, &c.origin, Some(&pkg)).unwrap_or_else(|d| panic!("{}: {}", c.stem, d[0].message.en));
            assert!(files[0].1.contains("crate::units::"), "{path}: {target:?} takes no type of a unit");
            modules.push((format!("{prefix}{i}"), files[0].1.clone()));
        }
    }
    assert!(!types.is_empty());
    let units: String = types
        .iter()
        .map(|t| format!("/// An amount in {t}.\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub struct {t}(pub i64);\n\nimpl From<{t}> for i64 {{\n    fn from(v: {t}) -> i64 {{\n        v.0\n    }}\n}}\n\n"))
        .collect();
    let work = TempDir::new("generated-rust-units");
    let dir = work.path().join("2024");
    rust_crate(&dir, "generated-units", "2024", &modules, Some(&units));
    if let Err(e) = rust_lints(&dir, false) {
        panic!("the Rust clients in a package:\n{e}");
    }
    eprintln!("built: {} Rust modules in a package, with the types {}", modules.len(), types.join(", "));
}

/// The crates a Rust client names in its head, and `chobo api` under `targets.rust.crates`, are at
/// the versions the runner's Cargo.lock holds, which every test of the clients builds with: what a
/// client says it is tested with is what it is tested with.
#[test]
fn the_crates_the_rust_clients_name_are_the_ones_the_tests_lock() {
    use chobo::target::Backend;
    let lock = std::fs::read_to_string(runner_dir().join("rust/Cargo.lock")).unwrap();
    let mut locked: Vec<(String, String)> = Vec::new();
    let mut name = None;
    for line in lock.lines() {
        if let Some(n) = line.strip_prefix("name = ") {
            name = Some(n.trim_matches('"').to_string());
        } else if let (Some(v), Some(n)) = (line.strip_prefix("version = "), name.take()) {
            locked.push((n, v.trim_matches('"').to_string()));
        }
    }
    let toml = std::fs::read_to_string(runner_dir().join("rust/Cargo.toml")).unwrap();
    for b in [Backend::Postgres, Backend::TigerBeetle, Backend::MySql] {
        for c in chobo::client::rust::crates(b) {
            let v = c.version.trim_start_matches('=');
            let in_lock: Vec<&String> = locked.iter().filter(|(n, _)| n == c.name).map(|(_, v)| v).collect();
            assert_eq!(in_lock, [v], "{}: the clients name {}, and the runner's Cargo.lock holds {in_lock:?}", c.name, c.version);
            for f in c.features {
                assert!(toml.lines().any(|l| l.starts_with(&format!("{} = ", c.name)) && l.contains(&format!("\"{f}\""))), "{}: the runner's Cargo.toml does not turn on {f}", c.name);
            }
        }
    }
}

/// What E063 lets a Rust name begin with and hold (src/client/xid.rs) is what rustc takes, over
/// every character a name of chobo's may hold: each one E063 lets a name begin with goes through
/// `cargo check` as a name, and so does each one it lets a name hold after its first; each one it
/// keeps out is an error that names it. The table's ranges are of characters chobo takes, so none
/// of it is dead.
#[test]
fn e063_keeps_out_what_rustc_keeps_out() {
    use chobo::client::{rust, xid};
    use chobo::syntax::combining;
    use std::fmt::Write as _;
    if !need(Need::Rustc) {
        return;
    }
    if let Err(why) = rust() {
        skip(&format!("{why}; what E063 keeps out is not held to rustc"));
        return;
    }
    let first = |c: char| !combining(c) && (c.is_alphabetic() || c == '_');
    let after = |c: char| !combining(c) && (c.is_alphanumeric() || c == '_');
    for (name, table, takes) in [("NOT_FIRST", xid::NOT_FIRST, &first as &dyn Fn(char) -> bool), ("NOT_AFTER", xid::NOT_AFTER, &after)] {
        for &(lo, hi) in table {
            for u in lo..=hi {
                let c = char::from_u32(u).unwrap();
                assert!(takes(c) && c != '_', "{name} has U+{u:04X}, which a name of chobo's does not hold there");
            }
        }
    }
    let head = "#![allow(uncommon_codepoints, mixed_script_confusables, confusable_idents)]\nmacro_rules! m {\n    ($($t:tt)*) => {};\n}\nm! {\n";
    let (mut good, mut bad) = (String::from(head), String::from(head));
    let (mut kept_out, mut let_in) = (Vec::new(), 0usize);
    let mut run = String::new();
    for c in (0..=0x10FFFFu32).filter_map(char::from_u32).filter(|c| *c != '_') {
        if first(c) {
            if rust::begins(c) {
                let_in += 1;
                writeln!(good, "{c}").unwrap();
            } else {
                kept_out.push(c);
                writeln!(bad, "{c}").unwrap();
            }
        }
        if after(c) {
            if rust::holds(c) {
                let_in += 1;
                run.push(c);
                if run.chars().count() == 64 {
                    writeln!(good, "a{run}").unwrap();
                    run.clear();
                }
            } else {
                kept_out.push(c);
                writeln!(bad, "a{c}").unwrap();
            }
        }
    }
    writeln!(good, "a{run}\n}}").unwrap();
    bad.push_str("}\n");
    let work = TempDir::new("xid-rustc");
    let check = |name: &str, text: &str| {
        let dir = work.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), format!("[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[lib]\npath = \"lib.rs\"\n\n[workspace]\n")).unwrap();
        std::fs::write(dir.join("lib.rs"), text).unwrap();
        // rustc's errors are read below by their words, so they come with no colour, whatever
        // CARGO_TERM_COLOR the run has (CI sets `always`)
        cargo_in(&dir).args(["check", "--offline", "--quiet", "--lib", "--color", "never"]).output().unwrap()
    };
    let out = check("xid-let-in", &good);
    assert!(out.status.success(), "rustc keeps out a character E063 lets in:\n{}", String::from_utf8_lossy(&out.stderr).lines().take(40).collect::<Vec<_>>().join("\n"));
    let out = check("xid-kept-out", &bad);
    assert!(!out.status.success(), "rustc takes every character E063 keeps out");
    let err = String::from_utf8_lossy(&out.stderr);
    let unnamed: Vec<String> = kept_out
        .iter()
        .filter(|c| !err.contains(&format!("unknown start of token: {}", c.escape_unicode())) && !err.lines().any(|l| l.starts_with("error: identifiers cannot contain emoji: ") && l.contains(**c)))
        .map(|c| format!("U+{:04X}", *c as u32))
        .collect();
    assert!(unnamed.is_empty(), "rustc takes {} of the characters E063 keeps out: {}", unnamed.len(), unnamed.join(" "));
    eprintln!("compared: {let_in} characters E063 lets in and {} it keeps out with rustc (Unicode {:?} in xid.rs, {:?} in char)", kept_out.len(), xid::UNICODE, char::UNICODE_VERSION);
}
