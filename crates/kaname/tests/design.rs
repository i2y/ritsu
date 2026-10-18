//! DESIGN.md shows the language in blocks of `kn`: every one reads as kaname (DESIGN 2.3), with a
//! heading `module design v1` put before one that shows declarations without it. A block that leaves
//! lines out (`…`) shows part of a module and is not read; there are few of them, and they are
//! counted. The examples the blocks are taken from are checked whole by `tests/examples.rs`.

use ritsu_base::text::Lang;

#[test]
fn every_block_of_kn_in_the_design_reads_as_kaname() {
    let design = std::fs::read_to_string("DESIGN.md").unwrap();
    let (mut read, mut left_out) = (0, 0);
    let mut wrong = Vec::new();
    for (k, part) in design.split("```kn\n").enumerate().skip(1) {
        let block = part.split("```").next().unwrap();
        if block.lines().any(|l| l.trim() == "…") {
            left_out += 1;
            continue;
        }
        let src = if block.lines().any(|l| l.starts_with("module ")) { block.to_string() } else { format!("module design v1\n\n{block}") };
        let p = kaname::parse::parse("design.kn", &src);
        if !p.diags.is_empty() || p.module.is_none() {
            let said: Vec<String> = p.diags.iter().map(|d| d.render(Lang::En)).collect();
            wrong.push(format!("block {k}:\n{block}\n{}", said.join("")));
        }
        read += 1;
    }
    assert!(read >= 7 && left_out <= 1, "{read} blocks read, {left_out} left out: were the fences changed?");
    assert!(wrong.is_empty(), "blocks of DESIGN.md that do not read as kaname:\n{}", wrong.join("\n"));
}
