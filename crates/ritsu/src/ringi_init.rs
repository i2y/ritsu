//! `ritsu ringi init [<dir>] --module <path>`: the first files of a Go project that serves requests
//! and approvals on the framework (ringi's DESIGN 16.4). One request type, the organization it
//! reads, a gate and a calendar under `project/`; the code `ritsu gen` makes of them, once; and the
//! Go of a server: its go.mod, the `//go:generate` line that makes the code again, the glue between
//! the server's questions and the generated gate, a notifier that writes to the log, the server's
//! command and its tests. `--lang ja` writes the project with Japanese names, and a Japanese README
//! beside the English one.
//!
//! Two options add to the project. `--mcp` serves the agents' tools at `/mcp` of the server (the
//! framework's module `mcp`, on the route `serve` adds), with a test that makes a request with them.
//! `--temporal` adds a request type whose approval starts a flow, the flow, the code that starts the
//! flows on Temporal and makes their workers (the framework's module `temporal`), the flow's tasks,
//! a worker's command, and a test that runs the flow on Temporal's dev server. A template of a file
//! the options change marks what an option adds or replaces with lines of their own (`{{if mcp}}`,
//! `{{if temporal}}`, `{{else}}`, `{{end}}`; see [`sections`]).
//!
//! The files are the user's from then on: `init` writes each once, and overwrites none. A file in
//! the way stops it before anything is written, and it names every one. The code it generates is
//! what the `//go:generate` line writes (it runs that line), so `go generate` changes nothing until
//! `project/` changes.
//!
//! It runs inside ritsu only: ringi's binary has no `ritsu gen` to make the code with, and its `init`
//! says to run this one. The templates are in `ringi_init/`, each with the suffix `.tmpl`, so that
//! no command that reads a tree's `.ringi`, `.gate` or `.cal` files reads them.

use ritsu_base::cli::{Args, Table, cannot_create, cannot_read, cannot_write, refuse};
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The framework's module, the server the files are written for.
pub const FRAMEWORK: &str = "github.com/i2y/ringi-server";

/// The version of the framework the files require: the one that takes the contract this ritsu
/// generates (ringi's DESIGN 12.4, at the version `ringi::lower::shape::CONTRACT` says). A release
/// of ritsu that changes the contract raises it to the framework's release that takes the new one;
/// ringi's DESIGN 16.4 lists the places to raise with it.
pub const FRAMEWORK_VERSION: &str = "v0.1.0";

/// The `go` line of the framework's go.mod at [`FRAMEWORK_VERSION`]: the project's is no earlier.
const FRAMEWORK_GO: &str = "1.26.0";

/// The Go module the generated gate imports.
const CEDAR_GO: &str = "github.com/cedar-policy/cedar-go";

/// The day the organization's data of the first files takes effect: its directory under
/// `project/org/`.
const ORG_DAY: &str = "2026-04-01";

/// The names `cmd/server/main.go` gives its imports and its variables: the project's package is
/// imported under another name when it would take one of them (`project`).
const MAIN_NAMES: &[&str] = &["context", "flag", "slog", "os", "signal", "syscall", "server", "serve", "main", "run", "flags", "log", "ctx", "stop", "stores", "cfg", "err", "error", "nil", "srv"];

/// The names the commands give their imports and variables with `--temporal`, besides
/// [`MAIN_NAMES`]: those of `cmd/server/main.go` for the flows, and those of `cmd/worker/main.go`.
const TEMPORAL_NAMES: &[&str] = &[
    "errors", "fmt", "sync", "time", "client", "sdklog", "temporal", "c", "flows", "cancel", "running", "ready", "cause", "string", "worker", "address", "workers", "i", "w", "started",
];

/// The Go modules the code of the flows imports, which `ritsu gen` writes it against: the project
/// requires the versions ritsu tests (`GO_MODULES`).
const TEMPORAL_SDK: &str = "go.temporal.io/sdk";
const TEMPORAL_API: &str = "go.temporal.io/api";

