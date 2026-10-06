//! The definition of a thing a file holds, as a language whose blocks are lines under a line gives
//! it to the port `Items` (DESIGN 6.4): the lines it is written on, each without its comment and
//! the spaces around it, each run of spaces outside a string as one, and each line put under the
//! line above it by its depth, two spaces a level, however deep it was indented. So lining up the
//! colons of a record, indenting by four, or adding a comment changes nothing, and yuen's link to
//! the thing stays as it was. dandori gives its tasks, cases and records so (dandori's DESIGN 0.3),
//! and sekisho its blocks (sekisho's DESIGN 8.2). A `#` outside a string starts a comment, and a
//! string is in `"…"` with `\"` and `\\` its escapes, as both languages read them.

/// A line as a definition counts it: without its comment and the spaces around it, and each run of
/// spaces outside a string as one.
pub fn plain(line: &str) -> String {
    let mut out = String::new();
    let (mut quoted, mut space) = (false, false);
    let mut chars = line.trim().chars();
    while let Some(c) = chars.next() {
        if quoted {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => quoted = false,
                _ => {}
            }
            continue;
        }
        match c {
            '#' => break,
            ' ' => space = true,
            _ => {
                if std::mem::take(&mut space) {
                    out.push(' ');
                }
                quoted = c == '"';
                out.push(c);
            }
        }
    }
    out
}

/// The definition of what is written on the lines `from` to `to` (from 1, both in): each line made
/// [`plain`], and put under the line above it by its depth, two spaces a level, however deep it was
/// indented. A line that is blank or only a comment is left out.
pub fn block(lines: &[&str], from: usize, to: usize) -> String {
    let rows: Vec<(usize, String)> = (from..=to)
        .filter_map(|n| {
            let raw = lines.get(n.checked_sub(1)?)?;
            let p = plain(raw);
            (!p.is_empty()).then(|| (raw.len() - raw.trim_start_matches(' ').len(), p))
        })
        .collect();
    let mut widths: Vec<usize> = rows.iter().map(|(w, _)| *w).collect();
    widths.sort_unstable();
    widths.dedup();
    let depth = |w: usize| widths.iter().position(|x| *x == w).unwrap_or(0);
    rows.iter().map(|(w, p)| format!("{}{p}", "  ".repeat(depth(*w)))).collect::<Vec<_>>().join("\n")
}

/// The last line of the block that starts on line `at` (from 1): that line and the lines under it,
/// up to the next line that is indented no more than it; a line that is blank or only a comment
/// does not end the block, nor is it its last line.
pub fn block_end(lines: &[&str], at: usize) -> usize {
    let indent = |l: &str| l.len() - l.trim_start_matches(' ').len();
    let Some(first) = at.checked_sub(1).and_then(|i| lines.get(i)) else { return at };
    let mine = indent(first);
    let mut end = at;
    for (i, l) in lines.iter().enumerate().skip(at) {
        if plain(l).is_empty() {
            continue;
        }
        if indent(l) <= mine {
            break;
        }
        end = i + 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_plain_without_its_comment_and_its_alignment() {
        assert_eq!(plain("  sku      : string   # the stock unit"), "sku : string");
        assert_eq!(plain("agent \"a  # b\"  # c"), "agent \"a  # b\"");
        assert_eq!(plain("jev \"say \\\"hi\\\"  #1\""), "jev \"say \\\"hi\\\"  #1\"");
        assert_eq!(plain("   # only a comment"), "");
    }

    #[test]
    fn a_block_keeps_its_depth_however_it_was_indented() {
        let two = ["task t() -> k", "  jev \"q\"", "    a \"x\"", "  timeout 10 seconds"];
        let four = ["task t() -> k", "    jev \"q\"   # asked", "", "        a \"x\"", "    timeout 10 seconds"];
        assert_eq!(block(&two, 1, 4), "task t() -> k\n  jev \"q\"\n    a \"x\"\n  timeout 10 seconds");
        assert_eq!(block(&four, 1, 5), block(&two, 1, 4));
    }

    #[test]
    fn a_block_ends_before_the_next_line_as_shallow_as_its_first() {
        let lines = ["role clerk", "  description \"x\"", "", "  # a comment", "  can a, b", "", "role manager", "  includes clerk"];
        assert_eq!(block_end(&lines, 1), 5);
        assert_eq!(block_end(&lines, 7), 8);
        assert_eq!(block_end(&lines, 2), 2);
    }
}
