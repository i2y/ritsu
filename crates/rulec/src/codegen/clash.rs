//! The names an enum's code takes beside the rule's own functions, in the languages whose module
//! puts the two in one place (§15.326).
//!
//! A rule named like the enum it answers (`rule size` with `enum size`), like a value of it
//! (`rule size_small`), or like a function its module writes for it (`rule size_label`,
//! `rule parse_size`) has a module where one name is declared twice: Go's package has `func Size`
//! and `type Size`, Python's module `def size_label` and the display names' `def size_label`. In
//! the language where that happens, and only there, the enum's type takes the first name from
//! `<type>2` that meets nothing, and every name made of the type follows it (`Size2`,
//! `Size2Small`, `ParseSize2`, `size2_label`). The rule's functions keep their names in every
//! language: they are what a caller calls. A module where nothing meets is written as it always
//! was.

use super::{lower_lead, pascal, pub_name, snake, Gen};
use crate::ast::FileKind;
use std::collections::{BTreeMap, HashMap, HashSet};

/// `text` with each word of it that `to` has a key for written as `to` says: a word is a run of
/// letters, digits and `_` that stands on its own (§15.336).
pub(super) fn rename_words(text: &str, to: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let put = |word: &mut String, out: &mut String| {
        if !word.is_empty() {
            out.push_str(to.get(word.as_str()).map(String::as_str).unwrap_or(word.as_str()));
            word.clear();
        }
    };
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            word.push(ch);
        } else {
            put(&mut word, &mut out);
            out.push(ch);
        }
    }
    put(&mut word, &mut out);
    out
}

/// The mark the generator writes before the rule's function where its name is a word that stands
/// for a name of the generated code's own elsewhere in the same text (§15.336): [`rename_code`]
/// leaves the word after it as it is, and takes every mark out.
pub(super) const MARK: char = '\u{E000}';

/// How a language writes the parts of its code that hold no names of the code: its strings, its
/// comments and its imports ([`rename_code`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Syntax {
    Python,
    /// TypeScript and JavaScript.
    Script,
    /// Ruby, and its signatures: an inline one (`#: Array[Integer]`) is code.
    Ruby,
    Rust,
    Swift,
}

/// `text`, code of the language `syntax` speaks, with each word of it that `to` has a key for
/// written as `to` says where the word is code (§15.336): not in a string or a comment, not right
/// after `.` or `::` (a member's or a path's own name), not on a line that imports, and not right
/// after [`MARK`]. The code inside a string — an f-string's `{…}`, a template's `${…}`, Ruby's `#{…}`,
/// Swift's `\(…)` — is code. In Ruby a word after `self.` is a function of the module's, which its
/// code calls with `self.` or without it. Every mark is taken out.
pub(super) fn rename_code(text: &str, to: &BTreeMap<String, String>, syntax: Syntax) -> String {
    let mut r = Renamer { cs: text.chars().collect(), i: 0, out: String::with_capacity(text.len() + 64), to, syntax };
    r.code(None);
    r.out.replace(MARK, "")
}

/// Whether the code in `text` reads `word`, where [`rename_code`] would write it apart: a word of
/// the code, not one of a string or a comment, nor a member's own name (§15.340).
pub(super) fn reads_word(text: &str, word: &str, syntax: Syntax) -> bool {
    const SEEN: char = '\u{E001}';
    rename_code(text, &BTreeMap::from([(word.to_string(), format!("{word}{SEEN}"))]), syntax).contains(SEEN)
}

/// Where `b` first parts from `a`, by the line: what a test's build says where the code
/// [`super::Gen::apart_text`] writes with the marks is other than the code.
pub(super) fn first_difference(a: &str, b: &str) -> String {
    let (la, lb): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let k = la.iter().zip(&lb).take_while(|(x, y)| x == y).count();
    format!("line {}:\n- {}\n+ {}", k + 1, la.get(k).unwrap_or(&""), lb.get(k).unwrap_or(&""))
}

/// Whether `text`, a module in the language `syntax` speaks, declares `name` twice at its top: the
/// rule's function, and a function or a constant of the module's own (§15.336).
pub(super) fn declares_twice(text: &str, name: &str, syntax: Syntax) -> bool {
    let heads: Vec<String> = match syntax {
        Syntax::Python => vec![format!("def {name}("), format!("{name} = ")],
        Syntax::Script => ["export function", "function", "export const", "const"].iter().map(|k| format!("{k} {name}{}", if k.ends_with("function") { "(" } else { " " })).collect(),
        // the module's functions, inside the rule's module
        Syntax::Ruby => vec![format!("def self.{name}(")],
        Syntax::Rust => ["pub fn", "pub(crate) fn", "fn", "const", "static", "pub const"].iter().map(|k| format!("{k} {name}{}", if k.ends_with("fn") { "(" } else { ":" })).collect(),
        // Swift's functions, the rule's and the helpers, are told apart by what they take
        Syntax::Swift => return false,
    };
    let declared = |l: &str| {
        let l = if syntax == Syntax::Ruby { l.trim_start() } else { l };
        heads.iter().any(|h| l.starts_with(h.as_str()))
    };
    text.lines().filter(|l| declared(l)).count() >= 2
}

/// Whether the word `w` stands in `text` on its own.
pub(super) fn has_word(text: &str, w: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(w).any(|(i, _)| !text[..i].chars().next_back().is_some_and(word) && !text[i + w.len()..].chars().next().is_some_and(word))
}

/// What Python's module imports from `typing` by name, which it reads from `typing` itself where a
/// name of the rule's hides one.
const PY_TYPING: &[&str] = &["Any", "NamedTuple", "NewType", "cast"];

