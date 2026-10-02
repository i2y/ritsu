//! Patterns (DESIGN §9): literals, `.`, classes (`[a-z]`, `[^0-9]`), `\d \w \s`
//! and their capitals, groups, `|`, `* + ?`, `{n}`, `{n,}`, `{n,m}`. There are no
//! backreferences and no lookaround, so a pattern compiles to a small program that
//! runs as a Pike VM: every place the program can be in is followed at once, one
//! character at a time, and the time is linear in the text whatever the pattern. A
//! pattern always matches the whole value, so it has no anchors.

use crate::diag::{t, Text};

/// The largest count a repetition may give: `{1000}`.
const MAX_COUNT: u32 = 1000;
/// The largest program a pattern may compile to, its counts spelled out.
const MAX_PROGRAM: usize = 100_000;

/// A pattern that parsed, ready to run.
#[derive(Debug, Clone)]
pub struct Pattern {
    pub source: String,
    prog: Vec<Inst>,
}

/// Why a pattern does not parse: the index (from 0) of the character in the
/// pattern where the problem is, and the message.
#[derive(Debug, Clone, PartialEq)]
pub struct Bad {
    pub at: usize,
    pub why: Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Short {
    Digit,
    Word,
    Space,
}

impl Short {
    fn has(self, c: char) -> bool {
        match self {
            Short::Digit => c.is_ascii_digit(),
            Short::Word => c.is_alphanumeric() || c == '_',
            Short::Space => c.is_whitespace(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
    Range(char, char),
    /// `\d`, `\w`, `\s`; negated for `\D`, `\W`, `\S`.
    Short(Short, bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Class {
    negated: bool,
    items: Vec<Item>,
}

impl Class {
    fn has(&self, c: char) -> bool {
        let inside = self.items.iter().any(|i| match i {
            Item::Range(lo, hi) => *lo <= c && c <= *hi,
            Item::Short(s, neg) => s.has(c) != *neg,
        });
        inside != self.negated
    }
}

#[derive(Debug, Clone)]
enum Node {
    Empty,
    Char(char),
    Any,
    Class(Class),
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Repeat { node: Box<Node>, min: u32, max: Option<u32> },
}

#[derive(Debug, Clone)]
enum Inst {
    Char(char),
    Any,
    Class(Class),
    Split(usize, usize),
    Jmp(usize),
    Match,
}

impl Pattern {
    /// Parses and compiles a pattern.
    pub fn new(source: &str) -> Result<Pattern, Bad> {
        let chars: Vec<char> = source.chars().collect();
        let mut p = Parser { c: &chars, i: 0 };
        let node = p.alt()?;
        if p.i < chars.len() {
            // only a `)` stops `alt` early
            return Err(bad(p.i, "a `)` that no `(` opens", "`(` で開いていない `)` があります"));
        }
        let mut prog = Vec::new();
        compile(&node, &mut prog);
        prog.push(Inst::Match);
        if prog.len() > MAX_PROGRAM {
            return Err(bad(
                0,
                "the pattern is too large once its counts are spelled out; use smaller counts",
                "回数を展開するとパターンが大きくなりすぎます。回数を小さくしてください",
            ));
        }
        Ok(Pattern { source: source.to_string(), prog })
    }

    /// The pattern in quotes, as a claims file writes it: a backslash bare before a
    /// character a string does not escape (`"\d+"`), doubled before one it does.
    pub fn shown(&self) -> String {
        let mut out = String::from("\"");
        let c: Vec<char> = self.source.chars().collect();
        for (i, &ch) in c.iter().enumerate() {
            match ch {
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                '\\' if matches!(c.get(i + 1), None | Some('n' | 't' | '"' | '\\')) => out.push_str("\\\\"),
                ch => out.push(ch),
            }
        }
        out.push('"');
        out
    }

    /// Whether the pattern matches the whole of `text`.
    pub fn matches(&self, text: &str) -> bool {
        let n = self.prog.len();
        let mut now = Threads::new(n);
        let mut next = Threads::new(n);
        self.add(&mut now, 0);
        for c in text.chars() {
            if now.list.is_empty() {
                return false;
            }
            next.clear();
            for k in 0..now.list.len() {
                let pc = now.list[k];
                let step = match &self.prog[pc] {
                    Inst::Char(x) => *x == c,
                    Inst::Any => true,
                    Inst::Class(cl) => cl.has(c),
                    _ => false,
                };
                if step {
                    self.add(&mut next, pc + 1);
                }
            }
            std::mem::swap(&mut now, &mut next);
        }
        now.list.iter().any(|&pc| matches!(self.prog[pc], Inst::Match))
    }

    /// Adds a place to a list, and every place it reaches without reading a
    /// character; each place at most once, so a loop that can match nothing ends.
    fn add(&self, list: &mut Threads, start: usize) {
        let mut stack = vec![start];
        while let Some(pc) = stack.pop() {
            if !list.insert(pc) {
                continue;
            }
            match self.prog[pc] {
                Inst::Jmp(to) => stack.push(to),
                Inst::Split(a, b) => {
                    stack.push(b);
                    stack.push(a);
                }
                _ => {}
            }
        }
    }
}

/// A set of places in the program, in the order they were added.
struct Threads {
    seen: Vec<bool>,
    list: Vec<usize>,
}

impl Threads {
    fn new(n: usize) -> Threads {
        Threads { seen: vec![false; n], list: Vec::new() }
    }

    fn insert(&mut self, pc: usize) -> bool {
        if self.seen[pc] {
            return false;
        }
        self.seen[pc] = true;
        self.list.push(pc);
        true
    }

    fn clear(&mut self) {
        for &pc in &self.list {
            self.seen[pc] = false;
        }
        self.list.clear();
    }
}

fn bad(at: usize, en: &str, ja: &str) -> Bad {
    Bad { at, why: t(en, ja) }
}

struct Parser<'a> {
    c: &'a [char],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn alt(&mut self) -> Result<Node, Bad> {
        let mut branches = vec![self.concat()?];
        while self.peek() == Some('|') {
            self.i += 1;
            branches.push(self.concat()?);
        }
        Ok(if branches.len() == 1 { branches.pop().expect("one") } else { Node::Alt(branches) })
    }

    fn concat(&mut self) -> Result<Node, Bad> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            items.push(self.quantified(atom)?);
        }
        Ok(match items.len() {
            0 => Node::Empty,
            1 => items.pop().expect("one"),
            _ => Node::Concat(items),
        })
    }

    /// An atom with the repetition after it, if any. A second repetition right after
    /// the first is refused: `a**` and `a+?` say nothing a group would not say.
    fn quantified(&mut self, atom: Node) -> Result<Node, Bad> {
        let Some((min, max)) = self.quantifier()? else {
            return Ok(atom);
        };
        let node = Node::Repeat { node: Box::new(atom), min, max };
        if let Some(q) = self.peek().filter(|q| matches!(q, '*' | '+' | '?' | '{')) {
            return Err(Bad {
                at: self.i,
                why: t(
                    format!("`{q}` follows another repetition; put the repeated part in a group, as `(a*){q}`"),
                    format!("`{q}` が別の繰り返しの直後にあります。繰り返す部分をグループにまとめてください（`(a*){q}` のように）"),
                ),
            });
        }
        Ok(node)
    }

    /// `*`, `+`, `?`, or a count in braces, as (min, max).
    fn quantifier(&mut self) -> Result<Option<(u32, Option<u32>)>, Bad> {
        let q = match self.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') => return self.count().map(Some),
            _ => return Ok(None),
        };
        self.i += 1;
        Ok(Some(q))
    }

