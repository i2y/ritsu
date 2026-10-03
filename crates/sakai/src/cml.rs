//! `sakai export cml` (DESIGN 8; PLAN C.14): the map as Context Mapper's CML. A context is a
//! `BoundedContext` named by its alias (CML's names are ASCII identifiers), with its description
//! as the `domainVisionStatement` and the names of its terms as its `responsibilities`; its
//! Japanese name and its owner go in a comment. The relationships come in the order the map
//! writes them, each with a comment in the words of the map; separate ways, which CML has no way
//! to write, is only a comment.

use crate::ast::Role;
use crate::check::Checked;
use crate::i18n::{Lang, say};
use crate::model::{Model, Rel, RelK};

/// A CML string.
fn string(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        if c == '"' || c == '\\' {
            o.push('\\');
        }
        o.push(c);
    }
    o.push('"');
    o
}

/// The upstream's role and what the relationship goes through: `OHS,PL` when a package it goes
/// through has an open host service, else `PL`; and the packages, with the services as
/// `Connect: <package>.<service>`.
fn upstream_side(m: &Model, up: usize, through: &[(String, crate::ast::Pos)]) -> (bool, String) {
    let mut ohs = false;
    let mut tech: Vec<String> = Vec::new();
    for (pkg, _) in through {
        let services: Vec<String> = m.contexts[up].published.iter().filter(|p| p.package == *pkg).flat_map(|p| p.services.iter().map(|(s, _)| format!("{pkg}.{s}"))).collect();
        if services.is_empty() {
            tech.push(pkg.clone());
        } else {
            ohs = true;
            tech.push(format!("Connect: {}", services.join(", ")));
        }
    }
    (ohs, tech.join("; "))
}

fn relationship(m: &Model, ci: usize, r: &Rel, lang: Lang) -> Option<String> {
    let me = &m.contexts[ci];
    let other = &m.contexts[r.partner];
    let (a, b) = (&me.alias, &other.alias);
    let (an, bn) = (&me.name, &other.name);
    match &r.kind {
        RelK::Upstream { through, .. } => {
            let (ohs, tech) = upstream_side(m, r.partner, through);
            let roles = r.roles();
            let customer = roles.contains(&Role::Customer);
            let acl = roles.contains(&Role::Acl);
            let (down, up, said) = if customer {
                let down = if acl { "D,C,ACL" } else { "D,C" };
                (down, "U,S,PL".to_string(), tr!("「{an}」は「{bn}」の顧客で、「{bn}」はそれを供給者として引き受ける", "{an} is {bn}'s customer, and {bn} its supplier"))
            } else if acl {
                ("D,ACL", if ohs { "U,OHS,PL".to_string() } else { "U,PL".to_string() }, tr!("「{an}」は「{bn}」のモデルを腐敗防止層で読み替える", "{an} maps {bn}'s model in an anticorruption layer"))
            } else {
                ("D,CF", if ohs { "U,OHS,PL".to_string() } else { "U,PL".to_string() }, tr!("「{an}」は「{bn}」に順応する", "{an} conforms to {bn}"))
            };
            Some(format!("  // {}\n  {a} [{down}]<-[{up}] {b} {{\n    implementationTechnology = {}\n  }}\n", say(&said, lang), string(&tech)))
        }
        RelK::Downstream => None,
        RelK::Kernel(_) => Some(format!("  // {}\n  {a} [SK]<->[SK] {b}\n", say(&tr!("「{an}」と「{bn}」の共有カーネル", "the shared kernel of {an} and {bn}"), lang))),
        RelK::Partnership => Some(format!("  // {}\n  {a} [P]<->[P] {b}\n", say(&tr!("「{an}」と「{bn}」のパートナーシップ", "the partnership of {an} and {bn}"), lang))),
        RelK::Separate => Some(format!(
            "  // {}\n",
            say(&tr!("「{an}」({a}) と「{bn}」({b}) は別々の道（CML には書く形が無い）", "{an} ({a}) and {bn} ({b}) go separate ways (CML has no way to write it)"), lang)
        )),
    }
}

/// The CML of a map that passed check. `from` is the map as the header names it.
pub fn render(c: &Checked, from: &str, lang: Lang) -> String {
    let m = &c.model;
    let mut s = match lang {
        Lang::En => format!("/* Written by `sakai export cml` from {from}. Edit the .ctx files and write it again. */\n"),
        Lang::Ja => format!("/* `sakai export cml --lang ja` が {from} から書いた。直すときは .ctx を直して書き直す。 */\n"),
    };
    let alias = m.map.ast.heading.alias.clone().unwrap_or_default();
    let contains: Vec<&str> = m.contexts.iter().map(|x| x.alias.as_str()).collect();
    s.push_str(&format!("ContextMap {alias} {{\n  type = SYSTEM_LANDSCAPE\n  state = AS_IS\n  contains {}\n", contains.join(", ")));
    let mut pairs: Vec<(usize, usize, &'static str)> = Vec::new();
    for (ci, x) in m.contexts.iter().enumerate() {
        for r in &x.rels {
            if matches!(r.kind, RelK::Kernel(_) | RelK::Partnership | RelK::Separate) {
                let key = (ci.min(r.partner), ci.max(r.partner), r.kind.words());
                if pairs.contains(&key) {
                    continue;
                }
                pairs.push(key);
            }
            if let Some(t) = relationship(m, ci, r, lang) {
                s.push('\n');
                s.push_str(&t);
            }
        }
    }
    s.push_str("}\n");
    for x in &m.contexts {
        let (n, a) = (&x.name, &x.alias);
        let said = match &x.ast.owner {
            Some(o) => {
                let o = &o.value;
                tr!("「{n}」({a})。持ち主: {o}", "{n} ({a}), owned by {o}")
            }
            None => tr!("「{n}」({a})", "{n} ({a})"),
        };
        s.push_str(&format!("\n// {}\n", say(&said, lang)));
        let mut body: Vec<String> = Vec::new();
        if let Some(d) = &x.ast.description {
            body.push(format!("  domainVisionStatement = {}", string(&d.value)));
        }
        let terms: Vec<String> = x.ast.terms.iter().filter(|t| t.as_term.is_none()).map(|t| string(&t.name)).collect();
        if !terms.is_empty() {
            body.push(format!("  responsibilities = {}", terms.join(", ")));
        }
        if body.is_empty() {
            s.push_str(&format!("BoundedContext {a}\n"));
        } else {
            s.push_str(&format!("BoundedContext {a} {{\n{}\n}}\n", body.join("\n")));
        }
    }
    s
}