/// The classes Python's module reads by their names, which an enum's type can hide: the errors it
/// raises and what it imports from `typing` (§15.336).
const PY_TYPES: &[&str] = &["ValueError", "AssertionError", "TypeError", "Any", "NamedTuple", "NewType"];

/// The globals TypeScript's and JavaScript's code reads as values or types, which `globalThis`
/// holds: not `globalThis` itself, `undefined` (a type as well), or the words the language reads as
/// keywords in a type (`any`, `readonly`, `interface`).
const TS_GLOBALS: &[&str] = &["Array", "BigInt", "Boolean", "Date", "Error", "JSON", "Map", "Math", "NaN", "Number", "Object", "Promise", "Set", "String", "Symbol", "console"];

/// What Ruby's module calls without a receiver, which `Kernel` answers.
const RB_KERNEL: &[&str] = &["format", "raise", "loop", "lambda", "proc", "puts", "print", "require"];

/// The prelude's types the Rust code names, by their paths.
const RS_PATHS: &[(&str, &str)] =
    &[("Option", "::std::option::Option"), ("Result", "::std::result::Result"), ("String", "::std::string::String"), ("Vec", "::std::vec::Vec")];

/// The types the Swift code names, which the module `Swift` holds.
const SW_TYPES: &[&str] = &["Array", "Bool", "Double", "Error", "Int", "Int64", "Result", "Set", "String"];

/// Whether the generated code of the backend `id` reaches the builtin `word` past a name of the
/// rule's that hides it at the top of the module (§15.336): the rule's function, or an enum's type
/// (`ty`). The code reaches it another way there ([`super::Gen::apart_text`]), and W121 does not say
/// the name; a builtin it has no other way to is said.
pub(crate) fn reaches_past(id: &str, word: &str, ty: bool) -> bool {
    match id {
        "python" | "numpy" if ty => PY_TYPES.contains(&word),
        "python" | "numpy" => {
            crate::backend::ALL.iter().find(|b| b.id == "python").is_some_and(|b| b.globals.contains(&word))
                || super::own::py_reads(true).contains(&word)
                || PY_TYPING.contains(&word)
                || ["enum", "annotations"].contains(&word)
        }
        "typescript" | "javascript" => TS_GLOBALS.contains(&word),
        "ruby" if ty => super::own::rb_reads().contains(&word),
        "ruby" => RB_KERNEL.contains(&word),
        "rust" | "wasm" => ty && RS_PATHS.iter().any(|(w, _)| *w == word),
        "swift" => ty && SW_TYPES.contains(&word),
        // the functions PHP's module calls from the global namespace (`\count`), which PHP names
        // with case set aside; an enum's class meets none of them
        "php" => !ty && ritsu_emit::copies::rulec::PHP_BUILTIN.iter().any(|w| w.eq_ignore_ascii_case(word)),
        "java" => super::java::reaches_past(word, ty),
        _ => false,
    }
}

/// The walk of [`rename_code`] through the text.
struct Renamer<'a> {
    cs: Vec<char>,
    i: usize,
    out: String,
    to: &'a BTreeMap<String, String>,
    syntax: Syntax,
}

