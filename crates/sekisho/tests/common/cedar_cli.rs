//! The official Cedar CLI (`cedar-policy-cli` 4.13.0), as the tests that hold Cedar to it run it
//! (DESIGN 6.1, 6.5): `tests/cedar.rs` holds the Cedar sekisho generates, `tests/cedar_in.rs` the
//! Cedar written by hand that the port answers for. Both also change a policy set at one place at a
//! time ([`policy_changes`]), and share the work among threads ([`each_on_threads`]).
//!
//! The CLI is found by ritsu-testkit (`RITSU_CEDAR`, or `cedar` on the PATH, at 4.13.0); without
//! it, a test that runs it says SKIP and passes.

use ritsu_base::cedar::{self, ActionScope, BinOp, CondKind, Effect, EntityOrSlot, EntityUid, Expr, ExprKind, Policy, PolicySet, Schema, Scope};
use ritsu_testkit::run::Ran;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// The longest one run of the CLI may take: `run-tests` on the 2,134 tests of the example takes a
/// third of a second on the machine the test was written on.
pub const LIMIT: Duration = Duration::from_secs(300);

/// The CLI's words without the colours it prints even into a pipe (`run-tests` colours `ok`, and
/// `NO_COLOR` does not stop it).
pub fn plain(s: &str) -> String {
    let esc = char::from(27u8);
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == esc && it.peek() == Some(&'[') {
            it.next();
            for d in it.by_ref() {
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// The official CLI.
pub struct Cli {
    pub bin: PathBuf,
}

impl Cli {
    /// The CLI at 4.13.0, or None with the SKIP line saying why.
    pub fn find() -> Option<Cli> {
        ritsu_testkit::cedar::cli().map(|bin| Cli { bin })
    }

    /// The CLI run in `dir` on the files there.
    pub fn run(&self, dir: &Path, args: &[&str]) -> Ran {
        let mut c = Command::new(&self.bin);
        c.current_dir(dir).args(args).env("CEDAR_ERROR_FORMAT", "plain").env("NO_COLOR", "1");
        let mut r = ritsu_testkit::run(&mut c, LIMIT);
        r.stdout = plain(&r.stdout);
        r.stderr = plain(&r.stderr);
        r
    }

    /// `cedar validate`, strict, failing on a warning too: in the Cedar formats, or in the JSON
    /// formats.
    pub fn validate(&self, dir: &Path, policies: &str, schema: &str, json: bool) -> Result<(), String> {
        let mut args = vec!["validate", "--schema", schema, "--policies", policies, "--validation-mode", "strict", "--deny-warnings"];
        if json {
            args.extend(["--schema-format", "json", "--policy-format", "json"]);
        }
        let r = self.run(dir, &args);
        if r.ok && r.both().contains("no errors or warnings") { Ok(()) } else { Err(format!("cedar {}: exit {:?}\n{}", args.join(" "), r.code, r.both())) }
    }

    /// `cedar format --check`: it prints the text as it lays it out, which is the text written.
    pub fn format_check(&self, dir: &Path, policies: &str, text: &str) -> Result<(), String> {
        let r = self.run(dir, &["format", "--check", "--policies", policies]);
        if r.ok && r.stdout == text {
            Ok(())
        } else {
            Err(format!("cedar format --check --policies {policies}: exit {:?}\n{}{}", r.code, ritsu_testkit::golden::line_diff(text, &r.stdout), r.stderr))
        }
    }

    /// A `translate-*` of the CLI prints what was written, to the byte.
    pub fn translates_to(&self, dir: &Path, args: &[&str], written: &str) -> Result<(), String> {
        let r = self.run(dir, args);
        if r.ok && r.stdout == written {
            Ok(())
        } else {
            Err(format!("cedar {}: exit {:?}, and what it prints differs from the file {}\n{}", args.join(" "), r.code, first_difference(&r.stdout, written), r.stderr))
        }
    }

    /// `cedar run-tests`: every one of the `n` cases passes.
    pub fn run_tests(&self, dir: &Path, policies: &str, schema: &str, tests: &str, n: usize, json: bool) -> Result<(), String> {
        let mut args = vec!["run-tests", "--policies", policies, "--schema", schema, "--tests", tests];
        if json {
            args.extend(["--schema-format", "json", "--policy-format", "json"]);
        }
        self.passes(dir, &args, n)
    }

    /// `cedar run-tests` without a schema, for policies that do not validate against theirs
    /// (`run-tests` given a schema validates the policies first, and fails every test when they do
    /// not): every one of the `n` cases passes.
    pub fn run_tests_without_schema(&self, dir: &Path, policies: &str, tests: &str, n: usize) -> Result<(), String> {
        self.passes(dir, &["run-tests", "--policies", policies, "--tests", tests], n)
    }

    /// `cedar run-tests` with `args`: every one of the `n` cases passes.
    fn passes(&self, dir: &Path, args: &[&str], n: usize) -> Result<(), String> {
        let r = self.run(dir, args);
        let all = format!("results: {n} passed, 0 failed");
        if r.ok && r.stdout.lines().any(|l| l == all) {
            return Ok(());
        }
        let failed: Vec<&str> = r.stdout.lines().filter(|l| l.starts_with("  test ") && !l.ends_with(" ok")).take(6).collect();
        let last = r.stdout.lines().rfind(|l| l.starts_with("results: ")).unwrap_or("no results line");
        Err(format!("cedar {}: exit {:?}, {last} (of {n})\n{}\n{}", args.join(" "), r.code, failed.join("\n"), r.stderr))
    }

    /// `cedar run-tests` on tests the schema is to refuse: each is said to be an error, and none
    /// is answered.
    pub fn refuses(&self, dir: &Path, policies: &str, schema: &str, tests: &str, names: &[String], json: bool) -> Result<(), String> {
        let mut args = vec!["run-tests", "--policies", policies, "--schema", schema, "--tests", tests];
        if json {
            args.extend(["--schema-format", "json", "--policy-format", "json"]);
        }
        let r = self.run(dir, &args);
        let answered: Vec<String> = names.iter().filter(|n| !r.stdout.lines().any(|l| l == format!("  test {n} ... error:"))).map(|n| format!("`{n}`")).collect();
        if answered.is_empty() {
            Ok(())
        } else {
            Err(format!("cedar {}: the schema takes {} of the {} tests that each leave out an attribute it requires: {}\n{}", args.join(" "), answered.len(), names.len(), answered.join(", "), r.stdout))
        }
    }
}

/// Where two texts first differ, with a little of each around it.
pub fn first_difference(cli: &str, file: &str) -> String {
    let at = cli.char_indices().zip(file.chars()).find(|((_, a), b)| a != b).map(|((i, _), _)| i).unwrap_or(cli.len().min(file.len()));
    let around = |s: &str| {
        let from = s.floor_char_boundary(at.saturating_sub(60));
        let to = s.ceil_char_boundary((at + 60).min(s.len()));
        format!("…{}…", &s[from..to])
    };
    format!("at byte {at}:\n  the CLI: {}\n  the file: {}", around(cli), around(file))
}

/// Every `.gate` under `examples/` and `tests/`, sorted.
pub fn gates() -> Vec<String> {
    fn walk(d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "gate") {
                out.push(p.to_string_lossy().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(Path::new("examples"), &mut out);
    walk(Path::new("tests"), &mut out);
    out.sort();
    out
}

/// `f` on each of `items`, on as many threads as the machine has cores (at most eight); the
/// answers in the order of the items.
pub fn each_on_threads<T: Sync, R: Send>(items: &[T], f: impl Fn(usize, &T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let answers: Mutex<Vec<(usize, R)>> = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= items.len() {
                        break;
                    }
                    let r = f(i, &items[i]);
                    answers.lock().unwrap().push((i, r));
                }
            });
        }
    });
    let mut a = answers.into_inner().unwrap();
    a.sort_by_key(|(i, _)| *i);
    a.into_iter().map(|(_, r)| r).collect()
}

