//! `ritsu.mod`, the manifest of a module (DESIGN 17.3): its name, its version, its dependencies and
//! the rest a person writes, a statement a line, in ritsu's lexis ([`super::lex`]).
//!
//! What is read keeps every line as it was, with its own ending, so that the text written back is
//! the bytes read. The commands that change the file (`ritsu mod add`, `get`, `remove`) change it a
//! line at a time: `add` puts a line after the last `require`, `get` changes the version word of
//! one `require` and no other byte of its line, `remove` takes one line out. None of them moves or
//! tidies another line, and each keeps the file's line endings and whether its last line ends
//! with one.
//!
//! A file that cannot be read is E301, a value of the wrong form E302 (DESIGN 17.20).

use super::lex::{self, Kind, Token};
use super::names::{self, ModuleName, Target};
use super::semver::Version;
use super::{Diag, E301, E302};
use crate::text::Text;
use crate::tr;

/// The manifest's file name, at the root of its module.
pub const FILE: &str = "ritsu.mod";

/// A value and where it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct At<T> {
    pub value: T,
    pub line: usize,
    pub col: usize,
}

/// Where a dependency comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A registry's repository (the dependency's module name) and the least version asked for.
    Oci { repository: String, min: Version },
    /// A directory, from the root of the module that requires it, `/` between its parts.
    Path { path: String },
}

impl Source {
    /// The words after the dependency's name on its `require` line.
    pub fn words(&self) -> String {
        match self {
            Source::Oci { repository, min } => format!("oci {} {min}", lex::quote(repository)),
            Source::Path { path } => format!("path {}", lex::quote(path)),
        }
    }
}

/// A `require` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Require {
    /// The dependency's name, `@<name>/` in a path.
    pub name: String,
    pub source: Source,
    pub line: usize,
    /// The column of the name.
    pub col: usize,
    /// The bytes of the version word in its line (an `oci` line's).
    version_at: Option<(usize, usize)>,
}

/// How a `signature` line asks a module to be signed (DESIGN 17.14).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Policy {
    /// A public key's file, from the root of the module; `tlog` false takes a signature the
    /// transparency log does not hold (`tlog off`).
    Key { path: String, tlog: bool },
    /// A signature with no key: who signed it, and the issuer of the identity.
    By { identity: String, issuer: String },
    /// The same, who signed it matched by a regular expression.
    Matching { pattern: String, issuer: String },
}

impl Policy {
    /// The words after the start the line covers, as `ritsu.mod` writes them.
    pub fn words(&self) -> String {
        match self {
            Policy::Key { path, tlog } => format!("key {}{}", lex::quote(path), if *tlog { "" } else { " tlog off" }),
            Policy::By { identity, issuer } => format!("by {} issuer {}", lex::quote(identity), lex::quote(issuer)),
            Policy::Matching { pattern, issuer } => format!("by matching {} issuer {}", lex::quote(pattern), lex::quote(issuer)),
        }
    }
}

/// A `signature` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    /// A module's name, or a start of names that ends with `/`.
    pub prefix: String,
    pub policy: Policy,
    pub line: usize,
}

impl Signature {
    /// The line as `ritsu.mod` writes it: what a record of a check says it checked.
    pub fn text(&self) -> String {
        format!("signature {} {}", lex::quote(&self.prefix), self.policy.words())
    }
}

/// A `library` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
    pub target: Target,
    pub name: String,
    pub line: usize,
}

/// A `ritsu.mod` read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// The path it was read from, as its diagnostics write it.
    pub file: String,
    /// Each line as it was, and its ending (`"\n"`, `"\r\n"`, or `""` for a last line with none).
    lines: Vec<(String, String)>,
    pub module: At<ModuleName>,
    pub version: Option<At<Version>>,
    pub description: Option<At<String>>,
    /// `ritsu >=…`: the least ritsu that reads the module.
    pub ritsu: Option<At<Version>>,
    pub license: Option<At<String>>,
    pub source: Option<At<String>>,
    pub requires: Vec<Require>,
    pub excludes: Vec<At<String>>,
    pub signatures: Vec<Signature>,
    pub libraries: Vec<Library>,
}

/// The words a statement starts with, in the order the file usually has them.
pub const STATEMENTS: [&str; 10] = ["module", "version", "description", "ritsu", "license", "source", "require", "exclude", "signature", "library"];

