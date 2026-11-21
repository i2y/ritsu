//! A name of the rule's that the generated code has for one of its own — a record or a type of the
//! runner's, a builtin it calls, a name it imports, a column of the query's own, a local or a
//! parameter of its functions, a helper — no longer breaks the code: the code's own name is written
//! apart where the two meet, and the rule's names, which a caller reads, keep theirs (§15.336).
//!
//! Each of these broke the code. Java's runner reads the wire into the records `Str` and `Pairs`
//! and reads an enum's value with a method of a type `E`, and the walk of a pick by weight gives
//! back a record `Walked`: a rule of one of those names is a class of that name, which hid the
//! runner's, and the code did not compile. PHP looks for a function in the rule's namespace before
//! the builtin of its name, so a rule named `implode`, `array_sum`, `min` or `count` answered the
//! module's calls of the builtin. The SQL query hands back `_id` and `_input_error` beside the
//! rule's columns and keeps `_raw_<output>` for itself, and the PostgreSQL function and its runner
//! read a record `_r` and a subquery `t`: an input, a definition or an output of one of those names
//! was a second column, or a second name, of one spelling. SQLite gives no column of the names
//! `true` and `false` in a `WITH`, so an input or a definition of such a name was read as the
//! string of its name. The service of Connect imports `Mapping`, `E` and the like beside the class
//! it names after the rule, catches `ValueError` and `KeyError`, names the class may have, and read
//! a sequence whose name starts with `_` under an attribute protobuf does not give it. Java's
//! functions keep locals (`w`, `f`, `total`) a value of the name hid. Java's files import
//! `ArrayList`, `List` and `Map` of `java.util` and read `String`, `Long` and `Math` of `java.lang`
//! by those names: a rule's class of an imported name did not compile beside the import, its
//! runner's import hid the rule's class, and an enum's type of such a name, a type inside the
//! rule's class, hid the class of the JDK there. An `enums` file did the same in its module, by an
//! enum's type, and by its own name in the runner of its prerequisites and in the module and the
//! runner of a rule that reads its enums (`List.Zone`). Its enums hid builtins in the other
//! languages too: the module of TypeScript and JavaScript reads `Object.values`, Rust's hands back
//! an `Option` and a `Vec`, and Swift's enums take their values as `String` and answer with a
//! `Bool`, a type of which the runner reads as well, since the files of a Swift module share one
//! scope. A value named `java` hid the package Java's function that counts the answers reads
//! `java.util.Objects` from. And the helpers of the function that reads a caller's object (`_at`,
//! `_seq_at`, `_largest`) were declared beside the rule's function of their name: TypeScript and
//! JavaScript did not load the module, and Python and Ruby answered with the helper. `rulec test`
//! did not see those two, because its runner calls neither function.
//!
//! The materials are in `tests/apart/`, English first, with a Japanese version beside each, one for
//! each way a name met. The first test looks at the code for the names written apart, and at the
//! rule's names kept; the second writes every material in every language and runs it with
//! `rulec test`; the third calls the rule's own function and the function that reads the object,
//! in Python, Ruby and JavaScript, and they answer alike.