/// One change of a policy set or of its schema: what it is, and the policies or the schema it makes.
pub struct Change {
    pub what: String,
    pub policies: Option<PolicySet>,
    pub schema: Option<Schema>,
}

pub fn with_kind(e: &Expr, kind: ExprKind) -> Expr {
    Expr { kind, line: e.line, col: e.col }
}

/// The operator put in another's place: the one that reads otherwise beside it.
pub fn other_ops(op: BinOp) -> &'static [BinOp] {
    match op {
        BinOp::Eq => &[BinOp::NotEq],
        BinOp::NotEq => &[BinOp::Eq],
        BinOp::Less => &[BinOp::LessEq],
        BinOp::LessEq => &[BinOp::Less],
        BinOp::Greater => &[BinOp::GreaterEq],
        BinOp::GreaterEq => &[BinOp::Greater],
        BinOp::And => &[BinOp::Or],
        BinOp::Or => &[BinOp::And],
        BinOp::In => &[BinOp::Eq],
        BinOp::Add | BinOp::Sub | BinOp::Mul => &[],
    }
}

/// Each change of an expression at one place, and what it is: a number moved by one, a string
/// given another letter, a role put in another's place, an operator put in another's place, a
/// `has` taken away from the `&&` it guards, a `!` taken away.
pub fn expr_changes(e: &Expr, roles: &[EntityUid]) -> Vec<(String, Expr)> {
    let mut out = Vec::new();
    match &e.kind {
        ExprKind::Long(n) => {
            for m in [n.checked_sub(1), n.checked_add(1)].into_iter().flatten() {
                out.push((format!("{n} made {m}"), with_kind(e, ExprKind::Long(m))));
            }
        }
        ExprKind::Str(s) => out.push((format!("\"{s}\" made \"{s}_\""), with_kind(e, ExprKind::Str(format!("{s}_"))))),
        ExprKind::Bool(b) => out.push((format!("{b} made {}", !b), with_kind(e, ExprKind::Bool(!b)))),
        ExprKind::Entity(u) => {
            for r in roles.iter().filter(|r| r.ty == u.ty && r.id != u.id) {
                out.push((format!("{}::\"{}\" made \"{}\"", u.ty, u.id, r.id), with_kind(e, ExprKind::Entity(r.clone()))));
            }
        }
        ExprKind::Binary { op, left, right } => {
            for o in other_ops(*op) {
                out.push((format!("`{}` made `{}`", op.as_str(), o.as_str()), with_kind(e, ExprKind::Binary { op: *o, left: left.clone(), right: right.clone() })));
            }
            if *op == BinOp::And {
                if matches!(left.kind, ExprKind::Has { .. }) {
                    out.push((format!("`{}` taken away", cedar::write_expr(left)), (**right).clone()));
                }
                if matches!(right.kind, ExprKind::Has { .. }) {
                    out.push((format!("`{}` taken away", cedar::write_expr(right)), (**left).clone()));
                }
            }
        }
        ExprKind::Not(inner) => out.push(("`!` taken away".to_string(), (**inner).clone())),
        _ => {}
    }
    // and the same at a place inside
    match &e.kind {
        ExprKind::Binary { op, left, right } => {
            for (w, l) in expr_changes(left, roles) {
                out.push((w, with_kind(e, ExprKind::Binary { op: *op, left: Box::new(l), right: right.clone() })));
            }
            for (w, r) in expr_changes(right, roles) {
                out.push((w, with_kind(e, ExprKind::Binary { op: *op, left: left.clone(), right: Box::new(r) })));
            }
        }
        ExprKind::Not(inner) => {
            for (w, x) in expr_changes(inner, roles) {
                out.push((w, with_kind(e, ExprKind::Not(Box::new(x)))));
            }
        }
        ExprKind::If { cond, then, els } => {
            for (w, x) in expr_changes(cond, roles) {
                out.push((w, with_kind(e, ExprKind::If { cond: Box::new(x), then: then.clone(), els: els.clone() })));
            }
            for (w, x) in expr_changes(then, roles) {
                out.push((w, with_kind(e, ExprKind::If { cond: cond.clone(), then: Box::new(x), els: els.clone() })));
            }
            for (w, x) in expr_changes(els, roles) {
                out.push((w, with_kind(e, ExprKind::If { cond: cond.clone(), then: then.clone(), els: Box::new(x) })));
            }
        }
        _ => {}
    }
    out
}

