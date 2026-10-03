//! Reading a `.req` (DESIGN 1, PLAN B.2): the heading, the sections in their order, the lines
//! of a requirement in theirs, and the records under links and waivers. What is wrong with the
//! words and the lines is E001–E006; whether the names exist is the next stage's.

use crate::ast::*;
use crate::date::{Day, Period};
use crate::diag::Diag;
use crate::i18n::Text;
use crate::kw;
use crate::lex::{self, Line, Tok, Token};

pub struct Parsed {
    pub file: Option<ReqFile>,
    pub diags: Vec<Diag>,
}

/// Something wrong on a line: the code, the column, the message, the notes.
struct Bad {
    code: &'static str,
    col: usize,
    msg: Text,
    notes: Vec<Text>,
}

fn bad(code: &'static str, col: usize, msg: Text) -> Bad {
    Bad { code, col, msg, notes: vec![] }
}

impl Bad {
    fn note(mut self, t: Text) -> Bad {
        self.notes.push(t);
        self
    }
}

/// A cursor over the tokens of one line.
struct Cur<'a> {
    toks: &'a [Token],
    i: usize,
    end: usize,
}

impl<'a> Cur<'a> {
    fn new(line: &'a Line) -> Cur<'a> {
        let toks = match line.naming {
            Some(n) => &line.tokens[..n],
            None => &line.tokens[..],
        };
        Cur { toks, i: 0, end: line.end }
    }

    fn peek(&self) -> Option<&'a Tok> {
        self.toks.get(self.i).map(|t| &t.tok)
    }

    fn col(&self) -> usize {
        self.toks.get(self.i).map(|t| t.col).unwrap_or(self.end)
    }

    fn span(&self, line: usize) -> Span {
        Span { line, col: self.col() }
    }

    fn bump(&mut self) {
        self.i += 1;
    }

    /// What is at the cursor, for a message: `` `x` `` or the end of the line.
    fn found(&self) -> Text {
        match self.peek() {
            Some(t) => {
                let s = t.spelled();
                tr!("`{s}` があります", "found `{s}`")
            }
            None => tr!("行が終わっています", "the line ends there"),
        }
    }

