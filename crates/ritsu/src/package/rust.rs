//! The crate of a package of Rust (DESIGN 9.3): the generators of the languages write its parts, as in
//! the other languages, and this writes what holds them together. Each directory's modules are declared
//! in the file of its name, Rust 2018's way (`rules.rs` and `rules/`), and the crate's root is `lib.rs`,
//! with the three lints a name of another script raises allowed there. The modules of a dependency are
//! under `deps/<name>/`, its kinds declared in `deps/<name>.rs`. The clients of the books wait on a
//! database's driver, so they, and the crates they name, are left out of the package's Wasm, which
//! kaname's modules answer in (kaname's DESIGN 36.10): `wasm.rs` and the `.wit` are written when the
//! package has modules. The crate's manifest names the crates of the clients the package holds, at
//! exactly the versions their tests run, as a package does in every language (DESIGN 9.3).
//!
//! A package of Rust holds no workflow and no code of a gate, which dandori and sekisho do not write in
//! Rust: `ritsu gen` writes the other parts and warns ([`left_out`]). It is not written as a library
//! (`--library`), which has three things to decide first ([`no_library`]; kaname's DESIGN 19).

use super::{Options, Refusal, refusal};
use ritsu_base::diag::Severity;
use ritsu_base::json::Json;
use ritsu_base::naming::Tool;
use ritsu_base::text::Lang;
use ritsu_base::tr;
use ritsu_emit::header::Comment;
use ritsu_emit::package::Place;
use ritsu_ports::Finding;
use ritsu_project::Project;
use std::collections::BTreeMap;
use std::rc::Rc;

/// The edition and the oldest Rust the generated code is built with, as the crate of kaname's tests
/// names them (crates/kaname/tools/rust/Cargo.toml).
const EDITION: &str = "2024";
const RUST_VERSION: &str = "1.94";

/// What no file of a directory's parts can be the module of: Rust reads `mod.rs` as the directory's own
/// file, beside the `rules.rs` that declares it (E0761). The others that no raw identifier writes are
/// declared with their paths ([`ritsu_emit::package::rust_mod_line`]).
pub const NO_FILE_OF_A_DIRECTORY: [&str; 1] = ["mod"];

/// Whether a dependency's name cannot name its module of Rust and its file. A dependency's module holds
/// modules in turn, which Rust looks for beside a file it is told the path of (a file like `mod.rs`), not
/// under its name, so a name that needs the path is no name for it.
pub fn no_dependency(name: &str) -> bool {
    ritsu_emit::ident::RUST_UNRAW.contains(&name) || NO_FILE_OF_A_DIRECTORY.contains(&name)
}

/// Why a package of Rust is not written as a library, whether `--target` or the `target` line of
/// ritsu.mod asks for it.
pub fn no_library(lang: Lang) -> Refusal {
    refusal(
        2,
        tr!(
            "`--library` は Rust のパッケージを書きません。クレートの名前、依存の版の書き方、ライブラリをまたぐ単位の型を決めていないからです。`--library` を付けずに書くか、Rust を出力先（`--target` か ritsu.mod の `target` の行）から外してください",
            "`--library` does not write a package of Rust: a crate's name, how it writes the versions of what it depends on, and the units' types across libraries are not decided. Write it without `--library`, or take Rust out of the targets (`--target`, or the `target` line of ritsu.mod)"
        ),
        lang,
    )
}

/// What a package of Rust leaves out, said on a line for each file: the text, and the warnings for the
/// JSON, each as `ritsu check --format json` writes one.
pub fn left_out(project: &Project, lang: Lang) -> (String, Vec<Json>) {
    let head = if lang == Lang::Ja { "警告" } else { "warning" };
    let mut said = String::new();
    let mut warned = Vec::new();
    for (tool, what) in [(Tool::Dandori, tr!("ワークフロー", "workflow")), (Tool::Sekisho, tr!("ゲートのコード", "code of a gate"))] {
        for f in project.of(tool) {
            let shown = &f.shown;
            let message = tr!(
                "Rust のパッケージには{}を入れないので、`{shown}` は入りません。ほかの部分は書きます",
                "A package of Rust holds no {}, so it leaves out `{shown}`; the other parts are written",
                what.ja;
                what.en
            );
            let message = message.get(lang);
            said.push_str(&format!("{head}: {message}\n"));
            let json = Json::obj([("code", Json::Null), ("severity", Json::str(Severity::Warning.key())), ("line", Json::Null), ("message", Json::str(message))]);
            warned.push(crate::check::entry(tool.word(), &Finding { code: String::new(), severity: Severity::Warning, file: Some(f.rel.clone()), line: None, text: String::new(), json }));
        }
    }
    (said, warned)
}

