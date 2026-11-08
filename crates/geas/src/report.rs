//! The report of a run, as a person and an agent read it: one TAP line per claim,
//! the checks that failed with the run that gets there, a claim's error as a
//! diagnostic, and the summary line. Or the same as JSON.

use crate::diag::{self, Diag, Show, count};
/// What a report shows of the spec and of the program's output, with any key in it masked
/// (ritsu-base's `secrets::mask`): a key there must not reach the logs.
use ritsu_base::secrets::mask;
use ritsu_base::text::Lang;
use crate::json;
use crate::run::{ClaimResult, ClaimStatus, Orders, Waited};

/// The lines for the claims; with `only_failures`, the claims that are not ok.
pub fn claims(file: &str, src: &str, results: &[ClaimResult], lang: Lang, only_failures: bool) -> String {
    results.iter().enumerate().map(|(i, r)| claim(file, src, i + 1, r, lang, only_failures)).collect()
}

/// The lines for one claim, the `n`th of its spec; with `only_failures`, nothing for one that is
/// ok.
pub fn claim(file: &str, src: &str, n: usize, r: &ClaimResult, lang: Lang, only_failures: bool) -> String {
    let mut out = String::new();
    {
        match &r.status {
            ClaimStatus::Ok => {
                if !only_failures {
                    // how many orders it held in (DESIGN §23)
                    let orders = match (&r.orders, lang) {
                        (None, _) => String::new(),
                        (Some(o), Lang::En) => format!(" ({})", count(o.count, "order", "orders")),
                        (Some(o), Lang::Ja) => format!("（{} 通りの順で）", o.count),
                    };
                    out.push_str(&format!("ok {n} - {}{orders}\n", r.name));
                }
            }
            ClaimStatus::Fail => {
                out.push_str(&format!("not ok {n} - {}\n", r.name));
                if let Some(o) = &r.orders {
                    out.push_str(&failed_orders(file, o, lang));
                    if let Some(text) = src.lines().nth(o.line.saturating_sub(1)) {
                        out.push_str(&format!("      {:>4} | {}\n", o.line, mask(text)));
                    }
                }
                // a screen two failed checks share is shown once
                let mut last_screen: Option<&Vec<String>> = None;
                // the `when`s whose time ran out, said once, before the first failed check that
                // reads what they kept (DESIGN §18)
                let mut said: Vec<usize> = Vec::new();
                // the `when` a check reads: the nearest above it as written, in the order the checks are of
                let shown = r.orders.as_ref().map(|o| o.shown);
                for c in r.checks.iter().filter(|c| !c.ok) {
                    let read = r.observations.iter().filter(|o| o.order == shown && o.line <= c.line).max_by_key(|o| o.line);
                    if let Some(o) = read
                        && let Some(w) = o.waited.as_ref().filter(|w| !w.held)
                        && !said.contains(&o.idx)
                    {
                        said.push(o.idx);
                        out.push_str(&ran_out(file, o.line, w, lang));
                        if let Some(text) = src.lines().nth(o.line.saturating_sub(1)) {
                            out.push_str(&format!("      {:>4} | {}\n", o.line, mask(text)));
                        }
                    }
                    // the values the check goes by, which its text writes `{name}`
                    let told = |is: &str, sep: &str| -> Vec<String> {
                        c.values.iter().map(|(n, v)| format!("{{{n}}}{is}{}{sep}", mask(&json::quote(v)))).collect()
                    };
                    out.push_str(&match lang {
                        Lang::En => {
                            let values = if c.values.is_empty() { String::new() } else { format!(" ({})", told(" is ", "").join(", ")) };
                            format!("    {file}:{}: {} — got {}{values}\n", c.line, mask(&c.text()), mask(c.actual.get(lang)))
                        }
                        Lang::Ja => {
                            // a value is set off by a space; words of Japanese are not
                            let a = mask(c.actual.get(lang));
                            let gap = if a.starts_with(|ch: char| ch.is_ascii()) { " " } else { "" };
                            let values = if c.values.is_empty() { String::new() } else { format!("（{}）", told(" は ", "").join("、")) };
                            format!("    {file}:{}: {} のはずが、実際は{gap}{a}{values}\n", c.line, mask(&c.text()))
                        }
                    });
                    if let Some(text) = src.lines().nth(c.line.saturating_sub(1)) {
                        out.push_str(&format!("      {:>4} | {}\n", c.line, mask(text)));
                    }
                    if let Some(sc) = &c.screen
                        && last_screen == Some(&sc.lines)
                        && sc.left == 0
                    {
                        out.push_str(tr!("      画面: 上と同じ\n", "      the screen: as above\n").get(lang));
                    } else if let Some(sc) = &c.screen {
                        last_screen = Some(&sc.lines);
                        out.push_str(tr!("      画面:\n", "      the screen:\n").get(lang));
                        if sc.lines.is_empty() {
                            out.push_str(tr!("        （何もありません）\n", "        (nothing)\n").get(lang));
                        }
                        for l in &sc.lines {
                            out.push_str(&format!("        {l}\n"));
                        }
                        if sc.left > 0 {
                            out.push_str(&match lang {
                                Lang::En => format!("        … and {} more nodes\n", sc.left),
                                Lang::Ja => format!("        …ほかにノードが {} 個あります\n", sc.left),
                            });
                        }
                    }
                }
                if !r.run.is_empty() {
                    out.push_str(tr!("      ここまでの実行:\n", "      the run that gets there:\n").get(lang));
                    for s in &r.run {
                        out.push_str(&format!("        {:>4}  {}\n", s.line, mask(s.step().text.get(lang))));
                    }
                }
            }
            ClaimStatus::Error(d) => {
                out.push_str(&match lang {
                    Lang::En => format!("not ok {n} - {} (error)\n", r.name),
                    Lang::Ja => format!("not ok {n} - {}（エラー）\n", r.name),
                });
                out.push_str(&diag::indent(&d.shown(file, src, lang), "    "));
            }
        }
    }
    out
}

