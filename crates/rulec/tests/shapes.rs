//! Shapes of rule the corpus does not have, checked only for "the generated code compiles".
//!
//! Three-way agreement runs over the corpus, and the corpus is made of the rules a business
//! actually writes. That leaves shapes that are legal but unrepresented, and a generator can
//! emit code for them that no compiler will accept — which is a much worse failure than a
//! wrong answer, because it stops at the point where someone is trying to use the output.
//! Each rule here is one such shape.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

fn run(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Write the rule, generate, and give back the output directory.
fn generate(tag: &str, rule: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rulec-shapes-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("r.rule");
    std::fs::write(&src, rule).unwrap();
    let (c, out) = run(&["gen", &src.to_string_lossy(), "--out", &dir.to_string_lossy()]);
    assert_eq!(c, 0, "{tag}: 生成できない:\n{out}");
    dir
}

/// `go build` over the generated package, plus `gofmt -l`, which has to come out empty
/// because the generator formats its own output (§8.5).
fn go_builds(dir: &Path, pkg: &str) {
    if !have("go") {
        eprintln!("注意: go が無いので Go 側を飛ばした");
        return;
    }
    let p = dir.join("go").join(pkg);
    let o = Command::new("go")
        .current_dir(&p)
        .args(["vet", "./..."])
        .env("GOPROXY", "off")
        .output()
        .expect("go を起動できない");
    assert!(o.status.success(), "生成した Go が通らない:\n{}", String::from_utf8_lossy(&o.stderr));
    let o = Command::new("gofmt").arg("-l").arg(&p).output().expect("gofmt を起動できない");
    let listed = String::from_utf8_lossy(&o.stdout);
    assert!(listed.trim().is_empty(), "gofmt が直したがっている: {listed}");
}

fn py_imports(dir: &Path, module: &str) {
    if !have("python3") {
        eprintln!("注意: python3 が無いので Python 側を飛ばした");
        return;
    }
    let o = Command::new("python3")
        .current_dir(dir.join("python"))
        .args(["-c", &format!("import {module}")])
        .output()
        .expect("python3 を起動できない");
    assert!(o.status.success(), "生成した Python が読み込めない:\n{}", String::from_utf8_lossy(&o.stderr));
}

/// One boolean output and a table column nothing downstream reads. Go refuses both a
/// `return 0` for a boolean and a local that is never read, and neither shape is in the
/// corpus, so both went out broken.
const UNUSED: &str = "\
rule 使わない列(unused_col) v1
description \"表が二つの列を出すが、片方は誰も読まない\"

inputs
  重量(weight) : mass[g] range >=0g <=1000g

outputs
  区分(cls) : bool

table 判定(judge)
policy first
| 重量   | -> 区分(cls) : bool | -> 補助(aux) : bool |
| <=100g | true                | false               |
| -      | false               | true                |

result 区分 = 区分
";

#[test]
fn 読まれない列があっても生成物はコンパイルできる() {
    let dir = generate("unused", UNUSED);
    go_builds(&dir, "unusedcol");
    py_imports(&dir, "unused_col");
    let go = std::fs::read_to_string(dir.join("go").join("unusedcol").join("unused_col.go")).unwrap();
    // The cell is still written out, so that the branch and the row stay 1:1.
    assert!(go.contains("補助 = false"), "セルが省かれている:\n{go}");
    assert!(go.contains("_ = 補助"), "読まれない局所変数に印が無い:\n{go}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 読まれない列はW111で名指しされる() {
    let dir = std::env::temp_dir().join(format!("rulec-shapes-w111-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("r.rule");
    std::fs::write(&src, UNUSED).unwrap();
    let (c, out) = run(&["check", &src.to_string_lossy()]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("W111"), "読まれない列が警告されない:\n{out}");
    assert!(out.contains("補助"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// One boolean output on its own: the guard's early return and the final return both have to
/// carry a boolean, not an integer.
const BOOL_ONLY: &str = "\
rule 可否だけ(bool_only) v1
description \"出力が真偽ひとつだけ\"

inputs
  重量(weight) : mass[g] range >=0g <=1000g

outputs
  可否(ok) : bool

table 判定(judge)
policy first
| 重量   | -> 可否(ok) : bool |
| <=100g | true               |
| -      | false              |

result 可否 = 可否
";

#[test]
fn 真偽ひとつだけを返す規則も生成物はコンパイルできる() {
    let dir = generate("boolonly", BOOL_ONLY);
    go_builds(&dir, "boolonly");
    py_imports(&dir, "bool_only");
    let go = std::fs::read_to_string(dir.join("go").join("boolonly").join("bool_only.go")).unwrap();
    assert!(go.contains("return false, fmt.Errorf"), "入口ガードが 0 を返している:\n{go}");
    let _ = std::fs::remove_dir_all(&dir);
}
