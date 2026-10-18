//! kaname's expressions are a superset of rulec's (DESIGN 2.4, 16.2): the right-hand side of every
//! `derive` and `define` of rulec's corpus, read as kaname, has the type rulec gives it (its unit and
//! tax, the step its values land on, an enum, a bool) and, on every input of rulec's vectors, the
//! value rulec's evaluator gives the name.
//!
//! Each expression is written into a module of its own: a function whose parameters are the names
//! the expression reads that rulec takes from outside (inputs, table outputs, the summaries of a
//! walk), typed as rulec types them, and whose `let` lines are the `derive` and `define` lines it
//! reads, in rulec's order, then its own (`let <name> : <type> = <expression>`, the type as the rule
//! writes it). The value of the last `let` is what is compared. A line's citation of its source
//! (`@<source> <fragment>`) is no expression and is left out; so are a `derive`'s range and a
//! comment. What cannot be read for a reason that is not kaname's (a name kaname cannot have, a name
//! a rule takes from another it applies) is counted by its reason, and the count is held to a bound.

mod common;

use kaname::interp::{Event, Interp, Stop};
use kaname::ty::Ty as KTy;
use kaname::value::{Program, Value};
use rulec::ast::{Expr, Item, RuleFile};
use rulec::eval::Val;
use rulec::types::{Checked, Ty};
use ritsu_units::Rat;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// Every `.rule` of rulec's corpus, its subdirectories too, in a fixed order.
fn corpus() -> Vec<PathBuf> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        let mut es: Vec<PathBuf> = std::fs::read_dir(d).unwrap().map(|e| e.unwrap().path()).collect();
        es.sort();
        for p in es {
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rule") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(Path::new("../rulec/tests/corpus"), &mut out);
    out
}

/// A `derive` or a `define`: its name, its line, and its expression.
struct Line<'a> {
    name: String,
    line: usize,
    expr: &'a Expr,
}

/// The `derive` and `define` items of a rule, with whether each is a line written in this file: the
/// items hold too what the rule takes from the rules it applies (`base:decay_rate`), the values it
/// opens to contributions (`open`, which rulec lowers into a `define`), and what contributions add.
fn lines<'f>(f: &'f RuleFile, src: &str) -> Vec<(Line<'f>, bool)> {
    let starts = |line: usize, name: &str| {
        src.lines().nth(line.wrapping_sub(1)).is_some_and(|l| (l.starts_with("derive ") || l.starts_with("define ")) && l.contains(name))
    };
    f.items
        .iter()
        .filter_map(|it| match it {
            Item::Derived(d) => Some((Line { name: d.name.text.clone(), line: d.span.line, expr: &d.expr }, d.span.file.is_none() && starts(d.span.line, &d.name.text))),
            Item::Define(d) => Some((Line { name: d.name.text.clone(), line: d.span.line, expr: &d.expr }, d.span.file.is_none() && starts(d.span.line, &d.name.text))),
            _ => None,
        })
        .collect()
}

/// The `derive` and `define` lines a file holds, as its text says.
fn written_lines(src: &str) -> usize {
    src.lines().filter(|l| l.starts_with("derive ") || l.starts_with("define ")).count()
}

/// The names an expression reads.
fn names(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Name(n, _) => {
            if !out.contains(n) {
                out.push(n.clone());
            }
        }
        Expr::Lit(..) => {}
        Expr::Bin(a, _, b, _) => {
            names(a, out);
            names(b, out);
        }
        Expr::Call(_, args, _) => {
            for a in args {
                names(a, out);
            }
        }
    }
}

/// The type and the right-hand side of a `derive` or `define` line, as written: what lies between
/// its `:` and its `=`, and after the `=`, its range, its citation and its comment taken off.
fn written(src: &str, line: usize) -> Option<(String, String)> {
    let text = src.lines().nth(line - 1)?;
    let colon = text.find(" : ").or_else(|| text.find(':'))?;
    let eq = colon + text[colon..].find(" = ")?;
    let ty = text[colon..eq].trim_start_matches(" : ").trim_start_matches(':').trim().to_string();
    let mut rhs = &text[eq + 3..];
    for cut in [" #", "@", "  range ", " range >", " range <"] {
        if let Some(i) = rhs.find(cut) {
            rhs = &rhs[..i];
        }
    }
    Some((ty, rhs.trim().to_string()))
}

