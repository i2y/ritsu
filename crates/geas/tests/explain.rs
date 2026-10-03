//! `geas explain`: the whole table in both languages, and every repro in it run to
//! check that it gives its own code, which keeps the table honest.

mod common;
use common::*;
use std::collections::BTreeSet;
use std::fs;

#[test]
fn the_whole_table_in_both_languages() {
    let s = Scratch::new("explain-all");
    for (lang, args) in [("en", vec!["explain", "--all"]), ("ja", vec!["explain", "--all", "--lang", "ja"])] {
        let (out, err, code) = run(s.path(), &args, &[]);
        assert_eq!((err.as_str(), code), ("", 0));
        golden(&format!("{lang}/explain/all.txt"), &out);
    }
    // one code, in either case, is the same text as in the whole table
    let (one, _, code) = run(s.path(), &["explain", "e033"], &[]);
    assert_eq!(code, 0);
    let (all, _, _) = run(s.path(), &["explain", "--all"], &[]);
    assert!(all.contains(&one), "{one}");
}

/// The codes the table has, with their repros, from `geas explain --all --json`.
fn table() -> Vec<Json> {
    let s = Scratch::new("explain-json");
    let (out, err, code) = run(s.path(), &["explain", "--all", "--json"], &[]);
    assert_eq!((err.as_str(), code), ("", 0));
    out.lines().map(json).collect()
}

#[test]
fn every_repro_gives_its_own_code() {
    let mut failures = Vec::new();
    for e in table() {
        let code = e.get("code").str().to_string();
        let repro = e.get("repro");
        let needs: Vec<&str> = repro.get("needs").arr().iter().map(Json::str).collect();
        let present = |tool: &&str| match *tool {
            "llvm-tools" => llvm_bin().is_some(),
            "chrome" => chrome_path().is_some(),
            _ => have(tool, &["--version"]),
        };
        if let Some(missing) = needs.iter().find(|tool| !present(tool)) {
            skip(&format!("{missing} is not at hand; the repro of {code} is not run"));
            continue;
        }
        let s = Scratch::new(&format!("repro-{code}"));
        let mut fixed_port = false;
        for f in repro.get("files").arr() {
            let text = f.get("text").str();
            fixed_port |= text.contains("port 8123");
            s.write(f.get("name").str(), text);
        }
        let _port = if fixed_port { Some(port_lock()) } else { None };
        let args: Vec<&str> = repro.get("args").arr().iter().map(Json::str).collect();
        let log = pid_log(&s);
        let log_s = log.to_str().expect("a UTF-8 path");
        let mut env: Vec<(&str, &str)> = vec![("GEAS_PID_LOG", log_s)];
        // the Chrome the tests found, unless the repro names its own
        let chrome = chrome_path().map(|c| c.to_string_lossy().into_owned());
        if let Some(c) = &chrome {
            env.push(("GEAS_CHROME", c.as_str()));
        }
        if let Json::Obj(pairs) = repro.get("env") {
            env.retain(|(k, _)| !pairs.iter().any(|(p, _)| p == k));
            env.extend(pairs.iter().map(|(k, v)| (k.as_str(), v.str())));
        }
        let (out, err, exit) = run(s.path(), &args, &env);
        let mark = format!("[{code}]");
        if !(out.contains(&mark) || err.contains(&mark)) {
            failures.push(format!("{code}: `geas {}` does not give it:\n{out}{err}", args.join(" ")));
        }
        if exit != repro.get("exit").num() as i32 {
            failures.push(format!("{code}: exits {exit}, the table says {}", repro.get("exit").num()));
        }
        no_process_left(&log);
        if needs.contains(&"chrome") || code == "E034" {
            no_chrome_left(&s);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Every code a golden shows is in the table: nothing geas prints lacks an entry.
#[test]
fn every_code_geas_prints_is_in_the_table() {
    let known: BTreeSet<String> = table().iter().map(|e| e.get("code").str().to_string()).collect();
    let mut seen = BTreeSet::new();
    let mut stack = vec![root().join("tests/golden")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("a golden directory") {
            let p = entry.expect("an entry").path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let text = fs::read_to_string(&p).expect("a golden file");
            for (i, _) in text.match_indices("[E").chain(text.match_indices("[W")) {
                let code = &text[i + 1..(i + 5).min(text.len())];
                if code.len() == 4 && code[1..].chars().all(|c| c.is_ascii_digit()) {
                    seen.insert(code.to_string());
                }
            }
        }
    }
    let missing: Vec<&String> = seen.difference(&known).collect();
    assert!(missing.is_empty(), "codes in goldens with no entry in the table: {missing:?}");
    // and the table has what stages B and C bring
    for c in [
        "E001", "E002", "E003", "E004", "E005", "E006", "E007", "E008", "E009", "E010", "E011", "E012", "E013", "E030", "E031",
        "E032", "E033", "E034", "E035", "E036", "E037", "E050", "E051", "E060", "E061", "E062", "E063", "E064", "E065", "E066",
        "E080", "E081", "W060", "W061",
    ] {
        assert!(known.contains(c), "{c} has no entry");
    }
}