/// The line of an `in any order` block whose claim failed in some of its orders: how many of how
/// many, the first of them, whose checks and run follow, and the numbers of the others.
fn failed_orders(file: &str, o: &Orders, lang: Lang) -> String {
    let (f, n) = (o.failed.len(), o.count);
    let first = o.failed.first().map_or(o.shown, |(k, _)| *k);
    let others: Vec<usize> = o.failed.iter().skip(1).map(|(k, _)| *k).collect();
    match lang {
        Lang::En => {
            let said = match others.len() {
                0 => format!("{f} of {n} orders fails: order {first}, below"),
                1 => format!("{f} of {n} orders fail; below is the first, order {first}, and the other is order {}", numbers(&others, lang)),
                _ => format!("{f} of {n} orders fail; below is the first, order {first}, and the others are orders {}", numbers(&others, lang)),
            };
            format!("    {file}:{}: in any order — {said}\n", o.line)
        }
        Lang::Ja => {
            let said = if others.is_empty() {
                format!("{n} 通りの順のうち {f} 通りで成り立ちません。下はその順（{first} 番目）です")
            } else {
                format!("{n} 通りの順のうち {f} 通りで成り立ちません。下はその最初の順（{first} 番目）で、ほかは {} 番目の順です", numbers(&others, lang))
            };
            format!("    {file}:{}: in any order — {said}\n", o.line)
        }
    }
}

/// Numbers in rising order, a run of three or more written as its ends: `4, 6-9 and 12`,
/// `4、6〜9、12`.
pub fn numbers(ns: &[usize], lang: Lang) -> String {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for &k in ns {
        match runs.last_mut() {
            Some((_, last)) if *last + 1 == k => *last = k,
            _ => runs.push((k, k)),
        }
    }
    let mut parts: Vec<String> = Vec::new();
    for (a, b) in runs {
        let dash = if lang == Lang::En { "-" } else { "〜" };
        match b - a {
            0 => parts.push(a.to_string()),
            1 => {
                parts.push(a.to_string());
                parts.push(b.to_string());
            }
            _ => parts.push(format!("{a}{dash}{b}")),
        }
    }
    match lang {
        Lang::Ja => parts.join("、"),
        Lang::En => match parts.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
            _ => parts.join(""),
        },
    }
}

/// The line of a `when` whose time ran out before every check under it held on one observation:
/// how many calls it made and how long they took, the checks below it reading the last.
fn ran_out(file: &str, line: usize, w: &Waited, lang: Lang) -> String {
    let spent = w.spent();
    match lang {
        Lang::En => format!(
            "    {file}:{line}: {} — no answer held every check under it ({}); the checks below read the last one\n",
            w.wait.display(),
            spent.en
        ),
        Lang::Ja => format!(
            "    {file}:{line}: {} 秒のうちに、下のチェックがそろって成り立つ結果はありませんでした（{}呼びました）。下のチェックは最後の結果を見ています\n",
            w.wait.secs,
            spent.ja
        ),
    }
}