impl Renamer<'_> {
    fn at(&self, k: usize) -> Option<char> {
        self.cs.get(self.i + k).copied()
    }

    fn word_char(&self, c: char) -> bool {
        c.is_alphanumeric() || c == '_' || (c == '$' && self.syntax == Syntax::Script)
    }

    /// Copies the characters up to the end of the line, the line's end not among them.
    fn rest_of_line(&mut self) {
        while let Some(c) = self.at(0) {
            if c == '\n' {
                break;
            }
            self.out.push(c);
            self.i += 1;
        }
    }

    /// Whether the line that starts here imports: Python's `import` and `from … import`,
    /// TypeScript's and Swift's `import`.
    fn imports_here(&self) -> bool {
        let mut k = self.i;
        while self.cs.get(k).is_some_and(|c| *c == ' ' || *c == '\t') {
            k += 1;
        }
        let starts = |w: &str| w.chars().enumerate().all(|(j, c)| self.cs.get(k + j) == Some(&c)) && self.cs.get(k + w.chars().count()) == Some(&' ');
        match self.syntax {
            Syntax::Python => starts("import") || starts("from"),
            Syntax::Script | Syntax::Swift => starts("import"),
            Syntax::Ruby | Syntax::Rust => false,
        }
    }

    /// The code up to the end of the text, or, inside a string's code, up to the `close` that ends
    /// it (`}`, or `)` in Swift), which is left for the string.
    fn code(&mut self, close: Option<char>) {
        let open = if close == Some(')') { '(' } else { '{' };
        let mut depth = 0usize;
        let mut line_start = true;
        while let Some(c) = self.at(0) {
            if line_start && self.imports_here() {
                self.rest_of_line();
                line_start = false;
                continue;
            }
            line_start = c == '\n';
            match c {
                c if Some(c) == close && depth == 0 => return,
                c if c == open => {
                    depth += 1;
                    self.out.push(c);
                    self.i += 1;
                }
                c if Some(c) == close || (close.is_none() && c == '}') => {
                    depth = depth.saturating_sub(1);
                    self.out.push(c);
                    self.i += 1;
                }
                // Ruby's signature inline, `#: Array[Integer]`, names the types as code does
                '#' if self.syntax == Syntax::Ruby && self.at(1) == Some(':') => {
                    self.out.push_str("#:");
                    self.i += 2;
                }
                '#' if matches!(self.syntax, Syntax::Python | Syntax::Ruby) => self.rest_of_line(),
                '/' if matches!(self.syntax, Syntax::Script | Syntax::Rust | Syntax::Swift) && self.at(1) == Some('/') => self.rest_of_line(),
                '/' if matches!(self.syntax, Syntax::Script | Syntax::Rust | Syntax::Swift) && self.at(1) == Some('*') => {
                    while let Some(d) = self.at(0) {
                        self.out.push(d);
                        self.i += 1;
                        if d == '*' && self.at(0) == Some('/') {
                            self.out.push('/');
                            self.i += 1;
                            break;
                        }
                    }
                }
                '"' => self.string('"', false),
                '\'' if self.syntax == Syntax::Rust => self.rust_quote(),
                '\'' if self.syntax == Syntax::Swift => {
                    self.out.push(c);
                    self.i += 1;
                }
                '\'' => self.string('\'', false),
                '`' if self.syntax == Syntax::Script => self.string('`', false),
                MARK => {
                    self.i += 1;
                    while let Some(d) = self.at(0).filter(|d| self.word_char(*d)) {
                        self.out.push(d);
                        self.i += 1;
                    }
                }
                c if self.word_char(c) => self.word(),
                _ => {
                    self.out.push(c);
                    self.i += 1;
                }
            }
        }
    }

    /// A word of the code, written as `to` says where it is one of its keys and stands where a name
    /// of the code is read; a string's prefix (Python's `f`, `r`, `b`, Rust's `r`, `b`), with the
    /// string after it.
    fn word(&mut self) {
        let start = self.i;
        while self.at(0).is_some_and(|d| self.word_char(d)) {
            self.i += 1;
        }
        let w: String = self.cs[start..self.i].iter().collect();
        let next = self.at(0);
        let prefix = |set: &str| w.chars().count() <= 3 && w.chars().all(|ch| set.contains(ch));
        match (self.syntax, next) {
            (Syntax::Python, Some(q @ ('"' | '\''))) if prefix("rRbBfFuU") => {
                self.out.push_str(&w);
                self.string(q, w.contains(['f', 'F']));
                return;
            }
            (Syntax::Rust, Some('"' | '#')) if prefix("rb") && (next == Some('"') || w.contains('r')) => {
                self.out.push_str(&w);
                self.rust_raw(w.contains('r'));
                return;
            }
            _ => {}
        }
        let before = |k: usize| start.checked_sub(k).and_then(|j| self.cs.get(j)).copied();
        // a member's name or a path's (`.x`, not a range's `..x`; `::x`); Ruby's `self.x` is the
        // module's own
        let member = (before(1) == Some('.') && before(2) != Some('.')) || (before(1) == Some(':') && before(2) == Some(':'));
        let own_self = self.syntax == Syntax::Ruby
            && before(1) == Some('.')
            && start >= 5
            && self.cs[start - 5..start - 1].iter().collect::<String>() == "self"
            && !before(6).is_some_and(|c| self.word_char(c));
        match self.to.get(&w) {
            Some(new) if !member || own_self => self.out.push_str(new),
            _ => self.out.push_str(&w),
        }
    }

    /// A string that the quote `q` at the walk opens: Python's and Swift's in three quotes too, and
    /// the code inside an f-string (`code_in`), a template, a string of Ruby's in double quotes, or
    /// Swift's `\(…)`.
    fn string(&mut self, q: char, f: bool) {
        let triple = matches!(self.syntax, Syntax::Python | Syntax::Swift) && self.at(1) == Some(q) && self.at(2) == Some(q);
        let n = if triple { 3 } else { 1 };
        for _ in 0..n {
            self.out.push(q);
            self.i += 1;
        }
        let code_in = match self.syntax {
            Syntax::Python => f,
            Syntax::Script => q == '`',
            Syntax::Ruby => q == '"',
            Syntax::Rust | Syntax::Swift => false,
        };
        while let Some(c) = self.at(0) {
            if c == '\\' && self.syntax == Syntax::Swift && self.at(1) == Some('(') {
                self.out.push_str("\\(");
                self.i += 2;
                self.code(Some(')'));
                if self.at(0) == Some(')') {
                    self.out.push(')');
                    self.i += 1;
                }
                continue;
            }
            if c == '\\' {
                self.out.push(c);
                self.i += 1;
                if let Some(d) = self.at(0) {
                    self.out.push(d);
                    self.i += 1;
                }
                continue;
            }
            if c == q && (!triple || (self.at(1) == Some(q) && self.at(2) == Some(q))) {
                for _ in 0..n {
                    self.out.push(q);
                    self.i += 1;
                }
                return;
            }
            // a string in one quote ends with its line, where a language closes none there
            if !triple && c == '\n' && q != '`' && self.syntax != Syntax::Rust {
                return;
            }
            if code_in && self.syntax == Syntax::Python && c == '{' && self.at(1) == Some('{') {
                // `{{`, a brace of the text
                self.out.push_str("{{");
                self.i += 2;
                continue;
            }
            let opens = match self.syntax {
                Syntax::Python => c == '{',
                Syntax::Script => c == '$' && self.at(1) == Some('{'),
                Syntax::Ruby => c == '#' && self.at(1) == Some('{'),
                Syntax::Rust | Syntax::Swift => false,
            };
            if code_in && opens {
                let skip = if self.syntax == Syntax::Python { 1 } else { 2 };
                for _ in 0..skip {
                    self.out.push(self.cs[self.i]);
                    self.i += 1;
                }
                self.code(Some('}'));
                if self.at(0) == Some('}') {
                    self.out.push('}');
                    self.i += 1;
                }
                continue;
            }
            self.out.push(c);
            self.i += 1;
        }
    }

    /// Rust's `'`: a character (`'x'`, `'\n'`), or a lifetime's mark (`'a`, `'static`).
    fn rust_quote(&mut self) {
        let ch = self.at(1) == Some('\\') || (self.at(1).is_some() && self.at(2) == Some('\''));
        self.out.push('\'');
        self.i += 1;
        if !ch {
            return;
        }
        while let Some(c) = self.at(0) {
            self.out.push(c);
            self.i += 1;
            if c == '\\' {
                if let Some(d) = self.at(0) {
                    self.out.push(d);
                    self.i += 1;
                }
                continue;
            }
            if c == '\'' {
                return;
            }
        }
    }

    /// Rust's string after its prefix (`r"…"`, `r#"…"#`, `b"…"`), the prefix written already.
    fn rust_raw(&mut self, raw: bool) {
        if !raw {
            self.string('"', false);
            return;
        }
        let mut hashes = 0;
        while self.at(0) == Some('#') {
            self.out.push('#');
            self.i += 1;
            hashes += 1;
        }
        // `r#name`, a raw identifier: the name is a word of the code
        if self.at(0) != Some('"') {
            return;
        }
        self.out.push('"');
        self.i += 1;
        while let Some(c) = self.at(0) {
            self.out.push(c);
            self.i += 1;
            if c == '"' && (0..hashes).all(|k| self.at(k) == Some('#')) {
                for _ in 0..hashes {
                    self.out.push('#');
                    self.i += 1;
                }
                return;
            }
        }
    }
}

