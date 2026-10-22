//! From the lines of tokens to the syntax tree (DESIGN 2.1, 2.3).
//!
//! A module is read a line at a time. The lines at the left margin are its parts, in a fixed order:
//! the head (`module`), `description`, the `use` lines, the declarations (`enum`, `record`, `fn`,
//! in any order), `examples` (E003 when a part is out of its place). A block is the lines indented
//! under its head, every one at the same indentation (E004 when one is not), and answers with its
//! last line (E111 when it has none). An expression is read from the tokens of one line by
//! precedence: `or`, `and`, `not`, one comparison (a second is E001), `in`, `+ -`, `* /`, a minus,
//! then the dots, calls and records after a value.
//!
//! What cannot be read is said where it is (E001–E004, E111, E602), a line at a time; a module with
//! any of these is not read further.

use crate::ast::*;
use crate::diag::Diag;
use crate::kw;
use crate::lex::{self, Line, Tok, Token};
use ritsu_base::text::Text;

/// What the parser made of a file: the module, when nothing stopped it, and what it said.
pub struct Parsed {
    pub module: Option<Module>,
    pub diags: Vec<Diag>,
}

/// Read `src`, the text of the file `file`.
pub fn parse(file: &str, src: &str) -> Parsed {
    let (lines, diags) = lex::lex(file, src);
    if !diags.is_empty() {
        return Parsed { module: None, diags };
    }
    let mut p = P { file, src, lines, i: 0, diags: Vec::new(), ids: 0, targs: Default::default(), ltypes: Default::default() };
    let m = p.module();
    let module = if p.diags.is_empty() { m } else { None };
    Parsed { module, diags: p.diags }
}

struct P<'a> {
    file: &'a str,
    src: &'a str,
    lines: Vec<Line>,
    i: usize,
    diags: Vec<Diag>,
    ids: usize,
    /// The type arguments written after the name a call calls, by the call's id (DESIGN 26.1).
    targs: std::collections::BTreeMap<usize, Vec<TypeRef>>,
    /// The types written on a lambda's parameters, by the lambda's id (DESIGN 27.1).
    ltypes: std::collections::BTreeMap<usize, Vec<Option<TypeRef>>>,
}

/// The tokens of one line, read from the left.
struct Cur<'t> {
    toks: &'t [Token],
    k: usize,
    /// Where the line is, for an error at its end.
    end: Span,
}

impl<'t> Cur<'t> {
    fn new(toks: &'t [Token], line: usize) -> Cur<'t> {
        let end = toks.last().map(|t| Span { line: t.span.line, col: t.span.end, end: t.span.end }).unwrap_or(Span { line, col: 1, end: 1 });
        Cur { toks, k: 0, end }
    }

    fn peek(&self) -> Option<&'t Tok> {
        self.toks.get(self.k).map(|t| &t.tok)
    }

    fn peek_at(&self, n: usize) -> Option<&'t Tok> {
        self.toks.get(self.k + n).map(|t| &t.tok)
    }

    fn span(&self) -> Span {
        self.toks.get(self.k).map(|t| t.span).unwrap_or(self.end)
    }

    fn prev_span(&self) -> Span {
        if self.k == 0 { self.span() } else { self.toks[self.k - 1].span }
    }

    fn next(&mut self) -> Option<&'t Token> {
        let t = self.toks.get(self.k);
        if t.is_some() {
            self.k += 1;
        }
        t
    }

    fn at_end(&self) -> bool {
        self.k >= self.toks.len()
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.k += 1;
            true
        } else {
            false
        }
    }

    fn eat_word(&mut self, w: &str) -> bool {
        if self.peek().is_some_and(|t| t.is_word(w)) {
            self.k += 1;
            true
        } else {
            false
        }
    }

    /// Whether `t` is ahead at the depth of brackets the cursor is at, before the line ends.
    fn ahead_at_depth(&self, want: &dyn Fn(&Tok) -> bool) -> bool {
        let mut depth = 0i32;
        for t in &self.toks[self.k..] {
            match t.tok {
                Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
                Tok::RParen | Tok::RBracket | Tok::RBrace => depth -= 1,
                _ if depth == 0 && want(&t.tok) => return true,
                _ => {}
            }
        }
        false
    }
}

/// A part of a module failed to read: the error is said already.
type R<T> = Option<T>;

/// The parts of a module, in their order (E003 when one comes after a later one).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
    Head,
    Description,
    Uses,
    Decls,
    Examples,
}

fn shown_tok(t: Option<&Tok>) -> String {
    t.map(Tok::shown).unwrap_or_default()
}

impl<'a> P<'a> {
    fn err(&mut self, code: &'static str, at: Span, msg: Text) -> &mut Diag {
        self.diags.push(Diag::error(code, self.file, at.line, at.col, msg).source(self.src));
        self.diags.last_mut().expect("just pushed")
    }

    fn id(&mut self) -> usize {
        self.ids += 1;
        self.ids - 1
    }

    fn mk(&mut self, span: Span, kind: ExprKind) -> Expr {
        Expr { id: self.id(), span, kind }
    }

    /// An error at the token the cursor is at: what was expected there.
    fn expected(&mut self, c: &Cur, what: Text) {
        let at = c.span();
        let got = shown_tok(c.peek());
        let msg = if got.is_empty() {
            tr!("行が終わっています。ここには{}が要ります", "The line ends where {} is needed", what.ja; what.en)
        } else {
            tr!("`{got}` を読めません。ここには{}が要ります", "`{got}` cannot be read here: {} is needed", what.ja; what.en)
        };
        self.err("E001", at, msg);
    }

    fn line_span(&self, i: usize) -> Span {
        let l = &self.lines[i];
        let first = l.tokens.first().map(|t| t.span).unwrap_or(Span { line: l.no, col: 1, end: 1 });
        Span { line: l.no, col: first.col, end: first.end }
    }

    /// The lines under a line that was not read (E003 said why): passed over, as noise.
    fn skip_body(&mut self) {
        while self.i < self.lines.len() && self.lines[self.i].indent > 0 {
            self.i += 1;
        }
    }

    /// Lines indented under nothing, from `self.i` up to the next line at the left margin: E004.
    fn stray(&mut self) {
        while self.i < self.lines.len() && self.lines[self.i].indent > 0 {
            let at = self.line_span(self.i);
            self.err("E004", at, tr!("この行は字下げされていますが、上に、この行を下に置ける行がありません", "This line is indented, and no line above takes it"));
            self.i += 1;
        }
    }

    // ── The module ──────────────────────────────────────────────────────────────

    fn module(&mut self) -> R<Module> {
        let Some(first) = self.lines.first().cloned() else {
            self.err("E003", Span { line: 1, col: 1, end: 1 }, tr!("ファイルが空です。一行目は `module <名前> v1` です", "The file is empty; its first line is `module <name> v1`"));
            return None;
        };
        if first.indent != 0 || !first.tokens[0].tok.is_word(kw::MODULE) {
            let at = self.line_span(0);
            self.err("E003", at, tr!("一行目は `module <名前> v1` です", "The first line is `module <name> v1`")).notes.push(tr!(
                "モジュールの名前と、人のための版の印を書きます（`module late_payment v1`）。名前が ASCII でなければ、丸括弧で別名を付けます（`module 支払いの遅れ(late_payment_ja) v1`）。",
                "It names the module and gives a version for people (`module late_payment v1`); a name that is not ASCII takes an alias in parentheses (`module 支払いの遅れ(late_payment_ja) v1`)."
            ));
            return None;
        }
        self.i = 1;
        let mut c = Cur::new(&first.tokens, first.no);
        c.next();
        let name = self.decl_name(&mut c, tr!("モジュールの名前", "the module's name"))?;
        let version = match c.next().map(|t| &t.tok) {
            Some(Tok::Word(v)) if v.len() > 1 && v.starts_with('v') && v[1..].chars().all(|d| d.is_ascii_digit()) => v.clone(),
            _ => {
                let at = c.prev_span();
                self.err("E003", at, tr!("モジュールの名前のあとに、版の印 `v1` を書いてください", "Write the version, `v1`, after the module's name"));
                return None;
            }
        };
        self.line_end(&c)?;
        let mut m = Module { name, version, description: None, uses: Vec::new(), enums: Vec::new(), records: Vec::new(), fns: Vec::new(), examples: Vec::new(), exprs: 0, targs: Default::default(), ltypes: Default::default() };
        let mut stage = Stage::Head;
        let mut examples_at: Option<usize> = None;
        while self.i < self.lines.len() {
            if self.lines[self.i].indent > 0 {
                self.stray();
                continue;
            }
            let mut line = self.lines[self.i].clone();
            let mut head = match &line.tokens[0].tok {
                Tok::Word(w) => w.clone(),
                t => {
                    let (at, t) = (line.tokens[0].span, t.shown());
                    self.err("E003", at, tr!("`{t}` で始まる行は、モジュールの中に書けません", "A line of the module cannot start with `{t}`"));
                    self.i += 1;
                    self.skip_body();
                    continue;
                }
            };
            let at = line.tokens[0].span;
            // `private fn …`, `private record …`, `private enum …` (DESIGN 28.2): the declaration after the
            // word, which only this module uses
            let mut private = false;
            if head == kw::PRIVATE {
                match line.tokens.get(1).map(|t| &t.tok) {
                    Some(Tok::Word(w)) if w == kw::FN || w == kw::RECORD || w == kw::ENUM => {
                        head = w.clone();
                        line.tokens.remove(0);
                        private = true;
                    }
                    _ => {
                        let at = line.tokens.get(1).map(|t| t.span).unwrap_or(at);
                        self.err("E003", at, tr!("`private` のあとには `fn`、`record`、`enum` を書きます", "`private` is followed by `fn`, `record` or `enum`"));
                        self.i += 1;
                        self.skip_body();
                        continue;
                    }
                }
            }
            let want = match head.as_str() {
                kw::DESCRIPTION => Stage::Description,
                kw::USE => Stage::Uses,
                kw::ENUM | kw::RECORD | kw::FN => Stage::Decls,
                kw::EXAMPLES => Stage::Examples,
                kw::MODULE => {
                    self.err("E003", at, tr!("`module` の行は一つだけです", "There is one `module` line"));
                    self.i += 1;
                    self.skip_body();
                    continue;
                }
                _ => {
                    self.err("E003", at, tr!("`{head}` で始まる行は、モジュールの中に書けません", "A line of the module cannot start with `{head}`")).notes.push(tr!(
                        "左端の行は `description`、`use`、`enum`、`record`、`fn`、`examples` のどれかで始まります。",
                        "A line at the left margin starts with `description`, `use`, `enum`, `record`, `fn` or `examples`."
                    ));
                    self.i += 1;
                    self.skip_body();
                    continue;
                }
            };
            if want < stage || (want == stage && matches!(want, Stage::Description | Stage::Examples)) {
                let msg = match (want, stage) {
                    (Stage::Description, Stage::Description) => tr!("`description` の行は一つだけです", "There is one `description` line"),
                    (Stage::Description, _) => tr!("`description` の行は、`module` の行のすぐあとに書いてください", "The `description` line comes right after the `module` line"),
                    (Stage::Uses, _) => tr!("`use` の行は、宣言（`enum`、`record`、`fn`）より前に書いてください", "The `use` lines come before the declarations (`enum`, `record`, `fn`)"),
                    (Stage::Examples, Stage::Examples) => tr!("`examples` は一つだけです。例は全部その下に並べてください", "There is one `examples`; put every example under it"),
                    _ => tr!("宣言（`enum`、`record`、`fn`）は、`examples` より前に書いてください", "The declarations (`enum`, `record`, `fn`) come before `examples`"),
                };
                self.err("E003", at, msg);
                self.i += 1;
                self.skip_body();
                continue;
            }
            stage = want;
            match head.as_str() {
                kw::DESCRIPTION => {
                    let mut c = Cur::new(&line.tokens, line.no);
                    c.next();
                    self.i += 1;
                    if let Some(d) = self.description(&mut c) {
                        m.description = Some(d);
                    }
                    self.stray();
                }
                kw::USE => {
                    self.i += 1;
                    if let Some(u) = self.use_line(&line) {
                        m.uses.push(u);
                    }
                    self.stray();
                }
                kw::ENUM => {
                    self.i += 1;
                    if let Some(e) = self.enum_decl(&line) {
                        m.enums.push(EnumDecl { private, ..e });
                    }
                    self.stray();
                }
                kw::RECORD => {
                    self.i += 1;
                    if let Some(r) = self.record(&line) {
                        m.records.push(RecordDecl { private, ..r });
                    }
                }
                kw::FN => {
                    self.i += 1;
                    if let Some(f) = self.function(&line) {
                        m.fns.push(FnDecl { private, ..f });
                    }
                }
                _ => {
                    self.i += 1;
                    if examples_at.is_none() {
                        examples_at = Some(line.no);
                    }
                    if line.tokens.len() > 1 {
                        let at = line.tokens[1].span;
                        self.err(
                            "E003",
                            at,
                            tr!("`examples` の行には何も続けません。例はその下に字下げして書いてください", "Nothing follows `examples` on its line; write the examples indented under it"),
                        );
                    }
                    m.examples = self.examples();
                }
            }
        }
        m.exprs = self.ids;
        m.targs = std::mem::take(&mut self.targs);
        m.ltypes = std::mem::take(&mut self.ltypes);
        Some(m)
    }

