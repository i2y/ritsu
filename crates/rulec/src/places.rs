//! What a run of `rulec gen` is to write, made sure of before it writes any of it (§15.334,
//! §15.335).
//!
//! Two files of a run do not write one place: one file under the output directory, two files whose
//! paths differ only in the case of their letters (one file where the file system does not tell
//! upper from lower case), or what one language holds as one, a Go package (its directory), a PHP
//! namespace, a Ruby module. `gen` used to write the second file over the first, or both into one
//! package that does not build, and exit 0; it now stops at 2 before it writes anything, naming the
//! two files and the place (§15.334). The places a rule writes are worked out from its alias and
//! from what the run is asked for, and no file's text is made for it ([`rule_files`]); tests/places.rs
//! holds them to what `gen` writes.
//!
//! A W121 that says the code of a target the run writes cannot be used as it stands
//! ([`Diag::targets`]) stops the run at 1 before it writes anything: E153 ([`stops`]). `rulec
//! check` says it as the warning it is, and `ritsu gen` asks the same of each rule it writes into a
//! package (`ports::Engine::stops`).

use crate::ast::{FileKind, Item, RuleFile};
use crate::diag::{Diag, Severity};
use crate::targets::{Asked, PROTO, Part};
use crate::types::{Checked, Ty};
use std::io::Write;
use std::path::PathBuf;

/// The targets an `enums` file's module is written in (`Gen::enums_module_files`, §15.212).
const ENUMS_TARGETS: &[&str] = &["python", "typescript", "javascript", "rust", "ruby", "php", "go", "swift", "java"];

/// The files the backend `id` writes of a rule whose files are named after `alias` (`pkg` its Go
/// package, `cls` its Java class), by path under the output directory, each with its part and
/// whether it is a file every rule writes the same (`python/_round_test.py`, `numpy/rulec_np.py`):
/// what the backend's `files` writes (src/backend.rs), without the text of any of them. Empty for
/// an id that is no backend's.
pub fn rule_files(id: &str, alias: &str, pkg: &str, cls: &str) -> Vec<(String, Part, bool)> {
    use Part::*;
    let own = |p: String, part: Part| (p, part, false);
    let shared = |p: &str, part: Part| (p.to_string(), part, true);
    match id {
        "python" => vec![
            own(format!("python/{alias}.py"), Library),
            own(format!("python/{alias}_runner.py"), Runner),
            own(format!("python/{alias}_mcp.py"), Mcp),
            own(format!("python/{alias}_service.py"), Connect),
            own(format!("python/{alias}_connect_runner.py"), ConnectRunner),
            shared("python/_round_test.py", RoundTest),
            own(format!("python/{alias}_page.html"), Mcp),
        ],
        "numpy" => vec![
            own(format!("numpy/{alias}.json"), Library),
            shared("numpy/rulec_np.py", Library),
            own(format!("numpy/{alias}_runner.py"), Runner),
            shared("numpy/_round_test.py", RoundTest),
        ],
        "typescript" => vec![
            own(format!("typescript/{alias}.ts"), Library),
            own(format!("typescript/{alias}_runner.ts"), Runner),
            own(format!("typescript/{alias}_mcp.ts"), Mcp),
            shared("typescript/_round_test.ts", RoundTest),
            own(format!("typescript/{alias}_page.html"), Mcp),
        ],
        "javascript" => vec![
            own(format!("javascript/{alias}.mjs"), Library),
            own(format!("javascript/{alias}_runner.mjs"), Runner),
            own(format!("javascript/{alias}_mcp.mjs"), Mcp),
            shared("javascript/_round_test.mjs", RoundTest),
            own(format!("javascript/{alias}_page.html"), Mcp),
        ],
        "rust" => vec![
            own(format!("rust/{alias}.rs"), Library),
            own(format!("rust/{alias}_runner.rs"), Runner),
            own(format!("rust/{alias}_proof.rs"), Proof),
            shared("rust/_round_test.rs", RoundTest),
        ],
        "ruby" => vec![
            own(format!("ruby/{alias}.rb"), Library),
            own(format!("ruby/{alias}_runner.rb"), Runner),
            own(format!("ruby/sig/{alias}.rbs"), Library),
            shared("ruby/_round_test.rb", RoundTest),
        ],
        "php" => vec![
            own(format!("php/{alias}.php"), Library),
            own(format!("php/{alias}_runner.php"), Runner),
            shared("php/_round_test.php", RoundTest),
        ],
        "go" => vec![
            own(format!("go/{pkg}/{alias}.go"), Library),
            own(format!("go/{pkg}/go.mod"), Library),
            own(format!("go/{pkg}runner/main.go"), Runner),
            own(format!("go/{pkg}runner/go.mod"), Runner),
            own(format!("go/{pkg}/round_test.go"), RoundTest),
        ],
        "swift" => vec![
            own(format!("swift/{alias}.swift"), Library),
            own(format!("swift/{alias}_runner.swift"), Runner),
            shared("swift/_round_test.swift", RoundTest),
        ],
        "java" => vec![
            own(format!("java/{cls}.java"), Library),
            own(format!("java/{cls}Runner.java"), Runner),
            shared("java/_RoundTest.java", RoundTest),
        ],
        "sql" => vec![
            own(format!("sql/{alias}.sql"), Library),
            own(format!("sql/{alias}_function.sql"), Library),
            own(format!("sql/{alias}_runner.py"), Runner),
            own(format!("sql/{alias}_function_runner.py"), Runner),
            shared("sql/_round_test.py", RoundTest),
        ],
        "wasm" => vec![
            own(format!("wasm/{alias}.rs"), Library),
            own(format!("wasm/{alias}_wasm.rs"), Library),
            own(format!("wasm/{alias}.wit"), Library),
            own(format!("wasm/{alias}_runner.mjs"), Runner),
            shared("wasm/_round.rs", RoundTest),
            shared("wasm/_round_test.mjs", RoundTest),
        ],
        _ => Vec::new(),
    }
}

