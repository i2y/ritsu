//! What `rulec gen` makes sure of before it writes anything (§15.334, §15.335). Two files of the run
//! that write one place (one file, two paths that differ only in the case of their letters, a Go
//! package, a PHP namespace, a Ruby module) stop it at 2, naming the two files and the place. A W121
//! that says the code of a target the run writes cannot be used as it stands stops it at 1, as
//! E153, with the warning's notes. Either way nothing is written. The places `gen` works out are
//! held to the files it writes. The wording is in `tests/golden/places` (`RULEC_BLESS=1 cargo test
//! --test places` writes it).

use ritsu_testkit::TempDir;
use ritsu_testkit::run::finish;
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, ready};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// How long a run of `rulec gen` may take: past it, the test fails saying what it ran.
const LIMIT: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// `rulec <args>` in `dir`, in `lang`: the exit code, the standard output and the standard error.
fn rulec_in(dir: &Path, lang: &str, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.env("RULEC_LANG", lang).env_remove("RITSU_LANG").current_dir(dir).args(args);
    if !args.contains(&"--lang") {
        c.args(["--lang", lang]);
    }
    let o = finish(&mut c, LIMIT, &format!("rulec {}", args.join(" ")));
    (o.code.unwrap_or(-1), o.stdout, o.stderr)
}

/// `gen <files> --target <target> --out out` in `dir`, in `lang`.
fn generate(dir: &Path, lang: &str, files: &[&str], target: &str) -> (i32, String, String) {
    let mut args = vec!["gen"];
    args.extend_from_slice(files);
    args.extend(["--target", target, "--out", "out"]);
    rulec_in(dir, lang, &args)
}

/// A rule that says whether a weight is large, named `name`.
fn size_rule(name: &str) -> String {
    format!(
        "rule {name} v1\ndescription \"Whether a weight is large (fictional)\"\n\ninputs\n  weight : number  range >=1 <=100\n\noutputs\n  big : bool\n\ntable big_of\n| weight | -> big |\n| <=10   | false  |\n| >10    | true   |\n"
    )
}

/// The same rule in Japanese, named `name` with its alias.
fn size_rule_ja(name: &str) -> String {
    format!(
        "rule {name} v1\ndescription \"重さが大きいかどうか（架空の例）\"\n\ninputs\n  重さ(weight) : number  range >=1 <=100\n\noutputs\n  大きい(big) : bool\n\ntable 判定(big_of)\n| 重さ  | -> 大きい |\n| <=10 | false   |\n| >10  | true    |\n"
    )
}

