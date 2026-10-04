//! ritsu's ports, as dandori answers them (ritsu's DESIGN 3.2 and 6.4): [`Engine`] gives what a
//! `.flow` holds (`Items`) and what it names outside itself (`References`). Both read the file
//! as it is written, without the rules it calls: a flow whose rules cannot be read still has
//! its tasks and its cases.

use crate::apis::ApiKind;
use crate::syntax::{self, Binding, Program};
use ritsu_base::naming::{Name as Naming, Tool};
use ritsu_base::text::Text;
use ritsu_ports::{Item, Reference, Said};
use std::path::Path;

/// dandori, as the ports reach it.
#[derive(Default)]
pub struct Engine;

/// The file and what it says, parsed as `dandori check` parses it; else what the parser says.
fn parsed(root: &Path, file: &str) -> Result<(String, Program), Vec<Said>> {
    let disk = ritsu_base::paths::on_disk(root, file);
    let path = disk.to_string_lossy().to_string();
    let src = ritsu_base::fs::read_to_string(&disk).map_err(|e| vec![Said::unreadable(&path, &e.to_string())])?;
    match syntax::parse(&src) {
        Ok(p) => Ok((src, p)),
        Err(d) => Err(vec![Said { code: d.code.to_string(), file: path, line: Some(d.line), message: Text { ja: d.ja, en: d.en } }]),
    }
}

/// A line as a definition counts it: without its comment and the spaces around it, and each run
/// of spaces outside a string as one, so that lining up the colons of a record changes nothing.
fn plain(line: &str) -> String {
    let mut out = String::new();
    let (mut quoted, mut space) = (false, false);
    let mut chars = line.trim().chars();
    while let Some(c) = chars.next() {
        if quoted {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => quoted = false,
                _ => {}
            }
            continue;
        }
        match c {
            '#' => break,
            ' ' => space = true,
            _ => {
                if std::mem::take(&mut space) {
                    out.push(' ');
                }
                quoted = c == '"';
                out.push(c);
            }
        }
    }
    out
}

/// The definition of what is written on the lines `from` to `to`: each line made [`plain`], and
/// put under the line above it by its depth, two spaces a level, however deep it was indented.
fn definition(lines: &[&str], from: usize, to: usize) -> String {
    let rows: Vec<(usize, String)> = (from..=to)
        .filter_map(|n| {
            let raw = lines.get(n.checked_sub(1)?)?;
            let p = plain(raw);
            (!p.is_empty()).then(|| (raw.len() - raw.trim_start_matches(' ').len(), p))
        })
        .collect();
    let mut widths: Vec<usize> = rows.iter().map(|(w, _)| *w).collect();
    widths.sort_unstable();
    widths.dedup();
    let depth = |w: usize| widths.iter().position(|x| *x == w).unwrap_or(0);
    rows.iter().map(|(w, p)| format!("{}{p}", "  ".repeat(depth(*w)))).collect::<Vec<_>>().join("\n")
}

impl Engine {
    /// `dandori check` of each file, as `ritsu check` prints it (ritsu's DESIGN 8.3), the rules the
    /// flows use read through `rules`: every finding, as the command prints it and as its
    /// `--format json` prints it, then the line that says a file passes. `files` are as the person
    /// gave them, from where the program runs; `root` is the project's.
    pub fn checked(&self, root: &Path, files: &[String], rules: std::rc::Rc<dyn ritsu_ports::Rules>, lang: ritsu_base::text::Lang) -> Vec<ritsu_ports::Checked> {
        let ports = ritsu_ports::Ports { rules, dates: std::rc::Rc::new(crate::sources::NoDates), books: std::rc::Rc::new(crate::sources::NoBooks) };
        self.checked_with(root, files, &ports, lang)
    }

