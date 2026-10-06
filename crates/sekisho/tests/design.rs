//! DESIGN.md shows the language by lines of the example: every line of its ```gate blocks (but `…`,
//! which leaves lines out) is a line of `examples/refunds/refunds.gate` or of `refunds.ja.gate`,
//! spaces in a row read as one, so that the document and the example cannot drift apart.

/// A line with the spaces after its indentation in a row read as one.
fn squeezed(l: &str) -> String {
    let body = l.trim_start();
    let indent = &l[..l.len() - body.len()];
    format!("{indent}{}", body.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[test]
fn every_line_of_the_design_is_a_line_of_the_example() {
    let design = std::fs::read_to_string("DESIGN.md").unwrap();
    let mut example: Vec<String> = Vec::new();
    for f in ["examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate"] {
        example.extend(std::fs::read_to_string(f).unwrap().lines().map(squeezed));
    }
    let mut blocks = 0;
    let mut missing = Vec::new();
    for (k, part) in design.split("```gate\n").enumerate().skip(1) {
        let block = part.split("```").next().unwrap();
        blocks += 1;
        for l in block.lines().filter(|l| !l.trim().is_empty() && l.trim() != "…") {
            if !example.contains(&squeezed(l)) {
                missing.push(format!("block {k}: {l}"));
            }
        }
    }
    assert!(blocks >= 10, "{blocks} blocks");
    assert!(missing.is_empty(), "lines of DESIGN.md that are no line of the example:\n{}", missing.join("\n"));
}
