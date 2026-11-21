//! A `.ringi` from its lines of tokens to its syntax tree (DESIGN 3.3).
//!
//! A line at the left margin starts a block; the lines indented under it, and the rows of a table
//! (`| … |`, at any indentation), are its body. Each block reads its own lines, a line at a time.
//! What does not read is E001, one a line, with what the line was expected to hold; the rest of the
//! file goes on being read, so one mistake does not hide the next.

use crate::ast::*;
use crate::diag::Diag;
use crate::kw;
use crate::lex::{Line, Tok, Token, lex};
use ritsu_base::text::Text;

/// What the parser came to: the file, when every line reads, and what did not.
pub struct Parsed {
    pub file: Option<File>,
    pub diags: Vec<Diag>,
}

pub fn parse(path: &str, src: &str) -> Parsed {
    let (lines, lexed) = lex(path, src);
    let mut p = P { path, src, diags: Vec::new() };
    let file = p.file(&lines);
    // a line the lexer stopped on says that alone: what the parser makes of its tokens is noise
    let bad: Vec<Option<usize>> = lexed.iter().map(|d| d.line).collect();
    let mut diags = lexed;
    diags.extend(p.diags.into_iter().filter(|d| !bad.contains(&d.line)));
    let ok = diags.iter().all(|d| !d.is_error());
    Parsed { file: if ok { file } else { None }, diags }
}

/// The values of a form as `simulate --form` takes them, written as a scenario's `form` line writes
/// them (DESIGN 10.2): `category = purchase, items = [{what: "Server", price: 1_200_000JPY}]`. What
/// does not read is said as the parser says it, at the column of the text given (`file` names it).
pub fn form_values(file: &str, src: &str) -> Result<Vec<(Word, Expr)>, Vec<Diag>> {
    let one_line = src.replace('\n', " ");
    let (lines, lexed) = lex(file, &one_line);
    if lexed.iter().any(|d| d.is_error()) {
        return Err(lexed);
    }
    let mut p = P { path: file, src: &one_line, diags: Vec::new() };
    let mut out = Vec::new();
    for l in &lines {
        let mut c = Cur::new(l);
        let read = (|| -> R<()> {
            loop {
                let n = p.ident(&mut c, tr!("フィールドの名前", "a field's name"))?;
                p.tok(&mut c, Tok::Eq)?;
                let v = p.expr(&mut c)?;
                out.push((n, v));
                if !c.eat(&Tok::Comma) {
                    break;
                }
            }
            p.end(&c)
        })();
        if read.is_err() && p.diags.is_empty() {
            p.err(c.here(), tr!("フォームの値として読めません", "This does not read as the values of a form"));
        }
    }
    if p.diags.iter().any(|d| d.is_error()) {
        return Err(p.diags);
    }
    Ok(out)
}

type R<T> = Result<T, ()>;

struct P<'a> {
    path: &'a str,
    src: &'a str,
    diags: Vec<Diag>,
}

/// The tokens of one line, read from the left.
struct Cur<'a> {
    t: &'a [Token],
    i: usize,
    line: usize,
}

impl<'a> Cur<'a> {
    fn new(l: &'a Line) -> Cur<'a> {
        Cur { t: &l.tokens, i: 0, line: l.no }
    }

    fn of(t: &'a [Token], line: usize) -> Cur<'a> {
        Cur { t, i: 0, line }
    }

    fn peek(&self) -> Option<&'a Token> {
        self.t.get(self.i)
    }

    fn peek_at(&self, k: usize) -> Option<&'a Token> {
        self.t.get(self.i + k)
    }

    fn next(&mut self) -> Option<&'a Token> {
        let t = self.t.get(self.i);
        if t.is_some() {
            self.i += 1;
        }
        t
    }

    fn done(&self) -> bool {
        self.i >= self.t.len()
    }

    fn is_word(&self, w: &str) -> bool {
        self.peek().is_some_and(|t| t.tok.is_word(w))
    }

    fn is_word_at(&self, k: usize, w: &str) -> bool {
        self.peek_at(k).is_some_and(|t| t.tok.is_word(w))
    }

    fn eat_word(&mut self, w: &str) -> bool {
        if self.is_word(w) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn is(&self, t: &Tok) -> bool {
        self.peek().is_some_and(|x| &x.tok == t)
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.is(t) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    /// Where the next token is, or the end of the line.
    fn here(&self) -> Span {
        match self.peek() {
            Some(t) => t.span,
            None => match self.t.last() {
                Some(t) => Span { line: t.span.line, col: t.span.end, end: t.span.end },
                None => Span { line: self.line, col: 1, end: 1 },
            },
        }
    }

    /// The span from where `from` starts to the token before the next.
    fn since(&self, from: Span) -> Span {
        let end = if self.i > 0 { self.t[self.i - 1].span.end } else { from.end };
        Span { line: from.line, col: from.col, end }
    }
}

/// A block: its first line at the left margin, and the lines under it.
struct Block<'a> {
    head: &'a Line,
    body: Vec<&'a Line>,
}

/// The rank of a block in a request's order (DESIGN 3.1), and whether it may be written more than
/// once.
fn request_rank(word: &str) -> Option<(u8, bool)> {
    Some(match word {
        "description" | "owner" | "effective from" => (1, false),
        "use" => (2, true),
        "form" => (3, false),
        "route" => (4, false),
        "who" => (5, false),
        "every stage" => (6, false),
        "table" => (7, true),
        "ticket" => (8, false),
        "claims" => (9, false),
        "examples" => (10, false),
        "scenario" => (11, true),
        "labels" => (12, true),
        _ => return None,
    })
}

/// The same, for a shared route's file (DESIGN 5.11).
fn route_rank(word: &str) -> Option<(u8, bool)> {
    Some(match word {
        "description" | "effective from" => (1, false),
        "use" => (2, true),
        "inputs" => (3, false),
        "route" => (4, false),
        "who" => (5, false),
        "every stage" => (6, false),
        "table" => (7, true),
        "labels" => (12, true),
        _ => return None,
    })
}

fn organization_rank(word: &str) -> Option<(u8, bool)> {
    Some(match word {
        "description" => (1, false),
        "positions" => (2, false),
        "team kinds" => (3, false),
        "data" => (4, false),
        _ => return None,
    })
}

fn masters_rank(word: &str) -> Option<(u8, bool)> {
    Some(match word {
        "description" => (1, false),
        "use" => (2, true),
        "master" => (3, true),
        _ => return None,
    })
}

/// The words a block can start with, as one word for the multi-word ones.
const BLOCK_WORDS: [&str; 22] = [
    "description",
    "owner",
    "effective from",
    "use",
    "form",
    "inputs",
    "route",
    "who",
    "every stage",
    "table",
    "ticket",
    "claims",
    "examples",
    "scenario",
    "labels",
    "positions",
    "team kinds",
    "data",
    "master",
    "request",
    "organization",
    "masters",
];

impl<'a> P<'a> {
    fn err(&mut self, at: Span, msg: Text) {
        self.diags.push(Diag::error("E001", self.path, at.line, at.col, msg).source(self.src));
    }

    fn err_note(&mut self, at: Span, msg: Text, note: Text) {
        self.diags.push(Diag::error("E001", self.path, at.line, at.col, msg).source(self.src).note(note));
    }

    fn at_code(&mut self, code: &'static str, at: Span, msg: Text, note: Option<Text>) {
        let mut d = Diag::error(code, self.path, at.line, at.col, msg).source(self.src);
        if let Some(n) = note {
            d = d.note(n);
        }
        self.diags.push(d);
    }

    /// E001: what is there is not what the line takes there.
    fn expected<T>(&mut self, c: &Cur, what: Text) -> R<T> {
        let at = c.here();
        let msg = match c.peek() {
            Some(t) => {
                let found = t.tok.shown();
                if t.tok.is_word(kw::STEP) {
                    self.err_note(at, tr!("`step` ではなく `stage` と書いてください", "Write `stage`, not `step`"), step_note());
                    return Err(());
                }
                tr!("{}を書くところに `{found}` があります", "Expected {}, found `{found}`", before_particle(&what.ja); what.en)
            }
            None => tr!("行が、{}の前で終わっています", "The line ends where {} is expected", before_particle(&what.ja); what.en),
        };
        self.err(at, msg);
        Err(())
    }

    fn word(&mut self, c: &mut Cur, w: &str) -> R<Span> {
        let at = c.here();
        if c.eat_word(w) {
            Ok(at)
        } else {
            self.expected(c, tr!("`{w}`", "`{w}`"))
        }
    }

    fn tok(&mut self, c: &mut Cur, t: Tok) -> R<Span> {
        let at = c.here();
        if c.eat(&t) {
            Ok(at)
        } else {
            let s = t.shown();
            self.expected(c, tr!("`{s}`", "`{s}`"))
        }
    }

    /// The end of a line: anything left is E001.
    fn end(&mut self, c: &Cur) -> R<()> {
        match c.peek() {
            None => Ok(()),
            Some(t) if t.tok == Tok::Eq => {
                let at = c.here();
                self.err(at, tr!("ここに `=` は書けません。等しいかは `is` で比べてください", "A `=` cannot stand here: compare with `is`"));
                Err(())
            }
            Some(t) if t.tok.is_word(kw::STEP) => self.expected(c, tr!("行の終わり", "the end of the line")),
            Some(t) => {
                let found = t.tok.shown();
                let at = c.here();
                self.err(at, tr!("`{found}` から先を読めません。この行はその前で終わります", "Nothing can follow here: `{found}` is left over"));
                Err(())
            }
        }
    }

    /// A word that is a name or a reference: any word.
    fn ident(&mut self, c: &mut Cur, what: Text) -> R<Word> {
        match c.peek() {
            Some(Token { tok: Tok::Word(w), span }) => {
                c.next();
                Ok(Word { text: w.clone(), span: *span })
            }
            _ => self.expected(c, what),
        }
    }

    /// A name as it is declared: a word, and its ASCII name in brackets right after it
    /// (`種別(category)`).
    fn name(&mut self, c: &mut Cur, what: Text) -> R<Name> {
        let w = self.ident(c, what)?;
        let mut alias = None;
        if let Some(Token { tok: Tok::LParen, span }) = c.peek()
            && span.col == w.span.end
        {
            c.next();
            let a = match c.peek() {
                Some(Token { tok: Tok::Word(a), span }) => Word { text: a.clone(), span: *span },
                _ => return self.expected(c, tr!("括弧の中の ASCII の名前", "the ASCII name in the brackets")),
            };
            c.next();
            self.tok(c, Tok::RParen)?;
            alias = Some(a);
        }
        Ok(Name { text: w.text, span: w.span, alias })
    }

    /// A whole number, with no unit (`3` business days, `at most 2 times`).
    fn int(&mut self, c: &mut Cur, what: Text) -> R<(u32, Span)> {
        match c.peek() {
            Some(Token { tok: Tok::Num(n), span }) if n.unit.is_empty() && n.value.is_int() && n.value.num >= 0 && n.value.num <= u32::MAX as i128 => {
                c.next();
                Ok((n.value.num as u32, *span))
            }
            _ => self.expected(c, what),
        }
    }

    fn string(&mut self, c: &mut Cur, what: Text) -> R<(String, Span)> {
        match c.peek() {
            Some(Token { tok: Tok::Str(s), span }) => {
                c.next();
                Ok((s.clone(), *span))
            }
            _ => self.expected(c, what),
        }
    }

    fn opt_string(&mut self, c: &mut Cur) -> Option<String> {
        match c.peek() {
            Some(Token { tok: Tok::Str(s), .. }) => {
                c.next();
                Some(s.clone())
            }
            _ => None,
        }
    }

    // ── The file and its blocks ──

