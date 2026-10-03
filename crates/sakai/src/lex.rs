//! Cutting a `.ctx` into lines of tokens (DESIGN 1.1, PLAN B.2).
//!
//! The language is written a line at a time, so the lexer keeps the lines: each has its
//! indentation, its tokens, and its characters without the comment. A word is everything up to
//! a blank, a `"`, a `#`, a `,` or a `->`; the parser tells a keyword, a name, a package and a
//! version apart by position, and reads a name of an artifact (DESIGN 2) from the characters of
//! the line, since such a name may hold a `,`.

use crate::diag::{self, Diag};

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Word(String),
    Str(String),
    Comma,
    Arrow,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    /// 1-based, in characters.
    pub col: usize,
    /// The column after the token.
    pub end: usize,
}

impl Token {
    pub fn word(&self) -> Option<&str> {
        match &self.tok {
            Tok::Word(w) => Some(w),
            _ => None,
        }
    }

    pub fn is(&self, w: &str) -> bool {
        self.word() == Some(w)
    }
}

#[derive(Clone, Debug)]
pub struct Line {
    /// 1-based.
    pub no: usize,
    /// Leading spaces.
    pub indent: usize,
    pub tokens: Vec<Token>,
    /// The characters of the line before the comment.
    pub chars: Vec<char>,
}

impl Line {
    /// The characters from column `from` to the end of the line (before the comment).
    pub fn rest_from(&self, from: usize) -> String {
        let a = from.saturating_sub(1).min(self.chars.len());
        self.chars[a..].iter().collect::<String>()
    }
}

/// Every line of the source that has a token, and what could not be read (E001, E005).
pub fn lex(file: &str, src: &str) -> (Vec<Line>, Vec<Diag>) {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut lines = Vec::new();
    let mut diags = Vec::new();
    for (i, text) in src.lines().enumerate() {
        let no = i + 1;
        let all: Vec<char> = text.chars().collect();
        let err = |code: &'static str, col: usize, msg: ritsu_base::text::Text| diag::at(code, file, no, col, msg).source(src);
        // Indentation is spaces. A tab is refused: how wide it is depends on the editor, and the
        // lines of a block are told apart by lining up.
        let mut indent = 0;
        while indent < all.len() && (all[indent] == ' ' || all[indent] == '\t') {
            if all[indent] == '\t' {
                diags.push(err("E005", indent + 1, tr!("字下げにタブがあります。字下げはスペースで書きます", "The indentation has a tab; indent with spaces")).note(tr!(
                    "タブの幅はエディタによって違うので、行がそろっているかを決められません。",
                    "How wide a tab is depends on the editor, so whether the lines line up cannot be told."
                )));
            }
            indent += 1;
        }
        // The comment: from a `#` outside a string to the end of the line.
        let mut end = all.len();
        let mut in_str = false;
        let mut k = 0;
        while k < all.len() {
            match all[k] {
                '\\' if in_str => k += 1,
                '"' => in_str = !in_str,
                '#' if !in_str => {
                    end = k;
                    break;
                }
                _ => {}
            }
            k += 1;
        }
        let cs: Vec<char> = all[..end].to_vec();
        let mut toks = Vec::new();
        let mut j = indent;
        let before = diags.len();
        while j < cs.len() {
            if diags[before..].iter().any(|d: &Diag| d.code != "E005") {
                break;
            }
            let c = cs[j];
            let col = j + 1;
            if c == ' ' || c == '\t' {
                j += 1;
                continue;
            }
            if c == '\u{3000}' {
                diags.push(err("E001", col, tr!("全角の空白があります。語と語のあいだは半角のスペースで区切ります", "There is a full-width space; separate words with ASCII spaces")));
                j += 1;
                continue;
            }
            if c == '"' {
                let mut s = String::new();
                let mut q = j + 1;
                let mut closed = false;
                while q < cs.len() {
                    match cs[q] {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' => match cs.get(q + 1) {
                            Some(e @ ('"' | '\\')) => {
                                s.push(*e);
                                q += 2;
                                continue;
                            }
                            other => {
                                let e = other.map(|c| c.to_string()).unwrap_or_default();
                                diags.push(err("E001", q + 1, tr!(
                                    "文字列の中のエスケープ `\\{e}` は使えません。使えるのは `\\\"` と `\\\\` だけです",
                                    "The escape `\\{e}` is not taken in a string; only `\\\"` and `\\\\` are"
                                )));
                                break;
                            }
                        },
                        ch => s.push(ch),
                    }
                    q += 1;
                }
                if diags.len() > before && diags[before..].iter().any(|d| d.code == "E001") {
                    break;
                }
                if !closed {
                    diags.push(err("E001", col, tr!("閉じていない文字列があります", "A string is not closed")).note(tr!(
                        "文字列は同じ行の `\"` で閉じます。",
                        "A string is closed with `\"` on the same line."
                    )));
                    break;
                }
                toks.push(Token { tok: Tok::Str(s), col, end: q + 2 });
                j = q + 1;
                continue;
            }
            if c == ',' {
                toks.push(Token { tok: Tok::Comma, col, end: col + 1 });
                j += 1;
                continue;
            }
            if c == '-' && cs.get(j + 1) == Some(&'>') {
                toks.push(Token { tok: Tok::Arrow, col, end: col + 2 });
                j += 2;
                continue;
            }
            let start = j;
            while j < cs.len() {
                let d = cs[j];
                if d == ' ' || d == '\t' || d == '"' || d == ',' || d == '\u{3000}' || (d == '-' && cs.get(j + 1) == Some(&'>')) {
                    break;
                }
                j += 1;
            }
            toks.push(Token { tok: Tok::Word(cs[start..j].iter().collect()), col: start + 1, end: j + 1 });
        }
        if !toks.is_empty() {
            lines.push(Line { no, indent, tokens: toks, chars: cs });
        }
    }
    (lines, diags)
}