use ritsu_testkit::run::finish;
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, skip};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The materials, each English one with its Japanese version.
const PAIRS: [(&str, &str); 33] = [
    ("rule_named_like_a_runner_record_str", "runnerの型と同じ名前の規則_str"),
    ("rule_named_like_a_runner_record_pairs", "runnerの型と同じ名前の規則_pairs"),
    ("rule_named_e", "eという規則"),
    ("rule_named_like_the_walk_record", "歩きの結果の型と同じ名前の規則"),
    ("input_named_like_the_walk_local", "歩きの局所と同じ名前の入力"),
    ("input_named_like_a_record_local", "記録の関数の局所と同じ名前の入力"),
    ("rule_named_like_a_builtin_php_calls", "PHPが呼ぶ組み込みと同じ名前の規則"),
    ("rule_reading_an_object_named_like_a_builtin_php_calls", "オブジェクトを読むPHPが呼ぶ組み込みと同じ名前の規則"),
    ("rule_named_like_a_builtin_php_counts_with", "答えを数えるPHPが呼ぶ組み込みと同じ名前の規則"),
    ("lottery_named_like_a_builtin_php_counts_with", "答えを数えるPHPが呼ぶ組み込みと同じ名前のくじ"),
    ("input_named_like_a_chances_local", "答えを数える関数の局所と同じ名前の入力"),
    ("inputs_named_like_the_querys_columns", "問い合わせが返す列と同じ名前の入力"),
    ("definitions_named_like_query_columns", "問い合わせの列と同じ名前の定義"),
    ("input_named_like_the_functions_record", "関数の記録の変数と同じ名前の入力"),
    ("output_named_like_the_runners_subquery", "runnerの副問い合わせと同じ名前の出力"),
    ("input_and_definition_sqlite_gives_no_column", "SQLiteが列にしない名前の入力と定義"),
    ("definition_sqlite_gives_no_column", "SQLiteが列にしない名前の定義"),
    ("rule_named_like_a_service_import", "サービスが読み込む名前と同じ名前の規則"),
    ("sequence_with_a_leading_underscore", "下線で始まる並び"),
    ("rule_named_like_a_builtin_the_service_catches", "サービスが捕まえる組み込みと同じ名前の規則"),
    ("rule_named_like_a_class_java_imports", "Javaが読み込むクラスと同じ名前の規則"),
    ("rule_named_like_a_class_the_java_runner_imports", "Javaのrunnerが読み込むクラスと同じ名前の規則"),
    ("enums_named_like_classes_the_java_module_reads", "Javaのモジュールが読むクラスと同じ名前の列挙"),
    ("input_named_like_the_package_java_reads", "Javaが読むパッケージと同じ名前の入力"),
    ("enums_file_named_like_a_class_the_java_runner_imports", "Javaのrunnerが読み込むクラスと同じ名前のenumsファイル"),
    ("enum_of_an_enums_file_named_like_a_class_java_reads", "Javaが読むクラスと同じ名前のenumsファイルの列挙"),
    ("rule_reading_enums_files_named_like_classes_java_imports", "Javaが読み込むクラスと同じ名前のenumsファイルを読む規則"),
    ("enums_named_like_builtins_the_typescript_code_reads", "TypeScriptのコードが読む組み込みと同じ名前のenumsファイルの列挙"),
    ("enums_named_like_types_the_rust_code_reads", "Rustのコードが読む型と同じ名前のenumsファイルの列挙"),
    ("enums_named_like_types_the_swift_code_reads", "Swiftのコードが読む型と同じ名前のenumsファイルの列挙"),
    ("rule_aliased_like_the_helper_at", "補助関数と同じ別名の規則_at"),
    ("rule_aliased_like_the_helper_seq_at", "補助関数と同じ別名の規則_seq_at"),
    ("rule_aliased_like_the_helper_largest", "補助関数と同じ別名の規則_largest"),
];

/// The materials whose rule is named like a helper of the function that reads a caller's object.
/// Go keeps out of a package a file whose name starts with `_`, which W121 says; these are written
/// in every other language.
const HELPERS: usize = 3;

const TARGETS: [&str; 13] = ["python", "numpy", "typescript", "javascript", "rust", "ruby", "php", "go", "swift", "java", "sql", "wasm", "proto"];
const EXTRAS: [&str; 5] = ["runner", "mcp", "connect", "round-test", "vectors"];

/// The materials of one version, by their paths from the crate's root: the ones written in every
/// language, and the ones named like a helper.
fn materials(ja: bool) -> (Vec<String>, Vec<String>) {
    let path = |(en, j): &(&str, &str)| format!("tests/apart/{}.rule", if ja { j } else { en });
    let k = PAIRS.len() - HELPERS;
    (PAIRS[..k].iter().map(path).collect(), PAIRS[k..].iter().map(path).collect())
}

