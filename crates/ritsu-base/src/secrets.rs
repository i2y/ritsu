//! Keys written into a file (DESIGN 16.3): the kinds of key whose provider fixes a prefix and a
//! shape, so that a value of that shape is seldom anything else, found in the whole text of a
//! file — its strings and its comments alike, since a key left in a comment reaches whoever can
//! read the repository as surely as one in a string.
//!
//! The shapes are gitleaks's default rules (`config/gitleaks.toml`), written by hand with no
//! regular expressions (P9): a prefix, the kinds of character, the lengths, and what may stand
//! before and after. Where a rule of gitleaks says nothing of what stands before or after the key,
//! a key that goes on into a letter or a digit on either side is not one. An AWS access key ID
//! that ends in `EXAMPLE` (AWS's documents', which gitleaks lets through too) and a key whose part
//! after its prefix is one character over and over (`ghp_` and 36 `x`) are examples, and not
//! found. A key is never given back whole: [`Found::shown`] is its fixed prefix and an ellipsis.

use crate::text::Text;
use crate::tr;
use std::sync::OnceLock;

/// One kind of key the scan knows (DESIGN 16.3).
#[derive(Debug, PartialEq)]
pub struct Kind {
    /// A short id, for tests and JSON: "aws-access-key-id", "github-token", "slack-token",
    /// "slack-webhook-url", "stripe-key", "openai-api-key", "anthropic-api-key", "google-api-key",
    /// "private-key".
    pub id: &'static str,
    /// What a message calls it: tr!("AWS のアクセスキー ID", "an AWS access key ID").
    pub name: Text,
    /// Whom to revoke it with: "AWS", "GitHub", "Slack", "Stripe", "OpenAI", "Anthropic", "Google";
    /// "" for a private key.
    pub provider: &'static str,
}

/// A key found in a text.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// Where it starts: the line and the column, from 1, the column in characters.
    pub line: usize,
    pub col: usize,
    pub kind: &'static Kind,
    /// The fixed prefix of the kind and an ellipsis, never the key: "AKIA…", "sk_live_…", and for a
    /// private key its `-----BEGIN … PRIVATE KEY-----` line.
    pub shown: String,
    /// Its length in characters (for a private key, of the header line).
    pub len: usize,
    /// The line says `ritsu: test secret`: a language reports only the ones where this is false.
    pub test: bool,
}

/// What a line says, in a comment, to have the key on it taken for a value for tests.
pub const TEST_MARK: &str = "ritsu: test secret";

const AWS: usize = 0;
const GITHUB: usize = 1;
const SLACK: usize = 2;
const SLACK_WEBHOOK: usize = 3;
const STRIPE: usize = 4;
const OPENAI: usize = 5;
const ANTHROPIC: usize = 6;
const GOOGLE: usize = 7;
const PRIVATE_KEY: usize = 8;

/// The kinds, in the order of DESIGN 16.3's table.
pub fn kinds() -> &'static [Kind] {
    static KINDS: OnceLock<Vec<Kind>> = OnceLock::new();
    KINDS.get_or_init(|| {
        vec![
            Kind { id: "aws-access-key-id", name: tr!("AWS のアクセスキー ID", "an AWS access key ID"), provider: "AWS" },
            Kind { id: "github-token", name: tr!("GitHub のトークン", "a GitHub token"), provider: "GitHub" },
            Kind { id: "slack-token", name: tr!("Slack のトークン", "a Slack token"), provider: "Slack" },
            Kind { id: "slack-webhook-url", name: tr!("Slack の Incoming Webhook の URL", "a Slack incoming webhook URL"), provider: "Slack" },
            Kind { id: "stripe-key", name: tr!("Stripe のシークレットキー", "a Stripe secret key"), provider: "Stripe" },
            Kind { id: "openai-api-key", name: tr!("OpenAI の API キー", "an OpenAI API key"), provider: "OpenAI" },
            Kind { id: "anthropic-api-key", name: tr!("Anthropic の API キー", "an Anthropic API key"), provider: "Anthropic" },
            Kind { id: "google-api-key", name: tr!("Google の API キー", "a Google API key"), provider: "Google" },
            Kind { id: "private-key", name: tr!("秘密鍵", "a private key"), provider: "" },
        ]
    })
}