    fn word(&mut self, w: &str) -> bool {
        if self.peek().is_some_and(|t| t.is_word(w)) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_word(&mut self, w: &str) -> Result<(), Bad> {
        if self.word(w) {
            Ok(())
        } else {
            let f = self.found();
            Err(bad("E002", self.col(), tr!("ここには `{w}` を書きます（{}）", "`{w}` belongs here ({})", f.ja; f.en)))
        }
    }

    /// A name: a word that is not a keyword (DESIGN 1.2).
    fn name(&mut self, what: &Text) -> Result<String, Bad> {
        match self.peek() {
            Some(Tok::Word(w)) if kw::is_reserved(w) => Err(bad(
                "E002",
                self.col(),
                tr!("`{w}` は yuen の語なので、{}の名前にできません", "`{w}` is a word of the language and cannot name {}", what.ja; what.en),
            )
            .note(tr!(
                "名前に使えないのは、行頭の語と、要件の中の語（確かめた記録の語を含む）です。",
                "A name cannot be a word that starts a line or a word of the lines of a requirement, its records included."
            ))),
            Some(Tok::Word(w)) => {
                let w = w.clone();
                self.bump();
                Ok(w)
            }
            _ => {
                let f = self.found();
                Err(bad("E002", self.col(), tr!("ここには{}の名前を書きます（{}）", "The name of {} belongs here ({})", what.ja, f.ja; what.en, f.en)))
            }
        }
    }

    fn string(&mut self, what: &Text) -> Result<String, Bad> {
        match self.peek() {
            Some(Tok::Str(s)) => {
                let s = s.clone();
                self.bump();
                Ok(s)
            }
            _ => {
                let f = self.found();
                Err(bad("E002", self.col(), tr!("ここには{}を `\"…\"` で書きます（{}）", "Write {} here, in quotes ({})", what.ja, f.ja; what.en, f.en)))
            }
        }
    }

    fn date(&mut self) -> Result<Day, Bad> {
        match self.peek() {
            Some(Tok::Date(d)) => {
                let d = *d;
                self.bump();
                Ok(d)
            }
            _ => {
                let f = self.found();
                Err(bad("E002", self.col(), tr!("ここには日付（`2026-10-03` の形）を書きます（{}）", "A date (`2026-10-03`) belongs here ({})", f.ja; f.en)))
            }
        }
    }

    /// `v<n>`, when the cursor is on one.
    fn version(&mut self) -> Result<Option<(u32, usize)>, Bad> {
        let col = self.col();
        let Some(Tok::Word(w)) = self.peek() else { return Ok(None) };
        let Some(d) = w.strip_prefix('v') else { return Ok(None) };
        if d.is_empty() || !d.chars().all(|c| c.is_ascii_digit()) {
            return Ok(None);
        }
        if d.len() > 1 && d.starts_with('0') || d.len() > 6 {
            return Err(bad("E001", col, tr!("版 `{w}` の形が崩れています。版は `v1`、`v2` のように書きます", "The version `{w}` is not written right; write `v1`, `v2`")));
        }
        let n = d.parse().unwrap_or(0);
        self.bump();
        Ok(Some((n, col)))
    }

    fn done(&self) -> Result<(), Bad> {
        match self.peek() {
            None => Ok(()),
            Some(t) => {
                let s = t.spelled();
                Err(bad("E002", self.col(), tr!("`{s}` はこの行に書けません。行はここまでです", "`{s}` does not belong on this line; the line ends before it")))
            }
        }
    }
}

/// A line's first word, if it starts with one.
fn first_word(line: &Line) -> Option<&str> {
    match line.tokens.first().map(|t| &t.tok) {
        Some(Tok::Word(w)) => Some(w.as_str()),
        _ => None,
    }
}

/// The rank of a line at the start of a file (DESIGN 1.1): the order the sections come in.
fn section_rank(w: &str) -> Option<u8> {
    match w {
        kw::DESCRIPTION => Some(1),
        kw::ROLE => Some(2),
        kw::SOURCE => Some(3),
        kw::SCOPE => Some(4),
        kw::REQUIREMENT => Some(5),
        _ => None,
    }
}

const SECTIONS: (&str, &str) = (
    "ファイルは、見出し（`requirements`）、`description`、`role`、`source`、`scope`、`requirement` の順に書きます。",
    "A file goes: the heading (`requirements`), `description`, `role`, `source`, `scope`, `requirement`.",
);

const REQ_LINES: (&str, &str) = (
    "要件の中は、`text`、`in force`、`owner`、`replaces`、`from`、`decided`、`satisfied by` と `not satisfied`、`verified by` と `not verified` の順に書きます。",
    "The lines of a requirement go: `text`, `in force`, `owner`, `replaces`, `from`, `decided`, `satisfied by` and `not satisfied`, `verified by` and `not verified`.",
);

/// The rank of a line of a requirement (DESIGN 1.5), and what it is called.
fn req_rank(c: &Cur) -> Option<(u8, &'static str)> {
    let w = |i: usize| match c.toks.get(i).map(|t| &t.tok) {
        Some(Tok::Word(w)) => w.as_str(),
        _ => "",
    };
    match (w(0), w(1)) {
        (kw::TEXT, _) => Some((0, "text")),
        (kw::IN, kw::FORCE) => Some((1, "in force")),
        (kw::OWNER, _) => Some((2, "owner")),
        (kw::REPLACES, _) => Some((3, "replaces")),
        (kw::FROM, _) => Some((4, "from")),
        (kw::DECIDED, _) => Some((5, "decided")),
        (kw::SATISFIED, kw::BY) => Some((6, "satisfied by")),
        (kw::NOT, kw::SATISFIED) => Some((6, "not satisfied")),
        (kw::VERIFIED, kw::BY) => Some((7, "verified by")),
        (kw::NOT, kw::VERIFIED) => Some((7, "not verified")),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
    /// Nothing above takes indented lines.
    None,
    /// The lines under a line that could not be read: skipped.
    Skip,
    /// The pins of a law source.
    Law(usize),
    /// The lines of a requirement.
    Req(usize),
}

/// What the last line of a requirement was, for the record that may come under it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Last {
    Other,
    From(usize),
    Link(usize),
    Waiver(usize),
}

pub fn parse(file: &str, rel: &str, src: &str) -> Parsed {
    let (lines, mut diags) = lex::lex(file, rel, src);
    let mut p = Parser { file, rel, src, diags: Vec::new() };
    let f = p.run(&lines);
    diags.append(&mut p.diags);
    diags.sort_by_key(|d| (d.line, d.col));
    let file = if diags.iter().any(|d| d.is_error()) { None } else { f };
    Parsed { file, diags }
}

struct Parser<'a> {
    file: &'a str,
    rel: &'a str,
    src: &'a str,
    diags: Vec<Diag>,
}