/// The files of the project, by language: the request type, the organization's shape, the
/// calendar and the gate.
const PROJECT_EN: &[(&str, &str)] = &[
    ("project/purchase.ringi", include_str!("ringi_init/en/purchase.ringi.tmpl")),
    ("project/shared/company.ringi", include_str!("ringi_init/en/company.ringi.tmpl")),
    ("project/shared/office.cal", include_str!("ringi_init/en/office.cal.tmpl")),
    ("project/shared/company.gate", include_str!("ringi_init/en/company.gate.tmpl")),
];
const PROJECT_JA: &[(&str, &str)] = &[
    ("project/purchase.ringi", include_str!("ringi_init/ja/purchase.ringi.tmpl")),
    ("project/shared/company.ringi", include_str!("ringi_init/ja/company.ringi.tmpl")),
    ("project/shared/office.cal", include_str!("ringi_init/ja/office.cal.tmpl")),
    ("project/shared/company.gate", include_str!("ringi_init/ja/company.gate.tmpl")),
];

/// The files of the project `--temporal` adds, by language: a request type whose approval starts a
/// flow, and the flow.
const FLOW_EN: &[(&str, &str)] = &[
    ("project/expense.ringi", include_str!("ringi_init/en/expense.ringi.tmpl")),
    ("project/shared/pay.flow", include_str!("ringi_init/en/pay.flow.tmpl")),
];
const FLOW_JA: &[(&str, &str)] = &[
    ("project/expense.ringi", include_str!("ringi_init/ja/expense.ringi.tmpl")),
    ("project/shared/pay.flow", include_str!("ringi_init/ja/pay.flow.tmpl")),
];

/// The organization's data, the same in both languages (the people have a name in each).
const ORG: &[(&str, &str)] = &[
    ("teams.csv", include_str!("ringi_init/org/teams.csv.tmpl")),
    ("people.csv", include_str!("ringi_init/org/people.csv.tmpl")),
    ("members.csv", include_str!("ringi_init/org/members.csv.tmpl")),
];

/// The Go, the README and the rest, with `{{…}}` for what the command line chooses. The project's
/// package's file is named after it (`{{package_file}}`, see [`written`]).
const GO: &[(&str, &str)] = &[
    ("go.mod", include_str!("ringi_init/go.mod.tmpl")),
    ("generate.go", include_str!("ringi_init/generate.go.tmpl")),
    ("{{package_file}}", include_str!("ringi_init/project.go.tmpl")),
    ("gate.go", include_str!("ringi_init/gate.go.tmpl")),
    ("notify.go", include_str!("ringi_init/notify.go.tmpl")),
    ("cmd/server/main.go", include_str!("ringi_init/main.go.tmpl")),
    ("conformance_test.go", include_str!("ringi_init/conformance_test.go.tmpl")),
    ("server_test.go", include_str!("ringi_init/server_test.go.tmpl")),
    ("pages_test.go", include_str!("ringi_init/pages_test.go.tmpl")),
    ("README.md", include_str!("ringi_init/README.md.tmpl")),
    (".gitignore", include_str!("ringi_init/gitignore.tmpl")),
];
const README_JA: (&str, &str) = ("README.ja.md", include_str!("ringi_init/README.ja.md.tmpl"));

/// The Go `--mcp` adds: the agents' tools at `/mcp`, and their test.
const MCP_GO: &[(&str, &str)] = &[("tools.go", include_str!("ringi_init/tools.go.tmpl")), ("tools_test.go", include_str!("ringi_init/tools_test.go.tmpl"))];

/// The Go `--temporal` adds: what starts the flows and makes their workers, the payment's tasks, the
/// worker's command, and the test that runs the payment.
const TEMPORAL_GO: &[(&str, &str)] = &[
    ("flows.go", include_str!("ringi_init/flows.go.tmpl")),
    ("payments.go", include_str!("ringi_init/payments.go.tmpl")),
    ("cmd/worker/main.go", include_str!("ringi_init/worker.go.tmpl")),
    ("flows_test.go", include_str!("ringi_init/flows_test.go.tmpl")),
];