/// A directory with each of `files` (its path, its text) in it.
fn project(name: &str, files: &[(&str, &str)]) -> TempDir {
    let t = TempDir::new(name);
    for (rel, text) in files {
        let p = t.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    t
}

/// What the runs said, in English and in Japanese, held to the golden file `name`.
fn golden(name: &str, text: &str) {
    ritsu_testkit::golden::golden(root().join(format!("tests/golden/places/{name}.txt")), text);
}

/// Two rules whose aliases make one name in a language that holds a package, a namespace or a
/// module as one, or one class file: each such target stops the run at 2 and writes nothing, in
/// both languages; a target that writes each rule into a module of its own writes both. A rule
/// named in Japanese with its alias is the same.
#[test]
fn two_rules_that_write_one_place_stop_gen_and_write_nothing() {
    let t = project("places-twins", &[("size_label.rule", &size_rule("size_label")), ("sizeLabel.rule", &size_rule("sizeLabel"))]);
    let dir = t.path();
    let files = ["size_label.rule", "sizeLabel.rule"];
    for target in ["go", "java", "php", "ruby"] {
        let mut said = String::new();
        for lang in ["en", "ja"] {
            let (code, stdout, stderr) = generate(dir, lang, &files, target);
            assert_eq!((code, stdout.as_str()), (2, ""), "{target} {lang}: {stderr}");
            assert!(!dir.join("out").exists(), "{target} {lang}: nothing is written");
            said.push_str(&stderr);
        }
        golden(&format!("twins-{target}"), &said);
    }
    for target in ["python", "typescript", "rust", "swift"] {
        let (code, _, stderr) = generate(dir, "en", &files, target);
        assert_eq!(code, 0, "{target}: {stderr}");
        let out = dir.join("out").join(target);
        assert!(std::fs::read_dir(&out).unwrap().count() >= 2, "{target}: both rules are written");
        std::fs::remove_dir_all(dir.join("out")).unwrap();
    }
    // named in Japanese, with the aliases
    let t = project("places-twins-ja", &[("大きさの印.rule", &size_rule_ja("大きさの印(size_label)")), ("大きさ.rule", &size_rule_ja("大きさ(sizeLabel)"))]);
    let (code, _, stderr) = generate(t.path(), "ja", &["大きさの印.rule", "大きさ.rule"], "go");
    assert_eq!(code, 2, "{stderr}");
    assert_eq!(
        stderr,
        "エラー: `大きさの印.rule` と `大きさ.rule` が、どちらも Go のパッケージ out/go/sizelabel/ に書きます（out/go/sizelabel/size_label.go と out/go/sizelabel/sizeLabel.go）。一つのパッケージで同じ名前を二度宣言するので組めません。どちらかの別名を変えてください\n"
    );
    assert!(!t.path().join("out").exists());
}

/// One file under one path, and under two paths that differ only in the case of their letters: two
/// rules of one alias in two directories, an `enums` file and a rule of one alias, and two aliases
/// that differ only in case. A PHP namespace is one whatever the case of its letters, and a Go
/// package is the directory both write into; Ruby tells its modules apart by case.
#[test]
fn one_path_under_one_spelling_or_two_stops_gen() {
    let enums = "enums size_label v1\ndescription \"Sizes (fictional)\"\n\nenum size = small | large\n";
    let t = project(
        "places-paths",
        &[
            ("a.rule", &size_rule("size_label")),
            ("b.rule", &size_rule("Size_Label")),
            ("c.rule", &size_rule("sizelabel")),
            ("one/x.rule", &size_rule("x")),
            ("two/x.rule", &size_rule("x")),
            ("sizes.rule", enums),
        ],
    );
    let dir = t.path();
    let mut said = String::new();
    for (files, target) in [
        (["a.rule", "b.rule"], "python"),
        (["one/x.rule", "two/x.rule"], "typescript"),
        (["sizes.rule", "a.rule"], "python"),
        (["a.rule", "c.rule"], "php"),
        (["a.rule", "c.rule"], "java"),
        (["a.rule", "c.rule"], "go"),
    ] {
        for lang in ["en", "ja"] {
            let (code, stdout, stderr) = generate(dir, lang, &files, target);
            assert_eq!((code, stdout.as_str()), (2, ""), "{files:?} {target} {lang}: {stderr}");
            assert!(!dir.join("out").exists(), "{files:?} {target} {lang}: nothing is written");
            said.push_str(&stderr);
        }
    }
    golden("paths", &said);
    let (code, _, stderr) = generate(dir, "en", &["a.rule", "c.rule"], "ruby");
    assert_eq!(code, 0, "{stderr}");
    assert!(dir.join("out/ruby/size_label.rb").is_file() && dir.join("out/ruby/sizelabel.rb").is_file());
}

/// What is not two places: a file every rule writes the same (the round trip's), one file named
/// twice, and the module of an `enums` file two rules read.
#[test]
fn what_every_rule_writes_the_same_and_one_file_named_twice_do_not_stop_gen() {
    let corpus = root().join("tests/corpus");
    let returns = std::fs::read_to_string(corpus.join("品目の返品.rule")).unwrap();
    let t = project(
        "places-shared",
        &[
            ("a.rule", &size_rule("size_label")),
            ("b.rule", &size_rule("parcel_size")),
            ("品目の返品.rule", &returns),
            ("品目の交換.rule", &returns.replacen("rule 品目の返品(goods_returns) v1", "rule 品目の交換(goods_exchanges) v1", 1)),
            ("shared/品目.rule", &std::fs::read_to_string(corpus.join("shared/品目.rule")).unwrap()),
        ],
    );
    let dir = t.path();
    let ok = |args: &[&str]| {
        let (code, stdout, stderr) = rulec_in(dir, "en", args);
        assert_eq!(code, 0, "{args:?}: {stdout}{stderr}");
        let _ = std::fs::remove_dir_all(dir.join("out"));
    };
    ok(&["gen", "a.rule", "b.rule", "--target", "python", "--target", "go", "--extra", "round-test", "--out", "out"]);
    ok(&["gen", "a.rule", "./a.rule", "--target", "typescript", "--target", "php", "--out", "out"]);
    ok(&["gen", "品目の返品.rule", "品目の交換.rule", "--target", "python", "--target", "go", "--target", "ruby", "--target", "php", "--out", "out"]);
}

/// A rule whose Python cannot take the name of an output (W121): `gen` with Python, or with the
/// Connect service beside it, says the warning as E153 with what stops, writes the targets it
/// does not name and nothing of the ones it names, and exits 1; `check` says the warning and
/// passes; a target the warning does not name writes the rule. A W121 that only says how the code
/// writes a name stops nothing.
#[test]
fn a_name_the_code_of_a_target_cannot_take_stops_gen_at_1() {
    let en = "rule parcel_flags v1\ndescription \"Whether a parcel is heavy, and whether to tell its sender (fictional)\"\n\ninputs\n  weight : number  range >=1 <=100\n\noutputs\n  heavy : bool\n  _tell : bool\n\ntable flags_of\n| weight | -> heavy | _tell |\n| <=10   | false    | false |\n| >10    | true     | true  |\n";
    let ja = "rule 小包の印(parcel_flags) v1\ndescription \"小包が重いかと、送り主に知らせるか（架空の例）\"\n\ninputs\n  重さ(weight) : number  range >=1 <=100\n\noutputs\n  重い(heavy)     : bool\n  知らせる(_tell) : bool\n\ntable 印の判定(flags_of)\n| 重さ  | -> 重い | 知らせる |\n| <=10 | false  | false   |\n| >10  | true   | true    |\n";
    let fee = "rule plot_fee v1\ndescription \"The fee for a class of plot (fictional)\"\n\ninputs\n  class : number  range >=1 <=3\n\noutputs\n  fee : number  round down(1)\n\nresult fee = class * 100\n";
    let t = project("places-e153", &[("parcel_flags.rule", en), ("小包の印.rule", ja), ("plot_fee.rule", fee)]);
    let dir = t.path();
    // check: the warning
    let (code, stdout, _) = rulec_in(dir, "en", &["check", "parcel_flags.rule"]);
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("warning[W121]: `_tell` starts with `_`") && !stdout.contains("E153"), "{stdout}");
    // gen: the error, and nothing written
    let mut said = String::new();
    for (lang, file) in [("en", "parcel_flags.rule"), ("ja", "小包の印.rule")] {
        let (code, stdout, stderr) = generate(dir, lang, &[file], "python");
        assert_eq!(code, 1, "{lang}: {stdout}{stderr}");
        assert!(!dir.join("out").exists(), "{lang}: nothing is written");
        said.push_str(&format!("{stdout}{stderr}"));
    }
    golden("E153", &said);
    // with a target the warning does not name beside one it does, that one is written and Python
    // is not
    let (code, stdout, _) = rulec_in(dir, "en", &["gen", "parcel_flags.rule", "--target", "typescript", "--target", "python", "--out", "out"]);
    assert_eq!(code, 1, "{stdout}");
    assert!(dir.join("out/typescript/parcel_flags.ts").is_file(), "{stdout}");
    assert!(!dir.join("out/python").exists(), "{stdout}");
    std::fs::remove_dir_all(dir.join("out")).unwrap();
    // the Connect service is named with Python; the target the warning does not name is written
    let (code, stdout, _) = rulec_in(dir, "en", &["gen", "parcel_flags.rule", "--target", "python", "--target", "proto", "--extra", "connect", "--out", "out"]);
    assert_eq!(code, 1, "{stdout}");
    assert!(stdout.contains(" The code `--target python` and `--extra connect` writes cannot be used with the name as it stands, so gen writes nothing of `--target python` and `--extra connect`; the targets it does not name are written.\n"), "{stdout}");
    assert!(dir.join("out/proto").exists(), "{stdout}");
    assert!(!dir.join("out/python").exists(), "{stdout}");
    std::fs::remove_dir_all(dir.join("out")).unwrap();
    let (code, stdout, _) = rulec_in(dir, "ja", &["gen", "小包の印.rule", "--target", "python", "--target", "proto", "--extra", "connect", "--out", "out"]);
    assert_eq!(code, 1, "{stdout}");
    assert!(stdout.contains(" `--target python`・`--extra connect` が書くコードは、この名前のままでは使えないので、gen は`--target python`・`--extra connect`を書かず、ほかの出力先は書きます。\n"), "{stdout}");
    std::fs::remove_dir_all(dir.join("out")).unwrap();
    // a target it does not name
    let (code, stdout, stderr) = generate(dir, "en", &["parcel_flags.rule"], "typescript");
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(dir.join("out/typescript/parcel_flags.ts").is_file());
    std::fs::remove_dir_all(dir.join("out")).unwrap();
    // a rule named like a module of Python's standard library, which its module beside the runner
    // is read in place of
    std::fs::write(dir.join("calendar.rule"), size_rule("calendar")).unwrap();
    let (code, stdout, _) = generate(dir, "en", &["calendar.rule"], "python");
    assert!(code == 1 && stdout.starts_with("error[E153]: `calendar` "), "{stdout}");
    assert!(!dir.join("out").exists());
    // a W121 that only says how the code writes the name: a keyword, with `_` after it
    let (code, stdout, _) = rulec_in(dir, "en", &["check", "plot_fee.rule"]);
    assert!(code == 0 && stdout.contains("warning[W121]: `class` is a keyword in a target language"), "{stdout}");
    let (code, stdout, stderr) = generate(dir, "en", &["plot_fee.rule"], "python");
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(dir.join("out/python/plot_fee.py").is_file());
}