/// The code of `rules` in `targets`, with the extras of those targets, written into `out` as `rulec
/// gen` writes it. The library is called, so that the code of a target a finding names is written
/// too: what it looks like and how it runs is the test's to judge.
fn generate(rules: &[String], out: &Path, targets: &[&str]) {
    let paths: Vec<String> = rules.iter().map(|r| root().join(r).to_string_lossy().into_owned()).collect();
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    // The service of Connect is Python and reads the stubs of the `.proto`; the vectors are no
    // target's.
    let extras: Vec<&str> = EXTRAS
        .iter()
        .copied()
        .filter(|e| match *e {
            "connect" => targets.contains(&"python") && targets.contains(&"proto"),
            "vectors" => true,
            e => targets.iter().any(|t| rulec::targets::has_extra(t, e)),
        })
        .collect();
    let asked = rulec::targets::Asked::new(targets, &extras).unwrap();
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let code = rulec::i18n::with(rulec::i18n::Lang::En, || rulec::codegen::generate(&refs, out.to_str().unwrap(), &asked, false, false, &mut o, &mut e));
    assert_eq!(code, 0, "{}{}", String::from_utf8_lossy(&o), String::from_utf8_lossy(&e));
}

/// Every file under `dir`, with what it holds.
fn files(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(files(&p));
        } else if let Ok(s) = std::fs::read_to_string(&p) {
            out.push((p, s));
        }
    }
    out
}

/// Whether some file of the code of `lang` (its directory under `out`) holds `needle`.
fn holds(out: &Path, lang: &str, needle: &str) -> bool {
    files(&out.join(lang)).iter().any(|(_, s)| s.contains(needle))
}

