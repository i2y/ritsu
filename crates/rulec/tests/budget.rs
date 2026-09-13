//! Synthetic benchmark for choosing the default check budget (§15-2).
//!
//! The design target is "12 columns, 500 rows, 4 numeric columns, within budget". That was an
//! estimate, not a measurement, so we measure here and set the default from it. Tables written by
//! people are almost always tilings, so the visit count should stay on the order of rows × columns
//! — that is the expectation in §6.3.

use std::time::Instant;

/// Build a rule whose table is a tiling with the given numbers of columns and rows.
/// As in real tables, an all-`-` default row at the end makes it complete.
fn synth(enum_cols: usize, num_cols: usize, rows: usize) -> String {
    const VALUES: &[&str] = &["甲", "乙", "丙", "丁", "戊", "己", "庚", "辛"];
    let mut s = String::from("rule 合成(synth) v1\n\nenum 区分(k) = ");
    s.push_str(
        &VALUES
            .iter()
            .enumerate()
            .map(|(i, v)| format!("{v}(v{i}) default"))
            .collect::<Vec<_>>()
            .join(" | "),
    );
    s.push_str("\n\ninputs\n");
    for i in 0..enum_cols {
        s.push_str(&format!("  e{i}(e{i}) : 区分\n"));
    }
    for i in 0..num_cols {
        s.push_str(&format!("  n{i}(n{i}) : money[円, incl_tax]  range >=0円 <=100000円\n"));
    }
    s.push_str("\noutputs\n  結果(r) : 区分\n\ntable t(t)\npolicy first\n|");
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
            // Shift the period per column, into a shape where coordinate compression pays off.
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
    // The default row
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

// The measurement that fixed the default budget. It takes ten-odd seconds to run, so it is off by
// default. Reproduce with `cargo test --release -- --ignored --nocapture`.
#[test]
#[ignore]
fn 予算の分布を測る() {
    // The §15-2 grid. The default budget is decided here.
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
    // Measure the regression corpus, to see where real tables sit relative to the synthetic grid.
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
    // The §6.3 design target: "12 columns, 500 rows, 4 numeric columns". If this fails, consider
    // strengthening the §6.3 pruning rather than raising the default budget.
    let (nodes, ms) = measure(8, 4, 500);
    println!("設計目標: {nodes} ノード / {ms} ms");
    assert!(
        nodes < rulec::region::DEFAULT_BUDGET,
        "設計目標が既定予算 {} を超えた: {nodes} ノード",
        rulec::region::DEFAULT_BUDGET
    );
    // Headroom dropping below 2x is the signal to decide again between budget and pruning.
    assert!(
        nodes * 2 < rulec::region::DEFAULT_BUDGET,
        "設計目標と既定予算の余裕が二倍を切った: {nodes} / {}",
        rulec::region::DEFAULT_BUDGET
    );
}

#[test]
fn 予算を使い切ったら未証明で止まる() {
    // §6.3: no quiet degradation into approximate checking by sampling. An unsound green negates
    // the reason this tool exists, so if it cannot prove, it stops with an error.
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/ゆうパック運賃.rule");
    let src = std::fs::read_to_string(&p).unwrap();
    let r = rulec::report_with(&src, "t.rule", 50);
    assert!(r.diags.iter().any(|d| d.code == "E109"), "予算超過は E109 で止まる");
    // Never pretend it passed.
    assert!(rulec::has_error(&r.diags));
}