/// Two files `gen` writes for Swift declare one name: the rule's module holds the type of an enum
/// an `apply` brings in, and the module of the `enums` file it reads holds an enum that spells the
/// same — which Swift, building the package into one module, reads as two declarations
/// (`invalid redeclaration`). `check` says W121 of each such name, naming the two files and what
/// each declares it as; `gen` asked for Swift says them as E153, writes nothing of any target and
/// exits 1; a target the warning does not name writes the rule (§15.345).
#[test]
fn two_swift_files_declaring_one_name_stop_gen_at_1() {
    let shared = "enums shared v1\n\nenum kind = goods | capex\nenum authority_decider = head | chief | board\n";
    let callee = "rule callee v1\ndescription \"Who approves, by size (fictional)\"\n\ninputs\n  amount : number  range >=0 <=100\n\noutputs\n  who : decider\n\nenum decider = head | chief\n\ntable who_of\n| amount | -> who  |\n| <=50   | head    |\n| >50    | chief   |\n";
    let caller = "rule caller v1\ndescription \"What shared's kind and the callee's answer give (fictional)\"\n\nimport enum kind from \"shared.rule\"\n\ninputs\n  amount : number  range >=0 <=100\n  kind   : kind\n\napply authority = \"callee.rule\" meaning:d12e7f865d93c2f4\n  amount = amount\n  who -> authority_who\n\noutputs\n  ok : bool\n\ntable ok_of\n| kind  | -> ok |\n| goods | true  |\n| capex | false |\n";
    let t = project("places-sw-twice", &[("shared.rule", shared), ("callee.rule", callee), ("caller.rule", caller)]);
    let dir = t.path();
    // check: a warning for each name, the two files that declare it named
    let (code, stdout, _) = rulec_in(dir, "en", &["check", "caller.rule"]);
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("warning[W121]: `AuthorityDecider` is declared twice in the code generated for Swift"), "{stdout}");
    assert!(stdout.contains("`caller.swift` declares `AuthorityDecider` as the type of the enum `authority:decider` an `apply` brings in"), "{stdout}");
    assert!(stdout.contains("`shared.swift` declares `AuthorityDecider` as the type of the enum `authority_decider` of the `enums` file"), "{stdout}");
    assert!(stdout.contains("warning[W121]: `Kind` is declared twice"), "{stdout}");
    let (code, stdout, _) = rulec_in(dir, "ja", &["check", "caller.rule"]);
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("警告[W121]: `AuthorityDecider` が、Swift の生成コードで二度宣言されます"), "{stdout}");
    // gen asked for Swift: the warnings as E153, nothing written, exit 1
    let (code, stdout, stderr) = generate(dir, "en", &["caller.rule"], "swift");
    assert_eq!(code, 1, "{stdout}{stderr}");
    assert!(stdout.contains("error[E153]: `AuthorityDecider` is declared twice"), "{stdout}");
    assert!(stdout.contains(" The code `--target swift` writes cannot be used with the name as it stands, so gen writes nothing of `--target swift`; the targets it does not name are written."), "{stdout}");
    assert!(!dir.join("out").exists(), "nothing is written");
    // with a target it does not name beside it, that one is written and Swift is not
    let (code, stdout, stderr) = rulec_in(dir, "en", &["gen", "caller.rule", "--target", "swift", "--target", "python", "--out", "out"]);
    assert_eq!(code, 1, "{stdout}{stderr}");
    assert!(dir.join("out/python/caller.py").is_file() && dir.join("out/python/shared.py").is_file(), "{stdout}");
    assert!(!dir.join("out/swift").exists(), "{stdout}");
    std::fs::remove_dir_all(dir.join("out")).unwrap();
    // a target it does not name writes the rule
    let (code, stdout, stderr) = generate(dir, "en", &["caller.rule"], "python");
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(dir.join("out/python/caller.py").is_file());
}