impl Parser<'_> {
    fn err(&mut self, code: &'static str, line: usize, col: usize, msg: Text) -> &mut Diag {
        self.diags.push(Diag::error(code, self.file, self.rel, line, col, msg).source(self.src));
        self.diags.last_mut().unwrap()
    }

    fn bad(&mut self, line: usize, b: Bad) {
        let mut d = Diag::error(b.code, self.file, self.rel, line, b.col, b.msg).source(self.src);
        d.notes = b.notes;
        self.diags.push(d);
    }

    fn run(&mut self, lines: &[Line]) -> Option<ReqFile> {
        let mut it = lines.iter().filter(|l| !l.tokens.is_empty() || l.bad);
        let Some(first) = it.next() else {
            self.err("E003", 1, 1, tr!("ファイルが空です。一行目は `requirements <名前> v1` です", "The file is empty; its first line is `requirements <name> v1`"));
            return None;
        };
        if first.bad {
            return None;
        }
        if first.indent > 0 || first_word(first) != Some(kw::REQUIREMENTS) {
            self.err("E003", first.no, first.tokens[0].col, tr!("ファイルは `requirements` の行で始めます", "A file starts with a `requirements` line")).notes.push(tr!(
                "一行目は `requirements 民法の期間 v1` のように、要件の集まりの名前と版を書きます。",
                "The first line names the set of requirements and its version, like `requirements payment_terms v1`."
            ));
            return None;
        }
        let header = {
            let mut c = Cur::new(first);
            c.bump();
            (|| -> Result<Header, Bad> {
                let span = c.span(first.no);
                let name = c.name(&tr!("ファイル", "the file"))?;
                let Some((version, _)) = c.version()? else {
                    let f = c.found();
                    return Err(bad("E002", c.col(), tr!("見出しの最後には版（`v1` のように）を書きます（{}）", "The heading ends with a version such as `v1` ({})", f.ja; f.en)));
                };
                c.done()?;
                Ok(Header { name, version, span })
            })()
        };
        let header = match header {
            Ok(h) => h,
            Err(b) => {
                self.bad(first.no, b);
                Header { name: String::new(), version: 0, span: Span { line: first.no, col: 1 } }
            }
        };
        let mut f = ReqFile { header, description: None, roles: vec![], sources: vec![], scopes: vec![], requirements: vec![] };
        let mut block = Block::None;
        let mut inner: Option<usize> = None;
        let mut last_rank = 0u8;
        let mut req_rank_now = 0u8;
        let mut last = Last::Other;
        for line in it {
            if line.bad {
                if line.indent == 0 {
                    block = Block::Skip;
                }
                last = Last::Other;
                continue;
            }
            if line.indent == 0 {
                inner = None;
                last = Last::Other;
                block = Block::None;
                let w = first_word(line).unwrap_or("");
                let col = line.tokens[0].col;
                if w == kw::REQUIREMENTS {
                    self.err("E004", line.no, col, tr!("見出しの `requirements` は、ファイルの一行目に一度だけ書きます", "The `requirements` heading comes once, on the first line of the file"));
                    block = Block::Skip;
                    continue;
                }
                let Some(rank) = section_rank(w) else {
                    let shown = line.tokens[0].tok.spelled();
                    self.err("E002", line.no, col, tr!("`{shown}` で始まる行はありません", "No line starts with `{shown}`")).notes.push(tr!("{}", "{}", SECTIONS.0; SECTIONS.1));
                    block = Block::Skip;
                    continue;
                };
                if rank < last_rank {
                    self.err("E004", line.no, col, tr!("`{w}` の行が、決まった順序より後ろにあります", "The `{w}` line is out of order")).notes.push(tr!("{}", "{}", SECTIONS.0; SECTIONS.1));
                }
                last_rank = last_rank.max(rank);
                let r = match w {
                    kw::DESCRIPTION => self.description(&mut f, line),
                    kw::ROLE => self.role(&mut f, line),
                    kw::SOURCE => self.source(&mut f, line).map(|law| {
                        if law {
                            block = Block::Law(f.sources.len() - 1);
                        }
                    }),
                    kw::SCOPE => self.scope(&mut f, line),
                    _ => self.requirement(&mut f, line).map(|()| {
                        block = Block::Req(f.requirements.len() - 1);
                        req_rank_now = 0;
                    }),
                };
                if let Err(b) = r {
                    self.bad(line.no, b);
                    block = Block::Skip;
                }
                continue;
            }
            // An indented line: a line of the block above.
            match block {
                Block::Skip => continue,
                Block::None => {
                    self.err("E005", line.no, line.indent + 1, tr!(
                        "この行は字下げされていますが、上に字下げで続く行がありません",
                        "This line is indented, but nothing above it takes indented lines"
                    ))
                    .notes
                    .push(tr!(
                        "字下げは、要件の中の行と、法令の出典の下の固定の行と、リンクの下の確かめた記録の行を表します。",
                        "Indentation marks the lines of a requirement, the pins under a law source, and the record under a link."
                    ));
                    continue;
                }
                _ => {}
            }
            let is_record = matches!(first_word(line), Some(kw::REVIEWED | kw::APPROVED));
            let i0 = *inner.get_or_insert(line.indent);
            if is_record && line.indent > i0 && matches!(block, Block::Req(_)) {
                // A record: right under the link or waiver it records (DESIGN 4.2).
                let Block::Req(ri) = block else { unreachable!() };
                let r = &mut f.requirements[ri];
                let slot = match last {
                    Last::From(i) => &mut r.from[i].record,
                    Last::Link(i) => &mut r.links[i].record,
                    Last::Waiver(i) => &mut r.waivers[i].record,
                    Last::Other => {
                        self.err("E005", line.no, line.indent + 1, tr!(
                            "確かめた記録の行が、リンクか見送りの下にありません",
                            "A record is not under a link or a waiver"
                        ))
                        .notes
                        .push(tr!(
                            "`reviewed` は `from`・`satisfied by`・`verified by` の行のすぐ下に、`approved` は `not satisfied`・`not verified` の行のすぐ下に、一段深く書きます。",
                            "`reviewed` goes right under a `from`, `satisfied by` or `verified by` line, and `approved` right under a `not satisfied` or `not verified` line, indented deeper."
                        ));
                        continue;
                    }
                };
                if slot.is_some() {
                    self.err("E004", line.no, line.indent + 1, tr!("確かめた記録は、一つのリンクに一つです", "A link has one record"))
                        .notes
                        .push(tr!("確かめ直したときは、`yuen review` が記録を書き換えます。前の記録は git の履歴に残ります。", "When it is looked at again, `yuen review` rewrites the record; the one before stays in the git history."));
                    continue;
                }
                let waiver = matches!(last, Last::Waiver(_));
                *slot = Some(RecordLine { line: line.no, indent: line.indent, parsed: record(line, waiver) });
                continue;
            }
            if line.indent != i0 {
                if is_record {
                    self.err("E005", line.no, line.indent + 1, tr!(
                        "確かめた記録の行は、リンクの行より深く字下げします",
                        "A record is indented deeper than its link"
                    ));
                } else {
                    self.err("E005", line.no, line.indent + 1, tr!(
                        "字下げが上の行とそろっていません（上は {i0} 文字、この行は {} 文字）",
                        "The indentation does not line up with the lines above ({i0} spaces there, {} here)",
                        line.indent
                    ));
                }
                last = Last::Other;
                continue;
            }
            match block {
                Block::Law(si) => {
                    if let Err(b) = self.pin(&mut f.sources[si], line) {
                        self.bad(line.no, b);
                    }
                }
                Block::Req(ri) => {
                    if is_record {
                        self.err("E005", line.no, line.indent + 1, tr!(
                            "確かめた記録の行は、リンクの行より深く字下げします",
                            "A record is indented deeper than its link"
                        ));
                        last = Last::Other;
                        continue;
                    }
                    match self.req_line(&mut f.requirements[ri], line, &mut req_rank_now) {
                        Ok(l) => last = l,
                        Err(b) => {
                            self.bad(line.no, b);
                            last = Last::Other;
                        }
                    }
                }
                _ => {}
            }
        }
        Some(f)
    }

    fn description(&mut self, f: &mut ReqFile, line: &Line) -> Result<(), Bad> {
        let mut c = Cur::new(line);
        let col = c.col();
        c.bump();
        let s = c.string(&tr!("説明", "the description"))?;
        c.done()?;
        if f.description.is_some() {
            return Err(bad("E004", col, tr!("`description` は一度だけ書けます", "`description` comes once")));
        }
        f.description = Some((s, Span { line: line.no, col }));
        Ok(())
    }

    fn role(&mut self, f: &mut ReqFile, line: &Line) -> Result<(), Bad> {
        let mut c = Cur::new(line);
        c.bump();
        let span = c.span(line.no);
        let name = c.name(&tr!("役割", "a role"))?;
        let description = match c.peek() {
            Some(Tok::Str(_)) => Some(c.string(&tr!("説明", "the description"))?),
            _ => None,
        };
        c.done()?;
        f.roles.push(RoleDecl { name, description, span });
        Ok(())
    }

    /// `source …`; true for a law, whose pins follow.
    fn source(&mut self, f: &mut ReqFile, line: &Line) -> Result<bool, Bad> {
        let mut c = Cur::new(line);
        c.bump();
        let span = c.span(line.no);
        let name = c.name(&tr!("出典", "a source"))?;
        if c.peek() != Some(&Tok::Eq) {
            let f = c.found();
            return Err(bad("E002", c.col(), tr!("出典の名前のあとには `=` を書きます（{}）", "`=` follows the name of a source ({})", f.ja; f.en)));
        }
        c.bump();
        if let Some(n) = line.naming {
            let w = crate::names::read(&line.tokens[n..], line.end).expect("a naming has a word");
            f.sources.push(SourceDecl { name, span, kind: SourceKind::Borrowed { naming: w } });
            return Ok(false);
        }
        if c.word(kw::LAW) {
            let db = if c.word(kw::ECFR) {
                LawDb::Ecfr
            } else {
                c.word(kw::EGOV);
                LawDb::Egov
            };
            let id = c.string(&tr!("法令の ID", "the id of the law"))?;
            c.expect_word(kw::ASOF)?;
            let asof = c.date()?;
            c.done()?;
            f.sources.push(SourceDecl { name, span, kind: SourceKind::Law { db, id, asof, pins: vec![] } });
            return Ok(true);
        }
        if c.word(kw::FILE) {
            let path_span = c.span(line.no);
            let path = c.string(&tr!("写しのパス", "the path of the copy"))?;
            let url = if c.word(kw::URL) { Some(c.string(&tr!("URL", "the URL"))?) } else { None };
            let pin = match c.peek() {
                Some(Tok::Sha(h)) => {
                    let h = h.clone();
                    c.bump();
                    Some(h)
                }
                _ => None,
            };
            c.done()?;
            f.sources.push(SourceDecl { name, span, kind: SourceKind::File { path, path_span, url, pin } });
            return Ok(false);
        }
        // `=` and nothing after it: the lexer did not start a naming.
        let fd = c.found();
        Err(bad("E002", c.col(), tr!(
            "`=` のあとには `law`、`file`、借りる出典の名指し（`koyomi \"x.cal\" source 民法`）のどれかを書きます（{}）",
            "`law`, `file`, or the naming of a source to borrow (`koyomi \"x.cal\" source 民法`) follows `=` ({})",
            fd.ja;
            fd.en
        )))
    }

    fn pin(&mut self, s: &mut SourceDecl, line: &Line) -> Result<(), Bad> {
        let mut c = Cur::new(line);
        let span = c.span(line.no);
        let fragment = match c.peek() {
            Some(Tok::Word(w)) => w.clone(),
            Some(Tok::Str(w)) => w.clone(),
            _ => {
                let f = c.found();
                return Err(bad("E002", c.col(), tr!("固定の行は `<条> sha256:<16 桁>` です（{}）", "A pin line is `<article> sha256:<16 digits>` ({})", f.ja; f.en)));
            }
        };
        c.bump();
        let pin = match c.peek() {
            Some(Tok::Sha(h)) => {
                let h = h.clone();
                c.bump();
                Some(h)
            }
            _ => None,
        };
        c.done()?;
        if let SourceKind::Law { pins, .. } = &mut s.kind {
            pins.push(PinLine { fragment, span, pin });
        }
        Ok(())
    }

    fn scope(&mut self, f: &mut ReqFile, line: &Line) -> Result<(), Bad> {
        let span = Span { line: line.no, col: line.tokens[0].col };
        let n = line.naming.unwrap_or(line.tokens.len());
        let Some(w) = crate::names::read(&line.tokens[n..], line.end) else {
            return Err(bad("E002", line.end, tr!("`scope` のあとに名指しを書きます（`scope file \"src/\"` のように）", "A naming follows `scope` (like `scope file \"src/\"`)")));
        };
        f.scopes.push(ScopeDecl { naming: w, span });
        Ok(())
    }

    fn requirement(&mut self, f: &mut ReqFile, line: &Line) -> Result<(), Bad> {
        let mut c = Cur::new(line);
        c.bump();
        let span = c.span(line.no);
        let name = c.name(&tr!("要件", "a requirement"))?;
        let alias = if c.peek() == Some(&Tok::LParen) {
            c.bump();
            let col = c.col();
            let a = match c.peek() {
                Some(Tok::Word(w)) => w.clone(),
                _ => {
                    let f = c.found();
                    return Err(bad("E001", col, tr!("丸括弧の中には別名を書きます（{}）", "The alias goes in the parentheses ({})", f.ja; f.en)));
                }
            };
            c.bump();
            if !is_alias(&a) {
                return Err(bad("E001", col, tr!("別名 `{a}` の形が崩れています。別名は ASCII の小文字で始め、小文字・数字・`_` で書きます", "The alias `{a}` is not written right: it starts with a lowercase ASCII letter and goes on in lowercase letters, digits and `_`")));
            }
            if c.peek() != Some(&Tok::RParen) {
                let f = c.found();
                return Err(bad("E001", c.col(), tr!("別名のあとの `)` がありません（{}）", "The `)` after the alias is missing ({})", f.ja; f.en)));
            }
            c.bump();
            Some((a, Span { line: line.no, col }))
        } else {
            None
        };
        let version = c.version()?.map(|(n, col)| (n, Span { line: line.no, col }));
        c.done()?;
        f.requirements.push(ReqDecl {
            name,
            alias,
            version,
            span,
            text: None,
            in_force: None,
            owner: None,
            replaces: vec![],
            from: vec![],
            decided: vec![],
            links: vec![],
            waivers: vec![],
        });
        Ok(())
    }

    fn req_line(&mut self, r: &mut ReqDecl, line: &Line, rank_now: &mut u8) -> Result<Last, Bad> {
        let mut c = Cur::new(line);
        let col = c.col();
        let Some((rank, what)) = req_rank(&c) else {
            let shown = line.tokens[0].tok.spelled();
            return Err(bad("E002", col, tr!("要件の中に `{shown}` で始まる行はありません", "No line of a requirement starts with `{shown}`")).note(tr!("{}", "{}", REQ_LINES.0; REQ_LINES.1)));
        };
        if rank < *rank_now {
            return Err(bad("E004", col, tr!("`{what}` の行が、決まった順序より後ろにあります", "The `{what}` line is out of order")).note(tr!("{}", "{}", REQ_LINES.0; REQ_LINES.1)));
        }
        let once = |taken: bool| -> Result<(), Bad> {
            if taken { Err(bad("E004", col, tr!("要件の `{what}` は一つだけです", "A requirement has one `{what}`"))) } else { Ok(()) }
        };
        *rank_now = rank;
        let sp = Span { line: line.no, col };
        match what {
            "text" => {
                once(r.text.is_some())?;
                c.bump();
                let s = c.string(&tr!("要件の文", "the text of the requirement"))?;
                c.done()?;
                r.text = Some((s, sp));
            }
            "in force" => {
                once(r.in_force.is_some())?;
                c.bump();
                c.bump();
                let p = period(&mut c)?;
                c.done()?;
                r.in_force = Some((p, sp));
            }
            "owner" => {
                once(r.owner.is_some())?;
                c.bump();
                let s = c.span(line.no);
                let n = c.name(&tr!("役割", "a role"))?;
                c.done()?;
                r.owner = Some((n, s));
            }
            "replaces" => {
                c.bump();
                let rr = req_ref(&mut c, line.no)?;
                c.done()?;
                r.replaces.push(rr);
            }
            "from" => {
                c.bump();
                let what = if c.peek() == Some(&Tok::At) {
                    c.bump();
                    let source_span = c.span(line.no);
                    let source = c.name(&tr!("出典", "a source"))?;
                    let mut fragments = Vec::new();
                    loop {
                        let fs = c.span(line.no);
                        match c.peek() {
                            Some(Tok::Word(w)) | Some(Tok::Str(w)) => {
                                fragments.push((w.clone(), fs));
                                c.bump();
                            }
                            None if fragments.is_empty() => break,
                            _ => {
                                let f = c.found();
                                return Err(bad("E002", c.col(), tr!("ここには引く条を書きます（{}）", "An article belongs here ({})", f.ja; f.en)));
                            }
                        }
                        if c.peek() == Some(&Tok::Comma) {
                            c.bump();
                            continue;
                        }
                        break;
                    }
                    FromWhat::Cite { source, source_span, fragments }
                } else {
                    FromWhat::Req(req_ref(&mut c, line.no)?)
                };
                c.done()?;
                r.from.push(FromLine { span: sp, what, record: None });
                return Ok(Last::From(r.from.len() - 1));
            }
            "decided" => {
                c.bump();
                let date = c.date()?;
                c.expect_word(kw::BY)?;
                let bs = c.span(line.no);
                let by = c.name(&tr!("役割", "a role"))?;
                let why = c.string(&tr!("理由", "the reason"))?;
                c.done()?;
                r.decided.push(Decided { date, by: (by, bs), why, span: sp });
            }
            "satisfied by" | "verified by" => {
                let side = if what == "satisfied by" { Side::Satisfied } else { Side::Verified };
                let n = line.naming.unwrap_or(line.tokens.len());
                let Some(w) = crate::names::read(&line.tokens[n..], line.end) else {
                    return Err(bad("E002", line.end, tr!("`{what}` のあとに成果物の名指しを書きます", "The naming of an artifact follows `{what}`")).note(tr!(
                        "名指しは `<ツール> \"<パス>\" [<種類> <名前>]` の形です（`file \"src/app.py\"`、`koyomi \"支払条件.cal\" date 支払日`）。",
                        "A naming is `<tool> \"<path>\" [<kind> <name>]` (`file \"src/app.py\"`, `koyomi \"terms.cal\" date pay_day`)."
                    )));
                };
                r.links.push(LinkLine { span: sp, side, naming: w, record: None });
                return Ok(Last::Link(r.links.len() - 1));
            }
            _ => {
                // `not satisfied "<reason>"`, `not verified "<reason>"`.
                let side = if what == "not satisfied" { Side::Satisfied } else { Side::Verified };
                c.bump();
                c.bump();
                let why = c.string(&tr!("見送る理由", "the reason for the waiver"))?;
                c.done()?;
                r.waivers.push(Waiver { span: sp, side, why, record: None });
                return Ok(Last::Waiver(r.waivers.len() - 1));
            }
        }
        Ok(Last::Other)
    }
}