/// The code a backend writes, by the language whose names it has: the languages whose module puts
/// an enum's names beside the rule's functions. The others keep the two apart (Rust and Swift by
/// their kinds, SQL and Wasm by having no names of an enum).
pub(crate) fn speech(id: &str) -> Option<&'static str> {
    match id {
        "python" | "numpy" => Some("python"),
        "typescript" | "javascript" => Some("typescript"),
        "go" => Some("go"),
        "java" => Some("java"),
        "ruby" => Some("ruby"),
        "php" => Some("php"),
        _ => None,
    }
}

/// `base` with every field named one of `keys`, at any depth, taken from `theirs`, a value of the
/// same shape: what `rulec api` says of a language, from the generator of that language.
pub(crate) fn splice_fields(base: &mut crate::json::Json, theirs: &crate::json::Json, keys: &[&str]) {
    use crate::json::Json;
    match (base, theirs) {
        (Json::Obj(ours), Json::Obj(other)) => {
            for (k, v) in ours.iter_mut() {
                let Some((_, w)) = other.iter().find(|(ok, _)| ok == k) else { continue };
                if keys.contains(&k.as_str()) {
                    *v = w.clone();
                } else {
                    splice_fields(v, w, keys);
                }
            }
        }
        (Json::Arr(ours), Json::Arr(other)) => {
            for (v, w) in ours.iter_mut().zip(other) {
                splice_fields(v, w, keys);
            }
        }
        _ => {}
    }
}

/// What a Swift file rulec writes declares at its top level for the module it is built into: each
/// type, constant and variable by its name, each function by its signature (two functions of one
/// name and other parameters are two). A declaration `private` keeps to its file is not counted,
/// nor one declared twice in the file.
fn sw_top_names(text: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for line in text.lines() {
        if line.starts_with([' ', '\t', '/', '}', '@']) {
            continue;
        }
        let rest = line.strip_prefix("public ").unwrap_or(line);
        if rest.starts_with("private ") || rest.starts_with("fileprivate ") {
            continue;
        }
        let rest = rest.strip_prefix("final ").unwrap_or(rest);
        if let Some(sig) = rest.strip_prefix("func ") {
            out.insert(format!("func {}", sig.split('{').next().unwrap_or(sig).trim()));
            continue;
        }
        for kw in ["enum ", "struct ", "class ", "protocol ", "typealias ", "let ", "var "] {
            if let Some(r) = rest.strip_prefix(kw) {
                let name: String = r.trim_start_matches('`').chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                if !name.is_empty() {
                    out.insert(name);
                }
                break;
            }
        }
    }
    out
}

impl<'a> Gen<'a> {
    /// The Swift files `gen` writes for the rule — its module as `gen` writes it (`alone`: with
    /// the enums it reads from `enums` files declared in it, §15.326), its runner, and the module
    /// of every `enums` file it reads: each by the name it is written under, with the names it
    /// declares at its top ([`sw_top_names`]) and the enum of the file each is the type of, where
    /// it is one.
    fn sw_files(&self, alone: bool) -> Vec<(String, HashSet<String>, HashMap<String, String>)> {
        let alias = pub_name(&self.f.name);
        let mut types: HashMap<String, String> = self.own_enums().into_iter().map(|(jp, t)| (t, jp)).collect();
        if alone {
            for (_, decls) in self.foreign_enums() {
                types.extend(decls.into_iter().map(|(jp, t)| (t, jp)));
            }
        }
        let mut files = vec![
            (format!("{alias}.swift"), sw_top_names(&self.sw_module(alone)), types),
            (format!("{alias}_runner.swift"), sw_top_names(&self.swift_runner()), HashMap::new()),
        ];
        for p in crate::enums::owner_paths(self.f) {
            let Ok(ef) = crate::enums::read_enums_file(&p) else { continue };
            let eg = Gen::new(&ef.file, &ef.checked, &ef.src, &ef.path);
            files.push((format!("{}.swift", pub_name(&ef.file.name)), sw_top_names(&eg.sw_enums_module()), eg.own_enums().into_iter().map(|(jp, t)| (t, jp)).collect()));
        }
        files
    }

