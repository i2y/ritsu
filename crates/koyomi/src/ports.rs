//! ritsu's ports, as koyomi answers them (ritsu's DESIGN 3.2): [`Engine`] implements the port of
//! dates (`ritsu_ports::Dates`) and gives what a `.cal` holds and what it names outside itself
//! (`Items`, `References`). koyomi computes a date on every input of its range, so the set of
//! days a date comes to and the days from its input are known exactly; the port hands them over
//! as they are.

use crate::ast::{At, SourceKind};
use crate::calendar::Loader;
use crate::check::{self, Checked, Inputs, Options};
use crate::date::Day;
use crate::interp;
use crate::resolve::Model;
use ritsu_base::naming::{Name as Naming, Tool};
use ritsu_base::text::Text;
use ritsu_ports::{DateCalendar, DateFacts, DateFunction, DateInput, DateKind, DateValue, DaySet, DaySpan, Found, Item, Reference, Said};
use std::path::Path;

/// koyomi, as the ports reach it.
#[derive(Default)]
pub struct Engine;

/// The dates file, checked as `koyomi check` checks it; else what check says.
fn model(file: &Path) -> Result<Model, Vec<Said>> {
    let path = file.to_string_lossy().to_string();
    let out = check::check_file(&path, &Options::default(), &mut Loader::default()).map_err(|e| vec![Said::unreadable(&path, &e)])?;
    if out.has_errors() {
        return Err(out.diags.iter().filter(|d| d.is_error()).map(Said::of).collect());
    }
    match out.checked {
        Some(Checked::Dates(m, _)) => Ok(*m),
        _ => Err(vec![Said {
            code: String::new(),
            file: path.clone(),
            line: None,
            message: ritsu_base::tr!("`{path}` は日付のファイルではなく、カレンダーです", "`{path}` is a calendar, not a dates file"),
        }]),
    }
}

/// The index of a date by its name.
fn date_index(m: &Model, date: &str, file: &Path) -> Result<usize, Vec<Said>> {
    m.dates.iter().position(|d| d.name == date).ok_or_else(|| {
        let path = file.to_string_lossy().to_string();
        vec![Said { code: String::new(), file: path.clone(), line: None, message: ritsu_base::tr!("`{path}` に日付 `{date}` はありません", "`{path}` has no date `{date}`") }]
    })
}

/// Every input of the range, with what each date comes to for it; Err with the input and why,
/// at the first input where a date stops.
fn walk(m: &Model, mut each: impl FnMut(&[i64], &[Day])) -> Result<(), (Vec<i64>, Text)> {
    let mut out = vec![Day(0); m.dates.len()];
    let mut evs = Vec::new();
    let mut it = Inputs::new(m);
    while let Some(vals) = it.next_input() {
        evs.clear();
        if let Err(stop) = interp::run(m, vals, &mut out, &mut evs) {
            return Err((vals.to_vec(), interp::fail_text(&stop.fail)));
        }
        each(vals, &out);
    }
    Ok(())
}

/// What a date's walk cannot hand over: more inputs than the check walks, or an input where the
/// date stops.
fn undecided(m: &Model, why: Result<(), (Vec<i64>, Text)>) -> Option<Text> {
    match why {
        Ok(()) => None,
        Err((vals, t)) => {
            let at: Vec<String> = m.inputs.iter().zip(&vals).map(|(i, v)| format!("{}={}", i.name, i.show(*v))).collect();
            let at = at.join(", ");
            Some(ritsu_base::tr!("{at} で計算が止まります（{}）", "the computation stops at {at} ({})", t.ja; t.en))
        }
    }
}

fn over_budget(m: &Model) -> Option<Text> {
    let combos = m.combinations();
    let budget = Options::default().budget;
    (combos > budget as u128).then(|| ritsu_base::tr!("入力の組み合わせが {combos} 通りあり、koyomi が確かめる {budget} 通りを超えます", "the inputs come to {combos} combinations, more than the {budget} koyomi checks"))
}