/// The entities a policy set names in its scopes and conditions whose type is `Role` (DESIGN
/// 5.1): each may be put in another's place.
pub fn roles_of(set: &PolicySet) -> Vec<EntityUid> {
    let mut out: Vec<EntityUid> = Vec::new();
    let mut add = |u: &EntityUid| {
        if u.ty.id == "Role" && !out.iter().any(|o| o.ty == u.ty && o.id == u.id) {
            out.push(EntityUid::new(u.ty.clone(), u.id.clone()));
        }
    };
    for p in &set.policies {
        for s in [&p.principal, &p.resource] {
            if let Scope::Eq(EntityOrSlot::Entity(u)) | Scope::In(EntityOrSlot::Entity(u)) | Scope::IsIn(_, EntityOrSlot::Entity(u)) = s {
                add(u);
            }
        }
        for c in &p.conditions {
            cedar::walk(&c.body, &mut |e| {
                if let ExprKind::Entity(u) = &e.kind {
                    add(u);
                }
            });
        }
    }
    out
}

/// The actions the policies name in their scopes.
pub fn actions_of(set: &PolicySet) -> Vec<EntityUid> {
    let mut out: Vec<EntityUid> = Vec::new();
    for p in &set.policies {
        let named: Vec<&EntityUid> = match &p.action {
            ActionScope::Eq(a) | ActionScope::In(a) => vec![a],
            ActionScope::InList(l) => l.iter().collect(),
            ActionScope::Any => vec![],
        };
        for a in named {
            if !out.iter().any(|o| o.ty == a.ty && o.id == a.id) {
                out.push(EntityUid::new(a.ty.clone(), a.id.clone()));
            }
        }
    }
    out
}

