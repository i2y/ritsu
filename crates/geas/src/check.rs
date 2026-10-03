//! A check against what its `when` observed (DESIGN §9). The subject gives a value,
//! or says why there is none; the matcher compares. A value that is not there (a
//! header not sent, a path that leads nowhere) fails every matcher but `does not
//! exist`, and a body that is not JSON fails every `body json` check, that one
//! included: a check is about a value, and a broken answer is not one.

use crate::diag::{same, t, Text};
use crate::json::{self, J};
use crate::model::{Check, Matcher, Subject, Value};
use crate::run::Obs;

pub struct CheckResult {
    pub line: usize,
    /// The subject and the matcher's words: `status is between`.
    pub label: String,
    /// The matcher's value as written, or nothing: `200 and 299`.
    pub expected: String,
    /// What came back; in words of either language when it is not a value.
    pub actual: Text,
    pub ok: bool,
    /// For a `screen` check that failed: the screen it saw, cut for the report.
    pub screen: Option<Shown>,
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
                Got::Missing(t(format!("<no header \"{name}\">"), format!("<ヘッダー \"{name}\" はありません>")))
            } else {
                // several of one name read as one, as RFC 9110 joins them
                Got::Text(values.join(", "))
            }
        }
        (Subject::Body, Obs::Http { body, .. }) => Got::Text(body.clone()),
        (Subject::Screen, Obs::Screen(screen)) => Got::Screen(screen.clone()),
        (Subject::BodyJson(path), Obs::Http { body, .. }) => match json::parse(body) {
            Err(e) => Got::Broken(t(format!("<body is not JSON: {e}>"), "<body が JSON ではありません>")),
            Ok(v) => match json::path_get(&v, path) {
                Ok(leaf) => Got::Json(leaf.clone()),
                Err(e) => Got::Missing(t(format!("<{}>", e.en()), format!("<{}>", e.ja()))),
            },
        },
        (s, _) => {
            // E007 refuses this before anything runs; kept so the run never panics.
            let w = s.word();
            Got::Broken(t(format!("<{w} is not observed by this `when`>"), format!("<この `when` の結果に {w} はありません>")))
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

fn is(g: &Got, v: &Value) -> bool {
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
        0 => t("no such node", "当てはまるノードがありません"),
        1 => t("1 such node", "当てはまるノードが 1 個あります"),
        n => t(format!("{n} such nodes"), format!("当てはまるノードが {n} 個あります")),
    }
}

/// What came back, as a failed check shows it, with why it does not fit the
/// matcher when the value is of another kind.
fn shown(m: &Matcher, g: &Got) -> Text {
    let value = match g {
        Got::Text(s) => json::quote(&short(trim_one_newline(s))),
        Got::Num(x) => json::render_num(*x),
        Got::Json(j) => short(&json::render(j)),
        Got::Missing(why) | Got::Broken(why) => return why.clone(),
        Got::Screen(screen) => {
            return match m {
                Matcher::ContainsNode(p) | Matcher::NotContainsNode(p) => nodes_found(p.found(screen).len()),
                _ => t("the screen", "画面"),
            };
        }
    };
    let numeric = matches!(m, Matcher::Above(_) | Matcher::Below(_) | Matcher::AtLeast(_) | Matcher::AtMost(_) | Matcher::Between(..));
    let textual = matches!(m, Matcher::Contains(_) | Matcher::NotContains(_) | Matcher::Matches(_) | Matcher::NotMatches(_));
    if numeric && as_number(g).is_none() {
        t(format!("{value} (not a number)"), format!("{value}（数ではありません）"))
    } else if textual && as_text(g).is_none() {
        t(format!("{value} (not a string)"), format!("{value}（文字列ではありません）"))
    } else {
        same(value)
    }
}

pub fn eval(check: &Check, obs: &Obs) -> CheckResult {
    let g = got(&check.subject, obs);
    let ok = holds(&check.matcher, &g);
    let screen = match (&g, &check.matcher) {
        (Got::Screen(s), Matcher::ContainsNode(p) | Matcher::NotContainsNode(p)) if !ok => {
            let (lines, left) = crate::screen::cut_for(s, p);
            Some(Shown { lines, left, json: s.json() })
        }
        _ => None,
    };
    CheckResult {
        line: check.pos.line,
        label: format!("{} {}", check.subject.label(), check.matcher.words()),
        expected: check.matcher.value(),
        actual: shown(&check.matcher, &g),
        ok,
        screen,
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
        let r = eval(&Check { subject, matcher, pos: Pos::default() }, obs);
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
    fn numbers_written_as_text() {
        for (s, n) in [("5", Some(5.0)), ("-1.5", Some(-1.5)), ("2e3", Some(2000.0)), ("+7", Some(7.0))] {
            assert_eq!(number_in(s), n, "{s}");
        }
        for s in ["", "1.", ".5", "inf", "NaN", "1e", "0x10", "1 2"] {
            assert_eq!(number_in(s), None, "{s}");
        }
    }
}
