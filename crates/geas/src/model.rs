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

#[derive(Debug, Clone)]
pub enum Call {
    Run(Vec<String>),
    Get(String),
    Post { path: String, body: Option<String> },
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
            Call::Get(p) => format!("get(\"{}\")", p),
            Call::Post { path, body } => match body {
                Some(b) => format!("post(\"{}\", body: \"{}\")", path, b),
                None => format!("post(\"{}\")", path),
            },
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
            Call::Get(_) => "get",
            Call::Post { .. } => "post",
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
            Call::Get(_) | Call::Post { .. } => Observes::Http,
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
}

impl Value {
    /// As the spec writes it: `"5"`, `5`, `true`, `null`.
    pub fn shown(&self) -> String {
        match self {
            Value::S(s) => crate::json::quote(s),
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

#[derive(Debug, Clone)]
pub enum Step {
    /// `pos` is where `when` is written.
    When { target: String, call: Call, pos: Pos },
    Then(Check),
}

#[derive(Debug, Clone)]
pub struct Claim {
    pub name: String,
    pub steps: Vec<Step>,
    /// Where `claim` is written.
    pub pos: Pos,
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