pub fn failed(results: &[ClaimResult]) -> usize {
    results.iter().filter(|r| !matches!(r.status, ClaimStatus::Ok)).count()
}

/// The last line of `check`.
pub fn check_summary(results: &[ClaimResult], journal: &str, lang: Lang) -> String {
    let (n, f) = (results.len(), failed(results));
    match lang {
        Lang::En => format!("{} · {} ok · {f} failed · journal: {journal}\n", count(n, "claim", "claims"), n - f),
        Lang::Ja => format!("主張 {n} 件 · 成り立った {} 件 · 成り立たなかった {f} 件 · ジャーナル: {journal}\n", n - f),
    }
}

/// The last line of a spec read and not run.
pub fn read_summary(n: usize, lang: Lang) -> String {
    match lang {
        Lang::En => format!("{} · not run\n", count(n, "claim", "claims")),
        Lang::Ja => format!("主張 {n} 件 · 走らせていません\n"),
    }
}

/// The last line of `snap`, and its warning when some claim did not hold.
pub fn snap_summary(results: &[ClaimResult], kept: usize, baseline: &str, lang: Lang) -> String {
    let (n, f) = (results.len(), failed(results));
    let mut out = match lang {
        Lang::En => format!(
            "{} · {} ok · {f} failed · baseline: {} → {baseline}\n",
            count(n, "claim", "claims"),
            n - f,
            count(kept, "interaction", "interactions")
        ),
        Lang::Ja => format!(
            "主張 {n} 件 · 成り立った {} 件 · 成り立たなかった {f} 件 · ベースライン: やりとり {kept} 件 → {baseline}\n",
            n - f
        ),
    };
    if f > 0 {
        out.push_str(tr!("注意: 成り立たない主張がある実行からベースラインを取りました\n", "warning: baseline recorded from a run with failing claims\n").get(lang));
    }
    out
}

/// The diagnostics of a claim that could not run, as JSON, for `drift --json`.
pub fn error_diagnostics(file: &str, results: &[ClaimResult], lang: Lang) -> Vec<String> {
    results
        .iter()
        .filter_map(|r| match &r.status {
            ClaimStatus::Error(d) => Some(d.json_in(file, lang)),
            _ => None,
        })
        .collect()
}

/// `check`, `snap`: the run as JSON (Appendix A).
pub fn claims_json(file: &str, results: &[ClaimResult], lang: Lang) -> String {
    claims_json_with(file, results, lang, failed(results) == 0, "")
}

