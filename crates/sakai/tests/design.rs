//! What DESIGN.md shows sakai printing is what sakai prints (PLAN B.11, 0.1): every fenced block
//! that starts with `$ sakai …` is run from the root of the repository and compared; every block
//! of sakai's diagnostics is a golden file of a mutant (or part of one); the ```json block of
//! chapter 9 is cut from the golden file of `sakai api`; and the name of 2.6 is what the name it
//! spells gives.

mod common;

use std::process::Command;

/// The fenced blocks of DESIGN.md: the info string, and the text.
fn blocks() -> Vec<(String, String)> {
    let text = std::fs::read_to_string("DESIGN.md").unwrap();
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
    out
}

#[test]
fn every_command_in_design_prints_what_design_shows() {
    let mut n = 0;
    let mut failures = Vec::new();
    for (_, b) in blocks().iter().filter(|(_, b)| b.starts_with("$ sakai ")) {
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
            let o = Command::new(env!("CARGO_BIN_EXE_sakai")).args(&args).env_remove("SAKAI_LANG").output().unwrap();
            let got = String::from_utf8_lossy(&o.stdout).to_string();
            if got != want {
                failures.push(format!("$ {cmd}\n--- DESIGN.md shows\n{want}--- it prints\n{got}"));
            }
            n += 1;
        }
    }
    assert!(n >= 2, "{n} commands in DESIGN.md");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The code of a diagnostic's first line, if the block is sakai's (its code is in the ledger).
fn sakai_code(b: &str) -> Option<String> {
    let first = b.lines().next()?;
    // The form of 5.1 is a template, not something printed.
    if first.contains("<ファイル>") {
        return None;
    }
    let rest = ["error[", "warning[", "note[", "エラー[", "警告[", "備考["].iter().find_map(|p| first.strip_prefix(p))?;
    let code = rest.split(']').next()?;
    sakai::codes::find(code).map(|_| code.to_string())
}

#[test]
fn every_diagnostic_design_shows_is_one_sakai_printed() {
    let goldens: Vec<String> = std::fs::read_dir("tests/golden").unwrap().filter_map(|e| e.ok()).filter(|e| e.path().is_file()).map(|e| std::fs::read_to_string(e.path()).unwrap()).collect();
    let mut n = 0;
    for (_, b) in blocks() {
        let Some(code) = sakai_code(&b) else { continue };
        assert!(goldens.iter().any(|g| g.contains(&b)), "DESIGN.md's {code} block is not in a golden file of tests/golden:\n{b}");
        n += 1;
    }
    assert!(n >= 3, "{n} diagnostics in DESIGN.md");
}

#[test]
fn the_api_design_shows_is_cut_from_the_golden_file() {
    let golden = std::fs::read_to_string("tests/golden/api/基本.json").unwrap();
    let mut n = 0;
    for (_, b) in blocks().iter().filter(|(info, b)| info == "json" && b.starts_with("{\n  \"sakai\"")) {
        let mut at = 0;
        for piece in b.split("…\n") {
            match golden[at..].find(piece) {
                Some(i) => at += i + piece.len(),
                None => panic!("this piece of DESIGN.md's api is not in tests/golden/api/基本.json, in its order:\n{piece}"),
            }
        }
        n += 1;
    }
    assert_eq!(n, 1, "one ```json block of the api");
}

#[test]
fn the_name_design_shows_in_json_is_the_name_it_spells() {
    let mut n = 0;
    for (_, b) in blocks().iter().filter(|(info, b)| info == "json" && b.starts_with("{\"text\"")) {
        let v: serde_json::Value = serde_json::from_str(b.trim()).unwrap();
        let name = sakai::naming::parse(v["text"].as_str().unwrap(), ".").unwrap();
        assert_eq!(serde_json::to_string(&name.to_json()).unwrap(), b.trim());
        n += 1;
    }
    assert!(n >= 1);
}

/// Whether the pieces of a block (cut where a line is `…`) are in `src`, in order.
fn in_order(src: &str, block: &str) -> bool {
    let mut at = 0;
    for piece in block.split("…\n") {
        match src[at..].find(piece) {
            Some(i) => at += i + piece.len(),
            None => return false,
        }
    }
    true
}

fn read_all(paths: &[String]) -> Vec<String> {
    paths.iter().map(|p| std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p}: {e}"))).collect()
}

fn files_in(dir: &str, ext: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path().to_string_lossy().to_string()).filter(|p| p.ends_with(ext)).collect();
    v.sort();
    v
}

/// What chapters 7 and 8 show is cut from what was written: the settings (```ini, ```js, ```java,
/// ```yaml) from the example's settings files, which `tests/build.rs` holds to what sakai writes;
/// the CML (```cml) from its golden files; the tools' output (```text) from
/// `tests/golden/imports`, which `tests/imports.rs` holds to what the tools say.
#[test]
fn the_settings_cml_and_tool_output_design_shows_are_what_was_written() {
    let ex = common::EXAMPLE;
    let settings = read_all(&[
        format!("{ex}/py/.importlinter"),
        format!("{ex}/ts/.dependency-cruiser.cjs"),
        format!("{ex}/java/src/test/java/SakaiContextsTest.java"),
        format!("{ex}/go/.go-arch-lint.yml"),
    ]);
    let cml = read_all(&files_in("tests/golden/cml", ".cml"));
    let outputs = read_all(&files_in("tests/golden/imports", ".txt"));
    let mut n = 0;
    for (info, b) in blocks() {
        let (sources, what) = match info.as_str() {
            "ini" | "js" | "java" | "yaml" => (&settings, "the example's settings files"),
            "cml" => (&cml, "tests/golden/cml"),
            "text" => (&outputs, "tests/golden/imports"),
            _ => continue,
        };
        assert!(sources.iter().any(|s| in_order(s, &b)), "this ```{info} block of DESIGN.md is not in {what}, in its order:\n{b}");
        n += 1;
    }
    assert!(n >= 9, "{n} blocks of settings, CML and tool output in DESIGN.md");
}
