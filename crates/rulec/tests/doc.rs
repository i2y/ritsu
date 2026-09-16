//! Rendering of a checked rule (§1.6). The surface for the people who approve it.
//!
//! The danger of this surface is that it "starts saying things it has not proven", so the tests
//! focus not on appearance but on **the correctness of what it says**. Given a rule whose groups do
//! not form a partition, it must not say "partitions exactly"; it must not put text that is not in
//! the source into a table; and it must not render a rule that fails the check prettily.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

const CORPUS: &[&str] = &[
    "tests/corpus/ゆうパック運賃.rule",
    "tests/corpus/クーポン割引.rule",
    "tests/corpus/クーポン併用.rule",
    "tests/corpus/送料.rule",
    "tests/corpus/期間区分.rule",
    "tests/corpus/適用順序.rule",
    "tests/corpus/クーポン一枚.rule",
    "tests/corpus/値引の充当.rule",
    "tests/corpus/決済手数料.rule",
    "tests/corpus/ポイント付与.rule",
    "tests/corpus/評価ランク.rule",
    "tests/corpus/会員特典.rule",
    "tests/corpus/ec261.rule",
    "tests/corpus/健康保険料.rule",
    "tests/corpus/厚生年金保険料.rule",
    "tests/corpus/所得税.rule",
    "tests/corpus/領収書の印紙税.rule",
    "tests/corpus/印紙税.rule",
];

/// A rule whose groups are broken in one of three ways. Every variant passes `check` (the `-` row
/// carries completeness).
fn groups_rule(g1: &str, g2: &str) -> String {
    format!(
        "rule t(t) v1\n\nenum 色(color) = 赤(r) | 青(b) | 緑(g)\n\
         group 暖色(warm) = {g1}\ngroup 寒色(cool) = {g2}\n\n\
         inputs\n  c(c) : 色\n  b(b) : bool\n\noutputs\n  r(r) : bool\n\n\
         table x(x)\npolicy first\n| c | b | -> r(r) : bool |\n\
         | 暖色 | true | true |\n| 寒色 | true | false |\n| - | - | false |\n"
    )
}

