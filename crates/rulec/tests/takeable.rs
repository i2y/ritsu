//! A name with no alias that has a character some language rulec writes takes in no name (§15.311):
//! `差²`, `面積㎡`, `rate%`, and `कीमत`, whose vowel sign Go takes in no name. The generated code of
//! every language writes it with `_` for each such character, one identifier for one name, and
//! `check` says so (W121). A public name of the kind is E011's, as it was. Up to 0.55.1, `check`
//! passed such a rule and the generated Python, TypeScript, JavaScript, Rust and Go did not compile.

use std::path::{Path, PathBuf};
use std::process::Command;
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_in(dir: &Path, lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(dir).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// The names of a group, a table's column, and five definitions, each with a character some
/// language takes in no name; `差_` is the author's own identifier, so `差²` takes a number after
/// the `_` it is written with.
const RULE: &str = "\
rule takeable v1

enum zone = north | south | east | west
group 遠い² = north, south

inputs
  place : zone
  a : number  range >=0 <=10
  b : number  range >=0 <=10
  c : number  range >=0 <=10
  d : number  range >=0 <=10
  e : number  range >=0 <=10

define 差² : number = a - 1
define 差_ : number = b + 1
define 面積㎡ : number = c + 3
define कीमत : number = d + 2
define rate% : number = e - 2

outputs
  fee : number  round down(1)

table steps
policy first
| e  | -> 段² : number |
| >5 | 2               |
| -  | 1               |

table by_place
policy first
| place | 段² | 差² | 差_ | 面積㎡ | कीमत | rate% | -> fee |
| 遠い² | -   | -   | -   | -      | -    | -     | 1      |
| east  | 2   | -   | -   | -      | -    | -     | 2      |
| east  | -   | >5  | -   | -      | -    | -     | 3      |
| east  | -   | -   | >8  | -      | -    | -     | 4      |
| west  | -   | -   | -   | >10    | -    | -     | 5      |
| west  | -   | -   | -   | -      | >11  | -     | 6      |
| west  | -   | -   | -   | -      | -    | >7    | 7      |
| -     | -   | -   | -   | -      | -    | -     | 0      |
";

/// What `check` says of each name, and the identifier the generated code writes for it.
const SAID: &[(&str, &str, &str)] = &[
    ("遠い²", "`²`", "遠い_"),
    ("差²", "`²`", "差_2"),
    ("面積㎡", "`㎡`", "面積_"),
    ("कीमत", "`ी`", "क_मत"),
    ("rate%", "`%`", "rate_"),
    ("段²", "`²`", "段_"),
];

#[test]
fn a_character_no_language_takes_is_written_as_an_underscore() {
    let tmp = TempDir::new("takeable");
    let d = tmp.path();
    std::fs::write(d.join("takeable.rule"), RULE).unwrap();
    let (c, out, _) = run_in(d, "en", &["check", "takeable.rule"]);
    assert_eq!(c, 0, "{out}");
    for (name, odd, id) in SAID {
        assert!(
            out.contains(&format!("warning[W121]: Some target languages take no {odd} in a name, so the generated code writes `{name}` as `{id}`")),
            "no W121 for `{name}`:\n{out}"
        );
    }
    assert!(out.contains("`差_` is the identifier of another name of this rule, and one identifier cannot be two names, so the generated code puts a number after it."), "{out}");
    assert!(out.contains("Languages that take no such character in a name include Go. One name is one identifier, so every language's generated code writes this name as `क_मत`"), "{out}");
    assert!(out.contains("Languages that take no such character in a name include Python, TypeScript, JavaScript, Rust and Go."), "{out}");
    assert!(out.contains("give it an alias (`面積㎡(<alias>)`)"), "{out}");
    let (c, ja, _) = run_in(d, "ja", &["check", "takeable.rule"]);
    assert_eq!(c, 0, "{ja}");
    assert!(ja.contains("警告[W121]: `差²` の `²` を名前に取らない言語があるので、生成コードでは `差_2` と書きます"), "{ja}");
    assert!(ja.contains("その字を名前に取らない言語には、Python・TypeScript・JavaScript・Rust・Go があります。"), "{ja}");

    // The identifiers, in the generated code of each language; the messages keep the names.
    let generated = d.join("gen");
    let out_dir = generated.to_string_lossy().into_owned();
    let (c, _, e) = run_in(d, "en", &rulec::targets::up_to_0_40(&["gen", "takeable.rule", "--out", &out_dir]));
    assert_eq!(c, 0, "{e}");
    let py = std::fs::read_to_string(generated.join("python/takeable.py")).unwrap();
    for (name, _, id) in SAID {
        assert!(py.contains(id), "takeable.py has no `{id}` for `{name}`:\n{py}");
    }
    for bad in ["差²", "面積㎡", "rate%", "कीमत"] {
        assert!(!py.lines().any(|l| l.trim_start().starts_with(&format!("{bad} ="))), "takeable.py assigns `{bad}`:\n{py}");
    }
    if !ready(Need::Python, || have("python3"), "no python3, so the generated code was not run") {
        return;
    }
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .env("TMPDIR", tmpdir_in(&generated))
        .args(["test", generated.to_str().unwrap()])
        .output()
        .expect("cannot start rulec test");
    let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{said}");
    assert!(said.lines().any(|l| l.starts_with("ok    takeable (Python)")), "{said}");
    for l in said.lines().filter(|l| l.starts_with("warning") || l.starts_with("skip")) {
        eprintln!("{l}");
    }
}

/// A shape's name is the parameter of `<rule>_from`, written as a value's is.
#[test]
fn a_shape_with_such_a_character_is_a_parameter_of_another_name() {
    let tmp = TempDir::new("takeable-shape");
    let d = tmp.path();
    std::fs::copy(root().join("tests/spellings/orders.schema.json"), d.join("orders.schema.json")).unwrap();
    std::fs::write(
        d.join("rush_fee.rule"),
        "rule rush_fee v1\n\nshape 注文² = jsonschema \"orders.schema.json\" \"#/$defs/Order\"\n\ninputs\n  rush : bool  from 注文².rush\n\noutputs\n  fee : number  round down(1)\n\ntable by_rush\npolicy unique\n| rush  | -> fee |\n| true  | 10     |\n| false | 5      |\n",
    )
    .unwrap();
    let (c, out, _) = run_in(d, "en", &["check", "rush_fee.rule"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("warning[W121]: Some target languages take no `²` in a name, so the generated code writes `注文²` as `注文_`"), "{out}");
    let (c, api, _) = run_in(d, "en", &["api", "rush_fee.rule"]);
    assert_eq!(c, 0);
    assert!(api.contains(r#""signature":"def rush_fee_from(注文_: dict[str, Any]) -> int:""#), "{api}");
    let generated = d.join("gen");
    let out_dir = generated.to_string_lossy().into_owned();
    let (c, _, e) = run_in(d, "en", &rulec::targets::up_to_0_40(&["gen", "rush_fee.rule", "--out", &out_dir]));
    assert_eq!(c, 0, "{e}");
    let py = std::fs::read_to_string(generated.join("python/rush_fee.py")).unwrap();
    // The parameter moves; what a refusal says names the shape as declared.
    assert!(py.contains("def rush_fee_from(注文_: dict[str, Any]) -> int:") && py.contains(r#"_bool_at(注文_, "rush", "注文²")"#), "{py}");
    if !ready(Need::Python, || have("python3"), "no python3, so the generated code was not run") {
        return;
    }
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .env("TMPDIR", tmpdir_in(&generated))
        .args(["test", generated.to_str().unwrap()])
        .output()
        .expect("cannot start rulec test");
    let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{said}");
    assert!(said.lines().any(|l| l.starts_with("ok    rush_fee (Python)")), "{said}");
}

/// An input, an output, a sequence and a field are public names: one that is no ASCII identifier
/// takes an alias (E011), and nothing is generated from the rule, so W121 does not say it again.
#[test]
fn a_public_name_with_such_a_character_is_e011_alone() {
    let tmp = TempDir::new("takeable-public");
    let d = tmp.path();
    std::fs::write(
        d.join("io_fee.rule"),
        "rule io_fee v1\n\ninputs\n  qty% : number  range >=0 <=10\n\noutputs\n  fee² : number  round down(1)\n\ntable by_qty\npolicy first\n| qty% | -> fee² |\n| >5   | 2       |\n| -    | 1       |\n",
    )
    .unwrap();
    let (c, out, _) = run_in(d, "en", &["check", "io_fee.rule"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("error[E011]") && out.contains("Missing: input qty% / output fee²"), "{out}");
    assert!(!out.contains("W121"), "{out}");
}