    /// `dandori check` of each file, as [`Engine::checked`] gives it, with the dates files and the
    /// books the flows use read through the ports of dates and books too (what `ritsu check` joins).
    pub fn checked_with(&self, root: &Path, files: &[String], ports: &ritsu_ports::Ports, lang: ritsu_base::text::Lang) -> Vec<ritsu_ports::Checked> {
        use ritsu_ports::{Checked as Unit, Finding, Part, Verdict};
        crate::sources::with_ports(ports.rules.clone(), ports.dates.clone(), ports.books.clone(), || {
            files
                .iter()
                .map(|f| match crate::check::check_file(Path::new(f)) {
                    Ok((src, c)) => {
                        let file = ritsu_base::paths::from_root(root, Path::new(f));
                        let mut parts: Vec<Part> = c
                            .diags
                            .iter()
                            .map(|d| {
                                let json = ritsu_base::json::parse(&d.to_json(lang).to_string()).unwrap_or(ritsu_base::json::Json::Null);
                                let severity = match d.severity {
                                    crate::diag::Severity::Error => ritsu_base::diag::Severity::Error,
                                    crate::diag::Severity::Warning => ritsu_base::diag::Severity::Warning,
                                };
                                Part::Finding(Finding { code: d.code.to_string(), severity, file: file.clone(), line: (d.line > 0).then_some(d.line), text: crate::commands::render(std::slice::from_ref(d), f, &src, lang), json })
                            })
                            .collect();
                        if c.model.is_some() {
                            parts.push(Part::Text(crate::commands::passed(f, c.diags.len(), lang)));
                        }
                        let verdict = if c.diags.iter().any(|d| d.code == "E018") {
                            Verdict::Unchecked
                        } else if crate::diag::has_errors(&c.diags) {
                            Verdict::Fails
                        } else {
                            Verdict::Passes
                        };
                        Unit { label: f.clone(), parts, verdict }
                    }
                    Err(msg) => Unit::unchecked(f, format!("{msg}\n")),
                })
                .collect()
        })
    }
}

impl ritsu_ports::Flows for Engine {
    /// The calls of rules in a flow that passes `dandori check`, with the range of each value given
    /// as dandori's own range check reads it (its E014): every value put in a variable anywhere in
    /// the flow, joined. A value with a place it comes from that has no range carries that place.
    fn rule_calls(&self, file: &Path, rules: std::rc::Rc<dyn ritsu_ports::Rules>) -> Result<Vec<ritsu_ports::RuleCall>, Vec<Said>> {
        crate::sources::with_rules(rules, || {
            let path = file.to_string_lossy().to_string();
            let (_, c) = crate::check::check_file(file).map_err(|e| vec![Said::unreadable(&path, &e)])?;
            let Some(m) = c.model else {
                return Err(c.diags.iter().filter(|d| d.severity == crate::diag::Severity::Error).map(|d| Said { code: d.code.to_string(), file: path.clone(), line: Some(d.line), message: Text { ja: d.ja.clone(), en: d.en.clone() } }).collect());
            };
            Ok(crate::ranges::rule_calls(&m)
                .into_iter()
                .map(|(line, r, args)| ritsu_ports::RuleCall {
                    line,
                    rule: m.rules[r].info.path.clone(),
                    name: m.rules[r].name.clone(),
                    args: args
                        .into_iter()
                        .map(|(input, shown, range, unknown)| ritsu_ports::CallArg {
                            input,
                            shown,
                            range: range.map(|r| (r.lo.map(i128::from), r.hi.map(i128::from))),
                            unknown: unknown.map(|t| Text { ja: t.ja, en: t.en }),
                            // where a value comes from is `crossings`'s, which reads the dates files and books too
                            from: Vec::new(),
                        })
                        .collect(),
                })
                .collect())
        })
    }

    /// Every call of a rule, a koyomi date and a chobo transfer in a flow that passes `dandori
    /// check`, with the places each value given can come from, and the span of each hold before a
    /// call its expiry can refuse (`crossings`); the flow read with the rules, the dates files and
    /// the books through `ports`.
    fn crossings(&self, file: &Path, ports: &ritsu_ports::Ports) -> Result<ritsu_ports::Crossings, Vec<Said>> {
        crate::sources::with_ports(ports.rules.clone(), ports.dates.clone(), ports.books.clone(), || Engine::model_with(file).map(|m| crate::crossings::of(&m)))
    }
}