    /// The rest of a line must be empty.
    fn line_end(&mut self, c: &Cur) -> R<()> {
        if c.at_end() {
            return Some(());
        }
        let (at, t) = (c.span(), shown_tok(c.peek()));
        self.err("E001", at, tr!("`{t}` を読めません。行はここで終わります", "`{t}` cannot be read: the line ends here"));
        None
    }

    fn description(&mut self, c: &mut Cur) -> R<(String, Span)> {
        match c.next() {
            Some(Token { tok: Tok::Str(s), span }) => {
                let d = (s.clone(), *span);
                self.line_end(c)?;
                Some(d)
            }
            _ => {
                let at = c.prev_span();
                self.err("E003", at, tr!("`description` のあとには、説明を `\"…\"` で書いてください", "After `description`, write the description in `\"…\"`"));
                None
            }
        }
    }

    /// A name and the alias in parentheses after it, when one is written and `then` does not say
    /// the parentheses are something else: `請求日(invoiced)`.
    fn decl_name(&mut self, c: &mut Cur, what: Text) -> R<Name> {
        let (text, span) = match c.next() {
            Some(Token { tok: Tok::Word(w), span }) => (w.clone(), *span),
            _ => {
                c.k = c.k.saturating_sub(1);
                self.expected(c, what);
                return None;
            }
        };
        if kw::is_reserved(&text) {
            self.err("E002", span, tr!("`{text}` は予約語なので、名前にできません", "`{text}` is a keyword and cannot be a name")).notes.push(reserved_note());
            return None;
        }
        let mut name = Name { text, alias: None, span };
        // `(alias)` right after the name: a word alone in parentheses
        if c.peek() == Some(&Tok::LParen) && matches!(c.peek_at(1), Some(Tok::Word(_))) && c.peek_at(2) == Some(&Tok::RParen) {
            c.next();
            let t = c.next().expect("a word");
            let Tok::Word(a) = &t.tok else { unreachable!() };
            name.alias = Some((a.clone(), t.span));
            c.next();
        }
        Some(name)
    }

    /// The type parameters after a declared name, when `[` comes there (DESIGN 26.1): `[T]`, `[K: key,
    /// V]`, `[要素(T)]`.
    fn type_params(&mut self, c: &mut Cur) -> R<Vec<TypeParam>> {
        let mut out = Vec::new();
        if !c.eat(&Tok::LBracket) {
            return Some(out);
        }
        loop {
            let name = self.decl_name(c, tr!("型の引数の名前", "the name of a type parameter"))?;
            let mut key = false;
            if c.eat(&Tok::Colon) {
                if !c.eat_word(kw::KEY) {
                    self.expected(c, tr!("`key`（鍵にできる型だけをとる型の引数）", "`key` (a type parameter of the types that can be keys)"));
                    return None;
                }
                key = true;
            }
            out.push(TypeParam { name, key });
            if c.eat(&Tok::Comma) {
                continue;
            }
            if c.eat(&Tok::RBracket) {
                return Some(out);
            }
            self.expected(c, tr!("`,` か `]`", "`,` or `]`"));
            return None;
        }
    }

    fn use_line(&mut self, line: &Line) -> R<Use> {
        let mut c = Cur::new(&line.tokens, line.no);
        c.next();
        let kind = match c.next().map(|t| &t.tok) {
            Some(Tok::Word(w)) if w == kw::RULE => UseKind::Rule,
            Some(Tok::Word(w)) if w == kw::DATES => UseKind::Dates,
            Some(Tok::Word(w)) if w == kw::BOOK => UseKind::Book,
            Some(Tok::Word(w)) if w == kw::MODULE => UseKind::Module,
            _ => {
                let at = c.prev_span();
                self.err(
                    "E003",
                    at,
                    tr!("`use` のあとには、読むものの種類 `rule`、`dates`、`book`、`module` のどれかを書いてください", "After `use`, write what it reads: `rule`, `dates`, `book` or `module`"),
                )
                .notes
                .push(tr!(
                    "`use rule charges from \"rules/charges.rule\"` のように、種類、モジュールの中での名前、`from`、ファイルのパスの順に書きます。",
                    "Write the kind, the name it has in the module, `from` and the path: `use rule charges from \"rules/charges.rule\"`."
                ));
                return None;
            }
        };
        let (name, name_span) = match c.next() {
            Some(Token { tok: Tok::Word(w), span }) => (w.clone(), *span),
            _ => {
                c.k -= 1;
                self.expected(&c, tr!("モジュールの中での名前", "the name it has in the module"));
                return None;
            }
        };
        if kw::is_reserved(&name) {
            self.err("E002", name_span, tr!("`{name}` は予約語なので、名前にできません", "`{name}` is a keyword and cannot be a name")).notes.push(reserved_note());
            return None;
        }
        if !c.eat_word(kw::FROM) {
            self.expected(&c, tr!("`from`", "`from`"));
            return None;
        }
        let (path, path_span) = match c.next() {
            Some(Token { tok: Tok::Str(s), span }) => (s.clone(), *span),
            _ => {
                c.k -= 1;
                self.expected(&c, tr!("ファイルのパス（`\"…\"`）", "the file's path (`\"…\"`)"));
                return None;
            }
        };
        self.line_end(&c)?;
        Some(Use { kind, name, name_span, path, path_span, line: line.no })
    }

    /// An enum: on one line (`enum tier = basic < silver < gold`), or with its values under it, a line
    /// each, a value's fields under the value (DESIGN 23.1).
    fn enum_decl(&mut self, line: &Line) -> R<EnumDecl> {
        let mut c = Cur::new(&line.tokens, line.no);
        c.next();
        let name = self.decl_name(&mut c, tr!("列挙の名前", "the enum's name"))?;
        let tparams = self.type_params(&mut c)?;
        if c.at_end() {
            return self.enum_listed(name, tparams, line);
        }
        if !c.eat(&Tok::Eq) {
            self.expected(&c, tr!("`=` と値か、行の終わり（値を下に一行ずつ書くとき）", "`=` and the values, or the end of the line (for the values written under it, a line each)"));
            return None;
        }
        let mut values = Vec::new();
        let mut seps: Vec<(bool, Span)> = Vec::new();
        loop {
            if let Some(Tok::Word(w)) = c.peek()
                && (w == "default" || w == "open")
            {
                let (at, w) = (c.span(), w.clone());
                self.err("E003", at, tr!("列挙に `{w}` は書けません", "An enum of kaname takes no `{w}`")).notes.push(tr!(
                    "`default` と `open` は rulec の表と寄与のための印です。kaname には表も寄与も無いので、意味を持ちません。",
                    "`default` and `open` are marks for rulec's tables and contributions; kaname has neither, so they would mean nothing."
                ));
                return None;
            }
            let v = self.decl_name(&mut c, tr!("値の名前", "a value's name"))?;
            let label = match c.peek() {
                Some(Tok::Str(s)) => {
                    let s = s.clone();
                    c.next();
                    Some(s)
                }
                _ => None,
            };
            values.push(EnumValueDecl { name: v, label, fields: Vec::new(), line: line.no });
            match c.peek() {
                None => break,
                Some(Tok::Pipe) => seps.push((false, c.span())),
                Some(Tok::Lt) => seps.push((true, c.span())),
                Some(Tok::Word(w)) if w == "default" || w == "open" => continue,
                _ => {
                    self.expected(&c, tr!("`|` か `<` と次の値", "`|` or `<` and the next value"));
                    return None;
                }
            }
            c.next();
        }
        let ordered = seps.first().is_some_and(|(o, _)| *o);
        if let Some((_, at)) = seps.iter().find(|(o, _)| *o != ordered) {
            self.err("E003", *at, tr!("列挙の値の区切りに、`|` と `<` が混ざっています", "The values of the enum are divided by both `|` and `<`")).notes.push(tr!(
                "順序のある列挙は全部を `<` で、順序の無い列挙は全部を `|` で区切ってください（rulec と同じです）。",
                "An ordered enum divides every value with `<`, and one with no order every value with `|`, as rulec's do."
            ));
            return None;
        }
        Some(EnumDecl { name, private: false, tparams, ordered, listed: false, values, line: line.no })
    }

