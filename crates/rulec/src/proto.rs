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
//! What is read is one production of the grammar for that: `enum <Name> { <VALUE> = <n>; … }`,
//! wherever it sits, nested in a message or not. Two readers were added beside it later, each
//! for one question a rule asks of the contract: the messages and their fields, as far as a
//! `from` path needs them (§15.125), and on a field, the Protovalidate rules that bound which
//! values pass (§15.132). The rules that relate fields to each other — CEL on a message or a
//! field, a `oneof` — are read as text here and made into a condition by `cel` and
//! `projection` (§15.140). Services, imports and every other option are skipped. The file is a
//! contract, not a program.


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
    /// Written `optional`: proto3's explicit presence, where an unset field is absent rather
    /// than zero.
    pub optional: bool,
    /// What Protovalidate lets through here, as far as it was read (§15.132).
    pub rules: Rules,
    /// The `json_name` option, when the field sets one (§15.133).
    pub json_name: Option<String>,
    /// The `oneof` it is a member of. A member has presence of its own, like an `optional`
    /// field, and `optional` is set on it too (§15.140).
    pub oneof: Option<String>,
}

impl Field {
    /// The name protojson writes this field under: its `json_name`, or the lowerCamelCase of
    /// its name (§15.133). Readers of protojson accept the name as written too.
    pub fn json(&self) -> String {
        self.json_name.clone().unwrap_or_else(|| json_name(&self.name))
    }
}

/// protobuf's own rule for a field's JSON name: an underscore is dropped and the letter after
/// it is capitalised, so `weight_g` is `weightG` and `order_lines` is `orderLines`.
pub fn json_name(field: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in field.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// The Protovalidate rules on one field that decide which values pass: `(buf.validate.field)`
/// in the field's options, in either spelling — `.int64.gte = 1` or `.int64 = {gte: 1}`
/// (§15.132).
///
/// Only the rules that bound a value the way a rule's input is bounded are read: the integer
/// comparisons, the element count of a collection, the listed values of a string, and the CEL
/// expressions, which are kept as text for `cel` to read (§15.140). Anything else that could
/// narrow what passes — a predefined rule, a pattern — is recorded by name in `unread` and not
/// interpreted. Reading it as not there makes the field
/// look wider than it is, never narrower: the check that uses this may then speak where it
/// did not need to, and never stays quiet where it should have spoken.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rules {
    pub required: bool,
    /// `IGNORE_IF_ZERO_VALUE` and its older names, or `IGNORE_ALWAYS`.
    pub ignore: Option<String>,
    pub int: IntRules,
    pub min_items: Option<i128>,
    pub max_items: Option<i128>,
    pub str_const: Option<String>,
    pub str_in: Vec<String>,
    pub str_not_in: Vec<String>,
    /// The least length the string rules ask for (`min_len`, `len`, and the two in bytes):
    /// what a date needs to know of them, which is whether "" passes (§15.133).
    pub str_min_len: Option<i128>,
    /// The CEL expressions on the field, from `cel` and `cel_expression`, `this` being the
    /// field. They are read by `cel` where the rule is compared (§15.140).
    pub cel: Vec<String>,
    pub unread: Vec<String>,
}

/// The integer rules, whichever of the ten integer kinds they were written under.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IntRules {
    pub konst: Option<i128>,
    pub gt: Option<i128>,
    pub gte: Option<i128>,
    pub lt: Option<i128>,
    pub lte: Option<i128>,
    pub in_: Vec<i128>,
    pub not_in: Vec<i128>,
}

/// The ten integer kinds of a `.proto`, which are also the names Protovalidate files their
/// rules under.
const INT_KINDS: &[&str] =
    &["int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32", "fixed64", "sfixed32", "sfixed64"];