/// Every key of the kinds in the whole text (strings and comments alike), in the order found.
/// An AWS key ID that ends in `EXAMPLE`, and a key whose part after its prefix is one character
/// over and over, are examples and not found.
pub fn scan(text: &str) -> Vec<Found> {
    let c: Vec<char> = text.chars().collect();
    // where each line starts, as character offsets
    let mut starts = vec![0usize];
    starts.extend(c.iter().enumerate().filter(|(_, ch)| **ch == '\n').map(|(i, _)| i + 1));
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let Some(m) = at(&c, i) else {
            i += 1;
            continue;
        };
        let line = starts.partition_point(|s| *s <= i);
        let from = starts[line - 1];
        let to = starts.get(line).map(|s| s - 1).unwrap_or(c.len());
        let on_line: String = c[from..to].iter().collect();
        if !m.example {
            out.push(Found { line, col: i - from + 1, kind: &kinds()[m.kind], shown: m.shown, len: m.len, test: on_line.contains(TEST_MARK) });
        }
        i = m.end.max(i + 1);
    }
    out
}

/// The text with every key in it (one for tests too) put as [`Found::shown`] gives it: its prefix
/// and an ellipsis, and a private key as its header line and an ellipsis. What a diagnostic shows
/// of a line goes through this, so that a key on a line some other diagnostic shows is not printed
/// either.
pub fn mask(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < c.len() {
        match at(&c, i) {
            Some(m) if !m.example => {
                out.push_str(&m.shown);
                if m.kind == PRIVATE_KEY {
                    // the header, and the body as an ellipsis on the lines it took
                    out.push('…');
                    if c[i..m.end].contains(&'\n') {
                        out.push('\n');
                    }
                }
                i = m.end.max(i + 1);
            }
            _ => {
                out.push(c[i]);
                i += 1;
            }
        }
    }
    out
}

/// A key at one place of the text.
struct Match {
    kind: usize,
    /// Where it ends (a character offset past it).
    end: usize,
    shown: String,
    len: usize,
    /// One of the examples the scan lets through.
    example: bool,
}