    /// The values of an enum written under it, a line each, and the fields under each value: the
    /// lines from `self.i` up to the next line at the left margin.
    fn enum_listed(&mut self, name: Name, tparams: Vec<TypeParam>, line: &Line) -> R<EnumDecl> {
        let end = self.body_end();
        if self.i == end {
            let at = name.span;
            self.err("E003", at, tr!("列挙に、値が一つもありません", "The enum has no value")).notes.push(tr!(
                "値は、`enum` の行に `= a | b` と書くか、`enum` の行の下に字下げして一行に一つずつ書きます。",
                "Write its values on the `enum` line (`= a | b`), or indented under it, one a line."
            ));
            return None;
        }
        let indent = self.lines[self.i].indent;
        let mut values: Vec<EnumValueDecl> = Vec::new();
        let mut failed = false;
        while self.i < end {
            let l = self.lines[self.i].clone();
            self.i += 1;
            if l.indent != indent {
                let at = self.line_span(self.i - 1);
                self.err("E004", at, tr!("列挙の値の字下げがそろっていません（{indent} 文字のはずが {} 文字）", "The values of the enum are not indented alike ({} spaces where the first has {indent})", l.indent));
                failed = true;
                continue;
            }
            let mut c = Cur::new(&l.tokens, l.no);
            if let Some(Tok::Word(w)) = c.peek()
                && (w == "default" || w == "open")
            {
                let (at, w) = (c.span(), w.clone());
                self.err("E003", at, tr!("列挙に `{w}` は書けません", "An enum of kaname takes no `{w}`")).notes.push(tr!(
                    "`default` と `open` は rulec の表と寄与のための印です。kaname には表も寄与も無いので、意味を持ちません。",
                    "`default` and `open` are marks for rulec's tables and contributions; kaname has neither, so they would mean nothing."
                ));
                self.skip_rest(indent, end);
                failed = true;
                continue;
            }
            let Some(v) = self.decl_name(&mut c, tr!("値の名前", "a value's name")) else {
                self.skip_rest(indent, end);
                failed = true;
                continue;
            };
            let label = match c.peek() {
                Some(Tok::Str(x)) => {
                    let x = x.clone();
                    c.next();
                    Some(x)
                }
                _ => None,
            };
            if self.line_end(&c).is_none() {
                self.skip_rest(indent, end);
                failed = true;
                continue;
            }
            // the fields under the value, as a record's
            let mut fields = Vec::new();
            let field_indent = self.lines.get(self.i).filter(|x| self.i < end && x.indent > indent).map(|x| x.indent);
            while let Some(fi) = field_indent
                && self.i < end
                && self.lines[self.i].indent > indent
            {
                let fl = self.lines[self.i].clone();
                self.i += 1;
                if fl.indent != fi {
                    let at = self.line_span(self.i - 1);
                    self.err("E004", at, tr!("フィールドの字下げがそろっていません（{fi} 文字のはずが {} 文字）", "The fields are not indented alike ({} spaces where the first has {fi})", fl.indent));
                    failed = true;
                    continue;
                }
                match self.field_line(&fl) {
                    Some(f) => fields.push(f),
                    None => failed = true,
                }
            }
            values.push(EnumValueDecl { name: v, label, fields, line: l.no });
        }
        if failed {
            return None;
        }
        Some(EnumDecl { name, private: false, tparams, ordered: false, listed: true, values, line: line.no })
    }

    /// A field: `<name> : <type>`, with its range and its count.
    fn field_line(&mut self, l: &Line) -> R<FieldDecl> {
        let mut c = Cur::new(&l.tokens, l.no);
        let n = self.decl_name(&mut c, tr!("フィールドの名前", "the field's name"))?;
        if !c.eat(&Tok::Colon) {
            self.expected(&c, tr!("`:` と型", "`:` and the type"));
            return None;
        }
        let ty = self.ty(&mut c)?;
        let bounds = self.bounds(&mut c)?;
        self.line_end(&c)?;
        Some(FieldDecl { name: n, ty, bounds, line: l.no })
    }

    /// The lines under a head line at the left margin: from `self.i` up to the next line there.
    fn body_end(&self) -> usize {
        let mut e = self.i;
        while e < self.lines.len() && self.lines[e].indent > 0 {
            e += 1;
        }
        e
    }

    fn record(&mut self, line: &Line) -> R<RecordDecl> {
        let mut c = Cur::new(&line.tokens, line.no);
        c.next();
        let name = self.decl_name(&mut c, tr!("レコードの名前", "the record's name"));
        let tparams = if name.is_some() { self.type_params(&mut c) } else { None };
        let end_ok = name.is_some() && tparams.is_some() && self.line_end(&c).is_some();
        let end = self.body_end();
        let mut fields = Vec::new();
        if self.i == end && end_ok {
            let at = c.prev_span();
            self.err("E003", at, tr!("レコードに、フィールドが一つもありません", "The record has no field")).notes.push(tr!(
                "フィールドは、`record` の行の下に字下げして、一行に一つずつ `<名前> : <型>` と書きます。",
                "Write its fields indented under the `record` line, one a line: `<name> : <type>`."
            ));
        }
        let indent = self.lines.get(self.i).map(|l| l.indent).unwrap_or(0);
        while self.i < end {
            let l = self.lines[self.i].clone();
            self.i += 1;
            if l.indent != indent {
                let at = self.line_span(self.i - 1);
                self.err(
                    "E004",
                    at,
                    tr!("フィールドの字下げがそろっていません（{indent} 文字のはずが {} 文字）", "The fields are not indented alike ({} spaces where the first has {indent})", l.indent),
                );
                continue;
            }
            if let Some(f) = self.field_line(&l) {
                fields.push(f);
            }
        }
        Some(RecordDecl { name: name?, private: false, tparams: tparams?, fields, line: line.no })
    }

    // ── Types ───────────────────────────────────────────────────────────────────

    fn ty(&mut self, c: &mut Cur) -> R<TypeRef> {
        let start = c.span();
        if c.peek() == Some(&Tok::LParen) {
            return self.fn_type(c);
        }
        let Some(Token { tok: Tok::Word(w), span }) = c.next() else {
            c.k = c.k.saturating_sub(1);
            self.expected(c, tr!("型", "a type"));
            return None;
        };
        let w = w.clone();
        let span = *span;
        let open = |p: &mut Self, c: &mut Cur| -> R<()> {
            if c.eat(&Tok::LBracket) {
                Some(())
            } else {
                p.expected(c, tr!("`[`", "`[`"));
                None
            }
        };
        let close = |p: &mut Self, c: &mut Cur| -> R<()> {
            if c.eat(&Tok::RBracket) {
                Some(())
            } else {
                p.expected(c, tr!("`]`", "`]`"));
                None
            }
        };
        let kind = match w.as_str() {
            kw::NUMBER => TypeKind::Number,
            kw::BOOL => TypeKind::Bool,
            kw::STRING => TypeKind::Str,
            kw::DATE => TypeKind::Date,
            kw::RATE => {
                if c.peek() == Some(&Tok::LBracket) {
                    c.next();
                    if !c.eat_word(kw::STEP) {
                        self.expected(c, tr!("`step` と刻み（`rate[step 0.01%]`）", "`step` and the step (`rate[step 0.01%]`)"));
                        return None;
                    }
                    let n = match c.next() {
                        Some(Token { tok: Tok::Num(n), .. }) => n.clone(),
                        _ => {
                            c.k -= 1;
                            self.expected(c, tr!("率の刻み（`0.01%`）", "the step of the rate (`0.01%`)"));
                            return None;
                        }
                    };
                    close(self, c)?;
                    TypeKind::Rate(Some(n))
                } else {
                    TypeKind::Rate(None)
                }
            }
            kw::LIST => {
                open(self, c)?;
                let inner = self.ty(c)?;
                close(self, c)?;
                TypeKind::List(Box::new(inner))
            }
            // `dict[K, V]`, `set[T]`, `entry[K, V]`, `indexed[T]`: types where `[` comes after them, else
            // the name of a record or an enum (DESIGN 24.2)
            kw::DICT | kw::ENTRY if c.peek() == Some(&Tok::LBracket) => {
                open(self, c)?;
                let k = self.ty(c)?;
                if !c.eat(&Tok::Comma) {
                    self.expected(c, tr!("`,` と値の型（`{w}[string, number]`）", "`,` and the type of the values (`{w}[string, number]`)"));
                    return None;
                }
                let v = self.ty(c)?;
                close(self, c)?;
                if w == kw::DICT { TypeKind::Dict(Box::new(k), Box::new(v)) } else { TypeKind::Entry(Box::new(k), Box::new(v)) }
            }
            // `result[T, E]` (DESIGN 26.3), a type where `[` comes after it
            kw::RESULT if c.peek() == Some(&Tok::LBracket) => {
                open(self, c)?;
                let t = self.ty(c)?;
                if !c.eat(&Tok::Comma) {
                    self.expected(c, tr!("`,` と、値が無い理由の型（`result[Order, string]`）", "`,` and the type of why there is no value (`result[Order, string]`)"));
                    return None;
                }
                let e = self.ty(c)?;
                close(self, c)?;
                TypeKind::Result(Box::new(t), Box::new(e))
            }
            kw::SET | kw::INDEXED if c.peek() == Some(&Tok::LBracket) => {
                open(self, c)?;
                let inner = self.ty(c)?;
                close(self, c)?;
                if w == kw::SET { TypeKind::Set(Box::new(inner)) } else { TypeKind::Indexed(Box::new(inner)) }
            }
            kw::RUNNING => {
                open(self, c)?;
                let item = self.ty(c)?;
                if !c.eat(&Tok::Comma) {
                    self.expected(c, tr!("`,` と重みの型（`running[Line, money[USDc]]`）", "`,` and the type of the weight (`running[Line, money[USDc]]`)"));
                    return None;
                }
                let weight = self.ty(c)?;
                close(self, c)?;
                TypeKind::Running(Box::new(item), Box::new(weight))
            }
            w if w == kw::MONEY || kw::QUANTITIES.contains(&w) => {
                open(self, c)?;
                let (unit, unit_span) = match c.next() {
                    Some(Token { tok: Tok::Word(u), span }) => (u.clone(), *span),
                    _ => {
                        c.k -= 1;
                        self.expected(c, tr!("単位（`GBP`、`USDc`、`kg`）", "a unit (`GBP`, `USDc`, `kg`)"));
                        return None;
                    }
                };
                let tax = if c.eat(&Tok::Comma) {
                    match c.next() {
                        Some(Token { tok: Tok::Word(t), .. }) => Some(t.clone()),
                        _ => {
                            c.k -= 1;
                            self.expected(c, tr!("`incl_tax` か `excl_tax`", "`incl_tax` or `excl_tax`"));
                            return None;
                        }
                    }
                } else {
                    None
                };
                close(self, c)?;
                TypeKind::Unit { dim: w.to_string(), unit, tax, unit_span }
            }
            _ => {
                if kw::is_reserved(&w) {
                    self.err("E001", span, tr!("`{w}` を読めません。ここには型が要ります", "`{w}` cannot be read here: a type is needed"));
                    return None;
                }
                let mut path = vec![(w.clone(), span)];
                while c.peek() == Some(&Tok::Dot) {
                    c.next();
                    match c.next() {
                        Some(Token { tok: Tok::Word(x), span }) => path.push((x.clone(), *span)),
                        _ => {
                            c.k -= 1;
                            self.expected(c, tr!("`.` のあとの名前", "a name after `.`"));
                            return None;
                        }
                    }
                }
                // the type arguments of a generic record or enum (DESIGN 26.1)
                let mut args = Vec::new();
                if c.eat(&Tok::LBracket) {
                    loop {
                        args.push(self.ty(c)?);
                        if c.eat(&Tok::Comma) {
                            continue;
                        }
                        if c.eat(&Tok::RBracket) {
                            break;
                        }
                        self.expected(c, tr!("`,` か `]`", "`,` or `]`"));
                        return None;
                    }
                }
                TypeKind::Named(path, args)
            }
        };
        let optional = c.eat(&Tok::Question);
        Some(TypeRef { kind, optional, span: start.to(c.prev_span()) })
    }

