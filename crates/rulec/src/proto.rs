//! Reading a `.proto` for the one thing a table depends on: which values an enum has (§15.59).
//!
//! The value set of an enum is usually not the rule's to decide. It is declared somewhere
//! else — a contract between systems, another file, often another repository — and it can
//! change without the rule changing. The tools that guard that contract guard its **shape**:
//! adding a value to an enum is a
//! compatible change, and on the wire it is. It is also the change that leaves a table with
//! no row for the new value, and the generated code answering from the default row for a tier
//! nobody priced. Nothing in the proto toolchain can see that, because the answer is not in
//! the proto.
//!
//! So a rule names where its enum comes from and `rulec check` holds the two together:
//!
//! ```text
//! import proto "api/v1/order.proto" MemberTier -> 会員区分
//! enum 会員区分(tier) = 一般(basic) | ゴールド(gold) | プラチナ(platinum) default
//! ```
//!
//! The `.proto` decides **which values exist**; the `.rule` decides **what they are called
//! here** and what each one costs. Neither can be derived from the other — a proto has no
//! Japanese in it, and a rule has no authority over the contract. What this module does is
//! read the set; holding the two together is `enums.rs`, which does it the same way for
//! every format.
//!
//! What is read is one production of the grammar: `enum <Name> { <VALUE> = <n>; … }`, wherever
//! it sits, nested in a message or not. Fields, services, options and imports are skipped. The
//! file is a contract, not a program, and the only part of it a table depends on is the set.


/// One value of an enum in a `.proto`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    /// As written, prefix and all: `MEMBER_TIER_GOLD`.
    pub name: String,
    pub number: i64,
}

/// One enum declared in a `.proto`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    pub name: String,
    pub values: Vec<Value>,
}

impl Enum {
    /// The ASCII aliases this enum asks a rule for, in declaration order.
    ///
    /// Two conventions are read, and only two. The value names carry the enum's own name as a
    /// prefix (`MemberTier` → `MEMBER_TIER_`), which `buf lint` enforces and which is there to
    /// keep C++ scoping from colliding; it is not part of the value. And the zero value is
    /// proto3's "not set", so `MEMBER_TIER_UNSPECIFIED` is not a value a table answers for —
    /// the generated code refuses it at the door like any other input that is not a member.
    ///
    /// A zero value named anything else is **not** dropped. A file that puts a real value at 0
    /// is unusual, and quietly deciding it means nothing is the one thing this must not do.
    pub fn aliases(&self) -> Vec<String> {
        let prefix = format!("{}_", upper_snake(&self.name));
        self.values
            .iter()
            .filter(|v| !(v.number == 0 && strip(&v.name, &prefix).eq_ignore_ascii_case("unspecified")))
            .map(|v| strip(&v.name, &prefix).to_ascii_lowercase())
            .collect()
    }
}

/// One field of a message, as far as a path needs it: its name, the word that names its
/// type, and whether it is a collection (§15.125).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    /// The type as written — `string`, `int64`, or a message name, qualified or not.
    pub ty: String,
    pub repeated: bool,
}

/// One message declared in a `.proto`, named as it is written. A nested message is listed
/// under its own short name as well, which is how a path names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub name: String,
    pub fields: Vec<Field>,
}

/// The kinds a proto scalar travels as. A rule's numbers are whole in their declared unit
/// (§2.1), so the floating kinds are named apart rather than folded in with the integers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scalar {
    Str,
    Int,
    Frac,
    Bool,
}

/// The kind a proto type word travels as, or `None` for a message or an enum. An enum is a
/// message-shaped name here and resolves to nothing, which is right: what a rule takes from
/// an enum is its value set (§15.59), and that is `import proto`'s business, not a path's.
pub fn scalar(ty: &str) -> Option<Scalar> {
    match ty.rsplit('.').next().unwrap_or(ty) {
        "string" => Some(Scalar::Str),
        "bool" => Some(Scalar::Bool),
        "double" | "float" => Some(Scalar::Frac),
        "int32" | "int64" | "uint32" | "uint64" | "sint32" | "sint64" | "fixed32" | "fixed64"
        | "sfixed32" | "sfixed64" => Some(Scalar::Int),
        // `bytes` is not a kind a rule's input can be, and saying so is better than calling
        // it a string: the two do not travel the same way.
        _ => None,
    }
}