/// The module of an `enums` file whose files are named after `alias`, in each language that has
/// one, by path under the output directory: what `Gen::enums_module_files` writes (§15.212).
pub fn enums_files(alias: &str, pkg: &str, cls: &str) -> Vec<String> {
    vec![
        format!("python/{alias}.py"),
        format!("typescript/{alias}.ts"),
        format!("javascript/{alias}.mjs"),
        format!("rust/{alias}.rs"),
        format!("ruby/{alias}.rb"),
        format!("ruby/sig/{alias}.rbs"),
        format!("php/{alias}.php"),
        format!("go/{pkg}/{alias}.go"),
        format!("go/{pkg}/go.mod"),
        format!("swift/{alias}.swift"),
        format!("java/{cls}.java"),
    ]
}

/// The runners of the `prerequisites` of an `enums` file (§15.222), by path under the output
/// directory: what `Gen::enums_runner_files` writes.
pub fn enums_runner_files(alias: &str, pkg: &str, cls: &str) -> Vec<String> {
    vec![
        format!("python/{alias}_runner.py"),
        format!("typescript/{alias}_runner.ts"),
        format!("javascript/{alias}_runner.mjs"),
        format!("rust/{alias}_runner.rs"),
        format!("ruby/{alias}_runner.rb"),
        format!("php/{alias}_runner.php"),
        format!("go/{pkg}runner/main.go"),
        format!("go/{pkg}runner/go.mod"),
        format!("swift/{alias}_runner.swift"),
        format!("java/{cls}Runner.java"),
    ]
}

/// What two files of a run cannot both write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A file, by its path under the output directory.
    File,
    /// A Go package, by its directory under the output directory: the files of one directory are
    /// one package, and two rules' files there declare the same names.
    GoPackage,
    /// A PHP namespace, which PHP does not tell apart by the case of its letters.
    PhpNamespace,
    /// A Ruby module: a second file that writes one reopens it, and its methods and constants
    /// replace the first one's.
    RubyModule,
}

