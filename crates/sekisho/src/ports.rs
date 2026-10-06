//! ritsu's ports, as sekisho answers them (DESIGN 8.2, 8.3; ritsu's DESIGN 3.2, 6.4). [`Engine`]
//! gives what a `.gate` holds (`Items`: its principals and resources with their attributes, its
//! roles, workflows and enums with their values, its actions with their inputs and computed
//! values, its policies, expectations and separations), what it names outside itself
//! (`References`: the files of its `use` lines and its workflows, and the operations its actions
//! guard), and what the checks across the borders ask of it (`Gates`: its actions with the
//! operations they guard, its workflows and policies, and how far someone who asks is allowed an
//! action over every combination the check walks).
//!
//! A gate is checked once in a run, with the languages the engine is handed (`Suite`), and what
//! the check found is kept for every question that follows: `ritsu check` checks each gate through
//! the engine ([`Engine::checked`]), and the checks across the borders, sakai and yuen then ask the
//! same engine. The definitions are the blocks as written, each line without its comment and its
//! alignment, under the one above by its depth (ritsu-base's [`ritsu_base::definition`], as
//! dandori's are).

use crate::check::{Options, Outcome};
use crate::suite::Suite;
use ritsu_base::naming::{Name, Tool};
use ritsu_base::text::Lang;
use ritsu_ports::{Allowance, Asker, Found, GateAction, GateFacts, GatePolicy, GateWorkflow, Item, Reference, Said};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A check kept for a run: by the root and the file as it is reached.
type Kept = RefCell<BTreeMap<(PathBuf, PathBuf), Rc<Result<Outcome, String>>>>;

/// sekisho, as the ports reach it, with the languages a gate reads joined (`ritsu sekisho` and
/// `ritsu check` hand it every one; the default joins none, and a gate that reads another language
/// then does not pass, E209).
#[derive(Default)]
pub struct Engine {
    suite: Suite,
    kept: Kept,
}

impl Engine {
    /// sekisho with the languages `suite` joins.
    pub fn new(suite: Suite) -> Engine {
        Engine { suite, kept: RefCell::default() }
    }

    /// The check of the file at `disk` with the references written from `root`, made once in a
    /// run; `shown` is how its diagnostics name it, when this is the first time it is asked. Err
    /// when the file cannot be read.
    fn outcome(&self, root: &Path, disk: &Path, shown: &str) -> Rc<Result<Outcome, String>> {
        let key = (root.to_path_buf(), disk.to_path_buf());
        if let Some(o) = self.kept.borrow().get(&key) {
            return o.clone();
        }
        let opts = Options { root: Some(root.to_path_buf()), ..Options::default() };
        let o = Rc::new(match ritsu_base::fs::read_to_string(disk) {
            Ok(src) => Ok(crate::check::check_text(shown, &src, &self.suite, &opts)),
            Err(e) => Err(e.to_string()),
        });
        self.kept.borrow_mut().insert(key, o.clone());
        o
    }

    /// The check of a file of the project (`file`, from `root`): what it found, when it passes;
    /// else what it says, as the ports say it.
    fn passing(&self, root: &Path, file: &str) -> Result<Rc<Result<Outcome, String>>, Vec<Said>> {
        let disk = ritsu_base::paths::on_disk(root, file);
        let path = disk.to_string_lossy().to_string();
        let o = self.outcome(root, &disk, &path);
        match o.as_ref() {
            Err(e) => Err(vec![Said::unreadable(&path, e)]),
            Ok(x) if x.has_errors() || x.walked.is_none() => Err(x.diags.iter().filter(|d| d.is_error()).map(|d| Said { file: path.clone(), ..Said::of(d) }).collect()),
            Ok(_) => Ok(o.clone()),
        }
    }

    /// `sekisho check` of each file, as `ritsu check` prints it (ritsu's DESIGN 8.3), with the
    /// languages the engine is handed: what [`crate::check::checked`] gives, each check kept for
    /// the questions the ports are asked after it. `files` are as the person gave them, from where
    /// the program runs; `root` is the project's.
    pub fn checked(&self, root: &Path, files: &[String], lang: Lang) -> Vec<ritsu_ports::Checked> {
        files
            .iter()
            .map(|f| {
                let disk = ritsu_base::paths::absolute(Path::new(f));
                let o = self.outcome(root, &disk, f);
                crate::check::unit(root, f, o.as_ref().as_ref().map_err(|e| e.clone()), lang)
            })
            .collect()
    }
}

/// A file of Cedar written by hand, which [`crate::cedar_in`] reads: its policies (`.cedar`) or its
/// schema (`.cedarschema`, `.cedarschema.json`).
pub fn is_cedar(file: &str) -> bool {
    Tool::Cedar.extensions().iter().any(|x| file.ends_with(x))
}

