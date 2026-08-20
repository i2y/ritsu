//! 検査予算の既定値を決めるための合成ベンチ（§15-2）。
//!
//! 設計目標は「列 12、行 500、数値列 4 で予算内」。これは見込みであって実測では
//! なかったので、ここで測って既定を決める。人間の書く表はほぼ敷き詰めなので、
//! 訪問数は行数×列数のオーダーに収まる、というのが §6.3 の見込み。

use std::time::Instant;

/// 列と行を指定して、敷き詰めの表を持つ規則を作る。
/// 実際の表と同じく、最後に全部 `-` の既定行を置いて完全にする。
fn synth(enum_cols: usize, num_cols: usize, rows: usize) -> String {
    const VALUES: &[&str] = &["甲", "乙", "丙", "丁", "戊", "己", "庚", "辛"];
    let mut s = String::from("規則 合成(synth) v1\n\n型 区分(k) = ");
    s.push_str(
        &VALUES
            .iter()
            .enumerate()
            .map(|(i, v)| format!("{v}(v{i}) 既定扱い"))
            .collect::<Vec<_>>()
            .join(" | "),
    );
    s.push_str("\n\n入力\n");
    for i in 0..enum_cols {
        s.push_str(&format!("  e{i}(e{i}) : 区分\n"));
    }
    for i in 0..num_cols {
        s.push_str(&format!("  n{i}(n{i}) : 金額[円, 税込]  範囲 >=0円 <=100000円\n"));
    }
    s.push_str("\n出力\n  結果(r) : 区分\n\n表 t(t)\n方式 上から\n|");
    for i in 0..enum_cols {
        s.push_str(&format!(" e{i} |"));
    }
    for i in 0..num_cols {
        s.push_str(&format!(" n{i} |"));
    }
    s.push_str(" → 結果(r) : 区分 |\n");

    for r in 0..rows.saturating_sub(1) {
        s.push('|');
        for c in 0..enum_cols {
            // 列ごとに周期をずらして、座標の圧縮が効く形にする。
            if (r + c) % 3 == 0 {
                s.push_str(&format!(" {} |", VALUES[(r + c) % VALUES.len()]));
            } else {
                s.push_str(" - |");
            }
        }
        for c in 0..num_cols {
            if (r + c) % 4 == 0 {
                s.push_str(&format!(" <={}円 |", 100 * ((r + c) % 20 + 1)));
            } else {
                s.push_str(" - |");
            }
        }
        s.push_str(&format!(" {} |\n", VALUES[r % VALUES.len()]));
    }
    // 既定行
    s.push('|');
    for _ in 0..(enum_cols + num_cols) {
        s.push_str(" - |");
    }
    s.push_str(" 甲 |\n");
    s
}

fn measure(enum_cols: usize, num_cols: usize, rows: usize) -> (i64, u128) {
    let src = synth(enum_cols, num_cols, rows);
    let t0 = Instant::now();
    let r = rulec::report(&src, "synth.rule");
    let ms = t0.elapsed().as_millis();

    (r.nodes, ms)
}

// 既定予算を決めたときの実測。回すと十数秒かかるので既定では走らせない。
// `cargo test --release -- --ignored --nocapture` で再現できる。
#[test]
#[ignore]
fn 予算の分布を測る() {
    // §15-2 の格子。既定予算はここで決める。
    println!("\n 列(列挙+数値) |   行 |      ノード |   ms | ノード/ms");
    let mut rows_out: Vec<(i64, u128, String)> = Vec::new();
    for &(e, n) in &[(2usize, 0usize), (4, 2), (6, 2), (8, 4), (6, 6)] {
        for &rows in &[10usize, 50, 200, 500, 1000] {
            let (nodes, ms) = measure(e, n, rows);
            let rate = if ms > 0 { nodes / ms as i64 } else { nodes };
            println!(" {e:>6}+{n:<6} | {rows:>4} | {nodes:>11} | {ms:>4} | {rate:>9}");
            rows_out.push((nodes, ms, format!("列 {e}+{n}、行 {rows}")));
        }
    }
    rows_out.sort_by_key(|(n, _, _)| *n);
    let worst = rows_out.last().unwrap();
    let p99 = &rows_out[rows_out.len() * 99 / 100];
    println!("\n p99 = {} ノード（{}）", p99.0, p99.2);
    println!(" 最悪 = {} ノード / {} ms（{}）", worst.0, worst.1, worst.2);
}

#[test]
fn 実表は桁違いに軽い() {
    // 回帰コーパスの実測。合成の格子と比べて、実際の表がどのあたりにいるかを見る。
    for f in [
        "tests/corpus/ゆうパック運賃.rule",
        "tests/corpus/クーポン割引.rule",
        "tests/corpus/クーポン併用.rule",
    ] {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(f);
        let src = std::fs::read_to_string(&p).unwrap();
        let r = rulec::report(&src, f);
        println!("{f}: {} ノード", r.nodes);
        assert!(r.nodes < rulec::region::DEFAULT_BUDGET / 100, "{f} が重すぎる");
    }
}

#[test]
fn 設計目標は既定予算に収まる() {
    // §6.3 の設計目標「列 12、行 500、数値列 4」。ここが外れたら、既定予算を
    // 上げるのではなく §6.3 の枝刈りを増強する側を検討する。
    let (nodes, ms) = measure(8, 4, 500);
    println!("設計目標: {nodes} ノード / {ms} ms");
    assert!(
        nodes < rulec::region::DEFAULT_BUDGET,
        "設計目標が既定予算 {} を超えた: {nodes} ノード",
        rulec::region::DEFAULT_BUDGET
    );
    // 余裕が二倍を切ったら、予算か枝刈りかを決め直す合図。
    assert!(
        nodes * 2 < rulec::region::DEFAULT_BUDGET,
        "設計目標と既定予算の余裕が二倍を切った: {nodes} / {}",
        rulec::region::DEFAULT_BUDGET
    );
}

#[test]
fn 予算を使い切ったら未証明で止まる() {
    // §6.3: サンプリングによる近似検査へ静かに縮退はしない。不健全な緑は
    // この道具が存在する理由の否定なので、証明できなければエラーで止める。
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/ゆうパック運賃.rule");
    let src = std::fs::read_to_string(&p).unwrap();
    let r = rulec::report_with(&src, "t.rule", 50);
    assert!(r.diags.iter().any(|d| d.code == "E109"), "予算超過は E109 で止まる");
    // 通ったふりをしない。
    assert!(rulec::has_error(&r.diags));
}