    fn file(&mut self, lines: &'a [Line]) -> Option<File> {
        let mut blocks: Vec<Block> = Vec::new();
        for l in lines {
            let is_row = l.tokens.first().is_some_and(|t| t.tok == Tok::Pipe);
            if l.indent == 0 && !is_row {
                blocks.push(Block { head: l, body: Vec::new() });
            } else if let Some(b) = blocks.last_mut() {
                b.body.push(l);
            } else {
                let at = Span { line: l.no, col: l.indent + 1, end: l.indent + 2 };
                self.err_note(
                    at,
                    tr!("ファイルの最初の行は、字下げせずに、ファイルの種類を書きます", "The first line of a file says the kind of file, at the left margin"),
                    first_line_note(),
                );
                return None;
            }
        }
        let Some(first) = blocks.first() else {
            self.diags.push(Diag::error("E001", self.path, 1, 1, tr!("ファイルが空です", "The file is empty")).note(first_line_note()));
            return None;
        };
        let mut f = self.head(first)?;
        let rank: fn(&str) -> Option<(u8, bool)> = match f.kind {
            FileKind::Request => request_rank,
            FileKind::Route => route_rank,
            FileKind::Organization => organization_rank,
            FileKind::Masters => masters_rank,
        };
        let mut highest = 0u8;
        let mut seen: Vec<&'static str> = Vec::new();
        for b in &blocks[1..] {
            let Some(word) = self.block_word(b.head) else { continue };
            let at = b.head.tokens[0].span;
            let Some((r, many)) = rank(word) else {
                let (kind, ja) = (f.kind.word(), f.kind.ja());
                self.at_code(
                    "E006",
                    at,
                    tr!("{ja}のファイル（`{kind}`）には `{word}` を書けません", "A file of the kind `{kind}` does not take `{word}`"),
                    Some(blocks_note(f.kind)),
                );
                continue;
            };
            if !many && seen.contains(&word) {
                self.at_code("E006", at, tr!("`{word}` は一つのファイルに一度だけ書けます", "A file takes `{word}` once"), None);
                continue;
            }
            if r < highest {
                self.err_note(at, tr!("`{word}` がほかのブロックの順から外れています", "`{word}` is out of the order of the blocks"), blocks_note(f.kind));
                // the block is there, out of its place: it is not missing as well
                seen.push(word);
                continue;
            }
            highest = r;
            seen.push(word);
            f.blocks.push((word, b.head.no));
            let _ = self.block(&mut f, word, b);
        }
        self.required(&f, &seen);
        Some(f)
    }

