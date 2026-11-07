//! A check against what its `when` observed (DESIGN §9). The subject gives a value,
//! or says why there is none; the matcher compares. A value that is not there (a
//! header not sent, a path that leads nowhere) fails every matcher but `does not
//! exist`, and a body that is not JSON fails every `body json` check, that one
//! included: a check is about a value, and a broken answer is not one.

use ritsu_base::text::Text;
use crate::json::{self, J};
use crate::keep::{Held, View};
use crate::model::{Check, Matcher, Subject, Value};
use crate::run::Obs;

pub struct CheckResult {
    pub line: usize,
    /// The subject and the matcher's words: `status is between`.
    pub label: String,
    /// The matcher's value as written, or nothing: `200 and 299`.
    pub expected: String,
    /// What came back; in words of either language when it is not a value. A report shows this:
    /// what the claim read from the environment is `{name}` in it (DESIGN §22).
    pub actual: Text,
    /// What the journal writes of it: every value the claim kept written `{name}`.
    pub recorded: String,
    pub ok: bool,
    /// For a `screen` check that failed: the screen it saw, cut for the report.
    pub screen: Option<Shown>,
    /// For a check that failed: the names it uses that the claim kept, and their values. What came
    /// from the environment is not among them.
    pub values: Vec<(String, String)>,
}

/// A screen as a failed check shows it (DESIGN §8.6, PLAN C6): at most 40 nodes,
/// the part the pattern speaks of first, and the whole as JSON for `--json`.
pub struct Shown {
    /// One node a line, two spaces a level.
    pub lines: Vec<String>,
    /// How many nodes were left out.
    pub left: usize,
    pub json: String,
}

impl CheckResult {
    /// The check as the spec writes it: `status is between 200 and 299`.
    pub fn text(&self) -> String {
        if self.expected.is_empty() { self.label.clone() } else { format!("{} {}", self.label, self.expected) }
    }
}

/// What a subject gave.
#[derive(Clone)]
enum Got {
    /// stdout, stderr, a body, a header's values.
    Text(String),
    /// exit, status.
    Num(f64),
    /// The value a JSON path leads to.
    Json(J),
    /// Not there: a header not sent, a path that leads nowhere.
    Missing(Text),
    /// Nothing a check can read: a body that is not JSON.
    Broken(Text),
    /// What a GUI target shows.
    Screen(crate::screen::Node),
}

/// A value cut to the 160 characters a failed check shows, `…` after when cut.
fn short(s: &str) -> String {
    let t: String = s.chars().take(160).collect();
    if t.len() < s.len() { format!("{t}…") } else { t }
}

/// A text subject is compared after dropping one trailing newline, as in v0.
pub fn trim_one_newline(s: &str) -> &str {
    s.strip_suffix('\n').unwrap_or(s)
}

fn got(subject: &Subject, obs: &Obs) -> Got {
    match (subject, obs) {
        (Subject::Stdout, Obs::Proc { stdout, .. }) => Got::Text(stdout.clone()),
        (Subject::Stderr, Obs::Proc { stderr, .. }) => Got::Text(stderr.clone()),
        (Subject::Exit, Obs::Proc { exit, .. }) => Got::Num(f64::from(*exit)),
        (Subject::Status, Obs::Http { status, .. }) => Got::Num(f64::from(*status)),
        (Subject::Header(name), Obs::Http { headers, .. }) => {
            let values: Vec<&str> = headers.iter().filter(|(k, _)| k == name).map(|(_, v)| v.as_str()).collect();
            if values.is_empty() {
                Got::Missing(tr!("<ヘッダー \"{name}\" はありません>", "<no header \"{name}\">"))
            } else {
                // several of one name read as one, as RFC 9110 joins them
                Got::Text(values.join(", "))
            }
        }
        (Subject::Body, Obs::Http { body, .. }) => Got::Text(body.clone()),
        (Subject::Screen, Obs::Screen(screen)) => Got::Screen(screen.clone()),
        (Subject::BodyJson(path), Obs::Http { body, .. }) => match json::parse(body) {
            Err(e) => Got::Broken(tr!("<body が JSON ではありません>", "<body is not JSON: {e}>")),
            Ok(v) => match json::path_get(&v, path) {
                Ok(leaf) => Got::Json(leaf.clone()),
                Err(e) => Got::Missing(tr!("<{}>", "<{}>", e.ja(); e.en())),
            },
        },
        (s, _) => {
            // E007 refuses this before anything runs; kept so the run never panics.
            let w = s.word();
            Got::Broken(tr!("<この `when` の結果に {w} はありません>", "<{w} is not observed by this `when`>"))
        }
    }
}

