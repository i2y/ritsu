//! The three levels of the tests (DESIGN 10.2), and what each needs.
//!
//! | level | what runs |
//! |---|---|
//! | `fast` | nothing but cargo (git may be used, and curl, to ask a server the test itself started) |
//! | `tools` | the tools installed on the machine: compilers and checkers of generated code, PostgreSQL, TigerBeetle, Chrome, Mermaid, xmllint, Lean, the linters, pixie's greeter, the binaries of the other languages a test runs (rulec, koyomi, chobo and dandori for sakai's) |
//! | `platforms` | services and clusters a test starts, and the network: Temporal, Argo on kind, LocalStack, Ollama, TypeSafe, e-Gov and the eCFR, Kani |
//!
//! `RITSU_TEST_LEVEL` names the highest level to run; a test that needs a higher one prints a
//! SKIP line and passes. Without it, every test runs what the machine has, as the crates did
//! before ritsu.

/// A level, lowest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Fast,
    Tools,
    Platforms,
}

impl Level {
    pub fn parse(s: &str) -> Option<Level> {
        match s.trim() {
            "fast" => Some(Level::Fast),
            "tools" => Some(Level::Tools),
            "platforms" => Some(Level::Platforms),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Level::Fast => "fast",
            Level::Tools => "tools",
            Level::Platforms => "platforms",
        }
    }
}

/// `RITSU_TEST_LEVEL`, when it names a level. A value that names none stops the test: a level
/// misspelled in CI would otherwise run everything and look like what was asked.
pub fn level() -> Option<Level> {
    let v = std::env::var("RITSU_TEST_LEVEL").ok().filter(|s| !s.is_empty())?;
    match Level::parse(&v) {
        Some(l) => Some(l),
        None => panic!("RITSU_TEST_LEVEL={v} is not a level; it is fast, tools or platforms"),
    }
}

/// Whether a test of level `l` runs.
pub fn allows(l: Level) -> bool {
    level().is_none_or(|top| l <= top)
}

/// What a test needs beyond cargo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    Node,
    Python,
    Go,
    Rustc,
    Ruby,
    Php,
    Swift,
    Java,
    Buf,
    Protoc,
    Postgres,
    TigerBeetle,
    Chrome,
    Mermaid,
    Xmllint,
    Lean,
    /// sakai's import-linter, dependency-cruiser, ArchUnit, Context Mapper, go-arch-lint.
    Linters,
    /// A greeter built with pixie (`GEAS_PIXIE_GREETER`), which geas's tests drive.
    Pixie,
    Temporal,
    Argo,
    LocalStack,
    Ollama,
    TypeSafe,
    /// e-Gov, the eCFR, or any server outside the machine.
    Network,
    Kani,
}

impl Need {
    pub fn level(self) -> Level {
        match self {
            Need::Temporal | Need::Argo | Need::LocalStack | Need::Ollama | Need::TypeSafe | Need::Network | Need::Kani => Level::Platforms,
            _ => Level::Tools,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Need::Node => "node",
            Need::Python => "python",
            Need::Go => "go",
            Need::Rustc => "rustc",
            Need::Ruby => "ruby",
            Need::Php => "php",
            Need::Swift => "swift",
            Need::Java => "java",
            Need::Buf => "buf",
            Need::Protoc => "protoc",
            Need::Postgres => "postgres",
            Need::TigerBeetle => "tigerbeetle",
            Need::Chrome => "chrome",
            Need::Mermaid => "mermaid",
            Need::Xmllint => "xmllint",
            Need::Lean => "lean",
            Need::Linters => "linters",
            Need::Pixie => "pixie's greeter",
            Need::Temporal => "temporal",
            Need::Argo => "argo",
            Need::LocalStack => "localstack",
            Need::Ollama => "ollama",
            Need::TypeSafe => "typesafe",
            Need::Network => "network",
            Need::Kani => "kani",
        }
    }
}

/// Whether the level lets a test run what needs `n`. When it does not, the SKIP line says so
/// and the test returns.
pub fn need(n: Need) -> bool {
    let l = n.level();
    if allows(l) {
        return true;
    }
    let top = level().map(Level::word).unwrap_or("");
    crate::skip::skip_for(crate::skip::Why::Level, &format!("needs {} (the {} level); RITSU_TEST_LEVEL is {top}", n.word(), l.word()));
    false
}

/// What a test asks before it runs something beyond cargo: whether the level lets it ([`need`],
/// a SKIP of the level when not), and whether it is on the machine (`found`, a SKIP saying
/// `reason` when not). True when the test goes on. rulec's and dandori's tests ask this way,
/// one question where they had asked the second alone.
pub fn ready(n: Need, found: impl FnOnce() -> bool, reason: &str) -> bool {
    if !need(n) {
        return false;
    }
    if found() {
        return true;
    }
    crate::skip::skip(reason);
    false
}