/// `ritsu ringi init …`, on the words after `init`: the exit code. What stops it is said as every
/// command of ritsu says it (DESIGN 6.2, item 10): on the standard error, and on a line that asks for
/// JSON, as `{"error": …}` on the standard output too.
pub fn command(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    ritsu_base::cli::answering(args, out, |out| run(args, out, err))
}

fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let lang = ringi::cli::lang_of(args);
    let table = ringi::cli::table();
    let cmd = table.command("init").expect("init is in ringi's table");
    let a = match table.parse(cmd, args) {
        Ok(a) => a,
        Err(e) => return refuse(err, &e, lang),
    };
    if a.has("--help") {
        let _ = write!(out, "{}", table.help_cmd(cmd, lang));
        return 0;
    }
    match init(&a, &table, lang) {
        Ok(said) => {
            let _ = write!(out, "{}", said.get(lang));
            0
        }
        Err(t) => refuse(err, &t, lang),
    }
}

/// What the command line chose.
struct Chosen {
    module: String,
    package: String,
    catalog: String,
    lang: Lang,
    /// `--mcp`: the agents' tools at `/mcp`.
    mcp: bool,
    /// `--temporal`: a request type with a flow, run on Temporal.
    temporal: bool,
}

fn init(a: &Args, table: &Table, lang: Lang) -> Result<Text, Text> {
    if a.pos.len() > 1 {
        return Err(table.takes_of("init"));
    }
    let given = match a.pos.first() {
        None => ".",
        Some(d) if d.trim_end_matches('/').is_empty() => "/",
        Some(d) => d.trim_end_matches('/'),
    };
    let Some(module) = a.get("--module") else {
        return Err(table.needs_of("init", "--module <path>"));
    };
    if !crate::package::good_module(module) {
        return Err(tr!(
            "`--module {module}` は Go のモジュールのパスにできません。`/` で区切った、英数字と `-._~` だけの部分で書きます（`example.com/acme/approvals`）",
            "`--module {module}` cannot be a Go module's path: write it as parts of letters, digits and `-._~`, between `/` (`example.com/acme/approvals`)"
        ));
    }
    let catalog = a.get("--catalog").map(str::to_string).unwrap_or_else(|| format!("https://{module}/a2ui/catalog.json"));
    if let Some(t) = ritsu_base::modules::manifest::catalog_problem(&catalog) {
        let flag = if a.get("--catalog").is_some() { "--catalog" } else { "--module" };
        return Err(tr!("`{flag}` から作るカタログの ID が使えません: ", "the catalog's ID made from `{flag}` cannot be used: ").then(&t));
    }
    // the project's names, its README and the code made of it are in `--lang`, else English,
    // whatever the environment says (DESIGN 4.1); `lang` is what the run prints
    let chosen = Chosen { module: module.to_string(), package: package_name(module), catalog, lang: Lang::made(a.get("--lang")), mcp: a.has("--mcp"), temporal: a.has("--temporal") };
    let dir = Path::new(given);
    if dir.exists() && !dir.is_dir() {
        return Err(tr!("`{given}` はディレクトリではありません", "`{given}` is not a directory"));
    }
    let mut files = written(&chosen);
    let made = generated(&files, lang)?;
    files.extend(made);
    // every file in the way, before anything is written: anything where a file is to go, and
    // anything but a directory where a directory is to go
    let shown = |p: &str| if given == "." { p.to_string() } else if given == "/" { format!("/{p}") } else { format!("{given}/{p}") };
    let mut in_way = std::collections::BTreeSet::new();
    for (path, _) in &files {
        let parts: Vec<&str> = path.split('/').collect();
        for end in 1..=parts.len() {
            let at = parts[..end].join("/");
            let there = dir.join(&at);
            let file = end == parts.len();
            if (file && std::fs::symlink_metadata(&there).is_ok()) || (!file && there.exists() && !there.is_dir()) {
                in_way.insert(shown(&at));
                break;
            }
        }
    }
    if !in_way.is_empty() {
        let n = in_way.len();
        let list: String = in_way.iter().map(|p| format!("\n  {p}")).collect();
        return Err(tr!(
            "ファイルがもうあります（{n} 個）。`init` はどれも上書きしないので、何も書きませんでした。空のディレクトリか、まだ無いディレクトリを渡してください:{list}",
            "{n} of the files are there already, and `init` overwrites none, so it wrote nothing; give it an empty directory, or one that is not there yet:{list}"
        ));
    }
    // written where a run that cannot write leaves nothing behind (ringi's DESIGN 16.4): the
    // directory made when it is not there, every file written into a place of the run's own inside
    // it, then moved into place once all of them are written; a write or a move that cannot be done
    // takes back what was moved, and the directory the run made is removed with it
    let made_dir = !dir.exists();
    if made_dir && let Err(e) = std::fs::create_dir_all(dir) {
        // the run writes into the directory of the first file first: the error names it, as it did
        let place = files.first().and_then(|(p, _)| p.rsplit_once('/').map(|(d, _)| d)).map_or_else(|| given.to_string(), |d| shown(d));
        return Err(cannot_create(&place, &e));
    }
    let stage = dir.join(format!(".ritsu-init-{}", std::process::id()));
    for (path, text) in &files {
        let to = stage.join(path);
        if let Some(p) = to.parent()
            && let Err(e) = std::fs::create_dir_all(p)
        {
            // the directory the file is to go in: the one given, or one under it
            let place = path.rsplit_once('/').map_or_else(|| given.to_string(), |(d, _)| shown(d));
            let _ = std::fs::remove_dir_all(&stage);
            if made_dir {
                let _ = std::fs::remove_dir(dir);
            }
            return Err(cannot_create(&place, &e));
        }
        if let Err(e) = std::fs::write(&to, text) {
            let _ = std::fs::remove_dir_all(&stage);
            if made_dir {
                let _ = std::fs::remove_dir(dir);
            }
            return Err(cannot_write(&shown(path), &e));
        }
    }
    let mut placed: Vec<PathBuf> = Vec::new();
    let mut made_dirs: Vec<PathBuf> = Vec::new();
    for (path, _) in &files {
        let to = dir.join(path);
        if let Some(p) = to.parent() {
            if p != dir && !p.exists() {
                made_dirs.push(p.to_path_buf());
            }
            if let Err(e) = std::fs::create_dir_all(p) {
                let place = path.rsplit_once('/').map_or_else(|| given.to_string(), |(d, _)| shown(d));
                take_back(&placed, &made_dirs, &stage, dir, made_dir);
                return Err(cannot_create(&place, &e));
            }
        }
        if let Err(e) = std::fs::rename(stage.join(path), &to) {
            take_back(&placed, &made_dirs, &stage, dir, made_dir);
            return Err(cannot_write(&shown(path), &e));
        }
        placed.push(to);
    }
    let _ = std::fs::remove_dir_all(&stage);
    let n = files.len();
    let module = &chosen.module;
    let (cd, place) = if given == "." { (String::new(), tr!("ここ", "here")) } else { (format!("  cd {given}\n"), tr!("{given} ", "in {given}")) };
    let (place_ja, place_en) = (place.get(Lang::Ja), place.get(Lang::En));
    // what the options add to the sentence: the request type with a flow, the agents' tools, the
    // worker, and how to run the server beside Temporal and the worker
    let (types_ja, types_en) = if chosen.temporal { ("申請の種類二つ（一つは flow を持つ）", "two request types, one with a flow, their") } else { ("申請の種類", "a request type, its") };
    let (tools_ja, tools_en) = if chosen.mcp { ("（/mcp にエージェントのツール）", " with the agents' tools at /mcp") } else { ("", "") };
    let (worker_ja, worker_en) = if chosen.temporal { ("、cmd/worker/ に flow のワーカー", ", a worker of the flow in cmd/worker/") } else { ("", "") };
    let server = "  go run ./cmd/server -dev-login\n";
    let (run_ja, run_en) = if chosen.temporal {
        (
            format!("を走らせ、Temporal の開発用サーバーを起動し（temporal server start-dev）、別々の端末で\n  go run ./cmd/worker\n{server}"),
            format!("start Temporal's dev server (temporal server start-dev), run each of\n  go run ./cmd/worker\n{server}in a terminal of its own, "),
        )
    } else {
        (server.to_string(), server.to_string())
    };
    Ok(tr!(
        "{place_ja}に {n} 個のファイルを書きました（モジュール {module}）。project/ に{types_ja}と組織と gate、go/ と requests/ にそこから ritsu が生成したコード、cmd/server/ にフレームワーク {FRAMEWORK} {FRAMEWORK_VERSION} のサーバー{tools_ja}{worker_ja}、それと試験です。次に:\n{cd}  go mod tidy\n  go test ./...\n{run_ja}を走らせ、ブラウザで http://127.0.0.1:8080/pages/ を開きます。続きは README.ja.md にあります\n",
        "wrote {n} files {place_en} (module {module}): {types_en} organization and a gate in project/, the code ritsu generates from them in go/ and requests/, a server of the framework {FRAMEWORK} {FRAMEWORK_VERSION} in cmd/server/{tools_en}{worker_en}, and tests. Next, run\n{cd}  go mod tidy\n  go test ./...\n{run_en}and open http://127.0.0.1:8080/pages/ in a browser. README.md says more\n"
    ))
}

