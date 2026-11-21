//! The names an enum's code takes beside the rule's own, in the languages whose module puts the two
//! in one place (§15.326).
//!
//! A rule named like the enum it answers, like a value of it, or like a function the code writes
//! for it had a module where one name was declared twice. Go's package of `rule band` declared
//! `func Band` and `type Band`, and Java nested `enum Band` in the class `Band`; Python's module of
//! `rule band_label` declared the display names' `def band_label` and then the rule's, which hid
//! the first; TypeScript's module of `rule tierLabel` declared `tierLabel` twice. An enum read from
//! an `enums` file met the rule's functions in the names the module hands on (`size_rank`,
//! `sizeLabel`, `ParseSize`), and an enum an `apply` brings in met them as the rule's own did.
//! Swift builds a rule into one module with the modules of the `enums` files it reads, and there an
//! enum of one file met an enum of another, the enum an `apply` brought in, one the rule declares,
//! or the rule's `Fired`. `check` warned of the Go and Java forms of a rule's own enum (W121) and
//! said nothing of the others, and `gen` wrote them all.
//!
//! In a language where the names meet, and only there, the enum's type now takes the first name
//! from `<type>2` that meets nothing, and every name made of the type follows it; the rule's
//! functions keep theirs, since they are what a caller calls. A rule whose Swift module would
//! declare one name twice declares the enums it reads itself, and is built without the modules of
//! their files. Code where nothing meets is written as it was. `check` says each enum whose type
//! takes a number with W121: the type it takes in each language, and the two names that met, which
//! keeps no target from being written (§15.337).
//!
//! The materials are in `tests/same_names`, English first, with Japanese versions of three forms
//! beside them: one rule for each way the names meet, for each of the three places an enum comes
//! from (the rule, an `enums` file, an `apply`), one for each way a Swift module meets, and one
//! where nothing meets.

use ritsu_testkit::run::finish;
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, skip};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/same_names")
}

/// How long a run of rulec may take: `rulec test` builds and runs the code of every language here.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(20 * 60);

fn rulec(cwd: &Path, tmp: &Path, args: &[&str]) -> (i32, String, String) {
    let o = finish(Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp).current_dir(cwd).args(args), RULEC_LIMIT, &format!("rulec {}", args.join(" ")));
    (o.code.unwrap_or(-1), o.stdout, o.stderr)
}

/// Every `.rule` file here, by its name.
fn materials() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(here())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rule"))
        .collect();
    out.sort();
    out
}

/// Every rule here generated into `out` with `flags`, or `names` only when given. The output of
/// the run, for what it stops.
fn generate_names(out: &Path, flags: &[&str], names: &[String]) -> String {
    let mut args: Vec<&str> = vec!["gen"];
    args.extend(names.iter().map(String::as_str));
    args.extend(["--out", out.to_str().unwrap()]);
    args.extend(flags);
    let (code, said, e) = rulec(&here(), &tmpdir_in(out), &args);
    assert_eq!(code, 0, "{said}{e}");
    format!("{said}{e}")
}

/// Every rule here generated into `out` with `flags`. A run that E153 stops answers its warnings
/// and exit 1: the output the targets it names is not written, the rest is.
fn generate(out: &Path, flags: &[&str]) -> String {
    let names = materials();
    let mut args: Vec<&str> = vec!["gen"];
    args.extend(names.iter().map(String::as_str));
    args.extend(["--out", out.to_str().unwrap()]);
    args.extend(flags);
    let (code, said, e) = rulec(&here(), &tmpdir_in(out), &args);
    assert!(code == 0 || (code == 1 && said.contains("E153")), "{code}: {said}{e}");
    format!("{said}{e}")
}

