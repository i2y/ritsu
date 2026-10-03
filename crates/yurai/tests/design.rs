//! What DESIGN.md shows a command printing is what the command prints. Every fenced block
//! that starts with `$ yurai …` is run from the root of the repository and compared, except
//! the blocks still marked 形の案 (a proposal for a later stage) within three lines of them,
//! and the commands that read the network (`source fetch` and `source outdated`): the tests
//! never do, and what DESIGN.md shows of them was taken from a real run on the day it says.

use std::process::Command;

#[test]
fn every_command_in_design_prints_what_design_shows() {
    let text = std::fs::read_to_string("DESIGN.md").unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks: Vec<(usize, usize, String)> = Vec::new();
    let mut start: Option<usize> = None;
    for (i, l) in lines.iter().enumerate() {
        if l.starts_with("```") {
            match start.take() {
                Some(s) => blocks.push((s, i, lines[s + 1..i].iter().map(|x| format!("{x}\n")).collect())),
                None => start = Some(i),
            }
        }
    }
    let mut n = 0;
    let mut failures = Vec::new();
    for (s, e, b) in blocks.iter().filter(|(_, _, b)| b.starts_with("$ yurai ")) {
        let near = lines[s.saturating_sub(3)..(*e + 4).min(lines.len())].join("\n");
        if near.contains("形の案") {
            continue;
        }
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
            if cmd.starts_with("yurai source fetch ") || cmd.starts_with("yurai source outdated ") {
                continue;
            }
            let args = split(&cmd);
            let o = Command::new(env!("CARGO_BIN_EXE_yurai")).args(&args[1..]).env_remove("YURAI_LANG").output().unwrap();
            let got = String::from_utf8_lossy(&o.stdout).to_string();
            if got != want {
                failures.push(format!("$ {cmd}\n--- DESIGN.md shows\n{want}--- it prints\n{got}"));
            }
            n += 1;
        }
    }
    assert!(n >= 5, "{n} commands in DESIGN.md");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A command line split as a shell would: spaces, and single quotes around a word with spaces.
fn split(cmd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut had = false;
    for c in cmd.chars() {
        match c {
            '\'' => {
                quoted = !quoted;
                had = true;
            }
            ' ' if !quoted => {
                if had || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                had = false;
            }
            _ => cur.push(c),
        }
    }
    if had || !cur.is_empty() {
        out.push(cur);
    }
    out
}
