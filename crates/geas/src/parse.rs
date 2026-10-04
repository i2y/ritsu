//! The claims file into the model, with the checks that need no run (E001-E011).
//!
//! The first E001 stops the parse: what follows a token the grammar does not allow
//! cannot be read reliably. Every other problem is collected, the parser skipping
//! past an unknown word to the end of its line, and once the file parses every static
//! check runs, so one `geas check` reports them all, in line order.

use ritsu_base::text::Text;
use crate::diag::{self, Diag};
use crate::lex::{self, Kind, Tok};
use crate::model::*;
use crate::pins;
use crate::regex::Pattern;
use crate::screen::{self, Label};
use crate::words;
use std::collections::HashMap;

/// A `run` or `serve` line: the command's words, or None when E010 refused it.
struct RawCmd {
    words: Option<Vec<String>>,
    /// Where `run` or `serve` is written.
    pos: Pos,
    /// Where `{port}` first appears inside the string: E010 on a target without a
    /// port points there.
    port_at: Option<Pos>,
}

/// A target as written, each line with its place, before E004 decides what it is.
struct RawTarget {
    name: String,
    name_pos: Pos,
    kw_pos: Pos,
    runs: Vec<RawCmd>,
    serves: Vec<RawCmd>,
    pixies: Vec<RawCmd>,
    drivers: Vec<RawCmd>,
    ports: Vec<(Port, Pos)>,
    serials: Vec<Pos>,
    /// A line E002 refused: what it was meant to say is unknown, so a line that
    /// seems missing is not reported as well.
    unknown_line: bool,
    pins: Vec<RawPin>,
}

enum RawStep {
    /// `call` is None when its name is unknown (E002 said so).
    When {
        target: String,
        target_pos: Pos,
        call: Option<Call>,
        call_word: String,
        call_pos: Pos,
        pos: Pos,
        /// Where a GUI call's arguments are written, by name (`path`, `name`,
        /// `text`, `key`, `ms`, `nth`, `field`, `into`), for E013 to point at.
        args_at: Vec<(&'static str, Pos)>,
    },
    /// `check` is None when its subject or its matcher is unknown.
    Then { check: Option<Check> },
}

struct RawClaim {
    name: String,
    name_pos: Pos,
    pos: Pos,
    steps: Vec<RawStep>,
}

struct RawSpec {
    targets: Vec<RawTarget>,
    claims: Vec<RawClaim>,
    masks: Vec<Mask>,
    /// The pins outside any target.
    pins: Vec<RawPin>,
}

/// The words a pin starts with (DESIGN §12).
const PIN_WORDS: &[&str] = &["env", "tz", "locale", "clock", "seed"];

/// The calls that act on a GUI and observe its screen (DESIGN §8.1).
const GUI_CALLS: &[&str] = &["open", "click", "input", "submit", "press", "advance"];

/// The words that start what can follow a check: the next step, or what the top
/// level holds. A node pattern ends before them.
const NEXT_WORDS: &[&str] = &["when", "then", "and", "target", "claim", "mask", "env", "tz", "locale", "clock", "seed"];

/// An E001 has been pushed and the parse stops.
struct Stop;

/// Where a GUI call's arguments are written, by name.
type ArgsAt = Vec<(&'static str, Pos)>;

/// One argument of a call: `"5"`, or `body: "5"`.
struct Arg {
    name: Option<Tok>,
    value: Tok,
}

fn at(t: &Tok) -> Pos {
    Pos { line: t.line, col: t.col }
}

/// The Japanese for "<what> を": a space after code or a Latin word, none after kana
/// or kanji, as Japanese text sets them.
fn ja_wo(what: &str) -> String {
    if what.ends_with(|c: char| c == '`' || c.is_ascii_alphanumeric()) {
        format!("{what} を")
    } else {
        format!("{what}を")
    }
}

struct P {
    toks: Vec<Tok>,
    pos: usize,
    diags: Vec<Diag>,
}

impl P {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos]
    }

    fn next(&mut self) -> Tok {
        let t = self.toks[self.pos].clone();
        if self.toks[self.pos].kind != Kind::Eof {
            self.pos += 1;
        }
        t
    }

    /// E001 at `found`: the grammar wants `what` there.
    fn expected(&mut self, found: &Tok, en: &str, ja: &str) -> Stop {
        let msg = if found.kind == Kind::Eof {
            Text::new(
                format!("{}書く前にファイルが終わっています", ja_wo(ja)),
                format!("expected {en}, found the end of the file"),
            )
        } else {
            let (fe, fj) = found.shown();
            Text::new(
                format!("{}書くところに {fj} があります", ja_wo(ja)),
                format!("expected {en}, found {fe}"),
            )
        };
        self.diags.push(diag::error("E001", found.line, found.col, msg));
        Stop
    }

    fn expect(&mut self, kind: Kind, en: &str, ja: &str) -> Result<Tok, Stop> {
        let t = self.next();
        if t.kind != kind {
            return Err(self.expected(&t, en, ja));
        }
        if kind == Kind::Str {
            self.plain(&t)?;
        }
        Ok(t)
    }

    fn e001(&mut self, tok: &Tok, en: String, ja: String) -> Stop {
        self.diags.push(diag::error("E001", tok.line, tok.col, Text::new(ja, en)));
        Stop
    }

    fn e002(&mut self, tok: &Tok, en: String, ja: String) {
        self.diags.push(diag::error("E002", tok.line, tok.col, Text::new(ja, en)));
    }

    /// Skips the rest of `line`, up to a `}` that may close the block on that line.
    fn skip_line(&mut self, line: usize) {
        while self.peek().line == line && !matches!(self.peek().kind, Kind::RBrace | Kind::Eof) {
            self.next();
        }
    }

    fn file(&mut self) -> Result<RawSpec, Stop> {
        let mut spec = RawSpec { targets: vec![], claims: vec![], masks: vec![], pins: vec![] };
        loop {
            let tk = self.peek().clone();
            match tk.kind {
                Kind::Eof => break,
                Kind::Ident if tk.text == "target" => {
                    let tg = self.target()?;
                    spec.targets.push(tg);
                }
                Kind::Ident if tk.text == "claim" => {
                    let c = self.claim()?;
                    spec.claims.push(c);
                }
                Kind::Ident if tk.text == "mask" => {
                    if let Some(m) = self.mask()? {
                        spec.masks.push(m);
                    }
                }
                Kind::Ident if PIN_WORDS.contains(&tk.text.as_str()) => {
                    self.next();
                    let pin = self.pin(&tk)?;
                    spec.pins.push(pin);
                }
                _ => {
                    self.next();
                    return Err(self.expected(
                        &tk,
                        "`target`, `claim`, `mask` or a pin (`env`, `tz`, `locale`, `clock`, `seed`)",
                        "`target`、`claim`、`mask`、固定（`env`、`tz`、`locale`、`clock`、`seed`）のどれか",
                    ));
                }
            }
        }
        Ok(spec)
    }

    fn target(&mut self) -> Result<RawTarget, Stop> {
        let kw = self.next();
        let name = self.expect(Kind::Ident, "the target's name", "ターゲットの名前")?;
        self.expect(Kind::LBrace, "`{`", "`{`")?;
        let mut rt = RawTarget {
            name: name.text.clone(),
            name_pos: at(&name),
            kw_pos: at(&kw),
            runs: vec![],
            serves: vec![],
            pixies: vec![],
            drivers: vec![],
            ports: vec![],
            serials: vec![],
            unknown_line: false,
            pins: vec![],
        };
        loop {
            let tk = self.next();
            match tk.kind {
                Kind::RBrace => break,
                Kind::Ident => match tk.text.as_str() {
                    "run" => {
                        let s = self.expect(Kind::Str, "the command, in quotes", "`\"` で囲んだコマンド")?;
                        let c = self.command(&rt.name, &s, at(&tk));
                        rt.runs.push(c);
                    }
                    "serve" => {
                        let s = self.expect(Kind::Str, "the command, in quotes", "`\"` で囲んだコマンド")?;
                        let c = self.command(&rt.name, &s, at(&tk));
                        rt.serves.push(c);
                    }
                    "pixie" => {
                        let s = self.expect(Kind::Str, "the app's command, in quotes", "`\"` で囲んだアプリのコマンド")?;
                        let c = self.command(&rt.name, &s, at(&tk));
                        rt.pixies.push(c);
                    }
                    "driver" => {
                        let s = self.expect(Kind::Str, "the driver's command, in quotes", "`\"` で囲んだドライバーのコマンド")?;
                        let c = self.command(&rt.name, &s, at(&tk));
                        rt.drivers.push(c);
                    }
                    "port" if self.peek().kind == Kind::Ident && self.peek().text == "auto" => {
                        self.next();
                        rt.ports.push((Port::Auto, at(&tk)));
                    }
                    "serial" => rt.serials.push(at(&tk)),
                    "port" => {
                        let n = self.expect(Kind::Num, "a port number or `auto`", "ポート番号か `auto`")?;
                        match n.text.parse::<u16>() {
                            Ok(p) if p > 0 => rt.ports.push((Port::Fixed(p), at(&tk))),
                            _ => {
                                return Err(self.e001(
                                    &n,
                                    format!("`{}` is not a port; a port is a whole number from 1 to 65535", n.text),
                                    format!("`{}` はポート番号になりません。ポート番号は 1 から 65535 までの整数です", n.text),
                                ));
                            }
                        }
                    }
                    w if PIN_WORDS.contains(&w) => {
                        let pin = self.pin(&tk)?;
                        rt.pins.push(pin);
                    }
                    other => {
                        self.e002(
                            &tk,
                            format!("a target has no line `{other}`; its lines are run, serve, pixie, driver, port, serial, and the pins env, tz, locale, clock and seed"),
                            format!("ターゲットに `{other}` という行は書けません。書けるのは run、serve、pixie、driver、port、serial と、固定の env、tz、locale、clock、seed です"),
                        );
                        rt.unknown_line = true;
                        self.skip_line(tk.line);
                    }
                },
                _ => {
                    return Err(self.expected(
                        &tk,
                        "`run`, `serve`, `pixie`, `driver`, `port`, `serial`, a pin or `}`",
                        "`run`、`serve`、`pixie`、`driver`、`port`、`serial`、固定、`}` のどれか",
                    ));
                }
            }
        }
        Ok(rt)
    }

    /// A command string into words, once, as the spec is read (DESIGN §11). E010,
    /// pointing inside the string, when it does not split or holds no word.
    fn command(&mut self, target: &str, s: &Tok, pos: Pos) -> RawCmd {
        let inside = |i: usize| s.cols.get(i).copied().unwrap_or(s.col);
        let port_at = s.text.find(words::PORT).map(|b| Pos { line: s.line, col: inside(s.text[..b].chars().count()) });
        let words = match words::split(&s.text) {
            Ok(w) if w.is_empty() => {
                self.diags.push(diag::error(
                    "E010",
                    s.line,
                    s.col,
                    tr!(
                        "`{target}` のコマンドが空です。少なくとも、起動するプログラムを書いてください",
                        "the command of `{target}` is empty: it needs at least the program to start",
                    ),
                ));
                None
            }
            Ok(w) => Some(w),
            Err(u) => {
                let (i, en, ja) = match u {
                    words::Unsplit::OpenSingle(i) => {
                        (i, "the command opens a single quote that nothing closes", "コマンドのシングルクォートが閉じていません")
                    }
                    words::Unsplit::OpenDouble(i) => {
                        (i, "the command opens a double quote that nothing closes", "コマンドのダブルクォートが閉じていません")
                    }
                    words::Unsplit::TrailingBackslash(i) => (
                        i,
                        "the command ends in a backslash, with nothing after it to keep",
                        "コマンドがバックスラッシュで終わっていて、そのあとに残す文字がありません",
                    ),
                };
                let mut d = diag::error("E010", s.line, inside(i), Text::new(ja, en)).note(split_rule());
                if matches!(u, words::Unsplit::OpenDouble(_)) {
                    d = d.note(tr!(
                        "主張のファイルでは文字列の中のダブルクォートを `\\\"` と書くので、シングルクォートのほうが書きやすく、`run \"python3 'my calc.py'\"` のように書けます",
                        "in a claims file a double quote inside a string is written `\\\"`, so single quotes are the easy form: `run \"python3 'my calc.py'\"`",
                    ));
                }
                self.diags.push(d);
                None
            }
        };
        RawCmd { words, pos, port_at }
    }

