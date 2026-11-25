//! `ritsu ringi init` (ringi's DESIGN 16.4): the first files of a Go project that serves requests on
//! the framework. Every file it writes from its templates, in English and with `--lang ja`, with no
//! option, `--mcp`, `--temporal` and both, is held to a golden file
//! (`tests/golden/ringi-init/<lang>[-mcp][-temporal].txt`; `RITSU_BLESS=1` writes them); the code it
//! generates is `ritsu gen`'s, whose own tests hold it, and here it is what the `//go:generate` line
//! of the project writes (the line with `--check` finds nothing to change). `ritsu check` passes the
//! project with no warning. What it writes does not follow the environment's languages. What stops
//! `init` is said in both languages, in the words every command of ritsu says it in, and nothing is
//! written: no `--module`, a path no Go module has, a value given to an option, files in the way
//! (each named), a place that cannot be written; a line that asks for JSON has it in JSON too. The
//! Go is built and tested by the framework's own test, which runs this command with each set of
//! options: this tree has no Go and no framework to build it with.

mod common;

use ritsu_testkit::TempDir;
use std::path::Path;
use std::process::Command;

/// The usage line of `ritsu ringi init`, named as it is run.
const USAGE: &str = "ritsu ringi init [<dir>] --module <path> [--catalog <url>] [--mcp] [--temporal] [--lang ja|en]";

