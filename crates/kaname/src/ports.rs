//! ritsu's ports, as kaname answers them (DESIGN 11.2, 14). [`Engine`] is kaname as `ritsu check`
//! joins it: it checks a project's `.kn` files with the rules, the dates files and the books it is
//! handed, and with what the checks across the borders came to at their calls (`Verdicts`), and says
//! of each what `kaname check` says. It answers what those checks ask of a module (`Programs`: every
//! call into rulec, koyomi and chobo, with what kaname knows of each value it gives, and what the
//! function's vectors give there), and what a module holds and names (`Items`, `References`).

use crate::check::{self, Checked, Run};
use crate::ranges::{Abs, CallSite, Gave, Iv, SiteKind, Src, What};
use crate::ty::{RecId, Ty};
use crate::value::Program;
use ritsu_base::text::Lang;
use ritsu_ports::{At, Given, Origin, ProgramCrossings, ProgramDateCall, ProgramRuleCall, ProgramTransfer, Said, Seen, Verdicts};
use std::path::Path;
use std::rc::Rc;

/// kaname, as `ritsu check` reaches it.
#[derive(Default)]
pub struct Engine;

impl Engine {
    /// `kaname check` of each file, as `ritsu check` prints it (ritsu's DESIGN 8.3), with the rules,
    /// dates files and books read through `ports`, and the effects decided with `verdicts` when they
    /// are handed over (DESIGN 4.3): every finding, as the command prints it and as its `--format
    /// json` prints it, then the line that says the file passes. `files` are as the person gave them,
    /// from where the program runs; `root` is the project's.
    pub fn checked_with(&self, root: &Path, files: &[String], ports: &ritsu_ports::Ports, verdicts: Option<Rc<dyn Verdicts>>, lang: Lang) -> Vec<ritsu_ports::Checked> {
        let mut suite: crate::suite::Suite = ports.clone().into();
        if let Some(v) = verdicts {
            suite = suite.with_verdicts(v);
        }
        let run = Run::new(suite);
        files
            .iter()
            .map(|f| match run.file(f) {
                Ok(o) => check::unit(root, f, Ok(&o), lang),
                Err(e) => check::unit(root, f, Err(e), lang),
            })
            .collect()
    }

    /// The module at `file`, its names and types passing, analysed with the languages of `ports` and
    /// no verdicts (what the checks across the borders are asked about); else what its check says.
    fn analysed(file: &Path, ports: &ritsu_ports::Ports) -> Result<Rc<Checked>, Vec<Said>> {
        let shown = file.to_string_lossy().to_string();
        let o = Run::new(ports.clone().into()).file(&shown).map_err(|e| vec![Said::unreadable(&shown, &e)])?;
        match &o.analysed {
            Some(c) => Ok(c.clone()),
            None => Err(o.diags.iter().filter(|d| d.is_error()).map(Said::of).collect()),
        }
    }
}

/// Where a value is, as a verdict names it.
fn at(sp: crate::ast::Span) -> At {
    At { line: sp.line, col: sp.col }
}

/// The whole numbers a value of `step` is a count of, on the wire, for the values from `lo` to `hi`:
/// the first count at or above the one end, the last at or below the other.
fn wire_ends(iv: Iv, step: crate::num::Rat) -> (Option<i128>, Option<i128>) {
    let count = |v: crate::num::Rat| v.checked_div(step);
    let floor = |v: crate::num::Rat| v.num.div_euclid(v.den);
    let ceil = |v: crate::num::Rat| -(-v.num).div_euclid(v.den);
    (iv.lo.and_then(count).map(ceil), iv.hi.and_then(count).map(floor))
}

/// What a value can be, on the wire (`Given::range`): a number counted in the unit of its type (a
/// rate as a count of its step), a date as its day number, a list by its count; the value when it
/// is there, for one that may be none.
fn wire_range(a: &Abs, t: &Ty) -> (Option<i128>, Option<i128>) {
    let t = match t {
        Ty::Opt(x) => x,
        x => x,
    };
    match (a.present(), t) {
        (Abs::Num(iv), Ty::Num(n)) => {
            let step = if n.is_rate() { n.step.unwrap_or(crate::num::Rat::new(1, 100)) } else { crate::num::Rat::int(1) };
            wire_ends(*iv, step)
        }
        (Abs::Date(iv), _) => wire_ends(*iv, crate::num::Rat::int(1)),
        (Abs::List(count, _), _) => wire_ends(*count, crate::num::Rat::int(1)),
        _ => (None, None),
    }
}