/// Whether a word is an alias: `[a-z][a-z0-9_]*` (DESIGN 1.2).
pub fn is_alias(a: &str) -> bool {
    let mut cs = a.chars();
    cs.next().is_some_and(|c| c.is_ascii_lowercase()) && cs.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn req_ref(c: &mut Cur, line: usize) -> Result<ReqRef, Bad> {
    let span = c.span(line);
    let name = c.name(&tr!("要件", "a requirement"))?;
    let version = c.version()?.map(|(n, _)| n);
    Ok(ReqRef { name, version, span })
}

/// `<date>..<date>`, `<date>..`, `..<date>` (DESIGN 1.5).
fn period(c: &mut Cur) -> Result<Period, Bad> {
    let col = c.col();
    let from = match c.peek() {
        Some(Tok::Date(d)) => {
            let d = *d;
            c.bump();
            Some(d)
        }
        _ => None,
    };
    if c.peek() != Some(&Tok::DotDot) {
        let f = c.found();
        return Err(bad("E002", c.col(), tr!(
            "期間は `2026-10-01..2027-03-31` のように `..` でつなぎます。片方は省けます（{}）",
            "A period joins two dates with `..`, as in `2026-10-01..2027-03-31`; either may be left out ({})",
            f.ja;
            f.en
        )));
    }
    c.bump();
    let to = match c.peek() {
        Some(Tok::Date(d)) => {
            let d = *d;
            c.bump();
            Some(d)
        }
        _ => None,
    };
    if from.is_none() && to.is_none() {
        return Err(bad("E002", col, tr!("期間の始まりと終わりの両方を省くことはできません", "A period cannot leave out both its start and its end")));
    }
    if let (Some(a), Some(b)) = (from, to)
        && b < a
    {
        return Err(bad("E006", col, tr!("期間の終わり {b} が、始まり {a} より前です", "The period ends on {b}, before it starts on {a}")));
    }
    Ok(Period { from, to })
}

/// A record line (DESIGN 4.2): what it says, or what is wrong with it (E305 at stage 6).
fn record(line: &Line, waiver: bool) -> Result<Record, (usize, Text)> {
    let mut c = Cur::new(line);
    let col = c.col();
    let word = first_word(line).unwrap_or("");
    if waiver && word == kw::REVIEWED {
        return Err((col, tr!("見送りの下の記録は `approved` です（`reviewed` はリンクの下）", "Under a waiver the record is `approved` (`reviewed` is under a link)")));
    }
    if !waiver && word == kw::APPROVED {
        return Err((col, tr!("リンクの下の記録は `reviewed` です（`approved` は見送りの下）", "Under a link the record is `reviewed` (`approved` is under a waiver)")));
    }
    c.bump();
    let fail = |c: &Cur, what: Text| -> (usize, Text) {
        let f = c.found();
        // A word of the language is set off with spaces in the Japanese: `ここには `->` を`.
        let w = if what.ja.starts_with('`') { format!(" {} ", what.ja) } else { what.ja.clone() };
        (c.col(), tr!("確かめた記録の形が崩れています。ここには{}を書きます（{}）", "The record is not written right: {} belongs here ({})", w, f.ja; what.en, f.en))
    };
    let date = match c.peek() {
        Some(Tok::Date(d)) => {
            let d = *d;
            c.bump();
            d
        }
        _ => return Err(fail(&c, tr!("日付", "the date"))),
    };
    if !c.word(kw::BY) {
        return Err(fail(&c, tr!("`by`", "`by`")));
    }
    let by_span = Span { line: line.no, col: c.col() };
    let by = match c.peek() {
        Some(Tok::Word(w)) => {
            let w = w.clone();
            c.bump();
            w
        }
        _ => return Err(fail(&c, tr!("役割", "a role"))),
    };
    let mut up = Vec::new();
    loop {
        match c.peek() {
            Some(Tok::Sha(h)) => {
                up.push(h.clone());
                c.bump();
            }
            _ => return Err(fail(&c, tr!("`sha256:` のハッシュ", "a `sha256:` hash"))),
        }
        if !waiver && c.peek() == Some(&Tok::Comma) {
            c.bump();
            continue;
        }
        break;
    }
    let down = if waiver {
        None
    } else {
        if c.peek() != Some(&Tok::Arrow) {
            return Err(fail(&c, tr!("`->`", "`->`")));
        }
        c.bump();
        match c.peek() {
            Some(Tok::Sha(h)) => {
                let h = h.clone();
                c.bump();
                Some(h)
            }
            _ => return Err(fail(&c, tr!("`sha256:` のハッシュ", "a `sha256:` hash"))),
        }
    };
    if let Some(t) = c.peek() {
        let s = t.spelled();
        return Err((c.col(), tr!("確かめた記録の形が崩れています。`{s}` は余分です", "The record is not written right: `{s}` is one word too many")));
    }
    Ok(Record { date, by, by_span, up, down })
}