impl Engine {
    /// `koyomi check` of each file, as `ritsu check` prints it (ritsu's DESIGN 8.3): every finding,
    /// as the command prints it and as its `--format json` prints it, then the line that says what
    /// was checked. One loader is shared by the files, as the command shares it. `files` are as
    /// the person gave them, from where the program runs; `root` is the project's.
    pub fn checked(&self, root: &Path, files: &[String], lang: ritsu_base::text::Lang) -> Vec<ritsu_ports::Checked> {
        use ritsu_ports::{Checked as Unit, Finding, Part, Verdict};
        let mut loader = Loader::default();
        files
            .iter()
            .map(|f| match check::check_file(f, &Options::default(), &mut loader) {
                Ok(o) => {
                    let mut parts: Vec<Part> = o.diags.iter().map(|d| Part::Finding(Finding::of(d, ritsu_base::paths::from_root(root, Path::new(&d.file)), lang))).collect();
                    let tail = check::render(&check::Outcome { path: o.path.clone(), diags: vec![], checked: None, ok: o.ok.clone() }, lang);
                    if !tail.is_empty() {
                        parts.push(Part::Text(tail));
                    }
                    Unit { label: f.clone(), parts, verdict: if o.has_errors() { Verdict::Fails } else { Verdict::Passes } }
                }
                Err(e) => {
                    let msg = ritsu_base::tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}");
                    let head = if lang == ritsu_base::text::Lang::Ja { "エラー" } else { "error" };
                    Unit::unchecked(f, format!("{head}: {}\n", msg.get(lang)))
                }
            })
            .collect()
    }
}

impl ritsu_ports::Dates for Engine {
    fn facts(&self, file: &Path) -> Result<DateFacts, Vec<Said>> {
        let m = model(file)?;
        Ok(DateFacts {
            name: m.file.name.text.clone(),
            alias: m.file.name.ascii().unwrap_or_default().to_string(),
            version: m.file.version.clone(),
            sha256: m.sha256.clone(),
            inputs: m.inputs.iter().map(|i| DateInput { name: i.name.clone(), alias: i.alias.clone(), kind: if i.ty == crate::ast::Ty::Date { DateKind::Date } else { DateKind::Int }, min: i.lo, max: i.hi }).collect(),
            functions: m
                .dates
                .iter()
                .map(|d| DateFunction {
                    name: d.name.clone(),
                    alias: d.alias.clone(),
                    params: d.params.iter().map(|k| m.inputs[*k].name.clone()).collect(),
                    at: d.at.map(|(at, _)| match at {
                        At::Time(t) => t,
                        At::EndOfDay => 24 * 60,
                    }),
                })
                .collect(),
            calendar: m.cal.as_ref().map(|c| DateCalendar { name: c.info.name.clone(), data: (c.data.0.0 as i64, c.data.1.0 as i64), offset: c.offset }),
            claims: m.claims.iter().map(|c| (c.name.clone(), c.text.clone())).collect(),
        })
    }

    fn values(&self, file: &Path, date: &str) -> Result<Found<DaySet>, Vec<Said>> {
        let m = model(file)?;
        let k = date_index(&m, date, file)?;
        if let Some(t) = over_budget(&m) {
            return Ok(Found::Undecided(t));
        }
        let mut set = DaySet::new();
        let r = walk(&m, |_, out| {
            set.insert(out[k].0 as i64);
        });
        Ok(match undecided(&m, r) {
            Some(t) => Found::Undecided(t),
            None => Found::Value(set),
        })
    }

    fn days(&self, file: &Path, date: &str) -> Result<Found<(i64, i64)>, Vec<Said>> {
        let m = model(file)?;
        let k = date_index(&m, date, file)?;
        if let Some(t) = over_budget(&m) {
            return Ok(Found::Undecided(t));
        }
        let di = m.date_input;
        let mut span: Option<(i64, i64)> = None;
        let r = walk(&m, |vals, out| {
            let d = out[k].0 as i64 - vals[di];
            span = Some(span.map_or((d, d), |(lo, hi)| (lo.min(d), hi.max(d))));
        });
        Ok(match (undecided(&m, r), span) {
            (Some(t), _) => Found::Undecided(t),
            (None, Some(s)) => Found::Value(s),
            (None, None) => Found::Undecided(ritsu_base::tr!("入力の範囲が空です", "the range of the inputs is empty")),
        })
    }