/// The run as JSON, with `ok` as the command decides it and more members (`map`'s)
/// before the closing brace.
pub fn claims_json_with(file: &str, results: &[ClaimResult], lang: Lang, ok: bool, more: &str) -> String {
    let claims: Vec<String> = results
        .iter()
        .map(|r| {
            let status = match &r.status {
                ClaimStatus::Ok => "ok",
                ClaimStatus::Fail => "fail",
                ClaimStatus::Error(_) => "error",
            };
            let error = match &r.status {
                ClaimStatus::Error(d) => {
                    let notes: Vec<String> = d.notes.iter().map(|n| json::quote(&mask(n.get(lang)))).collect();
                    format!(
                        "{{\"code\":\"{}\",\"line\":{},\"col\":{},\"message\":{},\"notes\":[{}]}}",
                        d.code,
                        d.line.unwrap_or(0),
                        d.col.unwrap_or(0),
                        json::quote(&mask(d.message.get(lang))),
                        notes.join(",")
                    )
                }
                _ => "null".into(),
            };
            let checks: Vec<String> = r
                .checks
                .iter()
                .map(|c| {
                    let screen = c.screen.as_ref().map(|sc| format!(",\"screen\":{}", sc.json)).unwrap_or_default();
                    // the values a failed check goes by, when it has any
                    let values = if c.values.is_empty() {
                        String::new()
                    } else {
                        let pairs: Vec<String> = c.values.iter().map(|(n, v)| format!("{}:{}", json::quote(n), json::quote(&mask(v)))).collect();
                        format!(",\"values\":{{{}}}", pairs.join(","))
                    };
                    format!(
                        "{{\"line\":{},\"check\":{},\"expected\":{},\"actual\":{},\"ok\":{}{}{}}}",
                        c.line,
                        json::quote(&c.label),
                        json::quote(&mask(&c.expected)),
                        json::quote(&mask(c.actual.get(lang))),
                        c.ok,
                        values,
                        screen
                    )
                })
                .collect();
            let run = if matches!(r.status, ClaimStatus::Ok) {
                String::new()
            } else {
                let steps: Vec<String> = r
                    .run
                    .iter()
                    .map(|s| {
                        // a `when` that waits: its `within`, and the calls and the time when it ran out
                        let wait = match &s.waited {
                            None => String::new(),
                            Some(w) if w.held => format!(",\"within\":{}", w.wait.secs),
                            Some(w) => format!(",\"within\":{},\"tries\":{},\"ms\":{}", w.wait.secs, w.tries, w.ms),
                        };
                        format!(
                            "{{\"line\":{},\"call\":{}{wait},\"observed\":{}}}",
                            s.line,
                            json::quote(&mask(&s.when)),
                            s.observed.as_deref().map(|o| json::quote(&mask(o))).unwrap_or_else(|| "null".into())
                        )
                    })
                    .collect();
                format!(",\"run\":[{}]", steps.join(","))
            };
            // a claim with `in any order`: how many orders, and those whose checks failed with the
            // lines of the block's steps as they ran; `checks` and `run` are of the first of them
            let orders = match &r.orders {
                None => String::new(),
                Some(o) if matches!(r.status, ClaimStatus::Ok) => format!(",\"orders\":{}", o.count),
                Some(o) => {
                    let failed: Vec<String> = o
                        .failed
                        .iter()
                        .map(|(k, lines)| format!("{{\"order\":{k},\"lines\":[{}]}}", lines.iter().map(usize::to_string).collect::<Vec<_>>().join(",")))
                        .collect();
                    format!(",\"orders\":{},\"failed_orders\":[{}]", o.count, failed.join(","))
                }
            };
            format!(
                "{{\"name\":{},\"line\":{},\"status\":\"{}\",\"error\":{},\"checks\":[{}]{}{}}}",
                json::quote(&r.name),
                r.line,
                status,
                error,
                checks.join(","),
                run,
                orders
            )
        })
        .collect();
    format!(
        "{{\"geas\":1,\"ok\":{},\"file\":{},\"claims\":[{}]{more}}}",
        ok,
        json::quote(file),
        claims.join(",")
    )
}

/// A spec geas could not run at all: it does not parse, cannot be read, or has no
/// usable baseline.
pub fn failure_json(file: &str, diags: &[(String, Diag)], lang: Lang) -> String {
    let ds: Vec<String> = diags.iter().map(|(f, d)| d.json_in(f, lang)).collect();
    format!("{{\"geas\":1,\"ok\":false,\"file\":{},\"diagnostics\":[{}]}}", json::quote(file), ds.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(failed: &[usize], lang: Lang) -> String {
        let o = Orders { count: 6, line: 7, failed: failed.iter().map(|k| (*k, vec![])).collect(), shown: failed[0] };
        failed_orders("f.geas", &o, lang)
    }

    #[test]
    fn the_failed_orders_are_counted_and_the_others_listed() {
        assert_eq!(header(&[2], Lang::En), "    f.geas:7: in any order — 1 of 6 orders fails: order 2, below\n");
        assert_eq!(header(&[2, 5], Lang::En), "    f.geas:7: in any order — 2 of 6 orders fail; below is the first, order 2, and the other is order 5\n");
        assert_eq!(
            header(&[2, 4, 5, 6], Lang::En),
            "    f.geas:7: in any order — 4 of 6 orders fail; below is the first, order 2, and the others are orders 4-6\n"
        );
        assert_eq!(header(&[2], Lang::Ja), "    f.geas:7: in any order — 6 通りの順のうち 1 通りで成り立ちません。下はその順（2 番目）です\n");
        assert_eq!(
            header(&[2, 4, 5, 6], Lang::Ja),
            "    f.geas:7: in any order — 6 通りの順のうち 4 通りで成り立ちません。下はその最初の順（2 番目）で、ほかは 4〜6 番目の順です\n"
        );
    }

    #[test]
    fn a_run_of_three_numbers_or_more_is_written_as_its_ends() {
        assert_eq!(numbers(&[4], Lang::En), "4");
        assert_eq!(numbers(&[4, 5], Lang::En), "4 and 5");
        assert_eq!(numbers(&[4, 6, 7, 8, 9, 12], Lang::En), "4, 6-9 and 12");
        assert_eq!(numbers(&[4, 6, 7, 8, 9, 12], Lang::Ja), "4、6〜9、12");
    }
}