/// One place a file of the run writes: `name` is the path for a file, the directory for a Go
/// package, the name for a namespace or a module; `file` is the file that writes it, by its path
/// under the output directory; `shared` is a file every rule writes the same.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub kind: Kind,
    pub name: String,
    pub file: String,
    pub shared: bool,
}

impl Place {
    fn file(rel: String, shared: bool) -> Place {
        Place { kind: Kind::File, name: rel.clone(), file: rel, shared }
    }

    fn named(kind: Kind, name: String, file: String) -> Place {
        Place { kind, name, file, shared: false }
    }
}

/// The places a rule writes in a run asked for `asked`, and the targets and extras whose code of it
/// the run writes, by the names `--target` and `--extra` take: each target given but one that writes
/// no rule that walks a sequence or reads a table's column of strings (as `generate` leaves them
/// out, §15.56, §15.101), `proto` for the rule's own `.proto`, `connect` for the Connect service
/// beside its Python.
pub fn of_rule(f: &RuleFile, c: &Checked, asked: &Asked) -> (Vec<Place>, Vec<&'static str>) {
    let alias = crate::codegen::pub_name_of(&f.name);
    let pkg = crate::backend::go_package(&alias);
    let cls = crate::codegen::java_class(&alias);
    let g = crate::codegen::Gen::for_names(f, c);
    let text_column = f.items.iter().any(|it| match it {
        Item::Table(t) => t.inputs.iter().any(|(n, _)| matches!(c.ty_of(n), Some(Ty::Str))),
        _ => false,
    });
    let mut places = Vec::new();
    let mut writes: Vec<&'static str> = Vec::new();
    for b in crate::backend::ALL.iter().filter(|b| asked.target(b.id)) {
        if (!f.elements.is_empty() && !b.folds) || (!b.texts && text_column) {
            continue;
        }
        writes.push(b.id);
        for (rel, part, shared) in rule_files(b.id, &alias, &pkg, &cls) {
            if asked.wants(b.id, part) {
                places.push(Place::file(rel, shared));
            }
        }
        places.extend(namespaces(b.id, &alias, &pkg, &g, asked.wants(b.id, Part::Runner)));
    }
    if asked.target(PROTO) && f.implements.is_none() {
        places.push(Place::file(format!("proto/{}", g.proto_path()), false));
        writes.push(PROTO);
    }
    if asked.extra("connect") && writes.contains(&"python") {
        writes.push("connect");
    }
    if asked.extra("vectors") {
        places.push(Place::file(format!("vectors/{alias}.jsonl"), false));
        places.push(Place::file(format!("vectors/{alias}.expected.jsonl"), false));
        if !g.prereq_graphs().is_empty() {
            places.extend(prereq_vectors(&alias));
        }
    }
    (places, writes)
}

/// The places an `enums` file writes in a run asked for `asked`: its module in each language
/// given, and, for one named on the command line (`named`), the runners of its `prerequisites` and
/// their vectors (§15.222). A rule that reads it has its module written, and nothing else of it.
pub fn of_enums(f: &RuleFile, c: &Checked, asked: &Asked, named: bool) -> Vec<Place> {
    let alias = crate::codegen::pub_name_of(&f.name);
    let pkg = crate::backend::go_package(&alias);
    let cls = crate::codegen::java_class(&alias);
    let g = crate::codegen::Gen::for_names(f, c);
    let of = |rel: &str| rel.split('/').next().unwrap_or_default().to_string();
    let mut places: Vec<Place> = enums_files(&alias, &pkg, &cls).into_iter().filter(|rel| asked.target(&of(rel))).map(|rel| Place::file(rel, false)).collect();
    let runners = named && !g.prereq_graphs().is_empty();
    for t in ENUMS_TARGETS.iter().filter(|t| asked.target(t)) {
        places.extend(namespaces(t, &alias, &pkg, &g, runners && asked.wants(t, Part::Runner)));
    }
    if runners {
        places.extend(enums_runner_files(&alias, &pkg, &cls).into_iter().filter(|rel| asked.wants(&of(rel), Part::Runner)).map(|rel| Place::file(rel, false)));
        if asked.extra("vectors") {
            places.extend(prereq_vectors(&alias));
        }
    }
    places
}

