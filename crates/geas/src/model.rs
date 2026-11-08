//! The claims file as geas runs it, after the parser and the static checks have
//! accepted it.

/// A place in the spec: 1-based line and column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

/// A target's command, split into words when the spec is read (DESIGN §11); a
/// word may still hold `{port}`, replaced when the program starts.
#[derive(Debug, Clone)]
pub enum TargetKind {
    Run(Vec<String>),
    /// A service; a claim may also drive it as a page in Chrome (DESIGN §8.4).
    Serve { words: Vec<String>, port: Port },
    /// `pixie "<app>"`: a pixie app, driven by replaying a script (DESIGN §8.3).
    Pixie(Vec<String>),
    /// `driver "<command>"`: a GUI reached through the driver protocol (DESIGN §8.5).
    Driver(Vec<String>),
}


/// A service's port: written in the spec, or handed out by geas for each instance
/// (DESIGN §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Port {
    Fixed(u16),
    Auto,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub name: String,
    pub kind: TargetKind,
    /// Where `target` is written.
    pub pos: Pos,
    /// The pins in effect: the target's own over the top level's (DESIGN §12).
    pub pins: Pins,
    /// `serial`: the claims that touch it run one at a time (DESIGN §10).
    pub serial: bool,
    /// `contract`: what the service answers is held to this document (DESIGN §20).
    pub contract: Option<Contract>,
}

/// A target's `contract` line.
#[derive(Debug, Clone)]
pub struct Contract {
    /// As written: from the spec's directory.
    pub file: String,
    pub pos: Pos,
    /// What it holds, once `contract::read` has read it.
    pub held: Option<std::sync::Arc<crate::contract::Held>>,
}

/// What a target's processes are given to see (DESIGN §12), as the spec writes it:
/// `{port}` in a value is still there, replaced when the program starts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pins {
    /// `env clean`: the environment is `PATH` and what the pins set or pass.
    pub clean: bool,
    /// `env pass "NAME"`, in order.
    pub pass: Vec<String>,
    /// `env "NAME" "value"`, in order.
    pub env: Vec<(String, String)>,
    pub tz: Option<String>,
    /// As written, `ja-JP`.
    pub locale: Option<String>,
    /// The time as written, and the variable it is passed in.
    pub clock: Option<(String, Option<String>)>,
    /// The seed, and the variable it is passed in.
    pub seed: Option<(u64, Option<String>)>,
}

impl Pins {
    pub fn is_empty(&self) -> bool {
        *self == Pins::default()
    }

    /// The variables the pins set, in order, `{port}` not yet replaced: `env`, then
    /// `TZ`, `LANG` and `LC_ALL` (`ja_JP.UTF-8`), then the variables `clock` and
    /// `seed` name. `env pass` sets nothing here: it keeps what geas has.
    pub fn vars(&self) -> Vec<(String, String)> {
        let mut v = self.env.clone();
        if let Some(tz) = &self.tz {
            v.push(("TZ".into(), tz.clone()));
        }
        if let Some(l) = &self.locale {
            let posix = format!("{}.UTF-8", l.replace('-', "_"));
            v.push(("LANG".into(), posix.clone()));
            v.push(("LC_ALL".into(), posix));
        }
        if let Some((at, Some(name))) = &self.clock {
            v.push((name.clone(), at.clone()));
        }
        if let Some((n, Some(name))) = &self.seed {
            v.push((name.clone(), n.to_string()));
        }
        v
    }

