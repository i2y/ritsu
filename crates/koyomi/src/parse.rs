//! Lines of tokens into a [`File`] (DESIGN 1, PLAN B.3). Produces E002–E005 here, and E001
//! and E006 come from the lexer. One error a line: after it, the rest of the line is skipped.

use crate::ast::*;
use crate::date::{Day, Missing};
use crate::diag::Diag;
use ritsu_base::text::Text;
use crate::kw;
use crate::lex::{self, Line, Tok, Token};

pub struct Parsed {
    pub file: Option<File>,
    pub diags: Vec<Diag>,
}

/// A syntax error inside a line: the column, the code and the message.
struct Bad {
    col: usize,
    code: &'static str,
    msg: Text,
    notes: Vec<Text>,
}

fn bad(col: usize, msg: Text) -> Bad {
    Bad { col, code: "E002", msg, notes: vec![] }
}

impl Bad {
    fn note(mut self, t: Text) -> Bad {
        self.notes.push(t);
        self
    }
}

/// How a token reads in a message.
fn show(t: &Tok) -> String {
    match t {
        Tok::Word(w) => w.clone(),
        Tok::Int(n) => n.to_string(),
        Tok::Date(d) => d.to_string(),
        Tok::MonthDay(m, d) => format!("{m:02}-{d:02}"),
        Tok::Time(h, m) => format!("{h:02}:{m:02}"),
        Tok::Str(s) => format!("\"{s}\""),
        Tok::Sha(s) => format!("sha256:{s}"),
        Tok::LParen => "(".into(),
        Tok::RParen => ")".into(),
        Tok::Colon => ":".into(),
        Tok::Comma => ",".into(),
        Tok::Pipe => "|".into(),
        Tok::At => "@".into(),
        Tok::DotDot => "..".into(),
        Tok::Arrow => "->".into(),
        Tok::Plus => "+".into(),
        Tok::Minus => "-".into(),
        Tok::Eq => "=".into(),
        Tok::Lt => "<".into(),
        Tok::Le => "<=".into(),
        Tok::Gt => ">".into(),
        Tok::Ge => ">=".into(),
    }
}

/// A cursor over the tokens of one line.
struct Cur<'a> {
    line: &'a Line,
    i: usize,
}

impl<'a> Cur<'a> {
    fn new(line: &'a Line) -> Cur<'a> {
        Cur { line, i: 0 }
    }

    fn peek(&self) -> Option<&'a Tok> {
        self.line.tokens.get(self.i).map(|t| &t.tok)
    }

    fn peek_at(&self, k: usize) -> Option<&'a Tok> {
        self.line.tokens.get(self.i + k).map(|t| &t.tok)
    }

    fn token(&self) -> Option<&'a Token> {
        self.line.tokens.get(self.i)
    }

    fn bump(&mut self) -> Option<&'a Token> {
        let t = self.line.tokens.get(self.i);
        self.i += 1;
        t
    }

    /// The column of the next token, or just past the end of the line.
    fn col(&self) -> usize {
        match self.token() {
            Some(t) => t.col,
            None => self.line.chars.len() + 1,
        }
    }

    fn span(&self) -> Span {
        Span { line: self.line.no, col: self.col() }
    }

    fn at_end(&self) -> bool {
        self.i >= self.line.tokens.len()
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Word(x)) if x == w)
    }

    fn eat_word(&mut self, w: &str) -> bool {
        if self.is_word(w) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn found(&self) -> Text {
        match self.peek() {
            Some(t) => tr!("`{}` があります", "found `{}`", show(t)),
            None => tr!("行が終わっています", "the line ends"),
        }
    }

    fn want_word(&mut self, w: &str) -> Result<(), Bad> {
        if self.eat_word(w) {
            Ok(())
        } else {
            Err(bad(self.col(), tr!("ここには `{w}` が要ります（{}）", "`{w}` is needed here ({})", self.found().ja; self.found().en)))
        }
    }

    fn want(&mut self, t: &Tok, what: &str) -> Result<(), Bad> {
        if self.eat(t) {
            Ok(())
        } else {
            Err(bad(self.col(), tr!("ここには `{what}` が要ります（{}）", "`{what}` is needed here ({})", self.found().ja; self.found().en)))
        }
    }

    fn name(&mut self, what: &Text) -> Result<(String, Span), Bad> {
        let span = self.span();
        match self.peek() {
            Some(Tok::Word(w)) => {
                self.i += 1;
                Ok((w.clone(), span))
            }
            _ => Err(bad(span.col, tr!("ここには{}の名前が要ります（{}）", "{} needs a name here ({})", what.ja, what.en; self.found().ja, self.found().en))),
        }
    }

    fn string(&mut self, what: &Text) -> Result<String, Bad> {
        match self.peek() {
            Some(Tok::Str(s)) => {
                self.i += 1;
                Ok(s.clone())
            }
            _ => Err(bad(self.col(), tr!("ここには{}を `\"…\"` で書きます（{}）", "{} goes here in quotes, `\"…\"` ({})", what.ja, what.en; self.found().ja, self.found().en))),
        }
    }

    fn date(&mut self) -> Result<(Day, Span), Bad> {
        let span = self.span();
        match self.peek() {
            Some(Tok::Date(d)) => {
                self.i += 1;
                Ok((*d, span))
            }
            _ => Err(bad(span.col, tr!("ここには日付（`2026-01-01` の形）が要ります（{}）", "a date (`2026-01-01`) is needed here ({})", self.found().ja; self.found().en))),
        }
    }

    fn done(&self) -> Result<(), Bad> {
        match self.peek() {
            None => Ok(()),
            Some(t) => Err(bad(self.col(), tr!("`{}` はここには書けません", "`{}` cannot be written here", show(t)))),
        }
    }
}

/// `受領日(received)`.
fn name_with_alias(c: &mut Cur, what: &Text) -> Result<Name, Bad> {
    let (text, span) = c.name(what)?;
    let mut alias = None;
    if c.peek() == Some(&Tok::LParen) {
        c.bump();
        let s = c.span();
        let (a, _) = c.name(&tr!("別名", "the alias"))?;
        c.want(&Tok::RParen, ")")?;
        alias = Some((a, s));
    }
    Ok(Name { text, alias, span })
}