    /// A pin, after its first word (DESIGN §12). The `env "NAME"` after `clock` or
    /// `seed` is read only on the same line, so that an `env` pin on the next line
    /// is not taken for it. E011 for a value geas cannot read.
    fn pin(&mut self, kw: &Tok) -> Result<RawPin, Stop> {
        let mut port_at = None;
        let kind = match kw.text.as_str() {
            "env" => {
                let next = self.next();
                match next.kind {
                    Kind::Ident if next.text == "clean" => PinKind::Clean,
                    Kind::Ident if next.text == "pass" => {
                        let n = self.expect(Kind::Str, "the variable's name, in quotes", "`\"` で囲んだ環境変数の名前")?;
                        self.var_name(&n);
                        PinKind::Pass(n.text)
                    }
                    Kind::Str => {
                        self.plain(&next)?;
                        self.var_name(&next);
                        let v = self.expect(Kind::Str, "the variable's value, in quotes", "`\"` で囲んだ環境変数の値")?;
                        port_at = v.text.find(words::PORT).map(|b| Pos {
                            line: v.line,
                            col: v.cols.get(v.text[..b].chars().count()).copied().unwrap_or(v.col),
                        });
                        PinKind::Set(next.text, v.text)
                    }
                    _ => {
                        return Err(self.expected(
                            &next,
                            "a variable's name in quotes, `clean` or `pass`",
                            "`\"` で囲んだ環境変数の名前、`clean`、`pass` のどれか",
                        ));
                    }
                }
            }
            "tz" => {
                let v = self.expect(Kind::Str, "a time zone, in quotes", "`\"` で囲んだタイムゾーン")?;
                if v.text.trim().is_empty() {
                    self.e011(&v, "a time zone has a name, such as `Asia/Tokyo` or `UTC`", "タイムゾーンには、`Asia/Tokyo` や `UTC` のような名前を書いてください".into());
                }
                PinKind::Tz(v.text)
            }
            "locale" => {
                let v = self.expect(Kind::Str, "a locale, in quotes", "`\"` で囲んだロケール")?;
                if !pins::locale(&v.text) {
                    let l = &v.text;
                    let (en, ja) = match pins::locale_meant(l) {
                        Some(m) => (
                            format!("`{l}` is not a locale written as a language and a region; write `{m}`"),
                            format!("`{l}` は、言語と地域の形で書いたロケールではありません。`{m}` と書いてください"),
                        ),
                        None => (
                            format!("`{l}` is not a locale written as a language and a region, such as `ja-JP` or `de-DE`"),
                            format!("`{l}` は、`ja-JP` や `de-DE` のように言語と地域の形で書いたロケールではありません"),
                        ),
                    };
                    self.e011(&v, &en, ja);
                }
                PinKind::Locale(v.text)
            }
            "clock" => {
                let v = self.expect(Kind::Str, "a time, in quotes", "`\"` で囲んだ時刻")?;
                if pins::rfc3339(&v.text).is_none() {
                    let s = &v.text;
                    self.e011(
                        &v,
                        &format!("`{s}` is not a time geas reads: write it as RFC 3339, such as `2026-08-29T09:00:00+09:00` or `2026-08-29T00:00:00Z`"),
                        format!("`{s}` は geas の読める時刻ではありません。RFC 3339 の形で、`2026-08-29T09:00:00+09:00` や `2026-08-29T00:00:00Z` のように書いてください"),
                    );
                }
                let env = self.pin_env(kw.line)?;
                PinKind::Clock(v.text, env)
            }
            "seed" => {
                let v = self.expect(Kind::Num, "the seed, a whole number", "乱数のシード（整数）")?;
                let n = match v.text.parse::<u64>() {
                    Ok(n) => n,
                    Err(_) => {
                        let s = &v.text;
                        self.e011(
                            &v,
                            &format!("`{s}` is not a seed; a seed is a whole number from 0"),
                            format!("`{s}` は乱数のシードになりません。シードは 0 以上の整数です"),
                        );
                        0
                    }
                };
                let env = self.pin_env(kw.line)?;
                PinKind::Seed(n, env)
            }
            _ => unreachable!("only a pin's first word comes here"),
        };
        Ok(RawPin { kind, pos: at(kw), port_at })
    }

    /// `env "NAME"` after `clock` or `seed`, on the same line.
    fn pin_env(&mut self, line: usize) -> Result<Option<String>, Stop> {
        if !(self.peek().kind == Kind::Ident && self.peek().text == "env" && self.peek().line == line) {
            return Ok(None);
        }
        self.next();
        let n = self.expect(Kind::Str, "the variable's name, in quotes", "`\"` で囲んだ環境変数の名前")?;
        self.var_name(&n);
        Ok(Some(n.text))
    }

    /// E011 for a variable's name a process cannot be given.
    fn var_name(&mut self, tok: &Tok) {
        let n = &tok.text;
        if n.is_empty() || n.contains('=') || n.contains('\0') {
            self.e011(
                tok,
                &format!("`{n}` is not a variable's name: it is empty or holds `=`"),
                format!("`{n}` は環境変数の名前になりません。空か、`=` を含んでいます"),
            );
        }
    }

    fn e011(&mut self, tok: &Tok, en: &str, ja: String) {
        self.diags.push(diag::error("E011", tok.line, tok.col, Text::new(ja, en)));
    }

    fn claim(&mut self) -> Result<RawClaim, Stop> {
        let kw = self.next();
        let name = self.expect(Kind::Str, "the claim's name, in quotes", "`\"` で囲んだ主張の名前")?;
        self.expect(Kind::LBrace, "`{`", "`{`")?;
        let mut steps = Vec::new();
        let mut seen_when = false;
        loop {
            let tk = self.next();
            match tk.kind {
                Kind::RBrace => break,
                Kind::Ident if tk.text == "when" => {
                    let s = self.when(&tk)?;
                    steps.push(s);
                    seen_when = true;
                }
                Kind::Ident if tk.text == "then" || tk.text == "and" => {
                    if !seen_when {
                        let w = &tk.text;
                        self.diags.push(diag::error(
                            "E005",
                            tk.line,
                            tk.col,
                            tr!(
                                "`when` より前に `{w}` があります。チェックは直前の `when` の結果を見ます",
                                "`{w}` comes before any `when`; a check reads what the `when` before it observed",
                            ),
                        ));
                    }
                    let check = self.check()?;
                    steps.push(RawStep::Then { check });
                }
                _ => {
                    return Err(self.expected(
                        &tk,
                        "`when`, `then`, `and` or `}`",
                        "`when`、`then`、`and`、`}` のどれか",
                    ));
                }
            }
        }
        if steps.is_empty() {
            self.diags.push(diag::error(
                "E005",
                kw.line,
                kw.col,
                tr!(
                    "主張 \"{}\" にステップがありません。主張は `when` から始めてください",
                    "the claim \"{}\" has no steps; a claim starts with a `when`",
                    name.text,
                ),
            ));
        }
        Ok(RawClaim { name: name.text.clone(), name_pos: at(&name), pos: at(&kw), steps })
    }

    fn when(&mut self, kw: &Tok) -> Result<RawStep, Stop> {
        let target = self.expect(Kind::Ident, "a target's name", "ターゲットの名前")?;
        self.expect(Kind::Dot, "`.`", "`.`")?;
        let method = self.expect(
            Kind::Ident,
            "a call (run, get, post, open, click, input, submit, press or advance)",
            "呼び出し（run、get、post、open、click、input、submit、press、advance）",
        )?;
        self.expect(Kind::LParen, "`(`", "`(`")?;
        let (args, close) = self.args()?;
        let (call, args_at) = if GUI_CALLS.contains(&method.text.as_str()) {
            self.gui_call(&method, args, &close)?
        } else {
            (self.call(&method, args, &close)?, vec![])
        };
        Ok(RawStep::When {
            target: target.text.clone(),
            target_pos: at(&target),
            call,
            call_word: method.text.clone(),
            call_pos: at(&method),
            pos: at(kw),
            args_at,
        })
    }

    /// The arguments after `(`, up to and with the `)`, whatever the call.
    fn args(&mut self) -> Result<(Vec<Arg>, Tok), Stop> {
        let mut args = Vec::new();
        if self.peek().kind == Kind::RParen {
            let close = self.next();
            return Ok((args, close));
        }
        loop {
            let tk = self.next();
            let arg = match tk.kind {
                Kind::Str | Kind::Num => {
                    self.plain(&tk)?;
                    Arg { name: None, value: tk }
                }
                Kind::Ident => {
                    self.expect(Kind::Colon, "`:`", "`:`")?;
                    let v = self.next();
                    if !matches!(v.kind, Kind::Str | Kind::Num) {
                        return Err(self.expected(&v, "a string or a number", "文字列か数"));
                    }
                    self.plain(&v)?;
                    Arg { name: Some(tk), value: v }
                }
                _ => return Err(self.expected(&tk, "a string", "文字列")),
            };
            args.push(arg);
            let sep = self.next();
            match sep.kind {
                Kind::Comma => continue,
                Kind::RParen => return Ok((args, sep)),
                _ => return Err(self.expected(&sep, "`,` or `)`", "`,` か `)`")),
            }
        }
    }

    /// The call a `when` makes, its arguments held to its grammar. None when the call
    /// itself is unknown (E002).
    fn call(&mut self, method: &Tok, args: Vec<Arg>, close: &Tok) -> Result<Option<Call>, Stop> {
        let word = method.text.as_str();
        let mut positional: Vec<Tok> = Vec::new();
        let mut body: Option<Tok> = None;
        for a in args {
            let Some(n) = a.name else {
                positional.push(a.value);
                continue;
            };
            match word {
                "post" if n.text == "body" && body.is_none() => body = Some(a.value),
                "post" if n.text == "body" => return Err(self.expected(&n, "`)`", "`)`")),
                "run" => self.e002(
                    &n,
                    format!("`run` takes no argument named `{}`; its arguments are strings", n.text),
                    format!("`run` に `{}` という名前の引数はありません。引数は文字列だけです", n.text),
                ),
                "get" => self.e002(
                    &n,
                    format!("`get` takes no argument named `{}`; it takes the path", n.text),
                    format!("`get` に `{}` という名前の引数はありません。書けるのはパスだけです", n.text),
                ),
                "post" => self.e002(
                    &n,
                    format!("`post` takes no argument named `{}`; it takes the path and `body:`", n.text),
                    format!("`post` に `{}` という名前の引数はありません。書けるのはパスと `body:` です", n.text),
                ),
                _ => {}
            }
        }
        match word {
            "run" => {
                let mut words = Vec::new();
                for v in positional {
                    if v.kind != Kind::Str {
                        return Err(self.expected(&v, "a string", "文字列"));
                    }
                    words.push(v.text);
                }
                Ok(Some(Call::Run(words)))
            }
            "get" | "post" => {
                let mut it = positional.into_iter();
                let path = match it.next() {
                    Some(p) if p.kind == Kind::Str => p.text,
                    Some(p) => return Err(self.expected(&p, "the path, in quotes", "`\"` で囲んだパス")),
                    None => return Err(self.expected(close, "the path, in quotes", "`\"` で囲んだパス")),
                };
                if let Some(extra) = it.next() {
                    return Err(self.expected(&extra, "`)`", "`)`"));
                }
                if word == "get" {
                    return Ok(Some(Call::Get(path)));
                }
                let body = match body {
                    Some(b) if b.kind == Kind::Str => Some(b.text),
                    Some(b) => return Err(self.expected(&b, "the body, in quotes", "`\"` で囲んだボディ")),
                    None => None,
                };
                Ok(Some(Call::Post { path, body }))
            }
            other => {
                self.e002(
                    method,
                    format!("`{other}` is not a call; the calls are run, get, post, open, click, input, submit, press and advance"),
                    format!("`{other}` という呼び出しはありません。呼び出しは run、get、post、open、click、input、submit、press、advance です"),
                );
                Ok(None)
            }
        }
    }

    /// A count written after `nth:` or `field:`: a whole number. `0` is read, and
    /// E013 says it counts from 1 once the target is known.
    fn count(&mut self, tok: &Tok) -> Result<usize, Stop> {
        match (&tok.kind, tok.text.parse::<usize>()) {
            (Kind::Num, Ok(n)) => Ok(n),
            _ => {
                let (fe, fj) = tok.shown();
                Err(self.e001(
                    tok,
                    format!("{fe} is not a whole number, which counts and places are written as"),
                    format!("{fj} は整数ではありません。個数や何番目かは、整数で書いてください"),
                ))
            }
        }
    }