    /// Every name two of [`Gen::sw_files`] declare, which a module the files are built into cannot
    /// hold twice (`invalid redeclaration`), with the files that declare it and the name of the
    /// enum of each it is the type of, where it is one.
    fn sw_twice(&self, alone: bool) -> Vec<(String, Vec<(String, Option<String>)>)> {
        let mut seen: Vec<(String, Vec<(String, Option<String>)>)> = Vec::new();
        for (file, names, enums) in self.sw_files(alone) {
            let mut names: Vec<String> = names.into_iter().collect();
            names.sort();
            for n in names {
                let jp = enums.get(&n).cloned();
                match seen.iter_mut().find(|(x, _)| *x == n) {
                    Some((_, fs)) => fs.push((file.clone(), jp)),
                    None => seen.push((n, vec![(file.clone(), jp)])),
                }
            }
        }
        seen.into_iter().filter(|(_, fs)| fs.len() > 1).collect()
    }

    /// Every name two of the Swift files `gen` writes for the rule declare, as it writes them —
    /// the module in the shape `gen` gives it (§15.345).
    pub(crate) fn sw_redeclared(&self) -> Vec<(String, Vec<(String, Option<String>)>)> {
        self.sw_twice(self.sw_alone())
    }

    /// Whether the rule's Swift declares the enums it reads from `enums` files itself, and is built
    /// without their modules (§15.326): when the module of each such file, built into one module
    /// with the rule's and its runner, would declare a name one of the others declares. Each file's
    /// module declares every enum of the file, read or not, so a rule's own enum, an enum an `apply`
    /// brings in, the rule's `Fired`, or an enum of another file can meet one the rule never reads.
    pub(crate) fn sw_alone(&self) -> bool {
        if crate::enums::owner_paths(self.f).is_empty() {
            return false;
        }
        !self.sw_twice(false).is_empty()
    }

    /// The enums this rule's module would declare that an `enums` file it reads declares as the
    /// same enum — the type's name as the Swift module writes it, and every value as written, in
    /// the same order — so the file's module is the one that declares the type, and this one is
    /// left the members it alone had, in an `extension` (§15.355). The rule's name of each, with
    /// the file's own declaration of it, which the extension is read against. An `apply` can hold
    /// an enum the file also declares, by a name it made of the apply's (`authority:decider`, of
    /// `authority_decider`); a rule that applies it and reads the file for anything else met two
    /// declarations of one type before this (§15.345).
    pub(crate) fn sw_shared_enums(&self) -> BTreeMap<String, crate::ast::EnumDecl> {
        let mut out = BTreeMap::new();
        for p in crate::enums::owner_paths(self.f) {
            let Ok(ef) = crate::enums::read_enums_file(&p) else { continue };
            let eg = Gen::new(&ef.file, &ef.checked, &ef.src, &ef.path);
            let names = sw_top_names(&eg.sw_enums_module());
            for d in &ef.file.enums {
                if d.from.is_some() || !names.contains(&eg.enum_type(&d.name.text)) {
                    continue;
                }
                for e in &self.f.enums {
                    if e.from.is_some() || out.contains_key(&e.name.text) {
                        continue;
                    }
                    if self.enum_type(&e.name.text) != eg.enum_type(&d.name.text) {
                        continue;
                    }
                    // the same enum: every value as the two write it, in the same order — the
                    // type's raw values and the names of its cases follow
                    let same = e.values.len() == d.values.len()
                        && e.values.iter().zip(&d.values).all(|(a, b)| a.text == b.text && a.ascii == b.ascii);
                    if same {
                        out.insert(e.name.text.clone(), d.clone());
                    }
                }
            }
        }
        out
    }

    /// The names the module of the rule declares for the rule itself in `lang`: its function, the
    /// one that gives the rows as well, the one that writes the record, the one that counts the
    /// answers and the one that reads the caller's object; Java's class.
    pub(crate) fn rule_fn_names(&self, lang: &str) -> Vec<String> {
        let a = pub_name(&self.f.name);
        let chances = crate::codegen::chances::chances_fn_name(&a, lang);
        match lang {
            "go" => {
                let r = pascal(&a);
                vec![r.clone(), format!("{r}Traced"), format!("{r}Record"), chances]
            }
            "java" => vec![crate::codegen::java_class(&a)],
            _ => vec![a.clone(), format!("{a}_traced"), format!("{a}_record"), chances, format!("{a}_from")],
        }
    }

    /// Every name the code of `lang` makes of the type `t` of the enum `jp`, the type first: the
    /// functions of its order and its display names where it has them, its parser, Go's constants.
    fn made_names(&self, lang: &str, jp: &str, t: &str) -> Vec<String> {
        let (ordered, labeled) = (self.c.ordered.contains(jp), self.c.labels.contains_key(jp));
        let mut out = vec![t.to_string()];
        match lang {
            "python" | "ruby" | "php" => {
                if ordered {
                    out.push(format!("{}_rank", snake(t)));
                }
                if labeled {
                    out.push(format!("{}_label", snake(t)));
                }
            }
            "typescript" => {
                out.push(format!("parse{t}"));
                if ordered {
                    out.push(format!("{}Rank", lower_lead(t)));
                }
                if labeled {
                    out.push(format!("{}Label", lower_lead(t)));
                }
            }
            "go" => {
                for v in self.c.enums.get(jp).into_iter().flatten() {
                    let member = self.value_names.get(&(jp.to_string(), v.clone())).map(|(_, a)| a.clone()).unwrap_or_else(|| pascal(v));
                    out.push(format!("{t}{member}"));
                }
                out.push(format!("Parse{t}"));
            }
            _ => {}
        }
        out
    }

