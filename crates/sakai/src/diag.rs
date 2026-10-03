//! Diagnostics (DESIGN 5.1): a code, a place, what is wrong in English and Japanese, notes,
//! what to write instead, and what is involved — the artifacts, the lines of the `.ctx` and the
//! relationships the diagnostic is about, one a line.
//!
//! What every language's diagnostic has is ritsu-base's (`ritsu_base::diag`): the headline, the
//! line of the source, `= ` notes, the fixed line, and the JSON with its keys in English whatever
//! the language. What is sakai's is [`Refs`], the things involved, printed after the fixed line.
//! A message is spaced and capitalized as the suite writes it ([`ritsu_base::text::spaced`]).
//!
//! A place is written twice (DESIGN 2.4): for a person from where sakai was run, the way the
//! path given was written ([`shown`]), and in the JSON from the root, with the root beside it.

use crate::naming::Name;
use crate::paths::shown;
use ritsu_base::diag::Extra;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text, pad, spaced, width};

pub use ritsu_base::diag::Severity;

/// One thing a diagnostic is about: the context it belongs to, where it is (a name, or a line of
/// a file), and what it is there.
#[derive(Clone, Debug, PartialEq)]
pub struct Ref {
    pub context: Option<String>,
    pub name: Option<Name>,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub what: Text,
    /// Where sakai read it: `proto import`, or (from stage C) the field of a tool's api.
    pub via: Option<String>,
}

impl Ref {
    /// A line of a file: `ctx/請求.ctx:12`, or an import line of a `.proto`.
    pub fn line(context: Option<&str>, file: &str, line: usize, what: Text) -> Ref {
        Ref { context: context.map(String::from), name: None, file: Some(file.to_string()), line: Some(line), what, via: None }
    }

    /// A named artifact or element.
    pub fn name(context: Option<&str>, name: Name, what: Text) -> Ref {
        Ref { context: context.map(String::from), name: Some(name), file: None, line: None, what, via: None }
    }

    pub fn via(mut self, v: &str) -> Ref {
        self.via = Some(v.to_string());
        self
    }

    /// `ctx/請求.ctx:12`, `proto/x.proto:5`, or the name's text. The place in a file is written from
    /// where sakai was run; a name keeps its path from the root, as names do everywhere (DESIGN 2.4).
    pub fn place(&self) -> String {
        match (&self.file, self.line, &self.name) {
            (Some(f), Some(l), _) => format!("{}:{l}", shown(f)),
            (Some(f), None, _) => shown(f),
            (None, _, Some(n)) => n.text(),
            (None, _, None) => String::new(),
        }
    }
}

/// sakai's part of a diagnostic: what is involved.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Refs(pub Vec<Ref>);

impl Extra for Refs {
    fn after_fix(&self, lang: Lang, out: &mut String) {
        let refs = &self.0;
        if refs.is_empty() {
            return;
        }
        out.push_str(if lang == Lang::Ja { "  関わるもの:\n" } else { "  involved:\n" });
        let cw = refs.iter().map(|r| r.context.as_deref().map(width).unwrap_or(0)).max().unwrap_or(0);
        let pw = refs.iter().map(|r| width(&r.place())).max().unwrap_or(0);
        for r in refs {
            // The context's column, when any line has one.
            let c = if cw == 0 { String::new() } else { format!("{}  ", pad(r.context.as_deref().unwrap_or(""), cw)) };
            // What a reference is, is often a line of a `.ctx` or a proto: written as it is.
            let what = r.what.get(lang).to_string();
            let line = if what.is_empty() { format!("{c}{}", r.place()) } else { format!("{c}{}  {what}", pad(&r.place(), pw)) };
            out.push_str(&format!("      {}\n", line.trim_end()));
        }
    }

    /// The things involved; a file's path from the root, as every path of the JSON is.
    fn json(&self, lang: Lang) -> Vec<(String, Json)> {
        let refs = self.0.iter().map(|r| {
            Json::obj([
                ("context", Json::opt_str(r.context.clone())),
                ("name", r.name.as_ref().map(Name::to_json).unwrap_or(Json::Null)),
                ("file", Json::opt_str(r.file.clone())),
                ("line", r.line.into()),
                ("what", Json::str(r.what.get(lang))),
                ("via", Json::opt_str(r.via.clone())),
            ])
        });
        vec![("references".into(), Json::arr(refs))]
    }

    fn say(&self, t: &Text, lang: Lang) -> String {
        spaced(t, lang)
    }
}

/// A diagnostic of sakai's: ritsu-base's, with what is involved.
pub type Diag = ritsu_base::diag::Diag<Refs>;

/// At a line and a column of a file (a path from the root).
pub fn at(code: &'static str, file: &str, line: usize, col: usize, message: Text) -> Diag {
    Diag::at(code, &shown(file), line, col, message).rel(file)
}

/// About a whole file: a tool's api says no line, or the finding is about the file itself.
pub fn whole(code: &'static str, file: &str, message: Text) -> Diag {
    Diag::whole(code, &shown(file), message).rel(file)
}

/// What sakai adds to a diagnostic as it is built.
pub trait DiagExt {
    /// One more thing involved.
    fn refer(self, r: Ref) -> Self;
}

impl DiagExt for Diag {
    fn refer(mut self, r: Ref) -> Diag {
        self.extra.0.push(r);
        self
    }
}

/// ritsu-base's JSON as serde_json's, for the outputs sakai writes with serde_json.
pub fn value(j: &Json) -> serde_json::Value {
    serde_json::from_str(&j.compact()).expect("ritsu-base writes JSON serde_json reads")
}

pub fn has_errors(diags: &[Diag]) -> bool {
    ritsu_base::diag::has_errors(diags)
}