    /// A GUI call (DESIGN §6, §8.1) and where its arguments are written:
    /// `open([path])`, `click(name[, nth: n])`, `input(text[, place])`,
    /// `submit([place])`, `press(key)`, `advance(ms)`, a place being `field: n` or
    /// `into: "<name>"[, nth: n]`.
    fn gui_call(&mut self, method: &Tok, args: Vec<Arg>, close: &Tok) -> Result<(Option<Call>, ArgsAt), Stop> {
        let word = method.text.as_str();
        let (named_ok, takes_en, takes_ja): (&[&str], &str, &str) = match word {
            "open" => (&[], "the path, or nothing", "パスだけで、何も書かなくてもかまいません"),
            "click" => (&["nth"], "the name and `nth:`", "名前と `nth:` です"),
            "input" => (&["field", "into", "nth"], "the text, and `field:`, or `into:` with `nth:`", "文字列と、`field:` か、`into:`（と `nth:`）です"),
            "submit" => (&["field", "into", "nth"], "`field:`, or `into:` with `nth:`", "`field:` か、`into:`（と `nth:`）です"),
            "press" => (&[], "the key", "キーだけです"),
            _ => (&[], "the milliseconds", "ミリ秒だけです"),
        };
        let mut positional: Vec<Tok> = Vec::new();
        let mut named: Vec<(Tok, Tok)> = Vec::new();
        let mut unknown = false;
        for a in args {
            match a.name {
                None if !named.is_empty() => return Err(self.expected(&a.value, "`)`", "`)`")),
                None => positional.push(a.value),
                Some(n) => {
                    if !named_ok.contains(&n.text.as_str()) {
                        self.e002(
                            &n,
                            format!("`{word}` takes no argument named `{}`; it takes {takes_en}", n.text),
                            format!("`{word}` に `{}` という名前の引数はありません。書けるのは{takes_ja}", n.text),
                        );
                        unknown = true;
                    } else if named.iter().any(|(m, _)| m.text == n.text) {
                        return Err(self.expected(&n, "`)`", "`)`"));
                    } else {
                        named.push((n, a.value));
                    }
                }
            }
        }
        let mut args_at: ArgsAt = Vec::new();
        // the positional argument a call takes: none, one, or one at most
        let (what_en, what_ja, kind, least, most): (&str, &str, Kind, usize, usize) = match word {
            "open" => ("the path, in quotes", "`\"` で囲んだパス", Kind::Str, 0, 1),
            "click" => ("the name of what to click, in quotes", "`\"` で囲んだ、クリックするものの名前", Kind::Str, 1, 1),
            "input" => ("the text to type, in quotes", "`\"` で囲んだ、入力する文字列", Kind::Str, 1, 1),
            "submit" => ("`field:`, `into:` or `)`", "`field:`、`into:`、`)` のどれか", Kind::Str, 0, 0),
            "press" => ("the key, in quotes, such as `\"enter\"`", "`\"` で囲んだキー（`\"enter\"` など）", Kind::Str, 1, 1),
            _ => ("the milliseconds, a whole number", "ミリ秒（整数）", Kind::Num, 1, 1),
        };
        if let Some(extra) = positional.get(most) {
            return Err(self.expected(extra, if most == 0 { what_en } else { "`)`" }, if most == 0 { what_ja } else { "`)`" }));
        }
        if positional.len() < least {
            let found = named.first().map(|(n, _)| n.clone()).unwrap_or_else(|| close.clone());
            return Err(self.expected(&found, what_en, what_ja));
        }
        if let Some(p) = positional.first()
            && p.kind != kind
        {
            return Err(self.expected(p, what_en, what_ja));
        }
        let first = positional.first().cloned();
        let get = |key: &str| named.iter().find(|(n, _)| n.text == key).cloned();
        let (nth, field, into) = (get("nth"), get("field"), get("into"));
        for (key, tok) in [("nth", &nth), ("field", &field), ("into", &into)] {
            if let Some((_, v)) = tok {
                args_at.push((key, at(v)));
            }
        }
        let nth_n = match &nth {
            Some((_, v)) => Some(self.count(v)?),
            None => None,
        };
        let place = if word == "input" || word == "submit" {
            match (&field, &into) {
                (Some(_), Some((n, _))) => {
                    self.e002(
                        n,
                        format!("`{word}` takes `field:` or `into:`, not both"),
                        format!("`{word}` に書けるのは `field:` か `into:` のどちらか一つです"),
                    );
                    unknown = true;
                    Place::First
                }
                (Some((_, v)), None) => {
                    if let Some((n, _)) = &nth {
                        self.e002(
                            n,
                            "`nth:` goes with `into:`, to pick among the fields of one name; a field by its place is `field: n`".to_string(),
                            "`nth:` は `into:` と一緒に書き、同じ名前のテキストフィールドのどれかを選びます。場所で選ぶなら、`field: n` と書いてください".to_string(),
                        );
                        unknown = true;
                    }
                    Place::Field(self.count(v)?)
                }
                (None, Some((_, v))) => {
                    if v.kind != Kind::Str {
                        return Err(self.expected(v, "the field's name, in quotes", "`\"` で囲んだテキストフィールドの名前"));
                    }
                    Place::Into { name: v.text.clone(), nth: nth_n }
                }
                (None, None) => {
                    if let Some((n, _)) = &nth {
                        self.e002(
                            n,
                            "`nth:` goes with `into:`, to pick among the fields of one name; a field by its place is `field: n`".to_string(),
                            "`nth:` は `into:` と一緒に書き、同じ名前のテキストフィールドのどれかを選びます。場所で選ぶなら、`field: n` と書いてください".to_string(),
                        );
                        unknown = true;
                    }
                    Place::First
                }
            }
        } else {
            Place::First
        };
        if let Some(f) = &first {
            let key = match word {
                "open" => "path",
                "click" => "name",
                "input" => "text",
                "press" => "key",
                _ => "ms",
            };
            args_at.push((key, at(f)));
        }
        let call = match word {
            "open" => Call::Open(first.map(|t| t.text)),
            "click" => Call::Click { name: first.map(|t| t.text).unwrap_or_default(), nth: nth_n },
            "input" => Call::Input { text: first.map(|t| t.text).unwrap_or_default(), place },
            "submit" => Call::Submit(place),
            "press" => Call::Press(first.map(|t| t.text).unwrap_or_default()),
            _ => {
                let tok = first.expect("advance has its argument");
                match tok.text.parse::<u64>() {
                    Ok(ms) => Call::Advance(ms),
                    Err(_) => {
                        return Err(self.e001(
                            &tok,
                            format!("`{}` is not a number of milliseconds; write a whole number", tok.text),
                            format!("`{}` はミリ秒として読めません。整数を書いてください", tok.text),
                        ));
                    }
                }
            }
        };
        Ok((if unknown { None } else { Some(call) }, args_at))
    }

    fn mask(&mut self) -> Result<Option<Mask>, Stop> {
        self.next(); // `mask`
        let what = self.expect(Kind::Ident, "`header`, `body json` or `screen`", "`header`、`body json`、`screen` のどれか")?;
        match what.text.as_str() {
            "screen" => Ok(self.node(true)?.map(Mask::Screen)),
            "header" => {
                let s = self.expect(Kind::Str, "the header's name, in quotes", "`\"` で囲んだヘッダーの名前")?;
                Ok(Some(Mask::Header(s.text.to_lowercase())))
            }
            "body" => {
                let j = self.next();
                if !(j.kind == Kind::Ident && j.text == "json") {
                    return Err(self.expected(&j, "`json`", "`json`"));
                }
                let s = self.expect(Kind::Str, "a JSON path, in quotes", "`\"` で囲んだ JSON のパス")?;
                self.json_path(&s);
                Ok(Some(Mask::BodyJson(s.text)))
            }
            other => {
                self.e002(
                    &what,
                    format!("`mask` takes `header \"<name>\"`, `body json \"<path>\"` or `screen <node>`, not `{other}`"),
                    format!("`mask` の後ろに書けるのは `header \"<名前>\"`、`body json \"<パス>\"`、`screen <ノード>` のどれかで、`{other}` は書けません"),
                );
                self.skip_line(what.line);
                Ok(None)
            }
        }
    }

    /// A check after `then` or `and`. None when its subject or matcher is unknown;
    /// the rest of that line is skipped, since it cannot be read as a check. A check
    /// E008 or E009 refuses is still returned, so that E007 can be said of it too.
    fn check(&mut self) -> Result<Option<Check>, Stop> {
        let subj = self.expect(
            Kind::Ident,
            "what to check (stdout, stderr, exit, status, header, body, body json or screen)",
            "チェックするもの（stdout、stderr、exit、status、header、body、body json、screen）",
        )?;
        let subject = match subj.text.as_str() {
            "screen" => Subject::Screen,
            "stdout" => Subject::Stdout,
            "stderr" => Subject::Stderr,
            "exit" => Subject::Exit,
            "status" => Subject::Status,
            "header" => {
                let h = self.expect(Kind::Str, "the header's name, in quotes", "`\"` で囲んだヘッダーの名前")?;
                Subject::Header(h.text.to_lowercase())
            }
            "body" => {
                if self.peek().kind == Kind::Ident && self.peek().text == "json" {
                    self.next();
                    let p = self.expect(Kind::Str, "a JSON path, in quotes", "`\"` で囲んだ JSON のパス")?;
                    self.json_path(&p);
                    Subject::BodyJson(p.text)
                } else {
                    Subject::Body
                }
            }
            other => {
                self.e002(
                    &subj,
                    format!("a check cannot name `{other}`; it names stdout, stderr, exit, status, header, body, body json or screen"),
                    format!("`{other}` はチェックできません。チェックできるのは stdout、stderr、exit、status、header、body、body json、screen です"),
                );
                self.skip_line(subj.line);
                return Ok(None);
            }
        };
        let Some(raw) = self.matcher()? else {
            return Ok(None);
        };
        if let Some(d) = misfit(&subject, &raw) {
            self.diags.push(d);
        }
        let matcher = self.build(raw);
        Ok(Some(Check { subject, matcher, pos: at(&subj) }))
    }

    /// The matcher after the subject, as written (DESIGN §6). None when its word is
    /// unknown (E002).
    fn matcher(&mut self) -> Result<Option<RawMatcher>, Stop> {
        let m = self.expect(
            Kind::Ident,
            "a way to compare (is, contains, matches or exists)",
            "比べ方（is、contains、matches、exists）",
        )?;
        let next_is = |p: &Self, w: &str| p.peek().kind == Kind::Ident && p.peek().text == w;
        let (form, values) = match m.text.as_str() {
            "is" => {
                if next_is(self, "not") {
                    self.next();
                    (Form::IsNot, vec![self.value()?])
                } else if next_is(self, "above") || next_is(self, "below") {
                    let w = self.next();
                    (if w.text == "above" { Form::Above } else { Form::Below }, vec![self.value()?])
                } else if next_is(self, "at") {
                    self.next();
                    let w = self.next();
                    let form = match (w.kind == Kind::Ident, w.text.as_str()) {
                        (true, "least") => Form::AtLeast,
                        (true, "most") => Form::AtMost,
                        _ => return Err(self.expected(&w, "`least` or `most`", "`least` か `most`")),
                    };
                    (form, vec![self.value()?])
                } else if next_is(self, "between") {
                    self.next();
                    let low = self.value()?;
                    let and = self.next();
                    if !(and.kind == Kind::Ident && and.text == "and") {
                        return Err(self.expected(&and, "`and`", "`and`"));
                    }
                    (Form::Between, vec![low, self.value()?])
                } else {
                    (Form::Is, vec![self.value()?])
                }
            }
            "contains" if self.node_ahead() => {
                let first = self.peek().clone();
                let Some(p) = self.node(false)? else {
                    return Ok(None);
                };
                return Ok(Some(RawMatcher { form: Form::ContainsNode, word: m, values: vec![], node: Some((p, first)) }));
            }
            "contains" => (Form::Contains, vec![self.value()?]),
            "matches" => (Form::Matches, vec![self.pattern()?]),
            "exists" => (Form::Exists, vec![]),
            "does" => {
                let not = self.next();
                if !(not.kind == Kind::Ident && not.text == "not") {
                    return Err(self.expected(&not, "`not`", "`not`"));
                }
                let w = self.expect(Kind::Ident, "`contain`, `match` or `exist`", "`contain`、`match`、`exist` のどれか")?;
                match w.text.as_str() {
                    "contain" if self.node_ahead() => {
                        let first = self.peek().clone();
                        let Some(p) = self.node(false)? else {
                            return Ok(None);
                        };
                        return Ok(Some(RawMatcher { form: Form::NotContainsNode, word: m, values: vec![], node: Some((p, first)) }));
                    }
                    "contain" => (Form::NotContains, vec![self.value()?]),
                    "match" => (Form::NotMatches, vec![self.pattern()?]),
                    "exist" => (Form::NotExists, vec![]),
                    other => {
                        self.e002(
                            &w,
                            format!("`does not {other}` is not a way to compare; after `does not` come contain, match and exist"),
                            format!("`does not {other}` という比べ方はありません。`does not` の後ろに書けるのは contain、match、exist です"),
                        );
                        self.skip_line(w.line);
                        return Ok(None);
                    }
                }
            }
            other => {
                self.e002(
                    &m,
                    format!("`{other}` is not a way to compare; the ways are is, is not, is above, is below, is at least, is at most, is between, contains, does not contain, matches, does not match, exists and does not exist"),
                    format!("`{other}` という比べ方はありません。書けるのは is、is not、is above、is below、is at least、is at most、is between、contains、does not contain、matches、does not match、exists、does not exist です"),
                );
                self.skip_line(m.line);
                return Ok(None);
            }
        };
        Ok(Some(RawMatcher { form, word: m, values, node: None }))
    }

    /// Whether a node pattern comes next, after `contains`: a word that is not a
    /// value (`true`, `false`, `null`) and does not start what follows a check.
    fn node_ahead(&self) -> bool {
        let p = self.peek();
        p.kind == Kind::Ident && !["true", "false", "null"].contains(&p.text.as_str()) && !NEXT_WORDS.contains(&p.text.as_str())
    }

    /// A role, E002 when it is not one a pattern may name.
    fn role(&mut self) -> Result<Option<String>, Stop> {
        let r = self.expect(Kind::Ident, "a role, such as `button` or `text`", "ロール（`button`、`text` など）")?;
        if screen::ROLES.contains(&r.text.as_str()) {
            return Ok(Some(r.text));
        }
        self.e002(
            &r,
            format!("`{}` is not a role; a role is one of WAI-ARIA's, such as button, link, textbox, heading, checkbox, dialog or listitem, or `text` for static text", r.text),
            format!("`{}` というロールはありません。ロールは WAI-ARIA のもの（button、link、textbox、heading、checkbox、dialog、listitem など）か、静的な文字列を表す `text` です", r.text),
        );
        self.skip_line(r.line);
        Ok(None)
    }

    /// A node's label, if one comes: a name in quotes, `containing "…"`, or
    /// `matching "…"`, whose pattern reads a backslash as `matches` does.
    fn label(&mut self) -> Result<Option<Label>, Stop> {
        let p = self.peek().clone();
        match (&p.kind, p.text.as_str()) {
            (Kind::Str, _) => {
                self.next();
                self.plain(&p)?;
                Ok(Some(Label::Is(p.text)))
            }
            (Kind::Ident, "containing") => {
                self.next();
                let s = self.expect(Kind::Str, "the part of the name, in quotes", "`\"` で囲んだ名前の一部")?;
                Ok(Some(Label::Containing(s.text)))
            }
            (Kind::Ident, "matching") => {
                self.next();
                let s = self.next();
                if s.kind != Kind::Str {
                    return Err(self.expected(&s, "a pattern, in quotes", "`\"` で囲んだパターン"));
                }
                Ok(Some(Label::Matching(self.compile(&s))))
            }
            _ => Ok(None),
        }
    }

