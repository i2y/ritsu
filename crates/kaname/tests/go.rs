//! The Go kaname generates (DESIGN 10, 16.4), held to the reference interpreter. Every module of the
//! examples and of `tests/modules/` goes into one package, beside the Go rulec, koyomi and
//! chobo write for the rules, the dates and the books they read (each language's own generator,
//! called as `ritsu gen` calls it), with the messages in English and in Japanese, in a module of
//! `tools/go`'s go.mod and go.sum (pgx, which the books' clients on PostgreSQL import):
//!
//! - `gofmt -l` lists none of kaname's files, and `go vet ./...` passes the package and the harness;
//! - the harness, built by go, runs every vector of every function, and the inputs outside what the
//!   parameters declare, and every answer is the reference interpreter's: the value, or the kind, the
//!   module, the line and the message of the failure (a vector that does not stop in the interpreter,
//!   past its 200 calls, is not run).
//!
//! Without go or gofmt, the test says SKIP and passes.

mod common;

use common::targets::*;
use kaname::r#gen::Target;
use ritsu_base::text::Lang;
use ritsu_testkit::{Need, TempDir};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(900);

/// `go`, run in `dir` outside any workspace, with -trimpath (Go's build cache keys a package by its
/// directory otherwise, and grows by the same packages at every run).
fn go_in(dir: &Path) -> Command {
    let mut c = Command::new("go");
    c.current_dir(dir).env("GOWORK", "off").env("GOFLAGS", "-trimpath");
    c
}

#[test]
fn the_generated_go_answers_as_the_reference_does() {
    if !ritsu_testkit::need(Need::Go) {
        return;
    }
    if !ritsu_testkit::tools::runs("go", &["version"]) || ritsu_testkit::tools::on_path("gofmt").is_none() {
        ritsu_testkit::skip("go or gofmt is not on the PATH; the Go kaname generates is not run");
        return;
    }
    let t = TempDir::new("kaname-go");
    let root = keep_in().unwrap_or_else(|| t.path().to_path_buf());
    let report = deep(move || {
        let modules = modules();
        let checked: Vec<_> = modules.iter().map(|(c, _)| c.clone()).collect();
        let mut out = Vec::new();
        for lang in [Lang::En, Lang::Ja] {
            let dir = root.join(format!("go-{}", if lang == Lang::Ja { "ja" } else { "en" }));
            write_package(&dir, Target::Go, lang, &modules);
            for f in ["go.mod", "go.sum"] {
                std::fs::copy(tools().join("go").join(f), dir.join(f)).unwrap();
            }
            std::fs::create_dir_all(dir.join("harness")).unwrap();
            std::fs::write(dir.join("harness/main.go"), kaname::r#gen::harness::program(&checked, Target::Go, PY_NAME, GO_PACKAGE)).unwrap();
            let (cases, diverged, outside) = cases(&modules, lang);
            std::fs::write(dir.join("cases.json"), cases_json(&cases)).unwrap();
            let r = ritsu_testkit::run(Command::new("gofmt").arg("-l").arg(Path::new(PY_NAME).join("glue")).arg("harness").current_dir(&dir), LIMIT);
            assert!(r.ok && r.stdout.trim().is_empty(), "gofmt -l lists files of kaname it would change ({lang:?}):\n{}", r.both());
            let r = ritsu_testkit::run(go_in(&dir).args(["vet", "./..."]), LIMIT);
            assert!(r.ok, "go vet finds fault with the Go kaname generates ({lang:?}):\n{}", r.both());
            let bin = dir.join("harness.bin");
            let r = ritsu_testkit::run(go_in(&dir).args(["build", "-o"]).arg(&bin).arg("./harness"), LIMIT);
            assert!(r.ok, "go build failed ({lang:?}):\n{}", r.both());
            let r = ritsu_testkit::run(Command::new(&bin).arg(dir.join("cases.json")).current_dir(&dir), LIMIT);
            assert!(r.ok, "the harness failed ({lang:?}):\n{}", r.both());
            let (agree, wrong) = compare(&cases, &r.stdout);
            assert!(wrong.is_empty(), "the Go ({lang:?}) answers otherwise than the reference:\n{}", wrong.join("\n"));
            out.push(format!("{lang:?}: {agree} of {} cases agree with the reference: {} vectors ({diverged} that do not stop are not run) and {outside} inputs outside what the parameters declare, in {} modules", cases.len() - diverged, cases.len() - diverged - outside, modules.len()));
            let _ = std::fs::remove_file(&bin);
            if lang == Lang::En {
                // the go that go.mod asks for, as it runs in the module
                let r = ritsu_testkit::run(go_in(&dir).arg("version"), LIMIT);
                out.insert(0, format!("gofmt -l and go vet ({}) pass the package in English and in Japanese", r.stdout.trim()));
            }
        }
        out
    });
    for line in report {
        println!("{line}");
    }
}