/// rulec's type of a name as kaname writes it, with the step rulec says its values land on.
fn type_text(c: &Checked, name: &str, ty: &Ty) -> Result<String, &'static str> {
    Ok(match ty {
        Ty::Money { cur, tax: Some(t) } => format!("money[{cur}, {t}]"),
        Ty::Money { cur, tax: None } => format!("money[{cur}]"),
        Ty::Qty { dim, unit } => format!("{dim}[{unit}]"),
        Ty::Rate => format!("rate[step {}%]", c.value_step(name).mul(Rat::int(100))),
        Ty::Number => "number".into(),
        Ty::Bool => "bool".into(),
        Ty::Str => "string".into(),
        Ty::Date => "date".into(),
        Ty::Enum(e) => e.clone(),
        Ty::Opt(t) => format!("{}?", type_text(c, name, t)?),
        Ty::Unknown => return Err("a name rulec gives no type"),
    })
}

/// A range wide enough for every value rulec's vectors give: in the type's unit, on its step.
fn wide(ty: &Ty) -> String {
    match ty.present() {
        Ty::Money { cur, .. } => format!(" range >=-1_000_000_000_000_000{cur} <=1_000_000_000_000_000{cur}"),
        Ty::Qty { unit, .. } => format!(" range >=-1_000_000_000_000_000{unit} <=1_000_000_000_000_000{unit}"),
        Ty::Rate => " range >=-1_000_000% <=1_000_000%".into(),
        Ty::Number => " range >=-1_000_000_000_000_000 <=1_000_000_000_000_000".into(),
        Ty::Date => " range >=0001-01-01 <=9999-12-31".into(),
        _ => String::new(),
    }
}

/// rulec's value of a name as kaname holds it.
fn value(v: &Val, ty: &Ty) -> Option<Value> {
    Some(match (v, ty) {
        (Val::Enum(n), Ty::Opt(_)) if n == "none" => Value::None,
        (v, Ty::Opt(t)) => value(v, t)?,
        (Val::Num(r), _) => Value::Num(*r),
        (Val::Bool(b), _) => Value::Bool(*b),
        (Val::Str(s), _) => Value::Str(s.clone()),
        (Val::Date(y, m, d), _) => Value::Date(kaname::date::day_of(*y as i64, *m as i64, *d as i64)),
        (Val::Enum(e), _) => Value::Enum(e.clone()),
        (Val::Seq(_), _) => return None,
    })
}

/// What happened to one expression.
#[derive(Default)]
struct Tally {
    expressions: usize,
    compared: usize,
    values: usize,
    skipped: BTreeMap<&'static str, usize>,
    wrong: Vec<String>,
}

