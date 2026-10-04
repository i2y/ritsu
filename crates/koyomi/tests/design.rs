//! What DESIGN.md shows a command printing is what the command prints. Every fenced block
//! that starts with `$ koyomi …` is run, from the root of the repository, and compared.

use std::process::Command;

#[test]
fn every_command_in_design_prints_what_design_shows() {
    let text = std::fs::read_to_string("DESIGN.md").unwrap();
    let mut blocks: Vec<String> = Vec::new();
    let mut cur: Option<String> = None;
    for l in text.lines() {
        if l.starts_with("```") {
            match cur.take() {
                Some(b) => blocks.push(b),
                None => cur = Some(String::new()),
            }
            continue;
        }
        if let Some(b) = cur.as_mut() {
            b.push_str(l);
            b.push('\n');
        }
    }
    let mut n = 0;
    let mut failures = Vec::new();
    for b in blocks.iter().filter(|b| b.starts_with("$ koyomi ")) {
        // A block may hold several commands, each followed by what it prints.
        let mut runs: Vec<(String, String)> = Vec::new();
        for l in b.lines() {
            if let Some(cmd) = l.strip_prefix("$ ") {
                runs.push((cmd.to_string(), String::new()));
            } else if let Some((_, out)) = runs.last_mut() {
                out.push_str(l);
                out.push('\n');
            }
        }
        for (cmd, want) in runs {
            let args: Vec<&str> = cmd.split_whitespace().skip(1).collect();
            let o = Command::new(env!("CARGO_BIN_EXE_koyomi")).args(&args).env_remove("KOYOMI_LANG").env_remove("RITSU_LANG").output().unwrap();
            let got = String::from_utf8_lossy(&o.stdout).to_string();
            if got != want {
                failures.push(format!("$ {cmd}\n--- DESIGN.md shows\n{want}--- it prints\n{got}"));
            }
            n += 1;
        }
    }
    assert!(n >= 8, "{n} commands in DESIGN.md");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The fenced blocks of DESIGN.md whose opening line is ```` ```<lang> ````, with their text.
fn blocks_of(text: &str, langs: &[&str]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut cur: Option<(String, String)> = None;
    for l in text.lines() {
        if let Some(info) = l.strip_prefix("```") {
            match cur.take() {
                Some(b) => out.push(b),
                None => cur = Some((info.trim().to_string(), String::new())),
            }
            continue;
        }
        if let Some((_, b)) = cur.as_mut() {
            b.push_str(l);
            b.push('\n');
        }
    }
    out.into_iter().filter(|(lang, _)| langs.contains(&lang.as_str())).collect()
}

#[test]
fn the_code_design_shows_is_the_code_gen_writes() {
    // Every ```ts, ```python, ```go, ```rust and ```sql block of DESIGN.md is an excerpt of
    // what gen writes for an example, and every line of a ```jsonl block is a line of the
    // vectors of one: DESIGN quotes real output, not a sketch.
    let text = std::fs::read_to_string("DESIGN.md").unwrap();
    let examples = [
        "examples/calendars/england_and_wales.cal",
        "examples/net30.cal",
        "examples/close_20th_pay_10th.cal",
        "examples/close_and_pay_on_given_days.cal",
        "examples/period_of_months.cal",
        "examples/calendars/tokyo_business_days.cal",
        "examples/calendars/civil_code_142_days.cal",
        "examples/payment_20th_close_next_10th.cal",
        "examples/closing_and_payment_days_as_inputs.cal",
        "examples/civil_code_period_end.cal",
        "examples/payment_20th_close_next_10th.ja.cal",
        "examples/civil_code_period_end.ja.cal",
        "examples/closing_and_payment_days_as_inputs.ja.cal",
        "examples/calendars/東京の営業日.cal",
        "examples/calendars/民法142条の休日.cal",
    ];
    let mut generated: Vec<(String, String)> = Vec::new();
    let mut vectors: Vec<String> = Vec::new();
    for p in examples {
        let mut o = koyomi::check::check(p).unwrap();
        let checked = o.checked.take().unwrap();
        for lang in [ritsu_base::text::Lang::En, ritsu_base::text::Lang::Ja] {
            let u = koyomi::codegen::unit_of(&checked, lang);
            for t in koyomi::naming::TARGETS {
                for (_, body) in koyomi::codegen::files(&u, t) {
                    generated.push((t.key().to_string(), body));
                }
            }
        }
        if let koyomi::check::Checked::Dates(m, _) = &checked
            && m.combinations() < 100_000
        {
            let w = koyomi::vectors::DatesLines::new(m);
            vectors.extend(koyomi::vectors::DatesRows::new(m).map(|r| w.line(&r)));
        }
    }
    let mut n = 0;
    for (lang, body) in blocks_of(&text, &["ts", "python", "go", "rust", "sql"]) {
        let key = match lang.as_str() {
            "ts" => "typescript",
            l => l,
        };
        let found = generated.iter().any(|(k, g)| k == key && g.contains(&body));
        assert!(found, "DESIGN.md's ```{lang} block is not in what gen writes for an example:\n{body}");
        n += 1;
    }
    for (_, body) in blocks_of(&text, &["jsonl"]) {
        for l in body.lines() {
            assert!(vectors.iter().any(|v| v == l), "DESIGN.md's vectors line is not a line of an example's vectors:\n{l}");
            n += 1;
        }
    }
    assert!(n >= 4, "{n} excerpts in DESIGN.md");
}

#[test]
fn the_page_design_shows_is_the_page_doc_writes() {
    // Every ```markdown block of DESIGN.md is cut from a page `koyomi doc` writes for an
    // example (a golden file of tests/golden/doc, which tests/doc.rs holds to doc): the pieces
    // between lines of `…` come in that order in one page.
    let text = std::fs::read_to_string("DESIGN.md").unwrap();
    let pages: Vec<String> = std::fs::read_dir("tests/golden/doc").unwrap().map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap()).collect();
    let mut n = 0;
    for (_, body) in blocks_of(&text, &["markdown"]) {
        let mut pieces: Vec<String> = vec![String::new()];
        for l in body.lines() {
            if l.trim() == "…" {
                pieces.push(String::new());
            } else {
                let p = pieces.last_mut().unwrap();
                p.push_str(l);
                p.push('\n');
            }
        }
        let found = pages.iter().any(|page| {
            let mut at = 0;
            pieces.iter().filter(|p| !p.is_empty()).all(|p| match page[at..].find(p.as_str()) {
                Some(i) => {
                    at += i + p.len();
                    true
                }
                None => false,
            })
        });
        assert!(found, "DESIGN.md's ```markdown block is not cut from a page doc writes:\n{body}");
        n += 1;
    }
    assert!(n >= 1, "{n} pages in DESIGN.md");
}