/// The values an integer kind can hold on the wire.
pub fn int_bounds(ty: &str) -> Option<(i128, i128)> {
    match ty.rsplit('.').next().unwrap_or(ty) {
        "int32" | "sint32" | "sfixed32" => Some((i32::MIN as i128, i32::MAX as i128)),
        "uint32" | "fixed32" => Some((0, u32::MAX as i128)),
        "int64" | "sint64" | "sfixed64" => Some((i64::MIN as i128, i64::MAX as i128)),
        "uint64" | "fixed64" => Some((0, u64::MAX as i128)),
        _ => None,
    }
}

/// One message declared in a `.proto`, named as it is written. A nested message is listed
/// under its own short name as well, which is how a path names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub name: String,
    pub fields: Vec<Field>,
    /// What Protovalidate asks of the message as a whole (§15.140).
    pub rules: MsgRules,
}

/// The rules on a message rather than on one field: `(buf.validate.message)` in its options,
/// and the `oneof`s it declares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MsgRules {
    /// The CEL expressions on the message, from `cel` and `cel_expression`, `this` being the
    /// message.
    pub cel: Vec<String>,
    /// The groups of fields of which at most one may be set: every `oneof` the message
    /// declares, and every `(buf.validate.message).oneof`. `required` asks for exactly one.
    pub oneofs: Vec<Oneof>,
    /// `(buf.validate.message).disabled`. Protovalidate has since removed it; where a release
    /// that still reads it validates the message, it validates nothing, so every rule of the
    /// message is dropped rather than trusted.
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Oneof {
    pub fields: Vec<String>,
    pub required: bool,
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
/// the members of a `oneof` are fields of the message it is in, and `map`, `reserved` and
/// `extend` are skipped. A `map` field is skipped rather than guessed at, so a path into one
/// gets stuck instead of being waved through. Of the options on a message, only Protovalidate's
/// are read.
pub fn messages(src: &str) -> Vec<Message> {
    let (text, strs) = lex_strings(src);
    let b: Vec<char> = text.chars().collect();
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
        let (fields, rules) = body_of(&b, j + 1, &strs);
        out.push(Message { name, fields, rules });
        // A nested message is found by the same walk, so the cursor only steps past `{`.
        i = j + 1;
    }
    out
}

/// The fields between `{` and its `}` and the rules on the message, the bodies of anything
/// nested skipped over — except a `oneof`, whose members are fields of this message.
fn body_of(b: &[char], from: usize, strs: &[String]) -> (Vec<Field>, MsgRules) {
    let mut out: Vec<Field> = Vec::new();
    let mut rules = MsgRules::default();
    let mut stmt = String::new();
    let mut opts = String::new();
    let mut i = from;
    let mut depth = 0usize;
    // The `oneof` the walk is inside, with the members found so far.
    let mut oneof: Option<(String, Oneof)> = None;
    while i < b.len() {
        let here = depth == 0 || (depth == 1 && oneof.is_some());
        // `option … ;` on the message or on a oneof: read whole, braces and all.
        if here && stmt.trim().is_empty() && word_starts_at(b, i, "option") {
            let (text, next) = statement(b, i + 6);
            option_of(&text, strs, &mut rules, oneof.as_mut().map(|(_, o)| o));
            stmt.clear();
            opts.clear();
            i = next;
            continue;
        }
        match b[i] {
            '{' => {
                // A nested message or enum, whose fields are its own; or a oneof, whose are not.
                let w: Vec<&str> = stmt.split_whitespace().collect();
                if depth == 0 && w.len() == 2 && w[0] == "oneof" {
                    oneof = Some((w[1].to_string(), Oneof { fields: Vec::new(), required: false }));
                }
                depth += 1;
                stmt.clear();
                opts.clear();
                i += 1;
            }
            '}' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                if depth == 0 {
                    if let Some((_, o)) = oneof.take() {
                        rules.oneofs.push(o);
                    }
                }
                stmt.clear();
                i += 1;
            }
            '[' => {
                // The field's options, which may hold lists of their own (`in: [1, 2]`): the
                // `]` that closes them is the one that brings the nesting back to zero.
                let (inner, next) = bracketed(b, i);
                if here {
                    opts = inner;
                }
                i = next;
            }
            ';' => {
                if here {
                    if let Some(mut f) = field_of(&stmt) {
                        f.rules = rules_of(&opts, strs);
                        f.json_name = option_list(&opts, strs).into_iter().find_map(|(n, v)| match v {
                            Tv::Str(s) if n == "json_name" => Some(s),
                            _ => None,
                        });
                        if let Some((name, o)) = oneof.as_mut() {
                            f.optional = true;
                            f.oneof = Some(name.clone());
                            o.fields.push(f.name.clone());
                        }
                        out.push(f);
                    }
                }
                stmt.clear();
                opts.clear();
                i += 1;
            }
            c => {
                stmt.push(c);
                i += 1;
            }
        }
    }
    if rules.disabled {
        for f in &mut out {
            f.rules = Rules { unread: vec!["(buf.validate.message).disabled".into()], ..Rules::default() };
        }
        rules.cel.clear();
        // A `oneof` of the file is the wire format's, and holds whatever validation says.
        rules.oneofs.retain(|o| o.fields.iter().all(|n| out.iter().any(|f| f.name == *n && f.oneof.is_some())));
        for o in &mut rules.oneofs {
            o.required = false;
        }
    }
    (out, rules)
}