    /// The names the module of the rule declares for the enum `jp` in `lang`, were its type `t`:
    /// what it declares of an enum it declares, and what it hands on of one it reads from an
    /// `enums` file (§15.319). Ruby's and PHP's types are of another kind than their functions, so
    /// only the functions are counted there; Java names an enum of an `enums` file by its class.
    pub(crate) fn enum_decl_names(&self, lang: &str, jp: &str, t: &str) -> Vec<String> {
        let foreign = self.c.owners.contains_key(jp);
        let mut out = self.made_names(lang, jp, t);
        match lang {
            "ruby" | "php" => {
                out.remove(0);
            }
            // The type of an `enums` file the package names by its owner is not declared here
            // (§15.239).
            "go" if foreign && self.go_taken_names().iter().any(|n| n == t) => {
                out.remove(0);
            }
            "java" => {
                out = if foreign { Vec::new() } else { vec![crate::codegen::java_class(t)] };
            }
            _ => {}
        }
        out
    }

    /// The name the module of an `enums` file gives what this module calls `ours`, a name `lang`'s
    /// code makes of the enum `jp`: the same name, unless the module hands the enum on under
    /// another type (§15.326).
    pub(crate) fn theirs(&self, lang: &str, jp: &str, ours: &str) -> String {
        let (o, t) = (self.owner_type(jp), self.enum_type(jp));
        if o == t {
            return ours.to_string();
        }
        let (mine, them) = (self.made_names(lang, jp, &t), self.made_names(lang, jp, &o));
        mine.iter().position(|n| n == ours).map(|i| them[i].clone()).unwrap_or_else(|| ours.to_string())
    }

    /// The enums whose names meet the rule's own in `lang`, by their type, each with the type it
    /// takes there instead (§15.326).
    pub(crate) fn renamed_types(&self, lang: &str) -> BTreeMap<String, String> {
        if self.f.kind == FileKind::Enums {
            return BTreeMap::new();
        }
        let rule: HashSet<String> = self.rule_fn_names(lang).into_iter().collect();
        // each type the rule's enums have, once, by one of the names that have it
        let mut types: Vec<(String, String)> = Vec::new();
        for e in &self.f.enums {
            let t = self.enum_type(&e.name.text);
            if !types.iter().any(|(_, x)| *x == t) {
                types.push((e.name.text.clone(), t));
            }
        }
        let mut out: BTreeMap<String, String> = BTreeMap::new();
        for (i, (jp, t)) in types.iter().enumerate() {
            if !self.enum_decl_names(lang, jp, t).iter().any(|n| rule.contains(n)) {
                continue;
            }
            // what the other enums take, as they are named by now
            let mut taken = rule.clone();
            for (k, (other, ot)) in types.iter().enumerate() {
                if k != i {
                    taken.extend(self.enum_decl_names(lang, other, out.get(ot).unwrap_or(ot)));
                }
            }
            let new = (2usize..)
                .map(|k| format!("{t}{k}"))
                .find(|c| self.enum_decl_names(lang, jp, c).iter().all(|n| !taken.contains(n)))
                .expect("the numbers run out before the names do");
            out.insert(t.clone(), new);
        }
        out
    }