    fn span(&self, file: &Path, date: &str) -> Result<Found<DaySpan>, Vec<Said>> {
        let m = model(file)?;
        let k = date_index(&m, date, file)?;
        if let Some(t) = over_budget(&m) {
            return Ok(Found::Undecided(t));
        }
        let di = m.date_input;
        // the first input of the walk that comes to the fewest days, and the first that comes to the most
        let mut span: Option<DaySpan> = None;
        let r = walk(&m, |vals, out| {
            let d = out[k].0 as i64 - vals[di];
            let at = Some(vals[di]);
            match &mut span {
                None => span = Some(DaySpan { fewest: d, most: d, fewest_at: at, most_at: at }),
                Some(s) => {
                    if d < s.fewest {
                        (s.fewest, s.fewest_at) = (d, at);
                    }
                    if d > s.most {
                        (s.most, s.most_at) = (d, at);
                    }
                }
            }
        });
        Ok(match (undecided(&m, r), span) {
            (Some(t), _) => Found::Undecided(t),
            (None, Some(s)) => Found::Value(s),
            (None, None) => Found::Undecided(ritsu_base::tr!("入力の範囲が空です", "the range of the inputs is empty")),
        })
    }

    fn input_for(&self, file: &Path, date: &str, day: ritsu_ports::Day) -> Result<Option<Vec<(String, i64)>>, Vec<Said>> {
        let m = model(file)?;
        let k = date_index(&m, date, file)?;
        if over_budget(&m).is_some() {
            return Ok(None);
        }
        let mut first: Option<Vec<i64>> = None;
        let _ = walk(&m, |vals, out| {
            if first.is_none() && out[k].0 as i64 == day {
                first = Some(vals.to_vec());
            }
        });
        Ok(first.map(|vals| m.inputs.iter().zip(vals).map(|(i, v)| (i.name.clone(), v)).collect()))
    }

    fn eval(&self, file: &Path, inputs: &[(String, i64)]) -> Result<Vec<(String, DateValue)>, Vec<Said>> {
        let m = model(file)?;
        let path = file.to_string_lossy().to_string();
        let mut vals = Vec::new();
        for i in &m.inputs {
            let v = inputs.iter().find(|(n, _)| *n == i.name).map(|(_, v)| *v).ok_or_else(|| {
                vec![Said { code: String::new(), file: path.clone(), line: None, message: ritsu_base::tr!("入力 `{}` がありません", "the input `{}` is missing", i.name) }]
            })?;
            if v < i.lo || v > i.hi {
                return Err(vec![Said { code: String::new(), file: path.clone(), line: None, message: ritsu_base::tr!("入力 `{}` の {} は範囲の外です", "{} of the input `{}` is outside its range", i.name, i.show(v); i.show(v), i.name) }]);
            }
            vals.push(v);
        }
        let mut out = vec![Day(0); m.dates.len()];
        let mut evs = Vec::new();
        let stopped = interp::run(&m, &vals, &mut out, &mut evs).err();
        // the dates computed before the one that stopped have their days; that one says why, and
        // those after it were not computed
        let done: Vec<usize> = match &stopped {
            None => m.order.clone(),
            Some(s) => m.order.iter().copied().take_while(|k| *k != s.date).collect(),
        };
        Ok(m.dates
            .iter()
            .enumerate()
            .map(|(k, d)| {
                let v = if done.contains(&k) {
                    DateValue::Day(out[k].0 as i64)
                } else if stopped.as_ref().is_some_and(|s| s.date == k) {
                    DateValue::Stopped(interp::fail_text(&stopped.as_ref().unwrap().fail))
                } else {
                    DateValue::Stopped(ritsu_base::tr!("前の日付の計算が止まったので、計算していません", "not computed: a date before it stopped"))
                };
                (d.name.clone(), v)
            })
            .collect())
    }
}

impl ritsu_ports::Sources for Engine {
    /// The sources a `.cal` declares — the laws a dates file cites, the table of holidays a
    /// calendar reads — each with its pins, when the file passes `koyomi check`, which holds every
    /// copy to its pin. A dates file and a calendar answer alike.
    fn sources(&self, file: &Path) -> Result<Vec<ritsu_ports::Source>, Vec<Said>> {
        let path = file.to_string_lossy().to_string();
        let out = check::check_file(&path, &Options::default(), &mut Loader::default()).map_err(|e| vec![Said::unreadable(&path, &e)])?;
        if out.has_errors() || out.checked.is_none() {
            return Err(out.diags.iter().filter(|d| d.is_error()).map(Said::of).collect());
        }
        let src = ritsu_base::fs::read_to_string(file).map_err(|e| vec![Said::unreadable(&path, &e.to_string())])?;
        let p = crate::parse::parse(&path, &src);
        let f = p.file.ok_or_else(|| p.diags.iter().filter(|d| d.is_error()).map(Said::of).collect::<Vec<_>>())?;
        Ok(f.sources
            .iter()
            .map(|s| ritsu_ports::Source {
                name: s.name.clone(),
                line: s.span.line,
                kind: match &s.kind {
                    SourceKind::Law { id, asof, pins } => ritsu_ports::SourceKind::Law {
                        db: "egov".into(),
                        id: id.clone(),
                        asof: asof.to_string(),
                        pins: pins.iter().filter_map(|p| p.pin.as_ref().map(|h| (p.fragment.clone(), h.clone()))).collect(),
                    },
                    SourceKind::File { path, url, pin, .. } => ritsu_ports::SourceKind::File { path: path.clone(), url: url.clone(), pin: pin.clone() },
                },
            })
            .collect())
    }
}