/// Whether `cmd` answers, as `tests/shapes.rs` asks it (swiftc's TMPDIR is swept with it).
fn have(cmd: &str) -> bool {
    let tmp = TempDir::new("probe");
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).env("TMPDIR", tmp.path()).output().map(|o| o.status.success()).unwrap_or(false))
}

/// The same enum brought in by two paths — an `apply`'s `authority:decider` and the enum
/// `authority_decider` the `enums` file it reads declares, the same values in the same order —
/// is declared once (§15.355): `check` says no W121, and `gen` writes the type in the file's
/// module alone, where the rule's own declaration held a member the file's does not (here the
/// labels) in an `extension` of the rule's. `swiftc` typechecks the three files together. The
/// same name over different values is W121 again — the test above holds it — and dropping the
/// test of the two being one enum puts the warnings back.
#[test]
fn the_same_enum_by_two_paths_is_declared_once() {
    let shared = "enums shared v1\n\nenum kind = goods | capex\nenum authority_decider = head | chief\n";
    let callee = "rule callee v1\ndescription \"Who approves, by size (fictional)\"\n\ninputs\n  amount : number  range >=0 <=100\n\noutputs\n  who : decider\n\nenum decider = head \"Head\" | chief \"Chief\"\n\ntable who_of\n| amount | -> who  |\n| <=50   | head    |\n| >50    | chief   |\n";
    let caller = "rule caller v1\ndescription \"What shared's kind and the callee's answer give (fictional)\"\n\nimport enum kind from \"shared.rule\"\n\ninputs\n  amount : number  range >=0 <=100\n  kind   : kind\n\napply authority = \"callee.rule\" meaning:fa545f892e0dd127\n  amount = amount\n  who -> authority_who\n\noutputs\n  ok : bool\n\ntable ok_of\n| kind  | -> ok |\n| goods | true  |\n| capex | false |\n";
    let shared_ja = "enums 共有(shared) v1\n\nenum 種類(kind) = 品物(goods) | 設備(capex)\nenum 決裁者(authority_decider) = 部長(head) | 本部長(chief)\n";
    let callee_ja = "rule 権限(callee) v1\ndescription \"金額で決める承認者（架空）\"\n\ninputs\n  金額(amount) : number  range >=0 <=100\n\noutputs\n  誰(who) : 決裁者\n\nenum 決裁者(decider) = 部長(head) \"部長\" | 本部長(chief) \"本部長\"\n\ntable 誰の表(who_of)\n| 金額 | -> 誰     |\n| <=50 | 部長   |\n| >50  | 本部長 |\n";
    let caller_ja = "rule 呼出(caller) v1\ndescription \"共有の種類と権限の答えが与えるもの（架空）\"\n\nimport enum 種類 from \"共有.rule\"\n\ninputs\n  金額(amount) : number  range >=0 <=100\n  種類(kind)   : 種類\n\napply 権限(authority) = \"権限.rule\" meaning:83ce155366e8180f\n  金額 = 金額\n  誰 -> 権限の誰(authority_who)\n\noutputs\n  良い(ok) : bool\n\ntable 判定(ok_of)\n| 種類 | -> 良い |\n| 品物 | true    |\n| 設備 | false   |\n";
    let t = project(
        "places-sw-shared",
        &[("shared.rule", shared), ("callee.rule", callee), ("caller.rule", caller), ("共有.rule", shared_ja), ("権限.rule", callee_ja), ("呼出.rule", caller_ja)],
    );
    let dir = t.path();
    for (lang, rule) in [("en", "caller.rule"), ("ja", "呼出.rule")] {
        // check: no name is declared twice
        let (code, stdout, _) = rulec_in(dir, lang, &["check", rule]);
        assert_eq!(code, 0, "{lang}: {stdout}");
        assert!(!stdout.contains("W121"), "{lang}: {stdout}");
        // gen: the file's module declares the type; the rule's module adds an extension
        let (code, stdout, stderr) = rulec_in(dir, lang, &["gen", rule, "--target", "swift", "--extra", "runner", "--out", "out"]);
        assert_eq!(code, 0, "{lang}: {stdout}{stderr}");
        let module = std::fs::read_to_string(dir.join("out/swift/caller.swift")).unwrap();
        let owner = std::fs::read_to_string(dir.join("out/swift/shared.swift")).unwrap();
        assert!(!module.contains("public enum AuthorityDecider"), "{lang}: the rule declares it\n{module}");
        assert!(module.contains("extension AuthorityDecider"), "{lang}: the rule adds the members it had\n{module}");
        assert!(owner.contains("public enum AuthorityDecider"), "{lang}: the file's module declares it\n{owner}");
        // the module the files are built into takes them together
        if ready(Need::Swift, || have("swiftc"), "swiftc が無いので Swift 側を飛ばした") {
            let sw = dir.join("out/swift");
            let o = Command::new("swiftc")
                .current_dir(&sw)
                .env("TMPDIR", tmpdir_in(dir))
                .args(["-typecheck"])
                .args(std::fs::read_dir(&sw).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect::<Vec<_>>())
                .output()
                .expect("swiftc を起動できない");
            let err = String::from_utf8_lossy(&o.stderr);
            assert!(o.status.success(), "{lang}: 生成した Swift が通らない:\n{err}");
            assert!(err.trim().is_empty(), "{lang}: 生成した Swift が警告を出している:\n{err}");
        }
        std::fs::remove_dir_all(dir.join("out")).unwrap();
    }
}