/// Whether a message name written one way is the one written another. A `.proto` may name a
/// message bare, package-qualified or nested-qualified, and a `shape` line may name it any
/// of those ways; the last segment is what they always agree on.
pub fn same_message(declared: &str, wanted: &str) -> bool {
    let last = |s: &str| s.rsplit('.').next().unwrap_or(s).to_string();
    declared == wanted || last(declared) == last(wanted)
}

/// Every message in the file, in the order they appear, nested ones included.
///
/// Read far enough for a path and no further: a field is `[repeated] <type> <name> = <n>;`,
/// and `oneof`, `map`, `option`, `reserved` and `extend` are skipped. A `map` field is
/// skipped rather than guessed at, so a path into one gets stuck instead of being waved
/// through.
pub fn messages(src: &str) -> Vec<Message> {
    let b: Vec<char> = strip_comments(src).chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !word_starts_at(&b, i, "message") {
            i += 1;
            continue;
        }
        let j = skip_ws(&b, i + 7);
        let (name, k) = ident(&b, j);
        let j = skip_ws(&b, k);
        if name.is_empty() || b.get(j) != Some(&'{') {
            i += 1;
            continue;
        }
        out.push(Message { name, fields: fields_of(&b, j + 1) });
        // A nested message is found by the same walk, so the cursor only steps past `{`.
        i = j + 1;
    }
    out
}

/// The fields between `{` and its `}`, the bodies of anything nested skipped over.
fn fields_of(b: &[char], from: usize) -> Vec<Field> {
    let mut out = Vec::new();
    let mut stmt = String::new();
    let mut i = from;
    let mut depth = 0usize;
    while i < b.len() {
        match b[i] {
            '{' => {
                // A nested message, enum or oneof. Its own fields belong to it, not here.
                depth += 1;
                stmt.clear();
                i += 1;
            }
            '}' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                i += 1;
            }
            '[' => {
                while i < b.len() && b[i] != ']' {
                    i += 1;
                }
                i += 1;
            }
            ';' => {
                if depth == 0 {
                    if let Some(f) = field_of(&stmt) {
                        out.push(f);
                    }
                }
                stmt.clear();
                i += 1;
            }
            c => {
                stmt.push(c);
                i += 1;
            }
        }
    }
    out
}

/// `repeated Line lines = 3` → a field. Anything that is not three words and a number is
/// not one.
fn field_of(stmt: &str) -> Option<Field> {
    let s = stmt.trim();
    let mut w: Vec<&str> = s.split_whitespace().collect();
    let repeated = w.first() == Some(&"repeated");
    if repeated || w.first() == Some(&"optional") {
        w.remove(0);
    }
    // `map<string, Line> by_id = 1` is not read: its shape is not a path's to guess.
    if w.len() < 4 || w[2] != "=" || w[0].starts_with("map<") {
        return None;
    }
    if matches!(w[0], "option" | "reserved" | "extensions" | "import" | "package" | "syntax") {
        return None;
    }
    w[3].trim_end_matches(';').parse::<i64>().ok()?;
    Some(Field { name: w[1].to_string(), ty: w[0].to_string(), repeated })
}

fn strip<'a>(name: &'a str, prefix: &str) -> &'a str {
    name.strip_prefix(prefix).unwrap_or(name)
}