    /// `{n}`, `{n,}` or `{n,m}`.
    fn count(&mut self) -> Result<(u32, Option<u32>), Bad> {
        let open = self.i;
        let not_a_count = || {
            bad(
                open,
                "`{` starts a count such as `{2}`, `{2,}` or `{2,5}`; write `\\{` for a brace",
                "`{` は `{2}`、`{2,}`、`{2,5}` のような回数を書き始める記号です。中かっこそのものは `\\{` と書きます",
            )
        };
        self.i += 1;
        let min = self.number().ok_or_else(not_a_count)?;
        let max = match self.peek() {
            Some('}') => Some(min),
            Some(',') => {
                self.i += 1;
                if self.peek() == Some('}') { None } else { Some(self.number().ok_or_else(not_a_count)?) }
            }
            _ => return Err(not_a_count()),
        };
        if self.peek() != Some('}') {
            return Err(not_a_count());
        }
        self.i += 1;
        let too_many = |n: u32| n > MAX_COUNT;
        if too_many(min) || max.is_some_and(too_many) {
            return Err(Bad {
                at: open,
                why: t(
                    format!("a count above {MAX_COUNT}"),
                    format!("{MAX_COUNT} を超える回数です"),
                ),
            });
        }
        if let Some(m) = max
            && m < min
        {
            return Err(Bad {
                at: open,
                why: t(
                    format!("the count `{{{min},{m}}}` runs backwards; the smaller number comes first"),
                    format!("回数 `{{{min},{m}}}` の大小が逆です。小さいほうを先に書きます"),
                ),
            });
        }
        Ok((min, max))
    }