/// A letter, a digit or `_`: what a regular expression's `\b` takes for a word.
fn word(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn alnum(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
}

/// Nothing of a word stands right before offset `i`.
fn starts_word(c: &[char], i: usize) -> bool {
    i == 0 || !word(c[i - 1])
}

/// `prefix` is at offset `i`, as written.
fn has(c: &[char], i: usize, prefix: &str) -> bool {
    let mut j = i;
    for p in prefix.chars() {
        if c.get(j) != Some(&p) {
            return false;
        }
        j += 1;
    }
    true
}

/// `prefix` is at offset `i`, in either case.
fn has_any_case(c: &[char], i: usize, prefix: &str) -> bool {
    let mut j = i;
    for p in prefix.chars() {
        match c.get(j) {
            Some(x) if x.eq_ignore_ascii_case(&p) => j += 1,
            _ => return false,
        }
    }
    true
}

/// How many characters from offset `i` on are of the kind `ok`.
fn run(c: &[char], i: usize, ok: impl Fn(char) -> bool) -> usize {
    c[i.min(c.len())..].iter().take_while(|ch| ok(**ch)).count()
}

/// What gitleaks asks to follow a key whose shape could go on (its `(?:[\x60'"\s;]|\\[nr]|$)`):
/// the end of the text, a backquote, a quote, a space or a line's end, `;`, or the escapes `\n`
/// and `\r` written in a string.
fn ends_value(c: &[char], j: usize) -> bool {
    match c.get(j) {
        None => true,
        Some('`' | '\'' | '"' | ';') => true,
        Some(ch) if ch.is_whitespace() => true,
        Some('\\') => matches!(c.get(j + 1), Some('n' | 'r')),
        _ => false,
    }
}

/// Whether the characters from `a` to `b`, leaving out `-` and `_`, are one character over and
/// over: an example's placeholder (`xxxx…`, `0000…`).
fn one_over_and_over(c: &[char], a: usize, b: usize) -> bool {
    let mut it = c[a..b].iter().filter(|ch| !matches!(ch, '-' | '_'));
    match it.next() {
        Some(first) => it.all(|ch| ch == first),
        None => true,
    }
}

fn key(kind: usize, c: &[char], i: usize, prefix_len: usize, end: usize) -> Match {
    let shown: String = c[i..i + prefix_len].iter().collect::<String>() + "…";
    Match { kind, end, shown, len: end - i, example: one_over_and_over(c, i + prefix_len, end) }
}

/// The key that starts at offset `i`, if one does.
fn at(c: &[char], i: usize) -> Option<Match> {
    match c[i] {
        'A' => aws(c, i).or_else(|| google(c, i)),
        'g' => github(c, i),
        'x' => slack(c, i),
        'h' => slack_webhook(c, i),
        's' => anthropic(c, i).or_else(|| openai(c, i)).or_else(|| stripe(c, i)),
        'r' => stripe(c, i),
        '-' => private_key(c, i),
        _ => None,
    }
}

/// gitleaks `aws-access-token`: `\b((?:A3T[A-Z0-9]|AKIA|ASIA|ABIA|ACCA)[A-Z2-7]{16})\b`, and its
/// allowlist `.+EXAMPLE$`.
fn aws(c: &[char], i: usize) -> Option<Match> {
    if !starts_word(c, i) {
        return None;
    }
    let shown = if ["AKIA", "ASIA", "ABIA", "ACCA"].iter().any(|p| has(c, i, p)) {
        4
    } else if has(c, i, "A3T") && c.get(i + 3).is_some_and(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit()) {
        3
    } else {
        return None;
    };
    let body = |ch: char| ch.is_ascii_uppercase() || ('2'..='7').contains(&ch);
    if run(c, i + 4, body) < 16 || c.get(i + 20).is_some_and(|ch| word(*ch)) {
        return None;
    }
    let mut m = key(AWS, c, i, shown, i + 20);
    m.example |= c[i + 13..i + 20].iter().collect::<String>() == "EXAMPLE";
    Some(m)
}

/// gitleaks `github-pat`, `github-oauth`, `github-app-token`, `github-refresh-token`
/// (`gh[pousr]_` and 36 letters and digits) and `github-fine-grained-pat` (`github_pat_` and 82
/// of `\w`).
fn github(c: &[char], i: usize) -> Option<Match> {
    if !starts_word(c, i) {
        return None;
    }
    if has(c, i, "github_pat_") {
        let n = run(c, i + 11, word);
        return (n == 82).then(|| key(GITHUB, c, i, 11, i + 93));
    }
    if has(c, i, "gh") && c.get(i + 2).is_some_and(|ch| "pousr".contains(*ch)) && c.get(i + 3) == Some(&'_') {
        let n = run(c, i + 4, alnum);
        return (n == 36).then(|| key(GITHUB, c, i, 4, i + 40));
    }
    None
}

/// gitleaks `slack-bot-token`, `slack-legacy-bot-token`, `slack-user-token`, `slack-app-token`,
/// `slack-config-access-token` and `slack-config-refresh-token`: the tokens that start `xoxb-`,
/// `xoxp-`, `xoxe-` (`xoxe.xoxb-`, `xoxe.xoxp-`) and `xapp-`.
fn slack(c: &[char], i: usize) -> Option<Match> {
    if !starts_word(c, i) {
        return None;
    }
    let digit = |ch: char| ch.is_ascii_digit();
    let tail = |ch: char| ch.is_ascii_alphanumeric() || ch == '-';
    if has(c, i, "xoxb-") {
        // xoxb-[0-9]{10,13}-[0-9]{10,13}[a-zA-Z0-9-]*
        let a = run(c, i + 5, digit);
        if (10..=13).contains(&a) && c.get(i + 5 + a) == Some(&'-') {
            let b0 = i + 6 + a;
            if run(c, b0, digit) >= 10 {
                let end = b0 + run(c, b0, tail);
                return Some(key(SLACK, c, i, 5, end));
            }
        }
        // xoxb-[0-9]{8,14}-[a-zA-Z0-9]{18,26}
        if (8..=14).contains(&a) && c.get(i + 5 + a) == Some(&'-') {
            let b0 = i + 6 + a;
            let b = run(c, b0, alnum);
            return (18..=26).contains(&b).then(|| key(SLACK, c, i, 5, b0 + b));
        }
        return None;
    }
    if has(c, i, "xoxe.xoxb-") || has(c, i, "xoxe.xoxp-") {
        // xoxe.xox[bp]-\d-[A-Z0-9]{163,166}, in either case
        if c.get(i + 10).is_some_and(|ch| ch.is_ascii_digit()) && c.get(i + 11) == Some(&'-') {
            let n = run(c, i + 12, alnum);
            return (163..=166).contains(&n).then(|| key(SLACK, c, i, 10, i + 12 + n));
        }
        return None;
    }
    if has(c, i, "xoxe-") && c.get(i + 5).is_some_and(|ch| ch.is_ascii_digit()) && c.get(i + 6) == Some(&'-') {
        // xoxe-\d-[A-Z0-9]{146}, in either case
        let n = run(c, i + 7, alnum);
        return (n == 146).then(|| key(SLACK, c, i, 5, i + 7 + n));
    }
    if has(c, i, "xoxp-") || has(c, i, "xoxe-") {
        // xox[pe](?:-[0-9]{10,13}){3}-[a-zA-Z0-9-]{28,34}
        let mut j = i + 4;
        for _ in 0..3 {
            if c.get(j) != Some(&'-') {
                return None;
            }
            let n = run(c, j + 1, digit);
            if !(10..=13).contains(&n) {
                return None;
            }
            j += 1 + n;
        }
        if c.get(j) != Some(&'-') {
            return None;
        }
        let n = run(c, j + 1, tail);
        return (28..=34).contains(&n).then(|| key(SLACK, c, i, 5, j + 1 + n));
    }
    if has(c, i, "xapp-") {
        // xapp-\d-[A-Z0-9]+-\d+-[a-z0-9]+, in either case
        if !(c.get(i + 5).is_some_and(|ch| ch.is_ascii_digit()) && c.get(i + 6) == Some(&'-')) {
            return None;
        }
        let a = run(c, i + 7, alnum);
        let j = i + 7 + a;
        if a == 0 || c.get(j) != Some(&'-') {
            return None;
        }
        let d = run(c, j + 1, digit);
        let k = j + 1 + d;
        if d == 0 || c.get(k) != Some(&'-') {
            return None;
        }
        let e = run(c, k + 1, alnum);
        return (e > 0).then(|| key(SLACK, c, i, 5, k + 1 + e));
    }
    None
}

/// gitleaks `slack-webhook-url`: `(?:https?://)?hooks.slack.com/(?:services|workflows|triggers)/`
/// and 43 to 56 of letters, digits, `+` and `/`.
fn slack_webhook(c: &[char], i: usize) -> Option<Match> {
    if !starts_word(c, i) {
        return None;
    }
    let host = if has(c, i, "https://") {
        i + 8
    } else if has(c, i, "http://") {
        i + 7
    } else {
        i
    };
    if !has(c, host, "hooks.slack.com/") {
        return None;
    }
    let mut j = host + 16;
    let path = ["services/", "workflows/", "triggers/"].into_iter().find(|p| has(c, j, p))?;
    j += path.chars().count();
    let n = run(c, j, |ch| ch.is_ascii_alphanumeric() || ch == '+' || ch == '/');
    (43..=56).contains(&n).then(|| key(SLACK_WEBHOOK, c, i, j - i, j + n))
}

/// gitleaks `stripe-access-token`: `\b((?:sk|rk)_(?:test|live|prod)_[a-zA-Z0-9]{10,99})` and
/// what ends a value.
fn stripe(c: &[char], i: usize) -> Option<Match> {
    if !starts_word(c, i) || !(has(c, i, "sk_") || has(c, i, "rk_")) {
        return None;
    }
    if !["test_", "live_", "prod_"].iter().any(|p| has(c, i + 3, p)) {
        return None;
    }
    let n = run(c, i + 8, alnum);
    ((10..=99).contains(&n) && ends_value(c, i + 8 + n)).then(|| key(STRIPE, c, i, 8, i + 8 + n))
}

/// gitleaks `openai-api-key`: `sk-proj-`, `sk-svcacct-` or `sk-admin-`, 74 or 58 of
/// `[A-Za-z0-9_-]`, `T3BlbkFJ`, 74 or 58 more; or `sk-`, 20 letters and digits, `T3BlbkFJ`, 20
/// more. Then what ends a value.
fn openai(c: &[char], i: usize) -> Option<Match> {
    if !starts_word(c, i) || !has(c, i, "sk-") {
        return None;
    }
    let part = |ch: char| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-';
    for p in ["sk-proj-", "sk-svcacct-", "sk-admin-"] {
        if !has(c, i, p) {
            continue;
        }
        let a0 = i + p.len();
        for a in [74, 58] {
            if run(c, a0, part) < a || !has(c, a0 + a, "T3BlbkFJ") {
                continue;
            }
            let b0 = a0 + a + 8;
            for b in [74, 58] {
                let end = b0 + b;
                // `\b` and then what ends a value: the key's last character is of a word
                if run(c, b0, part) >= b && word(c[end - 1]) && ends_value(c, end) {
                    return Some(key(OPENAI, c, i, p.len(), end));
                }
            }
        }
        return None;
    }
    if run(c, i + 3, alnum) >= 20 && has(c, i + 23, "T3BlbkFJ") && run(c, i + 31, alnum) >= 20 && ends_value(c, i + 51) {
        return Some(key(OPENAI, c, i, 3, i + 51));
    }
    None
}

/// gitleaks `anthropic-api-key` and `anthropic-admin-api-key`: `sk-ant-api03-` or
/// `sk-ant-admin01-`, 93 of `[a-zA-Z0-9_-]`, `AA`, and what ends a value.
fn anthropic(c: &[char], i: usize) -> Option<Match> {
    if !starts_word(c, i) {
        return None;
    }
    let p = ["sk-ant-api03-", "sk-ant-admin01-"].into_iter().find(|p| has(c, i, p))?;
    let a0 = i + p.len();
    let part = |ch: char| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-';
    let end = a0 + 95;
    (run(c, a0, part) >= 93 && has(c, a0 + 93, "AA") && ends_value(c, end)).then(|| key(ANTHROPIC, c, i, p.len(), end))
}

/// gitleaks `gcp-api-key`: `\b(AIza[\w-]{35})` and what ends a value.
fn google(c: &[char], i: usize) -> Option<Match> {
    if !starts_word(c, i) || !has(c, i, "AIza") {
        return None;
    }
    let n = run(c, i + 4, |ch| word(ch) || ch == '-');
    (n >= 35 && ends_value(c, i + 39)).then(|| key(GOOGLE, c, i, 4, i + 39))
}

/// gitleaks `private-key`, as DESIGN 16.3 reads it: a `-----BEGIN … PRIVATE KEY-----` line
/// (`PRIVATE KEY BLOCK-----` too, in either case), then 64 or more characters of base64. The
/// base64 may start on the header's line or the next, and go on over lines: a line's end (or
/// `\n` written in a string), the spaces that indent the next line and a comment's `#` or `//`
/// at its start are passed over; anything else ends it.
fn private_key(c: &[char], i: usize) -> Option<Match> {
    if !has_any_case(c, i, "-----BEGIN") {
        return None;
    }
    // [ A-Z0-9_-]{0,100} before PRIVATE KEY
    let mid = |ch: char| ch == ' ' || ch.is_ascii_alphanumeric() || ch == '_' || ch == '-';
    let mut j = i + 10;
    let limit = j + 100;
    while !has_any_case(c, j, "PRIVATE KEY") {
        if j >= limit || !c.get(j).is_some_and(|ch| mid(*ch)) {
            return None;
        }
        j += 1;
    }
    j += 11;
    if has_any_case(c, j, " BLOCK") {
        j += 6;
    }
    if !has(c, j, "-----") {
        return None;
    }
    let header = j + 5;
    let base64 = |ch: char| ch.is_ascii_alphanumeric() || ch == '+' || ch == '/' || ch == '=';
    let mut k = header;
    while matches!(c.get(k), Some(' ' | '\t')) {
        k += 1;
    }
    // Anything else on the header's line (a comment, `ritsu: test secret`) is passed over to the
    // line's end.
    let goes_on = |k: usize| matches!(c.get(k), Some(ch) if base64(*ch) || *ch == '\n' || *ch == '\r') || (c.get(k) == Some(&'\\') && matches!(c.get(k + 1), Some('n' | 'r')));
    if !goes_on(k) && k < c.len() {
        while k < c.len() && c[k] != '\n' {
            k += 1;
        }
    }
    let mut body: Vec<char> = Vec::new();
    loop {
        let broke = match c.get(k) {
            Some(ch) if base64(*ch) => {
                body.push(*ch);
                k += 1;
                false
            }
            Some('\r') => {
                k += 1;
                false
            }
            Some('\n') => {
                k += 1;
                true
            }
            Some('\\') if c.get(k + 1) == Some(&'r') => {
                k += 2;
                false
            }
            Some('\\') if c.get(k + 1) == Some(&'n') => {
                k += 2;
                true
            }
            _ => break,
        };
        // after a line's end: the next line's indent, and a comment's `#` or `//`
        if broke {
            while matches!(c.get(k), Some(' ' | '\t')) {
                k += 1;
            }
            if c.get(k) == Some(&'#') {
                k += 1;
            } else if has(c, k, "//") {
                k += 2;
            }
            while matches!(c.get(k), Some(' ' | '\t')) {
                k += 1;
            }
        }
    }
    if body.len() < 64 {
        return None;
    }
    let shown: String = c[i..header].iter().collect();
    let example = body.iter().filter(|ch| **ch != '=').all(|ch| Some(ch) == body.first());
    Some(Match { kind: PRIVATE_KEY, end: k, len: header - i, shown, example })
}