/// One `derive` or `define` of a rule: its module written, checked, its type and its values compared.
#[allow(clippy::too_many_arguments)]
fn one(file: &Path, src: &str, f: &RuleFile, c: &Checked, all: &[Line], here_lines: &[bool], k: usize, vectors: &[rulec::vectors::Vector], t: &mut Tally) {
    let target = &all[k];
    let here = format!("{}:{} {}", file.display(), target.line, target.name);
    // the `derive` and `define` lines it reads, in rulec's order, and the names it takes from outside
    // (a value opened to contributions among them: its value is what rulec's evaluator gives it)
    let defined: BTreeMap<&str, usize> = all.iter().enumerate().filter(|(i, _)| here_lines[*i]).map(|(i, l)| (l.name.as_str(), i)).collect();
    let mut need: BTreeSet<usize> = BTreeSet::from([k]);
    let mut stack = vec![k];
    let mut outside: Vec<String> = Vec::new();
    while let Some(i) = stack.pop() {
        let mut ns = Vec::new();
        names(all[i].expr, &mut ns);
        for n in ns {
            match defined.get(n.as_str()) {
                Some(&j) if j < i => {
                    if need.insert(j) {
                        stack.push(j);
                    }
                }
                _ => {
                    if !outside.contains(&n) {
                        outside.push(n);
                    }
                }
            }
        }
    }
    let reason = |t: &mut Tally, why: &'static str| {
        *t.skipped.entry(why).or_default() += 1;
    };
    if outside.iter().chain(need.iter().map(|i| &all[*i].name)).any(|n| n.contains(':')) {
        return reason(t, "a name the rule takes from a rule it applies (`base:decay_rate`)");
    }
    if outside.iter().chain(need.iter().map(|i| &all[*i].name)).any(|n| kaname::kw::is_reserved(n)) {
        return reason(t, "a name that is a keyword of kaname's");
    }
    // the parameters, as rulec types them
    let mut params = Vec::new();
    let mut enums: BTreeSet<String> = BTreeSet::new();
    for (i, n) in outside.iter().enumerate() {
        let Some(ty) = c.ty_of(n) else { return reason(t, "a name rulec does not know") };
        let Ok(text) = type_text(c, n, &ty) else { return reason(t, "a name rulec gives no type") };
        if let Ty::Enum(e) = ty.present() {
            enums.insert(e.clone());
        }
        // a value from outside is whole in its unit, or on a rate's step: else kaname has no
        // parameter for it
        if matches!(ty.present(), Ty::Money { .. } | Ty::Qty { .. } | Ty::Number) && !c.value_step(n).is_int() {
            return reason(t, "an unrounded value a table gives");
        }
        params.push(format!("{n}(p{i}): {text}{}", wide(&ty)));
    }
    // the lines, each with its type as the rule writes it
    let mut lets = Vec::new();
    for i in &need {
        let Some((ty, rhs)) = written(src, all[*i].line) else { return reason(t, "a line kaname's test cannot cut out") };
        if let Some(Ty::Enum(e)) = c.ty_of(&all[*i].name).as_ref().map(Ty::present) {
            enums.insert(e.clone());
        }
        lets.push(format!("  let {} : {ty} = {rhs}", all[*i].name));
    }
    let mut module = String::from("module superset v1\n\n");
    for (ei, e) in enums.iter().enumerate() {
        let vs = c.enums.get(e).cloned().unwrap_or_default();
        if vs.iter().any(|v| kaname::kw::is_reserved(v)) {
            return reason(t, "a value of an enum that is a keyword of kaname's");
        }
        let sep = if c.ordered.contains(e) { " < " } else { " | " };
        let values: Vec<String> = vs.iter().enumerate().map(|(vi, v)| format!("{v}(v{ei}_{vi})")).collect();
        module.push_str(&format!("enum {e}(e{ei}) = {}\n", values.join(sep)));
    }
    module.push_str(&format!("\nfn f({}) -> bool\n{}\n  true\n", params.join(", "), lets.join("\n")));
    t.expressions += 1;
    let o = common::check_text("superset.kn", &module);
    // the module as the check of types reads it: the analysis of ranges has its own tests, and the
    // ranges here are as wide as the expression's values can need
    let Some(m) = o.analysed.clone() else {
        t.wrong.push(format!("{here}: kaname does not read it:\n{module}\n{}", common::shown(&o, ritsu_base::text::Lang::En)));
        return;
    };
    // the type: the unit, the tax and the step, an enum, a bool
    let decl = &m.module.fns[0].body.lets[need.iter().position(|i| *i == k).unwrap()];
    let ours = m.typed.bound.get(&decl.value.id).cloned().unwrap_or(KTy::Error);
    let rulec_ty = c.ty_of(&target.name).unwrap();
    let same = match (&ours, &rulec_ty) {
        (KTy::Num(n), r) => {
            let step = c.grids.get(&target.name).copied().unwrap_or_else(|| c.value_step(&target.name));
            let unit = r.unit(None);
            let same_unit = unit.is_some_and(|u| kaname::ty::same_unit(&u, &n.unit));
            same_unit && n.step == Some(step)
        }
        (KTy::Bool, Ty::Bool) | (KTy::Date, Ty::Date) | (KTy::Str, Ty::Str) => true,
        (KTy::Enum(id), Ty::Enum(e)) => id.name == *e,
        _ => false,
    };
    if !same {
        let step = c.grids.get(&target.name).copied().unwrap_or_else(|| c.value_step(&target.name));
        t.wrong.push(format!("{here}: kaname types it {ours:?}, rulec {rulec_ty:?} on the step {step}"));
        return;
    }
    t.compared += 1;
    // the values, on every input of rulec's vectors
    let p = Program::of(&m);
    let suite = common::joined();
    for v in vectors {
        let inputs: HashMap<String, Val> = v.input.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let (_, _, binds) = rulec::eval::run_bindings(f, c, inputs);
        let Some(want) = binds.get(&target.name) else { continue };
        let mut args = Vec::new();
        for n in &outside {
            let ty = c.ty_of(n).unwrap();
            match binds.get(n).and_then(|b| value(b, &ty)) {
                Some(x) => args.push(x),
                None => break,
            }
        }
        if args.len() != outside.len() {
            continue;
        }
        let mut it = Interp::new(&p, &suite);
        it.lets = true;
        match it.call(&m, 0, args, 0) {
            Ok(_) => {}
            Err(Stop::Failed(fl)) => {
                t.wrong.push(format!("{here}: kaname fails with {} ({}) where rulec gives {want:?}", fl.kind.word(), fl.message.en));
                return;
            }
            Err(Stop::Diverged(_)) => unreachable!("no recursion"),
        }
        let got = it.trace.events.iter().rev().find_map(|e| match e {
            Event::Let(b) if b.name == target.name => Some(b.value.clone()),
            _ => None,
        });
        let want = value(want, &rulec_ty);
        if got != want {
            t.wrong.push(format!("{here}: kaname gives {got:?}, rulec {want:?}"));
            return;
        }
        t.values += 1;
    }
}

