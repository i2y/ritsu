#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Ident,
    Str,
    Num,
    LBrace,
    RBrace,
    LParen,
    RParen,
    Comma,
    Dot,
    Colon,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Tok {
    pub kind: Kind,
    pub text: String,
    pub line: usize,
    pub col: usize,
}

pub fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let b: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut col = 1usize;
    while i < b.len() {
        let c = b[i];
        let (l, co) = (line, col);
        match c {
            '\n' => {
                i += 1;
                line += 1;
                col = 1;
            }
            ' ' | '\t' | '\r' => {
                i += 1;
                col += 1;
            }
            '#' => {
                while i < b.len() && b[i] != '\n' {
                    i += 1;
                }
            }
            '{' | '}' | '(' | ')' | ',' | '.' | ':' => {
                let kind = match c {
                    '{' => Kind::LBrace,
                    '}' => Kind::RBrace,
                    '(' => Kind::LParen,
                    ')' => Kind::RParen,
                    ',' => Kind::Comma,
                    '.' => Kind::Dot,
                    _ => Kind::Colon,
                };
                toks.push(Tok { kind, text: c.to_string(), line: l, col: co });
                i += 1;
                col += 1;
            }
            '"' => {
                i += 1;
                col += 1;
                let mut s = String::new();
                loop {
                    if i >= b.len() || b[i] == '\n' {
                        return Err(format!("{}:{}: unterminated string", l, co));
                    }
                    let d = b[i];
                    if d == '"' {
                        i += 1;
                        col += 1;
                        break;
                    }
                    if d == '\\' {
                        if i + 1 >= b.len() {
                            return Err(format!("{}:{}: unterminated escape", line, col));
                        }
                        let e = b[i + 1];
                        s.push(match e {
                            'n' => '\n',
                            't' => '\t',
                            '"' => '"',
                            '\\' => '\\',
                            _ => return Err(format!("{}:{}: unknown escape \\{}", line, col, e)),
                        });
                        i += 2;
                        col += 2;
                    } else {
                        s.push(d);
                        i += 1;
                        col += 1;
                    }
                }
                toks.push(Tok { kind: Kind::Str, text: s, line: l, col: co });
            }
            c if c.is_ascii_digit()
                || (c == '-' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) =>
            {
                let mut s = String::new();
                s.push(c);
                i += 1;
                col += 1;
                while i < b.len() && (b[i].is_ascii_digit() || b[i] == '.') {
                    s.push(b[i]);
                    i += 1;
                    col += 1;
                }
                toks.push(Tok { kind: Kind::Num, text: s, line: l, col: co });
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut s = String::new();
                s.push(c);
                i += 1;
                col += 1;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == '_') {
                    s.push(b[i]);
                    i += 1;
                    col += 1;
                }
                toks.push(Tok { kind: Kind::Ident, text: s, line: l, col: co });
            }
            other => {
                return Err(format!("{}:{}: unexpected character {:?}", l, co, other));
            }
        }
    }
    toks.push(Tok { kind: Kind::Eof, text: String::new(), line, col });
    Ok(toks)
}
