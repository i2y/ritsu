//! The two versions of the example (PLAN A1): `refunds.gate` and `refunds.ja.gate` read with no
//! diagnostic, and are the same tree but for the names, the strings and the files they read. A
//! declared name is shown by what Cedar calls it (the Japanese version's aliases are the English
//! version's names), a reference by what Cedar calls the declaration it finds in the file (a
//! reference to something the file does not declare — a rule's input, output or value — by `?`),
//! and a `use` by its place among the uses.

use sekisho::ast::*;
use sekisho::parse::parse;

const EN: &str = "examples/refunds/refunds.gate";
const JA: &str = "examples/refunds/refunds.ja.gate";

fn read(path: &str) -> File {
    let src = std::fs::read_to_string(path).unwrap();
    let p = parse(path, &src);
    let said: Vec<String> = p.diags.iter().map(|d| d.render(ritsu_base::text::Lang::En)).collect();
    assert!(p.diags.is_empty(), "{path}:\n{}", said.join(""));
    p.file.unwrap()
}

/// What Cedar calls the declaration a word finds in the file, `?` for none.
fn found(f: &File, word: &str) -> String {
    let mut names: Vec<&Name> = Vec::new();
    for e in &f.enums {
        names.push(&e.name);
        names.extend(e.values.iter());
    }
    names.extend(f.roles.iter().map(|r| &r.name));
    for e in f.principals.iter().chain(f.resources.iter()) {
        names.push(&e.name);
        names.extend(e.attributes.iter().map(|a| &a.name));
    }
    names.extend(f.workflows.iter().map(|w| &w.name));
    for a in &f.actions {
        names.push(&a.name);
        names.extend(a.input.iter().map(|i| &i.name));
        names.extend(a.context.iter().map(|c| &c.name));
    }
    names.extend(f.policies.iter().map(|p| &p.name));
    if word == WORKFLOW_TYPE {
        return word.to_string();
    }
    names.iter().find(|n| n.is(word)).map(|n| n.ascii().to_string()).unwrap_or_else(|| "?".into())
}

/// A `use` by its place among the uses (its name is the file's own, in either language).
fn used(f: &File, word: &str) -> String {
    match f.uses.iter().position(|u| u.name.as_ref().is_some_and(|(n, _)| n == word)) {
        Some(i) => format!("use#{i}"),
        None => "?".into(),
    }
}

fn refs(f: &File, rs: &[Ref]) -> String {
    rs.iter().map(|r| found(f, &r.word)).collect::<Vec<_>>().join(",")
}

fn path(f: &File, p: &Path) -> String {
    match p {
        Path::Principal(r) => format!("principal.{}", found(f, &r.word)),
        Path::Resource(r) => format!("resource.{}", found(f, &r.word)),
        Path::Local(r) => found(f, &r.word),
    }
}

fn lit(l: &Lit) -> String {
    match l {
        Lit::Num(n) => format!("{}/{}{}", n.value.num, n.value.den, n.unit),
        Lit::Date(d) => sekisho::types::day_text(*d),
        Lit::Bool(b) => b.to_string(),
    }
}

fn expr(f: &File, e: &Expr) -> String {
    match e {
        Expr::Or(es) => format!("or({})", es.iter().map(|e| expr(f, e)).collect::<Vec<_>>().join(", ")),
        Expr::And(es) => format!("and({})", es.iter().map(|e| expr(f, e)).collect::<Vec<_>>().join(", ")),
        Expr::Not(e, _) => format!("not({})", expr(f, e)),
        Expr::Atom(a, _) => match a {
            Atom::InRole(r) => format!("in {}", found(f, &r.word)),
            Atom::InGroup(p) => format!("in {}", path(f, p)),
            Atom::IsType(r) => format!("is {}", found(f, &r.word)),
            Atom::IsWorkflow(r) => format!("is workflow {}", found(f, &r.word)),
            Atom::Is { left, not, right } => {
                let r = match right {
                    Rhs::Word(w) => found(f, &w.word),
                    Rhs::Principal(_) => "principal".into(),
                    Rhs::Path(p) => path(f, p),
                    Rhs::Lit(l, _) => lit(l),
                };
                format!("{} is{} {r}", path(f, left), if *not { " not" } else { "" })
            }
            Atom::Compare { left, op, right } => format!("{} {} {}", path(f, left), op.symbol(), lit(&right.0)),
            Atom::Holds(p) => path(f, p),
        },
    }
}

fn field(f: &File, x: &Field) -> String {
    let ty = match &x.ty {
        Type::Bool => "bool".to_string(),
        Type::Date => "date".to_string(),
        Type::Unit(u) => u.clone(),
        Type::Named(r) => found(f, &r.word),
    };
    let range = x.range.as_ref().map(|r| format!(" {:?}..{:?}", r.lo.as_ref().map(|l| lit(&l.0)), r.hi.as_ref().map(|l| lit(&l.0)))).unwrap_or_default();
    format!("{} : {ty}{}{range}", x.name.ascii(), if x.optional { "?" } else { "" })
}

