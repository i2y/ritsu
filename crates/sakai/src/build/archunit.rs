//! `SakaiContextsTest.java` for Java (DESIGN 7.4; PLAN C.9): a JUnit 5 test with one ArchUnit rule
//! a group. A group is a predicate on classes (its packages, without the packages of the groups
//! under them), and its rule says that no class outside the groups that may use it depends on
//! its classes (DESIGN 7.1).
//!
//! ArchUnit fails a rule whose `that()` matches no class (`failOnEmptyShould`). A group every
//! class may use has no class to check, and gets no rule; a comment says so.

use super::areas::{self, Areas};
use crate::check::Checked;
use ritsu_base::text::Lang;

/// A Java identifier from a group's name: `sakai-ordering-pl-shop.ordering.v1` →
/// `sakai_ordering_pl_shop_ordering_v1`.
fn ident(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect()
}

fn package(root: &str) -> String {
    if root == "." { String::new() } else { root.replace('/', ".") }
}

/// A Java string literal.
fn java(s: &str) -> String {
    serde_json::to_string(s).expect("a string is JSON")
}

/// The predicate of group `i`, as Java.
fn predicate(a: &Areas, i: usize) -> String {
    let mut parts: Vec<String> = Vec::new();
    for r in &a.areas[i].roots {
        let p = package(r);
        let inside = if p.is_empty() { "resideInAnyPackage(\"..\")".to_string() } else { format!("resideInAnyPackage({})", java(&format!("{p}.."))) };
        let nested: Vec<String> = a.nested(i, r).iter().map(|n| java(&format!("{}..", package(n)))).collect();
        if nested.is_empty() {
            parts.push(inside);
        } else {
            parts.push(format!("{inside}.and(resideOutsideOfPackages({}))", nested.join(", ")));
        }
    }
    if parts.len() == 1 { parts.remove(0) } else { format!("{}.or({})", parts[0], parts[1..].join(").or(")) }
}

pub fn render(c: &Checked, a: &Areas, lang: Lang) -> (String, usize) {
    let m = &c.model;
    let verb = tr!("使う", "used");
    let names: Vec<String> = a.areas.iter().map(|x| areas::name(m, x)).collect();
    let consts: Vec<String> = names.iter().map(|n| ident(n).to_uppercase()).collect();
    // The packages to analyze: the top of every group's directory.
    let mut tops: Vec<String> = a.areas.iter().flat_map(|x| x.files.iter()).filter_map(|f| f.split_once('/').map(|(t, _)| t.to_string())).collect();
    tops.sort();
    tops.dedup();
    let mut s = String::new();
    s.push_str("import static com.tngtech.archunit.core.domain.JavaClass.Predicates.resideInAnyPackage;\n");
    s.push_str("import static com.tngtech.archunit.core.domain.JavaClass.Predicates.resideOutsideOfPackages;\n");
    s.push_str("import static com.tngtech.archunit.lang.syntax.ArchRuleDefinition.noClasses;\n\n");
    s.push_str("import com.tngtech.archunit.base.DescribedPredicate;\n");
    s.push_str("import com.tngtech.archunit.core.domain.JavaClass;\n");
    s.push_str("import com.tngtech.archunit.junit.AnalyzeClasses;\n");
    s.push_str("import com.tngtech.archunit.junit.ArchTest;\n");
    s.push_str("import com.tngtech.archunit.lang.ArchRule;\n\n");
    s.push_str(&format!("@AnalyzeClasses(packages = {{{}}})\n", tops.iter().map(|t| java(t)).collect::<Vec<_>>().join(", ")));
    s.push_str("class SakaiContextsTest {\n");
    for (i, x) in a.areas.iter().enumerate() {
        s.push_str(&format!("  // {}\n", ritsu_base::text::spaced(&areas::phrase(m, x), lang)));
        s.push_str(&format!("  private static final DescribedPredicate<JavaClass> {} = {};\n", consts[i], predicate(a, i)));
    }
    let mut n = 0;
    for (i, x) in a.areas.iter().enumerate() {
        let importers = a.importers(i);
        s.push('\n');
        // A rule whose `that()` would match no class fails in ArchUnit; then every class may use it.
        let outside = a.areas.iter().enumerate().any(|(j, y)| !importers.contains(&j) && !y.files.is_empty());
        if !outside {
            let why = match lang {
                Lang::En => format!("  // {}: every class may use it, so there is no rule (ArchUnit fails a rule that checks no class).\n", names[i]),
                Lang::Ja => format!("  // {}: どのクラスも使ってよいので、規則を書かない（確かめるクラスが無い規則は、ArchUnit が失敗させる）。\n", names[i]),
            };
            s.push_str(&why);
            continue;
        }
        let allowed: Vec<&str> = importers.iter().map(|&j| consts[j].as_str()).collect();
        let allowed = if allowed.len() == 1 { allowed[0].to_string() } else { format!("{}.or({})", allowed[0], allowed[1..].join(").or(")) };
        s.push_str("  @ArchTest\n");
        s.push_str(&format!("  static final ArchRule {} =\n", ident(&names[i])));
        s.push_str(&format!("      noClasses().that(DescribedPredicate.not({allowed}))\n"));
        s.push_str(&format!("          .should().dependOnClassesThat({})\n", consts[i]));
        s.push_str(&format!("          .as({});\n", java(&areas::rule_text(m, a, i, &verb, lang))));
        n += 1;
        let _ = x;
    }
    s.push_str("}\n");
    (s, n)
}
