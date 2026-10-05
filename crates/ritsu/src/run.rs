//! `ritsu run <file.flow> --scenario <file.json>` (DESIGN 7.9, 8.1): a workflow run in dandori's
//! reference interpreter, its rules computed by rulec's evaluator, its dates by koyomi's interpreter
//! and the operations of its books by chobo's reference interpreter, on books that keep what was
//! done and whose holds expire as the run's time goes by; every other task is answered by the
//! scenario, as `dandori run` answers it (dandori's `computed`). Not a proof: one run of the three
//! languages' meanings at once, which the generated code can be held to.
//!
//! The text says each call with who answered it, each wait, each hold that expired, how the run
//! ended, and each book at the end. The JSON is one object: the trace `dandori run` prints, what
//! happened in order, the books, and the scenario with every answer filled in (`replay`), on which
//! `dandori run` makes the same trace.

use crate::cli;
use dandori::computed::{BookEnd, Event, Ran, Stopped, Who};
use dandori::interp::Passing;
use dandori::render::View;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{Books, Dates};
use ritsu_project::Joined;
use serde_json::{json, Value};
use std::path::Path;
use std::rc::Rc;

fn refuse(msg: Text, lang: Lang) -> u8 {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    eprintln!("{head}: {}", msg.get(lang));
    2
}