    /// A function type, `(T, Adjustment) -> T`, and the effect written after it (`(T) -> U total`,
    /// DESIGN 27.1); `?` after the effect makes it optional (E113 says it cannot be).
    fn fn_type(&mut self, c: &mut Cur) -> R<TypeRef> {
        let start = c.span();
        c.next();
        let mut params = Vec::new();
        if !c.eat(&Tok::RParen) {
            loop {
                params.push(self.ty(c)?);
                if c.eat(&Tok::Comma) {
                    continue;
                }
                if c.eat(&Tok::RParen) {
                    break;
                }
                self.expected(c, tr!("`,` か `)`", "`,` or `)`"));
                return None;
            }
        }
        if !c.eat(&Tok::Arrow) {
            self.expected(c, tr!("`->` と答えの型（`(number) -> number`）", "`->` and the type of the answer (`(number) -> number`)"));
            return None;
        }
        let ret = self.ty(c)?;
        let effect = self.effect_in(c, true)?;
        let optional = c.eat(&Tok::Question);
        Some(TypeRef { kind: TypeKind::Fn(params, Box::new(ret), effect), optional, span: start.to(c.prev_span()) })
    }

    /// `range …` and `count …`, each at most once, after a type.
    fn bounds(&mut self, c: &mut Cur) -> R<Bounds> {
        let mut b = Bounds::default();
        loop {
            let (is_range, at) = match c.peek() {
                Some(Tok::Word(w)) if w == kw::RANGE => (true, c.span()),
                Some(Tok::Word(w)) if w == kw::COUNT => (false, c.span()),
                _ => return Some(b),
            };
            c.next();
            let mut ends = Vec::new();
            while let Some(op) = match c.peek() {
                Some(Tok::Ge) => Some(Cmp::Ge),
                Some(Tok::Le) => Some(Cmp::Le),
                Some(Tok::Gt) => Some(Cmp::Gt),
                Some(Tok::Lt) => Some(Cmp::Lt),
                _ => None,
            } {
                let start = c.span();
                c.next();
                let neg = c.eat(&Tok::Minus);
                let value = match c.next() {
                    Some(Token { tok: Tok::Num(n), .. }) => BoundValue::Num(n.clone(), neg),
                    Some(Token { tok: Tok::Date(d), .. }) if !neg => BoundValue::Date(*d),
                    _ => {
                        c.k -= 1;
                        self.expected(c, tr!("範囲の端の数か日付（`>=1GBPc`、`<=2027-12-31`）", "the number or the date of the end (`>=1GBPc`, `<=2027-12-31`)"));
                        return None;
                    }
                };
                ends.push(Bound { op, value, span: start.to(c.prev_span()) });
            }
            let word = if is_range { kw::RANGE } else { kw::COUNT };
            if ends.is_empty() || ends.len() > 2 {
                self.err("E001", at, tr!("`{word}` のあとには、端を一つか二つ書いてください（`{word} >=1 <=30`）", "After `{word}`, write one end or two (`{word} >=1 <=30`)"));
                return None;
            }
            let slot = if is_range { &mut b.range } else { &mut b.count };
            if slot.is_some() {
                self.err("E001", at, tr!("`{word}` が二度書かれています", "`{word}` is written twice"));
                return None;
            }
            let span = at.to(c.prev_span());
            *slot = Some((ends, span));
        }
    }

    // ── Functions ───────────────────────────────────────────────────────────────

    fn function(&mut self, line: &Line) -> R<FnDecl> {
        let end = self.body_end();
        let f = self.fn_head(line);
        let body = match &f {
            Some(_) => self.fn_body(line, end),
            None => None,
        };
        // whatever was not read of the body is its errors'
        self.i = end;
        let (name, tparams, params, ret, ret_bounds, effect) = f?;
        let (description, body) = body?;
        let last = self.lines[..end].last().map(|l| l.no).unwrap_or(line.no);
        Some(FnDecl { name, private: false, tparams, params, ret, ret_bounds, effect, description, body, line: line.no, last })
    }

    #[allow(clippy::type_complexity)]
    fn fn_head(&mut self, line: &Line) -> R<(Name, Vec<TypeParam>, Vec<Param>, TypeRef, Bounds, Option<Effect>)> {
        let mut c = Cur::new(&line.tokens, line.no);
        c.next();
        // `fn 遅れの請求(claim)(請求書(invoice): …)`: an alias is a word alone in parentheses with
        // the parameters' parentheses after it, or the type parameters' brackets (DESIGN 26.1)
        let name = {
            let (text, span) = match c.next() {
                Some(Token { tok: Tok::Word(w), span }) => (w.clone(), *span),
                _ => {
                    c.k -= 1;
                    self.expected(&c, tr!("関数の名前", "the function's name"));
                    return None;
                }
            };
            if kw::is_reserved(&text) {
                self.err("E002", span, tr!("`{text}` は予約語なので、名前にできません", "`{text}` is a keyword and cannot be a name")).notes.push(reserved_note());
                return None;
            }
            let mut n = Name { text, alias: None, span };
            if c.peek() == Some(&Tok::LParen) && matches!(c.peek_at(1), Some(Tok::Word(_))) && c.peek_at(2) == Some(&Tok::RParen) && matches!(c.peek_at(3), Some(Tok::LParen | Tok::LBracket)) {
                c.next();
                let t = c.next().expect("a word");
                let Tok::Word(a) = &t.tok else { unreachable!() };
                n.alias = Some((a.clone(), t.span));
                c.next();
            }
            n
        };
        let tparams = self.type_params(&mut c)?;
        if !c.eat(&Tok::LParen) {
            self.expected(&c, tr!("`(` と引数", "`(` and the parameters"));
            return None;
        }
        let mut params = Vec::new();
        if !c.eat(&Tok::RParen) {
            loop {
                let pn = self.decl_name(&mut c, tr!("引数の名前", "a parameter's name"))?;
                if !c.eat(&Tok::Colon) {
                    self.expected(&c, tr!("`:` と引数の型", "`:` and the parameter's type"));
                    return None;
                }
                let ty = self.ty(&mut c)?;
                let bounds = self.bounds(&mut c)?;
                params.push(Param { name: pn, ty, bounds });
                if c.eat(&Tok::Comma) {
                    if c.eat(&Tok::RParen) {
                        break;
                    }
                    continue;
                }
                if c.eat(&Tok::RParen) {
                    break;
                }
                self.expected(&c, tr!("`,` か `)`", "`,` or `)`"));
                return None;
            }
        }
        if !c.eat(&Tok::Arrow) {
            self.expected(&c, tr!("`->` と結果の型", "`->` and the type of the result"));
            return None;
        }
        let ret = self.ty(&mut c)?;
        let ret_bounds = self.bounds(&mut c)?;
        let effect = self.effect(&mut c)?;
        self.line_end(&c)?;
        Some((name, tparams, params, ret, ret_bounds, effect))
    }

    /// `total`, `fails rule, overflow`, `fails zero diverges`, `diverges` (DESIGN 4).
    fn effect(&mut self, c: &mut Cur) -> R<Option<Effect>> {
        self.effect_in(c, false)
    }