/// How a statement is written, for the sentence that says a line is not.
fn form(word: &str) -> Text {
    match word {
        "module" => tr!("module <モジュールの名前>", "module <module name>"),
        "version" => tr!("version <バージョン>", "version <version>"),
        "description" => tr!("description \"<説明>\"", "description \"<text>\""),
        "ritsu" => tr!("ritsu >=<バージョン>", "ritsu >=<version>"),
        "license" => tr!("license \"<SPDX の式>\"", "license \"<SPDX expression>\""),
        "source" => tr!("source \"<https の URL>\"", "source \"<https URL>\""),
        "require" => tr!("require <名前> oci \"<リポジトリ>\" <バージョン>` か `require <名前> path \"<パス>\"", "require <name> oci \"<repository>\" <version>` or `require <name> path \"<path>\""),
        "exclude" => tr!("exclude \"<パス>\"", "exclude \"<path>\""),
        "signature" => tr!(
            "signature \"<頭の部分>\" key \"<パス>\"`（あとに `tlog off` を付けてもよい）、`signature \"<頭の部分>\" by \"<署名者>\" issuer \"<発行者>\"`、`signature \"<頭の部分>\" by matching \"<正規表現>\" issuer \"<発行者>\"",
            "signature \"<start>\" key \"<path>\"` (with `tlog off` after it, if need be), `signature \"<start>\" by \"<identity>\" issuer \"<issuer>\"` or `signature \"<start>\" by matching \"<pattern>\" issuer \"<issuer>\""
        ),
        _ => tr!("library typescript|python|go \"<名前>\"", "library typescript|python|go \"<name>\""),
    }
}

/// What the words of a line are expected to be: a word (`Some(text)` for one that has to be that
/// word), or a string.
#[derive(Clone, Copy)]
enum Want {
    Any,
    Exactly(&'static str),
    Str,
}

/// Why a line does not have the words its statement takes: the note after the sentence.
fn shape_note(want: Option<Want>, got: Option<&Token>) -> Text {
    match (want, got) {
        (Some(_), None) => tr!("値が足りません", "a value is missing"),
        (None, Some(t)) => {
            let x = shown(t);
            tr!("{x} は余分です", "{x} is one word too many")
        }
        (Some(Want::Str), Some(t)) => {
            let x = shown(t);
            tr!("{x} は `\"…\"` で囲んでください", "write {x} in quotes")
        }
        (Some(Want::Exactly(w)), Some(t)) => {
            let x = shown(t);
            tr!("{x} のところには `{w}` を書きます", "`{w}` comes where {x} is")
        }
        (Some(Want::Any), Some(t)) => {
            let x = shown(t);
            tr!("{x} は `\"…\"` で囲まずに書いてください", "write {x} as a word, not in quotes")
        }
        (None, None) => Text::default(),
    }
}

fn shown(t: &Token) -> String {
    match t.kind {
        Kind::Word => format!("`{}`", t.text),
        Kind::Str => format!("`{}`", lex::quote(&t.text)),
    }
}

/// Whether a token is what is wanted.
fn fits(want: Want, t: &Token) -> bool {
    match want {
        Want::Any => t.kind == Kind::Word,
        Want::Exactly(w) => t.is_word(w),
        Want::Str => t.kind == Kind::Str,
    }
}

/// A path written in `ritsu.mod`, from the root of the module: what is wrong with it, if anything.
/// `inside` asks it to stay in the module (`exclude`, a key) rather than lead to a directory beside
/// it (a `path` dependency).
pub fn path_problem(p: &str, inside: bool) -> Option<Text> {
    if p.is_empty() {
        return Some(tr!("パスが空です", "the path is empty"));
    }
    if crate::paths::is_absolute(p) {
        return Some(tr!("`{p}` は絶対パスです。モジュールのルートからの相対パスで書いてください", "`{p}` is an absolute path; write it from the root of the module"));
    }
    if p.contains('\\') {
        return Some(tr!("`{p}` に `\\` があります。パスは `/` で区切ってください", "`{p}` has a `\\` in it; the parts of a path have `/` between them"));
    }
    if inside {
        match crate::paths::join(".", p) {
            Ok(j) if j == "." => return Some(tr!("`{p}` はモジュールのルートそのものです", "`{p}` is the root of the module itself")),
            Ok(_) => {}
            Err(_) => return Some(tr!("`{p}` はモジュールの外を指します", "`{p}` leads outside the module")),
        }
    }
    None
}

struct Reader<'a> {
    file: &'a str,
    diags: Vec<Diag>,
}