    fn number(&mut self) -> Option<u32> {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        if self.i == start {
            return None;
        }
        // more digits than a count can hold reads as a count above the limit
        Some(self.c[start..self.i].iter().collect::<String>().parse().unwrap_or(u32::MAX))
    }

    fn atom(&mut self) -> Result<Node, Bad> {
        let at = self.i;
        let c = self.peek().expect("concat looked");
        self.i += 1;
        match c {
            '(' => {
                if self.peek() == Some('?') {
                    return Err(bad(
                        self.i,
                        "`(?` starts a lookaround or a flag in other pattern languages; geas's patterns have neither",
                        "`(?` はほかのパターンの書き方では先読みやフラグの始まりですが、geas のパターンにはどちらもありません",
                    ));
                }
                let inner = self.alt()?;
                if self.peek() != Some(')') {
                    return Err(bad(at, "a `(` that no `)` closes", "`)` で閉じていない `(` があります"));
                }
                self.i += 1;
                Ok(inner)
            }
            '[' => self.class(at),
            '.' => Ok(Node::Any),
            '\\' => self.escape(at),
            '*' | '+' | '?' => Err(Bad {
                at,
                why: t(
                    format!("`{c}` repeats nothing before it; write `\\{c}` for the character"),
                    format!("`{c}` の前に繰り返すものがありません。文字そのものは `\\{c}` と書きます"),
                ),
            }),
            '{' => Err(bad(
                at,
                "`{` repeats nothing before it; write `\\{` for a brace",
                "`{` の前に繰り返すものがありません。中かっこそのものは `\\{` と書きます",
            )),
            '^' | '$' => Err(Bad {
                at,
                why: t(
                    format!("a pattern always matches the whole value, so it takes no `{c}`; write `\\{c}` for the character"),
                    format!("パターンはいつも値の全体と照らし合わせるので、`{c}` は要りません。文字そのものは `\\{c}` と書きます"),
                ),
            }),
            c => Ok(Node::Char(c)),
        }
    }

    /// After a backslash, outside a class: a shorthand, a line break or a tab, or a
    /// character that is not a letter or a digit, taken as it is.
    fn escape(&mut self, at: usize) -> Result<Node, Bad> {
        match self.escaped(at)? {
            Esc::Char(c) => Ok(Node::Char(c)),
            Esc::Short(s, neg) => Ok(Node::Class(Class { negated: false, items: vec![Item::Short(s, neg)] })),
        }
    }

    fn escaped(&mut self, at: usize) -> Result<Esc, Bad> {
        let Some(e) = self.peek() else {
            return Err(bad(at, "the pattern ends in a backslash", "パターンがバックスラッシュで終わっています"));
        };
        self.i += 1;
        Ok(match e {
            'd' => Esc::Short(Short::Digit, false),
            'D' => Esc::Short(Short::Digit, true),
            'w' => Esc::Short(Short::Word, false),
            'W' => Esc::Short(Short::Word, true),
            's' => Esc::Short(Short::Space, false),
            'S' => Esc::Short(Short::Space, true),
            'n' => Esc::Char('\n'),
            't' => Esc::Char('\t'),
            'r' => Esc::Char('\r'),
            e if e.is_alphanumeric() => {
                return Err(Bad {
                    at,
                    why: t(
                        format!("`\\{e}` is not in geas's patterns; they take `\\d`, `\\w`, `\\s` and their capitals, `\\n`, `\\t`, `\\r`, and a backslash before a character that is not a letter or a digit"),
                        format!("`\\{e}` は geas のパターンにはありません。使えるのは `\\d`、`\\w`、`\\s` とその大文字、`\\n`、`\\t`、`\\r`、それに文字でも数字でもない文字の前のバックスラッシュです"),
                    ),
                });
            }
            e => Esc::Char(e),
        })
    }

