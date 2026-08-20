//! 過去再生（§10.3、§10.4）。
//!
//! 二つある。
//!
//! - **replay** — 規則を過去の記録に当て、そのとき実際に出た値（`observed`）と
//!   突き合わせる。相手は生きたプロセスではなく死んだ記録なので、アダプタは要らない。
//! - **diff** — 規則の二つの版を同じ記録に当て、**何件・いくら動くか**を出す。
//!   入れてみて後から数字を見る、を、入れる前に見る、に変えるための段。
//!
//! どちらも比較のあとは §10.4 の共通の機構（発火行クラスタ、Δ 統計、丸め差異タグ）に
//! 渡す。違うのは相手が誰かと、replay にだけ補完系の分離が要ることだけである。

use crate::ast::RuleFile;
use crate::eval;
use crate::fixtures::{Load, Manifest};
use crate::report::{wire, Mismatch, Report};
use crate::types::Checked;

/// 記録を規則に通し、`observed` と突き合わせる。
pub fn replay(f: &RuleFile, c: &Checked, l: &Load, m: &Manifest, source: &str) -> Report {
    let mut rep = Report::new(f, "観測");
    rep.impl_id = source.into();
    rep.fills_used = m.shown.clone();
    if l.dropped > 0 {
        rep.excluded.push(("欄が欠けていて既定値も無い".into(), l.dropped));
    }
    if !l.problems.is_empty() {
        rep.excluded.push(("形式が宣言と食い違う".into(), l.problems.len()));
    }

    for (id, r) in l.records.iter().enumerate() {
        let (outs, trace, _) = eval::run_all(f, c, r.input.clone().into_iter().collect());
        let pairs: Vec<(String, Option<crate::eval::Val>, Option<String>)> = outs
            .into_iter()
            .map(|(n, v)| {
                let theirs = wire(r.observed.get(&n));
                (n, v, theirs)
            })
            .collect();
        let same = pairs.iter().all(|(_, a, b)| wire(a.as_ref()) == *b);

        // §10.3: 実測系と補完系を必ず分けて集計し、見出しの一致率は実測系だけから。
        if r.filled.is_empty() {
            rep.total += 1;
            if same {
                rep.agreed += 1;
            }
        } else {
            rep.filled_total += 1;
            if same {
                rep.filled_agreed += 1;
            }
            for n in &r.filled {
                *rep.filled.entry(n.clone()).or_insert(0) += 1;
            }
        }
        if !same {
            rep.mismatches.push(Mismatch {
                id,
                tag: r.tag.clone(),
                input: r.input.clone(),
                outs: pairs,
                err: None,
                trace,
            });
        }
    }
    rep
}

/// 二つの版を同じ記録に当てて、動く件数と金額を出す。
/// クラスタの鍵は**両側の発火行の遷移**（`旧の行 → 新の行`）。§10.4 のとおり
/// 旧の出力値は鍵に入れない。表引きなら行の組が値の組を一意に決めるので情報が
/// 増えず、計算出力では値の数だけクラスタが割れて要約が死ぬ。
pub fn diff(
    old: (&RuleFile, &Checked),
    new: (&RuleFile, &Checked),
    l: &Load,
    m: &Manifest,
    label: (&str, &str),
) -> Report {
    let mut rep = Report::new(new.0, "旧版");
    rep.impl_id = format!("{} → {}", label.0, label.1);
    rep.fills_used = m.shown.clone();
    if l.dropped > 0 {
        rep.excluded.push(("欄が欠けていて既定値も無い".into(), l.dropped));
    }
    if !l.problems.is_empty() {
        rep.excluded.push(("形式が宣言と食い違う".into(), l.problems.len()));
    }

    for (id, r) in l.records.iter().enumerate() {
        let ins: std::collections::HashMap<_, _> = r.input.clone().into_iter().collect();
        let (o_outs, o_trace, _) = eval::run_all(old.0, old.1, ins.clone());
        let (n_outs, n_trace, _) = eval::run_all(new.0, new.1, ins);

        let pairs: Vec<(String, Option<crate::eval::Val>, Option<String>)> = n_outs
            .into_iter()
            .map(|(n, v)| {
                let theirs = o_outs.iter().find(|(m, _)| *m == n).and_then(|(_, v)| wire(v.as_ref()));
                (n, v, theirs)
            })
            .collect();
        let same = pairs.iter().all(|(_, a, b)| wire(a.as_ref()) == *b);

        if r.filled.is_empty() {
            rep.total += 1;
            if same {
                rep.agreed += 1;
            }
        } else {
            rep.filled_total += 1;
            if same {
                rep.filled_agreed += 1;
            }
            for n in &r.filled {
                *rep.filled.entry(n.clone()).or_insert(0) += 1;
            }
        }
        if !same {
            // 行が同じなら「行3」、動いたなら「行3→行5」。読み手が探すのは動いた行。
            let trace = transition(&o_trace, &n_trace);
            rep.mismatches.push(Mismatch {
                id,
                tag: r.tag.clone(),
                input: r.input.clone(),
                outs: pairs,
                err: None,
                trace,
            });
        }
    }
    rep
}

/// 発火行の遷移を一本の鍵にする。`表 基本送料 行2→行5` の形。
fn transition(old: &[String], new: &[String]) -> Vec<String> {
    let split = |s: &str| -> (String, String) {
        match s.rsplit_once(" 行") {
            Some((t, r)) => (t.to_string(), r.to_string()),
            None => (s.to_string(), String::new()),
        }
    };
    let mut out = Vec::new();
    let mut i = 0;
    let mut j = 0;
    while i < old.len() || j < new.len() {
        match (old.get(i), new.get(j)) {
            (Some(a), Some(b)) => {
                let (ta, ra) = split(a);
                let (tb, rb) = split(b);
                if ta == tb {
                    out.push(if ra == rb { format!("{ta} 行{ra}") } else { format!("{ta} 行{ra}→行{rb}") });
                    i += 1;
                    j += 1;
                } else {
                    // 表そのものが増減した版。並びを崩さずに両方出す。
                    out.push(format!("{ta} 行{ra}（旧のみ）"));
                    i += 1;
                }
            }
            (Some(a), None) => {
                out.push(format!("{a}（旧のみ）"));
                i += 1;
            }
            (None, Some(b)) => {
                out.push(format!("{b}（新のみ）"));
                j += 1;
            }
            (None, None) => break,
        }
    }
    out
}