impl Engine {
    /// The model of a flow that passes `dandori check`, read with the other languages through
    /// `ports`; else the errors of the check.
    fn model_with(file: &Path) -> Result<crate::model::Model, Vec<Said>> {
        let path = file.to_string_lossy().to_string();
        let (_, c) = crate::check::check_file(file).map_err(|e| vec![Said::unreadable(&path, &e)])?;
        c.model.ok_or_else(|| c.diags.iter().filter(|d| d.severity == crate::diag::Severity::Error).map(|d| Said { code: d.code.to_string(), file: path.clone(), line: Some(d.line), message: Text { ja: d.ja.clone(), en: d.en.clone() } }).collect())
    }
}

impl ritsu_ports::Items for Engine {
    /// Each task, case, record (and each of its fields), enum (and each of its values), input
    /// and output the file writes (ritsu's DESIGN 6.3). A task's, a case's and a record's
    /// definition is its block of lines, the declaration and everything under it; an enum's, a
    /// field's, an input's and an output's is its line; a value's is its name.
    fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        let (src, prog) = parsed(root, file)?;
        let lines: Vec<&str> = src.lines().collect();
        let blocks = syntax::blocks(&src);
        let block = |line: usize| blocks.iter().find(|(a, _)| *a == line).copied().unwrap_or((line, line));
        let naming = |kind: &str, name: &str| Naming::file(Tool::Dandori, file).with(kind, name);
        let one = |naming: Naming, (from, to): (usize, usize)| Item { naming, lines: (from, to), text: definition(&lines, from, to) };
        let mut out = Vec::new();
        for e in &prog.enums {
            let l = e.name.1.line;
            out.push(one(naming("enum", &e.name.0), (l, l)));
            for v in &e.values {
                out.push(Item { naming: naming("enum", &e.name.0).with("value", &v.0), lines: (v.1.line, v.1.line), text: v.0.clone() });
            }
        }
        for r in &prog.records {
            out.push(one(naming("record", &r.name.0), block(r.name.1.line)));
            for f in &r.fields {
                let l = f.name.1.line;
                out.push(one(naming("record", &r.name.0).with("field", &f.name.0), (l, l)));
            }
        }
        for (kind, fields) in [("input", &prog.inputs), ("output", &prog.outputs)] {
            for f in fields {
                let l = f.name.1.line;
                out.push(one(naming(kind, &f.name.0), (l, l)));
            }
        }
        for t in &prog.tasks {
            out.push(one(naming("task", &t.name.0), block(t.name.1.line)));
        }
        for c in &prog.cases {
            out.push(one(naming("case", &c.name.0), block(c.name.1.line)));
        }
        out.sort_by_key(|i| i.lines.0);
        Ok(out)
    }
}