/// `@民法 第141条, 第143条` at the end of a line, if there is one.
fn cite(c: &mut Cur) -> Result<Option<Cite>, Bad> {
    if c.peek() != Some(&Tok::At) {
        return Ok(None);
    }
    let span = c.span();
    c.bump();
    let (source, _) = c.name(&tr!("引く出典", "the citation"))?;
    let mut fragments = Vec::new();
    if let Some(Tok::Word(_)) = c.peek() {
        loop {
            let (f, s) = c.name(&tr!("引く条", "the cited article"))?;
            fragments.push((f, s));
            if !c.eat(&Tok::Comma) {
                break;
            }
        }
    }
    c.done()?;
    Ok(Some(Cite { source, fragments, span }))
}

/// The column where the citation starts, or the end of the line.
fn cite_col(line: &Line) -> usize {
    line.tokens.iter().find(|t| t.tok == Tok::At).map(|t| t.col).unwrap_or(line.chars.len() + 1)
}

fn missing(c: &mut Cur) -> Result<Option<(Missing, Span)>, Bad> {
    if !c.eat_word(kw::ELSE) {
        return Ok(None);
    }
    let span = c.span();
    let m = match c.peek() {
        Some(Tok::Word(w)) if w == kw::END_OF_MONTH => Missing::EndOfMonth,
        Some(Tok::Word(w)) if w == kw::START_OF_NEXT_MONTH => Missing::StartOfNextMonth,
        Some(Tok::Word(w)) if w == kw::REJECT => Missing::Reject,
        _ => {
            return Err(bad(span.col, tr!(
                "`else` のあとには、無い日の扱い（`end_of_month`、`start_of_next_month`、`reject` のどれか）を書きます（{}）",
                "`else` is followed by what to do with a missing day: `end_of_month`, `start_of_next_month` or `reject` ({})",
                c.found().ja;
                c.found().en
            )));
        }
    };
    c.bump();
    Ok(Some((m, span)))
}

/// A number: written out, or the name of an integer input.
fn arg(c: &mut Cur, what: &Text) -> Result<Arg, Bad> {
    let span = c.span();
    match c.peek() {
        Some(Tok::Int(n)) => {
            c.bump();
            Ok(Arg::Lit(*n))
        }
        Some(Tok::Word(w)) if !kw::is_reserved(w) => {
            c.bump();
            Ok(Arg::Name(w.clone(), span))
        }
        _ => Err(bad(span.col, tr!("ここには{}（数か、整数の入力の名前）が要ります（{}）", "{} goes here: a number or the name of an integer input ({})", what.ja, what.en; c.found().ja, c.found().en))),
    }
}

/// `+1`, `-1`, `+支払の月`, or nothing (which is `+0`).
fn months_away(c: &mut Cur) -> Result<(Sign, Arg), Bad> {
    let sign = if c.eat(&Tok::Plus) {
        Sign::Plus
    } else if c.eat(&Tok::Minus) {
        Sign::Minus
    } else {
        return Ok((Sign::Plus, Arg::Lit(0)));
    };
    Ok((sign, arg(c, &tr!("何か月先か", "how many months away"))?))
}

fn unit_day(c: &mut Cur) -> bool {
    c.eat_word(kw::DAY) || c.eat_word(kw::DAYS)
}

/// One operation (DESIGN 1.6), without the `if closed` in front of it.
fn op(c: &mut Cur, nested: bool) -> Result<Op, Bad> {
    let col = c.col();
    match c.peek() {
        Some(Tok::Plus) | Some(Tok::Minus) => {
            let sign = if c.eat(&Tok::Plus) { Sign::Plus } else {
                c.bump();
                Sign::Minus
            };
            let n = arg(c, &tr!("足す数", "the number to add"))?;
            if c.eat_word(kw::BUSINESS) {
                if !unit_day(c) {
                    return Err(bad(c.col(), tr!("`business` のあとには `days` を書きます", "`business` is followed by `days`")));
                }
                return Ok(Op::BusinessDays(sign, n));
            }
            if unit_day(c) {
                return Ok(Op::Days(sign, n));
            }
            if c.eat_word(kw::MONTH) || c.eat_word(kw::MONTHS) {
                return Ok(Op::Months(sign, n, missing(c)?));
            }
            if c.eat_word(kw::YEAR) || c.eat_word(kw::YEARS) {
                return Ok(Op::Years(sign, n, missing(c)?));
            }
            Err(bad(c.col(), tr!(
                "数のあとには単位（`days`、`business days`、`months`、`years`）を書きます（{}）",
                "the number is followed by its unit: `days`, `business days`, `months` or `years` ({})",
                c.found().ja;
                c.found().en
            )))
        }
        Some(Tok::Word(w)) => match w.as_str() {
            kw::DAY => {
                c.bump();
                let n = arg(c, &tr!("日", "the day of the month"))?;
                c.want_word(kw::OF)?;
                c.want_word(kw::MONTH)?;
                let (s, k) = months_away(c)?;
                Ok(Op::DayOfMonth(n, s, k, missing(c)?))
            }
            kw::START | kw::END => {
                let start = w == kw::START;
                c.bump();
                c.want_word(kw::OF)?;
                c.want_word(kw::MONTH)?;
                let (s, k) = months_away(c)?;
                Ok(if start { Op::StartOfMonth(s, k) } else { Op::EndOfMonth(s, k) })
            }
            kw::CLOSE => {
                c.bump();
                if c.eat_word(kw::END) {
                    c.want_word(kw::OF)?;
                    c.want_word(kw::MONTH)?;
                    return Ok(Op::CloseEndOfMonth);
                }
                c.want_word(kw::DAY)?;
                let n = arg(c, &tr!("締める日", "the closing day"))?;
                Ok(Op::CloseDay(n, missing(c)?))
            }
            kw::ROLL => {
                c.bump();
                let modified = c.eat_word(kw::MODIFIED);
                let conv = if c.eat_word(kw::FOLLOWING) {
                    if modified { Conv::ModifiedFollowing } else { Conv::Following }
                } else if c.eat_word(kw::PRECEDING) {
                    if modified { Conv::ModifiedPreceding } else { Conv::Preceding }
                } else {
                    return Err(bad(c.col(), tr!(
                        "`roll` のあとには `following`、`preceding`、`modified following`、`modified preceding` のどれかを書きます（{}）",
                        "`roll` is followed by `following`, `preceding`, `modified following` or `modified preceding` ({})",
                        c.found().ja;
                        c.found().en
                    )));
                };
                Ok(Op::Roll(conv))
            }
            kw::IF => {
                if nested {
                    return Err(bad(col, tr!("`if closed` は入れ子にできません。続けられる操作は一つです", "`if closed` cannot be nested; it takes one operation"))
                        .note(tr!("一般の条件分岐は持たず、「休みなら」だけを入れています（DESIGN 1.8）。", "There is no general branching, only \"if it is closed\" (DESIGN 1.8).")));
                }
                c.bump();
                c.want_word(kw::CLOSED)?;
                if c.at_end() || c.peek() == Some(&Tok::At) {
                    return Err(bad(c.col(), tr!("`if closed` のあとには、休みのときにする操作を一つ書きます", "`if closed` is followed by the one operation to do on a closed day")));
                }
                Ok(Op::IfClosed(Box::new(op(c, true)?)))
            }
            kw::AT if nested => Err(bad(col, tr!("`if closed` のあとに `at` は書けません", "`at` cannot follow `if closed`"))),
            _ => Err(unknown_op(col, c)),
        },
        _ => Err(unknown_op(col, c)),
    }
}

