//! The twins of the corpus: a rule written with Japanese names, and the same rule written with
//! English names (tests/corpus/twins.tsv).
//!
//! A rule's names are the business's own words, so the corpus began with Japanese ones. A twin is
//! the same rule with the names in English, `JPY` for `円`, the `万` and `億` multipliers written
//! out, and the description and the comments translated; it is made by substitution, not by
//! retyping, so no cell moves. What these tests hold is that it stays so: the pair is checked with
//! the same findings, audited with the same obligations, answers the same vectors, states the same
//! claims in its certificate, and promises its callers the same (the relations between inputs, the
//! units, ranges and roundings). Names are the one thing allowed to differ, so nothing here
//! compares one.
//!
//! Where the checker's output is ordered by name, the order differs between the two (the keys of a
//! certificate, the lines that walk a fold's transitions). The comparisons below are written to
//! hold in spite of that, and say where they fall back to a count.

use rulec::json::{self, Json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .current_dir(root())
        .args(args)
        .output()
        .expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The pairs of tests/corpus/twins.tsv as paths from the crate: the Japanese file, then its twin.
fn twins() -> Vec<(String, String)> {
    let table = std::fs::read_to_string(root().join("tests/corpus/twins.tsv")).expect("tests/corpus/twins.tsv");
    table
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (ja, en) = l.split_once('\t').unwrap_or_else(|| panic!("a pair is two names with a tab between: {l:?}"));
            (format!("tests/corpus/{ja}"), format!("tests/corpus/{en}"))
        })
        .collect()
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// The names of the rules in tests/corpus, sorted.
fn corpus_files() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(root().join("tests/corpus"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rule"))
        .collect();
    v.sort();
    v
}

/// What the first line of a rule calls it, and what its generated code is called:
/// `rule 送料(shipping_fee) v4` is (送料, shipping_fee), and `rule fee_demo v1` is (fee_demo, fee_demo).
fn names_of(rel: &str) -> (String, String) {
    let src = read(rel);
    let first = src.lines().find(|l| l.starts_with("rule ")).unwrap_or_else(|| panic!("{rel} has no rule line"));
    let head = first["rule ".len()..].split(' ').next().unwrap();
    match head.split_once('(') {
        Some((name, alias)) => (name.to_string(), alias.trim_end_matches(')').to_string()),
        None => (head.to_string(), head.to_string()),
    }
}

/// Every rule written with Japanese names has a twin, and the pairing is a pairing: one twin to
/// a rule, a twin's name is ASCII and is the name of its rule, and no two rules of the corpus
/// are generated under the same alias (the second would write over the first without a word, and
/// every test that generates the whole corpus would then run one of them twice).
#[test]
fn every_rule_written_in_japanese_has_an_english_twin() {
    let pairs = twins();
    let japanese: BTreeSet<String> = corpus_files().into_iter().filter(|n| !n.is_ascii()).collect();
    let listed: BTreeSet<String> = pairs.iter().map(|(ja, _)| ja["tests/corpus/".len()..].to_string()).collect();
    assert_eq!(japanese, listed, "tests/corpus/twins.tsv lists the rules with Japanese names, all of them and only them");

    let mut seen = BTreeSet::new();
    for (ja, en) in &pairs {
        let stem = en["tests/corpus/".len()..].trim_end_matches(".rule").to_string();
        assert!(stem.is_ascii() && en.ends_with(".rule"), "{en}: a twin has an English name");
        assert!(root().join(en).exists(), "{en}: the twin of {ja} is not there");
        assert!(seen.insert(en.clone()), "{en} is the twin of two rules");
        let (name, _) = names_of(en);
        assert_eq!(name, stem, "{en}: a rule is named after its file");
        assert!(name.is_ascii(), "{en}: the name of a twin is in English");
        assert_ne!(ja, en);
    }

    let mut alias_of: BTreeMap<String, String> = BTreeMap::new();
    for name in corpus_files() {
        let rel = format!("tests/corpus/{name}");
        let (_, alias) = names_of(&rel);
        if let Some(other) = alias_of.insert(alias.clone(), rel.clone()) {
            panic!("{rel} and {other} are both generated as `{alias}`");
        }
    }
}

/// The findings of `check`, by code, and the notes (the counts of shadow pairs) without the file
/// they are about. Sorted, because the order of the rows can follow the names.
fn findings(out: &str) -> Vec<String> {
    let mut v: Vec<String> = out
        .lines()
        .filter_map(|l| {
            if let Some(rest) = l.strip_prefix("error[").or_else(|| l.strip_prefix("warning[")) {
                return Some(rest.split(']').next().unwrap().to_string());
            }
            l.strip_prefix("note ").and_then(|r| r.split_once(": ")).map(|(_, text)| format!("note: {text}"))
        })
        .collect();
    v.sort();
    v
}

/// The twin passes `check` as its original does, with the same warnings and the same notes.
#[test]
fn a_twin_is_checked_with_the_same_findings_as_its_original() {
    let mut warned = 0;
    for (ja, en) in twins() {
        let (cj, oj) = run(&["check", &ja]);
        let (ce, oe) = run(&["check", &en]);
        assert_eq!((cj, ce), (0, 0), "{ja} / {en}\n{oj}\n{oe}");
        let (fj, fe) = (findings(&oj), findings(&oe));
        assert_eq!(fj, fe, "{ja} and {en} are not checked alike");
        warned += fj.iter().filter(|f| !f.starts_with("note:")).count();
    }
    assert!(warned > 0, "no twin carries a warning, so the comparison of warnings compares nothing");
}

/// Both are audited against the same obligations, and the same number of cases meets them.
#[test]
fn a_twin_has_the_same_coverage_as_its_original() {
    for (ja, en) in twins() {
        // `{"file":"…","vectors":70,…}`: everything after the file name, which is the one part
        // that names the file.
        let tail = |rel: &str| {
            let (c, out) = run(&["coverage", rel, "--format", "json"]);
            assert_eq!(c, 0, "{rel}: {out}");
            out[out.find("\"vectors\":").unwrap_or_else(|| panic!("{rel}: {out}"))..].to_string()
        };
        assert_eq!(tail(&ja), tail(&en), "{ja} and {en} are not audited alike");
    }
}

/// A rule that walks a sequence by `fold` has its transitions enumerated in the order of the
/// verdict names, so what two spellings of the names give is the same set in another order, and
/// the witnesses (the numbers inside the sequences) are picked in that order too.
fn has_fold(rel: &str) -> bool {
    read(rel).lines().any(|l| l.starts_with("fold "))
}

/// One consistent pairing of strings: what one spelling calls a value, the other calls the same
/// value every time it comes up, and two values are never called alike.
#[derive(Default)]
struct Pairing {
    forth: HashMap<String, String>,
    back: HashMap<String, String>,
}

impl Pairing {
    fn pair(&mut self, a: &str, b: &str) -> bool {
        if let Some(seen) = self.forth.get(a) {
            return seen == b && self.back.get(b).map(String::as_str) == Some(a);
        }
        if self.back.contains_key(b) {
            return false;
        }
        self.forth.insert(a.to_string(), b.to_string());
        self.back.insert(b.to_string(), a.to_string());
        true
    }
}

/// `a` and `b` have the same shape and the same numbers, and the strings in them are the same
/// values under one pairing for the position they sit at. Arrays are walked by index and
/// objects by position, because the keys are names.
fn same_values(a: &Json, b: &Json, at: &str, pairs: &mut HashMap<String, Pairing>) -> Result<(), String> {
    match (a, b) {
        (Json::Str(x), Json::Str(y)) => {
            if pairs.entry(at.to_string()).or_default().pair(x, y) {
                Ok(())
            } else {
                Err(format!("{at}: {x:?} against {y:?}"))
            }
        }
        (Json::Arr(x), Json::Arr(y)) => {
            if x.len() != y.len() {
                return Err(format!("{at}: {} elements against {}", x.len(), y.len()));
            }
            x.iter().zip(y).try_for_each(|(p, q)| same_values(p, q, &format!("{at}[]"), pairs))
        }
        (Json::Obj(x), Json::Obj(y)) => {
            if x.len() != y.len() {
                return Err(format!("{at}: {} fields against {}", x.len(), y.len()));
            }
            x.iter().zip(y).enumerate().try_for_each(|(i, ((_, p), (_, q)))| same_values(p, q, &format!("{at}/{i}"), pairs))
        }
        (p, q) if std::mem::discriminant(p) == std::mem::discriminant(q) && p == q => Ok(()),
        (p, q) => Err(format!("{at}: {} against {}", json::show(p), json::show(q))),
    }
}

/// The number a trace entry (`table 送料 row 3`) names, if it names a row.
fn row_of(entry: &str) -> Option<u64> {
    entry.rsplit_once(" row ").and_then(|(_, n)| n.split(' ').next()).and_then(|n| n.parse().ok())
}

fn vector_lines(rel: &str) -> Vec<Json> {
    let (c, out) = run(&["vectors", rel]);
    assert_eq!(c, 0, "{rel}");
    out.lines().filter(|l| !l.trim().is_empty()).map(|l| json::parse(l).unwrap_or_else(|e| panic!("{rel}: {e}: {l}"))).collect()
}

/// The twin answers the same questions with the same answers: as many cases, the same numbers in
/// the same places, a string standing for the same value every time, the same rows firing, and the
/// same reason for each case. Names are compared with nothing but one another, so a case that
/// used a different value for the same enum would fail.
#[test]
fn a_twin_answers_like_its_original() {
    for (ja, en) in twins() {
        let (vj, ve) = (vector_lines(&ja), vector_lines(&en));
        assert_eq!(vj.len(), ve.len(), "{ja} and {en} do not generate as many cases");
        assert!(!vj.is_empty());
        if has_fold(&ja) {
            // The same count, and the same audit above; the lines themselves come in another order.
            continue;
        }
        let mut pairs: HashMap<String, Pairing> = HashMap::new();
        let mut why = Pairing::default();
        for (i, (x, y)) in vj.iter().zip(&ve).enumerate() {
            for part in ["in", "out"] {
                let (p, q) = (x.get(part).unwrap(), y.get(part).unwrap());
                if let Err(e) = same_values(p, q, part, &mut pairs) {
                    panic!("{ja} and {en}, case {i}: {e}");
                }
            }
            let rows = |l: &Json| -> Vec<Option<u64>> {
                match l.get("trace") {
                    Some(Json::Arr(t)) => t.iter().map(|e| row_of(e.as_str().unwrap())).collect(),
                    _ => panic!("no trace"),
                }
            };
            assert_eq!(rows(x), rows(y), "{ja} and {en}, case {i}: other rows fire");
            let words = |l: &Json| -> Vec<String> {
                l.get("why").unwrap().as_str().unwrap().split(|c: char| c.is_whitespace() || c == ',' || c == ':').filter(|w| !w.is_empty()).map(String::from).collect()
            };
            let (wj, we) = (words(x), words(y));
            assert_eq!(wj.len(), we.len(), "{ja} and {en}, case {i}: the reason is not the same sentence");
            for (a, b) in wj.iter().zip(&we) {
                // The words of the sentence are the same; the names in it are paired.
                assert!(a == b || why.pair(a, b), "{ja} and {en}, case {i}: {a:?} against {b:?}");
            }
        }
    }
}

// ── What the certificate claims ─────────────────────────────────────────────────────────────

fn field<'a>(j: &'a Json, k: &str) -> &'a Json {
    j.get(k).unwrap_or_else(|| panic!("no `{k}` in {}", json::unparse(j)))
}

