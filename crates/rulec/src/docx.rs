//! The tables of a Word document (§15.82): `word/document.xml`, walked for `w:tbl`.
//!
//! A `.docx` costs almost nothing to read because the two readers it needs were already
//! written for the workbook (`ooxml`): the container is the same ZIP, the markup is the same
//! XML. What is here is only the shape of a Word table — rows, cells, the text inside them,
//! and the two ways a cell can be merged.
//!
//! **A merged cell is not filled in.** A cell that spans two columns leaves the second one
//! empty, and the continuation of a vertical merge is empty as the document has it. The copy
//! is evidence, so it says what the document says; filling it in would put values in it that
//! nobody wrote. The columns stay aligned either way, which is what a table needs.
//!
//! A table inside a cell is not a fragment of its own: its text flattens into the cell that
//! holds it. Fragments are counted over the tables of the document, and a reader counting
//! them by eye counts those.

use crate::ooxml::{attr, entries, part, unescape, walk, Node};

/// Every table of a Word document, in document order.
pub fn tables(bytes: &[u8]) -> Result<Vec<Vec<Vec<String>>>, String> {
    let es = entries(bytes)?;
    let xml = part(bytes, &es, "word/document.xml")?.ok_or_else(|| {
        tr!("docx ではありません（word/document.xml がありません）", "not a docx (there is no word/document.xml)")
    })?;
    Ok(read(&xml))
}

/// The element's own name, without the namespace prefix a Word document writes on every one
/// of them (`w:tbl`). A prefix is a choice of the writer, so nothing here depends on it.
fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn read(xml: &str) -> Vec<Vec<Vec<String>>> {
    let mut out: Vec<Vec<Vec<String>>> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cell = String::new();
    // How deep inside `w:tbl` the walk is: only the outermost table's rows are collected, and
    // a nested one's text goes on into the cell that holds it.
    let mut depth = 0usize;
    let mut in_cell = false;
    let mut span = 1usize;
    let mut taking = false;
    walk(xml, |n| match n {
        Node::Open { name, empty, .. } if local(name) == "tbl" && !empty => {
            depth += 1;
            if depth == 1 {
                rows.clear();
            }
        }
        Node::Close(name) if local(name) == "tbl" => {
            depth = depth.saturating_sub(1);
            if depth == 0 && !rows.is_empty() {
                out.push(std::mem::take(&mut rows));
            }
        }
        Node::Open { name, empty, .. } if depth == 1 && local(name) == "tr" && !empty => row.clear(),
        Node::Close(name) if depth == 1 && local(name) == "tr" => rows.push(std::mem::take(&mut row)),
        Node::Open { name, empty, .. } if depth == 1 && local(name) == "tc" && !empty => {
            cell.clear();
            span = 1;
            in_cell = true;
        }
        Node::Open { name, attrs, .. } if in_cell && local(name) == "gridSpan" => {
            span = attr(attrs, "w:val")
                .or_else(|| attr(attrs, "val"))
                .and_then(|v| v.parse().ok())
                .unwrap_or(1)
                .max(1);
        }
        Node::Close(name) if depth == 1 && local(name) == "tc" => {
            row.push(std::mem::take(&mut cell));
            for _ in 1..span {
                row.push(String::new());
            }
            in_cell = false;
        }
        Node::Open { name, empty, .. } if local(name) == "t" && !empty => taking = true,
        Node::Text(t) if taking => cell.push_str(&unescape(t)),
        Node::Close(name) if local(name) == "t" => taking = false,
        // A line break, a tab and the end of a paragraph all separate words inside one cell;
        // without them `990円` and `（税込）` would run together into one word.
        Node::Open { name, .. } if in_cell && matches!(local(name), "br" | "tab" | "cr") => cell.push(' '),
        Node::Close(name) if in_cell && local(name) == "p" => cell.push(' '),
        _ => {}
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tc(text: &str) -> String {
        format!("<w:tc><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:tc>")
    }

    #[test]
    fn rows_and_cells() {
        let xml = format!(
            "<w:document><w:body><w:p><w:r><w:t>前書き</w:t></w:r></w:p><w:tbl><w:tr>{}{}</w:tr><w:tr>{}{}</w:tr></w:tbl></w:body></w:document>",
            tc("あて先"),
            tc("運賃"),
            tc("近畿"),
            tc("990円")
        );
        assert_eq!(read(&xml), vec![vec![vec!["あて先 ", "運賃 "], vec!["近畿 ", "990円 "]]]);
    }

    #[test]
    fn a_spanning_cell_leaves_the_rest_empty() {
        let xml = format!(
            "<w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val=\"2\"/></w:tcPr><w:p><w:r><w:t>報酬月額</w:t></w:r></w:p></w:tc>{}</w:tr></w:tbl>",
            tc("全額")
        );
        assert_eq!(read(&xml), vec![vec![vec!["報酬月額 ", "", "全額 "]]]);
    }

    #[test]
    fn a_table_in_a_cell_stays_in_it() {
        let xml = format!(
            "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>外</w:t></w:r></w:p><w:tbl><w:tr>{}</w:tr></w:tbl></w:tc></w:tr></w:tbl>",
            tc("内")
        );
        let ts = read(&xml);
        assert_eq!(ts.len(), 1, "a nested table is not a fragment of its own");
        assert!(ts[0][0][0].contains('外') && ts[0][0][0].contains('内'), "{:?}", ts[0]);
    }

    #[test]
    fn the_namespace_prefix_is_not_part_of_the_name() {
        let xml = "<tbl><tr><tc><p><r><t>x</t></r></p></tc></tr></tbl>";
        assert_eq!(read(xml), vec![vec![vec!["x "]]]);
    }
}
