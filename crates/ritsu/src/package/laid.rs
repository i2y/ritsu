//! What kaname says of the code ringi lays for a project's requests, as `ritsu gen` says it (ringi's
//! DESIGN 9.2, 12.3). The glue of each request and the machine of the tickets are code ringi writes,
//! so anything kaname says of them is ringi's bug: an error, a warning, a function that is not
//! total, a call across the languages that does not fit. It stops the package, and it is said as
//! ringi's E201 at the request (the machine's at the first request), with kaname's codes and lines
//! and none of kaname's words, which are of code the person never wrote.

use ringi::ports::GlueSaid;
use ritsu_base::json::Json;
use ritsu_base::naming::Tool;
use ritsu_base::text::Lang;
use ritsu_ports::{Checked, Finding};
use ritsu_project::requests::{Laid, TICKET};
use ritsu_project::{File, Joined, Project};

/// The request a laid module of kaname is of, by its path from the root, and whether it is the
/// machine: the glue's own request; the machine's, the first request.
fn request_of<'a>(laid: &'a Laid, rel: &str) -> Option<(&'a File, bool)> {
    if rel == TICKET {
        return laid.kinds.first().map(|k| (&k.request, true));
    }
    laid.kinds.iter().find(|k| k.glue == rel).map(|k| (&k.request, false))
}

/// What kaname says of a laid module in its unit: each diagnostic, and each function whose effect
/// is not total, at its first place that can fail (or its first call, for one that may not stop).
fn said_of(u: &Checked, machine: bool) -> Vec<GlueSaid> {
    let mut out: Vec<GlueSaid> = u.findings().map(|f| GlueSaid { code: f.code.clone(), line: f.line, machine, not_total: false }).collect();
    for f in u.json.get("functions").and_then(Json::as_arr).unwrap_or(&[]) {
        let effect = f.get("effect");
        let fails: Vec<&str> = effect.and_then(|e| e.get("fails")).and_then(Json::as_arr).map(|a| a.iter().filter_map(Json::as_str).collect()).unwrap_or_default();
        let diverges = effect.and_then(|e| e.get("diverges")).and_then(Json::as_bool).unwrap_or(false);
        if fails.is_empty() && !diverges {
            continue;
        }
        let first_line = |key: &str| f.get(key).and_then(Json::as_arr).and_then(|a| a.first()).and_then(|p| p.get("line")).and_then(Json::as_int).and_then(|n| usize::try_from(n).ok());
        let written = match (fails.is_empty(), diverges) {
            (true, _) => "diverges".to_string(),
            (false, false) => format!("fails {}", fails.join(", ")),
            (false, true) => format!("fails {} diverges", fails.join(", ")),
        };
        out.push(GlueSaid { code: written, line: first_line("places").or_else(|| first_line("calls")), machine, not_total: true });
    }
    out
}

fn add<'a>(by: &mut Vec<(&'a File, Vec<GlueSaid>)>, request: &'a File, said: Vec<GlueSaid>) {
    match by.iter_mut().find(|(r, _)| r.rel == request.rel) {
        Some((_, all)) => all.extend(said),
        None => by.push((request, said)),
    }
}

/// The units of `all` of the modules `laid` lays (the project's, `project`) that say anything, and
/// the findings of the checks across the languages at them (`crossed`), taken out: each request they
/// are of gets one unit of ringi's in their place, its E201.
pub fn said_as_ringi(project: &Project, laid: &Laid, joined: &Joined, all: &mut Vec<(Tool, Checked)>, crossed: &mut Vec<Finding>, lang: Lang) {
    let mut by: Vec<(&File, Vec<GlueSaid>)> = Vec::new();
    let rel_of = |label: &str| project.files.iter().find(|f| f.shown == label).map(|f| f.rel.clone());
    all.retain(|(tool, u)| {
        let Some((request, machine)) = (*tool == Tool::Kaname).then(|| rel_of(&u.label)).flatten().and_then(|rel| request_of(laid, &rel)) else { return true };
        let said = said_of(u, machine);
        if said.is_empty() {
            return true;
        }
        add(&mut by, request, said);
        false
    });
    crossed.retain(|f| {
        let Some((request, machine)) = f.file.as_deref().and_then(|rel| request_of(laid, rel)) else { return true };
        add(&mut by, request, vec![GlueSaid { code: f.code.clone(), line: f.line, machine, not_total: false }]);
        false
    });
    for (request, said) in by {
        all.push((Tool::Ringi, joined.ringi.glue_refused(&project.root, &request.shown, &said, lang)));
    }
}

/// What the screens of each laid kind say before they are written (ringi's DESIGN 13.3, 4.8): E704
/// of a number's range, E703 of a file's labels, E701 and W702 of what a screen sends, each at the
/// request, as ringi's. The requests whose screens do not pass, by the label of their unit: their
/// files pass their check, and what stops them holds for `--target requests` alone.
pub fn screens_said(project: &Project, laid: &Laid, joined: &Joined, given: &ringi::screens::Given, all: &mut Vec<(Tool, Checked)>, lang: Lang) -> Vec<String> {
    let mut refused = Vec::new();
    for k in &laid.kinds {
        if let Some(u) = joined.ringi.screens_refused(&project.root, &k.request.shown, given, lang) {
            if u.verdict != ritsu_ports::Verdict::Passes {
                refused.push(u.label.clone());
            }
            all.push((Tool::Ringi, u));
        }
    }
    refused
}

#[cfg(test)]
mod tests {
    use super::*;
    use ritsu_base::diag::Severity;
    use ritsu_ports::{Part, Verdict};

    fn unit(findings: &[(&str, usize)], functions: Json) -> Checked {
        let parts = findings
            .iter()
            .map(|(code, line)| Part::Finding(Finding { code: code.to_string(), severity: Severity::of(code), file: Some("a.lowered/a.kn".into()), line: Some(*line), text: format!("warning[{code}]: …\n"), json: Json::Null }))
            .collect();
        Checked { label: "a.lowered/a.kn".into(), parts, verdict: Verdict::Passes, json: Json::obj([("functions", functions)]) }
    }

    #[test]
    fn a_warning_and_a_function_that_is_not_total_are_said() {
        let functions = ritsu_base::json::parse(r#"[{"function": "f", "effect": {"fails": [], "diverges": false}, "places": [], "calls": []},
            {"function": "g", "effect": {"fails": ["range"], "diverges": false}, "places": [{"line": 12}], "calls": []},
            {"function": "h", "effect": {"fails": [], "diverges": true}, "places": [], "calls": [{"line": 30, "function": "h"}]}]"#)
        .unwrap();
        let said = said_of(&unit(&[("W501", 7)], functions), false);
        let codes: Vec<(&str, Option<usize>, bool)> = said.iter().map(|s| (s.code.as_str(), s.line, s.not_total)).collect();
        assert_eq!(codes, [("W501", Some(7), false), ("fails range", Some(12), true), ("diverges", Some(30), true)]);
    }

    #[test]
    fn a_module_that_says_nothing_says_nothing() {
        let functions = ritsu_base::json::parse(r#"[{"function": "f", "effect": {"fails": [], "diverges": false}, "places": [], "calls": []}]"#).unwrap();
        assert!(said_of(&unit(&[], functions), true).is_empty());
    }
}