impl ritsu_ports::Gates for Engine {
    /// A gate's name, version, SHA-256 and namespace, its actions with the operations they guard
    /// (the references the check found, the ones the schema's `@guards` writes), its workflows with
    /// their `.flow` from the root, its own policies with their `@id`, its expectations and its
    /// separations; for Cedar written by hand, what [`crate::cedar_in`] reads of it.
    fn facts(&self, root: &Path, file: &str) -> Result<GateFacts, Vec<Said>> {
        if is_cedar(file) {
            return crate::cedar_in::facts(root, file);
        }
        let o = self.passing(root, file)?;
        let Ok(x) = o.as_ref() else { unreachable!("a check that passes") };
        let w = x.walked.as_ref().expect("a file that passes is walked");
        let g = &w.gate;
        let alias_of = |t: &usize| g.types[*t].named.alias.clone();
        Ok(GateFacts {
            name: g.named.name.clone(),
            alias: g.named.alias.clone(),
            version: g.version.clone(),
            sha256: ritsu_base::sha256::hex(g.src.as_bytes()),
            namespace: g.namespace.clone(),
            actions: g
                .actions
                .iter()
                .map(|a| GateAction {
                    name: a.named.name.clone(),
                    alias: a.named.alias.clone(),
                    line: a.named.line,
                    guards: a.references(),
                    principals: a.principals.iter().map(alias_of).collect(),
                    resources: a.resources.iter().map(alias_of).collect(),
                    nobody: a.nobody.as_ref().map(|(r, _)| r.clone()),
                })
                .collect(),
            workflows: g
                .workflows
                .iter()
                .filter_map(|wf| Some(GateWorkflow { name: wf.named.name.clone(), alias: wf.named.alias.clone(), flow: Name::file(Tool::Dandori, g.from_root(&wf.file)?), line: wf.named.line }))
                .collect(),
            policies: g
                .policies
                .iter()
                .enumerate()
                .filter(|(_, p)| p.from.is_none())
                .map(|(i, p)| GatePolicy { name: p.named.name.clone(), id: crate::cedar::policy_id(g, i), permit: p.permit, line: p.named.line })
                .collect(),
            expects: g.expects.iter().map(|e| (e.name.clone(), e.line)).collect(),
            separations: g.separates.iter().map(|s| (s.name.clone(), s.line)).collect(),
        })
    }

    /// How far the asker is allowed the action, over every combination the check walked
    /// ([`crate::checks::allowance`]); for Cedar written by hand, over every combination of the
    /// finite part its policies are written in ([`crate::cedar_in`]).
    fn allowed(&self, root: &Path, file: &str, action: &str, asker: &Asker) -> Result<Found<Allowance>, Vec<Said>> {
        if is_cedar(file) {
            return crate::cedar_in::allowed(root, file, action, asker);
        }
        let o = self.passing(root, file)?;
        let Ok(x) = o.as_ref() else { unreachable!("a check that passes") };
        let w = x.walked.as_ref().expect("a file that passes is walked");
        let g = &w.gate;
        let Some(ai) = g.actions.iter().position(|a| a.named.is(action)) else {
            let path = ritsu_base::paths::on_disk(root, file).to_string_lossy().to_string();
            return Err(vec![Said { code: String::new(), file: path.clone(), line: None, message: tr!("{path} に action `{action}` はありません", "There is no action `{action}` in {path}") }]);
        };
        let Some(space) = w.report.spaces.get(ai).and_then(|s| s.as_ref()) else {
            return Ok(Found::Undecided(tr!("action `{action}` の組み合わせを数えていません", "the combinations of the action `{action}` were not walked")));
        };
        let langs = crate::borders::Langs { rules: self.suite.rules.as_deref().filter(|r| r.joined()), dates: self.suite.dates.as_deref().filter(|d| d.joined()), books: None, flows: None };
        Ok(crate::checks::allowance(g, space, &w.known, &langs, asker))
    }
}

/// The file read and parsed, for what it holds and names; else what the parser says (or why it
/// cannot be read).
fn parsed(root: &Path, file: &str) -> Result<crate::ast::File, Vec<Said>> {
    let disk = ritsu_base::paths::on_disk(root, file);
    let path = disk.to_string_lossy().to_string();
    let src = ritsu_base::fs::read_to_string(&disk).map_err(|e| vec![Said::unreadable(&path, &e.to_string())])?;
    let p = crate::parse::parse(&path, &src);
    if crate::diag::has_errors(&p.diags) {
        return Err(p.diags.iter().filter(|d| d.is_error()).map(Said::of).collect());
    }
    p.file.ok_or_else(|| p.diags.iter().map(Said::of).collect())
}

