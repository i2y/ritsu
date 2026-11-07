//! What a claim keeps (DESIGN §22): the values its `keep` lines have read, `{name}` filled in from
//! them, a request as it goes out, and the same values written back as `{name}` in what geas
//! records, so that two runs write the same bytes however the service numbers what it makes.
//!
//! A value geas read from its own environment is told to nobody: whatever geas shows or writes has
//! it as `{name}`, wherever it stands.

use crate::json::{self, J};
use crate::model::{Call, Keep, KeepFrom, Matcher, Place, Value};
use crate::proc::Failure;
use crate::run::Obs;
use crate::screen::{self, Label, Node};
use ritsu_base::text::Text;
use std::cmp::Reverse;

/// One value a claim holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
    pub name: String,
    pub value: String,
    /// Read from geas's environment.
    pub secret: bool,
}

/// What a claim holds at one point of its run, in the order it was kept.
#[derive(Clone, Debug, Default)]
pub struct Held {
    kept: Vec<Kept>,
}

/// Who reads what comes out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// The journal, the baseline and drift: every value written `{name}`, where it stands as a word.
    Record,
    /// A person or an agent reading a report: what the claim read from the environment written
    /// `{name}`, wherever it stands, and the rest as it is.
    Report,
}

/// Every `{name}` in a string, a name being an identifier: the place of its `{`, counted in
/// characters as a string token's columns are, and the name.
pub fn names_in(s: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    for (chars, (i, c)) in s.char_indices().enumerate() {
        if c != '{' {
            continue;
        }
        let after = &s[i + 1..];
        if let Some(name) = name_at(after) {
            out.push((chars, name));
        }
    }
    out
}

/// The identifier at the start of `s`, when a `}` closes it.
fn name_at(s: &str) -> Option<&str> {
    let end = s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
    let name = &s[..end];
    let starts = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_');
    (starts && s[end..].starts_with('}')).then_some(name)
}

/// A letter or a digit of ASCII: what makes a value part of a longer word.
fn wordy(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_ascii_alphanumeric())
}