/// What the code writes apart, in each language, and what it keeps: (the language's directory,
/// what some file of it holds).
const WRITTEN: &[(&str, &str)] = &[
    // Java: the runner's records and the type of its enum reader, the record of the walk, and the
    // rule's classes, which keep their names.
    ("java", "record Str_("),
    ("java", "final class Str "),
    ("java", "record Pairs_("),
    ("java", "<E_> E_ member("),
    ("java", "record Walked_("),
    // Java: the classes of the JDK a name of the rule's hides, by their full names, where the rule's
    // class is named like one its files import and where an enum's type is named like one the
    // module reads; the class and the enums keep their names.
    ("java", "public final class ArrayList {"),
    ("java", "new java.util.ArrayList<>()"),
    ("java", "Map<String, String> out = new java.util.LinkedHashMap<>();"),
    ("java", "public enum String {"),
    ("java", "private final java.lang.String wire;"),
    ("java", "java.lang.Math."),
    // The same in the module of an `enums` file and in the runner of its prerequisites, and in the
    // module and the runner of a rule that reads its enums from files named like classes they
    // import, whose classes keep their names.
    ("java", "throw new java.lang.IllegalArgumentException("),
    ("java", "PrintStreamRunner() {\n    }\n\n    public static void main(String[] args) throws IOException {\n        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));\n        java.io.PrintStream out = new java.io.PrintStream("),
    ("java", "public static long zoneFee(List.Zone zone, PrintStream.Course course) {"),
    ("java", "java.util.List<Fired> trace = new ArrayList<>();"),
    ("java", ", List.Zone::from);"),
    ("java", ", PrintStream.Course::from);"),
    // A value named `java` is bound apart where the code reads a class of the JDK by its full name:
    // where the class is named like one its files import, and in the function that counts the
    // answers.
    ("java", "public static long arrayList(long items, boolean java_) {"),
    ("java", "public static Prize prizeDraw(long ticket, boolean java_) {"),
    // The module of an `enums` file reaches a builtin past an enum's type of its name, and so does
    // the runner of its prerequisites in Swift, whose files share one scope; the enums keep their
    // names.
    ("typescript", "globalThis.Object.values(Object)"),
    ("typescript", "export const Object = {"),
    ("javascript", "globalThis.Object.values(Object)"),
    ("rust", "pub fn parse(s: &str) -> ::std::option::Option<Option> {"),
    ("rust", "-> ::std::vec::Vec<::std::vec::Vec<Result>> {"),
    ("rust", "pub enum Option {"),
    ("swift", "public enum String: Swift.String,"),
    ("swift", "-> Swift.Bool {"),
    ("swift", "String(rawValue: Swift.String(h))!"),
    // PHP: the builtins the module calls, from the global namespace; the rule's functions keep
    // their names.
    ("php", "\\implode("),
    ("php", "function implode("),
    ("php", "\\array_sum("),
    ("php", "\\min("),
    ("php", "\\count("),
    // SQL: the query's own columns beside the inputs `_id` and `_input_error`, the value held before
    // the rounding of an output beside a definition `_raw_fee`, the record of the PostgreSQL
    // function beside an input `_r`, the subquery of its runner beside an output `t`, and the inputs
    // and the definitions SQLite gives no column of their names.
    ("sql", "\"_id_\""),
    ("sql", "SELECT \"_id_\","),
    ("sql", "\"_input_error_\""),
    ("sql", "\"_raw_fee_\""),
    ("sql", "\"_r_\" record;"),
    ("sql", ") AS t_'"),
    ("sql", "\"TRUE_\""),
    // The service of Connect: the names it imports beside its class, the builtin it catches beside a
    // class of its name, which keeps it, and the attribute protobuf gives a sequence that starts
    // with `_`.
    ("python", "Mapping as Mapping_"),
    ("python", "E_ = TypeVar(\"E_\""),
    ("python", "import builtins\n"),
    ("python", "except builtins.ValueError:"),
    ("python", "class ValueError(ValueErrorService):"),
    ("python", "pb__parcels"),
    // The helpers of the function that reads a caller's object, beside the rule's function of
    // their name.
    ("python", "def _at_(o: Any, k: str, at: str)"),
    ("python", "def _seq_at_(o: Any, k: str, at: str)"),
    ("python", "def _largest_(vs: list[int], empty: int)"),
    ("python", "def _at(total: USD, top_fragile: USD"),
    ("typescript", "function _at_(o: unknown, k: string, at: string)"),
    ("typescript", "function _largest_(vs: bigint[], empty: bigint)"),
    ("javascript", "function _at_(o, k, at)"),
    ("ruby", "def self._at_(o, k, at)"),
    ("ruby", "private def self._at_: (untyped, String, String) -> untyped"),
    ("ruby", "def self._seq_at_(o, k, at)"),
    ("php", "function _at_("),
    // The functions that count the answers: Ruby binds an input named like a constant under `_` and
    // its name, and the local Swift counts in keeps clear of the parameter the input is there.
    ("ruby", "def self.total_lottery_chances(_Total)"),
    ("swift", "var total_: Int64 = 0"),
];

/// The code of every material, in each version, has each of its own names written apart where a
/// name of the rule's meets it, and the rule's names as they were.
#[test]
fn the_code_writes_its_own_names_apart_and_keeps_the_rules() {
    for ja in [false, true] {
        let (every, helpers) = materials(ja);
        let t = TempDir::new("written-apart");
        let out = t.path().join("out");
        let no_go: Vec<&str> = TARGETS.iter().copied().filter(|t| *t != "go").collect();
        generate(&every, &out, &TARGETS);
        for (k, h) in helpers.iter().enumerate() {
            generate(std::slice::from_ref(h), &out.join(format!("h{k}")), &no_go);
        }
        let missing: Vec<String> = WRITTEN
            .iter()
            .filter(|(lang, needle)| !holds(&out, lang, needle) && !(0..HELPERS).any(|k| holds(&out.join(format!("h{k}")), lang, needle)))
            .map(|(lang, needle)| format!("{lang}: {needle}"))
            .collect();
        assert!(missing.is_empty(), "the {} materials: the code holds none of these:\n{}", if ja { "Japanese" } else { "English" }, missing.join("\n"));
    }
}

