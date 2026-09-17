//! `fold` (§15.56): the verdicts of a per-element table, reduced to one answer.
//!
//! The walk is checkable because a complete and unique table puts every element on exactly
//! one verdict of a finite enum, so what the fold has to answer for is a string over a finite
//! alphabet. What is tested here is the four holes that string can have — and that the
//! table's own proof is untouched by any of it.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-fold-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// A tariff sheet walked row by row: skip, halt, take, or hold the best so far.
const RULE: &str = r#"rule 全国運賃(freight) v1
description "運賃行を順に見て、一つの運賃に畳む"

enum 採用区分(verdict) = スキップ(skip) | 打ち切り(halt) | 確定(take) | 持ち越し(hold)
enum ゾーン区分(zone) = 近畿圏(kinki) | 遠隔地(remote)

elements 運賃行(freight_rows)
  行ゾーン(row_zone) : ゾーン区分
  閾値(threshold)    : money[円, incl_tax]  range >=0円 <=100万円
  行運賃(row_fee)    : money[円, incl_tax]  range >=0円 <=10万円

outputs
  運賃(fee) : money[円, incl_tax]  round up(1円)

table 行判定(row_of)
policy unique
| 行ゾーン | 閾値     | -> 採用(verdict) : 採用区分 |
| 近畿圏   | <=1000円 | 確定                        |
| 近畿圏   | >1000円  | 持ち越し                    |
| 遠隔地   | <=1000円 | スキップ                    |
| 遠隔地   | >1000円  | 打ち切り                    |

fold 採用 over 運賃行
  スキップ  -> next
  打ち切り  -> stop with 0円
  確定      -> take_unique 行運賃
  持ち越し  -> keep_max 行運賃 by 閾値
  empty     -> 0円
  exhausted -> held
"#;

fn write(d: &PathBuf, name: &str, body: &str) -> String {
    let p = d.join(name);
    std::fs::write(&p, body).unwrap();
    p.to_str().unwrap().to_string()
}

/// Whether a toolchain is on the PATH, so a missing one skips rather than fails.
fn have(cmd: &str) -> bool {
    Command::new("sh").args(["-c", &format!("command -v {cmd} >/dev/null 2>&1")]).status().map(|s| s.success()).unwrap_or(false)
}

fn codes(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|l| rulec::json::parse(l).ok())
        .filter_map(|j| j.get("code").and_then(|c| c.as_str()).map(|s| s.to_string()))
        .collect()
}

