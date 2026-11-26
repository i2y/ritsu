//! The depth of the JSON of every argument is read before the type of any of them (DESIGN 16.4):
//! an entry whose first argument is of another type and whose last nests deeper than the entries
//! read answers the depth, as Rust's entries and the door do.

mod common;

use common::targets::*;
use kaname::check::Checked;
use kaname::r#gen::Target;
use kaname::r#gen::harness::{JsonEntry, json_entries};
use ritsu_base::text::Lang;
use ritsu_testkit::{Need, TempDir};
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(120);

/// A function whose first argument is a record and whose second is a string: `[]` is no record, so
/// an entry that reads types first answers `submit`; the deep JSON of `organization` must be seen
/// first.
const SOURCE: &str = "module deepentry v1

record Form
  requester : string

fn probe(submit: Form, organization: string) -> number total
  1
";

fn checked() -> Rc<Checked> {
    let o = common::check_text("deepentry.kn", SOURCE);
    o.analysed.clone().unwrap_or_else(|| panic!("{}", common::shown(&o, Lang::En)))
}

fn entries_file(c: &Rc<Checked>, t: Target, head: &str, tail: &str) -> String {
    let modules = vec![c.clone()];
    json_entries(&modules, t, PY_NAME, GO_PACKAGE, &[JsonEntry { module: 0, function: "probe".into(), name: "Probe".into() }], head, tail, Lang::En)
}

fn write_case(root: &Path, t: Target) -> Rc<Checked> {
    let c = checked();
    let modules = vec![(c.clone(), "deepentry.kn".to_string())];
    write_package(root, t, Lang::En, &modules);
    c
}

/// Whether the answer is the depth of `organization`, never the type of `submit`.
fn expect_deep(out: &str, target: &str) {
    assert!(out.starts_with("error\torganization\t"), "{target}: {out}");
    assert!(out.contains("nests deeper than 512"), "{target}: {out}");
    assert!(!out.contains("`submit`"), "{target}: {out}");
}

#[test]
fn python_reads_the_depth_of_every_argument_first() {
    if !ritsu_testkit::need(Need::Python) {
        return;
    }
    if !ritsu_testkit::tools::runs("python3", &["--version"]) {
        ritsu_testkit::skip("python3 is not on the PATH; the Python entries are not run");
        return;
    }
    let t = TempDir::new("kaname-depth-python");
    let root = t.path().join("py");
    let c = write_case(&root, Target::Python);
    std::fs::write(root.join(PY_NAME).join("entries.py"), entries_file(&c, Target::Python, "", "")).unwrap();
    std::fs::write(
        root.join("runner.py"),
        r#"import sys

from pkg.entries import Probe
from pkg.glue import kaname

deep = "[" * 513 + "0" + "]" * 513
try:
    print("out\t" + Probe("[]", deep))
except kaname.KanameError as e:
    print("error\t" + str(e.path) + "\t" + e.message)
"#,
    )
    .unwrap();
    let r = ritsu_testkit::run(Command::new("python3").arg("runner.py").current_dir(&root), LIMIT);
    assert!(r.ok, "python3 failed:\n{}", r.both());
    expect_deep(r.stdout.trim(), "Python");
}

