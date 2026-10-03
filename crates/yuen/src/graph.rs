//! The fifth stage (DESIGN 5.4, 5.5, PLAN B.6): cycles among requirements, and the periods of
//! a requirement's versions and of what replaces what.

use crate::date::{Day, days};
use crate::diag::{ChainItem, Diag, DiagExt};
use ritsu_base::text::Text;
use crate::project::Project;

/// The cycles of `from <requirement>` and `replaces` (E405), and which versions are in a
/// cycle of `from`s (their ends cannot be made).
pub fn cycles(p: &Project) -> (Vec<Diag>, Vec<bool>) {
    let n = p.reqs.len();
    // Edges: (to, through a `from`?).
    let mut edges: Vec<Vec<(usize, bool, usize)>> = vec![Vec::new(); n];
    for (r, out) in edges.iter_mut().enumerate() {
        let d = p.decl(r);
        for (i, t) in p.names.from[r].iter().enumerate() {
            if let Some(t) = t {
                out.push((*t, true, d.from[i].span.line));
            }
        }
        for (i, t) in p.names.replaces[r].iter().enumerate() {
            if let Some(t) = t {
                out.push((*t, false, d.replaces[i].span.line));
            }
        }
    }
    let mut diags = Vec::new();
    let mut seen_cycles: Vec<Vec<usize>> = Vec::new();
    let mut in_from_cycle = vec![false; n];
    // Every cycle through a version, found by walking from it; each reported once, from the
    // version that comes first in the files.
    for start in 0..n {
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        let mut path: Vec<usize> = vec![start];
        let mut on: Vec<bool> = vec![false; n];
        on[start] = true;
        let mut visited = vec![false; n];
        while let Some((node, next)) = stack.last_mut().map(|(a, b)| (*a, b)) {
            if *next >= edges[node].len() {
                stack.pop();
                path.pop();
                on[node] = false;
                continue;
            }
            let (t, _, _) = edges[node][*next];
            *next += 1;
            if t == start {
                let mut cyc = path.clone();
                let mut key = cyc.clone();
                key.sort();
                if !seen_cycles.contains(&key) && *cyc.iter().min().unwrap() == start {
                    seen_cycles.push(key);
                    cyc.push(start);
                    diags.push(cycle_diag(p, &cyc));
                }
                continue;
            }
            if on[t] || visited[t] || t < start {
                continue;
            }
            visited[t] = true;
            on[t] = true;
            path.push(t);
            stack.push((t, 0));
        }
    }
    // Which versions can reach themselves through `from`s alone: their ends cannot be made.
    // (A version read from one of them has no end either; the cycle's diagnostic says why.)
    for r in 0..n {
        let mut seen = vec![false; n];
        let mut todo: Vec<usize> = edges[r].iter().filter(|(_, f, _)| *f).map(|(t, _, _)| *t).collect();
        while let Some(x) = todo.pop() {
            if x == r {
                in_from_cycle[r] = true;
                break;
            }
            if seen[x] {
                continue;
            }
            seen[x] = true;
            todo.extend(edges[x].iter().filter(|(_, f, _)| *f).map(|(t, _, _)| *t));
        }
    }
    (diags, in_from_cycle)
}

fn cycle_diag(p: &Project, cyc: &[usize]) -> Diag {
    let first = cyc[0];
    let fi = p.reqs[first].file;
    let names: Vec<String> = cyc.iter().map(|r| p.req_label(*r)).collect();
    let shown = names.join(" → ");
    let chain: Vec<ChainItem> = cyc[..cyc.len() - 1]
        .iter()
        .map(|r| {
            let (file, line) = p.req_place(*r);
            ChainItem { text: Text::same(p.req_label(*r)), file, line }
        })
        .collect();
    p.err(fi, "E405", p.decl(first).span, tr!("要件のあいだに循環があります: {shown}", "The requirements make a cycle: {shown}"))
        .note(tr!(
            "`from <要件>` と `replaces <要件>` は、元の要件と、置き換えられる要件を指します。たどって元に戻ってくるので、どれが先かを決められません。",
            "`from <requirement>` points at what a requirement is read from, and `replaces` at what it replaces; following them comes back here, so which comes first cannot be told."
        ))
        .chain(tr!("循環", "the cycle"), chain)
}