fn unknown_op(col: usize, c: &Cur) -> Bad {
    bad(col, tr!("ここには日付の操作を書きます（{}）", "an operation on the date goes here ({})", c.found().ja; c.found().en)).note(tr!(
        "書ける操作: `+ 30 days`、`+ 5 business days`、`+ 1 month else …`、`+ 1 year else …`、`day 10 of month +1`、`start of month +1`、`end of month +1`、`close day 20`、`close end of month`、`roll following`、`roll preceding`、`roll modified following`、`roll modified preceding`、`if closed <操作>`、最後の行に `at 09:00` か `at end of day`",
        "The operations: `+ 30 days`, `+ 5 business days`, `+ 1 month else …`, `+ 1 year else …`, `day 10 of month +1`, `start of month +1`, `end of month +1`, `close day 20`, `close end of month`, `roll following`, `roll preceding`, `roll modified following`, `roll modified preceding`, `if closed <operation>`, and on the last line `at 09:00` or `at end of day`."
    ))
}

/// `支払日` or `受領日 + 60 days` on one side of a claim.
fn date_ref(c: &mut Cur) -> Result<DateRef, Bad> {
    let (name, span) = c.name(&tr!("日付", "a date"))?;
    let mut offset = None;
    if matches!(c.peek(), Some(Tok::Plus) | Some(Tok::Minus)) {
        let sign = if c.eat(&Tok::Plus) { Sign::Plus } else {
            c.bump();
            Sign::Minus
        };
        let n = arg(c, &tr!("日数", "the number of days"))?;
        let business = c.eat_word(kw::BUSINESS);
        if !unit_day(c) {
            return Err(bad(c.col(), tr!(
                "条件の中で足せるのは `days` と `business days` だけです（{}）",
                "a claim adds only `days` or `business days` ({})",
                c.found().ja;
                c.found().en
            )));
        }
        offset = Some((sign, n, business));
    }
    Ok(DateRef { name, span, offset })
}

fn claim_body(c: &mut Cur) -> Result<ClaimKind, Bad> {
    if let (Some(Tok::Word(name)), Some(Tok::Word(is)), Some(Tok::Word(what))) = (c.peek(), c.peek_at(1), c.peek_at(2))
        && is == kw::IS
    {
        let span = c.span();
        if what == kw::MONOTONIC {
            let n = name.clone();
            c.i += 3;
            return Ok(ClaimKind::Monotonic(n, span));
        }
        if what == kw::OPEN {
            let n = name.clone();
            c.i += 3;
            return Ok(ClaimKind::IsOpen(DateRef { name: n, span, offset: None }));
        }
    }
    let a = date_ref(c)?;
    if c.eat_word(kw::IS) {
        if c.eat_word(kw::OPEN) {
            return Ok(ClaimKind::IsOpen(a));
        }
        return Err(bad(c.col(), tr!("`is` のあとには `open` か `monotonic` を書きます（{}）", "`is` is followed by `open` or `monotonic` ({})", c.found().ja; c.found().en)));
    }
    let cmp = match c.peek() {
        Some(Tok::Eq) => Cmp::Eq,
        Some(Tok::Lt) => Cmp::Lt,
        Some(Tok::Le) => Cmp::Le,
        Some(Tok::Gt) => Cmp::Gt,
        Some(Tok::Ge) => Cmp::Ge,
        _ => {
            return Err(bad(c.col(), tr!(
                "条件は `<日付> is open`、`<日付> is monotonic`、`<日付> <比較> <日付>` のどれかです（{}）",
                "a claim is `<date> is open`, `<date> is monotonic` or `<date> <comparison> <date>` ({})",
                c.found().ja;
                c.found().en
            ))
            .note(tr!("比較は `=`、`<`、`<=`、`>`、`>=` です。", "The comparisons are `=`, `<`, `<=`, `>` and `>=`.")));
        }
    };
    c.bump();
    let b = date_ref(c)?;
    Ok(ClaimKind::Compare(a, cmp, b))
}