/// `ritsu run <file.flow> --scenario <file.json> [--target <target>] [--format json]`: the exit
/// code. 0 when the run comes to its end, however it ends; 1 when the flow has errors, or the run
/// cannot go on; 2 for bad arguments, a file that cannot be read, or a scenario that cannot be run.
pub fn command(args: &[String], lang: Lang) -> u8 {
    let table = cli::table();
    let cmd = table.command("run").expect("run is in the table");
    let asked = args.iter().enumerate().find_map(|(i, x)| if x == "--lang" { args.get(i + 1).cloned() } else { x.strip_prefix("--lang=").map(str::to_string) });
    let lang = if asked.is_some() { Lang::pick(asked.as_deref(), "RITSU_LANG") } else { lang };
    let a = match table.parse(cmd, args) {
        Ok(a) => a,
        Err(e) => return refuse(e, lang),
    };
    if a.has("--help") {
        print!("{}", table.help_cmd(cmd, lang));
        return 0;
    }
    let [flow] = a.pos.as_slice() else {
        eprintln!("{}", table.usage_line(cmd));
        return 2;
    };
    let Some(scenario) = a.get("--scenario") else {
        return refuse(tr!("`--scenario <file.json>` が要ります", "`--scenario <file.json>` is required"), lang);
    };
    let json_out = a.get("--format") == Some("json");
    let target = a.get("--target").unwrap_or("reference").to_string();
    let view = match target.as_str() {
        "reference" | "asl" => View::Asl,
        // the three SDKs put the same names on the wire
        "temporal" | "temporal-python" | "temporal-go" => View::Temporal,
        "durable" => View::Durable,
        "argo" => View::Argo,
        _ => View::Graph,
    };
    // through ritsu_base::fs, as every file a language reads (DESIGN 4.15): the page in the browser runs on files in memory
    let sc: Value = match ritsu_base::fs::read_to_string(scenario).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string())) {
        Ok(v) => v,
        Err(e) => return refuse(tr!("シナリオ `{scenario}` を読めません: {e}", "cannot read the scenario `{scenario}`: {e}"), lang),
    };
    // the flow, checked with every language it reads joined, as `ritsu dandori` checks it
    let j = Joined::new();
    let (rules, dates, books): (_, Rc<dyn Dates>, Rc<dyn Books>) = (j.rules(), j.koyomi.clone(), j.chobo.clone());
    let path = Path::new(flow);
    let (src, checked) = match dandori::sources::with_ports(rules.clone(), dates.clone(), books.clone(), || dandori::check::check_file(path)) {
        Ok(x) => x,
        Err(msg) => {
            eprintln!("{msg}");
            return 2;
        }
    };
    let diagnostics: Vec<Value> = checked.diags.iter().map(|d| d.to_json(lang)).collect();
    if !json_out {
        eprint!("{}", dandori::commands::render(&checked.diags, flow, &src, lang));
    }
    let Some(model) = checked.model else {
        if json_out {
            println!("{}", pretty(&json!({ "ritsu": env!("CARGO_PKG_VERSION"), "flow": flow, "diagnostics": diagnostics })));
        }
        return 1;
    };
    match dandori::computed::run(&model, &sc, view, rules, dates, books) {
        Ok(ran) => {
            if json_out {
                println!("{}", pretty(&whole(flow, scenario, &target, diagnostics, &ran)));
            } else {
                print!("{}", text(flow, scenario, &ran, lang));
            }
            0
        }
        Err(Stopped::Scenario(t)) => refuse(tr!("シナリオ `{scenario}` を流せません: {}", "the scenario `{scenario}` cannot be run: {}", t.ja; t.en), lang),
        Err(Stopped::Run(e)) => {
            let head = if lang == Lang::Ja { "エラー" } else { "error" };
            let msg = tr!("流れが途中で止まりました: {e}", "the run could not go on: {e}");
            eprintln!("{head}: {}", msg.get(lang));
            1
        }
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

/// The JSON of a run: one object.
fn whole(flow: &str, scenario: &str, target: &str, diagnostics: Vec<Value>, ran: &Ran) -> Value {
    let events: Vec<Value> = ran
        .events
        .iter()
        .map(|e| match e {
            Event::Answer { who, call, args, answer } => json!({ "call": call, "args": args, "by": who.word(), "answer": answer }),
            Event::Pass { seconds, why, until, after } => {
                let mut o = serde_json::Map::new();
                o.insert("pass".into(), seconds_json(*seconds));
                o.insert("why".into(), json!(why_word(*why)));
                if let Some(u) = until {
                    o.insert("until".into(), json!(u));
                }
                o.insert("after".into(), json!(after));
                Value::Object(o)
            }
            Event::Expired { book, transfer, key } => json!({ "expired": { "book": book, "kind": transfer, "key": key } }),
        })
        .collect();
    let books: Vec<Value> = ran.books.iter().map(book_json).collect();
    json!({
        "ritsu": env!("CARGO_PKG_VERSION"),
        "flow": flow,
        "scenario": scenario,
        "target": target,
        "diagnostics": diagnostics,
        "start": ran.start,
        "end": ran.end,
        "elapsed": seconds_json(ran.elapsed),
        "events": events,
        "trace": ran.trace,
        "books": books,
        "replay": ran.replay,
    })
}

/// A book as `chobo run --format json` writes the accounts and the holds of a scenario's end.
fn book_json(b: &BookEnd) -> Value {
    let before: Vec<Value> = b
        .before
        .iter()
        .map(|(op, result)| {
            let mut o = op.as_object().cloned().unwrap_or_default();
            o.insert("result".into(), json!(result));
            Value::Object(o)
        })
        .collect();
    let accounts: Vec<Value> =
        b.accounts.iter().map(|(name, args, bal)| json!({ "account": name, "args": args, "posted": bal.posted as i64, "held_in": bal.held_in as i64, "held_out": bal.held_out as i64 })).collect();
    let holds: Vec<Value> = b.holds.iter().map(|(kind, key, state)| json!({ "kind": kind, "key": key, "state": state })).collect();
    json!({ "book": b.name, "file": b.file.display().to_string(), "before": before, "accounts": accounts, "holds": holds })
}

fn seconds_json(s: f64) -> Value {
    if s.fract() == 0.0 {
        json!(s as u64)
    } else {
        json!(s)
    }
}

fn why_word(w: Passing) -> &'static str {
    match w {
        Passing::Wait => "wait",
        Passing::Retry => "retry",
        Passing::Timeout => "timeout",
    }
}

// ── as a person reads it ──────────────────────────────────────────────────

/// How wide a string shows in a terminal: East Asian wide characters take two columns.
fn width(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let u = c as u32;
            let wide = matches!(u, 0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x20000..=0x3FFFD);
            if wide {
                2
            } else {
                1
            }
        })
        .sum()
}

fn pad(s: &str, w: usize) -> String {
    format!("{s}{}", " ".repeat(w.saturating_sub(width(s))))
}