    /// The same, after a function type when `in_type` (DESIGN 27.1): there a `,` after a kind goes on to
    /// the next parameter unless a kind follows it that is no parameter's name (`f: (T) -> U fails
    /// rule, zero: number` gives `f` the kind `rule`).
    fn effect_in(&mut self, c: &mut Cur, in_type: bool) -> R<Option<Effect>> {
        let start = c.span();
        match c.peek() {
            Some(Tok::Word(w)) if w == kw::TOTAL => {
                c.next();
                Some(Some(Effect { total: true, fails: vec![], diverges: false, span: start }))
            }
            Some(Tok::Word(w)) if w == kw::DIVERGES => {
                c.next();
                Some(Some(Effect { total: false, fails: vec![], diverges: true, span: start }))
            }
            Some(Tok::Word(w)) if w == kw::FAILS => {
                c.next();
                let mut fails = Vec::new();
                loop {
                    match c.next() {
                        Some(Token { tok: Tok::Word(k), span }) if FailKind::parse(k).is_some() => fails.push((FailKind::parse(k).expect("a kind"), *span)),
                        _ => {
                            c.k -= 1;
                            self.expected(c, tr!("失敗の種類（`rule`、`date`、`range`、`overflow`、`zero`）", "a kind of failure (`rule`, `date`, `range`, `overflow`, `zero`)"));
                            return None;
                        }
                    }
                    let kind_next = matches!(c.peek_at(1), Some(Tok::Word(k)) if FailKind::parse(k).is_some()) && !matches!(c.peek_at(2), Some(Tok::Colon | Tok::LParen));
                    if c.peek() != Some(&Tok::Comma) || (in_type && !kind_next) {
                        break;
                    }
                    c.next();
                }
                let diverges = c.eat_word(kw::DIVERGES);
                Some(Some(Effect { total: false, fails, diverges, span: start.to(c.prev_span()) }))
            }
            _ => Some(None),
        }
    }

    /// The body of a function: the lines under its head, a description first when it has one.
    fn fn_body(&mut self, head: &Line, end: usize) -> R<(Option<(String, Span)>, Block)> {
        if self.i >= end {
            let at = Span { line: head.no, col: 1, end: 1 };
            self.err("E111", at, tr!("関数に本体がありません", "The function has no body"))
                .notes
                .push(tr!("本体は、`fn` の行の下に字下げして書き、最後の行が関数の答えになります。", "Write the body indented under the `fn` line; its last line is the function's answer."));
            return None;
        }
        let indent = self.lines[self.i].indent;
        let mut description = None;
        if self.lines[self.i].tokens[0].tok.is_word(kw::DESCRIPTION) {
            let l = self.lines[self.i].clone();
            self.i += 1;
            let mut c = Cur::new(&l.tokens, l.no);
            c.next();
            description = Some(self.description(&mut c)?);
        }
        let block = self.block(indent, end, head.no);
        while self.i < end {
            let at = self.line_span(self.i);
            let l = self.lines[self.i].indent;
            self.err(
                "E004",
                at,
                tr!(
                    "この行の字下げ（{l} 文字）は、関数の本体の字下げ（{indent} 文字）とそろっていません",
                    "This line's indentation ({l} spaces) does not line up with the function's body ({indent} spaces)"
                ),
            );
            self.i += 1;
        }
        Some((description, block?))
    }

    /// The lines from `self.i` at exactly `indent`, with the deeper lines under each, up to a line
    /// less indented or `end`: the `let` lines, then the answer.
    fn block(&mut self, indent: usize, end: usize, head: usize) -> R<Block> {
        let mut lets: Vec<Let> = Vec::new();
        let mut answer: Option<Expr> = None;
        let mut failed = false;
        while self.i < end && self.lines[self.i].indent >= indent {
            let l = self.lines[self.i].clone();
            if l.indent > indent {
                let at = self.line_span(self.i);
                self.err(
                    "E004",
                    at,
                    tr!(
                        "この行の字下げ（{} 文字）は、上の行（{indent} 文字）とそろっていません",
                        "This line's indentation ({} spaces) does not line up with the lines above ({indent} spaces)",
                        l.indent
                    ),
                );
                self.i += 1;
                failed = true;
                continue;
            }
            if answer.is_some() {
                let at = self.line_span(self.i);
                self.err("E111", at, tr!("ブロックの答えのあとに、まだ行があります", "There are lines after the block's answer")).notes.push(tr!(
                    "ブロックの答えは最後の行だけです。途中で値を名付けるなら `let <名前> = …` と書いてください。",
                    "A block answers with its last line only; to name a value on the way, write `let <name> = …`."
                ));
                self.skip_under(indent, end);
                failed = true;
                continue;
            }
            if l.tokens[0].tok.is_word(kw::LET) {
                match self.let_line(&l, indent, end) {
                    Some(x) => lets.push(x),
                    None => failed = true,
                }
                continue;
            }
            match self.answer_line(&l, indent, end) {
                Some(e) => answer = Some(e),
                None => failed = true,
            }
        }
        if failed {
            return None;
        }
        match answer {
            Some(answer) => Some(Block { lets, answer }),
            None => {
                let at = lets.last().map(|x| x.span).unwrap_or(Span { line: head, col: 1, end: 1 });
                self.err("E111", at, tr!("ブロックに答えがありません。最後の行は値を表す式にしてください", "The block has no answer: its last line has to be an expression with a value")).notes.push(
                    tr!("`let` の行は値に名前を付けるだけです。ブロックの最後に、答えにする式を書いてください。", "A `let` line only names a value; end the block with the expression it answers."),
                );
                None
            }
        }
    }

    /// Pass over the line at `self.i` and the lines under it.
    fn skip_under(&mut self, indent: usize, end: usize) {
        self.i += 1;
        while self.i < end && self.lines[self.i].indent > indent {
            self.i += 1;
        }
    }

    /// A block under the line before `self.i`, deeper than `indent`; E111 when there is none.
    fn nested(&mut self, indent: usize, end: usize, head: usize, what: Text) -> R<Block> {
        if self.i < end && self.lines[self.i].indent > indent {
            let inner = self.lines[self.i].indent;
            return self.block(inner, end, head);
        }
        self.err("E111", Span { line: head, col: 1, end: 1 }, tr!("{}の下に、字下げしたブロックがありません", "There is no block indented under {}", what.ja; what.en));
        None
    }

    fn let_line(&mut self, l: &Line, indent: usize, end: usize) -> R<Let> {
        self.i += 1;
        let mut c = Cur::new(&l.tokens, l.no);
        c.next();
        let (name, span) = match c.next() {
            Some(Token { tok: Tok::Word(w), span }) => (w.clone(), *span),
            _ => {
                c.k -= 1;
                self.expected(&c, tr!("名前", "a name"));
                self.skip_rest(indent, end);
                return None;
            }
        };
        if kw::is_reserved(&name) {
            self.err("E002", span, tr!("`{name}` は予約語なので、名前にできません", "`{name}` is a keyword and cannot be a name")).notes.push(reserved_note());
            self.skip_rest(indent, end);
            return None;
        }
        let ty = if c.eat(&Tok::Colon) {
            match self.ty(&mut c) {
                Some(t) => Some(t),
                None => {
                    self.skip_rest(indent, end);
                    return None;
                }
            }
        } else {
            None
        };
        if !c.eat(&Tok::Eq) {
            self.expected(&c, tr!("`=` と値", "`=` and the value"));
            self.skip_rest(indent, end);
            return None;
        }
        // `let x = try e` (DESIGN 26.3)
        let tried = if c.peek().is_some_and(|t| t.is_word(kw::TRY)) {
            let at = c.span();
            c.next();
            if c.at_end() {
                self.expected(&c, tr!("`try` のあとの値", "the value after `try`"));
                self.skip_rest(indent, end);
                return None;
            }
            Some(at)
        } else {
            None
        };
        let value = if c.at_end() {
            let b = self.nested(indent, end, l.no, tr!("`let {name} =`", "`let {name} =`"))?;
            let sp = b.answer.span;
            if b.lets.is_empty() { b.answer } else { self.mk(sp, ExprKind::Block(Box::new(b))) }
        } else {
            let e = self.expr(&mut c);
            if e.is_none() || self.line_end(&c).is_none() {
                self.skip_rest(indent, end);
                return None;
            }
            e?
        };
        Some(Let { name, span, ty, value, tried })
    }

    /// Pass over the lines under the line just read.
    fn skip_rest(&mut self, indent: usize, end: usize) {
        while self.i < end && self.lines[self.i].indent > indent {
            self.i += 1;
        }
    }

    /// The answer of a block: an `if` with blocks under it, a `match` with its arms, or an
    /// expression on its line.
    fn answer_line(&mut self, l: &Line, indent: usize, end: usize) -> R<Expr> {
        let mut c = Cur::new(&l.tokens, l.no);
        if l.tokens[0].tok.is_word(kw::IF) && !c.ahead_at_depth(&|t| t.is_word(kw::THEN)) {
            return self.if_block(l, indent, end);
        }
        if l.tokens[0].tok.is_word(kw::MATCH) {
            return self.match_block(l, indent, end);
        }
        if l.tokens[0].tok.is_word(kw::ELSE) {
            let at = l.tokens[0].span;
            self.err("E001", at, tr!("`else` の前に、同じ字下げの `if` がありません", "There is no `if` at this indentation before this `else`"));
            self.skip_under(indent, end);
            return None;
        }
        self.i += 1;
        let e = self.expr(&mut c);
        if e.is_none() || self.line_end(&c).is_none() {
            self.skip_rest(indent, end);
            return None;
        }
        if self.i < end && self.lines[self.i].indent > indent {
            let at = self.line_span(self.i);
            self.err("E004", at, tr!("この行は字下げされていますが、上の行はブロックを取りません", "This line is indented, and the line above takes no block"));
            self.skip_rest(indent, end);
            return None;
        }
        e
    }