impl Reader<'_> {
    fn at(&mut self, code: &'static str, line: usize, col: usize, src: &str, message: Text) -> &mut Diag {
        self.diags.push(Diag::at(code, self.file, line, col, message).rel(self.file).source(&src_line(src, line)));
        self.diags.last_mut().unwrap()
    }
}

/// A one-line source that [`Diag::source`] reads its line `n` from.
fn src_line(text: &str, n: usize) -> String {
    let mut s = "\n".repeat(n.saturating_sub(1));
    s.push_str(text);
    s
}

/// `bytes`, read as the `ritsu.mod` at `file` (a path from the root, as diagnostics write it). Err
/// holds every E301 and E302 the file has.
pub fn parse(bytes: &[u8], file: &str) -> Result<Manifest, Vec<Diag>> {
    let mut r = Reader { file, diags: Vec::new() };
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => {
            let line = bytes[..e.valid_up_to()].iter().filter(|b| **b == b'\n').count() + 1;
            r.at(E301, line, 0, "", tr!("UTF-8 として読めないバイトがあります", "there are bytes that are not UTF-8"));
            return Err(r.diags);
        }
    };
    let lines: Vec<(String, String)> = lex::split(text).into_iter().map(|(l, e)| (l.to_string(), e.to_string())).collect();
    let mut module: Option<At<ModuleName>> = None;
    let mut once: Vec<(&'static str, usize)> = Vec::new();
    let mut m = Manifest {
        file: file.to_string(),
        lines: lines.clone(),
        module: At { value: ModuleName::Local(String::new()), line: 0, col: 0 },
        version: None,
        description: None,
        ritsu: None,
        license: None,
        source: None,
        requires: Vec::new(),
        excludes: Vec::new(),
        signatures: Vec::new(),
        libraries: Vec::new(),
    };
    let mut first = true;
    let mut said_first = false;
    for (i, (src, _)) in lines.iter().enumerate() {
        let n = i + 1;
        let lexed = match lex::line(src) {
            Ok(l) => l,
            Err(e) => {
                // what the line was is not known: it may have been the `module` line
                r.at(E301, n, e.col(), src, e.text());
                said_first |= first;
                first = false;
                continue;
            }
        };
        let ts = &lexed.tokens;
        let Some(head) = ts.first() else { continue };
        if head.kind == Kind::Str {
            let s = lex::quote(&head.text);
            r.at(E301, n, head.col, src, tr!("文は語で始めてください。`{s}` は文字列です", "a statement starts with a word, and `{s}` is a string"));
            said_first |= first;
            first = false;
            continue;
        }
        let Some(word) = STATEMENTS.into_iter().find(|w| *w == head.text) else {
            let w = &head.text;
            let words = STATEMENTS.map(|w| Text::same(format!("`{w}`")));
            let (ja, en) = (Text::list(&words).ja, Text::list(&words).en);
            r.at(E301, n, head.col, src, tr!("`{w}` で始まる文はありません。文は {ja} のどれかで始めてください", "no statement starts with `{w}`; a statement starts with one of {en}"));
            said_first |= first;
            first = false;
            continue;
        };
        if first && word != "module" {
            said_first = true;
            r.at(E301, n, head.col, src, tr!("最初の文は `module` です。`ritsu.mod` は `module <モジュールの名前>` の行で始めてください", "the first statement is `module`: start `ritsu.mod` with the line `module <module name>`"));
        }
        first = false;
        // the statements written once, and `library` once a target
        let key: Option<&'static str> = match word {
            "module" | "version" | "description" | "ritsu" | "license" | "source" => Some(word),
            _ => None,
        };
        if let Some(k) = key {
            if let Some((_, l)) = once.iter().find(|(w, _)| *w == k) {
                let l = *l;
                r.at(E301, n, head.col, src, tr!("`{k}` は一度しか書けません。{l} 行目にもう書いてあります", "`{k}` is written once, and it is already on line {l}"));
                continue;
            }
            once.push((k, n));
        }
        // the words each statement takes
        let shape: &[Want] = match word {
            "module" | "version" | "ritsu" => &[Want::Any],
            "description" | "license" | "source" | "exclude" => &[Want::Str],
            "library" => &[Want::Any, Want::Str],
            "require" => match ts.get(2).map(|t| t.text.as_str()) {
                Some("path") => &[Want::Any, Want::Exactly("path"), Want::Str],
                _ => &[Want::Any, Want::Exactly("oci"), Want::Str, Want::Any],
            },
            _ => match (ts.get(2).map(|t| t.text.as_str()), ts.get(3).map(|t| t.text.as_str())) {
                (Some("key"), _) if ts.len() > 4 => &[Want::Str, Want::Exactly("key"), Want::Str, Want::Exactly("tlog"), Want::Exactly("off")],
                (Some("key"), _) => &[Want::Str, Want::Exactly("key"), Want::Str],
                (Some("by"), Some("matching")) => &[Want::Str, Want::Exactly("by"), Want::Exactly("matching"), Want::Str, Want::Exactly("issuer"), Want::Str],
                (Some("by"), _) => &[Want::Str, Want::Exactly("by"), Want::Str, Want::Exactly("issuer"), Want::Str],
                _ => &[Want::Str, Want::Exactly("key"), Want::Str],
            },
        };
        let args = &ts[1..];
        let wrong = (0..shape.len().max(args.len())).find(|&k| match (shape.get(k), args.get(k)) {
            (Some(w), Some(t)) => !fits(*w, t),
            _ => true,
        });
        if let Some(k) = wrong {
            let (f, col) = (form(word), args.get(k).map_or(src.chars().count() + 1, |t| t.col));
            let d = r.at(E301, n, col, src, Text::new(format!("`{}` の行は `{}` の形で書いてください", word, f.ja), format!("write the `{}` line as `{}`", word, f.en)));
            d.notes.push(shape_note(shape.get(k).copied(), args.get(k)));
            continue;
        }
        let v = |k: usize| &args[k];
        match word {
            "module" => match ModuleName::parse(&v(0).text) {
                Ok(name) => module = Some(At { value: name, line: n, col: v(0).col }),
                Err(e) => {
                    let w = &v(0).text;
                    let d = r.at(E302, n, v(0).col, src, tr!("`{w}` はモジュールの名前として読めません", "`{w}` is not a module's name").then(&sep(&e.text())));
                    d.notes.push(tr!(
                        "配るモジュールの名前は `<レジストリ>/<リポジトリ>`（`example.com/acme/pricing` など）で、配らないモジュールの名前は、英小文字で始まり英小文字、数字、`_`、`-` でできた 64 字までの語です",
                        "a module that is published is named `<registry>/<repository>` (`example.com/acme/pricing`, say); one that is not is named with a word of lowercase letters, digits, `_` and `-` that starts with a letter, 64 characters at most"
                    ));
                }
            },
            "version" => match Version::parse(&v(0).text) {
                Ok(ver) => m.version = Some(At { value: ver, line: n, col: v(0).col }),
                Err(e) => {
                    r.at(E302, n, v(0).col, src, e.text(&v(0).text));
                }
            },
            "ritsu" => {
                let w = &v(0).text;
                match w.strip_prefix(">=").map(Version::parse) {
                    Some(Ok(ver)) => m.ritsu = Some(At { value: ver, line: n, col: v(0).col }),
                    Some(Err(e)) => {
                        let rest = &w[2..];
                        r.at(E302, n, v(0).col, src, e.text(rest));
                    }
                    None => {
                        r.at(E302, n, v(0).col, src, tr!(
                            "`ritsu` の値は `>=0.37.0` のように、`>=` とバージョンを続けて一語で書いてください",
                            "the value of `ritsu` is `>=` and a version as one word, as `>=0.37.0`"
                        ));
                    }
                }
            }
            "description" => m.description = Some(At { value: v(0).text.clone(), line: n, col: v(0).col }),
            "license" => {
                if v(0).text.trim().is_empty() {
                    r.at(E302, n, v(0).col, src, tr!("`license` が空です。SPDX の式（`MIT OR Apache-2.0` など）を書いてください", "the `license` is empty; write an SPDX expression (`MIT OR Apache-2.0`, say)"));
                } else {
                    m.license = Some(At { value: v(0).text.clone(), line: n, col: v(0).col });
                }
            }
            "source" => {
                let url = &v(0).text;
                if crate::urls::scheme_and_host(url).is_some_and(|(s, _)| s == "https") {
                    m.source = Some(At { value: url.clone(), line: n, col: v(0).col });
                } else {
                    r.at(E302, n, v(0).col, src, tr!("`source` の `{url}` は https の URL で書いてください", "the `source` `{url}` is an https URL"));
                }
            }
            "require" => {
                let name = &v(0).text;
                if let Err(e) = names::dependency_ok(name) {
                    let d = r.at(E302, n, v(0).col, src, tr!("依存の名前 `{name}` は使えません", "`{name}` cannot name a dependency").then(&sep(&e.text())));
                    d.notes.push(tr!(
                        "依存の名前は、英小文字で始まり英小文字、数字、`_` でできた 64 字までの語です。`@<名前>/` としてコマンドラインに書き、生成するパッケージでは Python のパッケージと Go のディレクトリの名前になるので、どの出力先でもそのまま使える字に限っています",
                        "a dependency's name is a word of lowercase letters, digits and `_` that starts with a letter, 64 characters at most: it is written `@<name>/` on a command line, and names a Python package and a directory of Go in a generated package, so it keeps to what every target takes as it is"
                    ));
                    continue;
                }
                if let Some(other) = m.requires.iter().find(|q| &q.name == name) {
                    let l = other.line;
                    r.at(E302, n, v(0).col, src, tr!("依存 `{name}` は、{l} 行目にもう書いてあります。依存の名前は一つの `ritsu.mod` の中で一度だけ使えます", "the dependency `{name}` is already on line {l}; a name is required once in a `ritsu.mod`"));
                    continue;
                }
                let source = if v(1).text == "path" {
                    if let Some(t) = path_problem(&v(2).text, false) {
                        r.at(E302, n, v(2).col, src, t);
                        continue;
                    }
                    Source::Path { path: v(2).text.clone() }
                } else {
                    let repo = &v(2).text;
                    match ModuleName::parse(repo) {
                        Ok(mn) if mn.is_published() => {}
                        Ok(_) => {
                            r.at(E302, n, v(2).col, src, tr!(
                                "`{repo}` は配らないモジュールの名前です。`oci` には `<レジストリ>/<リポジトリ>` を書いてください",
                                "`{repo}` is the name of a module that is not published; `oci` takes `<registry>/<repository>`"
                            ));
                            continue;
                        }
                        Err(e) => {
                            r.at(E302, n, v(2).col, src, tr!("`{repo}` はモジュールの名前として読めません", "`{repo}` is not a module's name").then(&sep(&e.text())));
                            continue;
                        }
                    }
                    match Version::parse(&v(3).text) {
                        Ok(min) => Source::Oci { repository: repo.clone(), min },
                        Err(e) => {
                            r.at(E302, n, v(3).col, src, e.text(&v(3).text));
                            continue;
                        }
                    }
                };
                let version_at = matches!(source, Source::Oci { .. }).then(|| (v(3).start, v(3).end));
                m.requires.push(Require { name: name.clone(), source, line: n, col: v(0).col, version_at });
            }
            "exclude" => match path_problem(&v(0).text, true) {
                Some(t) => {
                    r.at(E302, n, v(0).col, src, t);
                }
                None => m.excludes.push(At { value: v(0).text.clone(), line: n, col: v(0).col }),
            },
            "signature" => {
                let prefix = &v(0).text;
                if let Err(e) = names::prefix_ok(prefix) {
                    r.at(E302, n, v(0).col, src, tr!(
                        "`signature` の頭の部分 `{prefix}` は、配るモジュールの名前か、`/` で終わるその頭の部分（`\"example.com/acme/\"` など）で書いてください",
                        "the start `{prefix}` of a `signature` line is the name of a published module, or a start of one that ends with `/` (`\"example.com/acme/\"`, say)"
                    ).then(&sep(&e.text())));
                    continue;
                }
                let policy = match v(1).text.as_str() {
                    "key" => {
                        if let Some(t) = path_problem(&v(2).text, true) {
                            r.at(E302, n, v(2).col, src, t);
                            continue;
                        }
                        Policy::Key { path: v(2).text.clone(), tlog: args.len() == 3 }
                    }
                    _ if v(2).is_word("matching") => {
                        let (pattern, issuer) = (&v(3).text, &v(5).text);
                        if pattern.is_empty() || issuer.is_empty() {
                            r.at(E302, n, v(3).col, src, tr!("署名者の正規表現と発行者は、空にできません", "neither the pattern of who signed nor the issuer can be empty"));
                            continue;
                        }
                        Policy::Matching { pattern: pattern.clone(), issuer: issuer.clone() }
                    }
                    _ => {
                        let (identity, issuer) = (&v(2).text, &v(4).text);
                        if identity.is_empty() || issuer.is_empty() {
                            r.at(E302, n, v(2).col, src, tr!("署名者と発行者は、空にできません", "neither who signed nor the issuer can be empty"));
                            continue;
                        }
                        Policy::By { identity: identity.clone(), issuer: issuer.clone() }
                    }
                };
                m.signatures.push(Signature { prefix: prefix.clone(), policy, line: n });
            }
            _ => {
                let Some(target) = Target::from_word(&v(0).text) else {
                    let t = &v(0).text;
                    r.at(E301, n, v(0).col, src, tr!("`library` の出力先 `{t}` はありません。typescript、python、go のどれかです", "`{t}` is not a target of `library`; the targets are typescript, python and go"));
                    continue;
                };
                if let Some(other) = m.libraries.iter().find(|l| l.target == target) {
                    let (t, l) = (target.word(), other.line);
                    r.at(E301, n, v(0).col, src, tr!("`library {t}` は一度しか書けません。{l} 行目にもう書いてあります", "`library {t}` is written once, and it is already on line {l}"));
                    continue;
                }
                let name = &v(1).text;
                if let Err(e) = names::library_ok(target, name) {
                    let t = target.word();
                    r.at(E302, n, v(1).col, src, tr!("`library {t}` の名前 `{name}` は使えません", "`{name}` cannot name the `library {t}`").then(&sep(&e.0)));
                    continue;
                }
                m.libraries.push(Library { target, name: name.clone(), line: n });
            }
        }
    }
    // a `module` line that was written, and said wrong, is not said missing too
    match module {
        Some(at) => m.module = at,
        None if !said_first && !once.iter().any(|(w, _)| *w == "module") => {
            r.diags.push(Diag::whole(E301, file, tr!("`module` の行がありません。`ritsu.mod` は `module <モジュールの名前>` の行で始めてください", "there is no `module` line: start `ritsu.mod` with the line `module <module name>`")).rel(file));
        }
        None => {}
    }
    if r.diags.is_empty() { Ok(m) } else { Err(r.diags) }
}