    /// The word a block's first line starts with: one of [`BLOCK_WORDS`], with `every stage`, `team
    /// kinds` and `effective from` as one. None, said, for any other.
    fn block_word(&mut self, l: &Line) -> Option<&'static str> {
        let c = Cur::new(l);
        let first = match c.peek() {
            Some(Token { tok: Tok::Word(w), .. }) => w.clone(),
            _ => {
                let _ = self.expected::<()>(&c, tr!("ブロックの最初の語", "the first word of a block"));
                return None;
            }
        };
        let two = |second: &str| c.is_word_at(1, second);
        let word = match first.as_str() {
            "every" if two(kw::STAGE) => "every stage",
            "every" if two(kw::STEP) => {
                let at = c.peek_at(1).unwrap().span;
                self.err_note(at, tr!("`step` ではなく `stage` と書いてください", "Write `stage`, not `step`"), step_note());
                return None;
            }
            "team" if two(kw::KINDS) => "team kinds",
            "effective" if two(kw::FROM) => "effective from",
            "step" => {
                let at = c.here();
                self.err_note(at, tr!("`step` ではなく `stage` と書いてください", "Write `stage`, not `step`"), step_note());
                return None;
            }
            w => match BLOCK_WORDS.iter().find(|b| **b == w) {
                Some(b) => b,
                None => {
                    let at = c.here();
                    self.err_note(at, tr!("`{w}` で始まるブロックはありません", "No block starts with `{w}`"), tr!(
                        "字下げしない行は、ブロックの最初の行です。ブロックの中の行は、字下げして書いてください。",
                        "A line at the left margin starts a block; write the lines of a block indented under it."
                    ));
                    return None;
                }
            },
        };
        Some(word)
    }

    /// The blocks a file of its kind cannot do without (E001 at its first line).
    fn required(&mut self, f: &File, seen: &[&str]) {
        let needs: &[&str] = match f.kind {
            FileKind::Request => &["form", "route", "ticket"],
            FileKind::Route => &["route", "who"],
            FileKind::Organization => &["positions", "team kinds"],
            FileKind::Masters => &["master"],
        };
        for n in needs {
            if !seen.contains(n) {
                let (kind, ja) = (f.kind.word(), f.kind.ja());
                self.diags.push(
                    Diag::error("E001", self.path, f.name.span.line, 1, tr!("{ja}のファイル（`{kind}`）に `{n}` がありません", "A file of the kind `{kind}` needs `{n}`"))
                        .source(self.src)
                        .note(blocks_note(f.kind)),
                );
            }
        }
    }

    /// The first line: the kind of the file, its name, its version and its title.
    fn head(&mut self, b: &Block) -> Option<File> {
        let mut c = Cur::new(b.head);
        let kind = match c.peek() {
            Some(t) if t.tok.is_word(kw::REQUEST) => FileKind::Request,
            Some(t) if t.tok.is_word(kw::ROUTE) => FileKind::Route,
            Some(t) if t.tok.is_word(kw::ORGANIZATION) => FileKind::Organization,
            Some(t) if t.tok.is_word(kw::MASTERS) => FileKind::Masters,
            _ => {
                let at = c.here();
                self.err_note(at, tr!("ファイルの最初の行が、ファイルの種類を言っていません", "The first line does not say the kind of file"), first_line_note());
                return None;
            }
        };
        c.next();
        let name = self.name(&mut c, tr!("ファイルの名前", "the file's name")).ok()?;
        let version = match c.peek() {
            Some(Token { tok: Tok::Word(v), .. }) if v.len() > 1 && v.starts_with('v') && v[1..].chars().all(|d| d.is_ascii_digit()) => {
                let n = v[1..].parse::<u32>().ok();
                c.next();
                n
            }
            _ => None,
        };
        let Some(version) = version else {
            let _ = self.expected::<()>(&c, tr!("版（`v1`）", "the version (`v1`)"));
            return None;
        };
        let title = self.opt_string(&mut c);
        if title.is_none() && kind != FileKind::Masters {
            let _ = self.expected::<()>(&c, tr!("題（`\"Expense request\"`）", "the title (`\"Expense request\"`)"));
            return None;
        }
        self.end(&c).ok()?;
        if !b.body.is_empty() {
            let at = Span { line: b.body[0].no, col: b.body[0].indent + 1, end: b.body[0].indent + 2 };
            self.err(at, tr!("ファイルの最初の行の下には、字下げした行を書けません", "No line is indented under the first line of a file"));
        }
        Some(File {
            kind,
            name,
            version,
            title,
            description: None,
            owner: None,
            effective_from: None,
            uses: Vec::new(),
            form: Vec::new(),
            inputs: Vec::new(),
            route: None,
            who: Vec::new(),
            every_stage: None,
            tables: Vec::new(),
            ticket: None,
            claims: Vec::new(),
            examples: None,
            scenarios: Vec::new(),
            labels: Vec::new(),
            positions: Vec::new(),
            team_kinds: Vec::new(),
            data: None,
            masters: Vec::new(),
            blocks: vec![(kind.word(), b.head.no)],
        })
    }

    fn block(&mut self, f: &mut File, word: &str, b: &Block) -> R<()> {
        let mut c = Cur::new(b.head);
        match word {
            "description" => {
                c.next();
                let s = self.string(&mut c, tr!("説明の文字列", "the description, a string"))?;
                self.end(&c)?;
                self.no_body(b);
                f.description = Some(s);
            }
            "owner" => {
                c.next();
                let w = self.ident(&mut c, tr!("受け持つチーム", "the team that keeps it"))?;
                self.end(&c)?;
                self.no_body(b);
                f.owner = Some(w);
            }
            "effective from" => {
                c.next();
                c.next();
                let at = c.here();
                let d = match c.peek() {
                    Some(Token { tok: Tok::Date(d), .. }) => *d,
                    _ => return self.expected(&c, tr!("効き始める日（`2027-04-01`）", "the day it takes effect (`2027-04-01`)")),
                };
                c.next();
                self.end(&c)?;
                self.no_body(b);
                f.effective_from = Some((d, at));
            }
            "use" => {
                let u = self.use_line(&mut c, b.head.no)?;
                self.no_body(b);
                f.uses.push(u);
            }
            "form" => {
                c.next();
                self.end(&c)?;
                f.form = self.fields_block(b);
            }
            "inputs" => {
                c.next();
                self.end(&c)?;
                f.inputs = self.fields_block(b);
            }
            "route" => {
                c.next();
                f.route = Some(self.route(&mut c, b)?);
            }
            "who" => {
                c.next();
                self.end(&c)?;
                f.who = self.who_block(b);
            }
            "every stage" => {
                c.next();
                c.next();
                self.end(&c)?;
                f.every_stage = Some(self.deadlines(&b.body, b.head.no));
            }
            "table" => {
                c.next();
                let name = self.name(&mut c, tr!("表の名前", "the table's name"))?;
                self.end(&c)?;
                let t = self.named_table(name, b)?;
                f.tables.push(t);
            }
            "ticket" => {
                c.next();
                self.end(&c)?;
                f.ticket = Some(self.ticket(b));
            }
            "claims" => {
                c.next();
                self.end(&c)?;
                f.claims = self.claims(b);
            }
            "examples" => {
                c.next();
                self.end(&c)?;
                f.examples = Some(self.examples(b)?);
            }
            "scenario" => {
                c.next();
                let name = self.ident(&mut c, tr!("シナリオの名前", "the scenario's name"))?;
                self.end(&c)?;
                let s = self.scenario(name, b)?;
                f.scenarios.push(s);
            }
            "labels" => {
                c.next();
                let lang = self.ident(&mut c, tr!("言語（`ja` か `en`）", "the language (`ja` or `en`)"))?;
                if !kw::LANGS.contains(&lang.text.as_str()) {
                    let w = lang.text.clone();
                    self.err(lang.span, tr!("`labels` に書ける言語は `ja` と `en` で、`{w}` はありません", "`labels` takes `ja` or `en`, not `{w}`"));
                    return Err(());
                }
                self.end(&c)?;
                let l = self.labels(lang, b);
                f.labels.push(l);
            }
            "positions" => {
                c.next();
                self.end(&c)?;
                f.positions = self.titled_lines(&b.body, tr!("役職の名前", "a position's name"));
            }
            "team kinds" => {
                c.next();
                c.next();
                self.end(&c)?;
                f.team_kinds = self.titled_lines(&b.body, tr!("チームの種類の名前", "a kind of team's name"));
            }
            "data" => {
                c.next();
                let s = self.string(&mut c, tr!("データの置き場（`\"../org\"`）", "where the data is (`\"../org\"`)"))?;
                self.end(&c)?;
                self.no_body(b);
                f.data = Some(s);
            }
            "master" => {
                c.next();
                let m = self.master(&mut c, b)?;
                f.masters.push(m);
            }
            _ => {
                // the first line of another file: a file has one first line
                let at = b.head.tokens[0].span;
                self.at_code("E006", at, tr!("ファイルの種類を言う行は、最初の一行だけです", "Only the first line says the kind of file"), None);
            }
        }
        Ok(())
    }

    /// A block whose first line is all of it: a line under it is E001.
    fn no_body(&mut self, b: &Block) {
        if let Some(l) = b.body.first() {
            let at = Span { line: l.no, col: l.indent + 1, end: l.indent + 2 };
            let first = b.head.tokens.first().map(|t| t.tok.shown()).unwrap_or_default();
            self.err(at, tr!("`{first}` の行の下には、字下げした行を書けません", "No line is indented under a `{first}` line"));
        }
    }

    fn use_line(&mut self, c: &mut Cur, line: usize) -> R<Use> {
        c.next();
        let at = c.here();
        let kind_word = self.ident(c, tr!("読むものの種類（`calendar`、`book`、`flow` など）", "what it reads (`calendar`, `book`, `flow`, …)"))?;
        let Some(kind) = UseKind::of_word(&kind_word.text) else {
            let w = kind_word.text.clone();
            let kinds = UseKind::ALL.iter().map(|k| format!("`{}`", k.word())).collect::<Vec<_>>().join(" ");
            self.err(at, tr!("`use {w}` は読めません。`use` のあとには {kinds} のどれかを書きます", "`use {w}` does not read: `use` takes one of {kinds}"));
            return Err(());
        };
        let alias = if kind == UseKind::Gate {
            None
        } else {
            let n = self.name(c, tr!("別名", "the alias"))?;
            self.word(c, kw::FROM)?;
            Some(n)
        };
        let (path, path_span) = self.string(c, tr!("ファイルのパス（文字列）", "the file's path, a string"))?;
        self.end(c)?;
        Ok(Use { kind, alias, path, path_span, line })
    }

    // ── Fields ──

    /// The fields of a block, each at the block's indentation, with the lines of each under it.
    fn fields_block(&mut self, b: &Block) -> Vec<Field> {
        self.fields(&b.body)
    }

    fn fields(&mut self, lines: &[&Line]) -> Vec<Field> {
        let mut out = Vec::new();
        let Some(base) = lines.first().map(|l| l.indent) else { return out };
        let mut i = 0;
        while i < lines.len() {
            let l = lines[i];
            let mut j = i + 1;
            while j < lines.len() && lines[j].indent > base {
                j += 1;
            }
            if l.indent != base {
                let at = Span { line: l.no, col: l.indent + 1, end: l.indent + 2 };
                self.err(at, tr!("字下げが、上の行とそろっていません", "The indentation does not line up with the lines above"));
            } else if let Ok(f) = self.field(l, &lines[i + 1..j]) {
                out.push(f);
            }
            i = j;
        }
        out
    }

    fn field(&mut self, l: &Line, under: &[&Line]) -> R<Field> {
        let mut c = Cur::new(l);
        if c.peek().is_some_and(|t| t.tok == Tok::Pipe) {
            let at = c.here();
            self.err(at, tr!("ここには表を書けません", "A table cannot stand here"));
            return Err(());
        }
        let name = self.name(&mut c, tr!("フィールドの名前", "the field's name"))?;
        self.tok(&mut c, Tok::Colon)?;
        let ty_at = c.here();
        let ty = self.ty(&mut c)?;
        let ty_span = c.since(ty_at);
        let title = self.opt_string(&mut c);
        let mut f = Field {
            name,
            ty,
            ty_span,
            title,
            range: None,
            count: None,
            optional: None,
            required_when: None,
            shown_when: None,
            approvers_only: None,
            open_in: None,
            round: None,
            computed: None,
            options: Vec::new(),
            value_titles: Vec::new(),
            children: Vec::new(),
            line: l.no,
        };
        self.marks(&mut c, &mut f)?;
        if c.eat(&Tok::Eq) {
            let e = self.expr(&mut c)?;
            f.computed = Some(e);
            self.marks(&mut c, &mut f)?;
        }
        self.end(&c)?;
        match &f.ty {
            TypeExpr::Choice => {
                if under.is_empty() {
                    let n = &f.name.text;
                    self.err_note(
                        f.ty_span,
                        tr!("`{n}` の `choice` の下に、選べる値がありません", "The `choice` `{n}` has no values under it"),
                        tr!(
                            "選べる値を、`travel \"Travel\"` のように一行に一つ、字下げして書いてください。マスタや規則の列挙から選ぶなら `choice from m.accounts` と書きます。",
                            "Write the values under it, indented, one a line, as `travel \"Travel\"`; to choose from a master or a rule's enum, write `choice from m.accounts`."
                        ),
                    );
                    return Err(());
                }
                f.options = self.titled_lines(under, tr!("選択の値", "a value of the choice"));
            }
            // the titles of the values of a rule's enum, which the check holds to the enum's values
            // (a master's rows are its data, and take none)
            TypeExpr::ChoiceFrom(_) => f.value_titles = self.titled_lines(under, tr!("列挙の値", "a value of the enum")),
            TypeExpr::Rows => {
                if under.is_empty() {
                    self.err(f.name.span, tr!("明細の下に、行のフィールドがありません", "A `rows` field has no fields of its row under it"));
                    return Err(());
                }
                f.children = self.fields(under);
                if let Some(r) = f.children.iter().find(|x| x.ty == TypeExpr::Rows) {
                    let at = r.ty_span;
                    self.err(at, tr!("明細の行の中に、明細は書けません", "A row of a `rows` field cannot hold another `rows`"));
                    return Err(());
                }
            }
            _ => {
                if let Some(u) = under.first() {
                    let at = Span { line: u.no, col: u.indent + 1, end: u.indent + 2 };
                    self.err(at, tr!("下に行を書けるのは `choice`、`choice from`、`rows` のフィールドだけです", "Only a `choice`, a `choice from` or a `rows` field has lines under it"));
                    return Err(());
                }
            }
        }
        Ok(f)
    }

    fn ty(&mut self, c: &mut Cur) -> R<TypeExpr> {
        let at = c.here();
        let w = match c.peek() {
            Some(Token { tok: Tok::Word(w), .. }) => w.clone(),
            _ => return self.expected(c, tr!("型", "a type")),
        };
        c.next();
        Ok(match w.as_str() {
            "text" => TypeExpr::Text,
            "long" => {
                self.word(c, kw::TEXT)?;
                TypeExpr::LongText
            }
            "date" => TypeExpr::Date,
            "choice" => {
                if c.eat_word(kw::FROM) {
                    let mut p = vec![self.ident(c, tr!("マスタか規則の列挙", "a master or a rule's enum"))?];
                    while c.eat(&Tok::Dot) {
                        p.push(self.ident(c, tr!("名前", "a name"))?);
                    }
                    TypeExpr::ChoiceFrom(p)
                } else {
                    TypeExpr::Choice
                }
            }
            "attachment" => TypeExpr::Attachment,
            "person" => TypeExpr::Person,
            "team" => TypeExpr::Team,
            "bool" => TypeExpr::Bool,
            "rows" => TypeExpr::Rows,
            "number" => TypeExpr::Num(ritsu_units::Unit::number()),
            "rate" if !c.is(&Tok::LBracket) => TypeExpr::Num(ritsu_units::Unit::rate(None)),
            w if w == kw::MONEY || w == kw::RATE || kw::QUANTITIES.contains(&w) => {
                // `money[JPY]`, `rate[step 0.1%]`, `mass[kg]`: rulec's spelling, read by ritsu-units
                self.tok(c, Tok::LBracket)?;
                let mut inner = String::new();
                loop {
                    match c.next() {
                        Some(Token { tok: Tok::RBracket, .. }) => break,
                        Some(t) => {
                            if !inner.is_empty() && !matches!(t.tok, Tok::Comma) && !inner.ends_with(',') {
                                inner.push(' ');
                            } else if inner.ends_with(',') {
                                inner.push(' ');
                            }
                            inner.push_str(&t.tok.shown());
                        }
                        None => {
                            self.err(at, tr!("`[` が閉じていません", "The `[` is never closed"));
                            return Err(());
                        }
                    }
                }
                let spelled = format!("{w}[{inner}]");
                match ritsu_units::Unit::parse(&spelled) {
                    Ok(u) => TypeExpr::Num(u),
                    Err(p) => {
                        let why = p.text();
                        self.err(at, tr!("型 `{spelled}` を読めません。{}", "The type `{spelled}` does not read: {}", why.ja; why.en));
                        return Err(());
                    }
                }
            }
            "step" => {
                self.err_note(at, tr!("`step` ではなく `stage` と書いてください", "Write `stage`, not `step`"), step_note());
                return Err(());
            }
            other => {
                self.err_note(
                    at,
                    tr!("`{other}` は型ではありません", "`{other}` is not a type"),
                    tr!(
                        "型は `text`、`long text`、`number`、`money[JPY]` や `rate[step 1%]` などの単位の型、`date`、`choice`、`choice from …`、`attachment`、`person`、`team`、`bool`、`rows` のどれかです。",
                        "A type is `text`, `long text`, `number`, a type of a unit such as `money[JPY]` or `rate[step 1%]`, `date`, `choice`, `choice from …`, `attachment`, `person`, `team`, `bool` or `rows`."
                    ),
                );
                return Err(());
            }
        })
    }

    /// The marks after a field's type and title, up to `=` or the end of the line.
    fn marks(&mut self, c: &mut Cur, f: &mut Field) -> R<()> {
        loop {
            let at = c.here();
            let Some(Token { tok: Tok::Word(w), .. }) = c.peek() else { return Ok(()) };
            match w.as_str() {
                "range" => {
                    c.next();
                    f.range = Some(self.bounds(c, at, kw::RANGE)?);
                }
                "count" => {
                    c.next();
                    f.count = Some(self.bounds(c, at, kw::COUNT)?);
                }
                "optional" => {
                    c.next();
                    f.optional = Some(at);
                }
                "required" => {
                    c.next();
                    self.word(c, kw::WHEN)?;
                    f.required_when = Some(self.expr(c)?);
                }
                "shown" => {
                    c.next();
                    self.word(c, kw::WHEN)?;
                    f.shown_when = Some(self.expr(c)?);
                }
                "approvers" => {
                    c.next();
                    self.word(c, kw::ONLY)?;
                    f.approvers_only = Some(c.since(at));
                }
                "open" => {
                    c.next();
                    self.word(c, kw::IN)?;
                    f.open_in = Some(self.ident(c, tr!("カレンダーの別名", "a calendar's alias"))?);
                }
                "round" => {
                    c.next();
                    let mode = self.ident(c, tr!("丸め方（`down`、`up`、`half_up`、`half_down`、`half_even`）", "how it rounds (`down`, `up`, `half_up`, `half_down`, `half_even`)"))?;
                    if !kw::ROUNDINGS.contains(&mode.text.as_str()) {
                        let m = mode.text.clone();
                        self.err(mode.span, tr!("`{m}` は丸め方ではありません（`down`、`up`、`half_up`、`half_down`、`half_even`）", "`{m}` is no way of rounding (`down`, `up`, `half_up`, `half_down`, `half_even`)"));
                        return Err(());
                    }
                    self.tok(c, Tok::LParen)?;
                    let step = self.expr(c)?;
                    self.tok(c, Tok::RParen)?;
                    f.round = Some(Round { mode, step, span: c.since(at) });
                }
                _ => return Ok(()),
            }
        }
    }

    /// `>=1 <=20`: one end or two, each a comparison and a value.
    fn bounds(&mut self, c: &mut Cur, at: Span, word: &str) -> R<Bounds> {
        let mut b = Bounds { lo: None, hi: None, span: at };
        loop {
            let (lo, inclusive) = match c.peek().map(|t| &t.tok) {
                Some(Tok::Ge) => (true, true),
                Some(Tok::Gt) => (true, false),
                Some(Tok::Le) => (false, true),
                Some(Tok::Lt) => (false, false),
                _ => break,
            };
            c.next();
            let value = self.unary(c)?;
            let bound = Some(Bound { value, inclusive });
            if lo { b.lo = bound } else { b.hi = bound }
        }
        if b.lo.is_none() && b.hi.is_none() {
            return self.expected(c, tr!("`{word}` の端（`>=1 <=20` のように）", "the ends of the `{word}` (as in `>=1 <=20`)"));
        }
        b.span = c.since(at);
        Ok(b)
    }

    /// Lines of a name, its ASCII name and its text: the values of a choice, positions, kinds of team.
    fn titled_lines(&mut self, lines: &[&Line], what: Text) -> Vec<Titled> {
        let mut out = Vec::new();
        let Some(base) = lines.first().map(|l| l.indent) else { return out };
        for l in lines {
            if l.indent != base {
                let at = Span { line: l.no, col: l.indent + 1, end: l.indent + 2 };
                self.err(at, tr!("字下げが、上の行とそろっていません", "The indentation does not line up with the lines above"));
                continue;
            }
            let mut c = Cur::new(l);
            let Ok(name) = self.name(&mut c, what.clone()) else { continue };
            let title = self.opt_string(&mut c);
            if self.end(&c).is_ok() {
                out.push(Titled { name, title, line: l.no });
            }
        }
        out
    }

    // ── Expressions ──

    pub fn expr(&mut self, c: &mut Cur) -> R<Expr> {
        self.or(c)
    }

    fn or(&mut self, c: &mut Cur) -> R<Expr> {
        let mut a = self.and(c)?;
        while c.is_word(kw::OR) {
            c.next();
            let b = self.and(c)?;
            let span = join(a.span, b.span);
            a = Expr { kind: ExprKind::Or(Box::new(a), Box::new(b)), span };
        }
        Ok(a)
    }

    fn and(&mut self, c: &mut Cur) -> R<Expr> {
        let mut a = self.not(c)?;
        while c.is_word(kw::AND) {
            c.next();
            let b = self.not(c)?;
            let span = join(a.span, b.span);
            a = Expr { kind: ExprKind::And(Box::new(a), Box::new(b)), span };
        }
        Ok(a)
    }

    fn not(&mut self, c: &mut Cur) -> R<Expr> {
        let at = c.here();
        if c.eat_word(kw::NOT) {
            let e = self.not(c)?;
            let span = join(at, e.span);
            return Ok(Expr { kind: ExprKind::Not(Box::new(e)), span });
        }
        self.cmp(c)
    }

    fn cmp(&mut self, c: &mut Cur) -> R<Expr> {
        let a = self.add(c)?;
        let op = match c.peek().map(|t| &t.tok) {
            Some(Tok::Word(w)) if w == kw::IS => {
                c.next();
                if c.eat_word(kw::NOT) { CmpOp::IsNot } else { CmpOp::Is }
            }
            Some(Tok::Lt) => {
                c.next();
                CmpOp::Lt
            }
            Some(Tok::Le) => {
                c.next();
                CmpOp::Le
            }
            Some(Tok::Gt) => {
                c.next();
                CmpOp::Gt
            }
            Some(Tok::Ge) => {
                c.next();
                CmpOp::Ge
            }
            _ => return Ok(a),
        };
        let b = self.add(c)?;
        if matches!(c.peek().map(|t| &t.tok), Some(Tok::Lt | Tok::Le | Tok::Gt | Tok::Ge)) || c.is_word(kw::IS) {
            let at = c.here();
            self.err(at, tr!("比べを続けて書けません。`a < b and b < c` のように `and` でつないでください", "Comparisons cannot be chained: join them with `and`, as in `a < b and b < c`"));
            return Err(());
        }
        let span = join(a.span, b.span);
        Ok(Expr { kind: ExprKind::Cmp(op, Box::new(a), Box::new(b)), span })
    }

    fn add(&mut self, c: &mut Cur) -> R<Expr> {
        let mut a = self.mul(c)?;
        loop {
            let op = match c.peek().map(|t| &t.tok) {
                Some(Tok::Plus) => BinOp::Add,
                Some(Tok::Minus) => BinOp::Sub,
                _ => return Ok(a),
            };
            c.next();
            let b = self.mul(c)?;
            let span = join(a.span, b.span);
            a = Expr { kind: ExprKind::Bin(op, Box::new(a), Box::new(b)), span };
        }
    }

    fn mul(&mut self, c: &mut Cur) -> R<Expr> {
        let mut a = self.unary(c)?;
        loop {
            let op = match c.peek().map(|t| &t.tok) {
                Some(Tok::Star) => BinOp::Mul,
                Some(Tok::Slash) => BinOp::Div,
                _ => return Ok(a),
            };
            c.next();
            let b = self.unary(c)?;
            let span = join(a.span, b.span);
            a = Expr { kind: ExprKind::Bin(op, Box::new(a), Box::new(b)), span };
        }
    }

    fn unary(&mut self, c: &mut Cur) -> R<Expr> {
        let at = c.here();
        if c.eat(&Tok::Minus) {
            let e = self.unary(c)?;
            let span = join(at, e.span);
            return Ok(Expr { kind: ExprKind::Neg(Box::new(e)), span });
        }
        let a = self.atom(c)?;
        // `days business days in office`: a number of business days
        if c.is_word(kw::BUSINESS) {
            c.next();
            if !(c.eat_word(kw::DAYS) || c.eat_word(kw::DAY)) {
                return self.expected(c, tr!("`days`", "`days`"));
            }
            let cal = if c.eat_word(kw::IN) { Some(self.ident(c, tr!("カレンダーの別名", "a calendar's alias"))?) } else { None };
            let span = c.since(a.span);
            return Ok(Expr { kind: ExprKind::BusinessDays(Box::new(a), cal), span });
        }
        Ok(a)
    }

    fn atom(&mut self, c: &mut Cur) -> R<Expr> {
        let at = c.here();
        let Some(t) = c.peek() else {
            return self.expected(c, tr!("値か名前", "a value or a name"));
        };
        let kind = match &t.tok {
            Tok::Num(n) => {
                c.next();
                ExprKind::Num(n.clone())
            }
            Tok::Date(d) => {
                c.next();
                ExprKind::Date(*d)
            }
            Tok::Str(s) => {
                c.next();
                ExprKind::Str(s.clone())
            }
            Tok::LParen => {
                c.next();
                let e = self.expr(c)?;
                self.tok(c, Tok::RParen)?;
                return Ok(Expr { kind: e.kind, span: c.since(at) });
            }
            Tok::LBracket => {
                c.next();
                let mut xs = Vec::new();
                if !c.eat(&Tok::RBracket) {
                    loop {
                        xs.push(self.expr(c)?);
                        if c.eat(&Tok::RBracket) {
                            break;
                        }
                        self.tok(c, Tok::Comma)?;
                    }
                }
                ExprKind::List(xs)
            }
            Tok::LBrace => {
                c.next();
                let mut fs = Vec::new();
                if !c.eat(&Tok::RBrace) {
                    loop {
                        let n = self.ident(c, tr!("フィールドの名前", "a field's name"))?;
                        self.tok(c, Tok::Colon)?;
                        fs.push((n, self.expr(c)?));
                        if c.eat(&Tok::RBrace) {
                            break;
                        }
                        self.tok(c, Tok::Comma)?;
                    }
                }
                ExprKind::Record(fs)
            }
            Tok::Word(w) if w == kw::NONE => {
                c.next();
                ExprKind::None
            }
            Tok::Word(w) if w == kw::TRUE || w == kw::FALSE => {
                let b = w == kw::TRUE;
                c.next();
                ExprKind::Bool(b)
            }
            Tok::Word(w) if w == kw::SUM && c.peek_at(1).is_some_and(|t| t.tok == Tok::LParen) => {
                c.next();
                c.next();
                let p = self.path(c)?;
                self.tok(c, Tok::RParen)?;
                ExprKind::Sum(p)
            }
            Tok::Word(w) if w == kw::LOOKUP && matches!(c.peek_at(1).map(|t| &t.tok), Some(Tok::Word(_))) && c.peek_at(2).is_some_and(|t| t.tok == Tok::Dot) => {
                c.next();
                let p = self.path(c)?;
                self.word(c, kw::BY)?;
                let by = self.ident(c, tr!("引くフィールド", "the field it looks up by"))?;
                ExprKind::Lookup(p, by)
            }
            // the value of a `person` field and of a `team` field (DESIGN 10.2)
            Tok::Word(w) if w == kw::PERSON && matches!(c.peek_at(1).map(|t| &t.tok), Some(Tok::Str(_))) => {
                c.next();
                let Some(Token { tok: Tok::Str(id), .. }) = c.peek() else { unreachable!() };
                let id = id.clone();
                c.next();
                ExprKind::Person(id)
            }
            Tok::Word(w) if w == kw::TEAM && matches!(c.peek_at(1).map(|t| &t.tok), Some(Tok::Word(_))) => {
                c.next();
                let Some(Token { tok: Tok::Word(id), span }) = c.peek() else { unreachable!() };
                let w = Word { text: id.clone(), span: *span };
                c.next();
                ExprKind::Team(w)
            }
            Tok::Word(w) if kw::is_reserved(w) => {
                let w = w.clone();
                self.err(at, tr!("`{w}` は値を書くところには書けません", "`{w}` cannot stand where a value goes"));
                return Err(());
            }
            Tok::Word(_) => ExprKind::Path(self.path(c)?),
            Tok::Time(_) => {
                self.err(at, tr!("時刻は、期限と催促の `at` のあとにだけ書けます", "A time is written only after the `at` of a deadline or a reminder"));
                return Err(());
            }
            _ => return self.expected(c, tr!("値か名前", "a value or a name")),
        };
        Ok(Expr { kind, span: c.since(at) })
    }

    /// A name, or names joined with dots.
    fn path(&mut self, c: &mut Cur) -> R<Vec<Word>> {
        let mut p = vec![self.ident(c, tr!("名前", "a name"))?];
        while c.is(&Tok::Dot) {
            c.next();
            p.push(self.ident(c, tr!("名前", "a name"))?);
        }
        Ok(p)
    }

    /// `name: value, …` up to the closing `)`.
    fn args(&mut self, c: &mut Cur) -> R<Vec<Arg>> {
        let mut args = Vec::new();
        if c.eat(&Tok::RParen) {
            return Ok(args);
        }
        loop {
            let name = self.ident(c, tr!("引数の名前", "an argument's name"))?;
            self.tok(c, Tok::Colon)?;
            let value = self.expr(c)?;
            args.push(Arg { name, value });
            if c.eat(&Tok::RParen) {
                return Ok(args);
            }
            self.tok(c, Tok::Comma)?;
        }
    }

    /// `budget.reserve(request: ticket.id, …)`, `pay(…)`.
    fn call(&mut self, c: &mut Cur) -> R<Call> {
        let at = c.here();
        let target = self.path(c)?;
        self.tok(c, Tok::LParen)?;
        let args = self.args(c)?;
        Ok(Call { target, args, span: c.since(at) })
    }

    // ── Routes, stages, ways to choose ──

    fn route(&mut self, c: &mut Cur, b: &Block) -> R<Route> {
        if !c.done() {
            // `route standard(long: days > 5)`
            let alias = self.ident(c, tr!("共有の経路の別名か、行の終わり", "a shared route's alias, or the end of the line"))?;
            self.tok(c, Tok::LParen)?;
            let args = self.args(c)?;
            self.end(c)?;
            self.no_body(b);
            return Ok(Route::Shared { alias, args, line: b.head.no });
        }
        let Some(first) = b.body.first() else {
            self.err(b.head.tokens[0].span, tr!("`route` の下に、経路の表かステップの行がありません", "`route` has no table and no stage lines under it"));
            return Err(());
        };
        if first.tokens.first().is_some_and(|t| t.tok == Tok::Pipe) {
            let (head, rows) = self.grid(&b.body)?;
            return Ok(Route::Table(self.route_table(head, rows, b.head.no)?));
        }
        Ok(Route::Stages(self.stage_lines(&b.body)))
    }

    fn stage_lines(&mut self, lines: &[&Line]) -> Vec<StageLine> {
        let mut out = Vec::new();
        let Some(base) = lines.first().map(|l| l.indent) else { return out };
        let mut i = 0;
        while i < lines.len() {
            let l = lines[i];
            let mut j = i + 1;
            while j < lines.len() && lines[j].indent > base {
                j += 1;
            }
            if l.indent != base || l.tokens.first().is_some_and(|t| t.tok == Tok::Pipe) {
                let at = Span { line: l.no, col: l.indent + 1, end: l.indent + 2 };
                self.err(at, tr!("ステップの行がそろっていません。表とステップの行は、一つの経路に混ぜられません", "The stage lines do not line up; a route is a table or stage lines, not both"));
                i = j;
                continue;
            }
            let mut c = Cur::new(l);
            if let Ok((name, spec, spec_span)) = self.stage_head(&mut c) {
                let when = if c.eat_word(kw::WHEN) { self.expr(&mut c).ok() } else { None };
                if self.end(&c).is_ok() {
                    let under: Vec<&Line> = lines[i + 1..j].to_vec();
                    let deadlines = if under.is_empty() { None } else { Some(self.deadlines(&under, l.no)) };
                    out.push(StageLine { name, spec, spec_span, when, deadlines, line: l.no });
                }
            }
            i = j;
        }
        out
    }

    fn stage_head(&mut self, c: &mut Cur) -> R<(Name, Spec, Span)> {
        let name = self.name(c, tr!("ステップの名前", "the stage's name"))?;
        let at = c.here();
        let spec = self.spec(c, false)?;
        Ok((name, spec, c.since(at)))
    }

    /// Who answers a stage (DESIGN 5.1): a way to choose, several joined with `and`, a quorum, a
    /// circulation, a summary, or a rule's output. Empty only in a cell, where `cell` is true.
    fn spec(&mut self, c: &mut Cur, cell: bool) -> R<Spec> {
        let at = c.here();
        if c.done() {
            if cell {
                return Ok(Spec::Empty);
            }
            return self.expected(c, tr!("承認者の決め方", "who answers the stage"));
        }
        if c.is_word(kw::AT) && c.is_word_at(1, kw::LEAST) {
            c.next();
            c.next();
            let q = match c.peek() {
                Some(Token { tok: Tok::Num(n), span }) if n.value.is_int() && n.value.num >= 0 && n.value.num <= u32::MAX as i128 && (n.unit.is_empty() || n.unit == "%") => {
                    let v = n.value.num as u32;
                    if v == 0 {
                        self.err(*span, tr!("議決に要る数は 1 以上で書いてください", "A quorum is 1 or more"));
                        return Err(());
                    }
                    c.next();
                    if n.unit == "%" { Quorum::Percent(v) } else { Quorum::AtLeast(v) }
                }
                _ => return self.expected(c, tr!("議決に要る数（`2` か `60%`）", "the quorum (`2` or `60%`)")),
            };
            self.word(c, kw::OF)?;
            let w = self.ident(c, tr!("承認者の決め方の名前", "a way to choose approvers"))?;
            return Ok(Spec::Quorum(q, w, c.since(at)));
        }
        if c.is_word(kw::ALL) && c.is_word_at(1, kw::OF) {
            c.next();
            c.next();
            let w = self.ident(c, tr!("承認者の決め方の名前", "a way to choose approvers"))?;
            return Ok(Spec::Quorum(Quorum::All, w, c.since(at)));
        }
        if c.is_word(kw::CIRCULATE) && c.is_word_at(1, kw::TO) {
            c.next();
            c.next();
            return Ok(Spec::Circulate(self.ident(c, tr!("承認者の決め方の名前", "a way to choose approvers"))?));
        }
        if c.is_word(kw::NOTE) && c.is_word_at(1, kw::FROM) {
            c.next();
            c.next();
            return Ok(Spec::Note(self.ident(c, tr!("flow の別名", "a flow's alias"))?));
        }
        let first = self.ident(c, tr!("承認者の決め方", "who answers the stage"))?;
        if c.is(&Tok::Dot) {
            let mut p = vec![first];
            while c.eat(&Tok::Dot) {
                p.push(self.ident(c, tr!("規則の出力", "a rule's output"))?);
            }
            return Ok(Spec::RuleOutput(p));
        }
        let mut ws = vec![first];
        while c.is_word(kw::AND) {
            c.next();
            ws.push(self.ident(c, tr!("承認者の決め方の名前", "a way to choose approvers"))?);
        }
        Ok(Spec::Who(ws))
    }

    fn who_block(&mut self, b: &Block) -> Vec<Who> {
        let mut out = Vec::new();
        let Some(base) = b.body.first().map(|l| l.indent) else { return out };
        for l in &b.body {
            let mut c = Cur::new(l);
            if l.indent != base {
                let at = Span { line: l.no, col: l.indent + 1, end: l.indent + 2 };
                self.err(at, tr!("字下げが、上の行とそろっていません", "The indentation does not line up with the lines above"));
                continue;
            }
            if let Ok(w) = self.who_line(&mut c, l.no) {
                out.push(w);
            }
        }
        out
    }

    fn who_line(&mut self, c: &mut Cur, line: usize) -> R<Who> {
        let name = self.name(c, tr!("承認者の決め方の名前", "the name of a way to choose approvers"))?;
        self.tok(c, Tok::Eq)?;
        let at = c.here();
        let w = match c.peek() {
            Some(Token { tok: Tok::Word(w), .. }) => w.clone(),
            _ => return self.expected(c, tr!("承認者の決め方", "a way to choose approvers")),
        };
        c.next();
        let how = match w.as_str() {
            "person" => {
                let (id, span) = self.string(c, tr!("人の ID（`\"e9001\"`）", "the person's ID (`\"e9001\"`)"))?;
                How::Person(id, span)
            }
            "team" => How::Team(self.ident(c, tr!("チームの ID", "the team's ID"))?),
            "manager" => {
                self.word(c, kw::OF)?;
                self.word(c, kw::REQUESTER)?;
                How::ManagerOf
            }
            "head" => {
                self.word(c, kw::OF)?;
                self.word(c, kw::REQUESTER)?;
                self.tok(c, Tok::Dot)?;
                How::HeadOf(self.ident(c, tr!("チームの種類", "a kind of team"))?)
            }
            "nearest" => {
                let p = self.ident(c, tr!("役職", "a position"))?;
                self.word(c, kw::ABOVE)?;
                self.word(c, kw::REQUESTER)?;
                How::Nearest(p)
            }
            "chosen" => {
                self.word(c, kw::BY)?;
                let previous = if c.eat_word(kw::PREVIOUS) {
                    self.word(c, kw::APPROVER)?;
                    true
                } else if c.eat_word(kw::REQUESTER) {
                    false
                } else {
                    return self.expected(c, tr!("`requester` か `previous approver`", "`requester` or `previous approver`"));
                };
                self.word(c, kw::IN)?;
                How::ChosenBy { previous, field: self.ident(c, tr!("人のフィールド", "a field of a person"))? }
            }
            "lookup" => {
                let path = self.path(c)?;
                self.word(c, kw::BY)?;
                How::Lookup { path, by: self.ident(c, tr!("引くフィールド", "the field it looks up by"))? }
            }
            "agent" => {
                let flow = self.ident(c, tr!("flow の別名", "a flow's alias"))?;
                self.tok(c, Tok::Comma)?;
                self.word(c, kw::ELSE)?;
                How::Agent { flow, otherwise: self.ident(c, tr!("疑いのときに答える承認者の決め方", "the way to choose who answers when the agent doubts"))? }
            }
            other => {
                self.err_note(
                    at,
                    tr!("`{other}` は承認者の決め方ではありません", "`{other}` is no way to choose approvers"),
                    tr!(
                        "決め方は `person \"…\"`、`team …`、`manager of requester`、`head of requester.…`、`nearest … above requester`、`chosen by requester in …`、`chosen by previous approver in …`、`lookup … by …`、`agent …, else …` のどれかです。",
                        "A way to choose is `person \"…\"`, `team …`, `manager of requester`, `head of requester.…`, `nearest … above requester`, `chosen by requester in …`, `chosen by previous approver in …`, `lookup … by …` or `agent …, else …`."
                    ),
                );
                return Err(());
            }
        };
        self.end(c)?;
        Ok(Who { name, how, line })
    }

    // ── Deadlines ──

    fn deadlines(&mut self, lines: &[&Line], line: usize) -> Deadlines {
        let mut out = Vec::new();
        let base = lines.first().map(|l| l.indent).unwrap_or(0);
        for l in lines {
            if l.indent != base {
                let at = Span { line: l.no, col: l.indent + 1, end: l.indent + 2 };
                self.err(at, tr!("字下げが、上の行とそろっていません", "The indentation does not line up with the lines above"));
                continue;
            }
            let mut c = Cur::new(l);
            if let Ok(d) = self.deadline(&mut c, l.no) {
                out.push(d);
            }
        }
        if out.is_empty() && lines.is_empty() {
            self.err(Span { line, col: 1, end: 2 }, tr!("`every stage` の下に、期限の行がありません", "`every stage` has no lines of deadlines under it"));
        }
        Deadlines { lines: out, line }
    }

    /// `3 business days [in office]`.
    fn business_days(&mut self, c: &mut Cur) -> R<(u32, Option<Word>)> {
        let (n, _) = self.int(c, tr!("営業日の数", "a number of business days"))?;
        self.word(c, kw::BUSINESS)?;
        if !(c.eat_word(kw::DAYS) || c.eat_word(kw::DAY)) {
            return self.expected(c, tr!("`days`", "`days`"));
        }
        let cal = if c.is_word(kw::IN) && !c.is_word_at(1, kw::AT) {
            c.next();
            Some(self.ident(c, tr!("カレンダーの別名", "a calendar's alias"))?)
        } else {
            None
        };
        Ok((n, cal))
    }

    fn time(&mut self, c: &mut Cur) -> R<u32> {
        match c.peek() {
            Some(Token { tok: Tok::Time(t), .. }) => {
                let t = *t;
                c.next();
                Ok(t)
            }
            _ => self.expected(c, tr!("時刻（`09:00`）", "a time (`09:00`)")),
        }
    }

    fn deadline(&mut self, c: &mut Cur, line: usize) -> R<Deadline> {
        let at = c.here();
        let d = if c.eat_word(kw::DUE) {
            self.word(c, kw::IN)?;
            let (days, calendar) = self.business_days(c)?;
            self.word(c, kw::AT)?;
            let when = if c.eat_word(kw::END) {
                self.word(c, kw::OF)?;
                self.word(c, kw::DAY)?;
                At::EndOfDay
            } else {
                At::Time(self.time(c)?)
            };
            Deadline::Due { days, calendar, at: when, line }
        } else if c.eat_word(kw::REMIND) {
            self.word(c, kw::AFTER)?;
            let (days, calendar) = self.business_days(c)?;
            self.word(c, kw::AT)?;
            Deadline::Remind { days, calendar, at: self.time(c)?, line }
        } else if c.is_word(kw::WHEN) && c.is_word_at(1, kw::LATE) {
            c.next();
            c.next();
            let first = self.late(c)?;
            let then = if c.is(&Tok::Comma) {
                if !matches!(first, Late::Escalate(_)) {
                    return self.then_after_escalate(c, "when late");
                }
                c.next();
                self.word(c, kw::THEN)?;
                Some(self.late(c)?)
            } else {
                None
            };
            Deadline::Late { first, then, line }
        } else if c.is_word(kw::WHEN) && c.is_word_at(1, kw::EMPTY) {
            c.next();
            c.next();
            let what = if c.eat_word(kw::STAY) {
                WhenEmpty::Stay
            } else if c.eat_word(kw::ESCALATE) {
                // where nobody is above: `, then skip`, `, then stay`, or `, then <a way to
                // choose>` (DESIGN 5.4)
                let top = if c.eat(&Tok::Comma) {
                    self.word(c, kw::THEN)?;
                    if c.eat_word(kw::SKIP) {
                        Some(AtTop::Skip)
                    } else if c.eat_word(kw::STAY) {
                        Some(AtTop::Stay)
                    } else if c.is_word(kw::ESCALATE) {
                        let at = c.here();
                        self.err_note(
                            at,
                            tr!(
                                "`when empty` は二度上へ回しません。`then` のあとは `skip`、`stay`、承認者の決め方の名前のどれかです",
                                "`when empty` does not go up twice: after `then`, write `skip`, `stay` or a way to choose an approver"
                            ),
                            tr!(
                                "上へ回すのは、上に人がいる間ずっとです。`then` は、上に回せる人がいない（組織のいちばん上の）ときにすることを書きます。ステップを飛ばす（`skip`）、止まって管理する人を待つ（`stay`）、`who` に宣言した決め方へ回す（`then board`）のどれかです。",
                                "A stage goes up as long as someone is above. `then` says what it does where nobody is above (the top of the organization): it is skipped (`skip`), waits for an administrator (`stay`), or goes to a way to choose declared in `who` (`then board`)."
                            ),
                        );
                        return Err(());
                    } else if matches!(c.peek(), Some(Token { tok: Tok::Word(_), .. })) {
                        Some(AtTop::To(self.ident(c, tr!("承認者の決め方の名前", "a way to choose an approver"))?))
                    } else {
                        return self.expected(c, tr!("`skip`、`stay`、承認者の決め方の名前のどれか", "`skip`, `stay` or a way to choose an approver"));
                    }
                } else {
                    None
                };
                WhenEmpty::Escalate(top)
            } else if c.eat_word(kw::SKIP) {
                WhenEmpty::Skip
            } else {
                return self.expected(c, tr!("`stay`、`escalate`、`skip` のどれか", "one of `stay`, `escalate` and `skip`"));
            };
            if c.is(&Tok::Comma) && !matches!(what, WhenEmpty::Escalate(_)) {
                return self.then_after_escalate(c, "when empty");
            }
            Deadline::Empty { what, line }
        } else {
            let _ = at;
            return self
                .expected(c, tr!("期限の行（`due in …`、`remind after …`、`when late …`、`when empty …`）", "a line of deadlines (`due in …`, `remind after …`, `when late …`, `when empty …`)"))
                ;
        };
        self.end(c)?;
        Ok(d)
    }

    /// E001 at a `, then …` after what is not `escalate`: only a stage that goes up has a next step.
    fn then_after_escalate<T>(&mut self, c: &Cur, line: &str) -> R<T> {
        let at = c.here();
        self.err(at, tr!("`{line}` の `, then …` は、`escalate` のあとにだけ書けます", "In `{line}`, `, then …` follows `escalate` only"));
        Err(())
    }

    fn late(&mut self, c: &mut Cur) -> R<Late> {
        if c.eat_word(kw::ESCALATE) {
            let by = if c.eat_word(kw::BY) { Some(self.ident(c, tr!("表の名前", "a table's name"))?) } else { None };
            Ok(Late::Escalate(by))
        } else if c.eat_word(kw::EXPIRE) {
            Ok(Late::Expire)
        } else if c.eat_word(kw::STAY) {
            Ok(Late::Stay)
        } else {
            if c.is_word("up") {
                let at = c.here();
                self.err(at, tr!("上へ回すことは `escalate` と書いてください", "Passing a stage up is written `escalate`"));
                return Err(());
            }
            self.expected(c, tr!("`escalate`、`expire`、`stay` のどれか", "one of `escalate`, `expire` and `stay`"))
        }
    }

    // ── Tables ──

    /// The rows of a table: its header, and its rows, each as the tokens of its cells.
    #[allow(clippy::type_complexity)]
    fn grid<'l>(&mut self, lines: &[&'l Line]) -> R<(Vec<(Vec<Token>, Span)>, Vec<(Vec<(Vec<Token>, Span)>, usize)>)> {
        let mut rows = Vec::new();
        for l in lines {
            let toks = &l.tokens;
            if toks.first().is_none_or(|t| t.tok != Tok::Pipe) || toks.last().is_none_or(|t| t.tok != Tok::Pipe) || toks.len() < 2 {
                let at = Span { line: l.no, col: l.indent + 1, end: l.indent + 2 };
                self.err(at, tr!("表の行は `|` で始めて `|` で終えてください", "A row of a table starts and ends with `|`"));
                return Err(());
            }
            let mut cells = Vec::new();
            let mut cur: Vec<Token> = Vec::new();
            let mut from = toks[0].span;
            for t in &toks[1..] {
                if t.tok == Tok::Pipe {
                    let span = Span { line: l.no, col: from.end, end: t.span.col };
                    cells.push((std::mem::take(&mut cur), span));
                    from = t.span;
                } else {
                    cur.push(t.clone());
                }
            }
            rows.push((cells, l.no));
        }
        let Some((head, _)) = rows.first().cloned() else {
            return Err(());
        };
        let width = head.len();
        for (cells, no) in &rows[1..] {
            if cells.len() != width {
                let n = cells.len();
                self.err(Span { line: *no, col: 1, end: 2 }, tr!("この行のセルは {n} 個で、見出しの {width} 個と合いません", "This row has {n} cells, and the header has {width}"));
                return Err(());
            }
        }
        if rows.len() < 2 {
            self.err(Span { line: rows[0].1, col: 1, end: 2 }, tr!("表に、見出しのほかの行がありません", "The table has no row under its header"));
            return Err(());
        }
        Ok((head, rows[1..].to_vec()))
    }

    /// A header's columns: the names before `->`, and the names after it.
    fn header(&mut self, head: &[(Vec<Token>, Span)], line: usize, dotted_inputs: bool) -> R<(Vec<Vec<Word>>, Vec<Name>)> {
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut arrow = false;
        for (toks, span) in head {
            let mut c = Cur::of(toks, line);
            if c.eat(&Tok::Arrow) {
                if arrow {
                    self.err(*span, tr!("`->` は見出しに一度だけ書きます", "A header has one `->`"));
                    return Err(());
                }
                arrow = true;
            }
            if c.done() {
                self.err(*span, tr!("見出しのセルが空です", "A cell of the header is empty"));
                return Err(());
            }
            if arrow {
                let n = self.name(&mut c, tr!("出力の列の名前", "an output column's name"))?;
                self.end(&c)?;
                outputs.push(n);
            } else if dotted_inputs {
                let p = self.path(&mut c)?;
                // an input column may give its ASCII name too: `承認者(approver)`
                if let Some(Token { tok: Tok::LParen, .. }) = c.peek()
                    && p.len() == 1
                {
                    c.next();
                    self.ident(&mut c, tr!("括弧の中の ASCII の名前", "the ASCII name in the brackets"))?;
                    self.tok(&mut c, Tok::RParen)?;
                }
                self.end(&c)?;
                inputs.push(p);
            } else {
                let n = self.name(&mut c, tr!("入力の列の名前", "an input column's name"))?;
                self.end(&c)?;
                inputs.push(vec![Word { text: n.text, span: n.span }]);
            }
        }
        if !arrow {
            self.err_note(
                Span { line, col: 1, end: 2 },
                tr!("見出しに `->` がありません", "The header has no `->`"),
                tr!("入力の列のあと、最初の出力の列の前に `->` を書きます（`| category | amount | -> first |`）。", "Write `->` after the input columns, before the first output column (`| category | amount | -> first |`)."),
            );
            return Err(());
        }
        if outputs.is_empty() {
            self.err(Span { line, col: 1, end: 2 }, tr!("`->` のあとに出力の列がありません", "No output column follows `->`"));
            return Err(());
        }
        Ok((inputs, outputs))
    }

    fn route_table(&mut self, head: Vec<(Vec<Token>, Span)>, rows: Vec<(Vec<(Vec<Token>, Span)>, usize)>, line: usize) -> R<RouteTable> {
        let head_line = rows.first().map(|r| r.1 - 1).unwrap_or(line);
        let (inputs, stages) = self.header(&head, head_line.max(line + 1), true)?;
        let chooses_route = stages.len() == 1 && stages[0].text == kw::ROUTE && stages[0].alias.is_none();
        let n_in = inputs.len();
        let mut out = Vec::new();
        for (cells, no) in rows {
            let mut tests = Vec::new();
            let mut specs = Vec::new();
            let mut ok = true;
            for (k, (toks, span)) in cells.iter().enumerate() {
                let mut c = Cur::of(toks, no);
                if k < n_in {
                    match self.test(&mut c, *span) {
                        Ok(t) => tests.push((t, *span)),
                        Err(()) => ok = false,
                    }
                } else if chooses_route {
                    match self.ident(&mut c, tr!("共有の経路の別名", "a shared route's alias")) {
                        Ok(w) if self.end(&c).is_ok() => specs.push((Spec::Route(w), *span)),
                        _ => ok = false,
                    }
                } else {
                    match self.spec(&mut c, true) {
                        Ok(s) if self.end(&c).is_ok() => specs.push((s, *span)),
                        _ => ok = false,
                    }
                }
            }
            if ok {
                out.push(RouteRow { tests, stages: specs, line: no });
            }
        }
        Ok(RouteTable { inputs, stages, chooses_route, rows: out, line })
    }

    /// An input cell, in rulec's spelling: `-`, values, `not: …`, comparisons.
    fn test(&mut self, c: &mut Cur, span: Span) -> R<Test> {
        if c.done() {
            self.err_note(span, tr!("入力のセルが空です", "An input cell is empty"), tr!("どの値でもよいなら `-` と書いてください（値の無いことも含みます）。値の無いことを選ぶなら `none` と書いてください。", "Write `-` for any value (no value too), or `none` for no value."));
            return Err(());
        }
        if c.is(&Tok::Minus) && c.t.len() == 1 {
            c.next();
            return Ok(Test::Any);
        }
        if matches!(c.peek().map(|t| &t.tok), Some(Tok::Lt | Tok::Le | Tok::Gt | Tok::Ge)) {
            let mut cmps = Vec::new();
            while let Some(op) = match c.peek().map(|t| &t.tok) {
                Some(Tok::Lt) => Some(CmpOp::Lt),
                Some(Tok::Le) => Some(CmpOp::Le),
                Some(Tok::Gt) => Some(CmpOp::Gt),
                Some(Tok::Ge) => Some(CmpOp::Ge),
                _ => None,
            } {
                c.next();
                cmps.push((op, self.unary(c)?));
            }
            self.end(c)?;
            return Ok(Test::Compare(cmps));
        }
        let not = if c.is_word(kw::NOT) && c.peek_at(1).is_some_and(|t| t.tok == Tok::Colon) {
            c.next();
            c.next();
            true
        } else {
            false
        };
        let mut values = vec![self.unary(c)?];
        while c.eat(&Tok::Comma) {
            values.push(self.unary(c)?);
        }
        self.end(c)?;
        Ok(Test::Values { values, not })
    }

    fn named_table(&mut self, name: Name, b: &Block) -> R<NamedTable> {
        if b.body.is_empty() {
            self.err(name.span, tr!("表 `{}` の下に、表の行がありません", "The table `{}` has no rows under it", name.text));
            return Err(());
        }
        let (head, rows) = self.grid(&b.body)?;
        let head_line = b.body[0].no;
        let (inputs, outputs) = self.header(&head, head_line, false)?;
        // the input columns, read again for their ASCII names
        let mut in_names = Vec::new();
        for (toks, _) in head.iter().take(inputs.len()) {
            let mut c = Cur::of(toks, head_line);
            in_names.push(self.name(&mut c, tr!("入力の列の名前", "an input column's name"))?);
        }
        let n_in = inputs.len();
        let mut out = Vec::new();
        for (cells, no) in rows {
            let mut tests = Vec::new();
            let mut specs = Vec::new();
            for (k, (toks, span)) in cells.iter().enumerate() {
                let mut c = Cur::of(toks, no);
                if k < n_in {
                    tests.push((self.test(&mut c, *span)?, *span));
                } else {
                    let s = self.spec(&mut c, true)?;
                    self.end(&c)?;
                    specs.push((s, *span));
                }
            }
            out.push(RouteRow { tests, stages: specs, line: no });
        }
        Ok(NamedTable { name, inputs: in_names, outputs, rows: out, line: b.head.no })
    }

    fn examples(&mut self, b: &Block) -> R<Examples> {
        if b.body.is_empty() {
            self.err(b.head.tokens[0].span, tr!("`examples` の下に、表がありません", "`examples` has no table under it"));
            return Err(());
        }
        let (head, rows) = self.grid(&b.body)?;
        let head_line = b.body[0].no;
        let (inputs, stages) = self.header(&head, head_line, true)?;
        let stages: Vec<Word> = stages.into_iter().map(|n| Word { text: n.text, span: n.span }).collect();
        let n_in = inputs.len();
        let mut out = Vec::new();
        for (cells, no) in rows {
            let mut values = Vec::new();
            let mut specs = Vec::new();
            for (k, (toks, span)) in cells.iter().enumerate() {
                let mut c = Cur::of(toks, no);
                if k < n_in {
                    if c.done() {
                        self.err(*span, tr!("例の入力のセルが空です。値を書いてください", "An input cell of an example is empty; write a value"));
                        return Err(());
                    }
                    let v = self.unary(&mut c)?;
                    self.end(&c)?;
                    values.push((v, *span));
                } else {
                    let s = self.spec(&mut c, true)?;
                    self.end(&c)?;
                    specs.push((s, *span));
                }
            }
            out.push(ExampleRow { values, stages: specs, line: no });
        }
        Ok(Examples { inputs, stages, rows: out, line: b.head.no })
    }

    // ── A ticket ──

    fn ticket(&mut self, b: &Block) -> Ticket {
        let mut lines: Vec<TicketLine> = Vec::new();
        let base = b.body.first().map(|l| l.indent).unwrap_or(0);
        for l in &b.body {
            if l.indent != base {
                let at = Span { line: l.no, col: l.indent + 1, end: l.indent + 2 };
                self.err(at, tr!("字下げが、上の行とそろっていません", "The indentation does not line up with the lines above"));
                continue;
            }
            let mut c = Cur::new(l);
            let at = c.here();
            if let Ok(t) = self.ticket_line(&mut c, l.no) {
                // a rule of the ticket is written once; only `on` lines are as many as there are
                if let Some(word) = once_word(&t)
                    && let Some(first) = lines.iter().find(|x| once_word(x) == Some(word))
                {
                    let n = first.line();
                    self.at_code(
                        "E006",
                        at,
                        tr!("`ticket` に `{word}` は一度だけ書けます", "A `ticket` takes `{word}` once"),
                        Some(tr!("{n} 行目にもう書いてあります。一行にまとめてください。", "It is written on line {n} already: make the two one line.")),
                    );
                    continue;
                }
                lines.push(t);
            }
        }
        Ticket { lines, line: b.head.no }
    }

    /// `submitted by …` and `except …`: the people of each group, one group or more, with commas.
    fn groups(&mut self, c: &mut Cur) -> R<Vec<Group>> {
        let mut out = vec![self.group(c)?];
        while c.eat(&Tok::Comma) {
            out.push(self.group(c)?);
        }
        Ok(out)
    }

    fn group(&mut self, c: &mut Cur) -> R<Group> {
        let at = c.here();
        let team = || tr!("チームの ID", "the team's ID");
        if c.eat_word(kw::EVERYONE) {
            let within = if c.eat_word(kw::IN) { Some(self.ident(c, team())?) } else { None };
            Ok(Group::Everyone { within, span: at })
        } else if c.eat_word(kw::TEAM) {
            Ok(Group::Team(self.ident(c, team())?))
        } else if c.eat_word(kw::POSITION) {
            let position = self.ident(c, tr!("役職", "a position"))?;
            let higher = if c.eat_word(kw::OR) {
                self.word(c, kw::HIGHER)?;
                true
            } else {
                false
            };
            let within = if c.eat_word(kw::IN) { Some(self.ident(c, team())?) } else { None };
            Ok(Group::Position { position, higher, within })
        } else if c.eat_word(kw::PERSON) {
            let (id, span) = self.string(c, tr!("人の ID（`\"e9001\"`）", "the person's ID (`\"e9001\"`)"))?;
            Ok(Group::Person(id, span))
        } else {
            self.expected(
                c,
                tr!(
                    "出せる人（`everyone`、`everyone in …`、`team …`、`position …`、`person \"…\"`）",
                    "who may submit (`everyone`, `everyone in …`, `team …`, `position …` or `person \"…\"`)"
                ),
            )
        }
    }

    fn ticket_line(&mut self, c: &mut Cur, line: usize) -> R<TicketLine> {
        let t = if c.eat_word(kw::SELF) {
            self.word(c, kw::APPROVAL)?;
            let allowed = if c.eat_word(kw::NOT) {
                self.word(c, kw::ALLOWED)?;
                false
            } else {
                self.word(c, kw::ALLOWED)?;
                true
            };
            TicketLine::SelfApproval { allowed, line }
        } else if c.eat_word(kw::SAME) {
            for w in [kw::APPROVER, kw::APPROVES, kw::ONCE] {
                self.word(c, w)?;
            }
            // `, at the last stage`: one answer, at the last of the stages the approver answers
            if c.is_word(kw::AT) {
                // the comma left out
                self.tok(c, Tok::Comma)?;
            }
            let last = c.eat(&Tok::Comma);
            if last {
                for w in [kw::AT, kw::THE, kw::LAST, kw::STAGE] {
                    self.word(c, w)?;
                }
            }
            TicketLine::SameApprover { last, line }
        } else if c.is_word(kw::SUBMITTED) {
            let col = c.here().col;
            c.next();
            self.word(c, kw::BY)?;
            let groups = self.groups(c)?;
            let except = if c.eat_word(kw::EXCEPT) { self.groups(c)? } else { Vec::new() };
            TicketLine::SubmittedBy { groups, except, line, col }
        } else if c.eat_word(kw::SEND) {
            self.word(c, kw::BACK)?;
            self.word(c, kw::TO)?;
            let to_requester = if c.eat_word(kw::THE) {
                self.word(c, kw::REQUESTER)?;
                true
            } else if c.eat_word(kw::ANY) {
                self.word(c, kw::EARLIER)?;
                self.word(c, kw::STAGE)?;
                false
            } else {
                return self.expected(c, tr!("`the requester` か `any earlier stage`", "`the requester` or `any earlier stage`"));
            };
            self.tok(c, Tok::Comma)?;
            self.word(c, kw::AT)?;
            self.word(c, kw::MOST)?;
            let (times, _) = self.int(c, tr!("回数", "a number of times"))?;
            self.word(c, kw::TIMES)?;
            TicketLine::SendBack { to_requester, times, line }
        } else if c.eat_word(kw::RESUBMIT) {
            self.word(c, kw::WITHIN)?;
            let (days, calendar) = self.business_days(c)?;
            TicketLine::Resubmit { days, calendar, line }
        } else if c.eat_word(kw::WITHDRAW) {
            self.word(c, kw::UNTIL)?;
            if c.is_word(kw::IT) && c.is_word_at(1, kw::IS) {
                c.next();
                c.next();
                self.word(c, kw::DECIDED)?;
                TicketLine::Withdraw { until: None, line }
            } else {
                TicketLine::Withdraw { until: Some(self.ident(c, tr!("ステップの名前か `it is decided`", "a stage's name, or `it is decided`"))?), line }
            }
        } else if c.eat_word(kw::RETRACT) {
            self.word(c, kw::APPROVAL)?;
            for w in [kw::UNTIL, kw::THE, kw::NEXT, kw::STAGE, kw::ANSWERS] {
                self.word(c, w)?;
            }
            TicketLine::Retract { line }
        } else if c.eat_word(kw::PAUSE) {
            self.word(c, kw::AT)?;
            self.word(c, kw::MOST)?;
            let (days, calendar) = self.business_days(c)?;
            TicketLine::Pause { days, calendar, line }
        } else if c.eat_word(kw::APPROVERS) {
            self.word(c, kw::EDIT)?;
            let mut fields = vec![self.ident(c, tr!("フィールドの名前", "a field's name"))?];
            while c.eat(&Tok::Comma) {
                fields.push(self.ident(c, tr!("フィールドの名前", "a field's name"))?);
            }
            TicketLine::ApproversEdit { fields, line }
        } else if kw::COMMENT_REQUIRED.split(' ').next().is_some_and(|w| c.is_word(w)) {
            for w in kw::COMMENT_REQUIRED.split(' ') {
                self.word(c, w)?;
            }
            let mut ops: Vec<(Reasoned, Span)> = Vec::new();
            loop {
                let at = c.here();
                let Some(op) = [Reasoned::Reject, Reasoned::SendBack].into_iter().find(|o| o.word().split(' ').enumerate().all(|(i, w)| c.is_word_at(i, w))) else {
                    let all = kw::REASONED.map(|w| format!("`{w}`"));
                    return self.expected(c, tr!("理由を添える操作（{}）", "an operation given with a reason ({})", all.join(" か "); all.join(" or ")));
                };
                for _ in op.word().split(' ') {
                    c.next();
                }
                let span = c.since(at);
                if ops.iter().any(|(o, _)| *o == op) {
                    let w = op.word();
                    self.err(span, tr!("`{w}` を二度書いています。一度にしてください", "`{w}` is written twice: write it once"));
                    return Err(());
                }
                ops.push((op, span));
                if !c.eat(&Tok::Comma) {
                    break;
                }
            }
            TicketLine::CommentRequired { ops, line }
        } else if c.eat_word(kw::ON) {
            let mut endings = Vec::new();
            loop {
                let at = c.here();
                let e = if c.eat_word("submitted") {
                    Ending::Submitted
                } else if c.eat_word("approved") {
                    Ending::Approved
                } else if c.is_word(kw::SEND) || c.is_word("sent") {
                    c.next();
                    self.word(c, kw::BACK)?;
                    Ending::SentBack
                } else if c.eat_word("rejected") {
                    Ending::Rejected
                } else if c.eat_word("withdrawn") {
                    Ending::Withdrawn
                } else if c.eat_word("expired") {
                    Ending::Expired
                } else {
                    return self
                        .expected(c, tr!("`submitted`、`approved`、`sent back`、`rejected`、`withdrawn`、`expired` のどれか", "one of `submitted`, `approved`, `sent back`, `rejected`, `withdrawn` and `expired`"))
                        ;
                };
                endings.push((e, c.since(at)));
                if !c.eat(&Tok::Comma) {
                    break;
                }
            }
            let at = c.here();
            let action = if c.eat_word(kw::HOLD) {
                ActionKind::Hold
            } else if c.eat_word(kw::POST) {
                ActionKind::Post
            } else if c.eat_word(kw::VOID) {
                ActionKind::Void
            } else if c.eat_word(kw::LAUNCH) {
                ActionKind::Launch
            } else {
                if c.is_word("start") {
                    self.err(at, tr!("flow を始めることは `launch` と書いてください", "Starting a flow is written `launch`"));
                    return Err(());
                }
                return self.expected(c, tr!("`hold`、`post`、`void`、`launch` のどれか", "one of `hold`, `post`, `void` and `launch`"));
            };
            let call = self.call(c)?;
            let when = if c.eat_word(kw::WHEN) { Some(self.expr(c)?) } else { None };
            TicketLine::On { endings, action, call, when, line }
        } else if c.is_word("cancel") {
            let at = c.here();
            self.err(at, tr!("承認の取り消しは `retract approval until the next stage answers` と書いてください", "Taking an approval back is written `retract approval until the next stage answers`"));
            return Err(());
        } else {
            return self.expected(c, tr!("チケットの決まりの行", "a line of the ticket's rules"));
        };
        self.end(c)?;
        Ok(t)
    }

    // ── Claims, scenarios, labels ──

    fn claims(&mut self, b: &Block) -> Vec<Claim> {
        let mut out = Vec::new();
        let base = b.body.first().map(|l| l.indent).unwrap_or(0);
        for l in &b.body {
            if l.indent != base {
                let at = Span { line: l.no, col: l.indent + 1, end: l.indent + 2 };
                self.err(at, tr!("字下げが、上の行とそろっていません", "The indentation does not line up with the lines above"));
                continue;
            }
            let mut c = Cur::new(l);
            if let Ok(cl) = self.claim(&mut c, l.no) {
                out.push(cl);
            }
        }
        out
    }

    fn claim(&mut self, c: &mut Cur, line: usize) -> R<Claim> {
        let name = self.ident(c, tr!("主張の名前", "the claim's name"))?;
        self.tok(c, Tok::Colon)?;
        let body = if c.is_word(kw::EVERY) && c.is_word_at(1, kw::STAGE) {
            c.next();
            c.next();
            for w in [kw::REMINDS, kw::BEFORE, kw::IT, kw::IS, kw::DUE] {
                self.word(c, w)?;
            }
            ClaimBody::RemindsBeforeDue
        } else if c.is_word(kw::EVERY) && c.is_word_at(1, kw::TICKET) {
            c.next();
            c.next();
            self.word(c, kw::ENDS)?;
            self.word(c, kw::WITHIN)?;
            let (days, _) = self.int(c, tr!("営業日の数", "a number of business days"))?;
            self.word(c, kw::BUSINESS)?;
            if !(c.eat_word(kw::DAYS) || c.eat_word(kw::DAY)) {
                return self.expected(c, tr!("`days`", "`days`"));
            }
            ClaimBody::EndsWithin { days }
        } else if c.is_word(kw::EVERY) && c.is_word_at(1, kw::STEP) {
            c.next();
            return self.expected(c, tr!("`stage`", "`stage`"));
        } else if c.is_word(kw::POSSIBLE) && !c.is_word_at(1, kw::IS) {
            c.next();
            let stage = self.ident(c, tr!("ステップの名前", "a stage's name"))?;
            self.word(c, kw::IS)?;
            let spec = self.spec(c, false)?;
            ClaimBody::Possible { stage, spec }
        } else {
            let stage = self.ident(c, tr!("ステップの名前", "a stage's name"))?;
            self.word(c, kw::IS)?;
            let spec = self.spec(c, false)?;
            self.word(c, kw::ONLY)?;
            self.word(c, kw::WHEN)?;
            ClaimBody::OnlyWhen { stage, spec, when: self.expr(c)? }
        };
        self.end(c)?;
        Ok(Claim { name, body, line })
    }

    fn scenario(&mut self, name: Word, b: &Block) -> R<Scenario> {
        let mut s = Scenario { name, requester: None, given: Vec::new(), form: Vec::new(), events: Vec::new(), line: b.head.no };
        let rows: Vec<&Line> = b.body.iter().copied().filter(|l| l.tokens.first().is_some_and(|t| t.tok == Tok::Pipe)).collect();
        for l in b.body.iter().filter(|l| l.tokens.first().is_none_or(|t| t.tok != Tok::Pipe)) {
            let mut c = Cur::new(l);
            let _ = self.scenario_line(&mut c, &mut s);
        }
        if rows.is_empty() {
            self.err(s.name.span, tr!("シナリオに、イベントの表がありません", "The scenario has no table of events"));
            return Err(());
        }
        let (head, cells) = self.grid(&rows)?;
        let head_line = rows[0].no;
        // `| at | event | by | -> stage | status |`, with `| comment |` after `by` where a row writes
        // what a person wrote
        let mut words = Vec::new();
        for (toks, span) in &head {
            let mut c = Cur::of(toks, head_line);
            let arrow = c.eat(&Tok::Arrow);
            let w = match c.peek() {
                Some(Token { tok: Tok::Word(w), .. }) if c.t.len() == 1 + usize::from(arrow) => w.clone(),
                _ => String::new(),
            };
            if w == kw::STEP {
                self.err_note(*span, tr!("`step` ではなく `stage` と書いてください", "Write `stage`, not `step`"), step_note());
                return Err(());
            }
            words.push((arrow, w));
        }
        let full: Vec<(bool, String)> = kw::SCENARIO_COLUMNS.iter().map(|w| (*w == "stage", w.to_string())).collect();
        let short: Vec<(bool, String)> = full.iter().filter(|(_, w)| w != kw::COMMENT).cloned().collect();
        let with_comment = words == full;
        if !with_comment && words != short {
            self.err(
                Span { line: head_line, col: 1, end: 2 },
                tr!(
                    "シナリオの表の見出しは `| at | event | by | -> stage | status |` です。人が書いたコメントを書く表は、`by` のあとに `comment` の列を足します",
                    "A scenario's table has the header `| at | event | by | -> stage | status |`, and a `comment` column after `by` where a row writes what a person wrote"
                ),
            );
            return Err(());
        }
        for (row, no) in cells {
            if let Ok(e) = self.event(&row, no, with_comment) {
                s.events.push(e);
            }
        }
        Ok(s)
    }

    fn scenario_line(&mut self, c: &mut Cur, s: &mut Scenario) -> R<()> {
        if c.eat_word(kw::REQUESTER) {
            let at = c.here();
            let w = match c.peek() {
                Some(Token { tok: Tok::Word(w), span }) => Word { text: w.clone(), span: *span },
                Some(Token { tok: Tok::Str(w), span }) => Word { text: w.clone(), span: *span },
                _ => return self.expected(c, tr!("申請者の ID", "the requester's ID")),
            };
            let _ = at;
            c.next();
            s.requester = Some(w);
        } else if c.eat_word(kw::GIVEN) {
            s.given.push(self.call(c)?);
        } else if c.eat_word(kw::FORM) {
            loop {
                let n = self.ident(c, tr!("フィールドの名前", "a field's name"))?;
                self.tok(c, Tok::Eq)?;
                let v = self.expr(c)?;
                s.form.push((n, v));
                if !c.eat(&Tok::Comma) {
                    break;
                }
            }
        } else {
            return self.expected(c, tr!("`requester`、`given`、`form` のどれか", "one of `requester`, `given` and `form`"));
        }
        self.end(c)
    }

    fn event(&mut self, row: &[(Vec<Token>, Span)], no: usize, with_comment: bool) -> R<Event> {
        let words = |toks: &[Token]| -> Option<(String, Span)> {
            let mut out = Vec::new();
            for t in toks {
                match &t.tok {
                    Tok::Word(w) => out.push(w.clone()),
                    _ => return None,
                }
            }
            let first = toks.first()?.span;
            let last = toks.last()?.span;
            Some((out.join(" "), Span { line: first.line, col: first.col, end: last.end }))
        };
        // at
        let (at_toks, at_span) = &row[0];
        let at = match at_toks.as_slice() {
            [] => None,
            [Token { tok: Tok::Date(d), .. }, Token { tok: Tok::Time(t), .. }] => Some((*d, *t)),
            _ => {
                self.err(*at_span, tr!("`at` のセルは、日付と時刻（`2026-11-16 10:00`）で書いてください", "An `at` cell is a date and a time (`2026-11-16 10:00`)"));
                return Err(());
            }
        };
        // event: its name, and what it carries in brackets
        let (ev_toks, ev_span) = &row[1];
        let open = ev_toks.iter().position(|t| t.tok == Tok::LParen).unwrap_or(ev_toks.len());
        let Some((ev, ev_at)) = words(&ev_toks[..open]) else {
            self.err(*ev_span, tr!("`event` のセルにイベントがありません", "The `event` cell holds no event"));
            return Err(());
        };
        let args = if open < ev_toks.len() {
            let mut c = Cur::of(&ev_toks[open + 1..], no);
            let args = self.args(&mut c)?;
            self.end(&c)?;
            args
        } else {
            Vec::new()
        };
        if !kw::EVENTS.contains(&ev.as_str()) {
            let list = kw::EVENTS.iter().map(|e| format!("`{e}`")).collect::<Vec<_>>().join(" ");
            self.err_note(ev_at, tr!("`{ev}` はチケットのイベントではありません", "`{ev}` is no event of a ticket"), tr!("イベントは {list} のどれかです。", "An event is one of {list}."));
            return Err(());
        }
        // by
        let (by_toks, by_span) = &row[2];
        let by = match by_toks.as_slice() {
            [] => None,
            [Token { tok: Tok::Word(w), span }] => Some(Word { text: w.clone(), span: *span }),
            _ => {
                self.err(*by_span, tr!("`by` のセルは、人の ID 一つか空です", "A `by` cell is one person's ID, or empty"));
                return Err(());
            }
        };
        // comment: a text in quotes, or nothing
        let k = usize::from(with_comment);
        let comment = match row[3].0.as_slice() {
            _ if !with_comment => None,
            [] => None,
            [Token { tok: Tok::Str(w), span }] => Some(Word { text: w.clone(), span: *span }),
            _ => {
                self.err(row[3].1, tr!("`comment` のセルは、`\"` で囲んだ文一つか空です", "A `comment` cell is one text in quotes, or empty"));
                return Err(());
            }
        };
        // stage
        let (st_toks, st_span) = &row[3 + k];
        let stage = match st_toks.as_slice() {
            [] => None,
            [Token { tok: Tok::Word(w), span }] => Some(Word { text: w.clone(), span: *span }),
            _ => {
                self.err(*st_span, tr!("`stage` のセルは、ステップの名前一つか空です", "A `stage` cell is one stage's name, or empty"));
                return Err(());
            }
        };
        // status
        let (stat_toks, stat_span) = &row[4 + k];
        let Some((status, stat_at)) = words(stat_toks) else {
            self.err(*stat_span, tr!("`status` のセルに状態がありません", "The `status` cell holds no state"));
            return Err(());
        };
        if !kw::STATES.contains(&status.as_str()) {
            let list = kw::STATES.iter().map(|e| format!("`{e}`")).collect::<Vec<_>>().join(" ");
            self.err_note(stat_at, tr!("`{status}` はチケットの状態ではありません", "`{status}` is no state of a ticket"), tr!("状態は {list} のどれかです。", "A state is one of {list}."));
            return Err(());
        }
        Ok(Event { at, event: Word { text: ev, span: ev_at }, args, by, comment, stage, status: Word { text: status, span: stat_at }, line: no })
    }

    fn labels(&mut self, lang: Word, b: &Block) -> Labels {
        let mut entries = Vec::new();
        let base = b.body.first().map(|l| l.indent).unwrap_or(0);
        for l in &b.body {
            if l.indent != base {
                let at = Span { line: l.no, col: l.indent + 1, end: l.indent + 2 };
                self.err(at, tr!("字下げが、上の行とそろっていません", "The indentation does not line up with the lines above"));
                continue;
            }
            let mut c = Cur::new(l);
            let at = c.here();
            let key = if c.is_word(kw::REQUEST) && matches!(c.peek_at(1).map(|t| &t.tok), Some(Tok::Str(_))) {
                c.next();
                Ok(LabelKey::Request)
            } else if c.is_word(kw::STAGE) && matches!(c.peek_at(1).map(|t| &t.tok), Some(Tok::Word(_))) {
                c.next();
                self.ident(&mut c, tr!("ステップの名前", "a stage's name")).map(LabelKey::Stage)
            } else if c.is_word(kw::STEP) && matches!(c.peek_at(1).map(|t| &t.tok), Some(Tok::Word(_))) {
                self.err_note(at, tr!("`step` ではなく `stage` と書いてください", "Write `stage`, not `step`"), step_note());
                continue;
            } else if c.is_word(kw::WHO) && matches!(c.peek_at(1).map(|t| &t.tok), Some(Tok::Word(_))) {
                c.next();
                self.ident(&mut c, tr!("承認者の決め方の名前", "the name of a way to choose approvers")).map(LabelKey::Who)
            } else {
                self.path(&mut c).map(LabelKey::Field)
            };
            let Ok(key) = key else { continue };
            let Ok((text, _)) = self.string(&mut c, tr!("文言（文字列）", "the text, a string")) else { continue };
            if self.end(&c).is_ok() {
                entries.push(Label { key, text, span: c.since(at), line: l.no });
            }
        }
        Labels { lang, entries, line: b.head.no }
    }

    fn master(&mut self, c: &mut Cur, b: &Block) -> R<Master> {
        let name = self.name(c, tr!("マスタの名前", "the master's name"))?;
        let title = self.opt_string(c);
        self.end(c)?;
        let mut m = Master { name, title, key: None, fields: Vec::new(), data: None, line: b.head.no };
        let base = b.body.first().map(|l| l.indent).unwrap_or(0);
        let mut i = 0;
        while i < b.body.len() {
            let l = b.body[i];
            let mut j = i + 1;
            while j < b.body.len() && b.body[j].indent > base {
                j += 1;
            }
            let mut c = Cur::new(l);
            if l.indent != base {
                let at = Span { line: l.no, col: l.indent + 1, end: l.indent + 2 };
                self.err(at, tr!("字下げが、上の行とそろっていません", "The indentation does not line up with the lines above"));
            } else if c.eat_word(kw::KEY) {
                if let Ok(k) = self.ident(&mut c, tr!("行を見分ける列", "the column that tells the rows apart"))
                    && self.end(&c).is_ok()
                {
                    m.key = Some(k);
                }
            } else if c.eat_word(kw::FIELDS) {
                if self.end(&c).is_ok() {
                    m.fields = self.fields(&b.body[i + 1..j]);
                }
            } else if c.eat_word(kw::DATA) {
                if let Ok(s) = self.string(&mut c, tr!("データの置き場（文字列）", "where the data is, a string"))
                    && self.end(&c).is_ok()
                {
                    m.data = Some(s);
                }
            } else {
                let _ = self.expected::<()>(&c, tr!("`key`、`fields`、`data` のどれか", "one of `key`, `fields` and `data`"));
            }
            i = j;
        }
        let n = m.name.text.clone();
        if m.key.is_none() {
            self.err_note(
                m.name.span,
                tr!("マスタ `{n}` に `key` の行がありません", "The master `{n}` has no `key` line"),
                tr!("`key code` のように、行を見分けるフィールドを書いてください。", "Write the field that tells its rows apart, as in `key code`."),
            );
        }
        if m.fields.is_empty() {
            self.err_note(
                m.name.span,
                tr!("マスタ `{n}` にフィールドがありません", "The master `{n}` has no fields"),
                tr!("`fields` の行の下に、フィールドを字下げして書いてください。", "Write its fields indented under a `fields` line."),
            );
        }
        Ok(m)
    }
}

