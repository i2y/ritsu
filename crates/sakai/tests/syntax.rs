//! The syntax (PLAN B.2): every `.ctx` block of DESIGN.md and PLAN.md reads, E001 to E005 come
//! where they should, and positions count characters.

use sakai::ast::File;
use sakai::parse::parse;

/// The ```ctx blocks of a Markdown file, with the line each starts on.
pub fn ctx_blocks(path: &str) -> Vec<(usize, String)> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut out = Vec::new();
    let mut cur: Option<(usize, String)> = None;
    for (i, l) in text.lines().enumerate() {
        if l.starts_with("```") {
            match cur.take() {
                Some(b) => out.push(b),
                None if l == "```ctx" => cur = Some((i + 2, String::new())),
                None => {}
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

/// A block that is only some sections of a context file is read inside one.
fn as_file(block: &str) -> String {
    let first = block.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    if first.starts_with("map ") || first.starts_with("context ") {
        return block.to_string();
    }
    if first.starts_with("owns") {
        return format!("context 断片(fragment) v1\n{block}");
    }
    format!("context 断片(fragment) v1\nowns\n  dir \".\"\n{block}")
}

#[test]
fn every_ctx_block_of_design_and_plan_reads() {
    let mut n = 0;
    let mut failures = Vec::new();
    for doc in ["DESIGN.md", "PLAN.md"] {
        for (line, block) in ctx_blocks(doc) {
            let text = as_file(&block);
            let (f, ds) = parse("block.ctx", &text);
            if f.is_none() || !ds.is_empty() {
                let shown: String = ds.iter().map(|d| d.render(ritsu_base::text::Lang::En)).collect();
                failures.push(format!("{doc}:{line}:\n{shown}"));
            }
            n += 1;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(n >= 13, "{n} blocks");
}

#[test]
fn a_map_and_a_context_read_into_their_parts() {
    let (f, ds) = parse("m.ctx", "map 通販(shop) v1\ndescription \"d\"\nuse context \"a.ctx\"\ncovers \".\"\nproto root \"proto\"\ncode java \"java/src/main/java\"\n  test \"java/src/test/java\"\n");
    assert!(ds.is_empty(), "{ds:?}");
    let Some(File::Map(m)) = f else { panic!() };
    assert_eq!(m.heading.name, "通販");
    assert_eq!(m.heading.alias.as_deref(), Some("shop"));
    assert_eq!(m.code[0].test.as_ref().unwrap().value, "java/src/test/java");
    let src = "context 請求(billing) v1\nowns\n  dir \"../a\", rulec \"../b.rule\"\nterms\n  キャンセル \"返金すること\"\n  注文 as 受注.注文\nupstream 受注 customer, anticorruption layer\n  through shop.ordering.v1\n  enum OrderStatus -> rulec \"../b.rule\" enum 注文の状態\n  enum Packing -> 出荷の可否\n    A -> 待つ\n    B -> refuse \"欠品\"\n  term 引当 -> 押さえ\n";
    let (f, ds) = parse("c.ctx", src);
    assert!(ds.is_empty(), "{ds:?}");
    let Some(File::Context(c)) = f else { panic!() };
    assert_eq!(c.owns.len(), 2);
    assert_eq!(c.owns[1].tool, Some(sakai::naming::Tool::Rulec));
    assert_eq!(c.terms[1].as_term.as_ref().unwrap().term, "注文");
    let sakai::ast::RelKind::Upstream(u) = &c.relations[0].kind else { panic!() };
    assert_eq!(u.roles.len(), 2);
    assert_eq!(u.enums.len(), 2);
    assert_eq!(u.enums[1].values.len(), 2);
    assert_eq!(u.terms[0].to, "押さえ");
}

#[test]
fn columns_after_japanese_count_characters() {
    let (_, ds) = parse("c.ctx", "context 請求(billing) v1\nowns\n  dir \"a\"\nterms\n  キャンセル \"返金\" 余分\n");
    assert_eq!(ds[0].code, "E002");
    assert_eq!((ds[0].line, ds[0].col), (Some(5), Some(14)));
}

// ── The English twins ──

#[test]
fn a_map_and_a_context_read_into_their_parts_in_english() {
    let (f, ds) = parse("m.ctx", "map Shop(shop) v1\ndescription \"d\"\nuse context \"a.ctx\"\ncovers \".\"\nproto root \"proto\"\ncode java \"java/src/main/java\"\n  test \"java/src/test/java\"\n");
    assert!(ds.is_empty(), "{ds:?}");
    let Some(File::Map(m)) = f else { panic!() };
    assert_eq!(m.heading.name, "Shop");
    assert_eq!(m.heading.alias.as_deref(), Some("shop"));
    assert_eq!(m.code[0].test.as_ref().unwrap().value, "java/src/test/java");
    let src = "context Billing(billing) v1\nowns\n  dir \"../a\", rulec \"../b.rule\"\nterms\n  cancel \"Refunding it\"\n  order as Ordering.order\nupstream Ordering customer, anticorruption layer\n  through shop.ordering.v1\n  enum OrderStatus -> rulec \"../b.rule\" enum order_status\n  enum Packing -> shipping_decision\n    A -> wait\n    B -> refuse \"short\"\n  term reservation -> hold\n";
    let (f, ds) = parse("c.ctx", src);
    assert!(ds.is_empty(), "{ds:?}");
    let Some(File::Context(c)) = f else { panic!() };
    assert_eq!(c.owns.len(), 2);
    assert_eq!(c.owns[1].tool, Some(sakai::naming::Tool::Rulec));
    assert_eq!(c.terms[1].as_term.as_ref().unwrap().term, "order");
    let sakai::ast::RelKind::Upstream(u) = &c.relations[0].kind else { panic!() };
    assert_eq!(u.roles.len(), 2);
    assert_eq!(u.enums.len(), 2);
    assert_eq!(u.enums[1].values.len(), 2);
    assert_eq!(u.terms[0].to, "hold");
}

#[test]
fn columns_count_characters_in_english() {
    let (_, ds) = parse("c.ctx", "context Billing(billing) v1\nowns\n  dir \"a\"\nterms\n  cancel \"refund\" extra\n");
    assert_eq!(ds[0].code, "E002");
    assert_eq!((ds[0].line, ds[0].col), (Some(5), Some(19)));
}