    /// As the journal's `env` event and the baseline's first line write them: the
    /// members that are set, in a fixed order.
    pub fn json(&self) -> String {
        use crate::json::quote;
        let mut parts = Vec::new();
        if self.clean {
            parts.push("\"clean\":true".to_string());
        }
        if !self.pass.is_empty() {
            let names: Vec<String> = self.pass.iter().map(|n| quote(n)).collect();
            parts.push(format!("\"pass\":[{}]", names.join(",")));
        }
        if !self.env.is_empty() {
            let vars: Vec<String> = self.env.iter().map(|(k, v)| format!("{}:{}", quote(k), quote(v))).collect();
            parts.push(format!("\"env\":{{{}}}", vars.join(",")));
        }
        if let Some(tz) = &self.tz {
            parts.push(format!("\"tz\":{}", quote(tz)));
        }
        if let Some(l) = &self.locale {
            parts.push(format!("\"locale\":{}", quote(l)));
        }
        if let Some((at, env)) = &self.clock {
            parts.push(format!("\"clock\":{}", quote(at)));
            if let Some(n) = env {
                parts.push(format!("\"clock_env\":{}", quote(n)));
            }
        }
        if let Some((n, env)) = &self.seed {
            parts.push(format!("\"seed\":{n}"));
            if let Some(name) = env {
                parts.push(format!("\"seed_env\":{}", quote(name)));
            }
        }
        format!("{{{}}}", parts.join(","))
    }
}

