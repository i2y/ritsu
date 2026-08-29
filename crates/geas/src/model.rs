#[derive(Debug, Clone)]
pub enum TargetKind {
    Run(String),
    Serve { cmd: String, port: u16 },
}

#[derive(Debug, Clone)]
pub struct Target {
    pub name: String,
    pub kind: TargetKind,
    #[allow(dead_code)] // kept for future diagnostics (diff-to-claim coverage)
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum Call {
    Run(Vec<String>),
    Get(String),
    Post { path: String, body: Option<String> },
}

#[derive(Debug, Clone)]
pub enum Subject {
    Stdout,
    Stderr,
    Exit,
    Status,
    Body,
    BodyJson(String),
}

impl Subject {
    pub fn label(&self) -> String {
        match self {
            Subject::Stdout => "stdout".into(),
            Subject::Stderr => "stderr".into(),
            Subject::Exit => "exit".into(),
            Subject::Status => "status".into(),
            Subject::Body => "body".into(),
            Subject::BodyJson(p) => format!("body json \"{}\"", p),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Matcher {
    Is,
    Contains,
}

#[derive(Debug, Clone)]
pub enum Expected {
    S(String),
    N(f64),
}

#[derive(Debug, Clone)]
pub struct Check {
    pub subject: Subject,
    pub matcher: Matcher,
    pub expected: Expected,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum Step {
    When { target: String, call: Call, line: usize },
    Then(Check),
}

#[derive(Debug, Clone)]
pub struct Claim {
    pub name: String,
    pub steps: Vec<Step>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum Mask {
    Header(String),
    BodyJson(String),
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
}