/// One bound of a range: `>=2026-01-01` or `<=31`.
fn bound(c: &mut Cur, lo: &mut Option<(Lit, Span)>, hi: &mut Option<(Lit, Span)>) -> Result<(), Bad> {
    let span = c.span();
    let (is_lo, strict) = match c.peek() {
        Some(Tok::Ge) => (true, false),
        Some(Tok::Gt) => (true, true),
        Some(Tok::Le) => (false, false),
        Some(Tok::Lt) => (false, true),
        _ => return Err(bad(span.col, tr!("範囲の端は `>=` か `<=` で書きます（{}）", "an end of a range is written with `>=` or `<=` ({})", c.found().ja; c.found().en))),
    };
    c.bump();
    let neg = c.eat(&Tok::Minus);
    let lit = match c.peek() {
        Some(Tok::Date(d)) if !neg => Lit::Date(*d),
        Some(Tok::Int(n)) => Lit::Int(if neg { -*n } else { *n }),
        _ => return Err(bad(c.col(), tr!("範囲の端には日付か数を書きます（{}）", "an end of a range is a date or a number ({})", c.found().ja; c.found().en))),
    };
    c.bump();
    if strict {
        let (sym, fixed) = match (is_lo, lit) {
            (true, Lit::Date(d)) => (">", format!(">={}", d.next().map(|x| x.to_string()).unwrap_or_default())),
            (false, Lit::Date(d)) => ("<", format!("<={}", d.prev().map(|x| x.to_string()).unwrap_or_default())),
            (true, Lit::Int(n)) => (">", format!(">={}", n + 1)),
            (false, Lit::Int(n)) => ("<", format!("<={}", n - 1)),
        };
        return Err(Bad {
            col: span.col,
            code: "E013",
            msg: tr!("範囲の端は `>=` と `<=` で書きます（`{sym}` は使いません）", "Write the ends of a range with `>=` and `<=`, not `{sym}`"),
            notes: vec![tr!(
                "両端を含む書き方に一つにそろえています。この端なら `{fixed}` です。",
                "The ends are always written inclusive; this one is `{fixed}`."
            )],
        });
    }
    let slot = if is_lo { lo } else { hi };
    if slot.is_some() {
        return Err(bad(span.col, tr!("範囲のこちらの端は二度書かれています", "this end of the range is written twice")));
    }
    *slot = Some((lit, span));
    Ok(())
}

/// The order the line-head words come in, by file kind (DESIGN 1.1). `None`: the word is not
/// written in that kind of file.
fn rank(kind: Kind, w: &str) -> Option<u8> {
    match kind {
        Kind::Calendar => match w {
            kw::DESCRIPTION => Some(1),
            kw::OFFSET => Some(2),
            kw::USE => Some(3),
            kw::SOURCE => Some(4),
            kw::CLOSED | kw::OPEN => Some(5),
            _ => None,
        },
        Kind::Dates => match w {
            kw::DESCRIPTION => Some(1),
            kw::USE => Some(2),
            kw::SOURCE => Some(3),
            kw::INPUTS => Some(4),
            kw::DATE => Some(5),
            kw::CLAIMS => Some(6),
            kw::EXAMPLES => Some(7),
            _ => None,
        },
    }
}

fn order_text(kind: Kind) -> Text {
    match kind {
        Kind::Calendar => tr!(
            "calendar のファイルは、見出し、`description`、`offset`、`use calendar`、`source`、`closed` と `open` の順に書きます",
            "a calendar file goes: the heading, `description`, `offset`, `use calendar`, `source`, then `closed` and `open`"
        ),
        Kind::Dates => tr!(
            "dates のファイルは、見出し、`description`、`use calendar`、`source`、`inputs`、`date`、`claims`、`examples` の順に書きます",
            "a dates file goes: the heading, `description`, `use calendar`, `source`, `inputs`, `date`, `claims`, `examples`"
        ),
    }
}

/// The block indented lines belong to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
    None,
    /// The lines under a section that was refused: said once, at its head.
    Skip,
    Source(usize),
    Inputs,
    Date(usize),
    Claims,
    Examples,
}

pub fn parse(path: &str, src: &str) -> Parsed {
    let (lines, mut diags) = lex::lex(path, src);
    let bad_lines: std::collections::HashSet<usize> = diags.iter().filter_map(|d| d.line).collect();
    let mut p = Parser { path, src, diags: Vec::new(), file: None };
    p.run(&lines, &bad_lines);
    // A line the lexer could not read says so once; what the parser made of it is noise.
    p.diags.retain(|d| !d.line.is_some_and(|l| bad_lines.contains(&l)));
    diags.append(&mut p.diags);
    diags.sort_by_key(|d| (d.line, d.col));
    let file = if diags.iter().any(|d| d.is_error()) { None } else { p.file };
    Parsed { file, diags }
}

struct Parser<'a> {
    path: &'a str,
    src: &'a str,
    diags: Vec<Diag>,
    file: Option<File>,
}

impl<'a> Parser<'a> {
    fn err(&mut self, code: &'static str, line: usize, col: usize, msg: Text) -> &mut Diag {
        self.diags.push(Diag::error(code, self.path, line, col, msg).source(self.src));
        self.diags.last_mut().unwrap()
    }

    fn bad(&mut self, line: usize, b: Bad) {
        let mut d = Diag::error(b.code, self.path, line, b.col, b.msg).source(self.src);
        d.notes = b.notes;
        self.diags.push(d);
    }