    fn if_block(&mut self, l: &Line, indent: usize, end: usize) -> R<Expr> {
        // each `if` and `else if` with its block, then the `else` block
        let mut arms: Vec<(Expr, Block)> = Vec::new();
        let mut c = Cur::new(&l.tokens, l.no);
        c.next();
        self.i += 1;
        let start = l.tokens[0].span;
        let cond = self.expr(&mut c);
        let ok = cond.is_some() && self.line_end(&c).is_some();
        let then = if ok { self.nested(indent, end, l.no, tr!("`if`", "the `if`")) } else { None };
        if !ok {
            self.skip_rest(indent, end);
        }
        let (cond, then) = (cond?, then?);
        arms.push((cond, then));
        let otherwise = loop {
            let Some(next) = self.lines.get(self.i).filter(|x| self.i < end && x.indent == indent && x.tokens[0].tok.is_word(kw::ELSE)).cloned() else {
                self.err("E111", start, tr!("`if` に `else` がありません", "The `if` has no `else`")).notes.push(tr!(
                    "kaname の `if` は値を返すので、どの場合にも答えが要ります。条件が成り立たないときの答えを `else` の下に書いてください（値が無いことは `none` で言えます）。",
                    "An `if` of kaname is a value, so every case needs an answer: write the one for when the condition does not hold under `else` (`none` says there is no value)."
                ));
                return None;
            };
            self.i += 1;
            let mut c = Cur::new(&next.tokens, next.no);
            c.next();
            if c.eat_word(kw::IF) {
                let cond = self.expr(&mut c);
                let ok = cond.is_some() && self.line_end(&c).is_some();
                let block = if ok { self.nested(indent, end, next.no, tr!("`else if`", "the `else if`")) } else { None };
                if !ok {
                    self.skip_rest(indent, end);
                }
                arms.push((cond?, block?));
                continue;
            }
            self.line_end(&c)?;
            break self.nested(indent, end, next.no, tr!("`else`", "the `else`"))?;
        };
        // `if a … else if b … else …` is `if a … else (if b … else …)`
        let mut tail = otherwise;
        while let Some((cond, block)) = arms.pop() {
            let span = if arms.is_empty() { start } else { cond.span };
            let e = self.mk(span, ExprKind::If(Box::new(cond), Box::new(block), Box::new(tail)));
            tail = Block { lets: vec![], answer: e };
        }
        Some(tail.answer)
    }

    fn match_block(&mut self, l: &Line, indent: usize, end: usize) -> R<Expr> {
        let mut c = Cur::new(&l.tokens, l.no);
        c.next();
        self.i += 1;
        let start = l.tokens[0].span;
        let scrutinee = self.expr(&mut c);
        if scrutinee.is_none() || self.line_end(&c).is_none() {
            self.skip_rest(indent, end);
            return None;
        }
        if !(self.i < end && self.lines[self.i].indent > indent) {
            self.err("E111", start, tr!("`match` の下に、分岐が一つもありません", "There is no arm under the `match`"))
                .notes
                .push(tr!("分岐は `match` の下に字下げして、`<値> => <答え>` と一行に一つずつ書きます。", "Write the arms indented under the `match`, one a line: `<value> => <answer>`."));
            return None;
        }
        let arm_indent = self.lines[self.i].indent;
        let mut arms = Vec::new();
        let mut failed = false;
        while self.i < end && self.lines[self.i].indent > indent {
            let al = self.lines[self.i].clone();
            if al.indent != arm_indent {
                let at = self.line_span(self.i);
                self.err(
                    "E004",
                    at,
                    tr!("分岐の字下げがそろっていません（{arm_indent} 文字のはずが {} 文字）", "The arms are not indented alike ({} spaces where the first has {arm_indent})", al.indent),
                );
                self.i += 1;
                failed = true;
                continue;
            }
            self.i += 1;
            match self.arm(&al, arm_indent, end) {
                Some(a) => arms.push(a),
                None => {
                    failed = true;
                    self.skip_rest(arm_indent, end);
                }
            }
        }
        if failed {
            return None;
        }
        Some(self.mk(start, ExprKind::Match(Box::new(scrutinee?), arms)))
    }

    fn arm(&mut self, l: &Line, indent: usize, end: usize) -> R<Arm> {
        let mut c = Cur::new(&l.tokens, l.no);
        let start = c.span();
        let mut patterns = Vec::new();
        loop {
            let at = c.span();
            // a literal: a number (a minus before it too), a date, a string
            if matches!(c.peek(), Some(Tok::Num(_) | Tok::Date(_) | Tok::Str(_) | Tok::Minus)) {
                let neg = c.eat(&Tok::Minus);
                let lit = match c.next() {
                    Some(Token { tok: Tok::Num(n), span }) => {
                        let (n, sp) = (n.clone(), *span);
                        self.mk(sp, ExprKind::Num(n))
                    }
                    Some(Token { tok: Tok::Date(d), span }) if !neg => {
                        let (d, sp) = (*d, *span);
                        self.mk(sp, ExprKind::Date(d))
                    }
                    Some(Token { tok: Tok::Str(x), span }) if !neg => {
                        let (x, sp) = (x.clone(), *span);
                        self.mk(sp, ExprKind::Str(x))
                    }
                    _ => {
                        c.k = c.k.saturating_sub(1);
                        self.expected(&c, tr!("`-` のあとの数", "a number after `-`"));
                        return None;
                    }
                };
                let lit = if neg {
                    let sp = at.to(lit.span);
                    self.mk(sp, ExprKind::Neg(Box::new(lit)))
                } else {
                    lit
                };
                patterns.push(Pattern::Lit(Box::new(lit)));
                if !c.eat(&Tok::Comma) {
                    break;
                }
                continue;
            }
            let p = match c.next().map(|t| &t.tok) {
                Some(Tok::Word(w)) if w == kw::NONE => Pattern::None(at),
                Some(Tok::Word(w)) if w == kw::TRUE => Pattern::Bool(true, at),
                Some(Tok::Word(w)) if w == kw::FALSE => Pattern::Bool(false, at),
                Some(Tok::Word(w)) if w == kw::ELSE => Pattern::Else(at),
                Some(Tok::Word(w)) if w == kw::SOME => match c.next() {
                    Some(Token { tok: Tok::Word(x), span }) if !kw::is_reserved(x) => Pattern::Some(x.clone(), *span),
                    _ => {
                        c.k -= 1;
                        self.expected(&c, tr!("`some` のあとに、値に付ける名前", "the name the value takes, after `some`"));
                        return None;
                    }
                },
                Some(Tok::Word(w)) if !kw::is_reserved(w) => {
                    let mut path = vec![(w.clone(), at)];
                    while c.peek() == Some(&Tok::Dot) {
                        c.next();
                        match c.next() {
                            Some(Token { tok: Tok::Word(x), span }) => path.push((x.clone(), *span)),
                            _ => {
                                c.k -= 1;
                                self.expected(&c, tr!("`.` のあとの名前", "a name after `.`"));
                                return None;
                            }
                        }
                    }
                    let sp = at.to(c.prev_span());
                    // the name the value is given in the arm: `placed p`
                    let bind = match c.peek() {
                        Some(Tok::Word(x)) if !kw::is_reserved(x) => {
                            let b = (x.clone(), c.span());
                            c.next();
                            Some(b)
                        }
                        _ => None,
                    };
                    Pattern::Value(path, bind, sp)
                }
                _ => {
                    c.k = c.k.saturating_sub(1);
                    self.expected(
                        &c,
                        tr!(
                            "分岐の値（列挙の値、`true`、`false`、`none`、`some <名前>`、数・日付・文字列のリテラル、`else`）",
                            "the value of the arm (an enum's value, `true`, `false`, `none`, `some <name>`, a literal number, date or string, `else`)"
                        ),
                    );
                    return None;
                }
            };
            patterns.push(p);
            if !c.eat(&Tok::Comma) {
                break;
            }
        }
        if !c.eat(&Tok::FatArrow) {
            self.expected(&c, tr!("`=>` と答え", "`=>` and the answer"));
            return None;
        }
        let body = if c.at_end() {
            self.nested(indent, end, l.no, tr!("分岐", "the arm"))?
        } else {
            let e = self.expr(&mut c)?;
            self.line_end(&c)?;
            if self.i < end && self.lines[self.i].indent > indent {
                let at = self.line_span(self.i);
                self.err("E004", at, tr!("この行は字下げされていますが、上の分岐は答えを同じ行に書いています", "This line is indented, and the arm above writes its answer on its own line"));
                return None;
            }
            Block { lets: vec![], answer: e }
        };
        Some(Arm { patterns, body, span: start })
    }

    // ── Examples ────────────────────────────────────────────────────────────────

    /// The lines under `examples`: each a call and its answer, a line deeper than the first going
    /// on from the line before it.
    fn examples(&mut self) -> Vec<Example> {
        let end = self.body_end();
        let mut out = Vec::new();
        let Some(indent) = self.lines.get(self.i).filter(|_| self.i < end).map(|l| l.indent) else {
            return out;
        };
        while self.i < end {
            let l = self.lines[self.i].clone();
            self.i += 1;
            if l.indent < indent {
                let at = self.line_span(self.i - 1);
                self.err("E004", at, tr!("例の字下げがそろっていません（{indent} 文字のはずが {} 文字）", "The examples are not indented alike ({} spaces where the first has {indent})", l.indent));
                continue;
            }
            let mut toks = l.tokens.clone();
            while self.i < end && self.lines[self.i].indent > indent {
                toks.extend(self.lines[self.i].tokens.iter().cloned());
                self.i += 1;
            }
            if let Some(x) = self.example(&toks, l.no) {
                out.push(x);
            }
        }
        out
    }

    fn example(&mut self, toks: &[Token], line: usize) -> R<Example> {
        let mut c = Cur::new(toks, line);
        let call = self.postfix(&mut c)?;
        if !matches!(call.kind, ExprKind::Call(..)) {
            self.err("E602", call.span, tr!("例は関数の呼び出しで書いてください", "An example is a call of a function")).notes.push(example_note());
            return None;
        }
        if c.eat(&Tok::Eq) {
            let want = self.expr(&mut c)?;
            self.line_end(&c)?;
            return Some(Example { call, expect: Expect::Value(want), line });
        }
        if c.eat_word(kw::FAILS) {
            return match c.next() {
                Some(Token { tok: Tok::Word(k), span }) if FailKind::parse(k).is_some() => {
                    let x = Example { call, expect: Expect::Fails(FailKind::parse(k).expect("a kind"), *span), line };
                    self.line_end(&c)?;
                    Some(x)
                }
                _ => {
                    c.k = c.k.saturating_sub(1);
                    self.expected(&c, tr!("失敗の種類（`rule`、`date`、`range`、`overflow`、`zero`）", "a kind of failure (`rule`, `date`, `range`, `overflow`, `zero`)"));
                    None
                }
            };
        }
        let at = c.span();
        self.err("E602", at, tr!("例の呼び出しのあとには、`= <答え>` か `fails <種類>` を書いてください", "After the call of an example, write `= <answer>` or `fails <kind>`"))
            .notes
            .push(example_note());
        None
    }

    // ── Expressions ─────────────────────────────────────────────────────────────

    fn expr(&mut self, c: &mut Cur) -> R<Expr> {
        let mut a = self.and(c)?;
        while c.eat_word(kw::OR) {
            let b = self.and(c)?;
            let sp = a.span.to(b.span);
            a = self.mk(sp, ExprKind::Bin(Box::new(a), BinOp::Or, Box::new(b)));
        }
        Some(a)
    }

