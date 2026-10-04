//! What DESIGN.md shows a command printing is what the command prints, in English: the twin of
//! design.rs for the blocks that read the English material (their commands are written in ASCII:
//! `tests/fixtures/period_of_months`, the mutants of English names). design.rs runs every
//! `$ yuen …` block, these included; this one holds that there are the English ones, and that they
//! are right. Run with every language joined, as `ritsu yuen` runs it.

mod common;

/// The material of English: the fixtures written in English and the mutants of ASCII names (the
/// Japanese mutants have names in Japanese, and the fixtures `period`, `payment`, `rulec`, `koyomi`,
/// `chobo`, `proto`, `dandori` and `sakai` are Japanese).
const ENGLISH: [&str; 11] = [
    "tests/fixtures/period_of_months",
    "tests/fixtures/payment_policy",
    "tests/fixtures/fee_rules",
    "tests/fixtures/calendar_sources",
    "tests/fixtures/refunds_book",
    "tests/fixtures/warehouse_proto",
    "tests/fixtures/delivery_flow",
    "tests/fixtures/ordering_terms",
    "tests/fixtures/ecfr",
    "tests/fixtures/geas",
    "tests/mutants/E",
];

#[test]
fn every_command_on_english_material_in_design_prints_what_design_shows() {
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
    for (s, e, b) in blocks.iter().filter(|(_, _, b)| b.starts_with("$ yuen ")) {
        let near = lines[s.saturating_sub(3)..(*e + 4).min(lines.len())].join("\n");
        if near.contains("形の案") {
            continue;
        }
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
            if !cmd.is_ascii() || !ENGLISH.iter().any(|m| cmd.contains(m)) || cmd.starts_with("yuen source fetch ") || cmd.starts_with("yuen source outdated ") {
                continue;
            }
            let words: Vec<String> = cmd.split(' ').skip(1).map(|w| w.to_string()).collect();
            let words: Vec<&str> = words.iter().map(|w| w.as_str()).collect();
            let got = common::run(&words).stdout;
            if got != want {
                failures.push(format!("$ {cmd}\n--- DESIGN.md shows\n{want}--- it prints\n{got}"));
            }
            n += 1;
        }
    }
    assert!(n >= 5, "{n} commands on English material in DESIGN.md");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