    /// A node pattern (DESIGN §6): `["exactly" NUM] ROLE [label] ["with" "value"
    /// STR] [state] ["in" ROLE [label]]`. None when a word in it is unknown (E002),
    /// the rest of the line skipped. A mask names nodes, not a count of them.
    fn node(&mut self, in_mask: bool) -> Result<Option<screen::Pattern>, Stop> {
        let mut count = None;
        if self.peek().kind == Kind::Ident && self.peek().text == "exactly" {
            let e = self.next();
            if in_mask {
                return Err(self.e001(
                    &e,
                    "a mask takes nodes out of drift and names them without a count: `mask screen text \"clock\"`".to_string(),
                    "マスクはノードをドリフトから外すもので、数は書けません（`mask screen text \"clock\"`）".to_string(),
                ));
            }
            let n = self.next();
            count = Some(self.count(&n)?);
        }
        let Some(role) = self.role()? else {
            return Ok(None);
        };
        let label = self.label()?;
        let mut value = None;
        if self.peek().kind == Kind::Ident && self.peek().text == "with" {
            self.next();
            let v = self.next();
            if !(v.kind == Kind::Ident && v.text == "value") {
                return Err(self.expected(&v, "`value`", "`value`"));
            }
            let s = self.expect(Kind::Str, "the value, in quotes", "`\"` で囲んだ値")?;
            value = Some(s.text);
        }
        let mut state = None;
        if self.peek().kind == Kind::Ident
            && let Some(st) = screen::State::parse(&self.peek().text)
        {
            self.next();
            state = Some(st);
        }
        let mut within = None;
        if self.peek().kind == Kind::Ident && self.peek().text == "in" {
            self.next();
            let Some(r) = self.role()? else {
                return Ok(None);
            };
            within = Some((r, self.label()?));
        }
        let next = self.peek().clone();
        if next.kind == Kind::Ident && !NEXT_WORDS.contains(&next.text.as_str()) {
            self.e002(
                &next,
                format!("`{}` is not a state; after a node's role come its name, `with value \"…\"`, a state (disabled, enabled, checked or unchecked) and `in <role>`, in that order", next.text),
                format!("`{}` という状態はありません。ノードのロールの後ろには、名前、`with value \"…\"`、状態（disabled、enabled、checked、unchecked）、`in <ロール>` をこの順に書いてください", next.text),
            );
            self.skip_line(next.line);
            return Ok(None);
        }
        Ok(Some(screen::Pattern { count, role, label, value, state, within }))
    }

    /// A pattern in a string, E009 when it does not parse (then a pattern that
    /// matches only the empty value, since the spec will not run).
    fn compile(&mut self, tok: &Tok) -> crate::regex::Pattern {
        match Pattern::new(&tok.text) {
            Ok(p) => p,
            Err(bad) => {
                let col = tok.cols.get(bad.at).copied().unwrap_or(tok.col);
                self.diags.push(
                    diag::error(
                        "E009",
                        tok.line,
                        col,
                        tr!("パターンとして読めません: {}", "the pattern does not parse: {}", bad.why.ja; bad.why.en),
                    )
                    .note(pattern_rule()),
                );
                Pattern::new("").expect("the empty pattern")
            }
        }
    }

    /// A value: a string, a number, `true`, `false` or `null`.
    fn value(&mut self) -> Result<(Value, Tok), Stop> {
        let v = self.next();
        let value = match v.kind {
            Kind::Str => {
                self.plain(&v)?;
                Value::S(v.text.clone())
            }
            Kind::Num => match v.text.parse::<f64>() {
                Ok(n) => Value::N(n),
                Err(_) => {
                    return Err(self.e001(
                        &v,
                        format!("`{}` is not a number", v.text),
                        format!("`{}` は数として読めません", v.text),
                    ));
                }
            },
            Kind::Ident if v.text == "true" => Value::Bool(true),
            Kind::Ident if v.text == "false" => Value::Bool(false),
            Kind::Ident if v.text == "null" => Value::Null,
            _ => {
                return Err(self.expected(
                    &v,
                    "a value: a string, a number, true, false or null",
                    "値（文字列、数、true、false、null）",
                ));
            }
        };
        Ok((value, v))
    }

    /// The value after `matches`: a string read as a pattern, in which a backslash
    /// before a letter is the pattern's own (`\d`), or any other value for E008.
    fn pattern(&mut self) -> Result<(Value, Tok), Stop> {
        if self.peek().kind == Kind::Str {
            let v = self.next();
            return Ok((Value::S(v.text.clone()), v));
        }
        self.value()
    }

    /// The matcher a raw one stands for; E009 for a pattern that does not parse. A
    /// matcher E008 refused is built all the same, from what is there, since the
    /// spec will not run.
    fn build(&mut self, raw: RawMatcher) -> Matcher {
        let num = |i: usize| match raw.values.get(i) {
            Some((Value::N(n), _)) => *n,
            _ => 0.0,
        };
        let text = |i: usize| match raw.values.get(i) {
            Some((Value::S(s), _)) => s.clone(),
            _ => String::new(),
        };
        let value = || raw.values.first().map(|v| v.0.clone()).unwrap_or(Value::Null);
        match raw.form {
            Form::Is => Matcher::Is(value()),
            Form::IsNot => Matcher::IsNot(value()),
            Form::Above => Matcher::Above(num(0)),
            Form::Below => Matcher::Below(num(0)),
            Form::AtLeast => Matcher::AtLeast(num(0)),
            Form::AtMost => Matcher::AtMost(num(0)),
            Form::Between => Matcher::Between(num(0), num(1)),
            Form::Contains => Matcher::Contains(text(0)),
            Form::NotContains => Matcher::NotContains(text(0)),
            Form::Matches | Form::NotMatches => {
                let p = match raw.values.first() {
                    Some((Value::S(_), tok)) => self.compile(&tok.clone()),
                    _ => Pattern::new("").expect("the empty pattern"),
                };
                if raw.form == Form::Matches { Matcher::Matches(p) } else { Matcher::NotMatches(p) }
            }
            Form::Exists => Matcher::Exists,
            Form::NotExists => Matcher::NotExists,
            Form::ContainsNode => Matcher::ContainsNode(raw.node.expect("a node form has its node").0),
            Form::NotContainsNode => Matcher::NotContainsNode(raw.node.expect("a node form has its node").0),
        }
    }

    /// E009 for a JSON path that does not read, pointing inside the string.
    fn json_path(&mut self, tok: &Tok) {
        if let Err((at, why)) = crate::json::check_path(&tok.text) {
            let col = tok.cols.get(at).copied().unwrap_or(tok.col);
            self.diags.push(
                diag::error(
                    "E009",
                    tok.line,
                    col,
                    tr!("JSON のパスとして読めません: {}", "the JSON path does not read: {}", why.ja(); why.en()),
                )
                .note(tr!(
                    "パスは `.key`、`.a.b`、`.items[0].name` のように書いてください",
                    "a path is written `.key`, `.a.b` or `.items[0].name`",
                )),
            );
        }
    }

    /// E001 for a string with a backslash before a character other than `n`, `t`,
    /// `"` and `\`, outside a pattern.
    fn plain(&mut self, tok: &Tok) -> Result<(), Stop> {
        let Some((col, e)) = tok.odd else {
            return Ok(());
        };
        let mut d = diag::error(
            "E001",
            tok.line,
            col,
            tr!(
                "`\\{e}` というエスケープはありません。文字列で使えるのは `\\n`、`\\t`、`\\\"`、`\\\\` です",
                "`\\{e}` is not an escape; a string takes `\\n`, `\\t`, `\\\"` and `\\\\`",
            ),
        );
        if "dwsDWS".contains(e) {
            d = d.note(tr!(
                "`matches` の後ろのパターンなら `\\{e}` をそのまま書けます。それ以外では `\\\\{e}` と書いてください",
                "a pattern, after `matches`, reads `\\{e}` itself; anywhere else write `\\\\{e}`",
            ));
        }
        self.diags.push(d);
        Err(Stop)
    }
}

/// What patterns are made of, the note E009 gives.
fn pattern_rule() -> Text {
    tr!(
        "パターンに書けるのは、文字そのもの、`.`、`[a-z]` や `[^0-9]` のような文字クラス、`\\d`・`\\w`・`\\s` とその大文字、グループ、`|`、`*`・`+`・`?`、`{{n}}`・`{{n,}}`・`{{n,m}}` で、値の全体と照らし合わせます",
        "a pattern takes literals, `.`, classes such as `[a-z]` and `[^0-9]`, `\\d`, `\\w`, `\\s` and their capitals, groups, `|`, `*`, `+`, `?`, `{{n}}`, `{{n,}}` and `{{n,m}}`, and matches the whole value",
    )
}

/// A matcher's form, before it is held to its subject.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    Is,
    IsNot,
    Above,
    Below,
    AtLeast,
    AtMost,
    Between,
    Contains,
    NotContains,
    Matches,
    NotMatches,
    Exists,
    NotExists,
    /// `contains` with a node, on the screen.
    ContainsNode,
    NotContainsNode,
}

impl Form {
    fn words(self) -> &'static str {
        match self {
            Form::Is => "is",
            Form::IsNot => "is not",
            Form::Above => "is above",
            Form::Below => "is below",
            Form::AtLeast => "is at least",
            Form::AtMost => "is at most",
            Form::Between => "is between",
            Form::Contains | Form::ContainsNode => "contains",
            Form::NotContains | Form::NotContainsNode => "does not contain",
            Form::Matches => "matches",
            Form::NotMatches => "does not match",
            Form::Exists => "exists",
            Form::NotExists => "does not exist",
        }
    }

    fn numeric(self) -> bool {
        matches!(self, Form::Above | Form::Below | Form::AtLeast | Form::AtMost | Form::Between)
    }
}

/// A matcher as written: its form, the word it starts with, and its values, or
/// its node pattern with the token it starts at.
struct RawMatcher {
    form: Form,
    word: Tok,
    values: Vec<(Value, Tok)>,
    node: Option<(screen::Pattern, Tok)>,
}