/// Remove the files moved into place (`placed`), the directories the run made for them
/// (`made_dirs`, deepest first), the place the files were staged (`stage`), and the directory the
/// run made (`dir`, when `made_dir`): a run that cannot write leaves nothing behind.
fn take_back(placed: &[PathBuf], made_dirs: &[PathBuf], stage: &Path, dir: &Path, made_dir: bool) {
    for p in placed {
        let _ = std::fs::remove_file(p);
    }
    let mut ds: Vec<&PathBuf> = made_dirs.iter().collect();
    ds.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for d in ds {
        let _ = std::fs::remove_dir(d);
    }
    let _ = std::fs::remove_dir_all(stage);
    if made_dir {
        let _ = std::fs::remove_dir(dir);
    }
}

/// The Go package of the project's root: the last part of its module's path, as a Go identifier
/// (as `ritsu gen` names a package of Go), but never `main`, which a program cannot import, nor a
/// name that ends in `_test`, as Go reads the test files of such a package as the tests of another
/// one (`found packages approvals and approvals_test`): those are `project`.
fn package_name(module: &str) -> String {
    let n = crate::package::go_package_name(module, "project");
    if n == "main" || n.ends_with("_test") { "project".to_string() } else { n }
}

/// Whether Go builds a file named `<stem>.go` on one system only: when its name ends in a system or
/// an architecture Go knows (`go/build`), after a `_` (`approvals_windows.go`, `x_linux_arm64.go`).
fn one_systems(stem: &str) -> bool {
    const SYSTEMS: &[&str] = &[
        "aix", "android", "darwin", "dragonfly", "freebsd", "hurd", "illumos", "ios", "js", "linux", "nacl", "netbsd", "openbsd", "plan9", "solaris", "wasip1", "windows", "zos", "386", "amd64",
        "amd64p32", "arm", "armbe", "arm64", "arm64be", "loong64", "mips", "mipsle", "mips64", "mips64le", "mips64p32", "mips64p32le", "ppc", "ppc64", "ppc64le", "riscv", "riscv64", "s390",
        "s390x", "sparc", "sparc64", "wasm",
    ];
    stem.split_once('_').is_some_and(|(_, after)| after.rsplit('_').next().is_some_and(|last| SYSTEMS.contains(&last)))
}