impl ritsu_ports::Items for Engine {
    /// Each principal and resource (and each of its attributes), role, workflow, enum (and each of
    /// its values), action (and each of its inputs and computed values), policy (permit or forbid),
    /// expectation and separation the file writes (DESIGN 8.2), each by its name. A block's
    /// definition is its lines; an attribute's, an input's, a computed value's and an enum's is its
    /// line; an enum value's is its name. The file need only parse: what its names refer to is the
    /// check's to say.
    fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        let f = parsed(root, file)?;
        let lines: Vec<&str> = f.src.lines().collect();
        let naming = |kind: &str, name: &str| Name::file(Tool::Sekisho, file).with(kind, name);
        let one = |naming: Name, (from, to): (usize, usize)| Item { naming, lines: (from, to), text: ritsu_base::definition::block(&lines, from, to) };
        let mut out = Vec::new();
        for e in &f.enums {
            let l = e.span.line;
            out.push(one(naming("enum", &e.name.text), (l, l)));
            for v in &e.values {
                out.push(Item { naming: naming("enum", &e.name.text).with("value", &v.text), lines: (v.span.line, v.span.line), text: v.text.clone() });
            }
        }
        for r in &f.roles {
            out.push(one(naming("role", &r.name.text), r.lines));
        }
        for (kind, entities) in [("principal", &f.principals), ("resource", &f.resources)] {
            for x in entities {
                out.push(one(naming(kind, &x.name.text), x.lines));
                for a in &x.attributes {
                    let l = a.span.line;
                    out.push(one(naming(kind, &x.name.text).with("attribute", &a.name.text), (l, l)));
                }
            }
        }
        for w in &f.workflows {
            out.push(one(naming("workflow", &w.name.text), w.lines));
        }
        for a in &f.actions {
            out.push(one(naming("action", &a.name.text), a.lines));
            for i in &a.input {
                let l = i.span.line;
                out.push(one(naming("action", &a.name.text).with("input", &i.name.text), (l, l)));
            }
            for c in &a.context {
                let l = c.span.line;
                out.push(one(naming("action", &a.name.text).with("context", &c.name.text), (l, l)));
            }
        }
        for p in &f.policies {
            out.push(one(naming("policy", &p.name.text), p.lines));
        }
        for e in &f.expects {
            out.push(one(naming("expect", &e.name.text), e.lines));
        }
        for s in &f.separates {
            out.push(one(naming("separate", &s.name.text), s.lines));
        }
        out.sort_by_key(|i| i.lines.0);
        Ok(out)
    }
}

impl ritsu_ports::References for Engine {
    /// The files a gate reads (`use rule`, `use dates`, `use calendar`, `use openapi`, `use proto`,
    /// `use asyncapi`, `use book`, `use gate`), the `.flow` of each workflow it names (`workflow`),
    /// and each operation its actions guard (`guards`), as the check found it: an operation is named
    /// as the contract names it (its `operationId`, the service from the `.proto`'s package), so the
    /// operations a gate does not find (E202) and those of a gate whose names do not hold are not
    /// here. A file outside the root has no reference. For Cedar written by hand, the operations its
    /// schema's `@guards` names ([`crate::cedar_in`]).
    fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>> {
        if is_cedar(file) {
            return crate::cedar_in::references(root, file);
        }
        let f = parsed(root, file)?;
        let dir = ritsu_base::paths::parent(file);
        let from_root = |written: &str| ritsu_base::paths::join(&dir, written).ok();
        let mut out = Vec::new();
        for u in &f.uses {
            let Some(p) = from_root(&u.path.0) else { continue };
            let tool = match u.kind {
                crate::ast::UseKind::Rule => Tool::Rulec,
                crate::ast::UseKind::Dates | crate::ast::UseKind::Calendar => Tool::Koyomi,
                crate::ast::UseKind::Openapi => Tool::Openapi,
                crate::ast::UseKind::Proto => Tool::Proto,
                crate::ast::UseKind::Asyncapi => Tool::Asyncapi,
                crate::ast::UseKind::Book => Tool::Chobo,
                crate::ast::UseKind::Gate => Tool::Sekisho,
            };
            out.push(Reference { line: u.span.line, target: Name::file(tool, p), how: format!("use {}", u.kind.word()) });
        }
        for w in &f.workflows {
            if let Some(p) = from_root(&w.flow.0) {
                out.push(Reference { line: w.span.line, target: Name::file(Tool::Dandori, p), how: "workflow".into() });
            }
        }
        // the operations, as the check of the contracts found them
        let disk = ritsu_base::paths::on_disk(root, file);
        let o = self.outcome(root, &disk, &disk.to_string_lossy());
        if let Ok(x) = o.as_ref()
            && let Some(w) = &x.walked
        {
            for a in &w.gate.actions {
                for (n, line) in a.references() {
                    out.push(Reference { line, target: n, how: "guards".into() });
                }
            }
        }
        out.sort_by_key(|r| r.line);
        Ok(out)
    }
}
