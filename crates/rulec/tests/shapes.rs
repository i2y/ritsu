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

/// The generated Ruby has to parse, and the module has to load — Ruby raises at load time
/// for a constant that cannot exist, which is the failure the `GROUP_` prefix exists to
/// avoid (§15.20).
fn rb_loads(dir: &Path, module: &str) {
    if !have("ruby") {
        eprintln!("注意: ruby が無いので Ruby 側を飛ばした");
        return;
    }
    let o = Command::new("ruby")
        .current_dir(dir.join("ruby"))
        .args(["-c", &format!("{module}.rb")])
        .output()
        .expect("ruby を起動できない");
    assert!(o.status.success(), "生成した Ruby が構文として通らない:\n{}", String::from_utf8_lossy(&o.stderr));
    let o = Command::new("ruby")
        .current_dir(dir.join("ruby"))
        .args(["-e", &format!("require_relative {module:?}")])
        .output()
        .expect("ruby を起動できない");
    assert!(o.status.success(), "生成した Ruby が読み込めない:\n{}", String::from_utf8_lossy(&o.stderr));
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
    rb_loads(&dir, "unused_col");
    let go = std::fs::read_to_string(dir.join("go").join("unusedcol").join("unused_col.go")).unwrap();
    // The cell is still written out, so that the branch and the row stay 1:1.
    assert!(go.contains("aux = false"), "セルが省かれている:\n{go}");
    assert!(go.contains("_ = aux"), "読まれない局所変数に印が無い:\n{go}");
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
    rb_loads(&dir, "bool_only");
    let go = std::fs::read_to_string(dir.join("go").join("boolonly").join("bool_only.go")).unwrap();
    assert!(go.contains("return false, fmt.Errorf"), "入口ガードが 0 を返している:\n{go}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A rule written entirely in ASCII. §1.3 asks for an alias because a kanji has no uppercase
/// and cannot begin an exported Go identifier — which `shipping_fee` does not suffer from, so
/// demanding `rule shipping_fee(shipping_fee)` was ceremony with nothing behind it.
const ENGLISH: &str = "\
rule bulk_fee v1
description \"Written entirely in ASCII: no alias should be required anywhere\"

enum member_kind = basic | gold | platinum

inputs
  weight : mass[g]  range >=1g <=40kg
  member : member_kind

outputs
  fee : money[円, incl_tax]  round up(10円)

table base_fee
policy first
| weight  | member   | -> fee : money[円, incl_tax] |
| <=2000g | platinum | 0円                          |
| <=2000g | -        | 800円                        |
| >2000g  | -        | 1100円                       |

result fee = fee
";

#[test]
fn 全部asciiで書いた規則は別名を求められない() {
    let dir = std::env::temp_dir().join(format!("rulec-shapes-en-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("r.rule");
    std::fs::write(&src, ENGLISH).unwrap();
    let (c, out) = run(&["check", &src.to_string_lossy()]);
    assert_eq!(c, 0, "ASCII だけの規則が別名を要求されている:\n{out}");
    assert!(!out.contains("E011"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 全部asciiで書いた規則も生成物はコンパイルできる() {
    let dir = generate("english", ENGLISH);
    go_builds(&dir, "bulkfee");
    py_imports(&dir, "bulk_fee");
    rb_loads(&dir, "bulk_fee");
    let py = std::fs::read_to_string(dir.join("python").join("bulk_fee.py")).unwrap();
    assert!(py.contains("def bulk_fee(weight: Gram, member: MemberKind) -> YenInclTax:"), "{py}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §1.3: an alias on an internal name is optional, and writing one makes the generated code
/// use it. It used to be parsed and then thrown away, so `derive 残余(margin)` promised a
/// name the output never contained.
#[test]
fn 内部名の別名は生成コードの識別子になる() {
    let dir = generate("alias", "\
rule alias_demo(alias_demo) v1
description \"内部名に別名を書いたら、生成コードがその名前を使う\"

enum 色(color) = 赤(red) | 青(blue) | 緑(green)

inputs
  価格(price) : money[円, incl_tax] range >=0円 <=1万円
  色(c)       : 色

outputs
  結果(result) : money[円, incl_tax] round down(1円)

group 暖色(warm) = 赤
derive 半額(half) : money[円, incl_tax] = 価格 - 価格  range >=-1万円 <=1万円
define 高額(pricey) : bool = 価格 >= 5000円

table 判定(decide)
policy first
| 色   | 高額 | -> 中間(mid) : money[円, incl_tax] |
| 暖色 | true | 100円                              |
| -    | -    | 0円                                |

result 結果 = 中間
");
    let py = std::fs::read_to_string(dir.join("python").join("alias_demo.py")).unwrap();
    for want in ["_warm = frozenset(", "half = ", "pricey = ", "mid = 100", "c in _warm"] {
        assert!(py.contains(want), "`{want}` が生成 Python に無い:\n{py}");
    }
    for unwanted in ["暖色", "半額", "高額", "中間"] {
        // The cells in the row comments still carry the rule's own words; the identifiers
        // should not.
        let code: String = py.lines().filter(|l| !l.contains('#')).collect::<Vec<_>>().join("\n");
        assert!(!code.contains(unwanted), "`{unwanted}` が識別子として残っている:\n{code}");
    }
    go_builds(&dir, "aliasdemo");
    py_imports(&dir, "alias_demo");
    rb_loads(&dir, "alias_demo");
    let _ = std::fs::remove_dir_all(&dir);
}