/// The version a `ritsu >=` line of `bytes` asks for, read line by line even when the whole does
/// not read: a `ritsu.mod` a newer ritsu wrote may have a statement this one does not know, and
/// "run a newer ritsu" says more than the statement it stops at.
pub fn wants(bytes: &[u8]) -> Option<Version> {
    let text = String::from_utf8_lossy(bytes);
    lex::split(&text).into_iter().find_map(|(l, _)| {
        let ts = lex::line(l).ok()?.tokens;
        match ts.as_slice() {
            [w, v] if w.is_word("ritsu") && v.kind == Kind::Word => Version::parse(v.text.strip_prefix(">=")?).ok(),
            _ => None,
        }
    })
}

/// The reason after the sentence: `。` before it in Japanese, `: ` in English.
fn sep(t: &Text) -> Text {
    Text::new(format!("。{}", t.ja), format!(": {}", t.en))
}

/// Why a change to `ritsu.mod` cannot be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    /// No `require` line has the name.
    NotRequired(String),
    /// The name is required already, on a line.
    Required(String, usize),
    /// The dependency comes from a path, which has no version to change.
    NotOci(String),
    /// The name cannot name a dependency.
    BadName(String, names::NameError),
}

impl EditError {
    pub fn text(&self) -> Text {
        match self {
            EditError::NotRequired(n) => tr!("`ritsu.mod` に依存 `{n}` はありません", "`ritsu.mod` requires no `{n}`"),
            EditError::Required(n, l) => tr!("依存 `{n}` は、`ritsu.mod` の {l} 行目にもう書いてあります", "the dependency `{n}` is already on line {l} of `ritsu.mod`"),
            EditError::NotOci(n) => tr!(
                "依存 `{n}` はパスから読むので、バージョンを変えられません。レジストリから取るなら、`ritsu.mod` の行を `oci` に書き替えてください",
                "the dependency `{n}` is read from a path, which has no version to change; to take it from a registry, write its line in `ritsu.mod` with `oci`"
            ),
            EditError::BadName(n, e) => tr!("依存の名前 `{n}` は使えません", "`{n}` cannot name a dependency").then(&sep(&e.text())),
        }
    }
}