    fn run(&mut self, lines: &[Line], bad_lines: &std::collections::HashSet<usize>) {
        let mut it = lines.iter().filter(|l| !l.tokens.is_empty());
        // The heading.
        let Some(first) = it.next() else {
            self.err("E003", 1, 1, tr!("ファイルが空です。一行目は `calendar …` か `dates …` です", "The file is empty; its first line is `calendar …` or `dates …`"));
            return;
        };
        let kind = match &first.tokens[0].tok {
            Tok::Word(w) if w == kw::CALENDAR && first.indent == 0 => Kind::Calendar,
            Tok::Word(w) if w == kw::DATES && first.indent == 0 => Kind::Dates,
            _ => {
                if !bad_lines.contains(&first.no) {
                    self.err("E003", first.no, first.tokens[0].col, tr!(
                        "ファイルは `calendar` か `dates` の行で始めます",
                        "A file starts with a `calendar` or a `dates` line"
                    ))
                    .notes
                    .push(tr!(
                        "カレンダー（休みの決まり）は `calendar 東京の営業日(tokyo) v1`、日付の関数は `dates 支払条件(payment_terms) v1` のように書き始めます。",
                        "A calendar (which days are closed) starts like `calendar tokyo v1`; date functions start like `dates payment_terms v1`."
                    ));
                }
                return;
            }
        };
        let mut c = Cur::new(first);
        c.bump();
        let header = (|| -> Result<(Name, String), Bad> {
            let n = name_with_alias(&mut c, &tr!("ファイル", "the file"))?;
            let vcol = c.col();
            let v = match c.peek() {
                Some(Tok::Word(v)) if v.len() > 1 && v.starts_with('v') && v[1..].chars().all(|ch| ch.is_ascii_digit()) => v[1..].to_string(),
                _ => {
                    return Err(bad(vcol, tr!("見出しの最後には版（`v1` のように）を書きます（{}）", "the heading ends with a version such as `v1` ({})", c.found().ja; c.found().en)));
                }
            };
            c.bump();
            c.done()?;
            Ok((n, v))
        })();
        let (name, version) = match header {
            Ok(x) => x,
            Err(b) => {
                self.bad(first.no, b);
                (Name { text: String::new(), alias: None, span: Span { line: first.no, col: 1 } }, "0".into())
            }
        };
        let mut f = File {
            kind,
            path: self.path.to_string(),
            src: self.src.to_string(),
            name,
            version,
            description: None,
            offset: None,
            use_calendar: None,
            sources: vec![],
            rules: vec![],
            inputs: vec![],
            dates: vec![],
            claims: vec![],
            examples: None,
        };
        let mut block = Block::None;
        let mut block_indent: Option<usize> = None;
        let mut last_rank = 0u8;
        let mut seen: Vec<String> = Vec::new();
        let mut header_row = true;
        for line in it {
            if bad_lines.contains(&line.no) {
                continue;
            }
            let t0 = &line.tokens[0];
            let row = t0.tok == Tok::Pipe;
            if block == Block::Skip && (line.indent > 0 || row) {
                continue;
            }
            if line.indent > 0 && !(row && block == Block::Examples) {
                // A line of the block above.
                if block == Block::None || block == Block::Examples {
                    self.err("E005", line.no, line.indent + 1, tr!(
                        "この行は字下げされていますが、上に字下げで続く行がありません",
                        "This line is indented, but nothing above it takes indented lines"
                    ))
                    .notes
                    .push(tr!(
                        "字下げは、`inputs` と `claims` の中身、`date` の操作、`source` の下の行を表します。",
                        "Indentation marks what is inside `inputs` and `claims`, the operations of a `date`, and the lines under a `source`."
                    ));
                    continue;
                }
                match block_indent {
                    None => block_indent = Some(line.indent),
                    Some(i) if i != line.indent => {
                        self.err("E005", line.no, line.indent + 1, tr!(
                            "字下げが上の行とそろっていません（上は {i} 文字、この行は {} 文字）",
                            "The indentation does not line up with the line above ({i} spaces there, {} here)",
                            line.indent
                        ));
                        continue;
                    }
                    _ => {}
                }
                if let Err(b) = self.indented(&mut f, block, line) {
                    self.bad(line.no, b);
                }
                continue;
            }
            if row {
                if block != Block::Examples {
                    self.err("E002", line.no, t0.col, tr!(
                        "表の行（`|`）は `examples` の下にだけ書けます",
                        "A table row (`|`) is written only under `examples`"
                    ));
                    continue;
                }
                if let Err(b) = self.example_row(&mut f, line, header_row) {
                    self.bad(line.no, b);
                }
                header_row = false;
                continue;
            }
            // A line at the left margin starts a section.
            block_indent = None;
            let Tok::Word(w) = &t0.tok else {
                self.err("E002", line.no, t0.col, tr!("行の初めに `{}` は書けません", "A line cannot start with `{}`", show(&t0.tok)));
                block = Block::None;
                continue;
            };
            let w = w.as_str();
            let Some(r) = rank(kind, w) else {
                let other = rank(if kind == Kind::Calendar { Kind::Dates } else { Kind::Calendar }, w).is_some();
                if other {
                    let msg = match kind {
                        Kind::Calendar => tr!("calendar のファイルには `{w}` を書けません。日付の関数は dates のファイルに書きます", "A calendar file has no `{w}`; date functions go in a dates file"),
                        Kind::Dates if w == kw::OFFSET => tr!("dates のファイルには `offset` を書けません。オフセットはカレンダーに書きます", "A dates file has no `offset`; the offset belongs to the calendar"),
                        Kind::Dates => tr!("dates のファイルには `{w}` を書けません。休みの決まりはカレンダーのファイルに書き、`use calendar` で読みます", "A dates file has no `{w}`; which days are closed is written in a calendar file and read with `use calendar`"),
                    };
                    self.err("E004", line.no, t0.col, msg);
                } else if [kw::CALENDAR, kw::DATES].contains(&w) {
                    self.err("E004", line.no, t0.col, tr!("見出しは一つのファイルに一つです", "A file has one heading"));
                } else {
                    self.err("E002", line.no, t0.col, tr!("`{w}` は行の初めに書く語ではありません", "`{w}` is not a word a line starts with"))
                        .notes
                        .push(order_text(kind));
                }
                block = Block::Skip;
                continue;
            };
            let once = [kw::DESCRIPTION, kw::OFFSET, kw::USE, kw::INPUTS, kw::CLAIMS, kw::EXAMPLES];
            if once.contains(&w) && seen.iter().any(|s| s == w) {
                self.err("E004", line.no, t0.col, tr!("`{w}` が二度書かれています", "`{w}` is written twice"));
                block = Block::Skip;
                continue;
            }
            if r < last_rank {
                self.err("E004", line.no, t0.col, tr!("`{w}` の位置が違います", "`{w}` is out of place")).notes.push(order_text(kind));
                block = Block::Skip;
                continue;
            }
            last_rank = r;
            seen.push(w.to_string());
            block = Block::None;
            let res = self.top(&mut f, w, line, &mut block);
            if let Err(b) = res {
                self.bad(line.no, b);
                block = Block::Skip;
            }
            if block == Block::Examples {
                header_row = true;
            }
        }
        // The lines a block needs.
        for s in &f.sources {
            if let SourceKind::File { format, covers, .. } = &s.kind {
                if format.is_none() {
                    self.err("E004", s.span.line, s.span.col, tr!(
                        "出典「{}」に `format` の行がありません",
                        "The source {} has no `format` line",
                        s.name
                    ))
                    .notes
                    .push(tr!(
                        "下の行に `  format csv shift_jis` か `  format govuk \"england-and-wales\"` のように書きます。",
                        "Write it under the source, like `  format csv shift_jis` or `  format govuk \"england-and-wales\"`."
                    ));
                }
                if covers.is_none() {
                    self.err("E004", s.span.line, s.span.col, tr!(
                        "出典「{}」に `covers` の行がありません",
                        "The source {} has no `covers` line",
                        s.name
                    ))
                    .notes
                    .push(tr!(
                        "表が休みを全部載せている範囲を、`  covers 1955-01-01..2027-12-31` か `  covers listed years` と書きます。範囲は推しません（DESIGN 1.5）。",
                        "Write the span the table lists every closed day of: `  covers 1955-01-01..2027-12-31` or `  covers listed years`. It is never guessed (DESIGN 1.5)."
                    ));
                }
            }
        }
        if kind == Kind::Dates && f.inputs.is_empty() && bad_lines.is_empty() && !self.diags.iter().any(|d| d.is_error()) {
            let line = f.name.span.line;
            self.err("E004", line, 1, tr!("`inputs` の節がありません", "There is no `inputs` section")).notes.push(tr!(
                "日付の関数は、日付を一つ（と整数をいくつか）受け取ります。`inputs` の下に `受領日(received) : date  range >=2026-01-01 <=2026-12-31` のように書きます。",
                "Date functions take one date (and some integers). Write it under `inputs`, like `received : date  range >=2026-01-01 <=2026-12-31`."
            ));
        }
        if let Some(ex) = &f.examples
            && ex.columns.is_empty()
        {
            let s = ex.span;
            self.err("E002", s.line, s.col, tr!("`examples` の下に表がありません", "`examples` has no table under it"));
        }
        self.file = Some(f);
    }

