use crate::lex::{self, Kind, Tok};
use crate::model::*;

struct P {
    toks: Vec<Tok>,
    pos: usize,
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
    fn err(&self, tok: &Tok, msg: &str) -> String {
        format!("{}:{}: {}", tok.line, tok.col, msg)
    }
    fn expect(&mut self, kind: Kind, what: &str) -> Result<Tok, String> {
        let t = self.next();
        if t.kind == kind {
            Ok(t)
        } else {
            Err(format!(
                "{}:{}: expected {}, found `{}`",
                t.line,
                t.col,
                what,
                if t.kind == Kind::Eof { "<eof>" } else { &t.text }
            ))
        }
    }
    fn ident(&mut self) -> Result<Tok, String> {
        self.expect(Kind::Ident, "an identifier")
    }
    fn string(&mut self) -> Result<Tok, String> {
        self.expect(Kind::Str, "a string")
    }

    fn target(&mut self) -> Result<Target, String> {
        let kw = self.next(); // `target`
        let name = self.ident()?;
        self.expect(Kind::LBrace, "`{`")?;
        let mut run: Option<String> = None;
        let mut serve: Option<String> = None;
        let mut port: Option<u16> = None;
        loop {
            let t = self.next();
            match t.kind {
                Kind::RBrace => break,
                Kind::Ident => match t.text.as_str() {
                    "run" => run = Some(self.string()?.text),
                    "serve" => serve = Some(self.string()?.text),
                    "port" => {
                        let n = self.expect(Kind::Num, "a port number")?;
                        match n.text.parse::<u16>() {
                            Ok(p) => port = Some(p),
                            Err(_) => return Err(self.err(&n, "invalid port")),
                        }
                    }
                    _ => return Err(self.err(&t, "expected `run`, `serve` or `port`")),
                },
                _ => return Err(self.err(&t, "expected a target property or `}`")),
            }
        }
        let kind = match (run, serve) {
            (Some(r), None) => TargetKind::Run(r),
            (None, Some(s)) => {
                let Some(p) = port else {
                    return Err(format!(
                        "{}:{}: target `{}` uses `serve` and needs a `port`",
                        kw.line, kw.col, name.text
                    ));
                };
                TargetKind::Serve { cmd: s, port: p }
            }
            (Some(_), Some(_)) => {
                return Err(format!(
                    "{}:{}: target `{}` declares both `run` and `serve`",
                    kw.line, kw.col, name.text
                ));
            }
            (None, None) => {
                return Err(format!(
                    "{}:{}: target `{}` needs `run` or `serve`",
                    kw.line, kw.col, name.text
                ));
            }
        };
        Ok(Target { name: name.text, kind, line: kw.line })
    }

    fn claim(&mut self) -> Result<Claim, String> {
        let kw = self.next(); // `claim`
        let name = self.string()?;
        self.expect(Kind::LBrace, "`{`")?;
        let mut steps: Vec<Step> = Vec::new();
        loop {
            let t = self.next();
            match t.kind {
                Kind::RBrace => break,
                Kind::Ident if t.text == "when" => {
                    let step = self.when(t.line)?;
                    steps.push(step);
                }
                Kind::Ident if t.text == "then" || t.text == "and" => {
                    if steps.is_empty() {
                        return Err(self.err(&t, "`then` before any `when`"));
                    }
                    let check = self.check()?;
                    steps.push(Step::Then(check));
                }
                _ => return Err(self.err(&t, "expected `when`, `then`, `and` or `}`")),
            }
        }
        if steps.is_empty() {
            return Err(format!("{}:{}: claim has no steps", kw.line, kw.col));
        }
        Ok(Claim { name: name.text, steps, line: kw.line })
    }