impl Manifest {
    /// The text, which is the bytes it was read from until a change is made.
    pub fn text(&self) -> String {
        self.lines.iter().map(|(l, e)| format!("{l}{e}")).collect()
    }

    /// The `require` of a dependency.
    pub fn require(&self, name: &str) -> Option<&Require> {
        self.requires.iter().find(|r| r.name == name)
    }

    /// The line ending a new line takes: the file's first, or `\n` when no line has one.
    fn ending(&self) -> String {
        self.lines.iter().map(|(_, e)| e.as_str()).find(|e| !e.is_empty()).unwrap_or("\n").to_string()
    }

    /// The text with `require <name> <source>` after the last `require` line, or after one blank line
    /// at the end of the file when it has none (`ritsu mod add`).
    pub fn added(&self, name: &str, source: &Source) -> Result<String, EditError> {
        names::dependency_ok(name).map_err(|e| EditError::BadName(name.to_string(), e))?;
        if let Some(r) = self.require(name) {
            return Err(EditError::Required(name.to_string(), r.line));
        }
        let new = format!("require {name} {}", source.words());
        let nl = self.ending();
        let mut lines = self.lines.clone();
        match self.requires.iter().map(|r| r.line).max() {
            Some(l) => {
                // after the last `require` line; it may be the last line, with no ending
                if lines[l - 1].1.is_empty() {
                    lines[l - 1].1 = nl.clone();
                    lines.insert(l, (new, String::new()));
                } else {
                    lines.insert(l, (new, nl));
                }
            }
            None => {
                let had_end = lines.last().is_none_or(|(_, e)| !e.is_empty());
                if let Some(last) = lines.last_mut()
                    && last.1.is_empty()
                {
                    last.1 = nl.clone();
                }
                if lines.last().is_some_and(|(l, _)| !l.trim().is_empty()) {
                    lines.push((String::new(), nl.clone()));
                }
                lines.push((new, if had_end { nl } else { String::new() }));
            }
        }
        Ok(lines.iter().map(|(l, e)| format!("{l}{e}")).collect())
    }