fn items<'a>(j: &'a Json, k: &str) -> &'a [Json] {
    match field(j, k) {
        Json::Arr(a) => a,
        other => panic!("`{k}` is not an array: {}", json::unparse(other)),
    }
}

/// How many objects of each set of keys a value holds. A claim's keys are words of the
/// certificate and never names, except in the objects under `values`, which are keyed by the
/// names of the inputs they give a value to, so those count their members only.
fn shape(j: &Json, parent: &str, out: &mut BTreeMap<String, usize>) {
    match j {
        Json::Obj(m) => {
            let key = if parent == "values" {
                format!("names x {}", m.len())
            } else {
                let mut ks: Vec<&str> = m.iter().map(|(k, _)| k.as_str()).collect();
                ks.sort();
                ks.join(",")
            };
            *out.entry(key).or_default() += 1;
            for (k, v) in m {
                shape(v, k, out);
            }
        }
        Json::Arr(a) => a.iter().for_each(|v| shape(v, parent, out)),
        _ => {}
    }
}

fn shape_of(j: &Json) -> String {
    let mut m = BTreeMap::new();
    shape(j, "", &mut m);
    format!("{m:?}")
}

fn sorted<T: Ord>(mut v: Vec<T>) -> Vec<T> {
    v.sort();
    v
}