fn read(out: &Path, rel: &str) -> String {
    std::fs::read_to_string(out.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// What the code of each language says, where the names of an enum meet the rule's and where they
/// do not: (the generated file, what it says, what it does not say).
const SAYS: &[(&str, &[&str], &[&str])] = &[
    // The rule named like the enum it answers: Go's function and type, Java's class and nested enum.
    // Python spells the two apart and writes them as it did.
    ("go/band/band.go", &["type Band2 int", "\tBand2Child Band2 = iota", "func Band(in Input) (Output, error) {"], &["type Band int"]),
    ("java/Band.java", &["public final class Band {", "    public enum Band2 {"], &["public enum Band {"]),
    ("python/band.py", &["class Band(enum.Enum):", "def band(age: int) -> Output:"], &["Band2"]),
    // The Japanese version, under its alias
    ("go/bandja/band_ja.go", &["type BandJa2 int", "func BandJa(in Input) (Output, error) {"], &["type BandJa int"]),
    // Named like a function of the display names, of the parser, and like a value
    ("python/band_label.py", &["def band2_label(x: Band2) -> str:", "def band_label(age: int) -> Output:"], &["def band_label(x:"]),
    ("go/parseband/parse_band.go", &["func ParseBand2(s string) (Band2, bool)", "func ParseBand(in Input) (Output, error) {"], &[]),
    ("go/bandchild/band_child.go", &["\tBand2Child Band2 = iota", "func BandChild(in Input) (Output, error) {"], &[]),
    // In another case: TypeScript's function and type, and its display names
    ("typescript/Tier.ts", &["export const Tier2 = {", "export function Tier("], &["export const Tier = {"]),
    ("python/Tier.py", &["class Tier2(enum.Enum):", "def Tier(age: int) -> Output:"], &[]),
    ("typescript/tierLabel.ts", &["export function tier2Label(", "export function tierLabel("], &[]),
    // Read from an `enums` file: the module imports the owner's names and hands them on under its own
    ("python/size_rank.py", &["from shapes import Size as Size2, size_rank as size2_rank, size_label as size2_label"], &[]),
    ("python/size_ja_rank.py", &["from choices_ja import SizeJa as SizeJa2, size_ja_rank as size_ja2_rank, size_ja_label as size_ja2_label"], &[]),
    ("typescript/sizeLabel.ts", &["import { Size as Size2, sizeRank as size2Rank } from \"./shapes.ts\";", "export { sizeRank as size2Rank, sizeLabel as size2Label } from \"./shapes.ts\";"], &[]),
    ("javascript/sizeLabel.mjs", &["export { sizeRank as size2Rank, sizeLabel as size2Label } from \"./shapes.mjs\";"], &["export { sizeRank, sizeLabel }"]),
    ("go/parsesize/parse_size.go", &["type Size2 = shapes.Size", "func ParseSize2(s string) (Size2, bool) { return shapes.ParseSize(s) }", "func ParseSize(in Input) (Output, error) {"], &[]),
    ("go/sizesmall/size_small.go", &["\tSize2Small = shapes.SizeSmall"], &[]),
    // Brought in by an apply
    ("go/authoritydecider/authority_decider.go", &["type AuthorityDecider2 int", "func AuthorityDecider(in Input) (bool, error) {"], &[]),
    ("java/AuthorityDecider.java", &["    public enum AuthorityDecider2 {"], &[]),
    ("python/authority_decider_label.py", &["def authority_decider2_label(x: AuthorityDecider2) -> str:"], &[]),
    ("typescript/parseAuthorityDecider.ts", &["export function parseAuthorityDecider2(s: string): AuthorityDecider2 {", "export function parseAuthorityDecider("], &[]),
    // Nothing meets: the callee and a rule reading an `enums` file are written as they were
    ("go/authority/authority.go", &["type Decider int", "func Authority(in Input) (Decider, error) {"], &["Decider2"]),
    ("python/east.py", &["from places import Region as Region"], &["Region2"]),
    // Swift: a rule declares the enums it reads where its module would declare one name twice —
    // which is what E153 now stops (SWIFT_REFUSED, §15.335) — and is built with the modules of
    // their files where it would not
    ("swift/east.swift", &[], &["public enum Region"]),
    ("swift/size_rank.swift", &[], &["public enum Size"]),
];

/// The rules whose Swift is built with the modules of the `enums` files it reads, and those whose
/// Swift `gen` refuses: each redeclares a name another file of the package declares (a standalone
/// file was the answer until §15.345), so its own W121 names `swift` and the target is not
/// written for the run.
const SWIFT_WITH_OWNERS: &[&str] = &["east", "size_rank", "sizeLabel", "size_small", "parse_size", "Kind", "size_ja_rank"];
/// (file stem, the Swift module's name for it)
const SWIFT_REFUSED: &[(&str, &str)] = &[("approval", "approval"), ("first", "first"), ("far", "far"), ("lamp", "lamp"), ("止まるか", "lamp_ja")];

/// Where the names of an enum meet the rule's in a language, the enum's type takes a number there
/// and nowhere else, and every name made of it follows; where nothing meets, the code is written
/// as it was. Swift declares what a rule reads itself only where its module would hold one name
/// twice. `rulec api` says the names of each language as its code has them.
#[test]
fn the_type_takes_a_number_only_in_the_language_where_its_names_meet() {
    let t = TempDir::new("same-names-gen");
    let out = t.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    let said = generate(
        &out,
        &["--target", "python", "--target", "typescript", "--target", "javascript", "--target", "go", "--target", "java", "--target", "swift", "--extra", "runner", "--extra", "vectors"],
    );
    // The rules the package's Swift cannot hold: E153 names them and the target is not written.
    for name in ["AuthorityDecider", "Kind", "Region", "FirstBy", "Fired", "Light", "LightJa"] {
        assert!(said.contains(&format!("`{name}` is declared twice in the code generated for Swift")), "{name}: {said}");
    }
    assert!(!out.join("swift").exists(), "the stopped target is not written");
    let mut failures: Vec<String> = Vec::new();
    for (rel, says, not) in SAYS.iter().filter(|(rel, _, _)| !rel.starts_with("swift/")) {
        let text = read(&out, rel);
        failures.extend(says.iter().filter(|s| !text.contains(**s)).map(|s| format!("{rel} does not say {s:?}")));
        failures.extend(not.iter().filter(|s| text.contains(**s)).map(|s| format!("{rel} says {s:?}")));
    }
    // The Swift the rules it refuses aside writes, generated beside the ones it cannot take:
    // a rule's module is still built with the modules of the `enums` files it reads.
    let sw = t.path().join("out-sw");
    std::fs::create_dir_all(&sw).unwrap();
    let refused: Vec<&str> = SWIFT_REFUSED.iter().map(|(stem, _)| *stem).collect();
    let fine: Vec<String> = materials().into_iter().filter(|n| !refused.contains(&n.trim_end_matches(".rule"))).collect();
    generate_names(&sw, &["--target", "swift", "--extra", "runner", "--extra", "vectors"], &fine);
    for (rel, says, not) in SAYS.iter().filter(|(rel, _, _)| rel.starts_with("swift/")) {
        let text = read(&sw, rel);
        failures.extend(says.iter().filter(|s| !text.contains(**s)).map(|s| format!("{rel} does not say {s:?}")));
        failures.extend(not.iter().filter(|s| text.contains(**s)).map(|s| format!("{rel} says {s:?}")));
    }
    for alias in SWIFT_WITH_OWNERS {
        if !sw.join(format!("vectors/{alias}.owners.json")).exists() {
            failures.push(format!("{alias}: no owners.json, so its Swift is built without the modules of the files it reads"));
        }
    }
    for (stem, _) in SWIFT_REFUSED {
        if !said.contains(&format!("--> {stem}.rule")) {
            failures.push(format!("{stem}: no E153 names its file in the run's output"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    // `rulec api` takes each language's names from that language's code
    let (code, api, e) = rulec(&here(), &tmpdir_in(&out), &["api", "band.rule"]);
    assert_eq!(code, 0, "{e}");
    let j = rulec::json::parse(api.trim()).unwrap();
    let enum_alias = |lang: &str| -> String {
        let e = j.get(lang).and_then(|l| l.get("enums")).and_then(Json::as_arr).and_then(|a| a.first().cloned()).unwrap_or_else(|| panic!("{lang}: no enums in {api}"));
        e.get("alias").and_then(Json::as_str).unwrap().to_string()
    };
    for (lang, alias) in [("go", "Band2"), ("java", "Band2"), ("python", "Band"), ("typescript", "Band"), ("rust", "Band"), ("swift", "Band")] {
        assert_eq!(enum_alias(lang), alias, "{lang}: {api}");
    }
    let go_sig = j.get("go").and_then(|g| g.get("signature")).and_then(Json::as_str).unwrap();
    assert_eq!(go_sig, "func Band(in Input) (Output, error)", "{api}");
}

fn found(cmd: &str, arg: &str) -> bool {
    Command::new(cmd).arg(arg).output().is_ok_and(|o| o.status.success())
}

/// The code of every rule here builds and answers as the reference evaluator does, in every
/// language that is here, Swift with each rule built as the rule's file says.
#[test]
fn every_rule_here_runs_in_every_language() {
    if !ready(Need::Python, || found("python3", "--version"), "python3 is not here") {
        return;
    }
    let t = TempDir::new("same-names-run");
    let out = t.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    generate(&out, rulec::targets::UP_TO_0_40);
    let (_, said, e) = rulec(&here(), &tmpdir_in(&out), &["test", out.to_str().unwrap(), "--format", "json"]);
    let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result\n{e}"))).unwrap_or_else(|x| panic!("{x}: {said}"));
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
    assert!(broken.is_empty(), "these did not run or did not agree:\n{}", broken.join("\n"));
    // the rules each language ran, each once however many ways it ran it, less the language's
    // test of its rounding (`_round`)
    let ran = |lang: &str| {
        let mut rules: Vec<String> = results
            .iter()
            .filter(|r| field(r, "lang") == lang && field(r, "rule") != "_round" && r.get("ran").and_then(|v| v.as_bool()) == Some(true))
            .map(|r| field(r, "rule"))
            .collect();
        rules.sort();
        rules.dedup();
        rules.len()
    };
    // the languages where names meet, each where it is here
    let langs: [(Need, &str, &str, &str); 8] = [
        (Need::Python, "python", "python3", "--version"),
        (Need::Node, "typescript", "node", "--version"),
        (Need::Node, "javascript", "node", "--version"),
        (Need::Go, "go", "go", "version"),
        (Need::Java, "java", "javac", "-version"),
        (Need::Ruby, "ruby", "ruby", "--version"),
        (Need::Php, "php", "php", "--version"),
        (Need::Swift, "swift", "swift", "--version"),
    ];
    // every file here but the `enums` files, which decide nothing and are not run
    let rules = materials().iter().filter(|n| !std::fs::read_to_string(here().join(n)).unwrap().starts_with("enums ")).count();
    for (need, lang, cmd, arg) in langs {
        if ready(need, || found(cmd, arg), &format!("{cmd} is not here")) {
            // Swift is refused for the whole run: SWIFT_REFUSED's files cannot go in one module
            // with the enums they read, and the target's code cannot be taken (E153, §15.335).
            let want = if lang == "swift" { 0 } else { rules };
            assert_eq!(ran(lang), want, "{lang} ran {} of the {want} rules: {said}", ran(lang));
        }
    }
    println!("same_names: every rule here ran alike in {}", {
        let mut l: Vec<String> = results.iter().filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)).map(|r| field(r, "lang")).collect();
        l.sort();
        l.dedup();
        l.join(", ")
    });
}
