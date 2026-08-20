//! 検査済みの描画（§1.6）。承認者のための面。
//!
//! この面の危険は「証明していないことを言い始めること」なので、テストの主眼は
//! 見た目ではなく**言っていることの正しさ**に置く。群が分割になっていない規則を
//! 与えて「過不足なく分割しています」と言わないこと、原本に無い字面を表に
//! 出さないこと、検査を通らない規則を綺麗に描かないこと。

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
];

/// 群を三通りに崩した規則。どれも `check` は通る（完全性は `-` の行が担う）。
fn groups_rule(g1: &str, g2: &str) -> String {
    format!(
        "規則 t(t) v1\n\n型 色(color) = 赤(r) | 青(b) | 緑(g)\n\
         群 暖色(warm) = {g1}\n群 寒色(cool) = {g2}\n\n\
         入力\n  c(c) : 色\n  b(b) : 真偽\n\n出力\n  r(r) : 真偽\n\n\
         表 x(x)\n方式 上から\n| c | b | → r(r) : 真偽 |\n\
         | 暖色 | 真 | 真 |\n| 寒色 | 真 | 偽 |\n| - | - | 偽 |\n"
    )
}

fn doc_of(src: &str, name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("rulec-doc-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("t.rule");
    std::fs::write(&p, src).unwrap();
    let (c, out, e) = run(&["doc", p.to_str().unwrap()]);
    assert_eq!(c, 0, "描画できない: {out}{e}");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// §1.6 の中心。群の一語が隠しているものを、**言えることだけ**言う。
#[test]
fn 群の被覆は事実のとおりに言う() {
    // 分割になっている。
    let d = doc_of(&groups_rule("赤", "青 ・ 緑"), "part");
    assert!(d.contains("この 2 群は 色 の 3 値を過不足なく分割しています"), "{d}");

    // 覆うが重なる。「分割」と言ってはいけない。
    let d = doc_of(&groups_rule("赤 ・ 緑", "青 ・ 緑"), "dup");
    assert!(!d.contains("過不足なく分割"), "重なっているのに分割だと言っている:\n{d}");
    assert!(d.contains("すべて覆いますが、緑 が二つ以上の群に属します"), "{d}");

    // 覆いきらない。承認者が「緑 はどうなるのか」と問えるように名指しする。
    let d = doc_of(&groups_rule("赤", "青"), "gap");
    assert!(!d.contains("過不足なく分割"), "覆えていないのに分割だと言っている:\n{d}");
    assert!(d.contains("覆うのは 色 の 3 値のうち 2 値で、緑 はどの群にも属しません"), "{d}");

    // 事実の出どころを名乗る（検査器が確かめたのか、描画が数えたのか）。
    assert!(d.contains("この描画が宣言から数えました"), "{d}");
}

/// 禁則: 検査器の出力に無い文章は書かない。表のセルは原本の字面のまま。
#[test]
fn 表のセルは原本に遡れる() {
    for rel in CORPUS {
        let src = std::fs::read_to_string(root().join(rel)).unwrap();
        let (c, out, e) = run(&["doc", rel]);
        assert_eq!(c, 0, "{rel}: {e}");
        // 節ごとに区切り行を数える。`## 表` は「列 | 出どころ」が一つ目なので
        // データ表は二つ目、`## 例` は一つしかないので一つ目。
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
            // 先頭は行番号なので飛ばす（例の表には無い）。
            for cell in cells.iter().skip(1) {
                if cell.is_empty() || cell.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let raw = cell.replace("\\|", "|");
                assert!(
                    src.contains(&raw),
                    "{rel}: 原本に無い字面が表に出ている: `{raw}`\n行: {l}"
                );
                checked += 1;
            }
        }
        assert!(checked > 5, "{rel}: セルを一つも検査していない");
    }
}

/// §1.6: 壊れた規則の綺麗な描画は嘘になる。検査を通らなければ描かない。
#[test]
fn 検査を通らない規則は描かない() {
    let (c, out, e) = run(&["doc", "tests/mutants/m_e101.rule"]);
    assert_eq!(c, 1, "エラーのある規則を描いた");
    assert!(out.contains("error[E101]"), "何が悪いかを言う: {out}");
    assert!(e.contains("描画しません"), "{e}");
    assert!(!out.contains("# 規則"), "描画が始まってしまっている: {out}");
}

/// 古い描画が正の顔をして残るのが最大の危険（§1.6）。刻印で古さを検査できること。
#[test]
fn 原本の刻印が入る() {
    let rel = "tests/corpus/送料.rule";
    let (_, out, _) = run(&["doc", rel]);
    let first = out.lines().next().unwrap();
    assert!(first.contains(rel), "原本のパスを刻む: {first}");
    assert!(first.contains(&format!("rulec {}", env!("CARGO_PKG_VERSION"))), "道具の版を刻む: {first}");
    assert!(first.contains("sha256:"), "原本のハッシュを刻む: {first}");
    assert!(first.contains("正本は .rule のほう"), "一方向であることを言う: {first}");

    // 原本を一文字変えたら刻印が変わる。
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
    assert_ne!(h(&out), h(&other), "原本が変わったのに刻印が同じ");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 決定的であること。PR に貼るものが実行ごとに揺れると diff が読めない。
#[test]
fn 描画は決定的である() {
    for rel in CORPUS {
        let (_, a, _) = run(&["doc", rel]);
        let (_, b, _) = run(&["doc", rel]);
        assert_eq!(a, b, "{rel}: 二度で違う");
    }
}

/// 承認者が知るべきことが落ちていないこと（§1.6 の一覧）。
#[test]
fn 承認者が知るべきことが載る() {
    // W114 と番人の存在。
    let (_, d, _) = run(&["doc", "tests/corpus/クーポン併用.rule"]);
    assert!(d.contains("証明できていません**（W114）"), "{d}");
    assert!(d.contains("番人"), "番人の存在を言う: {d}");

    // 遮蔽の三分類と、要確認の行対＋証人。
    let (_, d, _) = run(&["doc", "tests/corpus/送料.rule"]);
    assert!(d.contains("**構造的** 2") && d.contains("**要確認** 1"), "{d}");
    assert!(d.contains("要確認: 同じ入力が 行1 と 行2"), "どの行対かを言う: {d}");
    assert!(d.contains("両方に当たる例:"), "証人を出す: {d}");
    assert!(d.contains("既定行"), "既定行を指す: {d}");
    assert!(d.contains("| 大口 | 定義 |"), "定義の式を出す: {d}");

    // 表の依存。
    let (_, d, _) = run(&["doc", "tests/corpus/ゆうパック運賃.rule"]);
    assert!(d.contains("→ 表 運賃表 の入力列"), "出力の行き先を言う: {d}");
    assert!(d.contains("`契約のみ`"), "宣言意図を言う: {d}");
    assert!(d.contains("この 3 件は `rulec check` が参照評価器で実行し"), "例が検証済みだと言う: {d}");

    // 既定扱い と 導出の範囲。
    let (_, d, _) = run(&["doc", "tests/corpus/クーポン割引.rule"]);
    assert!(d.contains("店内全商品（既定扱い）"), "{d}");
    assert!(d.contains("E112"), "導出範囲が検査済みだと言う: {d}");

    // 丸めの仮置きの出典が浮上すること（§7.2 の慣行、§16 第三への防波堤）。
    let (_, d, _) = run(&["doc", "tests/corpus/クーポン一枚.rule"]);
    assert!(d.contains("切り捨て(1円)"), "丸めを列に畳む: {d}");
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

/// README に貼った描画の抜粋が、実物と食い違わないこと。
/// 手で書き写したものは腐る（生成コードの抜粋と同じ理屈）。
#[test]
fn readmeの抜粋は実物と一致する() {
    let md = std::fs::read_to_string(root().join("README.md")).unwrap();
    let (_, real, _) = run(&["doc", "tests/corpus/ゆうパック運賃.rule"]);
    let i = md.find("## 群\n\n- **近畿圏**").expect("README に doc の抜粋が無い");
    let j = md[i..].find("```").expect("抜粋が閉じていない") + i;
    for l in md[i..j].lines() {
        if l.trim().is_empty() || l.trim() == "…" {
            continue;
        }
        assert!(real.contains(l), "README の抜粋が実物に無い:\n{l}");
    }
}