/// The text of a statement from `from` to its `;`, braces and brackets inside it skipped
/// over, and the position after the `;`.
fn statement(b: &[char], from: usize) -> (String, usize) {
    let mut depth = 0i32;
    let mut i = from;
    while i < b.len() {
        match b[i] {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            ';' if depth <= 0 => return (b[from..i].iter().collect(), i + 1),
            _ => {}
        }
        i += 1;
    }
    (b[from.min(b.len())..].iter().collect(), b.len())
}

/// One `option` of a message or a oneof: what Protovalidate asks of it, the rest skipped.
fn option_of(text: &str, strs: &[String], rules: &mut MsgRules, oneof: Option<&mut Oneof>) {
    const MSG: &str = "(buf.validate.message)";
    const ONEOF: &str = "(buf.validate.oneof)";
    let mut oneof = oneof;
    for (name, v) in option_list(text, strs) {
        if let Some(rest) = name.strip_prefix(MSG) {
            let path: Vec<&str> = rest.split('.').filter(|s| !s.is_empty()).collect();
            message_rule(&path, &v, rules);
        } else if let Some(rest) = name.strip_prefix(ONEOF) {
            let required = match (rest.trim_start_matches('.'), &v) {
                ("required", Tv::Id(t)) => t == "true",
                ("", Tv::Msg(kv)) => kv.iter().any(|(k, v)| k == "required" && matches!(v, Tv::Id(t) if t == "true")),
                _ => false,
            };
            if let Some(o) = oneof.as_deref_mut() {
                o.required |= required;
            }
        }
    }
}

/// One part of `(buf.validate.message)`, in any of the ways text format lets it be written.
fn message_rule(path: &[&str], v: &Tv, rules: &mut MsgRules) {
    let expression = |kv: &[(String, Tv)]| {
        kv.iter().find_map(|(k, v)| match v {
            Tv::Str(s) if k == "expression" => Some(s.clone()),
            _ => None,
        })
    };
    match (path, v) {
        ([], Tv::Msg(kv)) => {
            for (k, v) in kv {
                message_rule(&[k.as_str()], v, rules);
            }
        }
        (["cel"], Tv::Msg(kv)) => rules.cel.extend(expression(kv)),
        (["cel", "expression"], Tv::Str(s)) => rules.cel.push(s.clone()),
        (["cel_expression"], Tv::Str(s)) => rules.cel.push(s.clone()),
        (["cel" | "cel_expression" | "oneof"], Tv::List(xs)) => {
            for x in xs {
                message_rule(path, x, rules);
            }
        }
        (["oneof"], Tv::Msg(kv)) => {
            let fields = kv.iter().filter(|(k, _)| k == "fields").flat_map(|(_, v)| strs_of(v)).collect();
            let required = kv.iter().any(|(k, v)| k == "required" && matches!(v, Tv::Id(t) if t == "true"));
            rules.oneofs.push(Oneof { fields, required });
        }
        (["disabled"], Tv::Id(t)) => rules.disabled |= t == "true",
        _ => {}
    }
}