/// E008: the first way the matcher does not fit its subject or its value (DESIGN
/// §9), at the word or the value that does not fit.
fn misfit(subject: &Subject, raw: &RawMatcher) -> Option<Diag> {
    let s = subject.word();
    let f = raw.form.words();
    let at_word = |en: String, ja: String| Some(diag::error("E008", raw.word.line, raw.word.col, Text::new(ja, en)));
    let at_value = |i: usize, en: String, ja: String| {
        let tok = &raw.values[i].1;
        Some(diag::error("E008", tok.line, tok.col, Text::new(ja, en)))
    };
    let first = raw.values.first().map(|v| &v.0);
    let node_form = matches!(raw.form, Form::ContainsNode | Form::NotContainsNode);
    if *subject == Subject::Screen {
        if node_form {
            return None;
        }
        return at_word(
            format!("`screen` is checked for the nodes it holds, with `contains` or `does not contain` and a node, not with `{f}`: `screen contains button \"greet\"`"),
            format!("`screen` は画面にあるノードを、`contains` か `does not contain` と後ろに書いたノードでチェックします。`{f}` は使えません（`screen contains button \"greet\"`）"),
        );
    }
    if node_form {
        let tok = &raw.node.as_ref().expect("a node form has its node").1;
        let (en, ja) = if subject.is_number() {
            (
                format!("`{s}` is a number, and a node is what a screen holds; compare it with `is`"),
                format!("`{s}` は数で、ノードを持つのは画面です。`is` で比べてください"),
            )
        } else {
            (
                format!("`{s}` is a text, and a node is what a screen holds; to look for a part of the text, write it in quotes: `{s} {f} \"…\"`"),
                format!("`{s}` は文字列で、ノードを持つのは画面です。文字列の一部を探すなら、`\"` で囲んで書いてください（`{s} {f} \"…\"`）"),
            )
        };
        return Some(diag::error("E008", tok.line, tok.col, Text::new(ja, en)));
    }
    if subject.is_number() {
        match raw.form {
            Form::Contains | Form::NotContains => {
                return at_word(
                    format!("`{s}` is a number, and `{f}` looks for a part of a text; compare it with `is`"),
                    format!("`{s}` は数なので、文字列の一部を探す `{f}` は使えません。`is` で比べてください"),
                );
            }
            Form::Matches | Form::NotMatches => {
                return at_word(
                    format!("`{s}` is a number, and `{f}` holds a text to a pattern; compare it with `is` or `is between`"),
                    format!("`{s}` は数なので、文字列をパターンと照らし合わせる `{f}` は使えません。`is` か `is between` で比べてください"),
                );
            }
            Form::Is | Form::IsNot => match first {
                Some(Value::S(text)) => {
                    let n = text.trim().parse::<f64>().map(crate::json::render_num).unwrap_or_else(|_| "0".into());
                    return at_value(
                        0,
                        format!("`{s}` is a number; write the value without quotes: `{s} {f} {n}`"),
                        format!("`{s}` は数です。値は `\"` で囲まずに書いてください（`{s} {f} {n}`）"),
                    );
                }
                Some(v @ (Value::Bool(_) | Value::Null)) => {
                    let v = v.shown();
                    return at_value(
                        0,
                        format!("`{s}` is a number, and `{v}` is not one; write a number: `{s} {f} 0`"),
                        format!("`{s}` は数なので、数でない `{v}` とは比べられません。数を書いてください（`{s} {f} 0`）"),
                    );
                }
                _ => {}
            },
            _ => {}
        }
    }
    let text_subject = matches!(subject, Subject::Stdout | Subject::Stderr | Subject::Body | Subject::Header(_));
    if text_subject
        && matches!(raw.form, Form::Is | Form::IsNot)
        && let Some(v @ (Value::Bool(_) | Value::Null)) = first
    {
        let v = v.shown();
        return at_value(
            0,
            format!("`{v}` is a JSON value, and `{s}` is a text; to compare it with the word, write `{s} {f} \"{v}\"`"),
            format!("`{v}` は JSON の値で、`{s}` は文字列です。その語と比べるなら、`{s} {f} \"{v}\"` と書いてください"),
        );
    }
    if matches!(raw.form, Form::Exists | Form::NotExists) && !subject.may_be_absent() {
        return at_word(
            format!("`{f}` asks whether a header was sent or a JSON path leads to a value, and `{s}` is always there; check its value instead"),
            format!("`{f}` が確かめるのは、ヘッダーが送られたか、JSON のパスの先に値があるかです。`{s}` はいつもあるので、値のほうをチェックしてください"),
        );
    }
    if raw.form.numeric() {
        for (i, (v, _)) in raw.values.iter().enumerate() {
            if !matches!(v, Value::N(_)) {
                let n = match v {
                    Value::S(text) => text.trim().parse::<f64>().map(crate::json::render_num).unwrap_or_else(|_| "5".into()),
                    _ => "5".into(),
                };
                return at_value(
                    i,
                    format!("`{f}` compares numbers; write a number without quotes, as `{f} {n}`"),
                    format!("`{f}` は数を比べます。数を `\"` で囲まずに書いてください（`{f} {n}` のように）"),
                );
            }
        }
        if let [(Value::N(a), _), (Value::N(b), _)] = raw.values.as_slice()
            && a > b
        {
            let (a, b) = (crate::json::render_num(*a), crate::json::render_num(*b));
            return at_value(
                0,
                format!("`is between {a} and {b}` holds for no number; write the smaller end first: `is between {b} and {a}`"),
                format!("`is between {a} and {b}` に当てはまる数はありません。小さいほうを先に書いてください（`is between {b} and {a}`）"),
            );
        }
    }
    if matches!(raw.form, Form::Contains | Form::NotContains)
        && let Some(v) = first.filter(|v| !matches!(v, Value::S(_)))
    {
        let v = v.shown();
        return at_value(
            0,
            format!("`{f}` looks for a part of a text; write the value in quotes: `{f} \"{v}\"`"),
            format!("`{f}` は文字列の一部を探します。値は `\"` で囲んで書いてください（`{f} \"{v}\"`）"),
        );
    }
    if matches!(raw.form, Form::Matches | Form::NotMatches)
        && let Some(v) = first.filter(|v| !matches!(v, Value::S(_)))
    {
        let v = v.shown();
        return at_value(
            0,
            format!("`{f}` holds a text to a pattern written in quotes: `{f} \"{v}\"`"),
            format!("`{f}` は文字列を、`\"` で囲んだパターンと照らし合わせます（`{f} \"{v}\"`）"),
        );
    }
    None
}

/// Parses a claims file. On any error, the diagnostics in line order.
pub fn parse(src: &str) -> Result<Spec, Vec<Diag>> {
    let toks = match lex::lex(src) {
        Ok(t) => t,
        Err(d) => return Err(vec![d]),
    };
    let mut p = P { toks, pos: 0, diags: Vec::new() };
    let raw = p.file();
    let mut diags = p.diags;
    let spec = match raw {
        Ok(raw) => static_checks(raw, &mut diags),
        Err(Stop) => None,
    };
    diag::sort(&mut diags);
    match spec {
        Some(spec) if !diag::has_errors(&diags) => Ok(spec),
        _ => Err(diags),
    }
}

/// The first is on line N: the note E003 and E004 give a second of something.
fn first_on(line: usize) -> Text {
    tr!("一つ目は {line} 行目です", "the first is on line {line}")
}

/// How geas splits a command, the note E010 gives when one does not split.
fn split_rule() -> Text {
    tr!(
        "geas はコマンドを、シェルを通さずに自分で語に分けます。空白で区切り、`'…'` は中をそのまま残し、`\"…\"` は空白を残して中の `\\\"` と `\\\\` をエスケープとして扱い、クォートの外のバックスラッシュは次の文字をそのまま残します",
        "geas splits a command into words itself, never through a shell: blanks separate words, `'…'` keeps what is in it as it is, `\"…\"` keeps blanks and reads `\\\"` and `\\\\`, and outside quotes a backslash keeps the next character",
    )
}

/// What a target that has no port is, for a message: `runs a command`.
fn portless(word: &str) -> (&'static str, &'static str) {
    match word {
        "pixie" => ("is a pixie app", "pixie のアプリのターゲット"),
        "driver" => ("is a GUI reached through a driver", "ドライバーで動かす GUI のターゲット"),
        _ => ("runs a command", "コマンドを走らせるターゲット"),
    }
}

/// What a target is, as far as its lines say, with E004 for lines that do not fit
/// and E010 for `{port}` on a target that has no port.
fn target_kind(tg: &RawTarget, pins: Option<&Pins>, diags: &mut Vec<Diag>) -> Option<TargetKind> {
    let name = &tg.name;
    for (word, places) in [
        ("run", tg.runs.iter().map(|r| r.pos).collect::<Vec<_>>()),
        ("serve", tg.serves.iter().map(|r| r.pos).collect()),
        ("pixie", tg.pixies.iter().map(|r| r.pos).collect()),
        ("driver", tg.drivers.iter().map(|r| r.pos).collect()),
        ("port", tg.ports.iter().map(|r| r.1).collect()),
        ("serial", tg.serials.clone()),
    ] {
        if let [first, second, ..] = places.as_slice() {
            diags.push(
                diag::error(
                    "E004",
                    second.line,
                    second.col,
                    tr!(
                        "ターゲット `{name}` に `{word}` の行が二つあります",
                        "the target `{name}` has a second `{word}` line",
                    ),
                )
                .note(first_on(first.line)),
            );
        }
    }
    let mut kinds: Vec<(&str, &RawCmd)> = [
        ("run", tg.runs.first()),
        ("serve", tg.serves.first()),
        ("pixie", tg.pixies.first()),
        ("driver", tg.drivers.first()),
    ]
    .into_iter()
    .filter_map(|(w, c)| c.map(|c| (w, c)))
    .collect();
    kinds.sort_by_key(|(_, c)| (c.pos.line, c.pos.col));
    let (word, cmd) = match kinds.as_slice() {
        [] => {
            if tg.unknown_line {
                return None;
            }
            diags.push(diag::error(
                "E004",
                tg.kw_pos.line,
                tg.kw_pos.col,
                tr!(
                    "ターゲット `{name}` に `run`、`serve`、`pixie`、`driver` のどれもありません",
                    "the target `{name}` has none of `run`, `serve`, `pixie` and `driver`",
                ),
            ));
            return None;
        }
        [(a, _), (b, later), ..] => {
            let (en, ja) = if (*a, *b) == ("run", "serve") {
                (
                    format!("the target `{name}` has both `run` and `serve`; a target runs a command or serves, not both"),
                    format!("ターゲット `{name}` に `run` と `serve` の両方があります。ターゲットはコマンドかサービスのどちらか一つです"),
                )
            } else {
                (
                    format!("the target `{name}` has both `{a}` and `{b}`; a target is one of a command (`run`), a service (`serve`), a pixie app (`pixie`) and a GUI reached through a driver (`driver`)"),
                    format!("ターゲット `{name}` に `{a}` と `{b}` の両方があります。ターゲットは、コマンド（`run`）、サービス（`serve`）、pixie のアプリ（`pixie`）、ドライバーで動かす GUI（`driver`）のどれか一つです"),
                )
            };
            diags.push(diag::error("E004", later.pos.line, later.pos.col, Text::new(ja, en)));
            return None;
        }
        [(w, c)] => (*w, *c),
    };
    if word != "serve" {
        if let Some((_, p)) = tg.ports.first() {
            // said, and the target is still what its command makes it, so that its
            // claims are checked too
            diags.push(diag::error(
                "E004",
                p.line,
                p.col,
                tr!(
                    "ターゲット `{name}` に `port` がありますが `serve` がありません。ポートを持つのはサービスだけです",
                    "the target `{name}` has `port` but no `serve`; only a service has a port",
                ),
            ));
        } else if let Some(p) = cmd.port_at {
            let (en, ja) = portless(word);
            diags.push(
                diag::error(
                    "E010",
                    p.line,
                    p.col,
                    tr!(
                        "`{{port}}` はターゲットのポート番号に置き換わりますが、`{name}` は{ja}なので、ポートを持ちません",
                        "`{{port}}` stands for the target's port, and `{name}` {en}, which has none",
                    ),
                )
                .note(tr!("ポートを持つのはサービス（`serve` と `port` のあるターゲット）だけです", "only a service has a port: a target with `serve` and `port`")),
            );
            return None;
        }
        let words = cmd.words.clone()?;
        return Some(match word {
            "pixie" => TargetKind::Pixie(words),
            "driver" => TargetKind::Driver(words),
            _ => TargetKind::Run(words),
        });
    }
    let s = cmd;
    match tg.ports.first() {
        Some((Port::Auto, p)) => {
            let words = s.words.clone()?;
            let in_env = pins.is_some_and(|ps| ps.env.iter().any(|(_, v)| v.contains(words::PORT)));
            if s.port_at.is_none() && !in_env {
                diags.push(
                    diag::error(
                        "E004",
                        p.line,
                        p.col,
                        tr!(
                            "ターゲット `{name}` は `port auto` ですが、コマンドにも `env` にも `{{port}}` がありません。geas が渡すポート番号を、サービスが知る方法がありません",
                            "the target `{name}` has `port auto`, and neither its command nor its `env` says `{{port}}`: the service has no way to learn the port geas gives it",
                        ),
                    )
                    .note(tr!(
                        "サービスがポート番号を読むところに `{{port}}` を書いてください（`serve \"python3 server.py {{port}}\"`、または `env \"PORT\" \"{{port}}\"`）",
                        "write `{{port}}` where the service reads its port: `serve \"python3 server.py {{port}}\"`, or `env \"PORT\" \"{{port}}\"`",
                    )),
                );
                return None;
            }
            Some(TargetKind::Serve { words, port: Port::Auto })
        }
        Some((port, _)) => Some(TargetKind::Serve { words: s.words.clone()?, port: *port }),
        None if tg.unknown_line => None,
        None => {
            diags.push(diag::error(
                "E004",
                s.pos.line,
                s.pos.col,
                tr!(
                    "ターゲット `{name}` に `serve` はありますが `port` がありません。geas はそのポートが開くのを待ち、リクエストをそこへ送ります",
                    "the target `{name}` has `serve` but no `port`; geas waits for that port to open and sends its requests there",
                ),
            ));
            None
        }
    }
}

/// A pin as written (DESIGN §12).
#[derive(Clone, Debug)]
enum PinKind {
    Clean,
    Pass(String),
    Set(String, String),
    Tz(String),
    Locale(String),
    Clock(String, Option<String>),
    Seed(u64, Option<String>),
}

#[derive(Clone, Debug)]
struct RawPin {
    kind: PinKind,
    /// Where its first word is written.
    pos: Pos,
    /// For `env "NAME" "value"`: where `{port}` first appears in the value.
    port_at: Option<Pos>,
}

impl RawPin {
    /// What one place may pin once: a second of the same key is E003.
    fn key(&self) -> String {
        match &self.kind {
            PinKind::Clean => "clean".into(),
            PinKind::Pass(n) => format!("pass {n}"),
            PinKind::Set(n, _) => format!("env {n}"),
            PinKind::Tz(_) => "tz".into(),
            PinKind::Locale(_) => "locale".into(),
            PinKind::Clock(..) => "clock".into(),
            PinKind::Seed(..) => "seed".into(),
        }
    }

    /// The pin as a message names it.
    fn named(&self) -> (String, String) {
        match &self.kind {
            PinKind::Clean => ("`env clean`".into(), "`env clean`".into()),
            PinKind::Pass(n) => (format!("`env pass \"{n}\"`"), format!("`env pass \"{n}\"`")),
            PinKind::Set(n, _) => (format!("the variable `{n}`"), format!("環境変数 `{n}`")),
            PinKind::Tz(_) => ("`tz`".into(), "`tz`".into()),
            PinKind::Locale(_) => ("`locale`".into(), "`locale`".into()),
            PinKind::Clock(..) => ("`clock`".into(), "`clock`".into()),
            PinKind::Seed(..) => ("`seed`".into(), "`seed`".into()),
        }
    }

    /// The pin as written, short: `env "TZ"`, `tz`.
    fn written(&self) -> String {
        match &self.kind {
            PinKind::Clean => "`env clean`".into(),
            PinKind::Pass(n) => format!("`env pass \"{n}\"`"),
            PinKind::Set(n, _) => format!("`env \"{n}\"`"),
            PinKind::Tz(_) => "`tz`".into(),
            PinKind::Locale(_) => "`locale`".into(),
            PinKind::Clock(..) => "`clock`".into(),
            PinKind::Seed(..) => "`seed`".into(),
        }
    }