impl ritsu_ports::References for Engine {
    /// The rules a flow calls (`use rule`, with the ways written under it: `use rule … lambda`,
    /// `use rule … connect`, `use rule … local`), the dates files and the books it reads (`use
    /// dates`, `use book`, with the ways written under them too), the descriptions of the APIs its
    /// tasks call (`use proto`, `use openapi`, `use smithy`), the service of a `.proto` it implements
    /// (`implements`), each method of one a task calls (`connect`), each transfer of a book a task
    /// runs an operation of (`book`), and the `.flow` of each child workflow (`flow`). A service is
    /// named from its `.proto`'s package.
    fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>> {
        let (_, prog) = parsed(root, file)?;
        let from_root = |written: &str| ritsu_base::paths::join(&ritsu_base::paths::parent(file), written).ok();
        // the `.proto` a `use proto` name stands for, from the root
        let proto = |api: &str| prog.apis.iter().find(|a| a.kind == ApiKind::Proto && a.name.0 == api).and_then(|a| from_root(&a.path));
        let mut out = Vec::new();
        for u in &prog.uses {
            let Some(p) = from_root(&u.path) else { continue };
            let ways: Vec<&str> = [(u.lambda.is_some(), "lambda"), (u.connect.is_some(), "connect"), (u.local, "local")].into_iter().filter(|(w, _)| *w).map(|(_, n)| n).collect();
            let how = if ways.is_empty() { "use rule".to_string() } else { format!("use rule … {}", ways.join(", ")) };
            out.push(Reference { line: u.name.1.line, target: Naming::file(Tool::Rulec, p), how });
        }
        for u in &prog.dates {
            let Some(p) = from_root(&u.path) else { continue };
            let ways: Vec<&str> = [(u.lambda.is_some(), "lambda"), (u.local, "local")].into_iter().filter(|(w, _)| *w).map(|(_, n)| n).collect();
            let how = if ways.is_empty() { "use dates".to_string() } else { format!("use dates … {}", ways.join(", ")) };
            out.push(Reference { line: u.name.1.line, target: Naming::file(Tool::Koyomi, p), how });
        }
        // the `.book` a `use book` name stands for, from the root
        let book = |name: &str| prog.books.iter().find(|b| b.name.0 == name).and_then(|b| from_root(&b.path));
        for u in &prog.books {
            let Some(p) = from_root(&u.path) else { continue };
            let how = if u.lambda.is_some() { "use book … lambda" } else { "use book" };
            out.push(Reference { line: u.name.1.line, target: Naming::file(Tool::Chobo, p), how: how.into() });
        }
        for a in &prog.apis {
            let Some(p) = from_root(&a.path) else { continue };
            let (tool, how) = match a.kind {
                ApiKind::Proto => (Tool::Proto, "use proto"),
                ApiKind::OpenApi => (Tool::File, "use openapi"),
                ApiKind::Smithy => (Tool::File, "use smithy"),
            };
            out.push(Reference { line: a.name.1.line, target: Naming::file(tool, p), how: how.into() });
        }
        if let Some(q) = &prog.implements {
            if let (Some(p), Some(service)) = (proto(&q[0].0), q.last()) {
                out.push(Reference { line: q[0].1.line, target: Naming::file(Tool::Proto, p).with("service", &service.0), how: "implements".into() });
            }
        }
        for t in &prog.tasks {
            if let Some((Binding::Connect { api, method }, at)) = &t.binding {
                if let (Some(p), Some((service, m))) = (proto(&api.0), method.split_once('/')) {
                    // a service is not nested, so its name from the package is its last part
                    let service = service.rsplit('.').next().unwrap_or(service);
                    out.push(Reference { line: at.line, target: Naming::file(Tool::Proto, p).with("service", service).with("method", m), how: "connect".into() });
                }
            }
            if let Some((Binding::Book { book: b, transfer, .. }, at)) = &t.binding {
                if let Some(p) = book(&b.0) {
                    out.push(Reference { line: at.line, target: Naming::file(Tool::Chobo, p).with("transfer", &transfer.0), how: "book".into() });
                }
            }
            if let Some((written, at)) = &t.flow {
                if let Some(p) = from_root(written) {
                    out.push(Reference { line: at.line, target: Naming::file(Tool::Dandori, p), how: "flow".into() });
                }
            }
        }
        out.sort_by_key(|r| r.line);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_plain_without_its_comment_and_its_alignment() {
        assert_eq!(plain("  sku      : string   # the stock unit"), "sku : string");
        assert_eq!(plain("agent \"a  # b\"  # c"), "agent \"a  # b\"");
        assert_eq!(plain("jev \"say \\\"hi\\\"  #1\""), "jev \"say \\\"hi\\\"  #1\"");
        assert_eq!(plain("   # only a comment"), "");
    }

    #[test]
    fn a_block_keeps_its_depth_however_it_was_indented() {
        let two = ["task t() -> k", "  jev \"q\"", "    a \"x\"", "  timeout 10 seconds"];
        let four = ["task t() -> k", "    jev \"q\"   # asked", "", "        a \"x\"", "    timeout 10 seconds"];
        assert_eq!(definition(&two, 1, 4), "task t() -> k\n  jev \"q\"\n    a \"x\"\n  timeout 10 seconds");
        assert_eq!(definition(&four, 1, 5), definition(&two, 1, 4));
    }
}