/// How long `rulec test` may take over one directory of the materials.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(20 * 60);

/// `rulec test` over `out`: every result that ran is right, and what it could not run is a SKIP line.
/// Returns the languages that ran.
fn runs_alike(out: &Path, what: &str) -> Vec<String> {
    let tmp = tmpdir_in(out);
    let o = finish(
        Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", &tmp).args(["test", out.to_str().unwrap(), "--format", "json"]),
        RULEC_LIMIT,
        &format!("rulec test {what}"),
    );
    let j = rulec::json::parse(o.stdout.lines().next().unwrap_or_else(|| panic!("{what}: no result\n{}", o.stderr))).unwrap_or_else(|x| panic!("{what}: {x}: {}", o.stdout));
    for why in j.get("skipped").and_then(Json::as_arr).unwrap_or_default() {
        skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
    }
    let results = j.get("results").and_then(Json::as_arr).unwrap_or_default();
    let field = |r: &Json, k: &str| r.get(k).and_then(Json::as_str).unwrap_or_default().to_string();
    let broken: Vec<String> = results
        .iter()
        .filter(|r| r.get("ok").and_then(|v| v.as_bool()) != Some(true))
        .map(|r| format!("{} {}/{}: {}", field(r, "rule"), field(r, "lang"), field(r, "via"), field(r, "error")))
        .collect();
    assert!(broken.is_empty(), "{what}: these did not build or did not answer alike:\n{}", broken.join("\n"));
    let mut ran: Vec<String> = results.iter().filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)).map(|r| field(r, "lang")).collect();
    ran.sort();
    ran.dedup();
    ran
}

fn have(cmd: &str, arg: &str) -> bool {
    Command::new(cmd).arg(arg).output().is_ok_and(|o| o.status.success())
}

/// The code of every material builds and answers as the reference evaluator does, in every language
/// that is here: the materials of each version in one directory, and each one named like a helper
/// in one of its own, since they share their aliases' files with nothing but their rule's name.
#[test]
fn every_material_builds_and_answers_alike_in_every_language() {
    if !ready(Need::Python, || have("python3", "--version"), "python3 is not here") {
        return;
    }
    for ja in [false, true] {
        let version = if ja { "Japanese" } else { "English" };
        let (every, helpers) = materials(ja);
        let t = TempDir::new("written-apart-run");
        let out = t.path().join("every");
        generate(&every, &out, &TARGETS);
        let ran = runs_alike(&out, &format!("the {version} materials"));
        println!("written_apart: the {version} materials ran alike in {}", ran.join(", "));
        let no_go: Vec<&str> = TARGETS.iter().copied().filter(|t| *t != "go").collect();
        // Each in a directory named by its place: the compiler of Swift does not start where the
        // temporary directory `rulec test` is handed has a name in Japanese.
        for (k, h) in helpers.iter().enumerate() {
            let out = t.path().join(format!("h{k}"));
            generate(std::slice::from_ref(h), &out, &no_go);
            let ran = runs_alike(&out, h);
            println!("written_apart: {h} ran alike in {}", ran.join(", "));
        }
    }
}

/// An order with a fragile line and a line of many pieces, as the caller hands it in, and the same
/// order as the rule's own function takes it: the total, the largest fragile amount and the lines.
const ORDER: &str = r#"{"lines": [{"quantity": 2, "amount_usd": 1500, "fragile": true}, {"quantity": 60, "amount_usd": 3000, "fragile": false}]}"#;