    /// The variables the pin sets in a process's environment.
    fn variables(&self) -> Vec<String> {
        match &self.kind {
            PinKind::Clean => vec![],
            PinKind::Pass(n) | PinKind::Set(n, _) => vec![n.clone()],
            PinKind::Tz(_) => vec!["TZ".into()],
            PinKind::Locale(_) => vec!["LANG".into(), "LC_ALL".into()],
            PinKind::Clock(_, env) => env.iter().cloned().collect(),
            PinKind::Seed(_, env) => env.iter().cloned().collect(),
        }
    }
}

/// One place's pins with each key once; E003 for a second, which is dropped.
fn once(target: Option<&str>, pins: &[RawPin], diags: &mut Vec<Diag>) -> Vec<RawPin> {
    let mut out: Vec<RawPin> = Vec::new();
    for p in pins {
        if let Some(first) = out.iter().find(|q| q.key() == p.key()) {
            let (what_en, what_ja) = p.named();
            let (where_en, where_ja) = match target {
                Some(t) => (format!("in the target `{t}`"), format!("ターゲット `{t}` の中で")),
                None => ("outside any target".to_string(), "ターゲットの外で".to_string()),
            };
            let gap = if what_ja.starts_with('`') { " " } else { "" };
            diags.push(
                diag::error(
                    "E003",
                    p.pos.line,
                    p.pos.col,
                    tr!("{where_ja}{gap}{what_ja} を二回固定しています", "{what_en} is pinned twice {where_en}"),
                )
                .note(first_on(first.pos.line)),
            );
        } else {
            out.push(p.clone());
        }
    }
    out
}

/// The pins in effect on a target: the top level's, each replaced by the target's
/// pin of the same key, then the target's others.
fn in_effect(top: &[RawPin], own: &[RawPin]) -> Vec<RawPin> {
    let mut out: Vec<RawPin> = top.iter().map(|p| own.iter().find(|q| q.key() == p.key()).unwrap_or(p).clone()).collect();
    for p in own {
        if !top.iter().any(|q| q.key() == p.key()) {
            out.push(p.clone());
        }
    }
    out
}

/// A list of targets as a message names them: `a`, `a` and `b`, `a`, `b` and `c`.
fn targets_named(names: &[String]) -> (String, String) {
    let q: Vec<String> = names.iter().map(|n| format!("`{n}`")).collect();
    let en = match q.as_slice() {
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        [] => String::new(),
    };
    (en, q.join("、"))
}

/// The checks on the pins in effect on every target (DESIGN §12): a variable set by
/// two pins (E003), `clock` or `seed` with nothing to pass them in (E011), `{port}`
/// in a value on a target without a port (E010). A top-level pin is checked on
/// every target and said once, naming the targets it fails on. The pins of each
/// target, in the model's form.
fn pins_in_effect(raw: &RawSpec, diags: &mut Vec<Diag>) -> HashMap<String, Pins> {
    let top = once(None, &raw.pins, diags);
    let mut out = HashMap::new();
    // the services some claim drives as a page: `clock` and `seed` keep the page's
    // clock and random numbers there, without a variable
    let pages: Vec<&str> = raw
        .claims
        .iter()
        .flat_map(|c| c.steps.iter())
        .filter_map(|s| match s {
            RawStep::When { target, call: Some(call), .. } if call.observes() == Observes::Screen => Some(target.as_str()),
            _ => None,
        })
        .collect();
    let mut twice: Vec<Diag> = Vec::new();
    // per pin (or `{port}` in its value): the code and the targets it fails on
    let mut failing: Vec<(Pos, &'static str, Vec<String>)> = Vec::new();
    for tg in &raw.targets {
        let own = once(Some(&tg.name), &tg.pins, diags);
        let pins = in_effect(&top, &own);
        // each variable from one pin, the later one said
        let mut by_place: Vec<&RawPin> = pins.iter().collect();
        by_place.sort_by_key(|p| (p.pos.line, p.pos.col));
        let mut set: Vec<(String, &RawPin)> = Vec::new();
        for p in by_place {
            for v in p.variables() {
                let Some((_, first)) = set.iter().find(|(name, _)| *name == v) else {
                    set.push((v, p));
                    continue;
                };
                if twice.iter().any(|d| (d.line.unwrap_or(0), d.col.unwrap_or(0)) == (p.pos.line, p.pos.col)) {
                    continue;
                }
                let (a, b, name) = (first.written(), p.written(), &tg.name);
                twice.push(
                    diag::error(
                        "E003",
                        p.pos.line,
                        p.pos.col,
                        tr!(
                            "ターゲット `{name}` で、環境変数 `{v}` を二回設定しています（{a} と {b}）",
                            "the variable `{v}` is set twice for `{name}`: by {a} and by {b}",
                        ),
                    )
                    .note(first_on(first.pos.line)),
                );
            }
        }
        let mut fails = |pos: Pos, code: &'static str| match failing.iter_mut().find(|f| f.0 == pos && f.1 == code) {
            Some(f) => f.2.push(tg.name.clone()),
            None => failing.push((pos, code, vec![tg.name.clone()])),
        };
        let has_port = !tg.ports.is_empty() || !tg.serves.is_empty();
        // a driver is handed every pin in its first line, and a page's clock and
        // random numbers are geas's to keep (DESIGN §12)
        let keeps_time = !tg.drivers.is_empty() || (!tg.serves.is_empty() && pages.contains(&tg.name.as_str()));
        for p in &pins {
            match &p.kind {
                PinKind::Clock(_, None) | PinKind::Seed(_, None) if !keeps_time => fails(p.pos, "E011"),
                PinKind::Set(..) if !has_port => {
                    if let Some(at) = p.port_at {
                        fails(at, "E010");
                    }
                }
                _ => {}
            }
        }
        out.insert(tg.name.clone(), to_pins(&pins));
    }
    diags.extend(twice);
    for (pos, code, targets) in failing {
        let (en, ja) = targets_named(&targets);
        let commands = targets.iter().all(|n| raw.targets.iter().any(|t| t.name == *n && !t.runs.is_empty()));
        let d = if code == "E010" {
            let (what_en, what_ja) = if commands {
                ("runs a command, which has none", "コマンドを走らせるターゲットなので、ポートを持ちません")
            } else {
                ("has none: it is not a service", "サービスではないので、ポートを持ちません")
            };
            diag::error(
                "E010",
                pos.line,
                pos.col,
                tr!(
                    "`{{port}}` はターゲットのポート番号に置き換わりますが、{ja} は{what_ja}",
                    "`{{port}}` stands for the target's port, and {en} {what_en}",
                ),
            )
            .note(tr!("ポートを持つのはサービス（`serve` と `port` のあるターゲット）だけです", "only a service has a port: a target with `serve` and `port`"))
        } else if raw.pins.iter().chain(raw.targets.iter().flat_map(|t| t.pins.iter())).any(|p| p.pos == pos && matches!(p.kind, PinKind::Clock(..))) {
            diag::error(
                "E011",
                pos.line,
                pos.col,
                tr!(
                    "{ja} では、`clock` で時刻を固定できません。プログラムの時刻を外から設定する方法はないので、geas はプログラムが読む環境変数で時刻を渡します",
                    "`clock` cannot be kept for {en}: no switch sets the time of a program from outside, so geas passes it in a variable the program reads",
                ),
            )
            .note(tr!(
                "その環境変数の名前を、同じ行に書いてください（`clock \"2026-08-29T09:00:00+09:00\" env \"NOW\"`）",
                "name that variable on the same line: `clock \"2026-08-29T09:00:00+09:00\" env \"NOW\"`",
            ))
        } else {
            diag::error(
                "E011",
                pos.line,
                pos.col,
                tr!(
                    "{ja} では、`seed` で乱数のシードを固定できません。プログラムの乱数のシードを外から設定する方法はないので、geas はプログラムが読む環境変数でシードを渡します",
                    "`seed` cannot be kept for {en}: no switch seeds the random numbers of a program from outside, so geas passes the seed in a variable the program reads",
                ),
            )
            .note(tr!(
                "その環境変数の名前を、同じ行に書いてください（`seed 7 env \"SEED\"`）",
                "name that variable on the same line: `seed 7 env \"SEED\"`",
            ))
        };
        diags.push(d);
    }
    out
}

/// The model's form of the pins in effect.
fn to_pins(pins: &[RawPin]) -> Pins {
    let mut out = Pins::default();
    for p in pins {
        match &p.kind {
            PinKind::Clean => out.clean = true,
            PinKind::Pass(n) => out.pass.push(n.clone()),
            PinKind::Set(n, v) => out.env.push((n.clone(), v.clone())),
            PinKind::Tz(z) => out.tz = Some(z.clone()),
            PinKind::Locale(l) => out.locale = Some(l.clone()),
            PinKind::Clock(at, env) => out.clock = Some((at.clone(), env.clone())),
            PinKind::Seed(n, env) => out.seed = Some((*n, env.clone())),
        }
    }
    out
}

/// E002 (an unknown target), E003, E004, E006 and E007, over the whole file. The
/// model, when nothing the run needs is missing.
fn static_checks(raw: RawSpec, diags: &mut Vec<Diag>) -> Option<Spec> {
    let mut pins = pins_in_effect(&raw, diags);
    let mut first_target: HashMap<&str, Pos> = HashMap::new();
    let mut kinds: HashMap<&str, Option<TargetKind>> = HashMap::new();
    let mut targets = Vec::new();
    for tg in &raw.targets {
        if let Some(first) = first_target.get(tg.name.as_str()) {
            diags.push(
                diag::error(
                    "E003",
                    tg.name_pos.line,
                    tg.name_pos.col,
                    tr!(
                        "`{}` という名前のターゲットが二つあります",
                        "a second target named `{}`",
                        tg.name,
                    ),
                )
                .note(first_on(first.line)),
            );
        } else {
            first_target.insert(&tg.name, tg.name_pos);
        }
        let kind = target_kind(tg, pins.get(&tg.name), diags);
        kinds.entry(&tg.name).or_insert_with(|| kind.clone());
        if let Some(kind) = kind {
            let pins = pins.remove(&tg.name).unwrap_or_default();
            let serial = !tg.serials.is_empty();
            targets.push(Target { name: tg.name.clone(), kind, pos: tg.kw_pos, pins, serial });
        }
    }
    let names: Vec<&str> = raw.targets.iter().map(|t| t.name.as_str()).collect();

    let mut first_claim: HashMap<&str, Pos> = HashMap::new();
    let mut claims = Vec::new();
    let mut complete = true;
    for c in &raw.claims {
        if let Some(first) = first_claim.get(c.name.as_str()) {
            diags.push(
                diag::error(
                    "E003",
                    c.name_pos.line,
                    c.name_pos.col,
                    tr!(
                        "\"{}\" という名前の主張が二つあります",
                        "a second claim named \"{}\"",
                        c.name,
                    ),
                )
                .note(first_on(first.line)),
            );
        } else {
            first_claim.insert(&c.name, c.pos);
        }
        let mut steps = Vec::new();
        // the call of the last `when` and its line, for E007
        let mut last: Option<(Option<&Call>, usize)> = None;
        for s in &c.steps {
            match s {
                RawStep::When { target, target_pos, call, call_word, call_pos, pos, .. } => {
                    last = Some((call.as_ref(), pos.line));
                    match kinds.get(target.as_str()) {
                        None => {
                            let list = if names.is_empty() {
                                tr!("このファイルにはターゲットがありません", "this file declares no target")
                            } else {
                                tr!(
                                    "このファイルのターゲット: {}",
                                    "the targets in this file: {}",
                                    names.join("、");
                                    names.join(", "),
                                )
                            };
                            diags.push(
                                diag::error(
                                    "E002",
                                    target_pos.line,
                                    target_pos.col,
                                    tr!("ターゲット `{target}` はありません", "there is no target `{target}`"),
                                )
                                .note(list),
                            );
                        }
                        Some(kind) => {
                            if let (Some(kind), Some(call)) = (kind, call)
                                && let Some(d) = call_fits(target, kind, call, call_word, *call_pos)
                            {
                                diags.push(d);
                            }
                        }
                    }
                    match call {
                        Some(call) => steps.push(Step::When { target: target.clone(), call: call.clone(), pos: *pos }),
                        None => complete = false,
                    }
                }
                RawStep::Then { check } => {
                    let Some(check) = check else {
                        complete = false;
                        continue;
                    };
                    if let Some((Some(call), when_line)) = last
                        && check.subject.observed_by() != call.observes()
                    {
                        diags.push(not_observed(check, call, when_line));
                    }
                    steps.push(Step::Then(check.clone()));
                }
            }
        }
        claims.push(Claim { name: c.name.clone(), steps, pos: c.pos });
    }
    gui_checks(&raw.claims, &kinds, diags);
    if !complete || targets.len() != raw.targets.len() {
        return None;
    }
    Some(Spec { targets, claims, masks: raw.masks })
}