impl Held {
    pub fn is_empty(&self) -> bool {
        self.kept.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&Kept> {
        self.kept.iter().find(|k| k.name == name)
    }

    pub fn push(&mut self, name: &str, value: String, secret: bool) {
        self.kept.push(Kept { name: name.to_string(), value, secret });
    }

    /// Whether `s` has a `{name}` this claim holds.
    pub fn uses(&self, s: &str) -> bool {
        names_in(s).iter().any(|(_, n)| self.get(n).is_some())
    }

    /// `s` with each `{name}` this claim holds as `value` says, in one pass: what a value holds is
    /// not read again, and any other `{…}` stays as written.
    fn filled_with(&self, s: &str, value: &dyn Fn(&Kept) -> String) -> String {
        let mut out = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            match name_at(after).and_then(|n| self.get(n)) {
                Some(k) => {
                    out.push_str(&value(k));
                    rest = &after[k.name.len() + 1..];
                }
                None => {
                    out.push('{');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        out
    }

    /// `s` with each `{name}` this claim holds as its value.
    pub fn fill(&self, s: &str) -> String {
        self.filled_with(s, &|k| k.value.clone())
    }

    /// `s` filled as a report may say it: what came from the environment stays `{name}`.
    pub fn told(&self, s: &str) -> String {
        self.filled_with(s, &|k| if k.secret { format!("{{{}}}", k.name) } else { self.text(&k.value, View::Report) })
    }

    /// The arguments of a `run`, filled.
    pub fn fill_args(&self, args: &[String]) -> Vec<String> {
        args.iter().map(|a| self.fill(a)).collect()
    }

    /// A call as it goes out. `auto_port` is the port of a `port auto` service it goes to, which
    /// stands for `{port}` in a value (answers are recorded with it, DESIGN §10). Without a held
    /// name the call goes as it is written, as every call always has.
    pub fn sent(&self, call: &Call, auto_port: Option<u16>) -> Result<Call, Failure> {
        if self.is_empty() {
            return Ok(call.clone());
        }
        let value = |k: &Kept| match auto_port {
            Some(p) => k.value.replace("{port}", &p.to_string()),
            None => k.value.clone(),
        };
        let fill = |s: &str| self.filled_with(s, &value);
        let place = |p: &Place| match p {
            Place::Into { name, nth } => Place::Into { name: fill(name), nth: *nth },
            other => other.clone(),
        };
        Ok(match call {
            Call::Run(args) => Call::Run(args.iter().map(|a| fill(a)).collect()),
            Call::Http { method, path, body, headers } => {
                let mut sent = Vec::with_capacity(headers.len());
                for (name, written) in headers {
                    let v = fill(written);
                    if v.chars().any(char::is_control) {
                        return Err(self.no_header(name, written));
                    }
                    sent.push((name.clone(), v));
                }
                Call::Http { method: *method, path: self.sent_path(path, &value)?, body: body.as_deref().map(fill), headers: sent }
            }
            Call::Open(Some(path)) => Call::Open(Some(self.sent_path(path, &value)?)),
            Call::Click { name, nth } => Call::Click { name: fill(name), nth: *nth },
            Call::Input { text, place: p } => Call::Input { text: fill(text), place: place(p) },
            Call::Submit(p) => Call::Submit(place(p)),
            Call::Open(None) | Call::Press(_) | Call::Advance(_) => call.clone(),
        })
    }

    /// E038: a value that cannot go in a header.
    fn no_header(&self, header: &str, written: &str) -> Failure {
        let name = names_in(written).iter().filter_map(|(_, n)| self.get(n)).find(|k| k.value.chars().any(char::is_control)).map_or("", |k| k.name.as_str()).to_string();
        Failure {
            code: "E038",
            msg: tr!(
                "`{name}` の値はヘッダー `{header}` で送れません。改行か制御文字が入っています",
                "the value of `{name}` cannot go in the header `{header}`: it has a line break or a control character",
            ),
            notes: vec![],
        }
    }

    /// A path with held names as it goes out: filled, without the origin of an address on this
    /// machine, and with what a request line cannot hold written `%XX`. A path with no held name in
    /// it goes as it is written.
    fn sent_path(&self, written: &str, value: &dyn Fn(&Kept) -> String) -> Result<String, Failure> {
        if !self.uses(written) {
            return Ok(written.to_string());
        }
        let told = self.told(written);
        let filled = self.filled_with(written, value);
        let path = match without_origin(&filled) {
            Some(rest) => rest,
            None => {
                return Err(Failure {
                    code: "E038",
                    msg: tr!(
                        "呼び出しのパスが `{told}` になりました。別のホストなので送れません。geas が呼ぶのは自分で起動したサービスだけです",
                        "the path of this call comes to `{told}`, on another host: geas only calls the service it started",
                    ),
                    notes: vec![],
                });
            }
        };
        if !path.starts_with('/') {
            return Err(Failure {
                code: "E038",
                msg: tr!("呼び出しのパスが `{told}` になりました。`/` で始まっていないので送れません", "the path of this call comes to `{told}`, which does not start with `/`"),
                notes: vec![],
            });
        }
        Ok(escaped(&path))
    }

    /// The matcher with the held values in its strings. A value with a held name in it is compared
    /// as text; any other is compared as it always is.
    pub fn fill_matcher(&self, m: &Matcher) -> Matcher {
        self.matcher_with(m, &|s| self.fill(s))
    }

    /// The matcher as a screen with what came from the environment hidden is held to it.
    fn told_matcher(&self, m: &Matcher) -> Matcher {
        self.matcher_with(m, &|s| self.told(s))
    }

    fn matcher_with(&self, m: &Matcher, fill: &dyn Fn(&str) -> String) -> Matcher {
        let label = |l: &Label| match l {
            Label::Is(s) => Label::Is(fill(s)),
            Label::Containing(s) => Label::Containing(fill(s)),
            Label::Matching(p) => Label::Matching(p.clone()),
        };
        let pattern = |p: &screen::Pattern| screen::Pattern {
            label: p.label.as_ref().map(label),
            value: p.value.as_deref().map(fill),
            within: p.within.as_ref().map(|(role, l)| (role.clone(), l.as_ref().map(label))),
            ..p.clone()
        };
        match m {
            Matcher::Is(Value::S(s)) if self.uses(s) => Matcher::Is(Value::Filled(fill(s))),
            Matcher::IsNot(Value::S(s)) if self.uses(s) => Matcher::IsNot(Value::Filled(fill(s))),
            Matcher::Contains(s) => Matcher::Contains(fill(s)),
            Matcher::NotContains(s) => Matcher::NotContains(fill(s)),
            Matcher::ContainsNode(p) => Matcher::ContainsNode(pattern(p)),
            Matcher::NotContainsNode(p) => Matcher::NotContainsNode(pattern(p)),
            other => other.clone(),
        }
    }

    /// A screen as a failed check shows it, with what came from the environment hidden, and the
    /// pattern held to it.
    pub fn hidden_screen(&self, screen: &Node, m: &Matcher) -> (Node, Matcher) {
        (self.node(screen, View::Report), self.told_matcher(m))
    }

    /// The names a check uses that a report may say the values of, and those values: in the order
    /// the check writes them, each once.
    pub fn told_values(&self, m: &Matcher) -> Vec<(String, String)> {
        let mut strings: Vec<&str> = Vec::new();
        let label = |l: &Option<Label>, out: &mut Vec<String>| {
            if let Some(Label::Is(s) | Label::Containing(s)) = l {
                out.push(s.clone());
            }
        };
        let mut owned: Vec<String> = Vec::new();
        match m {
            Matcher::Is(Value::S(s)) | Matcher::IsNot(Value::S(s)) | Matcher::Contains(s) | Matcher::NotContains(s) => strings.push(s),
            Matcher::ContainsNode(p) | Matcher::NotContainsNode(p) => {
                label(&p.label, &mut owned);
                owned.extend(p.value.clone());
                if let Some((_, l)) = &p.within {
                    label(l, &mut owned);
                }
            }
            _ => {}
        }
        strings.extend(owned.iter().map(String::as_str));
        let mut out: Vec<(String, String)> = Vec::new();
        for s in strings {
            for (_, n) in names_in(s) {
                if let Some(k) = self.get(n)
                    && !k.secret
                    && !out.iter().any(|(name, _)| name == n)
                {
                    // a value kept from an answer can hold what came from the environment
                    out.push((k.name.clone(), self.text(&k.value, View::Report)));
                }
            }
        }
        out
    }

    /// The values a view writes as `{name}`, the longest first.
    fn replaced(&self, view: View) -> Vec<&Kept> {
        let mut order: Vec<&Kept> = self.kept.iter().filter(|k| k.secret || view == View::Record).collect();
        order.sort_by_key(|k| Reverse(k.value.len()));
        order
    }

    /// What a view writes as `{name}`, the longest first: each value, and what came from the
    /// environment also as a path sends it, which a service can answer with.
    fn forms(&self, view: View) -> Vec<(&Kept, String)> {
        let mut forms = Vec::new();
        for k in self.replaced(view) {
            forms.push((k, k.value.clone()));
            let sent = escaped(&k.value);
            if k.secret && sent != k.value {
                forms.push((k, sent));
            }
        }
        forms.sort_by_key(|(_, form)| Reverse(form.len()));
        forms
    }

    /// `s` as a view reads it, in one pass: a value written `{name}` is not read again. A value kept
    /// from an answer is written where it stands as a word; what came from the environment,
    /// wherever it stands.
    pub fn text(&self, s: &str, view: View) -> String {
        let forms = self.forms(view);
        if forms.is_empty() {
            return s.to_string();
        }
        let mut out = String::with_capacity(s.len());
        let mut before: Option<char> = None;
        let mut i = 0;
        'outer: while i < s.len() {
            let rest = &s[i..];
            for (k, form) in &forms {
                if let Some(after) = rest.strip_prefix(form.as_str())
                    && (k.secret || (!wordy(before) && !wordy(after.chars().next())))
                {
                    out.push('{');
                    out.push_str(&k.name);
                    out.push('}');
                    before = form.chars().last();
                    i += form.len();
                    continue 'outer;
                }
            }
            let c = rest.chars().next().expect("not at the end");
            out.push(c);
            before = Some(c);
            i += c.len_utf8();
        }
        out
    }

    /// Both languages of a message as a view reads them.
    pub fn words(&self, t: &Text, view: View) -> Text {
        Text::new(self.text(&t.ja, view), self.text(&t.en, view))
    }

    /// A JSON value as a view reads it: its strings and its keys by the rule of [`Held::text`], and a
    /// number as the string `"{name}"` when the whole number is a value. It stays JSON.
    pub fn json(&self, j: &J, view: View) -> J {
        match j {
            J::Str(s) => J::Str(self.text(s, view)),
            J::Num(n) => {
                let written = json::render_num(*n);
                if let Some(k) = self.replaced(view).iter().find(|k| k.value == written) {
                    return J::Str(format!("{{{}}}", k.name));
                }
                self.without_secrets(j.clone())
            }
            J::Arr(items) => self.without_secrets(J::Arr(items.iter().map(|i| self.json(i, view)).collect())),
            J::Obj(pairs) => self.without_secrets(J::Obj(pairs.iter().map(|(k, v)| (self.text(k, view), self.json(v, view))).collect())),
            J::Bool(_) | J::Null => self.without_secrets(j.clone()),
        }
    }

    /// A JSON value as a string when what came from the environment is in what it writes: such a
    /// value need not be one string of the answer, and can be a number, a truth value or an object.
    fn without_secrets(&self, j: J) -> J {
        if !self.kept.iter().any(|k| k.secret) {
            return j;
        }
        let written = json::render(&j);
        let hidden = self.text(&written, View::Report);
        if hidden != written { J::Str(hidden) } else { j }
    }

    /// A body as a view reads it: as JSON when it is JSON, written back compactly when a value was
    /// in it, and as text when it is not.
    fn body(&self, body: &str, view: View) -> String {
        match json::parse(body) {
            Ok(j) => {
                let was = json::render(&j);
                let now = json::render(&self.json(&j, view));
                if now == was { self.text(body, View::Report) } else { now }
            }
            Err(_) => self.text(body, view),
        }
    }

    fn node(&self, n: &Node, view: View) -> Node {
        Node {
            name: self.text(&n.name, view),
            value: self.text(&n.value, view),
            children: n.children.iter().map(|c| self.node(c, view)).collect(),
            ..n.clone()
        }
    }

    /// An observation as a view reads it. Its status and its exit stay the numbers they are.
    pub fn obs(&self, obs: &Obs, view: View) -> Obs {
        if self.replaced(view).is_empty() {
            return obs.clone();
        }
        match obs {
            Obs::Proc { stdout, stderr, exit } => Obs::Proc { stdout: self.text(stdout, view), stderr: self.text(stderr, view), exit: *exit },
            Obs::Http { status, headers, body } => Obs::Http {
                status: *status,
                headers: headers.iter().map(|(k, v)| (k.clone(), self.text(v, view))).collect(),
                body: self.body(body, view),
            },
            Obs::Screen(n) => Obs::Screen(self.node(n, view)),
        }
    }

    /// A failure with what came from the environment taken out of what it says.
    pub fn hide_failure(&self, f: Failure) -> Failure {
        Failure { code: f.code, msg: self.words(&f.msg, View::Report), notes: f.notes.iter().map(|n| self.words(n, View::Report)).collect() }
    }
}

/// The path of an address on this machine: what follows its origin, whatever port it says. `s`
/// itself when it has no origin, and None when it is on another host.
fn without_origin(s: &str) -> Option<String> {
    let Some(rest) = s.strip_prefix("http://").or_else(|| s.strip_prefix("https://")) else {
        return Some(s.to_string());
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(end);
    let host = if authority.starts_with('[') {
        authority.find(']').map_or(authority, |close| &authority[..=close])
    } else {
        authority.split(':').next().unwrap_or(authority)
    };
    if !["127.0.0.1", "localhost", "[::1]"].iter().any(|h| host.eq_ignore_ascii_case(h)) {
        return None;
    }
    Some(if path.starts_with('/') { path.to_string() } else { format!("/{path}") })
}

/// A path with what a request line cannot hold written `%XX`: `/`, `?`, `&` and `%` stay.
fn escaped(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b <= 0x20 || b >= 0x7F || matches!(b, b'"' | b'<' | b'>' | b'\\' | b'^' | b'`' | b'{' | b'|' | b'}') {
            out.push_str(&format!("%{b:02X}"));
        } else {
            out.push(b as char);
        }
    }
    out
}

/// What a `keep` found, when it cannot give a value.
pub enum NotRead {
    /// The answer does not have it: the claim fails on this, as a check does.
    Answer(Text),
    /// The environment does not have it: the claim cannot go on (E038).
    Env(Failure),
}

/// The value of a `keep`, and whether it came from the environment.
pub fn read(keep: &Keep, obs: Option<&Obs>) -> Result<(String, bool), NotRead> {
    match &keep.from {
        KeepFrom::Env(var) => match std::env::var(var) {
            Ok(v) if !v.is_empty() => Ok((v, true)),
            found => {
                let why = match found {
                    Ok(_) => tr!("空です", "it is empty"),
                    Err(std::env::VarError::NotPresent) => tr!("ありません", "it is not set"),
                    Err(std::env::VarError::NotUnicode(_)) => tr!("UTF-8 ではありません", "it is not UTF-8"),
                };
                Err(NotRead::Env(Failure {
                    code: "E038",
                    msg: tr!(
                        "`keep` が読む環境変数 `{var}` が{}",
                        "`keep` cannot read the environment variable `{var}`: {}",
                        why.ja; why.en
                    ),
                    notes: vec![tr!("geas を走らせる環境に設定してください", "set it where geas runs")],
                }))
            }
        },
        KeepFrom::Answer(subject) => {
            let obs = obs.expect("E005 refuses a `keep` of an answer before any `when`");
            crate::check::keep_value(subject, obs).map(|v| (v, false)).map_err(NotRead::Answer)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(values: &[(&str, &str, bool)]) -> Held {
        let mut h = Held::default();
        for (n, v, s) in values {
            h.push(n, v.to_string(), *s);
        }
        h
    }

    #[test]
    fn names_are_identifiers_in_braces() {
        assert_eq!(names_in("/a/{id}/{x_1}"), vec![(3, "id"), (8, "x_1")]);
        assert_eq!(names_in("{\"a\":1} {1a} {} {a b} {a"), Vec::<(usize, &str)>::new());
        assert_eq!(names_in("主張{id}"), vec![(2, "id")]);
    }

    #[test]
    fn filling_goes_once_and_leaves_the_rest() {
        let h = held(&[("a", "{b}", false), ("b", "2", false)]);
        assert_eq!(h.fill("{a} {b} {c} {\"k\":1}"), "{b} 2 {c} {\"k\":1}");
    }

    #[test]
    fn a_value_is_written_where_it_stands_as_a_word() {
        let h = held(&[("id", "1", false)]);
        assert_eq!(h.text("/v1/orders/1", View::Record), "/v1/orders/{id}");
        assert_eq!(h.text("11 1a a1 1", View::Record), "11 1a a1 {id}");
        assert_eq!(h.text("注文1番", View::Record), "注文{id}番");
        assert_eq!(h.text("/v1/orders/1", View::Report), "/v1/orders/1", "a report shows it");
    }

    #[test]
    fn the_longest_value_comes_first_and_a_placeholder_is_not_read_again() {
        let h = held(&[("a", "ab", false), ("b", "ab-cd", false), ("c", "{a}", false)]);
        assert_eq!(h.text("ab-cd ab {a}", View::Record), "{b} {a} {c}");
    }

    #[test]
    fn what_came_from_the_environment_goes_wherever_it_stands() {
        let h = held(&[("key", "s3cret", true), ("id", "7", false)]);
        for view in [View::Record, View::Report] {
            assert_eq!(h.text("xs3cretx", view), "x{key}x");
        }
        assert_eq!(h.text("7 s3cret", View::Report), "7 {key}");
        assert_eq!(h.told("/{id}/{key}"), "/7/{key}");
        assert_eq!(h.fill("/{id}/{key}"), "/7/s3cret");
        // a service that answers with the path it was sent answers it as a path sends it
        let h = held(&[("key", "a b", true), ("id", "7", false)]);
        assert_eq!(h.text("/saved/a%20b/7", View::Report), "/saved/{key}/7");
        assert_eq!(h.text("/saved/a b/7", View::Record), "/saved/{key}/{id}");
        // a value the service sent back is told without it
        let h = held(&[("key", "s3cret", true), ("back", "a s3cret", false)]);
        assert_eq!(h.told("{back}"), "a {key}");
        assert_eq!(h.told_values(&Matcher::Contains("{back}{key}".into())), vec![("back".to_string(), "a {key}".to_string())]);
    }

    #[test]
    fn json_stays_json() {
        let h = held(&[("id", "7", false), ("key", "12", true)]);
        let body = |s: &str, v| h.body(s, v);
        assert_eq!(body(r#"{"id":7,"qty":70,"s":"a 7","k":{"7":true}}"#, View::Record), r#"{"id":"{id}","qty":70,"s":"a {id}","k":{"{id}":true}}"#);
        assert_eq!(body(r#"{"id": 8}"#, View::Record), r#"{"id": 8}"#, "the bytes, when nothing is in it");
        assert_eq!(body("7 not json", View::Record), "{id} not json");
        assert_eq!(body(r#"{"n":3124}"#, View::Report), r#"{"n":"3{key}4"}"#, "a secret in a number is a secret");
        let word = held(&[("key", "true", true)]);
        assert_eq!(word.body(r#"{"a":true,"b":false}"#, View::Report), r#"{"a":"{key}","b":false}"#, "or in a truth value");
        let whole = held(&[("key", r#"{"type":"sa"}"#, true)]);
        assert_eq!(whole.body(r#"{"creds": {"type": "sa"}, "n": 1}"#, View::Report), r#"{"creds":"{key}","n":1}"#, "or a whole object");
        let spaced = held(&[("key", r#"{"a": 1}"#, true)]);
        assert_eq!(spaced.body(r#"[{"a": 1}]"#, View::Record), "[{key}]", "or as the answer spaces it");
        let kept = held(&[("yes", "true", false)]);
        assert_eq!(kept.body(r#"{"a":true}"#, View::Record), r#"{"a":true}"#, "a truth value kept stays what it is");
    }

    #[test]
    fn the_origin_of_an_address_on_this_machine_comes_off() {
        for (s, want) in [
            ("http://127.0.0.1:{port}/o/1", Some("/o/1")),
            ("https://localhost/o", Some("/o")),
            ("http://[::1]:80/o?x=1", Some("/o?x=1")),
            ("http://LOCALHOST", Some("/")),
            ("http://localhost?x", Some("/?x")),
            ("http://example.com/o", None),
            ("http://127.0.0.1.example.com/o", None),
            ("/o/1", Some("/o/1")),
            ("o/1", Some("o/1")),
        ] {
            assert_eq!(without_origin(s).as_deref(), want, "{s}");
        }
    }

    #[test]
    fn a_request_line_holds_what_it_can() {
        assert_eq!(escaped("/a b/c?d=e&f=%41"), "/a%20b/c?d=e&f=%41");
        assert_eq!(escaped("/\"<>\\^`{|}\u{7f}"), "/%22%3C%3E%5C%5E%60%7B%7C%7D%7F");
        assert_eq!(escaped("/注"), "/%E6%B3%A8");
    }

    #[test]
    fn a_call_goes_out_filled() {
        use crate::model::Method;
        let h = held(&[("id", "a b", false), ("at", "http://127.0.0.1:{port}/o", false), ("key", "k", true)]);
        let http = |path: &str, body: Option<&str>, headers: &[(&str, &str)]| Call::Http {
            method: Method::Get,
            path: path.into(),
            body: body.map(str::to_string),
            headers: headers.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
        };
        let shown = |c: &Call| c.display();
        let sent = h.sent(&http("/o/{id}", Some("{at} {port}"), &[("authorization", "Bearer {key}")]), Some(5000)).ok().expect("it goes");
        assert_eq!(shown(&sent), "get(\"/o/a%20b\", body: \"http://127.0.0.1:5000/o {port}\", header \"authorization\": \"Bearer k\")");
        // the origin comes off; a port that is not `port auto` leaves `{port}` in a body
        let sent = h.sent(&http("{at}/items", Some("{at}"), &[]), None).ok().expect("it goes");
        assert_eq!(shown(&sent), "get(\"/o/items\", body: \"http://127.0.0.1:{port}/o\")");
        // nothing held: the path as it is written
        let sent = Held::default().sent(&http("/a b/{id}", None, &[]), None).ok().expect("it goes");
        assert_eq!(shown(&sent), "get(\"/a b/{id}\")");
        // what cannot go
        let bad = held(&[("nl", "a\nb", false), ("far", "http://example.com/o", false), ("rel", "o/1", false)]);
        let code = |c: Call| bad.sent(&c, None).err().map(|f| f.code);
        assert_eq!(code(http("/", None, &[("x", "{nl}")])), Some("E038"));
        assert_eq!(code(http("{far}", None, &[])), Some("E038"));
        assert_eq!(code(http("{rel}", None, &[])), Some("E038"));
    }
}