/// The vectors of the functions of `prerequisites` (§15.222).
fn prereq_vectors(alias: &str) -> [Place; 2] {
    [Place::file(format!("vectors/{alias}.prereq.tsv"), false), Place::file(format!("vectors/{alias}.prereq.expected"), false)]
}

/// What the code of the language of `id` holds as one, of a file named after `alias`: its Go
/// package (and the runner's, with `runner`), its PHP namespace (which PHP does not tell apart by
/// the case of its letters, as [`clash`] reads it), its Ruby module.
fn namespaces(id: &str, alias: &str, pkg: &str, g: &crate::codegen::Gen, runner: bool) -> Vec<Place> {
    match id {
        "go" => {
            let mut v = vec![Place::named(Kind::GoPackage, format!("go/{pkg}"), format!("go/{pkg}/{alias}.go"))];
            if runner {
                v.push(Place::named(Kind::GoPackage, format!("go/{pkg}runner"), format!("go/{pkg}runner/main.go")));
            }
            v
        }
        "php" => vec![Place::named(Kind::PhpNamespace, g.php_ns(), format!("php/{alias}.php"))],
        "ruby" => vec![Place::named(Kind::RubyModule, crate::codegen::ruby_module(alias), format!("ruby/{alias}.rb"))],
        _ => Vec::new(),
    }
}

/// Two places of two files of the run that are one: the first file met that writes it and the
/// second, each as (the file of the run, its place).
#[derive(Debug)]
pub struct Clash<'a> {
    pub first: (&'a str, &'a Place),
    pub second: (&'a str, &'a Place),
}

/// The first two places of `placed` (each with the file of the run that writes it) that are one,
/// looked for in this order: a Go package, a PHP namespace, a Ruby module, a file, two paths that
/// differ only in case. Places of one file of the run are never two, and two files every rule
/// writes the same are one file written twice, not two.
pub fn clash<'a>(placed: &'a [(String, Place)]) -> Option<Clash<'a>> {
    // what is compared: the name, or the name in lower case where the case of its letters is not
    // told apart
    let passes: [(Kind, bool); 5] = [(Kind::GoPackage, false), (Kind::PhpNamespace, true), (Kind::RubyModule, false), (Kind::File, false), (Kind::File, true)];
    for (kind, folded) in passes {
        let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for (i, (from, p)) in placed.iter().enumerate() {
            if p.kind != kind {
                continue;
            }
            let key = if folded { p.name.to_lowercase() } else { p.name.clone() };
            match seen.get(&key) {
                Some(&j) => {
                    let (prev_from, prev) = (&placed[j].0, &placed[j].1);
                    let one = if kind == Kind::File && folded {
                        // one file under two spellings of its path, whoever writes them
                        prev.name != p.name
                    } else {
                        prev_from != from && !(prev.shared && p.shared)
                    };
                    if one {
                        return Some(Clash { first: (prev_from, prev), second: (from, p) });
                    }
                }
                None => {
                    seen.insert(key, i);
                }
            }
        }
    }
    None
}