/// E006: a call its target does not take.
fn call_fits(target: &str, kind: &TargetKind, call: &Call, word: &str, at: Pos) -> Option<Diag> {
    let gui = call.observes() == Observes::Screen;
    let (en, ja) = match kind {
        TargetKind::Run(_) if matches!(call, Call::Run(_)) => return None,
        TargetKind::Serve { .. } if !matches!(call, Call::Run(_)) => return None,
        TargetKind::Pixie(_) | TargetKind::Driver(_) if gui => return None,
        TargetKind::Run(_) => (
            format!("`{target}` runs a command, so it takes `run`, not `{word}`"),
            format!("`{target}` はコマンドを走らせるターゲットなので、呼べるのは `run` で、`{word}` は呼べません"),
        ),
        TargetKind::Serve { .. } => (
            format!("`{target}` is a service, so it takes `get` and `post`, and the actions on its page in a browser (open, click, input, submit, press, advance), not `{word}`"),
            format!("`{target}` はサービスのターゲットなので、呼べるのは `get` と `post`、ブラウザーで開いたページへの操作（open、click、input、submit、press、advance）で、`{word}` は呼べません"),
        ),
        TargetKind::Pixie(_) => (
            format!("`{target}` is a pixie app, so it takes the actions open, click, input, submit, press and advance, not `{word}`"),
            format!("`{target}` は pixie のアプリのターゲットなので、呼べるのは操作（open、click、input、submit、press、advance）で、`{word}` は呼べません"),
        ),
        TargetKind::Driver(_) => (
            format!("`{target}` is a GUI reached through a driver, so it takes the actions open, click, input, submit, press and advance, not `{word}`"),
            format!("`{target}` はドライバーで動かす GUI のターゲットなので、呼べるのは操作（open、click、input、submit、press、advance）で、`{word}` は呼べません"),
        ),
    };
    Some(diag::error("E006", at.line, at.col, Text::new(ja, en)))
}

/// E007: a check of something its `when` does not observe.
fn not_observed(check: &Check, call: &Call, when_line: usize) -> Diag {
    let s = check.subject.word();
    let word = call.word();
    let (gives_en, gives_ja) = match call.observes() {
        Observes::Process => ("stdout, stderr and exit", "stdout、stderr、exit"),
        Observes::Http => ("status, header, body and body json", "status、header、body、body json"),
        Observes::Screen => ("the screen", "画面"),
    };
    diag::error(
        "E007",
        check.pos.line,
        check.pos.col,
        tr!(
            "`{word}` の結果にあるのは {gives_ja} で、`{s}` はありません",
            "a `{word}` observes {gives_en}, not `{s}`",
        ),
    )
    .note(tr!(
        "このチェックが見るのは {when_line} 行目の `when` です",
        "the `when` it reads is on line {when_line}",
    ))
}

/// The keys `press` sends to a page in Chrome (DESIGN §8.4), and the chords made
/// of them with `cmd`, `ctrl`, `alt` and `shift`: a letter, a digit, or one of
/// these names.
pub const KEY_NAMES: &[&str] = &[
    "enter", "escape", "tab", "backspace", "delete", "space", "up", "down", "left", "right", "home", "end", "pageup", "pagedown",
    "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12",
];

/// Whether a page in Chrome can be sent a key or a chord: `enter`, `a`, `cmd-s`,
/// `shift-tab` (`+` reads as `-`).
pub fn known_key(chord: &str) -> bool {
    let parts: Vec<&str> = chord.split(['-', '+']).collect();
    let Some((key, mods)) = parts.split_last() else {
        return false;
    };
    let key_ok = KEY_NAMES.contains(key) || (key.chars().count() == 1 && key.chars().all(|c| c.is_ascii_alphanumeric()));
    key_ok && mods.iter().all(|m| ["cmd", "ctrl", "alt", "shift"].contains(m))
}

/// A `when` of a claim, as the GUI checks read it.
struct WhenAt<'a> {
    target: &'a str,
    call: &'a Call,
    pos: Pos,
    call_pos: Pos,
    args_at: &'a [(&'static str, Pos)],
}

impl WhenAt<'_> {
    /// Where an argument is written, else where the call is.
    fn arg(&self, key: &str) -> Pos {
        self.args_at.iter().find(|(k, _)| *k == key).map_or(self.call_pos, |(_, p)| *p)
    }
}