/// `MemberTier` → `MEMBER_TIER`. The value names of an enum are spelled this way by
/// convention, so this is how the prefix to take off is found.
pub fn upper_snake(camel: &str) -> String {
    let mut out = String::new();
    for (i, ch) in camel.chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 && !out.ends_with('_') {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

/// The `package` the file declares, when it declares one.
///
/// The value set is all a rule needs, but a `.proto` written **from** a rule needs one thing
/// more: an imported enum is named by its package there (§15.112), and a file with no package
/// names its enum bare.
pub fn package(src: &str) -> Option<String> {
    for line in strip_comments(src).lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("package ") {
            let name = rest.trim().trim_end_matches(';').trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Every enum in the file, in the order they appear.
pub fn enums(src: &str) -> Vec<Enum> {
    let b: Vec<char> = strip_comments(src).chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !word_starts_at(&b, i, "enum") {
            i += 1;
            continue;
        }
        let mut j = skip_ws(&b, i + 4);
        let (name, k) = ident(&b, j);
        if name.is_empty() {
            i += 1;
            continue;
        }
        j = skip_ws(&b, k);
        if b.get(j) != Some(&'{') {
            i += 1;
            continue;
        }
        let (values, end) = body(&b, j + 1);
        out.push(Enum { name, values });
        i = end;
    }
    out
}

/// The values between `{` and its `}`. Statements are separated by `;`; `option` and
/// `reserved` are not values, and a `[deprecated = true]` suffix is not part of one.
fn body(b: &[char], from: usize) -> (Vec<Value>, usize) {
    let mut values = Vec::new();
    let mut stmt = String::new();
    let mut i = from;
    while i < b.len() {
        match b[i] {
            '}' => {
                i += 1;
                break;
            }
            ';' => {
                if let Some(v) = value_of(&stmt) {
                    values.push(v);
                }
                stmt.clear();
                i += 1;
            }
            '[' => {
                // Field options belong to the value but say nothing about its number.
                while i < b.len() && b[i] != ']' {
                    i += 1;
                }
                i += 1;
            }
            c => {
                stmt.push(c);
                i += 1;
            }
        }
    }
    (values, i)
}

fn value_of(stmt: &str) -> Option<Value> {
    let s = stmt.trim();
    if s.is_empty() {
        return None;
    }
    let (name, rest) = s.split_once('=')?;
    let name = name.trim();
    if name.is_empty() || name.contains(char::is_whitespace) || name == "option" {
        return None;
    }
    Some(Value { name: name.to_string(), number: rest.trim().parse().ok()? })
}

/// Comments out, strings kept whole. A `//` inside a string literal is not a comment, and an
/// unterminated block comment eats the rest of the file the way protoc reads it.
fn strip_comments(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        match (b[i], b.get(i + 1)) {
            ('/', Some('/')) => {
                while i < b.len() && b[i] != '\n' {
                    i += 1;
                }
            }
            ('/', Some('*')) => {
                i += 2;
                while i < b.len() && !(b[i] == '*' && b.get(i + 1) == Some(&'/')) {
                    i += 1;
                }
                i = (i + 2).min(b.len());
            }
            ('"', _) | ('\'', _) => {
                let q = b[i];
                out.push(b[i]);
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                // The text of a string is not read; keeping the quotes keeps statements apart.
                out.push(q);
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

fn word_starts_at(b: &[char], i: usize, w: &str) -> bool {
    let ws: Vec<char> = w.chars().collect();
    if i + ws.len() > b.len() || b[i..i + ws.len()] != ws[..] {
        return false;
    }
    let before = i == 0 || !is_ident(b[i - 1]);
    let after = b.get(i + ws.len()).is_none_or(|c| !is_ident(*c));
    before && after
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.'
}

fn ident(b: &[char], from: usize) -> (String, usize) {
    let mut i = from;
    let mut s = String::new();
    while i < b.len() && is_ident(b[i]) {
        s.push(b[i]);
        i += 1;
    }
    (s, i)
}

fn skip_ws(b: &[char], from: usize) -> usize {
    let mut i = from;
    while i < b.len() && b[i].is_whitespace() {
        i += 1;
    }
    i
}