    /// A line at the left margin.
    fn top(&mut self, f: &mut File, w: &str, line: &Line, block: &mut Block) -> Result<(), Bad> {
        let mut c = Cur::new(line);
        let span = c.span();
        c.bump();
        match w {
            kw::DESCRIPTION => {
                f.description = Some(c.string(&tr!("説明", "the description"))?);
                c.done()
            }
            kw::OFFSET => {
                let (text, col) = line.rest.clone().unwrap_or_default();
                f.offset = Some((text, Span { line: line.no, col }));
                Ok(())
            }
            kw::USE => {
                c.want_word(kw::CALENDAR)?;
                let s = c.span();
                let p = c.string(&tr!("カレンダーのファイル", "the calendar file"))?;
                c.done()?;
                f.use_calendar = Some((p, s));
                Ok(())
            }
            kw::SOURCE => {
                let (name, _) = c.name(&tr!("出典", "the source"))?;
                c.want(&Tok::Eq, "=")?;
                let kind = if c.eat_word(kw::FILE) {
                    let path = c.string(&tr!("写しのファイル", "the copy's file"))?;
                    let mut url = None;
                    let mut pin = None;
                    while !c.at_end() {
                        if c.eat_word(kw::URL) {
                            url = Some(c.string(&tr!("URL", "the URL"))?);
                        } else if let Some(Tok::Sha(h)) = c.peek() {
                            pin = Some(h.clone());
                            c.bump();
                        } else {
                            c.done()?;
                        }
                    }
                    if f.kind == Kind::Dates {
                        return Err(Bad {
                            col: span.col,
                            code: "E004",
                            msg: tr!("祝日の表はカレンダーのファイルに書きます", "A table of holidays belongs in a calendar file"),
                            notes: vec![tr!(
                                "dates のファイルに書ける出典は法令（`= law`）だけです。表は `use calendar` で読むカレンダーに書きます。",
                                "The only source a dates file declares is a law (`= law`); tables go in the calendar it reads with `use calendar`."
                            )],
                        });
                    }
                    SourceKind::File { path, url, pin, format: None, covers: None }
                } else if c.eat_word(kw::LAW) {
                    let id = c.string(&tr!("法令 ID", "the law id"))?;
                    c.want_word(kw::ASOF)?;
                    let (asof, _) = c.date()?;
                    c.done()?;
                    SourceKind::Law { id, asof, pins: vec![] }
                } else {
                    return Err(bad(c.col(), tr!("`=` のあとには `file` か `law` を書きます（{}）", "`=` is followed by `file` or `law` ({})", c.found().ja; c.found().en)));
                };
                f.sources.push(SourceDecl { name, span: Span { line: line.no, col: line.tokens[1].col }, kind });
                *block = Block::Source(f.sources.len() - 1);
                Ok(())
            }
            kw::CLOSED | kw::OPEN => {
                let open = w == kw::OPEN;
                let what = if !open && c.eat_word(kw::WEEKLY) {
                    let mut days = Vec::new();
                    loop {
                        let col = c.col();
                        let d = match c.peek() {
                            Some(Tok::Word(x)) => kw::weekday(x),
                            _ => None,
                        };
                        let Some(d) = d else {
                            return Err(bad(col, tr!("曜日は `mon` `tue` `wed` `thu` `fri` `sat` `sun` で書きます（{}）", "a day of the week is `mon`, `tue`, `wed`, `thu`, `fri`, `sat` or `sun` ({})", c.found().ja; c.found().en)));
                        };
                        c.bump();
                        if days.contains(&d) {
                            return Err(bad(col, tr!("同じ曜日が二度あります", "the same day of the week is written twice")));
                        }
                        days.push(d);
                        if !c.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    RuleKind::Weekly(days)
                } else if !open && c.eat_word(kw::EVERY) {
                    let md = |c: &mut Cur| -> Result<(u32, u32), Bad> {
                        match c.peek() {
                            Some(Tok::MonthDay(m, d)) => {
                                let r = (*m, *d);
                                c.bump();
                                Ok(r)
                            }
                            _ => Err(bad(c.col(), tr!("毎年の休みは月日（`12-29` の形）で書きます（{}）", "a closure of every year is a month and day, like `12-29` ({})", c.found().ja; c.found().en))),
                        }
                    };
                    let from = md(&mut c)?;
                    let to = if c.eat(&Tok::DotDot) { md(&mut c)? } else { from };
                    let name = if let Some(Tok::Str(s)) = c.peek() {
                        let s = s.clone();
                        c.bump();
                        Some(s)
                    } else {
                        None
                    };
                    RuleKind::Every { from, to, name }
                } else if let Some(Tok::Date(_)) = c.peek() {
                    let (from, _) = c.date()?;
                    let to = if c.eat(&Tok::DotDot) { c.date()?.0 } else { from };
                    if to < from {
                        return Err(bad(c.col(), tr!("区間の終わり {to} が始まり {from} より前です", "the end {to} comes before the start {from}")));
                    }
                    let name = if let Some(Tok::Str(s)) = c.peek() {
                        let s = s.clone();
                        c.bump();
                        Some(s)
                    } else {
                        None
                    };
                    if open { RuleKind::Open { from, to, name } } else { RuleKind::Days { from, to, name } }
                } else if !open && matches!(c.peek(), Some(Tok::Word(_))) {
                    let (n, s) = c.name(&tr!("表", "the table"))?;
                    RuleKind::Table(n, s)
                } else if open {
                    return Err(bad(c.col(), tr!("`open` のあとには日付（`2026-12-28` の形）を書きます（{}）", "`open` is followed by a date, like `2026-12-28` ({})", c.found().ja; c.found().en)));
                } else {
                    return Err(bad(c.col(), tr!(
                        "`closed` のあとには `weekly`、`every`、日付、表の名前のどれかを書きます（{}）",
                        "`closed` is followed by `weekly`, `every`, a date, or the name of a table ({})",
                        c.found().ja;
                        c.found().en
                    )));
                };
                let ci = cite(&mut c)?;
                c.done()?;
                f.rules.push(CalRule { what, span, cite: ci });
                Ok(())
            }
            kw::INPUTS => {
                c.done()?;
                *block = Block::Inputs;
                Ok(())
            }
            kw::DATE => {
                let name = name_with_alias(&mut c, &tr!("日付", "the date"))?;
                c.want(&Tok::Eq, "=")?;
                let start = c.name(&tr!("始まりの日付", "the date it starts from"))?;
                let ci = cite(&mut c)?;
                c.done()?;
                f.dates.push(DateDecl { name, span: Span { line: line.no, col: line.tokens[1].col }, start, cite: ci, ops: vec![], at: None });
                *block = Block::Date(f.dates.len() - 1);
                Ok(())
            }
            kw::CLAIMS => {
                c.done()?;
                *block = Block::Claims;
                Ok(())
            }
            kw::EXAMPLES => {
                c.done()?;
                f.examples = Some(Examples { span, columns: vec![], rows: vec![] });
                *block = Block::Examples;
                Ok(())
            }
            _ => Err(bad(span.col, tr!("`{w}` は行の初めに書く語ではありません", "`{w}` is not a word a line starts with"))),
        }
    }

    /// An indented line of the block above.
    fn indented(&mut self, f: &mut File, block: Block, line: &Line) -> Result<(), Bad> {
        let mut c = Cur::new(line);
        let span = c.span();
        match block {
            Block::Source(i) => {
                let s = &mut f.sources[i];
                match &mut s.kind {
                    SourceKind::File { format, covers, .. } => {
                        if c.eat_word(kw::FORMAT) {
                            if format.is_some() {
                                return Err(Bad { col: span.col, code: "E004", msg: tr!("`format` が二度書かれています", "`format` is written twice"), notes: vec![] });
                            }
                            let fm = if c.eat_word(kw::CSV) {
                                let sj = if c.eat_word(kw::SHIFT_JIS) {
                                    true
                                } else {
                                    c.eat_word(kw::UTF8);
                                    false
                                };
                                Format::Csv { shift_jis: sj }
                            } else if c.eat_word(kw::GOVUK) {
                                Format::GovUk { division: c.string(&tr!("地域", "the division"))? }
                            } else {
                                return Err(bad(c.col(), tr!("形式は `csv` か `govuk` です（{}）", "the format is `csv` or `govuk` ({})", c.found().ja; c.found().en)));
                            };
                            c.done()?;
                            *format = Some((fm, span));
                            Ok(())
                        } else if c.eat_word(kw::COVERS) {
                            if covers.is_some() {
                                return Err(Bad { col: span.col, code: "E004", msg: tr!("`covers` が二度書かれています", "`covers` is written twice"), notes: vec![] });
                            }
                            let cv = if c.eat_word(kw::LISTED) {
                                c.want_word(kw::YEARS)?;
                                Covers::ListedYears
                            } else {
                                let (a, _) = c.date()?;
                                c.want(&Tok::DotDot, "..")?;
                                let (b, _) = c.date()?;
                                if b < a {
                                    return Err(bad(span.col, tr!("範囲の終わり {b} が始まり {a} より前です", "the end {b} comes before the start {a}")));
                                }
                                Covers::Range(a, b)
                            };
                            c.done()?;
                            *covers = Some((cv, span));
                            Ok(())
                        } else {
                            Err(bad(span.col, tr!("表の出典の下には `format` と `covers` の行を書きます（{}）", "the lines under a table are `format` and `covers` ({})", c.found().ja; c.found().en)))
                        }
                    }
                    SourceKind::Law { pins, .. } => {
                        let (frag, _) = c.name(&tr!("固定する条", "the article to pin"))?;
                        let pin = match c.peek() {
                            Some(Tok::Sha(h)) => {
                                let h = h.clone();
                                c.bump();
                                Some(h)
                            }
                            _ => None,
                        };
                        c.done()?;
                        if pins.iter().any(|p| p.fragment == frag) {
                            return Err(Bad { col: span.col, code: "E004", msg: tr!("{frag} の固定が二度書かれています", "{frag} is pinned twice"), notes: vec![] });
                        }
                        pins.push(LawPin { fragment: frag, pin, span });
                        Ok(())
                    }
                }
            }
            Block::Inputs => {
                let name = name_with_alias(&mut c, &tr!("入力", "the input"))?;
                c.want(&Tok::Colon, ":")?;
                let ty = if c.eat_word(kw::DATE) {
                    Ty::Date
                } else if c.eat_word(kw::INT) {
                    Ty::Int
                } else {
                    return Err(bad(c.col(), tr!("入力の型は `date` か `int` です（{}）", "an input's type is `date` or `int` ({})", c.found().ja; c.found().en)));
                };
                let range_span = c.span();
                let (mut lo, mut hi) = (None, None);
                if c.eat_word(kw::RANGE) {
                    while !c.at_end() {
                        bound(&mut c, &mut lo, &mut hi)?;
                    }
                }
                c.done()?;
                f.inputs.push(Input { name, ty, span, lo, hi, range_span });
                Ok(())
            }
            Block::Date(i) => {
                let d = &mut f.dates[i];
                if d.at.is_some() {
                    return Err(bad(span.col, tr!("`at` の行は日付の最後の行です。そのあとに操作は書けません", "`at` is the last line of a date; no operation comes after it")));
                }
                if c.is_word(kw::AT) {
                    c.bump();
                    let at = match c.peek() {
                        Some(Tok::Time(h, m)) => {
                            let a = At::Time(h * 60 + m);
                            c.bump();
                            a
                        }
                        Some(Tok::Word(w)) if w == kw::END => {
                            c.bump();
                            c.want_word(kw::OF)?;
                            c.want_word(kw::DAY)?;
                            At::EndOfDay
                        }
                        _ => return Err(bad(c.col(), tr!("`at` のあとには時刻（`09:00`）か `end of day` を書きます（{}）", "`at` is followed by a time (`09:00`) or `end of day` ({})", c.found().ja; c.found().en))),
                    };
                    let ci = cite(&mut c)?;
                    c.done()?;
                    d.at = Some((at, span, ci));
                    return Ok(());
                }
                let o = op(&mut c, false)?;
                // An `else` the operation did not take: it never lands on a missing day (W201).
                let stray_else = missing(&mut c)?;
                let text = line.slice(span.col, cite_col(line));
                let ci = cite(&mut c)?;
                c.done()?;
                d.ops.push(OpLine { op: o, span, text, cite: ci, stray_else });
                Ok(())
            }
            Block::Claims => {
                let (name, _) = c.name(&tr!("条件", "the claim"))?;
                c.want(&Tok::Colon, ":")?;
                let body_col = c.col();
                let kind = claim_body(&mut c)?;
                let text = line.slice(body_col, cite_col(line));
                let ci = cite(&mut c)?;
                c.done()?;
                f.claims.push(Claim { name, span, kind, text, cite: ci });
                Ok(())
            }
            Block::Examples | Block::None | Block::Skip => Ok(()),
        }
    }

    /// A row of the `examples` table: the first is the heading.
    fn example_row(&mut self, f: &mut File, line: &Line, heading: bool) -> Result<(), Bad> {
        let ex = f.examples.as_mut().unwrap();
        // Cut the row at its pipes.
        let mut cells: Vec<Vec<&Token>> = Vec::new();
        let mut cur: Option<Vec<&Token>> = None;
        for t in &line.tokens {
            if t.tok == Tok::Pipe {
                if let Some(c) = cur.take() {
                    cells.push(c);
                }
                cur = Some(Vec::new());
            } else if let Some(c) = cur.as_mut() {
                c.push(t);
            }
        }
        if let Some(c) = cur
            && !c.is_empty()
        {
            return Err(bad(c[0].col, tr!("表の行は `|` で終えます", "a table row ends with `|`")));
        }
        let span = Span { line: line.no, col: line.tokens[0].col };
        if heading {
            for cell in &cells {
                let (output, rest): (bool, &[&Token]) = match cell.first().map(|t| &t.tok) {
                    Some(Tok::Arrow) => (true, &cell[1..]),
                    _ => (false, &cell[..]),
                };
                match rest {
                    [t] if matches!(t.tok, Tok::Word(_)) => {
                        let Tok::Word(w) = &t.tok else { unreachable!() };
                        ex.columns.push(Column { name: w.clone(), output, span: Span { line: line.no, col: t.col } });
                    }
                    _ => {
                        let col = cell.first().map(|t| t.col).unwrap_or(span.col);
                        return Err(bad(col, tr!(
                            "表の見出しには、入力の名前か、`-> <日付の名前>` を書きます",
                            "a heading of the table is the name of an input, or `-> <date>`"
                        )));
                    }
                }
            }
            return Ok(());
        }
        if cells.len() != ex.columns.len() {
            return Err(bad(span.col, tr!(
                "この行の値は {} 個ですが、見出しは {} 列です",
                "this row has {} values, and the heading has {} columns",
                cells.len(),
                ex.columns.len()
            )));
        }
        let mut row = Row { span, cells: vec![] };
        for cell in &cells {
            let col = cell.first().map(|t| t.col).unwrap_or(span.col);
            let lit = match cell.as_slice() {
                [t] => match &t.tok {
                    Tok::Date(d) => Lit::Date(*d),
                    Tok::Int(n) => Lit::Int(*n),
                    _ => return Err(bad(col, tr!("表の値は日付か数です", "a value in the table is a date or a number"))),
                },
                [m, t] if m.tok == Tok::Minus => match &t.tok {
                    Tok::Int(n) => Lit::Int(-n),
                    _ => return Err(bad(col, tr!("表の値は日付か数です", "a value in the table is a date or a number"))),
                },
                [] => return Err(bad(col, tr!("表の値が空です", "a value in the table is empty"))),
                _ => return Err(bad(col, tr!("表の値は一つずつ書きます", "a cell of the table holds one value"))),
            };
            row.cells.push((lit, Span { line: line.no, col }));
        }
        ex.rows.push(row);
        Ok(())
    }
}