/// The text between the `[` at `open` and the `]` that matches it, and the position after
/// that `]`. Strings are placeholders by now (`lex_strings`), so a bracket inside one is not seen.
fn bracketed(b: &[char], open: usize) -> (String, usize) {
    let mut depth = 0usize;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return (b[open + 1..i].iter().collect(), i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    (b[(open + 1).min(b.len())..].iter().collect(), b.len())
}

/// `repeated Line lines = 3` → a field. Anything that is not three words and a number is
/// not one.
fn field_of(stmt: &str) -> Option<Field> {
    let s = stmt.trim();
    let mut w: Vec<&str> = s.split_whitespace().collect();
    let repeated = w.first() == Some(&"repeated");
    let optional = w.first() == Some(&"optional");
    if repeated || optional {
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
    Some(Field { name: w[1].to_string(), ty: w[0].to_string(), repeated, optional, rules: Rules::default(), json_name: None, oneof: None })
}

// --- Protovalidate, as far as a value's bounds go (§15.132) -------------------------------

/// A value in the text format an option is written in: `1`, `"a"`, `IGNORE_ALWAYS`,
/// `[1, 2]`, `{gte: 1, lte: 5}`.
#[derive(Debug, Clone)]
enum Tv {
    Num(String),
    Str(String),
    Id(String),
    List(Vec<Tv>),
    Msg(Vec<(String, Tv)>),
}

struct Cur<'a> {
    b: &'a [char],
    i: usize,
    strs: &'a [String],
}

impl Cur<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.b.get(self.i).copied()
    }

    fn word(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || matches!(c, '_' | '.' | '-' | '+') {
                s.push(c);
                self.i += 1;
            } else {
                break;
            }
        }
        s
    }

    /// A key of a message value: a field name, or `[an.extension]`.
    fn key(&mut self) -> Option<String> {
        self.ws();
        if self.peek() == Some('[') {
            let (inner, next) = bracketed(self.b, self.i);
            self.i = next;
            return Some(format!("[{inner}]"));
        }
        let w = self.word();
        (!w.is_empty()).then_some(w)
    }

    fn value(&mut self) -> Option<Tv> {
        self.ws();
        match self.peek()? {
            '{' => {
                self.i += 1;
                let mut kv = Vec::new();
                loop {
                    self.ws();
                    match self.peek()? {
                        '}' => {
                            self.i += 1;
                            break;
                        }
                        ',' | ';' => {
                            self.i += 1;
                            continue;
                        }
                        _ => {}
                    }
                    let k = self.key()?;
                    self.ws();
                    if self.peek() == Some(':') {
                        self.i += 1;
                    }
                    kv.push((k, self.value()?));
                }
                Some(Tv::Msg(kv))
            }
            '[' => {
                self.i += 1;
                let mut vs = Vec::new();
                loop {
                    self.ws();
                    match self.peek()? {
                        ']' => {
                            self.i += 1;
                            break;
                        }
                        ',' => {
                            self.i += 1;
                            continue;
                        }
                        _ => {}
                    }
                    vs.push(self.value()?);
                }
                Some(Tv::List(vs))
            }
            q @ ('"' | '\'') => {
                self.i += 1;
                let mut n = String::new();
                while let Some(c) = self.peek() {
                    self.i += 1;
                    if c == q {
                        break;
                    }
                    n.push(c);
                }
                Some(Tv::Str(self.strs.get(n.parse::<usize>().ok()?)?.clone()))
            }
            _ => {
                let w = self.word();
                if w.is_empty() {
                    None
                } else if w.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '-' | '+' | '.')) {
                    Some(Tv::Num(w))
                } else {
                    Some(Tv::Id(w))
                }
            }
        }
    }
}