fn join(a: Span, b: Span) -> Span {
    if a.line == b.line { Span { line: a.line, col: a.col.min(b.col), end: a.end.max(b.end) } } else { a }
}

/// The words a rule of the ticket begins with, for a rule a `ticket` takes once; None for `on`, of
/// which a ticket takes as many as it has.
fn once_word(t: &TicketLine) -> Option<&'static str> {
    Some(match t {
        TicketLine::SelfApproval { .. } => "self approval",
        TicketLine::SameApprover { .. } => "same approver approves once",
        TicketLine::SubmittedBy { .. } => "submitted by",
        TicketLine::SendBack { .. } => "send back",
        TicketLine::Resubmit { .. } => "resubmit within",
        TicketLine::Withdraw { .. } => "withdraw until",
        TicketLine::Retract { .. } => "retract approval",
        TicketLine::Pause { .. } => "pause at most",
        TicketLine::ApproversEdit { .. } => "approvers edit",
        TicketLine::CommentRequired { .. } => kw::COMMENT_REQUIRED,
        TicketLine::On { .. } => return None,
    })
}

/// A Japanese phrase a particle follows: with a space before the particle when the phrase ends with
/// a backquote, as the suite's Japanese writes it (`` `v1` を``, `営業日の数を`).
fn before_particle(t: &str) -> String {
    if t.ends_with('`') { format!("{t} ") } else { t.to_string() }
}