/// The files `init` writes from its templates, by their paths under the directory.
fn written(c: &Chosen) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let (project, flow) = if c.lang == Lang::Ja { (PROJECT_JA, FLOW_JA) } else { (PROJECT_EN, FLOW_EN) };
    let flow: &[(&str, &str)] = if c.temporal { flow } else { &[] };
    for (path, text) in project.iter().chain(flow) {
        out.push((path.to_string(), text.to_string()));
    }
    for (name, text) in ORG {
        out.push((format!("project/org/{ORG_DAY}/{name}"), text.to_string()));
    }
    let last = c.module.rsplit('/').next().unwrap_or(&c.module);
    let taken = |n: &str| MAIN_NAMES.contains(&n) || (c.temporal && TEMPORAL_NAMES.contains(&n));
    let root = if taken(&c.package) { "project".to_string() } else { c.package.clone() };
    let root_import = if root == last { format!("\"{}\"", c.module) } else { format!("{root} \"{}\"", c.module) };
    let (cedar, cedar_go) = crate::package::go_module(CEDAR_GO).expect("the generated gate's Go module is one ritsu tests");
    let (sdk, sdk_go) = crate::package::go_module(TEMPORAL_SDK).expect("the flows' Go modules are ones ritsu tests");
    let (api, api_go) = crate::package::go_module(TEMPORAL_API).expect("the flows' Go modules are ones ritsu tests");
    let mut asked = vec![FRAMEWORK_GO, cedar_go];
    if c.temporal {
        asked.extend([sdk_go, api_go]);
    }
    let go = crate::package::go_line_of(asked);
    let ja_link = if c.lang == Lang::Ja { "\nJapanese: [README.ja.md](README.ja.md).\n" } else { "" };
    // the tests `go test ./...` runs: three, and one for each option
    let tests = 3 + usize::from(c.mcp) + usize::from(c.temporal);
    let (test_count, test_count_ja) = [("three", "三つ"), ("four", "四つ"), ("five", "五つ")][tests - 3];
    let mut on = Vec::new();
    if c.mcp {
        on.push("mcp");
    }
    if c.temporal {
        on.push("temporal");
    }
    let mut go_files: Vec<(&str, &str)> = GO.to_vec();
    if c.lang == Lang::Ja {
        go_files.push(README_JA);
    }
    if c.mcp {
        go_files.extend(MCP_GO);
    }
    if c.temporal {
        go_files.extend(TEMPORAL_GO);
    }
    // the package's file is named after it, but where another file has that name (a module
    // `…/gate`, or `…/tools` with `--mcp`), or where Go would build it on one system only (a
    // module `…/approvals-windows`): then it is `project.go`
    let named = format!("{}.go", c.package);
    let package_file = if one_systems(&c.package) || go_files.iter().any(|(p, _)| *p == named) { "project.go".to_string() } else { named };
    let vars: Vec<(&str, String)> = vec![
        ("module", c.module.clone()),
        ("package_file", package_file),
        ("package", c.package.clone()),
        ("title", last.to_string()),
        ("root", root),
        ("root_import", root_import),
        ("framework_version", FRAMEWORK_VERSION.to_string()),
        ("framework", FRAMEWORK.to_string()),
        ("cedar_go", cedar.to_string()),
        ("go", go.to_string()),
        ("catalog", c.catalog.clone()),
        ("lang", if c.lang == Lang::Ja { "ja" } else { "en" }.to_string()),
        ("ja_link", ja_link.to_string()),
        ("temporal_sdk", sdk.to_string()),
        ("temporal_api", api.to_string()),
        ("test_count", test_count.to_string()),
        ("test_count_ja", test_count_ja.to_string()),
    ];
    for (path, text) in go_files {
        out.push((fill(path, &vars), fill(&sections(text, &on), &vars)));
    }
    out
}