/// What one table of a certificate says, with every name left out and every list that may follow
/// the names put in an order that does not: its policy and the shape of its axes, each row's
/// tests and how many points it accepts on each axis, which pairs of rows are told apart, which
/// rows are refuted, undecided, unused or unreachable, which rows a point reaches, and the shape
/// of the three trees (`cover`, `linear`, `above`).
fn table_claims(t: &Json) -> String {
    let axes: Vec<String> = sorted(
        items(t, "axes")
            .iter()
            .map(|a| format!("{}:{}:{}", field(a, "kind").as_str().unwrap_or("?"), items(a, "coords").len(), json::unparse(field(a, "step"))))
            .collect(),
    );
    let rows: Vec<String> = items(t, "rows")
        .iter()
        .map(|r| {
            let tests = items(r, "tests");
            let cells = sorted(tests.iter().map(|x| field(x, "cell").as_str().unwrap_or("?").to_string()).collect());
            let cmps = sorted(
                tests
                    .iter()
                    .flat_map(|x| match x.get("tests") {
                        Some(Json::Arr(a)) => a.iter().map(|c| format!("{}{}", json::show(c.get("op").unwrap_or(&Json::Null)), json::show(c.get("value").unwrap_or(&Json::Null)))).collect::<Vec<_>>(),
                        _ => vec![],
                    })
                    .collect::<Vec<_>>(),
            );
            let accepts = sorted(items(r, "accepts").iter().map(|a| match a { Json::Arr(v) => v.len(), _ => 0 }).collect());
            let produces: Vec<bool> = items(r, "produces").iter().map(|p| matches!(p, Json::Null)).collect();
            format!("{}|{}|{:?}|{:?}|{:?}|{:?}", json::show(field(r, "row")), tests.len(), cells, cmps, accepts, produces)
        })
        .collect();
    let reach = sorted(items(t, "reach").iter().map(|r| json::show(field(r, "row"))).collect());
    let above = field(t, "above");
    let linear = field(t, "linear");
    format!(
        "{}|{}|{:?}|{}|{:?}|{:?}|disjoint {}|refuted {}|undecided {}|unused {}|unreachable {}|reach {:?}|constraints {}|never {}|apart {}|extra {}|facts {}|cover {}|linear {}|above {}",
        json::show(field(t, "policy")),
        json::show(field(t, "outputs")),
        axes,
        items(t, "decides").len(),
        rows.len(),
        rows,
        items(t, "disjoint").len(),
        items(t, "refuted").len(),
        items(t, "undecided").len(),
        items(t, "unused").len(),
        items(t, "unreachable").len(),
        reach,
        items(t, "constraints").len(),
        items(above, "never").len(),
        items(above, "apart").len(),
        items(linear, "extra").len(),
        items(linear, "facts").len(),
        shape_of(field(t, "cover")),
        shape_of(linear),
        shape_of(above),
    )
}