fn ritsu(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in common::LANG_VARS {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Every file under `dir`, by its path from it, in the order of the paths.
fn files(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, rel: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(root.join(rel)).unwrap() {
            let r = rel.join(e.unwrap().file_name());
            if root.join(&r).is_dir() {
                walk(root, &r, out);
            } else {
                out.push(r.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, Path::new(""), &mut out);
    out.sort();
    out
}

fn generated(path: &str) -> bool {
    path.starts_with("go/") || path.starts_with("requests/")
}

/// `ritsu ringi init approvals --module example.com/acme/approvals --lang <lang>` with `options`, in
/// a new directory: the directory, and the project in it. What it says names what it wrote, and the
/// commands to run next: with `--temporal`, the worker's beside the server's.
fn init(lang: &str, options: &[&str]) -> (TempDir, std::path::PathBuf) {
    let t = TempDir::new(&format!("ringi-init-{lang}{}", options.concat()));
    let mut args = vec!["ringi", "init", "approvals", "--module", "example.com/acme/approvals", "--lang", lang];
    args.extend(options);
    let (code, out, err) = ritsu(t.path(), &args);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let project = t.path().join("approvals");
    let n = files(&project).len();
    let said = if lang == "ja" { format!("approvals に {n} 個のファイルを書きました（モジュール example.com/acme/approvals）\n  project/ ") } else { format!("wrote {n} files in approvals (module example.com/acme/approvals)\n  project/ ") };
    assert!(out.starts_with(&said), "{out}");
    let temporal = options.contains(&"--temporal");
    let mcp = options.contains(&"--mcp");
    let next = match (temporal, lang) {
        (false, _) => "  cd approvals\n  go mod tidy\n  go test ./...\n  go run ./cmd/server -dev-login\n".to_string(),
        (true, "ja") => "  cd approvals\n  go mod tidy\n  go test ./...\n次の三つは、別々の端末で起動します:\n  temporal server start-dev\n  go run ./cmd/worker\n  go run ./cmd/server -dev-login\n起動したら、ブラウザで ".to_string(),
        (true, _) => "  cd approvals\n  go mod tidy\n  go test ./...\nStart these three, each in a terminal of its own:\n  temporal server start-dev\n  go run ./cmd/worker\n  go run ./cmd/server -dev-login\nThen open ".to_string(),
    };
    assert!(out.contains(&next), "{out}");
    let (tools, worker) = ("\n  tools.go ", "\n  cmd/worker/ ");
    assert_eq!((out.contains(tools), out.contains(worker)), (mcp, temporal), "{out}");
    (t, project)
}

/// The `//go:generate` line of `generate.go`, as words, ritsu's name taken off its head.
fn generate_line(project: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(project.join("generate.go")).unwrap();
    let line = text.lines().find_map(|l| l.strip_prefix("//go:generate ")).expect("generate.go has a //go:generate line");
    let mut words: Vec<String> = line.split_whitespace().map(str::to_string).collect();
    assert_eq!(words[0], "ritsu", "{line}");
    words.remove(0);
    words
}

/// The files each option adds: `--mcp`'s, and `--temporal`'s (from its templates, and some of the
/// code generated from the request type and the flow it adds).
const MCP_FILES: &[&str] = &["tools.go", "tools_test.go"];
const TEMPORAL_FILES: &[&str] = &[
    "project/expense.ringi",
    "project/shared/pay.flow",
    "flows.go",
    "payments.go",
    "cmd/worker/main.go",
    "flows_test.go",
    "go/flows/expense_payment/worker.go",
    "requests/types/expense.json",
    "requests/vectors/expense.jsonl",
];

/// The project `init` writes in `lang` with `options`: every file from its templates held to the
/// golden file of the language and the options; the files of an option there when it is given and
/// only then; the code it generates is what the `//go:generate` line writes, which with `--check`
/// finds nothing to change (a line that wrote elsewhere, or other code, would find files missing or
/// not the same); and `ritsu check` passes the project with no warning.
fn the_first_files(lang: &str, options: &[&str]) {
    let (_t, project) = init(lang, options);
    let all = files(&project);
    let mut text = String::new();
    for f in all.iter().filter(|f| !generated(f)) {
        text.push_str(&format!("==> {f} <==\n{}", std::fs::read_to_string(project.join(f)).unwrap()));
    }
    let golden: String = options.iter().map(|o| format!("-{}", o.trim_start_matches('-'))).collect();
    ritsu_testkit::golden(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/ringi-init/{lang}{golden}.txt")), &text);
    for f in ["go/ticketkinds/ticketkinds.go", "go/authz/company/company.go", "requests/openapi.json", "requests/types/purchase.json", "requests/vectors/purchase.jsonl", "requests/mcp/tools.json", "README.md"] {
        assert!(all.iter().any(|a| a == f), "{lang}: no {f} in {all:?}");
    }
    assert_eq!(all.iter().any(|a| a == "README.ja.md"), lang == "ja", "{all:?}");
    for (option, added) in [("--mcp", MCP_FILES), ("--temporal", TEMPORAL_FILES)] {
        for f in added {
            assert_eq!(all.iter().any(|a| a == f), options.contains(&option), "{lang} {options:?}: {f} in {all:?}");
        }
    }
    let mut args = generate_line(&project);
    assert!(args.windows(2).any(|w| w[0] == "--lang" && w[1] == lang), "{args:?}");
    args.push("--check".to_string());
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, out, err) = ritsu(&project, &args);
    assert_eq!((code, err.as_str()), (0, ""), "{lang}: `ritsu {}`: {out}", args.join(" "));
    let (code, out, err) = ritsu(&project, &["check", "project", "--lang", lang]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(!out.contains("warning[") && !out.contains("error["), "{out}");
    let all_pass = match (options.contains(&"--temporal"), lang) {
        (false, "ja") => "ritsu check: ファイル 4 個（koyomi 1、sekisho 1、ringi 2）。どれも検査を通りました。",
        (false, _) => "ritsu check: 4 files (koyomi 1, sekisho 1, ringi 2): all pass;",
        (true, "ja") => "ritsu check: ファイル 6 個（koyomi 1、dandori 1、sekisho 1、ringi 3）。どれも検査を通りました。",
        (true, _) => "ritsu check: 6 files (koyomi 1, dandori 1, sekisho 1, ringi 3): all pass;",
    };
    assert!(out.contains(all_pass), "{out}");
}

#[test]
fn the_first_files_in_english() {
    the_first_files("en", &[]);
}

#[test]
fn the_first_files_in_japanese() {
    the_first_files("ja", &[]);
}

#[test]
fn the_agents_tools_in_english() {
    the_first_files("en", &["--mcp"]);
}

#[test]
fn the_agents_tools_in_japanese() {
    the_first_files("ja", &["--mcp"]);
}

#[test]
fn a_flow_on_temporal_in_english() {
    the_first_files("en", &["--temporal"]);
}

#[test]
fn a_flow_on_temporal_in_japanese() {
    the_first_files("ja", &["--temporal"]);
}

#[test]
fn the_agents_tools_and_a_flow_in_english() {
    the_first_files("en", &["--mcp", "--temporal"]);
}

#[test]
fn the_agents_tools_and_a_flow_in_japanese() {
    the_first_files("ja", &["--mcp", "--temporal"]);
}

/// What `init` writes is in `--lang`, else English, whatever the environment's languages say: the
/// project with both options, written with every `*_LANG` variable `ja`, is the one written with
/// none, byte for byte. What it prints follows them.
#[test]
fn what_init_writes_does_not_follow_the_environment() {
    let written = |vars: bool| {
        let t = TempDir::new(&format!("ringi-init-vars-{vars}"));
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(t.path()).args(["ringi", "init", "approvals", "--module", "example.com/acme/approvals", "--mcp", "--temporal"]);
        for v in common::LANG_VARS {
            if vars {
                c.env(v, "ja");
            } else {
                c.env_remove(v);
            }
        }
        let o = c.output().expect("could not run ritsu");
        assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
        let project = t.path().join("approvals");
        let all: Vec<(String, Vec<u8>)> = files(&project).into_iter().map(|f| (f.clone(), std::fs::read(project.join(&f)).unwrap())).collect();
        (String::from_utf8_lossy(&o.stdout).into_owned(), all)
    };
    let (plain_said, plain) = written(false);
    let (vars_said, vars) = written(true);
    assert!(plain_said.starts_with("wrote ") && vars_said.contains("個のファイルを書きました"), "{plain_said}\n{vars_said}");
    assert_eq!(plain.iter().map(|(f, _)| f).collect::<Vec<_>>(), vars.iter().map(|(f, _)| f).collect::<Vec<_>>());
    for ((f, a), (_, b)) in plain.iter().zip(&vars) {
        assert!(a == b, "{f} follows the environment");
    }
}

#[test]
fn help() {
    let t = TempDir::new("ringi-init-help");
    let (code, out, _) = ritsu(t.path(), &["ringi", "init", "--help"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("ritsu ringi init — write the first files of a Go project") && out.contains(&format!("  {USAGE}\n")), "{out}");
    assert!(out.contains("serve the agents' tools over MCP at `/mcp` of the server") && out.contains("add a request type whose approval starts a flow"), "{out}");
    let (code, out, _) = ritsu(t.path(), &["ringi", "help", "init", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("ritsu ringi init — 申請と承認のサーバーを始める Go のプロジェクトの、最初のファイルを書く。") && out.contains(&format!("  {USAGE}\n")), "{out}");
    assert!(out.contains("サーバーの `/mcp` で、エージェントのツールを MCP で出す") && out.contains("承認されると flow で払い戻す経費の精算を、申請の種類として足し"), "{out}");
    assert!(files(t.path()).is_empty());
}

#[test]
fn what_stops_init_writes_nothing() {
    let t = TempDir::new("ringi-init-stops");
    let stops = |args: &[&str], en: &str, ja: &str| {
        for (lang, want) in [("en", en), ("ja", ja)] {
            let mut args = args.to_vec();
            args.extend(["--lang", lang]);
            let (code, out, err) = ritsu(t.path(), &args);
            assert_eq!((code, out.as_str()), (2, ""), "{args:?}: {err}");
            assert!(err.starts_with(want), "{args:?}: {err}");
        }
    };
    // what every command of ritsu says (ritsu's DESIGN 6.2, item 10), with the usage line
    stops(
        &["ringi", "init", "approvals"],
        &format!("error: `ritsu ringi init` needs `--module <path>`\nusage: {USAGE}\n"),
        &format!("エラー: `ritsu ringi init` には `--module <path>` が要ります\n使い方: {USAGE}\n"),
    );
    stops(
        &["ringi", "init", "approvals", "--module", "example.com/acme approvals"],
        "error: `--module example.com/acme approvals` cannot be a Go module's path: ",
        "エラー: `--module example.com/acme approvals` は Go のモジュールのパスにできません。",
    );
    stops(
        &["ringi", "init", "a", "b", "--module", "example.com/acme/approvals"],
        &format!("error: `ritsu ringi init` takes [<dir>]\nusage: {USAGE}\n"),
        &format!("エラー: `ritsu ringi init` に渡すのは [<dir>] です\n使い方: {USAGE}\n"),
    );
    stops(
        &["ringi", "init", "approvals", "--module", "example.com/acme/approvals", "--catalog", "ftp://example.com/catalog.json"],
        "error: the catalog's ID made from `--catalog` cannot be used: ",
        "エラー: `--catalog` から作るカタログの ID が使えません: ",
    );
    // the options take no value, and each is given once
    stops(
        &["ringi", "init", "approvals", "--module", "example.com/acme/approvals", "--mcp=yes"],
        "error: `--mcp` takes no value (it was given `=yes`)\n",
        "エラー: `--mcp` は値を取りません（`=yes` が付いています）\n",
    );
    stops(&["ringi", "init", "approvals", "--module", "example.com/acme/approvals", "--temporal", "--temporal"], "error: `--temporal` is given twice\n", "エラー: `--temporal` が二度書かれています\n");
    // a file where the directory is to be, or where one of its directories is to be
    std::fs::write(t.path().join("a-file"), "mine").unwrap();
    stops(&["ringi", "init", "a-file", "--module", "example.com/acme/approvals"], "error: `a-file` is not a directory\n", "エラー: `a-file` はディレクトリではありません\n");
    stops(
        &["ringi", "init", "a-file/approvals", "--module", "example.com/acme/approvals"],
        "error: cannot create `a-file/approvals/project`: ",
        "エラー: `a-file/approvals/project` を作れません: ",
    );
    // a line that asks for JSON is answered in JSON too, though `init` takes no `--format`
    let (code, out, err) = ritsu(t.path(), &["ringi", "init", "approvals", "--module", "example.com/acme/approvals", "--format", "json"]);
    let said = "unknown flag `--format`; run `ritsu ringi init --help`";
    assert_eq!((code, out, err), (2, format!("{{\"error\":{{\"code\":2,\"message\":\"{said}\"}}}}\n"), format!("error: {said}\n")));
    assert_eq!(files(t.path()), vec!["a-file".to_string()]);
}

#[test]
fn files_in_the_way_are_each_named_and_none_is_overwritten() {
    let t = TempDir::new("ringi-init-in-the-way");
    std::fs::create_dir_all(t.path().join("approvals/cmd")).unwrap();
    std::fs::write(t.path().join("approvals/go.mod"), "module mine\n").unwrap();
    std::fs::write(t.path().join("approvals/project"), "mine").unwrap();
    std::fs::write(t.path().join("approvals/cmd/server"), "mine").unwrap();
    std::fs::write(t.path().join("approvals/notes.txt"), "mine").unwrap();
    let before = files(t.path());
    for (lang, want) in [
        ("en", "error: 3 of the files are there already, and `init` overwrites none, so it wrote nothing; give it an empty directory, or one that is not there yet:\n  approvals/cmd/server\n  approvals/go.mod\n  approvals/project\n"),
        ("ja", "エラー: ファイルがもうあります（3 個）。`init` はどれも上書きしないので、何も書きませんでした。空のディレクトリか、まだ無いディレクトリを渡してください:\n  approvals/cmd/server\n  approvals/go.mod\n  approvals/project\n"),
    ] {
        let (code, out, err) = ritsu(t.path(), &["ringi", "init", "approvals", "--module", "example.com/acme/approvals", "--lang", lang]);
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", want));
        assert_eq!(files(t.path()), before);
        assert_eq!(std::fs::read_to_string(t.path().join("approvals/go.mod")).unwrap(), "module mine\n");
    }
    // the files an option adds are in the way as the others are
    std::fs::create_dir_all(t.path().join("flows/cmd")).unwrap();
    std::fs::write(t.path().join("flows/cmd/worker"), "mine").unwrap();
    std::fs::write(t.path().join("flows/tools.go"), "mine").unwrap();
    let before = files(t.path());
    for (lang, want) in [
        ("en", "error: 2 of the files are there already, and `init` overwrites none, so it wrote nothing; give it an empty directory, or one that is not there yet:\n  flows/cmd/worker\n  flows/tools.go\n"),
        ("ja", "エラー: ファイルがもうあります（2 個）。`init` はどれも上書きしないので、何も書きませんでした。空のディレクトリか、まだ無いディレクトリを渡してください:\n  flows/cmd/worker\n  flows/tools.go\n"),
    ] {
        let (code, out, err) = ritsu(t.path(), &["ringi", "init", "flows", "--module", "example.com/acme/flows", "--mcp", "--temporal", "--lang", lang]);
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", want));
        assert_eq!(files(t.path()), before);
    }
    // without the options, those files are not in the way
    let (code, out, err) = ritsu(t.path(), &["ringi", "init", "flows", "--module", "example.com/acme/flows"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert_eq!(std::fs::read_to_string(t.path().join("flows/tools.go")).unwrap(), "mine");
    // a project written once is not written again: every file of it is named
    let (_u, project) = init("en", &[]);
    let n = files(&project).len();
    std::fs::write(project.join("README.md"), "mine").unwrap();
    let (code, _, err) = ritsu(project.parent().unwrap(), &["ringi", "init", "approvals", "--module", "example.com/acme/approvals"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.starts_with(&format!("error: {n} of the files are there already")) && err.contains("\n  approvals/README.md\n") && err.lines().count() == n + 1, "{err}");
    assert_eq!(std::fs::read_to_string(project.join("README.md")).unwrap(), "mine");
}

#[test]
fn init_writes_in_the_current_directory_and_names_the_package_after_the_module() {
    let t = TempDir::new("ringi-init-here");
    let (code, out, err) = ritsu(t.path(), &["ringi", "init", "--module", "example.com/acme/my-approvals"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(out.contains(" files here (module example.com/acme/my-approvals)") && !out.contains("  cd "), "{out}");
    // the package is the last part of the module's path as a Go identifier, and the server imports
    // it by that name
    let root = std::fs::read_to_string(t.path().join("my_approvals.go")).unwrap();
    assert!(root.contains("\npackage my_approvals\n"), "{root}");
    let main = std::fs::read_to_string(t.path().join("cmd/server/main.go")).unwrap();
    assert!(main.contains("\tmy_approvals \"example.com/acme/my-approvals\"\n") && main.contains("my_approvals.Parts(log)"), "{main}");
    // a package named as one of the server's own names is imported under another
    let u = TempDir::new("ringi-init-serve");
    let (code, out, err) = ritsu(u.path(), &["ringi", "init", "--module", "example.com/acme/serve"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let main = std::fs::read_to_string(u.path().join("cmd/server/main.go")).unwrap();
    assert!(main.contains("\tproject \"example.com/acme/serve\"\n") && main.contains("project.Parts(log)"), "{main}");
    // with --temporal, the commands have names of their own to keep: a package named `flows` is
    // imported as `project` by both, and is `flows` without the option; and as the option writes a
    // `flows.go` of its own, the package's file is `project.go`, which the README names
    let v = TempDir::new("ringi-init-flows");
    let (code, out, err) = ritsu(v.path(), &["ringi", "init", "--module", "example.com/acme/flows", "--temporal"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let root = std::fs::read_to_string(v.path().join("project.go")).unwrap();
    assert!(root.contains("\npackage flows\n") && root.contains("\nfunc Parts("), "{root}");
    let flows = std::fs::read_to_string(v.path().join("flows.go")).unwrap();
    assert!(flows.starts_with("package flows\n") && flows.contains("\nfunc Workers("), "{flows}");
    assert!(std::fs::read_to_string(v.path().join("README.md")).unwrap().contains("| `project.go` | What the project gives the server"));
    let main = std::fs::read_to_string(v.path().join("cmd/server/main.go")).unwrap();
    assert!(main.contains("\tproject \"example.com/acme/flows\"\n") && main.contains("project.Parts(log)") && main.contains("project.Flows(c, cfg.Packages...)"), "{main}");
    let worker = std::fs::read_to_string(v.path().join("cmd/worker/main.go")).unwrap();
    assert!(worker.contains("\tproject \"example.com/acme/flows\"\n") && worker.contains("project.Workers(c, log)"), "{worker}");
    let w = TempDir::new("ringi-init-flows-plain");
    let (code, out, err) = ritsu(w.path(), &["ringi", "init", "--module", "example.com/acme/flows"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let main = std::fs::read_to_string(w.path().join("cmd/server/main.go")).unwrap();
    assert!(main.contains("\t\"example.com/acme/flows\"\n") && main.contains("flows.Parts(log)"), "{main}");
    assert!(std::fs::read_to_string(w.path().join("flows.go")).unwrap().contains("\nfunc Parts("));
}

/// The package's file is `project.go` where its own name is taken or Go reads it apart: a package
/// named as a file `init` writes, and one whose name ends in a system Go knows, which Go would build
/// on that system only. A name that ends in `_test` names no package, as Go reads the test files of
/// such a package as the tests of another: the package is then `project`.
#[test]
fn the_package_and_its_file_are_named_as_go_reads_them() {
    // the gate's glue keeps `gate.go`
    let g = TempDir::new("ringi-init-gate");
    let (code, out, err) = ritsu(g.path(), &["ringi", "init", "--module", "example.com/acme/gate"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let root = std::fs::read_to_string(g.path().join("project.go")).unwrap();
    assert!(root.contains("\npackage gate\n") && root.contains("\nfunc Parts("), "{root}");
    let glue = std::fs::read_to_string(g.path().join("gate.go")).unwrap();
    assert!(glue.starts_with("package gate\n") && glue.contains("func (g *Gate) Allow("), "{glue}");
    // `approvals_windows.go` would be built on Windows only
    let s = TempDir::new("ringi-init-windows");
    let (code, out, err) = ritsu(s.path(), &["ringi", "init", "--module", "example.com/acme/approvals-windows"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(!s.path().join("approvals_windows.go").exists());
    let root = std::fs::read_to_string(s.path().join("project.go")).unwrap();
    assert!(root.contains("\npackage approvals_windows\n") && root.contains("\nfunc Parts("), "{root}");
    assert!(std::fs::read_to_string(s.path().join("README.md")).unwrap().contains("| `project.go` | What the project gives the server"));
    // a package `approvals_test` would not build with its tests
    let x = TempDir::new("ringi-init-test");
    let (code, out, err) = ritsu(x.path(), &["ringi", "init", "--module", "example.com/acme/approvals-test"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let root = std::fs::read_to_string(x.path().join("project.go")).unwrap();
    assert!(root.contains("\npackage project\n") && root.contains("\nfunc Parts("), "{root}");
    assert!(std::fs::read_to_string(x.path().join("server_test.go")).unwrap().starts_with("package project\n"));
    let main = std::fs::read_to_string(x.path().join("cmd/server/main.go")).unwrap();
    assert!(main.contains("\tproject \"example.com/acme/approvals-test\"\n") && main.contains("project.Parts(log)"), "{main}");
}
