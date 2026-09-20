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

/// Run the generated code in every language a toolchain is present for, and hold it to the
/// reference evaluator over the whole vector suite — what `rulec test` does for the corpus.
///
/// Compiling is not the same as agreeing, and the difference is not academic: §15.88 found a
/// `string` output that compiled everywhere and answered **0** everywhere, and an `optional`
/// input that did not compile at all in half the targets. The corpus is the only material
/// that was ever run, so a shape the corpus does not have was a shape nobody ran.
fn agrees_everywhere(tag: &str, dir: &Path) {
    // The wording moves with `--lang` (the suite is pinned to Japanese by
    // `.cargo/config.toml`), so what is asserted is the exit code and the absence of a
    // failing line — neither of which is prose.
    let (c, out) = run(&["test", &dir.to_string_lossy()]);
    assert!(!out.lines().any(|l| l.starts_with("FAIL")), "{tag}: 一致しない言語がある:\n{out}");
    assert_eq!(c, 0, "{tag}: rulec test が 0 で終わらない:\n{out}");
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

/// The generated Swift has to type-check, module and runner together — which is how
/// `rulec test` builds them, and the only way `@main` on the runner is exercised. A warning
/// counts as a failure: the module is marked DO NOT EDIT, so nobody can quiet one.
fn sw_typechecks(dir: &Path, module: &str) {
    if !have("swiftc") {
        eprintln!("注意: swiftc が無いので Swift 側を飛ばした");
        return;
    }
    let o = Command::new("swiftc")
        .current_dir(dir.join("swift"))
        .args(["-typecheck", &format!("{module}.swift"), &format!("{module}_runner.swift")])
        .output()
        .expect("swiftc を起動できない");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "生成した Swift が通らない:\n{err}");
    assert!(err.trim().is_empty(), "生成した Swift が警告を出している:\n{err}");
}

/// The generated PHP has to parse, and the file has to load — PHP raises at compile time
/// for a duplicate name or a constant expression it cannot fold, which is what the keyword
/// suffix and the group constants exist to avoid (§15.77).
fn php_loads(dir: &Path, module: &str) {
    if !have("php") {
        eprintln!("注意: php が無いので PHP 側を飛ばした");
        return;
    }
    let o = Command::new("php")
        .current_dir(dir.join("php"))
        .args(["-n", "-l", &format!("{module}.php")])
        .output()
        .expect("php を起動できない");
    assert!(o.status.success(), "生成した PHP が構文として通らない:\n{}", String::from_utf8_lossy(&o.stdout));
    let o = Command::new("php")
        .current_dir(dir.join("php"))
        .args(["-n", "-r", &format!("require_once '{module}.php';")])
        .output()
        .expect("php を起動できない");
    assert!(o.status.success(), "生成した PHP が読み込めない:\n{}", String::from_utf8_lossy(&o.stdout));
}

/// The generated Java has to compile, module and runner together, at the floor the output
/// claims (§15.78). A warning counts as a failure for the same reason it does in Swift: the
/// file says DO NOT EDIT, so nobody can quiet one.
fn java_compiles(dir: &Path, class: &str) {
    if !have("javac") {
        eprintln!("注意: javac が無いので Java 側を飛ばした");
        return;
    }
    let mut args: Vec<String> = rulec::backend::JAVAC_FLAGS.iter().map(|s| s.to_string()).collect();
    args.push(format!("{class}.java"));
    args.push(format!("{class}Runner.java"));
    let o = Command::new("javac")
        .current_dir(dir.join("java"))
        .args(["-Xlint:all"])
        .args(&args)
        .output()
        .expect("javac を起動できない");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "生成した Java が通らない:\n{err}");
    assert!(err.trim().is_empty(), "生成した Java が警告を出している:\n{err}");
}