/// One value a call is given, as `Programs` hands it over.
fn given(p: &Program, g: &Gave) -> Given {
    let range = wire_range(&g.value, &g.ty);
    let mut from: Vec<Origin> = Vec::new();
    for s in &g.from {
        let o = match s {
            Src::Day(canon, date) => Origin::Day { file: p.file(canon), date: date.clone() },
            Src::Output(canon, output) => Origin::Output { rule: p.file(canon), output: output.clone() },
            Src::Kaname => Origin::Range(range.0, range.1),
            Src::Outputs(_) => continue,
        };
        if !from.contains(&o) {
            from.push(o);
        }
    }
    if from.is_empty() {
        from.push(Origin::Range(range.0, range.1));
    }
    // a list a rule walks: each numeric field of its elements
    let mut fields = Vec::new();
    if let (Ty::List(el), Abs::List(_, elem)) = (&g.ty, &g.value)
        && let Ty::Record(id @ RecId::Element(..)) = &**el
    {
        for (f, slot) in p.fields(id) {
            if matches!(slot.ty, Ty::Num(_)) {
                fields.push((f.text.clone(), wire_range(&elem.field(&f.text), &slot.ty)));
            }
        }
    }
    Given { at: at(g.span), to: g.to.clone(), shown: g.shown.clone(), range, from, fields }
}

/// Every call a module's functions make into the other languages (DESIGN 14.2).
fn crossings_of(c: &Rc<Checked>) -> ProgramCrossings {
    let mut out = ProgramCrossings::default();
    let Some(a) = c.analysis.get() else { return out };
    let p = Program::of(c);
    for (f, facts) in a.fns.iter().enumerate() {
        let function = c.decls.fns[f].name.text.clone();
        for site in &facts.sites {
            push_site(&p, &function, site, &mut out);
        }
    }
    out
}

fn push_site(p: &Program, function: &str, site: &CallSite, out: &mut ProgramCrossings) {
    match &site.kind {
        SiteKind::Rule { canon, name } => {
            let args: Vec<Given> = site.given.iter().map(|g| given(p, g)).collect();
            // the longest list the call gives a list the rule walks, when every one has a top
            let tops: Vec<Option<i128>> = site
                .given
                .iter()
                .filter_map(|g| match (&g.ty, &g.value) {
                    (Ty::List(_), Abs::List(count, _)) => Some(wire_ends(*count, crate::num::Rat::int(1)).1),
                    _ => None,
                })
                .collect();
            let max_len = if tops.is_empty() { None } else { tops.iter().copied().collect::<Option<Vec<i128>>>().and_then(|ts| ts.into_iter().max()) };
            out.rules.push(ProgramRuleCall {
                at: at(site.span),
                function: function.to_string(),
                rule: p.file(canon),
                name: name.clone(),
                args,
                relations: site.relations.iter().map(|(a, r, b)| (a.clone(), r.shown().to_string(), b.clone())).collect(),
                max_len,
            });
        }
        SiteKind::Date { canon, name, date } => {
            let Some(facts) = p.dates.get(canon) else { return };
            let day = site.given.iter().find(|g| facts.inputs.iter().any(|i| i.name == g.to && i.kind == ritsu_ports::DateKind::Date));
            if let Some(g) = day {
                out.dates.push(ProgramDateCall { at: at(site.span), function: function.to_string(), file: p.file(canon), name: name.clone(), date: date.clone(), day: given(p, g) });
            }
        }
        SiteKind::Transfer { id, name } => {
            out.transfers.push(ProgramTransfer {
                at: at(site.span),
                function: function.to_string(),
                book: p.file(&id.book),
                transfer: id.transfer.clone(),
                op: id.op.clone(),
                name: name.clone(),
                amounts: site.given.iter().map(|g| given(p, g)).collect(),
            });
        }
    }
}