/// A number written as one: an optional sign, digits, an optional fraction and
/// exponent. `inf`, `NaN` and the like are not numbers here.
fn number_in(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let mut i = 0;
    if matches!(b.first(), Some(b'-' | b'+')) {
        i += 1;
    }
    let digits = |i: &mut usize| {
        let start = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i > start
    };
    if !digits(&mut i) {
        return None;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        if !digits(&mut i) {
            return None;
        }
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
            i += 1;
        }
        if !digits(&mut i) {
            return None;
        }
    }
    if i != b.len() {
        return None;
    }
    s.parse().ok()
}

/// The number a value is, for `is above` and the rest: exit and status, a JSON
/// number, and a text (or a JSON string) that is a number once trimmed.
fn as_number(g: &Got) -> Option<f64> {
    match g {
        Got::Num(x) | Got::Json(J::Num(x)) => Some(*x),
        Got::Text(s) | Got::Json(J::Str(s)) => number_in(s.trim()),
        _ => None,
    }
}

/// The text a value is, for `contains` and `matches`: a text, or a JSON string.
fn as_text(g: &Got) -> Option<&str> {
    match g {
        Got::Text(s) | Got::Json(J::Str(s)) => Some(s),
        _ => None,
    }
}

fn same_num(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// A value as the text a `{name}` compares with: a string as it is, a number as it is written, a
/// truth value as its word, an array or an object as JSON. A null, or nothing, is no text.
fn as_written(g: &Got) -> Option<String> {
    match g {
        Got::Text(s) => Some(trim_one_newline(s).to_string()),
        Got::Num(x) => Some(json::render_num(*x)),
        Got::Json(J::Null) => None,
        Got::Json(J::Str(s)) => Some(s.clone()),
        Got::Json(j) => Some(json::render(j)),
        Got::Missing(_) | Got::Broken(_) | Got::Screen(_) => None,
    }
}

/// The value a `keep` reads of what its `when` observed, or what stands in its place when it cannot
/// read one: the words a check says, `null`, or `""`.
pub fn keep_value(subject: &Subject, obs: &Obs) -> Result<String, Text> {
    let g = got(subject, obs);
    match &g {
        Got::Missing(why) | Got::Broken(why) => return Err(why.clone()),
        Got::Json(J::Null) => return Err(Text::same("null")),
        _ => {}
    }
    match as_written(&g) {
        Some(v) if !v.is_empty() => Ok(v),
        _ => Err(Text::same("\"\"")),
    }
}

fn is(g: &Got, v: &Value) -> bool {
    if let Value::Filled(want) = v {
        return as_written(g).is_some_and(|s| s == *want);
    }
    match (g, v) {
        (Got::Text(s), Value::S(want)) => trim_one_newline(s) == want,
        (Got::Text(s), Value::N(n)) => trim_one_newline(s) == json::render_num(*n),
        (Got::Num(x), Value::N(n)) => same_num(*x, *n),
        (Got::Json(J::Str(s)), Value::S(want)) => s == want,
        (Got::Json(J::Num(x)), Value::N(n)) => same_num(*x, *n),
        (Got::Json(J::Str(s)), Value::N(n)) => s == &json::render_num(*n),
        (Got::Json(J::Bool(b)), Value::Bool(want)) => b == want,
        (Got::Json(J::Null), Value::Null) => true,
        _ => false,
    }
}

fn holds(m: &Matcher, g: &Got) -> bool {
    let there = matches!(g, Got::Text(_) | Got::Num(_) | Got::Json(_));
    if let Got::Screen(screen) = g {
        return match m {
            Matcher::ContainsNode(p) => p.holds(screen),
            Matcher::NotContainsNode(p) => !p.holds(screen),
            // E008 refuses any other matcher on the screen
            _ => false,
        };
    }
    match m {
        Matcher::Is(v) => is(g, v),
        Matcher::IsNot(v) => there && !is(g, v),
        Matcher::Above(n) => as_number(g).is_some_and(|x| x > *n),
        Matcher::Below(n) => as_number(g).is_some_and(|x| x < *n),
        Matcher::AtLeast(n) => as_number(g).is_some_and(|x| x >= *n),
        Matcher::AtMost(n) => as_number(g).is_some_and(|x| x <= *n),
        Matcher::Between(a, b) => as_number(g).is_some_and(|x| *a <= x && x <= *b),
        Matcher::Contains(s) => as_text(g).is_some_and(|x| x.contains(s.as_str())),
        Matcher::NotContains(s) => as_text(g).is_some_and(|x| !x.contains(s.as_str())),
        Matcher::Matches(p) => as_text(g).is_some_and(|x| p.matches(trim_one_newline(x))),
        Matcher::NotMatches(p) => as_text(g).is_some_and(|x| !p.matches(trim_one_newline(x))),
        Matcher::Exists => there,
        Matcher::NotExists => matches!(g, Got::Missing(_)),
        // E008 refuses a node on anything but the screen
        Matcher::ContainsNode(_) | Matcher::NotContainsNode(_) => false,
    }
}

/// How many nodes of the screen a pattern matches, as a failed check says it.
fn nodes_found(n: usize) -> Text {
    match n {
        0 => tr!("当てはまるノードがありません", "no such node"),
        1 => tr!("当てはまるノードが 1 個あります", "1 such node"),
        n => tr!("当てはまるノードが {n} 個あります", "{n} such nodes"),
    }
}

/// What a subject gave as a view reads it. It is read before it is cut, so a cut does not leave
/// half of a value behind.
fn seen(g: &Got, held: &Held, view: View) -> Got {
    match g {
        Got::Text(s) => Got::Text(held.text(s, view)),
        Got::Json(j) => Got::Json(held.json(j, view)),
        Got::Missing(why) => Got::Missing(held.words(why, view)),
        Got::Broken(why) => Got::Broken(held.words(why, view)),
        Got::Num(_) | Got::Screen(_) => g.clone(),
    }
}

/// What came back, as a failed check shows it, with why it does not fit the
/// matcher when the value is of another kind. `g` is what the subject gave, and `read` the same as
/// the reader of this text may read it.
fn shown(m: &Matcher, g: &Got, read: &Got) -> Text {
    let value = match read {
        Got::Text(s) => json::quote(&short(trim_one_newline(s))),
        Got::Num(x) => json::render_num(*x),
        Got::Json(j) => short(&json::render(j)),
        Got::Missing(why) | Got::Broken(why) => return why.clone(),
        Got::Screen(screen) => {
            return match m {
                Matcher::ContainsNode(p) | Matcher::NotContainsNode(p) => nodes_found(p.found(screen).len()),
                _ => tr!("画面", "the screen"),
            };
        }
    };
    let numeric = matches!(m, Matcher::Above(_) | Matcher::Below(_) | Matcher::AtLeast(_) | Matcher::AtMost(_) | Matcher::Between(..));
    let textual = matches!(m, Matcher::Contains(_) | Matcher::NotContains(_) | Matcher::Matches(_) | Matcher::NotMatches(_));
    if numeric && as_number(g).is_none() {
        tr!("{value}（数ではありません）", "{value} (not a number)")
    } else if textual && as_text(g).is_none() {
        tr!("{value}（文字列ではありません）", "{value} (not a string)")
    } else {
        Text::same(value)
    }
}

/// A check held to what its `when` observed, with the values the claim holds in its strings. Its
/// label and its value stay as they are written.
pub fn eval(check: &Check, obs: &Obs, held: &Held) -> CheckResult {
    let filled = held.fill_matcher(&check.matcher);
    let g = got(&check.subject, obs);
    let ok = holds(&filled, &g);
    let screen = match (&g, &filled) {
        (Got::Screen(s), Matcher::ContainsNode(_) | Matcher::NotContainsNode(_)) if !ok => {
            let (hidden, told) = held.hidden_screen(s, &check.matcher);
            match &told {
                Matcher::ContainsNode(p) | Matcher::NotContainsNode(p) => {
                    let (lines, left) = crate::screen::cut_for(&hidden, p);
                    Some(Shown { lines, left, json: hidden.json() })
                }
                _ => None,
            }
        }
        _ => None,
    };
    CheckResult {
        line: check.pos.line,
        label: format!("{} {}", check.subject.label(), check.matcher.words()),
        expected: check.matcher.value(),
        actual: shown(&filled, &g, &seen(&g, held, View::Report)),
        recorded: shown(&filled, &g, &seen(&g, held, View::Record)).en,
        ok,
        screen,
        values: if ok { vec![] } else { held.told_values(&check.matcher) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pos;
    use crate::regex::Pattern;

    fn http(body: &str) -> Obs {
        Obs::Http {
            status: 201,
            headers: vec![("location".into(), "/a".into()), ("set-cookie".into(), "a=1".into()), ("set-cookie".into(), "b=2".into())],
            body: body.into(),
        }
    }

    fn proc(stdout: &str, exit: i32) -> Obs {
        Obs::Proc { stdout: stdout.into(), stderr: String::new(), exit }
    }

    fn ok(subject: Subject, matcher: Matcher, obs: &Obs) -> (bool, String) {
        let r = eval(&Check { subject, matcher, pos: Pos::default() }, obs, &Held::default());
        (r.ok, r.actual.en)
    }

    fn p(s: &str) -> Pattern {
        Pattern::new(s).unwrap()
    }

    #[test]
    fn text_subjects() {
        let o = proc(" 42 \n", 0);
        assert!(ok(Subject::Stdout, Matcher::Is(Value::S(" 42 ".into())), &o).0);
        assert!(ok(Subject::Stdout, Matcher::IsNot(Value::S("42".into())), &o).0);
        assert!(ok(Subject::Stdout, Matcher::Between(40.0, 42.0), &o).0, "a text that is a number once trimmed");
        assert!(ok(Subject::Stdout, Matcher::Contains("42 \n".into()), &o).0, "contains looks in the whole text");
        assert!(ok(Subject::Stdout, Matcher::Matches(p(" \\d+ ")), &o).0, "matches drops one trailing newline");
        assert_eq!(ok(Subject::Stdout, Matcher::Above(1.0), &proc("many\n", 0)), (false, "\"many\" (not a number)".into()));
        assert!(!ok(Subject::Stdout, Matcher::Above(1.0), &proc("inf\n", 0)).0);
    }

    #[test]
    fn numbers() {
        let o = proc("", 2);
        assert!(ok(Subject::Exit, Matcher::Is(Value::N(2.0)), &o).0);
        assert!(ok(Subject::Exit, Matcher::IsNot(Value::N(0.0)), &o).0);
        assert!(ok(Subject::Exit, Matcher::AtLeast(2.0), &o).0 && ok(Subject::Exit, Matcher::AtMost(2.0), &o).0);
        assert!(!ok(Subject::Exit, Matcher::Above(2.0), &o).0 && !ok(Subject::Exit, Matcher::Below(2.0), &o).0);
        assert!(ok(Subject::Status, Matcher::Between(200.0, 299.0), &http("")).0);
    }

    #[test]
    fn headers() {
        let o = http("");
        assert!(ok(Subject::Header("location".into()), Matcher::Is(Value::S("/a".into())), &o).0);
        assert!(ok(Subject::Header("set-cookie".into()), Matcher::Is(Value::S("a=1, b=2".into())), &o).0);
        assert!(ok(Subject::Header("location".into()), Matcher::Exists, &o).0);
        assert_eq!(ok(Subject::Header("etag".into()), Matcher::Exists, &o), (false, "<no header \"etag\">".into()));
        assert!(ok(Subject::Header("etag".into()), Matcher::NotExists, &o).0);
        // a check of a header that was not sent fails, `is not` too
        assert!(!ok(Subject::Header("etag".into()), Matcher::IsNot(Value::S("x".into())), &o).0);
    }

    #[test]
    fn json_values() {
        let o = http(r#"{"ok":true,"n":5,"s":"5","none":null,"id":"a1","list":[1]}"#);
        let j = |path: &str, m: Matcher| ok(Subject::BodyJson(path.into()), m, &o);
        assert!(j(".ok", Matcher::Is(Value::Bool(true))).0);
        assert!(j(".none", Matcher::Is(Value::Null)).0);
        assert!(j(".n", Matcher::Is(Value::N(5.0))).0 && j(".s", Matcher::Is(Value::N(5.0))).0);
        assert!(!j(".n", Matcher::Is(Value::S("5".into()))).0);
        assert!(j(".n", Matcher::Above(4.0)).0 && j(".s", Matcher::Above(4.0)).0);
        assert!(j(".id", Matcher::Matches(p("[a-z]\\d"))).0);
        assert_eq!(j(".list", Matcher::Contains("1".into())), (false, "[1] (not a string)".into()));
        assert!(j(".nope", Matcher::NotExists).0 && !j(".nope", Matcher::Exists).0);
        assert!(!j(".nope", Matcher::NotContains("x".into())).0);
        // a body that is not JSON fails every `body json` check
        let broken = http("<html>");
        assert!(!ok(Subject::BodyJson(".a".into()), Matcher::NotExists, &broken).0);
    }

    #[test]
    fn a_kept_value_is_compared_as_text() {
        let mut held = Held::default();
        held.push("id", "7".into(), false);
        held.push("yes", "true".into(), false);
        let o = http(r#"{"n":7,"s":"7","b":true,"none":null,"list":[7],"x":8}"#);
        let holds_of = |path: &str, want: &str| {
            let c = Check { subject: Subject::BodyJson(path.into()), matcher: Matcher::Is(Value::S(want.into())), pos: Pos::default() };
            eval(&c, &o, &held).ok
        };
        assert!(holds_of(".n", "{id}") && holds_of(".s", "{id}") && holds_of(".b", "{yes}"));
        assert!(!holds_of(".x", "{id}") && !holds_of(".none", "{id}") && !holds_of(".nope", "{id}"));
        // a value without a held name in it is compared as it always is
        assert!(!holds_of(".n", "7") && holds_of(".s", "7") && !holds_of(".n", "{other}"));
        let status = Check { subject: Subject::Status, matcher: Matcher::Is(Value::S("{code}".into())), pos: Pos::default() };
        let mut code = Held::default();
        code.push("code", "201".into(), false);
        assert!(eval(&status, &o, &code).ok);
    }

    #[test]
    fn a_failed_check_tells_the_values_it_uses_and_the_journal_writes_their_names() {
        let mut held = Held::default();
        held.push("id", "a1".into(), false);
        held.push("key", "k9".into(), true);
        let o = Obs::Http { status: 200, headers: vec![], body: "a1 k9".into() };
        let c = Check { subject: Subject::Body, matcher: Matcher::Is(Value::S("{id} {key}".into())), pos: Pos::default() };
        assert!(eval(&c, &o, &held).ok);
        let c = Check { subject: Subject::Body, matcher: Matcher::Contains("{id}{key}".into()), pos: Pos::default() };
        let r = eval(&c, &o, &held);
        assert!(!r.ok);
        assert_eq!(r.expected, "\"{id}{key}\"");
        assert_eq!(r.actual.en, "\"a1 {key}\"");
        assert_eq!(r.recorded, "\"{id} {key}\"");
        assert_eq!(r.values, vec![("id".to_string(), "a1".to_string())]);
    }

    #[test]
    fn what_a_keep_reads() {
        let o = http(r#"{"n":7,"f":1.5,"s":"a","b":false,"none":null,"e":"","list":[1,"x"],"o":{"a":1}}"#);
        let of = |path: &str| keep_value(&Subject::BodyJson(path.into()), &o).map_err(|t| t.en);
        assert_eq!(of(".n"), Ok("7".to_string()));
        assert_eq!(of(".f"), Ok("1.5".to_string()));
        assert_eq!(of(".s"), Ok("a".to_string()));
        assert_eq!(of(".b"), Ok("false".to_string()));
        assert_eq!(of(".list"), Ok(r#"[1,"x"]"#.to_string()));
        assert_eq!(of(".o"), Ok(r#"{"a":1}"#.to_string()));
        assert_eq!(of(".none"), Err("null".to_string()));
        assert_eq!(of(".e"), Err("\"\"".to_string()));
        assert!(of(".nope").is_err());
        assert_eq!(keep_value(&Subject::Header("set-cookie".into()), &o), Ok("a=1, b=2".to_string()));
        assert!(keep_value(&Subject::Header("etag".into()), &o).is_err());
        assert_eq!(keep_value(&Subject::Stdout, &proc("a\n\n", 0)), Ok("a\n".to_string()));
        assert!(keep_value(&Subject::Stdout, &proc("\n", 0)).is_err());
        assert!(keep_value(&Subject::BodyJson(".a".into()), &http("<html>")).is_err());
    }

    #[test]
    fn numbers_written_as_text() {
        for (s, n) in [("5", Some(5.0)), ("-1.5", Some(-1.5)), ("2e3", Some(2000.0)), ("+7", Some(7.0))] {
            assert_eq!(number_in(s), n, "{s}");
        }
        for s in ["", "1.", ".5", "inf", "NaN", "1e", "0x10", "1 2"] {
            assert_eq!(number_in(s), None, "{s}");
        }
    }
}