/// The periods of the versions (E406–E408) and of replacing (E409).
pub fn periods(p: &Project) -> Vec<Diag> {
    let mut diags = Vec::new();
    for (name, vs) in &p.by_name {
        if vs.len() < 2 {
            continue;
        }
        let mut broken = false;
        for r in vs {
            if p.decl(*r).in_force.is_none() {
                let fi = p.reqs[*r].file;
                let v = p.reqs[*r].version;
                diags.push(p.err(fi, "E408", p.decl(*r).span, tr!("要件「{name}」の v{v} に `in force` がありません", "{name} v{v} has no `in force`")).note(tr!(
                    "版が二つ以上ある要件は、どの版にも効力の期間を書きます。期間で、どの日にどの版が効くかが決まります。",
                    "When a requirement has more than one version, each has a period: the periods say which version holds on which day."
                )));
                broken = true;
            }
        }
        if broken {
            continue;
        }
        let mut order: Vec<usize> = vs.clone();
        order.sort_by_key(|r| (p.decl(*r).in_force.unwrap().0.start(), p.reqs[*r].version));
        let label = |r: usize| format!("v{}", p.reqs[r].version);
        let period_of = |r: usize| p.decl(r).in_force.unwrap();
        let item = |r: usize| {
            let (file, line) = (p.file_of(r).display.clone(), period_of(r).1.line);
            ChainItem { text: Text::same(format!("v{} in force {}", p.reqs[r].version, period_of(r).0)), file, line }
        };
        let chain: Vec<ChainItem> = order.iter().map(|r| item(*r)).collect();
        for w in order.windows(2) {
            if p.reqs[w[0]].version > p.reqs[w[1]].version {
                let (a, b) = (label(w[0]), label(w[1]));
                let fi = p.reqs[w[1]].file;
                diags.push(
                    p.err(fi, "E408", period_of(w[1]).1, tr!("要件「{name}」の版の番号が、期間の順に増えていません（{a} の期間が {b} より前）", "The versions of {name} do not number in the order of their periods ({a} comes before {b})"))
                        .chain(tr!("期間の順", "in the order of the periods"), chain.clone()),
                );
                broken = true;
            }
        }
        for (i, r) in order.iter().enumerate() {
            let (per, sp) = period_of(*r);
            let fi = p.reqs[*r].file;
            let v = label(*r);
            if per.to.is_none() && i + 1 < order.len() {
                diags.push(p.err(fi, "E408", sp, tr!("要件「{name}」の {v} は終わりを開けていますが、最後の版ではありません", "{name} {v} leaves its end open, and it is not the last version")).note(tr!(
                    "終わりを開けられるのは最後の版だけです。次の版が始まる前の日を、終わりに書きます。",
                    "Only the last version leaves its end open; end it on the day before the next one starts."
                )));
                broken = true;
            }
            if per.from.is_none() && i > 0 {
                diags.push(p.err(fi, "E408", sp, tr!("要件「{name}」の {v} は始まりを開けていますが、最初の版ではありません", "{name} {v} leaves its start open, and it is not the first version")));
                broken = true;
            }
        }
        if broken {
            continue;
        }
        for w in order.windows(2) {
            let (a, b) = (period_of(w[0]).0, period_of(w[1]).0);
            let (va, vb) = (label(w[0]), label(w[1]));
            let fi = p.reqs[w[1]].file;
            let sp = period_of(w[1]).1;
            let end_a = a.end();
            let start_b = b.start();
            let after = end_a.next();
            if Some(start_b) == after {
                continue;
            }
            if start_b > end_a {
                let (g0, g1) = (after.unwrap(), start_b.prev().unwrap());
                let gap = days(g0, g1);
                let fix = format!("in force {}..{}", g0, b.to.map(|d| d.to_string()).unwrap_or_default());
                diags.push(
                    p.err(fi, "E406", sp, tr!("要件「{name}」の {va} と {vb} のあいだの {gap} が、どの版の期間にも入りません", "{gap}, between {name} {va} and {vb}, falls in no version's period"))
                        .note(tr!("{va} の期間は `{a}`、{vb} の期間は `{b}` です。", "{va} is in force `{a}`, and {vb} `{b}`."))
                        .chain(tr!("期間の順", "in the order of the periods"), chain.clone())
                        .fix_trimmed(format!("  {fix}")),
                );
            } else {
                let o1 = end_a.min(b.end());
                let over = days(start_b, o1);
                diags.push(
                    p.err(fi, "E407", sp, tr!("要件「{name}」の {va} と {vb} の期間が {over} で重なります", "{name} {va} and {vb} are both in force on {over}"))
                        .note(tr!("{va} の期間は `{a}`、{vb} の期間は `{b}` です。一つの日に効く版は一つです。", "{va} is in force `{a}`, and {vb} `{b}`; one version holds on a day."))
                        .chain(tr!("期間の順", "in the order of the periods"), chain.clone()),
                );
            }
        }
    }
    // Replacing (E409): what is replaced ends, and what replaces it starts the day after.
    for r in 0..p.reqs.len() {
        let d = p.decl(r);
        let fi = p.reqs[r].file;
        for (i, t) in p.names.replaces[r].iter().enumerate() {
            let Some(t) = t else { continue };
            if *t == r {
                continue;
            }
            let rr = &d.replaces[i];
            let old = p.req_label(*t);
            let me = p.req_label(r);
            let Some(end) = p.decl(*t).in_force.and_then(|(per, _)| per.to) else {
                diags.push(p.err(fi, "E409", rr.span, tr!("置き換えられる要件「{old}」に終わりの日がありません", "{old}, which this replaces, has no end")).note(tr!(
                    "置き換えられる要件には、`in force …..2027-03-31` のように終わりの日を書きます。置き換える要件は、その翌日から効きます。",
                    "Give what is replaced an end, as `in force …..2027-03-31`; what replaces it holds from the day after."
                )));
                continue;
            };
            let want: Day = match end.next() {
                Some(d) => d,
                None => continue,
            };
            let start = d.in_force.and_then(|(per, _)| per.from);
            if start != Some(want) {
                let fix = format!("  in force {want}..{}", d.in_force.and_then(|(per, _)| per.to).map(|x| x.to_string()).unwrap_or_default());
                let now = match d.in_force {
                    Some((per, _)) => tr!("いまの期間は `{per}` です。", "Its period now is `{per}`."),
                    None => tr!("いまは期間がありません。", "It has no period now."),
                };
                let sp = d.in_force.map(|(_, s)| s).unwrap_or(d.span);
                diags.push(
                    p.err(fi, "E409", sp, tr!("「{me}」は「{old}」を置き換えるのに、{old} の終わり（{end}）の翌日の {want} から始まっていません", "{me} replaces {old}, and does not start on {want}, the day after {old} ends ({end})"))
                        .note(now)
                        .fix_trimmed(fix),
                );
            }
        }
    }
    diags
}