    /// A class, after its `[`.
    fn class(&mut self, open: usize) -> Result<Node, Bad> {
        let unclosed = || bad(open, "a `[` that no `]` closes", "`]` で閉じていない `[` があります");
        let negated = self.peek() == Some('^');
        if negated {
            self.i += 1;
        }
        let mut items = Vec::new();
        loop {
            let at = self.i;
            let Some(c) = self.peek() else {
                return Err(unclosed());
            };
            self.i += 1;
            let lo = match c {
                ']' if items.is_empty() => {
                    return Err(bad(
                        at,
                        "a class names at least one character; write `\\]` for a bracket",
                        "文字クラスには少なくとも一つの文字を書きます。角かっこそのものは `\\]` と書きます",
                    ));
                }
                ']' => break,
                '\\' => match self.escaped(at)? {
                    Esc::Char(x) => x,
                    Esc::Short(s, neg) => {
                        items.push(Item::Short(s, neg));
                        continue;
                    }
                },
                x => x,
            };
            // a range, unless the `-` is the class's last character
            if self.peek() == Some('-') && self.c.get(self.i + 1).is_some_and(|&n| n != ']') {
                self.i += 1;
                let hi_at = self.i;
                let hi = match self.peek() {
                    Some('\\') => {
                        self.i += 1;
                        match self.escaped(hi_at)? {
                            Esc::Char(x) => x,
                            Esc::Short(..) => {
                                return Err(bad(
                                    hi_at,
                                    "a range ends in a character, not in a class such as `\\d`",
                                    "範囲の終わりには文字を書きます。`\\d` のような文字クラスは書けません",
                                ));
                            }
                        }
                    }
                    Some(x) => {
                        self.i += 1;
                        x
                    }
                    None => return Err(unclosed()),
                };
                if hi < lo {
                    return Err(Bad {
                        at,
                        why: t(
                            format!("the range `{lo}-{hi}` runs backwards; the smaller character comes first"),
                            format!("範囲 `{lo}-{hi}` の大小が逆です。小さいほうの文字を先に書きます"),
                        ),
                    });
                }
                items.push(Item::Range(lo, hi));
            } else {
                items.push(Item::Range(lo, lo));
            }
        }
        Ok(Node::Class(Class { negated, items }))
    }
}

enum Esc {
    Char(char),
    Short(Short, bool),
}

fn compile(node: &Node, prog: &mut Vec<Inst>) {
    // a pattern that will be refused as too large stops growing here
    if prog.len() > MAX_PROGRAM {
        return;
    }
    match node {
        Node::Empty => {}
        Node::Char(c) => prog.push(Inst::Char(*c)),
        Node::Any => prog.push(Inst::Any),
        Node::Class(cl) => prog.push(Inst::Class(cl.clone())),
        Node::Concat(items) => {
            for n in items {
                compile(n, prog);
            }
        }
        Node::Alt(branches) => {
            // split to each branch in turn, every branch jumping to the end
            let mut jumps = Vec::new();
            for (k, b) in branches.iter().enumerate() {
                if k + 1 < branches.len() {
                    let split = prog.len();
                    prog.push(Inst::Split(split + 1, 0));
                    compile(b, prog);
                    jumps.push(prog.len());
                    prog.push(Inst::Jmp(0));
                    let next = prog.len();
                    prog[split] = Inst::Split(split + 1, next);
                } else {
                    compile(b, prog);
                }
            }
            let end = prog.len();
            for j in jumps {
                prog[j] = Inst::Jmp(end);
            }
        }
        Node::Repeat { node, min, max } => {
            for _ in 0..*min {
                compile(node, prog);
            }
            match max {
                None => {
                    // e*: L: split(body, out); body; jmp L
                    let l = prog.len();
                    prog.push(Inst::Split(l + 1, 0));
                    compile(node, prog);
                    prog.push(Inst::Jmp(l));
                    let out = prog.len();
                    prog[l] = Inst::Split(l + 1, out);
                }
                Some(m) => {
                    // each optional copy may stop the repetition: split(body, out)
                    let mut splits = Vec::new();
                    for _ in *min..*m {
                        splits.push(prog.len());
                        prog.push(Inst::Split(0, 0));
                        compile(node, prog);
                    }
                    let out = prog.len();
                    for s in splits {
                        prog[s] = Inst::Split(s + 1, out);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(p: &str, text: &str) -> bool {
        Pattern::new(p).unwrap_or_else(|e| panic!("{p:?}: {e:?}")).matches(text)
    }

    #[test]
    fn literals_and_the_whole_value() {
        assert!(m("abc", "abc"));
        assert!(!m("abc", "abcd"));
        assert!(!m("abc", "xabc"));
        assert!(m("", ""));
        assert!(!m("", "a"));
        assert!(m("a.c", "a\nc"), "`.` is any character, a line break too");
        assert!(m("主張", "主張"));
    }

    #[test]
    fn classes_and_shorthands() {
        assert!(m("[a-z]+", "geas"));
        assert!(!m("[a-z]+", "Geas"));
        assert!(m("[^0-9]*", "abc"));
        assert!(!m("[^0-9]*", "a1"));
        assert!(m("\\d{4}-\\d{2}-\\d{2}", "2026-10-03"));
        assert!(!m("\\d+", "１２"), "\\d is 0-9 only");
        assert!(m("\\w+", "主張_1"));
        assert!(m("\\s*x\\s*", " \tx\n"));
        assert!(m("\\D\\W\\S", "a-b"));
        assert!(m("[\\d_]+", "1_2"));
        assert!(m("[-a]+", "-a-"));
        assert!(m("[a-]+", "a-"));
        assert!(m("[\\]]", "]"));
        assert!(m("\\.\\*\\(\\)\\[\\{\\^\\$\\\\", ".*()[{^$\\"));
    }

    #[test]
    fn groups_alternatives_and_repetition() {
        assert!(m("(ab|cd)+", "abcdab"));
        assert!(!m("(ab|cd)+", ""));
        assert!(m("colou?r", "color") && m("colou?r", "colour"));
        assert!(m("a{3}", "aaa") && !m("a{3}", "aa") && !m("a{3}", "aaaa"));
        assert!(m("a{2,}", "aaaaa") && !m("a{2,}", "a"));
        assert!(m("a{2,3}", "aa") && m("a{2,3}", "aaa") && !m("a{2,3}", "aaaa"));
        assert!(m("(a*)*b", "aab"));
        assert!(m("(|a)b", "b") && m("(|a)b", "ab"));
        assert!(m("ok|", "") && m("ok|", "ok"));
        assert!(m("HTTP/1\\.[01] \\d{3} .*", "HTTP/1.1 200 OK"));
    }

    #[test]
    fn a_pattern_that_backtracking_would_take_forever_on() {
        let text = "a".repeat(30_000);
        let start = std::time::Instant::now();
        assert!(!m("(a|aa)*b", &text));
        assert!(m("(a|aa)*", &text));
        assert!(!m("(a*)*b", &text));
        assert!(start.elapsed() < std::time::Duration::from_secs(1), "{:?}", start.elapsed());
    }

    fn err(p: &str) -> (usize, String) {
        let e = Pattern::new(p).expect_err(p);
        (e.at, e.why.en)
    }

    #[test]
    fn patterns_that_do_not_parse() {
        assert_eq!(err("(ab").0, 0);
        assert_eq!(err("ab)").0, 2);
        assert_eq!(err("[ab").0, 0);
        assert_eq!(err("a[]").0, 2);
        assert_eq!(err("*a").0, 0);
        assert_eq!(err("a**").0, 2);
        assert_eq!(err("a+?").0, 2);
        assert_eq!(err("a{x}").0, 1);
        assert_eq!(err("a{2").0, 1);
        assert_eq!(err("a{3,2}"), (1, "the count `{3,2}` runs backwards; the smaller number comes first".into()));
        assert_eq!(err("a{1001}"), (1, "a count above 1000".into()));
        assert_eq!(err("^ab$").0, 0);
        assert_eq!(err("ab\\").0, 2);
        assert_eq!(err("a\\bc").0, 1);
        assert_eq!(err("(?=a)").0, 1);
        assert_eq!(err("[z-a]").0, 1);
        assert_eq!(err("[a-\\d]").0, 3);
        assert!(err("(a{1000}){1000}").1.contains("too large"));
    }

    #[test]
    fn shown_as_a_claims_file_writes_it() {
        let shown = |p: &str| Pattern::new(p).unwrap().shown();
        assert_eq!(shown("\\d{4}-\\d{2}"), "\"\\d{4}-\\d{2}\"");
        assert_eq!(shown("a\\.b"), "\"a\\.b\"");
        assert_eq!(shown("\\\\"), "\"\\\\\\\\\"");
        assert_eq!(shown("say \"hi\"\n"), "\"say \\\"hi\\\"\\n\"");
        assert_eq!(shown("\\n"), "\"\\\\n\"");
    }

    #[test]
    fn messages_in_both_languages() {
        let e = Pattern::new("(ab").unwrap_err();
        assert_eq!(e.why.ja, "`)` で閉じていない `(` があります");
    }
}