/// The values a place across the borders is taken to by the inputs the vectors solve for (DESIGN
/// 5.7, 9): a step past each end of what takes it, where kaname knows the ends — below 0 for an
/// amount, past the range of the date input of a koyomi date or of a rule.
fn targets(p: &Program, suite: &crate::suite::Suite, site: &CallSite, pl: &crate::ranges::Place) -> Vec<crate::num::Rat> {
    use crate::num::Rat;
    let ends = |lo: Option<i128>, hi: Option<i128>| -> Vec<Rat> { [lo.map(|v| Rat::int(v - 1)), hi.map(|v| Rat::int(v + 1))].into_iter().flatten().collect() };
    match (&pl.what, &site.kind) {
        (What::Amount { .. }, _) => vec![Rat::int(-1)],
        (What::DateDay { input, .. }, SiteKind::Date { canon, .. }) => {
            p.dates.get(canon).and_then(|f| f.inputs.iter().find(|i| i.name == *input)).map(|i| ends(Some(i.min as i128), Some(i.max as i128))).unwrap_or_default()
        }
        (What::RuleDate { input, .. }, SiteKind::Rule { canon, .. }) => match suite.rules.as_ref().and_then(|r| r.date_range(&p.file(canon), input).ok()) {
            Some((lo, hi)) => ends(lo.map(i128::from), hi.map(i128::from)),
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// What the vectors of the function `f` give each of its places across the borders, a run at a
/// time (DESIGN 5.7, 14.2): its vectors, and the inputs that take the value of a place that is a sum
/// of the function's parameters a step past what takes it.
fn seen_of(c: &Rc<Checked>, suite: &crate::suite::Suite, f: usize) -> Vec<Seen> {
    use crate::interp::Interp;
    use crate::vectors::{self, Vector};
    let Some(a) = c.analysis.get() else { return Vec::new() };
    let facts = &a.fns[f];
    if facts.sites.is_empty() {
        return Vec::new();
    }
    let p = Program::of(c);
    let (mut inputs, _) = vectors::choose_with(&p, c, f, &facts.boundaries);
    let base = vectors::base_input(&p, c, f);
    for site in &facts.sites {
        for pl in facts.places.iter().filter(|pl| pl.what.across().is_some()) {
            let here = site.given.iter().any(|g| g.expr == pl.expr) || pl.expr == site.expr;
            let Some(lin) = pl.lin.as_ref().filter(|_| here) else { continue };
            for t in targets(&p, suite, site, pl) {
                for (input, _) in vectors::reach(&p, c, f, &base, lin, t) {
                    inputs.push(Vector { input, why: None });
                }
            }
        }
    }
    let info = &c.decls.fns[f];
    let mut out: Vec<Seen> = Vec::new();
    let mut tried = std::collections::BTreeSet::new();
    for v in inputs {
        if !tried.insert(format!("{:?}", v.input)) {
            continue;
        }
        let mut it = Interp::new(&p, suite);
        if it.check_entry(c, f, &v.input).is_err() {
            continue;
        }
        it.given = Some(Vec::new());
        let _ = it.call(c, f, v.input.clone(), 0);
        let records: Vec<_> = it.given.take().unwrap_or_default().into_iter().filter(|(canon, _, _)| *canon == c.canon).collect();
        if records.is_empty() {
            continue;
        }
        let call = format!("{}({})", info.name.text, info.params.iter().zip(&v.input).map(|(prm, x)| format!("{}: {}", prm.name.text, p.show(c, x, &prm.slot.ty))).collect::<Vec<_>>().join(", "));
        let input = ritsu_base::json::Json::obj(info.params.iter().zip(&v.input).map(|(prm, x)| (prm.name.text.clone(), p.to_json(x, &prm.slot.ty)))).compact();
        for (_, expr, values) in records {
            let Some(site) = facts.sites.iter().find(|s| s.expr == expr) else { continue };
            let mut push = |at: At, given: ritsu_ports::Values| {
                if !out.iter().any(|s| s.at == at && s.given == given) {
                    out.push(Seen { at, call: call.clone(), input: input.clone(), given });
                }
            };
            let date = |t: &Ty| matches!(t, Ty::Date) || matches!(t, Ty::Opt(x) if **x == Ty::Date);
            match &site.kind {
                // a precondition is about the call; a day given to a rule's date input, about it alone
                SiteKind::Rule { .. } => {
                    push(at(site.span), values.clone());
                    for g in site.given.iter().filter(|g| date(&g.ty)) {
                        if let Some(v) = values.iter().find(|(n, _)| *n == g.to) {
                            push(at(g.span), vec![v.clone()]);
                        }
                    }
                }
                SiteKind::Date { .. } | SiteKind::Transfer { .. } => {
                    for g in &site.given {
                        if let Some(v) = values.iter().find(|(n, _)| *n == g.to) {
                            push(at(g.span), vec![v.clone()]);
                        }
                    }
                }
            }
        }
    }
    out
}

impl ritsu_ports::Programs for Engine {
    /// Every call of the module at `file` into rulec, koyomi and chobo (DESIGN 14.2), from its
    /// analysis of ranges: what kaname finds each value given can be, where it can come from, and the
    /// relations between a rule's inputs at the call. A module whose names or types do not pass has
    /// none, and its check's errors are what is said.
    fn crossings(&self, file: &Path, ports: &ritsu_ports::Ports) -> Result<ProgramCrossings, Vec<Said>> {
        Engine::analysed(file, ports).map(|c| crossings_of(&c))
    }

    /// What the vectors of `function` give each of its places across the borders (DESIGN 5.7):
    /// asked only where ritsu-cross cannot show a check with kaname's ranges.
    fn seen(&self, file: &Path, ports: &ritsu_ports::Ports, function: &str) -> Vec<Seen> {
        let Ok(c) = Engine::analysed(file, ports) else { return Vec::new() };
        let Some(f) = c.decls.fns.iter().position(|x| x.name.text == function) else { return Vec::new() };
        seen_of(&c, &ports.clone().into(), f)
    }
}

/// The module at `file` (from `root`), parsed: what `Items` and `References` read. The file need
/// only parse: what its names refer to is the check's to say.
fn parsed(root: &Path, file: &str) -> Result<(crate::ast::Module, String), Vec<Said>> {
    let disk = ritsu_base::paths::on_disk(root, file);
    let path = disk.to_string_lossy().to_string();
    let src = ritsu_base::fs::read_to_string(&disk).map_err(|e| vec![Said::unreadable(&path, &e.to_string())])?;
    let p = crate::parse::parse(&path, &src);
    if crate::diag::has_errors(&p.diags) {
        return Err(p.diags.iter().filter(|d| d.is_error()).map(Said::of).collect());
    }
    match p.module {
        Some(m) => Ok((m, src)),
        None => Err(p.diags.iter().map(Said::of).collect()),
    }
}

impl ritsu_ports::Items for Engine {
    /// Each function, record (and each of its fields) and enum (and each of its values) the module
    /// declares (DESIGN 14.6), by its name, as `kaname "pricing.kn" function quote` names it. A
    /// function's and a record's definition are their lines; a field's and an enum's, its line; an
    /// enum's value's, its name with its alias. Each definition is made as dandori's and sekisho's
    /// are (`ritsu_base::definition`): without comments, the spaces around them and the alignment.
    fn items(&self, root: &Path, file: &str) -> Result<Vec<ritsu_ports::Item>, Vec<Said>> {
        use ritsu_base::naming::{Name, Tool};
        use ritsu_ports::Item;
        let (m, src) = parsed(root, file)?;
        let lines: Vec<&str> = src.lines().collect();
        let naming = |kind: &str, name: &str| Name::file(Tool::Kaname, file).with(kind, name);
        let one = |naming: Name, (from, to): (usize, usize)| Item { naming, lines: (from, to), text: ritsu_base::definition::block(&lines, from, to) };
        let mut out = Vec::new();
        for e in &m.enums {
            out.push(one(naming("enum", &e.name.text), (e.line, e.line)));
            for v in &e.values {
                let written = match &v.name.alias {
                    Some((a, _)) => format!("{}({a})", v.name.text),
                    None => v.name.text.clone(),
                };
                out.push(Item { naming: naming("enum", &e.name.text).with("value", &v.name.text), lines: (e.line, e.line), text: written });
            }
        }
        for r in &m.records {
            out.push(one(naming("record", &r.name.text), (r.line, ritsu_base::definition::block_end(&lines, r.line))));
            for f in &r.fields {
                out.push(one(naming("record", &r.name.text).with("field", &f.name.text), (f.line, f.line)));
            }
        }
        for f in &m.fns {
            // the body, to the last line of its answer (a record or a list goes on over lines)
            let end = f.last.max(ritsu_base::definition::block_end(&lines, f.line));
            out.push(one(naming("function", &f.name.text), (f.line, end)));
        }
        out.sort_by_key(|i| i.lines.0);
        Ok(out)
    }
}

impl ritsu_ports::References for Engine {
    /// The files the module reads, a `use` line each (DESIGN 14.6): a rule (`use rule`), a dates file
    /// (`use dates`), a book (`use book`) and another module (`use module`), from the root. A file
    /// outside the root has no reference.
    fn references(&self, root: &Path, file: &str) -> Result<Vec<ritsu_ports::Reference>, Vec<Said>> {
        use crate::ast::UseKind;
        use ritsu_base::naming::{Name, Tool};
        let (m, _) = parsed(root, file)?;
        let dir = ritsu_base::paths::parent(file);
        let mut out = Vec::new();
        for u in &m.uses {
            let Ok(p) = ritsu_base::paths::join(&dir, &u.path) else { continue };
            let tool = match u.kind {
                UseKind::Rule => Tool::Rulec,
                UseKind::Dates => Tool::Koyomi,
                UseKind::Book => Tool::Chobo,
                UseKind::Module => Tool::Kaname,
            };
            out.push(ritsu_ports::Reference { line: u.line, target: Name::file(tool, p), how: format!("use {}", u.kind.word()) });
        }
        Ok(out)
    }
}