/// The generated TypeScript runner has to at least load. It is the other half of the pair
/// that calls the rule by its bare name, so it is the other one a rule aliased `d` broke.
fn ts_runs(dir: &Path, module: &str) {
    if !have("node") {
        eprintln!("注意: node が無いので TypeScript 側を飛ばした");
        return;
    }
    let o = Command::new("node")
        .current_dir(dir.join("typescript"))
        .args(["--no-warnings", &format!("{module}_runner.ts")])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("node を起動できない");
    assert!(
        o.status.success(),
        "生成した TypeScript のランナーが走らない:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
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
    php_loads(&dir, "unused_col");
    java_compiles(&dir, "UnusedCol");
    sw_typechecks(&dir, "unused_col");
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
    php_loads(&dir, "bool_only");
    java_compiles(&dir, "BoolOnly");
    sw_typechecks(&dir, "bool_only");
    let go = std::fs::read_to_string(dir.join("go").join("boolonly").join("bool_only.go")).unwrap();
    assert!(go.contains("return false, nil, fmt.Errorf"), "入口ガードが 0 を返している:\n{go}");
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
    php_loads(&dir, "bulk_fee");
    java_compiles(&dir, "BulkFee");
    sw_typechecks(&dir, "bulk_fee");
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
    php_loads(&dir, "alias_demo");
    java_compiles(&dir, "AliasDemo");
    sw_typechecks(&dir, "alias_demo");
    let _ = std::fs::remove_dir_all(&dir);
}

/// An alias that happens to be one of Swift's own words, at every level a rule has one:
/// an input, an output, an enum's members, a group and a table. Swift is the only target
/// that has to write such a name in backticks, and the rule is not the same at a call site
/// — a keyword is taken there as it stands, and backticks around it are a warning.
///
/// **Rust does not survive this shape today**, which is why it is not checked here: an alias
/// of `where` reaches `pub fn keyword_demo(where: Gram, …)` and `rustc` refuses it. Raw
/// identifiers (`r#where`) are the fix, and it is a separate change from this one.
#[test]
fn swiftの予約語に当たる別名でも生成物はコンパイルできる() {
    let dir = generate("keyword", "\
rule 予約語(keyword_demo) v1
description \"Swift の予約語に当たる別名を、入力・出力・列挙・グループ・表に書く\"

enum 区分(kind) = 内部(class) | 外部(protocol)

inputs
  重量(where) : mass[g] range >=0g <=1000g
  種別(kind)  : 区分

outputs
  可否(guard) : bool

group 内側(internal) = 内部

table 判定(switch)
policy first
| 種別 | 重量   | -> 可否(guard) : bool |
| 内側 | <=500g | true                  |
| 内側 | >500g  | false                 |
| 外部 | -      | false                 |

result 可否 = 可否
");
    let sw = std::fs::read_to_string(dir.join("swift").join("keyword_demo.swift")).unwrap();
    for want in [
        "case `class` = \"内部\"",
        "public func keywordDemo(`where`: Gram, kind: Kind) throws -> Bool {",
        "let `guard`: Bool",
        // The underscore goes on before the escaping: `_`internal`` is not an identifier.
        "private let _internal: Set<Kind>",
    ] {
        assert!(sw.contains(want), "`{want}` が生成 Swift に無い:\n{sw}");
    }
    let run = std::fs::read_to_string(dir.join("swift").join("keyword_demo_runner.swift")).unwrap();
    assert!(run.contains("keywordDemoTraced(where: "), "呼び出しの引数ラベルが逆に囲まれている:\n{run}");
    // The public function hands its arguments on the same way: label bare, value escaped.
    assert!(sw.contains("try keywordDemoTraced(where: `where`, kind: kind).0"), "{sw}");
    py_imports(&dir, "keyword_demo");
    rb_loads(&dir, "keyword_demo");
    php_loads(&dir, "keyword_demo");
    java_compiles(&dir, "KeywordDemo");
    go_builds(&dir, "keyworddemo");
    sw_typechecks(&dir, "keyword_demo");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every identifier a runner binds, read out of a runner that was just generated.
///
/// Hard-coding the list would go stale the first time someone adds a local — which is
/// precisely how this class of bug arrives — so it is discovered instead.
fn runner_locals(dir: &Path, alias: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut take = |body: &str, kw: &str| {
        for (i, _) in body.match_indices(kw) {
            let rest = &body[i + kw.len()..];
            let name: String =
                rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            // A rule alias is an ASCII identifier; anything else cannot collide.
            if !name.is_empty() && !name.starts_with(|c: char| c.is_ascii_digit()) && !out.contains(&name) {
                out.push(name);
            }
        }
    };
    let ts = std::fs::read_to_string(dir.join("typescript").join(format!("{alias}_runner.ts"))).unwrap();
    take(&ts, "const ");
    let sw = std::fs::read_to_string(dir.join("swift").join(format!("{alias}_runner.swift"))).unwrap();
    take(&sw, "let ");
    out.retain(|n| n != alias);
    assert!(out.len() > 3, "ランナーから局所変数を読み出せていない: {out:?}");
    out
}

/// A rule whose alias is a name the runner itself uses for a local.
///
/// The TypeScript and Swift runners call the rule by its bare name, in the same scope as the
/// locals holding the parsed line — so `rule d` bound the JSON object to `d` and the call
/// resolved to *that*: a type error in Swift, a runtime one in TypeScript. Python, Ruby,
/// Rust and Go qualify the call (`m.d`, `Mod.d`, `r::d`, `pkg.D`) and never had it.
///
/// `d` is the one that actually collided; the others are the rest of the locals, checked so
/// that the next local someone adds is not free to reintroduce it.
#[test]
fn ランナーの局所変数と同じ名前の規則でも生成物は動く() {
    // `d` is the one that actually collided, and it gets the full sweep.
    let dir = generate("locald", &collide_rule("d"));
    py_imports(&dir, "d");
    rb_loads(&dir, "d");
    php_loads(&dir, "d");
    java_compiles(&dir, "D");
    go_builds(&dir, "d");
    sw_typechecks(&dir, "d");
    ts_runs(&dir, "d");

    // The rest are whatever those two runners bind today. Only TypeScript and Swift call the
    // rule by its bare name, so only they can be shadowed; the other four qualify the call.
    let names = runner_locals(&dir, "d");
    let _ = std::fs::remove_dir_all(&dir);
    for name in names {
        let d = generate(&format!("local{name}"), &collide_rule(&name));
        sw_typechecks(&d, &name);
        ts_runs(&d, &name);
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// The smallest rule that reaches the runner, under a chosen alias.
fn collide_rule(alias: &str) -> String {
    format!(
        "\
rule {alias} v1
description \"別名がランナーの局所変数と衝突する\"

inputs
  重量(weight) : mass[g] range >=0g <=1000g

outputs
  料(fee) : money[円, incl_tax] round down(1円)

define 重い(heavy) : bool = 重量 >= 500g

table 判定(judge)
policy first
| 重い | -> 料(fee) : money[円, incl_tax] |
| true | 100円                            |
| -    | 0円                              |

result 料 = 料
"
    )
}

/// A date literal inside a definition is its day number in the generated code, the same
/// integer a cell's date becomes. It used to come out as `0`, so `作成日 <= 2027-03-31`
/// read `made <= 0` in every language while the evaluator read the date; the first corpus
/// rule to compare a date with a literal in a definition found it (§15.38).
#[test]
fn 定義の中の日付リテラルは通算日になる() {
    let dir = generate(
        "datelit",
        "\
rule 期限(deadline) v1

inputs
  作成日(made) : date range >=2020-01-01 <=2030-12-31

outputs
  可否(ok) : bool

define 期間内(within) : bool = 作成日 <= 2027-03-31

table 判定(judge)
policy unique
| 期間内 | -> 可否(ok) : bool |
| true   | true               |
| false  | false              |

examples
| 作成日     | -> 可否 |
| 2027-03-31 | true    |
| 2027-04-01 | false   |
",
    );
    // 2027-03-31 is day 20908 from 1970-01-01.
    let py = std::fs::read_to_string(dir.join("python").join("deadline.py")).unwrap();
    assert!(py.contains("made <= 20908"), "日付が通算日になっていない:\n{py}");
    let go = std::fs::read_to_string(dir.join("go").join("deadline").join("deadline.go")).unwrap();
    assert!(go.contains("<= 20908"), "{go}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The five dimensions added in §15.83 and §15.84, in one rule. None of them is in the
/// corpus, so nothing else holds their brand names to what a compiler will accept: `坪` and
/// `℉` are not identifiers in any target language, and the unit reaches the generated code
/// as one (`Tsubo`, `Fahrenheit`). The literals exercise both kinds of conversion — the
/// scaling every other unit uses, and the one offset in the table (41℉ is 5℃ exactly).
const DIMENSIONS: &str = "\
rule facility(facility) v1
description \"面積・体積・時間・温度・音量を一本に入れた規則。生成物が全言語でコンパイルできることを押さえる\"

enum verdict(verdict) = allowed(allowed) | barred(barred)

inputs
  floor(floor) : area[坪]         range >=0坪 <=1000坪
  tank(tank)   : volume[kL]       range >=0kL <=100kL
  shift(shift) : duration[min]    range >=0min <=600min
  temp(temp)   : temperature[℉]   range >=0℉ <=200℉
  noise(noise) : sound[dB]        range >=0dB <=130dB

outputs
  outcome(outcome) : verdict

table judge(judge)
policy first
| floor  | tank   | shift    | temp    | noise   | -> outcome(outcome) : verdict |
| <10坪  | -      | -        | -       | -       | barred                      |
| -      | >50kL  | -        | -       | -       | barred                      |
| -      | -      | >8h      | -       | -       | barred                      |
| -      | -      | -        | >100℉   | -       | barred                      |
| -      | -      | -        | -       | >=85dB  | barred                      |
| -      | -      | -        | -       | -       | allowed                     |

examples
| floor | tank | shift   | temp | noise | -> outcome |
| 100坪 | 10kL | 60min   | 41℉  | 60dB  | allowed   |
| 5坪   | 10kL | 60min   | 41℉  | 60dB  | barred    |
| 100坪 | 10kL | 540min  | 41℉  | 60dB  | barred    |
";

#[test]
fn 新しい次元の規則も生成物はコンパイルできる() {
    let dir = generate("dimensions", DIMENSIONS);
    go_builds(&dir, "facility");
    py_imports(&dir, "facility");
    rb_loads(&dir, "facility");
    php_loads(&dir, "facility");
    java_compiles(&dir, "Facility");
    sw_typechecks(&dir, "facility");
    ts_runs(&dir, "facility");
    // The unit's own spelling stays in the comment; the identifier is ASCII.
    let py = std::fs::read_to_string(dir.join("python").join("facility.py")).unwrap();
    for brand in ["Tsubo", "Kiloliter", "Minute", "Fahrenheit", "Decibel"] {
        assert!(py.contains(brand), "{brand} が生成コードに無い:\n{py}");
    }
    assert!(py.contains("area[坪]") && py.contains("temperature[℉]"), "単位が注記に残っていない:\n{py}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The words the corpus never uses. Counted from the grammar against `tests/corpus/*.rule`
/// (§15.88): the corpus is the only material `tests/threeway.rs` runs, so a word absent from
/// it is a word whose generated code has never been executed, in any language. Two of them
/// were broken — a `string` output answered 0 everywhere, and `optional` did not compile —
/// and neither `check` nor a compiler had anything to say about it.
///
/// What is here, and nowhere in the corpus: `not` in a cell, `none` and `T?`, a `string`
/// output, `round half_even`, and `constraint`.
const SURFACE: &str = "\
rule surface(surface) v1
description \"コーパスが一度も走らせていない語だけを集めた規則\"

enum 区分(kind) = 甲(a) | 乙(b) | 丙(c)
group 前半(first_half) = 甲, 乙

inputs
  下限(lo)   : mass[g]  range >=0g <=1000g
  上限(hi)   : mass[g]  range >=0g <=1000g
  種別(k)    : 区分
  備考(memo) : 区分?

constraint 下限 <= 上限

outputs
  料金(fee)   : money[円, incl_tax]  round half_even(10円)
  名札(label) : string

derive 幅(width) : mass[g] = 上限 - 下限  range >=-1000g <=1000g

table 判定(judge)
policy unique
| 種別     | 備考 | 幅     | -> 率(share) : rate[step 1%] | 名札(label) : string |
| 前半     | none | -      | 25%                          | \"前半・記載なし\"       |
| 前半     | 甲   | -      | 30%                          | \"前半・甲\"            |
| 前半     | 乙   | -      | 35%                          | \"前半・乙\"            |
| 前半     | 丙   | -      | 40%                          | \"前半・丙\"            |
| not 前半 | -    | <500g  | 75%                          | \"後半・狭い\"          |
| not 前半 | -    | >=500g | 100%                         | \"後半・広い\"          |

define 素(base) : money[円, incl_tax] = 1000円 × 率

result 料金 = 素

examples
| 下限 | 上限 | 種別 | 備考 | -> 料金 | 名札            |
| 0g   | 100g | 甲   | none | 250円   | \"前半・記載なし\" |
| 0g   | 100g | 乙   | 丙   | 400円   | \"前半・丙\"      |
| 0g   | 100g | 丙   | 甲   | 750円   | \"後半・狭い\"    |
| 0g   | 900g | 丙   | 甲   | 1000円  | \"後半・広い\"    |
";

#[test]
fn コーパスに無い語も生成物はコンパイルできる() {
    let dir = generate("surface", SURFACE);
    go_builds(&dir, "surface");
    py_imports(&dir, "surface");
    rb_loads(&dir, "surface");
    php_loads(&dir, "surface");
    java_compiles(&dir, "Surface");
    sw_typechecks(&dir, "surface");
    ts_runs(&dir, "surface");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn コーパスに無い語も評価器と全言語で一致する() {
    let dir = generate("surface-run", SURFACE);
    agrees_everywhere("surface", &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 新しい次元の規則も評価器と全言語で一致する() {
    let dir = generate("dimensions-run", DIMENSIONS);
    agrees_everywhere("dimensions", &dir);
    let _ = std::fs::remove_dir_all(&dir);
}
