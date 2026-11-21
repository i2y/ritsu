//! A lowered file as it is written, a line at a time, each line with the line of the request it
//! comes from and what it is there (DESIGN 6.5, the map of lines): what a diagnostic of the lowered
//! file is said at.

use ritsu_base::text::Text;
use ritsu_ports::LineFrom;

/// A lowered file being written.
#[derive(Default)]
pub struct Out {
    pub text: String,
    pub lines: Vec<LineFrom>,
}

impl Out {
    /// A line, from the line `from` of the request (0 for none), which is `what` there.
    pub fn put(&mut self, line: impl AsRef<str>, from: usize, what: &Text) {
        self.text.push_str(line.as_ref().trim_end());
        self.text.push('\n');
        self.lines.push(LineFrom { line: from, what: what.clone() });
    }

    pub fn blank(&mut self) {
        self.put("", 0, &Text::default());
    }

    /// A table: the head (the inputs, then the outputs after `->`) and the rows, each with where it
    /// comes from, the columns lined up.
    pub fn table(&mut self, inputs: &[String], outputs: &[String], head: (usize, &Text), rows: &[(Vec<String>, usize, Text)]) {
        let mut heads: Vec<String> = inputs.to_vec();
        for (i, o) in outputs.iter().enumerate() {
            heads.push(if i == 0 { format!("-> {o}") } else { o.clone() });
        }
        let width = |c: usize| rows.iter().map(|r| r.0.get(c).map(|s| s.chars().count()).unwrap_or(0)).chain([heads[c].chars().count()]).max().unwrap_or(0);
        let widths: Vec<usize> = (0..heads.len()).map(width).collect();
        let line = |cells: &[String]| {
            let mut s = String::from("|");
            for (c, w) in widths.iter().enumerate() {
                let cell = cells.get(c).map(String::as_str).unwrap_or("");
                let pad = w - cell.chars().count();
                s.push_str(&format!(" {cell}{} |", " ".repeat(pad)));
            }
            s
        };
        self.put(line(&heads), head.0, head.1);
        for (cells, from, what) in rows {
            self.put(line(cells), *from, what);
        }
    }

    /// Where the line `n` (from 1) comes from.
    pub fn from_line(lines: &[LineFrom], n: usize) -> Option<&LineFrom> {
        n.checked_sub(1).and_then(|i| lines.get(i))
    }
}