    fn and(&mut self, c: &mut Cur) -> R<Expr> {
        let mut a = self.not(c)?;
        while c.eat_word(kw::AND) {
            let b = self.not(c)?;
            let sp = a.span.to(b.span);
            a = self.mk(sp, ExprKind::Bin(Box::new(a), BinOp::And, Box::new(b)));
        }
        Some(a)
    }

    fn not(&mut self, c: &mut Cur) -> R<Expr> {
        let at = c.span();
        if c.eat_word(kw::NOT) {
            let x = self.not(c)?;
            let sp = at.to(x.span);
            return Some(self.mk(sp, ExprKind::Not(Box::new(x))));
        }
        self.cmp(c)
    }

    fn cmp_op(t: Option<&Tok>) -> Option<BinOp> {
        match t {
            Some(Tok::Eq) => Some(BinOp::Eq),
            Some(Tok::Ne) => Some(BinOp::Ne),
            Some(Tok::Lt) => Some(BinOp::Lt),
            Some(Tok::Le) => Some(BinOp::Le),
            Some(Tok::Gt) => Some(BinOp::Gt),
            Some(Tok::Ge) => Some(BinOp::Ge),
            _ => None,
        }
    }

    fn cmp(&mut self, c: &mut Cur) -> R<Expr> {
        let a = self.conv(c)?;
        let Some(op) = Self::cmp_op(c.peek()) else { return Some(a) };
        c.next();
        let b = self.conv(c)?;
        if let Some(op2) = Self::cmp_op(c.peek()) {
            let at = c.span();
            let (o1, o2) = (op.shown(), op2.shown());
            self.err("E001", at, tr!("比べを続けて書けません（`… {o1} … {o2} …`）", "Comparisons cannot be chained (`… {o1} … {o2} …`)")).notes.push(tr!(
                "一つの式の比べは一つまでです。`a <= b and b <= c` のように `and` でつないでください。",
                "An expression compares once; join two comparisons with `and`, as in `a <= b and b <= c`."
            ));
            return None;
        }
        let sp = a.span.to(b.span);
        Some(self.mk(sp, ExprKind::Bin(Box::new(a), op, Box::new(b))))
    }

    fn conv(&mut self, c: &mut Cur) -> R<Expr> {
        let a = self.sum(c)?;
        if c.eat_word(kw::IN) {
            return match c.next() {
                Some(Token { tok: Tok::Word(u), span }) => {
                    let sp = a.span.to(*span);
                    Some(self.mk(sp, ExprKind::In(Box::new(a), u.clone(), *span)))
                }
                _ => {
                    c.k -= 1;
                    self.expected(c, tr!("`in` のあとに単位（`USDc`、`lb`）", "a unit after `in` (`USDc`, `lb`)"));
                    None
                }
            };
        }
        Some(a)
    }

    fn sum(&mut self, c: &mut Cur) -> R<Expr> {
        let mut a = self.product(c)?;
        loop {
            let op = match c.peek() {
                Some(Tok::Plus) => BinOp::Add,
                Some(Tok::Minus) => BinOp::Sub,
                _ => return Some(a),
            };
            c.next();
            let b = self.product(c)?;
            let sp = a.span.to(b.span);
            a = self.mk(sp, ExprKind::Bin(Box::new(a), op, Box::new(b)));
        }
    }

    fn product(&mut self, c: &mut Cur) -> R<Expr> {
        let mut a = self.unary(c)?;
        loop {
            let op = match c.peek() {
                Some(Tok::Star) => BinOp::Mul,
                Some(Tok::Slash) => BinOp::Div,
                _ => return Some(a),
            };
            c.next();
            let b = self.unary(c)?;
            let sp = a.span.to(b.span);
            a = self.mk(sp, ExprKind::Bin(Box::new(a), op, Box::new(b)));
        }
    }

    fn unary(&mut self, c: &mut Cur) -> R<Expr> {
        let at = c.span();
        if c.eat(&Tok::Minus) {
            let x = self.unary(c)?;
            let sp = at.to(x.span);
            return Some(self.mk(sp, ExprKind::Neg(Box::new(x))));
        }
        self.postfix(c)
    }

    fn postfix(&mut self, c: &mut Cur) -> R<Expr> {
        let mut e = self.primary(c)?;
        loop {
            match c.peek() {
                Some(Tok::Dot) => {
                    c.next();
                    match c.next() {
                        Some(Token { tok: Tok::Word(w), span }) => {
                            let sp = e.span.to(*span);
                            e = self.mk(sp, ExprKind::Field(Box::new(e), w.clone(), *span));
                        }
                        _ => {
                            c.k -= 1;
                            self.expected(c, tr!("`.` のあとの名前", "a name after `.`"));
                            return None;
                        }
                    }
                }
                Some(Tok::LParen) => {
                    c.next();
                    let args = self.args(c)?;
                    let sp = e.span.to(c.prev_span());
                    e = self.mk(sp, ExprKind::Call(Box::new(e), args));
                }
                // the type arguments of a call, after the name it calls: `empty_page[Invoice]()` (DESIGN
                // 26.1)
                Some(Tok::LBracket) if e.path().is_some() => {
                    c.next();
                    let mut targs = Vec::new();
                    loop {
                        targs.push(self.ty(c)?);
                        if c.eat(&Tok::Comma) {
                            continue;
                        }
                        if c.eat(&Tok::RBracket) {
                            break;
                        }
                        self.expected(c, tr!("`,` か `]`", "`,` or `]`"));
                        return None;
                    }
                    if !c.eat(&Tok::LParen) {
                        self.expected(c, tr!("型の引数のあとに、`(` と引数", "`(` and the arguments, after the type arguments"));
                        return None;
                    }
                    let args = self.args(c)?;
                    let sp = e.span.to(c.prev_span());
                    e = self.mk(sp, ExprKind::Call(Box::new(e), args));
                    self.targs.insert(e.id, targs);
                }
                Some(Tok::LBrace) if e.path().is_some() => {
                    c.next();
                    let path = e.path().expect("a path");
                    let (base, fields) = self.field_inits(c)?;
                    let sp = e.span.to(c.prev_span());
                    e = self.mk(sp, ExprKind::Record(path, base.map(Box::new), fields));
                }
                _ => return Some(e),
            }
        }
    }

    /// The arguments after `(`, up to the `)`: each `name: value` or a value.
    fn args(&mut self, c: &mut Cur) -> R<Vec<Arg>> {
        let mut out = Vec::new();
        if c.eat(&Tok::RParen) {
            return Some(out);
        }
        loop {
            if let (Some(Tok::Word(w)), Some(Tok::Colon)) = (c.peek(), c.peek_at(1)) {
                let (w, sp) = (w.clone(), c.span());
                c.next();
                c.next();
                let v = self.expr(c)?;
                out.push(Arg::Named(w, sp, v));
            } else {
                let v = self.expr(c)?;
                out.push(Arg::Pos(v));
            }
            if c.eat(&Tok::Comma) {
                if c.eat(&Tok::RParen) {
                    return Some(out);
                }
                continue;
            }
            if c.eat(&Tok::RParen) {
                return Some(out);
            }
            self.expected(c, tr!("`,` か `)`", "`,` or `)`"));
            return None;
        }
    }

    /// The fields after `{`, up to the `}`: the value the others are taken from (`..p`, first), then
    /// each `name: value`.
    fn field_inits(&mut self, c: &mut Cur) -> R<(Option<Expr>, Vec<FieldInit>)> {
        let mut out = Vec::new();
        if c.eat(&Tok::RBrace) {
            return Some((None, out));
        }
        let mut base = None;
        if c.eat(&Tok::DotDot) {
            base = Some(self.expr(c)?);
            if c.eat(&Tok::RBrace) {
                return Some((base, out));
            }
            if !c.eat(&Tok::Comma) {
                self.expected(c, tr!("`,` か `}}`", "`,` or `}}`"));
                return None;
            }
            if c.eat(&Tok::RBrace) {
                return Some((base, out));
            }
        }
        loop {
            if c.peek() == Some(&Tok::DotDot) {
                let at = c.span();
                self.err("E001", at, tr!("`..` は `{{` のすぐあとに、一度だけ書いてください", "`..` comes once, right after `{{`")).notes.push(tr!(
                    "`Order.placed {{ ..p, on: later }}` のように、元にする値を初めに書き、替えるフィールドをそのあとに並べます。",
                    "Write the value the fields are taken from first, then the fields that change: `Order.placed {{ ..p, on: later }}`."
                ));
                return None;
            }
            let (name, span) = match (c.peek(), c.peek_at(1)) {
                (Some(Tok::Word(w)), Some(Tok::Colon)) => (w.clone(), c.span()),
                _ => {
                    self.expected(c, tr!("フィールドの名前と `:`（`due: …`）", "a field's name and `:` (`due: …`)"));
                    return None;
                }
            };
            c.next();
            c.next();
            let value = self.expr(c)?;
            out.push(FieldInit { name, span, value });
            if c.eat(&Tok::Comma) {
                if c.eat(&Tok::RBrace) {
                    return Some((base, out));
                }
                continue;
            }
            if c.eat(&Tok::RBrace) {
                return Some((base, out));
            }
            self.expected(c, tr!("`,` か `}}`", "`,` or `}}`"));
            return None;
        }
    }