/// Every target and every extra, over rules that write what each writes (a rule of its own, one
/// that walks a sequence, one that reads an `enums` file, an `enums` file with `prerequisites`):
/// each file the places name is a file `gen` writes, and each file `gen` writes is named, but the
/// files of the module (`proto/buf.yaml` and the like) and the vectors written as the rule asks
/// (named after the rule, so one that clashes clashes with the rule's own vectors first). Each Go
/// package is a directory `gen` writes into, and each PHP namespace and Ruby module the one its
/// file declares.
#[test]
fn the_places_gen_works_out_are_the_files_it_writes() {
    let corpus = root().join("tests/corpus");
    let names = ["member_shipping_fee.rule", "小包のまとめ出荷.rule", "品目の返品.rule", "shared/certifications.rule"];
    let mut files: Vec<(String, String)> = names.iter().map(|n| (n.to_string(), std::fs::read_to_string(corpus.join(n)).unwrap())).collect();
    files.push(("shared/品目.rule".to_string(), std::fs::read_to_string(corpus.join("shared/品目.rule")).unwrap()));
    let t = project("places-all", &files.iter().map(|(n, s)| (n.as_str(), s.as_str())).collect::<Vec<_>>());
    let dir = t.path();
    let targets = rulec::targets::target_names();
    let extras = rulec::targets::EXTRAS;
    let mut args = vec!["gen"];
    args.extend(names);
    for t in targets {
        args.extend(["--target", *t]);
    }
    for e in extras {
        args.extend(["--extra", *e]);
    }
    args.extend(["--out", "out", "--format", "json"]);
    let (code, stdout, stderr) = rulec_in(dir, "en", &args);
    assert_eq!(code, 0, "{stdout}{stderr}");
    let v = rulec::json::parse(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    let written: BTreeSet<String> = v.get("written").and_then(rulec::json::Json::as_arr).unwrap().iter().map(|p| p.as_str().unwrap().strip_prefix("out/").unwrap().to_string()).collect();
    // the places, worked out where the files are
    let asked = rulec::targets::Asked::new(targets, extras).unwrap();
    let paths: Vec<String> = names.iter().map(|n| dir.join(n).to_string_lossy().into_owned()).collect();
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    let placed = rulec::places::places(&paths, &asked);
    let named: BTreeSet<String> = placed.iter().filter(|(_, p)| p.kind == rulec::places::Kind::File).map(|(_, p)| p.name.clone()).collect();
    let unwritten: Vec<&String> = named.difference(&written).collect();
    assert!(unwritten.is_empty(), "named, and not written: {unwritten:?}");
    let module = |p: &str| p.starts_with("proto/buf.");
    let as_the_rule_asks = |p: &str| {
        let Some(name) = p.strip_prefix("vectors/") else { return false };
        [".refused.jsonl", ".owners.json", ".labels.", ".chances.", ".traces."].iter().any(|s| name.contains(s))
    };
    let unnamed: Vec<&String> = written.iter().filter(|p| !named.contains(*p) && !module(p) && !as_the_rule_asks(p)).collect();
    assert!(unnamed.is_empty(), "written, and not named: {unnamed:?}");
    for (from, p) in &placed {
        let text = || std::fs::read_to_string(dir.join("out").join(&p.file)).unwrap_or_else(|e| panic!("{from}: {}: {e}", p.file));
        match p.kind {
            rulec::places::Kind::GoPackage => assert!(dir.join("out").join(&p.name).is_dir() && p.file.starts_with(&format!("{}/", p.name)), "{from}: {p:?}"),
            rulec::places::Kind::PhpNamespace => assert!(text().lines().any(|l| l == format!("namespace {};", p.name)), "{from}: {p:?}"),
            rulec::places::Kind::RubyModule => assert!(text().lines().any(|l| l == format!("module {}", p.name)), "{from}: {p:?}"),
            rulec::places::Kind::File => {}
        }
    }
    // each kind is there to compare
    for kind in [rulec::places::Kind::GoPackage, rulec::places::Kind::PhpNamespace, rulec::places::Kind::RubyModule] {
        assert!(placed.iter().any(|(_, p)| p.kind == kind), "{kind:?}");
    }
}