/// What `gen` says of a clash, with the paths under `out_dir` (§15.334): the two files, the place,
/// and to give one of them another alias.
pub fn clash_text(c: &Clash, out_dir: &str) -> String {
    let ((a, pa), (b, pb)) = (c.first, c.second);
    let at = |rel: &str| if out_dir.is_empty() { rel.to_string() } else { format!("{out_dir}/{rel}") };
    let (p, q) = (at(&pa.file), at(&pb.file));
    // one file, whatever else the two hold as one (two rules of one alias write one Go package too)
    if pa.file == pb.file {
        return tr!("`{a}` と `{b}` が、同じ {p} を書きます。どちらかの別名を変えてください", "`{a}` and `{b}` both write {p}; give one of them another alias");
    }
    match pa.kind {
        Kind::File => tr!(
            "`{a}` の {p} と `{b}` の {q} は、大文字と小文字だけが違います。大文字と小文字を区別しないファイルシステムでは一つのファイルなので、どちらかの別名を変えてください",
            "`{a}` writes {p} and `{b}` writes {q}, which differ only in the case of their letters and are one file where the file system does not tell upper from lower case; give one of them another alias"
        ),
        Kind::GoPackage => {
            let d = at(&pa.name);
            tr!(
                "`{a}` と `{b}` が、どちらも Go のパッケージ {d}/ に書きます（{p} と {q}）。一つのパッケージで同じ名前を二度宣言するので組めません。どちらかの別名を変えてください",
                "`{a}` and `{b}` both write into the Go package {d}/ ({p} and {q}), which then declares the same names twice and does not build; give one of them another alias"
            )
        }
        Kind::PhpNamespace => {
            let (m, n) = (&pa.name, &pb.name);
            if m == n {
                tr!(
                    "`{a}` と `{b}` が、同じ PHP の名前空間 `{m}` を宣言します（{p} と {q}）。二つを読み込むと同じクラスを二度宣言するので、どちらかの別名を変えてください",
                    "`{a}` and `{b}` both declare the PHP namespace `{m}` ({p} and {q}), so loading the two declares the same classes twice; give one of them another alias"
                )
            } else {
                tr!(
                    "`{a}` の PHP の名前空間 `{m}`（{p}）と `{b}` の `{n}`（{q}）は、大文字と小文字だけが違い、PHP には一つの名前空間です。二つを読み込むと同じクラスを二度宣言するので、どちらかの別名を変えてください",
                    "`{a}` declares the PHP namespace `{m}` ({p}) and `{b}` declares `{n}` ({q}), which differ only in the case of their letters and are one namespace to PHP, so loading the two declares the same classes twice; give one of them another alias"
                )
            }
        }
        Kind::RubyModule => {
            let m = &pa.name;
            tr!(
                "`{a}` と `{b}` が、同じ Ruby のモジュール `{m}` を書きます（{p} と {q}）。二つを読み込むと、後のファイルが先のファイルのメソッドと定数を上書きするので、どちらかの別名を変えてください",
                "`{a}` and `{b}` both write the Ruby module `{m}` ({p} and {q}), so loading the two lets the later file overwrite the methods and constants of the earlier; give one of them another alias"
            )
        }
    }
}

/// What stops the code of the targets `writes` says the run writes (§15.335): E153, each W121 of
/// `diags` that names one of them as a target whose code would not build or would answer otherwise
/// ([`Diag::targets`]), as the warning says it, at the same place, with a last note that names the
/// targets and says that nothing is written. An error, as kaname's E008 is of its W001.
pub fn stops(diags: &[Diag], writes: &dyn Fn(&str) -> bool) -> Vec<Diag> {
    stops_where(diags, &|_, t| writes(t))
}

/// [`stops`], for a package of `target` that holds each rule in a module of its own (`ritsu gen`'s):
/// a W121 that names the target only for the module `rulec gen` writes beside the language's own
/// ([`Diag::beside`]: a rule named like a module of the standard library) does not stop it.
pub fn package_stops(diags: &[Diag], target: &str) -> Vec<Diag> {
    stops_where(diags, &|d, t| t == target && !d.beside.contains(&t))
}

/// The targets and extras each W121 of `diags` names that `writes` says of it — the same picking
/// as [`stops_where`], as the set of what [`plan`] takes out of the run (§15.335).
fn stop_targets(diags: &[Diag], writes: &dyn Fn(&Diag, &str) -> bool) -> Vec<&'static str> {
    diags
        .iter()
        .filter(|d| d.code == "W121")
        .flat_map(|d| d.targets.iter().copied().filter(|t| writes(d, t)))
        .collect()
}