/// What a file's first line says.
fn first_line_note() -> Text {
    tr!(
        "最初の行は `request <名前> v<版> \"<題>\"` のように、`request`、`route`、`organization`、`masters` のどれかで始めます。",
        "The first line starts with `request`, `route`, `organization` or `masters`, as in `request <name> v<version> \"<title>\"`."
    )
}

/// What `step` is, and what ringi writes (DESIGN 3.6).
fn step_note() -> Text {
    tr!(
        "rulec と kaname では `step` は率の型の刻み（`rate[step 1%]`）です。一つのファイルで二つの意味にならないよう、ringi はステップを `stage` と書きます。",
        "In rulec and kaname `step` is the step of a rate's type (`rate[step 1%]`); so that one file does not give the word two meanings, ringi calls a stage `stage`."
    )
}

/// The blocks of a file of a kind, in their order.
fn blocks_note(kind: FileKind) -> Text {
    match kind {
        FileKind::Request => tr!(
            "申請の種類のファイルには、`description`、`owner`、`effective from`、`use`、`form`、`route`、`who`、`every stage`、`table`、`ticket`、`claims`、`examples`、`scenario`、`labels` をこの順で書きます（最初の三つの順は問いません）。",
            "A request's file holds `description`, `owner`, `effective from`, `use`, `form`, `route`, `who`, `every stage`, `table`, `ticket`, `claims`, `examples`, `scenario` and `labels`, in this order (the first three in any order)."
        ),
        FileKind::Route => tr!(
            "共有の経路のファイルには、`description`、`effective from`、`use`、`inputs`、`route`、`who`、`every stage`、`table`、`labels` をこの順で書きます（最初の二つの順は問いません）。",
            "A shared route's file holds `description`, `effective from`, `use`, `inputs`, `route`, `who`, `every stage`, `table` and `labels`, in this order (the first two in any order)."
        ),
        FileKind::Organization => tr!(
            "組織のファイルには、`description`、`positions`、`team kinds`、`data` をこの順で書きます。",
            "An organization's file holds `description`, `positions`, `team kinds` and `data`, in this order."
        ),
        FileKind::Masters => tr!(
            "マスタのファイルには、`description`、`use organization`（人の列を組織と比べるとき）、`master` をこの順で書きます。",
            "A file of masters holds `description`, `use organization` (to check the people of its rows against the organization) and `master` blocks, in this order."
        ),
    }
}