fn doc_of(src: &str, name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("rulec-doc-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("t.rule");
    std::fs::write(&p, src).unwrap();
    let (c, out, e) = run(&["doc", p.to_str().unwrap()]);
    assert_eq!(c, 0, "資料を書き出せない: {out}{e}");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// The heart of §1.6. Of what a single group name hides, say **only what can be said**.
#[test]
fn グループのカバーは事実のとおりに言う() {
    // A partition.
    let d = doc_of(&groups_rule("赤", "青, 緑"), "part");
    assert!(d.contains("この 2 グループは 色 の 3 値を過不足なく分割しています"), "{d}");

    // Covers, but overlaps. It must not say "partition".
    let d = doc_of(&groups_rule("赤, 緑", "青, 緑"), "dup");
    assert!(!d.contains("過不足なく分割"), "重なっているのに分割だと言っている:\n{d}");
    assert!(d.contains("すべて覆いますが、緑 が二つ以上のグループに入っています"), "{d}");

    // Does not cover everything. Name the gap so the approver can ask "what happens to 緑?".
    let d = doc_of(&groups_rule("赤", "青"), "gap");
    assert!(!d.contains("過不足なく分割"), "覆えていないのに分割だと言っている:\n{d}");
    assert!(d.contains("覆うのは 色 の 3 値のうち 2 値で、緑 はどのグループにも入っていません"), "{d}");

    // State where the fact comes from (whether the checker verified it or the renderer counted it).
    assert!(d.contains("この資料が宣言から数えました"), "{d}");
}

/// Prohibition: never write prose that is not in the checker's output. Table cells keep the exact
/// text of the source.
#[test]
fn 表のセルはもとの規則までたどれる() {
    for rel in CORPUS {
        let src = std::fs::read_to_string(root().join(rel)).unwrap();
        let (c, out, e) = run(&["doc", rel]);
        assert_eq!(c, 0, "{rel}: {e}");
        // Count separator lines per section. Under `## 表` the "column | origin" table comes first,
        // so the data table is the second one; `## 例` has only one, so it is the first.
        let mut want: Option<usize> = None;
        let mut seen = 0usize;
        let mut checked = 0;
        for l in out.lines() {
            if l.starts_with("## ") {
                want = if l.starts_with("## 表 ") {
                    Some(2)
                } else if l.starts_with("## 例") {
                    Some(1)
                } else {
                    None
                };
                seen = 0;
                continue;
            }
            if l.starts_with("|---") {
                seen += 1;
                continue;
            }
            if want != Some(seen) || !l.starts_with("| ") || l.contains("→") {
                continue;
            }
            let cells: Vec<&str> = l.trim_matches('|').split('|').map(|c| c.trim()).collect();
            // Skip the first cell, the row number (the examples table has none).
            for cell in cells.iter().skip(1) {
                if cell.is_empty() || cell.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let raw = cell.replace("\\|", "|");
                assert!(
                    src.contains(&raw),
                    "{rel}: もとの規則に無い字が表に出ている: `{raw}`\n行: {l}"
                );
                checked += 1;
            }
        }
        assert!(checked > 5, "{rel}: セルを一つも検査していない");
    }
}

/// §1.6: a pretty rendering of a broken rule is a lie. If it fails the check, do not render.
#[test]
fn 検査を通らない規則は描かない() {
    let (c, out, e) = run(&["doc", "tests/mutants/m_e101.rule"]);
    assert_eq!(c, 1, "エラーのある規則を描いた");
    assert!(out.contains("error[E101]"), "何が悪いかを言う: {out}");
    assert!(e.contains("資料を書き出しません"), "{e}");
    assert!(!out.contains("# 規則"), "資料が書き出され始めている: {out}");
}

/// The greatest danger is a stale rendering that lingers looking authoritative (§1.6). The stamp
/// must make staleness checkable.
#[test]
fn もとの規則の刻印が入る() {
    let rel = "tests/corpus/送料.rule";
    let (_, out, _) = run(&["doc", rel]);
    let first = out.lines().next().unwrap();
    assert!(first.contains(rel), "もとの規則のパスを刻む: {first}");
    assert!(first.contains(&format!("rulec {}", env!("CARGO_PKG_VERSION"))), "道具の版を刻む: {first}");
    assert!(first.contains("sha256:"), "もとの規則のハッシュを刻む: {first}");
    assert!(first.contains("本物は .rule のほう"), "一方向であることを言う: {first}");

    // Changing one character of the source changes the stamp.
    let dir = std::env::temp_dir().join(format!("rulec-doc-stamp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::fs::read_to_string(root().join(rel)).unwrap();
    let p = dir.join("送料.rule");
    let n = src.matches("1100円").count();
    assert_eq!(n, 1, "書き換えが当たっていない");
    std::fs::write(&p, src.replace("1100円", "1110円")).unwrap();
    let (_, other, _) = run(&["doc", p.to_str().unwrap()]);
    let h = |s: &str| s.lines().next().unwrap().split("sha256:").nth(1).unwrap()[..12].to_string();
    assert_ne!(h(&out), h(&other), "もとの規則が変わったのに刻印が同じ");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Must be deterministic. If what gets pasted into a PR wobbled from run to run, the diff would be
/// unreadable.
#[test]
fn 書き出す資料は決定的である() {
    for rel in CORPUS {
        let (_, a, _) = run(&["doc", rel]);
        let (_, b, _) = run(&["doc", rel]);
        assert_eq!(a, b, "{rel}: 二度で違う");
    }
}

/// Nothing an approver needs to know is dropped (the list in §1.6).
#[test]
fn 承認者が知るべきことが載る() {
    // W114 and the presence of the guard.
    let (_, d, _) = run(&["doc", "tests/corpus/クーポン併用.rule"]);
    assert!(d.contains("証明できていません**（W114）"), "{d}");
    assert!(d.contains("ガード"), "ガードの存在を言う: {d}");

    // The three shadowing classes, and the needs-confirmation row pair plus its witness.
    let (_, d, _) = run(&["doc", "tests/corpus/送料.rule"]);
    assert!(d.contains("**階段** 2") && d.contains("**要確認** 1"), "{d}");
    assert!(d.contains("要確認: 同じ入力が 行1 と 行2"), "どの行対かを言う: {d}");
    assert!(d.contains("両方に当てはまる例:"), "証人を出す: {d}");
    assert!(d.contains("既定行"), "既定行を指す: {d}");
    assert!(d.contains("| 大口 | 定義 |"), "定義の式を出す: {d}");

    // Table dependencies.
    let (_, d, _) = run(&["doc", "tests/corpus/ゆうパック運賃.rule"]);
    assert!(d.contains("→ 表 運賃表 の入力列"), "出力の行き先を言う: {d}");
    assert!(d.contains("`contract_only`"), "宣言意図を言う: {d}");
    assert!(d.contains("この 3 件は `rulec check` が参照評価器で実行し"), "例が検証済みだと言う: {d}");

    // default, and the range of a derivation.
    let (_, d, _) = run(&["doc", "tests/corpus/クーポン割引.rule"]);
    assert!(d.contains("店内全商品（default）"), "{d}");
    assert!(d.contains("E112"), "導出範囲が検査済みだと言う: {d}");

    // The origin of a provisionally placed rounding must surface (the §7.2 practice, a breakwater
    // against the third danger in §16).
    let (_, d, _) = run(&["doc", "tests/corpus/クーポン一枚.rule"]);
    assert!(d.contains("down(1円)"), "丸めを列に畳む: {d}");
}

/// Where a table came from is written as a comment at the end of the `table` line, and a row
/// taken from somewhere else carries its own (§15.32). Both are source text, so the rendering
/// may show them — and the approver is who they are for.
#[test]
fn 出典のコメントが承認者に届く() {
    let src = "rule t(t) v1\n\nenum 色(color) = 赤(r) | 青(b)\n\ninputs\n  c(c) : 色\n\n\
               outputs\n  n(n) : number  round down(1)\n\n\
               table x(x)  # 出典: 料金表 2026-04 版 p.3\npolicy unique\n\
               | c  | -> n(n) : number |\n| 赤 | 1                |\n\
               | 青 | 2                |  # 出典: 改定のお知らせ 2026-06\n\n\
               examples\n| c  | -> n |\n| 赤 | 1    |  # 料金表の計算例 1\n";
    let d = doc_of(src, "source");
    // The table's own source sits right under its heading, before anything the rendering adds.
    assert!(d.contains("## 表 x（policy unique）\n\n出典: 料金表 2026-04 版 p.3\n\n| 列 |"), "{d}");
    // A row's comment becomes a notes column; a row without one has an empty cell there.
    assert!(d.contains("| 2 | 青 | 2 | 出典: 改定のお知らせ 2026-06 |\n"), "{d}");
    assert!(d.contains("| 1 | 赤 | 1 |  |\n"), "{d}");
    assert!(d.contains("| 赤 | 1 | 料金表の計算例 1 |\n"), "{d}");

    // No comment, no column: a table without any renders exactly as before.
    let d = doc_of(&groups_rule("赤", "青, 緑"), "plain");
    assert!(d.contains("| # | c | b | → r（bool） |\n"), "{d}");
    assert!(!d.contains("| 注記 |\n|---|---|---|---|---|"), "コメントの無い表に注記の列が出ている:\n{d}");
}

/// The page the approver can try a case on (§15.37): the same document, with every table row
/// carrying its table's name and number so the script can light it up, the generated
/// JavaScript module inside it, and a description of the inputs for the form. The module is
/// the one `rulec gen` writes, so node has to accept it as it stands.
#[test]
fn html版は表の行に名前と番号を持ち_生成したjavascriptを積む() {
    let (c, html, e) = run(&["doc", "tests/corpus/送料.rule", "--format", "html"]);
    assert_eq!(c, 0, "{e}");
    assert!(html.starts_with("<!doctype html>"), "{}", &html[..60]);
    assert!(html.contains("<tr data-t=\"基本送料\" data-r=\"1\">"), "{html}");
    assert!(html.contains("<tr data-t=\"負担判定\" data-r=\"3\">"), "{html}");
    assert!(html.contains("<section id=\"try\">"), "試す欄が無い");
    assert!(html.contains("export function shipping_fee_traced("), "生成 JavaScript が入っていない");
    assert!(html.contains("const FN = { run: shipping_fee_traced, record: shipping_fee_record };"), "{html}");
    assert!(html.contains("\"inputs\":[{\"name\":\"届け先\",\"alias\":\"dest\",\"kind\":\"enum\""), "{html}");
    assert!(html.contains("\"examples\":[{\"届け先\":\"沖縄県\",\"重量\":\"2500\""), "例がワイヤ形式で入る: {html}");
    // A case is a link: `?example=2` and `?dest=…&weight=…` open the page on it.
    assert!(html.contains("params.get(\"example\")") && html.contains("history.replaceState"), "住所に件が乗らない: {html}");
    // Every cell of every data table is on the page, as in the markdown.
    let (_, md, _) = run(&["doc", "tests/corpus/送料.rule"]);
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    for l in md.lines().filter(|l| l.starts_with("| ") && !l.contains("→")) {
        for cell in l.trim_matches('|').split('|').map(|c| c.trim()).filter(|c| !c.is_empty()) {
            // A code span or bold in a cell becomes markup around the same text.
            let plain = cell.replace("\\|", "|").replace('`', "").replace("**", "");
            assert!(html.contains(&esc(&plain)), "セル `{cell}` が HTML に無い");
        }
    }
    // The module inside the page is what node runs: a syntax error here is a broken page.
    if have("node") {
        let start = html.find("<script type=\"module\">\n").expect("script が無い") + "<script type=\"module\">\n".len();
        let end = html[start..].find("</script>").unwrap() + start;
        let dir = std::env::temp_dir().join(format!("rulec-doc-html-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("page.mjs");
        std::fs::write(&p, &html[start..end]).unwrap();
        let o = Command::new("node").args(["--check"]).arg(&p).output().expect("node を起動できない");
        assert!(o.status.success(), "ページの script を node が読めない:\n{}", String::from_utf8_lossy(&o.stderr));
        let _ = std::fs::remove_dir_all(&dir);
    }
    // `--out` writes `<alias>.html`.
    let dir = std::env::temp_dir().join(format!("rulec-doc-html-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let d = dir.to_string_lossy().to_string();
    let (c, out, _) = run(&["doc", "tests/corpus/送料.rule", "--format", "html", "--out", &d]);
    assert_eq!(c, 0, "{out}");
    assert!(dir.join("shipping_fee.html").exists(), "shipping_fee.html が出ていない");
    let _ = std::fs::remove_dir_all(&dir);
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

#[test]
fn out_で書き出せる() {
    let dir = std::env::temp_dir().join(format!("rulec-doc-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let d = dir.to_string_lossy().to_string();
    let (c, out, _) = run(&["doc", "tests/corpus", "--out", &d]);
    assert_eq!(c, 0, "{out}");
    let n = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(n, CORPUS.len(), "コーパスの本数だけ出るはず");
    let one = std::fs::read_to_string(dir.join("shipping_fee.md")).expect("送料 が出ていない");
    assert!(one.starts_with("<!-- rulec "), "{one}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The rendering excerpt pasted into the documentation must not diverge from the real
/// thing. Hand-copied text rots (the same reasoning as for the generated-code excerpts).
/// Both language pages carry the same excerpt, so both are held to the output.
#[test]
fn 資料の抜粋は実物と一致する() {
    let (_, real, _) = run(&["doc", "tests/corpus/ゆうパック運賃.rule"]);
    for page in ["website/docs/checks.md", "website/docs-ja/checks.md"] {
        let md = std::fs::read_to_string(root().join(page)).unwrap();
        let i = md.find("## グループ\n").unwrap_or_else(|| panic!("{page} に doc の抜粋が無い"));
        let j = md[i..].find("```").expect("抜粋が閉じていない") + i;
        for l in md[i..j].lines() {
            if l.trim().is_empty() || l.trim() == "…" {
                continue;
            }
            assert!(real.contains(l), "{page} の抜粋が実物に無い:\n{l}");
        }
    }
}

/// §1.6 condition (a): a gate for the folded headers too.
/// Header folding is outside the cell text-match test, which makes it the only unguarded place
/// where invention could creep in. Constrain it by composition: **only tokens that come from
/// declarations may appear in a header** (output name, type, unit, tax class, rounding mode and
/// grid).
#[test]
fn 畳んだ見出しは宣言由来のトークンだけでできている() {
    for rel in CORPUS {
        // Types are rendered in canonical form (`money[円,incl_tax]` in the source becomes
        // `money[円, incl_tax]`). Whitespace differences are spelling variation, so strip them before
        // matching. Only whitespace is stripped; the words themselves must be in the source.
        let src: String =
            std::fs::read_to_string(root().join(rel)).unwrap().chars().filter(|c| !c.is_whitespace()).collect();
        let (c, out, e) = run(&["doc", rel]);
        assert_eq!(c, 0, "{rel}: {e}");
        let mut n = 0;
        // Only the header rows of data tables (the `| 列 | 出どころ |` table is a different thing).
        for l in out.lines().filter(|l| l.starts_with("| # |")) {
            for cell in l.trim_matches('|').split('|').map(|x| x.trim()) {
                let Some(body) = cell.strip_prefix("→ ") else { continue };
                n += 1;
                // The name (before the parenthesis), and the parenthesized part split on `/`.
                let (name, rest) = match body.split_once('（') {
                    Some((a, b)) => (a.trim(), b.trim_end_matches('）')),
                    None => (body, ""),
                };
                let bare = |t: &str| -> String { t.chars().filter(|c| !c.is_whitespace()).collect() };
                assert!(
                    src.contains(&bare(name)),
                    "{rel}: 見出しの出力名がもとの規則に無い: `{name}`"
                );
                for tok in rest.split(" / ") {
                    if tok.is_empty() {
                        continue;
                    }
                    // Rounding is `mode(grid)`. Match the mode and the grid against the source
                    // separately.
                    let parts: Vec<&str> = match tok.split_once('(') {
                        Some((m, g)) => vec![m, g.trim_end_matches(')')],
                        None => vec![tok],
                    };
                    for p in parts {
                        assert!(
                            src.contains(&bare(p)),
                            "{rel}: 見出しにもとの規則に無い語がある: `{p}`（見出し: {body}）"
                        );
                    }
                }
            }
        }
        assert!(n > 0, "{rel}: 見出しを一つも検査していない");
    }
}

/// §1.6 condition (b): `not: <group>` is not expanded, but the count is attached.
/// A list of 41 prefectures does not survive visual inspection, so not expanding is right, but
/// "it is the complement" alone cannot answer the approver's first question (is the scale
/// plausible?).
#[test]
fn 以外のグループには件数を添える() {
    // 基本送料 in 送料 uses `not: 遠隔地`. 遠隔地 has 2 values, so 45 remain.
    let (_, d, _) = run(&["doc", "tests/corpus/送料.rule"]);
    assert!(
        d.contains("**遠隔地**（2 値、`not: 遠隔地` は残り 45 値）"),
        "補集合の件数が無い:\n{d}"
    );
    assert!(!d.contains("北海道 ・ 沖縄県、青森県"), "41 県を展開してしまっている");

    // 運賃表 in ゆうパック運賃 only lists groups and never uses `not:`.
    // Giving the scale of a form that is not used would be noise, so no complement is written.
    let (_, d, _) = run(&["doc", "tests/corpus/ゆうパック運賃.rule"]);
    assert!(d.contains("**近畿圏**（6 値）"), "グループの値数は出す:\n{d}");
    assert!(!d.contains("`not: 近畿圏` は残り"), "使っていない形の件数を出している");
}

/// The page is also the tool's view (SEP-1865, §15.52). Inside a host's frame it introduces
/// itself and opens on the case the tool was called with; the handshake is what makes that
/// happen, and nothing else in the suite would notice if it were dropped.
#[test]
fn ページはホストの枠の中で名乗る() {
    let dir = std::env::temp_dir().join(format!("rulec-doc-ui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
        .args(["doc", "tests/corpus/送料.rule", "--format", "html"])
        .output()
        .expect("rulec を起動できない");
    let html = String::from_utf8_lossy(&o.stdout).into_owned();
    for needle in [
        "window.parent !== window",
        "\"ui/initialize\"",
        "ui/notifications/initialized",
        "ui/notifications/tool-input",
        "ui/notifications/tool-result",
        "structuredContent",
    ] {
        assert!(html.contains(needle), "ページに {needle} が無い");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