    fn primary(&mut self, c: &mut Cur) -> R<Expr> {
        let at = c.span();
        let Some(t) = c.peek() else {
            self.expected(c, tr!("値", "a value"));
            return None;
        };
        match t {
            Tok::Num(n) => {
                let n = n.clone();
                c.next();
                Some(self.mk(at, ExprKind::Num(n)))
            }
            Tok::Date(d) => {
                let d = *d;
                c.next();
                Some(self.mk(at, ExprKind::Date(d)))
            }
            Tok::Str(s) => {
                let s = s.clone();
                c.next();
                Some(self.mk(at, ExprKind::Str(s)))
            }
            Tok::LBracket => {
                c.next();
                // `[:]`, the dict with no pairs (DESIGN 24.2)
                if c.peek() == Some(&Tok::Colon) && c.peek_at(1) == Some(&Tok::RBracket) {
                    c.next();
                    c.next();
                    let sp = at.to(c.prev_span());
                    return Some(self.mk(sp, ExprKind::Dict(Vec::new())));
                }
                let mut xs = Vec::new();
                if !c.eat(&Tok::RBracket) {
                    let first = self.expr(c)?;
                    // a key and `:`: a dict, its pairs each a key and a value
                    if c.eat(&Tok::Colon) {
                        let mut pairs = vec![(first, self.expr(c)?)];
                        loop {
                            if c.eat(&Tok::Comma) {
                                if c.eat(&Tok::RBracket) {
                                    break;
                                }
                                let k = self.expr(c)?;
                                if !c.eat(&Tok::Colon) {
                                    self.expected(c, tr!("`:` と値（`[\"a\": 1]`）", "`:` and the value (`[\"a\": 1]`)"));
                                    return None;
                                }
                                let v = self.expr(c)?;
                                pairs.push((k, v));
                                continue;
                            }
                            if c.eat(&Tok::RBracket) {
                                break;
                            }
                            self.expected(c, tr!("`,` か `]`", "`,` or `]`"));
                            return None;
                        }
                        let sp = at.to(c.prev_span());
                        return Some(self.mk(sp, ExprKind::Dict(pairs)));
                    }
                    xs.push(first);
                    loop {
                        if c.eat(&Tok::Comma) {
                            if c.eat(&Tok::RBracket) {
                                break;
                            }
                            xs.push(self.expr(c)?);
                            continue;
                        }
                        if c.eat(&Tok::RBracket) {
                            break;
                        }
                        self.expected(c, tr!("`,` か `]`", "`,` or `]`"));
                        return None;
                    }
                }
                let sp = at.to(c.prev_span());
                Some(self.mk(sp, ExprKind::List(xs)))
            }
            Tok::LParen => {
                // `(acc, x) => …`, `(l: Line) => …`, `() => …`: a `(` whose `)` has `=>` after it (DESIGN
                // 27.1), which no value in parentheses has
                let mut depth = 0i32;
                let mut close = None;
                for (i, t) in c.toks[c.k..].iter().enumerate() {
                    match t.tok {
                        Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
                        Tok::RParen | Tok::RBracket | Tok::RBrace => {
                            depth -= 1;
                            if depth == 0 {
                                close = Some(c.k + i);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if close.is_some_and(|k| c.toks.get(k + 1).map(|t| &t.tok) == Some(&Tok::FatArrow)) {
                    let (words, tys) = self.lambda_params(c)?;
                    let body = self.expr(c)?;
                    let sp = at.to(body.span);
                    let e = self.mk(sp, ExprKind::Lambda(words, Box::new(body)));
                    if tys.iter().any(Option::is_some) {
                        self.ltypes.insert(e.id, tys);
                    }
                    return Some(e);
                }
                c.next();
                let e = self.expr(c)?;
                if !c.eat(&Tok::RParen) {
                    self.expected(c, tr!("`)`", "`)`"));
                    return None;
                }
                Some(e)
            }
            Tok::Word(w) => {
                let w = w.clone();
                match w.as_str() {
                    kw::TRUE | kw::FALSE => {
                        c.next();
                        Some(self.mk(at, ExprKind::Bool(w == kw::TRUE)))
                    }
                    kw::NONE => {
                        c.next();
                        Some(self.mk(at, ExprKind::None))
                    }
                    kw::IF => {
                        c.next();
                        let cond = self.expr(c)?;
                        if !c.eat_word(kw::THEN) {
                            self.expected(c, tr!("`then`", "`then`"));
                            return None;
                        }
                        let a = self.expr(c)?;
                        if !c.eat_word(kw::ELSE) {
                            let sp = c.span();
                            self.err("E111", sp, tr!("`if` に `else` がありません", "The `if` has no `else`")).notes.push(tr!(
                                "一行の `if` は `if <条件> then <値> else <値>` と書きます。値が無いことは `none` で言えます。",
                                "An `if` on one line is `if <condition> then <value> else <value>`; `none` says there is no value."
                            ));
                            return None;
                        }
                        let b = self.expr(c)?;
                        let sp = at.to(b.span);
                        let (ta, tb) = (Block { lets: vec![], answer: a }, Block { lets: vec![], answer: b });
                        Some(self.mk(sp, ExprKind::If(Box::new(cond), Box::new(ta), Box::new(tb))))
                    }
                    kw::TRY => {
                        self.err("E115", at, tr!("ここには `try` を書けません", "`try` cannot be written here")).notes.push(try_note());
                        None
                    }
                    w if kw::is_reserved(w) => {
                        let w = w.to_string();
                        self.err("E001", at, tr!("`{w}` を読めません。ここには値が要ります", "`{w}` cannot be read here: a value is needed"));
                        None
                    }
                    _ => {
                        c.next();
                        if c.peek() == Some(&Tok::FatArrow) {
                            c.next();
                            let body = self.expr(c)?;
                            let sp = at.to(body.span);
                            return Some(self.mk(sp, ExprKind::Lambda(vec![(w, at)], Box::new(body))));
                        }
                        Some(self.mk(at, ExprKind::Var(w)))
                    }
                }
            }
            _ => {
                self.expected(c, tr!("値", "a value"));
                None
            }
        }
    }
}

impl P<'_> {
    /// The parameters of a lambda in parentheses, each a name and the type written after it, up to the
    /// `=>` after the `)` (DESIGN 27.1).
    #[allow(clippy::type_complexity)]
    fn lambda_params(&mut self, c: &mut Cur) -> R<(Vec<(String, Span)>, Vec<Option<TypeRef>>)> {
        c.next();
        let (mut words, mut tys) = (Vec::new(), Vec::new());
        if !c.eat(&Tok::RParen) {
            loop {
                let (w, sp) = match c.peek() {
                    Some(Tok::Word(w)) => (w.clone(), c.span()),
                    _ => {
                        self.expected(c, tr!("ラムダの引数の名前", "the name of a parameter of the lambda"));
                        return None;
                    }
                };
                c.next();
                if kw::is_reserved(&w) {
                    self.err("E002", sp, tr!("`{w}` は予約語なので、名前にできません", "`{w}` is a keyword and cannot be a name")).notes.push(reserved_note());
                    return None;
                }
                let t = if c.eat(&Tok::Colon) { Some(self.ty(c)?) } else { None };
                words.push((w, sp));
                tys.push(t);
                if c.eat(&Tok::Comma) {
                    continue;
                }
                if c.eat(&Tok::RParen) {
                    break;
                }
                self.expected(c, tr!("`,` か `)`", "`,` or `)`"));
                return None;
            }
        }
        // the `=>` the `)` was found to have after it
        c.next();
        Some((words, tys))
    }
}

fn reserved_note() -> Text {
    let words = kw::RESERVED.iter().map(|w| format!("`{w}`")).collect::<Vec<_>>().join(" ");
    tr!(
        "名前にできない語は、式の中で別の意味に読める {words} の 14 語です。ほかの語に替えてください。",
        "The fourteen words an expression reads as something else cannot be names: {words}. Choose another word."
    )
}

/// Where `try` is written (E115, DESIGN 26.3).
pub fn try_note() -> Text {
    tr!(
        "`try` は `let x = try <値>` の形で、関数の答えに続くブロック（関数の本体と、そこから答えにつながる `if` と `match` の分岐）の `let` にだけ書けます。`err` か `none` なら、それが関数の答えになります。",
        "`try` is written as `let x = try <value>`, in a `let` of a block the function answers with (its body, and the arms of an `if` or a `match` that answer for it): an `err` or a `none` of the value is then the function's answer."
    )
}

fn example_note() -> Text {
    tr!(
        "例は `<関数>(<引数>: <値>, …) = <答え>` か、失敗するなら `<関数>(…) fails <種類>` と一行に書きます。続きは、その行より深く字下げして次の行に書けます。",
        "An example is `<function>(<parameter>: <value>, …) = <answer>`, or `<function>(…) fails <kind>` for one that fails, on a line; it may go on on the lines below, indented deeper."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(src: &str) -> Module {
        let p = parse("t.kn", src);
        assert!(p.diags.is_empty(), "{:?}", p.diags.iter().map(|d| d.render(ritsu_base::text::Lang::En)).collect::<Vec<_>>());
        p.module.expect("a module")
    }

    fn codes(src: &str) -> Vec<&'static str> {
        parse("t.kn", src).diags.iter().map(|d| d.code).collect()
    }

    #[test]
    fn a_function_with_blocks() {
        let m = ok("module m v1\n\nfn f(x: number range >=0 <=9) -> number? total\n  let y = x + 1\n  if y >= 3\n    y\n  else if y = 2\n    none\n  else\n    0\n");
        assert_eq!(m.fns.len(), 1);
        let f = &m.fns[0];
        assert_eq!(f.body.lets.len(), 1);
        assert!(matches!(f.body.answer.kind, ExprKind::If(..)));
        assert!(f.effect.as_ref().is_some_and(|e| e.total));
    }

    #[test]
    fn the_parts_in_their_order() {
        assert_eq!(codes("fn f() -> number\n  1\n"), vec!["E003"]);
        assert_eq!(codes("module m v1\nfn f() -> number\n  1\nuse rule r from \"r.rule\"\n"), vec!["E003"]);
        assert_eq!(codes("module m v1\nfn f() -> number\n  let x = 1\n"), vec!["E111"]);
        assert_eq!(codes("module m v1\nfn f(x: bool) -> number\n  if x\n    1\n"), vec!["E111"]);
        assert_eq!(codes("module m v1\nfn f(x: number range >=0 <=1) -> bool\n  0 < x < 1\n"), vec!["E001"]);
        assert_eq!(codes("module m v1\nfn f() -> number\n  let in = 3\n  3\n"), vec!["E002"]);
        assert_eq!(codes("module m v1\nrecord R\n  a : number range >=0 <=1\n   b : number range >=0 <=1\n"), vec!["E004"]);
        assert_eq!(codes("module m v1\nenum e = a | b default\n"), vec!["E003"]);
    }

    #[test]
    fn examples_go_on_on_deeper_lines() {
        let m = ok("module m v1\nfn f(x: number range >=0 <=9) -> number\n  x\nexamples\n  f(x: 1)\n    = 1\n  f(x: 2) fails range\n");
        assert_eq!(m.examples.len(), 2);
        assert_eq!(codes("module m v1\nfn f() -> number\n  1\nexamples\n  1 = 1\n"), vec!["E602"]);
    }
}
