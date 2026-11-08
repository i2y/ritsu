//! `unique` on a field of `elements` (§15.290): the door refuses two elements with the same value of
//! the field, in the reference evaluator and in every language; a `sum` the values of a `unique` field
//! of an enum or a truth value take past its range, each value once, is E147, for the rule alone and
//! for the rule with its contributions; and `unique` where it cannot stand is E148.
//!
//! The materials: `tests/merging/unique/` — the learning of a province, an academy that adds forty a
//! season and one that adds ten — in English, with `州の学問.rule` and its two contributions beside
//! them in Japanese.

use ritsu_testkit::{Need, TempDir, ready, skip};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures() -> PathBuf {
    root().join("tests/merging/unique")
}

fn run_in(dir: &Path, lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(dir).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn has_all(out: &str, said: &[&str]) {
    for s in said {
        assert!(out.contains(s), "missing `{s}`:\n{out}");
    }
}

/// The story the check is for: a mod adds a building to an open enum and its row to an open table.
/// The rule alone passes; with the academy that adds forty, a temple and a library beside it take the
/// buildings to 43, past the 40 the `sum` takes, and the check stops it, naming the row the
/// contribution added; with the academy that adds ten the buildings add 13 at most, and it passes.
/// The same in Japanese.
#[test]
fn a_mod_that_takes_a_sum_past_its_range_is_stopped_and_one_inside_passes() {
    let d = fixtures();
    let (c, out, _) = run_in(&d, "en", &["check", "province_learning.rule"]);
    assert_eq!(c, 0, "{out}");
    assert!(!out.contains("E147"), "{out}");

    let (c, out, _) = run_in(&d, "en", &["check", "province_learning.rule", "contrib/academy.rule"]);
    assert_eq!(c, 1, "{out}");
    has_all(
        &out,
        &[
            "error[E147]: With each `building` once, `building_learning` can reach 43, past the 40 its range takes",
            "--> province_learning.rule:19",
            "can reach 43",
            "The most the element of each `building` can add, largest first: academy 40, library 2, temple 1.",
            "  academy 40: the row contribution academy adds (contrib/academy.rule:8)",
            "the generated code refuses such a sequence at its door (`building_learning is out of range`)",
        ],
    );
    assert_eq!(out.matches("error[").count(), 1, "{out}");
    // The contribution checked on its own is checked with the rule it adds to.
    let (c, out, _) = run_in(&d, "en", &["check", "contrib/academy.rule"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("error[E147]"), "{out}");

    let (c, out, _) = run_in(&d, "en", &["check", "province_learning.rule", "contrib/small_academy.rule"]);
    assert_eq!(c, 0, "{out}");
    has_all(&out, &["ok province_learning.rule (1 contribution: contrib/small_academy.rule)"]);

    let (c, out, _) = run_in(&d, "ja", &["check", "州の学問.rule"]);
    assert_eq!(c, 0, "{out}");
    let (c, out, _) = run_in(&d, "ja", &["check", "州の学問.rule", "contrib/学問所.rule"]);
    assert_eq!(c, 1, "{out}");
    has_all(
        &out,
        &[
            "エラー[E147]: 建物 が一つずつでも、建物の学問 は 43 に届き、範囲の 40 を越えます",
            "--> 州の学問.rule:19",
            "建物 の値ごとに、その要素が足しうる最大を、大きい順に: 学問所 40、書庫 2、寺 1。",
            "  学問所 40: 寄与 学問所 が足した行です（contrib/学問所.rule:8）",
            "生成コードはそういう並びを入口で受け付けません（「建物の学問 が範囲の外です」）",
        ],
    );
    let (c, out, _) = run_in(&d, "ja", &["check", "州の学問.rule", "contrib/小さな学問所.rule"]);
    assert_eq!(c, 0, "{out}");
}

/// A rule of four buildings that add 20, 15, 10 and 5, under a `sum` that takes 40. Each once, they
/// add 50. A `count` over the sequence caps how many elements the door takes, at the smallest top a
/// `count` declares (§15.58), and the reach is then the largest that many: 35 for two, which passes,
/// and 45 for three, which is E147 and says the cap took the rest out.
#[test]
fn a_count_cap_adds_the_largest_that_many() {
    let rule = |cap: &str| {
        format!(
            "rule capped(capped) v1\n\n\
             enum building(building) = temple(temple) | library(library) | academy(academy) | shrine(shrine)\n\n\
             elements buildings(buildings)\n  building(building) : building  unique\n\n\
             outputs\n  learning(learning) : number  round down(1)\n\n\
             table learning_of\npolicy unique\n\
             | building | -> learning_add : number |\n\
             | temple   | 20                       |\n\
             | library  | 15                       |\n\
             | academy  | 10                       |\n\
             | shrine   | 5                        |\n\n\
             count temples(temples) over buildings where building = temple  range >=0 <={cap}\n\n\
             sum building_learning(building_learning) over buildings of learning_add  range >=0 <=40\n\n\
             define learning : number = 1 + building_learning + temples\n"
        )
    };
    let t = TempDir::new("unique-cap");
    for (cap, want) in [("2", None), ("3", Some(45)), ("4", Some(50))] {
        std::fs::write(t.path().join("capped.rule"), rule(cap)).unwrap();
        let (c, out, _) = run_in(t.path(), "en", &["check", "capped.rule"]);
        match want {
            None => assert_eq!(c, 0, "cap {cap}: {out}"),
            Some(reach) => {
                assert_eq!(c, 1, "cap {cap}: {out}");
                assert!(out.contains(&format!("`building_learning` can reach {reach}, past the 40")), "cap {cap}: {out}");
            }
        }
        if cap == "3" {
            has_all(&out, &["largest first: temple 20, library 15, academy 10.", "A `count` caps the sequence at 3 elements, so the 3 largest are added."]);
            assert!(!out.contains("shrine 5"), "{out}");
        }
        if cap == "4" {
            // Four values and a cap of four: the cap takes nothing out, and says nothing.
            assert!(!out.contains("A `count` caps"), "{out}");
        }
    }
}

/// `unique` on a number, an amount or a string holds at the door, and caps no `sum`: a field with that
/// many values to take is not counted value by value, so the check says nothing of a `sum` over the
/// sequence, and the door refuses two elements with the same value, compared as values (`1` and `1.0`
/// are one number).
#[test]
fn unique_on_a_number_or_a_string_is_the_doors_alone() {
    let t = TempDir::new("unique-number");
    std::fs::write(
        t.path().join("lots.rule"),
        "rule lots_fee(lots_fee) v1\n\n\
         elements lots(lots)\n  lot(lot) : number  range >=1 <=99  unique\n  tag(tag) : string  unique\n  fee(fee) : money[USD]  range >=0USD <=100USD\n\n\
         outputs\n  total(total) : money[USD]  round down(1USD)\n\n\
         sum fees(fees) over lots of fee  range >=0USD <=300USD\n\n\
         define total : money[USD] = fees\n",
    )
    .unwrap();
    let (c, out, _) = run_in(t.path(), "en", &["check", "lots.rule"]);
    assert_eq!(c, 0, "{out}");
    assert!(!out.contains("E147"), "{out}");
    for (input, said) in [
        (r#"{"lots":[{"lot":1,"tag":"a","fee":10},{"lot":1.0,"tag":"b","fee":10}]}"#, "refused (input): lots has two elements with the same lot"),
        (r#"{"lots":[{"lot":1,"tag":"a","fee":10},{"lot":2,"tag":"a","fee":10}]}"#, "refused (input): lots has two elements with the same tag"),
        (r#"{"lots":[{"lot":1,"tag":"a","fee":10},{"lot":2,"tag":"b","fee":10}]}"#, "total = 20USD"),
    ] {
        std::fs::write(t.path().join("in.json"), input).unwrap();
        let (c, out, e) = run_in(t.path(), "en", &["run", "lots.rule", "--input", "in.json"]);
        assert_eq!(c, 0, "{out}{e}");
        assert!(out.contains(said), "{input}: {out}");
    }
}

/// `unique` stands only on a field of `elements` that is always there: not on an input's line, not on
/// a field that may be absent, and not beside `random` (whose draws are each their own; on a field of
/// an element `random` is E070 as well).
#[test]
fn unique_stands_only_on_a_field_that_is_always_there() {
    let codes = |src: &str| -> Vec<(String, Vec<String>)> {
        rulec::i18n::with(rulec::i18n::Lang::En, || {
            rulec::check_source(src, "u.rule").into_iter().filter(|d| d.code == "E148").map(|d| (d.title.clone(), d.marks.iter().map(|m| m.label.clone()).collect())).collect()
        })
    };
    let title = "Only a field of `elements` that is always there can be `unique`";
    for (src, mark, also) in [
        (
            "rule t(t) v1\n\nenum kind(kind) = a(a) | b(b)\n\ninputs\n  k(k) : kind  unique\n\noutputs\n  r(r) : number  round down(1)\n\n\
             table j\npolicy unique\n| k | -> r |\n| a | 1 |\n| b | 2 |\n",
            "k is an input",
            None,
        ),
        (
            "rule t(t) v1\n\nelements lots(lots)\n  tag(tag) : string?  unique\n  fee(fee) : money[USD]  range >=0USD <=100USD\n\n\
             outputs\n  total(total) : money[USD]  round down(1USD)\n\nsum fees(fees) over lots of fee  range >=0USD <=300USD\n\n\
             define total : money[USD] = fees\n",
            "tag may be absent (`?`)",
            None,
        ),
        (
            "rule t(t) v1\n\nelements cards(cards)\n  face(face) : number  range >=1 <=6  random unique\n\n\
             outputs\n  total(total) : number  round down(1)\n\nsum faces(faces) over cards of face  range >=0 <=60\n\n\
             define total : number = faces\n",
            "face is `random`",
            Some("E070"),
        ),
    ] {
        let got = codes(src);
        assert_eq!(got.len(), 1, "{src}\n{got:?}");
        assert_eq!(got[0].0, title);
        assert!(got[0].1.iter().any(|l| l == mark), "{src}\n{got:?}");
        let all: Vec<&str> = rulec::check_source(src, "u.rule").iter().map(|d| d.code).collect();
        if let Some(code) = also {
            assert!(all.contains(&code), "{all:?}");
        }
    }
    // Still a word that names: an enum value called `unique`, read by a range on an input.
    let named = "rule t(t) v1\n\nenum lvl(lvl) = low(low) < unique(unique) < high(high)\n\ninputs\n  x(x) : lvl  range >=unique\n\n\
                 outputs\n  r(r) : number  round down(1)\n\ntable j\npolicy unique\n| x      | -> r |\n| unique | 1    |\n| high   | 2    |\n";
    let ds = rulec::check_source(named, "u.rule");
    assert!(!ds.iter().any(|d| d.code.starts_with('E')), "{:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
}

/// The door refuses two elements with the same value of a `unique` field, in the reference evaluator
/// and with the sentence of the rule's language; `rulec vectors` writes the refused input that holds
/// every language to it, and an example that claims an answer for such a sequence is E019.
#[test]
fn the_door_refuses_two_elements_with_the_same_value() {
    let d = fixtures();
    let t = TempDir::new("unique-door");
    std::fs::write(t.path().join("two.json"), r#"{"buildings":[{"building":"temple"},{"building":"temple"}]}"#).unwrap();
    std::fs::write(t.path().join("three.json"), r#"{"buildings":[{"building":"temple"},{"building":"library"},{"building":"market"}]}"#).unwrap();
    let two = t.path().join("two.json").to_string_lossy().into_owned();
    let three = t.path().join("three.json").to_string_lossy().into_owned();
    let (c, out, _) = run_in(&d, "en", &["run", "province_learning.rule", "--input", &two]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("refused (input): buildings has two elements with the same building"), "{out}");
    let (_, out, _) = run_in(&d, "en", &["run", "province_learning.rule", "--input", &three]);
    assert!(out.contains("learning = 4"), "{out}");
    std::fs::write(t.path().join("two_ja.json"), r#"{"建物一覧":[{"建物":"寺"},{"建物":"寺"}]}"#).unwrap();
    let two_ja = t.path().join("two_ja.json").to_string_lossy().into_owned();
    let (_, out, _) = run_in(&d, "ja", &["run", "州の学問.rule", "--input", &two_ja]);
    assert!(out.contains("建物一覧 の要素に 建物 が同じものが二つあります"), "{out}");

    // The refused input the vectors carry, which `rulec test` holds every language to.
    let (f, c) = {
        let path = d.join("province_learning.rule").to_string_lossy().into_owned();
        rulec::prepare(&std::fs::read_to_string(&path).unwrap(), &path).unwrap_or_else(|_| panic!("province_learning.rule does not prepare"))
    };
    let vs = rulec::vectors::generate(&f, &c);
    let refused = rulec::i18n::with(rulec::i18n::Lang::En, || rulec::vectors::door_cases(&f, &c, &vs));
    let line = refused.iter().find(|l| l.contains("has two elements")).unwrap_or_else(|| panic!("{refused:?}"));
    assert!(line.contains(r#""error":"buildings has two elements with the same building""#), "{line}");
    // No answered vector has two elements alike: the door would refuse it.
    for v in &vs {
        let j = rulec::json::parse(&rulec::vectors::in_object(&f, &c, v)).unwrap();
        assert!(rulec::door::refusal(&f, &c, &j).is_none(), "{}", j.compact());
    }

    let ex = std::fs::read_to_string(d.join("province_learning.rule")).unwrap().replace(
        "sequence none_built\n| building |\n",
        "sequence none_built\n| building |\n\nsequence two_temples\n| building |\n| temple   |\n| temple   |\n",
    ) + "| two_temples           | 3           |\n";
    let ds = rulec::i18n::with(rulec::i18n::Lang::En, || rulec::check_source(&ex, "ex.rule"));
    let e019: Vec<&rulec::diag::Diag> = ds.iter().filter(|d| d.code == "E019").collect();
    assert_eq!(e019.len(), 1, "{:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
    assert_eq!(e019[0].title, "An example's sequence has two elements with the same `unique` field");
}

/// What else reads the mark: `rulec fmt` leaves it as written, `rulec api` puts `unique: true` on
/// the field and the precondition beside the others, `rulec doc` says it of the field, and the
/// meaning and the signature a contribution pins both change with it, since the door does.
#[test]
fn fmt_api_doc_and_the_pins_read_the_mark() {
    let d = fixtures();
    let (c, out, _) = run_in(&d, "en", &["fmt", "--check", "province_learning.rule", "州の学問.rule", "list.rule"]);
    assert_eq!(c, 0, "{out}");
    let t = TempDir::new("unique-fmt");
    let src = std::fs::read_to_string(d.join("province_learning.rule")).unwrap();
    // A table set out of line: fmt aligns it and leaves the field's line, `unique` with it, alone.
    std::fs::write(t.path().join("p.rule"), src.replace("| temple               | 1                        |", "| temple | 1 |")).unwrap();
    let (c, _, _) = run_in(t.path(), "en", &["fmt", "p.rule"]);
    assert_eq!(c, 0);
    assert_eq!(std::fs::read_to_string(t.path().join("p.rule")).unwrap(), src);

    let (c, out, _) = run_in(&d, "en", &["api", "province_learning.rule"]);
    assert_eq!(c, 0, "{out}");
    let j = rulec::json::parse(out.lines().next().unwrap()).unwrap();
    let pre = j.get("preconditions").and_then(|p| p.as_arr()).unwrap();
    assert!(pre.iter().any(|p| p.compact() == r#"{"kind":"unique","sequence":"buildings","field":"building"}"#), "{pre:?}");
    let field = j.get("python").and_then(|p| p.get("params")).and_then(|p| p.as_arr()).and_then(|p| p.first().cloned()).and_then(|p| p.get("elements").cloned()).unwrap();
    assert_eq!(field.compact(), r#"[{"name":"building","alias":"building","type":"Building","optional":false,"unique":true}]"#);
    let (_, out, _) = run_in(&d, "en", &["api", "list.rule"]);
    assert!(out.contains(r#""unique":true"#), "{out}");

    let (_, en, _) = run_in(&d, "en", &["doc", "province_learning.rule"]);
    assert!(en.contains("| building | building (3 values) |  | no two elements have the same value (`unique`) |"), "{en}");
    let (_, ja, _) = run_in(&d, "ja", &["doc", "州の学問.rule"]);
    assert!(ja.contains("同じ値の要素は二つありません（`unique`）"), "{ja}");

    // The meaning of the text, and the shape a contribution pins.
    let without = src.replace("building(building) : building  unique", "building(building) : building");
    assert_ne!(rulec::link::pin::meaning(&src), rulec::link::pin::meaning(&without));
    std::fs::write(t.path().join("province_learning.rule"), &src).unwrap();
    std::fs::create_dir_all(t.path().join("contrib")).unwrap();
    let small = std::fs::read_to_string(d.join("contrib/small_academy.rule")).unwrap();
    std::fs::write(t.path().join("contrib/small_academy.rule"), small).unwrap();
    let (c, out, _) = run_in(t.path(), "en", &["check", "province_learning.rule", "contrib/small_academy.rule"]);
    assert_eq!(c, 0, "{out}");
    std::fs::write(t.path().join("province_learning.rule"), &without).unwrap();
    let (c, out, _) = run_in(t.path(), "en", &["check", "province_learning.rule", "contrib/small_academy.rule"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("error[E040]"), "{out}");
}

/// Every language the door is generated for refuses two elements with the same value of a `unique`
/// field, with the sentence the reference evaluator says, held by `rulec test`: the province with the
/// academy that adds ten, and a rule with `unique` on a field of each kind — an enum, a number, a
/// string, a date, a truth value and an amount — so that each language compares each as a value.
/// SQL and NumPy do not walk a sequence and are not generated for either.
#[test]
fn every_language_refuses_two_elements_with_the_same_value() {
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let t = TempDir::new("unique-langs");
    std::fs::write(
        t.path().join("each_kind.rule"),
        "rule each_kind(each_kind) v1\n\n\
         enum building(building) = temple(temple) | library(library) | market(market)\n\n\
         elements lots(lots)\n\
         \x20 building(building) : building  unique\n\
         \x20 lot(lot)           : number  range >=1 <=99  unique\n\
         \x20 tag(tag)           : string  unique\n\
         \x20 opened(opened)     : date  range >=2026-01-01 <=2026-12-31  unique\n\
         \x20 coastal(coastal)   : bool  unique\n\
         \x20 price(price)       : money[JPY]  range >=0JPY <=1000JPY  unique\n\n\
         outputs\n  total(total) : money[JPY]  round down(1JPY)\n\n\
         sum price_total(price_total) over lots of price  range >=0JPY <=3000JPY\n\n\
         define total : money[JPY] = price_total\n",
    )
    .unwrap();
    let d = fixtures();
    let targets = [
        "--target", "python", "--target", "typescript", "--target", "javascript", "--target", "rust", "--target", "ruby", "--target", "php", "--target", "go",
        "--target", "swift", "--target", "java", "--target", "wasm", "--extra", "runner", "--extra", "vectors",
    ];
    for (dir, files, alias, fields) in [
        (d.clone(), vec!["province_learning.rule", "contrib/small_academy.rule"], "province_learning", &["building"][..]),
        (t.path().to_path_buf(), vec!["each_kind.rule"], "each_kind", &["building", "lot", "tag", "opened", "coastal", "price"][..]),
    ] {
        let out = t.path().join(format!("out-{alias}"));
        let o = out.to_string_lossy().into_owned();
        let mut args = vec!["gen"];
        args.extend(files.iter().copied());
        args.extend(["--out", o.as_str()]);
        args.extend(targets);
        let (c, said, e) = run_in(&dir, "en", &args);
        assert_eq!(c, 0, "{alias}: {said}{e}");
        let refused = std::fs::read_to_string(out.join("vectors").join(format!("{alias}.refused.jsonl"))).unwrap();
        let seq = if alias == "each_kind" { "lots" } else { "buildings" };
        for f in fields {
            assert!(refused.contains(&format!(r#""error":"{seq} has two elements with the same {f}""#)), "{alias} {f}:\n{refused}");
        }
        let tmp = TempDir::new("unique-langs-tmp");
        let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
            .env("RULEC_LANG", "en")
            .env("TMPDIR", tmp.path())
            .args(["test", &out.to_string_lossy(), "--format", "json"])
            .output()
            .expect("cannot start rulec");
        let said = String::from_utf8_lossy(&o.stdout).into_owned();
        let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result: {}", String::from_utf8_lossy(&o.stderr)))).unwrap_or_else(|x| panic!("{x}: {said}"));
        for why in j.get("skipped").and_then(|v| v.as_arr()).unwrap_or_default() {
            skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
        }
        let mut ran = Vec::new();
        for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
            if r.get("rule").and_then(|v| v.as_str()) != Some(alias) || r.get("ran").and_then(|v| v.as_bool()) != Some(true) {
                continue;
            }
            assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{alias}: {r:?}");
            assert!(r.get("refused").and_then(|v| v.as_int()).is_some_and(|n| n >= fields.len() as i128), "{alias}: {r:?}");
            ran.push(format!("{}/{}", r.get("lang").and_then(|v| v.as_str()).unwrap_or_default(), r.get("via").and_then(|v| v.as_str()).unwrap_or_default()));
        }
        assert!(ran.iter().any(|x| x.starts_with("python/")), "{alias}: Python did not run: {ran:?}");
        println!("unique: {alias} refused by {}", ran.join(", "));
    }
}