#[test]
fn 要素ごとの表はいままでどおり検査される() {
    let d = dir("ok");
    let p = write(&d, "r.rule", RULE);
    let (code, out, e) = run(&["check", &p]);
    assert_eq!(code, 0, "{out}{e}");

    // The element's own table is proved the way any table is: take a row away and the gap
    // comes back with the element that falls through it.
    let holed = RULE.replace("| 遠隔地   | >1000円  | 打ち切り                    |\n", "");
    let p = write(&d, "holed.rule", &holed);
    let (code, out, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert!(codes(&out).contains(&"E101".to_string()), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 空と終端の答えは宣言が要る() {
    let d = dir("answers");
    for (name, cut, want) in [
        ("empty", "  empty     -> 0円\n", "E022"),
        ("exhausted", "  exhausted -> held\n", "E023"),
    ] {
        let p = write(&d, &format!("{name}.rule"), &RULE.replace(cut, ""));
        let (code, out, _) = run(&["check", &p, "--format", "json"]);
        assert_eq!(code, 1, "{name}: {out}");
        assert!(codes(&out).contains(&want.to_string()), "{name}: {out}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 腕の無い判定は穴で_届かない腕は注意() {
    let d = dir("arms");
    // A verdict the table produces, with nothing to do about it.
    let p = write(&d, "hole.rule", &RULE.replace("  持ち越し  -> keep_max 行運賃 by 閾値\n", ""));
    let (code, out, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert!(codes(&out).contains(&"E024".to_string()), "{out}");
    assert!(out.contains("持ち越し"), "どの判定か言っていない: {out}");

    // An arm for a verdict no row can reach.
    let unreachable = RULE
        .replace("| 近畿圏   | >1000円  | 持ち越し                    |", "| 近畿圏   | >1000円  | スキップ                    |");
    let p = write(&d, "unreachable.rule", &unreachable);
    let (_, out, _) = run(&["check", &p, "--format", "json"]);
    assert!(codes(&out).contains(&"W115".to_string()), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The one the proposal called the biggest: a bare `take` cannot be written, so the author
/// chooses between "only one may match" and "the first wins" when the line is first typed.
#[test]
fn takeは一意か先頭かを書かせる() {
    let d = dir("take");
    let p = write(&d, "bare.rule", &RULE.replace("take_unique 行運賃", "take 行運賃"));
    let (code, out, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert!(codes(&out).contains(&"E021".to_string()), "{out}");
    assert!(out.contains("take_unique") && out.contains("take_first"), "選択肢を挙げていない: {out}");

    // Both spellings are accepted, and they are different rules.
    for arm in ["take_unique 行運賃", "take_first 行運賃"] {
        let p = write(&d, "arm.rule", &RULE.replace("take_unique 行運賃", arm));
        let (code, out, e) = run(&["check", &p]);
        assert_eq!(code, 0, "{arm}: {out}{e}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// A walk is written where it can be written, and the rest are named rather than left to
/// produce a file that cannot run (§15.56). SQL is the one that cannot: a single query has
/// nowhere to carry a value from row to row and stop early.
#[test]
fn 七言語に生成し_SQLは名指しで断る() {
    let d = dir("gen");
    let p = write(&d, "r.rule", RULE);
    let out = d.join("out");
    let (code, said, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");
    for lang in ["python", "typescript", "javascript", "rust", "ruby", "go", "swift"] {
        assert!(out.join(lang).exists(), "{lang} に生成されていない");
    }
    assert!(!out.join("sql").exists(), "SQL は書けないはず");
    assert!(said.contains("SQL"), "書けない言語を名指ししていない: {said}");

    let p = write(&d, "ex.rule", &format!("{RULE}\nexamples\n| 行ゾーン | 閾値 | 行運賃 | -> 運賃 |\n| 近畿圏 | 500円 | 800円 | 800円 |\n"));
    let (code, out, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert!(codes(&out).contains(&"E025".to_string()), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The inventory has to say the sequence is there, and say it the way the generated file
/// spells it — a caller that reads `rulec api` rather than the code would otherwise build a
/// call with one argument missing.
#[test]
fn 目録は列を欄として載せる() {
    let d = dir("api");
    let p = write(&d, "r.rule", RULE);
    let out = d.join("out");
    let (code, said, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");
    let (code, said, e) = run(&["api", &p]);
    assert_eq!(code, 0, "{e}");
    let j = rulec::json::parse(said.trim()).unwrap();

    for (lang, file) in [
        ("python", out.join("python/freight.py")),
        ("typescript", out.join("typescript/freight.ts")),
        ("javascript", out.join("javascript/freight.mjs")),
        ("rust", out.join("rust/freight.rs")),
        ("go", out.join("go/freight/freight.go")),
        ("swift", out.join("swift/freight.swift")),
    ] {
        let e = j.get(lang).unwrap();
        let src = std::fs::read_to_string(&file).unwrap();
        for k in ["signature", "traced_signature", "record_signature"] {
            let sig = e.get(k).and_then(|v| v.as_str()).unwrap();
            assert!(src.contains(sig), "{lang} の {k} が生成物に無い: {sig}");
        }
        // The sequence is a parameter of the inventory too, with the fields one element
        // carries — not just a name in a signature string.
        let key = if lang == "go" { "input_fields" } else { "params" };
        let rulec::json::Json::Arr(ps) = e.get(key).unwrap() else { panic!() };
        let seq = ps.iter().find(|p| p.get("name").and_then(|v| v.as_str()) == Some("運賃行")).expect("列が欄に無い");
        let rulec::json::Json::Arr(fs) = seq.get("elements").expect("要素の欄が無い") else { panic!() };
        assert_eq!(fs.len(), 3, "{lang}: 要素の欄が三つでない");
        let th = fs.iter().find(|f| f.get("name").and_then(|v| v.as_str()) == Some("閾値")).unwrap();
        assert!(th.get("range").is_some(), "{lang}: 要素の欄に範囲が無い");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// A DOM small enough to run the panel the page carries: elements, the five ids the script
/// looks up, and the listeners it attaches. Nothing here is a browser — what is being tested
/// is the generated script, not the rendering.
const DOM: &str = r##"
class El {
  constructor(tag) {
    this.tagName = tag.toUpperCase();
    this.children = [];
    this.listeners = {};
    this.classList = { add() {}, remove() {}, contains: () => false };
    this.style = {};
    this.value = "";
    this.checked = false;
    this._text = "";
    this.name = "";
  }
  get textContent() { return this._text; }
  set textContent(v) { this._text = String(v); }
  appendChild(c) { this.children.push(c); c.parentNode = this; return c; }
  remove() { const p = this.parentNode; if (p) p.children = p.children.filter((x) => x !== this); }
  addEventListener(k, f) { (this.listeners[k] ||= []).push(f); }
  click() { for (const f of this.listeners.click || []) f(); }
  querySelectorAll() { return []; }
  get elements() {
    const o = {};
    const walk = (n) => { if (n.name) o[n.name] = n; n.children.forEach(walk); };
    this.children.forEach(walk);
    return o;
  }
}
const ids = {};
for (const id of ["try-form", "try-rows", "try-buttons", "try-result", "try-record"]) {
  ids["#" + id] = new El(id === "try-form" ? "form" : "div");
}
globalThis.document = { createElement: (t) => new El(t), querySelector: (s) => ids[s] ?? null, querySelectorAll: () => [] };
globalThis.CSS = { escape: (s) => s };
globalThis.history = { replaceState() {} };
globalThis.location = { hash: "" };
globalThis.window = { addEventListener() {} };
globalThis.window.parent = globalThis.window;
export { ids };
"##;

/// Driving that panel: two rows, then Compute, then both rows removed and Compute again.
const DRIVE: &str = r##"
const box = ids["#try-rows"];
box.children[box.children.length - 1].click();  // one more row
const body = box.children[1].children[1];
const set = (r, vals) => { const tds = body.children[r].children; vals.forEach((v, i) => { tds[i].children[0].value = v; }); };
set(0, ["近畿圏", "500", "800"]);
set(1, ["近畿圏", "2000", "1500"]);
ids["#try-buttons"].children[0].click();
console.log(JSON.stringify({ result: ids["#try-result"].textContent, record: ids["#try-record"].textContent }));
body.children.slice().forEach((tr) => tr.children[tr.children.length - 1].children[0].click());
ids["#try-buttons"].children[0].click();
console.log(JSON.stringify({ result: ids["#try-result"].textContent, record: ids["#try-record"].textContent }));
"##;

/// The page an approver reads is generated for a walk too, and a page that cannot run is
/// worse than no page: the panel has to build an editor for the sequence and pass it as the
/// argument the module takes (§15.52, §15.56).
#[test]
fn ページの試用欄は列を編集して走る() {
    if !have("node") {
        eprintln!("注意: node が無いので飛ばした");
        return;
    }
    let d = dir("page");
    let p = write(&d, "r.rule", RULE);
    let out = d.join("out");
    let (code, said, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");

    // The page's own module script, with the stub in front of it and the driver behind.
    let html = std::fs::read_to_string(out.join("javascript/freight_page.html")).unwrap();
    let head = html.find("<script type=\"module\">").expect("ページに script が無い") + "<script type=\"module\">".len();
    let body = &html[head..head + html[head..].find("</script>").expect("script が閉じていない")];
    write(&d, "dom.mjs", DOM);
    let p = write(&d, "page.mjs", &format!("import {{ ids }} from \"./dom.mjs\";\n{body}\n{DRIVE}"));

    let o = Command::new("node").arg(&p).output().expect("node を起動できない");
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(o.status.success(), "{said}{}", String::from_utf8_lossy(&o.stderr));
    let mut lines = said.lines();
    let two = rulec::json::parse(lines.next().expect("一行目が無い")).unwrap();
    let none = rulec::json::parse(lines.next().expect("二行目が無い")).unwrap();

    // 500円 is 確定 and 2000円 is 持ち越し, so the taken value wins.
    assert!(two.get("result").and_then(|v| v.as_str()).unwrap().contains("800"), "{said}");
    let rec = two.get("record").and_then(|v| v.as_str()).unwrap();
    assert!(rec.contains("\"運賃行\":[{"), "列が記録に無い: {rec}");
    assert!(rec.contains("\"閾値\":2000"), "二行目が記録に無い: {rec}");
    // Both rows removed: the empty sequence is its own answer, not a crash.
    assert!(none.get("result").and_then(|v| v.as_str()).unwrap().contains("0"), "{said}");
    assert!(none.get("record").and_then(|v| v.as_str()).unwrap().contains("\"運賃行\":[]"), "{said}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The seven generated walks answer what the reference evaluator answered, over the whole
/// vector suite. A language whose toolchain is missing is skipped by `rulec test` itself.
#[test]
fn 生成された歩きは参照評価器と一致する() {
    if !have("python3") {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    let d = dir("agree");
    let p = write(&d, "r.rule", RULE);
    let out = d.join("out");
    let (code, said, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");

    let (_, said, _) = run(&["test", out.to_str().unwrap(), "--format", "json"]);
    let j = rulec::json::parse(said.lines().next().expect("結果が無い")).unwrap();
    let rulec::json::Json::Arr(rs) = j.get("results").unwrap() else { panic!("{said}") };
    let mut ran = 0;
    for r in rs {
        if r.get("ran") != Some(&rulec::json::Json::Bool(true)) {
            continue;
        }
        ran += 1;
        assert_eq!(r.get("ok"), Some(&rulec::json::Json::Bool(true)), "{said}");
    }
    assert!(ran >= 1, "どの言語も走らなかった: {said}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 列は一つで_二本目は断る() {
    let d = dir("two");
    let p = write(&d, "two.rule", &RULE.replace("outputs\n", "elements 別の列(others)\n  m(m) : number  range >=0 <=9\n\noutputs\n"));
    let (code, out, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert!(codes(&out).contains(&"E020".to_string()), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The walk itself: the evaluator's answers, and the suite that covers them (§15.56).
#[test]
fn 歩きはベクタで覆われる() {
    let d = dir("vectors");
    let p = write(&d, "r.rule", RULE);
    let (code, out, e) = run(&["vectors", &p]);
    assert_eq!(code, 0, "{e}");

    let mut lengths: std::collections::BTreeSet<usize> = Default::default();
    let mut answers: std::collections::BTreeSet<i128> = Default::default();
    for line in out.lines().filter(|l| !l.trim().is_empty()) {
        let j = rulec::json::parse(line).unwrap();
        let seq = j.get("in").and_then(|i| i.get("運賃行")).expect("列が in に無い");
        let rulec::json::Json::Arr(xs) = seq else { panic!("列が配列でない: {line}") };
        lengths.insert(xs.len());
        // Every element carries its own fields, as integers in the canonical unit.
        for x in xs {
            assert!(x.get("行ゾーン").is_some() && x.get("閾値").is_some(), "{line}");
        }
        let v = j.get("out").and_then(|o| o.get("運賃")).and_then(|v| v.as_int()).expect("答えが無い");
        answers.insert(v);
    }
    assert!(lengths.contains(&0) && lengths.contains(&1) && lengths.contains(&2), "長さが揃っていない: {lengths:?}");
    // A suite where every case answers the same thing would not tell a wrong walk from a
    // right one.
    assert!(answers.len() >= 2, "答えが一種類しかない: {answers:?}");

    // The fifth criterion is reported, and what it cannot cover it names.
    let (code, cov, _) = run(&["coverage", &p, "--format", "json"]);
    assert_eq!(code, 1, "覆えない義務があるので 1");
    let j = rulec::json::parse(cov.lines().next().unwrap()).unwrap();
    let rulec::json::Json::Arr(cs) = j.get("criteria").unwrap() else { panic!() };
    let fold = cs
        .iter()
        .find(|c| c.get("name").and_then(|n| n.as_str()) == Some("fold_transition"))
        .expect("fold_transition が無い");
    let total = fold.get("total").and_then(|v| v.as_int()).unwrap();
    let met = fold.get("satisfied").and_then(|v| v.as_int()).unwrap();
    assert_eq!(total, 21, "義務は ゼロ件 + 判定4 + 対16");
    assert_eq!(met, 20, "覆えるのは take_unique の矛盾を除く 20");
    assert!(cov.contains("確定"), "覆えない遷移を名指ししていない: {cov}");
    let _ = std::fs::remove_dir_all(&d);
}
