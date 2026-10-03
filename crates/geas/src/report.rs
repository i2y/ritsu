//! The report of a run, as a person and an agent read it: one TAP line per claim,
//! the checks that failed with the run that gets there, a claim's error as a
//! diagnostic, and the summary line. Or the same as JSON.

use crate::diag::{self, Diag, Show, count};
use ritsu_base::text::Lang;
use crate::json;
use crate::run::{ClaimResult, ClaimStatus};

/// The lines for the claims; with `only_failures`, the claims that are not ok.
pub fn claims(file: &str, src: &str, results: &[ClaimResult], lang: Lang, only_failures: bool) -> String {
    let mut out = String::new();
    for (i, r) in results.iter().enumerate() {
        let n = i + 1;
        match &r.status {
            ClaimStatus::Ok => {
                if !only_failures {
                    out.push_str(&format!("ok {n} - {}\n", r.name));
                }
            }
            ClaimStatus::Fail => {
                out.push_str(&format!("not ok {n} - {}\n", r.name));
                // a screen two failed checks share is shown once
                let mut last_screen: Option<&Vec<String>> = None;
                for c in r.checks.iter().filter(|c| !c.ok) {
                    out.push_str(&match lang {
                        Lang::En => format!("    {file}:{}: {} — got {}\n", c.line, c.text(), c.actual.get(lang)),
                        Lang::Ja => {
                            // a value is set off by a space; words of Japanese are not
                            let a = c.actual.get(lang);
                            let gap = if a.starts_with(|ch: char| ch.is_ascii()) { " " } else { "" };
                            format!("    {file}:{}: {} のはずが、実際は{gap}{a}\n", c.line, c.text())
                        }
                    });
                    if let Some(text) = src.lines().nth(c.line.saturating_sub(1)) {
                        out.push_str(&format!("      {:>4} | {}\n", c.line, text));
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
                        out.push_str(&format!("        {:>4}  {}\n", s.line, s.step().text.get(lang)));
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
                    let notes: Vec<String> = d.notes.iter().map(|n| json::quote(n.get(lang))).collect();
                    format!(
                        "{{\"code\":\"{}\",\"line\":{},\"col\":{},\"message\":{},\"notes\":[{}]}}",
                        d.code,
                        d.line.unwrap_or(0),
                        d.col.unwrap_or(0),
                        json::quote(d.message.get(lang)),
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
                    format!(
                        "{{\"line\":{},\"check\":{},\"expected\":{},\"actual\":{},\"ok\":{}{}}}",
                        c.line,
                        json::quote(&c.label),
                        json::quote(&c.expected),
                        json::quote(c.actual.get(lang)),
                        c.ok,
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
                        format!(
                            "{{\"line\":{},\"call\":{},\"observed\":{}}}",
                            s.line,
                            json::quote(&s.when),
                            s.observed.as_deref().map(json::quote).unwrap_or_else(|| "null".into())
                        )
                    })
                    .collect();
                format!(",\"run\":[{}]", steps.join(","))
            };
            format!(
                "{{\"name\":{},\"line\":{},\"status\":\"{}\",\"error\":{},\"checks\":[{}]{}}}",
                json::quote(&r.name),
                r.line,
                status,
                error,
                checks.join(","),
                run
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