    /// The generator of the code of the backend `id`, when what it writes there is not what this one
    /// writes: the same generator, with the enums whose names meet the rule's own there under the
    /// types they take in that language (§15.326), and the rule's values and the generated code's
    /// own names written apart where they would meet (§15.336). None where nothing meets and this
    /// generator is every language's, so that the code is written as it always was.
    pub(crate) fn for_target(&self, id: &str) -> Option<Gen<'a>> {
        // this one is that backend's own already
        if self.made_for == Some(id) {
            return None;
        }
        let base = Gen::bare(self.f, self.c, &self.src, &self.path);
        let lang = speech(id);
        let renamed = lang.map(|l| base.renamed_types(l)).unwrap_or_default();
        let (binds, owns) = base.apart(id);
        // A generator another backend's own starts over from the generator of every language.
        if renamed.is_empty() && binds.is_empty() && owns.is_empty() && self.made_for.is_none() {
            return None;
        }
        let mut g = base;
        g.w114 = self.w114.clone();
        g.shown = self.shown.clone();
        g.package = self.package.clone();
        g.place = self.place.clone();
        g.python_package = self.python_package;
        g.made_for = crate::backend::ALL.iter().find(|b| b.id == id).map(|b| b.id);
        g.binds = binds;
        g.owns = owns;
        let declared: HashSet<&str> = self.f.enums.iter().map(|e| e.name.text.as_str()).collect();
        for (jp, t) in g.enum_names.iter_mut() {
            if let Some(n) = renamed.get(t).filter(|_| declared.contains(jp.as_str())) {
                g.renamed_from.insert(jp.clone(), t.clone());
                *t = n.clone();
            }
        }
        for ((jp, _), (t, _)) in g.value_names.iter_mut() {
            if let Some(n) = renamed.get(t).filter(|_| declared.contains(jp.as_str())) {
                *t = n.clone();
            }
        }
        Some(g)
    }

    /// What the code of the backend `id` writes apart from the rule's names (§15.336): each value its
    /// functions bind under another identifier than the one it writes it as, by name, and each name
    /// of its own it writes otherwise. A value of the rule's that would hide, inside the rule's
    /// functions, a name the generated code reads there — a helper, a type, a builtin, a local of its
    /// own — is bound under its identifier with `_` after it, as many as it takes to meet nothing
    /// (`Gen::temp`): only the code inside the functions reads the binding, and what a caller sees of
    /// the value keeps its identifier.
    fn apart(&self, id: &str) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
        use super::spellings::{Meets, Target};
        let mut binds = BTreeMap::new();
        let targets: &[Target] = match id {
            "python" => &[Target::Python, Target::Connect],
            "typescript" => &[Target::TypeScript],
            "javascript" => &[Target::JavaScript],
            "rust" => &[Target::Rust],
            "wasm" => &[Target::Wasm],
            "go" => &[Target::Go],
            "swift" => &[Target::Swift],
            "java" => &[Target::Java],
            "ruby" => &[Target::Ruby],
            _ => &[],
        };
        if self.f.kind != FileKind::Rule {
            // An `enums` file declares types and no function: its module reaches a builtin an
            // enum's type hides there past it, as a rule's module does, in the languages whose
            // module of the file reads one by its name (TypeScript and JavaScript, Rust, Swift).
            let owns = if self.f.kind == FileKind::Enums && matches!(id, "typescript" | "javascript" | "rust" | "swift") { self.hidden_builtins(id) } else { BTreeMap::new() };
            return (binds, owns);
        }
        let taken = super::spellings::taken(self.f, self.c);
        let mut owns = super::apart::owns(self, id, &taken);
        owns.extend(self.hidden_builtins(id));
        // The identifier each value is written as where the functions hold it: under the alias any
        // line of it gives it, else its name; a sequence and a shape, under their own.
        let names = super::value_names(self.f);
        let written = |n: &str| -> String {
            match self.idents.get(n) {
                Some(w) => w.clone(),
                None => names.iter().find(|(x, _)| x.text == n).map(|(x, _)| pub_name(x)).unwrap_or_else(|| n.to_string()),
            }
        };
        // Python's plain function keeps the answer in a local apart from its parameters itself
        // (`free_of`): the input is not bound apart for it.
        let local = |t: &super::spellings::Taken| matches!(t.target, Target::Python | Target::Connect) && matches!(t.meets, Meets::Read { what: super::own::What::Local, .. });
        // What each value is bound under instead, before it is made to meet nothing: a value that
        // hides a name the functions read, with `_` after it; in Ruby, a value whose identifier
        // starts with a capital letter, which is a constant there, with `_` before it.
        let mut wanted: Vec<(String, String)> = Vec::new();
        for t in taken.iter().filter(|t| targets.contains(&t.target) && matches!(t.meets, Meets::Read { .. }) && !local(t)) {
            if !wanted.iter().any(|(n, _)| *n == t.name) {
                wanted.push((t.name.clone(), format!("{}_", written(&t.name))));
            }
        }
        // `_` is no variable in Go, Rust, Swift and Java (the blank identifier, the pattern that binds
        // nothing, a keyword): a value of the name is bound as `__` where the functions hold it. A
        // parameter is a field of `in` in Go and is labelled by its name in Swift, which stay.
        if matches!(id, "go" | "rust" | "wasm" | "swift" | "java") {
            let passed = |n: &str| self.f.inputs.iter().any(|i| i.name.text == n) || self.f.elements.iter().any(|el| el.name.text == n);
            let fields: Vec<&str> = self.f.elements.iter().flat_map(|el| el.fields.iter().map(|fd| fd.name.text.as_str())).collect();
            let groups: Vec<&str> = self.f.groups.iter().map(|g| g.name.text.as_str()).chain(self.f.shapes.iter().map(|d| d.name.text.as_str())).collect();
            for (n, _) in &names {
                let t = n.text.as_str();
                if fields.contains(&t) || groups.contains(&t) || super::brought_in(t) || (matches!(id, "go" | "swift") && passed(t)) {
                    continue;
                }
                if written(t) == "_" && !wanted.iter().any(|(x, _)| x == t) {
                    wanted.push((t.to_string(), "__".to_string()));
                }
            }
        }
        if id == "ruby" {
            let held = names.iter().map(|(n, _)| *n);
            let fields: Vec<&str> = self.f.elements.iter().flat_map(|el| el.fields.iter().map(|fd| fd.name.text.as_str())).collect();
            let groups: Vec<&str> = self.f.groups.iter().map(|g| g.name.text.as_str()).collect();
            for n in held {
                if fields.contains(&n.text.as_str()) || groups.contains(&n.text.as_str()) || super::brought_in(&n.text) {
                    continue;
                }
                let w = written(&n.text);
                if w.chars().next().is_some_and(char::is_uppercase) && !wanted.iter().any(|(x, _)| *x == n.text) {
                    wanted.push((n.text.clone(), format!("_{w}")));
                }
            }
        }
        if wanted.is_empty() {
            return (binds, owns);
        }
        let t = targets[0];
        // a value's identifier as the language writes it where the functions hold it
        let spell = |id: &str| -> String {
            match t {
                Target::Swift => super::camel(id),
                Target::Java => super::java::java_name_pub(id),
                _ => id.to_string(),
            }
        };
        // every name the code of the language has there: the rule's, as it writes them, and its own
        let has = super::own::Has::of(self.f, self.c);
        let mut words: HashSet<String> = HashSet::new();
        for (n, _) in &names {
            words.insert(spell(&pub_name(n)));
            words.insert(spell(&self.written(&n.text)));
        }
        for e in &self.f.enums {
            words.insert(spell(&pub_name(&e.name)));
        }
        words.extend(super::own::reads(t, self.f, &has).into_iter().map(|o| o.name));
        words.extend(super::own::tops(t, &has).into_iter().map(|o| o.name));
        if let Some(b) = crate::backend::ALL.iter().find(|b| b.id == id) {
            words.extend(b.reserved.iter().chain(b.globals.iter()).map(|w| w.to_string()));
        }
        for (n, mut b) in wanted {
            // as many `_` after it as it takes, which every way a language spells a value keeps
            while words.contains(&spell(&b)) {
                b.push('_');
            }
            words.insert(spell(&b));
            binds.insert(n, b);
        }
        (binds, owns)
    }

    /// The builtins, and what Python's module imports, that a name of the rule's hides in the module
    /// the backend `id` writes, each with how the code reaches it past that name (§15.336): Python's
    /// through `builtins` and `typing` imported under names of their own (`_builtins.abs`,
    /// `_typing.NamedTuple`) and `enum` under one (`_enum`), TypeScript's and JavaScript's through
    /// `globalThis` (`globalThis.JSON`), Ruby's from the top (`::Struct`, `::Kernel.format`), Rust's
    /// by their paths (`::std::string::String`), Swift's through its module (`Swift.String`). The
    /// rule's function hides one where its name is one, and an enum's type where it is one
    /// ([`reaches_past`] says which). Python's modules imported so are kept under the keys
    /// `import builtins`, `import typing` and `import enum`, which no word is. Empty where nothing is
    /// hidden, as for most rules.
    fn hidden_builtins(&self, id: &str) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let rule = pub_name(&self.f.name);
        let types: Vec<String> = self.c.enums.keys().map(|jp| self.enum_type(jp)).collect();
        // each name of the rule's that hides a builtin, the function's first; an `enums` file has
        // no function
        let function = Some((&rule, false)).filter(|_| self.f.kind == FileKind::Rule);
        let hiding: Vec<(&String, bool)> = function.into_iter().chain(types.iter().map(|t| (t, true))).filter(|(w, ty)| reaches_past(id, w, *ty)).collect();
        match id {
            "python" | "numpy" => {
                // what a name of the module's own reads from `builtins`, from `typing`, and `enum`
                let mut from: Vec<(&str, String)> = Vec::new();
                let mut hides_enum = false;
                for (w, _) in &hiding {
                    match w.as_str() {
                        "annotations" => {
                            out.insert("annotations".to_string(), "_annotations".to_string());
                        }
                        "enum" => hides_enum = true,
                        w if PY_TYPING.contains(&w) => from.push(("typing", w.to_string())),
                        w => from.push(("builtins", w.to_string())),
                    }
                }
                if from.is_empty() && !hides_enum {
                    return out;
                }
                // the names a module of the rule's has where the code reads the modules so imported
                let mut taken: HashSet<String> = HashSet::new();
                for (n, _) in super::value_names(self.f) {
                    taken.insert(pub_name(n));
                    taken.insert(self.written(&n.text));
                }
                taken.insert(rule.clone());
                taken.extend(types.iter().cloned());
                let fresh = |base: &str| -> String {
                    let mut a = base.to_string();
                    while taken.contains(&a) {
                        a.push('_');
                    }
                    a
                };
                for m in ["builtins", "typing"] {
                    if from.iter().any(|(x, _)| *x == m) {
                        let a = fresh(&format!("_{m}"));
                        for (_, w) in from.iter().filter(|(x, _)| *x == m) {
                            out.insert(w.clone(), format!("{a}.{w}"));
                        }
                        out.insert(format!("import {m}"), a);
                    }
                }
                if hides_enum {
                    let a = fresh("_enum");
                    out.insert("enum".to_string(), a.clone());
                    out.insert("import enum".to_string(), a);
                }
            }
            "typescript" | "javascript" => {
                for (w, _) in hiding {
                    out.insert(w.clone(), format!("globalThis.{w}"));
                }
            }
            "ruby" => {
                for (w, ty) in hiding {
                    out.insert(w.clone(), if ty { format!("::{w}") } else { format!("::Kernel.{w}") });
                }
            }
            "rust" | "wasm" => {
                for (w, _) in hiding {
                    if let Some((_, path)) = RS_PATHS.iter().find(|(x, _)| x == w) {
                        out.insert(w.clone(), path.to_string());
                    }
                }
            }
            "swift" => {
                for (w, _) in hiding {
                    out.insert(w.clone(), format!("Swift.{w}"));
                }
            }
            _ => {}
        }
        out
    }

    /// The type of the enum `jp` in the module that declares it: for an enum of an `enums` file
    /// this module hands on under another name (§15.326), the name the file's module gives it.
    pub(crate) fn owner_type(&self, jp: &str) -> String {
        match self.renamed_from.get(jp) {
            Some(t) if self.c.owners.contains_key(jp) => t.clone(),
            _ => self.enum_type(jp),
        }
    }

    /// The type the `.proto` declares the enum `jp` as, which the generator of every language
    /// writes it as: the protobuf classes have that name where this language's code takes another
    /// (§15.326).
    pub(crate) fn proto_type(&self, jp: &str) -> String {
        self.renamed_from.get(jp).cloned().unwrap_or_else(|| self.enum_type(jp))
    }

    /// Whether the enum `jp`, of an `enums` file, is one this module hands on under another name
    /// than the file's (§15.326): the code that runs the module then finds its names in this module,
    /// as it finds those of the rule's own enums.
    pub(crate) fn handed_here(&self, jp: &str) -> bool {
        self.c.owners.contains_key(jp) && self.renamed_from.contains_key(jp)
    }
}