/// The five HTTP methods a call can use (DESIGN §22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    /// The word a spec writes it with: `get`.
    pub fn word(self) -> &'static str {
        match self {
            Method::Get => "get",
            Method::Post => "post",
            Method::Put => "put",
            Method::Patch => "patch",
            Method::Delete => "delete",
        }
    }

    /// As a request line and a contract name it: `GET`.
    pub fn name(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }

    /// The method a call's word stands for.
    pub fn of(word: &str) -> Option<Method> {
        match word {
            "get" => Some(Method::Get),
            "post" => Some(Method::Post),
            "put" => Some(Method::Put),
            "patch" => Some(Method::Patch),
            "delete" => Some(Method::Delete),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Call {
    Run(Vec<String>),
    /// A request to a service. `get` has no body; the headers go in the order they are written.
    Http { method: Method, path: String, body: Option<String>, headers: Vec<(String, String)> },
    /// The app's first screen; in a browser, that path of the target (`/` when
    /// none is written).
    Open(Option<String>),
    /// The n-th (from 1) button, link, checkbox, radio, switch, tab, menu item or
    /// option of that name.
    Click { name: String, nth: Option<usize> },
    /// Type into a text field: what it holds becomes the text.
    Input { text: String, place: Place },
    /// Enter in a text field.
    Submit(Place),
    /// A key or a chord: `enter`, `cmd-s`.
    Press(String),
    /// Move the app's clock forward, in milliseconds.
    Advance(u64),
}

/// Which text field `input` and `submit` act on (DESIGN §8.1).
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    /// Nothing written: the first text field.
    First,
    /// `field: n`, from 1.
    Field(usize),
    /// `into: "<name>"`, with `nth: n` when written.
    Into { name: String, nth: Option<usize> },
}

impl Place {
    /// As the spec writes it among a call's arguments: `field: 2`, `into: "name"`.
    fn display(&self) -> Option<String> {
        match self {
            Place::First => None,
            Place::Field(n) => Some(format!("field: {n}")),
            Place::Into { name, nth: None } => Some(format!("into: {}", crate::json::quote(name))),
            Place::Into { name, nth: Some(n) } => Some(format!("into: {}, nth: {n}", crate::json::quote(name))),
        }
    }
}

impl Call {
    /// The call as the spec writes it after the target: `get("/total")`.
    pub fn display(&self) -> String {
        use crate::json::quote;
        match self {
            Call::Run(args) => {
                let a: Vec<String> = args.iter().map(|s| format!("\"{}\"", s)).collect();
                format!("run({})", a.join(", "))
            }
            // a baseline compares this text with what it keeps, so a call without headers stays
            // what it has always been: its path and its body as written, not escaped
            Call::Http { method, path, body, headers } => {
                let mut s = format!("{}(\"{}\"", method.word(), path);
                if let Some(b) = body {
                    s.push_str(&format!(", body: \"{}\"", b));
                }
                for (name, value) in headers {
                    s.push_str(&format!(", header {}: {}", quote(name), quote(value)));
                }
                s.push(')');
                s
            }
            Call::Open(None) => "open()".into(),
            Call::Open(Some(p)) => format!("open({})", quote(p)),
            Call::Click { name, nth: None } => format!("click({})", quote(name)),
            Call::Click { name, nth: Some(n) } => format!("click({}, nth: {n})", quote(name)),
            Call::Input { text, place } => match place.display() {
                None => format!("input({})", quote(text)),
                Some(p) => format!("input({}, {p})", quote(text)),
            },
            Call::Submit(place) => format!("submit({})", place.display().unwrap_or_default()),
            Call::Press(k) => format!("press({})", quote(k)),
            Call::Advance(ms) => format!("advance({ms})"),
        }
    }

    /// The word the call is written with: `run`, `click`.
    pub fn word(&self) -> &'static str {
        match self {
            Call::Run(_) => "run",
            Call::Http { method, .. } => method.word(),
            Call::Open(_) => "open",
            Call::Click { .. } => "click",
            Call::Input { .. } => "input",
            Call::Submit(_) => "submit",
            Call::Press(_) => "press",
            Call::Advance(_) => "advance",
        }
    }

    /// What a `when` with this call observes: a process, an HTTP exchange, or a
    /// screen.
    pub fn observes(&self) -> Observes {
        match self {
            Call::Run(_) => Observes::Process,
            Call::Http { .. } => Observes::Http,
            _ => Observes::Screen,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observes {
    Process,
    Http,
    Screen,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Subject {
    Stdout,
    Stderr,
    Exit,
    Status,
    /// A header by its name, lower-cased: names are compared without regard to case.
    Header(String),
    Body,
    BodyJson(String),
    /// What a GUI target shows (DESIGN §8).
    Screen,
}

impl Subject {
    pub fn label(&self) -> String {
        match self {
            Subject::Stdout => "stdout".into(),
            Subject::Stderr => "stderr".into(),
            Subject::Exit => "exit".into(),
            Subject::Status => "status".into(),
            Subject::Header(h) => format!("header \"{}\"", h),
            Subject::Body => "body".into(),
            Subject::BodyJson(p) => format!("body json \"{}\"", p),
            Subject::Screen => "screen".into(),
        }
    }

    /// The word the subject is written with.
    pub fn word(&self) -> &'static str {
        match self {
            Subject::Stdout => "stdout",
            Subject::Stderr => "stderr",
            Subject::Exit => "exit",
            Subject::Status => "status",
            Subject::Header(_) => "header",
            Subject::Body => "body",
            Subject::BodyJson(_) => "body json",
            Subject::Screen => "screen",
        }
    }

    pub fn observed_by(&self) -> Observes {
        match self {
            Subject::Stdout | Subject::Stderr | Subject::Exit => Observes::Process,
            Subject::Status | Subject::Header(_) | Subject::Body | Subject::BodyJson(_) => Observes::Http,
            Subject::Screen => Observes::Screen,
        }
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Subject::Exit | Subject::Status)
    }

    /// Whether the subject may be absent: a header not sent, a path that leads
    /// nowhere.
    pub fn may_be_absent(&self) -> bool {
        matches!(self, Subject::Header(_) | Subject::BodyJson(_))
    }
}

/// A value a check compares with.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    S(String),
    N(f64),
    Bool(bool),
    Null,
    /// A string with the values a claim kept in it (DESIGN §22): compared as text with whatever
    /// the subject gives, so `"{id}"` holds of the number 7 when the value kept is 7.
    Filled(String),
}

impl Value {
    /// As the spec writes it: `"5"`, `5`, `true`, `null`.
    pub fn shown(&self) -> String {
        match self {
            Value::S(s) | Value::Filled(s) => crate::json::quote(s),
            Value::N(n) => crate::json::render_num(*n),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".into(),
        }
    }
}

/// How a check compares (DESIGN §9).
#[derive(Debug, Clone)]
pub enum Matcher {
    Is(Value),
    IsNot(Value),
    Above(f64),
    Below(f64),
    AtLeast(f64),
    AtMost(f64),
    /// Both ends in.
    Between(f64, f64),
    Contains(String),
    NotContains(String),
    Matches(crate::regex::Pattern),
    NotMatches(crate::regex::Pattern),
    Exists,
    NotExists,
    /// `screen contains <node>`: some node matches, or `exactly n` do.
    ContainsNode(crate::screen::Pattern),
    /// `screen does not contain <node>`.
    NotContainsNode(crate::screen::Pattern),
}

impl Matcher {
    /// Its words as the spec writes them, without the value: `is not`, `is above`.
    pub fn words(&self) -> &'static str {
        match self {
            Matcher::Is(_) => "is",
            Matcher::IsNot(_) => "is not",
            Matcher::Above(_) => "is above",
            Matcher::Below(_) => "is below",
            Matcher::AtLeast(_) => "is at least",
            Matcher::AtMost(_) => "is at most",
            Matcher::Between(..) => "is between",
            Matcher::Contains(_) | Matcher::ContainsNode(_) => "contains",
            Matcher::NotContains(_) | Matcher::NotContainsNode(_) => "does not contain",
            Matcher::Matches(_) => "matches",
            Matcher::NotMatches(_) => "does not match",
            Matcher::Exists => "exists",
            Matcher::NotExists => "does not exist",
        }
    }

    /// Its value as the spec writes it, or nothing: `"5"`, `200 and 299`.
    pub fn value(&self) -> String {
        use crate::json::{quote, render_num};
        match self {
            Matcher::Is(v) | Matcher::IsNot(v) => v.shown(),
            Matcher::Above(n) | Matcher::Below(n) | Matcher::AtLeast(n) | Matcher::AtMost(n) => render_num(*n),
            Matcher::Between(a, b) => format!("{} and {}", render_num(*a), render_num(*b)),
            Matcher::Contains(s) | Matcher::NotContains(s) => quote(s),
            Matcher::Matches(p) | Matcher::NotMatches(p) => p.shown(),
            Matcher::Exists | Matcher::NotExists => String::new(),
            Matcher::ContainsNode(p) | Matcher::NotContainsNode(p) => p.shown(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Check {
    pub subject: Subject,
    pub matcher: Matcher,
    /// Where the subject is written.
    pub pos: Pos,
}

/// `within <n> seconds` after a call (DESIGN §18): the call is made again until every check under
/// its `when` holds on one observation, for at most `secs` seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wait {
    /// From 1 to [`Wait::MOST`].
    pub secs: u64,
    /// Where `within` is written.
    pub pos: Pos,
}

impl Wait {
    /// The longest a step waits, in seconds.
    pub const MOST: u64 = 60;
    /// The time between an observation and the next call.
    pub const EVERY: std::time::Duration = std::time::Duration::from_millis(100);

    /// As a spec writes it: `within 10 seconds`, `within 1 second`.
    pub fn display(&self) -> String {
        format!("within {} {}", self.secs, if self.secs == 1 { "second" } else { "seconds" })
    }
}

/// Where a `keep` reads its value (DESIGN §22).
#[derive(Debug, Clone, PartialEq)]
pub enum KeepFrom {
    /// What the `when` before it observed: stdout, stderr, a header, the body, a JSON path.
    Answer(Subject),
    /// geas's own environment. The value is told to nobody (DESIGN §22).
    Env(String),
}

/// `keep <source> as <name>`.
#[derive(Debug, Clone)]
pub struct Keep {
    pub from: KeepFrom,
    pub name: String,
    /// Where `keep` is written.
    pub pos: Pos,
}

impl Keep {
    /// The source as the spec writes it: `keep body json ".id"`.
    pub fn label(&self) -> String {
        match &self.from {
            KeepFrom::Answer(s) => format!("keep {}", s.label()),
            KeepFrom::Env(var) => format!("keep env {}", crate::json::quote(var)),
        }
    }

    /// As the spec writes it: `keep body json ".id" as id`.
    pub fn display(&self) -> String {
        format!("{} as {}", self.label(), self.name)
    }
}

#[derive(Debug, Clone)]
pub enum Step {
    /// `pos` is where `when` is written; `wait`, what its `within` says.
    When { target: String, call: Call, pos: Pos, wait: Option<Wait> },
    Then(Check),
    Keep(Keep),
}

#[derive(Debug, Clone)]
pub struct Claim {
    pub name: String,
    /// In the order written, the steps of an `in any order` block among them.
    pub steps: Vec<Step>,
    /// Where `claim` is written.
    pub pos: Pos,
    /// Its `in any order` block (DESIGN §23).
    pub any_order: Option<AnyOrder>,
}

/// `in any order { … }` (DESIGN §23): steps of a claim whose order is not decided. Each is a `when`
/// and the checks and `keep`s under it, and the claim runs once in every order they can come in. Two
/// steps written the same are one and the same in an order, so an order is counted once.
#[derive(Debug, Clone)]
pub struct AnyOrder {
    /// Where `in` is written.
    pub pos: Pos,
    /// Each step of the block, as the range of the claim's steps it takes, in the order written.
    pub steps: Vec<std::ops::Range<usize>>,
}

/// One order a claim runs in: its number from 1, its steps in that order, where each of them is
/// written among the claim's steps, and the lines of the block's `when`s as they come in it.
#[derive(Debug, Clone)]
pub struct Order {
    pub number: usize,
    pub steps: Vec<Step>,
    pub places: Vec<usize>,
    pub lines: Vec<usize>,
}

impl AnyOrder {
    /// The most orders one block may make: six steps, none the same.
    pub const MOST: usize = 720;

    /// For each step of the block, the place of the first step written the same as it: the call, its
    /// `within`, and the checks and `keep`s under it, whatever the spaces, the comments, or `then` and
    /// `and`.
    pub fn kinds(&self, steps: &[Step]) -> Vec<usize> {
        let written: Vec<String> = self.steps.iter().map(|r| steps[r.clone()].iter().map(Step::as_written).collect::<Vec<_>>().join("\n")).collect();
        (0..written.len()).map(|i| written.iter().position(|w| *w == written[i]).expect("itself")).collect()
    }

    /// How many orders the block makes, n! over the factorial of how many times each step is written,
    /// in decimal: a block of many steps makes more than a machine word holds.
    pub fn count(&self, steps: &[Step]) -> String {
        let kinds = self.kinds(steps);
        let mut times: Vec<usize> = Vec::new();
        for (i, k) in kinds.iter().enumerate() {
            if *k == i {
                times.push(kinds.iter().filter(|x| *x == k).count());
            }
        }
        orders_of(&times)
    }

    /// Every order, each as the places of the block's steps (from 0) in the order they run: the order
    /// written first, then the rest as a dictionary sorts them by those places. Of steps written the
    /// same, the one written first comes first. None when there are more than [`AnyOrder::MOST`].
    pub fn orders(&self, steps: &[Step]) -> Option<Vec<Vec<usize>>> {
        if self.count(steps).parse::<usize>().map_or(true, |n| n > AnyOrder::MOST) {
            return None;
        }
        let kinds = self.kinds(steps);
        let mut seq = kinds.clone();
        seq.sort_unstable();
        let mut out = Vec::new();
        loop {
            // the k-th time a kind comes, it is the k-th step of that kind as written
            let mut used = vec![0usize; kinds.len()];
            let order: Vec<usize> = seq
                .iter()
                .map(|k| {
                    let place = kinds.iter().enumerate().filter(|(_, x)| *x == k).nth(used[*k]).map(|(i, _)| i).expect("as many as there are");
                    used[*k] += 1;
                    place
                })
                .collect();
            out.push(order);
            if !next_permutation(&mut seq) {
                break;
            }
        }
        out.sort();
        Some(out)
    }
}

/// The next permutation of `v` as a dictionary sorts them, each told once however often a member
/// comes; false after the last.
fn next_permutation(v: &mut [usize]) -> bool {
    if v.len() < 2 {
        return false;
    }
    let mut i = v.len() - 1;
    while i > 0 && v[i - 1] >= v[i] {
        i -= 1;
    }
    if i == 0 {
        return false;
    }
    let mut j = v.len() - 1;
    while v[j] <= v[i - 1] {
        j -= 1;
    }
    v.swap(i - 1, j);
    v[i..].reverse();
    true
}

/// How many orders things can come in, each kind as many times as `times` says: n! / (k1! k2! …),
/// in decimal. Worked with digits of base 10⁹, multiplying by the next count of things and dividing
/// by how many of its kind have come, which leaves a whole number at every step.
pub fn orders_of(times: &[usize]) -> String {
    const BASE: u64 = 1_000_000_000;
    // little end first
    let mut n: Vec<u64> = vec![1];
    let mut total: u64 = 0;
    for &k in times {
        for j in 1..=k as u64 {
            total += 1;
            let mut carry = 0;
            for d in n.iter_mut() {
                let x = *d * total + carry;
                *d = x % BASE;
                carry = x / BASE;
            }
            while carry > 0 {
                n.push(carry % BASE);
                carry /= BASE;
            }
            let mut rest = 0;
            for d in n.iter_mut().rev() {
                let x = rest * BASE + *d;
                *d = x / j;
                rest = x % j;
            }
            while n.len() > 1 && n.last() == Some(&0) {
                n.pop();
            }
        }
    }
    let mut s = n.last().expect("a digit").to_string();
    for d in n.iter().rev().skip(1) {
        s.push_str(&format!("{d:09}"));
    }
    s
}

impl Step {
    /// What the step is, without where it is written: two steps that give the same are written the
    /// same.
    fn as_written(&self) -> String {
        match self {
            Step::When { target, call, wait, .. } => format!("when {target:?} {call:?} {:?}", wait.map(|w| w.secs)),
            Step::Then(c) => format!("then {:?} {:?}", c.subject, c.matcher),
            Step::Keep(k) => format!("keep {:?} {:?}", k.from, k.name),
        }
    }

    /// The line it is written on.
    pub fn line(&self) -> usize {
        match self {
            Step::When { pos, .. } => pos.line,
            Step::Then(c) => c.pos.line,
            Step::Keep(k) => k.pos.line,
        }
    }
}

impl Claim {
    /// The orders the claim runs in, numbered from 1, or None when it has no `in any order` block or
    /// the block makes more orders than [`AnyOrder::MOST`] (E018 refuses that).
    pub fn orders(&self) -> Option<Vec<Order>> {
        let block = self.any_order.as_ref()?;
        let (start, end) = (block.steps.first()?.start, block.steps.last()?.end);
        let orders = block.orders(&self.steps)?;
        Some(
            orders
                .into_iter()
                .enumerate()
                .map(|(i, order)| {
                    let mut places: Vec<usize> = (0..start).collect();
                    let mut lines = Vec::new();
                    for u in order {
                        let r = block.steps[u].clone();
                        lines.push(self.steps[r.start].line());
                        places.extend(r);
                    }
                    places.extend(end..self.steps.len());
                    Order { number: i + 1, steps: places.iter().map(|p| self.steps[*p].clone()).collect(), places, lines }
                })
                .collect(),
        )
    }
}

#[derive(Debug, Clone)]
pub enum Mask {
    Header(String),
    BodyJson(String),
    /// `mask screen <node>`: the nodes it matches, and what is under them.
    Screen(crate::screen::Pattern),
}

#[derive(Debug, Clone)]
pub struct Spec {
    pub targets: Vec<Target>,
    pub claims: Vec<Claim>,
    pub masks: Vec<Mask>,
    /// What the parser says of a spec that runs all the same: `geas check` shows it.
    pub warnings: Vec<crate::diag::Diag>,
}

impl Spec {
    pub fn target(&self, name: &str) -> Option<&Target> {
        self.targets.iter().find(|t| t.name == name)
    }

    /// The `mask screen` patterns.
    pub fn screen_masks(&self) -> Vec<&crate::screen::Pattern> {
        self.masks
            .iter()
            .filter_map(|m| match m {
                Mask::Screen(p) => Some(p),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http(method: Method, path: &str, body: Option<&str>, headers: &[(&str, &str)]) -> Call {
        Call::Http { method, path: path.into(), body: body.map(str::to_string), headers: headers.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect() }
    }

    /// A baseline compares this text with what it keeps: a call without headers is the bytes it has
    /// always been, its path and its body not escaped, so an old baseline still reads.
    #[test]
    fn a_call_is_written_as_it_always_was() {
        assert_eq!(http(Method::Get, "/x", None, &[]).display(), r#"get("/x")"#);
        assert_eq!(http(Method::Post, "/x", None, &[]).display(), r#"post("/x")"#);
        assert_eq!(http(Method::Post, "/x", Some("b"), &[]).display(), r#"post("/x", body: "b")"#);
        assert_eq!(http(Method::Post, "/x\"y", Some("{\"a\":\"b\"}"), &[]).display(), "post(\"/x\"y\", body: \"{\"a\":\"b\"}\")");
        assert_eq!(http(Method::Post, "/x", Some("a\nb"), &[]).display(), "post(\"/x\", body: \"a\nb\")");
    }

    #[test]
    fn the_new_calls_and_the_headers_come_after_the_rest() {
        assert_eq!(http(Method::Put, "/x", Some("b"), &[]).display(), r#"put("/x", body: "b")"#);
        assert_eq!(http(Method::Patch, "/x", None, &[]).display(), r#"patch("/x")"#);
        assert_eq!(http(Method::Delete, "/x", None, &[("a", "b")]).display(), r#"delete("/x", header "a": "b")"#);
        assert_eq!(http(Method::Get, "/x", None, &[("b", "1"), ("a", "2"), ("b", "3")]).display(), r#"get("/x", header "b": "1", header "a": "2", header "b": "3")"#);
        assert_eq!(http(Method::Post, "/x", Some("b"), &[("a", "say \"c\"")]).display(), r#"post("/x", body: "b", header "a": "say \"c\"")"#);
    }

    #[test]
    fn the_five_methods_by_their_words() {
        for (word, name, m) in [("get", "GET", Method::Get), ("post", "POST", Method::Post), ("put", "PUT", Method::Put), ("patch", "PATCH", Method::Patch), ("delete", "DELETE", Method::Delete)] {
            assert_eq!(Method::of(word), Some(m));
            assert_eq!((m.word(), m.name()), (word, name));
        }
        for word in ["head", "options", "GET", ""] {
            assert_eq!(Method::of(word), None);
        }
    }

    /// A block of `when`s on `/a`, `/b`…, each followed by a check: the letters say which steps are
    /// written the same.
    fn block(letters: &str) -> (Vec<Step>, AnyOrder) {
        let mut steps = Vec::new();
        let mut ranges = Vec::new();
        for (i, c) in letters.chars().enumerate() {
            let at = Pos { line: 10 + 2 * i, col: 3 };
            let start = steps.len();
            steps.push(Step::When { target: "api".into(), call: http(Method::Post, &format!("/{c}"), None, &[]), pos: at, wait: None });
            steps.push(Step::Then(Check { subject: Subject::Status, matcher: Matcher::Is(Value::N(200.0)), pos: Pos { line: at.line + 1, col: 3 } }));
            ranges.push(start..steps.len());
        }
        (steps, AnyOrder { pos: Pos { line: 9, col: 3 }, steps: ranges })
    }

    #[test]
    fn the_orders_of_a_block_the_written_one_first() {
        let (steps, b) = block("abc");
        assert_eq!(b.count(&steps), "6");
        let orders = b.orders(&steps).expect("six");
        assert_eq!(orders, vec![vec![0, 1, 2], vec![0, 2, 1], vec![1, 0, 2], vec![1, 2, 0], vec![2, 0, 1], vec![2, 1, 0]]);
    }

    #[test]
    fn steps_written_the_same_are_one_in_an_order() {
        // four steps, one of them twice: 4! / 2! = 12
        let (steps, b) = block("abca");
        assert_eq!(b.kinds(&steps), vec![0, 1, 2, 0]);
        assert_eq!(b.count(&steps), "12");
        let orders = b.orders(&steps).expect("twelve");
        assert_eq!(orders.len(), 12);
        // the written order first; of the two the same, the one written first comes first
        assert_eq!(orders[0], vec![0, 1, 2, 3]);
        assert!(orders.iter().all(|o| o.iter().position(|u| *u == 0) < o.iter().position(|u| *u == 3)), "{orders:?}");
        let mut seen = orders.clone();
        seen.dedup();
        assert_eq!(seen.len(), 12);
        // the same step twice and nothing else: one order
        let (steps, b) = block("aa");
        assert_eq!(b.orders(&steps), Some(vec![vec![0, 1]]));
    }

    #[test]
    fn a_block_of_more_than_720_orders_has_none() {
        let (steps, b) = block("abcdef");
        assert_eq!(b.orders(&steps).map(|o| o.len()), Some(720));
        let (steps, b) = block("abcdefg");
        assert_eq!(b.count(&steps), "5040");
        assert_eq!(b.orders(&steps), None);
        // steps written the same make fewer orders: eight steps, five of them the same, 8!/5! = 336
        let (steps, b) = block("abcdaaaa");
        assert_eq!(b.count(&steps), "336");
        assert_eq!(b.orders(&steps).map(|o| o.len()), Some(336));
        // and eight with two the same, 8!/2! = 20160
        let (steps, b) = block("abcdefga");
        assert_eq!(b.count(&steps), "20160");
        assert_eq!(b.orders(&steps), None);
    }

    #[test]
    fn counts_beyond_a_machine_word() {
        assert_eq!(orders_of(&[1; 6]), "720");
        assert_eq!(orders_of(&[2, 1, 1]), "12");
        assert_eq!(orders_of(&[]), "1");
        // 30! and 40! / (20! 20!)
        assert_eq!(orders_of(&[1; 30]), "265252859812191058636308480000000");
        assert_eq!(orders_of(&[20, 20]), "137846528820");
    }

    #[test]
    fn a_claim_in_each_of_its_orders() {
        let (mut steps, mut b) = block("ab");
        steps.insert(0, Step::When { target: "api".into(), call: http(Method::Post, "/first", None, &[]), pos: Pos { line: 8, col: 3 }, wait: None });
        steps.push(Step::When { target: "api".into(), call: http(Method::Get, "/last", None, &[]), pos: Pos { line: 20, col: 3 }, wait: None });
        b.steps = b.steps.iter().map(|r| r.start + 1..r.end + 1).collect();
        let claim = Claim { name: "c".into(), steps, pos: Pos::default(), any_order: Some(b) };
        let orders = claim.orders().expect("two");
        assert_eq!(orders.len(), 2);
        assert_eq!((orders[0].number, orders[0].places.clone(), orders[0].lines.clone()), (1, vec![0, 1, 2, 3, 4, 5], vec![10, 12]));
        assert_eq!((orders[1].number, orders[1].places.clone(), orders[1].lines.clone()), (2, vec![0, 3, 4, 1, 2, 5], vec![12, 10]));
        assert_eq!(orders[1].steps.iter().map(Step::line).collect::<Vec<_>>(), vec![8, 12, 13, 10, 11, 20]);
    }

    #[test]
    fn a_keep_as_its_spec_writes_it() {
        let keep = |from| Keep { from, name: "id".into(), pos: Pos::default() };
        assert_eq!(keep(KeepFrom::Answer(Subject::BodyJson(".id".into()))).display(), r#"keep body json ".id" as id"#);
        assert_eq!(keep(KeepFrom::Answer(Subject::Stdout)).display(), "keep stdout as id");
        assert_eq!(keep(KeepFrom::Env("KEY".into())).display(), r#"keep env "KEY" as id"#);
        assert_eq!(keep(KeepFrom::Env("KEY".into())).label(), r#"keep env "KEY""#);
    }
}