/// Each change of the policies at one place.
pub fn policy_changes(set: &PolicySet) -> Vec<Change> {
    let roles = roles_of(set);
    let actions = actions_of(set);
    let mut out = Vec::new();
    for (i, p) in set.policies.iter().enumerate() {
        let mut push = |what: String, q: Policy| {
            let mut s = set.clone();
            s.policies[i] = q;
            out.push(Change { what: format!("{}: {what}", p.id), policies: Some(s), schema: None });
        };
        let mut q = p.clone();
        q.effect = if p.effect == Effect::Forbid { Effect::Permit } else { Effect::Forbid };
        push(if p.effect == Effect::Forbid { "forbid made permit".into() } else { "permit made forbid".into() }, q);
        if let Scope::In(EntityOrSlot::Entity(u)) = &p.principal {
            let mut q = p.clone();
            q.principal = Scope::Eq(EntityOrSlot::Entity(u.clone()));
            push(format!("principal in {}::\"{}\" made ==", u.ty, u.id), q);
            for r in roles.iter().filter(|r| r.ty == u.ty && r.id != u.id) {
                let mut q = p.clone();
                q.principal = Scope::In(EntityOrSlot::Entity(r.clone()));
                push(format!("principal in \"{}\" made in \"{}\"", u.id, r.id), q);
            }
        }
        match &p.action {
            ActionScope::Eq(a) => {
                for b in actions.iter().filter(|b| b.id != a.id) {
                    let mut q = p.clone();
                    q.action = ActionScope::Eq(b.clone());
                    push(format!("action == \"{}\" made \"{}\"", a.id, b.id), q);
                }
            }
            ActionScope::InList(l) if l.len() > 1 => {
                for (j, a) in l.iter().enumerate() {
                    let mut q = p.clone();
                    let mut l2 = l.clone();
                    l2.remove(j);
                    q.action = ActionScope::InList(l2);
                    push(format!("\"{}\" taken out of the actions", a.id), q);
                }
            }
            _ => {}
        }
        for (j, c) in p.conditions.iter().enumerate() {
            let shown = format!("{} {{ {} }}", if c.kind == CondKind::When { "when" } else { "unless" }, cedar::write_expr(&c.body));
            let mut q = p.clone();
            q.conditions[j].kind = if c.kind == CondKind::When { CondKind::Unless } else { CondKind::When };
            push(format!("`{shown}` made {}", if c.kind == CondKind::When { "unless" } else { "when" }), q);
            let mut q = p.clone();
            q.conditions.remove(j);
            push(format!("`{shown}` taken away"), q);
            for (w, e) in expr_changes(&c.body, &roles) {
                let mut q = p.clone();
                q.conditions[j].body = e;
                push(format!("in `{shown}`, {w}"), q);
            }
        }
        let mut s = set.clone();
        s.policies.remove(i);
        out.push(Change { what: format!("{}: the policy taken away", p.id), policies: Some(s), schema: None });
    }
    out
}