/// Everything a certificate claims, without a name in it.
fn claims(rel: &str) -> Vec<String> {
    let (c, out) = run(&["certificate", rel]);
    assert_eq!(c, 0, "{rel}");
    let cert = json::parse(out.trim()).unwrap_or_else(|e| panic!("{rel}: {e}"));
    let count = |k: &str| match field(&cert, k) {
        Json::Obj(m) => m.len(),
        Json::Arr(a) => a.len(),
        _ => 0,
    };
    let member_lens = |k: &str| -> Vec<usize> {
        match field(&cert, k) {
            Json::Obj(m) => sorted(m.iter().map(|(_, v)| if let Json::Arr(a) = v { a.len() } else { 0 }).collect()),
            _ => vec![],
        }
    };
    let ranges = match field(&cert, "ranges") {
        Json::Obj(m) => sorted(m.iter().map(|(_, v)| json::unparse(v)).collect()),
        _ => vec![],
    };
    let machine = field(&cert, "machine");
    let mut v = vec![
        format!("types {}", count("types")),
        format!("ranges {ranges:?}"),
        format!("groups {:?}", member_lens("groups")),
        format!("enums {:?}", member_lens("enums")),
        format!("constraints {}", count("constraints")),
        format!("contracts {}", count("contracts")),
        format!("values {} {}", count("values"), shape_of(field(&cert, "values"))),
        format!("machine {}", if matches!(machine, Json::Null) { "none".to_string() } else { shape_of(machine) }),
    ];
    v.extend(items(&cert, "tables").iter().map(table_claims));
    v
}

