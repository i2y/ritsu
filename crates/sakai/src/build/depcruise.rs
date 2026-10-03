//! `.dependency-cruiser.cjs` for JavaScript and TypeScript (DESIGN 7.3; PLAN C.8): one `forbidden`
//! rule a group. `to.path` matches the group's files and `from.pathNot` the files of the groups
//! that may import it (DESIGN 7.1). A group's directory that holds another group leaves it out
//! with a lookahead (`^delivery/(?!acl/inventory/)`), so each pattern matches the group's files
//! and no other's.
//!
//! The file is CommonJS rather than JSON so that it can start with the header: dependency-cruiser
//! 16.10.4 refuses a key it does not know, at the top and under `options` alike (DESIGN 7.3).

use super::areas::{self, Areas};
use crate::check::Checked;
use crate::i18n::Lang;

/// `s` with the characters a regular expression gives a meaning to escaped.
fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\^$.|?*+()[]{}".contains(c) {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

/// The pattern of a directory, from the code directory, with the directories of other groups
/// under it left out.
fn one(root: &str, nested: &[String]) -> String {
    let rel: Vec<String> = nested.iter().map(|n| if root == "." { escape(n) } else { escape(n.strip_prefix(&format!("{root}/")).unwrap_or(n)) }).collect();
    let head = if root == "." { String::new() } else { format!("{}/", escape(root)) };
    if rel.is_empty() { head } else { format!("{head}(?!(?:{})/)", rel.join("|")) }
}

/// The pattern of the files of the groups `group`.
fn pattern(a: &Areas, group: &[usize]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for &i in group {
        for r in &a.areas[i].roots {
            let nested: Vec<String> = a.nested(i, r).into_iter().filter(|n| !group.iter().any(|&g| a.areas[g].roots.contains(n))).collect();
            let p = one(r, &nested);
            if !parts.contains(&p) {
                parts.push(p);
            }
        }
    }
    format!("^(?:{})", parts.join("|"))
}

/// A JavaScript string literal (JSON's escapes are JavaScript's).
fn js(s: &str) -> String {
    serde_json::to_string(s).expect("a string is JSON")
}

pub fn render(c: &Checked, a: &Areas, lang: Lang) -> (String, usize) {
    let m = &c.model;
    let verb = tr!("import する", "imported");
    let mut s = String::from("module.exports = {\n  forbidden: [\n");
    for (i, area) in a.areas.iter().enumerate() {
        s.push_str("    {\n");
        s.push_str(&format!("      name: {},\n", js(&areas::name(m, area))));
        s.push_str(&format!("      comment: {},\n", js(&areas::rule_text(m, a, i, &verb, lang))));
        s.push_str("      severity: \"error\",\n");
        s.push_str(&format!("      from: {{ pathNot: {} }},\n", js(&pattern(a, &a.importers(i)))));
        s.push_str(&format!("      to: {{ path: {} }},\n", js(&pattern(a, &[i]))));
        s.push_str("    },\n");
    }
    s.push_str("  ],\n  options: {\n    tsPreCompilationDeps: true,\n    doNotFollow: { path: \"node_modules\" },\n");
    if !a.except.is_empty() {
        let ex: Vec<String> = a.except.iter().map(|e| format!("{}(?:/|$)", escape(e))).collect();
        s.push_str(&format!("    exclude: {{ path: {} }},\n", js(&format!("^(?:{})", ex.join("|")))));
    }
    s.push_str("  },\n};\n");
    (s, a.areas.len())
}