/// A template with the sections of the options in `on` kept, and those of the others dropped. A
/// line `{{if <option>}}` opens a section, `{{else}}` turns it over, and `{{end}}` closes it;
/// sections nest. The lines of the marks are dropped, so a template with no option on is what it
/// says outside its sections and in their `{{else}}`.
fn sections(text: &str, on: &[&str]) -> String {
    let mut out = String::new();
    // for each open section: whether its lines are kept, and whether those of the one around it are
    let mut open: Vec<(bool, bool)> = Vec::new();
    let kept = |open: &[(bool, bool)]| open.last().is_none_or(|&(this, around)| this && around);
    for line in text.split_inclusive('\n') {
        let mark = line.trim_end_matches('\n');
        if let Some(option) = mark.strip_prefix("{{if ").and_then(|m| m.strip_suffix("}}")) {
            let around = kept(&open);
            open.push((on.contains(&option), around));
        } else if mark == "{{else}}" {
            let top = open.last_mut().expect("a template's {{else}} is inside an {{if}}");
            top.0 = !top.0;
        } else if mark == "{{end}}" {
            open.pop().expect("a template's {{end}} closes an {{if}}");
        } else if kept(&open) {
            out.push_str(line);
        }
    }
    debug_assert!(open.is_empty(), "a template's {{if}} is not closed: {text}");
    out
}

