//! What a `.req` says, as the parser reads it (DESIGN 1).

use crate::date::{Day, Period};
use ritsu_base::text::Text;
use crate::names::Written;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug)]
pub struct ReqFile {
    pub header: Header,
    pub description: Option<(String, Span)>,
    pub roles: Vec<RoleDecl>,
    pub sources: Vec<SourceDecl>,
    pub scopes: Vec<ScopeDecl>,
    pub requirements: Vec<ReqDecl>,
}

#[derive(Clone, Debug)]
pub struct Header {
    pub name: String,
    pub version: u32,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct RoleDecl {
    pub name: String,
    pub description: Option<String>,
    pub span: Span,
}

/// Where a law is read from: e-Gov or the eCFR (ritsu-base's).
pub use ritsu_base::sources::LawDb;

#[derive(Clone, Debug)]
pub struct SourceDecl {
    pub name: String,
    pub span: Span,
    pub kind: SourceKind,
}

#[derive(Clone, Debug)]
pub enum SourceKind {
    /// `law [egov|ecfr] "<id>" asof <date>`, and a pin line under it per article.
    Law { db: LawDb, id: String, asof: Day, pins: Vec<PinLine> },
    /// `file "<path>" [url "<url>"] [sha256:<16>]`.
    File { path: String, path_span: Span, url: Option<String>, pin: Option<String> },
    /// `<tool> "<path>" source <name>`: a source a rule or a calendar pins (DESIGN 1.4).
    Borrowed { naming: Written },
    /// `openspec "<path>"`, an OpenSpec spec, and a pin line under it per requirement (DESIGN 20).
    OpenSpec { path: String, path_span: Span, pins: Vec<PinLine> },
}

#[derive(Clone, Debug)]
pub struct PinLine {
    pub fragment: String,
    pub span: Span,
    pub pin: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ScopeDecl {
    pub naming: Written,
    pub span: Span,
}

/// `<name> [v<n>]` where a requirement is pointed at (`from`, `replaces`).
#[derive(Clone, Debug)]
pub struct ReqRef {
    pub name: String,
    pub version: Option<u32>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    Satisfied,
    Verified,
}

impl Side {
    pub fn word(self) -> &'static str {
        match self {
            Side::Satisfied => "satisfied",
            Side::Verified => "verified",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReqDecl {
    pub name: String,
    pub alias: Option<(String, Span)>,
    pub version: Option<(u32, Span)>,
    /// Where the name is.
    pub span: Span,
    pub text: Option<(String, Span)>,
    pub in_force: Option<(Period, Span)>,
    pub owner: Option<(String, Span)>,
    pub replaces: Vec<ReqRef>,
    pub from: Vec<FromLine>,
    pub decided: Vec<Decided>,
    pub links: Vec<LinkLine>,
    pub waivers: Vec<Waiver>,
}

#[derive(Clone, Debug)]
pub enum FromWhat {
    /// `from @<source> [<article>[, <article>…]]`.
    Cite { source: String, source_span: Span, fragments: Vec<(String, Span)> },
    /// `from <requirement> [v<n>]`.
    Req(ReqRef),
}

#[derive(Clone, Debug)]
pub struct FromLine {
    pub span: Span,
    pub what: FromWhat,
    pub record: Option<RecordLine>,
}

#[derive(Clone, Debug)]
pub struct Decided {
    pub date: Day,
    pub by: (String, Span),
    pub why: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct LinkLine {
    pub span: Span,
    pub side: Side,
    pub naming: Written,
    pub record: Option<RecordLine>,
}

#[derive(Clone, Debug)]
pub struct Waiver {
    pub span: Span,
    pub side: Side,
    pub why: String,
    pub record: Option<RecordLine>,
}

/// `reviewed …` under a link, `approved …` under a waiver (DESIGN 4.2).
#[derive(Clone, Debug)]
pub struct RecordLine {
    pub line: usize,
    pub indent: usize,
    /// What it says, or where and how it is broken (E305, said at stage 6).
    pub parsed: Result<Record, (usize, Text)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub date: Day,
    pub by: String,
    pub by_span: Span,
    /// The hashes of the link's upper ends, in the order the link names them; for an
    /// approval, the requirement's.
    pub up: Vec<String>,
    /// The hash of the lower end; None for an approval.
    pub down: Option<String>,
}