#[test]
fn every_expression_of_rulecs_corpus_is_kanames_with_its_type_and_its_values() {
    let mut t = Tally::default();
    let (mut files, mut written) = (0, 0);
    for file in corpus() {
        let src = std::fs::read_to_string(&file).unwrap();
        written += written_lines(&src);
        let path = file.to_string_lossy().to_string();
        let Ok((f, c)) = rulec::prepare(&src, &path) else {
            *t.skipped.entry("a file rulec reads only with another (a contribution, an enums file)").or_default() += written_lines(&src);
            continue;
        };
        let (all, here): (Vec<Line>, Vec<bool>) = lines(&f, &src).into_iter().unzip();
        if !here.iter().any(|h| *h) {
            continue;
        }
        files += 1;
        let vectors = rulec::vectors::generate(&f, &c);
        for k in 0..all.len() {
            // a line of a rule applied or of a contribution merged is read in its own file
            if here[k] {
                one(&file, &src, &f, &c, &all, &here, k, &vectors, &mut t);
            }
        }
    }
    let skipped: usize = t.skipped.values().sum();
    eprintln!(
        "superset: {written} `derive` and `define` lines in {} files of rulec's corpus; {} read as kaname, {} with rulec's type, {} values compared on rulec's vectors; {skipped} not read",
        files, t.expressions, t.compared, t.values
    );
    for (why, n) in &t.skipped {
        eprintln!("  not read, {n}: {why}");
    }
    assert!(t.wrong.is_empty(), "{} of the expressions differ:\n{}", t.wrong.len(), t.wrong.join("\n\n"));
    assert_eq!(t.compared, t.expressions);
    assert_eq!(t.expressions + skipped, written, "every line is read or counted with its reason");
    // the expressions left out, by their reasons: none is, and one that comes fails here
    assert_eq!(skipped, 0, "{:?}", t.skipped);
}

/// What kaname adds to rulec's expressions (DESIGN 2.4, 16.2) — a division by a value, `in`, the
/// difference of two dates, comparisons put together with `and`, `or` and `not` — still gives no
/// unrounded value to a place: a quotient by a value and a conversion that lands between whole units
/// are E103 until a rounding says how.
#[test]
fn what_kaname_adds_keeps_unrounded_values_out_of_places() {
    let codes = |body: &str| -> Vec<&'static str> { common::codes_before_analysis(&common::check_text("t.kn", &format!("module m v1\n\n{body}"))) };
    let none: Vec<&str> = Vec::new();
    // a division by a value
    assert_eq!(codes("fn f(a: money[USDc] range >=0USDc <=100USDc, n: number range >=1 <=9) -> money[USDc]\n  a / n\n"), ["E103"]);
    assert_eq!(codes("fn f(a: money[USDc] range >=0USDc <=100USDc, n: number range >=1 <=9) -> money[USDc]\n  down(a / n, 1USDc)\n"), none);
    assert_eq!(codes("fn f(a: number range >=0 <=100, b: number range >=1 <=9) -> number\n  a / b\n"), ["E103"]);
    // a conversion into a larger unit
    assert_eq!(codes("fn f(w: mass[oz] range >=0oz <=100oz) -> mass[lb]\n  w in lb\n"), ["E103"]);
    assert_eq!(codes("fn f(w: mass[oz] range >=0oz <=100oz) -> mass[lb]\n  up(w in lb, 1lb)\n"), none);
    assert_eq!(codes("fn f(w: mass[lb] range >=0lb <=100lb) -> mass[oz]\n  w in oz\n"), none);
    // the days between two dates are whole; comparisons put together are a bool
    assert_eq!(codes("fn f(a: date range >=2026-01-01 <=2026-12-31, b: date range >=2026-01-01 <=2026-12-31) -> number\n  b - a\n"), none);
    assert_eq!(codes("fn f(a: number range >=0 <=9, b: number range >=0 <=9) -> bool\n  a > 1 and not (b < 2 or a = b)\n"), none);
    // the latitude of a declaration is a `let`'s alone: a place takes no rate for a number
    assert_eq!(codes("fn f(n: number range >=0 <=50) -> number\n  let r : rate = n / 50\n  0\n"), none);
    assert_eq!(codes("fn f(r: rate[step 1%]) -> number\n  r\n"), ["E102"]);
    assert_eq!(codes("fn f(r: rate[step 10%]) -> money[JPY, incl_tax]\n  let cost : money[JPY, incl_tax] = 10_000JPY * r\n  cost\n"), none);
    assert_eq!(codes("fn f(a: money[JPY] range >=0JPY <=9JPY, b: money[JPY, incl_tax] range >=0JPY <=9JPY) -> bool\n  a < b\n"), ["E102"]);
}