/// Each W121 of `diags`, as E153, that names a target `writes` says of it.
fn stops_where(diags: &[Diag], writes: &dyn Fn(&Diag, &str) -> bool) -> Vec<Diag> {
    diags
        .iter()
        .filter(|d| d.code == "W121")
        .filter_map(|d| {
            let named: Vec<&str> = d.targets.iter().copied().filter(|t| writes(d, t)).collect();
            if named.is_empty() {
                return None;
            }
            let mut e = d.clone();
            e.code = "E153";
            e.severity = Severity::Error;
            Some(e.note(stop_note(&named)))
        })
        .collect()
}

/// The last note of E153: the flags whose code cannot be used, that gen writes nothing of them,
/// and that the targets it does not name are written.
fn stop_note(named: &[&str]) -> String {
    let flags: Vec<String> = named.iter().map(|t| if crate::targets::EXTRAS.contains(t) { format!("`--extra {t}`") } else { format!("`--target {t}`") }).collect();
    let flags = match flags.split_last() {
        Some((last, rest)) if !rest.is_empty() => {
            if crate::i18n::ja() {
                flags.join("・")
            } else {
                format!("{} and {last}", rest.join(", "))
            }
        }
        _ => flags.concat(),
    };
    tr!(
        "{flags} が書くコードは、この名前のままでは使えないので、gen は{flags}を書かず、ほかの出力先は書きます。",
        "The code {flags} writes cannot be used with the name as it stands, so gen writes nothing of {flags}; the targets it does not name are written."
    )
}

/// One file of the run: as the run names it, and as it is on the disk, so that one file reached by
/// two spellings of its path is one.
struct Source {
    shown: String,
    same: PathBuf,
}

/// The index of the file at `path` among `sources`, put there when it is new, and whether it is.
fn source(sources: &mut Vec<Source>, path: &str) -> (usize, bool) {
    let same = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
    match sources.iter().position(|s| s.same == same) {
        Some(i) => (i, false),
        None => {
            sources.push(Source { shown: path.to_string(), same });
            (sources.len() - 1, true)
        }
    }
}

/// What a run of `files` asked for `asked` is to write, as [`plan`] works it out: each place, with
/// the file of the run that writes it, as the run names the file. What tests/places.rs holds to what
/// `gen` writes.
pub fn places(files: &[&str], asked: &Asked) -> Vec<(String, Place)> {
    plan(files, asked).0
}

/// What a file of the run is, for [`plan`]'s second pass: its place among the run's `sources`, what
/// it was read and checked as, and for an `enums` file whether the run names it itself.
enum Pending {
    Rule(usize, RuleFile, Checked),
    Enums(usize, RuleFile, Checked, bool),
}