/// The twin's certificate states what its original's states, table by table: the same tiling, the
/// same parting of rows, the same rows reached, the same things left undecided. What the
/// re-checker makes of each one is `tests/cert.rs`'s, which reads every file of the corpus.
#[test]
fn a_twin_states_the_same_claims_as_its_original() {
    let mut tables = 0;
    for (ja, en) in twins() {
        let (cj, ce) = (claims(&ja), claims(&en));
        assert_eq!(cj.len(), ce.len(), "{ja} and {en} do not state as many things");
        for (i, (a, b)) in cj.iter().zip(&ce).enumerate() {
            assert_eq!(a, b, "{ja} and {en}: claim {i} differs");
        }
        tables += cj.len() - 8;
    }
    assert!(tables >= 40, "{tables} tables compared");
}

// ── What the rule promises the code that calls it ───────────────────────────────────────────

/// What a rule promises to its callers, with no name in it: the relations between inputs it
/// relies on, the unit and range of every input and output, the rounding, how many values each
/// enum has, and whether it applies, cites, reads a contract or carries a state. `円` and `JPY`
/// are one currency, and print as they were spelled.
fn promises(rel: &str) -> Vec<String> {
    let (c, out) = run(&["api", rel]);
    assert_eq!(c, 0, "{rel}");
    let api = json::parse(out.trim()).unwrap_or_else(|e| panic!("{rel}: {e}"));
    let unit = |u: Option<&Json>| -> String {
        match u {
            Some(Json::Str(s)) if s == "円" => "JPY".to_string(),
            Some(Json::Str(s)) => s.clone(),
            _ => "-".to_string(),
        }
    };
    let shown = |j: Option<&Json>| j.map(json::unparse).unwrap_or_else(|| "-".to_string());
    let py = field(&api, "python");
    let mut v = vec![format!(
        "preconditions {:?}",
        items(&api, "preconditions")
            .iter()
            .map(|p| format!("{}{}", json::show(p.get("kind").unwrap_or(&Json::Null)), json::show(p.get("op").unwrap_or(&Json::Null))))
            .collect::<Vec<_>>()
    )];
    v.push(format!(
        "applies {} sources {} projection {} machine {}",
        items(&api, "applies").len(),
        items(&api, "sources").len(),
        !matches!(field(&api, "projection"), Json::Null),
        !matches!(field(&api, "machine"), Json::Null)
    ));
    for p in items(py, "params") {
        v.push(format!("in {} {} {}", unit(p.get("unit")), shown(p.get("range")), json::show(field(p, "optional"))));
    }
    for o in items(py, "outputs") {
        v.push(format!("out {} {} {}", unit(o.get("unit")), shown(o.get("rounding")), json::show(field(o, "optional"))));
    }
    v.push(format!("enums {:?}", sorted(items(py, "enums").iter().map(|e| items(e, "values").len()).collect::<Vec<_>>())));
    v
}

/// The twin promises its callers what its original does: the same relations between inputs, the
/// same range and unit on every input and output in the same order, the same rounding, and as many
/// values to every enum.
#[test]
fn a_twin_makes_the_same_promises_as_its_original() {
    let mut relations = 0;
    for (ja, en) in twins() {
        let (pj, pe) = (promises(&ja), promises(&en));
        assert_eq!(pj, pe, "{ja} and {en} do not promise the same");
        relations += pj[0].matches("constraint").count();
    }
    assert!(relations >= 4, "{relations} relations between inputs compared");
}