/// A template with each `{{name}}` filled in.
fn fill(text: &str, vars: &[(&str, String)]) -> String {
    let mut s = text.to_string();
    for (name, value) in vars {
        s = s.replace(&format!("{{{{{name}}}}}"), value);
    }
    debug_assert!(!s.contains("{{"), "a template's name is not filled in: {s}");
    s
}

/// What `ritsu gen` makes of the project's files (`files`, as [`written`] makes them): the
/// `//go:generate` line of their `generate.go`, run on a copy of `project/` in a directory of its own
/// under the system's temporary one, read back, and the directory removed. The line's paths count
/// from the project's root, and here from that directory. What `init` writes is so what the line
/// writes, whatever it says. Each file by its path under the project's root.
fn generated(files: &[(String, String)], lang: Lang) -> Result<Vec<(String, String)>, Text> {
    let tmp = Scratch::new()?;
    for (path, text) in files.iter().filter(|(p, _)| p.starts_with("project/")) {
        let to = tmp.0.join(path);
        if let Some(p) = to.parent()
            && let Err(e) = std::fs::create_dir_all(p)
        {
            return Err(cannot_create(&p.display().to_string(), &e));
        }
        if let Err(e) = std::fs::write(&to, text) {
            return Err(cannot_write(&to.display().to_string(), &e));
        }
    }
    let line = files.iter().find(|(p, _)| p == "generate.go").and_then(|(_, t)| t.lines().find_map(|l| l.strip_prefix("//go:generate ritsu gen "))).expect("generate.go runs ritsu gen");
    let words: Vec<&str> = line.split_whitespace().collect();
    let at = |w: &str| if w == "." { tmp.0.to_string_lossy().into_owned() } else { tmp.0.join(w).to_string_lossy().into_owned() };
    let mut args = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        let value = || words.get(i + 1).copied().expect("each flag of the line takes a value");
        match w {
            // the line's paths
            "--root" | "--out" => args.extend([w.to_string(), at(value())]),
            // its other flags, whose values are no paths
            f if f.starts_with("--") => args.extend([f.to_string(), value().to_string()]),
            p => {
                args.push(at(p));
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    crate::package::quietly(&args, lang)?;
    let mut out = Vec::new();
    read_tree(&tmp.0, Path::new(""), &mut out)?;
    out.retain(|(p, _)| !p.starts_with("project/"));
    if out.is_empty() {
        return Err(tr!("`ritsu gen` が何も書きませんでした", "`ritsu gen` wrote nothing"));
    }
    Ok(out)
}

/// Every file under `root/rel`, by its path from `root`, in the order of the paths.
fn read_tree(root: &Path, rel: &Path, out: &mut Vec<(String, String)>) -> Result<(), Text> {
    let at = root.join(rel);
    let cannot = |p: &Path, e: std::io::Error| cannot_read(&p.display().to_string(), &e);
    let mut names: Vec<String> = std::fs::read_dir(&at).map_err(|e| cannot(&at, e))?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for n in names {
        let r = rel.join(&n);
        if root.join(&r).is_dir() {
            read_tree(root, &r, out)?;
        } else {
            let p = root.join(&r);
            let text = std::fs::read_to_string(&p).map_err(|e| cannot(&p, e))?;
            out.push((r.to_string_lossy().replace('\\', "/"), text));
        }
    }
    Ok(())
}

/// A directory of the run's own under the system's temporary one, removed when it is dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch, Text> {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("ritsu-ringi-init-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| cannot_create(&dir.display().to_string(), &e))?;
        Ok(Scratch(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