#[test]
fn typescript_reads_the_depth_of_every_argument_first() {
    if !ritsu_testkit::need(Need::Node) {
        return;
    }
    let tsc = tools().join("node_modules/.bin/tsc");
    if !tsc.is_file() || !ritsu_testkit::tools::runs("node", &["--version"]) {
        ritsu_testkit::skip("node or tsc is not installed; the TypeScript entries are not run");
        return;
    }
    let t = TempDir::new("kaname-depth-typescript");
    let root = t.path().join("ts");
    let c = write_case(&root, Target::TypeScript);
    std::fs::write(root.join("entries.ts"), entries_file(&c, Target::TypeScript, "", "export { Probe };\n")).unwrap();
    let types = tools().join("node_modules/@types");
    std::fs::write(
        root.join("tsconfig.json"),
        format!(
            "{{\n  \"compilerOptions\": {{\n    \"strict\": true,\n    \"erasableSyntaxOnly\": true,\n    \"module\": \"commonjs\",\n    \"target\": \"es2022\",\n    \"rootDir\": \".\",\n    \"outDir\": \"dist\",\n    \"types\": [\"node\"],\n    \"typeRoots\": [{}],\n    \"skipLibCheck\": true\n  }},\n  \"include\": [\"**/*.ts\"],\n  \"exclude\": [\"dist\", \"node_modules\"]\n}}\n",
            ritsu_base::json::quote(&types.to_string_lossy())
        ),
    )
    .unwrap();
    let r = ritsu_testkit::run(Command::new(&tsc).arg("-p").arg(root.join("tsconfig.json")).current_dir(&root), LIMIT);
    assert!(r.ok, "tsc failed:\n{}", r.both());
    std::fs::write(
        root.join("runner.js"),
        r#"const entries = require("./dist/entries.js");
const deep = "[".repeat(513) + "0" + "]".repeat(513);
try {
  console.log("out\t" + entries.Probe("[]", deep));
} catch (e) {
  console.log("error\t" + e.path + "\t" + e.message);
}
"#,
    )
    .unwrap();
    let r = ritsu_testkit::run(Command::new("node").arg("runner.js").current_dir(&root), LIMIT);
    assert!(r.ok, "node failed:\n{}", r.both());
    expect_deep(r.stdout.trim(), "TypeScript");
}

#[test]
fn go_reads_the_depth_of_every_argument_first() {
    if !ritsu_testkit::need(Need::Go) {
        return;
    }
    if !ritsu_testkit::tools::runs("go", &["version"]) {
        ritsu_testkit::skip("go is not on the PATH; the Go entries are not run");
        return;
    }
    let t = TempDir::new("kaname-depth-go");
    let root = t.path().join("go");
    let c = write_case(&root, Target::Go);
    for f in ["go.mod", "go.sum"] {
        std::fs::copy(tools().join("go").join(f), root.join(f)).unwrap();
    }
    std::fs::create_dir_all(root.join("entries")).unwrap();
    std::fs::write(root.join("entries").join("entries.go"), entries_file(&c, Target::Go, "package entries\n\n", "")).unwrap();
    std::fs::write(
        root.join("main.go"),
        r#"package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"strings"

	"kanametest/entries"
	"kanametest/pkg/glue/kaname"
)

func main() {
	deep := strings.Repeat("[", 513) + "0" + strings.Repeat("]", 513)
	out, err := entries.Probe(json.RawMessage("[]"), json.RawMessage(deep))
	if err != nil {
		var k *kaname.KanameError
		if errors.As(err, &k) {
			fmt.Printf("error\t%s\t%s\n", k.Path, err.Error())
		} else {
			fmt.Printf("error\t\t%s\n", err.Error())
		}
		return
	}
	fmt.Printf("out\t%s\n", out)
}
"#,
    )
    .unwrap();
    let mut build = Command::new("go");
    build.current_dir(&root).env("GOWORK", "off").env("GOFLAGS", "-mod=mod").args(["build", "-o", "runner", "."]);
    let r = ritsu_testkit::run(&mut build, LIMIT);
    assert!(r.ok, "go build failed:\n{}", r.both());
    let r = ritsu_testkit::run(Command::new(root.join("runner")).current_dir(&root), LIMIT);
    assert!(r.ok, "the Go entries failed:\n{}", r.both());
    expect_deep(r.stdout.trim(), "Go");
}

/// The generated text itself pins the order: every argument is scanned before the first is decoded.
#[test]
fn the_depth_scan_comes_before_the_type_reading() {
    let c = checked();
    for t in [Target::TypeScript, Target::Python, Target::Go] {
        let s = entries_file(&c, t, "", "");
        let scan = match t {
            Target::TypeScript => "for (const part of",
            Target::Python => "_scan_deep(",
            _ => "for _, part := range",
        };
        let decode = match t {
            Target::TypeScript => "a = [",
            Target::Python => "a = [",
            _ => "m0.Probe(",
        };
        assert!(s.find(scan).zip(s.find(decode)).is_some_and(|(a, b)| a < b), "{t:?}: the depth scan does not come first\n{s}");
    }
}