/// A value on one line, as JSON writes it.
fn compact(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// `reserve(order: "A-1", sku: "pen", qty: 3)`.
fn call_text(call: &str, args: &serde_json::Map<String, Value>) -> String {
    let shown: Vec<String> = args.iter().map(|(k, v)| format!("{k}: {}", compact(v))).collect();
    format!("{call}({})", shown.join(", "))
}

/// A length of time: `22 days 23:00:00`, `10 seconds`; `22 日と 23:00:00`.
fn duration_text(seconds: f64, lang: Lang) -> String {
    let whole = seconds.floor() as u64;
    let frac = seconds - whole as f64;
    let (days, rest) = (whole / 86_400, whole % 86_400);
    let clock = {
        let s = format!("{:02}:{:02}:{:02}", rest / 3600, rest / 60 % 60, rest % 60);
        if frac > 0.0 { format!("{s}{}", format!("{frac:.3}").trim_start_matches('0')) } else { s }
    };
    match (days, rest == 0 && frac == 0.0, lang) {
        (0, _, Lang::En) if whole < 60 => format!("{} second{}", fmt_seconds(seconds), if seconds == 1.0 { "" } else { "s" }),
        (0, _, Lang::Ja) if whole < 60 => format!("{} 秒", fmt_seconds(seconds)),
        (0, _, _) => clock,
        (d, true, Lang::En) => format!("{d} day{}", if d == 1 { "" } else { "s" }),
        (d, true, Lang::Ja) => format!("{d} 日"),
        (d, false, Lang::En) => format!("{d} day{} {clock}", if d == 1 { "" } else { "s" }),
        (d, false, Lang::Ja) => format!("{d} 日と {clock}"),
    }
}

fn fmt_seconds(s: f64) -> String {
    if s.fract() == 0.0 {
        format!("{}", s as u64)
    } else {
        format!("{s}")
    }
}

/// What an answer says, as a person reads it: a book's done, done before or refusal in chobo's
/// words; a value as JSON writes it; an error by its name, with its cause when it has one.
fn answer_text(who: Who, answer: &Value, lang: Lang) -> String {
    if answer.get("cancel").is_some() {
        return tr!("キャンセル", "cancelled").get(lang).to_string();
    }
    if let Some(v) = answer.get("ok") {
        if who == Who::Chobo {
            return match v["result"].as_str() {
                Some("done_before") => tr!("done_before（前に済んでいる）", "done_before").get(lang).to_string(),
                _ => tr!("通る", "done").get(lang).to_string(),
            };
        }
        return compact(v);
    }
    let kind = answer["error"].as_str().unwrap_or("failure");
    match (who, answer.get("cause").and_then(|c| c.as_str())) {
        (Who::Chobo, Some(reason)) if kind == reason => tr!("{reason} で拒否される", "refused: {reason}").get(lang).to_string(),
        (Who::Chobo, Some(reason)) => tr!("{reason} で拒否される（{kind}）", "refused: {reason} ({kind})").get(lang).to_string(),
        (_, Some(cause)) => tr!("エラー {kind}: {cause}", "error {kind}: {cause}").get(lang).to_string(),
        (_, None) => tr!("エラー {kind}", "error {kind}").get(lang).to_string(),
    }
}

/// How the run ended, as a person reads it.
fn end_text(end: &Value, lang: Lang) -> String {
    if let Some(v) = end.get("succeed") {
        return tr!("終わり: succeed {}", "end: succeed {}", compact(v)).get(lang).to_string();
    }
    if let Some(f) = end.get("fail") {
        let (error, cause) = (f["error"].as_str().unwrap_or_default(), compact(&f["cause"]));
        return tr!("終わり: fail {error}（{cause}）", "end: fail {error} ({cause})").get(lang).to_string();
    }
    tr!("終わり: キャンセル", "end: cancelled").get(lang).to_string()
}

/// The text of a run.
fn text(flow: &str, scenario: &str, ran: &Ran, lang: Lang) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}\n", tr!("{flow}（シナリオ {scenario}、{} から）", "{flow} (scenario {scenario}, from {})", ran.start).get(lang)));
    for b in ran.books.iter().filter(|b| !b.before.is_empty()) {
        out.push_str(&format!("{}\n", tr!("走らせる前の帳簿 {}:", "before the run, book {}:", b.name).get(lang)));
        let rows: Vec<(String, String)> = b
            .before
            .iter()
            .map(|(op, result)| {
                let args = op["args"].as_object().cloned().unwrap_or_default();
                let call = call_text(&format!("{}.{}", op["kind"].as_str().unwrap_or_default(), op["op"].as_str().unwrap_or_default()), &args);
                let said = if result == "done_before" { tr!("done_before（前に済んでいる）", "done_before") } else { tr!("通る", "done") };
                (call, said.get(lang).to_string())
            })
            .collect();
        let w = rows.iter().map(|(a, _)| width(a)).max().unwrap_or(0);
        for (a, b) in rows {
            out.push_str(&format!("  {}  {b}\n", pad(&a, w)));
        }
    }
    // each event a row: what happened, who answered (for a call), and what it came to
    out.push_str(&format!("{}\n", tr!("実行:", "the run:").get(lang)));
    let mut rows: Vec<(String, String, String)> = Vec::new();
    for e in &ran.events {
        match e {
            Event::Answer { who, call, args, answer } => rows.push((call_text(call, args), who.word().to_string(), answer_text(*who, answer, lang))),
            Event::Pass { seconds, why, until, after } => {
                let what = match (why, until) {
                    (Passing::Wait, Some(u)) => format!("wait until {u}"),
                    (Passing::Wait, None) => format!("wait {}", duration_text(*seconds, Lang::En)),
                    (Passing::Retry, _) => tr!("リトライの前の待ち", "the wait before a retry").get(lang).to_string(),
                    (Passing::Timeout, _) => tr!("タイムアウト", "the timeout").get(lang).to_string(),
                };
                let (ja, en) = (duration_text(*seconds, Lang::Ja), duration_text(*seconds, Lang::En));
                let went = tr!("{ja}、{after} まで", "{en}, to {after}");
                rows.push((what, String::new(), went.get(lang).to_string()));
            }
            Event::Expired { book, transfer, key } => {
                let hold = format!("{transfer}({})", key.join(", "));
                rows.push((tr!("帳簿 {book} の仮押さえ {hold}", "the hold {hold} of the book {book}").get(lang).to_string(), String::new(), tr!("期限切れ", "expired").get(lang).to_string()));
            }
        }
    }
    let wa = rows.iter().map(|(a, _, _)| width(a)).max().unwrap_or(0);
    let wb = rows.iter().map(|(_, b, _)| width(b)).max().unwrap_or(0);
    for (a, b, c) in rows {
        out.push_str(&format!("  {}  {}  {c}\n", pad(&a, wa), pad(&b, wb)));
    }
    out.push_str(&format!("{}\n", end_text(&ran.trace["end"], lang)));
    for b in &ran.books {
        out.push_str(&format!("{}\n", tr!("終わりの帳簿 {}（{}）:", "book {} ({}) at the end:", b.name, b.file.display()).get(lang)));
        if !b.accounts.is_empty() {
            out.push_str(&format!("  {}\n", tr!("勘定:", "accounts:").get(lang)));
            let names: Vec<String> = b.accounts.iter().map(|(n, args, _)| if args.is_empty() { n.clone() } else { format!("{n}({})", args.join(", ")) }).collect();
            let w = names.iter().map(|n| width(n)).max().unwrap_or(0);
            for ((_, _, bal), name) in b.accounts.iter().zip(&names) {
                let (p, o, i) = (bal.posted, bal.held_out, bal.held_in);
                let line = tr!("確定 {p}、出ていく仮押さえ {o}、入ってくる仮押さえ {i}", "posted {p}, held out {o}, held in {i}");
                out.push_str(&format!("    {}  {}\n", pad(name, w), line.get(lang)));
            }
        }
        if !b.holds.is_empty() {
            out.push_str(&format!("  {}\n", tr!("仮押さえ:", "holds:").get(lang)));
            let names: Vec<String> = b.holds.iter().map(|(k, key, _)| format!("{k}({})", key.join(", "))).collect();
            let w = names.iter().map(|n| width(n)).max().unwrap_or(0);
            for ((_, _, state), name) in b.holds.iter().zip(&names) {
                let s = match state.as_str() {
                    "held" => tr!("押さえ中", "held"),
                    "posted" => tr!("確定", "posted"),
                    "voided" => tr!("取消", "voided"),
                    _ => tr!("期限切れ", "expired"),
                };
                out.push_str(&format!("    {}  {}\n", pad(name, w), s.get(lang)));
            }
        }
        if b.accounts.is_empty() && b.holds.is_empty() {
            out.push_str(&format!("  {}\n", tr!("何も動いていません", "nothing has moved").get(lang)));
        }
    }
    out
}