fn args(f: &File, args: &[Arg]) -> String {
    args.iter()
        .map(|a| {
            let v = match &a.value {
                ArgValue::Path(p) => path(f, p),
                ArgValue::Today(_) => "today".into(),
                ArgValue::Lit(l, _) => lit(l),
            };
            format!("{}: {v}", found(f, &a.name.word))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn body(f: &File, b: &Body) -> Vec<String> {
    let mut out = Vec::new();
    if let Some((w, _)) = &b.principal {
        out.push(match w {
            Who::In(rs) => format!("  principal in {}", refs(f, rs)),
            Who::Is(r) => format!("  principal is {}", found(f, &r.word)),
            Who::Workflow(r) => format!("  principal is workflow {}", found(f, &r.word)),
        });
    }
    out.push(match &b.action.0 {
        What::Any => "  action any".into(),
        What::Actions(rs) => format!("  action {}", refs(f, rs)),
    });
    for c in &b.conds {
        out.push(format!("  {} {}", if c.when { "when" } else { "unless" }, expr(f, &c.expr)));
    }
    out
}

/// The tree, a line at a time, without the names that differ between the versions.
fn shape(f: &File) -> Vec<String> {
    let mut out = vec![format!("gate {}", f.version)];
    for u in &f.uses {
        out.push(format!("use {}", u.kind.word()));
    }
    if let Some(t) = &f.today {
        out.push(format!("today {:?}..{:?} offset {}", t.range.lo.as_ref().map(|l| lit(&l.0)), t.range.hi.as_ref().map(|l| lit(&l.0)), t.offset.0));
    }
    for e in &f.enums {
        out.push(format!("enum {} = {}", e.name.ascii(), e.values.iter().map(|v| v.ascii()).collect::<Vec<_>>().join(" | ")));
    }
    for r in &f.roles {
        out.push(format!("role {} includes [{}] can {:?}", r.name.ascii(), refs(f, &r.includes), r.can.as_ref().map(|(c, _)| refs(f, c))));
    }
    for e in f.principals.iter().chain(f.resources.iter()) {
        out.push(format!("{:?} {} roles [{}]", e.kind, e.name.ascii(), refs(f, &e.roles)));
        out.extend(e.attributes.iter().map(|a| format!("  {}", field(f, a))));
    }
    for w in &f.workflows {
        out.push(format!("workflow {}", w.name.ascii()));
    }
    for a in &f.actions {
        out.push(format!("action {}", a.name.ascii()));
        out.extend(a.guards.iter().map(|g| format!("  guards {} {}", used(f, &g.api.word), g.operation.0)));
        out.push(format!("  principal {}", refs(f, &a.principals.0)));
        out.push(format!("  resource {} from {:?}", refs(f, &a.resources.0), a.resource_from.as_ref().map(|r| &r.0)));
        out.extend(a.input.iter().map(|i| format!("  input {}", field(f, i))));
        for c in &a.context {
            let v = match &c.value {
                Computation::Rule { rule, args: xs, output } => format!("{}({}).{}", used(f, &rule.word), args(f, xs), found(f, &output.word)),
                Computation::Date { op, date } => {
                    let d = match date {
                        DateValue::Call { dates, date, args: xs } => format!("{}.{}({})", used(f, &dates.word), found(f, &date.word), args(f, xs)),
                        DateValue::Attr(p) => path(f, p),
                    };
                    format!("today {} {d}", op.symbol())
                }
                Computation::Open { calendar } => format!("today is open in {}", used(f, &calendar.word)),
            };
            out.push(format!("  context {} = {v}", c.name.ascii()));
        }
    }
    for p in &f.policies {
        out.push(format!("{} {}", p.effect.word(), p.name.ascii()));
        out.extend(body(f, &p.body));
    }
    for e in &f.expects {
        out.push(format!("expect {}", if e.allow { "allow" } else { "deny" }));
        out.extend(body(f, &e.body));
    }
    for s in &f.separates {
        out.push(format!("separate {}", refs(f, &s.actions.0)));
    }
    out
}

#[test]
fn both_versions_of_the_example_read() {
    let en = read(EN);
    let ja = read(JA);
    assert_eq!((en.actions.len(), en.policies.len(), en.expects.len(), en.separates.len()), (3, 10, 3, 1));
    assert_eq!((ja.alias(), en.alias()), ("refunds_ja", "refunds"));
}

#[test]
fn the_two_versions_are_one_tree_but_for_the_names() {
    let (en, ja) = (shape(&read(EN)), shape(&read(JA)));
    for (i, (a, b)) in en.iter().zip(ja.iter()).enumerate() {
        assert_eq!(a, b, "line {i} of the shape");
    }
    assert_eq!(en.len(), ja.len());
}