/// E012 and E013, claim by claim (DESIGN §8.2, §13): another target's `when`
/// between two actions on a pixie app, which runs a claim's actions as one script;
/// and an argument an action cannot take there.
fn gui_checks(claims: &[RawClaim], kinds: &HashMap<&str, Option<TargetKind>>, diags: &mut Vec<Diag>) {
    for c in claims {
        let whens: Vec<WhenAt> = c
            .steps
            .iter()
            .filter_map(|s| match s {
                RawStep::When { target, call: Some(call), pos, call_pos, args_at, .. } => {
                    Some(WhenAt { target, call, pos: *pos, call_pos: *call_pos, args_at })
                }
                _ => None,
            })
            .collect();
        let kind = |w: &WhenAt| kinds.get(w.target).cloned().flatten();
        // E012: between the first and the last action on a pixie app, every `when` is on it
        let mut pixies: Vec<&str> = whens.iter().filter(|w| matches!(kind(w), Some(TargetKind::Pixie(_)))).map(|w| w.target).collect();
        pixies.dedup();
        let mut seen = std::collections::HashSet::new();
        for app in pixies {
            if !seen.insert(app) {
                continue;
            }
            let on: Vec<usize> = whens.iter().enumerate().filter(|(_, w)| w.target == app).map(|(i, _)| i).collect();
            let (first, last) = (on[0], on[on.len() - 1]);
            for (i, w) in whens.iter().enumerate().take(last).skip(first + 1) {
                if w.target == app {
                    continue;
                }
                let before = on.iter().rev().find(|k| **k < i).map(|k| whens[*k].pos.line).unwrap_or(0);
                let after = on.iter().find(|k| **k > i).map(|k| whens[*k].pos.line).unwrap_or(0);
                let other = w.target;
                diags.push(
                    diag::error(
                        "E012",
                        w.pos.line,
                        w.pos.col,
                        tr!(
                            "pixie のアプリ `{app}` に対する二つの操作のあいだに、`{other}` の `when` があります。pixie は主張の中のアプリへの操作を一つのスクリプトとしてまとめて実行するので、そのあいだアプリは動いていません",
                            "a `when` on `{other}` comes between two actions on `{app}`, a pixie app; pixie runs a claim's actions on an app as one script, so the app is not running between them",
                        ),
                    )
                    .note(tr!(
                        "その前後の `{app}` への操作は、{before} 行目と {after} 行目です",
                        "the actions on `{app}` before and after it are on lines {before} and {after}",
                    ))
                    .note(tr!(
                        "この `when` を、`{app}` への最初の操作の前か、最後の操作のあとに移してください",
                        "move this `when` before the first action on `{app}` or after the last",
                    )),
                );
            }
        }
        // E013
        let mut opened: Vec<&str> = Vec::new();
        let mut acted: Vec<&str> = Vec::new();
        for w in &whens {
            let Some(k) = kind(w) else {
                continue;
            };
            let (tg, call) = (w.target, w.call);
            let mut e013 = |pos: Pos, en: String, ja: String| diags.push(diag::error("E013", pos.line, pos.col, Text::new(ja, en)));
            // counts from 1, on any target
            let zero = match call {
                Call::Click { nth: Some(0), .. } => Some("nth"),
                Call::Input { place, .. } | Call::Submit(place) => match place {
                    Place::Field(0) => Some("field"),
                    Place::Into { nth: Some(0), .. } => Some("nth"),
                    _ => None,
                },
                _ => None,
            };
            if let Some(key) = zero {
                e013(
                    w.arg(key),
                    format!("`{key}:` counts from 1: the first is `{key}: 1`"),
                    format!("`{key}:` は 1 から数えます。最初のものは `{key}: 1` です"),
                );
            }
            if let Call::Advance(0) = call {
                e013(
                    w.arg("ms"),
                    "`advance` moves the app's clock forward by at least 1 ms".into(),
                    "`advance` で進める時刻は、1 ミリ秒以上にしてください".into(),
                );
            }
            match k {
                TargetKind::Pixie(_) => {
                    if let Call::Input { place: Place::Into { .. }, .. } | Call::Submit(Place::Into { .. }) = call {
                        e013(
                            w.arg("into"),
                            format!("`{tg}` is a pixie app, which reaches a text field by its place among the fields only; write `field: n`"),
                            format!("`{tg}` は pixie のアプリで、テキストフィールドは何番目かでしか選べません。`field: n` と書いてください"),
                        );
                    }
                    match call {
                        Call::Open(Some(_)) => e013(
                            w.arg("path"),
                            format!("`{tg}` is a pixie app, which has no paths: `open()` is its first screen"),
                            format!("`{tg}` は pixie のアプリなので、パスはありません。`open()` がアプリの最初の画面です"),
                        ),
                        Call::Open(None) if acted.contains(&tg) => e013(
                            w.call_pos,
                            format!("`{tg}` is a pixie app, which starts once in a claim: `open()` is its first screen, and comes before the other actions on it"),
                            format!("`{tg}` は pixie のアプリで、主張の中で一度だけ起動します。`open()` はアプリの最初の画面なので、ほかの操作より前に書いてください"),
                        ),
                        _ => {}
                    }
                    acted.push(tg);
                }
                TargetKind::Serve { .. } => match call {
                    Call::Open(Some(p)) if !p.starts_with('/') => e013(
                        w.arg("path"),
                        format!("a page's path starts with `/`: `open(\"/{p}\")`"),
                        format!("ページのパスは、`/` から始めてください（`open(\"/{p}\")`）"),
                    ),
                    Call::Open(_) => opened.push(tg),
                    Call::Get(_) | Call::Post { .. } | Call::Run(_) => {}
                    _ if !opened.contains(&tg) => e013(
                        w.call_pos,
                        format!("the page of `{tg}` is not open in this claim yet: open it first, `when {tg}.open(\"/\")`"),
                        format!("この主張では、`{tg}` のページをまだ開いていません。先に `when {tg}.open(\"/\")` で開いてください"),
                    ),
                    Call::Press(k) if !known_key(k) => {
                        let named = &KEY_NAMES[..KEY_NAMES.len() - 12];
                        e013(
                            w.arg("key"),
                            format!("`{k}` is not a key geas sends to a page; it sends a letter, a digit, {}, f1 to f12, and chords of them with cmd, ctrl, alt and shift, such as `cmd-s`", named.join(", ")),
                            format!("`{k}` は、geas がページに送れるキーではありません。送れるのは、英字、数字、{}、f1〜f12 と、それに cmd・ctrl・alt・shift を組み合わせたもの（`cmd-s` など）です", named.join("・")),
                        )
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CALC: &str = "target calc {\n  run \"python3 calc.py\"\n}\n\n\
                        claim \"adds\" {\n  when calc.run(\"2\", \"+\", \"3\")\n  then stdout is \"5\"\n  and  exit is 0\n}\n";

    fn errs(src: &str) -> Vec<(&'static str, usize, usize)> {
        parse(src).unwrap_err().iter().map(|d| (d.code, d.line.unwrap_or(0), d.col.unwrap_or(0))).collect()
    }

    #[test]
    fn a_process_target_and_a_claim() {
        let spec = parse(CALC).unwrap();
        assert_eq!(spec.targets.len(), 1);
        assert!(matches!(&spec.targets[0].kind, TargetKind::Run(w) if w == &["python3", "calc.py"]));
        let claim = &spec.claims[0];
        assert_eq!((claim.name.as_str(), claim.pos.line), ("adds", 5));
        assert_eq!(claim.steps.len(), 3);
        match &claim.steps[0] {
            Step::When { target, call: Call::Run(args), pos } => {
                assert_eq!((target.as_str(), pos.line, pos.col), ("calc", 6, 3));
                assert_eq!(args, &["2", "+", "3"]);
            }
            other => panic!("{other:?}"),
        }
        match &claim.steps[2] {
            Step::Then(c) => {
                assert!(matches!(c.subject, Subject::Exit));
                assert!(matches!(c.matcher, Matcher::Is(Value::N(n)) if n == 0.0));
                assert_eq!((c.pos.line, c.pos.col), (8, 8));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_service_target_masks_and_http_calls() {
        let src = "target api {\n  serve \"python3 server.py 8123\"\n  port 8123\n}\n\
                   mask header \"Date\"\nmask body json \".id\"\n\
                   claim \"adds\" {\n  when api.post(\"/add\", body: \"5\")\n  when api.get(\"/total\")\n  then body json \".total\" is 5\n}\n";
        let spec = parse(src).unwrap();
        assert!(matches!(&spec.targets[0].kind, TargetKind::Serve { words, port: Port::Fixed(8123) } if words == &["python3", "server.py", "8123"]));
        assert!(matches!(&spec.masks[0], Mask::Header(h) if h == "date"));
        assert!(matches!(&spec.masks[1], Mask::BodyJson(p) if p == ".id"));
        let steps = &spec.claims[0].steps;
        assert!(matches!(&steps[0], Step::When { call: Call::Post { path, body: Some(b) }, .. } if path == "/add" && b == "5"));
        assert!(matches!(&steps[1], Step::When { call: Call::Get(p), .. } if p == "/total"));
        assert!(matches!(&steps[2], Step::Then(Check { subject: Subject::BodyJson(p), .. }) if p == ".total"));
    }

    #[test]
    fn the_first_syntax_error_stops_the_parse() {
        // the unknown subject on line 12 is reported; the stray `)` on line 13 stops the
        // parse, so the unknown target on line 13 is never looked for
        let src = format!("{CALC}claim \"b\" {{\n  when calc.run(\"1\")\n  then output is \"1\"\n  when api.get(\"/\"))\n}}\n");
        assert_eq!(errs(&src), vec![("E002", 12, 8), ("E001", 13, 20)]);
    }

    #[test]
    fn every_static_error_is_reported_in_line_order() {
        let src = "claim \"a\" {\n  when api.get(\"/\")\n  then stdout is \"x\"\n}\n\
                   target calc {\n  run \"x\"\n  port 8123\n}\n\
                   claim \"a\" {\n  when calc.get(\"/\")\n  then exit contains \"1\"\n}\n\
                   claim \"c\" {\n  when calc.run()\n  then status is 200\n}\n";
        assert_eq!(
            errs(src),
            vec![
                ("E002", 2, 8),
                ("E007", 3, 8),
                ("E004", 7, 3),
                ("E003", 9, 7),
                ("E006", 10, 13),
                ("E007", 11, 8),
                ("E008", 11, 13),
                ("E007", 15, 8),
            ]
        );
    }

    #[test]
    fn steps_and_targets() {
        assert_eq!(errs("claim \"x\" {\n}\n"), vec![("E005", 1, 1)]);
        assert_eq!(
            errs("target calc {\n  run \"x\"\n}\nclaim \"x\" {\n  then stdout is \"5\"\n  when calc.run()\n}\n"),
            vec![("E005", 5, 3)]
        );
        assert_eq!(errs("target calc {\n}\n"), vec![("E004", 1, 1)]);
        assert_eq!(errs("target calc {\n  run \"a\"\n  run \"b\"\n}\n"), vec![("E004", 3, 3)]);
        assert_eq!(errs("target t { exec \"x\" }\n"), vec![("E002", 1, 12)]);
        assert_eq!(errs("target t {\n  serve \"x\"\n  prot 8123\n}\n"), vec![("E002", 3, 3)]);
    }

    #[test]
    fn commands_are_words_once_the_spec_is_read() {
        let src = "target calc {\n  run \"python3 'my calc.py' -v\"\n}\nclaim \"x\" {\n  when calc.run(\"a b\")\n}\n";
        let spec = parse(src).unwrap();
        assert!(matches!(&spec.targets[0].kind, TargetKind::Run(w) if w == &["python3", "my calc.py", "-v"]));
        // E010 points inside the string: the open quote, the trailing backslash,
        // `{port}` on a command; an empty command at the string
        let one = |cmd: &str| errs(&format!("target t {{\n  run \"{cmd}\"\n}}\n"));
        assert_eq!(one("python3 'my calc.py"), vec![("E010", 2, 16)]);
        assert_eq!(one("echo \\\"hi"), vec![("E010", 2, 13)]);
        assert_eq!(one("echo hi\\\\"), vec![("E010", 2, 15)]);
        assert_eq!(one("x --port={port}"), vec![("E010", 2, 17)]);
        assert_eq!(one(" "), vec![("E010", 2, 7)]);
        // a service replaces `{port}` when it starts
        let spec = parse("target api {\n  serve \"srv --port={port}\"\n  port 8123\n}\n").unwrap();
        assert!(matches!(&spec.targets[0].kind, TargetKind::Serve { words, .. } if words == &["srv", "--port={port}"]));
    }

    /// One check under a `get`, and what the parser makes of it, or its errors.
    fn one_check(check: &str) -> Result<Check, Vec<(&'static str, usize, usize)>> {
        let src = format!("target api {{\n  serve \"x\"\n  port 8123\n}}\nclaim \"c\" {{\n  when api.get(\"/\")\n  then {check}\n}}\n");
        match parse(&src) {
            Ok(spec) => match &spec.claims[0].steps[1] {
                Step::Then(c) => Ok(c.clone()),
                other => panic!("{other:?}"),
            },
            Err(ds) => Err(ds.iter().map(|d| (d.code, d.line.unwrap_or(0), d.col.unwrap_or(0))).collect()),
        }
    }

    #[test]
    fn every_matcher_form() {
        let words = |check: &str| {
            let c = one_check(check).unwrap_or_else(|e| panic!("{check}: {e:?}"));
            format!("{} {} {}", c.subject.label(), c.matcher.words(), c.matcher.value()).trim_end().to_string()
        };
        assert_eq!(words("status is not 500"), "status is not 500");
        assert_eq!(words("status is above 199"), "status is above 199");
        assert_eq!(words("status is below 300"), "status is below 300");
        assert_eq!(words("status is at least 200"), "status is at least 200");
        assert_eq!(words("status is at most 299"), "status is at most 299");
        assert_eq!(words("status is between 200 and 299"), "status is between 200 and 299");
        assert_eq!(words("header \"Content-Type\" contains \"json\""), "header \"content-type\" contains \"json\"");
        assert_eq!(words("body does not contain \"error\""), "body does not contain \"error\"");
        assert_eq!(words("body json \".id\" matches \"\\d+\""), "body json \".id\" matches \"\\d+\"");
        assert_eq!(words("body does not match \"x*\""), "body does not match \"x*\"");
        assert_eq!(words("header \"etag\" exists"), "header \"etag\" exists");
        assert_eq!(words("body json \".error\" does not exist"), "body json \".error\" does not exist");
        assert_eq!(words("body json \".ok\" is true"), "body json \".ok\" is true");
        assert_eq!(words("body json \".x\" is null"), "body json \".x\" is null");
    }

    #[test]
    fn matchers_that_do_not_fit() {
        // E008 at the word, or at the value, that does not fit
        assert_eq!(one_check("status matches \"2..\"").unwrap_err(), vec![("E008", 7, 15)]);
        assert_eq!(one_check("status is \"200\"").unwrap_err(), vec![("E008", 7, 18)]);
        assert_eq!(one_check("body is true").unwrap_err(), vec![("E008", 7, 16)]);
        assert_eq!(one_check("status exists").unwrap_err(), vec![("E008", 7, 15)]);
        assert_eq!(one_check("status is above \"5\"").unwrap_err(), vec![("E008", 7, 24)]);
        assert_eq!(one_check("status is between 5 and 1").unwrap_err(), vec![("E008", 7, 26)]);
        assert_eq!(one_check("body contains 5").unwrap_err(), vec![("E008", 7, 22)]);
        assert_eq!(one_check("body matches 5").unwrap_err(), vec![("E008", 7, 21)]);
        // E009 inside the pattern or the path
        assert_eq!(one_check("body matches \"a(b\"").unwrap_err(), vec![("E009", 7, 23)]);
        assert_eq!(one_check("body json \".a[x]\" is 1").unwrap_err(), vec![("E009", 7, 21)]);
        // a backslash before a letter is a pattern's, and E001 anywhere else
        assert!(one_check("body matches \"\\d+\"").is_ok());
        assert_eq!(one_check("body is \"\\d\"").unwrap_err(), vec![("E001", 7, 17)]);
        // the grammar's own
        assert_eq!(one_check("status is at 5").unwrap_err(), vec![("E001", 7, 21)]);
        assert_eq!(one_check("status is between 1 or 5").unwrap_err(), vec![("E001", 7, 28)]);
        assert_eq!(one_check("body does contain \"x\"").unwrap_err(), vec![("E001", 7, 18)]);
        assert_eq!(one_check("body does not equal \"x\"").unwrap_err(), vec![("E002", 7, 22)]);
        assert_eq!(one_check("body equals \"x\"").unwrap_err(), vec![("E002", 7, 13)]);
    }

    #[test]
    fn pins_in_effect_on_each_target() {
        let src = "env \"A\" \"top\"\nenv \"B\" \"top\"\ntz \"UTC\"\nenv clean\n\
                   target t {\n  run \"x\"\n  env \"A\" \"own\"\n  tz \"Asia/Tokyo\"\n  env pass \"HOME\"\n}\n\
                   target u {\n  serve \"y {port}\"\n  port 8123\n  clock \"2026-08-29T00:00:00Z\" env \"NOW\"\n  env \"URL\" \"http://127.0.0.1:{port}\"\n}\n";
        let spec = parse(src).unwrap();
        let t = &spec.targets[0].pins;
        assert_eq!(t.env, vec![("A".to_string(), "own".to_string()), ("B".to_string(), "top".to_string())]);
        assert_eq!((t.tz.as_deref(), t.clean, t.pass.as_slice()), (Some("Asia/Tokyo"), true, &["HOME".to_string()][..]));
        assert_eq!(t.json(), "{\"clean\":true,\"pass\":[\"HOME\"],\"env\":{\"A\":\"own\",\"B\":\"top\"},\"tz\":\"Asia/Tokyo\"}");
        let u = &spec.targets[1].pins;
        assert_eq!(u.clock, Some(("2026-08-29T00:00:00Z".to_string(), Some("NOW".to_string()))));
        assert_eq!(u.vars().last().map(|(k, v)| (k.as_str(), v.as_str())), Some(("NOW", "2026-08-29T00:00:00Z")));
        // `env` after `clock` belongs to it only on the same line
        let src = "target t {\n  run \"x\"\n  clock \"2026-08-29T00:00:00Z\"\n  env \"NOW\" \"1\"\n}\n";
        assert_eq!(errs(src), vec![("E011", 3, 3)]);
    }

    #[test]
    fn gui_targets_their_calls_and_screen_checks() {
        let src = "target app {\n  pixie \"build/greeter\"\n}\n\
                   target d {\n  driver \"python3 d.py\"\n  clock \"2026-08-29T00:00:00Z\"\n}\n\
                   target web {\n  serve \"python3 s.py {port}\"\n  port auto\n  seed 7\n}\n\
                   mask screen text matching \"\\d+:\\d+\" in status\n\
                   claim \"a\" {\n  when app.open()\n  when app.input(\"Ada\", field: 2)\n  when app.click(\"greet\", nth: 2)\n  when app.submit()\n  when app.press(\"cmd-s\")\n  when app.advance(500)\n  then screen contains exactly 2 button containing \"gr\" with value \"x\" disabled in dialog \"Confirm\"\n  and screen does not contain text \"oops\"\n}\n\
                   claim \"b\" {\n  when web.open(\"/\")\n  when web.input(\"milk\", into: \"Item\", nth: 1)\n  when web.submit(into: \"Item\")\n  when web.get(\"/api\")\n  then status is 200\n  when d.click(\"x\")\n}\n";
        let spec = parse(src).unwrap_or_else(|d| panic!("{d:?}"));
        assert!(matches!(&spec.targets[0].kind, TargetKind::Pixie(w) if w == &["build/greeter"]));
        assert!(matches!(&spec.targets[1].kind, TargetKind::Driver(w) if w == &["python3", "d.py"]));
        let calls = |c: usize| -> Vec<String> {
            spec.claims[c]
                .steps
                .iter()
                .filter_map(|s| match s {
                    Step::When { call, .. } => Some(call.display()),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(
            calls(0),
            ["open()", "input(\"Ada\", field: 2)", "click(\"greet\", nth: 2)", "submit()", "press(\"cmd-s\")", "advance(500)"]
        );
        assert_eq!(calls(1), ["open(\"/\")", "input(\"milk\", into: \"Item\", nth: 1)", "submit(into: \"Item\")", "get(\"/api\")", "click(\"x\")"]);
        let check = |c: usize, k: usize| match &spec.claims[c].steps[k] {
            Step::Then(ch) => format!("{} {} {}", ch.subject.label(), ch.matcher.words(), ch.matcher.value()),
            other => panic!("{other:?}"),
        };
        assert_eq!(check(0, 6), "screen contains exactly 2 button containing \"gr\" with value \"x\" disabled in dialog \"Confirm\"");
        assert_eq!(check(0, 7), "screen does not contain text \"oops\"");
        assert!(matches!(&spec.masks[0], Mask::Screen(p) if p.shown() == "text matching \"\\d+:\\d+\" in status"));
        // a driver takes `clock` without a variable in its pins line, and a page
        // keeps it itself; a service no claim opens in a browser does not
        let api = "target api {\n  serve \"python3 s.py {port}\"\n  port auto\n  clock \"2026-08-29T00:00:00Z\"\n}\nclaim \"x\" {\n  when api.get(\"/\")\n}\n";
        assert_eq!(errs(api), vec![("E011", 4, 3)]);
        assert!(parse(&api.replace("api.get(\"/\")", "api.open(\"/\")")).is_ok());
    }

    #[test]
    fn a_claim_on_a_pixie_app_keeps_its_actions_together() {
        let src = "target app {\n  pixie \"greeter\"\n}\ntarget calc {\n  run \"x\"\n}\n\
                   claim \"ok before and after\" {\n  when calc.run()\n  when app.input(\"a\")\n  when app.click(\"b\")\n  when calc.run()\n}\n\
                   claim \"between\" {\n  when app.input(\"a\")\n  when calc.run()\n  when app.click(\"b\")\n}\n";
        assert_eq!(errs(src), vec![("E012", 15, 3)]);
    }

    #[test]
    fn messages_in_both_languages() {
        let d = &parse("target calc {\n  run \"x\"\n}\nclaim \"x\" {\n  when api.get(\"/\")\n}\n").unwrap_err()[0];
        assert_eq!(d.message.en, "there is no target `api`");
        assert_eq!(d.message.ja, "ターゲット `api` はありません");
        assert_eq!(d.notes[0].ja, "このファイルのターゲット: calc");
        let d = &parse("target {\n}\n").unwrap_err()[0];
        assert_eq!(d.message.en, "expected the target's name, found `{`");
        assert_eq!(d.message.ja, "ターゲットの名前を書くところに `{` があります");
        let d = &parse("claim \"x\"").unwrap_err()[0];
        assert_eq!(d.message.ja, "`{` を書く前にファイルが終わっています");
    }
}