/// A line and the lines indented under it.
#[derive(Clone, Debug)]
pub struct Node {
    /// The index of the line in what [`lex`] gave.
    pub line: usize,
    pub children: Vec<Node>,
}

/// The lines as a tree by their indentation (E005): every line under the nearest line above it
/// that is indented less, every line of one block indented alike.
pub fn tree(file: &str, src: &str, lines: &[Line]) -> (Vec<Node>, Vec<Diag>) {
    let mut diags = Vec::new();
    let mut top: Vec<Node> = Vec::new();
    // The path of open nodes, as (indent, the indices to reach it).
    let mut stack: Vec<(usize, Vec<usize>)> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        while stack.last().is_some_and(|(ind, _)| *ind >= l.indent) {
            stack.pop();
        }
        let node = Node { line: i, children: vec![] };
        match stack.last() {
            None => {
                if l.indent > 0 {
                    diags.push(
                        diag::at("E005", file, l.no, l.indent + 1, tr!("字下げした行の上に、それを受ける行がありません", "This line is indented, and no line above takes it"))
                            .source(src)
                            .note(tr!("節の最初の行は字下げしません。", "The first line of a section is not indented.")),
                    );
                    continue;
                }
                top.push(node);
                stack.push((l.indent, vec![top.len() - 1]));
            }
            Some((_, at)) => {
                let at = at.clone();
                let parent = walk_mut(&mut top, &at);
                if let Some(first) = parent.children.first() {
                    let want = lines[first.line].indent;
                    if want != l.indent {
                        diags.push(
                            diag::at("E005", file, l.no, l.indent + 1, tr!("字下げがそろっていません", "The indentation does not line up"))
                                .source(src)
                                .note(tr!(
                                    "同じ節の行は、同じ幅だけ字下げします。上の行の字下げは {want} 文字です。",
                                    "The lines of one block are indented alike; the line above is indented by {want}."
                                )),
                        );
                        continue;
                    }
                }
                parent.children.push(node);
                let mut path = at;
                path.push(parent.children.len() - 1);
                stack.push((l.indent, path));
            }
        }
    }
    (top, diags)
}

fn walk_mut<'a>(top: &'a mut [Node], at: &[usize]) -> &'a mut Node {
    let mut n = &mut top[at[0]];
    for &i in &at[1..] {
        n = &mut n.children[i];
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_counted_in_characters() {
        let (ls, ds) = lex("t.ctx", "context 在庫(inventory) v1\n  dir \"../a\", \"../b\" # c\n");
        assert!(ds.is_empty());
        assert_eq!(ls[0].tokens[1].col, 9);
        assert_eq!(ls[0].tokens[2].col, 23);
        assert_eq!(ls[1].tokens[1].tok, Tok::Str("../a".into()));
        assert_eq!(ls[1].tokens[2].tok, Tok::Comma);
        assert_eq!(ls[1].tokens.len(), 4);
    }

    #[test]
    fn an_arrow_cuts_a_word() {
        let (ls, _) = lex("t.ctx", "A->B\n");
        let ts: Vec<&Tok> = ls[0].tokens.iter().map(|t| &t.tok).collect();
        assert_eq!(ts, vec![&Tok::Word("A".into()), &Tok::Arrow, &Tok::Word("B".into())]);
    }

    #[test]
    fn a_tree_by_indentation() {
        let src = "a\n  b\n    c\n  d\ne\n";
        let (ls, _) = lex("t.ctx", src);
        let (t, ds) = tree("t.ctx", src, &ls);
        assert!(ds.is_empty());
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].children.len(), 2);
        assert_eq!(t[0].children[0].children.len(), 1);
        let bad = "a\n    b\n  c\n";
        let (ls, _) = lex("t.ctx", bad);
        let (_, ds) = tree("t.ctx", bad, &ls);
        assert_eq!(ds[0].code, "E005");
    }
}