/// The two files of the package's Wasm, when it has modules of kaname: the three functions of the ABI,
/// and the `.wit`, with the module's version from its ritsu.mod where it has one.
pub fn wasm_files(project: &Project, modules: &[(Rc<kaname::check::Checked>, String, Place)], name: &str, lang: Lang) -> Vec<(String, String, Option<String>)> {
    let version = match project.tree.as_deref() {
        Some(Ok(tree)) => tree.main().manifest.version.as_ref().map(|v| v.value.to_string()),
        _ => None,
    };
    let wit = kaname::r#gen::wasm::wit(modules, name, version.as_deref(), lang);
    vec![(kaname::r#gen::wasm::FILE.to_string(), kaname::r#gen::wasm::abi(lang), None), (kaname::r#gen::wasm::wit_file(name), wit, None)]
}

/// What holds the crate together, by the paths under the package's directory: `Cargo.toml`, `lib.rs`,
/// the file of each kind of each module's directory (glue/'s is kaname's), and in a module, the files
/// of the dependencies' directories.
pub fn crate_files(o: &Options, all: &BTreeMap<String, BTreeMap<&str, Vec<String>>>, files: &[(String, String, Option<String>)]) -> Vec<(String, String, Option<String>)> {
    let generated = ritsu_emit::header::generated("ritsu");
    let head = Comment::Slashes.line(&generated);
    let what = if all.values().any(|k| k.contains_key("glue")) {
        "The rules, the dates, the clients of the books and the functional cores (kaname's modules) of a project, as one package (ritsu gen)."
    } else {
        "The rules, the dates and the clients of the books of a project, as one package (ritsu gen)."
    };
    let mut out = Vec::new();
    // the dependencies of each module's directory the package holds parts of, by their names
    let mut below: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for d in all.keys() {
        if let Some(up) = super::parent_module_dir(d) {
            below.entry(up).or_default().push(ritsu_emit::package::chain(d).last().copied().unwrap_or_default());
        }
    }
    for (dir, names) in &below {
        let mut index = format!("{head}//! The dependencies of the package whose parts it holds, each by its name.\n\n");
        for n in names {
            index.push_str(&format!("pub mod {};\n", ritsu_emit::ident::rust_name(n)));
        }
        out.push((format!("{dir}deps.rs"), index, None));
    }
    let none = BTreeMap::new();
    let dirs: Vec<&str> = std::iter::once("").chain(all.keys().map(String::as_str).filter(|d| !d.is_empty())).collect();
    for dir in dirs {
        let kinds = all.get(dir).unwrap_or(&none);
        let prefix = ritsu_emit::package::chain(dir).iter().map(|n| format!("@{n}")).collect::<Vec<_>>().join("/");
        for (kind, modules) in kinds.iter().filter(|(k, _)| **k != "glue") {
            let whose = if dir.is_empty() { "the package".to_string() } else { format!("the dependency `{prefix}`") };
            let mut index = format!("{head}//! The {kind} of {whose}, a module each.\n\n");
            for m in modules {
                // the rules' and the dates' modules are their languages' code, which clippy is not held
                // to here (chobo's clients are, in chobo's tests)
                if *kind != "books" {
                    index.push_str("#[allow(clippy::all)]\n");
                }
                index.push_str(&ritsu_emit::package::rust_mod_line(&format!("{kind}/"), m));
            }
            out.push((format!("{dir}{kind}.rs"), index, None));
        }
        // the kinds and the dependencies of the directory, in the order of their names
        let mut entries: Vec<&str> = kinds.keys().copied().collect();
        if below.contains_key(dir) {
            entries.push("deps");
        }
        entries.sort();
        let mut lines = String::new();
        for k in entries {
            // the books' clients wait on a database's driver, which the Wasm has none of
            lines.push_str(&if k == "books" { kaname::r#gen::wasm::BOOKS.to_string() } else { format!("pub mod {k};\n") });
        }
        if dir.is_empty() {
            if files.iter().any(|(p, _, _)| p == "units.rs") {
                lines.push_str("pub mod units;\n");
            }
            if files.iter().any(|(p, _, _)| p == kaname::r#gen::wasm::FILE) {
                lines.push_str(kaname::r#gen::wasm::DECLARE);
            }
            out.push(("lib.rs".to_string(), format!("{head}//! {what}\n#![allow(confusable_idents, mixed_script_confusables, uncommon_codepoints)]\n\n{lines}"), None));
        } else {
            let what = format!("The parts of the dependency `{prefix}` the package holds, by their kinds.");
            out.push((format!("{}.rs", dir.trim_end_matches('/')), format!("{head}//! {what}\n\n{lines}"), None));
        }
    }
    out.push(("Cargo.toml".to_string(), manifest(o, all, what), None));
    out
}

/// The crate's manifest: its name, the line that builds its Wasm when it has one, and the crates of the
/// books' clients at exactly the versions their tests run (DESIGN 9.3), which the Wasm leaves out with
/// the clients. chobo's own line asks for a range where the client alone leaves the version to the crate
/// it goes into.
fn manifest(o: &Options, all: &BTreeMap<String, BTreeMap<&str, Vec<String>>>, what: &str) -> String {
    let head = Comment::Hash.line(&ritsu_emit::header::generated("ritsu"));
    let wasm = if all.values().any(|k| k.contains_key("glue")) { format!("#\n# Its Wasm, which answers through kaname's modules:\n#   {}\n", ritsu_emit::wasm::CARGO_LINE) } else { String::new() };
    let mut s = format!("{head}# {what}\n{wasm}\n[package]\nname = {}\nversion = \"0.0.0\"\nedition = \"{EDITION}\"\nrust-version = \"{RUST_VERSION}\"\npublish = false\n\n[lib]\npath = \"lib.rs\"\n", ritsu_emit::lit::json(&o.name));
    if all.values().any(|k| k.contains_key("books")) {
        s.push_str("\n[target.'cfg(not(target_arch = \"wasm32\"))'.dependencies]\n");
        for c in chobo::client::rust::crates(o.books.chobo(super::Target::Rust).backend()) {
            let exact = format!("\"={}\"", c.version.trim_start_matches('='));
            s.push_str(&format!("{}\n", c.line().replacen(&format!("\"{}\"", c.version), &exact, 1)));
        }
    }
    s
}
