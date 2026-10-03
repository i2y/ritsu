//! The syntax tree of a book, every node with its place in the source (PLAN B3).

use crate::diag::{Diag, Text};
use crate::syntax::{self, Line, Tok, Token};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Span {
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Book {
    pub name: Name,
    pub version: u32,
    pub description: Option<String>,
    pub units: Vec<Unit>,
    pub accounts: Vec<Account>,
    pub transfers: Vec<Transfer>,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub name: Name,
    pub scale: Option<(String, Span)>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: Name,
    pub ty: Name,
}

#[derive(Clone, Debug)]
pub struct Account {
    pub name: Name,
    /// None when the account kind is written without parentheses
    pub params: Option<Vec<Param>>,
    pub unit: Name,
    pub outside: Option<Span>,
    pub descriptions: Vec<(String, Span)>,
    pub bounds: Vec<BoundLine>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct BoundLine {
    /// `at most` (true) or `at least` (false)
    pub upper: bool,
    pub value: String,
    pub value_span: Span,
    pub refusal: Option<Name>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Transfer {
    pub name: Name,
    pub params: Vec<Param>,
    pub descriptions: Vec<(String, Span)>,
    pub keys: Vec<KeyLine>,
    pub pendings: Vec<PendingLine>,
    pub moves: Vec<MoveLine>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct KeyLine {
    pub names: Vec<Name>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum PendingKind {
    After { n: String, n_span: Span, unit: Name },
    Never,
    /// `pending` and nothing after it (E040)
    Missing,
}

#[derive(Clone, Debug)]
pub struct PendingLine {
    pub kind: PendingKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct MoveLine {
    pub amount: AmountExpr,
    pub from: AccountRef,
    pub to: AccountRef,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum AmountExpr {
    Name(Name),
    Num(String, Span),
}

#[derive(Clone, Debug)]
pub struct AccountRef {
    pub name: Name,
    pub args: Option<Vec<ArgExpr>>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ArgExpr {
    Name(Name),
    Str(String, Span),
}

/// The shapes of the lines, for the messages that say how to write one.
fn form_book() -> Text {
    tr!("`book <名前> v<番号>`", "`book <name> v<number>`")
}
fn form_description() -> Text {
    tr!("`description \"<説明>\"`", "`description \"<text>\"`")
}
fn form_unit() -> Text {
    tr!("`unit <名前>` か `unit <名前> scale <小数の桁数>`", "`unit <name>` or `unit <name> scale <decimal places>`")
}
fn form_account() -> Text {
    tr!(
        "`account <名前>(<引数>: string, …) : <単位>`（引数が無ければ括弧は書かない。外の勘定は最後に `outside`）",
        "`account <name>(<parameter>: string, …) : <unit>` (no parentheses without parameters; `outside` at the end for an outside account)"
    )
}
fn form_bound() -> Text {
    tr!("`at least <数> refused as <理由>` か `at most <数> refused as <理由>`", "`at least <number> refused as <reason>` or `at most <number> refused as <reason>`")
}
fn form_transfer() -> Text {
    tr!("`transfer <名前>(<引数>: <型>, …)`（型は string か単位の名前）", "`transfer <name>(<parameter>: <type>, …)` (the type is string or a unit)")
}
fn form_key() -> Text {
    tr!("`key <引数>, <引数>, …`", "`key <parameter>, <parameter>, …`")
}
fn form_pending() -> Text {
    tr!(
        "`pending expires after <数> <seconds|minutes|hours|days>` か `pending never expires`",
        "`pending expires after <n> <seconds|minutes|hours|days>` or `pending never expires`"
    )
}
fn form_move() -> Text {
    tr!("`move <額> from <勘定> to <勘定>`", "`move <amount> from <account> to <account>`")
}

fn show(t: &Tok) -> String {
    match t {
        Tok::Word(w) => w.clone(),
        Tok::Num(n) => n.clone(),
        Tok::Str(s) => format!("\"{s}\""),
        Tok::LParen => "(".into(),
        Tok::RParen => ")".into(),
        Tok::Colon => ":".into(),
        Tok::Comma => ",".into(),
    }
}

struct Cur<'a> {
    line: &'a Line,
    i: usize,
    form: Text,
}

impl<'a> Cur<'a> {
    fn new(line: &'a Line, form: Text) -> Cur<'a> {
        Cur { line, i: 0, form }
    }

    fn peek(&self) -> Option<&'a Token> {
        self.line.toks.get(self.i)
    }

    fn span_of(&self, t: &Token) -> Span {
        Span { line: self.line.no, col: t.col, len: t.len }
    }

    fn here(&self) -> Span {
        match self.peek() {
            Some(t) => self.span_of(t),
            None => {
                let col = self.line.toks.last().map(|t| t.col + t.len).unwrap_or(self.line.indent + 1);
                Span { line: self.line.no, col, len: 0 }
            }
        }
    }

    fn err(&self) -> Diag {
        let at = self.here();
        let msg = match self.peek() {
            Some(t) => {
                let s = show(&t.tok);
                tr!("`{s}` は読めません。{{form}} の形で書いてください", "unexpected `{s}`; write {{form}}")
            }
            None => tr!("行が途中で終わっています。{{form}} の形で書いてください", "the line ends early; write {{form}}"),
        }
        .sub("form", &self.form);
        Diag::error("E001", at.line, at.col, msg)
    }

    fn at_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Token { tok: Tok::Word(x), .. }) if x == w)
    }

    fn at(&self, t: &Tok) -> bool {
        self.peek().is_some_and(|x| &x.tok == t)
    }

    fn word(&mut self, w: &str) -> Result<Span, Diag> {
        if self.at_word(w) {
            let s = self.here();
            self.i += 1;
            Ok(s)
        } else {
            Err(self.err())
        }
    }

    fn tok(&mut self, t: Tok) -> Result<Span, Diag> {
        if self.at(&t) {
            let s = self.here();
            self.i += 1;
            Ok(s)
        } else {
            Err(self.err())
        }
    }

    fn name(&mut self) -> Result<Name, Diag> {
        match self.peek() {
            Some(Token { tok: Tok::Word(w), .. }) => {
                let span = self.here();
                if syntax::reserved(w) {
                    return Err(Diag::error(
                        "E001",
                        span.line,
                        span.col,
                        tr!("`{w}` はキーワードなので名前に使えません", "`{w}` is a keyword and cannot be a name"),
                    ));
                }
                self.i += 1;
                Ok(Name { text: w.clone(), span })
            }
            _ => Err(self.err()),
        }
    }

    fn num(&mut self) -> Result<(String, Span), Diag> {
        match self.peek() {
            Some(Token { tok: Tok::Num(n), .. }) => {
                let s = self.here();
                self.i += 1;
                Ok((n.clone(), s))
            }
            _ => Err(self.err()),
        }
    }

    fn string(&mut self) -> Result<(String, Span), Diag> {
        match self.peek() {
            Some(Token { tok: Tok::Str(v), .. }) => {
                let s = self.here();
                self.i += 1;
                Ok((v.clone(), s))
            }
            _ => Err(self.err()),
        }
    }

    fn end(&self) -> Result<(), Diag> {
        if self.peek().is_none() { Ok(()) } else { Err(self.err()) }
    }

    fn start(&self) -> Span {
        Span { line: self.line.no, col: self.line.indent + 1, len: 0 }
    }
}

fn params(c: &mut Cur) -> Result<Vec<Param>, Diag> {
    let mut ps = Vec::new();
    c.tok(Tok::LParen)?;
    if c.at(&Tok::RParen) {
        c.i += 1;
        return Ok(ps);
    }
    loop {
        let name = c.name()?;
        c.tok(Tok::Colon)?;
        let ty = match c.peek() {
            Some(Token { tok: Tok::Word(w), .. }) => {
                let span = c.here();
                c.i += 1;
                Name { text: w.clone(), span }
            }
            _ => return Err(c.err()),
        };
        ps.push(Param { name, ty });
        if c.at(&Tok::Comma) {
            c.i += 1;
            continue;
        }
        c.tok(Tok::RParen)?;
        return Ok(ps);
    }
}

fn account_ref(c: &mut Cur) -> Result<AccountRef, Diag> {
    let name = c.name()?;
    let span = name.span;
    if !c.at(&Tok::LParen) {
        return Ok(AccountRef { name, args: None, span });
    }
    c.i += 1;
    let mut args = Vec::new();
    if !c.at(&Tok::RParen) {
        loop {
            match c.peek() {
                Some(Token { tok: Tok::Str(_), .. }) => {
                    let (v, s) = c.string()?;
                    args.push(ArgExpr::Str(v, s));
                }
                _ => args.push(ArgExpr::Name(c.name()?)),
            }
            if c.at(&Tok::Comma) {
                c.i += 1;
                continue;
            }
            break;
        }
    }
    c.tok(Tok::RParen)?;
    Ok(AccountRef { name, args: Some(args), span })
}

enum Open {
    None,
    Account(usize),
    Transfer(usize),
}

/// Read the lines into a book. A line that does not read is reported (E001) and left out;
/// when any line is left out, the caller stops here (the rest would be guesses).
pub fn parse(lines: &[Line]) -> (Option<Book>, Vec<Diag>) {
    let mut diags = Vec::new();
    let mut it = lines.iter().peekable();
    let Some(first) = it.next() else {
        diags.push(Diag::error("E001", 1, 1, tr!("帳簿が空です。最初の行に {{form}} を書いてください", "the book is empty; start it with {{form}}").sub("form", &form_book())));
        return (None, diags);
    };
    let head = (|| -> Result<(Name, u32), Diag> {
        let mut c = Cur::new(first, form_book());
        if first.indent != 0 {
            return Err(Diag::error("E001", first.no, 1, tr!("`book` の行は字下げしません", "the `book` line is not indented")));
        }
        c.word("book")?;
        let name = c.name()?;
        let vspan = c.here();
        let version = match c.peek() {
            Some(Token { tok: Tok::Word(w), .. }) if w.len() > 1 && w.starts_with('v') && w[1..].chars().all(|d| d.is_ascii_digit()) => {
                w[1..].parse::<u32>().ok().filter(|n| *n >= 1)
            }
            _ => None,
        };
        let Some(version) = version else {
            return Err(Diag::error("E001", vspan.line, vspan.col, tr!("バージョンは `v1` のように v と 1 以上の数で書きます", "the version is a v and a number from 1, like `v1`")));
        };
        c.i += 1;
        c.end()?;
        Ok((name, version))
    })();
    let (name, version) = match head {
        Ok(x) => x,
        Err(d) => {
            diags.push(d);
            return (None, diags);
        }
    };
    let mut book = Book { name, version, description: None, units: vec![], accounts: vec![], transfers: vec![] };
    if let Some(l) = it.peek() {
        if l.indent == 0 && matches!(l.toks.first(), Some(Token { tok: Tok::Word(w), .. }) if w == "description") {
            let l = it.next().unwrap();
            let mut c = Cur::new(l, form_description());
            let r = c.word("description").and_then(|_| c.string()).and_then(|(s, _)| c.end().map(|_| s));
            match r {
                Ok(s) => book.description = Some(s),
                Err(d) => diags.push(d),
            }
        }
    }
    let mut open = Open::None;
    let mut body_indent: Option<usize> = None;
    for l in it {
        let head_word = match l.toks.first() {
            Some(Token { tok: Tok::Word(w), .. }) => w.as_str(),
            _ => "",
        };
        if l.indent > 0 {
            let ok_indent = match body_indent {
                None => {
                    body_indent = Some(l.indent);
                    true
                }
                Some(n) => n == l.indent,
            };
            if !ok_indent {
                diags.push(Diag::error(
                    "E001",
                    l.no,
                    1,
                    tr!("字下げがそろっていません。同じブロックの行は同じ数の空白で字下げします", "the indentation does not line up with the lines above it in the same block"),
                ));
                continue;
            }
            match open {
                Open::None => {
                    diags.push(Diag::error(
                        "E001",
                        l.no,
                        1,
                        tr!("字下げした行を書けるのは `account` と `transfer` の下だけです", "only lines under `account` and `transfer` are indented"),
                    ));
                }
                Open::Account(a) => match account_line(l) {
                    Ok(AccountLine::Description(s, sp)) => book.accounts[a].descriptions.push((s, sp)),
                    Ok(AccountLine::Bound(b)) => book.accounts[a].bounds.push(b),
                    Err(d) => diags.push(d),
                },
                Open::Transfer(t) => match transfer_line(l) {
                    Ok(TransferLine::Description(s, sp)) => book.transfers[t].descriptions.push((s, sp)),
                    Ok(TransferLine::Key(k)) => book.transfers[t].keys.push(k),
                    Ok(TransferLine::Pending(p)) => book.transfers[t].pendings.push(p),
                    Ok(TransferLine::Move(m)) => book.transfers[t].moves.push(m),
                    Err(d) => diags.push(d),
                },
            }
            continue;
        }
        body_indent = None;
        open = Open::None;
        match head_word {
            "unit" => match unit_line(l) {
                Ok(u) => book.units.push(u),
                Err(d) => diags.push(d),
            },
            "account" => match account_head(l) {
                Ok(a) => {
                    book.accounts.push(a);
                    open = Open::Account(book.accounts.len() - 1);
                }
                Err(d) => diags.push(d),
            },
            "transfer" => match transfer_head(l) {
                Ok(t) => {
                    book.transfers.push(t);
                    open = Open::Transfer(book.transfers.len() - 1);
                }
                Err(d) => diags.push(d),
            },
            "book" => diags.push(Diag::error("E001", l.no, 1, tr!("`book` の行は先頭に一度だけ書きます", "`book` comes once, on the first line"))),
            "description" => diags.push(Diag::error(
                "E001",
                l.no,
                1,
                tr!(
                    "帳簿の `description` は `book` の行のすぐ下に書きます。勘定や振替の説明は、その下に字下げして書きます",
                    "the book's `description` goes right under the `book` line; an account's or a transfer's goes indented under it"
                ),
            )),
            "key" | "pending" | "move" | "at" => diags.push(Diag::error(
                "E001",
                l.no,
                1,
                tr!("`{head_word}` の行は、`account` か `transfer` の下に字下げして書きます", "a `{head_word}` line goes indented under an `account` or a `transfer`"),
            )),
            _ => {
                let s = l.toks.first().map(|t| show(&t.tok)).unwrap_or_default();
                diags.push(Diag::error(
                    "E001",
                    l.no,
                    1,
                    tr!(
                        "`{s}` は読めません。行の頭に書けるのは `unit`、`account`、`transfer` です",
                        "unexpected `{s}`; a line starts with `unit`, `account` or `transfer`"
                    ),
                ));
            }
        }
    }
    (Some(book), diags)
}

fn unit_line(l: &Line) -> Result<Unit, Diag> {
    let mut c = Cur::new(l, form_unit());
    c.word("unit")?;
    let name = c.name()?;
    let scale = if c.at_word("scale") {
        c.i += 1;
        Some(c.num()?)
    } else {
        None
    };
    c.end()?;
    Ok(Unit { name, scale, line: l.no })
}

fn account_head(l: &Line) -> Result<Account, Diag> {
    let mut c = Cur::new(l, form_account());
    c.word("account")?;
    let name = c.name()?;
    let params = if c.at(&Tok::LParen) { Some(params(&mut c)?) } else { None };
    c.tok(Tok::Colon)?;
    let unit = c.name()?;
    let outside = if c.at_word("outside") {
        let s = c.here();
        c.i += 1;
        Some(s)
    } else {
        None
    };
    c.end()?;
    Ok(Account { name, params, unit, outside, descriptions: vec![], bounds: vec![], line: l.no })
}

enum AccountLine {
    Description(String, Span),
    Bound(BoundLine),
}

fn account_line(l: &Line) -> Result<AccountLine, Diag> {
    if matches!(l.toks.first(), Some(Token { tok: Tok::Word(w), .. }) if w == "description") {
        let mut c = Cur::new(l, form_description());
        let sp = c.word("description")?;
        let (s, _) = c.string()?;
        c.end()?;
        return Ok(AccountLine::Description(s, sp));
    }
    let mut c = Cur::new(l, form_bound());
    if !c.at_word("at") {
        let s = l.toks.first().map(|t| show(&t.tok)).unwrap_or_default();
        return Err(Diag::error(
            "E001",
            l.no,
            l.indent + 1,
            tr!(
                "`{s}` は読めません。勘定の下に書けるのは `description`、`at least`、`at most` の行です",
                "unexpected `{s}`; the lines under an account are `description`, `at least` and `at most`"
            ),
        ));
    }
    let span = c.start();
    c.i += 1;
    let upper = if c.at_word("least") {
        false
    } else if c.at_word("most") {
        true
    } else {
        return Err(c.err());
    };
    c.i += 1;
    let (value, value_span) = c.num()?;
    let refusal = if c.at_word("refused") {
        c.i += 1;
        c.word("as")?;
        Some(c.name()?)
    } else {
        None
    };
    c.end()?;
    Ok(AccountLine::Bound(BoundLine { upper, value, value_span, refusal, span }))
}

fn transfer_head(l: &Line) -> Result<Transfer, Diag> {
    let mut c = Cur::new(l, form_transfer());
    c.word("transfer")?;
    let name = c.name()?;
    let ps = params(&mut c)?;
    c.end()?;
    Ok(Transfer { name, params: ps, descriptions: vec![], keys: vec![], pendings: vec![], moves: vec![], line: l.no })
}

enum TransferLine {
    Description(String, Span),
    Key(KeyLine),
    Pending(PendingLine),
    Move(MoveLine),
}

fn transfer_line(l: &Line) -> Result<TransferLine, Diag> {
    let head = match l.toks.first() {
        Some(Token { tok: Tok::Word(w), .. }) => w.as_str(),
        _ => "",
    };
    match head {
        "description" => {
            let mut c = Cur::new(l, form_description());
            let sp = c.word("description")?;
            let (s, _) = c.string()?;
            c.end()?;
            Ok(TransferLine::Description(s, sp))
        }
        "key" => {
            let mut c = Cur::new(l, form_key());
            let span = c.start();
            c.word("key")?;
            let mut names = vec![c.name()?];
            while c.at(&Tok::Comma) {
                c.i += 1;
                names.push(c.name()?);
            }
            c.end()?;
            Ok(TransferLine::Key(KeyLine { names, span }))
        }
        "pending" => {
            let mut c = Cur::new(l, form_pending());
            let span = c.start();
            c.word("pending")?;
            let kind = if c.peek().is_none() {
                PendingKind::Missing
            } else if c.at_word("never") {
                c.i += 1;
                c.word("expires")?;
                PendingKind::Never
            } else {
                c.word("expires")?;
                c.word("after")?;
                let (n, n_span) = c.num()?;
                let unit = match c.peek() {
                    Some(Token { tok: Tok::Word(w), .. }) if syntax::duration(w).is_some() => {
                        let s = c.here();
                        c.i += 1;
                        Name { text: w.clone(), span: s }
                    }
                    _ => return Err(c.err()),
                };
                PendingKind::After { n, n_span, unit }
            };
            c.end()?;
            Ok(TransferLine::Pending(PendingLine { kind, span }))
        }
        "move" => {
            let mut c = Cur::new(l, form_move());
            let span = c.start();
            c.word("move")?;
            let amount = match c.peek() {
                Some(Token { tok: Tok::Num(_), .. }) => {
                    let (n, s) = c.num()?;
                    AmountExpr::Num(n, s)
                }
                _ => AmountExpr::Name(c.name()?),
            };
            c.word("from")?;
            let from = account_ref(&mut c)?;
            c.word("to")?;
            let to = account_ref(&mut c)?;
            c.end()?;
            Ok(TransferLine::Move(MoveLine { amount, from, to, span }))
        }
        _ => {
            let s = l.toks.first().map(|t| show(&t.tok)).unwrap_or_default();
            Err(Diag::error(
                "E001",
                l.no,
                l.indent + 1,
                tr!(
                    "`{s}` は読めません。振替の下に書けるのは `description`、`key`、`pending`、`move` の行です",
                    "unexpected `{s}`; the lines under a transfer are `description`, `key`, `pending` and `move`"
                ),
            ))
        }
    }
}
