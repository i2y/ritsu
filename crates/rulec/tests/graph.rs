//! `rulec graph` — the rule as one graph (§15.117).
//!
//! The graph is a **second rendering** of what `doc` already prints under every table, and
//! the point of a second rendering is that it can be held to the first. Everything else
//! here is the same discipline: the edges land on real nodes, the shapes the drawing will
//! be about are actually in the corpus, and the three frames a picture needs — the element,
//! the apply, the wall the caller crosses — are on the nodes rather than in the reader's
//! head.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.code() == Some(0) || o.status.code() == Some(1), "{}", String::from_utf8_lossy(&o.stderr));
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

fn graph(rule: &str) -> rulec::json::Json {
    let (c, out) = run(&["graph", rule]);
    assert_eq!(c, 0, "{rule}");
    rulec::json::parse(out.trim()).unwrap_or_else(|e| panic!("{rule}: JSON として読めない: {e}"))
}

fn arr<'a>(j: &'a rulec::json::Json, k: &str) -> &'a Vec<rulec::json::Json> {
    match j.get(k) {
        Some(rulec::json::Json::Arr(a)) => a,
        _ => panic!("{k} が配列でない"),
    }
}

fn s(j: &rulec::json::Json, k: &str) -> String {
    j.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string()
}

fn corpus() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(root().join("tests/corpus"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rule"))
        .map(|p| format!("tests/corpus/{}", p.file_name().unwrap().to_string_lossy()))
        .collect();
    v.sort();
    v
}

/// Every edge lands on a node the graph declares, and every value has something that
/// decides it. A dangling edge would draw an arrow from nowhere.
#[test]
fn 辺は実在する節点をつなぎ_どの値にも出どころがある() {
    for rule in corpus() {
        let g = graph(&rule);
        let names: Vec<String> = arr(&g, "nodes").iter().map(|n| s(n, "name")).collect();
        for e in arr(&g, "edges") {
            for end in ["from", "to"] {
                let n = s(e, end);
                assert!(names.contains(&n), "{rule}: 辺が知らない節点 {n} を指している");
            }
        }
        for n in arr(&g, "nodes") {
            if s(n, "kind") == "value" {
                assert!(
                    !arr(n, "by").is_empty(),
                    "{rule}: 値 {} に出どころが無い",
                    s(n, "name")
                );
            }
        }
    }
}

/// **The graph reaches the answer.** Every value the rule declares as an output has a node,
/// whatever decided it. A `result` line is the case this was written for: it decides the
/// output in one line and appears in no `item`, so a graph built by walking the items alone
/// stops one step short of the value the caller asked for — and the board then draws every
/// decider except the one that produced the answer.
#[test]
fn 宣言した出力はすべて_グラフの節点になる() {
    for rule in corpus() {
        let g = graph(&rule);
        let outs: Vec<String> = arr(&g, "nodes")
            .iter()
            .filter(|n| matches!(n.get("output"), Some(rulec::json::Json::Bool(true))))
            .map(|n| s(n, "name"))
            .collect();
        let src = std::fs::read_to_string(root().join(&rule)).expect(&rule);
        for line in src.lines() {
            if let Some(r) = line.strip_prefix("result ") {
                let name = r.split('=').next().unwrap_or("").trim();
                let name = name.split('(').next().unwrap_or(name).trim();
                assert!(
                    outs.iter().any(|o| o == name),
                    "{rule}: result {name} に節点が無い（出力の節点は {outs:?}）"
                );
            }
        }
        assert!(!outs.is_empty(), "{rule}: 出力の節点が一つも無い");
    }
}

/// **The graph says what `doc` says.** Under every table, `doc` prints the columns it reads
/// and where each comes from; the graph is that same relation gathered up. Two renderings of
/// one fact drift the moment nobody holds them together, and then the picture quietly stops
/// being about the rule.
#[test]
fn 出どころは_docが表ごとに言うことと一致する() {
    for rule in corpus() {
        let (c, doc) = run(&["doc", &rule, "--lang", "en"]);
        assert_eq!(c, 0, "{rule}");
        let g = graph(&rule);
        let mut table: Option<String> = None;
        let mut in_cols = false;
        for line in doc.lines() {
            if let Some(rest) = line.strip_prefix("## Table ") {
                table = rest.split(" (").next().map(|s| s.to_string());
                in_cols = false;
            } else if let Some(rest) = line.strip_prefix("## Clause ") {
                table = rest.split(" →").next().map(|s| s.trim().to_string());
                in_cols = false;
            } else if line.starts_with("| Column | Source |") {
                in_cols = true;
            } else if in_cols {
                let cell = line.trim_start_matches('|').split('|').next().unwrap_or("").trim().to_string();
                if cell.is_empty() || cell.starts_with("---") {
                    continue;
                }
                if cell.starts_with('→') {
                    in_cols = false;
                    continue;
                }
                let Some(t) = table.as_ref() else { continue };
                let found = arr(&g, "edges").iter().any(|e| &s(e, "via") == t && s(e, "from") == cell);
                assert!(
                    found,
                    "{rule}: doc は 表 {t} が {cell} を読むと言っているのに、グラフに辺が無い"
                );
            }
        }
    }
}

/// The shape the picture is for: two columns above, both cut from one input. It is what
/// §15.114 was about, and in the text it is two tables read side by side.
#[test]
fn 同じ入力から出た二つの列が_下で合流するのが見える() {
    let g = graph("tests/corpus/二つの区分.rule");
    let up: Vec<String> = arr(&g, "edges")
        .iter()
        .filter(|e| s(e, "from") == "重量")
        .map(|e| s(e, "to"))
        .collect();
    assert_eq!(up.len(), 2, "重量 から二つ出ていない: {up:?}");
    for down in ["料金", "手数料"] {
        let feeds: Vec<String> =
            arr(&g, "edges").iter().filter(|e| s(e, "to") == down).map(|e| s(e, "from")).collect();
        for u in &up {
            assert!(feeds.contains(u), "{down} が {u} を読んでいない: {feeds:?}");
        }
    }
}

/// The element frame. A value decided from a field of an element is decided once per
/// element; a `sum`, a `count` and a `fold` are the ways out. Nothing in the rule's text
/// says which side of that line a value is on.
#[test]
fn 要素ごとに決まる値に印が付く() {
    let g = graph("tests/corpus/全国運賃.rule");
    let per = |n: &str| {
        arr(&g, "nodes")
            .iter()
            .find(|x| s(x, "name") == n)
            .unwrap_or_else(|| panic!("{n} が無い"))
            .get("per_element")
            .is_some()
    };
    assert!(per("採用"), "要素ごとの表の出力に印が無い");
    assert!(!per("運賃"), "畳んだあとの答えに印が付いている");
    let walk: Vec<String> = arr(&g, "edges")
        .iter()
        .filter(|e| s(e, "kind") == "walk")
        .map(|e| format!("{}→{}", s(e, "from"), s(e, "to")))
        .collect();
    assert!(!walk.is_empty(), "枠を出る辺が無い: {walk:?}");
}

/// The apply frame: what another rule brought in says which apply it came through, so a
/// picture can put a border round it and name the file it is from (§15.69).
#[test]
fn 準用が持ち込んだ値は_どの準用から来たかを言う() {
    let g = graph("tests/corpus/非常勤退職手当.rule");
    let brought: Vec<String> = arr(&g, "nodes")
        .iter()
        .filter(|n| n.get("from_apply").is_some())
        .map(|n| s(n, "name"))
        .collect();
    assert!(!brought.is_empty(), "準用から来た値が一つも印を持っていない");
    for n in &brought {
        assert!(n.starts_with("退職手当:"), "{n} に準用の名前が付いていない");
    }
}

/// The wall. What the caller has to satisfy at every crossing travels with the graph, in
/// the same form `api` gives it — a drawing that stops at the rule's own boundary hides the
/// half the caller has to get right (§15.116).
#[test]
fn 呼び手が満たすべき前提が_グラフにも乗る() {
    for rule in ["tests/corpus/買物かごの送料.rule", "tests/corpus/比例配分.rule", "tests/corpus/送料.rule"] {
        let g = graph(rule);
        let (c, api) = run(&["api", rule]);
        assert_eq!(c, 0);
        let a = rulec::json::parse(api.trim()).unwrap();
        assert_eq!(
            arr(&g, "preconditions").len(),
            arr(&a, "preconditions").len(),
            "{rule}: graph と api で前提の数が違う"
        );
    }
}

/// **A graph needs less than a proof.** Which value is read while which other is decided
/// is settled once the names and the types resolve: a table with a gap in it has the same
/// edges as one without, and the picture is most wanted while the rule is still being
/// fixed. So a rule that fails E101 still has a graph — and one whose names do not resolve
/// does not, because then there is nothing to draw an edge between.
#[test]
fn グラフは検査より手前で出る() {
    let d = std::env::temp_dir().join(format!("rulec-graph-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();

    let hole = d.join("hole.rule");
    std::fs::write(&hole, "rule t(t) v1\n\ninputs\n  x(x) : bool\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| x | -> r(r) : bool |\n| true | true |\n").unwrap();
    let (c, _) = run(&["check", hole.to_str().unwrap()]);
    assert_eq!(c, 1, "穴のある規則が check を通ってしまった");
    let g = graph(hole.to_str().unwrap());
    assert_eq!(arr(&g, "edges").len(), 1, "穴があっても辺は同じはず");

    let broken = d.join("broken.rule");
    std::fs::write(&broken, "rule t(t) v1\n\ninputs\n  x(x) : bool\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| しらない列 | -> r(r) : bool |\n| true | true |\n").unwrap();
    let (c, out) = run(&["graph", broken.to_str().unwrap()]);
    assert_eq!(c, 1, "名前が解決しないのにグラフが出た: {out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **The board is the data, laid out.** Every card is a decider of `rulec graph` and every
/// wire an edge between two of them, so the page's picture cannot quietly start being about
/// something else. The page carries the graph as data and builds the board from the document
/// it already is — which is why a reader with no script still has that document.
#[test]
fn ページの盤面は_グラフそのものから組み立てられる() {
    for rule in ["tests/corpus/二つの区分.rule", "tests/corpus/全国運賃.rule", "tests/corpus/非常勤退職手当.rule"] {
        let g = graph(rule);
        let (c, html) = run(&["doc", rule, "--format", "html"]);
        assert_eq!(c, 0, "{rule}");
        let i = html.find("const GRAPH = ").expect("盤面のデータがページに無い");
        let tail = &html[i + "const GRAPH = ".len()..];
        let data = rulec::json::parse(tail[..tail.find(";\n").unwrap()].trim())
            .unwrap_or_else(|e| panic!("{rule}: 盤面のデータが JSON として読めない: {e}"));

        // Every decider is a card, in exactly one column, and nothing else is.
        let want: Vec<String> = arr(&g, "nodes")
            .iter()
            .filter(|n| n.get("by").is_some())
            .map(|n| s(n, "name"))
            .collect();
        let got: Vec<String> = arr(&data, "nodes").iter().map(|n| s(n, "v")).collect();
        assert_eq!(got.len(), want.len(), "{rule}: 箱の数が決め手の数と違う\n{got:?}\n{want:?}");
        for w in &want {
            assert!(got.contains(w), "{rule}: {w} の箱が無い");
        }
        let placed: Vec<String> = match data.get("cols") {
            Some(rulec::json::Json::Arr(cs)) => cs
                .iter()
                .flat_map(|c| match c {
                    rulec::json::Json::Arr(v) => v.iter().filter_map(|x| x.as_str().map(|t| t.to_string())).collect(),
                    _ => Vec::new(),
                })
                .collect(),
            _ => panic!("cols が配列でない"),
        };
        assert_eq!(placed.len(), want.len(), "{rule}: 列に置かれた箱の数が合わない: {placed:?}");

        // A wire for every edge between two deciders, and no other.
        let want_e: Vec<String> = arr(&g, "edges")
            .iter()
            .filter(|e| want.contains(&s(e, "from")) && want.contains(&s(e, "to")))
            .map(|e| format!("{}→{}", s(e, "from"), s(e, "to")))
            .collect();
        let got_e: Vec<String> =
            arr(&data, "edges").iter().map(|e| format!("{}→{}", s(e, "from"), s(e, "to"))).collect();
        for w in &want_e {
            assert!(got_e.contains(w), "{rule}: 矢印 {w} が無い");
        }
        assert_eq!(got_e.len(), want_e.len(), "{rule}: 矢印の数が違う");

        // The document is still the document: the board is what the script makes of it.
        assert!(html.contains("<main>"), "{rule}: 文書そのものが無い");
        // A card finds its table through a row, not a heading: the row's `data-t` is the
        // name the trace uses, the same in either language and after an `apply` renamed it.
        for t in arr(&data, "nodes").iter().flat_map(|n| match n.get("tables") {
            Some(rulec::json::Json::Arr(a)) => a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>(),
            _ => Vec::new(),
        }) {
            assert!(
                html.contains(&format!("data-t=\"{t}\"")),
                "{rule}: 表 {t} の行に印が無いので、盤面が取り出せない"
            );
        }
    }
}

/// The trace lights the board in the colour the rows light in, and the answer lands in the
/// card that decides it. The page already knew which rows matched; what the board adds is
/// *where* they are, and that has to come from the same list or it is a second opinion.
#[test]
fn 盤面は当てはまった行と同じ色で光る() {
    let (c, html) = run(&["doc", "tests/corpus/二つの区分.rule", "--format", "html"]);
    assert_eq!(c, 0);
    for want in [
        "rulecBoardFill(trace, outs)",   // out of the very trace the rows come from
        "window.rulecBoardFill = function",
        ".gcard.sel",                     // the one card that carries colour
        "tr.hit td { background: #ffe9a8; }", // and the rows, in the same colour
        "mk(\"div\", \"split\")",   // both borders move
        "mk(\"div\", \"hsplit\")",
    ] {
        assert!(html.contains(want), "ページに `{want}` が無い");
    }
}