    /// The text with the version of an `oci` dependency changed, and no other byte of the line
    /// (`ritsu mod get`).
    pub fn with_version(&self, name: &str, version: &Version) -> Result<String, EditError> {
        let r = self.require(name).ok_or_else(|| EditError::NotRequired(name.to_string()))?;
        let Some((a, b)) = r.version_at else { return Err(EditError::NotOci(name.to_string())) };
        let mut lines = self.lines.clone();
        let line = &mut lines[r.line - 1].0;
        line.replace_range(a..b, &version.to_string());
        Ok(lines.iter().map(|(l, e)| format!("{l}{e}")).collect())
    }

    /// The text without the `require` line of a dependency (`ritsu mod remove`). A comment above it
    /// stays: which line it is about is not the command's to tell.
    pub fn removed(&self, name: &str) -> Result<String, EditError> {
        let r = self.require(name).ok_or_else(|| EditError::NotRequired(name.to_string()))?;
        let mut lines = self.lines.clone();
        let (_, ending) = lines.remove(r.line - 1);
        // the last line keeps having, or not having, an ending
        if ending.is_empty()
            && let Some(last) = lines.last_mut()
        {
            last.1 = String::new();
        }
        Ok(lines.iter().map(|(l, e)| format!("{l}{e}")).collect())
    }
}

/// The `ritsu.mod` `ritsu mod init` writes: the module's name and version, the least ritsu that
/// reads it, and `exclude "generated"`, where `ritsu gen` writes by default.
pub fn init(name: &ModuleName, version: &Version, ritsu: &Version) -> String {
    format!("module {name}\nversion {version}\nritsu >={ritsu}\n\nexclude \"generated\"\n")
}