/// The options of one field as `(name, value)` pairs, in the order written. A pair that
/// does not read is skipped to the next comma rather than taking the rest down with it.
fn option_list(text: &str, strs: &[String]) -> Vec<(String, Tv)> {
    let b: Vec<char> = text.chars().collect();
    let mut c = Cur { b: &b, i: 0, strs };
    let mut out = Vec::new();
    let skip_to_comma = |c: &mut Cur| {
        let mut depth = 0i32;
        while let Some(ch) = c.peek() {
            c.i += 1;
            match ch {
                '{' | '[' | '(' => depth += 1,
                '}' | ']' | ')' => depth -= 1,
                ',' if depth <= 0 => break,
                _ => {}
            }
        }
    };
    loop {
        c.ws();
        if c.i >= b.len() {
            break;
        }
        let start = c.i;
        let mut depth = 0i32;
        while let Some(ch) = c.peek() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                '=' | ',' if depth == 0 => break,
                _ => {}
            }
            c.i += 1;
        }
        let name: String = b[start..c.i].iter().filter(|ch| !ch.is_whitespace()).collect();
        if c.peek() != Some('=') {
            skip_to_comma(&mut c);
            continue;
        }
        c.i += 1;
        match c.value() {
            Some(v) => {
                out.push((name, v));
                c.ws();
                if c.peek() == Some(',') {
                    c.i += 1;
                }
            }
            None => skip_to_comma(&mut c),
        }
    }
    out
}

fn flatten(path: Vec<String>, v: Tv, out: &mut Vec<(Vec<String>, Tv)>) {
    match v {
        Tv::Msg(kv) => {
            for (k, v) in kv {
                let mut p = path.clone();
                p.push(k);
                flatten(p, v, out);
            }
        }
        other => out.push((path, other)),
    }
}

fn int_of(v: &Tv) -> Option<i128> {
    let Tv::Num(s) = v else { return None };
    let (neg, digits) = match s.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let n = match digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) {
        Some(h) => i128::from_str_radix(h, 16).ok()?,
        // `1.0` is an integer written as a float; `1.5` bounds nothing an integer can be.
        None => match digits.split_once('.') {
            Some((w, f)) if f.chars().all(|c| c == '0') => w.parse().ok()?,
            Some(_) => return None,
            None => digits.parse().ok()?,
        },
    };
    Some(if neg { -n } else { n })
}

fn ints_of(v: &Tv) -> Vec<i128> {
    match v {
        Tv::List(vs) => vs.iter().filter_map(int_of).collect(),
        one => int_of(one).into_iter().collect(),
    }
}

