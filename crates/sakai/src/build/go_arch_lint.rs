//! `.go-arch-lint.yml` for Go (DESIGN 7.5; PLAN C.10): one component a group, `in` its
//! directories, and under `deps` the components it may depend on (DESIGN 7.1). go-arch-lint reads
//! a file into the deepest component whose directory holds it, as the map does, and stops a file
//! that no component holds, so a directory the map gains and the settings lack does not pass.

use super::areas::{self, Areas};
use crate::check::Checked;
use ritsu_base::text::Lang;

/// A YAML string in double quotes (JSON's strings are YAML's).
fn yaml(s: &str) -> String {
    serde_json::to_string(s).expect("a string is JSON")
}

fn glob(root: &str) -> String {
    if root == "." { "**".to_string() } else { format!("{root}/**") }
}

/// A regular expression that matches a path and what is under it, for `excludeFiles`.
fn exclude(p: &str) -> String {
    let mut o = String::from("^");
    for c in p.chars() {
        if "\\^$.|?*+()[]{}".contains(c) {
            o.push('\\');
        }
        o.push(c);
    }
    o.push_str("(/|$)");
    o
}

pub fn render(c: &Checked, a: &Areas, lang: Lang) -> (String, usize) {
    let m = &c.model;
    let names: Vec<String> = a.areas.iter().map(|x| areas::name(m, x)).collect();
    let mut s = String::from("version: 3\nworkdir: .\n\nallow:\n  depOnAnyVendor: true\n");
    if !a.except.is_empty() {
        s.push_str("\nexcludeFiles:\n");
        for e in &a.except {
            s.push_str(&format!("  - {}\n", yaml(&exclude(e))));
        }
    }
    s.push_str("\ncomponents:\n");
    for (i, x) in a.areas.iter().enumerate() {
        s.push_str(&format!("  # {}\n", ritsu_base::text::spaced(&areas::phrase(m, x), lang)));
        if x.roots.len() == 1 {
            s.push_str(&format!("  {}:\n    in: {}\n", names[i], yaml(&glob(&x.roots[0]))));
        } else {
            s.push_str(&format!("  {}:\n    in:\n", names[i]));
            for r in &x.roots {
                s.push_str(&format!("      - {}\n", yaml(&glob(r))));
            }
        }
    }
    s.push_str("\ndeps:\n");
    for (i, _) in a.areas.iter().enumerate() {
        // A component lists itself: go-arch-lint v1.19.0 stops an import between two packages of
        // one component that does not (`inventory` importing `inventory/ledger`).
        let may: Vec<&str> = (0..a.areas.len()).filter(|&j| a.allowed[i][j]).map(|j| names[j].as_str()).collect();
        s.push_str(&format!("  {}:\n    mayDependOn:\n", names[i]));
        for d in may {
            s.push_str(&format!("      - {d}\n"));
        }
    }
    (s, a.areas.len())
}