/// What one language says when it calls the rule's own function and the function that reads the
/// object: both answers, or the name of what each raised.
fn called(lang: &str, dir: &Path, alias: &str) -> Option<(String, String)> {
    let (cmd, args): (&str, Vec<String>) = match lang {
        "python" => {
            let script = format!(
                "import importlib.util, json\n\
                 spec = importlib.util.spec_from_file_location('m', '{alias}.py')\n\
                 m = importlib.util.module_from_spec(spec)\n\
                 spec.loader.exec_module(m)\n\
                 def call(f):\n    try:\n        return repr(f())\n    except Exception as e:\n        return type(e).__name__\n\
                 print(call(lambda: getattr(m, '{alias}')(4500, 1500, [m.Element(2, 1500), m.Element(60, 3000)])))\n\
                 print(call(lambda: getattr(m, '{alias}_from')(json.loads('{ORDER}'))))\n"
            );
            ("python3", vec!["-c".into(), script])
        }
        "ruby" => {
            let src = std::fs::read_to_string(dir.join(format!("{alias}.rb"))).ok()?;
            let module = src.lines().find_map(|l| l.strip_prefix("module ").map(|m| m.trim().to_string()))?;
            let script = format!(
                "require 'json'\nrequire_relative '{alias}'\n\
                 def call; yield.inspect; rescue StandardError => e; e.class.name; end\n\
                 puts(call {{ {module}.send(:{alias}, 4500, 1500, [{module}::Element.new(quantity: 2, amount: 1500), {module}::Element.new(quantity: 60, amount: 3000)]) }})\n\
                 puts(call {{ {module}.send(:{alias}_from, JSON.parse('{ORDER}')) }})\n"
            );
            ("ruby", vec!["-e".into(), script])
        }
        _ => {
            let script = format!(
                "import * as m from './{alias}.mjs';\n\
                 const call = (f) => {{ try {{ return String(f()); }} catch (e) {{ return e.constructor.name; }} }};\n\
                 console.log(call(() => m['{alias}'](4500n, 1500n, [{{ quantity: 2n, amount: 1500n }}, {{ quantity: 60n, amount: 3000n }}])));\n\
                 console.log(call(() => m['{alias}_from'](JSON.parse('{ORDER}'))));\n"
            );
            std::fs::write(dir.join("call.mjs"), script).ok()?;
            ("node", vec!["call.mjs".into()])
        }
    };
    let o = Command::new(cmd).args(&args).current_dir(dir).output().ok()?;
    let said = String::from_utf8_lossy(&o.stdout).to_string();
    let mut lines = said.lines();
    Some((lines.next()?.to_string(), lines.next()?.to_string()))
}

/// The rule's own function and the function that reads a caller's object answer alike for a rule
/// named like a helper of the reading function, in Python, Ruby and JavaScript: the runner of
/// `rulec test` calls neither, and the helper had answered in the rule's place.
#[test]
fn the_rules_function_and_the_reading_function_answer_alike() {
    let langs: Vec<&str> = [("python", Need::Python, "python3"), ("ruby", Need::Ruby, "ruby"), ("javascript", Need::Node, "node")]
        .into_iter()
        .filter(|(lang, need, cmd)| ready(*need, || have(cmd, "--version"), &format!("{cmd} is not here; the {lang} functions are not called")))
        .map(|(lang, _, _)| lang)
        .collect();
    if langs.is_empty() {
        return;
    }
    let mut called_any = 0;
    for ja in [false, true] {
        let (_, helpers) = materials(ja);
        for h in &helpers {
            let src = std::fs::read_to_string(root().join(h)).unwrap();
            let alias = src.lines().next().and_then(|l| l.split_once('(')).and_then(|(_, r)| r.split_once(')')).map(|(a, _)| a.to_string()).unwrap();
            let t = TempDir::new("written-apart-call");
            generate(std::slice::from_ref(h), t.path(), &langs);
            for lang in &langs {
                let (plain, read) = called(lang, &t.path().join(lang), &alias).unwrap_or_else(|| panic!("{h}: {lang} said nothing"));
                assert!(plain.chars().all(|c| c.is_ascii_digit()) && !plain.is_empty(), "{h}: {lang}: the rule's function gave {plain}");
                assert_eq!(plain, read, "{h}: {lang}: the rule's function and the function that reads the order answer otherwise");
                called_any += 1;
            }
        }
    }
    println!("written_apart: the two functions answered alike {called_any} times");
}
