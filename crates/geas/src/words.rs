//! Command strings into words (DESIGN §11). geas splits a `run` or `serve` string
//! itself, never through a shell, so a claims file means the same on every machine:
//! white space separates words; `'…'` keeps everything in it as it is; `"…"` keeps
//! white space and reads `\"` and `\\`; outside quotes, a backslash keeps the next
//! character as it is. Nothing else is special: no variables, globs, `~`, pipes or
//! redirections. `{port}` is replaced in each word after splitting.

/// What a target's command and its `env` values say where its port goes.
pub const PORT: &str = "{port}";

/// Why a command does not split into words: the index (from 0) of the character in
/// the command string where the problem opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsplit {
    /// A `'` that nothing closes.
    OpenSingle(usize),
    /// A `"` that nothing closes.
    OpenDouble(usize),
    /// A backslash with nothing after it to keep.
    TrailingBackslash(usize),
}

/// The words of a command string.
pub fn split(s: &str) -> Result<Vec<String>, Unsplit> {
    let c: Vec<char> = s.chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();
    // a word has begun, even an empty one (`''`)
    let mut begun = false;
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            if begun {
                words.push(std::mem::take(&mut word));
                begun = false;
            }
            i += 1;
            continue;
        }
        begun = true;
        match ch {
            '\'' => {
                let open = i;
                i += 1;
                loop {
                    match c.get(i) {
                        None => return Err(Unsplit::OpenSingle(open)),
                        Some('\'') => break,
                        Some(&x) => word.push(x),
                    }
                    i += 1;
                }
                i += 1;
            }
            '"' => {
                let open = i;
                i += 1;
                loop {
                    match c.get(i) {
                        None => return Err(Unsplit::OpenDouble(open)),
                        Some('"') => break,
                        Some('\\') if matches!(c.get(i + 1), Some('"' | '\\')) => {
                            word.push(c[i + 1]);
                            i += 1;
                        }
                        Some(&x) => word.push(x),
                    }
                    i += 1;
                }
                i += 1;
            }
            '\\' => match c.get(i + 1) {
                None => return Err(Unsplit::TrailingBackslash(i)),
                Some(&x) => {
                    word.push(x);
                    i += 2;
                }
            },
            x => {
                word.push(x);
                i += 1;
            }
        }
    }
    if begun {
        words.push(word);
    }
    Ok(words)
}

/// The words with `{port}` replaced by the port.
pub fn with_port(words: &[String], port: u16) -> Vec<String> {
    let p = port.to_string();
    words.iter().map(|w| w.replace(PORT, &p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> Vec<String> {
        split(s).unwrap_or_else(|e| panic!("{s:?}: {e:?}"))
    }

    #[test]
    fn a_table_of_commands_and_their_words() {
        let table: &[(&str, &[&str])] = &[
            ("python3 calc.py", &["python3", "calc.py"]),
            ("  python3\tcalc.py  ", &["python3", "calc.py"]),
            ("python3 'my calc.py'", &["python3", "my calc.py"]),
            ("python3 \"my calc.py\"", &["python3", "my calc.py"]),
            ("python3 my\\ calc.py", &["python3", "my calc.py"]),
            ("echo '' x", &["echo", "", "x"]),
            ("echo \"\"", &["echo", ""]),
            ("echo a'b c'd", &["echo", "ab cd"]),
            ("echo 'a\\b'", &["echo", "a\\b"]),
            ("echo \"a\\\"b\" \"c\\\\d\"", &["echo", "a\"b", "c\\d"]),
            ("echo \"a\\nb\"", &["echo", "a\\nb"]),
            ("echo \\'x\\'", &["echo", "'x'"]),
            ("echo 'say \"hi\"'", &["echo", "say \"hi\""]),
            ("sh -c 'cd tools && ./gen'", &["sh", "-c", "cd tools && ./gen"]),
            ("echo $HOME ~ *.py | wc", &["echo", "$HOME", "~", "*.py", "|", "wc"]),
            ("server --port={port}", &["server", "--port={port}"]),
            ("", &[]),
            ("   ", &[]),
        ];
        for (s, want) in table {
            assert_eq!(w(s), want.iter().map(|x| x.to_string()).collect::<Vec<_>>(), "{s:?}");
        }
    }

    #[test]
    fn commands_that_do_not_split() {
        assert_eq!(split("python3 'my calc.py"), Err(Unsplit::OpenSingle(8)));
        assert_eq!(split("echo \"a"), Err(Unsplit::OpenDouble(5)));
        assert_eq!(split("echo \"a\\\""), Err(Unsplit::OpenDouble(5)));
        assert_eq!(split("echo a\\"), Err(Unsplit::TrailingBackslash(6)));
    }

    #[test]
    fn the_port_inside_a_word() {
        let words = w("server --port={port} --url=http://127.0.0.1:{port}/");
        assert_eq!(with_port(&words, 8123), ["server", "--port=8123", "--url=http://127.0.0.1:8123/"]);
    }
}