    fn when(&mut self, line: usize) -> Result<Step, String> {
        let target = self.ident()?;
        self.expect(Kind::Dot, "`.`")?;
        let method = self.ident()?;
        self.expect(Kind::LParen, "`(`")?;
        let call = match method.text.as_str() {
            "run" => {
                let mut args = Vec::new();
                if self.peek().kind != Kind::RParen {
                    loop {
                        args.push(self.string()?.text);
                        if self.peek().kind == Kind::Comma {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                Call::Run(args)
            }
            "get" => Call::Get(self.string()?.text),
            "post" => {
                let path = self.string()?.text;
                let mut body = None;
                if self.peek().kind == Kind::Comma {
                    self.next();
                    let k = self.ident()?;
                    if k.text != "body" {
                        return Err(self.err(&k, "expected `body:`"));
                    }
                    self.expect(Kind::Colon, "`:`")?;
                    body = Some(self.string()?.text);
                }
                Call::Post { path, body }
            }
            _ => return Err(self.err(&method, "unknown method (expected run, get or post)")),
        };
        self.expect(Kind::RParen, "`)`")?;
        Ok(Step::When { target: target.text, call, line })
    }

    fn mask(&mut self) -> Result<Mask, String> {
        self.next(); // `mask`
        let what = self.ident()?;
        match what.text.as_str() {
            "header" => Ok(Mask::Header(self.string()?.text.to_lowercase())),
            "body" => {
                let j = self.ident()?;
                if j.text != "json" {
                    return Err(self.err(&j, "expected `mask body json \"<path>\"`"));
                }
                Ok(Mask::BodyJson(self.string()?.text))
            }
            _ => Err(self.err(&what, "expected `mask header \"...\"` or `mask body json \"...\"`")),
        }
    }

    fn check(&mut self) -> Result<Check, String> {
        let subj = self.ident()?;
        let subject = match subj.text.as_str() {
            "stdout" => Subject::Stdout,
            "stderr" => Subject::Stderr,
            "exit" => Subject::Exit,
            "status" => Subject::Status,
            "body" => {
                if self.peek().kind == Kind::Ident && self.peek().text == "json" {
                    self.next();
                    Subject::BodyJson(self.string()?.text)
                } else {
                    Subject::Body
                }
            }
            _ => {
                return Err(self.err(
                    &subj,
                    "unknown subject (expected stdout, stderr, exit, status or body)",
                ));
            }
        };
        let m = self.ident()?;
        let matcher = match m.text.as_str() {
            "is" => Matcher::Is,
            "contains" => Matcher::Contains,
            _ => return Err(self.err(&m, "expected `is` or `contains`")),
        };
        let v = self.next();
        let expected = match v.kind {
            Kind::Str => Expected::S(v.text.clone()),
            Kind::Num => match v.text.parse::<f64>() {
                Ok(n) => Expected::N(n),
                Err(_) => return Err(self.err(&v, "invalid number")),
            },
            _ => return Err(self.err(&v, "expected a string or a number")),
        };
        let numeric_subject = matches!(subject, Subject::Exit | Subject::Status);
        if numeric_subject && matcher == Matcher::Contains {
            return Err(self.err(&m, "`contains` is not valid on a numeric subject"));
        }
        if numeric_subject && matches!(expected, Expected::S(_)) {
            return Err(self.err(&v, "this subject compares against a number"));
        }
        if matcher == Matcher::Contains && matches!(expected, Expected::N(_)) {
            return Err(self.err(&v, "`contains` takes a string"));
        }
        Ok(Check { subject, matcher, expected, line: subj.line })
    }
}

pub fn parse(src: &str) -> Result<Spec, String> {
    let toks = lex::lex(src)?;
    let mut p = P { toks, pos: 0 };
    let mut targets = Vec::new();
    let mut claims = Vec::new();
    let mut masks = Vec::new();
    loop {
        let t = p.peek().clone();
        match t.kind {
            Kind::Eof => break,
            Kind::Ident if t.text == "target" => targets.push(p.target()?),
            Kind::Ident if t.text == "claim" => claims.push(p.claim()?),
            Kind::Ident if t.text == "mask" => masks.push(p.mask()?),
            _ => {
                return Err(p.err(&t, "expected `target`, `claim` or `mask` at top level"));
            }
        }
    }
    let spec = Spec { targets, claims, masks };
    validate(&spec)?;
    Ok(spec)
}

fn validate(spec: &Spec) -> Result<(), String> {
    for claim in &spec.claims {
        for step in &claim.steps {
            if let Step::When { target, call, line } = step {
                let Some(t) = spec.target(target) else {
                    return Err(format!("{}: unknown target `{}`", line, target));
                };
                let ok = match (&t.kind, call) {
                    (TargetKind::Run(_), Call::Run(_)) => true,
                    (TargetKind::Serve { .. }, Call::Get(_) | Call::Post { .. }) => true,
                    _ => false,
                };
                if !ok {
                    return Err(format!(
                        "{}: target `{}` does not support this method (run targets take .run, serve targets take .get/.post)",
                        line, target
                    ));
                }
            }
        }
    }
    Ok(())
}
