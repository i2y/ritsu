//! Which values of an enum need which before them (§15.222): the `prerequisites` block.
//!
//! What the corpus-wide suites already hold of the rules of the corpus that have one — their
//! vectors and functions in every language (tests/threeway.rs), their certificates in both
//! re-checkers (tests/cert.rs, tests/lean.rs), the mutants' codes (tests/m0.rs) and the goldens —
//! is not repeated here. These are the rest: the words the block reads by place, the shapes E071
//! stops, the formatter, an `enums` file whose module holds the functions, a rule whose block is
//! over an enum it reads from one, what `rulec gen` and `rulec api` say of the targets that have no
//! room for the functions, a certificate that lies, and the pages.

use std::path::{Path, PathBuf};
use std::process::Command;
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, skip};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("cannot start rulec");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn run(args: &[&str]) -> (i32, String, String) {
    run_in(&root(), args)
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn json(text: &str) -> rulec::json::Json {
    rulec::json::parse(text.trim()).unwrap_or_else(|e| panic!("not JSON: {e}\n{text}"))
}

/// The codes `check` prints for a rule written into `dir`, each once, sorted.
fn codes_of(dir: &Path, name: &str, src: &str) -> Vec<String> {
    std::fs::write(dir.join(name), src).unwrap();
    let (_, out, _) = run_in(dir, &["check", name, "--format", "json"]);
    let mut v: Vec<String> = out.lines().filter_map(|l| rulec::json::parse(l).ok()).filter_map(|j| j.get("code").and_then(|c| c.as_str()).map(String::from)).collect();
    v.sort();
    v.dedup();
    v
}

const COURSES: &str = "tests/corpus/course_catalog.rule";

/// The rules of the corpus with a block, and the files of enums with one, pass with nothing said:
/// an enum no row names, and the column of the table `ordered by` reads, are the block's (no W111).
#[test]
fn the_corpus_with_prerequisites_passes_with_nothing_said() {
    for rel in [
        COURSES,
        "tests/corpus/科目の履修.rule",
        "tests/corpus/exam_fee.rule",
        "tests/corpus/受験料.rule",
        "tests/corpus/shared/certifications.rule",
        "tests/corpus/shared/資格.rule",
    ] {
        let (c, out, _) = run(&["check", rel]);
        assert_eq!(c, 0, "{rel}:\n{out}");
        assert_eq!(out.trim(), format!("ok {rel}"), "{rel} says more than ok:\n{out}");
    }
    // An enum that only a block is over, and a table only `ordered by` reads.
    let t = TempDir::new("prereq-w111");
    let src = "rule t(t) v1\n\nenum k(k) = a(a) | b(b)\n\ninputs\n  x(x) : bool\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| x | -> r(r) : bool |\n| - | true           |\n\nprerequisites p(p) over k\n  start a\n  b     requires a\n";
    assert!(codes_of(t.path(), "only.rule", src).is_empty(), "an enum only a block is over is said to be unused");
}

/// None of the block's words is reserved (§15.157): a value may be called `start`, `requires`,
/// `ordered` or `nothing`, and a declaration `prerequisites`. Each is read by its place.
#[test]
fn the_words_of_the_block_are_read_by_their_place() {
    let t = TempDir::new("prereq-words");
    let src = "rule t(t) v1\n\nenum k(k) = start(start) | requires(requires) | ordered(ordered) | nothing(nothing) | prerequisites(prerequisites)\n\ninputs\n  prerequisites(p) : k\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| prerequisites | -> r(r) : bool |\n| -             | true           |\n\nprerequisites tree(tree) over k\n  start         nothing\n  start         requires nothing\n  requires      requires nothing or start\n  ordered       requires start, requires\n  prerequisites requires nothing \"kept out\"\n";
    let codes = codes_of(t.path(), "words.rule", src);
    assert!(codes.is_empty(), "the words read by place are taken for something else: {codes:?}");
    let (_, api, _) = run_in(t.path(), &["api", "words.rule"]);
    let j = json(&api);
    let p = j.get("prerequisites").and_then(|p| p.as_arr()).and_then(|a| a.first()).expect("no prerequisites in the api");
    assert_eq!(rulec::json::unparse(p.get("start").unwrap()), "[\"nothing\"]");
    assert_eq!(rulec::json::unparse(p.get("requires").unwrap()), "{\"ordered\":[[\"start\"],[\"requires\"]],\"requires\":[[\"nothing\",\"start\"]],\"start\":[[\"nothing\"]]}");
    assert_eq!(rulec::json::unparse(p.get("never").unwrap()), "{\"prerequisites\":\"kept out\"}");
}

/// The shapes E071 stops, each alone: the parse's and the check's.
#[test]
fn a_misshapen_block_is_e071() {
    let t = TempDir::new("prereq-e071");
    let head = "rule t(t) v1\n\nenum k(k) = a(a) | b(b) | c(c)\nenum s(s) = one(one) < two(two)\n\ninputs\n  x(x) : k\n  y(y) : bool\n\noutputs\n  r(r) : s\n\ntable j(j)\npolicy unique\n| x    | -> r(r) : s |\n| a, c | one         |\n| b    | two         |\n\ntable two_cols(two_cols)\npolicy unique\n| x | y | -> z(z) : s |\n| - | - | one         |\n\n";
    let cases: &[(&str, &str)] = &[
        ("no over", "prerequisites p(p)\n  start a\n  b requires a\n  c requires a\n"),
        ("over a type that is no enum", "prerequisites p(p) over bool\n  start a\n"),
        ("no start line", "prerequisites p(p) over k\n  b requires a\n  c requires a\n  a requires b\n"),
        ("a value twice", "prerequisites p(p) over k\n  start a, b\n  b requires a\n  c requires a\n"),
        ("a name that is no value", "prerequisites p(p) over k\n  start a\n  b requires d\n  c requires a\n"),
        ("a value with no line", "prerequisites p(p) over k\n  start a\n  b requires a\n"),
        ("nothing after requires", "prerequisites p(p) over k\n  start a\n  b requires\n  c requires a\n"),
        ("an empty reason", "prerequisites p(p) over k\n  start a\n  b requires nothing \"\"\n  c requires a\n"),
        ("an empty side of or", "prerequisites p(p) over k\n  start a\n  b requires a or\n  c requires a\n"),
        ("a line that reads as nothing", "prerequisites p(p) over k\n  start a\n  b needs a\n  c requires a\n"),
        ("two start lines", "prerequisites p(p) over k\n  start a\n  start b\n  c requires a\n"),
        ("an order by no table", "prerequisites p(p) over k\n  start a\n  b requires a\n  c requires a\n  ordered by missing\n"),
        ("an order by a table of two columns", "prerequisites p(p) over k\n  start a\n  b requires a\n  c requires a\n  ordered by two_cols\n"),
    ];
    for (what, block) in cases {
        let codes = codes_of(t.path(), "m.rule", &format!("{head}{block}"));
        assert!(codes.contains(&"E071".to_string()), "{what}: no E071 in {codes:?}");
        assert!(codes.iter().all(|c| c == "E071" || c.starts_with('W')), "{what}: more than E071: {codes:?}");
    }
    // In an `enums` file a block takes no `ordered by`: there is no table to order by.
    let src = "enums e(e) v1\n\nenum k(k) = a(a) | b(b)\n\nprerequisites p(p) over k\n  start a\n  b     requires a\n  ordered by j\n";
    assert_eq!(codes_of(t.path(), "e.rule", src), vec!["E071".to_string()]);
}

/// The block's name names functions, so it takes an ASCII alias where it has none, and two
/// blocks of one file have two names.
#[test]
fn a_block_needs_an_alias_and_a_name_of_its_own() {
    let t = TempDir::new("prereq-names");
    let base = "rule t(t) v1\n\nenum k(k) = a(a) | b(b)\n\ninputs\n  x(x) : k\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| x | -> r(r) : bool |\n| - | true           |\n\n";
    let block = |name: &str| format!("prerequisites {name} over k\n  start a\n  b     requires a\n");
    assert_eq!(codes_of(t.path(), "a.rule", &format!("{base}{}", block("履修"))), vec!["E011".to_string()]);
    assert!(codes_of(t.path(), "b.rule", &format!("{base}{}", block("履修(enrollment)"))).is_empty());
    assert_eq!(codes_of(t.path(), "c.rule", &format!("{base}{}\n{}", block("p"), block("p"))), vec!["E034".to_string()]);
}

/// `rulec fmt` aligns the block — the values and `start` padded so that `requires` and the start
/// values line up, a list's comma against the word before it, a comment two spaces off — and
/// leaves what it wrote as it is.
#[test]
fn fmt_aligns_the_block() {
    let t = TempDir::new("prereq-fmt");
    let src = std::fs::read_to_string(root().join(COURSES)).unwrap();
    let messy = src
        .replace("  start             intro_programming, discrete_math, calculus", " start intro_programming ,discrete_math,calculus")
        .replace("  statistics        requires calculus", "  statistics requires   calculus  # first year")
        .replace("  ordered by level_of", "    ordered   by   level_of");
    std::fs::write(t.path().join("c.rule"), &messy).unwrap();
    let (c, _, e) = run_in(t.path(), &["fmt", "c.rule"]);
    assert_eq!(c, 0, "{e}");
    let got = std::fs::read_to_string(t.path().join("c.rule")).unwrap();
    assert_eq!(got, src.replace("  statistics        requires calculus", "  statistics        requires calculus  # first year"));
    let (c, out, _) = run_in(t.path(), &["fmt", "c.rule", "--check"]);
    assert_eq!(c, 0, "the formatted block is not formatted: {out}");
    let (c, out, _) = run(&["fmt", COURSES, "tests/corpus/科目の履修.rule", "tests/corpus/shared/certifications.rule", "--check"]);
    assert_eq!(c, 0, "{out}");
}

/// The graph a block is read as, and the two functions over it: what a value needs, and whether it
/// can be taken with what is held — the answers every language is held to.
#[test]
fn the_reference_answers_what_a_value_needs_and_whether_it_can_be_taken() {
    let src = std::fs::read_to_string(root().join(COURSES)).unwrap();
    let (f, c) = rulec::prepare(&src, COURSES).map_err(|_| "does not prepare").unwrap();
    let g = rulec::prereq::Graph::all(&f, &c).remove(0);
    let v = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(g.requires("calculus"), Vec::<Vec<String>>::new());
    assert_eq!(g.requires("machine_learning"), vec![v(&["algorithms"]), v(&["statistics", "linear_algebra"])]);
    assert_eq!(g.requires("mainframe_cobol"), vec![Vec::<String>::new()]);
    assert!(g.available("calculus", &[]));
    assert!(!g.available("calculus", &v(&["calculus"])));
    assert!(g.available("machine_learning", &v(&["algorithms", "linear_algebra"])));
    assert!(!g.available("machine_learning", &v(&["algorithms"])));
    assert!(!g.available("mainframe_cobol", &g.values.iter().filter(|x| *x != "mainframe_cobol").cloned().collect::<Vec<_>>()));
    assert_eq!(g.reached().len(), 13);
    let topo = g.topological().expect("no order");
    let at = |x: &str| topo.iter().position(|y| y == x).unwrap();
    for (a, b) in g.edges() {
        assert!(at(&b) < at(&a), "{b} comes after {a}");
    }
}

/// `rulec gen` names the targets that get no functions, and why; `rulec api` gives the functions
/// in each language that has them and says which do not.
#[test]
fn gen_and_api_say_where_the_functions_are() {
    let t = TempDir::new("prereq-gen");
    let out = t.path().join("gen");
    let (c, said, e) = run(&["gen", COURSES, "--out", out.to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    for name in ["SQL", "Wasm"] {
        assert!(said.contains(&format!("{name}: the functions of `prerequisites` are not generated for this target: it has one entry, and no room for a second function")), "{said}");
    }
    assert!(!said.contains("NumPy: the functions"), "{said}");
    let tsv = std::fs::read_to_string(out.join("vectors/course_catalog.prereq.tsv")).unwrap();
    let expected = std::fs::read_to_string(out.join("vectors/course_catalog.prereq.expected")).unwrap();
    assert_eq!(tsv.lines().count(), expected.lines().count());
    assert!(tsv.contains("enrollment\tmachine_learning\tlinear_algebra,algorithms\n"), "{tsv}");
    assert!(expected.contains("true\t[algorithms][statistics|linear_algebra]\n"), "{expected}");
    let py = std::fs::read_to_string(out.join("python/course_catalog.py")).unwrap();
    assert!(py.contains("def enrollment_requires(value: Course) -> list[list[Course]]:"), "{py}");
    assert!(py.contains("def enrollment_available(value: Course, held: list[Course]) -> bool:"), "{py}");
    assert!(!std::fs::read_to_string(out.join("sql/course_catalog.sql")).unwrap().contains("enrollment"));

    let (c, api, _) = run(&["api", COURSES]);
    assert_eq!(c, 0);
    let j = json(&api);
    let p = j.get("prerequisites").and_then(|p| p.as_arr()).and_then(|a| a.first()).expect("no prerequisites");
    let f = p.get("functions").unwrap();
    let at = |lang: &str, k: &str| f.get(lang).and_then(|x| x.get(k)).and_then(|x| x.as_str()).unwrap_or_default().to_string();
    assert_eq!(at("typescript", "available"), "enrollmentAvailable");
    assert_eq!(at("go", "requires_signature"), "func EnrollmentRequires(value Course) [][]Course");
    assert_eq!(at("java", "available"), "CourseCatalog.enrollmentAvailable");
    assert_eq!(at("numpy", "available"), "Rule.available(\"enrollment\", value, held)");
    let without = p.get("without").unwrap();
    assert!(without.get("sql").is_some() && without.get("wasm").is_some() && without.get("python").is_none());
    assert_eq!(p.get("ordered_by").and_then(|x| x.as_str()), Some("level_of"));
    // A rule with no block has no key: no other inventory changes.
    let (_, api, _) = run(&["api", "tests/corpus/member_tier_shipping.rule"]);
    assert!(json(&api).get("prerequisites").is_none());
    // The `enums` file names NumPy too: it has no plan there.
    let (_, api, _) = run(&["api", "tests/corpus/shared/certifications.rule"]);
    let j = json(&api);
    let p = j.get("prerequisites").and_then(|p| p.as_arr()).and_then(|a| a.first()).unwrap();
    assert!(p.get("without").unwrap().get("numpy").is_some());
    assert_eq!(p.get("functions").unwrap().get("python").unwrap().get("requires").and_then(|x| x.as_str()), Some("exam_path_requires"));
}

/// An `enums` file with a block, and a rule whose block is over an enum it reads from one: the
/// functions sit in the module of the file that holds the block, and every language answers them
/// as the reference does — through runners of the `enums` file's own for the first.
#[test]
fn every_language_answers_a_block_in_an_enums_file_and_over_an_enum_read_from_one() {
    if !ready(Need::Python, || have("python3"), "no python3, so nothing was run") {
        return;
    }
    let t = TempDir::new("prereq-langs");
    let d = t.path();
    std::fs::create_dir_all(d.join("shared")).unwrap();
    std::fs::write(
        d.join("shared/content.rule"),
        "enums content v1\n\nenum tech = agriculture | pottery | animal_husbandry | archery | the_wheel | trapping | writing | philosophy\nenum era = ancient < classical < medieval\n",
    )
    .unwrap();
    std::fs::write(
        d.join("tech_tree.rule"),
        "rule tech_tree v1\n\nimport enum tech from \"shared/content.rule\"\nimport enum era from \"shared/content.rule\"\n\ninputs\n  t : tech\n\noutputs\n  e : era\n\n\
         table era_of\npolicy unique\n| t                                               | -> e      |\n| agriculture, pottery, animal_husbandry, archery | ancient   |\n| the_wheel, trapping, writing                    | classical |\n| philosophy                                      | medieval  |\n\n\
         prerequisites research over tech\n  start            agriculture\n  pottery          requires agriculture\n  animal_husbandry requires agriculture\n  archery          requires agriculture\n  trapping         requires animal_husbandry\n  the_wheel        requires animal_husbandry, archery\n  writing          requires pottery\n  philosophy       requires writing or the_wheel\n  ordered by era_of\n\n\
         examples\n| t           | -> e     |\n| agriculture | ancient  |\n| philosophy  | medieval |\n",
    )
    .unwrap();
    let (c, out, _) = run_in(d, &["check", "tech_tree.rule"]);
    assert_eq!(c, 0, "{out}");
    let generated = d.join("gen");
    let certs = root().join("tests/corpus/shared/certifications.rule");
    let (c, _, e) = run_in(d, &["gen", "tech_tree.rule", certs.to_str().unwrap(), "--out", generated.to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    for f in ["python/certifications_runner.py", "go/certificationsrunner/main.go", "java/CertificationsRunner.java", "vectors/certifications.prereq.tsv"] {
        assert!(generated.join(f).exists(), "no {f}");
    }
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .env("TMPDIR", tmpdir_in(&generated))
        .args(["test", generated.to_str().unwrap()])
        .output()
        .expect("cannot start rulec test");
    let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{said}");
    for (rule, cases) in [("tech_tree", 0), ("certifications", 29)] {
        let langs: Vec<&str> = said.lines().filter(|l| l.starts_with(&format!("ok    {rule} (")) && l.contains(", prerequisites)")).collect();
        assert!(!langs.is_empty(), "{rule}: no language answered the prerequisites:\n{said}");
        if cases > 0 {
            assert!(langs.iter().all(|l| l.ends_with(&format!("{cases} prerequisite cases"))), "{said}");
        }
    }
    assert!(said.contains("note: SQL: tech_tree's prerequisites have no functions in this target, which has one entry, so none were compared"), "{said}");
    for l in said.lines().filter(|l| l.starts_with("warning")) {
        skip(l);
    }
}

/// The certificate's `prerequisites` is held by `tools/recheck.py` to the graph and to the rows of
/// the table that orders it; one that lies is refused. An `enums` file's certificate carries its
/// block too, with no table.
#[test]
fn the_certificate_of_the_prerequisites_is_rechecked() {
    if !ready(Need::Python, || have("python3"), "no python3, so tools/recheck.py was not run") {
        return;
    }
    let recheck = |cert: &str| -> (bool, String) {
        let t = TempDir::new("prereq-cert");
        let p = t.path().join("c.json");
        std::fs::write(&p, cert).unwrap();
        let o = Command::new("python3").arg(root().join("tools/recheck.py")).arg(&p).output().expect("cannot start python3");
        (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let (c, cert, _) = run(&["certificate", COURSES]);
    assert_eq!(c, 0);
    let (ok, said) = recheck(&cert);
    assert!(ok, "{said}");
    assert!(said.contains("prerequisites enrollment over course: 14 values and 15 edges with no cycle, 13 reached from start and 1 no case can take, and level_of keeps its order along 15 edges"), "{said}");
    let lies: &[(&str, &str, &str)] = &[
        ("a value said twice", "\"start\":[\"intro_programming\",\"discrete_math\",\"calculus\"]", "\"start\":[\"intro_programming\",\"discrete_math\",\"calculus\",\"statistics\"]"),
        ("an order a prerequisite does not keep", "\"topological\":[\"intro_programming\",", "\"topological\":[\"statistics\",\"intro_programming\","),
        ("a value reached too early", "\"reached\":[\"intro_programming\",", "\"reached\":[\"capstone\",\"intro_programming\","),
        ("an answer the table does not write", "\"capstone\":\"fourth\"", "\"capstone\":\"first\""),
    ];
    for (what, from, to) in lies {
        assert!(cert.contains(from), "{what}: the certificate has no {from}");
        let (ok, said) = recheck(&cert.replacen(from, to, 1));
        assert!(!ok, "{what}: a lying certificate passed:\n{said}");
    }
    let (c, cert, _) = run(&["certificate", "tests/corpus/shared/certifications.rule"]);
    assert_eq!(c, 0);
    let (ok, said) = recheck(&cert);
    assert!(ok && said.contains("prerequisites exam_path over certification: 7 values"), "{said}");
}

/// The page for people shows each block — the table of what each value needs, its picture, and
/// what the check showed — and the HTML page draws the picture in its place; the customer reads
/// what comes before what; an `enums` file's page has its block too.
#[test]
fn the_pages_show_the_block() {
    let (c, md, _) = run(&["doc", COURSES]);
    assert_eq!(c, 0);
    for want in [
        "## Prerequisites: enrollment",
        "| machine_learning | algorithms, statistics or linear_algebra | fourth |",
        "| mainframe_cobol | no case can take it: withdrawn from the catalog in 2026 | fourth |",
        "```mermaid\n%% prerequisites 0\nflowchart LR\n",
        "  p0v9 -.-> p0v12\n",
        "- Every value but those no case can take (mainframe_cobol) can be reached from `start` (E137).",
        "- In the order of table level_of, no value comes before what it needs (E138).",
    ] {
        assert!(md.contains(want), "no {want:?} in:\n{md}");
    }
    let t = TempDir::new("prereq-doc");
    let (c, _, e) = run(&["doc", COURSES, "--format", "html", "--out", t.path().to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    let html = std::fs::read_to_string(t.path().join("course_catalog.html")).unwrap();
    assert!(html.contains("<title>The prerequisites enrollment</title>"), "no picture");
    assert!(!html.contains("<!--PREREQ") && !html.contains("<!--MACHINE"), "a marker is left on the page");
    let (c, customer, _) = run(&["doc", COURSES, "--audience", "customer"]);
    assert_eq!(c, 0);
    assert!(customer.contains("- **machine_learning**: after algorithms and either statistics or linear_algebra"), "{customer}");
    let (c, md, _) = run(&["doc", "tests/corpus/shared/資格.rule", "--lang", "ja"]);
    assert_eq!(c, 0);
    assert!(md.contains("## 前提のつながり: 受験の前提") && md.contains("| 設計プロフェッショナル | 開発アソシエイト or 運用アソシエイト |"), "{md}");
}