fn strs_of(v: &Tv) -> Vec<String> {
    match v {
        Tv::List(vs) => vs.iter().filter_map(|x| if let Tv::Str(s) = x { Some(s.clone()) } else { None }).collect(),
        Tv::Str(s) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// What `(buf.validate.field)` says about one field. The text is the inside of the field's
/// `[...]`, with its strings replaced by the placeholders `lex_strings` leaves; `strs` holds them.
fn rules_of(text: &str, strs: &[String]) -> Rules {
    const FIELD: &str = "(buf.validate.field)";
    let mut r = Rules::default();
    if text.trim().is_empty() {
        return r;
    }
    let mut leaves = Vec::new();
    for (name, v) in option_list(text, strs) {
        let Some(rest) = name.strip_prefix(FIELD) else { continue };
        if rest.contains('(') {
            // `(buf.validate.field).int64.(my.rule) = …`: a predefined rule.
            r.unread.push(rest.trim_start_matches('.').to_string());
            continue;
        }
        let base: Vec<String> = rest.split('.').filter(|s| !s.is_empty()).map(String::from).collect();
        flatten(base, v, &mut leaves);
    }
    for (path, v) in leaves {
        let p: Vec<&str> = path.iter().map(String::as_str).collect();
        match p.as_slice() {
            ["required"] => r.required = matches!(&v, Tv::Id(t) if t == "true"),
            ["ignore"] => r.ignore = if let Tv::Id(t) = &v { Some(t.clone()) } else { None },
            [k, rule] if INT_KINDS.contains(k) => match *rule {
                "const" => r.int.konst = int_of(&v),
                "gt" => r.int.gt = int_of(&v),
                "gte" => r.int.gte = int_of(&v),
                "lt" => r.int.lt = int_of(&v),
                "lte" => r.int.lte = int_of(&v),
                "in" => r.int.in_.extend(ints_of(&v)),
                "not_in" => r.int.not_in.extend(ints_of(&v)),
                "example" => {}
                _ => r.unread.push(path.join(".")),
            },
            ["repeated", "min_items"] => r.min_items = int_of(&v),
            ["repeated", "max_items"] => r.max_items = int_of(&v),
            // `unique` and the rules on each element say nothing about how many there are.
            ["repeated", ..] => {}
            ["string", "const"] => r.str_const = strs_of(&v).into_iter().next(),
            ["string", "in"] => r.str_in.extend(strs_of(&v)),
            ["string", "not_in"] => r.str_not_in.extend(strs_of(&v)),
            ["string", "example"] => {}
            ["string", "min_len" | "len" | "min_bytes" | "len_bytes"] => {
                r.str_min_len = r.str_min_len.max(int_of(&v));
                r.unread.push(path.join("."));
            }
            ["string", ..] => r.unread.push(path.join(".")),
            ["cel", "expression"] | ["cel_expression"] => r.cel.extend(strs_of(&v)),
            ["cel"] => {
                if let Tv::List(xs) = &v {
                    for x in xs {
                        if let Tv::Msg(kv) = x {
                            r.cel.extend(kv.iter().filter(|(k, _)| k == "expression").flat_map(|(_, v)| strs_of(v)));
                        }
                    }
                }
            }
            ["cel", ..] => {}
            _ if p.iter().any(|s| s.starts_with('[')) => r.unread.push(path.join(".")),
            // The rules of the other kinds (`double`, `timestamp`, `map`, `enum`, …) bound
            // nothing this reader compares.
            _ => {}
        }
    }
    r
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
                i = bracketed(b, i).1;
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

/// Comments out. A `//` inside a string literal is not a comment, and an unterminated block
/// comment eats the rest of the file the way protoc reads it.
fn strip_comments(src: &str) -> String {
    lex_strings(src).0
}

/// Comments out, and every string literal replaced by its number in the list returned beside
/// the text: `"honshu"` becomes `"0"`. The statements are split on `;`, `{` and `[`, and none of
/// those may be taken from inside a string; the options that list values (`in: ["a", "b"]`)
/// read the strings back from the list (§15.132).
fn lex_strings(src: &str) -> (String, Vec<String>) {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::new();
    let mut strs = Vec::new();
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
                i += 1;
                let mut s = String::new();
                while i < b.len() && b[i] != q {
                    if b[i] == '\\' && i + 1 < b.len() {
                        i += 1;
                        s.push(match b[i] {
                            'n' => '\n',
                            't' => '\t',
                            c => c,
                        });
                    } else {
                        s.push(b[i]);
                    }
                    i += 1;
                }
                i += 1;
                out.push(q);
                out.push_str(&strs.len().to_string());
                out.push(q);
                strs.push(s);
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    (out, strs)
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

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: &str = r#"
syntax = "proto3";
package shop.v1;

message Order {
  // The two spellings Protovalidate accepts, one per field.
  int64 weight_g = 1 [(buf.validate.field).int64 = {gte: 1, lte: 40000}];
  int64 total_jpy = 2 [
    (buf.validate.field).int64.gte = 0,  // a comment inside the options
    (buf.validate.field).int64.lte = 10000000
  ];
  // A list inside the options: the `]` of `[1, 2]` is not the end of them.
  repeated Line lines = 3 [(buf.validate.field).repeated = {min_items: 1, max_items: 50, items: {message: {required: true}}}];
  string zone = 4 [(buf.validate.field).string = {in: ["honshu", "hokkaido; okinawa"]}];
  int32 tier = 5 [(buf.validate.field).required = true, (buf.validate.field).ignore = IGNORE_IF_ZERO_VALUE, (buf.validate.field).cel = {id: "x", expression: "this > 0 && this != 7"}];
  string note = 6 [json_name = "memo", deprecated = true];
  sint32 delta = 7 [(buf.validate.field).sint32 = {gte: -5, lte: 0x10, not_in: [3, 4]}];
  int64 custom = 8 [(buf.validate.field).int64.(my.rule) = 3];
  optional int64 coupon = 9;
}

message Line {
  int64 amount = 1 [(buf.validate.field).int64 = {in: [100, 200]}];
}
"#;

    fn field<'a>(ms: &'a [Message], m: &str, f: &str) -> &'a Field {
        ms.iter().find(|x| x.name == m).and_then(|x| x.fields.iter().find(|y| y.name == f)).unwrap()
    }

    #[test]
    fn 注釈の二通りの書き方をどちらも読む() {
        let ms = messages(ORDER);
        let w = &field(&ms, "Order", "weight_g").rules.int;
        assert_eq!((w.gte, w.lte), (Some(1), Some(40000)));
        let t = &field(&ms, "Order", "total_jpy").rules.int;
        assert_eq!((t.gte, t.lte), (Some(0), Some(10_000_000)));
    }

    #[test]
    fn 入れ子のリストの後ろのフィールドも読む() {
        let ms = messages(ORDER);
        let l = &field(&ms, "Order", "lines");
        assert!(l.repeated);
        assert_eq!((l.rules.min_items, l.rules.max_items), (Some(1), Some(50)));
        // Before the bracket was matched by nesting, `]` of an inner list ended the options
        // and the `}` after it ended the message: every field below was lost.
        assert_eq!(ms.iter().find(|m| m.name == "Order").unwrap().fields.len(), 9);
        assert_eq!(field(&ms, "Line", "amount").rules.int.in_, vec![100, 200]);
    }

    #[test]
    fn 文字列の中の区切りで文が切れない() {
        let ms = messages(ORDER);
        assert_eq!(field(&ms, "Order", "zone").rules.str_in, vec!["honshu".to_string(), "hokkaido; okinawa".to_string()]);
    }

    #[test]
    fn 読まなかった規則は名前を残す() {
        let ms = messages(ORDER);
        let r = &field(&ms, "Order", "tier").rules;
        assert!(r.required);
        assert_eq!(r.ignore.as_deref(), Some("IGNORE_IF_ZERO_VALUE"));
        // CEL is kept as text for `cel` to read (§15.140), and is no longer an unread rule.
        assert_eq!(r.cel, vec!["this > 0 && this != 7".to_string()]);
        assert!(r.unread.is_empty());
        assert_eq!(field(&ms, "Order", "custom").rules.unread, vec!["int64.(my.rule)".to_string()]);
    }

    const RULES: &str = r#"
syntax = "proto3";
package shop.v1;

message Quote {
  option (buf.validate.message).cel = {
    id: "weight_order",
    message: "min must not exceed max",
    expression: "this.min_weight <= this.max_weight"
  };
  option (buf.validate.message).cel_expression = "this.max_weight <= 30000";
  option (buf.validate.message).oneof = {fields: ["coupon", "points"], required: true};
  option deprecated = true;

  int64 min_weight = 1 [(buf.validate.field).cel_expression = "this >= 1"];
  int64 max_weight = 2 [(buf.validate.field) = {cel: [{id: "a", expression: "this >= 1"}, {id: "b", expression: "this <= 50000"}]}];
  int64 coupon = 3;
  int64 points = 4;
  oneof payment {
    option (buf.validate.oneof).required = true;
    Card card = 5;
    string bank = 6 [(buf.validate.field).string.min_len = 1];
  }
  string memo = 7;
}

message Card {
  string number = 1;
}

message Old {
  option (buf.validate.message).disabled = true;
  int64 n = 1 [(buf.validate.field).int64.gte = 1];
  oneof pick {
    int64 a = 2;
    int64 b = 3;
  }
}
"#;

    #[test]
    fn メッセージの規則とoneofを読む() {
        let ms = messages(RULES);
        let q = ms.iter().find(|m| m.name == "Quote").unwrap();
        assert_eq!(q.rules.cel, vec!["this.min_weight <= this.max_weight".to_string(), "this.max_weight <= 30000".to_string()]);
        assert_eq!(field(&ms, "Quote", "min_weight").rules.cel, vec!["this >= 1".to_string()]);
        assert_eq!(field(&ms, "Quote", "max_weight").rules.cel, vec!["this >= 1".to_string(), "this <= 50000".to_string()]);
        // The members of a oneof are fields of the message, with presence of their own.
        let bank = field(&ms, "Quote", "bank");
        assert!(bank.optional && bank.oneof.as_deref() == Some("payment"));
        assert_eq!(bank.rules.str_min_len, Some(1));
        assert!(field(&ms, "Quote", "card").oneof.is_some());
        assert_eq!(field(&ms, "Quote", "memo").oneof, None);
        assert_eq!(q.fields.len(), 7);
        assert_eq!(
            q.rules.oneofs,
            vec![
                Oneof { fields: vec!["coupon".into(), "points".into()], required: true },
                Oneof { fields: vec!["card".into(), "bank".into()], required: true },
            ]
        );
    }

    #[test]
    fn 検証を切ったメッセージの規則は信じない() {
        let ms = messages(RULES);
        let old = ms.iter().find(|m| m.name == "Old").unwrap();
        assert!(old.rules.disabled);
        assert_eq!(field(&ms, "Old", "n").rules.int.gte, None);
        // The oneof itself is the wire's, and stays.
        assert_eq!(old.rules.oneofs, vec![Oneof { fields: vec!["a".into(), "b".into()], required: false }]);
    }

    #[test]
    fn 検証でない選択肢は読み飛ばす() {
        let ms = messages(ORDER);
        assert_eq!(field(&ms, "Order", "note").rules, Rules::default());
        let c = field(&ms, "Order", "coupon");
        assert!(c.optional && c.rules == Rules::default());
    }

    #[test]
    fn protojson_の名前() {
        let ms = messages(ORDER);
        assert_eq!(field(&ms, "Order", "weight_g").json(), "weightG");
        assert_eq!(field(&ms, "Order", "note").json(), "memo");
        assert_eq!(json_name("order_lines"), "orderLines");
        assert_eq!(json_name("zone"), "zone");
        assert_eq!(json_name("a1_b"), "a1B");
    }

    #[test]
    fn 負の数と十六進と除外の一覧() {
        let ms = messages(ORDER);
        let d = &field(&ms, "Order", "delta").rules.int;
        assert_eq!((d.gte, d.lte, d.not_in.clone()), (Some(-5), Some(16), vec![3, 4]));
    }

    #[test]
    fn 列挙の読み取りは変わらない() {
        let src = "enum MemberTier { MEMBER_TIER_UNSPECIFIED = 0; MEMBER_TIER_GOLD = 1 [(foo) = {a: [1, 2]}]; MEMBER_TIER_BASIC = 2; }";
        let es = enums(src);
        assert_eq!(es[0].aliases(), vec!["gold".to_string(), "basic".to_string()]);
    }
}