/// A line of a `.cal` as its definition counts it: without its comment and the spaces around it.
fn plain(line: &str) -> String {
    let mut out = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => break,
            _ => {}
        }
        out.push(c);
    }
    out.trim().to_string()
}

fn parsed(root: &Path, file: &str) -> Result<crate::ast::File, Vec<Said>> {
    let disk = ritsu_base::paths::on_disk(root, file);
    let path = disk.to_string_lossy().to_string();
    let src = ritsu_base::fs::read_to_string(&disk).map_err(|e| vec![Said::unreadable(&path, &e.to_string())])?;
    let p = crate::parse::parse(&path, &src);
    match p.file {
        Some(f) if !p.diags.iter().any(|d| d.is_error()) => Ok(f),
        _ => Err(p.diags.iter().filter(|d| d.is_error()).map(Said::of).collect()),
    }
}

impl ritsu_ports::Items for Engine {
    /// Each input, date, claim and source a `.cal` writes. A date's definition is its block of
    /// lines — `date … =` and the operations under it, with `at` — and a claim's is its line;
    /// each line without its comment and the spaces around it.
    fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        let f = parsed(root, file)?;
        let lines: Vec<&str> = f.src.lines().collect();
        let text = |from: usize, to: usize| -> String { lines.get(from.saturating_sub(1)..to.min(lines.len())).unwrap_or_default().iter().map(|l| plain(l)).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n") };
        let naming = |kind: &str, name: &str| Naming::file(Tool::Koyomi, file).with(kind, name);
        let mut out = Vec::new();
        for i in &f.inputs {
            out.push(Item { naming: naming("input", &i.name.text), lines: (i.span.line, i.span.line), text: text(i.span.line, i.span.line) });
        }
        for d in &f.dates {
            let mut end = d.ops.iter().map(|o| o.span.line).fold(d.span.line, usize::max);
            if let Some((_, sp, _)) = &d.at {
                end = end.max(sp.line);
            }
            out.push(Item { naming: naming("date", &d.name.text), lines: (d.span.line, end), text: text(d.span.line, end) });
        }
        for c in &f.claims {
            out.push(Item { naming: naming("claim", &c.name), lines: (c.span.line, c.span.line), text: text(c.span.line, c.span.line) });
        }
        for s in &f.sources {
            let end = match &s.kind {
                SourceKind::Law { pins, .. } => pins.iter().map(|p| p.span.line).fold(s.span.line, usize::max),
                SourceKind::File { .. } => s.span.line,
            };
            out.push(Item { naming: naming("source", &s.name), lines: (s.span.line, end), text: text(s.span.line, end) });
        }
        out.sort_by_key(|i| i.lines.0);
        Ok(out)
    }
}

impl ritsu_ports::References for Engine {
    /// The calendar a dates file reads (`use calendar`), the calendar a calendar reads, and the
    /// file a `source` copies.
    fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>> {
        let f = parsed(root, file)?;
        let from_root = |written: &str| ritsu_base::paths::join(&ritsu_base::paths::parent(file), written).ok();
        let mut out = Vec::new();
        if let Some((p, sp)) = &f.use_calendar
            && let Some(p) = from_root(p)
        {
            out.push(Reference { line: sp.line, target: Naming::file(Tool::Koyomi, p), how: "use calendar".into() });
        }
        for s in &f.sources {
            if let SourceKind::File { path, .. } = &s.kind
                && let Some(p) = from_root(path)
            {
                out.push(Reference { line: s.span.line, target: Naming::file(Tool::File, p), how: "source".into() });
            }
        }
        out.sort_by_key(|r| r.line);
        Ok(out)
    }
}