/// The places of the run, and what E153 says of its files, in the words of `check`, and the targets
/// and extras the warnings name: each of `files` that reads and passes check, with the `enums`
/// files it takes its types from (read once, however many rules read one). One that does not read
/// or pass is left to `generate`, which stops at it as it did. The places are worked out in a
/// second pass, of the targets and extras no W121 of the run names: a name one target's code
/// cannot take stops the target in every file's, not only the file that says it (§15.335).
fn plan(files: &[&str], asked: &Asked) -> (Vec<(String, Place)>, String, Vec<&'static str>) {
    let mut sources: Vec<Source> = Vec::new();
    let mut pending: Vec<Pending> = Vec::new();
    let mut stopping = String::new();
    let mut stopped: Vec<&'static str> = Vec::new();
    for path in files {
        let Ok(src) = ritsu_base::fs::read_to_string(path) else { continue };
        let report = crate::report(&src, path);
        if crate::has_error(&report.diags) {
            continue;
        }
        let Ok((f, c)) = crate::prepare(&src, path) else { continue };
        let (i, new) = source(&mut sources, path);
        let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
        if f.kind == FileKind::Enums {
            pending.push(Pending::Enums(i, f, c, true));
            if new {
                let writes = |_: &Diag, t: &str| asked.target(t) && ENUMS_TARGETS.contains(&t);
                stopped.extend(stop_targets(&report.diags, &writes));
                stopping.push_str(&crate::findings_text(&stops_where(&report.diags, &writes), &lines));
            }
            continue;
        }
        let (_, writes) = of_rule(&f, &c, asked);
        let owners = crate::enums::owner_paths(&f);
        pending.push(Pending::Rule(i, f, c));
        if new {
            stopped.extend(stop_targets(&report.diags, &|_, t| writes.contains(&t)));
            stopping.push_str(&crate::findings_text(&stops(&report.diags, &|t| writes.contains(&t)), &lines));
        }
        // the modules of the `enums` files it reads its types from (§15.212)
        for p in owners {
            let Ok(e) = crate::enums::read_enums_file(&p) else { continue };
            let (j, new) = source(&mut sources, &p);
            pending.push(Pending::Enums(j, e.file, e.checked, false));
            if new {
                let r = crate::report(&e.src, &e.path);
                let lines: Vec<String> = e.src.lines().map(|s| s.to_string()).collect();
                let writes = |_: &Diag, t: &str| asked.target(t) && ENUMS_TARGETS.contains(&t);
                stopped.extend(stop_targets(&r.diags, &writes));
                stopping.push_str(&crate::findings_text(&stops_where(&r.diags, &writes), &lines));
            }
        }
    }
    let asked = asked.without(&stopped);
    let mut placed: Vec<(usize, Place)> = Vec::new();
    for p in pending {
        match p {
            Pending::Rule(i, f, c) => placed.extend(of_rule(&f, &c, &asked).0.into_iter().map(|p| (i, p))),
            Pending::Enums(i, f, c, named) => placed.extend(of_enums(&f, &c, &asked, named).into_iter().map(|p| (i, p))),
        }
    }
    (placed.into_iter().map(|(i, p)| (sources[i].shown.clone(), p)).collect(), stopping, stopped)
}

/// What `rulec gen` does next of what [`stopped`] finds (cli.rs, ahead of `codegen::generate`).
pub enum Stops {
    /// Nothing is stopped and no two files write one place: the run writes all of `asked`.
    Go,
    /// E153 names targets whose code cannot take a name: the run writes what is left of `asked`
    /// — the targets and extras no warning names — then says the refusal and exits with `code`
    /// (§15.335).
    Rest(Asked, u8),
    /// Nothing is written: the code the run exits with — every target it writes stopped (E153, 1),
    /// two files writing one place (§15.334, 2).
    Halt(u8),
}

/// What stops `rulec gen` before it writes anything (cli.rs, ahead of `codegen::generate`): each
/// W121 of the run's files that names a target it writes, said as E153 on `out` in the words of
/// `check`, and the run goes on without the targets they name (§15.335). When they name all of it,
/// or when two files write one place (§15.334), nothing is written. The run is read in the
/// language it prints in, as `generate` reads each rule, so the check of each rule is made once.
pub fn stopped(files: &[&str], out_dir: &str, asked: &Asked, out: &mut dyn Write, err: &mut dyn Write) -> Stops {
    let out_dir = out_dir.trim_end_matches('/');
    let (placed, stopping, stopped) = plan(files, asked);
    if !stopping.is_empty() {
        let _ = write!(out, "{stopping}");
        let _ = writeln!(err, "{}", ritsu_base::cli::refusal(&tr!("生成するコードの名前を直すまで、E153 が挙げた出力先は書きません（E153）", "the targets E153 names are not written until the names of the code are put right"), crate::i18n::base()));
        let asked = asked.without(&stopped);
        return if asked.targets().is_empty() && asked.extras().is_empty() { Stops::Halt(1) } else { Stops::Rest(asked, 1) };
    }
    let Some(c) = clash(&placed) else { return Stops::Go };
    let _ = writeln!(err, "{}", ritsu_base::cli::refusal(&clash_text(&c, out_dir), crate::i18n::base()));
    Stops::Halt(2)
}
